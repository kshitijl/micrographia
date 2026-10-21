import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

// A cloud chamber (scenario cloud): air, and α particles waiting in a pinned column at the left.
// Measured natively (crates/md/tests/cloud.rs): at 10 σ/τ the α flies straight for ~150 σ between
// sharp turns, and energy is conserved through its hard collisions.
//
// Ionisation is the page's: each look, the flying α loses C/v² per σ of path, and the air
// molecules nearest that stretch become ions (species 2), one per W of energy lost.

const AIR = 0;
const ALPHA = 1;
const ION = 2;
const ALPHAS = 12;
/** Bethe-like loss, ε per σ at speed v: C / v². With v₀ = 10 the range is v₀⁴/4C ≈ 150 σ. */
const C = 16;
/** Energy per ion made (ε). */
const W = 0.2;
/** Below this speed (σ/τ) the α is just a helium atom among the air. */
const STOP = 1.5;
const STEPS = 50;

interface Track {
  id: number;
  points: { x: number; y: number }[];
  /** Ions made at each distance along the track. */
  ions: { s: number; n: number }[];
  length: number;
  made: number;
  carry: number;
  kinks: number;
  v: number;
  done: boolean;
}
let fired = 0;
let track: Track | null = null;
/** Energy the α's have spent ionising, in all. */
let lost = 0;
let aim: { x0: number; y0: number; x1: number; y1: number } | null = null;
/** Droplets: where each ion was made (σ) and when (τ). They stay where they formed, as in a
 * photograph, and fade over FADE. */
let droplets: { x: number; y: number; t: number }[] = [];
const FADE = 120;

