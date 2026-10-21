import { $, cssVar } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";
import { RelGas, juttnerEnergy } from "./gas";

// A relativistic hard-disk gas (gas.ts; c = 1, m = 1). Checked in Node: energy and momentum
// conserved to 1e-12 over 6,000 steps; the thermometer kT = ⟨p·v⟩/2 and the energy agree with
// Jüttner's 1 + kT + kT²/(1 + kT) within 0.6% from kT 0.05 to 1.8.

const DT = 0.02;
const STEPS = 25; // per frame: half a time unit
const N1 = 600;
const L1 = 50;
const N2 = 300;
const L2 = 35;
/** Part 2: energy per molecule (mc²) of the cold and the hot gas at the start. */
const COLD = 1.01;
const HOT = 2;
/** The push on each molecule (mc² per unit length). */
const PUSH = 0.004;
/** Heat added or taken by a button, per molecule (mc²). */
const DOSE = 0.25;

// ---- Drawing ----

function speedColor(v: number) {
  const [a, b, c] = [cssColor("--species-2"), cssColor("--cream"), cssColor("--highlight")];
  const [p, q, t] = v < 0.5 ? [a, b, v / 0.5] : [b, c, (v - 0.5) / 0.5];
  return `rgb(${[0, 1, 2].map((k) => Math.round(255 * (p[k] + (q[k] - p[k]) * Math.min(1, t)))).join(",")})`;
}

function drawGas(canvas: HTMLCanvasElement, g: RelGas, bar: number | null = null) {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.round(canvas.clientWidth * dpr);
  canvas.height = Math.round(canvas.clientHeight * dpr);
  const c = canvas.getContext("2d")!;
  const [W, H] = [canvas.width, canvas.height];
  c.fillStyle = cssVar("--stage-bg");
  c.fillRect(0, 0, W, H);
  const s = W / g.L;
  for (let i = 0; i < g.n; i++) {
    const e = g.energyOf(i);
    c.fillStyle = speedColor(Math.hypot(g.px[i], g.py[i]) / e);
    c.beginPath();
    c.arc(g.x[i] * s, H - g.y[i] * s, 0.5 * s, 0, 2 * Math.PI);
    c.fill();
  }
  if (bar !== null) {
    // How far the molecules have flowed, on average, in box widths: a bar along the bottom.
    const f = ((bar / g.L) % 1 + 1) % 1;
    c.fillStyle = cssVar("--cream");
    c.globalAlpha = 0.8;
    c.fillRect(0, H - 6 * dpr, W * f, 6 * dpr);
    c.globalAlpha = 1;
    c.font = `${12 * dpr}px system-ui, sans-serif`;
    c.fillText(`${(bar / g.L).toFixed(2)} boxes`, 6 * dpr, H - 12 * dpr);
  }
}

// ---- Part 1 ----

let gas = new RelGas(N1, L1, 1.05, 1);
let running = false;
let time = 0;
const hist = new Float64Array(30);
let heating: { x: number; y: number } | null = null;
let energyLine: { x: number; y: number }[] = [];
let etPoints: { x: number; y: number }[] = [];
let recentKT: number[] = [];
let frames = 0;

