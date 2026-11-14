import { runCrc } from "../core";
import type { CrcTrace, DivStep } from "../types";
import { badge, bitRow, channelPanel, el, newChannelState, section, type Tone } from "../components";
import type { Page } from "./page";

interface Preset {
  id: string;
  label: string;
  poly: number; // generator without its leading term
  width: number;
}

const PRESETS: Preset[] = [
  { id: "crc3", label: "CRC-3: x^3 + x + 1 (teaching example)", poly: 0x3, width: 3 },
  { id: "crc4", label: "CRC-4: x^4 + x + 1", poly: 0x3, width: 4 },
  { id: "crc8", label: "CRC-8: x^8 + x^2 + x + 1", poly: 0x07, width: 8 },
  { id: "crc16", label: "CRC-16/XMODEM: x^16 + x^12 + x^5 + 1", poly: 0x1021, width: 16 },
];

const MAX_DATA_BITS = 32;

const bitString = (b: boolean[]) => b.map((x) => (x ? "1" : "0")).join("");

function polyString(generator: boolean[]): string {
  const top = generator.length - 1;
  return generator
    .map((b, i) => (b ? top - i : -1))
    .filter((p) => p >= 0)
    .map((p) => (p === 0 ? "1" : p === 1 ? "x" : `x^${p}`))
    .join(" + ");
}

/** Classic long-division layout. Steps where the leading bit is 0 only shift, so they are skipped. */
function divisionText(initial: boolean[], generator: boolean[], steps: DivStep[]): string {
  const lines = [bitString(initial)];
  for (const s of steps) {
    if (!s.xored) continue;
    lines.push(" ".repeat(s.offset) + bitString(generator));
    lines.push(" ".repeat(s.offset) + "-".repeat(generator.length));
    lines.push(" ".repeat(s.offset + 1) + bitString(s.row.slice(s.offset + 1)));
  }
  return lines.join("\n");
}

function describe(t: CrcTrace): { text: string; tone: Tone } {
  if (t.verdict.kind === "error_detected") {
    return { text: "Error detected: remainder is not zero. The frame would be discarded.", tone: "warn" };
  }
  return t.undetected_error
    ? { text: "Remainder is zero, but the frame was corrupted (the error pattern is divisible by the generator)", tone: "bad" }
    : { text: "Remainder is zero: frame accepted", tone: "ok" };
}

