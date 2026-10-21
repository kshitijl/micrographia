import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

// Classical heat capacities (scenario heatcap): atoms, two-atom molecules on springs, a crystal,
// each 400 molecules at constant energy, heated by counted doses. Measured natively
// (crates/md/tests/heatcap.rs): 1.01k, 2.58k (2.39k with a spring 5× stiffer), 1.98k per molecule.
// Part 2: real (3D) molecules: the quantum rotor and oscillator, from their partition functions.

const N = 400;
const KINDS = ["atoms", "molecules", "crystal"] as const;
const COLORS = ["--trace-1", "--trace-2", "--trace-3"];

interface Box {
  kind: number;
  host: SimHost<DiscSim> | null;
  /** Heat put in so far, per molecule (ε). */
  heat: number;
  /** Recent temperatures, for a steady reading. */
  recent: number[];
  t: number;
  /** (heat per molecule, kT) before each dose, once settled. */
  marks: { q: number; t: number }[];
  line: { x: number; y: number }[];
  energy: { x: number; y: number }[];
  /** Per molecule, in kT: flying, tumbling, vibrating (moving), stored. */
  parts: number[];
  ke: number;
  /** Potential and total energy at the start (the crystal's: its perfect lattice's), per box. */
  pe0: number;
  e0: number;
}
const boxes: Box[] = KINDS.map((_, kind) => ({ kind, host: null, heat: 0, recent: [], t: NaN, marks: [], line: [], energy: [], parts: [0, 0, 0, 0], ke: 0, pe0: NaN, e0: NaN }));

