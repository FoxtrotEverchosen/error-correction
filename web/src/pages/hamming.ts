import { channelBsc, channelBurst, runHamming } from "../core";
import type { HammingTrace } from "../types";
import { badge, bitRow, el, section, channelPanel, newChannelState, type BitCell, type Tone } from "../components";
import type { Page } from "./page";

const bits = (b: boolean[]) => b.map((x) => (x ? "1" : "0")).join("");

function describe(t: HammingTrace): { text: string; tone: Tone } {
  switch (t.verdict.kind) {
    case "no_error":
      return t.decoded_correctly
        ? { text: "No error detected", tone: "ok" }
        : { text: "No error detected, but the data is wrong", tone: "bad" };
    case "corrected":
      return t.decoded_correctly
        ? { text: `Error at position ${t.verdict.position} corrected`, tone: "ok" }
        : { text: `Miscorrected: "fixed" position ${t.verdict.position}, data is still wrong`, tone: "bad" };
    case "double_error_detected":
      return { text: "Double error detected, cannot correct", tone: "warn" };
  }
}

export const hammingPage: Page = {
  id: "hamming",
  title: "Hamming (7,4)",
  group: "Correction",
  mount(container) {
    const data = [true, false, true, true];
    const flips = new Set<number>();
    let extended = false;
    const channelState = newChannelState();

    const draw = () => {
      let t: HammingTrace;
      try {
        t = runHamming(data, [...flips], extended);
      } catch (e) {
        container.replaceChildren(el("p", { class: "error", text: String(e) }));
        return;
      }

      // --- Controls ---
      const dataCells: BitCell[] = data.map((v, i) => ({
        value: v,
        label: `d${i + 1}`,
        onClick: () => { data[i] = !data[i]; draw(); },
      }));
      const box = el("input");
      box.type = "checkbox";
      box.checked = extended;
      box.addEventListener("change", () => {
        extended = box.checked;
        if (!extended) flips.delete(8);
        draw();
      });
      const reset = el("button", { text: "Clear errors" });
      reset.addEventListener("click", () => { flips.clear(); draw(); });
      const controls = section(
        "1. Data (click a bit to change it)",
        bitRow(dataCells),
        el("label", { class: "option" }, [box, " Extended Hamming (SECDED, adds position 8)"]),
        reset,
      );

      const channelControls = channelPanel(channelState, extended ? 8 : 7, (zeroIndexed) => {
        flips.clear();
        zeroIndexed.forEach((i) => flips.add(i + 1)); // Hamming positions are 1-indexed
        draw();
      });

      // --- Sent / received ---
      const sent = bitRow(
        t.codeword.map((v, i) => ({
          value: v,
          label: String(i + 1),
          kind: t.parity_positions.includes(i + 1) ? "parity" : "data",
        })),
      );
      const received = bitRow(
        t.received.map((v, i) => ({
          value: v,
          label: String(i + 1),
          kind: t.parity_positions.includes(i + 1) ? "parity" : "data",
          flipped: t.flipped.includes(i + 1),
          onClick: () => {
            if (flips.has(i + 1)) flips.delete(i + 1);
            else flips.add(i + 1);
            draw();
          },
        })),
      );

      // --- Parity checks ---
      const checks = el("ul", { class: "checks" });
      t.parity_groups.slice(0, 3).forEach((group, i) => {
        const fail = t.syndrome_bits[i];
        checks.append(
          el("li", {}, [
            `p${t.parity_positions[i]} covers ${group.join(", ")}: `,
            badge(fail ? "fail" : "ok", fail ? "bad" : "ok"),
          ]),
        );
      });
      if (t.overall_parity_fail !== null) {
        checks.append(
          el("li", {}, [
            "overall parity covers all 8: ",
            badge(t.overall_parity_fail ? "fail" : "ok", t.overall_parity_fail ? "bad" : "ok"),
          ]),
        );
      }
      // syndrome_bits = [p1, p2, p4]; written as binary the p4 check is the leftmost digit.
      const synBinary = [...t.syndrome_bits].reverse().map((b) => (b ? "1" : "0")).join("");

      // --- Result ---
      const { text, tone } = describe(t);
      const fixedPos = t.verdict.kind === "corrected" ? t.verdict.position : -1;
      const corrected = bitRow(
        t.corrected.map((v, i) => ({
          value: v,
          label: String(i + 1),
          kind: t.parity_positions.includes(i + 1) ? "parity" : "data",
          highlight: i + 1 === fixedPos,
        })),
      );

      const result = section(
        "5. Result",
        el("p", {}, [badge(text, tone)]),
        corrected,
        el("p", { text: `Original data: ${bits(t.data)}   Decoded data: ${bits(t.decoded)}` }),
      );
      if (t.verdict.kind === "double_error_detected") {
        result.append(
          el("p", {
            class: "note",
            text: "The decoded data is not trusted here. A real receiver would discard the frame and ask for a retransmission.",
          }),
        );
      }

      container.replaceChildren(
        el("h2", { text: "Hamming code" }),
        controls,
        channelControls,
        section("2. Sent codeword (blue = data, orange = parity)", sent),
        section("3. Received (click a bit to flip it, as the channel would)", received),
        section(
          "4. Parity checks and syndrome",
          checks,
          el("p", { text: `Syndrome = ${synBinary}\u2082 = ${t.syndrome}` + (t.syndrome ? ` \u2192 position ${t.syndrome} is suspect` : "") }),
        ),
        result,
      );
    };

    draw();
  },
};
