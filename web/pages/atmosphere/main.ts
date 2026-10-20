import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const BINS = 20;
const heavyIn = $<HTMLInputElement>("heavy");
const bath = $<HTMLInputElement>("bath");
let counts = [new Float64Array(BINS), new Float64Array(BINS)];
let ke = new Float64Array(BINS);
let keN = new Float64Array(BINS);
let frames = 0;
let box = [1, 1];
let g = 0;
let tMean = 1;

const density = new LinesPlot($<HTMLCanvasElement>("density"), {
  xLabel: "height (σ)",
  yLabel: "density",
  log: true,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const tplot = new LinesPlot($<HTMLCanvasElement>("temp"), {
  xLabel: "height (σ)",
  yLabel: "temperature",
  yMin: 0,
  yMax: 2,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

function clear() {
  counts = [new Float64Array(BINS), new Float64Array(BINS)];
  ke = new Float64Array(BINS);
  keN = new Float64Array(BINS);
  frames = 0;
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const { ke: k } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const masses = Array.from(sim.masses());
  const dh = box[1] / BINS;
  let total = 0;
  for (let i = 0; i < species.length; i++) {
    const b = Math.min(BINS - 1, Math.floor(pos[2 * i + 1] / dh));
    counts[species[i]][b]++;
    ke[b] += k[i];
    keN[b]++;
    total += k[i];
  }
  frames++;
  tMean = total / species.length;
  const area = dh * box[0] * frames;
  const meas = [0, 1].map((s) => Array.from(counts[s], (c, b) => ({ x: (b + 0.5) * dh, y: c / area })));
  // Theory: exponential with the measured temperature, normalized to the same number of molecules
  // (ignoring the thin layers at the soft floor and ceiling).
  const theory = [0, 1].map((s) => {
    const n = counts[s].reduce((a, c) => a + c, 0) / frames;
    const lam = (masses[s] * g) / tMean;
    const z = (1 - Math.exp(-lam * box[1])) / lam;
    return Array.from({ length: BINS }, (_, b) => ({ x: (b + 0.5) * dh, y: (n / box[0] / z) * Math.exp(-lam * (b + 0.5) * dh) }));
  });
  density.set([
    { points: theory[0], color: cssVar("--muted"), dashed: true },
    { points: theory[1], color: cssVar("--muted"), dashed: true },
    { points: meas[0], color: cssVar("--accent"), dots: true },
    { points: meas[1], color: cssVar("--species-3"), dots: true },
  ]);
  tplot.set([
    { points: Array.from(ke, (e, b) => ({ x: (b + 0.5) * dh, y: keN[b] > 50 ? e / keN[b] : NaN })), color: cssVar("--accent"), dots: true },
    { points: [{ x: 0, y: tMean }, { x: box[1], y: tMean }], color: cssVar("--muted"), dashed: true },
  ]);
  const top = Array.from(ke, (e, b) => (keN[b] > 50 ? e / keN[b] : NaN)).filter(Number.isFinite);
  $("status").textContent = `${frames} snapshots. Temperature from the lowest to the highest well-populated layer: ${top[0]?.toFixed(3)} → ${top[top.length - 1]?.toFixed(3)}.`;
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "atmosphere",
  options: () => ({ n: 6000, heavy: Number(heavyIn.value), heavy_share: 0.3, scale: 0.35, temperature: 1, bath: bath.checked ? 1 : 0, seed: 11 }),
  names: ["light gas", "heavy gas"],
  colors: ["--cream", "--species-3"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    box = Array.from(sim.box_size());
    g = 1 / (0.35 * box[1]);
    clear();
  },
});

function restart() {
  const run = host.running;
  void host.create().then(() => (host.running = run));
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("clear").onclick = clear;
$("reset").onclick = restart;
heavyIn.oninput = () => {
  $("hv").textContent = heavyIn.value;
  $("mv").textContent = heavyIn.value;
};
heavyIn.onchange = restart;
bath.onchange = restart;

declare global {
  interface Window {
    atmosphereTest?: { ready: Promise<void>; run(n: number): Promise<{ slopes: number[]; predicted: number[]; tBottom: number; tTop: number }> };
  }
}

window.atmosphereTest = {
  ready: host.ready,
  async run(n: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      clear();
      for (let k = 0; k < n; k++) {
        sim.step(100);
        await readout(sim);
      }
      const masses = Array.from(sim.masses());
      const dh = box[1] / BINS;
      // Fit ln density against height over the middle bins.
      const slopes = [0, 1].map((s) => {
        const pts = Array.from(counts[s], (c, b) => ({ x: (b + 0.5) * dh, y: Math.log(Math.max(c, 1)) })).slice(2, 14);
        const mx = pts.reduce((a, p) => a + p.x, 0) / pts.length;
        const my = pts.reduce((a, p) => a + p.y, 0) / pts.length;
        return pts.reduce((a, p) => a + (p.x - mx) * (p.y - my), 0) / pts.reduce((a, p) => a + (p.x - mx) ** 2, 0);
      });
      const t = Array.from(ke, (e, b) => e / keN[b]);
      return { slopes, predicted: masses.map((m) => (-m * g) / tMean), tBottom: t[1], tTop: t[12] };
    });
  },
};
