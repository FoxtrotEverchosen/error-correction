use wasm_bindgen::prelude::*;
use ecc_core::utils::Xorshift64Star;
use ecc_core::{hamming, channel, parity, compare};
use serde::Serialize;

/// Serialize into plain JS objects (not `Map`s), with `None` as `null`.
fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    Ok(value.serialize(&serializer)?)
}

/// `data`: 4 values, 0 or 1. `flips`: 1-indexed positions to flip.
#[wasm_bindgen]
pub fn hamming_run(data: Vec<u8>, flips: Vec<u32>, extended: bool) -> Result<JsValue, JsError> {
    let data: Vec<bool> = data.into_iter().map(|b| b != 0).collect();
    let flips: Vec<usize> = flips.into_iter().map(|p| p as usize).collect();
    let trace = hamming::hamming_run(&data, &flips, extended)?;
    to_js(&trace)
}

/// Random bit flips (0-indexed positions) for a stream of `len` bits.
#[wasm_bindgen]
pub fn channel_bsc(len: u32, ber: f64, seed: u64) -> Result<Vec<u32>, JsError> {
    let mut rng = Xorshift64Star::new(seed);
    let flips = channel::bsc_flips(len as usize, ber, &mut rng)?;
    Ok(flips.into_iter().map(|p| p as u32).collect())
}

/// One burst of `burst_len` consecutive flips (0-indexed positions).
#[wasm_bindgen]
pub fn channel_burst(len: u32, burst_len: u32, seed: u64) -> Vec<u32> {
    let mut rng = Xorshift64Star::new(seed);
    channel::burst_flips(len as usize, burst_len as usize, &mut rng)
        .into_iter()
        .map(|p| p as u32)
        .collect()
}

#[wasm_bindgen]
pub fn parity_run(data: Vec<u8>, flips: Vec<u32>) -> Result<JsValue, JsError> {
    let data: Vec<bool> = data.into_iter().map(|b| b != 0).collect();
    let flips: Vec<usize> = flips.into_iter().map(|p| p as usize).collect();
    to_js(&parity::parity_run(&data, &flips)?)
}

#[wasm_bindgen]
pub fn parity2d_run(data: Vec<u8>, rows: u32, cols: u32, flips: Vec<u32>) -> Result<JsValue, JsError> {
    let data: Vec<bool> = data.into_iter().map(|b| b != 0).collect();
    let flips: Vec<usize> = flips.into_iter().map(|p| p as usize).collect();
    to_js(&parity::parity2d_run(&data, rows as usize, cols as usize, &flips)?)
}

#[wasm_bindgen]
pub fn compare_sweep(bers: Vec<f64>, burst_len: u32, trials: u32, seed: u64) -> Result<JsValue, JsError> {
    let points = compare::compare_sweep(&bers, burst_len as usize, trials as usize, seed)?;
    to_js(&points)
}
