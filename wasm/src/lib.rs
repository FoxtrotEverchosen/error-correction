use wasm_bindgen::prelude::*;
use ecc_core::utils::Xorshift64Star;

#[wasm_bindgen]
pub fn hello(seed: u64) -> u64 {
    let mut rng = Xorshift64Star::new(seed);
    rng.next()
}

