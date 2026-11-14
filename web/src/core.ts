import init, { hamming_run, channel_bsc, channel_burst, parity_run, parity2d_run, compare_sweep, crc_run } from "ecc-wasm";
import type { HammingTrace, ParityTrace, Parity2dTrace, SweepPoint, CrcTrace } from "./types";

const toBytes = (data: boolean[]) => Uint8Array.from(data, (b) => (b ? 1 : 0));

export async function initCore(): Promise<void> {
  await init();
}

export function runHamming(
  data: boolean[],
  flips: number[],
  extended: boolean,
): HammingTrace {
  return hamming_run(
    toBytes(data),
    Uint32Array.from(flips),
    extended,
  ) as HammingTrace;
}

export function channelBsc(len: number, ber: number, seed: bigint): number[] {
  return Array.from(channel_bsc(len, ber, seed)); // 0-indexed
}

export function channelBurst(len: number, burstLen: number, seed: bigint): number[] {
  return Array.from(channel_burst(len, burstLen, seed)); // 0-indexed
}

export function runParity(data: boolean[], flips: number[]): ParityTrace {
  return parity_run(toBytes(data), Uint32Array.from(flips)) as ParityTrace;
}

export function runParity2d(data: boolean[], rows: number, cols: number, flips: number[]): Parity2dTrace {
  return parity2d_run(toBytes(data), rows, cols, Uint32Array.from(flips)) as Parity2dTrace;
}

export function runCompare(bers: number[], burstLen: number, trials: number, seed: bigint): SweepPoint[] {
  return compare_sweep(Float64Array.from(bers), burstLen, trials, seed) as SweepPoint[];
}

export function runCrc(data: boolean[], poly: number, width: number, flips: number[]): CrcTrace {
  return crc_run(toBytes(data), poly, width, Uint32Array.from(flips)) as CrcTrace;
}
