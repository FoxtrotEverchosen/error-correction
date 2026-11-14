import { runCompare } from "../core";
import type { Counts, SweepPoint } from "../types";
import { el, section } from "../components";
import { logLogChart, type ChartSeries } from "../charts";
import type { Page } from "./page";

const SWEEP_BERS = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2];

type Metric = "silent" | "failed";
const METRIC_LABEL: Record<Metric, string> = {
  silent: "Silent errors (wrong data delivered)",
  failed: "Not delivered correctly (detected + silent)",
};

function pct(count: number, n: number): string {
  if (count === 0) return "0%";
  const p = (count / n) * 100;
  if (p < 0.01) return "<0.01%";
  return `${p >= 10 ? p.toFixed(1) : p.toFixed(2)}%`;
}

function numInput(value: number, min: number, max: number, step: number): HTMLInputElement {
  const input = el("input");
  input.type = "number";
  input.min = String(min);
  input.max = String(max);
  input.step = String(step);
  input.value = String(value);
  return input;
}

function outcomeBar(c: Counts, n: number): HTMLElement {
  const bar = el("div", { class: "stack" });
  for (const key of ["clean", "corrected", "detected", "silent"] as const) {
    if (c[key] === 0) continue;
    const seg = el("div", { class: `seg ${key}`, title: `${key}: ${c[key]} of ${n}` });
    seg.style.width = `${(c[key] / n) * 100}%`;
    bar.append(seg);
  }
  return bar;
}

function resultsTable(p: SweepPoint): HTMLElement {
  const head = el(
    "tr",
    {},
    ["Scheme", "Code rate", "Outcomes", "Clean", "Corrected", "Detected", "Silent"].map((h) => el("th", { text: h })),
  );
  const rows = p.schemes.map((s) => {
    const c = s.counts;
    return el("tr", {}, [
      el("td", { text: s.name }),
      el("td", { text: `${s.data_bits}/${s.sent_bits} = ${s.code_rate.toFixed(2)}` }),
      el("td", {}, [outcomeBar(c, p.trials)]),
      el("td", { class: "num", text: pct(c.clean, p.trials) }),
      el("td", { class: "num", text: pct(c.corrected, p.trials) }),
      el("td", { class: "num", text: pct(c.detected, p.trials) }),
      el("td", { class: "num", text: pct(c.silent, p.trials) }),
    ]);
  });
  return el("table", {}, [el("thead", {}, [head]), el("tbody", {}, rows)]);
}

function definitions(): HTMLElement {
  const item = (term: string, text: string) => el("li", {}, [el("strong", { text: term }), ` ${text}`]);
  return el("ul", { class: "defs" }, [
    item("Clean:", "no bit was flipped."),
    item("Corrected:", "bits were flipped, the decoder repaired them and the payload arrived intact."),
    item("Detected:", "the receiver noticed a problem it can't repair and discards the payload (a real protocol would ask for a retransmission)."),
    item("Silent:", "the receiver delivered wrong data without noticing. This is the residual error rate, the dangerous case."),
  ]);
}

function chartSeries(sweep: SweepPoint[], metric: Metric): ChartSeries[] {
  return sweep[0].schemes.map((s0, i) => ({
    name: s0.name,
    cls: `s-${s0.id}`,
    points: sweep.map((pt) => {
      const c = pt.schemes[i].counts;
      const count = metric === "silent" ? c.silent : c.silent + c.detected;
      return {
        x: pt.ber,
        y: count / pt.trials,
        tip: `${s0.name}: ${count} of ${pt.trials} payloads (${pct(count, pt.trials)}) at error rate ${pt.ber}`,
      };
    }),
  }));
}

function legend(p: SweepPoint): HTMLElement {
  return el(
    "div",
    { class: "legend" },
    p.schemes.map((s) =>
      el("span", { class: `legend-item s-${s.id}` }, [
        el("span", { class: "swatch" }),
        el("span", { class: "legend-text", text: s.name }),
      ]),
    ),
  );
}

