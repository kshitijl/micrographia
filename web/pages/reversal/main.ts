import { DiscSim, loadWasm, webgpuProblem } from "../../src/lib/wasm";
import { TimePlot } from "../../src/lib/plot";
import { cssColor, onThemeChange } from "../../src/lib/theme";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const canvas = $<HTMLCanvasElement>("canvas");
const overlay = $<HTMLDivElement>("overlay");
const runBtn = $<HTMLButtonElement>("run");
const revBtn = $<HTMLButtonElement>("reverse");
const resetBtn = $<HTMLButtonElement>("reset");
const nudge = $<HTMLInputElement>("nudge");
const speed = $<HTMLInputElement>("speed");
const size = $<HTMLSelectElement>("size");
const status = $<HTMLDivElement>("status");

const css = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const plot = new TimePlot($<HTMLCanvasElement>("plot"), {
  yMin: 0,
  yMax: 1,
  yLabel: "mixed",
  xLabel: "elapsed time (τ)",
  colors: () => [css("--plot-forward"), css("--plot-back")],
  ink: () => css("--muted"),
  rule: () => css("--rule"),
});

let sim: DiscSim | null = null;
let running = false;
let elapsed = 0; // steps run in either direction
let observing = false;
let paused = false; // the UI loop yields to test hooks
let mix0: number | null = null; // raw mixing of the starting blob (coarse bins straddle its edge)

function setStatus(text: string, kind: "" | "good" | "bad" = "") {
  status.textContent = text;
  status.className = `status ${kind}`;
}

function applyColors() {
  if (!sim) return;
  const c = [...cssColor("--coffee"), ...cssColor("--cream"), 0.3, 0.5, 0.8, 1, 0.8, 0.3, 0.3, 1, ...cssColor("--highlight"), ...cssColor("--stage-bg")];
  sim.set_colors(new Float32Array(c));
}

function sizeCanvas() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.max(1, Math.round(canvas.clientWidth * dpr));
  canvas.height = Math.max(1, Math.round(canvas.clientHeight * dpr));
}

async function create() {
  sim?.free();
  sim = null;
  running = false;
  elapsed = 0;
  mix0 = null;
  plot.clear();
  updateButtons();
  overlay.style.display = "grid";
  overlay.textContent = "Starting the GPU…";
  sizeCanvas();
  const n = Number(size.value);
  sim = await DiscSim.create(canvas, "cream", { n, density: 0.5, temperature: 1.0, seed: 7, warmup: 1000, blob: 0.2 });
  applyColors();
  overlay.style.display = "none";
  setStatus("Press Run, let the cream spread, then reverse time.");
  updateButtons();
  sim.render();
  observe();
}

function updateButtons() {
  runBtn.textContent = running ? "Pause" : "Run";
  revBtn.disabled = !sim || sim.steps() === 0;
}

function observe() {
  if (!sim || observing) return;
  observing = true;
  const s = sim;
  const back = s.steps() < 0;
  const t = elapsed * s.dt();
  s.observe().then((r: { mixing: number; temperature: number }) => {
    observing = false;
    if (s !== sim) return;
    mix0 ??= r.mixing;
    const m = Math.max(0, (r.mixing - mix0) / (1 - mix0));
    $("mix").textContent = `${Math.round(m * 100)}%`;
    $("temp").textContent = r.temperature.toFixed(3);
    plot.add(t, m, back ? 1 : 0);
    plot.draw();
  });
}

async function arrived() {
  if (!sim) return;
  running = false;
  // Restore forward-running velocities: the state should now be the initial one.
  sim.reverse();
  sim.set_highlight(-1);
  const r = await sim.compare_to_initial();
  const n = sim.n().toLocaleString();
  if (r.identical) {
    setStatus(`Back at t = 0. Every bit of all ${n} molecules' positions and velocities matches the start (state hash ${r.hash}).`, "good");
  } else {
    setStatus(
      `Back at t = 0, but ${r.differ.toLocaleString()} of ${n} molecules are somewhere else, by up to ${r.maxDeviation.toFixed(1)} σ. ` +
        `A nudge of 2⁻³² σ to one molecule was enough.`,
      "bad",
    );
  }
  updateButtons();
}

function frame() {
  requestAnimationFrame(frame);
  if (!sim || paused) return;
  const s = sim;
  if (running && !observing) {
    let k = Number(speed.value);
    const steps = s.steps();
    if (steps < 0) k = Math.min(k, -steps);
    if (k > 0) {
      s.step(k);
      elapsed += k;
    }
    $("time").textContent = (Math.abs(s.steps()) * s.dt()).toFixed(1);
    $("dir").textContent = s.steps() < 0 ? "backward" : "forward";
    observe();
    if (steps === 0 || s.steps() === 0) updateButtons();
    if (steps < 0 && s.steps() === 0) void arrived();
  }
  s.render();
}

runBtn.onclick = () => {
  if (!sim) return;
  running = !running;
  if (running && sim.steps() === 0) setStatus("Running forward. Reverse time whenever you like.");
  updateButtons();
};

revBtn.onclick = () => {
  if (!sim || sim.steps() === 0) return;
  const forward = sim.steps() > 0;
  if (forward && nudge.checked) {
    const i = sim.central_marked();
    sim.flip_bit(i);
    sim.set_highlight(i);
  }
  sim.reverse();
  plot.mark(elapsed * sim.dt(), "reversed");
  setStatus(
    forward
      ? nudge.checked
        ? "Reversed, with one molecule (red) nudged by 2⁻³² σ."
        : "Reversed exactly. Watch the cream un-mix."
      : "Forward again.",
  );
  running = true;
  updateButtons();
};

resetBtn.onclick = () => void create();
size.onchange = () => void create();
speed.oninput = () => ($("speedv").textContent = `${speed.value} steps/frame`);
onThemeChange(applyColors);

// Hooks for the browser tests: run a full round trip without the UI loop.
declare global {
  interface Window {
    reversalTest?: { ready: Promise<void>; roundTrip(steps: number, nudge: boolean): Promise<{ identical: boolean; differ: number; maxDeviation: number }> };
  }
}

async function main() {
  const problem = await webgpuProblem();
  if (problem) {
    overlay.textContent = problem;
    throw new Error(problem);
  }
  await loadWasm();
  await create();
  requestAnimationFrame(frame);
}

const ready = main();
window.reversalTest = {
  ready,
  async roundTrip(steps: number, withNudge: boolean) {
    await ready;
    const s = sim!;
    paused = true;
    s.step(steps);
    if (withNudge) s.flip_bit(s.central_marked());
    s.reverse();
    s.step(steps);
    s.reverse();
    const r = await s.compare_to_initial();
    paused = false;
    return r;
  },
};
