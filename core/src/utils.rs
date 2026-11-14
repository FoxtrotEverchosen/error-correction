// Simple Xorshift64* was chosen as quick source of reproducible PRNG.
// Based on work of Sebastiano Vigna [https://vigna.di.unimi.it/ftp/papers/xorshift.pdf]

pub struct Xorshift64Star{
    x: u64,
}

impl Xorshift64Star{
    pub fn new(seed: u64) -> Self{
        Self {
            // seed eq to 0 will alsway produce output of 0, so it must be avoided
            x: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed }
        }
    }

    pub fn next(&mut self) -> u64 {
        self.x ^= self.x >> 12;
        self.x ^= self.x << 25;
        self.x ^= self.x >> 27;
        self.x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}
