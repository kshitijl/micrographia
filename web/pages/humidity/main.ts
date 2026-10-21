import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// Humid air over a pool (scenario humid). Measured natively (crates/md/tests/humid.rs): cooled
// from kT 0.44 to 0.40, the vapour condenses to saturation (~0.023) within ~100 τ.

const W = 120;
const H = 100;
const DEPTH = 14;
/** Water molecules counted as vapour: in the air between these heights (σ). */
const Y0 = DEPTH + 8;
const Y1 = H - 4;
/** The cold glass: a strip this wide at the right wall, whose molecules lose GLASS of their
 * speed at each readout (the glass takes their heat). */
const STRIP = 6;
const GLASS = 0.15;

/** Saturated vapour density (the Gibbs-ensemble fit; scenarios::saturated_vapour_density). */
const sat = (t: number) => 0.023 * Math.exp(-4.07 * (1 / t - 1 / 0.4));
/** The temperature at which vapour density `rho` would be saturated. */
const dewPoint = (rho: number) => 1 / (1 / 0.4 - Math.log(rho / 0.023) / 4.07);

const tIn = $<HTMLInputElement>("t");
const satPlot = new LinesPlot($<HTMLCanvasElement>("sat"), { xLabel: "kT", yLabel: "water vapour per σ²", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const timePlot = new LinesPlot($<HTMLCanvasElement>("time"), { xLabel: "time (τ)", yLabel: "water vapour per σ²", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const curve = Array.from({ length: 41 }, (_, j) => ({ x: 0.38 + 0.002 * j, y: sat(0.38 + 0.002 * j) }));
let glass = false;
let breathing = false;
let line: { x: number; y: number }[] = [];
let satLine: { x: number; y: number }[] = [];
/** Recent vapour readings, averaged for the readouts (one reading is noisy). */
const recent: number[] = [];

const temperature = () => Number(tIn.value);
let fogNow = 0;
/** Clusters this big or bigger (molecules within 1.5 σ of each other) are droplets: fog. */
const DROPLET = 8;

/** Water molecules in the air (between Y0 and Y1, outside the glass's strip): those in small
 * clusters are vapour (a saturated vapour has dimers and trimers too), those in big ones fog. */
function airWater(pos: Float32Array, species: Uint32Array) {
  const ids: number[] = [];
  for (let i = 0; i < species.length; i++) if (species[i] === 0 && pos[2 * i + 1] > Y0 && pos[2 * i + 1] < Y1 && pos[2 * i] < W - STRIP) ids.push(i);
  const parent = ids.map((_, k) => k);
  const find = (k: number): number => (parent[k] === k ? k : (parent[k] = find(parent[k])));
  const cell = 1.5;
  const grid = new Map<number, number[]>();
  const key = (cx: number, cy: number) => cx * 1009 + cy;
  ids.forEach((i, k) => {
    const g = key(Math.floor(pos[2 * i] / cell), Math.floor(pos[2 * i + 1] / cell));
    if (!grid.has(g)) grid.set(g, []);
    grid.get(g)!.push(k);
  });
  ids.forEach((i, k) => {
    const [cx, cy] = [Math.floor(pos[2 * i] / cell), Math.floor(pos[2 * i + 1] / cell)];
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (const m of grid.get(key(cx + dx, cy + dy)) ?? []) {
          const j = ids[m];
          if (m > k && (pos[2 * j] - pos[2 * i]) ** 2 + (pos[2 * j + 1] - pos[2 * i + 1]) ** 2 < cell * cell) parent[find(m)] = find(k);
        }
  });
  const size = new Map<number, number>();
  ids.forEach((_, k) => size.set(find(k), (size.get(find(k)) ?? 0) + 1));
  let [vapour, fog] = [0, 0];
  ids.forEach((_, k) => ((size.get(find(k)) ?? 0) >= DROPLET ? fog++ : vapour++));
  return { vapour, fog };
}

function overlay(sim: DiscSim) {
  const g = cssColor("--plot-back");
  sim.set_overlay(new Float32Array(glass ? [W - 1.5, DEPTH + 3, W, H, g[0], g[1], g[2], 0.6] : []));
}

async function readout(sim: DiscSim) {
  if (glass) sim.kick_region(W - STRIP, DEPTH + 3, W + 1, H + 1, 1 - GLASS, 0, 0);
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const { vapour: count, fog: fogCount } = airWater(pos, species);
  const rho = count / ((W - STRIP) * (Y1 - Y0));
  fogNow = fogCount;
  recent.push(rho);
  if (recent.length > 20) recent.shift();
  const avg = recent.reduce((a, x) => a + x, 0) / recent.length;
  const t = temperature();
  const time = sim.steps() * sim.dt();
  line.push({ x: time, y: rho });
  satLine.push({ x: time, y: sat(t) });
  if (line.length > 2000) [line, satLine].forEach((l) => l.splice(0, 1));
  satPlot.set([
    { points: curve, color: cssVar("--muted") },
    { points: [{ x: t, y: avg }], color: cssVar("--accent"), dots: true },
  ]);
  timePlot.set([
    { points: satLine, color: cssVar("--muted"), dashed: true },
    { points: line, color: cssVar("--species-2") },
  ]);
  $("rv").textContent = avg.toFixed(4);
  $("rs").textContent = sat(t).toFixed(4);
  $("rh").textContent = `${Math.round((100 * avg) / sat(t))}%`;
  $("dew").textContent = avg > 0 ? `kT ${dewPoint(avg).toFixed(3)}` : "–";
  $("fog").textContent = fogNow ? `${fogNow} molecules` : "none";
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "humid",
  options: () => ({ width: W, height: H, depth: DEPTH, temperature: temperature(), seed: 2 }),
  names: ["water", "air"],
  colors: ["--species-2", "--muted"],
  stepsPerFrame: () => 50,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    line = [];
    satLine = [];
    recent.length = 0;
    overlay(sim);
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("glass").onclick = () => {
  glass = !glass;
  $("glass").textContent = glass ? "Take out the cold glass" : "Put in a cold glass";
  if (host.sim) overlay(host.sim);
};
$("breathe").onclick = () => {
  breathing = !breathing;
  $("breathe").classList.toggle("on", breathing);
  host.canvas.style.cursor = breathing ? "crosshair" : "";
};
tIn.oninput = () => {
  $("tv").textContent = temperature().toFixed(3);
  host.sim?.set_thermostat(temperature(), 0.1);
};
// The humidifier: air molecules within 6 σ of the click become water vapour.
host.canvas.addEventListener("pointerdown", async (e) => {
  const sim = host.sim;
  if (!breathing || !sim) return;
  const r = host.canvas.getBoundingClientRect();
  const [x0, y0, x1, y1] = Array.from(sim.view());
  const x = x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
  const y = y1 - ((e.clientY - r.top) / r.height) * (y1 - y0);
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const ids: number[] = [];
  for (let i = 0; i < species.length; i++) if (species[i] === 1 && (pos[2 * i] - x) ** 2 + (pos[2 * i + 1] - y) ** 2 < 36) ids.push(i);
  if (host.sim !== sim) return;
  sim.set_species(new Uint32Array(ids), 0);
  if (!host.running) sim.render();
});

declare global {
  interface Window {
    humidityTest?: { ready: Promise<void>; cool(from: number, to: number, steps: number): Promise<{ before: number; after: number }> };
  }
}

window.humidityTest = {
  ready: host.ready,
  /** Start again saturated at `from`, settle, cool the bath to `to`, run `steps`; the vapour
   * density before and after (averaged over 10 readings each). */
  async cool(from: number, to: number, steps: number) {
    tIn.value = String(from);
    $("tv").textContent = from.toFixed(3);
    await host.create();
    return host.exclusive(async (sim) => {
      const avg = async () => {
        let s = 0;
        for (let k = 0; k < 10; k++) {
          sim.step(1000);
          await readout(sim);
          s += line[line.length - 1].y / 10;
        }
        return s;
      };
      sim.step(10000);
      const before = await avg();
      tIn.value = String(to);
      $("tv").textContent = to.toFixed(3);
      sim.set_thermostat(to, 0.1);
      for (let k = 0; k < steps / 1000; k++) sim.step(1000);
      const after = await avg();
      sim.render();
      return { before, after };
    });
  },
};
