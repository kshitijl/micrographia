import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// Two gases heated alike (scenario enthalpy): one under a locked lid, one under a free weighted
// piston. Measured natively (crates/md/tests/enthalpy.rs, heated slowly): c_V = 1.01 and
// c_p = 1.95 per molecule (2D ideal: 1 and 2); the free lid rose 32.4 σ (ideal 33.5).

const N = 1500;
const WIDTH = 100;
const RHO = 0.1;
/** The load that balances the gas at the start (as the scenario computes it). */
const F0 = RHO * 1 * (1 + 0.5 * Math.PI * RHO * 0.75) * WIDTH;
/** The free piston's mass. */
const MASS = 5;
/** Share of the free lid's speed taken away by friction at each readout. */
const FRICTION = 0.1;
/** Heater power per slider unit: ε per molecule per τ. */
const POWER = 2e-4;

const power = $<HTMLInputElement>("power");
const weight = $<HTMLInputElement>("weight");
const plot = new LinesPlot($<HTMLCanvasElement>("plot"), { xLabel: "heat added per molecule (ε)", yLabel: "kT", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

interface Box {
  locked: boolean;
  heat: number; // ε, in total
  burst: number; // ε waiting to be added
  lastSteps: number;
  start: { u: number; h: number } | null;
  line: { x: number; y: number }[];
  t: number;
  u: number;
  h: number;
  lifted: number; // work done by the gas on the lid (ε)
}
const boxes: Record<"V" | "P", Box> = {
  V: { locked: true, heat: 0, burst: 0, lastSteps: 0, start: null, line: [], t: NaN, u: NaN, h: NaN, lifted: 0 },
  P: { locked: false, heat: 0, burst: 0, lastSteps: 0, start: null, line: [], t: NaN, u: NaN, h: NaN, lifted: 0 },
};

function drawLid(sim: DiscSim, h: number, locked: boolean) {
  const lid = cssColor("--muted");
  const w = cssColor("--accent");
  const rects = [0, h, WIDTH, h + 2, lid[0], lid[1], lid[2], 1];
  if (locked) {
    // Clamps on both sides.
    rects.push(-3, h - 2, 0, h + 4, lid[0], lid[1], lid[2], 1, WIDTH, h - 2, WIDTH + 3, h + 4, lid[0], lid[1], lid[2], 1);
  } else {
    // The weight: a block whose size goes with the load.
    const f = Number(weight.value);
    const bw = 30 * Math.sqrt(f);
    const bh = 14 * Math.sqrt(f);
    rects.push(WIDTH / 2 - bw / 2, h + 2, WIDTH / 2 + bw / 2, h + 2 + bh, w[0], w[1], w[2], 1);
  }
  sim.set_overlay(new Float32Array(rects));
}

async function readout(key: "V" | "P", sim: DiscSim) {
  const b = boxes[key];
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const k = ke.reduce((a, x) => a + x, 0);
  // The heater: scale every velocity by s, adding exactly K (s² − 1); the same dose per τ for both.
  const steps = sim.steps();
  const dtRun = Math.max(0, (steps - b.lastSteps) * sim.dt());
  b.lastSteps = steps;
  const dq = Number(power.value) * POWER * N * dtRun + b.burst;
  b.burst = 0;
  if (dq > 0 && k > 0) {
    const s = Math.sqrt(1 + dq / k);
    sim.kick_region(-1, -1, WIDTH + 1, 4 * (N / (RHO * WIDTH)) + 1, s, 0, 0);
    b.heat += k * (s * s - 1);
  }
  const pe = (await sim.potential_energy()) as number;
  const [h, vp, , workByGas] = Array.from((await sim.piston()) as Float64Array);
  // Friction on the free lid: slow it by FRICTION each readout and put the kinetic energy that
  // removes into the gas as heat (as friction does), so its bounce dies away and nothing is lost.
  if (!b.locked && vp !== 0) {
    const kept = 1 - FRICTION;
    const dk = 0.5 * MASS * vp * vp * (1 - kept * kept);
    sim.set_piston_speed(vp * kept);
    const k2 = k + (dq > 0 ? dq : 0);
    if (k2 > 0) sim.kick_region(-1, -1, WIDTH + 1, 4 * (N / (RHO * WIDTH)) + 1, Math.sqrt(1 + dk / k2), 0, 0);
  }
  b.start ??= { u: k + pe, h };
  b.t = k / N;
  b.u = k + pe;
  b.h = h;
  b.lifted = workByGas;
  const q = b.heat / N;
  if (!b.line.length || q - b.line[b.line.length - 1].x > 0.003) b.line.push({ x: q, y: b.t });
  drawLid(sim, h, b.locked);
  show();
}

function show() {
  const [v, p] = [boxes.V, boxes.P];
  plot.set([
    { points: v.line, color: cssVar("--plot-back") },
    { points: p.line, color: cssVar("--accent") },
  ]);
  $("q").textContent = `${(v.heat / N).toFixed(3)} ε, ${(p.heat / N).toFixed(3)} ε`;
  $("t").textContent = `${v.t.toFixed(3)}, ${p.t.toFixed(3)}`;
  if (p.start) {
    $("dh").textContent = `${(p.h - p.start.h).toFixed(1)} σ`;
    $("split").textContent = `${(p.u - p.start.u).toFixed(0)} ε, ${p.lifted.toFixed(0)} ε`;
  }
  // Heat per degree from each line's overall slope.
  const c = (l: { x: number; y: number }[]) => (l.length > 5 && l[l.length - 1].y - l[0].y > 0.02 ? ((l[l.length - 1].x - l[0].x) / (l[l.length - 1].y - l[0].y)).toFixed(2) : "–");
  $("c").textContent = `${c(v.line)} k, ${c(p.line)} k`;
}

function makeHost(key: "V" | "P") {
  const b = boxes[key];
  return new SimHost<DiscSim>({
    canvas: $<HTMLCanvasElement>(`canvas${key}`),
    overlay: $(`overlay${key}`),
    scenario: "enthalpy",
    options: () => ({ n: N, width: WIDTH, density: RHO, temperature: 1, locked: b.locked ? 1 : 0, mass: MASS, force: F0 * Number(weight.value), seed: 5 }),
    names: ["molecules"],
    label: b.locked ? "lid locked" : "lid free",
    colors: ["--species-2"],
    stepsPerFrame: () => 100,
    afterSteps: (sim) => readout(key, sim),
    // Energy in: the heat added, and the work done on the gas by the lid (minus the work the gas does on it).
    energy: { work: async (sim) => b.heat - Array.from((await sim.piston()) as Float64Array)[3], workLabel: "total minus the heat and work put in" },
    onCreate: async (sim) => {
      Object.assign(b, { heat: 0, burst: 0, lastSteps: sim.steps(), start: null, line: [] });
      const [w, h] = Array.from(sim.box_size());
      sim.set_view(-4, -1, w + 4, h * 0.6);
      await readout(key, sim);
    },
  });
}
const hostV = makeHost("V");
const hostP = makeHost("P");
const both = [hostV, hostP];

$("run").onclick = () => {
  const run = !hostV.running;
  both.forEach((h) => (h.running = run));
  $("run").textContent = run ? "Pause both" : "Run both";
};
$("burst").onclick = () => {
  boxes.V.burst += 0.05 * N;
  boxes.P.burst += 0.05 * N;
};
$("reset").onclick = () => {
  weight.value = "1";
  showWeight();
  for (const h of both) {
    const run = h.running;
    void h.create().then(() => (h.running = run));
  }
};
const showPower = () => ($("powerv").textContent = Number(power.value) === 0 ? "off" : `${(Number(power.value) * POWER).toFixed(4)} ε per molecule per τ`);
power.oninput = showPower;
showPower();
const showWeight = () => {
  const f = Number(weight.value);
  $("weightv").textContent = `${f.toFixed(2)} × (pressure ${(F0 * f / WIDTH).toFixed(3)} ε/σ²)`;
  hostP.sim?.set_piston_load(MASS, F0 * f);
  if (hostP.sim && boxes.P.start) drawLid(hostP.sim, boxes.P.h, false);
};
weight.oninput = showWeight;
showWeight();

// Drag on the free lid's box: up makes the weight lighter, down heavier.
{
  const c = hostP.canvas;
  let drag: number | null = null;
  c.addEventListener("pointerdown", (e) => {
    c.setPointerCapture(e.pointerId);
    drag = e.clientY;
  });
  c.addEventListener("pointermove", (e) => {
    if (drag === null) return;
    const f = Math.min(3, Math.max(0.25, Number(weight.value) * Math.exp((e.clientY - drag) * 0.004)));
    drag = e.clientY;
    weight.value = String(f);
    showWeight();
  });
  c.addEventListener("pointerup", () => (drag = null));
}

declare global {
  interface Window {
    enthalpyTest?: { ready: Promise<void>; heat(q: number, doses: number): Promise<{ v: number; p: number; dh: number }> };
  }
}

window.enthalpyTest = {
  ready: Promise.all(both.map((h) => h.ready)).then(() => {}),
  /** Heat both by `q` per molecule in `doses` doses, 500 steps apart; then let them settle and
   * return each one's ΔT per unit heat per molecule, and the free lid's rise. */
  async heat(q: number, doses: number) {
    const run = async (key: "V" | "P", host: SimHost<DiscSim>) =>
      host.exclusive(async (sim) => {
        const b = boxes[key];
        const avg = async () => {
          let [t, h] = [0, 0];
          for (let k = 0; k < 40; k++) {
            sim.step(500);
            await readout(key, sim);
            t += b.t / 40;
            h += b.h / 40;
          }
          return { t, h };
        };
        // Let the lid settle from the start first (friction acts at each readout).
        for (let k = 0; k < 40; k++) {
          sim.step(500);
          await readout(key, sim);
        }
        const before = await avg();
        const h0 = b.heat;
        for (let d = 0; d < doses; d++) {
          b.burst += (q * N) / doses;
          sim.step(500);
          await readout(key, sim);
        }
        sim.step(10000);
        const after = await avg();
        return { r: (after.t - before.t) / ((b.heat - h0) / N), dh: after.h - before.h };
      });
    power.value = "0";
    showPower();
    const v = await run("V", hostV);
    const p = await run("P", hostP);
    return { v: v.r, p: p.r, dh: p.dh };
  },
};
