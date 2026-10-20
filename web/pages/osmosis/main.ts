import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot, ScatterPlot } from "../../src/lib/plot";
import { Tools } from "../../src/lib/interact";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const cIn = $<HTMLInputElement>("c");
const tIn = $<HTMLInputElement>("t");
const attract = $<HTMLInputElement>("attract");

let base: { imp: Float64Array; steps: number } | null = null;
let soluteCount = 0;
let box = [1, 1];
let piNow = NaN;
let cNow = NaN;
const recorded: { x: number; y: number; e: number }[] = [];

const plot = new ScatterPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "solute concentration c (per σ²)",
  yLabel: "Π / kT (points) and van 't Hoff (line)",
  point: () => cssVar("--accent"),
  line: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const temperature = () => Number(tIn.value);

async function readout(sim: DiscSim) {
  const l = (await sim.ledger(1)) as { impulse: Float64Array };
  if (!base) {
    base = { imp: l.impulse.slice(), steps: host.elapsed };
    return;
  }
  const t = (host.elapsed - base.steps) * sim.dt();
  if (t <= 0) return;
  const d = (w: number) => l.impulse[w] - base!.imp[w];
  const pi = (d(5) - d(4)) / (t * box[1]);
  const pl = d(0) / (t * box[1]);
  const pr = d(1) / (t * box[1]);
  piNow = pi;
  cNow = soluteCount / (box[1] * (box[0] / 2 - 0.5));
  $("pi").textContent = pi.toFixed(4);
  $("vth").textContent = (cNow * temperature()).toFixed(4);
  $("pl").textContent = pl.toFixed(3);
  $("pr").textContent = pr.toFixed(3);
  $("status").textContent = `Averaging over ${t.toFixed(0)} τ. Right wall minus left wall: ${(pr - pl).toFixed(4)}; push on the membrane: ${pi.toFixed(4)}.`;
}

