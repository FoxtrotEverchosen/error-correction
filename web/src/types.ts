export type Verdict =
  | { kind: "no_error" }
  | { kind: "corrected"; position: number }
  | { kind: "double_error_detected" };

export interface HammingTrace {
  extended: boolean;
  data: boolean[];
  codeword: boolean[];
  parity_positions: number[];
  data_positions: number[];
  parity_groups: number[][];
  received: boolean[];
  flipped: number[];
  syndrome_bits: boolean[];
  syndrome: number;
  overall_parity_fail: boolean | null;
  verdict: Verdict;
  corrected: boolean[];
  decoded: boolean[];
  decoded_correctly: boolean;
}

export type ParityVerdict = { kind: "no_error_detected" } | { kind: "error_detected" };

export interface ParityTrace {
  data: boolean[];
  parity_bit: boolean;
  codeword: boolean[];
  received: boolean[];
  flipped: number[];
  ones_received: number;
  verdict: ParityVerdict;
  undetected_error: boolean;
}

export type Parity2dVerdict =
  | { kind: "no_error_detected" }
  | { kind: "corrected"; row: number; col: number }
  | { kind: "uncorrectable" };

export interface Parity2dTrace {
  rows: number;
  cols: number;
  data: boolean[];
  codeword: boolean[];
  received: boolean[];
  flipped: number[];
  failing_rows: number[];
  failing_cols: number[];
  verdict: Parity2dVerdict;
  corrected: boolean[];
  decoded: boolean[];
  decoded_correctly: boolean;
}

export type SchemeId = "uncoded" | "parity" | "parity2d" | "hamming" | "extended_hamming";

export interface Counts {
  clean: number;
  corrected: number;
  detected: number;
  silent: number;
}

export interface SchemeStats {
  id: SchemeId;
  name: string;
  data_bits: number;
  sent_bits: number;
  code_rate: number;
  counts: Counts;
}

export interface SweepPoint {
  ber: number;
  trials: number;
  schemes: SchemeStats[];
}
