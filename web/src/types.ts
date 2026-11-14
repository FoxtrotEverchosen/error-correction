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
