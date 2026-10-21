import { $, cssVar } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { formatN, N_TRUE, record } from "../../src/lib/perrin";

// Independent radioactive atoms: in each small time step δt every atom decays with probability
// λ δt, λ = ln 2 / half-life; a fixed share of decays reach the screen and flash. The number
// decaying in a step is drawn exactly (binomial, by a Poisson draw, as λ δt is tiny).

const SEEN = 0.1;
const DT = 1 / 60; // s per frame
const nIn = $<HTMLInputElement>("n");
const halfIn = $<HTMLInputElement>("half");
const intervalIn = $<HTMLInputElement>("interval");
const screen = $<HTMLCanvasElement>("screen");
const poisson = new LinesPlot($<HTMLCanvasElement>("poisson"), { xLabel: "flashes in one interval", yLabel: "how often", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const decay = new LinesPlot($<HTMLCanvasElement>("decay"), { xLabel: "time (s)", yLabel: "flashes per interval", log: true, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

// mulberry32: small, and good enough for counting statistics (a plain LCG's draws are correlated).
let seed = 5;
const rand = () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
/** A Poisson draw with mean `mu` (Knuth for small means, a normal approximation for large). */
function poissonDraw(mu: number) {
  if (mu > 30) return Math.max(0, Math.round(mu + Math.sqrt(mu) * Math.sqrt(-2 * Math.log(1 - rand())) * Math.cos(2 * Math.PI * rand())));
  const l = Math.exp(-mu);
  let [k, p] = [0, 1];
  do {
    k++;
    p *= rand();
  } while (p > l);
  return k - 1;
}

let atoms = 0;
let time = 0;
let counted = 0;
let running = false;
let intervalCount = 0;
let intervalStart = 0;
let counts: number[] = []; // flashes per completed interval
let series: { x: number; y: number }[] = [];
let flashes: { x: number; y: number; age: number }[] = [];

function fresh() {
  atoms = Number(nIn.value);
  time = 0;
  counted = 0;
  intervalCount = 0;
  intervalStart = 0;
  counts = [];
  series = [];
  flashes = [];
}

function step(dt: number) {
  const lambda = Math.LN2 / Number(halfIn.value);
  const decays = Math.min(atoms, poissonDraw(atoms * lambda * dt));
  atoms -= decays;
  let seen = 0;
  for (let k = 0; k < decays; k++) if (rand() < SEEN) seen++;
  for (let k = 0; k < seen; k++) flashes.push({ x: rand(), y: rand(), age: 0 });
  counted += seen;
  intervalCount += seen;
  time += dt;
  const iv = Number(intervalIn.value);
  if (time - intervalStart >= iv) {
    counts.push(intervalCount);
    series.push({ x: intervalStart + iv / 2, y: Math.max(intervalCount, 0.5) });
    intervalCount = 0;
    intervalStart += iv;
  }
}

/** The half-life from all the flashes so far: for an exponential decay watched for a time T, the
 * flashes' average time t̄ satisfies 1/λ − T/(e^{λT} − 1) = t̄ (the maximum-likelihood rate). Every
 * flash counts, including the late, sparse ones a fit of ln(counts) would have to drop. */
function halfLife() {
  if (series.length < 8) return NaN;
  const iv = Number(intervalIn.value);
  const T = series[series.length - 1].x + iv / 2;
  let [c, ct] = [0, 0];
  for (let k = 0; k < counts.length; k++) {
    c += counts[k];
    ct += counts[k] * (k + 0.5) * iv;
  }
  const tbar = ct / c;
  if (!(tbar < T / 2)) return NaN; // no decay seen yet
  const f = (l: number) => 1 / l - T / Math.expm1(l * T) - tbar;
  let [lo, hi] = [1e-6, 50 / T];
  for (let k = 0; k < 100; k++) {
    const m = 0.5 * (lo + hi);
    if (f(m) > 0) lo = m;
    else hi = m;
  }
  return Math.LN2 / (0.5 * (lo + hi));
}

function draw() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  screen.width = Math.round(screen.clientWidth * dpr);
  screen.height = Math.round(screen.clientHeight * dpr);
  const g = screen.getContext("2d")!;
  const [W, H] = [screen.width, screen.height];
  g.fillStyle = "#050605";
  g.fillRect(0, 0, W, H);
  for (const f of flashes) {
    const a = Math.max(0, 1 - f.age / 0.25);
    g.fillStyle = `rgba(120, 255, 160, ${a})`;
    g.beginPath();
    g.arc(f.x * W, f.y * H, (1.2 + 2 * a) * dpr, 0, 2 * Math.PI);
    g.fill();
  }
  // Recent intervals only, for the histogram: the rate changes as the sample decays.
  const recent = counts.slice(-200);
  const mean = recent.reduce((s, c) => s + c, 0) / Math.max(1, recent.length);
  const freq = new Map<number, number>();
  for (const c of recent) freq.set(c, (freq.get(c) ?? 0) + 1);
  const max = Math.max(5, ...recent) + 2;
  const fact = (k: number) => {
    let f = 0;
    for (let j = 2; j <= k; j++) f += Math.log(j);
    return f;
  };
  const pred = Array.from({ length: max }, (_, k) => ({ x: k, y: recent.length * Math.exp(k * Math.log(Math.max(mean, 1e-9)) - mean - fact(k)) }));
  poisson.set([
    { points: Array.from({ length: max }, (_, k) => ({ x: k, y: freq.get(k) ?? 0 })), color: cssVar("--accent"), dots: true },
    { points: pred, color: cssVar("--muted") },
  ]);
  decay.set([{ points: series, color: cssVar("--species-2"), dots: true }]);
  const sd = Math.sqrt(recent.reduce((s, c) => s + (c - mean) ** 2, 0) / Math.max(1, recent.length - 1));
  $("left").textContent = atoms.toLocaleString();
  $("count").textContent = counted.toLocaleString();
  $("mean").textContent = recent.length > 3 ? `${mean.toFixed(1)} ± ${sd.toFixed(1)} (√ of the mean: ${Math.sqrt(mean).toFixed(1)})` : "–";
  const h = halfLife();
  $("hm").textContent = Number.isFinite(h) ? `${h.toFixed(0)} s (set: ${halfIn.value} s)` : "counting…";
  // Rutherford's count: the flashes seen so far are SEEN of the atoms that decayed, a share
  // 1 − e^(−λT) of the sample; so the sample had c / (SEEN (1 − e^(−λT))) atoms, and it weighed
  // (on an imaginary balance) its atoms × 226 / N grams.
  const c = counts.reduce((s, x) => s + x, 0);
  if (Number.isFinite(h)) {
    const T = counts.length * Number(intervalIn.value);
    const atomsSeen = c / (SEEN * -Math.expm1((-Math.LN2 * T) / h));
    const n = (atomsSeen * 226) / sampleGrams();
    $("nav").textContent = `${formatN(n)} (sample: ${sampleGrams().toExponential(2)} g)`;
    record("counting", n, `flashes from a weighed sample, ${c.toLocaleString()} counted`);
  } else $("nav").textContent = `sample: ${sampleGrams().toExponential(2)} g`;
}

/** What the sample weighed at the start (g): an isotope of atomic weight 226, like radium. */
const sampleGrams = () => (Number(nIn.value) * 226) / N_TRUE;

function frame() {
  if (running) {
    step(DT);
    for (const f of flashes) f.age += DT;
    flashes = flashes.filter((f) => f.age < 0.25);
    draw();
  }
  requestAnimationFrame(frame);
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("fresh").onclick = () => {
  fresh();
  draw();
};
const labels = () => {
  $("nv").textContent = Number(nIn.value).toLocaleString();
  $("hv").textContent = `${halfIn.value} s`;
  $("iv").textContent = `${intervalIn.value} s`;
};
for (const el of [nIn, halfIn, intervalIn]) el.oninput = labels;
nIn.onchange = () => {
  fresh();
  draw();
};
labels();
fresh();
draw();
new ResizeObserver(draw).observe(screen);
requestAnimationFrame(frame);

declare global {
  interface Window {
    countingTest?: { run(seconds: number, half: number): { mean: number; variance: number; half: number } };
  }
}

window.countingTest = {
  /** Run `seconds` of a fresh sample with this half-life (1 s intervals); the counts' mean and
   * variance over the first 200 intervals (about steady if the half-life is long), and the fitted
   * half-life. */
  run(seconds: number, half: number) {
    halfIn.value = String(half);
    labels();
    fresh();
    for (let t = 0; t < seconds; t += DT) step(DT);
    draw();
    const first = counts.slice(0, 200);
    const mean = first.reduce((s, c) => s + c, 0) / first.length;
    const variance = first.reduce((s, c) => s + (c - mean) ** 2, 0) / (first.length - 1);
    return { mean, variance, half: halfLife() };
  },
};
