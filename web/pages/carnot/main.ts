import { $, cssVar, SimHost } from "../../src/lib/host";
import { ScatterPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const TC = 1.0;
const H1 = 32;
const ISO = 1.5;
const uIn = $<HTMLInputElement>("u");
const thIn = $<HTMLInputElement>("th");
const modeSel = $<HTMLSelectElement>("mode");
const contactIn = $<HTMLInputElement>("contact");
const contact = () => Number(contactIn.value);

type Walls = "hot" | "cold" | "insulated";
interface Leg {
  name: string;
  walls: Walls;
  target: number;
}

let width = 64;
let legs: Leg[] = [];
let leg = 0;
let legStart: { heat: number; work: number } | null = null;
let cycle: { heat: number[]; work: number[] } = { heat: [], work: [] };
let lastT = NaN;
const results: { x: number; y: number; e: number }[] = [];
let lastEff = NaN;
// Totals over the cycles since the last reset or change: single cycles are noisy because the gas
// does not return to exactly the same energy, but the totals converge.
let totals = { w: 0, qh: 0, qc: 0, cycles: 0 };

const plot = new ScatterPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "piston speed (σ/τ)",
  yLabel: "efficiency per cycle (line: Carnot limit)",
  point: () => cssVar("--accent"),
  line: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const th = () => Number(thIn.value);
const engine = () => modeSel.value === "engine";
const limit = () => (engine() ? 1 - TC / th() : th() / (th() - TC));

function plan() {
  const h2 = H1 * ISO;
  const h3 = h2 * (th() / TC);
  const h4 = H1 * (th() / TC);
  legs = engine()
    ? [
        { name: "hot, expanding", walls: "hot", target: h2 },
        { name: "insulated, expanding", walls: "insulated", target: h3 },
        { name: "cold, compressing", walls: "cold", target: h4 },
        { name: "insulated, compressing", walls: "insulated", target: H1 },
      ]
    : [
        { name: "insulated, expanding", walls: "insulated", target: h4 },
        { name: "cold, expanding", walls: "cold", target: h3 },
        { name: "insulated, compressing", walls: "insulated", target: h2 },
        { name: "hot, compressing", walls: "hot", target: H1 },
      ];
  $("effk").textContent = engine() ? "efficiency (all cycles)" : "heat delivered / work (all cycles)";
  $("limk").textContent = engine() ? "Carnot limit" : "limit Th/(Th−Tc)";
  $("lim").textContent = engine() ? `${(100 * limit()).toFixed(1)}%` : limit().toFixed(2);
}

function startLeg(sim: DiscSim, k: number, h: number) {
  leg = k;
  const l = legs[k];
  // The bath touches every molecule during the isothermal legs and is disconnected otherwise.
  sim.set_thermostat(l.walls === "hot" ? th() : TC, l.walls === "insulated" ? 0 : contact());
  sim.set_piston_speed((l.target > h ? 1 : -1) * Number(uIn.value));
  $("leg").textContent = `${k + 1}. ${l.name}`;
}

function finishCycle() {
  const [q1, q2, q3, q4] = cycle.heat;
  console.log(`cycle heats ${cycle.heat.map((x) => x.toFixed(1))} works ${cycle.work.map((x) => x.toFixed(1))} T ${lastT.toFixed(3)}`);
  const w = cycle.work.reduce((a, b) => a + b, 0);
  if (engine()) {
    lastEff = w / q1;
    $("eff").textContent = `${(100 * lastEff).toFixed(1)}%`;
    $("status").textContent = `Last cycle: heat in ${q1.toFixed(0)} ε, heat out ${(-q3).toFixed(0)} ε, work out ${w.toFixed(0)} ε (heat during insulated legs ${(q2 + q4).toFixed(1)} ε).`;
  } else {
    lastEff = -q4 / -w;
    $("eff").textContent = lastEff.toFixed(2);
    $("status").textContent = `Last cycle: heat taken from the cold side ${q2.toFixed(0)} ε, delivered to the hot side ${(-q4).toFixed(0)} ε, work put in ${(-w).toFixed(0)} ε.`;
  }
  results.push({ x: Number(uIn.value), y: lastEff, e: 0 });
  plot.set(results, limit());
}

async function readout(sim: DiscSim) {
  const [h, , , workByGas] = Array.from((await sim.piston()) as Float64Array);
  const l = (await sim.ledger(1)) as { energy: Float64Array };
  // Ledger slot 6 is the bulk bath (walls are insulating).
  const heat = l.energy[6];
  const r = (await sim.observe()) as { temperature: number };
  lastT = r.temperature;
  $("temp").textContent = lastT.toFixed(3);
  const c = cssColor(legs[leg].walls === "hot" ? "--species-3" : legs[leg].walls === "cold" ? "--species-2" : "--muted");
  const p = cssColor("--accent");
  // Piston, and a coloured frame showing which bath (if any) is connected.
  sim.set_overlay(new Float32Array([0, h, width, h + 3, p[0], p[1], p[2], 1, -1.5, 0, 0, h, c[0], c[1], c[2], 1, width, 0, width + 1.5, h, c[0], c[1], c[2], 1, -1.5, -1.5, width + 1.5, 0, c[0], c[1], c[2], 1]));
  legStart ??= { heat, work: workByGas };
  const target = legs[leg].target;
  const up = target > legs[(leg + 3) % 4].target || (leg === 0 && engine());
  const reached = up ? h >= target : h <= target;
  if (reached) {
    cycle.heat.push(heat - legStart.heat);
    cycle.work.push(workByGas - legStart.work);
    legStart = { heat, work: workByGas };
    const next = (leg + 1) % 4;
    if (next === 0) {
      finishCycle();
      cycle = { heat: [], work: [] };
    }
    startLeg(sim, next, h);
  }
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "carnot",
  options: () => ({ n: 800, width: 64, start: H1, top: 4 * H1 * ISO, hot: th(), contact: contact(), seed: 12 }),
  colors: ["--cream"],
  stepsPerFrame: () => 60,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    width = sim.box_size()[0];
    sim.set_view(-2, -2, width + 2, 4 * H1 * ISO);
    sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3")]));
    sim.color_by_speed(3);
    plan();
    legStart = null;
    cycle = { heat: [], work: [] };
    totals = { w: 0, qh: 0, qc: 0, cycles: 0 };
    startLeg(sim, 0, H1);
  },
});

