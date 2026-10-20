import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const BINS = 20;
const startSel = $<HTMLSelectElement>("start");
const gravityIn = $<HTMLInputElement>("gravity");
const tempIn = $<HTMLInputElement>("heat");
/** Gravity at 1×: the light gas's scale height is 0.35 of the column at kT = 1. */
let g1 = 0;
let counts = [new Float64Array(BINS), new Float64Array(BINS)];
let ke = new Float64Array(BINS);
let keN = new Float64Array(BINS);
let frames = 0;
let box = [1, 1];
let g = 0;
let tMean = 1;

const density = new LinesPlot($<HTMLCanvasElement>("density"), {
  xLabel: "height (σ)",
  yLabel: "density",
  log: true,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const tplot = new LinesPlot($<HTMLCanvasElement>("temp"), {
  xLabel: "height (σ)",
  yLabel: "temperature",
  yMin: 0,
  yMax: 2,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

function clear() {
  recentN = [new Float64Array(BINS), new Float64Array(BINS)];
  recentE = new Float64Array(BINS);
  counts = [new Float64Array(BINS), new Float64Array(BINS)];
  ke = new Float64Array(BINS);
  keN = new Float64Array(BINS);
  frames = 0;
}

// Recent profiles for the side plots: exponential moving averages over ~20 snapshots, so they
// follow the gas as it settles.
let recentN = [new Float64Array(BINS), new Float64Array(BINS)];
let recentE = new Float64Array(BINS);
const ALPHA = 0.05;

/** One vertical profile beside the column: a bar per height slice (bottom of the canvas = floor),
 * with optional dashed comparison values, and the maximum printed at the top. */
function side(id: string, values: number[][], colors: (string | ((b: number) => string))[], dashed: number[] | null, max: number, label: (v: number) => string) {
  const c = $<HTMLCanvasElement>(id);
  const dpr = window.devicePixelRatio || 1;
  const [W, H] = [c.clientWidth, c.clientHeight];
  if (c.width !== Math.round(W * dpr) || c.height !== Math.round(H * dpr)) {
    c.width = Math.round(W * dpr);
    c.height = Math.round(H * dpr);
  }
  const ctx = c.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  const n = values[0].length;
  const bh = H / n;
  for (let b = 0; b < n; b++) {
    let x = 0;
    values.forEach((v, k) => {
      const w = (Math.max(0, v[b]) / max) * W;
      const c = colors[k];
      ctx.fillStyle = typeof c === "string" ? c : c(b);
      ctx.fillRect(x, H - (b + 1) * bh + 0.5, w, bh - 1);
      x += w;
    });
  }
  if (dashed) {
    ctx.strokeStyle = cssVar("--text");
    ctx.setLineDash([3, 3]);
    ctx.beginPath();
    dashed.forEach((v, b) => {
      const [x, y] = [(Math.max(0, v) / max) * W, H - (b + 0.5) * bh];
      if (b === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
    ctx.setLineDash([]);
  }
  ctx.font = "10px system-ui, sans-serif";
  const text = label(max);
  ctx.fillStyle = cssVar("--bg");
  ctx.fillRect(0, 0, ctx.measureText(text).width + 4, 12);
  ctx.fillStyle = cssVar("--muted");
  ctx.fillText(text, 2, 10);
}

/** Blue for colder than the average, grey for the average, red for hotter (full colour at ±40%). */
function warmth(r: number) {
  const [cold, mid, hot] = [[60, 120, 210], [150, 140, 130], [215, 65, 45]];
  const t = Math.max(-1, Math.min(1, (r - 1) / 0.4));
  const [a, b] = t < 0 ? [mid, cold] : [mid, hot];
  const c = a.map((x, i) => Math.round(x + (b[i] - x) * Math.abs(t)));
  return `rgb(${c.join(",")})`;
}

function drawSides(masses: number[]) {
  const dh = box[1] / BINS;
  const area = dh * box[0];
  const rho = [0, 1].map((s) => Array.from(recentN[s], (c) => c / area));
  const n = rho[0].map((r, b) => r + rho[1][b]);
  const t = Array.from(recentE, (e, b) => (recentN[0][b] + recentN[1][b] > 5 ? e / (recentN[0][b] + recentN[1][b]) : 0));
  // Pressure at the middle of each slice: the weight of everything above it, per unit width.
  const p = new Array(BINS).fill(0);
  let above = 0;
  for (let b = BINS - 1; b >= 0; b--) {
    const own = (masses[0] * rho[0][b] + masses[1] * rho[1][b]) * g * dh;
    p[b] = above + own / 2;
    above += own;
  }
  const nkT = n.map((x, b) => x * t[b]);
  const tAvg = t.reduce((a, x, b) => a + x * n[b], 0) / Math.max(1e-9, n.reduce((a, x) => a + x, 0));
  side("sideRho", rho, [cssVar("--cream"), cssVar("--species-3")], null, Math.max(...n) * 1.1 || 1, (v) => v.toFixed(2));
  side("sideP", [p], [cssVar("--plot-back")], nkT, Math.max(...p, ...nkT) * 1.1 || 1, (v) => v.toFixed(3));
  side("sideT", [t], [(b) => warmth(t[b] / tAvg)], new Array(BINS).fill(tAvg), Math.max(...t, 0.1) * 1.3, (v) => v.toFixed(2));
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const { ke: k } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const masses = Array.from(sim.masses());
  const dh = box[1] / BINS;
  let total = 0;
  const nowN = [new Float64Array(BINS), new Float64Array(BINS)];
  const nowE = new Float64Array(BINS);
  for (let i = 0; i < species.length; i++) {
    const b = Math.min(BINS - 1, Math.floor(pos[2 * i + 1] / dh));
    nowN[species[i]][b]++;
    nowE[b] += k[i];
    counts[species[i]][b]++;
    ke[b] += k[i];
    keN[b]++;
    total += k[i];
  }
  frames++;
  tMean = total / species.length;
  const first = recentN[0].every((x) => x === 0) && recentN[1].every((x) => x === 0);
  for (let s = 0; s < 2; s++) for (let b = 0; b < BINS; b++) recentN[s][b] = first ? nowN[s][b] : (1 - ALPHA) * recentN[s][b] + ALPHA * nowN[s][b];
  for (let b = 0; b < BINS; b++) recentE[b] = first ? nowE[b] : (1 - ALPHA) * recentE[b] + ALPHA * nowE[b];
  drawSides(masses);
  const area = dh * box[0] * frames;
  const meas = [0, 1].map((s) => Array.from(counts[s], (c, b) => ({ x: (b + 0.5) * dh, y: c / area })));
  // Theory: exponential with the measured temperature, normalized to the same number of molecules
  // (ignoring the thin layers at the soft floor and ceiling).
  const theory = [0, 1].map((s) => {
    const n = counts[s].reduce((a, c) => a + c, 0) / frames;
    const lam = (masses[s] * g) / tMean;
    const z = (1 - Math.exp(-lam * box[1])) / lam;
    return Array.from({ length: BINS }, (_, b) => ({ x: (b + 0.5) * dh, y: (n / box[0] / z) * Math.exp(-lam * (b + 0.5) * dh) }));
  });
  density.set([
    { points: theory[0], color: cssVar("--muted"), dashed: true },
    { points: theory[1], color: cssVar("--muted"), dashed: true },
    { points: meas[0], color: cssVar("--accent"), dots: true },
    { points: meas[1], color: cssVar("--species-3"), dots: true },
  ]);
  tplot.set([
    { points: Array.from(ke, (e, b) => ({ x: (b + 0.5) * dh, y: keN[b] > 50 ? e / keN[b] : NaN })), color: cssVar("--accent"), dots: true },
    { points: [{ x: 0, y: tMean }, { x: box[1], y: tMean }], color: cssVar("--muted"), dashed: true },
  ]);
  const top = Array.from(ke, (e, b) => (keN[b] > 50 ? e / keN[b] : NaN)).filter(Number.isFinite);
  $("status").textContent = `${frames} snapshots. Temperature from the lowest to the highest well-populated layer: ${top[0]?.toFixed(3)} → ${top[top.length - 1]?.toFixed(3)}.`;
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "atmosphere",
  // No heat bath: the column is sealed off. Half the usual time step, for the hot starts.
  options: () => ({ n: 6000, heavy: 4, heavy_share: 0.3, scale: 0.35, gravity: Number(gravityIn.value), temperature: 1, start: Number(startSel.value), dt: 0.0025, seed: 11 }),
  names: ["light gas", "heavy gas"],
  colors: ["--cream", "--species-3"],
  stepsPerFrame: () => 200,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    box = Array.from(sim.box_size());
    g1 = 1 / (0.35 * box[1]);
    g = Number(gravityIn.value) * g1;
    clear();
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
$("clear").onclick = clear;
$("reset").onclick = restart;
startSel.onchange = restart;
gravityIn.oninput = () => {
  $("gv").textContent = `${Number(gravityIn.value).toFixed(1)}×`;
  g = Number(gravityIn.value) * g1;
  host.sim?.set_gravity(0, -g);
  clear();
};
// Heat or cool: rescale every molecule's velocity so the average kinetic energy is the target.
tempIn.oninput = () => ($("tv").textContent = Number(tempIn.value).toFixed(1));
tempIn.onchange = () =>
  void host.exclusive(async (sim) => {
    const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
    const now = ke.reduce((a, b) => a + b, 0) / ke.length;
    sim.kick_region(-1, -1, box[0] + 1, box[1] + 1, Math.sqrt(Number(tempIn.value) / now), 0, 0);
    clear();
  });

// ---- The slab picture: discrete pressure balance → the exponential. ----
const SLAB_X = 3; // m g H / kT for the drawing
const slabs = new BarPlot($<HTMLCanvasElement>("slabs"), {
  labels: [],
  yLabel: "density (bottom slab = 1)",
  xLabel: "height",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
function drawSlabs() {
  const n = Number($<HTMLInputElement>("nslabs").value);
  $("nslabsv").textContent = String(n);
  // 60 bars; each slab covers 60/n of them.
  const bars = 60;
  const f = 1 - SLAB_X / n;
  const values = Array.from({ length: bars }, (_, b) => Math.max(0, f) ** Math.floor((b * n) / bars));
  const theory = Array.from({ length: bars }, (_, b) => Math.exp((-SLAB_X * (b + 0.5)) / bars));
  slabs.set(values, theory, Array.from({ length: bars }, () => ""));
  $("slabstatus").textContent =
    n <= SLAB_X
      ? `With ${n} slab${n > 1 ? "s" : ""} the steps are too coarse: the first slab's weight alone exceeds its pressure. More slabs.`
      : `Each slab is (1 − ${SLAB_X}/${n}) = ${f.toFixed(3)} times the one below. At the top: ${f.toFixed(3)}^${n} = ${(f ** n).toFixed(4)}, against e^−${SLAB_X} = ${Math.exp(-SLAB_X).toFixed(4)}.`;
}
$<HTMLInputElement>("nslabs").oninput = drawSlabs;
drawSlabs();

declare global {
  interface Window {
    atmosphereTest?: { ready: Promise<void>; run(n: number, settle?: number): Promise<{ slopes: number[]; predicted: number[]; tBottom: number; tTop: number }> };
  }
}

window.atmosphereTest = {
  ready: host.ready,
  async run(n: number, settle = 0) {
    await host.ready;
    return host.exclusive(async (sim) => {
      sim.step(settle);
      clear();
      for (let k = 0; k < n; k++) {
        sim.step(200);
        await readout(sim);
      }
      const masses = Array.from(sim.masses());
      const dh = box[1] / BINS;
      // Fit ln density against height over the middle bins.
      const slopes = [0, 1].map((s) => {
        const pts = Array.from(counts[s], (c, b) => ({ x: (b + 0.5) * dh, y: Math.log(Math.max(c, 1)) })).slice(2, 14);
        const mx = pts.reduce((a, p) => a + p.x, 0) / pts.length;
        const my = pts.reduce((a, p) => a + p.y, 0) / pts.length;
        return pts.reduce((a, p) => a + (p.x - mx) * (p.y - my), 0) / pts.reduce((a, p) => a + (p.x - mx) ** 2, 0);
      });
      const t = Array.from(ke, (e, b) => e / keN[b]);
      return { slopes, predicted: masses.map((m) => (-m * g) / tMean), tBottom: t[1], tTop: t[12] };
    });
  },
};
