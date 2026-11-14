import { channelBsc, channelBurst } from "./core";

export type Tone = "ok" | "warn" | "bad";

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: { class?: string; text?: string; title?: string } = {},
  children: (Node | string)[] = [],
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (props.class) node.className = props.class;
  if (props.text !== undefined) node.textContent = props.text;
  if (props.title) node.title = props.title;
  node.append(...children);
  return node;
}

export function section(title: string, ...children: Node[]): HTMLElement {
  return el("section", {}, [el("h3", { text: title }), ...children]);
}

export function badge(text: string, tone: Tone): HTMLElement {
  return el("span", { class: `badge ${tone}`, text });
}

export interface BitCell {
  value: boolean;
  label?: string;
  kind?: "data" | "parity";
  flipped?: boolean;
  fail?: boolean;
  highlight?: boolean;
  onClick?: () => void;
}

function bitCell(c: BitCell): HTMLElement {
  const classes = ["bit"];
  if (c.kind) classes.push(c.kind);
  if (c.fail) classes.push("fail");
  if (c.flipped) classes.push("flipped");
  if (c.highlight) classes.push("highlight");
  if (c.onClick) classes.push("clickable");
  const cell = el(c.onClick ? "button" : "div", { class: classes.join(" ") }, [
    el("span", { class: "bit-value", text: c.value ? "1" : "0" }),
    el("span", { class: "bit-label", text: c.label ?? "" }),
  ]);
  if (c.onClick) cell.addEventListener("click", c.onClick);
  return cell;
}

export function bitRow(cells: BitCell[]): HTMLElement {
  const row = el("div", { class: "bits" });
  for (const c of cells) row.append(bitCell(c));
  return row;
}

export function bitGrid(rows: BitCell[][]): HTMLElement {
  const grid = el("div", { class: "grid" });
  grid.style.gridTemplateColumns = `repeat(${rows[0].length}, auto)`;
  for (const row of rows) for (const c of row) grid.append(bitCell(c));
  return grid;
}

// --- Shared noisy-channel controls ---

export interface ChannelState {
  ber: number;
  burstLen: number;
  seed: bigint;
}

export const newChannelState = (): ChannelState => ({ ber: 0.1, burstLen: 3, seed: 1n });

/** Calls `onApply` with 0-indexed flip positions for a stream of `len` bits. */
export function channelPanel(
  state: ChannelState,
  len: number,
  onApply: (flips: number[]) => void,
): HTMLElement {
  const num = (value: number, min: number, max: number, step: number, set: (n: number) => void) => {
    const input = el("input");
    input.type = "number";
    input.min = String(min);
    input.max = String(max);
    input.step = String(step);
    input.value = String(value);
    input.addEventListener("change", () => {
      const n = Number(input.value);
      if (Number.isFinite(n)) set(Math.min(max, Math.max(min, n)));
    });
    return input;
  };
  const freshSeed = () => {
    state.seed = BigInt(Math.floor(Math.random() * 2 ** 32)) + 1n;
    return state.seed;
  };

  const random = el("button", { text: "Random errors" });
  random.addEventListener("click", () => onApply(channelBsc(len, state.ber, freshSeed())));
  const burst = el("button", { text: "Burst" });
  burst.addEventListener("click", () =>
    onApply(channelBurst(len, Math.min(state.burstLen, len), freshSeed())),
  );

  return section(
    "Noisy channel (seeded)",
    el("label", { class: "option" }, [
      "Bit error rate: ",
      num(state.ber, 0, 1, 0.01, (n) => (state.ber = n)),
      " ",
      random,
    ]),
    el("label", { class: "option" }, [
      "Burst length: ",
      num(state.burstLen, 1, len, 1, (n) => (state.burstLen = Math.round(n))),
      " ",
      burst,
    ]),
    el("p", { class: "note", text: `Last seed: ${state.seed}` }),
  );
}
