import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { Spoon } from "../../src/lib/interact";
import { LinesPlot } from "../../src/lib/plot";

// A salt crystal in water (scenario solubility, salt ε = 3, hydration ε = 1.3). Measured natively
// (crates/md/tests/solubility.rs): after 2,000 τ about 63 of 256 ions dissolved at kT 0.7, 155 at
// 0.9 (still rising) and 171 at 1.1.

const tIn = $<HTMLInputElement>("t");
const plot = new LinesPlot($<HTMLCanvasElement>("plot"), { xLabel: "time (τ)", yLabel: "ions dissolved", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
let line: { x: number; y: number }[] = [];
let prev: { x: number; y: number }[] = [];
let lastRead = -Infinity;
let sprinkling = false;

/** Ions with no opposite ion within 1.4 σ, and the rest; and the water count. */
function count(pos: Float32Array, species: Uint32Array, box: number[]) {
  const ions: number[] = [];
  let water = 0;
  for (let i = 0; i < species.length; i++) (species[i] > 0 ? ions.push(i) : water++);
  const cell = 1.4;
  const grid = new Map<number, number[]>();
  const key = (x: number, y: number) => x * 4099 + y;
  const nc = [Math.floor(box[0] / cell), Math.floor(box[1] / cell)];
  const cellOf = (i: number) => [Math.min(nc[0] - 1, Math.floor(pos[2 * i] / cell)), Math.min(nc[1] - 1, Math.floor(pos[2 * i + 1] / cell))];
  for (const i of ions) {
    const [cx, cy] = cellOf(i);
    const k = key(cx, cy);
    if (!grid.has(k)) grid.set(k, []);
    grid.get(k)!.push(i);
  }
  let free = 0;
  for (const i of ions) {
    const [cx, cy] = cellOf(i);
    let paired = false;
    for (let dx = -1; dx <= 1 && !paired; dx++)
      for (let dy = -1; dy <= 1 && !paired; dy++)
        for (const j of grid.get(key((cx + dx + nc[0]) % nc[0], (cy + dy + nc[1]) % nc[1])) ?? []) {
          if (species[j] === species[i]) continue;
          let [ddx, ddy] = [Math.abs(pos[2 * j] - pos[2 * i]), Math.abs(pos[2 * j + 1] - pos[2 * i + 1])];
          ddx = Math.min(ddx, box[0] - ddx);
          ddy = Math.min(ddy, box[1] - ddy);
          if (ddx * ddx + ddy * ddy < 1.96) {
            paired = true;
            break;
          }
        }
    if (!paired) free++;
  }
  return { free, bound: ions.length - free, water };
}

async function readout(sim: DiscSim, force = false) {
  const now = performance.now();
  if (!force && now - lastRead < 400) return;
  lastRead = now;
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const c = count(pos, species, Array.from(sim.box_size()));
  const time = sim.steps() * sim.dt();
  line.push({ x: time, y: c.free });
  plot.set([
    { points: prev, color: cssVar("--muted") },
    { points: line, color: cssVar("--accent") },
  ]);
  $("diss").textContent = String(c.free);
  $("xtal").textContent = String(c.bound);
  $("conc").textContent = ((100 * c.free) / c.water).toFixed(2);
  $("time").textContent = `${time.toFixed(0)} τ`;
  return c;
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "solubility",
  options: () => ({ n: 6000, ions: 16, salt: 3, hydration: 1.3, temperature: Number(tIn.value), seed: 3 }),
  names: ["water", "cations", "anions"],
  colors: ["--species-2", "--species-3", "--cream"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim).then(() => {}),
  onCreate: async (sim) => {
    if (line.length) prev = line;
    line = [];
    await readout(sim, true);
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
tIn.oninput = () => {
  $("tv").textContent = Number(tIn.value).toFixed(2);
  host.sim?.set_thermostat(Number(tIn.value), 0.2);
};
new Spoon(host, $<HTMLButtonElement>("spoon"), { size: 10, taupersecond: () => (host.running ? 100 * 60 * 0.005 : 1) });
// Sprinkle: water molecules within 3 σ of the click become ions, alternating kinds (a pinch of
// salt, which then dissolves or joins the crystal).
$("sprinkle").onclick = () => {
  sprinkling = !sprinkling;
  $("sprinkle").classList.toggle("on", sprinkling);
  host.canvas.style.cursor = sprinkling ? "crosshair" : "";
};
host.canvas.addEventListener("pointerdown", async (e) => {
  const sim = host.sim;
  if (!sprinkling || !sim) return;
  const r = host.canvas.getBoundingClientRect();
  const [x0, y0, x1, y1] = Array.from(sim.view());
  const x = x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
  const y = y1 - ((e.clientY - r.top) / r.height) * (y1 - y0);
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const near: number[] = [];
  for (let i = 0; i < species.length; i++) if (species[i] === 0 && (pos[2 * i] - x) ** 2 + (pos[2 * i + 1] - y) ** 2 < 9) near.push(i);
  if (host.sim !== sim) return;
  sim.set_species(new Uint32Array(near.filter((_, k) => k % 2 === 0)), 1);
  sim.set_species(new Uint32Array(near.filter((_, k) => k % 2 === 1)), 2);
  void readout(sim, true);
});

declare global {
  interface Window {
    dissolvingTest?: { ready: Promise<void>; run(t: number, steps: number): Promise<number> };
  }
}

window.dissolvingTest = {
  ready: host.ready,
  /** Start again at kT `t`, run `steps`; ions dissolved. */
  async run(t: number, steps: number) {
    tIn.value = String(t);
    await host.create();
    return host.exclusive(async (sim) => {
      for (let k = 0; k < steps / 2000; k++) sim.step(2000);
      const c = await readout(sim, true);
      sim.render();
      return c!.free;
    });
  },
};
