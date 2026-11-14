//! Monte Carlo comparison of all schemes on the same 16-bit payloads.

use crate::channel::{self, ChannelError};
use crate::hamming::{self, HammingError, Verdict};
use crate::parity::{self, Parity2dVerdict, ParityError, ParityVerdict};
use crate::utils::Xorshift64Star;
use serde::Serialize;
use std::fmt;

pub const PAYLOAD_BITS: usize = 16;
pub const MAX_TRIALS: usize = 200_000;

const GRID_ROWS: usize = 4;
const GRID_COLS: usize = 4;
// Codeword lengths of the two Hamming variants (see hamming.rs).
const HAMMING_LEN: usize = 7;
const EXTENDED_LEN: usize = 8;
const HAMMING_BLOCKS: usize = PAYLOAD_BITS / hamming::DATA_BITS;
const SCHEME_COUNT: usize = 5;
const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

const _: () = assert!(GRID_ROWS * GRID_COLS == PAYLOAD_BITS);

#[derive(Debug, Clone, PartialEq)]
pub enum CompareError {
    NoTrials,
    TooManyTrials { max: usize },
    NoErrorRates,
    Channel(ChannelError),
    Codec(String),
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTrials => write!(f, "number of trials must be at least 1"),
            Self::TooManyTrials { max } => write!(f, "too many trials (max {max})"),
            Self::NoErrorRates => write!(f, "at least one error rate is required"),
            Self::Channel(e) => write!(f, "channel: {e}"),
            Self::Codec(msg) => write!(f, "codec: {msg}"),
        }
    }
}

impl std::error::Error for CompareError {}

