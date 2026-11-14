import { runRepetition } from "../core";
import type { RepetitionTrace } from "../types";
import {
  badge, bitGrid, bitRow, channelPanel, el, newChannelState, section, type BitCell, type Tone,
} from "../components";
import type { Page } from "./page";

const COPY_OPTIONS = [3, 5, 7];
const bit = (b: boolean) => (b ? "1" : "0");
const bits = (b: boolean[]) => b.map(bit).join("");

function describe(t: RepetitionTrace): { text: string; tone: Tone } {
  const wrong = t.groups.filter((g) => !g.correct).length;
  if (wrong > 0) {
    return {
      text: `Wrong data delivered: more than half of the copies were flipped for ${wrong} bit${wrong > 1 ? "s" : ""}, so the vote went the wrong way and the receiver can't tell`,
      tone: "bad",
    };
  }
  if (t.verdict.kind === "corrected") {
    const n = t.verdict.groups;
    return {
      text: `${n} bit${n > 1 ? "s" : ""} had disagreeing copies; the majority vote recovered the data`,
      tone: "ok",
    };
  }
  return { text: "All copies agree: no error detected", tone: "ok" };
}

export const repetitionPage: Page = {
  id: "repetition",
  title: "Repetition code",
  group: "Correction",
  mount(container) {
    const data = [true, false, true, true];
    let copies = 3;
    const flips = new Set<number>(); // 0-indexed over the whole codeword
    const channelState = newChannelState();

    const draw = () => {
      let t: RepetitionTrace;
      try {
        t = runRepetition(data, copies, [...flips]);
      } catch (e) {
        container.replaceChildren(el("p", { class: "error", text: String(e) }));
        return;
      }
      const tolerated = (copies - 1) / 2;

      // --- Controls ---
      const select = el("select");
      for (const n of COPY_OPTIONS) {
        const opt = el("option", { text: `${n} copies` });
        opt.value = String(n);
        opt.selected = n === copies;
        select.append(opt);
      }
      select.addEventListener("change", () => {
        copies = Number(select.value);
        flips.clear();
        draw();
      });
      const clear = el("button", { text: "Clear errors" });
      clear.addEventListener("click", () => { flips.clear(); draw(); });

      const controls = section(
        "1. Data (click a bit to change it)",
        bitRow(data.map((v, i) => ({
          value: v,
          label: `d${i + 1}`,
          onClick: () => { data[i] = !data[i]; draw(); },
        }))),
        el("label", { class: "option" }, [
          "Send each bit: ", select,
          ` (code rate 1/${copies}; up to ${tolerated} flipped ${tolerated === 1 ? "copy" : "copies"} per bit get outvoted)`,
        ]),
      );

      const channel = channelPanel(channelState, t.codeword.length, (zeroIndexed) => {
        flips.clear();
        zeroIndexed.forEach((i) => flips.add(i));
        draw();
      });

      // --- Grids: one row per data bit, one column per copy ---
      const copyKind = (j: number) => (j === 0 ? "data" : "parity");
      const sent = bitGrid(
        t.data.map((_, g) =>
          Array.from({ length: copies }, (_, j): BitCell => ({
            value: t.codeword[g * copies + j],
            label: `#${j + 1}`,
            kind: copyKind(j),
          })),
        ),
      );
      const received = bitGrid(
        t.groups.map((grp, g): BitCell[] => {
          const cells = Array.from({ length: copies }, (_, j): BitCell => {
            const i = g * copies + j;
            return {
              value: t.received[i],
              label: `#${j + 1}`,
              kind: copyKind(j),
              flipped: t.flipped.includes(i),
              fail: !grp.unanimous,
              onClick: () => {
                if (flips.has(i)) flips.delete(i); else flips.add(i);
                draw();
              },
            };
          });
          const vote: BitCell = {
            value: grp.vote,
            label: "vote",
            kind: "data",
            flipped: !grp.correct,
            highlight: grp.correct && !grp.unanimous,
          };
          return [...cells, vote];
        }),
      );

      // --- Vote details and result ---
      const votes = el("ul", { class: "checks" });
      t.groups.forEach((g, i) => {
        const [text, tone]: [string, Tone] = !g.correct
          ? ["wrong", "bad"]
          : g.unanimous
            ? ["unanimous", "ok"]
            : ["outvoted", "ok"];
        votes.append(
          el("li", {}, [`d${i + 1}: ${g.ones} of ${copies} copies are 1 \u2192 vote ${bit(g.vote)} `, badge(text, tone)]),
        );
      });
      const { text, tone } = describe(t);

      container.replaceChildren(
        el("h2", { text: "Repetition code" }),
        el("p", { class: "note", text: "The simplest error-correcting code: send every bit several times and let the receiver take a majority vote. Each row below is one data bit and its copies." }),
        controls,
        channel,
        section("2. Sent (blue = the data bit, orange = its extra copies)", sent),
        section("3. Received (click a copy to flip it; the last column is the vote, tinted rows have disagreeing copies)", received, clear),
        section(
          "4. Majority vote and result",
          votes,
          el("p", {}, [badge(text, tone)]),
          el("p", { text: `Original data: ${bits(t.data)}   Decoded data: ${bits(t.decoded)}` }),
        ),
        el("p", { class: "note", text: `A bit survives as long as fewer than half of its copies are flipped. The price is a code rate of only 1/${copies}: two thirds of the traffic is overhead with 3 copies, yet Hamming (7,4) protects about as well at a rate of 4/7. This makes it a useful baseline on the comparison page.` }),
        el("p", { class: "note", text: "The layout matters: all copies of a bit travel next to each other. Try a burst of length 2: it's harmless when it straddles two bits (one flip each) and fatal when both flips land in the same bit's copies. With 3 copies, a burst of 3 always wins. Spreading the copies apart is the idea behind interleaving." }),
      );
    };

    draw();
  },
};