function overlay(sim: DiscSim) {
  const c = cssColor("--accent");
  const r: number[] = [];
  const x = box[0] / 2;
  for (let y = 0; y < box[1]; y += 2) r.push(x - 0.15, y, x + 0.15, y + 1.2, c[0], c[1], c[2], 0.9);
  sim.set_overlay(new Float32Array(r));
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "osmosis",
  options: () => ({ n: 16000, density: 0.5, solute: Number(cIn.value), temperature: temperature(), attraction: attract.checked ? 1 : 0, seed: 8 }),
  names: ["water", "solute"],
  colors: ["--species-2", "--cream"],
  stepsPerFrame: () => 40,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    box = Array.from(sim.box_size());
    const h = (await sim.histogram()) as { marked: Uint32Array };
    soluteCount = h.marked.reduce((a, b) => a + b, 0);
    base = null;
    overlay(sim);
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
$("reset").onclick = restart;
$("record").onclick = () => {
  if (!Number.isFinite(piNow)) return;
  recorded.push({ x: cNow, y: piNow / temperature(), e: 0 });
  plot.set(recorded, 1);
};
cIn.oninput = () => ($("cv").textContent = `${Math.round(100 * Number(cIn.value))}%`);
cIn.onchange = restart;
attract.onchange = restart;
tIn.oninput = () => ($("tv").textContent = Number(tIn.value).toFixed(2));
tIn.onchange = () => {
  host.sim?.set_thermostat(temperature(), 0.5);
  base = null;
};

declare global {
  interface Window {
    osmosisTest?: { ready: Promise<void>; measure(steps: number): Promise<{ pi: number; c: number; t: number }> };
  }
}

window.osmosisTest = {
  ready: host.ready,
  async measure(steps: number) {
    await host.ready;
    await host.exclusive(async (sim) => {
      base = null;
      await readout(sim);
      for (let k = 0; k < steps / 1000; k++) {
        sim.step(1000);
        host.elapsed += 1000;
      }
      await readout(sim);
    });
    return { pi: piNow, c: cNow, t: temperature() };
  },
};

// ---- 1. A membrane that can move. ----
const W_A = 96;
const H_A = 48;
let trackA: { x: number; y: number }[] = [];
const plotA = new LinesPlot($<HTMLCanvasElement>("plotA"), { xLabel: "time (τ)", yLabel: "membrane position (σ)", yMin: 0, yMax: W_A, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

async function readoutA(sim: DiscSim) {
  const beads = (await sim.positions(2)) as Float32Array;
  let x = 0;
  for (let k = 0; k < beads.length; k += 2) x += beads[k];
  x /= beads.length / 2;
  const t = sim.steps() * sim.dt();
  trackA.push({ x: t, y: x });
  plotA.set([{ points: trackA, color: cssVar("--accent") }]);
  $("xm").textContent = `${x.toFixed(1)} σ`;
  $("tA").textContent = t.toFixed(0);
}

const hostA = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasA"),
  overlay: $("overlayA"),
  scenario: "osmosis_free",
  options: () => ({ width: W_A, height: H_A, solute: 0.5, seed: 21 }),
  names: ["water", "solute", "membrane beads"],
  label: "a membrane that can move",
  colors: ["--species-2", "--cream", "--accent"],
  stepsPerFrame: () => 60,
  afterSteps: (sim) => readoutA(sim),
  onCreate: async (sim) => {
    trackA = [];
    await readoutA(sim);
  },
});
new Tools({ host: hostA, container: $("toolsA"), after: (sim) => readoutA(sim) });
$("runA").onclick = () => {
  hostA.running = !hostA.running;
  $("runA").textContent = hostA.running ? "Pause" : "Run";
};
$("resetA").onclick = () => void hostA.create().then(() => ($("runA").textContent = "Run"));

// ---- 2. Water climbing a tube (an osmometer). ----
const W_B = 32;
const H_B = 112;
const G_B = 0.006;
const T_B = 0.5;
let trackB: { x: number; y: number }[] = [];
let predB: { x: number; y: number }[] = [];
const plotB = new LinesPlot($<HTMLCanvasElement>("plotB"), { xLabel: "time (τ)", yLabel: "Δh (σ)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
let levels = { l: NaN, r: NaN, pred: NaN, smooth: NaN };
let rawB: number[] = [];

/** The surface height of the liquid in x ∈ [x0, x1): the top of its density profile (1 σ bins,
 * smoothed over 3) where it falls through 0.35 (vapour ~0.01, liquid ~0.7); and the density in
 * the lower half of the column. As in crates/md/tests/osmosis_demos.rs. */
function surface(pos: Float32Array, species: Uint32Array, x0: number, x1: number): [number, number] {
  const c = new Float64Array(H_B);
  for (let i = 0; i < species.length; i++) {
    const [x, y] = [pos[2 * i], pos[2 * i + 1]];
    if (species[i] !== 2 && x >= x0 && x < x1 && y >= 0 && y < H_B) c[Math.floor(y)] += 1 / (x1 - x0);
  }
  const sm = Array.from(c, (_, k) => (c[Math.max(0, k - 1)] + c[k] + c[Math.min(H_B - 1, k + 1)]) / 3);
  let top = H_B - 1;
  while (top > 0 && sm[top] <= 0.35) top--;
  const y = top + 0.5 + (top + 1 < H_B ? (sm[top] - 0.35) / Math.max(1e-9, sm[top] - sm[top + 1]) : 0);
  const lo = Math.max(3, Math.floor(top / 2));
  let rho = 0;
  for (let k = 2; k < lo; k++) rho += sm[k];
  return [y, rho / (lo - 2)];
}

/** Levels in each arm, the height difference (smoothed over the last ~100 readings) and van 't
 * Hoff's Δh = c kT / (ρ g). */
async function readoutB(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const half = W_B / 2;
  const [hl, rl] = surface(pos, species, 2, half - 1);
  const [hr, rr] = surface(pos, species, half + 1, W_B - 2);
  let sol = 0;
  for (let i = 0; i < species.length; i++) if (species[i] === 1 && pos[2 * i + 1] < hr + 3) sol++;
  const c = sol / (hr * (half - 1));
  const pred = (c * T_B) / (0.5 * (rl + rr) * G_B);
  const t = sim.steps() * sim.dt();
  rawB.push(hr - hl);
  if (rawB.length > 100) rawB.shift();
  const smooth = rawB.reduce((a, b) => a + b, 0) / rawB.length;
  levels = { l: hl, r: hr, pred, smooth };
  trackB.push({ x: t, y: smooth });
  predB.push({ x: t, y: pred });
  plotB.set([
    { points: predB, color: cssVar("--muted"), dashed: true },
    { points: trackB, color: cssVar("--accent") },
  ]);
  $("hl").textContent = `${hl.toFixed(1)} σ`;
  $("hr").textContent = `${hr.toFixed(1)} σ`;
  $("dh").textContent = `${smooth.toFixed(1)} σ`;
  $("dhp").textContent = `${pred.toFixed(1)} σ`;
  // The membrane, and a mark at each arm's level.
  const m = cssColor("--accent");
  const mark = cssColor("--highlight");
  const ov: number[] = [];
  for (let y = 0; y < H_B; y += 2) ov.push(half - 0.15, y, half + 0.15, y + 1.2, ...m);
  ov.push(0, hl - 0.15, half - 0.5, hl + 0.15, ...mark, half + 0.5, hr - 0.15, W_B, hr + 0.15, ...mark);
  sim.set_overlay(new Float32Array(ov));
}

const hostB = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "osmometer",
  options: () => ({ width: W_B, height: H_B, gravity: G_B, temperature: T_B, solute: 0.3, seed: 22 }),
  names: ["water", "solute", "wall beads"],
  label: "water climbing a tube",
  colors: ["--species-2", "--cream", "--muted"],
  stepsPerFrame: () => Number($<HTMLInputElement>("speedB").value),
  afterSteps: (sim) => readoutB(sim),
  onCreate: async (sim) => {
    sim.set_view(0, 0, W_B, 64);
    trackB = [];
    predB = [];
    rawB = [];
    await readoutB(sim);
  },
});
$("runB").onclick = () => {
  hostB.running = !hostB.running;
  $("runB").textContent = hostB.running ? "Pause" : "Run";
};
$("resetB").onclick = () => void hostB.create().then(() => ($("runB").textContent = "Run"));
$<HTMLInputElement>("speedB").oninput = () => ($("speedBv").textContent = `${$<HTMLInputElement>("speedB").value} steps/frame`);

declare global {
  interface Window {
    osmosisDemos?: {
      ready: Promise<void>;
      membrane(steps: number): Promise<{ start: number; end: number }>;
      tube(steps: number): Promise<{ dh: number[]; pred: number }>;
    };
  }
}

window.osmosisDemos = {
  ready: Promise.all([hostA.ready, hostB.ready]).then(() => undefined),
  async membrane(steps: number) {
    await hostA.ready;
    return hostA.exclusive(async (sim) => {
      const start = trackA[trackA.length - 1].y;
      for (let k = 0; k < steps / 2000; k++) {
        sim.step(2000);
        await readoutA(sim);
      }
      return { start, end: trackA[trackA.length - 1].y };
    });
  },
  async tube(steps: number) {
    await hostB.ready;
    return hostB.exclusive(async (sim) => {
      const dh: number[] = [];
      for (let k = 0; k < steps / 2000; k++) {
        sim.step(2000);
        await readoutB(sim);
        dh.push(levels.smooth);
      }
      return { dh, pred: levels.pred };
    });
  },
};
