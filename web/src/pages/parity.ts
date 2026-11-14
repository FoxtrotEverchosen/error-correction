import { runParity } from "../core";
import type { ParityTrace } from "../types";
import { badge, bitRow, channelPanel, el, newChannelState, section, type Tone } from "../components";
import type { Page } from "./page";

const DATA_LEN = 7;

function describe(t: ParityTrace): { text: string; tone: Tone } {
  if (t.verdict.kind === "error_detected") {
    return { text: "Error detected (odd number of 1s). The receiver can't tell which bit is wrong.", tone: "warn" };
  }
  return t.undetected_error
    ? { text: "No error detected, but the data was corrupted (an even number of bits flipped)", tone: "bad" }
    : { text: "No error detected", tone: "ok" };
}

export const parityPage: Page = {
  id: "parity",
  title: "Parity bit",
  group: "Detection",
  mount(container) {
    const data = [true, false, true, true, false, false, true];
    const flips = new Set<number>(); // 0-indexed positions in the 8-bit frame
    const channelState = newChannelState();

    const label = (i: number) => (i < DATA_LEN ? `d${i + 1}` : "p");
    const kind = (i: number) => (i < DATA_LEN ? "data" : "parity");

    const draw = () => {
      let t: ParityTrace;
      try {
        t = runParity(data, [...flips]);
      } catch (e) {
        container.replaceChildren(el("p", { class: "error", text: String(e) }));
        return;
      }

      const controls = section(
        "1. Data (click a bit to change it)",
        bitRow(data.map((v, i) => ({
          value: v,
          label: label(i),
          onClick: () => { data[i] = !data[i]; draw(); },
        }))),
      );
      const clear = el("button", { text: "Clear errors" });
      clear.addEventListener("click", () => { flips.clear(); draw(); });
      controls.append(clear);

      const channel = channelPanel(channelState, DATA_LEN + 1, (zeroIndexed) => {
        flips.clear();
        zeroIndexed.forEach((i) => flips.add(i));
        draw();
      });

      const sent = bitRow(t.codeword.map((v, i) => ({ value: v, label: label(i), kind: kind(i) })));
      const received = bitRow(t.received.map((v, i) => ({
        value: v,
        label: label(i),
        kind: kind(i),
        flipped: t.flipped.includes(i),
        onClick: () => {
          if (flips.has(i)) flips.delete(i); else flips.add(i);
          draw();
        },
      })));

      const even = t.ones_received % 2 === 0;
      const { text, tone } = describe(t);

      container.replaceChildren(
        el("h2", { text: "Single parity bit (even parity)" }),
        el("p", { class: "note", text: "The sender appends one bit so the total number of 1s is even. The receiver only checks that." }),
        controls,
        channel,
        section("2. Sent frame (blue = data, orange = parity)", sent),
        section("3. Received (click a bit to flip it)", received),
        section(
          "4. Check",
          el("p", { text: `Number of 1s in the received frame: ${t.ones_received} (${even ? "even, check passes" : "odd, check fails"})` }),
          el("p", {}, [badge(text, tone)]),
        ),
        el("p", { class: "note", text: "Try flipping two bits: the count stays even and the error goes unnoticed." }),
      );
    };

    draw();
  },
};
