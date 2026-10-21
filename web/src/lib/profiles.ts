import { $, cssVar } from "./host";

/** One vertical profile beside a column: a bar per height slice (bottom of the canvas = floor),
 * from `lo` (left edge) to `hi` (right edge), with optional dashed comparison values and the
 * range printed at the top. Several `values` are stacked side by side. */
export function side(
  id: string,
  p: {
    values: number[][];
    colors: (string | ((b: number) => string))[];
    dashed?: number[];
    /** A second comparison, dotted in the accent colour. */
    guide?: number[];
    lo?: number;
    hi: number;
    label: (v: number) => string;
    /** The part of the canvas height the bars cover, as fractions from the bottom (default the
     * whole height): to line the bars up with a box drawn inside a larger view. */
    span?: [number, number];
  },
) {
  const c = $<HTMLCanvasElement>(id);
  const dpr = window.devicePixelRatio || 1;
  const [W, H] = [c.clientWidth, c.clientHeight];
  if (c.width !== Math.round(W * dpr) || c.height !== Math.round(H * dpr)) {
    c.width = Math.round(W * dpr);
    c.height = Math.round(H * dpr);
  }
  const ctx = c.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  const lo = p.lo ?? 0;
  const x = (v: number) => (Math.max(0, v - lo) / (p.hi - lo)) * W;
  const n = p.values[0].length;
  const [f0, f1] = p.span ?? [0, 1];
  const bh = (H * (f1 - f0)) / n;
  const base = H * (1 - f0); // the pixel row of the bottom of bin 0
  for (let b = 0; b < n; b++) {
    let x0 = 0;
    p.values.forEach((v, k) => {
      const w = k === 0 ? x(v[b]) : (Math.max(0, v[b]) / (p.hi - lo)) * W; // stacked on the first
      const col = p.colors[k];
      ctx.fillStyle = typeof col === "string" ? col : col(b);
      ctx.fillRect(x0, base - (b + 1) * bh + 0.5, w, bh - (n > 40 ? 0 : 1));
      x0 += w;
    });
  }
  for (const [line, color, dash] of [
    [p.dashed, "--text", [3, 3]],
    [p.guide, "--accent", [1, 2]],
  ] as const) {
    if (!line) continue;
    ctx.strokeStyle = cssVar(color);
    ctx.lineWidth = color === "--accent" ? 2 : 1;
    ctx.setLineDash([...dash]);
    ctx.beginPath();
    line.forEach((v, b) => {
      const y = base - (b + 0.5) * bh;
      if (b === 0) ctx.moveTo(x(v), y);
      else ctx.lineTo(x(v), y);
    });
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.lineWidth = 1;
  }
  ctx.font = "10px system-ui, sans-serif";
  const text = p.lo === undefined ? p.label(p.hi) : `${p.label(lo)}–${p.label(p.hi)}`;
  ctx.fillStyle = cssVar("--bg");
  ctx.fillRect(0, 0, ctx.measureText(text).width + 4, 12);
  ctx.fillStyle = cssVar("--muted");
  ctx.fillText(text, 2, 10);
}

/** Blue for colder than the reference, grey for the reference, red for hotter (full colour at
 * ±`spread` as a fraction). */
export function warmth(r: number, spread = 0.4) {
  const [cold, mid, hot] = [[60, 120, 210], [150, 140, 130], [215, 65, 45]];
  const t = Math.max(-1, Math.min(1, (r - 1) / spread));
  const [a, b] = t < 0 ? [mid, cold] : [mid, hot];
  const c = a.map((x, i) => Math.round(x + (b[i] - x) * Math.abs(t)));
  return `rgb(${c.join(",")})`;
}
