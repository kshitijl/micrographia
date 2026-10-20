import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const A = { lp: 1.2, ls: 0.5 };
const SIDE = 16;
const lpIn = $<HTMLInputElement>("lp");
const lsIn = $<HTMLInputElement>("ls");
const B = () => ({ lp: Number(lpIn.value), ls: Number(lsIn.value) });

/** Lennard-Jones with ε = 1, truncated and shifted at 2.5 σ. */
function phi(r2: number): number {
  if (r2 >= 6.25) return 0;
  const s6 = 1 / (r2 * r2 * r2);
  const c6 = 1 / 6.25 ** 3;
  return 4 * s6 * (s6 - 1) - 4 * c6 * (c6 - 1);
}

// ---- Part 1: direct counting ----
let counts = { a: [0, 0], b: [0, 0], frames: 0 };

async function count(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  for (let i = 0; i < species.length; i++) {
    const s = species[i];
    if (s !== 1 && s !== 3) continue;
    const x = pos[2 * i];
    const y = pos[2 * i + 1];
    const cx = (Math.floor(x / SIDE) + 0.5) * SIDE;
    const cy = (Math.floor(y / SIDE) + 0.5) * SIDE;
    const r = Math.hypot(x - cx, y - cy);
    const c = s === 1 ? counts.a : counts.b;
    if (r < 0.8) c[0]++;
    else if (r > 4) c[1]++;
  }
  counts.frames++;
  const ka = counts.a[0] / Math.max(1, counts.a[1]);
  const kb = counts.b[0] / Math.max(1, counts.b[1]);
  $("ka").textContent = ka.toFixed(3);
  $("kb").textContent = kb.toFixed(3);
  $("n1").textContent = String(counts.frames);
  if (counts.a[0] > 20 && counts.b[0] > 20) {
    const ddg = -Math.log(kb / ka);
    $("ddg1").textContent = `${ddg.toFixed(2)} kT`;
    measured = ddg;
    compare();
  }
}

let measured = NaN;
let predicted = NaN;

function compare() {
  if (Number.isFinite(measured) && Number.isFinite(predicted)) {
    $("status2").textContent = `Predicted ${predicted.toFixed(2)} kT, measured ${measured.toFixed(2)} kT: B binds ${Math.exp(-predicted).toFixed(1)}× ${predicted < 0 ? "more" : "less"} tightly than A (predicted). At body temperature 1 kT ≈ 0.62 kcal/mol.`;
  }
}

const direct = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas1"),
  overlay: $("overlay1"),
  scenario: "binding",
  options: () => ({ boxes_x: 8, boxes_y: 6, eps_a: A.lp, eps_b: B().lp, eps_ls: A.ls, eps_ls_b: B().ls, seed: 17 }),
  colors: ["--species-2", "--cream", "--accent", "--species-3"],
  stepsPerFrame: () => 400,
  afterSteps: (sim) => count(sim),
  onCreate: () => {
    counts = { a: [0, 0], b: [0, 0], frames: 0 };
    measured = NaN;
  },
});

// ---- Part 2: alchemical windows ----
const WINDOWS = 11;
const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "λ (0 = A, 1 = B)",
  yLabel: "⟨∂U/∂λ⟩ (kT)",
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
let dudl: number[][] = [[], []];
let fepSteps: number[][] = [[], []];
let running = false;

/** ∂U/∂λ for the ligand in each leg (the left box is the pocket leg). */
async function sampleDudl(sim: DiscSim): Promise<[number, number]> {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const b = B();
  const out: [number, number] = [0, 0];
  for (let leg = 0; leg < 2; leg++) {
    let lig = -1;
    for (let i = 0; i < species.length; i++) if (species[i] === 1 && pos[2 * i] < SIDE === (leg === 0)) lig = i;
    let pocket = 0;
    let solvent = 0;
    for (let i = 0; i < species.length; i++) {
      if (pos[2 * i] < SIDE !== (leg === 0)) continue;
      const r2 = (pos[2 * i] - pos[2 * lig]) ** 2 + (pos[2 * i + 1] - pos[2 * lig + 1]) ** 2;
      if (species[i] === 2) pocket += phi(r2);
      else if (species[i] === 0) solvent += phi(r2);
    }
    out[leg] = pocket * (b.lp - A.lp) + solvent * (b.ls - A.ls);
  }
  return out;
}

