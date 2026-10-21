import { FluidSim, GasSim, GibbsSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";

// ---- 1. The lattice gas (crates/lattice: Kawasaki Monte Carlo on the GPU). Natively its
// coexisting densities match Onsager's to ~2% (crates/lattice/tests/gas.rs). ----

const TC = 1 / (2 * Math.log(1 + Math.SQRT2));
const tIn = $<HTMLInputElement>("t");
const gIn = $<HTMLInputElement>("g");
const hotIn = $<HTMLInputElement>("hot");
const spf = $<HTMLInputElement>("spf");
/** Rows of the hot plate. */
const HOT_ROWS = 6;
let start = 2; // 0 slab, 1 pool, 2 a quench (random)
let tool: "dropper" | "sponge" | null = null;

/** Onsager's coexisting densities at kT = t, or null above the critical point. */
function onsager(t: number): [number, number] | null {
  const m4 = 1 - Math.sinh(0.5 / t) ** -4;
  if (m4 <= 0) return null;
  const m = m4 ** 0.125;
  return [(1 - m) / 2, (1 + m) / 2];
}

const pd = new LinesPlot($<HTMLCanvasElement>("pd"), { xLabel: "density (molecules per site)", yLabel: "kT", yMin: 0.25, yMax: 0.65, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const eplot = new LinesPlot($<HTMLCanvasElement>("eplot"), { xLabel: "sweeps", yLabel: "energy per molecule (ε)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const exact: { x: number; y: number }[] = [];
for (let t = 0.25; t < TC; t += 0.005) exact.push({ x: onsager(t)![0], y: t });
exact.push({ x: 0.5, y: TC });
for (let t = TC - 0.005; t >= 0.25; t -= 0.005) exact.push({ x: onsager(t)![1], y: t });
/** Measured densities, averaged per temperature (rounded to 0.005). */
const measured = new Map<number, { l: number; v: number; n: number }>();
const eLine: { x: number; y: number }[] = [];

const temperature = () => Number(tIn.value);

function drawPhaseDiagram() {
  const pts: { x: number; y: number }[] = [];
  for (const [t, m] of measured) {
    if (m.n < 3) continue;
    pts.push({ x: m.v / m.n, y: t }, { x: m.l / m.n, y: t });
  }
  pd.set([
    { points: exact, color: cssVar("--muted"), dashed: true },
    { points: pts, color: cssVar("--accent"), dots: true },
  ]);
}

async function readout(sim: GasSim) {
  const [rows, blocks] = (await Promise.all([sim.rows(), sim.blocks()])) as [{ molecules: Uint32Array; energy: number }, Uint32Array];
  const n = rows.molecules.reduce((a, b) => a + b, 0);
  // Blocks of 36 sites: fuller than ¾ count as liquid, emptier than ¼ as vapour.
  let [ls, ln, vs, vn] = [0, 0, 0, 0];
  for (const b of blocks) {
    if (b > 27) {
      ls += b;
      ln++;
    } else if (b < 9) {
      vs += b;
      vn++;
    }
  }
  const [rl, rv] = [ln > 0 ? ls / (36 * ln) : NaN, vn > 0 ? vs / (36 * vn) : NaN];
  // Average only well-separated phases (each with a good share of the box), once the sweeps at
  // this temperature have let the vapour settle (molecules diffuse ~1 site per sweep, so a few
  // thousand sweeps to cross the vapour between drops).
  const t = Math.round(temperature() / 0.005) * 0.005;
  const settled = sim.steps() - changedAt > 2000;
  if (settled && ln > 0.05 * blocks.length && vn > 0.05 * blocks.length && Number(gIn.value) === 0 && Number(hotIn.value) === 0) {
    const m = measured.get(t) ?? { l: 0, v: 0, n: 0 };
    m.l += rl;
    m.v += rv;
    m.n++;
    measured.set(t, m);
    drawPhaseDiagram();
  }
  eLine.push({ x: sim.steps(), y: rows.energy / Math.max(1, n) });
  if (eLine.length > 2000) eLine.splice(0, 1);
  eplot.set([{ points: eLine, color: cssVar("--accent") }]);
  $("rl").textContent = Number.isFinite(rl) && ln > 0.02 * blocks.length ? rl.toFixed(3) : "–";
  $("rv").textContent = Number.isFinite(rv) && vn > 0.02 * blocks.length ? rv.toFixed(4) : "–";
  $("e").textContent = `${(rows.energy / Math.max(1, n)).toFixed(3)} ε`;
  $("sweeps").textContent = sim.steps().toLocaleString();
}

let changedAt = 0;
let lastRead = 0;
const host = new SimHost<GasSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "gas",
  create: (c, name, o) => GasSim.create(c, name, o),
  options: () => {
    const side = Number($<HTMLSelectElement>("size").value);
    return { w: side, h: side, periodic: start === 1 ? 0 : 1, wet: 0, temperature: temperature(), gravity: Number(gIn.value), start, fill: start === 1 ? 0.3 : 0.35, seed: 1 };
  },
  names: [],
  colors: ["--species-2"],
  stepsPerFrame: () => Number(spf.value),
  // The readout reads a few thousand numbers back; a few times a second is plenty.
  afterSteps: async (sim) => {
    const now = performance.now();
    if (now - lastRead < 250) return;
    lastRead = now;
    await readout(sim);
  },
  energy: false,
  stepName: "sweeps",
  onCreate: async (sim) => {
    changedAt = 0;
    eLine.length = 0;
    applyHot(sim);
    await readout(sim);
  },
});

function applyHot(sim: GasSim) {
  const t = Number(hotIn.value);
  sim.set_hot_plate(t > 0 ? HOT_ROWS : 0, t);
  $("hotv").textContent = t > 0 ? `kT = ${t.toFixed(2)} (bottom ${HOT_ROWS} rows)` : "off";
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
const restartWith = (s: number) => {
  start = s;
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
$("startMix").onclick = () => restartWith(2);
$("startSlab").onclick = () => restartWith(0);
$("startPool").onclick = () => {
  // A pool needs a floor (walls) and gravity to hold it there.
  if (Number(gIn.value) === 0) {
    gIn.value = "0.01";
    showG();
  }
  restartWith(1);
};
$("size").onchange = () => restartWith(start);
tIn.oninput = () => {
  $("tv").textContent = temperature().toFixed(3);
  host.sim?.set_temperature(temperature());
  changedAt = host.sim?.steps() ?? 0;
};
const showG = () => ($("gv").textContent = Number(gIn.value) === 0 ? "off" : `${Number(gIn.value).toFixed(4)} ε per row`);
gIn.oninput = () => {
  showG();
  host.sim?.set_gravity(Number(gIn.value));
};
hotIn.oninput = () => host.sim && applyHot(host.sim);
spf.oninput = () => ($("spfv").textContent = spf.value);
showG();

// The dropper and the sponge: drag on the lattice to add or take away molecules.
const setTool = (t: typeof tool) => {
  tool = tool === t ? null : t;
  $("dropper").classList.toggle("on", tool === "dropper");
  $("sponge").classList.toggle("on", tool === "sponge");
  host.canvas.style.cursor = tool ? "crosshair" : "";
};
$("dropper").onclick = () => setTool("dropper");
$("sponge").onclick = () => setTool("sponge");
let painting = false;
const paintAt = (e: PointerEvent) => {
  const sim = host.sim;
  if (!sim || !tool) return;
  const r = host.canvas.getBoundingClientRect();
  const [x0, y0, x1, y1] = Array.from(sim.view());
  const x = x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
  const y = y1 - ((e.clientY - r.top) / r.height) * (y1 - y0);
  const radius = Math.max(4, 0.03 * sim.width());
  sim.paint(x, y, radius, tool === "dropper", 0.35);
  if (!host.running) sim.render();
};
host.canvas.addEventListener("pointerdown", (e) => {
  if (!tool) return;
  host.canvas.setPointerCapture(e.pointerId);
  painting = true;
  paintAt(e);
});
host.canvas.addEventListener("pointermove", (e) => painting && paintAt(e));
host.canvas.addEventListener("pointerup", () => (painting = false));

drawPhaseDiagram();

declare global {
  interface Window {
    coexistenceTest?: {
      ready: Promise<void>;
      measure(t: number, sweeps: number, side: number): Promise<{ liquid: number; vapour: number; exact: [number, number] | null }>;
      /** The fluid's temperature (in T_c) and density up the middle column, at these rows. */
      fluidColumn(rows: number[]): Promise<{ t: number[]; rho: number[] }>;
      /** Start the fluid as a pool with gravity `g` and the hot plate at `hot` T_c, run `steps`. */
      boil(hot: number, g: number, steps: number): Promise<void>;
      /** Gibbs ensemble: start `n` molecules at kT = t, run `cycles`, return the two densities
       * averaged over the second half. */
      gibbs(n: number, t: number, cycles: number): Promise<{ vapour: number; liquid: number }>;
    };
  }
}

window.coexistenceTest = {
  ready: host.ready,
  async gibbs(n: number, t: number, cycles: number) {
    $<HTMLSelectElement>("nC").value = String(n);
    tC.value = String(t);
    await hostC.create();
    return hostC.exclusive(async (h) => {
      h.g.run(cycles / 2);
      let [v, l, k] = [0, 0, 0];
      for (let s = 0; s < 50; s++) {
        h.g.run(cycles / 100);
        const [r0, , r1] = Array.from(h.g.measure());
        v += Math.min(r0, r1);
        l += Math.max(r0, r1);
        k++;
      }
      h.render();
      return { vapour: v / k, liquid: l / k };
    });
  },
  async boil(hot: number, g: number, steps: number) {
    gB.value = String(g);
    hotB.value = String(hot);
    startB = 0;
    await hostB.create();
    await hostB.exclusive(async (sim) => {
      sim.step(steps);
      await sim.sync();
      sim.render();
    });
  },
  async fluidColumn(rows: number[]) {
    return hostB.exclusive(async (sim) => {
      const { rho, temp } = (await sim.fields()) as { rho: Float32Array; temp: Float32Array };
      const [w, tc] = [sim.width(), sim.critical_temperature()];
      return { t: rows.map((y) => temp[y * w + w / 2] / tc), rho: rows.map((y) => rho[y * w + w / 2]) };
    });
  },
  /** Start from a slab at kT = t on a `side`² lattice, run `sweeps`, and average the densities
   * over the last half. */
  async measure(t: number, sweeps: number, side: number) {
    tIn.value = String(t);
    $<HTMLSelectElement>("size").value = String(side);
    start = 0;
    await host.create();
    return host.exclusive(async (sim) => {
      sim.step(sweeps / 2);
      let [l, v, k] = [0, 0, 0];
      for (let s = 0; s < 10; s++) {
        sim.step(sweeps / 20);
        const blocks = (await sim.blocks()) as Uint32Array;
        let [ls, ln, vs, vn] = [0, 0, 0, 0];
        for (const b of blocks) {
          if (b > 27) {
            ls += b;
            ln++;
          } else if (b < 9) {
            vs += b;
            vn++;
          }
        }
        l += ls / (36 * ln);
        v += vs / (36 * vn);
        k++;
      }
      sim.render();
      return { liquid: l / k, vapour: v / k, exact: onsager(t) };
    });
  },
};

// ---- 2. Lattice Boltzmann (crates/lattice: Shan–Chen with a Carnahan–Starling equation of
// state). Natively its liquid density matches the model's prediction to 0.2%. ----

const tB = $<HTMLInputElement>("tB");
const gB = $<HTMLInputElement>("gB");
const hotB = $<HTMLInputElement>("hotB");
const spfB = $<HTMLInputElement>("spfB");
let startB = 1; // 0 a pool, 1 a quench, 2 a drop
let toolB: "drop" | "bubble" | null = null;
const plotB = new LinesPlot($<HTMLCanvasElement>("plotB"), { xLabel: "steps", yLabel: "liquid share", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const fLine: { x: number; y: number }[] = [];
let lastReadB = 0;

async function readoutB(sim: FluidSim) {
  const { rho } = (await sim.fields()) as { rho: Float32Array };
  const [rv, rl] = Array.from(sim.coexistence());
  const mid = 0.5 * (rv + rl);
  const w = sim.width();
  let [ls, ln, vs, vn] = [0, 0, 0, 0];
  // Fluid sites only (rows 1..h−2); "deep" liquid and vapour, away from interfaces.
  for (let i = w; i < rho.length - w; i++) {
    const r = rho[i];
    if (r > mid) {
      ln++;
      if (r > 0.9 * rl) ls += r;
    } else {
      vn++;
      if (r < 2 * rv) vs += r;
    }
  }
  const deepL = Array.from(rho).filter((r) => r > 0.9 * rl).length;
  const deepV = Array.from(rho.subarray(w, rho.length - w)).filter((r) => r < 2 * rv).length;
  const share = ln / Math.max(1, ln + vn);
  fLine.push({ x: sim.steps(), y: share });
  if (fLine.length > 2000) fLine.splice(0, 1);
  plotB.set([{ points: fLine, color: cssVar("--species-2") }]);
  $("rlB").textContent = deepL ? `${(ls / deepL).toFixed(3)} (model ${rl.toFixed(3)})` : "–";
  $("rvB").textContent = deepV ? `${(vs / deepV).toFixed(4)} (model ${rv.toFixed(4)})` : "–";
  $("fB").textContent = `${Math.round(100 * share)}%`;
  $("stepsB").textContent = sim.steps().toLocaleString();
}

const hostB = new SimHost<FluidSim>({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "fluid",
  create: (c, name, o) => FluidSim.create(c, name, o),
  options: () => ({ w: 256, h: 256, temperature: Number(tB.value), start: startB, fill: startB === 0 ? 0.35 : startB === 2 ? 0.3 : 1.0, gravity: Number(gB.value), seed: 1 }),
  names: [],
  label: "lattice Boltzmann",
  colors: ["--species-2", "--muted", "--plot-back", "--highlight"],
  stepsPerFrame: () => Number(spfB.value),
  afterSteps: async (sim) => {
    const now = performance.now();
    if (now - lastReadB < 250) return;
    lastReadB = now;
    await readoutB(sim);
  },
  energy: false,
  stepName: "steps",
  onCreate: async (sim) => {
    fLine.length = 0;
    sim.color_mode(Number($<HTMLSelectElement>("colorB").value));
    applyHotB(sim);
    await readoutB(sim);
  },
});

function applyHotB(sim: FluidSim) {
  const f = Number(hotB.value);
  sim.set_hot_plate(f, 0.5);
  $("hotBv").innerHTML = f > 0 ? `${f.toFixed(2)} T<sub>c</sub> (the middle half of the floor)` : "off";
}

$("runB").onclick = () => {
  hostB.running = !hostB.running;
  $("runB").textContent = hostB.running ? "Pause" : "Run";
};
const restartB = (s: number) => {
  startB = s;
  const run = hostB.running;
  void hostB.create().then(() => (hostB.running = run));
};
$("startBMix").onclick = () => restartB(1);
$("startBPool").onclick = () => {
  if (Number(gB.value) === 0) {
    gB.value = "0.00002";
    showGB();
  }
  restartB(0);
};
$("startBDrop").onclick = () => restartB(2);
tB.oninput = () => {
  $("tBv").innerHTML = `${Number(tB.value).toFixed(2)} T<sub>c</sub>`;
  hostB.sim?.set_temperature(Number(tB.value));
};
const showGB = () => ($("gBv").textContent = Number(gB.value) === 0 ? "off" : `${(Number(gB.value) * 1e6).toFixed(0)} × 10⁻⁶`);
gB.oninput = () => {
  showGB();
  hostB.sim?.set_gravity(Number(gB.value));
};
hotB.oninput = () => hostB.sim && applyHotB(hostB.sim);
spfB.oninput = () => ($("spfBv").textContent = spfB.value);
$<HTMLSelectElement>("colorB").onchange = () => hostB.sim?.color_mode(Number($<HTMLSelectElement>("colorB").value));
showGB();
const setToolB = (t: typeof toolB) => {
  toolB = toolB === t ? null : t;
  $("dropB").classList.toggle("on", toolB === "drop");
  $("bubbleB").classList.toggle("on", toolB === "bubble");
  hostB.canvas.style.cursor = toolB ? "crosshair" : "";
};
$("dropB").onclick = () => setToolB("drop");
$("bubbleB").onclick = () => setToolB("bubble");
hostB.canvas.addEventListener("pointerdown", (e) => {
  const sim = hostB.sim;
  if (!sim || !toolB) return;
  const r = hostB.canvas.getBoundingClientRect();
  const [x0, y0, x1, y1] = Array.from(sim.view());
  const x = x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
  const y = y1 - ((e.clientY - r.top) / r.height) * (y1 - y0);
  sim.paint(x, y, 0.06 * sim.width(), toolB === "drop");
  if (!hostB.running) sim.render();
});

// ---- 3. The Gibbs ensemble (crates/mc, on the CPU as WebAssembly). Natively, at kT 0.40 the
// boxes hold vapour 0.024 and liquid 0.747 at equal pressures (crates/mc/tests/gibbs.rs). ----

/** Molecular dynamics of the same model: a liquid slab with its vapour pre-filled near
 * saturation, held 1,000 τ (crates/md/tests/coexistence.rs, slab_with_prefilled_vapour). */
const MD_COEX: { t: number; v: number; l: number }[] = [
  { t: 0.4, v: 0.023, l: 0.742 },
  { t: 0.44, v: 0.058, l: 0.68 },
  { t: 0.46, v: 0.077, l: 0.645 },
];

/** The Gibbs simulation as SimHost expects a simulation; it draws the two boxes on a 2D canvas. */
class GibbsHosted {
  readonly g: GibbsSim;
  private colors = { a: "#4f7cc4", b: "#e5483a", ink: "#888", bg: "#1d1712" };
  constructor(
    private canvas: HTMLCanvasElement,
    n: number,
    t: number,
  ) {
    this.g = new GibbsSim(n, 0.3, t, BigInt(7));
  }
  step(k: number) {
    this.g.run(k);
  }
  /** Yield to the browser (the moves ran synchronously). */
  sync() {
    return new Promise((r) => setTimeout(r, 0));
  }
  render() {
    const c = this.canvas.getContext("2d");
    if (!c) return;
    const [W, H] = [this.canvas.width, this.canvas.height];
    c.fillStyle = this.colors.bg;
    c.fillRect(0, 0, W, H);
    const [a, b] = Array.from(this.g.sides());
    const gap = 0.08 * Math.max(a, b);
    const s = Math.min((0.94 * W) / (a + b + gap), (0.92 * H) / Math.max(a, b));
    const y0 = H / 2;
    const x0 = W / 2 - (s * (a + b + gap)) / 2;
    const boxes: [number, number, string][] = [
      [x0, a, this.colors.a],
      [x0 + s * (a + gap), b, this.colors.b],
    ];
    boxes.forEach(([x, side, col], k) => {
      const top = y0 - (s * side) / 2;
      c.strokeStyle = this.colors.ink;
      c.lineWidth = 1;
      c.strokeRect(x, top, s * side, s * side);
      const p = this.g.positions(k);
      c.fillStyle = col;
      const r = Math.max(0.8, 0.5 * s);
      for (let i = 0; i < p.length; i += 2) {
        c.beginPath();
        c.arc(x + s * p[i], top + s * (side - p[i + 1]), r, 0, 2 * Math.PI);
        c.fill();
      }
    });
  }
  resize() {}
  /** Species 0 colours the left box, the highlight the right one, species 1 the outlines. */
  set_colors(f: Float32Array) {
    const hex = (k: number) => `rgb(${Math.round(255 * f[4 * k])}, ${Math.round(255 * f[4 * k + 1])}, ${Math.round(255 * f[4 * k + 2])})`;
    this.colors = { a: hex(0), b: hex(4), ink: hex(1), bg: hex(5) };
  }
  describe() {
    return [
      `Method: Gibbs-ensemble Monte Carlo (Panagiotopoulos 1987), ${this.g.counts()[0] + this.g.counts()[1]} molecules in two periodic square boxes. A cycle is N tries at moving a molecule within its box, one try at passing area from one box to the other, and N/4 tries at moving a molecule to a random place in the other box, each accepted with the Metropolis rule for this ensemble.`,
      "Forces: Lennard-Jones, cut and shifted at 2.5 σ (the molecular-dynamics pages' model).",
      "Ensemble: constant total N, total area and T (the moves are a heat bath): the Gibbs ensemble. No time, only equilibrium: one cycle is not a duration.",
      "Runs on the CPU (as WebAssembly): its moves are sequential at the sizes it needs, so a GPU would be slower.",
    ];
  }
  energy_note() {
    return "";
  }
  dt() {
    return 1;
  }
  steps() {
    return this.g.cycles();
  }
  free() {
    this.g.free();
  }
}

const tC = $<HTMLInputElement>("tC");
const spfC = $<HTMLInputElement>("spfC");
const plotC = new LinesPlot($<HTMLCanvasElement>("plotC"), { xLabel: "cycles", yLabel: "density", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const pdC = new LinesPlot($<HTMLCanvasElement>("pdC"), { xLabel: "density", yLabel: "kT", yMin: 0.3, yMax: 0.52, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const d0: { x: number; y: number }[] = [];
const d1: { x: number; y: number }[] = [];
/** Averaged (vapour, liquid) per temperature, after the boxes settle. */
const gemc = new Map<number, { v: number; l: number; n: number }>();
let changedC = 0;
/** Recent pressure readings of each box, for a running average (one reading is very noisy). */
const pHist: [number, number][] = [];

function drawPdC() {
  const pts: { x: number; y: number }[] = [];
  for (const [t, m] of gemc) if (m.n >= 5) pts.push({ x: m.v / m.n, y: t }, { x: m.l / m.n, y: t });
  const md = MD_COEX.flatMap((p) => [
    { x: p.v, y: p.t },
    { x: p.l, y: p.t },
  ]);
  pdC.set([
    { points: md, color: cssVar("--muted"), dots: true },
    { points: pts, color: cssVar("--accent"), dots: true },
  ]);
}

function readoutC(h: GibbsHosted) {
  const [r0, p0, r1, p1] = Array.from(h.g.measure());
  const cyc = h.g.cycles();
  d0.push({ x: cyc, y: r0 });
  d1.push({ x: cyc, y: r1 });
  if (d0.length > 1500) [d0, d1].forEach((l) => l.splice(0, 1));
  plotC.set([
    { points: d0, color: cssVar("--species-2") },
    { points: d1, color: cssVar("--highlight") },
  ]);
  // Average once the boxes have split clearly and settled at this temperature.
  const t = Math.round(h.g.temperature() / 0.005) * 0.005;
  const [v, l] = [Math.min(r0, r1), Math.max(r0, r1)];
  if (cyc - changedC > 1500 && l > 3 * v) {
    const m = gemc.get(t) ?? { v: 0, l: 0, n: 0 };
    m.v += v;
    m.l += l;
    m.n++;
    gemc.set(t, m);
    drawPdC();
  }
  $("r0C").textContent = r0.toFixed(3);
  $("r1C").textContent = r1.toFixed(3);
  pHist.push([p0, p1]);
  if (pHist.length > 50) pHist.splice(0, 1);
  const avg = (k: 0 | 1) => pHist.reduce((a, p) => a + p[k], 0) / pHist.length;
  $("pC").textContent = `${avg(0).toFixed(4)}, ${avg(1).toFixed(4)}`;
  $("xC").textContent = `${(100 * h.g.acceptance()[2]).toFixed(1)}%`;
}

let lastReadC = 0;
const hostC = new SimHost<GibbsHosted>({
  canvas: $<HTMLCanvasElement>("canvasC"),
  overlay: $("overlayC"),
  scenario: "gibbs",
  create: async (c) => new GibbsHosted(c, Number($<HTMLSelectElement>("nC").value), Number(tC.value)),
  options: () => ({}),
  names: [],
  label: "Gibbs ensemble",
  colors: ["--species-2", "--muted", "--species-2", "--species-3"],
  stepsPerFrame: () => Number(spfC.value),
  afterSteps: (h) => {
    const now = performance.now();
    if (now - lastReadC < 200) return;
    lastReadC = now;
    readoutC(h);
  },
  energy: false,
  stepName: "cycles",
  onCreate: (h) => {
    changedC = 0;
    pHist.length = 0;
    [d0, d1].forEach((l) => (l.length = 0));
    readoutC(h);
  },
});

$("runC").onclick = () => {
  hostC.running = !hostC.running;
  $("runC").textContent = hostC.running ? "Pause" : "Run";
};
const restartC = () => {
  const run = hostC.running;
  void hostC.create().then(() => (hostC.running = run));
};
$("resetC").onclick = restartC;
$("nC").onchange = restartC;
tC.oninput = () => {
  $("tCv").textContent = Number(tC.value).toFixed(3);
  hostC.sim?.g.set_temperature(Number(tC.value));
  changedC = hostC.sim?.g.cycles() ?? 0;
  pHist.length = 0;
};
spfC.oninput = () => ($("spfCv").textContent = spfC.value);
drawPdC();