function reset() {
  const run = host.running;
  void host.create().then(() => (host.running = run));
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run cycles";
};
$("reset").onclick = reset;
modeSel.onchange = () => {
  results.length = 0;
  plot.set(results, limit());
  reset();
};
uIn.oninput = () => ($("uv").textContent = `${Number(uIn.value).toFixed(2)} σ/τ`);
uIn.onchange = () => (totals = { w: 0, qh: 0, qc: 0, cycles: 0 });
thIn.oninput = () => ($("thv").textContent = Number(thIn.value).toFixed(2));
contactIn.oninput = () => ($("cv").textContent = `${contact().toFixed(2)} /τ`);
thIn.onchange = () => {
  results.length = 0;
  reset();
};

declare global {
  interface Window {
    carnotTest?: { ready: Promise<void>; cycles(n: number, u: number): Promise<{ eff: number[]; total: number; limit: number }> };
  }
}

window.carnotTest = {
  ready: host.ready,
  async cycles(n: number, u: number) {
    await host.ready;
    uIn.value = String(u);
    await host.create();
    const start = results.length;
    await host.exclusive(async (sim) => {
      while (results.length < start + n) {
        sim.step(100);
        await readout(sim);
      }
    });
    return { eff: results.slice(start).map((p) => p.y), total: lastEff, limit: limit() };
  },
};
