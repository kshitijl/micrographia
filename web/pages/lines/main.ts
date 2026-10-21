import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { Tools } from "../../src/lib/interact";
import type { DiscSim } from "../../src/lib/wasm";

// Doppler-broadened lines from a gas of light (m = 1) and heavy (m = 16) atoms (scenario heavy,
// no balls). In lab units: kT = 1 is T_LAB, mass 1 is one atomic mass unit.

const T_LAB = 1000; // K at kT = 1
const K = 1.380649e-23;
const U = 1.66054e-27;
const C = 2.998e8;
/** The two lines, nm: hydrogen's Balmer α and oxygen's 777 nm triplet. */
const LAMBDA = [656.3, 777.2];
const MASSES = [1, 16];
/** Speeds in the simulation (σ/τ) to m/s: at kT = 1, m = 1, √(kT/m) = 1. */
const V_LAB = Math.sqrt((K * T_LAB) / U);
/** The profile: Δλ from −40 to +40 pm, in half-picometre bins. */
const SPAN = 40;
const BINS = 160;

const profile = new LinesPlot($<HTMLCanvasElement>("profile"), { xLabel: "shift from the line's centre (pm; + is redder)", yLabel: "light", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const hist = [new Float64Array(BINS), new Float64Array(BINS)];
let filled = 0;
let kt = 1;
let shift = [0, 0];

/** Δλ (pm) of an atom moving at vx (σ/τ) towards the spectroscope, at +x: bluer. */
const dl = (k: number, vx: number) => -LAMBDA[k] * 1e3 * ((vx * V_LAB) / C);

async function readout(sim: DiscSim, tools: Tools | null) {
  const v = (await sim.velocities()) as Float32Array;
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const r = tools?.rect;
  const h = [new Float64Array(BINS), new Float64Array(BINS)];
  const sum = [0, 0];
  const n = [0, 0];
  let [ke, all] = [0, 0];
  for (let i = 0; i < species.length; i++) {
    const s = species[i];
    if (s > 1) continue;
    ke += 0.5 * MASSES[s] * (v[2 * i] ** 2 + v[2 * i + 1] ** 2);
    all++;
    if (r && !(pos[2 * i] >= r.x0 && pos[2 * i] <= r.x1 && pos[2 * i + 1] >= r.y0 && pos[2 * i + 1] <= r.y1)) continue;
    const d = dl(s, v[2 * i]);
    sum[s] += d;
    n[s]++;
    const b = Math.floor(((d + SPAN) / (2 * SPAN)) * BINS);
    if (b >= 0 && b < BINS) h[s][b]++;
  }
  kt = ke / all;
  shift = sum.map((x, k) => x / Math.max(1, n[k]));
  // Average the profiles over recent readings (one look is noisy).
  const a = filled < 10 ? 1 / ++filled : 0.1;
  for (let k = 0; k < 2; k++) for (let b = 0; b < BINS; b++) hist[k][b] = (1 - a) * hist[k][b] + (a * h[k][b]) / Math.max(1, n[k]);
  show();
}

/** Width at half height (pm) of a measured profile: the interpolated crossings either side of
 * the peak (from a 3-bin smoothed profile). */
function fwhm(p: Float64Array) {
  const s = Array.from(p, (_, b) => (p[Math.max(0, b - 1)] + p[b] + p[Math.min(BINS - 1, b + 1)]) / 3);
  let top = 0;
  for (let b = 0; b < BINS; b++) if (s[b] > s[top]) top = b;
  const half = s[top] / 2;
  let lo = top;
  while (lo > 0 && s[lo] > half) lo--;
  let hi = top;
  while (hi < BINS - 1 && s[hi] > half) hi++;
  if (lo === 0 || hi === BINS - 1) return NaN;
  const at = (b: number, c: number) => b + (half - s[b]) / (s[c] - s[b]);
  const w = (2 * SPAN) / BINS;
  return (at(hi - 1, hi) - at(lo, lo + 1)) * w;
}

/** Doppler's Gaussian width (FWHM, pm) at kT (sim units) for line k and mass m. */
const doppler = (k: number, m: number, t: number) => 2.3548 * LAMBDA[k] * 1e3 * ((Math.sqrt(t / m) * V_LAB) / C);

function show() {
  const w = (2 * SPAN) / BINS;
  const x = (b: number) => -SPAN + (b + 0.5) * w;
  const gauss = (k: number) => {
    const s = doppler(k, MASSES[k], kt) / 2.3548;
    return Array.from({ length: 121 }, (_, j) => {
      const d = -SPAN + (2 * SPAN * j) / 120;
      return { x: d, y: (w / (s * Math.sqrt(2 * Math.PI))) * Math.exp(-((d - shift[k]) ** 2) / (2 * s * s)) };
    });
  };
  profile.set([
    { points: gauss(0), color: cssVar("--muted") },
    { points: gauss(1), color: cssVar("--muted") },
    { points: Array.from(hist[0], (y, b) => ({ x: x(b), y })), color: cssVar("--trace-1"), dots: true },
    { points: Array.from(hist[1], (y, b) => ({ x: x(b), y })), color: cssVar("--trace-2"), dots: true },
  ]);
  const wid = [fwhm(hist[0]), fwhm(hist[1])];
  $("t").textContent = `${(kt * T_LAB).toFixed(0)} K`;
  $("w").textContent = wid.map((x, k) => `${Number.isFinite(x) ? x.toFixed(1) : "–"} pm (Doppler: ${doppler(k, MASSES[k], kt).toFixed(1)})`).join(", ");
  // m = kT (2.3548 λ / c FWHM)²: the mass that would give this width at this temperature.
  const mass = wid.map((x, k) => (kt * (2.3548 * LAMBDA[k] * 1e3 * (V_LAB / C)) ** 2) / (x * x));
  $("m").textContent = mass.map((m) => (Number.isFinite(m) ? `${m.toFixed(1)} u` : "–")).join(", ");
  $("shift").textContent = shift.map((s) => `${(-s).toFixed(1)} pm`).join(", ");
  drawSpectrum(wid);
}

/** The spectroscope's view: a dark band with each line drawn as its profile's brightness. */
function drawSpectrum(_w: number[]) {
  const c = $<HTMLCanvasElement>("spectrum");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  const [W, H] = [c.width, c.height];
  g.fillStyle = "#050403";
  g.fillRect(0, 0, W, H);
  const colours = ["255,40,30", "150,20,20"];
  for (let k = 0; k < 2; k++) {
    const x0 = (k === 0 ? 0.08 : 0.58) * W;
    const span = 0.34 * W;
    const top = Math.max(...hist[k]) || 1;
    for (let b = 0; b < BINS; b++) {
      g.fillStyle = `rgba(${colours[k]},${Math.min(1, hist[k][b] / top)})`;
      g.fillRect(x0 + (b / BINS) * span, 0, span / BINS + 1, H);
    }
    g.fillStyle = "rgba(255,255,255,0.6)";
    g.font = `${11 * dpr}px system-ui, sans-serif`;
    g.fillText(`${LAMBDA[k]} nm`, x0, H - 6 * dpr);
  }
}

let tools: Tools | null = null;
const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "heavy",
  options: () => ({ n: 3000, density: 0.15, heavy: 16, balls: 0, seed: 2 }),
  names: ["hydrogen", "oxygen"],
  colors: ["--trace-1", "--trace-2"],
  stepsPerFrame: () => 40,
  afterSteps: (sim) => readout(sim, tools),
  onCreate: async (sim) => {
    hist.forEach((h) => h.fill(0));
    filled = 0;
    await readout(sim, tools);
  },
});
tools = new Tools({
  host,
  container: $("tools"),
  actions: [
    { label: "Heat (1.5× faster)", scale: 1.5 },
    { label: "Cool (½ as fast)", scale: 0.5 },
    { label: "Shove towards you →", scale: 1, add: [1.5, 0] },
  ],
  after: (sim) => {
    filled = 0;
    return readout(sim, tools);
  },
});

