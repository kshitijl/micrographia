import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

// The depletion force (scenario depletion): two big disks (σ 6) held still among ideal small
// disks, the push on each added up from the small disks' WCA forces. Measured natively
// (crates/md/tests/depletion.rs, a crowd of 0.3 per σ²) at a centre separation of 6.3:
// 1.01 ± 0.07 (AO 0.90); at 6.8: 0.51 ± 0.13 (AO 0.50); at 8.5: −0.07 ± 0.10; at twice the
// temperature, 1.33 ± 0.16 at 6.3. The page's crowd is four times as dense, for a well of ~3 kT.

const BIG = 6;
const RR = 0.5 * (BIG + 1); // R + r
const W = 60;
const H = 45;
const SPEED = 0.3; // σ/τ, each, while pulled or pushed

let held = true;
let moving = 0; // −1 together, +1 apart
/** The running average at the current gap: sum of forces, samples, sum of squares. */
let acc = { f: 0, f2: 0, n: 0, kt: 0 };
let points: { x: number; y: number }[] = [];
let state = { gap: NaN, n: 0, kt: 1, centres: [[0, 0], [0, 0]] as number[][] };

const curve = new LinesPlot($<HTMLCanvasElement>("curve"), { xLabel: "gap between the big disks (σ)", yLabel: "push together (ε/σ)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** WCA force at r for diameter s. */
function wca(r: number, s: number) {
  if (r >= s * 2 ** (1 / 6)) return 0;
  const x6 = (s / r) ** 6;
  return (24 * x6 * (2 * x6 - 1)) / r;
}

/** Asakura–Oosawa at gap g, with R + r where the WCA energy is kT. */
function ao(g: number, n: number, kt: number) {
  const rr = RR / ((1 + Math.sqrt(kt)) / 2) ** (1 / 6);
  const d = g + BIG;
  return d < 2 * rr ? 2 * n * kt * Math.sqrt(rr * rr - (d * d) / 4) : 0;
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const v = (await sim.velocities()) as Float32Array;
  // The big disks are ids 0 and 1 (left, right).
  const c = [0, 1].map((i) => [pos[2 * i], pos[2 * i + 1]]);
  const fx = [0, 0];
  let [ke, m] = [0, 0];
  for (let i = 2; i < species.length; i++) {
    for (let k = 0; k < 2; k++) {
      let dx = c[k][0] - pos[2 * i];
      let dy = c[k][1] - pos[2 * i + 1];
      dx -= W * Math.round(dx / W);
      dy -= H * Math.round(dy / H);
      const r = Math.hypot(dx, dy);
      if (r < RR * 1.13) fx[k] += (wca(r, RR) * dx) / r;
    }
    ke += 0.5 * (v[2 * i] ** 2 + v[2 * i + 1] ** 2);
    m++;
  }
  const kt = ke / m;
  const d = Math.hypot(c[1][0] - c[0][0], c[1][1] - c[0][1]);
  state = { gap: d - BIG, n: m / (W * H - 2 * Math.PI * RR * RR), kt, centres: c };
  if (held && moving === 0) {
    const f = 0.5 * (fx[0] - fx[1]);
    acc = { f: acc.f + f, f2: acc.f2 + f * f, n: acc.n + 1, kt: acc.kt + kt };
  }
  draw(sim, fx);
}

function draw(sim: DiscSim, fx: number[]) {
  const rects: number[] = [];
  const cream = cssColor("--cream");
  const hl = cssColor("--highlight");
  // The depleted zones: rings of radius R + r.
  for (const c of state.centres)
    for (let k = 0; k < 90; k++) {
      const a = (2 * Math.PI * k) / 90;
      const [x, y] = [c[0] + RR * Math.cos(a), c[1] + RR * Math.sin(a)];
      rects.push(x - 0.15, y - 0.15, x + 0.15, y + 0.15, cream[0], cream[1], cream[2], 0.7);
    }
  // The push, averaged, as an arrow on each (while held).
  const mean = acc.n ? acc.f / acc.n : 0;
  if (held) {
    const len = Math.max(-6, Math.min(6, 1.2 * mean));
    for (const [k, sgn] of [[0, 1], [1, -1]] as const) {
      const c = state.centres[k];
      const [x0, x1] = [c[0], c[0] + sgn * len];
      rects.push(Math.min(x0, x1), c[1] - 0.25, Math.max(x0, x1), c[1] + 0.25, hl[0], hl[1], hl[2], 0.9);
    }
  }
  void fx;
  sim.set_overlay(new Float32Array(rects));
  show();
}

function show() {
  const mean = acc.n ? acc.f / acc.n : NaN;
  const se = acc.n > 10 ? Math.sqrt((acc.f2 / acc.n - mean * mean) / acc.n) * 4 : NaN; // ×4: the knocks are correlated over ~16 looks
  const kt = acc.n ? acc.kt / acc.n : state.kt;
  $("gap").textContent = Number.isFinite(state.gap) ? state.gap.toFixed(2) : "–";
  $("f").textContent = !held ? "let go" : acc.n > 20 ? `${mean.toFixed(2)} ± ${se.toFixed(2)} (${acc.n} looks)` : "averaging…";
  $("ao").textContent = Number.isFinite(state.gap) ? ao(state.gap, state.n, kt).toFixed(2) : "–";
  $("t").textContent = state.kt.toFixed(3);
  const gs = Array.from({ length: 61 }, (_, k) => -0.5 + (3 * k) / 60);
  curve.set([
    { points: gs.map((g) => ({ x: g, y: ao(g, state.n, kt) })), color: cssVar("--muted") },
    { points: [...points, ...(held && acc.n > 20 ? [{ x: state.gap, y: mean }] : [])], color: cssVar("--accent"), dots: true },
  ]);
}

/** Keep the average here as a point, and start a new one. */
function bank() {
  if (held && acc.n > 200) points.push({ x: state.gap, y: acc.f / acc.n });
  acc = { f: 0, f2: 0, n: 0, kt: 0 };
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "depletion",
  options: () => ({ width: W, height: H, big: BIG, separation: BIG + 0.3, density: 1.2, seed: 4 }),
  names: ["small disks", "big disks (free)", "big disk (held)", "big disk (held)"],
  colors: ["--species-3", "--species-2", "--species-2", "--species-2"],
  stepsPerFrame: () => 50,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    held = true;
    moving = 0;
    acc = { f: 0, f2: 0, n: 0, kt: 0 };
    points = [];
    await readout(sim);
  },
});

