use serde::Serialize;
use std::fmt;

pub const MAX_WIDTH: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub enum CrcError {
    EmptyData,
    BadWidth { width: usize },
    PolyTooWide { poly: u64, width: usize },
    FlipOutOfRange { position: usize, len: usize },
    DuplicateFlip { position: usize },
}

impl fmt::Display for CrcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyData => write!(f, "data must not be empty"),
            Self::BadWidth { width } => {
                write!(f, "CRC width must be in 1..={MAX_WIDTH}, got {width}")
            }
            Self::PolyTooWide { poly, width } => {
                write!(f, "polynomial {poly:#x} does not fit in {width} bits")
            }
            Self::FlipOutOfRange { position, len } => {
                write!(f, "flip position {position} out of range 0..{len}")
            }
            Self::DuplicateFlip { position } => {
                write!(f, "position {position} flipped more than once")
            }
        }
    }
}

impl std::error::Error for CrcError {}

/// One quotient position of the long division.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DivStep {
    /// Index of the bit being cleared in this step.
    pub offset: usize,
    /// True if that bit was 1, so the generator was XORed in.
    pub xored: bool,
    /// The whole working register after this step. Bits 0..=offset are all 0.
    pub row: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CrcVerdict {
    /// Remainder was zero. (The frame may still be corrupted, see `undetected_error`.)
    NoErrorDetected,
    ErrorDetected,
}

/// All positions are 0-indexed over the frame: `data` bits, then the CRC bits.
#[derive(Debug, Clone, Serialize)]
pub struct CrcTrace {
    pub width: usize,
    /// width + 1 bits, leading 1 included (e.g. 1011 for x^3 + x + 1).
    pub generator: Vec<bool>,
    pub data: Vec<bool>,
    /// The remainder of (data followed by `width` zeros) divided by the generator.
    pub crc: Vec<bool>,
    /// data followed by crc
    pub codeword: Vec<bool>,
    pub encode_steps: Vec<DivStep>,
    pub received: Vec<bool>,
    pub flipped: Vec<usize>,
    pub check_steps: Vec<DivStep>,
    /// Remainder of the received frame; all zeros means the check passes.
    pub remainder: Vec<bool>,
    pub verdict: CrcVerdict,
    /// True if the frame was corrupted but the remainder is still zero.
    pub undetected_error: bool,
}

/// `poly` is the generator without its leading x^width term, e.g. 0b011 for
/// x^3 + x + 1 with width 3.
fn generator_bits(poly: u64, width: usize) -> Vec<bool> {
    let mut g = vec![true];
    g.extend((0..width).rev().map(|i| (poly >> i) & 1 == 1));
    g
}

/// Long division over GF(2), in place. One step per quotient position; the
/// remainder ends up in the last `generator.len() - 1` bits of `bits`.
fn divide(bits: &mut [bool], generator: &[bool], steps: usize) -> Vec<DivStep> {
    let mut trace = Vec::with_capacity(steps);
    for i in 0..steps {
        let xored = bits[i];
        if xored {
            for (j, &g) in generator.iter().enumerate() {
                bits[i + j] ^= g;
            }
        }
        trace.push(DivStep { offset: i, xored, row: bits.to_vec() });
    }
    trace
}