const all = (scale: number) => {
  const sim = host.sim;
  if (!sim) return;
  const [w, h] = Array.from(sim.box_size());
  sim.kick_region(-1, -1, w + 1, h + 1, scale, 0, 0);
  filled = 0;
  void readout(sim, tools);
};
$("hotter").onclick = () => all(Math.SQRT2);
$("colder").onclick = () => all(Math.SQRT1_2);
$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};

declare global {
  interface Window {
    linesTest?: { ready: Promise<void>; measure(scale: number, steps: number): Promise<{ t: number; widths: number[]; doppler: number[]; masses: number[] }> };
  }
}

window.linesTest = {
  ready: host.ready,
  /** Scale every speed by `scale`, settle, then average the lines over `steps` (a look every 200):
   * temperature (K), widths (pm), Doppler's widths and the masses inferred. */
  async measure(scale: number, steps: number) {
    return host.exclusive(async (sim) => {
      const [w, h] = Array.from(sim.box_size());
      sim.kick_region(-1, -1, w + 1, h + 1, scale, 0, 0);
      sim.step(4000);
      hist.forEach((x) => x.fill(0));
      filled = 0;
      // A plain average over every look.
      let looks = 0;
      const acc = [new Float64Array(BINS), new Float64Array(BINS)];
      let tSum = 0;
      for (let k = 0; k < steps / 200; k++) {
        sim.step(200);
        await readout(sim, null);
        looks++;
        tSum += kt;
        if (filled >= 10) for (let s = 0; s < 2; s++) for (let b = 0; b < BINS; b++) acc[s][b] += hist[s][b];
      }
      for (let s = 0; s < 2; s++) hist[s].set(acc[s]);
      const t = tSum / looks;
      const widths = [fwhm(hist[0]), fwhm(hist[1])];
      sim.render();
      return {
        t: t * T_LAB,
        widths,
        doppler: [doppler(0, 1, t), doppler(1, 16, t)],
        masses: widths.map((x, k) => (t * (2.3548 * LAMBDA[k] * 1e3 * (V_LAB / C)) ** 2) / (x * x)),
      };
    });
  },
};
