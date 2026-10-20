import { $, cssVar, SimHost } from "../../src/lib/host";
import { ScatterPlot, TimePlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const CHAINS = 64;
/** The handles' springs: soft enough to stretch several σ. */
const GAUGE = 0.07;
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

/** The pull, from the springs: k × (post − handle); and the springs drawn, coloured by tension. */
async function readout(sim: DiscSim) {
  const handles = (await sim.positions(3)) as Float32Array;
  // Species 1: each chain's left anchor, then (after the chain) its post.
  const fixed = (await sim.positions(1)) as Float32Array;
  const r = (await sim.observe()) as { temperature: number };
  // The pinned anchors and posts carry no kinetic energy; rescale to the moving beads only.
  const n = sim.n();
  lastT = (r.temperature * (n - 1)) / (n - 2 * CHAINS - 1);
  let d = 0;
  const ov: number[] = [];
  const [slack, taut] = [cssColor("--species-2"), cssColor("--species-3")];
  for (let c = 0; c < CHAINS; c++) {
    const [hx, hy] = [handles[2 * c], handles[2 * c + 1]];
    const [px, py] = [fixed[4 * c + 2], fixed[4 * c + 3]];
    d += px - hx;
    // The spring as a row of dots, coloured from slack (blue) to taut (red) over 0–7 σ.
    const len = Math.hypot(px - hx, py - hy);
    const f = Math.min(1, len / 7);
    const col = slack.map((s, i) => s + f * (taut[i] - s));
    const dots = Math.max(2, Math.ceil(len / 0.35));
    for (let k = 0; k <= dots; k++) {
      const [x, y] = [hx + ((px - hx) * k) / dots, hy + ((py - hy) * k) / dots];
      ov.push(x - 0.22, y - 0.22, x + 0.22, y + 0.22, col[0], col[1], col[2], 1);
    }
    ov.push(px - 0.35, py - 1.6, px + 0.35, py + 1.6, ...taut);
  }
  sim.set_overlay(new Float32Array(ov));
  const stretch = d / CHAINS;
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
  options: () => ({ chains: CHAINS, length: 40, stretch: Number(xIn.value), temperature: Number(tIn.value), gauge: GAUGE, seed: 10 }),
  names: ["beads", "posts", "end bead", "handle"],
  label: "feel the pull",
  colors: ["--cream", "--species-3", "--cream", "--species-3"],
  stepsPerFrame: () => 25,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    // Zoom in on the top three chains, from the left anchors to the posts.
    const b = sim.box_size();
    const ends = (await sim.positions(1)) as Float32Array;
    const [x0, x1] = [ends[0], ends[2]];
    const mid = (x0 + x1) / 2;
    const half = Math.max(20, (x1 - x0) / 2 + 6);
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

xIn.oninput = () => ($("xv").textContent = `${Math.round(100 * Number(xIn.value))}%`);
xIn.onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
tIn.oninput = () => ($("tv").textContent = Number(tIn.value).toFixed(2));
tIn.onchange = () => {
  host.sim?.set_thermostat(Number(tIn.value), 1);
  clear();
  $("status").textContent = "Temperature changed. The chains need about 100 τ (~20 s) to settle before the average means much.";
};

// ---- 2. Why stretching warms it: insulated chains, pulled or released quickly. ----
const STRETCH_B = 0.5;
let sepB = STRETCH_B;
let contourB = 1;
let moving = 0;
let target = 0;
/** Pulling speed: slow enough for the chains to follow (a fast release would just push on them). */
const SPEED_B = 0.15;
const plotB = new TimePlot($<HTMLCanvasElement>("plotB"), {
  yMin: 0.5,
  yMax: 1.6,
  yLabel: "temperature",
  xLabel: "time (τ)",
  colors: () => [cssVar("--accent")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
let tB = 1;

async function readoutB(sim: DiscSim) {
  const r = (await sim.observe()) as { temperature: number };
  const n = sim.n();
  tB = (r.temperature * (n - 1)) / (n - 2 * CHAINS - 1);
  plotB.add(sim.steps() * sim.dt(), tB);
  plotB.draw();
  $("tB").textContent = tB.toFixed(3);
  $("xB").textContent = `${Math.round(100 * sepB)}%`;
}

const hostB = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "rubber",
  options: () => ({ chains: CHAINS, length: 40, stretch: STRETCH_B, temperature: 1, seed: 12 }),
  names: ["beads", "left anchor", "end bead", "right anchor"],
  label: "why stretching warms it",
  colors: ["--cream", "--species-3", "--cream", "--species-3"],
  stepsPerFrame: () => 25,
  afterSteps: async (sim) => {
    if (moving !== 0) {
      sepB += (moving * 25 * sim.dt()) / contourB;
      if ((moving > 0 && sepB >= target) || (moving < 0 && sepB <= target)) {
        moving = 0;
        sim.set_species_velocity(3, 0, 0);
      }
    }
    await readoutB(sim);
  },
  onCreate: async (sim) => {
    const b = sim.box_size();
    sim.set_view(0, b[1] - 6 * 16, b[0], b[1]);
    sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3")]));
    sim.color_by_speed(2.5);
    // The warm-up ran with a heat bath; from here on the chains are insulated.
    sim.set_thermostat(1, 0);
    sepB = STRETCH_B;
    contourB = 39 * 0.97;
    moving = 0;
    plotB.clear();
    await readoutB(sim);
  },
});

/** Move the right ends until they are at `to` of the chains' length. */
function move(to: number) {
  const sim = hostB.sim;
  if (!sim || Math.abs(to - sepB) < 0.01) return;
  moving = Math.sign(to - sepB) * SPEED_B;
  target = to;
  sim.set_species_velocity(3, moving, 0);
  hostB.running = true;
  $("runB").textContent = "Pause";
}
$("runB").onclick = () => {
  hostB.running = !hostB.running;
  $("runB").textContent = hostB.running ? "Pause" : "Run";
};
$("pullB").onclick = () => move(0.8);
$("releaseB").onclick = () => move(STRETCH_B);
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
      // Chains of 40 beads take ~100 τ to settle into their stretched shapes.
      for (let k = 0; k < 8; k++) sim.step(2000);
      clear();
      for (let k = 0; k < frames; k++) {
        sim.step(50);
        await readout(sim);
      }
      return fSum / fN;
    });
  },
  /** Temperatures before, after a stretch at SPEED_B for `steps`, and after the same release. */
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
      const t0 = await avg();
      sim.set_species_velocity(3, SPEED_B, 0);
      sim.step(steps);
      sim.set_species_velocity(3, 0, 0);
      const t1 = await avg();
      sim.set_species_velocity(3, -SPEED_B, 0);
      sim.step(steps);
      sim.set_species_velocity(3, 0, 0);
      const t2 = await avg();
      return [t0, t1, t2];
    });
  },
};
