import { $, cssVar } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// The countercurrent multiplier, depth by depth (the textbook stepwise model, as in Guyton's and
// Koeppen & Stanton's diagrams): osmolarity in the descending arm D, the ascending arm A, the
// tissue I (equal to D) and the collecting duct C, at n depths from the cortex (0) to the tip.

const BLOOD = 300; // mOsm/L
/** RT at 37 °C, J per mmol: the least work to move 1 mmol from concentration a to b is RT ln(b/a). */
const RT = 2.58;

const pumpIn = $<HTMLInputElement>("pump");
const adhIn = $<HTMLInputElement>("adh");
const lenIn = $<HTMLInputElement>("len");
const washIn = $<HTMLInputElement>("wash");
const plot = new LinesPlot($<HTMLCanvasElement>("plot"), { xLabel: "depth (cortex → tip)", yLabel: "mOsm/L", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

let n = 8;
let D = new Float64Array(n);
let A = new Float64Array(n);
let I = new Float64Array(n);
let C = new Float64Array(n);
let work = 0; // J, per litre of fluid through the loop, averaged
let running = false;
let phase = 0; // 0..1: how far the fluid has moved towards the next depth (for drawing)

function reset() {
  n = Number(lenIn.value);
  D = new Float64Array(n).fill(BLOOD);
  A = new Float64Array(n).fill(BLOOD);
  I = new Float64Array(n).fill(BLOOD);
  C = new Float64Array(n).fill(BLOOD);
  work = 0;
}

/** One step of the multiplier (the textbook stepwise model); returns the pumps' work this step
 * (J per litre of fluid). At each depth the pumps move salt from the ascending fluid into the
 * tissue, and the descending fluid (whose water leaves freely) matches the tissue: the two sides
 * split about their mean, PUMP apart. Blood washes the tissue a little towards 300. Then the
 * fluid moves on one depth. */
function step() {
  const pump = Number(pumpIn.value);
  const wash = Number(washIn.value);
  let w = 0;
  for (let i = 0; i < n; i++) {
    const m = 0.5 * (A[i] + D[i]);
    const d = Math.min(pump / 2, m - 1);
    const moved = A[i] - (m - d);
    if (moved > 0) w += moved * RT * Math.log((m + d) / (m - d));
    A[i] = m - d;
    D[i] = m + d;
    D[i] -= wash * (D[i] - BLOOD);
    I[i] = D[i];
  }
  const bottom = D[n - 1];
  for (let i = n - 1; i > 0; i--) D[i] = D[i - 1];
  D[0] = BLOOD;
  for (let i = 0; i < n - 1; i++) A[i] = A[i + 1];
  A[n - 1] = bottom;
  // The collecting duct: the fluid leaving the loop goes on through the cortex, where the
  // ascending arm's pumps bring it to PUMP below the cortex's 300 (a dilute fluid); then, with
  // ADH, it comes back to 300 there, and flows down through the medulla losing water towards the
  // tissue's osmolarity as fast as ADH allows.
  const adh = Number(adhIn.value);
  const dilute = Math.min(A[0], BLOOD - pump);
  C[0] = dilute + adh * (BLOOD - dilute);
  for (let i = 1; i < n; i++) C[i] = C[i - 1] + adh * 0.6 * (I[i] - C[i - 1]);
  return w;
}

// ---- Drawing: the loop as a hairpin in the tissue, coloured by saltiness. ----
const canvas = $<HTMLCanvasElement>("canvas");
function color(osm: number) {
  // From blood's 300 (cream) to 1,200 and beyond (deep red); below 300 (dilute) blue.
  const [lo, mid, hi] = [cssColor("--species-2"), cssColor("--cream"), cssColor("--species-3")];
  const t = Math.max(-1, Math.min(1, osm < BLOOD ? (osm - BLOOD) / 200 : (osm - BLOOD) / 900));
  const [a, b] = t < 0 ? [mid, lo] : [mid, hi];
  const f = Math.abs(t);
  return `rgb(${[0, 1, 2].map((k) => Math.round(255 * (a[k] + (b[k] - a[k]) * f))).join(",")})`;
}

function draw() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const [Wc, Hc] = [canvas.clientWidth, canvas.clientHeight];
  canvas.width = Math.round(Wc * dpr);
  canvas.height = Math.round(Hc * dpr);
  const g = canvas.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, Wc, Hc);
  const top = 0.1 * Hc;
  const depth = 0.8 * Hc;
  const h = depth / n;
  const [xd, xa, xc] = [0.3 * Wc, 0.48 * Wc, 0.72 * Wc];
  const tw = 0.07 * Wc;
  // Tissue bands.
  for (let i = 0; i < n; i++) {
    g.globalAlpha = 0.45;
    g.fillStyle = color(I[i]);
    g.fillRect(0.12 * Wc, top + i * h, 0.76 * Wc, h + 0.5);
  }
  g.globalAlpha = 1;
  // The tubes.
  const tube = (x: number, vals: Float64Array) => {
    for (let i = 0; i < n; i++) {
      g.fillStyle = color(vals[i]);
      g.fillRect(x - tw / 2, top + i * h, tw, h + 0.5);
    }
    g.strokeStyle = cssVar("--muted");
    g.strokeRect(x - tw / 2, top, tw, depth);
  };
  tube(xd, D);
  tube(xa, A);
  tube(xc, C);
  // The bend.
  g.fillStyle = color(D[n - 1]);
  g.fillRect(xd - tw / 2, top + depth, xa - xd + tw, 0.02 * Hc);
  // Flow: dots moving down, round and up, and down the duct.
  g.fillStyle = cssVar("--ink") || "#fff";
  g.globalAlpha = 0.7;
  for (let i = 0; i < n; i++) {
    const y = top + (i + phase) * h;
    g.beginPath();
    g.arc(xd, y, 2.5, 0, 2 * Math.PI);
    g.arc(xc, y, 2.5, 0, 2 * Math.PI);
    g.fill();
    g.beginPath();
    g.arc(xa, top + (n - i - phase) * h, 2.5, 0, 2 * Math.PI);
    g.fill();
  }
  g.globalAlpha = 1;
  // Pumps: arrows from the ascending arm into the tissue, as thick as their push.
  g.strokeStyle = cssVar("--highlight");
  g.lineWidth = 1 + 3 * (Number(pumpIn.value) / 300);
  for (let i = 0; i < n; i++) {
    const y = top + (i + 0.5) * h;
    g.beginPath();
    g.moveTo(xa + tw / 2, y);
    g.lineTo(xa + tw / 2 + 0.05 * Wc, y);
    g.stroke();
  }
  g.fillStyle = cssVar("--cream");
  g.font = "12px system-ui, sans-serif";
  g.textAlign = "center";
  g.fillText("down", xd, top - 8);
  g.fillText("up", xa, top - 8);
  g.fillText("collecting duct", xc, top - 8);
  g.fillText("cortex", 0.06 * Wc, top + 10);
  g.fillText("tip", 0.06 * Wc, top + depth);
  g.fillText(`urine: ${C[n - 1].toFixed(0)}`, xc, top + depth + 0.06 * Hc);

  const xs = (v: Float64Array) => Array.from(v, (y, i) => ({ x: i, y }));
  plot.set([
    { points: xs(D), color: cssVar("--species-2") },
    { points: xs(A), color: cssVar("--species-3") },
    { points: xs(I), color: cssVar("--muted"), dashed: true },
    { points: xs(C), color: cssVar("--accent") },
  ]);
  $("urine").textContent = `${C[n - 1].toFixed(0)} mOsm/L`;
  $("out") && ($("out").textContent = `${Math.min(A[0], BLOOD - Number(pumpIn.value)).toFixed(0)} mOsm/L`);
  $("tip").textContent = `${I[n - 1].toFixed(0)} mOsm/L`;
  // The work, per litre of urine: the pumps' work per litre through the loop, over the share of
  // it that ends up as urine (fluid conserving its salt: urine volume ∝ 1 / its osmolarity).
  const urineShare = BLOOD / Math.max(BLOOD, C[n - 1]);
  $("work").textContent = `${(work / urineShare / 1000).toFixed(1)} kJ (in this model)`;
}

