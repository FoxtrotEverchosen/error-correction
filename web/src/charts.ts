const NS = "http://www.w3.org/2000/svg";

function svg(
  tag: string,
  attrs: Record<string, string | number> = {},
  children: (Node | string)[] = [],
): SVGElement {
  const node = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, String(v));
  node.append(...children);
  return node as SVGElement;
}

export interface ChartPoint { x: number; y: number; tip: string }
export interface ChartSeries { name: string; cls: string; points: ChartPoint[] }

const W = 640;
const H = 340;
const M = { l: 64, r: 16, t: 12, b: 48 };

const tickLabel = (exp: number) => (exp >= -3 ? String(10 ** exp) : `1e${exp}`);

/** Log-log line chart. Points with y = 0 can't be drawn: the line breaks there. */
export function logLogChart(series: ChartSeries[], xLabel: string, yLabel: string): SVGElement {
  const root = svg("svg", {
    viewBox: `0 0 ${W} ${H}`,
    class: "chart",
    role: "img",
    "aria-label": `${yLabel} versus ${xLabel}`,
  });
  const xs = series.flatMap((s) => s.points.map((p) => p.x));
  const ys = series.flatMap((s) => s.points.map((p) => p.y)).filter((y) => y > 0);
  if (xs.length === 0 || ys.length === 0) {
    root.append(
      svg("text", { x: W / 2, y: H / 2, "text-anchor": "middle", class: "chart-text" }, [
        "No events observed. Try more trials or a higher error rate.",
      ]),
    );
    return root;
  }

  const lx0 = Math.log10(Math.min(...xs));
  const lx1 = Math.log10(Math.max(...xs));
  const yMinExp = Math.min(-1, Math.floor(Math.log10(Math.min(...ys)) + 1e-9));
  const px = (x: number) => M.l + ((Math.log10(x) - lx0) / (lx1 - lx0 || 1)) * (W - M.l - M.r);
  const py = (y: number) => M.t + (Math.log10(y) / yMinExp) * (H - M.t - M.b);

  for (let e = yMinExp; e <= 0; e++) {
    const y = py(10 ** e);
    root.append(
      svg("line", { x1: M.l, x2: W - M.r, y1: y, y2: y, class: "grid-line" }),
      svg("text", { x: M.l - 8, y: y + 4, "text-anchor": "end", class: "chart-text" }, [tickLabel(e)]),
    );
  }
  for (let e = Math.ceil(lx0 - 1e-9); e <= Math.floor(lx1 + 1e-9); e++) {
    const x = px(10 ** e);
    root.append(
      svg("line", { x1: x, x2: x, y1: M.t, y2: H - M.b, class: "grid-line" }),
      svg("text", { x, y: H - M.b + 16, "text-anchor": "middle", class: "chart-text" }, [tickLabel(e)]),
    );
  }

  const midY = (M.t + H - M.b) / 2;
  root.append(
    svg("text", { x: (M.l + W - M.r) / 2, y: H - 8, "text-anchor": "middle", class: "chart-text" }, [xLabel]),
    svg("text", { x: 14, y: midY, transform: `rotate(-90 14 ${midY})`, "text-anchor": "middle", class: "chart-text" }, [yLabel]),
  );

  for (const s of series) {
    let d = "";
    let pen = false;
    for (const p of s.points) {
      if (p.y > 0) {
        d += `${pen ? "L" : "M"}${px(p.x).toFixed(1)} ${py(p.y).toFixed(1)}`;
        pen = true;
      } else {
        pen = false;
      }
    }
    if (d) root.append(svg("path", { d, class: `line ${s.cls}` }));
    for (const p of s.points) {
      if (p.y > 0) {
        root.append(svg("circle", { cx: px(p.x), cy: py(p.y), r: 3.5, class: `dot ${s.cls}` }, [svg("title", {}, [p.tip])]));
      }
    }
  }
  return root;
}