function draw() {
  const lam = (k: number) => k / (WINDOWS - 1);
  plot.set([
    { points: dudl[0].map((v, k) => ({ x: lam(k), y: v })), color: cssVar("--accent"), dots: true },
    { points: dudl[1].map((v, k) => ({ x: lam(k), y: v })), color: cssVar("--plot-back"), dots: true },
  ]);
}

async function runFep(sim: DiscSim, samples = 100, every = 100, equil = 2000) {
  running = true;
  dudl = [[], []];
  fepSteps = [[], []];
  const b = B();
  for (let k = 0; k < WINDOWS; k++) {
    const lam = k / (WINDOWS - 1);
    $("lam").textContent = lam.toFixed(1);
    sim.set_attraction(1, 2, A.lp + lam * (b.lp - A.lp));
    sim.set_attraction(0, 1, A.ls + lam * (b.ls - A.ls));
    sim.step(equil);
    const s: number[][] = [[], []];
    for (let n = 0; n < samples; n++) {
      sim.step(every);
      const d = await sampleDudl(sim);
      s[0].push(d[0]);
      s[1].push(d[1]);
      if (n % 10 === 0) sim.render();
    }
    const dl = 1 / (WINDOWS - 1);
    for (let leg = 0; leg < 2; leg++) {
      dudl[leg].push(s[leg].reduce((x, y) => x + y, 0) / samples);
      // Forward exponential averaging to the next window (U is linear in λ).
      if (k + 1 < WINDOWS) fepSteps[leg].push(-Math.log(s[leg].reduce((x, y) => x + Math.exp(-y * dl), 0) / samples));
    }
    draw();
    const ti = (leg: number) => {
      let t = 0;
      for (let j = 0; j + 1 < dudl[leg].length; j++) t += (0.5 * (dudl[leg][j] + dudl[leg][j + 1])) / (WINDOWS - 1);
      return t;
    };
    $("gb").textContent = `${ti(0).toFixed(2)} kT`;
    $("gf").textContent = `${ti(1).toFixed(2)} kT`;
    predicted = ti(0) - ti(1);
    $("ddg2").textContent = k > 0 ? `${predicted.toFixed(2)} kT` : "–";
  }
  const fep = fepSteps[0].reduce((x, y) => x + y, 0) - fepSteps[1].reduce((x, y) => x + y, 0);
  $("status2").textContent = `Done. Thermodynamic integration: ${predicted.toFixed(2)} kT; free-energy perturbation: ${fep.toFixed(2)} kT.`;
  compare();
  running = false;
  return { ti: predicted, fep };
}

const alchemy = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas2"),
  overlay: $("overlay2"),
  scenario: "fep",
  options: () => ({ eps_lp: A.lp, eps_ls: A.ls, tether: 20, seed: 18 }),
  colors: ["--species-2", "--cream", "--accent", "--cream"],
  stepsPerFrame: () => 20,
  onCreate: (sim) => sim.set_view(SIDE / 2 - 5, SIDE / 2 - 4, (3 * SIDE) / 2 + 5, SIDE / 2 + 4),
});

$("run1").onclick = () => {
  direct.running = !direct.running;
  $("run1").textContent = direct.running ? "Pause" : "Run";
};
$("run2").onclick = async () => {
  if (running) return;
  await alchemy.create();
  await alchemy.exclusive((sim) => runFep(sim));
};
function changed() {
  $("lpv").textContent = Number(lpIn.value).toFixed(2);
  $("lsv").textContent = Number(lsIn.value).toFixed(2);
}
lpIn.oninput = changed;
lsIn.oninput = changed;
lpIn.onchange = lsIn.onchange = () => {
  const run = direct.running;
  void direct.create().then(() => (direct.running = run));
  predicted = NaN;
};

declare global {
  interface Window {
    fepTest?: { ready: Promise<void>; fep(): Promise<{ ti: number; fep: number }>; direct(frames: number): Promise<number> };
  }
}

window.fepTest = {
  ready: Promise.all([direct.ready, alchemy.ready]).then(() => undefined),
  async fep() {
    await alchemy.ready;
    return alchemy.exclusive((sim) => runFep(sim, 150));
  },
  async direct(frames: number) {
    await direct.ready;
    return direct.exclusive(async (sim) => {
      sim.step(10000);
      counts = { a: [0, 0], b: [0, 0], frames: 0 };
      for (let k = 0; k < frames; k++) {
        sim.step(500);
        await count(sim);
      }
      return measured;
    });
  },
};