const speedPlot = new LinesPlot($<HTMLCanvasElement>("speeds"), { xLabel: "speed (in c)", yLabel: "share of molecules", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const etPlot = new LinesPlot($<HTMLCanvasElement>("et"), { xLabel: "kT (mc²)", yLabel: "energy per molecule − mc²", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const energyPlot = new LinesPlot($<HTMLCanvasElement>("energy"), { xLabel: "time (σ/c)", yLabel: "energy per molecule (mc²)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** Speed distributions on [0, 1.5) at kT, normalised to bins of 1/30 c: Jüttner (2D,
 * ∝ γ⁴ v e^(−γ/kT)) and Maxwell (∝ v e^(−v²/2kT)). */
function theory(t: number) {
  const bins = 45;
  const w = 1 / 30;
  const j = Array.from({ length: bins }, (_, k) => {
    const v = (k + 0.5) * w;
    if (v >= 1) return 0;
    const g = 1 / Math.sqrt(1 - v * v);
    return g ** 4 * v * Math.exp(-(g - 1) / t);
  });
  const m = Array.from({ length: bins }, (_, k) => {
    const v = (k + 0.5) * w;
    return v * Math.exp(-(v * v) / (2 * t));
  });
  // Maxwell's normalisation over all speeds (∫ v e^(−v²/2kT) dv = kT); Jüttner's numerically.
  const sj = j.reduce((a, x) => a + x, 0);
  return {
    juttner: j.map((y, k) => ({ x: (k + 0.5) * w, y: y / sj })),
    maxwell: m.map((y, k) => ({ x: (k + 0.5) * w, y: (y * w) / t })),
  };
}

function readout1() {
  const n = gas.n;
  const h = new Float64Array(30);
  let vs = 0;
  for (let i = 0; i < n; i++) {
    const v = Math.hypot(gas.px[i], gas.py[i]) / gas.energyOf(i);
    vs += v;
    h[Math.min(29, Math.floor(v * 30))]++;
  }
  for (let k = 0; k < 30; k++) hist[k] = 0.9 * hist[k] + (0.1 * h[k]) / n;
  const kt = gas.kT();
  recentKT.push(kt);
  if (recentKT.length > 20) recentKT.shift();
  const t = recentKT.reduce((a, x) => a + x, 0) / recentKT.length;
  const e = gas.totals().e / n;
  energyLine.push({ x: time, y: e });
  if (energyLine.length > 1500) energyLine.shift();
  if (frames % 10 === 0) {
    etPoints.push({ x: t, y: e - 1 });
    if (etPoints.length > 300) etPoints.shift();
  }
  const th = theory(t);
  speedPlot.set([
    { points: [{ x: 1, y: 0 }, { x: 1, y: Math.max(...hist, ...th.juttner.map((p) => p.y)) * 1.1 }], color: cssVar("--rule"), dashed: true },
    { points: th.maxwell, color: cssVar("--highlight"), dashed: true },
    { points: th.juttner, color: cssVar("--muted") },
    { points: Array.from(hist, (y, k) => ({ x: (k + 0.5) / 30, y })), color: cssVar("--accent"), dots: true },
  ]);
  const tMax = Math.max(0.5, ...etPoints.map((p) => p.x)) * 1.1;
  const curve = Array.from({ length: 60 }, (_, k) => (k / 59) * tMax);
  etPlot.set([
    { points: curve.map((x) => ({ x, y: x })), color: cssVar("--highlight"), dashed: true },
    { points: curve.map((x) => ({ x, y: juttnerEnergy(x) - 1 })), color: cssVar("--muted") },
    { points: etPoints, color: cssVar("--accent"), dots: true },
  ]);
  energyPlot.set([{ points: energyLine, color: cssVar("--accent") }]);
  $("kt").textContent = t.toFixed(3);
  $("v").textContent = (vs / n).toFixed(3);
  $("e").textContent = `${e.toFixed(3)} (of which rest mass 1)`;
  // dE/dT from Jüttner's formula at this temperature: 1 + (2t + t²)/(1 + t)².
  $("c").textContent = `${(1 + (2 * t + t * t) / (1 + t) ** 2).toFixed(2)} k at this kT (Newton, 2D: 1 k)`;
  drawGas($<HTMLCanvasElement>("gas"), gas);
}

function reset1() {
  gas = new RelGas(N1, L1, 1.05, 1);
  time = 0;
  hist.fill(0);
  energyLine = [];
  etPoints = [];
  recentKT = [];
  readout1();
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("heat").onclick = () => {
  gas.heat(DOSE * gas.n);
  readout1();
};
$("cool").onclick = () => {
  const room = gas.totals().e - gas.n;
  gas.heat(-Math.min(DOSE * gas.n, 0.9 * room));
  readout1();
};
$("reset").onclick = reset1;
{
  const c = $<HTMLCanvasElement>("gas");
  const at = (e: PointerEvent) => {
    const r = c.getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * L1, y: (1 - (e.clientY - r.top) / r.height) * L1 };
  };
  c.addEventListener("pointerdown", (e) => {
    c.setPointerCapture(e.pointerId);
    heating = at(e);
  });
  c.addEventListener("pointermove", (e) => heating && (heating = at(e)));
  c.addEventListener("pointerup", () => (heating = null));
  c.addEventListener("pointercancel", () => (heating = null));
}

/** The hot finger: molecules within 5 σ of it speed up by 3% in momentum, each frame. */
function heatAt(p: { x: number; y: number }) {
  for (let i = 0; i < gas.n; i++) {
    if ((gas.x[i] - p.x) ** 2 + (gas.y[i] - p.y) ** 2 < 25) {
      gas.px[i] *= 1.03;
      gas.py[i] *= 1.03;
    }
  }
}

// ---- Part 2 ----

interface Body {
  gas: RelGas;
  /** The molecules' mean x (unwrapped) at recent times: (t, x), and q = ∫ P dt / N, the
   * momentum per molecule given so far, integrated over time. */
  track: { x: number; y: number; q: number }[];
  /** Energy and pressure × area at rest, per molecule (mc²). */
  rest: { e: number; pa: number };
}
let cold: Body;
let hot: Body;
let time2 = 0;
let pushing = false;
/** Steps until the hot gas's pressure, after a heating, is read again. */
let hotPressurePending = 0;

const trackPlot = new LinesPlot($<HTMLCanvasElement>("track"), { xLabel: "time (σ/c)", yLabel: "how far it has gone (σ)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** Where the molecules have got to, on average (unwrapped x): its rate of change is their mean
 * velocity, which in a gas in equilibrium is the speed of the gas as a whole. */
function centre(g: RelGas) {
  let s = 0;
  for (let i = 0; i < g.n; i++) s += g.x[i] + g.L * g.wrapX[i];
  return s / g.n;
}

/** Inertia per molecule, m in P = N m v: the molecules' mean x moves by ∫ v dt = q / m, so m is
 * the slope of q against it: a least-squares line, which averages out the jiggle of the mean
 * and, by its intercept, the short lag after each push starts (until collisions catch up, the
 * molecules respond to it one by one). Works however the push came. */
function inertia(b: Body) {
  const t = b.track;
  if (t.length < 10 || t[t.length - 1].q < 0.5) return NaN;
  let [sq, sx, sqq, sqx] = [0, 0, 0, 0];
  for (const p of t) {
    sq += p.q;
    sx += p.y;
    sqq += p.q * p.q;
    sqx += p.q * p.y;
  }
  const n = t.length;
  return (sqq - (sq * sq) / n) / (sqx - (sq * sx) / n);
}

/** Add a point to a body's track (after its gas has moved on by `dt`). */
function addTrack(b: Body, dt: number) {
  const last = b.track[b.track.length - 1];
  const q = last.q + (b.gas.totals().px / b.gas.n) * dt;
  b.track.push({ x: last.x + dt, y: centre(b.gas), q });
  if (b.track.length > 20000) b.track.splice(0, 10000);
}

function reset2(seed = 0) {
  const make = (e: number, seed: number): Body => {
    const g = new RelGas(N2, L2, e, seed);
    g.step(500, DT); // let it settle into its distribution
    g.pressureArea();
    g.step(1500, DT);
    return { gas: g, track: [], rest: { e: g.totals().e / g.n, pa: g.pressureArea() / g.n } };
  };
  cold = make(COLD, 2 + 2 * seed);
  hot = make(HOT, 3 + 2 * seed);
  time2 = 0;
  for (const b of [cold, hot]) b.track.push({ x: 0, y: centre(b.gas), q: 0 });
  readout2();
}

/** Heat a gas in its own frame (it keeps its speed, so its momentum rises: heat has momentum). */
function heatMoving(g: RelGas, q: number) {
  const { e, px, py } = g.totals();
  const [bx, by] = [px / e, py / e];
  const b2 = bx * bx + by * by;
  const gam = 1 / Math.sqrt(1 - b2);
  const boost = (sign: number) => {
    for (let i = 0; i < g.n; i++) {
      const ei = g.energyOf(i);
      const bp = bx * g.px[i] + by * g.py[i];
      const k = (b2 > 1e-30 ? ((gam - 1) * bp) / b2 : 0) - sign * gam * ei;
      g.px[i] += k * bx;
      g.py[i] += k * by;
    }
  };
  boost(1);
  g.heat(q);
  boost(-1);
}

function readout2() {

  trackPlot.set([
    { points: cold.track.map((p) => ({ x: p.x, y: p.y - cold.track[0].y })), color: cssVar("--species-2") },
    { points: hot.track.map((p) => ({ x: p.x, y: p.y - hot.track[0].y })), color: cssVar("--highlight") },
  ]);
  const show = (b: Body, id: string) => {
    const m = inertia(b);
    $(id).textContent = `${Number.isFinite(m) ? m.toFixed(2) : "–"}; ${b.rest.e.toFixed(2)}, ${(b.rest.e + b.rest.pa).toFixed(2)}`;
  };
  show(cold, "mc");
  show(hot, "mh");
  const v = (b: Body) => {
    const t = b.gas.totals();
    return t.px / t.e;
  };
  $("vv").textContent = `${v(cold).toFixed(3)}, ${v(hot).toFixed(3)}`;
  drawGas($<HTMLCanvasElement>("cold"), cold.gas, centre(cold.gas) - cold.track[0].y);
  drawGas($<HTMLCanvasElement>("hot"), hot.gas, centre(hot.gas) - hot.track[0].y);
}

{
  const b = $("push");
  const on = (e: PointerEvent) => {
    b.setPointerCapture(e.pointerId);
    pushing = true;
    b.classList.add("on");
  };
  const off = () => {
    pushing = false;
    b.classList.remove("on");
  };
  b.addEventListener("pointerdown", on);
  b.addEventListener("pointerup", off);
  b.addEventListener("pointercancel", off);
}
$("heat2").onclick = () => {
  heatMoving(hot.gas, DOSE * 2 * hot.gas.n);
  // Its energy and pressure at rest (in its own frame) after the heating, measured afresh.
  hot.rest.e += DOSE * 2;
  hot.gas.pressureArea();
  hot.track = [{ x: time2, y: centre(hot.gas), q: 0 }];
  hotPressurePending = 1500;
  readout2();
};
$("reset2").onclick = () => reset2();

function frame() {
  if (running) {
    if (heating) heatAt(heating);
    gas.step(STEPS, DT);
    time += STEPS * DT;
    frames++;
    readout1();
  }
  // Part 2 runs whenever it is visible enough to matter: always (it is cheap).
  const f = pushing ? PUSH : 0;
  for (const b of [cold, hot]) {
    b.gas.step(STEPS, DT, f);
    addTrack(b, STEPS * DT);
  }
  time2 += STEPS * DT;
  if (hotPressurePending > 0 && (hotPressurePending -= STEPS) <= 0) hot.rest.pa = hot.gas.pressureArea() / hot.gas.n;
  readout2();
  requestAnimationFrame(frame);
}

reset1();
reset2();
requestAnimationFrame(frame);

declare global {
  interface Window {
    relativityTest?: {
      heat(doses: number, settle: number): { kT: number; e: number; juttner: number; vMean: number; vMax: number; drift: number };
      push(time: number, seed: number, f: number): { cold: { m: number; e: number; h: number }; hot: { m: number; e: number; h: number } };
    };
  }
}

window.relativityTest = {
  /** Start again, add `doses` doses of heat, settle for `settle` steps: kT, energy per molecule,
   * Jüttner's energy at that kT, the mean and largest speed, and the energy's drift while settling. */
  heat(doses, settle) {
    running = false;
    reset1();
    for (let k = 0; k < doses; k++) gas.heat(DOSE * gas.n);
    gas.step(2000, DT);
    const e0 = gas.totals().e;
    let kt = 0;
    for (let k = 0; k < 20; k++) {
      gas.step(settle / 20, DT);
      kt += gas.kT() / 20;
    }
    const e1 = gas.totals().e;
    let [vs, vmax] = [0, 0];
    for (let i = 0; i < gas.n; i++) {
      const v = Math.hypot(gas.px[i], gas.py[i]) / gas.energyOf(i);
      vs += v / gas.n;
      vmax = Math.max(vmax, v);
    }
    readout1();
    return { kT: kt, e: e1 / gas.n, juttner: juttnerEnergy(kt), vMean: vs, vMax: vmax, drift: (e1 - e0) / gas.n };
  },
  /** Start part 2 again (seed `seed`) and push both for `time` with `f` on each molecule:
   * inertia per molecule, and the energy and the enthalpy (E + pA) per molecule at rest. */
  push(t, seed, f) {
    reset2(seed);
    for (let k = 0; k < t / (STEPS * DT); k++) {
      for (const b of [cold, hot]) {
        b.gas.step(STEPS, DT, f);
        addTrack(b, STEPS * DT);
      }
      time2 += STEPS * DT;
    }
    readout2();
    const r = (b: Body) => ({ m: inertia(b), e: b.rest.e, h: b.rest.e + b.rest.pa });
    return { cold: r(cold), hot: r(hot) };
  },
};
