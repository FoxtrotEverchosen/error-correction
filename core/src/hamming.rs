use serde::Serialize;
use std::fmt;

pub const DATA_BITS: usize = 4;
const HAMMING_LEN: usize = 7;
const EXTENDED_LEN: usize = 8;

// Positions are 1-indexed everywhere in the public API (see trace docs).
const PARITY_POSITIONS: [usize; 3] = [1, 2, 4];
const DATA_POSITIONS: [usize; 4] = [3, 5, 6, 7];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    NoError,
    /// The decoder believes the bit at `position` was flipped and fixed it.
    /// With 3+ errors this belief can be wrong (miscorrection).
    Corrected { position: usize },
    /// Extended Hamming only: an even number of errors was detected,
    /// no correction attempted.
    DoubleErrorDetected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HammingError {
    WrongDataLength { expected: usize, got: usize },
    FlipOutOfRange { position: usize, max: usize },
    DuplicateFlip { position: usize },
}

impl fmt::Display for HammingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongDataLength { expected, got } => {
                write!(f, "expected {expected} data bits, got {got}")
            }
            Self::FlipOutOfRange { position, max } => {
                write!(f, "flip position {position} out of range 1..={max}")
            }
            Self::DuplicateFlip { position } => {
                write!(f, "position {position} flipped more than once")
            }
        }
    }
}

impl std::error::Error for HammingError {}

/// Everything the UI needs to render one encode -> channel -> decode run.
/// All positions are 1-indexed; `codeword[pos - 1]` is the bit at `pos`.
#[derive(Debug, Clone, Serialize)]
pub struct HammingTrace {
    pub extended: bool,
    pub data: Vec<bool>,
    pub codeword: Vec<bool>,
    /// Parity bit positions, e.g. [1, 2, 4] (plus 8 when extended).
    pub parity_positions: Vec<usize>,
    pub data_positions: Vec<usize>,
    /// parity_groups[i] = positions covered by parity_positions[i].
    pub parity_groups: Vec<Vec<usize>>,
    pub received: Vec<bool>,
    /// Sorted positions that were flipped in the channel.
    pub flipped: Vec<usize>,
    /// One entry per Hamming check (positions 1, 2, 4); true = check FAILED.
    pub syndrome_bits: Vec<bool>,
    /// Syndrome as a number: 0 = all checks passed, else the suspected position.
    pub syndrome: usize,
    /// Extended only: true if the overall parity check failed.
    pub overall_parity_fail: Option<bool>,
    pub verdict: Verdict,
    pub corrected: Vec<bool>,
    pub decoded: Vec<bool>,
    /// Whether `decoded == data`. False with a "Corrected" verdict means miscorrection.
    pub decoded_correctly: bool,
}

/// Positions covered by the parity bit at `parity_pos` (Hamming 7 layout).
fn parity_group(parity_pos: usize) -> Vec<usize> {
    (1..=HAMMING_LEN).filter(|pos| pos & parity_pos != 0).collect()
}

/// XOR of the bits at the given 1-indexed positions.
fn xor_at(bits: &[bool], positions: impl IntoIterator<Item = usize>) -> bool {
    positions.into_iter().fold(false, |acc, p| acc ^ bits[p - 1])
}

