import { $, cssVar, SimHost } from "../../src/lib/host";
import { ScatterPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const CHAINS = 64;
const xIn = $<HTMLInputElement>("x");
const tIn = $<HTMLInputElement>("t");
let fSum = 0;
let fN = 0;
let lastT = 1;
const points: { x: number; y: number; e: number }[] = [];

const plot = new ScatterPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "temperature",
  yLabel: "pull on each end (points), proportional to T (line)",
  point: () => cssVar("--accent"),
  line: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

/** Kremer–Grest bond (FENE k = 30, R0 = 1.5) plus WCA: the force on the anchor along +x. */
function anchorForce(dx: number, dy: number): number {
  const r2 = dx * dx + dy * dy;
  let fr = -30 / (1 - Math.min(r2 / 2.25, 0.999999));
  if (r2 < 1.2599) {
    const s6 = 1 / (r2 * r2 * r2);
    fr += (24 * s6 * (2 * s6 - 1)) / r2;
  }
  return fr * dx;
}

async function readout(sim: DiscSim) {
  const anchors = (await sim.positions(3)) as Float32Array;
  const beads = (await sim.positions(2)) as Float32Array;
  const r = (await sim.observe()) as { temperature: number };
  // The pinned anchors carry no kinetic energy; rescale to the moving beads only.
  const n = sim.n();
  lastT = (r.temperature * (n - 1)) / (n - 2 * CHAINS - 1);
  let f = 0;
  for (let c = 0; c < anchors.length / 2; c++) f += -anchorForce(anchors[2 * c] - beads[2 * c], anchors[2 * c + 1] - beads[2 * c + 1]);
  fSum += f / (anchors.length / 2);
  fN++;
  $("f").textContent = (fSum / fN).toFixed(3);
  $("temp").textContent = lastT.toFixed(3);
}

function clear() {
  fSum = 0;
  fN = 0;
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "rubber",
  options: () => ({ chains: CHAINS, length: 40, stretch: Number(xIn.value), temperature: Number(tIn.value), seed: 10 }),
  names: ["beads", "left anchor", "end bead", "right anchor"],
  colors: ["--cream", "--species-3", "--cream", "--species-3"],
  stepsPerFrame: () => 25,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    // Show the top few chains.
    const b = sim.box_size();
    sim.set_view(0, b[1] - 6 * 16, b[0], b[1]);
    clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("record").onclick = () => {
  if (fN < 20) return;
  points.push({ x: lastT, y: fSum / fN, e: 0 });
  const slope = points.reduce((a, p) => a + p.x * p.y, 0) / points.reduce((a, p) => a + p.x * p.x, 0);
  plot.set(points, slope);
  $("status").textContent = `${points.length} points. The line is pull = ${slope.toFixed(3)} × T.`;
};
$("stretch").onclick = () => {
  const sim = host.sim;
  if (!sim) return;
  sim.set_thermostat(Number(tIn.value), 0);
  sim.set_species_velocity(3, 0.4, 0);
  host.running = true;
  $("status").textContent = "Heat bath off; pulling the right ends at 0.4 σ/τ. Watch the temperature.";
  window.setTimeout(() => {
    sim.set_species_velocity(3, 0, 0);
    $("status").textContent = "Stopped pulling. With no heat bath the chains stay warmer; Reset to start again.";
  }, 6000);
};
xIn.oninput = () => ($("xv").textContent = `${Math.round(100 * Number(xIn.value))}%`);
xIn.onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
tIn.oninput = () => ($("tv").textContent = Number(tIn.value).toFixed(2));
tIn.onchange = () => {
  host.sim?.set_thermostat(Number(tIn.value), 1);
  clear();
  $("status").textContent = "Temperature changed. The chains need about 100 τ (~20 s) to settle before the average means much.";
};

declare global {
  interface Window {
    rubberTest?: { ready: Promise<void>; pull(t: number, frames: number): Promise<number>; stretch(steps: number): Promise<[number, number]> };
  }
}

window.rubberTest = {
  ready: host.ready,
  async pull(t: number, frames: number) {
    await host.ready;
    tIn.value = String(t);
    await host.create();
    return host.exclusive(async (sim) => {
      // Chains of 40 beads take ~100 τ to settle into their stretched shapes.
      for (let k = 0; k < 8; k++) sim.step(2000);
      clear();
      for (let k = 0; k < frames; k++) {
        sim.step(50);
        await readout(sim);
      }
      return fSum / fN;
    });
  },
  async stretch(steps: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      await readout(sim);
      const t0 = lastT;
      sim.set_thermostat(Number(tIn.value), 0);
      sim.set_species_velocity(3, 0.4, 0);
      sim.step(steps);
      sim.set_species_velocity(3, 0, 0);
      sim.step(200);
      await readout(sim);
      return [t0, lastT];
    });
  },
};
