import { $, cssVar, SimHost } from "../../src/lib/host";
import { ScatterPlot, TimePlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

// Species (the rubber scenario with a gauge): 0 beads, 1 the fixed end, 2 the end we move
// (pinned, moved with set_species_velocity), 3 the handles on springs.
const CHAINS = 64;
const LENGTH = 40;
const CONTOUR = (LENGTH - 1) * 0.97;
/** The handles' springs: soft enough to stretch several σ. */
const GAUGE = 0.12;
/** Light enough handles that the springs settle in ~100 τ. */
const HANDLE_MASS = 5;
/** The moving end stays between these fractions of the chain's length (near full extension the
 * Kremer–Grest bonds reach their 1.5 σ limit), and moves no faster than this. */
const [MIN_X, MAX_X] = [0.3, 0.8];
const MAX_SPEED = 0.12;
const xIn = $<HTMLInputElement>("x");
const tIn = $<HTMLInputElement>("t");
let fSum = 0;
let fN = 0;
let lastT = 1;
const points: { x: number; y: number; e: number }[] = [];

const plot = new ScatterPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "temperature",
  yLabel: "pull on each end (points), proportional to T (line)",
  point: () => cssVar("--accent"),
  line: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

/** Draw each spring between a handle and its post as dots, from blue (slack) to red (taut at
 * `taut` σ); returns the mean stretch along x. */
function springs(handles: Float32Array, posts: Float32Array, ov: number[], tautAt = 7): number {
  const [slack, taut] = [cssColor("--species-2"), cssColor("--species-3")];
  let d = 0;
  for (let c = 0; c < handles.length / 2; c++) {
    const [hx, hy, px, py] = [handles[2 * c], handles[2 * c + 1], posts[2 * c], posts[2 * c + 1]];
    d += Math.abs(px - hx);
    const len = Math.hypot(px - hx, py - hy);
    const f = Math.min(1, len / tautAt);
    const col = slack.map((s, i) => s + f * (taut[i] - s));
    const dots = Math.max(2, Math.ceil(len / 0.35));
    for (let k = 0; k <= dots; k++) {
      const [x, y] = [hx + ((px - hx) * k) / dots, hy + ((py - hy) * k) / dots];
      ov.push(x - 0.22, y - 0.22, x + 0.22, y + 0.22, col[0], col[1], col[2], 1);
    }
    ov.push(px - 0.35, py - 1.6, px + 0.35, py + 1.6, ...taut);
  }
  return d / (handles.length / 2);
}

/** Move species 2 toward x = `target` (σ), at most MAX_SPEED; returns its x now. */
async function steer(sim: DiscSim, target: number): Promise<number> {
  const x = ((await sim.positions(2)) as Float32Array)[0];
  const gap = target - x;
  const v = Math.abs(gap) < 0.05 ? 0 : Math.max(-MAX_SPEED, Math.min(MAX_SPEED, gap / 2));
  sim.set_species_velocity(2, v, 0);
  return x;
}

/** The moving beads' temperature (the fixed ends carry no kinetic energy). */
async function temperature(sim: DiscSim): Promise<number> {
  const r = (await sim.observe()) as { temperature: number };
  const n = sim.n();
  return (r.temperature * (n - 1)) / (n - 2 * CHAINS - 1);
}

// ---- 1. Feel the pull: handles on springs at the right ends; the slider moves the posts. ----
/** Where the posts start (at 50% of the chain's length; the scenario's layout puts them about,
 * not exactly, there), and where they should be now. */
let postStart = 0;
let postTarget = 0;

async function readout(sim: DiscSim) {
  const [handles, posts] = (await Promise.all([sim.positions(3), sim.positions(2)])) as [Float32Array, Float32Array];
  lastT = await temperature(sim);
  await steer(sim, postTarget);
  const ov: number[] = [];
  const stretch = springs(handles, posts, ov);
  sim.set_overlay(new Float32Array(ov));
  fSum += GAUGE * stretch;
  fN++;
  $("f").textContent = (fSum / fN).toFixed(3);
  $("dx").textContent = `${stretch.toFixed(2)} σ`;
  $("temp").textContent = lastT.toFixed(3);
}

function clear() {
  fSum = 0;
  fN = 0;
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "rubber",
  options: () => ({ chains: CHAINS, length: LENGTH, stretch: 0.5, temperature: Number(tIn.value), gauge: GAUGE, handle_mass: HANDLE_MASS, seed: 10 }),
  names: ["beads", "left anchors", "posts", "handles"],
  label: "feel the pull",
  colors: ["--cream", "--species-3", "--species-3", "--cream"],
  stepsPerFrame: () => 25,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    // Zoom in on the top three chains, from the left anchors to the posts.
    const b = sim.box_size();
    const [anchors, posts] = (await Promise.all([sim.positions(1), sim.positions(2)])) as [Float32Array, Float32Array];
    postStart = posts[0];
    postTarget = postStart + (Number(xIn.value) - 0.5) * CONTOUR;
    const mid = (anchors[0] + posts[0]) / 2;
    const half = Math.max(20, (MAX_X * CONTOUR) / 2 + 8);
    sim.set_view(mid - half, b[1] - 3 * 16, mid + half, b[1]);
    clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("record").onclick = () => {
  if (fN < 20) return;
  points.push({ x: lastT, y: fSum / fN, e: 0 });
  const slope = points.reduce((a, p) => a + p.x * p.y, 0) / points.reduce((a, p) => a + p.x * p.x, 0);
  plot.set(points, slope);
  $("status").textContent = `${points.length} points. The line is pull = ${slope.toFixed(3)} × T.`;
};
// The slider moves the posts smoothly to the new place (the averages restart).
xIn.oninput = () => {
  $("xv").textContent = `${Math.round(100 * Number(xIn.value))}%`;
  postTarget = postStart + (Number(xIn.value) - 0.5) * CONTOUR;
  clear();
};
tIn.oninput = () => ($("tv").textContent = Number(tIn.value).toFixed(2));
tIn.onchange = () => {
  host.sim?.set_thermostat(Number(tIn.value), 1);
  clear();
  $("status").textContent = "Temperature changed. The chains need about 100 τ (~20 s) to settle before the average means much.";
};

// ---- 2. Why stretching warms it: insulated chains on springs at the left; drag the right end. ----
const STRETCH_B = 0.5;
/** A stiffer spring than in part 1, so that pulling the right end stretches the chains rather
 * than mostly the spring. */
const GAUGE_B = 0.5;
let anchorTarget = 0;
/** Where the right ends start (at STRETCH_B of the chain's length from the posts). */
let anchorStart = 0;
let postsB: Float32Array | null = null;
let tB = 1;
const plotB = new TimePlot($<HTMLCanvasElement>("plotB"), {
  yMin: 0.5,
  yMax: 1.6,
  yLabel: "temperature",
  xLabel: "time (τ)",
  colors: () => [cssVar("--accent")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

async function readoutB(sim: DiscSim) {
  const [handles, posts] = (await Promise.all([sim.positions(3), sim.positions(1)])) as [Float32Array, Float32Array];
  postsB = posts;
  tB = await temperature(sim);
  const x = await steer(sim, anchorTarget);
  const ov: number[] = [];
  springs(handles, posts, ov, 3);
  sim.set_overlay(new Float32Array(ov));
  plotB.add(sim.steps() * sim.dt(), tB);
  plotB.draw();
  $("tB").textContent = tB.toFixed(3);
  $("xB").textContent = `${Math.round(100 * (STRETCH_B + (x - anchorStart) / CONTOUR))}%`;
}

const hostB = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "rubber",
  options: () => ({ chains: CHAINS, length: LENGTH, stretch: STRETCH_B, temperature: 1, gauge: GAUGE_B, gauge_side: -1, handle_mass: HANDLE_MASS, seed: 12 }),
  names: ["beads", "posts", "right ends", "handles"],
  label: "why stretching warms it",
  colors: ["--cream", "--species-3", "--species-3", "--cream"],
  stepsPerFrame: () => 25,
  afterSteps: (sim) => readoutB(sim),
  onCreate: async (sim) => {
    const b = sim.box_size();
    const [posts, ends] = (await Promise.all([sim.positions(1), sim.positions(2)])) as [Float32Array, Float32Array];
    anchorTarget = anchorStart = ends[0];
    const mid = posts[0] + (MAX_X * CONTOUR) / 2;
    const half = (MAX_X * CONTOUR) / 2 + 10;
    sim.set_view(mid - half, b[1] - 3 * 16, mid + half, b[1]);
    sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3")]));
    sim.color_by_speed(2.5);
    // The warm-up ran with a heat bath; from here on the chains are insulated.
    sim.set_thermostat(1, 0);
    plotB.clear();
    await readoutB(sim);
  },
});

/** Where the right ends should go: `f` of the chain's length (from where the posts are). */
function aimB(f: number) {
  if (!postsB) return;
  anchorTarget = anchorStart + (Math.max(MIN_X, Math.min(MAX_X, f)) - STRETCH_B) * CONTOUR;
  hostB.running = true;
  $("runB").textContent = "Pause";
}

// Drag the right ends: press anywhere on the simulation and move left and right.
const canvasB = $<HTMLCanvasElement>("canvasB");
canvasB.style.cursor = "ew-resize";
let dragging = false;
const dragTo = (e: PointerEvent) => {
  const sim = hostB.sim;
  if (!sim || !postsB) return;
  const r = canvasB.getBoundingClientRect();
  const v = Array.from(sim.view());
  const x = v[0] + ((e.clientX - r.left) / r.width) * (v[2] - v[0]);
  aimB(STRETCH_B + (x - anchorStart) / CONTOUR);
};
canvasB.addEventListener("pointerdown", (e) => {
  dragging = true;
  canvasB.setPointerCapture(e.pointerId);
  dragTo(e);
});
canvasB.addEventListener("pointermove", (e) => dragging && dragTo(e));
canvasB.addEventListener("pointerup", () => (dragging = false));

$("runB").onclick = () => {
  hostB.running = !hostB.running;
  $("runB").textContent = hostB.running ? "Pause" : "Run";
};
$("pullB").onclick = () => aimB(MAX_X);
$("releaseB").onclick = () => aimB(STRETCH_B);
$("resetB").onclick = () => void hostB.create().then(() => ($("runB").textContent = "Run"));

declare global {
  interface Window {
    rubberTest?: { ready: Promise<void>; pull(t: number, frames: number): Promise<number>; stretchAndRelease(steps: number): Promise<[number, number, number]> };
  }
}

window.rubberTest = {
  ready: Promise.all([host.ready, hostB.ready]).then(() => undefined),
  async pull(t: number, frames: number) {
    await host.ready;
    tIn.value = String(t);
    await host.create();
    return host.exclusive(async (sim) => {
      // The chains and their springs take ~150 τ to settle.
      for (let k = 0; k < 15; k++) sim.step(2000);
      clear();
      for (let k = 0; k < frames; k++) {
        sim.step(50);
        await readout(sim);
      }
      return fSum / fN;
    });
  },
  /** Temperatures before, after stretching the right ends to MAX_X (insulated), and after
   * releasing them back, each move taking `steps`. */
  async stretchAndRelease(steps: number) {
    await hostB.ready;
    return hostB.exclusive(async (sim) => {
      const avg = async () => {
        let t = 0;
        for (let k = 0; k < 10; k++) {
          sim.step(100);
          await readoutB(sim);
          t += tB / 10;
        }
        return t;
      };
      const moveTo = async (f: number) => {
        aimB(f);
        for (let k = 0; k < steps / 50; k++) {
          sim.step(50);
          await readoutB(sim);
        }
        sim.set_species_velocity(2, 0, 0);
      };
      const t0 = await avg();
      await moveTo(MAX_X);
      const t1 = await avg();
      await moveTo(STRETCH_B);
      const t2 = await avg();
      return [t0, t1, t2];
    });
  },
};