let acc = 0;
function frame() {
  if (running) {
    // Thirty steps of flow per second.
    acc += 0.5;
    while (acc >= 1) {
      work = 0.9 * work + 0.1 * step();
      acc -= 1;
    }
    phase = acc;
    draw();
  }
  requestAnimationFrame(frame);
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("reset").onclick = () => {
  reset();
  draw();
};
const labels = () => {
  $("pumpv").textContent = `${pumpIn.value} mOsm`;
  const a = Number(adhIn.value);
  $("adhv").textContent = a > 0.7 ? "high" : a > 0.2 ? "some" : a > 0 ? "little" : "none";
  const l = Number(lenIn.value);
  $("lenv").textContent = l <= 6 ? "short (a beaver's)" : l <= 10 ? "a human's" : "long (a kangaroo rat's)";
  const w = Number(washIn.value);
  $("washv").textContent = w < 0.03 ? "low" : w < 0.08 ? "some" : "high";
};
for (const el of [pumpIn, adhIn, washIn]) el.oninput = labels;
lenIn.oninput = () => {
  labels();
  // A longer loop: keep what is there and add depths at the bottom, like the top's.
  const m = Number(lenIn.value);
  const grow = (v: Float64Array) => Float64Array.from({ length: m }, (_, i) => v[Math.min(i, v.length - 1)]);
  [D, A, I, C] = [grow(D), grow(A), grow(I), grow(C)];
  n = m;
  draw();
};
labels();
reset();
draw();
new ResizeObserver(draw).observe(canvas);
requestAnimationFrame(frame);

declare global {
  interface Window {
    kidneyTest?: { run(steps: number, opts: { pump?: number; adh?: number; len?: number; wash?: number }): { urine: number; tip: number; out: number } };
  }
}

window.kidneyTest = {
  /** Start again with these settings and run `steps` steps: urine, the tip's osmolarity, and
   * the ascending arm's output (to the distal tubule). */
  run(steps, opts) {
    if (opts.pump !== undefined) pumpIn.value = String(opts.pump);
    if (opts.adh !== undefined) adhIn.value = String(opts.adh);
    if (opts.len !== undefined) lenIn.value = String(opts.len);
    if (opts.wash !== undefined) washIn.value = String(opts.wash);
    labels();
    reset();
    for (let k = 0; k < steps; k++) work = 0.9 * work + 0.1 * step();
    draw();
    return { urine: C[n - 1], tip: I[n - 1], out: A[0] };
  },
};
