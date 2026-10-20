import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const BINS = 80;
const tIn = $<HTMLInputElement>("t");
let box = [200, 160];
let sumV = 0;
let sumL = 0;
let frames = 0;
let lastP = NaN;
const points: { x: number; y: number }[] = [];

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "1 / T",
  yLabel: "ln (vapour pressure)",
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const temperature = () => Number(tIn.value);

function clear() {
  sumV = 0;
  sumL = 0;
  frames = 0;
}

function fit(): { slope: number; intercept: number } | null {
  if (points.length < 2) return null;
  const n = points.length;
  const mx = points.reduce((a, p) => a + p.x, 0) / n;
  const my = points.reduce((a, p) => a + p.y, 0) / n;
  const slope = points.reduce((a, p) => a + (p.x - mx) * (p.y - my), 0) / points.reduce((a, p) => a + (p.x - mx) ** 2, 0);
  return { slope, intercept: my - slope * mx };
}

function redraw() {
  const f = fit();
  const series = [{ points: points.slice(), color: cssVar("--accent"), dots: true }];
  if (f) {
    const xs = points.map((p) => p.x);
    const [a, b] = [Math.min(...xs), Math.max(...xs)];
    series.push({ points: [{ x: a, y: f.intercept + f.slope * a }, { x: b, y: f.intercept + f.slope * b }], color: cssVar("--plot-back"), dots: false });
    $("lat").textContent = `${(-f.slope).toFixed(2)} ε`;
  }
  plot.set(series);
}

async function readout(sim: DiscSim) {
  const h = (await sim.histogram()) as { all: Uint32Array };
  const dh = box[1] / BINS;
  const rho = Array.from(h.all, (c) => c / (dh * box[0]));
  // The surface: the first bin (from the bottom) where the density drops below half the liquid's.
  const liquidRho = rho.slice(3, 8).reduce((a, b) => a + b, 0) / 5;
  let surface = rho.findIndex((r, k) => k > 3 && r < 0.5 * liquidRho);
  if (surface < 0) surface = BINS - 1;
  // Vapour: from 6 σ above the surface to 6 σ below the ceiling.
  const lo = Math.min(BINS - 2, surface + Math.ceil(6 / dh));
  const hi = BINS - Math.ceil(6 / dh);
  let nv = 0;
  for (let k = lo; k < hi; k++) nv += h.all[k];
  const vapour = nv / ((hi - lo) * dh * box[0]);
  sumV += vapour;
  sumL += liquidRho;
  frames++;
  const rv = sumV / frames;
  lastP = rv * temperature();
  $("nv").textContent = String(nv);
  $("pv").textContent = lastP.toExponential(2);
  $("rl").textContent = (sumL / frames).toFixed(3);
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "boiling",
  options: () => ({ n: 6000, width: 200, height: 160, temperature: temperature(), seed: 13 }),
  colors: ["--species-2"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    box = Array.from(sim.box_size());
    clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("record").onclick = () => {
  if (frames < 20 || !(lastP > 0)) return;
  points.push({ x: 1 / temperature(), y: Math.log(lastP) });
  redraw();
  $("status").textContent = `${points.length} points recorded. Change the temperature, wait, and record again.`;
};
tIn.oninput = () => ($("tv").textContent = temperature().toFixed(3));
tIn.onchange = () => {
  host.sim?.set_thermostat(temperature(), 0.5);
  clear();
};

declare global {
  interface Window {
    boilingTest?: { ready: Promise<void>; measure(t: number, eq: number, frames: number): Promise<{ p: number; liquid: number }>; record(): number };
  }
}

window.boilingTest = {
  ready: host.ready,
  async measure(t: number, eq: number, n: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      tIn.value = String(t);
      sim.set_thermostat(t, 0.5);
      for (let k = 0; k < eq / 1000; k++) sim.step(1000);
      clear();
      for (let k = 0; k < n; k++) {
        sim.step(200);
        await readout(sim);
      }
      points.push({ x: 1 / t, y: Math.log(lastP) });
      redraw();
      return { p: lastP, liquid: sumL / frames };
    });
  },
  record() {
    const f = fit();
    return f ? -f.slope : NaN;
  },
};
