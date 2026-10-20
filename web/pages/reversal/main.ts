import { DiscSim, loadWasm, webgpuProblem } from "../../src/lib/wasm";
import { TimePlot } from "../../src/lib/plot";
import { cssColor, onThemeChange } from "../../src/lib/theme";
import { showAbout } from "../../src/lib/host";

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

// Four traced molecules (two cream, two coffee). Their outward path is a stack of samples; on
// the way back the page lands on the same step counts and compares positions bit for bit (f64
// holds them exactly). A sample that matches is erased, like XOR-ing it out; one that doesn't
// stays, with the molecule's actual position drawn white next to it.
const TRACED = 4;
let traced: number[] = [];
let path: { step: number; pos: Float64Array }[] = [];
let kept: number[][] = []; // per molecule: outward points that the way back did not match (x, y, ...)
let strays: number[][] = []; // per molecule: where the molecule actually was at those steps
let tracing = false;

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
  path = [];
  plot.clear();
  updateButtons();
  overlay.style.display = "grid";
  overlay.textContent = "Starting the GPU…";
  sizeCanvas();
  const n = Number(size.value);
  sim = await DiscSim.create(canvas, "cream", { n, density: 0.5, temperature: 1.0, seed: 7, warmup: 1000, blob: 0.2, aspect: canvas.width / canvas.height });
  applyColors();
  showAbout(sim, ["coffee", "cream"]);
  traced = await pickTraced(sim);
  path = [];
  kept = traced.map(() => []);
  strays = traced.map(() => []);
  overlay.style.display = "none";
  setStatus("Press Run, let the cream spread, then reverse time.");
  updateButtons();
  sim.render();
  observe();
  trace();
}

/** Two cream molecules (the blob's centre and one partway out) and two coffee molecules just
 * outside the blob. */
async function pickTraced(s: DiscSim): Promise<number[]> {
  const cream = Array.from(s.representatives(1, 2, 0.6));
  const { pos, species } = (await s.all_positions()) as { pos: Float32Array; species: Uint32Array };
  let [cx, cy, n] = [0, 0, 0];
  for (let i = 0; i < species.length; i++) if (species[i] === 1) [cx, cy, n] = [cx + pos[2 * i], cy + pos[2 * i + 1], n + 1];
  [cx, cy] = [cx / n, cy / n];
  let r = 0;
  for (let i = 0; i < species.length; i++) if (species[i] === 1) r = Math.max(r, Math.hypot(pos[2 * i] - cx, pos[2 * i + 1] - cy));
  const coffee = [2.2, 5.3].map((a) => {
    const [tx, ty] = [cx + 1.3 * r * Math.cos(a), cy + 1.3 * r * Math.sin(a)];
    let best = -1;
    let d = Infinity;
    for (let i = 0; i < species.length; i++) {
      const e = (pos[2 * i] - tx) ** 2 + (pos[2 * i + 1] - ty) ** 2;
      if (species[i] === 0 && e < d) [best, d] = [i, e];
    }
    return best;
  });
  return [...cream, ...coffee];
}

/** Record (going forward) or compare and erase (coming back) the traced molecules' positions. */
function trace(back = false) {
  if (!sim || tracing || !traced.length) return;
  tracing = true;
  const s = sim;
  const step = s.steps();
  s.positions_of(new Uint32Array(traced)).then((p: Float64Array) => {
    tracing = false;
    if (s !== sim) return;
    if (!back) {
      if (!path.length || path[path.length - 1].step < step) path.push({ step, pos: p });
    } else {
      const top = path[path.length - 1];
      if (top && top.step === -step) {
        path.pop();
        traced.forEach((_, k) => {
          if (top.pos[2 * k] !== p[2 * k] || top.pos[2 * k + 1] !== p[2 * k + 1]) {
            kept[k].push(top.pos[2 * k], top.pos[2 * k + 1]);
            strays[k].push(p[2 * k], p[2 * k + 1]);
          }
        });
      }
    }
    drawTrails(p);
  });
}

/** Steps to take this frame going backward, landing exactly on the next recorded sample. */
function backwardSteps(k: number, steps: number): number {
  if (tracing) return 0; // wait for the comparison at the current step
  const top = path[path.length - 1];
  const target = top && top.step < -steps ? top.step : top && top.step === -steps ? -steps : 0;
  return Math.min(k, -steps - target);
}

function drawTrails(now: Float64Array) {
  if (!sim) return;
  const r: number[] = [];
  const white = cssColor("--trace-back");
  traced.forEach((_, k) => {
    const c = cssColor(`--trace-${k + 1}`);
    const dot = (x: number, y: number, d: number, col: number[]) => r.push(x - d, y - d, x + d, y + d, col[0], col[1], col[2], 0.9);
    for (const q of path) dot(q.pos[2 * k], q.pos[2 * k + 1], 0.18, c);
    for (let j = 0; j < kept[k].length; j += 2) dot(kept[k][j], kept[k][j + 1], 0.18, c);
    for (let j = 0; j < strays[k].length; j += 2) dot(strays[k][j], strays[k][j + 1], 0.12, white);
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
    if (steps < 0) k = backwardSteps(k, steps);
    if (k > 0) {
      s.step(k);
      elapsed += k;
    }
    $("time").textContent = (Math.abs(s.steps()) * s.dt()).toFixed(1);
    $("dir").textContent = s.steps() < 0 ? "backward" : "forward";
    observe();
    trace(steps < 0);
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
$("nudge-traced").onclick = () => nudge("the 4 traced molecules", (s) => traced.forEach((i) => s.flip_bit(i)));
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
      /** Outward samples not yet retraced, points where the way back strayed, and how many
       * molecules are traced. */
      trails(): { going: number; kept: number; traced: number };
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
    return { going: path.length, kept: kept.reduce((n, a) => n + a.length / 2, 0), traced: traced.length };
  },
};
