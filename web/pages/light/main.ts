import { $, cssVar } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { formatN, R, record } from "../../src/lib/perrin";

// Part 1: the light in a hot box as its modes (standing waves). BINS frequency bins, each with
// PER_BIN sample modes standing for the ν² modes a real box has there; each mode trades energy
// with the walls by the Metropolis rule at temperature T: a quantum hν at a time (Planck), or any
// amount (classical). The spectrum is ν² × the bin's mean energy per mode.
// Part 2: the photoelectric effect, as grains of light (Einstein) or as a wave.

const H_EV = 4.1357e-15; // eV s
const K_EV = 8.617e-5; // eV / K
const C_NM = 2.998e17; // nm / s
const BINS = 80;
const DNU = 1.5e13; // Hz
const PER_BIN = 300;
/** Sodium's work function (eV). */
const WORK = 2.3;

let seed = 11;
/** mulberry32. */
function rand() {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

// ---- Part 1 ----

type Light = "quantum" | "classical";
let light: Light = "quantum";
const nu = Float64Array.from({ length: BINS }, (_, k) => (k + 0.5) * DNU);
const quantum = Float64Array.from(nu, (v) => H_EV * v);
/** Each sample mode's energy (eV): a whole number of quanta when the light is quantum. */
const energy = new Float64Array(BINS * PER_BIN);
/** The bins' mean energy per mode, averaged over recent sweeps. */
const mean = new Float64Array(BINS);
let running = false;

const tIn = $<HTMLInputElement>("t");
const kT = () => K_EV * Number(tIn.value);

/** One sweep: each mode in turn offered a change of energy. */
function sweep() {
  const t = kT();
  for (let k = 0; k < BINS; k++) {
    const q = quantum[k];
    for (let i = k * PER_BIN; i < (k + 1) * PER_BIN; i++) {
      const d = light === "quantum" ? (rand() < 0.5 ? q : -q) : (2 * rand() - 1) * t;
      const e = energy[i] + d;
      if (e < -1e-12) continue;
      if (d <= 0 || rand() < Math.exp(-d / t)) energy[i] = e;
    }
  }
}

function measure(blend: number) {
  for (let k = 0; k < BINS; k++) {
    let s = 0;
    for (let i = k * PER_BIN; i < (k + 1) * PER_BIN; i++) s += energy[i];
    mean[k] = blend * mean[k] + (1 - blend) * (s / PER_BIN);
  }
}

/** Planck's mean energy per mode, and the classical kT. */
const planck = (k: number) => quantum[k] / Math.expm1(quantum[k] / kT());
/** The spectrum per unit frequency (arbitrary units: ν² in 10²⁸ Hz², energy in eV). */
const spectrum = (perMode: (k: number) => number) => Array.from(nu, (v, k) => ({ x: v / 1e14, y: (v / 1e14) ** 2 * perMode(k) }));

/** CIE 1931 colour matching functions (Wyman, Sloan & Shirley's multi-lobe fit). */
function cie(l: number) {
  const g = (m: number, s1: number, s2: number) => Math.exp(-0.5 * ((l - m) / (l < m ? s1 : s2)) ** 2);
  return [
    1.056 * g(599.8, 37.9, 31.0) + 0.362 * g(442.0, 16.0, 26.7) - 0.065 * g(501.1, 20.4, 26.2),
    0.821 * g(568.8, 46.9, 40.5) + 0.286 * g(530.9, 16.3, 31.1),
    1.217 * g(437.0, 11.8, 36.0) + 0.681 * g(459.0, 26.0, 13.8),
  ];
}

/** XYZ of a spectrum given per mode (eV) in each bin. */
function xyz(perMode: (k: number) => number) {
  const out = [0, 0, 0];
  for (let k = 0; k < BINS; k++) {
    const l = C_NM / nu[k];
    if (l < 360 || l > 830) continue;
    const c = cie(l);
    const u = nu[k] ** 2 * perMode(k);
    for (let a = 0; a < 3; a++) out[a] += u * c[a];
  }
  return out;
}

/** The sun's surface, for a reference brightness. */
const SUN_Y = (() => {
  const t = K_EV * 5800;
  return xyz((k) => quantum[k] / Math.expm1(quantum[k] / t))[1];
})();

/** The colour of the glow: hue from the spectrum, brightness on a log scale (the eye's). */
function glowColor() {
  const [X, Y, Z] = xyz((k) => mean[k]);
  if (!(Y > 0)) return "rgb(0,0,0)";
  const rgb = [3.2406 * X - 1.5372 * Y - 0.4986 * Z, -0.9689 * X + 1.8758 * Y + 0.0415 * Z, 0.0557 * X - 0.204 * Y + 1.057 * Z].map((v) => Math.max(0, v));
  const top = Math.max(...rgb);
  const b = Math.max(0.03, Math.min(1, 1 + Math.log10(Y / SUN_Y) / 6));
  const gamma = (v: number) => (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
  return `rgb(${rgb.map((v) => Math.round(255 * gamma((b * v) / top))).join(",")})`;
}

const specPlot = new LinesPlot($<HTMLCanvasElement>("spectrum"), {
  xLabel: "frequency (10¹⁴ Hz; visible light from 4 to 7.5)",
  yLabel: "energy at each frequency",
  yMin: 0,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const binOf = (v: number) => Math.min(BINS - 1, Math.floor(v / DNU));
const VIOLET = binOf(C_NM / 400);

function draw1() {
  const p = spectrum(planck);
  const top = 1.8 * Math.max(...p.map((q) => q.y));
  const cap = (pts: { x: number; y: number }[]) => pts.map((q) => ({ x: q.x, y: Math.min(q.y, top) }));
  specPlot.set([
    { points: cap(spectrum(() => kT())), color: cssVar("--highlight"), dashed: true },
    { points: p, color: cssVar("--muted") },
    { points: cap(spectrum((k) => mean[k])), color: cssVar("--accent") },
  ]);
  $("glow").style.background = glowColor();
  const sum = (pts: { y: number }[]) => pts.reduce((a, q) => a + q.y, 0);
  $("nq").textContent = `×${(sum(spectrum((k) => mean[k])) / sum(p)).toFixed(2)}`;
  const m = spectrum((k) => mean[k]);
  const peak = m.reduce((a, q) => (q.y > a.y ? q : a), m[0]);
  $("peak").textContent = `${peak.x.toFixed(1)} × 10¹⁴ Hz (Planck: ${((2.821 * kT()) / H_EV / 1e14).toFixed(1)})`;
  const pk = binOf(peak.x * 1e14);
  $("per").textContent = `${(mean[pk] / kT()).toFixed(2)}, ${(mean[VIOLET] / kT()).toExponential(1)}`;
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("mode").onclick = () => {
  if (light === "quantum") light = "classical";
  else {
    // Back to lumps: each mode keeps the nearest whole number of quanta.
    light = "quantum";
    for (let k = 0; k < BINS; k++) for (let i = k * PER_BIN; i < (k + 1) * PER_BIN; i++) energy[i] = Math.round(energy[i] / quantum[k]) * quantum[k];
  }
  $("mode").textContent = light === "quantum" ? "Make light classical" : "Make light come in lumps";
};
tIn.oninput = () => {
  $("tv").textContent = `${Number(tIn.value).toLocaleString("en")} K`;
  draw1();
};

// ---- Part 2 ----

type Theory = "grains" | "wave";
let theory: Theory = "grains";
let running2 = false;
const lambdaIn = $<HTMLInputElement>("lambda");
const brightIn = $<HTMLInputElement>("bright");
const metal = $<HTMLCanvasElement>("metal");
const kePlot = new LinesPlot($<HTMLCanvasElement>("ke"), { xLabel: "frequency of the light (10¹⁴ Hz)", yLabel: "fastest electrons (eV)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** A grain of light on its way down (y from 0 at the top to 1 at the metal). */
interface Photon { x: number; y: number }
/** An electron flying up from the metal at its speed (heights per second). */
interface Electron { x: number; y: number; v: number }
let photons: Photon[] = [];
let electrons: Electron[] = [];
/** Wave theory: the energy each of the metal's surface electrons has soaked up (eV). */
const SURFACE = 120;
const soaked = new Float64Array(SURFACE);
/** Emissions in the last few seconds: (time, energy). */
let recent: { t: number; e: number }[] = [];
let clock = 0;
/** The fastest electrons seen at each frequency tried, by theory. */
const fastest = { grains: new Map<number, number>(), wave: new Map<number, number>() };

const photonEnergy = () => (H_EV * C_NM) / Number(lambdaIn.value);
/** Grains per second: a fixed power, so fewer grains of bluer light. */
const photonRate = () => 60 * Number(brightIn.value) * (Number(lambdaIn.value) / 500);

function emit(x: number, e: number) {
  electrons.push({ x, y: 1, v: 0.5 * Math.sqrt(e) });
  recent.push({ t: clock, e });
  const key = Math.round(Number(lambdaIn.value));
  const m = fastest[theory];
  m.set(key, Math.max(m.get(key) ?? 0, e));
}

/** Advance the light and the electrons by dt seconds. */
function step2(dt: number, animate: boolean) {
  clock += dt;
  if (theory === "grains") {
    // Grains arrive at random (Poisson); each that reaches the metal gives all of its energy to
    // one electron, which gets out if that is more than the work W, less a random loss on the way.
    let n = 0;
    const mu = photonRate() * dt;
    for (let p = Math.exp(-mu), s = p, u = rand(); u > s; ) {
      n++;
      p *= mu / n;
      s += p;
    }
    for (let k = 0; k < n; k++) {
      if (animate) photons.push({ x: rand(), y: 0 });
      else hit(rand());
    }
  } else {
    // A wave: energy pours steadily onto every surface electron, in proportion to the brightness,
    // whatever the colour; one leaves once it has W, with an energy set by the wave's strength.
    const b = Number(brightIn.value);
    for (let i = 0; i < SURFACE; i++) {
      soaked[i] += 0.6 * b * (0.5 + rand()) * dt;
      if (soaked[i] >= WORK) {
        soaked[i] = 0;
        emit((i + 0.5) / SURFACE, 0.25 * b * (0.5 + rand()));
      }
    }
  }
  if (animate) {
    for (const p of photons) p.y += 1.6 * dt;
    for (const p of photons) if (p.y >= 1) hit(p.x);
    photons = photons.filter((p) => p.y < 1);
    for (const e of electrons) e.y -= e.v * dt;
    electrons = electrons.filter((e) => e.y > 0);
  }
  recent = recent.filter((r) => r.t > clock - 5);
}

function hit(x: number) {
  const e = photonEnergy() - WORK + 0.2 * Math.log(1 - rand());
  if (e > 0 && rand() < 0.5) emit(x, e);
}

/** A rough colour for a wavelength (nm): ultraviolet drawn grey-violet, infrared dark red. */
function waveColor(l: number) {
  let [r, g, b] = [0, 0, 0];
  if (l < 380) [r, g, b] = [0.45, 0.35, 0.6];
  else if (l < 440) [r, g, b] = [(440 - l) / 60, 0, 1];
  else if (l < 490) [r, g, b] = [0, (l - 440) / 50, 1];
  else if (l < 510) [r, g, b] = [0, 1, (510 - l) / 20];
  else if (l < 580) [r, g, b] = [(l - 510) / 70, 1, 0];
  else if (l < 645) [r, g, b] = [1, (645 - l) / 65, 0];
  else [r, g, b] = [l > 700 ? 0.6 : 1, 0, 0];
  return `rgb(${[r, g, b].map((v) => Math.round(255 * v)).join(",")})`;
}

function draw2() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  const [Wc, Hc] = [metal.clientWidth, metal.clientHeight];
  metal.width = Math.round(Wc * dpr);
  metal.height = Math.round(Hc * dpr);
  const g = metal.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, Wc, Hc);
  const ground = 0.85 * Hc;
  const col = waveColor(Number(lambdaIn.value));
  const l = Number(lambdaIn.value);
  if (theory === "grains") {
    // Each grain a short wiggle, its wavelength drawn to scale (roughly).
    g.strokeStyle = col;
    g.lineWidth = 1.5;
    for (const p of photons) {
      const [x, y] = [p.x * Wc, p.y * ground];
      g.beginPath();
      for (let s = 0; s <= 24; s++) g.lineTo(x + 3 * Math.sin((s * 2 * Math.PI * 300) / l / 6), y - 24 + s);
      g.stroke();
    }
  } else {
    // Wavefronts across the whole width, fainter for dimmer light.
    g.strokeStyle = col;
    g.globalAlpha = Math.min(1, 0.15 + 0.1 * Number(brightIn.value));
    const gap = (0.06 * l) / 400 * ground;
    const off = ((clock * 1.6 * ground) % gap) - gap;
    for (let y = off; y < ground; y += gap) {
      g.beginPath();
      g.moveTo(0, y);
      g.lineTo(Wc, y);
      g.stroke();
    }
    g.globalAlpha = 1;
  }
  // The metal; in the wave theory its electrons glow as they soak up energy.
  g.fillStyle = cssVar("--muted");
  g.fillRect(0, ground, Wc, Hc - ground);
  g.fillStyle = cssVar("--cream");
  g.font = "12px system-ui, sans-serif";
  g.fillText("sodium", 8, Hc - 8);
  if (theory === "wave") {
    for (let i = 0; i < SURFACE; i++) {
      g.globalAlpha = 0.2 + 0.8 * (soaked[i] / WORK);
      g.fillStyle = cssVar("--highlight");
      g.fillRect(((i + 0.2) / SURFACE) * Wc, ground + 3, (0.6 / SURFACE) * Wc, 6);
    }
    g.globalAlpha = 1;
  }
  g.fillStyle = cssVar("--species-2");
  for (const e of electrons) {
    g.beginPath();
    g.arc(e.x * Wc, e.y * ground, 3, 0, 2 * Math.PI);
    g.fill();
  }
  // Readouts.
  const span = Math.min(5, clock);
  $("rate").textContent = span > 0.5 ? `${(recent.length / span).toFixed(1)}` : "–";
  $("kemax").textContent = recent.length ? `${Math.max(...recent.map((r) => r.e)).toFixed(2)} eV` : "none";
  // The plot: hν − W, and the fastest electrons at each colour tried.
  const line = Array.from({ length: 40 }, (_, k) => 2 + 0.33 * k).map((v) => ({ x: v, y: Math.max(0, H_EV * v * 1e14 - WORK) }));
  const pts = (m: Map<number, number>) => [...m].map(([lam, e]) => ({ x: C_NM / lam / 1e14, y: e })).sort((a, b) => a.x - b.x);
  kePlot.set([
    { points: line, color: cssVar("--muted") },
    { points: pts(fastest.grains), color: cssVar("--accent"), dots: true },
    { points: pts(fastest.wave), color: cssVar("--highlight"), dots: true },
  ]);
  planckCount(pts(fastest.grains));
}

/** Planck's constant from the fastest electrons (the slope of their energy against frequency,
 * over the colours that freed any), then Boltzmann's from the peak of part 1's spectrum (Wien:
 * hν_peak = 2.821 kT), and N = R/k. */
function planckCount(points: { x: number; y: number }[]) {
  const p = points.filter((q) => q.y > 0);
  if (p.length < 2 || p[p.length - 1].x - p[0].x < 1) {
    $("nh").textContent = "try two or more colours that free electrons";
    return;
  }
  const n = p.length;
  const [sx, sy] = [p.reduce((a, q) => a + q.x, 0), p.reduce((a, q) => a + q.y, 0)];
  const sxx = p.reduce((a, q) => a + q.x * q.x, 0);
  const sxy = p.reduce((a, q) => a + q.x * q.y, 0);
  const h = (n * sxy - sx * sy) / (n * sxx - sx * sx) / 1e14; // eV s
  // The spectrum's peak, between bins (a parabola through the highest and its neighbours).
  const u = Array.from(nu, (v, k) => v * v * mean[k]);
  let k = 1;
  for (let j = 1; j < BINS - 1; j++) if (u[j] > u[k]) k = j;
  const [a, b, c] = [u[k - 1], u[k], u[k + 1]];
  const peak = (k + 0.5 + (0.5 * (a - c)) / (a - 2 * b + c)) * DNU;
  if (!(b > 0)) {
    $("nh").textContent = `h = ${h.toExponential(3)} eV s; now run part 1 for its spectrum's peak`;
    return;
  }
  const kB = (h * peak) / (2.821 * Number(tIn.value)) * 1.602e-19; // J/K
  const nA = R / kB;
  $("nh").textContent = `h = ${h.toExponential(3)} eV s, k = ${kB.toExponential(3)} J/K: N = ${formatN(nA)}`;
  record("blackbody", nA, `h from ${p.length} colours, k from the spectrum's peak at ${tIn.value} K`);
}

const labels2 = () => {
  const l = Number(lambdaIn.value);
  const name = l < 380 ? "ultraviolet" : l < 450 ? "violet" : l < 490 ? "blue" : l < 570 ? "green" : l < 590 ? "yellow" : l < 620 ? "orange" : "red";
  $("lv").textContent = `${l} nm (${name}, ${photonEnergy().toFixed(2)} eV a grain)`;
  $("bv").textContent = `×${Number(brightIn.value).toFixed(1)}`;
};
lambdaIn.oninput = () => {
  labels2();
  recent = [];
};
brightIn.oninput = () => {
  labels2();
  recent = [];
};
$("run2").onclick = () => {
  running2 = !running2;
  $("run2").textContent = running2 ? "Pause" : "Run";
};
$("theory").onclick = () => {
  theory = theory === "grains" ? "wave" : "grains";
  $("theory").textContent = theory === "grains" ? "Wave theory" : "Grains of light";
  photons = [];
  soaked.fill(0);
  recent = [];
};

let lastTime = performance.now();
function frame(now: number) {
  const dt = Math.min(0.05, (now - lastTime) / 1000);
  lastTime = now;
  if (running) {
    sweep();
    measure(0.97);
    draw1();
  }
  if (running2) {
    step2(dt, true);
    draw2();
  }
  requestAnimationFrame(frame);
}

labels2();
draw1();
draw2();
new ResizeObserver(draw2).observe(metal);
requestAnimationFrame(frame);

declare global {
  interface Window {
    lightTest?: {
      box(t: number, kind: Light, sweeps: number): { planckRatio: number[]; perModeLow: number; perModeViolet: number; glow: string };
      photo(kind: Theory, lambda: number, bright: number, seconds: number): { rate: number; keMax: number };
    };
  }
}

window.lightTest = {
  /** Fill the box at temperature `t` (K) for `sweeps` sweeps, then average as many more: the
   * measured over Planck's energy per mode in a few bins near the peak, the energy per mode (in
   * kT) in the lowest bin and in violet, and the glow's colour. */
  box(t, kind, sweeps) {
    tIn.value = String(t);
    light = kind;
    energy.fill(0);
    for (let s = 0; s < sweeps; s++) sweep();
    mean.fill(0);
    const sum = new Float64Array(BINS);
    for (let s = 0; s < sweeps; s++) {
      sweep();
      measure(0);
      for (let k = 0; k < BINS; k++) sum[k] += mean[k] / sweeps;
    }
    mean.set(sum);
    draw1();
    const pk = binOf((2.821 * kT()) / H_EV);
    return {
      planckRatio: [pk - 2, pk, pk + 3].map((k) => mean[k] / planck(k)),
      perModeLow: mean[0] / kT(),
      perModeViolet: mean[VIOLET] / kT(),
      glow: glowColor(),
    };
  },
  /** Shine light of wavelength `lambda` (nm) and brightness `bright` for `seconds`: electrons
   * per second and the fastest's energy (eV). */
  photo(kind, lambda, bright, seconds) {
    theory = kind;
    lambdaIn.value = String(lambda);
    brightIn.value = String(bright);
    labels2();
    soaked.fill(0);
    recent = [];
    clock = 0;
    let count = 0;
    let keMax = 0;
    for (let k = 0; k < seconds * 100; k++) {
      step2(0.01, false);
      for (const r of recent) if (r.t > clock - 0.01 + 1e-9) {
        count++;
        keMax = Math.max(keMax, r.e);
      }
    }
    draw2();
    return { rate: count / seconds, keMax };
  },
};
