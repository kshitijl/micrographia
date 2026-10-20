import { $, cssVar, SimHost } from "../../src/lib/host";
import { TimePlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const BEADS = 44;
// Mean area in matching saline, from long native runs (crates/md/tests/cell.rs).
const ISOTONIC = 255;
const outsideSel = $<HTMLSelectElement>("outside");
let areas: number[] = [];

const plot = new TimePlot($<HTMLCanvasElement>("plot"), {
  yMin: 100,
  yMax: 400,
  yLabel: "cell area (σ²)",
  xLabel: "time (τ)",
  colors: () => [cssVar("--accent")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

async function readout(sim: DiscSim) {
  const ring = (await sim.positions(2)) as Float32Array;
  let a = 0;
  for (let k = 0; k < BEADS; k++) {
    const j = (k + 1) % BEADS;
    a += ring[2 * k] * ring[2 * j + 1] - ring[2 * j] * ring[2 * k + 1];
  }
  a = Math.abs(a) / 2;
  areas.push(a);
  const recent = areas.slice(-40);
  const mean = recent.reduce((x, y) => x + y, 0) / recent.length;
  $("area").textContent = mean.toFixed(0);
  $("rel").textContent = `${mean >= ISOTONIC ? "+" : "−"}${Math.abs(Math.round((100 * (mean - ISOTONIC)) / ISOTONIC))}%`;
  plot.add(host.elapsed * sim.dt(), a);
  plot.draw();
  const out = Number(outsideSel.value);
  $("status").textContent = out === 0 ? "Pure water: water flows in and the cell swells." : out < 0.5 ? "Saline: in and out flows balance." : "Brine: water flows out and the cell shrinks.";
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "cell",
  options: () => ({ inside: 0.35, outside: Number(outsideSel.value), stiffness: 2, bend: 20, seed: 19 }),
  names: ["water", "salt", "membrane beads"],
  colors: ["--species-2", "--cream", "--accent"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: () => {
    areas = [];
    plot.clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
outsideSel.onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};

declare global {
  interface Window {
    salineTest?: { ready: Promise<void>; area(outside: number, steps: number): Promise<number> };
  }
}

window.salineTest = {
  ready: host.ready,
  async area(outside: number, steps: number) {
    outsideSel.value = String(outside);
    await host.create();
    return host.exclusive(async (sim) => {
      sim.step(20000);
      areas = [];
      for (let k = 0; k < steps / 1000; k++) {
        sim.step(1000);
        await readout(sim);
      }
      return areas.reduce((x, y) => x + y, 0) / areas.length;
    });
  },
};
