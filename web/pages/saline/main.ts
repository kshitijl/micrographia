import { $, cssVar, SimHost } from "../../src/lib/host";
import { TimePlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const CELLS = 4; // 2 × 2
const BEADS = 44;
// Cell areas (σ²) on this page (browser runs, starting at radius 9): ~135 in brine, ~205–220 in
// saline; in pure water they swell past 272 within ~250 τ. A real red cell bursts once swollen to
// the most its membrane can enclose (its membrane can barely stretch); here a cell bursts when
// its area, averaged over ~10 τ, passes LYSIS.
const SHRUNK = 140;
const ISOTONIC = 212;
const LYSIS = 272;
const AVERAGE = 20; // readings (0.5 τ apart) in the running average
const SETTLE = 60; // τ before any cell can burst
const outsideSel = $<HTMLSelectElement>("outside");

let history: number[][] = [];
let burst: boolean[] = [];
/** Bonds removed when a cell burst, so its opening is drawn as a gap. */
let broken = new Set<string>();

const plot = new TimePlot($<HTMLCanvasElement>("plot"), {
  yMin: 100,
  yMax: 360,
  yLabel: "average cell area (σ²)",
  xLabel: "time (τ)",
  colors: () => [cssVar("--accent")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

function area(p: Float32Array, c: number): number {
  let a = 0;
  for (let k = 0; k < BEADS; k++) {
    const [i, j] = [c * BEADS + k, c * BEADS + ((k + 1) % BEADS)];
    a += p[2 * i] * p[2 * j + 1] - p[2 * j] * p[2 * i + 1];
  }
  return Math.abs(a) / 2;
}

/** Open the membrane of cell `c` where it is most stretched: remove the spring there and the two
 * bending springs that span it. */
function burstCell(sim: DiscSim, p: Float32Array, c: number) {
  let worst = 0;
  let at = 0;
  for (let k = 0; k < BEADS; k++) {
    const [i, j] = [c * BEADS + k, c * BEADS + ((k + 1) % BEADS)];
    const d = Math.hypot(p[2 * i] - p[2 * j], p[2 * i + 1] - p[2 * j + 1]);
    if (d > worst) [worst, at] = [d, k];
  }
  const id = (k: number) => c * BEADS + (((k % BEADS) + BEADS) % BEADS);
  sim.remove_bond(id(at), id(at + 1));
  sim.remove_bond(id(at - 1), id(at + 1));
  sim.remove_bond(id(at), id(at + 2));
  broken.add(`${id(at)}-${id(at + 1)}`);
  burst[c] = true;
}

/** Colour from blue (shrunken) through white (normal) to red (about to burst). */
function fullness(a: number): number[] {
  const [blue, white, red] = [cssColor("--species-2"), [1, 1, 1, 1], cssColor("--highlight")];
  const mix = (x: number[], y: number[], f: number) => x.map((v, i) => v + Math.max(0, Math.min(1, f)) * (y[i] - v));
  return a < ISOTONIC ? mix(blue, white, (a - SHRUNK) / (ISOTONIC - SHRUNK)) : mix(white, red, (a - ISOTONIC) / (LYSIS - ISOTONIC));
}

async function readout(sim: DiscSim) {
  const p = (await sim.positions(2)) as Float32Array;
  const ov: number[] = [];
  let total = 0;
  for (let c = 0; c < CELLS; c++) {
    const a = area(p, c);
    history[c].push(a);
    if (history[c].length > AVERAGE) history[c].shift();
    const avg = history[c].reduce((x, y) => x + y, 0) / history[c].length;
    total += avg;
    // Give the cells time to settle into their natural size first.
    if (!burst[c] && sim.steps() * sim.dt() > SETTLE && history[c].length === AVERAGE && avg > LYSIS) burstCell(sim, p, c);
    // The membrane, segment by segment, coloured by how full the cell is (grey once burst).
    const col = burst[c] ? cssColor("--muted") : fullness(avg);
    for (let k = 0; k < BEADS; k++) {
      const [i, j] = [c * BEADS + k, c * BEADS + ((k + 1) % BEADS)];
      if (broken.has(`${i}-${j}`)) continue;
      for (let s = 0; s <= 3; s++) {
        const [x, y] = [p[2 * i] + ((p[2 * j] - p[2 * i]) * s) / 3, p[2 * i + 1] + ((p[2 * j + 1] - p[2 * i + 1]) * s) / 3];
        ov.push(x - 0.45, y - 0.45, x + 0.45, y + 0.45, col[0], col[1], col[2], 1);
      }
    }
  }
  sim.set_overlay(new Float32Array(ov));
  const mean = total / CELLS;
  plot.add(host.elapsed * sim.dt(), mean);
  plot.draw();
  const n = burst.filter((b) => b).length;
  $("area").textContent = mean.toFixed(0);
  $("rel").textContent = `${mean >= ISOTONIC ? "+" : "−"}${Math.abs(Math.round((100 * (mean - ISOTONIC)) / ISOTONIC))}%`;
  $("burst").textContent = `${n} of ${CELLS}`;
  const out = Number(outsideSel.value);
  $("status").textContent =
    out === 0
      ? n > 0
        ? `Pure water: ${n} cell${n > 1 ? "s have" : " has"} burst. Their salt leaks out through the tear.`
        : "Pure water: water rushes in, the cells swell and turn red as they near their limit…"
      : out < 0.5
        ? "Saline: in and out flows balance, and the cells keep their size."
        : "Brine: water is drawn out; the cells shrink, wrinkle and turn blue.";
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "cell",
  // The rings start a little small (radius 9, springs relaxed at radius 10), so they approach
  // their natural size from below instead of starting over-full.
  options: () => ({ cells_x: 2, cells_y: 2, width: 48, radius: 9, rest: 10 / 9, inside: 0.35, outside: Number(outsideSel.value), stiffness: 20, bend: 10, seed: 19 }),
  names: ["water", "salt", "membrane beads"],
  colors: ["--species-2", "--cream", "--accent"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    history = Array.from({ length: CELLS }, () => []);
    burst = new Array(CELLS).fill(false);
    broken = new Set();
    plot.clear();
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
outsideSel.onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};

// ---- Part 2: one big cell with a stiff membrane, which tears when its tension passes a limit. ----
// Measured natively (crates/md/tests/cells.rs, big_cell_tension): the 10 τ-averaged tension
// k·(mean spring length − rest) is ~5.9 in pure water, 4.0–4.5 in saline (4.9 at first), ~3.4 in
// brine; thermal wriggling alone gives ~3.5.
const BEADS_B = 154;
const RADIUS_B = 35;
const K_B = 150;
const REST_B = (2 * Math.PI * RADIUS_B) / BEADS_B;
const SLACK_B = 3.4;
const SALINE_B = 4.2;
const LIMIT_B = 5.4;
const WINDOW_B = 40; // readings, 0.5 τ apart
const outsideB = $<HTMLSelectElement>("outsideB");
let tensions: number[] = [];
let ring: Float32Array | null = null; // the membrane beads' latest positions
let tornB = false;
let brokenB = new Set<string>();

const plotB = new TimePlot($<HTMLCanvasElement>("plotB"), {
  yMin: 2.5,
  yMax: 7,
  yLabel: "membrane tension (kT/σ)",
  xLabel: "time (τ)",
  colors: () => [cssVar("--accent")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
  lines: [{ y: LIMIT_B, color: () => cssVar("--highlight") }],
});

/** Colour from blue (slack) through white (as in saline) to red (near tearing). */
function tensionColor(t: number): number[] {
  const [blue, white, red] = [cssColor("--species-2"), [1, 1, 1, 1], cssColor("--highlight")];
  const mix = (x: number[], y: number[], f: number) => x.map((v, i) => v + Math.max(0, Math.min(1, f)) * (y[i] - v));
  return t < SALINE_B ? mix(blue, white, (t - SLACK_B) / (SALINE_B - SLACK_B)) : mix(white, red, (t - SALINE_B) / (LIMIT_B - SALINE_B));
}

async function readoutB(sim: DiscSim) {
  await applyJobs(sim);
  const p = (await sim.positions(2)) as Float32Array;
  ring = p;
  const len = (k: number) => {
    const [i, j] = [k, (k + 1) % BEADS_B];
    return Math.hypot(p[2 * i] - p[2 * j], p[2 * i + 1] - p[2 * j + 1]);
  };
  // The tension: how much harder than at rest the intact springs pull, on average.
  let sum = 0;
  let n = 0;
  for (let k = 0; k < BEADS_B; k++) {
    if (brokenB.has(`${k}`)) continue;
    sum += len(k);
    n++;
  }
  tensions.push(K_B * (sum / n - REST_B));
  if (tensions.length > WINDOW_B) tensions.shift();
  const avg = tensions.reduce((a, b) => a + b, 0) / tensions.length;
  if (!tornB && tensions.length === WINDOW_B && avg > LIMIT_B) {
    // Tear at the most stretched spring (and the two bending springs across it).
    let at = 0;
    for (let k = 1; k < BEADS_B; k++) if (len(k) > len(at)) at = k;
    const id = (k: number) => ((k % BEADS_B) + BEADS_B) % BEADS_B;
    sim.remove_bond(id(at), id(at + 1));
    sim.remove_bond(id(at - 1), id(at + 1));
    sim.remove_bond(id(at), id(at + 2));
    brokenB.add(`${id(at)}`);
    tornB = true;
    plotB.mark(hostB.elapsed * sim.dt(), "torn");
  }
  const col = tornB ? cssColor("--muted") : tensionColor(avg);
  const ov: number[] = [];
  for (let k = 0; k < BEADS_B; k++) {
    if (brokenB.has(`${k}`)) continue;
    const [i, j] = [k, (k + 1) % BEADS_B];
    for (let s = 0; s <= 3; s++) {
      const [x, y] = [p[2 * i] + ((p[2 * j] - p[2 * i]) * s) / 3, p[2 * i + 1] + ((p[2 * j + 1] - p[2 * i + 1]) * s) / 3];
      ov.push(x - 0.45, y - 0.45, x + 0.45, y + 0.45, col[0], col[1], col[2], 1);
    }
  }
  sim.set_overlay(new Float32Array(ov));
  const t = hostB.elapsed * sim.dt();
  plotB.add(t, avg);
  plotB.draw();
  $("tensionB").textContent = avg.toFixed(2);
  const out = Number(outsideB.value);
  $("statusB").textContent = tornB
    ? "The membrane tore where it was most stretched, and the salt is spilling out."
    : out === 0
      ? "Pure water: the cell can't swell, so the salt's osmotic push pulls the membrane tighter and tighter…"
      : out < 0.75
        ? "Saline: the pushes balance; the tension stays at its thermal baseline."
        : "Brine: water is drawn out; the membrane goes slack and crumples a little.";
}

const hostB = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "cell",
  options: () => ({ width: 96, radius: RADIUS_B, beads: BEADS_B, inside: 0.5, outside: Number(outsideB.value), stiffness: K_B, bend: K_B / 2, bath: 0, seed: 19 }),
  names: ["water", "salt", "membrane beads"],
  label: "a big cell",
  colors: ["--species-2", "--cream", "--accent"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readoutB(sim),
  onCreate: async (sim) => {
    tensions = [];
    saltSeed = 1;
    tornB = false;
    brokenB = new Set();
    plotB.clear();
    $("limitB").textContent = LIMIT_B.toFixed(1);
    // The box is square; show all of it, with margins, in the 4:3 stage.
    const [w, h] = Array.from(sim.box_size());
    sim.set_view(-w / 6, 0, w + w / 6, h); // corners
    await readoutB(sim);
  },
});

// ---- Adding and removing salt by hand. ----
// Salt molecules are bigger than water molecules. A water molecule is only turned into salt if
// it has room for the bigger size: no water within 0.85 σ, no salt within 1.13 σ and no membrane
// bead within 1.69 σ (the salt's repulsion ranges). Then nothing overlaps and adding salt costs no
// energy at all. Taking salt out (salt → water) always has room.
const ROOM = [0.85, 1.13, 1.69]; // by the neighbour's species: water, salt, membrane
const BRUSH = 4; // σ, the brush's radius
const POUR = 6; // σ: pour into the water this far from the cell (at its starting size) or more
let saltSeed = 1; // for choosing molecules (reset with the simulation, so runs repeat)
const saltRandom = () => ((saltSeed = (saltSeed * 1664525 + 1013904223) >>> 0) / 4294967296);

/** Turn some molecules in `inside(x, y)` into salt (`add`) or back into water: each candidate
 * with probability `fraction`. Call with the animation stopped (positions must not move between
 * reading them and changing species). Returns how many changed. */
async function changeSalt(sim: DiscSim, inside: (x: number, y: number) => boolean, add: boolean, fraction: number) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const [w, h] = Array.from(sim.box_size());
  const n = species.length;
  const kind = species.slice();
  const picked: number[] = [];
  if (!add) {
    for (let i = 0; i < n; i++) if (kind[i] === 1 && inside(pos[2 * i], pos[2 * i + 1]) && saltRandom() < fraction) picked.push(i);
  } else {
    // A grid of 2 σ cells, to find neighbours quickly (the box is periodic).
    const [gx, gy] = [Math.floor(w / 2), Math.floor(h / 2)];
    const cell = (x: number, y: number) => ((Math.floor((y / h) * gy) + gy) % gy) * gx + ((Math.floor((x / w) * gx) + gx) % gx);
    const grid: number[][] = Array.from({ length: gx * gy }, () => []);
    for (let i = 0; i < n; i++) grid[cell(pos[2 * i], pos[2 * i + 1])].push(i);
    for (let i = 0; i < n; i++) {
      const [x, y] = [pos[2 * i], pos[2 * i + 1]];
      if (kind[i] !== 0 || !inside(x, y) || saltRandom() >= fraction) continue;
      let room = true;
      for (let oy = -1; oy <= 1 && room; oy++)
        for (let ox = -1; ox <= 1 && room; ox++)
          for (const j of grid[cell(x + ox * (w / gx), y + oy * (h / gy))]) {
            if (j === i) continue;
            let [dx, dy] = [pos[2 * j] - x, pos[2 * j + 1] - y];
            dx -= w * Math.round(dx / w);
            dy -= h * Math.round(dy / h);
            if (dx * dx + dy * dy < ROOM[kind[j]] ** 2) {
              room = false;
              break;
            }
          }
      if (room) {
        kind[i] = 1;
        picked.push(i);
      }
    }
  }
  sim.set_species(new Uint32Array(picked), add ? 1 : 0);
  return picked.length;
}

// Strokes and pours wait in a queue and are applied in the next frame's readout, when the
// simulation is between steps anyway (one read of all positions per frame at most, instead of
// pausing the animation for every pointer event).
const brushSel = $<HTMLSelectElement>("brushB");
const canvasB = $<HTMLCanvasElement>("canvasB");
type SaltJob = { kind: "brush"; x: number; y: number; add: boolean } | { kind: "pour"; add: boolean };
let jobs: SaltJob[] = [];

async function applyJobs(sim: DiscSim) {
  if (!jobs.length) return;
  const todo = jobs;
  jobs = [];
  for (const add of [true, false]) {
    const strokes = todo.filter((j): j is Extract<SaltJob, { kind: "brush" }> => j.kind === "brush" && j.add === add);
    if (strokes.length)
      await changeSalt(sim, (a, b) => strokes.some((t) => (a - t.x) ** 2 + (b - t.y) ** 2 < BRUSH * BRUSH), add, add ? 0.5 : 0.6);
    if (todo.some((j) => j.kind === "pour" && j.add === add)) await pour(sim, add);
  }
  void hostB.energy?.sample(sim);
}

/** Queue a job; if the simulation is paused, apply it now. */
function queue(job: SaltJob) {
  jobs.push(job);
  if (!hostB.running && hostB.sim) void hostB.exclusive((sim) => applyJobs(sim).then(() => readoutB(sim))).then(() => hostB.sim?.render());
}

let painting = false;
let lastStroke = 0;
function paint(e: PointerEvent) {
  const sim = hostB.sim;
  if (!sim || performance.now() - lastStroke < 30) return;
  lastStroke = performance.now();
  const r = canvasB.getBoundingClientRect();
  const v = sim.view();
  const x = v[0] + ((e.clientX - r.left) / r.width) * (v[2] - v[0]);
  const y = v[3] - ((e.clientY - r.top) / r.height) * (v[3] - v[1]);
  queue({ kind: "brush", x, y, add: brushSel.value === "add" });
}
canvasB.addEventListener("pointerdown", (e) => {
  canvasB.setPointerCapture(e.pointerId);
  painting = true;
  paint(e);
});
canvasB.addEventListener("pointermove", (e) => painting && paint(e));
canvasB.addEventListener("pointerup", () => (painting = false));

/** Pour salt into (or take it out of) the water around the cell, away from it: it then spreads. */
function pour(sim: DiscSim, add: boolean) {
  const [w, h] = Array.from(sim.box_size());
  const far = (x: number, y: number) => Math.hypot(x - w / 2, y - h / 2) > RADIUS_B + POUR;
  return changeSalt(sim, far, add, add ? 0.3 : 0.5);
}
$("saltB").onclick = () => queue({ kind: "pour", add: true });
$("freshB").onclick = () => queue({ kind: "pour", add: false });

$("runB").onclick = () => {
  hostB.running = !hostB.running;
  $("runB").textContent = hostB.running ? "Pause" : "Run";
};
outsideB.onchange = () => {
  const run = hostB.running;
  void hostB.create().then(() => (hostB.running = run));
};

declare global {
  interface Window {
    salineTest?: {
      ready: Promise<void>;
      run(outside: number, steps: number): Promise<{ area: number; burst: number }>;
      runB(outside: number, steps: number): Promise<{ tension: number; torn: boolean; tornAt: number }>;
      dose(outside: number, add: boolean, doses: number, steps: number): Promise<{ trace: number[]; temps: number[]; changed: number[]; torn: boolean; tornAt: number }>;
    };
  }
}

window.salineTest = {
  ready: host.ready,
  /** Average cell area (intact cells, last readings) and how many burst, after `steps`. */
  async run(outside: number, steps: number) {
    outsideSel.value = String(outside);
    await host.create();
    return host.exclusive(async (sim) => {
      for (let k = 0; k < steps / 100; k++) {
        sim.step(100);
        await readout(sim);
      }
      const intact = history.filter((_, c) => !burst[c]);
      const area = intact.length ? intact.reduce((s, h) => s + h.reduce((x, y) => x + y, 0) / h.length, 0) / intact.length : NaN;
      return { area, burst: burst.filter((b) => b).length };
    });
  },
  /** The big cell: its tension (last 20 τ average, before any tear) and whether, and when, it tore. */
  async runB(outside: number, steps: number) {
    await hostB.ready;
    outsideB.value = String(outside);
    await hostB.create();
    return hostB.exclusive(async (sim) => {
      let tornAt = NaN;
      let tension = NaN;
      for (let k = 0; k < steps / 100; k++) {
        sim.step(100);
        hostB.elapsed += 100;
        await readoutB(sim);
        if (tornB && Number.isNaN(tornAt)) tornAt = hostB.elapsed * sim.dt();
        if (!tornB) tension = tensions.reduce((a, b) => a + b, 0) / tensions.length;
      }
      return { tension, torn: tornB, tornAt };
    });
  },
  /** Start the big cell in `outside`, then every 20 τ pour salt in (or take it out) around it, `doses` times, running `steps` in all. The tension every 10 τ, and the temperature. */
  async dose(outside: number, add: boolean, doses: number, steps: number) {
    await hostB.ready;
    outsideB.value = String(outside);
    await hostB.create();
    return hostB.exclusive(async (sim) => {
      let tornAt = NaN;
      const [trace, temps, changed] = [[] as number[], [] as number[], [] as number[]];
      for (let k = 0; k < steps / 100; k++) {
        if (k > 0 && k % 40 === 0 && k / 40 <= doses) changed.push(await pour(sim, add));
        sim.step(100);
        hostB.elapsed += 100;
        await readoutB(sim);
        if (tornB && Number.isNaN(tornAt)) tornAt = hostB.elapsed * sim.dt();
        if (k % 20 === 19) {
          trace.push(tensions.reduce((a, b) => a + b, 0) / tensions.length);
          const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
          temps.push(ke.reduce((a, b) => a + b, 0) / ke.length);
        }
      }
      return { trace, temps, changed, torn: tornB, tornAt };
    });
  },
};