pub fn crc_run(
    data: &[bool],
    poly: u64,
    width: usize,
    flips: &[usize],
) -> Result<CrcTrace, CrcError> {
    if data.is_empty() {
        return Err(CrcError::EmptyData);
    }
    if width == 0 || width > MAX_WIDTH {
        return Err(CrcError::BadWidth { width });
    }
    if poly >> width != 0 {
        return Err(CrcError::PolyTooWide { poly, width });
    }
    let len = data.len() + width;
    let mut seen = vec![false; len];
    for &p in flips {
        if p >= len {
            return Err(CrcError::FlipOutOfRange { position: p, len });
        }
        if seen[p] {
            return Err(CrcError::DuplicateFlip { position: p });
        }
        seen[p] = true;
    }
    let mut flipped = flips.to_vec();
    flipped.sort_unstable();

    let generator = generator_bits(poly, width);

    // --- Encode: divide data followed by `width` zeros ---
    let mut work = data.to_vec();
    work.resize(len, false);
    let encode_steps = divide(&mut work, &generator, data.len());
    let crc = work[data.len()..].to_vec();
    let mut codeword = data.to_vec();
    codeword.extend_from_slice(&crc);

    // --- Channel ---
    let mut received = codeword.clone();
    for &p in &flipped {
        received[p] = !received[p];
    }

    // --- Check: divide the received frame, no padding ---
    let mut check = received.clone();
    let check_steps = divide(&mut check, &generator, data.len());
    let remainder = check[data.len()..].to_vec();
    let detected = remainder.iter().any(|&b| b);
    let verdict = if detected {
        CrcVerdict::ErrorDetected
    } else {
        CrcVerdict::NoErrorDetected
    };
    let undetected_error = !detected && received != codeword;

    Ok(CrcTrace {
        width,
        generator,
        data: data.to_vec(),
        crc,
        codeword,
        encode_steps,
        received,
        flipped,
        check_steps,
        remainder,
        verdict,
        undetected_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(s: &str) -> Vec<bool> {
        s.chars().map(|c| c == '1').collect()
    }

    fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
        bytes
            .iter()
            .flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1 == 1))
            .collect()
    }

    fn to_value(bits: &[bool]) -> u64 {
        bits.iter().fold(0, |acc, &b| (acc << 1) | u64::from(b))
    }

    #[test]
    fn wikipedia_example() {
        // Message 11010011101100, generator 1011 (x^3 + x + 1) -> CRC 100.
        let t = crc_run(&bits("11010011101100"), 0b011, 3, &[]).unwrap();
        assert_eq!(t.generator, bits("1011"));
        assert_eq!(t.crc, bits("100"));
        assert_eq!(t.codeword, bits("11010011101100100"));
        assert_eq!(t.verdict, CrcVerdict::NoErrorDetected);
        assert!(t.remainder.iter().all(|&b| !b));
        assert!(!t.undetected_error);
    }

    #[test]
    fn standard_check_values() {
        let msg = bytes_to_bits(b"123456789");
        // CRC-8 (poly 0x07, init 0, no reflection, no final XOR)
        assert_eq!(to_value(&crc_run(&msg, 0x07, 8, &[]).unwrap().crc), 0xF4);
        // CRC-16/XMODEM (poly 0x1021, init 0, no reflection, no final XOR)
        assert_eq!(to_value(&crc_run(&msg, 0x1021, 16, &[]).unwrap().crc), 0x31C3);
    }

    #[test]
    fn steps_have_expected_shape() {
        let data = bits("11010011101100");
        let t = crc_run(&data, 0b011, 3, &[]).unwrap();
        assert_eq!(t.encode_steps.len(), data.len());
        assert_eq!(t.check_steps.len(), data.len());
        let last = t.encode_steps.last().unwrap();
        assert_eq!(&last.row[data.len()..], &t.crc[..]);
        for s in &t.encode_steps {
            assert!(s.row[..=s.offset].iter().all(|&b| !b));
        }
    }

    #[test]
    fn clean_frames_always_pass() {
        for (width, poly) in [(3, 0b011), (4, 0x3), (8, 0x07), (16, 0x1021)] {
            for seed in 0u32..64 {
                let data: Vec<bool> = (0..20).map(|i| (seed.wrapping_mul(2654435761) >> i) & 1 == 1).collect();
                let t = crc_run(&data, poly, width, &[]).unwrap();
                assert_eq!(t.verdict, CrcVerdict::NoErrorDetected);
                assert!(t.remainder.iter().all(|&b| !b));
            }
        }
    }

    #[test]
    fn every_single_flip_is_detected() {
        let data = bits("11010011101100");
        for (width, poly) in [(3, 0b011), (4, 0x3), (8, 0x07), (16, 0x1021)] {
            for pos in 0..data.len() + width {
                let t = crc_run(&data, poly, width, &[pos]).unwrap();
                assert_eq!(t.verdict, CrcVerdict::ErrorDetected, "width={width} pos={pos}");
            }
        }
    }

    #[test]
    fn every_burst_up_to_width_is_detected() {
        // Any error confined to `width` consecutive bits is caught, whatever the pattern.
        let data = bits("11010011101100");
        let width = 3;
        let len = data.len() + width;
        for start in 0..=len - width {
            for pat in 1u32..(1u32 << width) {
                let flips: Vec<usize> = (0..width)
                    .filter(|j| (pat >> (width - 1 - j)) & 1 == 1)
                    .map(|j| start + j)
                    .collect();
                let t = crc_run(&data, 0b011, width, &flips).unwrap();
                assert_eq!(t.verdict, CrcVerdict::ErrorDetected, "start={start} pat={pat}");
            }
        }
    }

    #[test]
    fn some_double_errors_slip_through() {
        // x^3 + x + 1 divides x^7 + 1, so flips 7 positions apart are invisible.
        let t = crc_run(&bits("11010011101100"), 0b011, 3, &[0, 7]).unwrap();
        assert_eq!(t.verdict, CrcVerdict::NoErrorDetected);
        assert!(t.undetected_error);
    }

    #[test]
    fn rejects_bad_input() {
        let d = bits("1011");
        assert_eq!(crc_run(&[], 0b011, 3, &[]).unwrap_err(), CrcError::EmptyData);
        assert_eq!(crc_run(&d, 0, 0, &[]).unwrap_err(), CrcError::BadWidth { width: 0 });
        assert_eq!(crc_run(&d, 0, 33, &[]).unwrap_err(), CrcError::BadWidth { width: 33 });
        assert_eq!(
            crc_run(&d, 0b1000, 3, &[]).unwrap_err(),
            CrcError::PolyTooWide { poly: 8, width: 3 }
        );
        assert_eq!(
            crc_run(&d, 0b011, 3, &[7]).unwrap_err(),
            CrcError::FlipOutOfRange { position: 7, len: 7 }
        );
        assert_eq!(
            crc_run(&d, 0b011, 3, &[2, 2]).unwrap_err(),
            CrcError::DuplicateFlip { position: 2 }
        );
    }
}