export const comparePage: Page = {
  id: "compare",
  title: "Method comparison",
  group: "Compare",
  mount(container) {
    const berIn = numInput(0.02, 0, 1, 0.005);
    const burstIn = numInput(1, 1, 8, 1);
    const trialsIn = numInput(10000, 100, 100000, 1000);
    const seedIn = numInput(1, 1, Number.MAX_SAFE_INTEGER, 1);
    const runBtn = el("button", { text: "Run" });
    const seedBtn = el("button", { text: "New seed" });
    const status = el("p", { class: "note" });
    const results = el("div");
    const chartHost = el("div");
    const metricSel = el("select");
    for (const [value, label] of Object.entries(METRIC_LABEL)) {
      const opt = el("option", { text: label });
      opt.value = value;
      metricSel.append(opt);
    }
    let sweep: SweepPoint[] = [];

    const drawChart = () => {
      if (sweep.length === 0) return;
      const metric = metricSel.value as Metric;
      chartHost.replaceChildren(
        logLogChart(chartSeries(sweep, metric), "Channel bit error rate (log scale)", "Share of payloads (log scale)"),
        legend(sweep[0]),
      );
    };
    metricSel.addEventListener("change", drawChart);

    const readNum = (input: HTMLInputElement, min: number, max: number, fallback: number): number => {
      const n = Number(input.value);
      const v = Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : fallback;
      input.value = String(v);
      return v;
    };

    const run = async () => {
      const ber = readNum(berIn, 0, 1, 0.02);
      const burst = Math.round(readNum(burstIn, 1, 8, 1));
      const trials = Math.round(readNum(trialsIn, 100, 100000, 10000));
      const seed = BigInt(Math.trunc(readNum(seedIn, 1, Number.MAX_SAFE_INTEGER, 1)));
      runBtn.disabled = true;
      status.textContent = "Running...";
      await new Promise((r) => setTimeout(r, 30)); // let the browser paint the status first
      try {
        const t0 = performance.now();
        const point = runCompare([ber], burst, trials, seed)[0];
        sweep = runCompare(SWEEP_BERS, burst, trials, seed);
        const ms = Math.round(performance.now() - t0);
        status.textContent = `Simulated ${trials.toLocaleString()} payloads per scheme at ${SWEEP_BERS.length + 1} error rates in ${ms} ms (seed ${seed}).`;
        results.replaceChildren(
          section(
            `Outcomes at bit error rate ${ber}` + (burst > 1 ? `, errors in bursts of ${burst}` : ", independent errors"),
            resultsTable(point),
            definitions(),
          ),
          section(
            "Reliability across error rates",
            el("label", { class: "option" }, ["Show: ", metricSel]),
            chartHost,
            el("p", {
              class: "note",
              text:
                "Each point is a Monte Carlo estimate. Outcomes rarer than about 1 in N trials usually aren't observed, so those points are left out and the line breaks. " +
                "In the silent-errors view, a steeper line means more bit errors are needed to fool the scheme: roughly 1 for uncoded, 2 for parity and Hamming, 3 for 2D parity and extended Hamming.",
            }),
          ),
        );
        drawChart();
      } catch (e) {
        status.textContent = `Error: ${e}`;
      } finally {
        runBtn.disabled = false;
      }
    };
    runBtn.addEventListener("click", run);
    seedBtn.addEventListener("click", () => {
      seedIn.value = String(1 + Math.floor(Math.random() * 2 ** 32));
      run();
    });

    container.replaceChildren(
      el("h2", { text: "Method comparison" }),
      el("p", {
        class: "note",
        text:
          "Every trial sends the same random 16-bit payload through each scheme over the same kind of noisy channel. " +
          "Uncoded: the 16 bits as they are. Parity: one parity bit over all 16. 2D parity: one 4x4 grid (25 bits). " +
          "Hamming and Extended Hamming: four codewords of 7 or 8 bits sent back to back, so a burst can span two of them.",
      }),
      section(
        "Channel and simulation",
        el("label", { class: "option" }, ["Bit error rate: ", berIn]),
        el("label", { class: "option" }, ["Burst length: ", burstIn, " (1 = independent errors, larger = errors come in clusters, same average rate)"]),
        el("label", { class: "option" }, ["Trials per scheme: ", trialsIn, " (large values take a few seconds)"]),
        el("label", { class: "option" }, ["Seed: ", seedIn, " ", seedBtn]),
        runBtn,
      ),
      status,
      results,
    );
    run();
  },
};
