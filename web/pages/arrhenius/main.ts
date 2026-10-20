import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const BOXES = [32, 24];
const SIDE = 8;
const EVERY = 200; // steps between snapshots
// Hot to cold, each temperature with its own equilibration: from a run at a hotter temperature
// the occupancy of the pockets takes ~500 τ to settle at kT = 0.8 (crates/md/tests/binding.rs).
const SWEEP: [number, number][] = [
  [1.6, 10000],
  [1.25, 20000],
  [1.0, 40000],
  [0.8, 60000],
];
const tempIn = $<HTMLInputElement>("temp");

/** Escapes and time spent caught, per temperature. */
const data = new Map<number, { escapes: number; bound: number }>();
let caught: boolean[] = [];
let sweeping = false;
/** Snapshots still to skip after a temperature change, while the pockets' occupancy settles. */
let settle = 0;
const SETTLE = 150;

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "1 / temperature",
  yLabel: "ln (escape rate)",
  yMin: -8,
  yMax: -2,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const temperature = () => Number(tempIn.value);

/** Weighted least-squares line through (1/T, ln k): weights are escape counts (var ln k ≈ 1/n). */
function fit(): { slope: number; icpt: number } | null {
  const pts = [...data].filter(([, d]) => d.escapes >= 5).map(([t, d]) => ({ x: 1 / t, y: Math.log(d.escapes / d.bound), w: d.escapes }));
  if (pts.length < 2) return null;
  const W = pts.reduce((s, p) => s + p.w, 0);
  const mx = pts.reduce((s, p) => s + p.w * p.x, 0) / W;
  const my = pts.reduce((s, p) => s + p.w * p.y, 0) / W;
  const sxy = pts.reduce((s, p) => s + p.w * (p.x - mx) * (p.y - my), 0);
  const sxx = pts.reduce((s, p) => s + p.w * (p.x - mx) ** 2, 0);
  if (sxx < 1e-6) return null;
  const slope = sxy / sxx;
  return { slope, icpt: my - slope * mx };
}

function draw() {
  const pts = [...data].filter(([, d]) => d.escapes > 0).map(([t, d]) => ({ x: 1 / t, y: Math.log(d.escapes / d.bound) }));
  const f = fit();
  const xs = pts.map((p) => p.x);
  const line = f && xs.length ? [Math.min(...xs) - 0.05, Math.max(...xs) + 0.05].map((x) => ({ x, y: f.icpt + f.slope * x })) : [];
  plot.set([
    { points: line, color: cssVar("--plot-back"), dashed: true },
    { points: pts, color: cssVar("--accent"), dots: true },
  ]);
  $("ea").textContent = f ? `${(-f.slope).toFixed(1)} ε` : "–";
}

function gridLines(): number[] {
  const line = cssColor("--box-line");
  const r: number[] = [];
  const [W, H] = [BOXES[0] * SIDE, BOXES[1] * SIDE];
  const t = 0.08;
  for (let i = 0; i <= BOXES[0]; i++) r.push(i * SIDE - t, 0, i * SIDE + t, H, ...line);
  for (let j = 0; j <= BOXES[1]; j++) r.push(0, j * SIDE - t, W, j * SIDE + t, ...line);
  return r;
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const t = temperature();
  // While settling, follow the ligands but count nothing.
  const d = settle > 0 ? { escapes: 0, bound: 0 } : (data.get(t) ?? { escapes: 0, bound: 0 });
  if (settle > 0) settle--;
  else data.set(t, d);
  const lit: number[] = gridLines();
  const hit = cssColor("--box-hit");
  let n = 0;
  for (let i = 0; i < species.length; i++) {
    if (species[i] !== 1 && species[i] !== 3) continue;
    const bx = Math.floor(pos[2 * i] / SIDE);
    const by = Math.floor(pos[2 * i + 1] / SIDE);
    const box = by * BOXES[0] + bx;
    const r = Math.hypot(pos[2 * i] - (bx + 0.5) * SIDE, pos[2 * i + 1] - (by + 0.5) * SIDE);
    if (r < 0.8) caught[box] = true;
    else if (r > 3 && caught[box]) {
      caught[box] = false;
      d.escapes++;
    }
    if (caught[box]) {
      n++;
      d.bound += EVERY * sim.dt();
      const c = [(bx + 0.5) * SIDE, (by + 0.5) * SIDE];
      lit.push(c[0] - 2, c[1] - 2, c[0] + 2, c[1] + 2, ...hit);
    }
  }
  sim.set_overlay(new Float32Array(lit));
  $("caught").textContent = `${n} of ${BOXES[0] * BOXES[1]}`;
  $("esc").textContent = settle > 0 ? "settling…" : String(data.get(t)?.escapes ?? 0);
  $("rate").textContent = settle === 0 && d.bound > 0 && d.escapes > 0 ? (d.escapes / d.bound).toFixed(4) : "–";
  draw();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "binding",
  options: () => ({ boxes_x: BOXES[0], boxes_y: BOXES[1], log2_box: 3, eps_a: 1.6, eps_b: 1.6, temperature: temperature(), seed: 23 }),
  names: ["solvent", "molecule", "pocket", "molecule"],
  colors: ["--species-2", "--cream", "--accent", "--cream"],
  stepsPerFrame: () => EVERY,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    caught = [];
    data.clear();
    settle = SETTLE;
    sim.set_overlay(new Float32Array(gridLines()));
    draw();
  },
});

