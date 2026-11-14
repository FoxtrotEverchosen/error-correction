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
  highlight?: boolean;
  onClick?: () => void;
}

export function bitRow(cells: BitCell[]): HTMLElement {
  const row = el("div", { class: "bits" });
  for (const c of cells) {
    const classes = ["bit"];
    if (c.kind) classes.push(c.kind);
    if (c.flipped) classes.push("flipped");
    if (c.highlight) classes.push("highlight");
    if (c.onClick) classes.push("clickable");
    const cell = el(c.onClick ? "button" : "div", { class: classes.join(" ") }, [
      el("span", { class: "bit-value", text: c.value ? "1" : "0" }),
      el("span", { class: "bit-label", text: c.label ?? "" }),
    ]);
    if (c.onClick) cell.addEventListener("click", c.onClick);
    row.append(cell);
  }
  return row;
}
