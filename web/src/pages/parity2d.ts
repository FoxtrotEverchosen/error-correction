import { runParity2d } from "../core";
import type { Parity2dTrace } from "../types";
import {
  badge, bitGrid, channelPanel, el, newChannelState, section, type BitCell, type Tone,
} from "../components";
import type { Page } from "./page";

function describe(t: Parity2dTrace): { text: string; tone: Tone } {
  switch (t.verdict.kind) {
    case "no_error_detected":
      return t.decoded_correctly
        ? { text: "No error detected", tone: "ok" }
        : { text: "No error detected, but the data is wrong (the errors cancel out in every row and column)", tone: "bad" };
    case "corrected":
      return t.decoded_correctly
        ? { text: `Single error at row ${t.verdict.row + 1}, column ${t.verdict.col + 1} corrected`, tone: "ok" }
        : { text: `Miscorrected: "fixed" row ${t.verdict.row + 1}, column ${t.verdict.col + 1}, but the data is still wrong`, tone: "bad" };
    case "uncorrectable":
      return { text: "Errors detected but they can't be located, the frame would be discarded", tone: "warn" };
  }
}

const list = (xs: number[]) => (xs.length ? xs.map((x) => x + 1).join(", ") : "none");

export const parity2dPage: Page = {
  id: "parity2d",
  title: "2D parity",
  group: "Correction",
  mount(container) {
    let rows = 4;
    let cols = 4;
    let data: boolean[] = Array.from({ length: rows * cols }, (_, i) => i % 3 === 0);
    const flips = new Set<number>(); // flat, row-major, 0-indexed over the (rows+1) x (cols+1) grid
    const channelState = newChannelState();

    const resize = (r: number, c: number) => {
      const old = data, oldRows = rows, oldCols = cols;
      data = Array.from({ length: r * c }, (_, i) => {
        const rr = Math.floor(i / c), cc = i % c;
        return rr < oldRows && cc < oldCols ? old[rr * oldCols + cc] : false;
      });
      rows = r;
      cols = c;
      flips.clear();
      draw();
    };

    const sizeInput = (value: number, set: (n: number) => void) => {
      const input = el("input");
      input.type = "number";
      input.min = "2"; input.max = "8"; input.step = "1";
      input.value = String(value);
      input.addEventListener("change", () => {
        const n = Math.round(Number(input.value));
        if (Number.isFinite(n)) set(Math.min(8, Math.max(2, n)));
      });
      return input;
    };

    const draw = () => {
      let t: Parity2dTrace;
      try {
        t = runParity2d(data, rows, cols, [...flips]);
      } catch (e) {
        container.replaceChildren(el("p", { class: "error", text: String(e) }));
        return;
      }
      const w = cols + 1, h = rows + 1;
      const failRows = new Set(t.failing_rows);
      const failCols = new Set(t.failing_cols);

      const build = (
        values: boolean[],
        extra: (i: number, r: number, c: number) => Partial<BitCell> = () => ({}),
      ): BitCell[][] =>
        Array.from({ length: h }, (_, r) =>
          Array.from({ length: w }, (_, c): BitCell => {
            const i = r * w + c;
            return { value: values[i], kind: r < rows && c < cols ? "data" : "parity", ...extra(i, r, c) };
          }),
        );

      // --- Controls ---
      const dataGrid = bitGrid(
        Array.from({ length: rows }, (_, r) =>
          Array.from({ length: cols }, (_, c): BitCell => ({
            value: data[r * cols + c],
            kind: "data",
            onClick: () => { data[r * cols + c] = !data[r * cols + c]; draw(); },
          })),
        ),
      );
      const clear = el("button", { text: "Clear errors" });
      clear.addEventListener("click", () => { flips.clear(); draw(); });
      const controls = section(
        "1. Data (click a bit to change it)",
        el("label", { class: "option" }, [
          "Rows: ", sizeInput(rows, (n) => resize(n, cols)),
          "  Columns: ", sizeInput(cols, (n) => resize(rows, n)),
        ]),
        dataGrid,
        clear,
      );

      const channel = channelPanel(channelState, w * h, (zeroIndexed) => {
        flips.clear();
        zeroIndexed.forEach((i) => flips.add(i));
        draw();
      });

      // --- Sent / received ---
      const sent = bitGrid(build(t.codeword));
      const received = bitGrid(
        build(t.received, (i, r, c) => ({
          flipped: t.flipped.includes(i),
          fail: failRows.has(r) || failCols.has(c),
          onClick: () => {
            if (flips.has(i)) flips.delete(i); else flips.add(i);
            draw();
          },
        })),
      );

      // --- Result ---
      const { text, tone } = describe(t);
      const fixedIdx = t.verdict.kind === "corrected" ? t.verdict.row * w + t.verdict.col : -1;
      const correctedGrid = bitGrid(build(t.corrected, (i) => ({ highlight: i === fixedIdx })));

      const checks = el("div", {}, [
        el("p", { text: `Failing rows: ${list(t.failing_rows)}` }),
        el("p", { text: `Failing columns: ${list(t.failing_cols)}` }),
      ]);
      if (t.verdict.kind === "corrected") {
        checks.append(el("p", { text: `One row and one column fail, so the error is at their intersection: row ${t.verdict.row + 1}, column ${t.verdict.col + 1}.` }));
      }

      const result = section("5. Result", el("p", {}, [badge(text, tone)]), correctedGrid);
      if (t.verdict.kind === "uncorrectable") {
        result.append(el("p", { class: "note", text: "The decoded data is not trusted here. A real receiver would discard the frame and ask for a retransmission." }));
      }

      container.replaceChildren(
        el("h2", { text: "2D parity (product code)" }),
        el("p", { class: "note", text: "Every row and column gets a parity bit, plus one in the corner. Row and column checks include the parity bits themselves." }),
        controls,
        channel,
        section("2. Sent grid (blue = data, orange = parity)", sent),
        section("3. Received (click a bit to flip it; tinted cells are in a failing row or column)", received),
        section("4. Parity checks", checks),
        result,
        el("p", { class: "note", text: "Try flipping the four corners of a rectangle: every row and column keeps even parity, so nothing is detected. Three corners of a rectangle get 'corrected' to the wrong bit." }),
      );
    };

    draw();
  },
};
