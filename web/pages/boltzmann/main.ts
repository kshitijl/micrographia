import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, LinesPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const BOXES = [8, 6];
const SIDE = 16;
const BINS = 30;
/** Bin width of the entropy estimate (kT units). */
const DE = 0.05;
/** Snapshots of the outlined box kept for its histogram (one per frame, ~2 τ at 8 steps/frame). */
const KEEP = 100;
const startSel = $<HTMLSelectElement>("start");
const speedIn = $<HTMLInputElement>("speed");
const sourceSel = $<HTMLSelectElement>("source");
const viewSel = $<HTMLSelectElement>("view");

let outlined = 0;
let boxKe: Float32Array[] = []; // recent snapshots of the outlined box's kinetic energies
let lastKe: Float32Array | null = null;
let total0 = NaN;
let pe = 0;
let frames = 0;
const series = { t: [] as number[], total: [] as number[], tbox: [] as number[], s: [] as number[], smax: [] as number[] };

const hist = new BarPlot($<HTMLCanvasElement>("hist"), {
  labels: [],
  yLabel: "probability density",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const energyPlot = new LinesPlot($<HTMLCanvasElement>("energy"), { xLabel: "time (τ)", yLabel: "energy per molecule (kT)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const entropyPlot = new LinesPlot($<HTMLCanvasElement>("entropy"), { xLabel: "time (τ)", yLabel: "entropy per molecule (k)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** Boltzmann's H-function entropy −Σ p ln(p/ΔE) of energies (flat density of states in 2D), with
 * the Miller–Madow correction (m − 1)/2N for a histogram of N samples in m occupied bins. */
export function entropy(ke: ArrayLike<number>): number {
  const counts = new Map<number, number>();
  for (let i = 0; i < ke.length; i++) {
    const b = Math.floor(ke[i] / DE);
    counts.set(b, (counts.get(b) ?? 0) + 1);
  }
  const n = ke.length;
  let s = 0;
  for (const c of counts.values()) s -= (c / n) * Math.log(c / n / DE);
  return s + (counts.size - 1) / (2 * n);
}

function boxOf(x: number, y: number): number {
  return Math.min(BOXES[1] - 1, Math.floor(y / SIDE)) * BOXES[0] + Math.min(BOXES[0] - 1, Math.floor(x / SIDE));
}

function overlay(): number[] {
  const line = cssColor("--box-line");
  const r: number[] = [];
  const [W, H] = [BOXES[0] * SIDE, BOXES[1] * SIDE];
  const t = 0.12;
  for (let i = 0; i <= BOXES[0]; i++) r.push(i * SIDE - t, 0, i * SIDE + t, H, ...line);
  for (let j = 0; j <= BOXES[1]; j++) r.push(0, j * SIDE - t, W, j * SIDE + t, ...line);
  const [x0, y0] = [(outlined % BOXES[0]) * SIDE, Math.floor(outlined / BOXES[0]) * SIDE];
  const hi = cssColor("--highlight");
  const w = 0.35;
  r.push(x0, y0, x0 + SIDE, y0 + w, ...hi, x0, y0 + SIDE - w, x0 + SIDE, y0 + SIDE, ...hi, x0, y0, x0 + w, y0 + SIDE, ...hi, x0 + SIDE - w, y0, x0 + SIDE, y0 + SIDE, ...hi);
  return r;
}

/** Histogram of `values` (energies, or speeds if the view is speed) against Boltzmann at kT. */
function drawHistogram(ke: ArrayLike<number>, kT: number) {
  const speed = viewSel.value === "speed";
  const hiX = speed ? 4 : 6;
  const w = hiX / BINS;
  const counts = new Float64Array(BINS);
  for (let i = 0; i < ke.length; i++) {
    const x = speed ? Math.sqrt((2 * ke[i]) / kT) : ke[i] / kT;
    const b = Math.floor(x / w);
    if (b < BINS) counts[b]++;
  }
  const measured = Array.from(counts, (c) => c / ke.length / w);
  const theory = Array.from({ length: BINS }, (_, i) => {
    const x = (i + 0.5) * w;
    return speed ? x * Math.exp(-0.5 * x * x) : Math.exp(-x);
  });
  hist.configure({ log: viewSel.value === "log", xLabel: speed ? "speed / √(kT/m)" : "energy / kT" });
  hist.set(
    measured,
    theory,
    Array.from({ length: BINS }, (_, i) => (i * w).toFixed(1)),
  );
}

function redraw(kT: number) {
  if (lastKe) drawHistogram(sourceSel.value === "box" ? concat(boxKe) : lastKe, kT);
  const pts = (ys: number[]) => ys.map((y, k) => ({ x: series.t[k], y }));
  energyPlot.set([
    { points: pts(series.total), color: cssVar("--accent") },
    { points: pts(series.tbox), color: cssVar("--plot-back") },
  ]);
  entropyPlot.set([
    { points: pts(series.smax), color: cssVar("--muted"), dashed: true },
    { points: pts(series.s), color: cssVar("--accent") },
  ]);
}

function concat(a: Float32Array[]): Float32Array {
  const out = new Float32Array(a.reduce((n, x) => n + x.length, 0));
  let o = 0;
  for (const x of a) {
    out.set(x, o);
    o += x.length;
  }
  return out;
}

async function readout(sim: DiscSim) {
  // Kinetic and potential energy from the same step (a stale one makes the total wobble by ~1%).
  const [{ ke }, { pos }, u] = (await Promise.all([sim.kinetic_energies(), sim.all_positions(), sim.potential_energy()])) as [{ ke: Float32Array }, { pos: Float32Array }, number];
  pe = u;
  frames++;
  const n = ke.length;
  let kin = 0;
  const box: number[] = [];
  for (let i = 0; i < n; i++) {
    kin += ke[i];
    if (boxOf(pos[2 * i], pos[2 * i + 1]) === outlined) box.push(ke[i]);
  }
  const kT = kin / n;
  const total = (kin + pe) / n;
  if (!Number.isFinite(total0)) total0 = total;
  boxKe.push(Float32Array.from(box));
  if (boxKe.length > KEEP) boxKe.shift();
  lastKe = ke;
  const t = (sim.steps() * sim.dt());
  const tbox = box.reduce((a, b) => a + b, 0) / Math.max(1, box.length);
  const s = entropy(ke);
  series.t.push(t);
  series.total.push(total);
  series.tbox.push(tbox);
  series.s.push(s);
  series.smax.push(1 + Math.log(kT));
  $("time").textContent = t.toFixed(1);
  $("etot").textContent = `${(total * n).toFixed(0)} (${total >= total0 ? "+" : "−"}${Math.abs((100 * (total - total0)) / total0).toFixed(2)}%)`;
  $("tbox").textContent = tbox.toFixed(2);
  $("ent").textContent = s.toFixed(3);
  redraw(kT);
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "ensemble",
  // Half the usual time step: the hot starts have fast molecules, and the total energy should
  // hold to a fraction of a percent (crates/md/tests/ensemble.rs).
  options: () => ({ boxes_x: BOXES[0], boxes_y: BOXES[1], per_box: 64, start: Number(startSel.value), dt: 0.0025, seed: 41 }),
  colors: ["--cream"],
  stepsPerFrame: () => Number(speedIn.value),
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    boxKe = [];
    lastKe = null;
    total0 = NaN;
    frames = 0;
    for (const k of Object.keys(series) as (keyof typeof series)[]) series[k] = [];
    sim.color_by_speed(3);
    sim.set_overlay(new Float32Array(overlay()));
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("reset").onclick = () => void host.create().then(() => ($("run").textContent = "Run"));
startSel.onchange = () => void host.create().then(() => ($("run").textContent = "Run"));
speedIn.oninput = () => ($("speedv").textContent = `${speedIn.value} steps/frame`);
sourceSel.onchange = viewSel.onchange = () => redraw(1);
$<HTMLCanvasElement>("canvas").onclick = (ev) => {
  const c = ev.currentTarget as HTMLCanvasElement;
  const r = c.getBoundingClientRect();
  const x = ((ev.clientX - r.left) / r.width) * BOXES[0] * SIDE;
  const y = (1 - (ev.clientY - r.top) / r.height) * BOXES[1] * SIDE;
  outlined = boxOf(x, y);
  boxKe = [];
  for (const k of ["tbox"] as const) series[k] = series[k].map(() => NaN);
  host.sim?.set_overlay(new Float32Array(overlay()));
};

declare global {
  interface Window {
    boltzmannTest?: {
      ready: Promise<void>;
      /** Recreate with a start, run `frames` readouts `every` steps apart; return the series and
       * the worst deviation of the pooled energy histogram from Boltzmann (fraction of its peak). */
      run(start: number, frames: number, every: number): Promise<{ t: number[]; total: number[]; tbox: number[]; s: number[]; smax: number[]; worst: number }>;
    };
  }
}

window.boltzmannTest = {
  ready: host.ready,
  async run(start: number, n: number, every: number) {
    await host.ready;
    startSel.value = String(start);
    await host.create();
    return host.exclusive(async (sim) => {
      for (let f = 0; f < n; f++) {
        sim.step(every);
        await readout(sim);
      }
      const ke = lastKe!;
      const kT = ke.reduce((a, b) => a + b, 0) / ke.length;
      const w = 6 / BINS;
      const counts = new Float64Array(BINS);
      for (const e of ke) if (e / kT / w < BINS) counts[Math.floor(e / kT / w)]++;
      let worst = 0;
      counts.forEach((c, i) => (worst = Math.max(worst, Math.abs(c / ke.length / w - Math.exp(-(i + 0.5) * w)))));
      return { ...series, worst };
    });
  },
};
