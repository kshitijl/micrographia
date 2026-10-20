import { $, cssVar, SimHost } from "../../src/lib/host";
import { TimePlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const TAIL = 3;
const LEN = TAIL + 1;
const tIn = $<HTMLInputElement>("t");
const epsIn = $<HTMLInputElement>("eps");
const headIn = $<HTMLInputElement>("head");
let box = [1, 1];
let lastAgg = 0;
let lastBig = 0;

const plot = new TimePlot($<HTMLCanvasElement>("plot"), {
  yMin: 0,
  yMax: 1,
  yLabel: "share in aggregates",
  xLabel: "time (τ)",
  colors: () => [cssVar("--accent")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

/** Aggregates: lipids whose tail beads are within 1.5 σ of another lipid's tails (union–find). */
function clusters(pos: Float32Array, species: Uint32Array): [number, number] {
  const tails: number[] = [];
  for (let i = 0; i < species.length; i++) if (species[i] === 1) tails.push(i);
  const nl = species.length / LEN;
  const parent = Array.from({ length: nl }, (_, i) => i);
  const find = (x: number): number => {
    while (parent[x] !== x) {
      parent[x] = parent[parent[x]];
      x = parent[x];
    }
    return x;
  };
  const touching = new Uint8Array(nl);
  const cell = 1.5;
  const gx = Math.floor(box[0] / cell);
  const gy = Math.floor(box[1] / cell);
  const grid: number[][] = Array.from({ length: gx * gy }, () => []);
  const cellOf = (i: number) => [Math.min(gx - 1, Math.floor((pos[2 * i] / box[0]) * gx)), Math.min(gy - 1, Math.floor((pos[2 * i + 1] / box[1]) * gy))];
  for (const i of tails) {
    const [cx, cy] = cellOf(i);
    grid[cy * gx + cx].push(i);
  }
  for (const i of tails) {
    const [cx, cy] = cellOf(i);
    for (let oy = -1; oy <= 1; oy++)
      for (let ox = -1; ox <= 1; ox++) {
        for (const j of grid[((cy + oy + gy) % gy) * gx + ((cx + ox + gx) % gx)]) {
          if (j <= i || Math.floor(j / LEN) === Math.floor(i / LEN)) continue;
          let dx = Math.abs(pos[2 * i] - pos[2 * j]);
          dx = Math.min(dx, box[0] - dx);
          let dy = Math.abs(pos[2 * i + 1] - pos[2 * j + 1]);
          dy = Math.min(dy, box[1] - dy);
          if (dx * dx + dy * dy < 2.25) {
            const a = Math.floor(i / LEN);
            const b = Math.floor(j / LEN);
            touching[a] = touching[b] = 1;
            parent[find(a)] = find(b);
          }
        }
      }
  }
  const size = new Map<number, number>();
  for (let l = 0; l < nl; l++) size.set(find(l), (size.get(find(l)) ?? 0) + 1);
  return [touching.reduce((a, b) => a + b, 0) / nl, Math.max(...size.values())];
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  [lastAgg, lastBig] = clusters(pos, species);
  $("agg").textContent = `${Math.round(100 * lastAgg)}%`;
  $("big").textContent = `${lastBig} lipids`;
  plot.add(host.elapsed * sim.dt(), lastAgg);
  plot.draw();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "lipids",
  options: () => ({ lipids: 800, tail: TAIL, density: 0.25, temperature: Number(tIn.value), attraction: Number(epsIn.value), head: Number(headIn.value), seed: 14 }),
  colors: ["--species-2", "--cream"],
  stepsPerFrame: () => 200,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    box = Array.from(sim.box_size());
    plot.clear();
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
tIn.oninput = () => ($("tv").textContent = Number(tIn.value).toFixed(2));
tIn.onchange = () => host.sim?.set_thermostat(Number(tIn.value), 0.5);
epsIn.oninput = () => ($("ev").textContent = Number(epsIn.value).toFixed(1));
epsIn.onchange = () => host.sim?.set_attraction(1, 1, Number(epsIn.value));
headIn.oninput = () => ($("hv").textContent = Number(headIn.value).toFixed(2));
headIn.onchange = restart;

declare global {
  interface Window {
    lipidsTest?: { ready: Promise<void>; run(steps: number): Promise<[number, number]> };
  }
}

window.lipidsTest = {
  ready: host.ready,
  async run(steps: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      for (let k = 0; k < steps / 5000; k++) sim.step(5000);
      await readout(sim);
      return [lastAgg, lastBig] as [number, number];
    });
  },
};