export const crcPage: Page = {
  id: "crc",
  title: "CRC",
  group: "Detection",
  mount(container) {
    let dataText = "11010011101100";
    let preset = PRESETS[0];
    let inputError = "";
    const flips = new Set<number>(); // 0-indexed over the whole frame
    const channelState = newChannelState();

    const draw = () => {
      const data = [...dataText].map((ch) => ch === "1");
      let t: CrcTrace;
      try {
        t = runCrc(data, preset.poly, preset.width, [...flips]);
      } catch (e) {
        container.replaceChildren(el("p", { class: "error", text: String(e) }));
        return;
      }
      const dataLen = t.data.length;
      const label = (i: number) => (i < dataLen ? `d${i + 1}` : `c${i - dataLen + 1}`);
      const kind = (i: number) => (i < dataLen ? "data" : "parity");

      // --- Controls ---
      const dataInput = el("input");
      dataInput.type = "text";
      dataInput.value = dataText;
      dataInput.size = 36;
      dataInput.maxLength = MAX_DATA_BITS;
      dataInput.spellcheck = false;
      // 'change' (not 'input'): draw() rebuilds the DOM and would steal focus while typing.
      dataInput.addEventListener("change", () => {
        const v = dataInput.value.replace(/\s+/g, "");
        if (new RegExp(`^[01]{1,${MAX_DATA_BITS}}$`).test(v)) {
          dataText = v;
          inputError = "";
          flips.clear();
        } else {
          inputError = `Enter 1 to ${MAX_DATA_BITS} bits, using only 0 and 1.`;
        }
        draw();
      });

      const select = el("select");
      for (const p of PRESETS) {
        const opt = el("option", { text: p.label });
        opt.value = p.id;
        opt.selected = p.id === preset.id;
        select.append(opt);
      }
      select.addEventListener("change", () => {
        preset = PRESETS.find((p) => p.id === select.value) ?? PRESETS[0];
        flips.clear();
        draw();
      });

      const random = el("button", { text: "Random data" });
      random.addEventListener("click", () => {
        dataText = Array.from({ length: dataLen }, () => (Math.random() < 0.5 ? "1" : "0")).join("");
        inputError = "";
        flips.clear();
        draw();
      });
      const clear = el("button", { text: "Clear errors" });
      clear.addEventListener("click", () => { flips.clear(); draw(); });

      const controls = section(
        "1. Message and generator",
        el("label", { class: "option" }, ["Message bits: ", dataInput, " ", random]),
        ...(inputError ? [el("p", { class: "error", text: inputError })] : []),
        el("label", { class: "option" }, ["Generator: ", select]),
        el("p", { text: `Generator ${polyString(t.generator)} = ${bitString(t.generator)} (${t.generator.length} bits), so the CRC is ${t.width} bits.` }),
      );

      // --- Sender ---
      const sender = section(
        `2. Sender: divide the message plus ${t.width} zeros by the generator`,
        el("pre", { class: "division", text: divisionText([...t.data, ...Array(t.width).fill(false)], t.generator, t.encode_steps) }),
        el("p", { text: `The remainder is the CRC: ${bitString(t.crc)}. It replaces the appended zeros.` }),
      );

      // --- Frames ---
      const sent = bitRow(t.codeword.map((v, i) => ({ value: v, label: label(i), kind: kind(i) })));
      const channel = channelPanel(channelState, t.codeword.length, (zeroIndexed) => {
        flips.clear();
        zeroIndexed.forEach((i) => flips.add(i));
        draw();
      });
      const received = bitRow(
        t.received.map((v, i) => ({
          value: v,
          label: label(i),
          kind: kind(i),
          flipped: t.flipped.includes(i),
          onClick: () => {
            if (flips.has(i)) flips.delete(i); else flips.add(i);
            draw();
          },
        })),
      );

      // --- Receiver ---
      const { text, tone } = describe(t);
      const receiver = section(
        "5. Receiver: divide the received frame by the generator",
        el("p", { text: `Remainder: ${bitString(t.remainder)}` + (t.remainder.some(Boolean) ? "" : " (all zeros)") }),
        el("p", {}, [badge(text, tone)]),
        el("details", {}, [
          el("summary", { text: "Show the receiver's division" }),
          el("pre", { class: "division", text: divisionText(t.received, t.generator, t.check_steps) }),
        ]),
      );

      container.replaceChildren(
        el("h2", { text: "Cyclic redundancy check (CRC)" }),
        el("p", { class: "note", text: "The sender divides the message by a fixed generator (XOR instead of subtraction) and appends the remainder. The receiver divides the whole frame: any nonzero remainder means corruption. CRC only detects errors, it can't say which bit is wrong." }),
        controls,
        sender,
        section("3. Sent frame (blue = data, orange = CRC)", sent),
        channel,
        section("4. Received (click a bit to flip it)", received, clear),
        receiver,
        el("p", { class: "note", text: `Every single flip and every burst of up to ${t.width} consecutive bits is always caught. Other error patterns slip through with probability of about 1 in ${2 ** t.width}. With the CRC-3 preset, flipping two bits exactly 7 apart (for example d1 and d8) goes undetected, because x^7 + 1 is divisible by the generator.` }),
        el("p", { class: "note", text: "This page shows the plain math. Protocols like Ethernet's CRC-32 add an initial value, bit reflection and a final XOR on top of it, so their results differ from this division." }),
      );
    };

    draw();
  },
};
