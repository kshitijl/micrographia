import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, ScatterPlot } from "../../src/lib/plot";
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
