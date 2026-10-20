import { DiscSim, loadWasm, webgpuProblem } from "../../src/lib/wasm";
import { TimePlot } from "../../src/lib/plot";
import { cssColor, onThemeChange } from "../../src/lib/theme";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const canvas = $<HTMLCanvasElement>("canvas");
const overlay = $<HTMLDivElement>("overlay");
const runBtn = $<HTMLButtonElement>("run");
const revBtn = $<HTMLButtonElement>("reverse");
const resetBtn = $<HTMLButtonElement>("reset");
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
let nudged = ""; // what was nudged since the last reversal, for the arrival message

// Five traced cream molecules: their paths, in segments that break where a molecule wraps
// round the periodic box or the direction of time changes.
const TRACED = 5;
let traced: number[] = [];
let trails: { back: boolean; pts: number[] }[][] = [];
let tracing = false;
let boxSize = [0, 0];

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
  nudged = "";
  traced = [];
  trails = [];
  plot.clear();
  updateButtons();
  overlay.style.display = "grid";
  overlay.textContent = "Starting the GPU…";
  sizeCanvas();
  const n = Number(size.value);
  sim = await DiscSim.create(canvas, "cream", { n, density: 0.5, temperature: 1.0, seed: 7, warmup: 1000, blob: 0.2, aspect: canvas.width / canvas.height });
  applyColors();
  boxSize = Array.from(sim.box_size());
  traced = Array.from(sim.representatives(1, TRACED, 0.6));
  trails = traced.map(() => []);
  overlay.style.display = "none";
  setStatus("Press Run, let the cream spread, then reverse time.");
  updateButtons();
  sim.render();
  observe();
  trace();
}

/** Append the traced molecules' current positions to their paths and redraw the overlay. */
function trace() {
  if (!sim || tracing || !traced.length) return;
  tracing = true;
  const s = sim;
  const back = s.steps() < 0;
  s.positions_of(new Uint32Array(traced)).then((p: Float32Array) => {
    tracing = false;
    if (s !== sim) return;
    traced.forEach((_, k) => {
      const [x, y] = [p[2 * k], p[2 * k + 1]];
      const segs = trails[k];
      let seg = segs[segs.length - 1];
      const last = seg ? [seg.pts[seg.pts.length - 2], seg.pts[seg.pts.length - 1]] : null;
      const wrapped = !!last && (Math.abs(x - last[0]) > boxSize[0] / 2 || Math.abs(y - last[1]) > boxSize[1] / 2);
      if (!seg || seg.back !== back || wrapped) {
        seg = { back, pts: [] };
        segs.push(seg);
      }
      // Only record a point once the molecule has moved a little, so paths stay light.
      const end = seg.pts.length ? [seg.pts[seg.pts.length - 2], seg.pts[seg.pts.length - 1]] : null;
      if (!end || (x - end[0]) ** 2 + (y - end[1]) ** 2 > 0.04) seg.pts.push(x, y);
    });
    drawTrails(p);
  });
}

function drawTrails(now: Float32Array) {
  if (!sim) return;
  const r: number[] = [];
  const back = cssColor("--trace-back");
  traced.forEach((_, k) => {
    const c = cssColor(`--trace-${k + 1}`);
    // Outward paths first, return paths (smaller, white) on top: if they coincide, the white
    // dots sit inside the coloured ones.
    for (const backward of [false, true]) {
      const d = backward ? 0.09 : 0.18;
      const col = backward ? back : c;
      for (const seg of trails[k]) {
        if (seg.back !== backward) continue;
        for (let j = 0; j < seg.pts.length; j += 2) r.push(seg.pts[j] - d, seg.pts[j + 1] - d, seg.pts[j] + d, seg.pts[j + 1] + d, col[0], col[1], col[2], 0.9);
      }
    }
    // The molecule itself, marked.
    const [x, y] = [now[2 * k], now[2 * k + 1]];
    r.push(x - 0.75, y - 0.75, x + 0.75, y + 0.75, c[0], c[1], c[2], 0.45);
  });
  sim.set_overlay(new Float32Array(r));
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
    setStatus(`Back at t = 0. Every bit of all ${n} molecules' positions and velocities matches the start (state hash ${r.hash}), and every traced path was retraced exactly.`, "good");
  } else {
    setStatus(
      `Back at t = 0, but ${r.differ.toLocaleString()} of ${n} molecules are somewhere else, by up to ${r.maxDeviation.toFixed(1)} σ. ` +
        (nudged ? `A nudge of 2⁻³² σ to ${nudged} was enough.` : ""),
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
    trace();
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
  sim.reverse();
  plot.mark(elapsed * sim.dt(), "reversed");
  setStatus(
    forward
      ? nudged
        ? `Reversed, after nudging ${nudged} by 2⁻³² σ. Watch whether the white paths stay on the coloured ones.`
        : "Reversed exactly. Watch the cream un-mix and the white paths retrace the coloured ones."
      : "Forward again.",
  );
  if (!forward) nudged = "";
  running = true;
  updateButtons();
};

function nudge(what: string, f: (s: DiscSim) => void) {
  if (!sim) return;
  f(sim);
  nudged = nudged ? `${nudged} and ${what}` : what;
  plot.mark(elapsed * sim.dt(), "nudge");
  setStatus(`Nudged ${what} by 2⁻³² σ.${sim.steps() > 0 ? " Now reverse time." : ""}`);
}
$("nudge-traced").onclick = () => nudge("the 5 traced molecules", (s) => traced.forEach((i) => s.flip_bit(i)));
$("nudge-cream").onclick = () => nudge("all the cream", (s) => s.flip_species_bit(1 << 1));
$("nudge-all").onclick = () => nudge("every molecule", (s) => s.flip_species_bit(0b11));

resetBtn.onclick = () => void create();
size.onchange = () => void create();
speed.oninput = () => ($("speedv").textContent = `${speed.value} steps/frame`);
onThemeChange(applyColors);

// Hooks for the browser tests: run a full round trip without the UI loop.
declare global {
  interface Window {
    reversalTest?: {
      ready: Promise<void>;
      roundTrip(steps: number, nudge: boolean): Promise<{ identical: boolean; differ: number; maxDeviation: number }>;
      /** Recorded path points of the traced molecules, going and coming back. */
      trails(): { going: number; back: number; traced: number };
    };
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
  trails() {
    const count = (back: boolean) => trails.flat().filter((g) => g.back === back).reduce((n, g) => n + g.pts.length / 2, 0);
    return { going: count(false), back: count(true), traced: traced.length };
  },
};
