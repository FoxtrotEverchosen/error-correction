use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ParityError {
    EmptyData,
    BadDimensions { rows: usize, cols: usize, got: usize },
    FlipOutOfRange { position: usize, len: usize },
    DuplicateFlip { position: usize },
}

impl fmt::Display for ParityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyData => write!(f, "data must not be empty"),
            Self::BadDimensions { rows, cols, got } => {
                write!(f, "expected {rows}x{cols} = {} data bits, got {got}", rows * cols)
            }
            Self::FlipOutOfRange { position, len } => {
                write!(f, "flip position {position} out of range 0..{len}")
            }
            Self::DuplicateFlip { position } => write!(f, "position {position} flipped more than once"),
        }
    }
}

impl std::error::Error for ParityError {}

fn xor_all(bits: &[bool]) -> bool {
    bits.iter().fold(false, |acc, &b| acc ^ b)
}

/// Validates 0-indexed flip positions and returns them sorted.
fn checked_flips(flips: &[usize], len: usize) -> Result<Vec<usize>, ParityError> {
    let mut seen = vec![false; len];
    for &p in flips {
        if p >= len {
            return Err(ParityError::FlipOutOfRange { position: p, len });
        }
        if seen[p] {
            return Err(ParityError::DuplicateFlip { position: p });
        }
        seen[p] = true;
    }
    let mut sorted = flips.to_vec();
    sorted.sort_unstable();
    Ok(sorted)
}

fn apply_flips(bits: &[bool], flips: &[usize]) -> Vec<bool> {
    let mut out = bits.to_vec();
    for &p in flips {
        out[p] = !out[p];
    }
    out
}

// ---------------------------------------------------------------- single parity

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParityVerdict {
    NoErrorDetected,
    ErrorDetected,
}

/// Even parity. All positions are 0-indexed; the parity bit is the last one.
#[derive(Debug, Clone, Serialize)]
pub struct ParityTrace {
    pub data: Vec<bool>,
    pub parity_bit: bool,
    /// data followed by the parity bit
    pub codeword: Vec<bool>,
    pub received: Vec<bool>,
    pub flipped: Vec<usize>,
    pub ones_received: usize,
    pub verdict: ParityVerdict,
    /// True if the data was corrupted but the check still passed.
    pub undetected_error: bool,
}

pub fn parity_run(data: &[bool], flips: &[usize]) -> Result<ParityTrace, ParityError> {
    if data.is_empty() {
        return Err(ParityError::EmptyData);
    }
    let len = data.len() + 1;
    let flipped = checked_flips(flips, len)?;

    let parity_bit = xor_all(data);
    let mut codeword = data.to_vec();
    codeword.push(parity_bit);

    let received = apply_flips(&codeword, &flipped);
    let ones_received = received.iter().filter(|&&b| b).count();
    let check_fails = ones_received % 2 == 1;
    let verdict = if check_fails {
        ParityVerdict::ErrorDetected
    } else {
        ParityVerdict::NoErrorDetected
    };
    let undetected_error = !check_fails && received != codeword;

    Ok(ParityTrace {
        data: data.to_vec(),
        parity_bit,
        codeword,
        received,
        flipped,
        ones_received,
        verdict,
        undetected_error,
    })
}

// ---------------------------------------------------------------- 2D parity

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Parity2dVerdict {
    NoErrorDetected,
    /// Exactly one row and one column failed; the bit at their intersection was flipped back.
    /// With 3+ errors this belief can be wrong (miscorrection).
    Corrected { row: usize, col: usize },
    /// Errors detected but they cannot be pinpointed.
    Uncorrectable,
}

/// Grids are stored flat, row-major, 0-indexed. The codeword grid is
/// (rows + 1) x (cols + 1): the extra column holds row parities, the extra row
/// holds column parities, and the corner is the parity of the parity bits.
#[derive(Debug, Clone, Serialize)]
pub struct Parity2dTrace {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<bool>,
    pub codeword: Vec<bool>,
    pub received: Vec<bool>,
    pub flipped: Vec<usize>,
    /// Indices (0..=rows) of rows whose parity check failed.
    pub failing_rows: Vec<usize>,
    /// Indices (0..=cols) of columns whose parity check failed.
    pub failing_cols: Vec<usize>,
    pub verdict: Parity2dVerdict,
    pub corrected: Vec<bool>,
    pub decoded: Vec<bool>,
    pub decoded_correctly: bool,
}

