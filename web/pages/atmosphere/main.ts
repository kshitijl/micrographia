import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { side, warmth } from "./profiles";
import "./convection";
import { cssColor, onThemeChange } from "../../src/lib/theme";
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
  side("sideRho", { values: rho, colors: [cssVar("--cream"), cssVar("--species-3")], hi: Math.max(...n) * 1.1 || 1, label: (v) => v.toFixed(2) });
  side("sideP", { values: [p], colors: [cssVar("--plot-back")], dashed: nkT, hi: Math.max(...p, ...nkT) * 1.1 || 1, label: (v) => v.toFixed(3) });
  side("sideT", { values: [t], colors: [(b) => warmth(t[b] / tAvg)], dashed: new Array(BINS).fill(tAvg), hi: Math.max(...t, 0.1) * 1.3, label: (v) => v.toFixed(2) });
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
// Drawn upright: on the left the column of slabs, each coloured by the weight of air pressing
// down on it; on the right each slab's density as a bar, against the exponential (dashed).
const SLAB_X = 3; // m g H / kT for the drawing
function drawSlabs() {
  const n = Number($<HTMLInputElement>("nslabs").value);
  $("nslabsv").textContent = String(n);
  const f = Math.max(0, 1 - SLAB_X / n);
  const c = $<HTMLCanvasElement>("slabs");
  const dpr = window.devicePixelRatio || 1;
  const [W, H] = [c.clientWidth, c.clientHeight];
  c.width = Math.round(W * dpr);
  c.height = Math.round(H * dpr);
  const ctx = c.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  const [top, bottom] = [8, H - 22];
  const colW = Math.min(170, W * 0.4);
  const [px0, px1] = [colW + 24, W - 8];
  const y = (z: number) => bottom - z * (bottom - top); // z: height as a fraction of the drawing
  const sh = (bottom - top) / n;
  const [light, dark] = [cssColor("--surface-2"), cssColor("--accent")];
  ctx.font = "11px system-ui, sans-serif";
  ctx.textBaseline = "middle";
  for (let k = 0; k < n; k++) {
    // Slab k has density f^k (bottom slab = 1), and carries the weight of every slab above it,
    // including those above the drawing: a geometric series summing to f^(k+1), the pressure at
    // its top in units of the pressure at the floor.
    const density = f ** k;
    const load = f ** (k + 1);
    const shade = f > 0 ? load / f : 0; // full colour for the bottom slab, which carries the most
    const mix = light.slice(0, 3).map((a, i) => Math.round(255 * (a + (dark[i] - a) * shade)));
    ctx.fillStyle = `rgb(${mix.join(",")})`;
    ctx.fillRect(0, y((k + 1) / n) + 0.5, colW, sh - 1);
    if (sh >= 16) {
      ctx.fillStyle = shade > 0.55 ? cssVar("--accent-ink") : cssVar("--text");
      ctx.fillText(`holds ${load.toFixed(2)} p₀`, 6, y((k + 0.5) / n));
    }
    ctx.fillStyle = cssVar("--accent");
    ctx.fillRect(px0, y((k + 1) / n) + 0.5, density * (px1 - px0), sh - 1);
  }
  ctx.strokeStyle = cssVar("--plot-back");
  ctx.lineWidth = 2;
  ctx.setLineDash([5, 4]);
  ctx.beginPath();
  for (let i = 0; i <= 100; i++) {
    const z = i / 100;
    const xx = px0 + Math.exp(-SLAB_X * z) * (px1 - px0);
    if (i === 0) ctx.moveTo(xx, y(z));
    else ctx.lineTo(xx, y(z));
  }
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.lineWidth = 1;
  ctx.strokeStyle = cssVar("--rule");
  ctx.beginPath();
  ctx.moveTo(px0, top);
  ctx.lineTo(px0, bottom);
  ctx.lineTo(px1, bottom);
  ctx.stroke();
  ctx.fillStyle = cssVar("--muted");
  ctx.textBaseline = "alphabetic";
  ctx.fillText("slabs (colour: weight above)", 0, H - 6);
  ctx.fillText("density (floor = 1)  →", px0, H - 6);
  $("slabstatus").textContent =
    n <= SLAB_X
      ? `With ${n} slab${n > 1 ? "s" : ""} the steps are too coarse: the first slab's weight alone exceeds its pressure. More slabs.`
      : `Each slab is (1 − ${SLAB_X}/${n}) = ${f.toFixed(3)} times the one below. At the top: ${f.toFixed(3)}^${n} = ${(f ** n).toFixed(4)}, against e^−${SLAB_X} = ${Math.exp(-SLAB_X).toFixed(4)}.`;
}
$<HTMLInputElement>("nslabs").oninput = drawSlabs;
onThemeChange(drawSlabs);

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
