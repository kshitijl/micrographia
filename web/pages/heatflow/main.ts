import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, LinesPlot, ScatterPlot } from "../../src/lib/plot";
import { Tools } from "../../src/lib/interact";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const COLS = 128;
const SIDE = 8; // σ: each column is 8 σ wide and 16 σ tall
const BINS = 31;
const TC = 1.0;
const windowSel = $<HTMLSelectElement>("window");
const thInput = $<HTMLInputElement>("th");

let qs: number[] = [];
let last: { hot: Float64Array; cold: Float64Array } | null = null;
let stepsInWindow = 0;

const hist = new BarPlot($<HTMLCanvasElement>("hist"), {
  labels: [],
  yLabel: "share of windows",
  xLabel: "heat Q passed hot → cold in one window (ε)",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const ratio = new ScatterPlot($<HTMLCanvasElement>("ratio"), {
  xLabel: "Q (ε)",
  yLabel: "ln[P(Q) / P(−Q)]",
  point: () => cssVar("--accent"),
  line: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const th = () => Number(thInput.value);
const windowSteps = () => Number(windowSel.value);
const predicted = () => 1 / TC - 1 / th();

function clear() {
  qs = [];
  last = null;
  stepsInWindow = 0;
  $("pred").textContent = predicted().toFixed(3);
}

function analyse() {
  if (qs.length < 50) return;
  const sorted = [...qs].sort((a, b) => a - b);
  const qmax = Math.max(Math.abs(sorted[Math.floor(0.002 * sorted.length)]), Math.abs(sorted[Math.floor(0.998 * sorted.length)]));
  const w = (2 * qmax) / BINS;
  const counts = new Array(BINS).fill(0);
  for (const q of qs) {
    const b = Math.floor((q + qmax) / w);
    if (b >= 0 && b < BINS) counts[b]++;
  }
  // Bars: forward bins in the accent colour, backward bins drawn as the "theory" overlay colour.
  const share = counts.map((c) => c / qs.length);
  const backward = share.map((v, i) => ((i + 0.5) * w - qmax < 0 ? v : 0));
  hist.set(
    share.map((v, i) => ((i + 0.5) * w - qmax >= 0 ? v : 0)),
    backward,
    counts.map((_, i) => ((i + 0.5) * w - qmax).toFixed(1)),
  );
  // ln P(Q)/P(−Q) for symmetric bin pairs with enough counts.
  const pts: { x: number; y: number; e: number }[] = [];
  let num = 0;
  let den = 0;
  for (let i = 0; i < BINS; i++) {
    const j = BINS - 1 - i;
    const x = (i + 0.5) * w - qmax;
    if (x <= 0) continue;
    if (counts[i] >= 8 && counts[j] >= 8) {
      const y = Math.log(counts[i] / counts[j]);
      const e = Math.sqrt(1 / counts[i] + 1 / counts[j]);
      pts.push({ x, y, e });
      num += (x * y) / (e * e);
      den += (x * x) / (e * e);
    }
  }
  const slope = den > 0 ? num / den : NaN;
  ratio.set(pts, predicted());
  const neg = qs.filter((q) => q < 0).length / qs.length;
  const mean = qs.reduce((a, b) => a + b, 0) / qs.length;
  $("neg").textContent = `${(100 * neg).toFixed(1)}%`;
  $("mean").textContent = `${mean.toFixed(2)} ε`;
  $("slope").textContent = Number.isFinite(slope) ? slope.toFixed(3) : "–";
  $("status").textContent = `${qs.length.toLocaleString()} windows measured. Backward flow in ${(100 * neg).toFixed(1)}% of them.`;
}

async function readout(sim: DiscSim) {
  stepsInWindow += 50;
  if (stepsInWindow < windowSteps()) return;
  stepsInWindow = 0;
  const l = (await sim.ledger(COLS)) as { energy: Float64Array };
  // Walls: 2 = y-low (hot), 3 = y-high (cold); slots are columns.
  const hot = l.energy.slice(2 * COLS, 3 * COLS);
  const cold = l.energy.slice(3 * COLS, 4 * COLS);
  if (last) {
    for (let c = 0; c < COLS; c++) {
      const fromHot = hot[c] - last.hot[c];
      const toCold = -(cold[c] - last.cold[c]);
      qs.push(0.5 * (fromHot + toCold));
    }
    analyse();
  }
  last = { hot, cold };
}

function walls(sim: DiscSim) {
  const hot = cssColor("--species-3");
  const cold = cssColor("--species-2");
  const r: number[] = [0, -0.4, COLS * SIDE, 0.25, ...hot, 0, 2 * SIDE - 0.25, COLS * SIDE, 2 * SIDE + 0.4, ...cold];
  const line = cssColor("--box-line");
  for (let c = 0; c <= COLS; c++) r.push(c * SIDE - 0.08, 0, c * SIDE + 0.08, 2 * SIDE, ...line);
  sim.set_overlay(new Float32Array(r));
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "heatflow",
  options: () => ({ columns: COLS, per_box: 12, log2_width: 3, hot: th(), cold: TC, seed: 4 }),
  names: ["molecules"],
  colors: ["--cream"],
  stepsPerFrame: () => 50,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    sim.set_view(0, -0.6, 6 * SIDE, 2 * SIDE + 0.6);
    sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3")]));
    sim.color_by_speed(2.5);
    walls(sim);
    clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("clear").onclick = clear;
windowSel.onchange = clear;
thInput.oninput = () => ($("thv").textContent = thInput.value);
thInput.onchange = () => {
  host.sim?.set_wall_temperature(2, th());
  clear();
};

declare global {
  interface Window {
    heatflowTest?: { ready: Promise<void>; run(windows: number): Promise<{ n: number; negative: number; slope: number; predicted: number }> };
  }
}

window.heatflowTest = {
  ready: host.ready,
  async run(windows: number) {
    await host.ready;
    clear();
    await host.exclusive(async (sim) => {
      for (let k = 0; k <= windows * (windowSteps() / 50); k++) {
        sim.step(50);
        await readout(sim);
      }
    });
    const neg = qs.filter((q) => q < 0).length / qs.length;
    return { n: qs.length, negative: neg, slope: Number($("slope").textContent), predicted: predicted() };
  },
};

// ---- 1. Colour means speed: a small gas to poke at. ----
const speedColors = (sim: DiscSim, vmax: number) => {
  sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3")]));
  sim.color_by_speed(vmax);
};
const hostA = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasA"),
  overlay: $("overlayA"),
  scenario: "boltzmann",
  options: () => ({ n: 600, density: 0.3, temperature: 1, seed: 8 }),
  names: ["molecules", "molecules"],
  label: "colour means speed",
  colors: ["--cream", "--cream"],
  stepsPerFrame: () => 4,
  onCreate: (sim) => speedColors(sim, 3),
});
new Tools({ host: hostA, container: $("toolsA") });
$("runA").onclick = () => {
  hostA.running = !hostA.running;
  $("runA").textContent = hostA.running ? "Pause" : "Run";
};

// ---- 2. What the walls do: departures from each wall, by speed. ----
const HOT_B = 2.5;
const COLD_B = 0.4;
const H_B = 32; // σ: the box is 16 σ wide and 32 σ tall
const SPEED_BINS = 24;
const VMAX_B = 4;
let departures = { hot: [] as number[], cold: [] as number[] };
let prevY: Float32Array | null = null;
let prevUp: boolean[] = [];

const depPlot = new LinesPlot($<HTMLCanvasElement>("departures"), {
  xLabel: "speed leaving the wall (σ/τ)",
  yLabel: "share of departures",
  yMin: 0,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

/** Speeds of molecules leaving a wall at kT: p(v) ∝ v² e^(−v²/2kT) in 2D (flux-weighted Maxwell). */
const leaving = (v: number, t: number) => (v * v * Math.exp((-v * v) / (2 * t))) / (t ** 1.5 * Math.sqrt(Math.PI / 2));

function drawDepartures() {
  const w = VMAX_B / SPEED_BINS;
  const hist = (vs: number[]) => {
    const c = new Array(SPEED_BINS).fill(0);
    for (const v of vs) if (v < VMAX_B) c[Math.floor(v / w)]++;
    return c.map((x, i) => ({ x: (i + 0.5) * w, y: x / Math.max(1, vs.length) / w }));
  };
  const theory = (t: number) => Array.from({ length: 80 }, (_, i) => ({ x: ((i + 0.5) / 80) * VMAX_B, y: leaving(((i + 0.5) / 80) * VMAX_B, t) }));
  depPlot.set([
    { points: theory(HOT_B), color: cssVar("--muted"), dashed: true },
    { points: theory(COLD_B), color: cssVar("--muted"), dashed: true },
    { points: hist(departures.hot), color: cssVar("--species-3"), dots: true },
    { points: hist(departures.cold), color: cssVar("--species-2"), dots: true },
  ]);
  $("nhot").textContent = String(departures.hot.length);
  $("ncold").textContent = String(departures.cold.length);
  if (departures.hot.length > 20 && departures.cold.length > 20) {
    const med = [...departures.hot].sort((a, b) => a - b)[Math.floor(departures.hot.length / 2)];
    const faster = departures.cold.filter((v) => v > med).length / departures.cold.length;
    $("overlap").textContent = `${(100 * faster).toFixed(1)}%`;
  }
}

/** A molecule near a wall that has just turned round has left it: record its speed. */
async function readoutB(sim: DiscSim) {
  const [{ pos }, { ke }] = (await Promise.all([sim.all_positions(), sim.kinetic_energies()])) as [{ pos: Float32Array }, { ke: Float32Array }];
  const n = ke.length;
  const y = new Float32Array(n);
  for (let i = 0; i < n; i++) y[i] = pos[2 * i + 1];
  if (prevY) {
    for (let i = 0; i < n; i++) {
      const up = y[i] > prevY[i];
      if (up !== prevUp[i]) {
        const v = Math.sqrt(2 * ke[i]);
        if (up && y[i] < 1.5) departures.hot.push(v);
        else if (!up && y[i] > H_B - 1.5) departures.cold.push(v);
      }
      prevUp[i] = up;
    }
  }
  prevY = y;
  drawDepartures();
}

const hostB = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "heatflow",
  options: () => ({ columns: 1, per_box: 40, log2_width: 4, hot: HOT_B, cold: COLD_B, seed: 9 }),
  names: ["molecules"],
  label: "what the walls do",
  colors: ["--cream"],
  // Few steps per frame: a molecule moves under 0.1 σ between readings, so a turn at a wall is
  // seen near the wall.
  stepsPerFrame: () => 8,
  afterSteps: (sim) => readoutB(sim),
  onCreate: (sim) => {
    speedColors(sim, 3.5);
    const hot = cssColor("--species-3");
    const cold = cssColor("--species-2");
    sim.set_overlay(new Float32Array([0, -0.5, 16, 0.3, ...hot, 0, H_B - 0.3, 16, H_B + 0.5, ...cold]));
    sim.set_view(-4, -0.6, 20, H_B + 0.6);
    departures = { hot: [], cold: [] };
    prevY = null;
    prevUp = [];
  },
});
$("runB").onclick = () => {
  hostB.running = !hostB.running;
  $("runB").textContent = hostB.running ? "Pause" : "Run";
};

declare global {
  interface Window {
    wallsTest?: { ready: Promise<void>; run(frames: number): Promise<{ hot: number[]; cold: number[] }> };
  }
}

window.wallsTest = {
  ready: hostB.ready,
  async run(frames: number) {
    await hostB.ready;
    return hostB.exclusive(async (sim) => {
      for (let f = 0; f < frames; f++) {
        sim.step(8);
        await readoutB(sim);
      }
      return departures;
    });
  },
};