const tq = new LinesPlot($<HTMLCanvasElement>("tq"), { xLabel: "heat added per molecule (ε)", yLabel: "kT", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const energyPlot = new LinesPlot($<HTMLCanvasElement>("energy"), { xLabel: "time (τ)", yLabel: "change in (E − heat) per molecule", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const doseIn = $<HTMLInputElement>("dose");

async function readout(b: Box, sim: DiscSim) {
  const v = (await sim.velocities()) as Float32Array;
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  const pe = (await sim.potential_energy()) as number;
  const [w, h] = Array.from(sim.box_size());
  const n = v.length / 2;
  let ke = 0;
  for (let i = 0; i < n; i++) ke += 0.5 * (v[2 * i] ** 2 + v[2 * i + 1] ** 2);
  const t = ke / n; // 2D, unit masses: kT per atom
  const mols = b.kind === 1 ? n / 2 : n;
  // Where the energy is: for a molecule, its centre's motion, its tumbling (relative motion
  // across the bond) and its vibration (along it); for the crystal, all of it is vibration.
  let [fly, spin, vib] = [0, 0, 0];
  if (b.kind === 0) fly = ke;
  else if (b.kind === 2) vib = ke;
  else
    for (let m = 0; m < n / 2; m++) {
      const [a, c] = [2 * m, 2 * m + 1];
      const [vx, vy] = [0.5 * (v[2 * a] + v[2 * c]), 0.5 * (v[2 * a + 1] + v[2 * c + 1])];
      fly += vx * vx + vy * vy;
      let rx = pos[2 * a] - pos[2 * c];
      let ry = pos[2 * a + 1] - pos[2 * c + 1];
      rx -= w * Math.round(rx / w);
      ry -= h * Math.round(ry / h);
      const r = Math.hypot(rx, ry);
      const [ux, uy] = [v[2 * a] - v[2 * c], v[2 * a + 1] - v[2 * c + 1]];
      const along = (ux * rx + uy * ry) / r;
      const rel = 0.25 * (ux * ux + uy * uy);
      vib += 0.25 * along * along;
      spin += rel - 0.25 * along * along;
    }
  if (!Number.isFinite(b.pe0)) {
    b.pe0 = pe;
    b.e0 = (ke + pe) / mols;
  }
  b.parts = [fly, spin, vib, pe - b.pe0].map((x) => x / (mols * t));
  b.ke = ke;
  b.recent.push(t);
  if (b.recent.length > 40) b.recent.shift();
  b.t = b.recent.reduce((s, x) => s + x, 0) / b.recent.length;
  const time = sim.steps() * sim.dt();
  b.line.push({ x: b.heat, y: t });
  b.energy.push({ x: time, y: (ke + pe) / mols - b.heat - b.e0 });
  for (const l of [b.line, b.energy]) if (l.length > 3000) l.shift();
  show();
}

/** Heat per degree per molecule: from the first settled mark to the latest. */
function capacity(b: Box) {
  const m = b.marks;
  if (m.length < 2) return NaN;
  const [a, z] = [m[0], m[m.length - 1]];
  return (z.q - a.q) / (z.t - a.t);
}

function show() {
  tq.set(boxes.map((b) => ({ points: b.line, color: cssVar(COLORS[b.kind]) })));
  energyPlot.set(boxes.map((b) => ({ points: b.energy, color: cssVar(COLORS[b.kind]) })));
  const f = (x: number, d: number) => (Number.isFinite(x) ? x.toFixed(d) : "–");
  $("t").textContent = boxes.map((b) => f(b.t, 3)).join(", ");
  $("c").textContent = boxes.map((b) => f(capacity(b), 2)).join(", ");
  drawParts();
}

/** Three stacked bars: where each box's energy is, per molecule, in kT. */
function drawParts() {
  const c = $<HTMLCanvasElement>("where");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const [W, H] = [c.clientWidth, c.clientHeight];
  g.clearRect(0, 0, W, H);
  const cols = ["--trace-4", "--trace-2", "--trace-1", "--muted"].map(cssVar);
  const left = 80;
  const scale = (W - left - 20) / 3; // 3 kT across
  g.font = "12px system-ui, sans-serif";
  g.textBaseline = "middle";
  boxes.forEach((b, k) => {
    const y = 10 + k * ((H - 30) / 3);
    const bh = (H - 30) / 3 - 8;
    g.fillStyle = cssVar("--muted");
    g.fillText(KINDS[b.kind], 4, y + bh / 2);
    let x = left;
    b.parts.forEach((p, j) => {
      g.fillStyle = cols[j];
      g.fillRect(x, y, Math.max(0, p) * scale, bh);
      x += Math.max(0, p) * scale;
    });
  });
  g.fillStyle = cssVar("--muted");
  g.textBaseline = "alphabetic";
  for (let k = 0; k <= 3; k++) g.fillText(`${k} kT`, left + k * scale - 8, H - 4);
}

/** Give each molecule `q` (ε) on average, by scaling every velocity (negative: take it out). */
function dose(b: Box, q: number) {
  const sim = b.host?.sim;
  if (!sim || !(b.ke > 0)) return;
  // A mark first: the heat and temperature before this dose.
  if (b.recent.length >= 20) b.marks.push({ q: b.heat, t: b.t });
  const s2 = Math.max(0.05, 1 + (q * N) / b.ke); // N molecules in every box
  const [w, h] = Array.from(sim.box_size());
  sim.kick_region(-1, -1, w + 1, h + 1, Math.sqrt(s2), 0, 0);
  b.heat += (b.ke * (s2 - 1)) / N;
  b.ke *= s2;
  b.recent = [];
}

for (const b of boxes) {
  b.host = new SimHost<DiscSim>({
    canvas: $<HTMLCanvasElement>(`canvas${b.kind}`),
    overlay: $(`overlay${b.kind}`),
    scenario: "heatcap",
    options: () => ({ kind: b.kind, n: N, temperature: 0.2, density: 0.1, k: 200, seed: 3 + b.kind }),
    names: [KINDS[b.kind]],
    label: KINDS[b.kind],
    colors: [COLORS[b.kind]],
    energy: false,
    cpu: true,
    stepsPerFrame: () => 50,
    afterSteps: (sim) => readout(b, sim),
    onCreate: async (sim) => {
      Object.assign(b, { heat: 0, recent: [], t: NaN, marks: [], line: [], energy: [], pe0: NaN, e0: NaN });
      await readout(b, sim);
    },
  });
}

$("run").onclick = () => {
  const on = !boxes[0].host!.running;
  for (const b of boxes) b.host!.running = on;
  $("run").textContent = on ? "Pause" : "Run all three";
};
$("heat").onclick = () => boxes.forEach((b) => dose(b, Number(doseIn.value)));
$("cool").onclick = () => boxes.forEach((b) => dose(b, -Number(doseIn.value)));
$("reset").onclick = () => boxes.forEach((b) => void b.host!.create());
doseIn.oninput = () => ($("dosev").textContent = `${Number(doseIn.value).toFixed(2)} ε`);

// ---- Part 2: quanta ----

const GASES = [
  { name: "H₂", rot: 85.3, vib: 6332, color: "--trace-3" },
  { name: "N₂", rot: 2.88, vib: 3374, color: "--trace-2" },
  // Argon: its mass in a nucleus ~4 fm across, so ħ²/2Ik ~ 10⁹ K; and nothing to vibrate.
  { name: "argon", rot: 1e9, vib: Infinity, color: "--trace-1" },
];

/** A rotor's levels J(J + 1) θ, each 2J + 1 times over: heat capacity (k) at T, from the energy's
 * variance. */
function rotor(theta: number, t: number) {
  let [z, e, e2] = [0, 0, 0];
  for (let j = 0; j * (j + 1) * (theta / t) < 60; j++) {
    const g = 2 * j + 1;
    const x = (j * (j + 1) * theta) / t;
    const w = g * Math.exp(-x);
    z += w;
    e += w * x;
    e2 += w * x * x;
  }
  return e2 / z - (e / z) ** 2;
}
const oscillator = (theta: number, t: number) => {
  const x = theta / t;
  return x > 300 ? 0 : (x * x * Math.exp(x)) / Math.expm1(x) ** 2;
};
const heatCapacity = (g: (typeof GASES)[number], t: number) => 1.5 + rotor(g.rot, t) + oscillator(g.vib, t);

const quantum = new LinesPlot($<HTMLCanvasElement>("quantum"), { xLabel: "temperature (log₁₀ K: 0 is 1 K, 2 is 100 K, 4 is 10,000 K)", yLabel: "heat per degree, per molecule (k)", yMin: 0, yMax: 3.8, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const levels = new BarPlot($<HTMLCanvasElement>("levels"), {
  labels: ["spin 0", "1", "2", "3", "4", "5", "vib 0", "1", "2"],
  yLabel: "share of molecules",
  yMax: 1,
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--muted"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const tempIn = $<HTMLInputElement>("temp");

function drawQuantum() {
  const lt = Number(tempIn.value);
  const t = 10 ** lt;
  const xs = Array.from({ length: 201 }, (_, k) => (4 * k) / 200);
  quantum.set([
    { points: [{ x: 0, y: 3.5 }, { x: 4, y: 3.5 }], color: cssVar("--muted"), dashed: true },
    { points: [{ x: 0, y: 1.5 }, { x: 4, y: 1.5 }], color: cssVar("--muted"), dashed: true },
    ...GASES.map((g) => ({ points: xs.map((x) => ({ x, y: heatCapacity(g, 10 ** x) })), color: cssVar(g.color) })),
    { points: [{ x: lt, y: 0 }, { x: lt, y: 3.8 }], color: cssVar("--rule") },
  ]);
  $("tempv").textContent = `${t < 10 ? t.toFixed(1) : Math.round(t).toLocaleString("en")} K`;
  $("cq").textContent = GASES.map((g) => heatCapacity(g, t).toFixed(2)).join(", ");
  // Hydrogen's occupations: spin levels J = 0…5, vibration levels 0…2.
  const h = GASES[0];
  const spinW = Array.from({ length: 30 }, (_, j) => (2 * j + 1) * Math.exp(-(j * (j + 1) * h.rot) / t));
  const zs = spinW.reduce((a, x) => a + x, 0);
  const vibW = Array.from({ length: 30 }, (_, n) => Math.exp(-(n * h.vib) / t));
  const zv = vibW.reduce((a, x) => a + x, 0);
  levels.set([...spinW.slice(0, 6).map((x) => x / zs), ...vibW.slice(0, 3).map((x) => x / zv)]);
  $("occ").textContent = `${(100 * (1 - spinW[0] / zs)).toFixed(1)}%, ${(100 * (1 - vibW[0] / zv)).toFixed(2)}%`;
}
tempIn.oninput = drawQuantum;
drawQuantum();

declare global {
  interface Window {
    heatcapTest?: { ready: Promise<void>; measure(doses: number, settle: number): Promise<number[]>; quantum(t: number): number[] };
  }
}

window.heatcapTest = {
  ready: Promise.all(boxes.map((b) => b.host!.ready)).then(() => {}),
  /** Each box: settle, average its temperature, add `doses` doses of 0.1 ε per molecule, settle,
   * average again: heat per degree per molecule. */
  async measure(doses: number, settle: number) {
    const out: number[] = [];
    for (const b of boxes) {
      out.push(
        await b.host!.exclusive(async (sim) => {
          const avg = async () => {
            let s = 0;
            for (let k = 0; k < 40; k++) {
              sim.step(200);
              await readout(b, sim);
              s += b.recent[b.recent.length - 1] / 40;
            }
            return s;
          };
          sim.step(settle);
          await readout(b, sim);
          const t0 = await avg();
          const q0 = b.heat;
          for (let k = 0; k < doses; k++) {
            await readout(b, sim);
            dose(b, 0.1);
            sim.step(500);
          }
          sim.step(settle);
          const t1 = await avg();
          sim.render();
          return (b.heat - q0) / (t1 - t0);
        }),
      );
    }
    return out;
  },
  /** Heat per degree of H₂, N₂ and argon at temperature t (K). */
  quantum: (t: number) => GASES.map((g) => heatCapacity(g, t)),
};
