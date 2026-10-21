import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

// Gas solubility (scenario soda): LJ water with a gas it attracts (CO₂) under a headspace, in a
// heat bath. Measured natively (crates/md/tests/soda.rs): the drink is 5.6, 4.8 and 2.8 times as
// crowded with CO₂ as the headspace at kT 0.40, 0.425 and 0.45.

const W = 100;
const H = 60;
const WATER = 0;
const CO2 = 1;
const AIR = 2;
const VENT_ABOVE = 15; // the open bottle: CO₂ this far above the surface drifts away
const VENT_EVERY = 1000;

const tIn = $<HTMLInputElement>("t");
let open = false;
let lastVent = 0;
let surface = 20;
let lineL: { x: number; y: number }[] = [];
let lineG: { x: number; y: number }[] = [];
const recent: { nl: number; ng: number; cl: number; cg: number }[] = [];
const plot = new LinesPlot($<HTMLCanvasElement>("counts"), { xLabel: "time (τ)", yLabel: "CO₂ molecules", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  // The drink's surface: where the water's density falls below half the liquid's.
  const prof = new Float64Array(H);
  for (let i = 0; i < species.length; i++) if (species[i] === WATER) prof[Math.min(H - 1, Math.floor(pos[2 * i + 1]))] += 1 / W;
  let s = 6;
  while (s < H - 2 && !(prof[s] < 0.35 && prof[s + 1] < 0.35)) s++;
  surface = s;
  let [nl, ng] = [0, 0];
  const vent: number[] = [];
  for (let i = 0; i < species.length; i++) {
    if (species[i] !== CO2) continue;
    const y = pos[2 * i + 1];
    if (y < s - 2) nl++;
    else if (y > s + 4) ng++;
    if (y > s + VENT_ABOVE) vent.push(i);
  }
  const steps = sim.steps();
  if (open && steps - lastVent >= VENT_EVERY && vent.length) {
    sim.set_species(new Uint32Array(vent), AIR);
    lastVent = steps;
  }
  recent.push({ nl, ng, cl: nl / (W * (s - 2)), cg: ng / (W * (H - s - 5.5)) });
  if (recent.length > 100) recent.shift();
  const t = steps * sim.dt();
  lineL.push({ x: t, y: nl });
  lineG.push({ x: t, y: ng });
  if (lineL.length > 3000) [lineL, lineG].forEach((l) => l.shift());
  show();
}

function show() {
  plot.set([
    { points: lineL, color: cssVar("--highlight") },
    { points: lineG, color: cssVar("--muted") },
  ]);
  const avg = (f: (r: (typeof recent)[number]) => number) => recent.reduce((a, r) => a + f(r), 0) / recent.length;
  $("nl").textContent = avg((r) => r.nl).toFixed(0);
  $("ng").textContent = avg((r) => r.ng).toFixed(0);
  const cg = avg((r) => r.cg);
  $("ratio").textContent = cg > 0 ? `${(avg((r) => r.cl) / cg).toFixed(1)}×` : "–";
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "soda",
  options: () => ({ width: W, height: H, depth: 12, temperature: Number(tIn.value), gas: 0.02, solvation: 1, rate: 0.03, seed: 3 }),
  names: ["water", "carbon dioxide", "air"],
  colors: ["--species-2", "--highlight", "--muted"],
  stepsPerFrame: () => 50,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    open = false;
    $("open").textContent = "Open the bottle";
    lineL = [];
    lineG = [];
    recent.length = 0;
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("open").onclick = () => {
  open = !open;
  $("open").textContent = open ? "Close the bottle" : "Open the bottle";
};
const showT = () => ($("tv").textContent = `kT ${Number(tIn.value).toFixed(3)}`);
tIn.oninput = () => {
  showT();
  host.sim?.set_thermostat(Number(tIn.value), 0.03);
  recent.length = 0;
};
showT();

declare global {
  interface Window {
    sodaTest?: { ready: Promise<void>; settle(t: number, steps: number): Promise<{ ratio: number; dissolved: number }>; vent(steps: number): Promise<{ before: number; after: number }> };
  }
}

window.sodaTest = {
  ready: host.ready,
  /** Set the temperature, run `steps`, and average the crowding ratio and the dissolved count
   * over the second half. */
  async settle(t: number, steps: number) {
    tIn.value = String(t);
    showT();
    return host.exclusive(async (sim) => {
      sim.set_thermostat(t, 0.03);
      let [cl, cg, nl, k] = [0, 0, 0, 0];
      for (let s = 0; s < steps / 500; s++) {
        sim.step(500);
        await readout(sim);
        if (s >= steps / 1000) {
          const r = recent[recent.length - 1];
          [cl, cg, nl, k] = [cl + r.cl, cg + r.cg, nl + r.nl, k + 1];
        }
      }
      sim.render();
      return { ratio: cl / cg, dissolved: nl / k };
    });
  },
  /** Open the bottle and run `steps`: dissolved CO₂ before and after. */
  async vent(steps: number) {
    return host.exclusive(async (sim) => {
      const before = recent[recent.length - 1].nl;
      open = true;
      for (let s = 0; s < steps / 500; s++) {
        sim.step(500);
        await readout(sim);
      }
      sim.render();
      return { before, after: recent[recent.length - 1].nl };
    });
  },
};
