import { GasSim } from "../../src/lib/wasm";
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
    coexistenceTest?: { ready: Promise<void>; measure(t: number, sweeps: number, side: number): Promise<{ liquid: number; vapour: number; exact: [number, number] | null }> };
  }
}

window.coexistenceTest = {
  ready: host.ready,
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
