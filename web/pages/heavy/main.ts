import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import { EnergyPanel, Tools } from "../../src/lib/interact";
import type { DiscSim } from "../../src/lib/wasm";

const NAMES = ["light", "heavy", "ball"];
let keSum = [0, 0, 0];
let count = [0, 0, 0];
let trails: number[][] = [];
let masses = [1, 16, 400];

const opts = (yLabel: string) => ({
  labels: NAMES,
  yLabel,
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const keBars = new BarPlot($<HTMLCanvasElement>("ke"), opts("average kinetic energy / kT"));
const speedBars = new BarPlot($<HTMLCanvasElement>("speed"), opts("rms speed (σ/τ)"));

const ballMass = () => Math.round(10 ** Number($<HTMLInputElement>("ballmass").value));

function clear() {
  keSum = [0, 0, 0];
  count = [0, 0, 0];
}

const energy = new EnergyPanel($("energy"));

async function readout(sim: DiscSim) {
  await energy.sample(sim);
  const { ke, species } = (await sim.kinetic_energies()) as { ke: Float32Array; species: Uint32Array };
  const balls = (await sim.positions(2)) as Float32Array;
  let total = 0;
  for (let i = 0; i < ke.length; i++) {
    keSum[species[i]] += ke[i];
    count[species[i]]++;
    total += ke[i];
  }
  // The gas temperature, from all molecules (2 degrees of freedom each).
  const t = total / (ke.length - 1);
  const mean = keSum.map((s, k) => s / Math.max(1, count[k]));
  keBars.configure({ yMax: 1.6 });
  keBars.set(
    mean.map((m) => m / t),
    [1, 1, 1],
  );
  speedBars.set(
    mean.map((m, k) => Math.sqrt((2 * m) / masses[k])),
    masses.map((m) => Math.sqrt((2 * t) / m)),
  );
  // Trails of the balls.
  for (let b = 0; b < balls.length / 2; b++) {
    trails[b] ??= [];
    trails[b].push(balls[2 * b], balls[2 * b + 1]);
    if (trails[b].length > 1200) trails[b].splice(0, 2);
  }
  const c = cssColor("--species-3");
  const rects: number[] = [];
  const s = 0.35;
  for (const tr of trails) {
    for (let k = 0; k < tr.length; k += 2) rects.push(tr[k] - s, tr[k + 1] - s, tr[k] + s, tr[k + 1] + s, c[0], c[1], c[2], 0.25 + 0.5 * (k / tr.length));
  }
  sim.set_overlay(new Float32Array(rects));
  const ratio = mean[2] / mean[0];
  if (count[2] > 300) $("status").textContent = `Balls carry ${ratio.toFixed(2)}× the light molecules' average kinetic energy, while moving ${Math.sqrt(masses[2] / masses[0]).toFixed(0)}× slower.`;
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "heavy",
  options: () => ({ n: 6000, density: 0.4, heavy: 16, balls: 3, ball_size: 6, ball_mass: ballMass(), seed: 2 }),
  colors: ["--cream", "--species-2", "--species-3"],
  stepsPerFrame: () => 20,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    masses = Array.from(sim.masses());
    trails = [];
    clear();
    energy.clear();
  },
});

new Tools({ host, container: $("tools"), after: (sim) => readout(sim) });

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("clear").onclick = clear;
$("reset").onclick = () => void host.create();
$<HTMLInputElement>("ballmass").oninput = () => {
  $("bmv").textContent = String(ballMass());
  $("bm").textContent = String(ballMass());
};
$<HTMLInputElement>("ballmass").onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};

declare global {
  interface Window {
    heavyTest?: { ready: Promise<void>; run(frames: number): Promise<number[]> };
  }
}

window.heavyTest = {
  ready: host.ready,
  async run(frames: number) {
    await host.ready;
    clear();
    await host.exclusive(async (sim) => {
      for (let f = 0; f < frames; f++) {
        sim.step(50);
        await readout(sim);
      }
    });
    return keSum.map((s, k) => s / count[k]);
  },
};
