import { $, cssVar } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";

// A flashing ratchet: overdamped Brownian motors in a sawtooth potential of period 1 (one step),
// height V0, its crest at A; friction 1, so D = kT; the grip switches off for `off` and on for ON.

const MOTORS = 30;
const V0 = 5; // in kT at room temperature (kT = 1)
const A = 0.2; // the crest's place in each period: steep side behind, gentle ahead
const ON = 0.05;
const DT = 1e-4;
const STEPS_PER_FRAME = 200;

type Grip = "flashing" | "on" | "off";
let grip: Grip = "flashing";
let x = new Float64Array(MOTORS);
let time = 0;
let fuel = 0; // energy put in by switching on (kT units), in all
let workDone = 0; // against the load
let gripOn = true;
let phase = 0;
let running = false;
let line: { x: number; y: number }[] = [];
let fuelLine: { x: number; y: number }[] = [];

const tIn = $<HTMLInputElement>("t");
const fIn = $<HTMLInputElement>("f");
const offIn = $<HTMLInputElement>("off");
const plot = new LinesPlot($<HTMLCanvasElement>("drift"), { xLabel: "time", yLabel: "steps", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

let seed = 9;
function rand() {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
let spare: number | null = null;
function gauss() {
  if (spare !== null) {
    const s = spare;
    spare = null;
    return s;
  }
  const r = Math.sqrt(-2 * Math.log(1 - rand()));
  const a = 2 * Math.PI * rand();
  spare = r * Math.sin(a);
  return r * Math.cos(a);
}

/** The sawtooth: rising from 0 to V0 over [0, A), falling back over [A, 1). */
function potential(u: number) {
  const f = u - Math.floor(u);
  return f < A ? (V0 * f) / A : (V0 * (1 - f)) / (1 - A);
}
function force(u: number) {
  const f = u - Math.floor(u);
  return f < A ? -V0 / A : V0 / (1 - A);
}

function reset() {
  // Every motor starts at the bottom of a groove.
  x = new Float64Array(MOTORS);
  time = 0;
  fuel = 0;
  workDone = 0;
  phase = 0;
  gripOn = grip !== "off";
  line = [];
  fuelLine = [];
}

function step(n: number) {
  const kT = Number(tIn.value);
  const load = Number(fIn.value);
  const off = Number(offIn.value);
  const noise = Math.sqrt(2 * kT * DT);
  for (let s = 0; s < n; s++) {
    // The grip's schedule.
    if (grip === "flashing") {
      phase += DT;
      const want = phase % (ON + off) < ON;
      if (want && !gripOn) for (let i = 0; i < MOTORS; i++) fuel += potential(x[i]);
      gripOn = want;
    } else gripOn = grip === "on";
    for (let i = 0; i < MOTORS; i++) {
      const f = (gripOn ? force(x[i]) : 0) - load;
      const dx = f * DT + noise * gauss();
      workDone += load * dx;
      x[i] += dx;
    }
    time += DT;
  }
}

const mean = () => x.reduce((a, v) => a + v, 0) / MOTORS;

function draw() {
  const c = $<HTMLCanvasElement>("track");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const [W, H] = [c.clientWidth, c.clientHeight];
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, W, H);
  // Ten steps across, following the motors' average position.
  const view = 10;
  const x0 = Math.floor(mean()) - 4;
  const px = (u: number) => ((u - x0) / view) * W;
  const lane = H / MOTORS;
  for (let i = 0; i < MOTORS; i++) {
    const y = (i + 0.85) * lane;
    if (gripOn) {
      g.strokeStyle = cssVar("--muted");
      g.lineWidth = 1;
      g.beginPath();
      for (let k = 0; k <= 200; k++) {
        const u = x0 + (view * k) / 200;
        const yy = y - (potential(u) / V0) * lane * 0.75;
        if (k === 0) g.moveTo(px(u), yy);
        else g.lineTo(px(u), yy);
      }
      g.stroke();
    } else {
      g.strokeStyle = cssVar("--rule");
      g.beginPath();
      g.moveTo(0, y);
      g.lineTo(W, y);
      g.stroke();
    }
    const u = x[i];
    const yy = y - (gripOn ? (potential(u) / V0) * lane * 0.75 : 0) - 0.18 * lane;
    g.fillStyle = cssVar("--species-2");
    g.beginPath();
    g.arc(px(u), yy, Math.max(2, lane * 0.22), 0, 2 * Math.PI);
    g.fill();
  }
  g.fillStyle = cssVar("--cream");
  g.font = "12px system-ui, sans-serif";
  g.fillText(gripOn ? "grip on" : "grip off", 8, 14);
  plot.set([
    { points: line, color: cssVar("--accent") },
    { points: fuelLine, color: cssVar("--muted"), dashed: true },
  ]);
  const m = mean();
  $("speed").textContent = time > 0.5 ? (m / time).toFixed(2) : "–";
  $("fuel").textContent = m > 1 ? `${(fuel / MOTORS / m).toFixed(1)} kT` : "–";
  $("work").textContent = m > 1 && Number(fIn.value) > 0 ? `${(workDone / MOTORS / m).toFixed(1)} kT` : "–";
}

function frame() {
  if (running) {
    step(STEPS_PER_FRAME);
    line.push({ x: time, y: mean() });
    fuelLine.push({ x: time, y: fuel / MOTORS / 10 });
    if (line.length > 4000) {
      line.shift();
      fuelLine.shift();
    }
    draw();
  }
  requestAnimationFrame(frame);
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("mode").onclick = () => {
  grip = grip === "flashing" ? "on" : grip === "on" ? "off" : "flashing";
  $("mode").textContent = `Grip: ${grip === "flashing" ? "flashing" : grip === "on" ? "always on" : "always off"}`;
  // A fresh baseline for the speed, from here.
  line = [];
  fuelLine = [];
  // Shift back by whole steps (the track looks the same), so the distance counts from here.
  const m = Math.round(mean());
  for (let i = 0; i < MOTORS; i++) x[i] -= m;
  time = 0;
  fuel = 0;
  workDone = 0;
};
$("reset").onclick = () => {
  reset();
  draw();
};
// Click a lane: kick that motor a step forwards or back at random (as a passing molecule might).
$<HTMLCanvasElement>("track").addEventListener("pointerdown", (e) => {
  const c = e.currentTarget as HTMLCanvasElement;
  const r = c.getBoundingClientRect();
  const i = Math.min(MOTORS - 1, Math.floor(((e.clientY - r.top) / r.height) * MOTORS));
  x[i] += rand() < 0.5 ? 0.6 : -0.6;
  draw();
});
const labels = () => {
  $("tv").textContent = `×${Number(tIn.value).toFixed(2)}`;
  $("fv").textContent = Number(fIn.value) ? `${Number(fIn.value).toFixed(2)} kT per step` : "none";
  $("offv").textContent = `${Number(offIn.value).toFixed(3)} (on: ${ON})`;
};
for (const el of [tIn, fIn, offIn]) el.addEventListener("input", labels);
labels();
reset();
draw();
new ResizeObserver(draw).observe($("track"));
requestAnimationFrame(frame);

declare global {
  interface Window {
    motorTest?: { run(mode: Grip, kT: number, load: number, time: number): { speed: number; fuelPerStep: number } };
  }
}

window.motorTest = {
  /** Start again with this grip, temperature and load, and run for `t`: speed (steps per unit
   * time) and fuel burnt per step (kT). */
  run(mode, kT, load, t) {
    grip = mode;
    tIn.value = String(kT);
    fIn.value = String(load);
    labels();
    reset();
    step(Math.round(t / DT));
    draw();
    const m = mean();
    return { speed: m / time, fuelPerStep: fuel / MOTORS / m };
  },
};
