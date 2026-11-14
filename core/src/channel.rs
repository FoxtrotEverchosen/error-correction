use crate::utils::Xorshift64Star;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelError {
    InvalidBer(f64),
}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBer(p) => write!(f, "bit error rate must be in 0..=1, got {p}"),
        }
    }
}

impl std::error::Error for ChannelError {}

/// Uniform f64 in [0, 1), built from the top 53 bits of the generator output.
fn next_unit(rng: &mut Xorshift64Star) -> f64 {
    (rng.next() >> 11) as f64 / (1u64 << 53) as f64
}

/// Binary symmetric channel: each of `len` bits flips independently with
/// probability `ber`. Returns sorted, unique, 0-indexed positions.
pub fn bsc_flips(
    len: usize,
    ber: f64,
    rng: &mut Xorshift64Star,
) -> Result<Vec<usize>, ChannelError> {
    // Written as a range check so NaN is rejected too.
    if !(0.0..=1.0).contains(&ber) {
        return Err(ChannelError::InvalidBer(ber));
    }
    Ok((0..len).filter(|_| next_unit(rng) < ber).collect())
}

/// Burst channel: `burst_len` consecutive bits flip, starting at a random
/// position. The burst is clamped to `len`. Sorted, 0-indexed positions.
pub fn burst_flips(len: usize, burst_len: usize, rng: &mut Xorshift64Star) -> Vec<usize> {
    let burst_len = burst_len.min(len);
    if burst_len == 0 {
        return Vec::new();
    }
    let starts = len - burst_len + 1; // number of valid start positions
    let start = ((next_unit(rng) * starts as f64) as usize).min(starts - 1);
    (start..start + burst_len).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bsc_is_deterministic() {
        let a = bsc_flips(1000, 0.1, &mut Xorshift64Star::new(42)).unwrap();
        let b = bsc_flips(1000, 0.1, &mut Xorshift64Star::new(42)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn bsc_extremes() {
        let mut rng = Xorshift64Star::new(1);
        assert!(bsc_flips(100, 0.0, &mut rng).unwrap().is_empty());
        assert_eq!(bsc_flips(100, 1.0, &mut rng).unwrap(), (0..100).collect::<Vec<_>>());
    }

    #[test]
    fn bsc_rate_is_roughly_right() {
        // Fixed seed, so this is deterministic, not flaky.
        let n = bsc_flips(100_000, 0.1, &mut Xorshift64Star::new(7)).unwrap().len();
        assert!((9_500..10_500).contains(&n), "got {n} flips");
    }

    #[test]
    fn bsc_rejects_bad_ber() {
        let mut rng = Xorshift64Star::new(1);
        assert!(bsc_flips(10, -0.1, &mut rng).is_err());
        assert!(bsc_flips(10, 1.5, &mut rng).is_err());
        assert!(bsc_flips(10, f64::NAN, &mut rng).is_err());
    }

    #[test]
    fn burst_is_contiguous_and_in_range() {
        for seed in 1..200 {
            let mut rng = Xorshift64Star::new(seed);
            let flips = burst_flips(8, 3, &mut rng);
            assert_eq!(flips.len(), 3);
            assert!(flips.windows(2).all(|w| w[1] == w[0] + 1));
            assert!(*flips.last().unwrap() < 8);
        }
    }

    #[test]
    fn burst_edge_cases() {
        let mut rng = Xorshift64Star::new(5);
        assert!(burst_flips(8, 0, &mut rng).is_empty());
        assert!(burst_flips(0, 3, &mut rng).is_empty());
        assert_eq!(burst_flips(5, 99, &mut rng), vec![0, 1, 2, 3, 4]); // clamped
    }
}