/// Encode `data` (4 bits), flip the given positions, then decode.
/// With `extended = true`, an 8th overall-parity bit is added (SECDED).
pub fn hamming_run(
    data: &[bool],
    flips: &[usize],
    extended: bool,
) -> Result<HammingTrace, HammingError> {
    if data.len() != DATA_BITS {
        return Err(HammingError::WrongDataLength {
            expected: DATA_BITS,
            got: data.len(),
        });
    }
    let len = if extended { EXTENDED_LEN } else { HAMMING_LEN };

    let mut seen = vec![false; len + 1];
    for &pos in flips {
        if pos < 1 || pos > len {
            return Err(HammingError::FlipOutOfRange { position: pos, max: len });
        }
        if seen[pos] {
            return Err(HammingError::DuplicateFlip { position: pos });
        }
        seen[pos] = true;
    }

    let groups: Vec<Vec<usize>> = PARITY_POSITIONS.iter().map(|&p| parity_group(p)).collect();

    // --- Encode ---
    let mut codeword = vec![false; len];
    for (&pos, &bit) in DATA_POSITIONS.iter().zip(data) {
        codeword[pos - 1] = bit;
    }
    for (&p, group) in PARITY_POSITIONS.iter().zip(&groups) {
        // Parity slots are still false, so XOR over the whole group is correct.
        codeword[p - 1] = xor_at(&codeword, group.iter().copied());
    }
    if extended {
        codeword[HAMMING_LEN] = xor_at(&codeword, 1..=HAMMING_LEN);
    }

    // --- Channel ---
    let mut received = codeword.clone();
    let mut flipped = flips.to_vec();
    flipped.sort_unstable();
    for &pos in &flipped {
        received[pos - 1] = !received[pos - 1];
    }

    // --- Decode ---
    let syndrome_bits: Vec<bool> = groups
        .iter()
        .map(|g| xor_at(&received, g.iter().copied()))
        .collect();
    let mut syndrome = 0;
    for (&p, &fail) in PARITY_POSITIONS.iter().zip(&syndrome_bits) {
        if fail {
            syndrome |= p;
        }
    }
    let overall_parity_fail = extended.then(|| xor_at(&received, 1..=EXTENDED_LEN));

    let verdict = match overall_parity_fail {
        None => {
            if syndrome == 0 {
                Verdict::NoError
            } else {
                Verdict::Corrected { position: syndrome }
            }
        }
        Some(overall_fail) => match (syndrome, overall_fail) {
            (0, false) => Verdict::NoError,
            // Only the overall parity bit itself was hit.
            (0, true) => Verdict::Corrected { position: EXTENDED_LEN },
            (s, true) => Verdict::Corrected { position: s },
            // Checks fail but overall parity is even => even number of errors.
            (_, false) => Verdict::DoubleErrorDetected,
        },
    };

    let mut corrected = received.clone();
    if let Verdict::Corrected { position } = verdict {
        corrected[position - 1] = !corrected[position - 1];
    }
    let decoded: Vec<bool> = DATA_POSITIONS.iter().map(|&p| corrected[p - 1]).collect();
    let decoded_correctly = decoded == data;

    let mut parity_positions = PARITY_POSITIONS.to_vec();
    let mut parity_groups = groups;
    if extended {
        parity_positions.push(EXTENDED_LEN);
        parity_groups.push((1..=EXTENDED_LEN).collect());
    }

    Ok(HammingTrace {
        extended,
        data: data.to_vec(),
        codeword,
        parity_positions,
        data_positions: DATA_POSITIONS.to_vec(),
        parity_groups,
        received,
        flipped,
        syndrome_bits,
        syndrome,
        overall_parity_fail,
        verdict,
        corrected,
        decoded,
        decoded_correctly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nibble(n: u8) -> Vec<bool> {
        (0..4).map(|i| (n >> i) & 1 == 1).collect()
    }

    #[test]
    fn known_codeword() {
        // Classic example: data 1011 -> codeword 0110011
        let t = hamming_run(&[true, false, true, true], &[], false).unwrap();
        assert_eq!(
            t.codeword,
            vec![false, true, true, false, false, true, true]
        );
        assert_eq!(t.verdict, Verdict::NoError);
    }

    #[test]
    fn no_errors_roundtrip() {
        for ext in [false, true] {
            for n in 0..16 {
                let t = hamming_run(&nibble(n), &[], ext).unwrap();
                assert_eq!(t.verdict, Verdict::NoError);
                assert!(t.decoded_correctly);
            }
        }
    }

    #[test]
    fn every_single_flip_is_corrected() {
        for ext in [false, true] {
            let len = if ext { 8 } else { 7 };
            for n in 0..16 {
                for pos in 1..=len {
                    let t = hamming_run(&nibble(n), &[pos], ext).unwrap();
                    assert_eq!(t.verdict, Verdict::Corrected { position: pos });
                    assert_eq!(t.syndrome_bits.len(), 3);
                    assert!(t.decoded_correctly, "n={n} pos={pos} ext={ext}");
                }
            }
        }
    }

    #[test]
    fn plain_hamming_miscorrects_double_errors() {
        for n in 0..16 {
            for a in 1..=7 {
                for b in (a + 1)..=7 {
                    let t = hamming_run(&nibble(n), &[a, b], false).unwrap();
                    assert!(matches!(t.verdict, Verdict::Corrected { .. }));
                    assert!(!t.decoded_correctly);
                }
            }
        }
    }

    #[test]
    fn extended_detects_every_double_error() {
        for n in 0..16 {
            for a in 1..=8 {
                for b in (a + 1)..=8 {
                    let t = hamming_run(&nibble(n), &[a, b], true).unwrap();
                    assert_eq!(t.verdict, Verdict::DoubleErrorDetected);
                }
            }
        }
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            hamming_run(&[true; 3], &[], false).unwrap_err(),
            HammingError::WrongDataLength { expected: 4, got: 3 }
        );
        assert_eq!(
            hamming_run(&nibble(0), &[8], false).unwrap_err(),
            HammingError::FlipOutOfRange { position: 8, max: 7 }
        );
        assert_eq!(
            hamming_run(&nibble(0), &[0], true).unwrap_err(),
            HammingError::FlipOutOfRange { position: 0, max: 8 }
        );
        assert_eq!(
            hamming_run(&nibble(0), &[3, 3], false).unwrap_err(),
            HammingError::DuplicateFlip { position: 3 }
        );
    }
}
