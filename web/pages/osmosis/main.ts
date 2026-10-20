import { $, cssVar, SimHost } from "../../src/lib/host";
import { ScatterPlot } from "../../src/lib/plot";
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
