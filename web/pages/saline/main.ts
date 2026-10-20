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

declare global {
  interface Window {
    salineTest?: { ready: Promise<void>; run(outside: number, steps: number): Promise<{ area: number; burst: number }> };
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
};