function move(dir: number) {
  const sim = host.sim;
  if (!sim || !held) return;
  if (dir !== 0) bank();
  moving = dir;
  sim.set_species_velocity(2, -dir * SPEED, 0);
  sim.set_species_velocity(3, dir * SPEED, 0);
  if (dir === 0) acc = { f: 0, f2: 0, n: 0, kt: 0 };
}
for (const [id, dir] of [["apart", 1], ["together", -1]] as const) {
  const b = $(id);
  b.addEventListener("pointerdown", (e) => {
    b.setPointerCapture(e.pointerId);
    if (!host.running) $("run").click();
    move(dir);
  });
  const stop = () => moving === dir && move(0);
  b.addEventListener("pointerup", stop);
  b.addEventListener("pointercancel", stop);
}
const scaleAll = (s: number) => {
  const sim = host.sim;
  if (!sim) return;
  sim.kick_region(-1, -1, W + 1, H + 1, s, 0, 0);
  bank();
};
$("hotter").onclick = () => scaleAll(Math.SQRT2);
$("colder").onclick = () => scaleAll(Math.SQRT1_2);
$("letgo").onclick = () => {
  const sim = host.sim;
  if (!sim) return;
  if (held) {
    bank();
    sim.set_species(new Uint32Array([0, 1]), 1);
    held = false;
    $("letgo").textContent = "Hold them again";
  } else {
    sim.set_species(new Uint32Array([0]), 2);
    sim.set_species(new Uint32Array([1]), 3);
    sim.set_species_velocity(2, 0, 0);
    sim.set_species_velocity(3, 0, 0);
    held = true;
    acc = { f: 0, f2: 0, n: 0, kt: 0 };
    $("letgo").textContent = "Let go";
  }
};
$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};

declare global {
  interface Window {
    depletionTest?: { ready: Promise<void>; measure(looks: number): Promise<{ gap: number; force: number; ao: number; kt: number }>; free(steps: number): Promise<{ gaps: number[] }> };
  }
}

window.depletionTest = {
  ready: host.ready,
  /** Average the push over `looks` looks (50 steps apart), held where they are. */
  async measure(looks: number) {
    return host.exclusive(async (sim) => {
      acc = { f: 0, f2: 0, n: 0, kt: 0 };
      for (let k = 0; k < looks; k++) {
        sim.step(50);
        await readout(sim);
      }
      sim.render();
      const kt = acc.kt / acc.n;
      return { gap: state.gap, force: acc.f / acc.n, ao: ao(state.gap, state.n, kt), kt };
    });
  },
  /** Let them go and follow the gap every 500 steps. */
  async free(steps: number) {
    return host.exclusive(async (sim) => {
      if (held) $("letgo").click();
      const gaps: number[] = [];
      for (let k = 0; k < steps / 500; k++) {
        sim.step(500);
        await readout(sim);
        gaps.push(state.gap);
      }
      sim.render();
      return { gaps };
    });
  },
};
