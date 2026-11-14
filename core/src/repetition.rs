use serde::Serialize;
use std::fmt;

pub const MIN_COPIES: usize = 3;
pub const MAX_COPIES: usize = 9;

#[derive(Debug, Clone, PartialEq)]
pub enum RepetitionError {
    EmptyData,
    BadCopies { copies: usize },
    FlipOutOfRange { position: usize, len: usize },
    DuplicateFlip { position: usize },
}

impl fmt::Display for RepetitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyData => write!(f, "data must not be empty"),
            Self::BadCopies { copies } => write!(
                f,
                "copies must be odd and in {MIN_COPIES}..={MAX_COPIES}, got {copies}"
            ),
            Self::FlipOutOfRange { position, len } => {
                write!(f, "flip position {position} out of range 0..{len}")
            }
            Self::DuplicateFlip { position } => {
                write!(f, "position {position} flipped more than once")
            }
        }
    }
}

impl std::error::Error for RepetitionError {}

/// The vote for one data bit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroupVote {
    /// Number of received copies that are 1.
    pub ones: usize,
    pub vote: bool,
    /// All copies agree (the receiver sees nothing wrong).
    pub unanimous: bool,
    /// Whether the vote equals the original bit (the receiver can't know this).
    pub correct: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepetitionVerdict {
    /// Every group was unanimous.
    NoErrorDetected,
    /// `groups` groups had disagreeing copies and the majority overruled the minority.
    /// With too many flips in a group this belief can be wrong.
    Corrected { groups: usize },
}

/// Positions are 0-indexed over the codeword. The copies of data bit `g` are
/// at positions `g * copies .. (g + 1) * copies`.
#[derive(Debug, Clone, Serialize)]
pub struct RepetitionTrace {
    pub copies: usize,
    pub data: Vec<bool>,
    pub codeword: Vec<bool>,
    pub received: Vec<bool>,
    pub flipped: Vec<usize>,
    pub groups: Vec<GroupVote>,
    pub verdict: RepetitionVerdict,
    pub decoded: Vec<bool>,
    pub decoded_correctly: bool,
}

pub fn repetition_run(
    data: &[bool],
    copies: usize,
    flips: &[usize],
) -> Result<RepetitionTrace, RepetitionError> {
    if data.is_empty() {
        return Err(RepetitionError::EmptyData);
    }
    if !(MIN_COPIES..=MAX_COPIES).contains(&copies) || copies % 2 == 0 {
        return Err(RepetitionError::BadCopies { copies });
    }
    let len = data.len() * copies;
    let mut seen = vec![false; len];
    for &p in flips {
        if p >= len {
            return Err(RepetitionError::FlipOutOfRange { position: p, len });
        }
        if seen[p] {
            return Err(RepetitionError::DuplicateFlip { position: p });
        }
        seen[p] = true;
    }
    let mut flipped = flips.to_vec();
    flipped.sort_unstable();

    // --- Encode ---
    let codeword: Vec<bool> = data
        .iter()
        .flat_map(|&b| std::iter::repeat(b).take(copies))
        .collect();

    // --- Channel ---
    let mut received = codeword.clone();
    for &p in &flipped {
        received[p] = !received[p];
    }

    // --- Decode: majority vote per group (copies is odd, so no ties) ---
    let groups: Vec<GroupVote> = received
        .chunks(copies)
        .zip(data)
        .map(|(chunk, &original)| {
            let ones = chunk.iter().filter(|&&b| b).count();
            let vote = ones * 2 > copies;
            GroupVote {
                ones,
                vote,
                unanimous: ones == 0 || ones == copies,
                correct: vote == original,
            }
        })
        .collect();

    let disagreeing = groups.iter().filter(|g| !g.unanimous).count();
    let verdict = if disagreeing == 0 {
        RepetitionVerdict::NoErrorDetected
    } else {
        RepetitionVerdict::Corrected { groups: disagreeing }
    };
    let decoded: Vec<bool> = groups.iter().map(|g| g.vote).collect();
    let decoded_correctly = decoded == data;

    Ok(RepetitionTrace {
        copies,
        data: data.to_vec(),
        codeword,
        received,
        flipped,
        groups,
        verdict,
        decoded,
        decoded_correctly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codeword() {
        let t = repetition_run(&[true, false], 3, &[]).unwrap();
        assert_eq!(t.codeword, vec![true, true, true, false, false, false]);
        assert_eq!(t.verdict, RepetitionVerdict::NoErrorDetected);
        assert!(t.decoded_correctly);
    }

    /// Two data bits, every possible flip pattern.
    fn check_exhaustive(copies: usize) {
        let tolerated = (copies - 1) / 2;
        for n in 0..4u8 {
            let data = vec![n & 1 == 1, n & 2 == 2];
            for mask in 0..(1usize << (2 * copies)) {
                let flips: Vec<usize> = (0..2 * copies).filter(|b| (mask >> b) & 1 == 1).collect();
                let t = repetition_run(&data, copies, &flips).unwrap();
                let per_group: Vec<usize> = (0..2)
                    .map(|g| flips.iter().filter(|&&p| p / copies == g).count())
                    .collect();
                let ok = per_group.iter().all(|&k| k <= tolerated);
                assert_eq!(t.decoded_correctly, ok, "copies={copies} mask={mask:b}");
                let disagreeing = per_group.iter().filter(|&&k| k > 0 && k < copies).count();
                let expected = if disagreeing == 0 {
                    RepetitionVerdict::NoErrorDetected
                } else {
                    RepetitionVerdict::Corrected { groups: disagreeing }
                };
                assert_eq!(t.verdict, expected, "copies={copies} mask={mask:b}");
            }
        }
    }

    #[test]
    fn exhaustive_three_copies() {
        check_exhaustive(3);
    }

    #[test]
    fn exhaustive_five_copies() {
        check_exhaustive(5);
    }

    #[test]
    fn flipping_every_copy_is_a_silent_error() {
        let t = repetition_run(&[true, false, true, true], 3, &[0, 1, 2]).unwrap();
        assert_eq!(t.verdict, RepetitionVerdict::NoErrorDetected);
        assert!(!t.decoded_correctly);
    }

    #[test]
    fn burst_survival_depends_on_layout() {
        let data = [true, false, true, true];
        // Positions 2 and 3 straddle two groups: one flip each, both outvoted.
        assert!(repetition_run(&data, 3, &[2, 3]).unwrap().decoded_correctly);
        // Positions 0 and 1 hit the same group twice.
        assert!(!repetition_run(&data, 3, &[0, 1]).unwrap().decoded_correctly);
    }

    #[test]
    fn burst_of_three_always_defeats_three_copies() {
        let data = [true, false, true, true];
        for start in 0..=(data.len() * 3 - 3) {
            let t = repetition_run(&data, 3, &[start, start + 1, start + 2]).unwrap();
            assert!(!t.decoded_correctly, "start={start}");
        }
    }

    #[test]
    fn rejects_bad_input() {
        let d = [true, false];
        assert_eq!(repetition_run(&[], 3, &[]).unwrap_err(), RepetitionError::EmptyData);
        for copies in [0, 1, 2, 4, 11] {
            assert_eq!(
                repetition_run(&d, copies, &[]).unwrap_err(),
                RepetitionError::BadCopies { copies }
            );
        }
        assert_eq!(
            repetition_run(&d, 3, &[6]).unwrap_err(),
            RepetitionError::FlipOutOfRange { position: 6, len: 6 }
        );
        assert_eq!(
            repetition_run(&d, 3, &[1, 1]).unwrap_err(),
            RepetitionError::DuplicateFlip { position: 1 }
        );
    }
}
