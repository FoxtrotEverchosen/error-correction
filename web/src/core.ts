import init, { hamming_run, channel_bsc, channel_burst } from "ecc-wasm";
import type { HammingTrace } from "./types";

export async function initCore(): Promise<void> {
  await init();
}

export function runHamming(
  data: boolean[],
  flips: number[],
  extended: boolean,
): HammingTrace {
  return hamming_run(
    Uint8Array.from(data, (b) => (b ? 1 : 0)),
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
