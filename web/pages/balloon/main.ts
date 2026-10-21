import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

/** The membrane's position (σ): 32 per unit of the box's scale. */
let WALL = 32;
let HEIGHT = 96;
const heavyIn = $<HTMLInputElement>("heavy");
let start = [0, 0];
let history: { t: number; f: [number, number] }[] = [];
let lastRatio = NaN;

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "time (τ)",
  yLabel: "share still inside",
  log: true,
  yMax: 1.05,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

function wall(sim: DiscSim) {
  const c = cssColor("--accent");
  const r: number[] = [];
  // Wall segments between pores (pores are 0.6 σ wide, every 16 σ).
  for (let y = 0; y < HEIGHT; y += 16) r.push(WALL - 0.3, y + 0.6, WALL + 0.3, y + 16, c[0], c[1], c[2], 1);
  sim.set_overlay(new Float32Array(r));
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const inside = [0, 0];
  for (let i = 0; i < species.length; i++) if (pos[2 * i] < WALL) inside[species[i]]++;
  if (!history.length) start = inside.slice();
  const f: [number, number] = [inside[0] / start[0], inside[1] / start[1]];
  const t = host.elapsed * sim.dt();
  history.push({ t, f });
  // Rates from the logarithm of the remaining fraction (the leak is exponential early on).
  if (f[0] < 0.97 && f[1] < 0.995) lastRatio = Math.log(f[0]) / Math.log(f[1]);
  $("nl").textContent = `${Math.round(100 * f[0])}%`;
  $("nh").textContent = `${Math.round(100 * f[1])}%`;
  $("ratio").textContent = Number.isFinite(lastRatio) ? lastRatio.toFixed(2) : "–";
  plot.set([
    { points: history.map((h) => ({ x: h.t, y: h.f[0] })), color: cssVar("--accent") },
    { points: history.map((h) => ({ x: h.t, y: h.f[1] })), color: cssVar("--species-2") },
  ]);
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "balloon",
  options: () => ({ scale: Number($<HTMLSelectElement>("nmol").value), heavy: Number(heavyIn.value), pore: 0.6, seed: 15 }),
  names: ["helium", "air"],
  colors: ["--cream", "--species-2"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    const [w, h] = Array.from(sim.box_size());
    WALL = w / 4;
    HEIGHT = h;
    history = [];
    lastRatio = NaN;
    $("graham").textContent = Math.sqrt(Number(heavyIn.value)).toFixed(2);
    wall(sim);
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("reset").onclick = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
heavyIn.oninput = () => {
  $("hv").textContent = heavyIn.value;
  $("mv").textContent = heavyIn.value;
};
heavyIn.onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};

declare global {
  interface Window {
    balloonTest?: { ready: Promise<void>; run(steps: number): Promise<{ ratio: number; graham: number }> };
  }
}

window.balloonTest = {
  ready: host.ready,
  async run(steps: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      for (let k = 0; k < steps / 1000; k++) {
        sim.step(1000);
        host.elapsed += 1000;
      }
      await readout(sim);
      return { ratio: lastRatio, graham: Math.sqrt(Number(heavyIn.value)) };
    });
  },
};

$("nmol").onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