impl From<ChannelError> for CompareError {
    fn from(e: ChannelError) -> Self {
        Self::Channel(e)
    }
}
impl From<HammingError> for CompareError {
    fn from(e: HammingError) -> Self {
        Self::Codec(e.to_string())
    }
}
impl From<ParityError> for CompareError {
    fn from(e: ParityError) -> Self {
        Self::Codec(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    Uncoded,
    Parity,
    Parity2d,
    Hamming,
    ExtendedHamming,
}

impl Scheme {
    pub const ALL: [Scheme; SCHEME_COUNT] = [
        Scheme::Uncoded,
        Scheme::Parity,
        Scheme::Parity2d,
        Scheme::Hamming,
        Scheme::ExtendedHamming,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Scheme::Uncoded => "Uncoded",
            Scheme::Parity => "Parity bit",
            Scheme::Parity2d => "2D parity (4x4)",
            Scheme::Hamming => "Hamming (7,4)",
            Scheme::ExtendedHamming => "Extended Hamming (8,4)",
        }
    }

    /// Bits on the wire for one payload.
    pub fn sent_bits(self) -> usize {
        match self {
            Scheme::Uncoded => PAYLOAD_BITS,
            Scheme::Parity => PAYLOAD_BITS + 1,
            Scheme::Parity2d => (GRID_ROWS + 1) * (GRID_COLS + 1),
            Scheme::Hamming => HAMMING_BLOCKS * HAMMING_LEN,
            Scheme::ExtendedHamming => HAMMING_BLOCKS * EXTENDED_LEN,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub clean: u32,
    pub corrected: u32,
    pub detected: u32,
    pub silent: u32,
}

impl Counts {
    fn add(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Clean => self.clean += 1,
            Outcome::Corrected => self.corrected += 1,
            Outcome::Detected => self.detected += 1,
            Outcome::Silent => self.silent += 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SchemeStats {
    pub id: Scheme,
    pub name: &'static str,
    pub data_bits: usize,
    pub sent_bits: usize,
    pub code_rate: f64,
    pub counts: Counts,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SweepPoint {
    pub ber: f64,
    pub trials: usize,
    pub schemes: Vec<SchemeStats>,
}

#[derive(Debug, Clone, Copy)]
enum Outcome {
    Clean,
    Corrected,
    Detected,
    Silent,
}

/// What the receiver does with one decoded block.
#[derive(Debug, Clone, Copy)]
enum BlockResult {
    Detected,
    Delivered { correct: bool },
}

/// Payload-level outcome from its blocks: any detection discards the payload,
/// otherwise any wrong block makes it a silent error.
fn combine(any_flips: bool, blocks: &[BlockResult]) -> Outcome {
    let mut detected = false;
    let mut wrong = false;
    for block in blocks {
        match block {
            BlockResult::Detected => detected = true,
            BlockResult::Delivered { correct } => wrong |= !correct,
        }
    }
    if detected {
        Outcome::Detected
    } else if wrong {
        Outcome::Silent
    } else if any_flips {
        Outcome::Corrected
    } else {
        Outcome::Clean
    }
}

/// Precomputed `hamming_run` results for every (data nibble, flip pattern),
/// so the simulation reuses the exact same decoder without per-trial allocations.
struct HammingTable {
    len: usize,
    /// Indexed by `(nibble << len) | flip_mask`; bit `b` of the mask is position `b + 1`.
    results: Vec<BlockResult>,
}

impl HammingTable {
    fn build(extended: bool) -> Result<Self, CompareError> {
        let len = if extended { EXTENDED_LEN } else { HAMMING_LEN };
        let mut results = Vec::with_capacity(16 << len);
        for nibble in 0..16usize {
            let data: Vec<bool> = (0..hamming::DATA_BITS).map(|j| (nibble >> j) & 1 == 1).collect();
            for mask in 0..(1usize << len) {
                let flips: Vec<usize> =
                    (0..len).filter(|b| (mask >> b) & 1 == 1).map(|b| b + 1).collect();
                let trace = hamming::hamming_run(&data, &flips, extended)?;
                results.push(match trace.verdict {
                    Verdict::DoubleErrorDetected => BlockResult::Detected,
                    _ => BlockResult::Delivered { correct: trace.decoded_correctly },
                });
            }
        }
        Ok(Self { len, results })
    }
}

fn simulate(
    scheme: Scheme,
    payload: &[bool; PAYLOAD_BITS],
    flips: &[usize],
    tables: &[HammingTable; 2],
) -> Result<Outcome, CompareError> {
    let any_flips = !flips.is_empty();
    let outcome = match scheme {
        Scheme::Uncoded => combine(any_flips, &[BlockResult::Delivered { correct: !any_flips }]),
        Scheme::Parity => {
            let t = parity::parity_run(payload, flips)?;
            let block = match t.verdict {
                ParityVerdict::ErrorDetected => BlockResult::Detected,
                ParityVerdict::NoErrorDetected => BlockResult::Delivered {
                    correct: t.received[..PAYLOAD_BITS] == payload[..],
                },
            };
            combine(any_flips, &[block])
        }
        Scheme::Parity2d => {
            let t = parity::parity2d_run(payload, GRID_ROWS, GRID_COLS, flips)?;
            let block = match t.verdict {
                Parity2dVerdict::Uncorrectable => BlockResult::Detected,
                _ => BlockResult::Delivered { correct: t.decoded_correctly },
            };
            combine(any_flips, &[block])
        }
        Scheme::Hamming | Scheme::ExtendedHamming => {
            let table = &tables[usize::from(scheme == Scheme::ExtendedHamming)];
            let blocks: [BlockResult; HAMMING_BLOCKS] = std::array::from_fn(|c| {
                let nibble = (0..hamming::DATA_BITS).fold(0usize, |acc, j| {
                    acc | (usize::from(payload[c * hamming::DATA_BITS + j]) << j)
                });
                // The blocks are sent back to back, so the flips are positions in the whole stream.
                let mask = flips
                    .iter()
                    .filter(|&&p| p / table.len == c)
                    .fold(0usize, |acc, &p| acc | (1usize << (p % table.len)));
                table.results[(nibble << table.len) | mask]
            });
            combine(any_flips, &blocks)
        }
    };
    Ok(outcome)
}

/// Seeded generator with a short warm-up, since xorshift outputs are weak
/// right after seeding with a small number.
fn make_rng(seed: u64) -> Xorshift64Star {
    let mut rng = Xorshift64Star::new(seed);
    for _ in 0..16 {
        rng.next();
    }
    rng
}

fn run_point(
    ber: f64,
    burst_len: usize,
    trials: usize,
    seed: u64,
    tables: &[HammingTable; 2],
) -> Result<SweepPoint, CompareError> {
    // Every scheme sees the same payload sequence and its own channel stream.
    // The seeds don't depend on `ber`, so flips at a higher rate include those at a lower one.
    let mut payload_rng = make_rng(seed);
    let mut channel_rngs: Vec<Xorshift64Star> = (0..SCHEME_COUNT)
        .map(|i| make_rng(seed ^ GOLDEN.wrapping_mul(i as u64 + 1)))
        .collect();
    let mut counts = [Counts::default(); SCHEME_COUNT];

    for _ in 0..trials {
        let word = payload_rng.next() >> (64 - PAYLOAD_BITS);
        let payload: [bool; PAYLOAD_BITS] = std::array::from_fn(|j| (word >> j) & 1 == 1);
        for (i, &scheme) in Scheme::ALL.iter().enumerate() {
            let flips = channel::clustered_flips(
                scheme.sent_bits(),
                ber,
                burst_len,
                &mut channel_rngs[i],
            )?;
            counts[i].add(simulate(scheme, &payload, &flips, tables)?);
        }
    }

    let schemes = Scheme::ALL
        .iter()
        .zip(counts)
        .map(|(&scheme, c)| SchemeStats {
            id: scheme,
            name: scheme.name(),
            data_bits: PAYLOAD_BITS,
            sent_bits: scheme.sent_bits(),
            code_rate: PAYLOAD_BITS as f64 / scheme.sent_bits() as f64,
            counts: c,
        })
        .collect();
    Ok(SweepPoint { ber, trials, schemes })
}

/// Simulate `trials` payloads per scheme at each bit error rate in `bers`.
/// `burst_len = 1` means independent errors; larger values cluster them.
pub fn compare_sweep(
    bers: &[f64],
    burst_len: usize,
    trials: usize,
    seed: u64,
) -> Result<Vec<SweepPoint>, CompareError> {
    if trials == 0 {
        return Err(CompareError::NoTrials);
    }
    if trials > MAX_TRIALS {
        return Err(CompareError::TooManyTrials { max: MAX_TRIALS });
    }
    if bers.is_empty() {
        return Err(CompareError::NoErrorRates);
    }
    let tables = [HammingTable::build(false)?, HammingTable::build(true)?];
    bers.iter()
        .map(|&ber| run_point(ber, burst_len, trials, seed, &tables))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(ber: f64, burst: usize, trials: usize, seed: u64) -> SweepPoint {
        compare_sweep(&[ber], burst, trials, seed).unwrap().remove(0)
    }

    fn stats(p: &SweepPoint, s: Scheme) -> &SchemeStats {
        p.schemes.iter().find(|x| x.id == s).unwrap()
    }

    fn frac(count: u32, p: &SweepPoint) -> f64 {
        count as f64 / p.trials as f64
    }

    #[test]
    fn error_free_channel_is_always_clean() {
        let p = point(0.0, 1, 500, 1);
        for s in &p.schemes {
            assert_eq!(s.counts, Counts { clean: 500, ..Counts::default() });
        }
    }

    #[test]
    fn counts_add_up() {
        let p = point(0.05, 3, 2000, 5);
        for s in &p.schemes {
            let c = s.counts;
            assert_eq!(c.clean + c.corrected + c.detected + c.silent, 2000);
        }
    }

    #[test]
    fn same_seed_same_result() {
        let a = compare_sweep(&[0.01, 0.1], 2, 1000, 77).unwrap();
        let b = compare_sweep(&[0.01, 0.1], 2, 1000, 77).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn code_rates() {
        let p = point(0.0, 1, 1, 1);
        let sent = |s| stats(&p, s).sent_bits;
        assert_eq!(sent(Scheme::Uncoded), 16);
        assert_eq!(sent(Scheme::Parity), 17);
        assert_eq!(sent(Scheme::Parity2d), 25);
        assert_eq!(sent(Scheme::Hamming), 28);
        assert_eq!(sent(Scheme::ExtendedHamming), 32);
        assert!((stats(&p, Scheme::Hamming).code_rate - 16.0 / 28.0).abs() < 1e-12);
    }

    #[test]
    fn detection_only_schemes_never_correct() {
        let p = point(0.05, 1, 5000, 3);
        let u = stats(&p, Scheme::Uncoded).counts;
        assert_eq!((u.corrected, u.detected), (0, 0));
        assert_eq!(stats(&p, Scheme::Parity).counts.corrected, 0);
    }

    #[test]
    fn matches_theory_for_independent_errors() {
        // BER 0.02, 16-bit payload. Expected values from the binomial distribution:
        // uncoded silent 27.6%; parity clean 70.9%, detected 25.0%, silent 4.0%;
        // Hamming never detects and is silent 3.1%. Bounds are about 4 sigma wide.
        let p = point(0.02, 1, 20_000, 11);
        let u = stats(&p, Scheme::Uncoded).counts;
        assert!((0.26..0.29).contains(&frac(u.silent, &p)));
        let par = stats(&p, Scheme::Parity).counts;
        assert!((0.695..0.725).contains(&frac(par.clean, &p)));
        assert!((0.236..0.264).contains(&frac(par.detected, &p)));
        assert!((0.033..0.048).contains(&frac(par.silent, &p)));
        let h = stats(&p, Scheme::Hamming).counts;
        assert_eq!(h.detected, 0);
        assert!((0.025..0.038).contains(&frac(h.silent, &p)));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(compare_sweep(&[0.1], 1, 0, 1).unwrap_err(), CompareError::NoTrials);
        assert_eq!(
            compare_sweep(&[0.1], 1, MAX_TRIALS + 1, 1).unwrap_err(),
            CompareError::TooManyTrials { max: MAX_TRIALS }
        );
        assert_eq!(compare_sweep(&[], 1, 10, 1).unwrap_err(), CompareError::NoErrorRates);
        assert_eq!(
            compare_sweep(&[1.5], 1, 10, 1).unwrap_err(),
            CompareError::Channel(ChannelError::InvalidBer(1.5))
        );
    }
}