function setTemperature(sim: DiscSim, t: number) {
  tempIn.value = String(t);
  $("tv").textContent = t.toFixed(2);
  sim.set_thermostat(t, 0.5);
}

async function sweep(frames = 200) {
  sweeping = true;
  const run = host.running;
  host.running = false;
  await host.exclusive(async (sim) => {
    for (const [t, equil] of SWEEP) {
      setTemperature(sim, t);
      data.delete(t);
      $("status").textContent = `Settling, then counting at kT = ${t.toFixed(2)} ε…`;
      sim.step(equil);
      settle = 0;
      for (let f = 0; f < frames; f++) {
        sim.step(EVERY);
        await readout(sim);
        if (f % 5 === 0) sim.render();
      }
    }
  });
  const f = fit();
  if (f) {
    const ratio = Math.exp(-f.slope * (1 / SWEEP[SWEEP.length - 1][0] - 1 / SWEEP[0][0]));
    $("status").textContent = `Doubling the temperature made escapes ${ratio.toFixed(0)}× faster. The points lie on a line: k ∝ e^(−Eₐ/kT) with Eₐ ≈ ${(-f.slope).toFixed(1)} ε.`;
  }
  sweeping = false;
  host.running = run;
  return f;
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("sweep").onclick = () => {
  if (!sweeping) void sweep();
};
tempIn.oninput = () => ($("tv").textContent = temperature().toFixed(2));
tempIn.onchange = () =>
  void host.exclusive(async (sim) => {
    setTemperature(sim, temperature());
    settle = SETTLE;
  });

// ---- The calculator: real activation energies. ----
const R = 8.314e-3; // kJ/mol/K
const eaIn = $<HTMLInputElement>("eareal");
function factor(ea: number, c1: number, c2: number) {
  return Math.exp((ea / R) * (1 / (c1 + 273.15) - 1 / (c2 + 273.15)));
}
function fmt(x: number) {
  return x >= 10 ? `${x.toFixed(0)}× faster` : x >= 1 ? `${x.toFixed(1)}× faster` : `${(1 / x).toFixed(1)}× slower`;
}
function calc() {
  const ea = Number(eaIn.value);
  $("eav").textContent = `${ea} kJ/mol`;
  $("f10").textContent = fmt(factor(ea, 20, 30));
  $("ffridge").textContent = fmt(factor(ea, 4, 37));
  $("fcook").textContent = fmt(factor(ea, 100, 120));
  $("fleh").textContent = fmt(factor(ea, 100, 87));
  $("calcstatus").textContent = `${ea} kJ/mol is ${(ea / (R * 298)).toFixed(0)} kT at room temperature.`;
}
eaIn.oninput = calc;
calc();

declare global {
  interface Window {
    arrheniusTest?: { ready: Promise<void>; sweep(frames: number): Promise<{ ea: number; rates: number[] }> };
  }
}

window.arrheniusTest = {
  ready: host.ready,
  async sweep(frames: number) {
    await host.ready;
    const f = await sweep(frames);
    return { ea: f ? -f.slope : NaN, rates: SWEEP.map(([t]) => { const d = data.get(t)!; return d.escapes / d.bound; }) };
  },
};