const bragg = new LinesPlot($<HTMLCanvasElement>("bragg"), { xLabel: "distance along the track (σ)", yLabel: "ions per σ", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

function source(i: number) {
  return { x: 3, y: 60 + 1.5 * (i - 0.5 * (ALPHAS - 1)) };
}

/** Fire the next α from the source with velocity (vx, vy). */
function fire(sim: DiscSim, vx: number, vy: number) {
  if (fired >= ALPHAS) return;
  const id = fired++;
  const p = source(id);
  sim.set_species(new Uint32Array([id]), ALPHA);
  sim.kick_region(p.x - 0.5, p.y - 0.5, p.x + 0.5, p.y + 0.5, 1, vx, vy);
  track = { id, points: [p], ions: [], length: 0, made: 0, carry: 0, kinks: 0, v: Math.hypot(vx, vy), done: false };
}

async function readout(sim: DiscSim) {
  const t = track;
  if (t && !t.done) {
    const v = (await sim.velocities()) as Float32Array;
    const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
    const [x, y] = [pos[2 * t.id], pos[2 * t.id + 1]];
    const [vx, vy] = [v[2 * t.id], v[2 * t.id + 1]];
    const last = t.points[t.points.length - 1];
    const ds = Math.hypot(x - last.x, y - last.y);
    const speed = Math.hypot(vx, vy);
    if (ds > 0.05) {
      // A sharp turn: the direction of this stretch against the last.
      if (t.points.length >= 2) {
        const p = t.points[t.points.length - 2];
        const a1 = Math.atan2(last.y - p.y, last.x - p.x);
        const a2 = Math.atan2(y - last.y, x - last.x);
        let d = Math.abs(a2 - a1);
        if (d > Math.PI) d = 2 * Math.PI - d;
        if (d > (20 * Math.PI) / 180 && ds > 0.2) t.kinks++;
      }
      // Ionisation: lose C/v² per σ (no more than it has above the stopping speed).
      const ke = 0.5 * speed * speed;
      const de = Math.min((C / Math.max(speed * speed, 1)) * ds, Math.max(0, ke - 0.5 * STOP * STOP));
      if (de > 0 && speed > 0) {
        const s = Math.sqrt(Math.max(0, 1 - de / ke));
        sim.kick_region(x - 0.1, y - 0.1, x + 0.1, y + 0.1, s, 0, 0);
        lost += de;
      }
      // Ions: one per W of energy lost; the air molecules nearest this stretch are marked (as
      // many as are there: a molecule can lose several electrons).
      t.carry += de / W;
      const want = Math.floor(t.carry);
      t.carry -= want;
      t.made += want;
      t.ions.push({ s: t.length + ds / 2, n: want });
      if (want > 0) {
        const near: { i: number; d: number }[] = [];
        const [ux, uy] = [(x - last.x) / ds, (y - last.y) / ds];
        for (let i = 0; i < species.length; i++) {
          if (species[i] !== AIR) continue;
          const [rx, ry] = [pos[2 * i] - last.x, pos[2 * i + 1] - last.y];
          const along = rx * ux + ry * uy;
          if (along < -1 || along > ds + 1) continue;
          const across = Math.abs(rx * uy - ry * ux);
          if (across < 2) near.push({ i, d: across + 0.3 * Math.random() });
        }
        near.sort((a, b) => a.d - b.d);
        const ids = near.slice(0, want).map((q) => q.i);
        if (ids.length) sim.set_species(new Uint32Array(ids), ION);
        const now = sim.steps() * sim.dt();
        for (const i of ids) droplets.push({ x: pos[2 * i], y: pos[2 * i + 1], t: now });
      }
      t.length += ds;
      t.points.push({ x, y });
    }
    t.v = speed;
    if (speed <= STOP * 1.05) t.done = true;
  }
  draw(sim);
}

function draw(sim: DiscSim) {
  const rects: number[] = [];
  const hl = cssColor("--highlight");
  const cream = cssColor("--cream");
  const now = sim.steps() * sim.dt();
  droplets = droplets.filter((d) => now - d.t < FADE);
  for (const d of droplets) rects.push(d.x - 0.7, d.y - 0.7, d.x + 0.7, d.y + 0.7, cream[0], cream[1], cream[2], 0.85 * (1 - (now - d.t) / FADE));
  if (aim) {
    // The aim: a line of dots from the source, as long as the speed.
    const p = source(Math.min(fired, ALPHAS - 1));
    const [dx, dy] = [aim.x1 - aim.x0, aim.y1 - aim.y0];
    for (let k = 1; k <= 12; k++) {
      const [x, y] = [p.x + (dx * k) / 12, p.y + (dy * k) / 12];
      rects.push(x - 0.4, y - 0.4, x + 0.4, y + 0.4, hl[0], hl[1], hl[2], 0.8);
    }
  }
  sim.set_overlay(new Float32Array(rects));
  const t = track;
  $("left").textContent = String(ALPHAS - fired);
  if (!t) return;
  $("range").textContent = `${t.length.toFixed(0)} σ, ${t.made} ions`;
  $("kinks").textContent = String(t.kinks);
  $("speed").textContent = t.done ? "stopped (now a helium atom)" : `${t.v.toFixed(1)} σ/τ`;
  const bins = new Map<number, number>();
  for (const q of t.ions) {
    const b = Math.floor(q.s / 10);
    bins.set(b, (bins.get(b) ?? 0) + q.n);
  }
  const max = Math.max(0, ...bins.keys());
  bragg.set([{ points: Array.from({ length: max + 1 }, (_, b) => ({ x: 10 * b + 5, y: (bins.get(b) ?? 0) / 10 })), color: cssVar("--accent"), dots: true }]);
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "cloud",
  options: () => ({ width: 240, height: 120, density: 0.1, temperature: 1, alphas: ALPHAS, core: 0.04, seed: 5 }),
  names: ["air", "α particles", "ionised air", "α particles waiting"],
  colors: ["--muted", "--highlight", "--cream", "--highlight"],
  stepsPerFrame: () => STEPS,
  afterSteps: (sim) => readout(sim),
  energy: { work: async () => -lost, workLabel: "total plus what the α's spent ionising" },
  onCreate: async (sim) => {
    fired = 0;
    track = null;
    lost = 0;
    droplets = [];
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("fire").onclick = () => {
  if (host.sim) fire(host.sim, 10, 0);
  if (!host.running) $("run").click();
};
$("sweep").onclick = () => {
  const sim = host.sim;
  if (!sim) return;
  void (async () => {
    const { species } = (await sim.all_positions()) as { species: Uint32Array };
    const ids: number[] = [];
    for (let i = 0; i < species.length; i++) if (species[i] === ION) ids.push(i);
    if (ids.length) sim.set_species(new Uint32Array(ids), AIR);
    droplets = [];
    draw(sim);
    if (!host.running) sim.render();
  })();
};
$("reset").onclick = () => void host.create();
{
  const c = host.canvas;
  const at = (e: PointerEvent) => {
    const r = c.getBoundingClientRect();
    const v = host.sim ? Array.from(host.sim.view()) : [0, 0, 240, 120];
    return [v[0] + ((e.clientX - r.left) / r.width) * (v[2] - v[0]), v[3] - ((e.clientY - r.top) / r.height) * (v[3] - v[1])];
  };
  c.addEventListener("pointerdown", (e) => {
    c.setPointerCapture(e.pointerId);
    const [x, y] = at(e);
    aim = { x0: x, y0: y, x1: x, y1: y };
  });
  c.addEventListener("pointermove", (e) => {
    if (!aim) return;
    [aim.x1, aim.y1] = at(e);
    if (host.sim && !host.running) draw(host.sim);
  });
  c.addEventListener("pointerup", () => {
    if (!aim || !host.sim) return;
    const [dx, dy] = [aim.x1 - aim.x0, aim.y1 - aim.y0];
    const len = Math.hypot(dx, dy);
    aim = null;
    if (len < 3) return;
    // Speed from the drag's length: 40 σ of drag is 10 σ/τ, between 4 and 14.
    const v = Math.max(4, Math.min(14, len / 4));
    fire(host.sim, (v * dx) / len, (v * dy) / len);
    if (!host.running) $("run").click();
  });
}

declare global {
  interface Window {
    cloudTest?: { ready: Promise<void>; shoot(v: number, maxSteps: number): Promise<{ length: number; made: number; kinks: number; firstThird: number; lastThird: number }> };
  }
}

window.cloudTest = {
  ready: host.ready,
  /** Fire one α straight at speed v and follow it until it stops: its track's length, ions made,
   * sharp turns, and the ions made in the first and last thirds of the track. */
  async shoot(v: number, maxSteps: number) {
    return host.exclusive(async (sim) => {
      fire(sim, v, 0);
      for (let k = 0; k < maxSteps / STEPS && !track!.done; k++) {
        sim.step(STEPS);
        await readout(sim);
      }
      const t = track!;
      const third = (lo: number, hi: number) => t.ions.filter((q) => q.s >= lo * t.length && q.s < hi * t.length).reduce((a, q) => a + q.n, 0);
      sim.render();
      return { length: t.length, made: t.made, kinks: t.kinks, firstThird: third(0, 1 / 3), lastThird: third(2 / 3, 1.01) };
    });
  },
};
