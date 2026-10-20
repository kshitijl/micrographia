import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, TimePlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const N = 20000;
const BINS = 40;
const kSel = $<HTMLSelectElement>("k");
const view = $<HTMLSelectElement>("view");

let counts = new Float64Array(BINS);
let samples = 0;
let red = 0;
let tMean = 1;

const bars = new BarPlot($<HTMLCanvasElement>("hist"), {
  labels: [],
  yLabel: "probability density",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const trace = new TimePlot($<HTMLCanvasElement>("trace"), {
  yMin: 0,
  yMax: 6,
  yLabel: "red molecule's energy / kT",
  xLabel: "time (τ)",
  colors: () => [cssVar("--highlight")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

/** ln Γ(x) (Lanczos), for the gamma density. */
function lnGamma(x: number): number {
  const g = [676.5203681218851, -1259.1392167224028, 771.3234287776531, -176.6150291621406, 12.507343278686905, -0.13857109526572012, 9.984369578019572e-6, 1.5056327351493116e-7];
  if (x < 0.5) return Math.log(Math.PI / Math.sin(Math.PI * x)) - lnGamma(1 - x);
  x -= 1;
  let a = 0.9999999999998099;
  const t = x + 7.5;
  for (let i = 0; i < 8; i++) a += g[i] / (x + i + 1);
  return 0.5 * Math.log(2 * Math.PI) + (x + 0.5) * Math.log(t) - t + Math.log(a);
}

/** The histogram's x range, in units of kT (energy per molecule) or √(kT/m) (speed). */
function range(): [number, number] {
  const k = Number(kSel.value);
  if (view.value === "speed") return [0, 4];
  if (k === 1) return [0, 6];
  const w = 4.5 / Math.sqrt(k);
  return [Math.max(0, 1 - w), 1 + w];
}

/** Theoretical density at x (same units as `range`). */
function theory(x: number): number {
  const k = Number(kSel.value);
  if (view.value === "speed") return x * Math.exp(-0.5 * x * x); // 2D Maxwell, v in √(kT/m)
  // Energy per molecule e = E/k of a group of k: Gamma(k, kT/k).
  if (x <= 0) return k === 1 ? 1 : 0;
  return Math.exp(k * Math.log(k) + (k - 1) * Math.log(x) - k * x - lnGamma(k));
}

function clear() {
  counts = new Float64Array(BINS);
  samples = 0;
  $("samples").textContent = "0";
}

function redraw() {
  const [lo, hi] = range();
  const w = (hi - lo) / BINS;
  const total = counts.reduce((a, b) => a + b, 0) || 1;
  const measured = Array.from(counts, (c) => c / total / w);
  const th = Array.from({ length: BINS }, (_, i) => theory(lo + (i + 0.5) * w));
  const labels = Array.from({ length: BINS }, (_, i) => (lo + i * w).toFixed(view.value === "speed" || Number(kSel.value) === 1 ? 1 : 2));
  bars.configure({ log: view.value === "log", xLabel: view.value === "speed" ? "speed / √(kT/m)" : "energy per molecule / kT" });
  bars.set(measured, th, labels);
}

async function readout(sim: DiscSim, t: number) {
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const pe = (await sim.potential_energy()) as number;
  let sum = 0;
  for (let i = 0; i < ke.length; i++) sum += ke[i];
  tMean = sum / (ke.length - 1);
  const k = Number(kSel.value);
  const [lo, hi] = range();
  const w = (hi - lo) / BINS;
  const add = (x: number) => {
    const b = Math.floor((x - lo) / w);
    if (b >= 0 && b < BINS) counts[b]++;
  };
  if (view.value === "speed") {
    for (let i = 0; i < ke.length; i++) add(Math.sqrt((2 * ke[i]) / tMean));
  } else {
    for (let g = 0; g + k <= ke.length; g += k) {
      let e = 0;
      for (let i = g; i < g + k; i++) e += ke[i];
      add(e / k / tMean);
    }
  }
  samples += view.value === "speed" ? ke.length : Math.floor(ke.length / k);
  $("samples").textContent = samples.toLocaleString();
  $("temp").textContent = tMean.toFixed(4);
  $("etot").textContent = (sum + pe).toFixed(1);
  $("eone").textContent = (ke[red] / tMean).toFixed(2) + " kT";
  trace.add(t, ke[red] / tMean);
  trace.draw();
  redraw();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "boltzmann",
  options: () => ({ n: N, density: 0.5, temperature: 1, seed: 5 }),
  colors: ["--species-2", "--species-2"],
  stepsPerFrame: () => 10,
  afterSteps: (sim) => readout(sim, host.elapsed * sim.dt()),
  onCreate: (sim) => {
    red = sim.central(1);
    sim.set_highlight(red);
    clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("clear").onclick = clear;
kSel.onchange = () => {
  $("kv").textContent = kSel.selectedOptions[0].text;
  clear();
};
view.onchange = clear;

declare global {
  interface Window {
    boltzmannTest?: { ready: Promise<void>; collect(frames: number, k: number, v: string): Promise<{ measured: number[]; theory: number[] }> };
  }
}

window.boltzmannTest = {
  ready: host.ready,
  async collect(frames: number, k: number, v: string) {
    await host.ready;
    kSel.value = String(k);
    view.value = v;
    clear();
    await host.exclusive(async (sim) => {
      for (let f = 0; f < frames; f++) {
        sim.step(20);
        await readout(sim, 0);
      }
    });
    const [lo, hi] = range();
    const w = (hi - lo) / BINS;
    const total = counts.reduce((a, b) => a + b, 0);
    return {
      measured: Array.from(counts, (c) => c / total / w),
      theory: Array.from({ length: BINS }, (_, i) => theory(lo + (i + 0.5) * w)),
    };
  },
};