pub fn parity2d_run(
    data: &[bool],
    rows: usize,
    cols: usize,
    flips: &[usize],
) -> Result<Parity2dTrace, ParityError> {
    if rows == 0 || cols == 0 || data.len() != rows * cols {
        return Err(ParityError::BadDimensions { rows, cols, got: data.len() });
    }
    let (w, h) = (cols + 1, rows + 1);
    let flipped = checked_flips(flips, w * h)?;

    // --- Encode ---
    let mut codeword = vec![false; w * h];
    for r in 0..rows {
        for c in 0..cols {
            codeword[r * w + c] = data[r * cols + c];
        }
        codeword[r * w + cols] = xor_all(&codeword[r * w..r * w + cols]);
    }
    // Column pass also covers c == cols: the XOR of the row parities (the corner).
    for c in 0..w {
        codeword[rows * w + c] = (0..rows).fold(false, |acc, r| acc ^ codeword[r * w + c]);
    }

    // --- Channel ---
    let received = apply_flips(&codeword, &flipped);

    // --- Decode ---
    let failing_rows: Vec<usize> = (0..h)
        .filter(|&r| xor_all(&received[r * w..(r + 1) * w]))
        .collect();
    let failing_cols: Vec<usize> = (0..w)
        .filter(|&c| (0..h).fold(false, |acc, r| acc ^ received[r * w + c]))
        .collect();

    let verdict = match (failing_rows.as_slice(), failing_cols.as_slice()) {
        ([], []) => Parity2dVerdict::NoErrorDetected,
        ([r], [c]) => Parity2dVerdict::Corrected { row: *r, col: *c },
        _ => Parity2dVerdict::Uncorrectable,
    };

    let mut corrected = received.clone();
    if let Parity2dVerdict::Corrected { row, col } = verdict {
        corrected[row * w + col] = !corrected[row * w + col];
    }
    let decoded: Vec<bool> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .map(|(r, c)| corrected[r * w + c])
        .collect();
    let decoded_correctly = decoded == data;

    Ok(Parity2dTrace {
        rows,
        cols,
        data: data.to_vec(),
        codeword,
        received,
        flipped,
        failing_rows,
        failing_cols,
        verdict,
        corrected,
        decoded,
        decoded_correctly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(n: u32, len: usize) -> Vec<bool> {
        (0..len).map(|i| (n >> i) & 1 == 1).collect()
    }

    #[test]
    fn parity_known_values() {
        let t = parity_run(&[true, false, true], &[]).unwrap();
        assert!(!t.parity_bit); // two ones -> even already
        let t = parity_run(&[true, false, false], &[]).unwrap();
        assert!(t.parity_bit);
        assert_eq!(t.verdict, ParityVerdict::NoErrorDetected);
    }

    #[test]
    fn parity_detects_every_single_flip() {
        for n in 0..128 {
            for pos in 0..8 {
                let t = parity_run(&bits(n, 7), &[pos]).unwrap();
                assert_eq!(t.verdict, ParityVerdict::ErrorDetected);
                assert!(!t.undetected_error);
            }
        }
    }

    #[test]
    fn parity_misses_double_flips() {
        let t = parity_run(&bits(0b1011001, 7), &[1, 5]).unwrap();
        assert_eq!(t.verdict, ParityVerdict::NoErrorDetected);
        assert!(t.undetected_error);
    }

    #[test]
    fn parity_rejects_bad_input() {
        assert_eq!(parity_run(&[], &[]).unwrap_err(), ParityError::EmptyData);
        assert_eq!(
            parity_run(&[true; 7], &[8]).unwrap_err(),
            ParityError::FlipOutOfRange { position: 8, len: 8 }
        );
        assert_eq!(
            parity_run(&[true; 7], &[2, 2]).unwrap_err(),
            ParityError::DuplicateFlip { position: 2 }
        );
    }

    #[test]
    fn p2d_clean_roundtrip() {
        let t = parity2d_run(&bits(0b101101110011, 12), 3, 4, &[]).unwrap();
        assert_eq!(t.verdict, Parity2dVerdict::NoErrorDetected);
        assert!(t.decoded_correctly);
        assert!(t.failing_rows.is_empty() && t.failing_cols.is_empty());
    }

    #[test]
    fn p2d_every_single_flip_is_corrected() {
        let (rows, cols) = (3, 4);
        let w = cols + 1;
        for n in (0..(1u32 << 12)).step_by(37) {
            let data = bits(n, rows * cols);
            for pos in 0..(rows + 1) * w {
                let t = parity2d_run(&data, rows, cols, &[pos]).unwrap();
                assert_eq!(t.verdict, Parity2dVerdict::Corrected { row: pos / w, col: pos % w });
                assert!(t.decoded_correctly);
                assert_eq!(t.corrected, t.codeword);
            }
        }
    }

    #[test]
    fn p2d_two_flips_are_detected_not_corrected() {
        let data = bits(0b101101110011, 12);
        // Same row (data cells 0 and 1): two failing columns, no failing row.
        let t = parity2d_run(&data, 3, 4, &[0, 1]).unwrap();
        assert_eq!(t.verdict, Parity2dVerdict::Uncorrectable);
        // Different rows and columns: two failing rows and two failing columns.
        let t = parity2d_run(&data, 3, 4, &[0, 6]).unwrap();
        assert_eq!(t.verdict, Parity2dVerdict::Uncorrectable);
    }

    #[test]
    fn p2d_rectangle_of_four_is_undetected() {
        let data = bits(0b101101110011, 12);
        // Width is 5: cells (0,0), (0,1), (1,0), (1,1).
        let t = parity2d_run(&data, 3, 4, &[0, 1, 5, 6]).unwrap();
        assert_eq!(t.verdict, Parity2dVerdict::NoErrorDetected);
        assert!(!t.decoded_correctly);
    }

    #[test]
    fn p2d_three_corners_miscorrect() {
        let data = bits(0b101101110011, 12);
        // Three corners of a rectangle look like one error at the fourth, (1,1).
        let t = parity2d_run(&data, 3, 4, &[0, 1, 5]).unwrap();
        assert_eq!(t.verdict, Parity2dVerdict::Corrected { row: 1, col: 1 });
        assert!(!t.decoded_correctly);
    }

    #[test]
    fn p2d_rejects_bad_input() {
        assert_eq!(
            parity2d_run(&[true; 5], 2, 3, &[]).unwrap_err(),
            ParityError::BadDimensions { rows: 2, cols: 3, got: 5 }
        );
        assert_eq!(
            parity2d_run(&[true; 6], 2, 3, &[12]).unwrap_err(),
            ParityError::FlipOutOfRange { position: 12, len: 12 }
        );
    }
}
