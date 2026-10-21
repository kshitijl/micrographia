import { BallSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// A Lennard-Jones crystal in 3D (scenario crystal3: an FCC cube in an empty periodic box three
// times its volume), heated or cooled in exactly counted amounts. Measured natively
// (crates/md/tests/crystal3.rs): it melts in ~60 τ at kT 0.9; held at 0.42 the drop stays
// liquid ~320 τ, then freezes.

const power = $<HTMLInputElement>("power");
const cut = $<HTMLInputElement>("cut");
const colorBy = $<HTMLSelectElement>("colorby");
const onlyCrystal = $<HTMLInputElement>("onlycrystal");
const plotT = new LinesPlot($<HTMLCanvasElement>("plotT"), { xLabel: "heat added per atom (ε)", yLabel: "kT", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const plotX = new LinesPlot($<HTMLCanvasElement>("plotX"), { xLabel: "time (τ)", yLabel: "in a crystal", yMin: 0, yMax: 1, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
/** Heater power per unit of the slider: ε per atom per τ. */
const POWER = 1e-3;
/** Neighbours within this distance count for the crystal test (the first minimum of g(r)). */
const R_CUT = 1.5;

let heat = 0; // ε, in total
let lastSteps = 0;
let lastCrystal = -Infinity; // wall time of the last crystal test
let crystal = 0;
let bonds: Uint8Array | null = null;
const tLine: { x: number; y: number }[] = [];
const xLine: { x: number; y: number }[] = [];
const camera = { yaw: 0.6, pitch: 0.35, distance: 1.25 };

/** Values for the renderer from the crystal bonds: 0 crystal (blue) to 1 liquid (orange); −1 hides. */
function showCrystal(sim: BallSim) {
  if (!bonds || colorBy.value !== "crystal") return;
  const v = new Float32Array(bonds.length);
  for (let i = 0; i < bonds.length; i++) {
    const t = 1 - Math.min(1, Math.max(0, (bonds[i] - 3) / 5));
    v[i] = onlyCrystal.checked && bonds[i] < 7 ? -1 : t;
  }
  sim.color_by_values(v);
}

function applyColoring(sim: BallSim) {
  sim.set_ramp_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3"), ...cssColor("--cream")]));
  if (colorBy.value === "speed") sim.color_by_speed(2.5);
  else if (colorBy.value === "plain") sim.color_by_species();
  else showCrystal(sim);
}

async function readout(sim: BallSim) {
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const n = ke.length;
  const k = ke.reduce((a, b) => a + b, 0);
  // The heater (or fridge): scale every velocity by s, adding exactly K (s² − 1).
  const steps = sim.steps();
  const dtRun = Math.max(0, (steps - lastSteps) * sim.dt());
  lastSteps = steps;
  const dq = Number(power.value) * POWER * n * dtRun;
  if (dq !== 0 && k > 0) {
    const s = Math.sqrt(Math.max(0.5, 1 + dq / k));
    sim.scale_velocities(s);
    heat += k * (s * s - 1);
  }
  // The crystal test costs ~30 ms (8,788 atoms), so run it about once a second.
  const now = performance.now();
  if (now - lastCrystal > 1000 || !bonds) {
    lastCrystal = now;
    bonds = (await sim.crystal_bonds(R_CUT)) as Uint8Array;
    crystal = bonds.reduce((a, b) => a + (b >= 7 ? 1 : 0), 0) / n;
    showCrystal(sim);
    const time = steps * sim.dt();
    if (!xLine.length || time > xLine[xLine.length - 1].x) xLine.push({ x: time, y: crystal });
    if (xLine.length > 3000) xLine.splice(0, 1);
    plotX.set([{ points: xLine, color: cssVar("--species-2") }]);
  }
  const t = (2 / 3) * (k / n);
  const q = heat / n;
  if (!tLine.length || Math.abs(q - tLine[tLine.length - 1].x) > 0.002) tLine.push({ x: q, y: t });
  if (tLine.length > 4000) tLine.splice(0, 1);
  plotT.set([{ points: tLine, color: cssVar("--accent") }]);
  $("t").textContent = t.toFixed(3);
  $("xtal").textContent = `${Math.round(100 * crystal)}%`;
  $("heat").textContent = `${q >= 0 ? "" : "−"}${Math.abs(q).toFixed(3)} ε`;
  $("time").textContent = `${(steps * sim.dt()).toFixed(0)} τ`;
}

const host = new SimHost<BallSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "crystal3",
  create: (c, name, o) => BallSim.create(c, name, o),
  options: () => ({ n: Number($<HTMLSelectElement>("nmol").value), room: 3, temperature: 0.5, seed: 1 }),
  names: ["atoms"],
  colors: ["--species-2"],
  stepsPerFrame: () => 20,
  afterSteps: (sim) => readout(sim),
  energy: { work: async () => heat, workLabel: "total minus the heat you added" },
  onCreate: async (sim) => {
    heat = 0;
    lastSteps = sim.steps();
    bonds = null;
    tLine.length = 0;
    xLine.length = 0;
    sim.set_camera(camera.yaw, camera.pitch, camera.distance);
    sim.set_cut(Number(cut.value));
    await readout(sim);
    applyColoring(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
const restart = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
$("reset").onclick = restart;
$("nmol").onchange = restart;
colorBy.onchange = () => host.sim && applyColoring(host.sim);
onlyCrystal.onchange = () => host.sim && showCrystal(host.sim);
const showPower = () => {
  const p = Number(power.value) * POWER;
  $("powerv").textContent = p === 0 ? "off" : p > 0 ? `heating, ${p.toFixed(3)} ε per atom per τ` : `cooling, ${(-p).toFixed(3)} ε per atom per τ`;
};
power.oninput = showPower;
showPower();
cut.oninput = () => {
  const f = Number(cut.value);
  $("cutv").textContent = f === 0 ? "none" : `${Math.round(100 * f)}%`;
  host.sim?.set_cut(f);
};

// Drag to turn the crystal, scroll to zoom.
const canvas = host.canvas;
let drag: { x: number; y: number } | null = null;
canvas.addEventListener("pointerdown", (e) => {
  canvas.setPointerCapture(e.pointerId);
  drag = { x: e.clientX, y: e.clientY };
  $("stage").style.cursor = "grabbing";
});
canvas.addEventListener("pointermove", (e) => {
  if (!drag) return;
  camera.yaw -= (e.clientX - drag.x) * 0.008;
  camera.pitch = Math.max(-1.4, Math.min(1.4, camera.pitch + (e.clientY - drag.y) * 0.008));
  drag = { x: e.clientX, y: e.clientY };
  host.sim?.set_camera(camera.yaw, camera.pitch, camera.distance);
});
const endDrag = () => {
  drag = null;
  $("stage").style.cursor = "grab";
};
canvas.addEventListener("pointerup", endDrag);
canvas.addEventListener("pointercancel", endDrag);
canvas.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    camera.distance = Math.max(0.6, Math.min(3, camera.distance * Math.exp(e.deltaY * 0.001)));
    host.sim?.set_camera(camera.yaw, camera.pitch, camera.distance);
  },
  { passive: false },
);

declare global {
  interface Window {
    crystalTest?: {
      ready: Promise<void>;
      run(steps: number, heater: number): Promise<{ t: number; crystal: number; heat: number; steps: number }>;
      timing(steps: number): Promise<{ stepsPerS: number; crystalMs: number; readoutMs: number }>;
    };
  }
}

window.crystalTest = {
  ready: host.ready,
  /** Run `steps` steps with the heater at `heater` (slider units), reading out every 200 steps. */
  async run(steps: number, heater: number) {
    power.value = String(heater);
    return host.exclusive(async (sim) => {
      for (let k = 0; k < steps; k += 200) {
        sim.step(200);
        lastCrystal = -Infinity;
        await readout(sim);
      }
      sim.render();
      const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
      return { t: (2 / 3) * (ke.reduce((a, b) => a + b, 0) / ke.length), crystal, heat: heat / sim.n(), steps: sim.steps() };
    });
  },
  /** How fast the engine runs alone, and what the crystal test and a whole readout cost. */
  async timing(steps: number) {
    return host.exclusive(async (sim) => {
      sim.step(100);
      await sim.sync();
      let t0 = performance.now();
      sim.step(steps);
      await sim.sync();
      const stepsPerS = (steps / (performance.now() - t0)) * 1000;
      t0 = performance.now();
      await sim.crystal_bonds(R_CUT);
      const crystalMs = performance.now() - t0;
      t0 = performance.now();
      lastCrystal = -Infinity;
      await readout(sim);
      return { stepsPerS, crystalMs, readoutMs: performance.now() - t0 };
    });
  },
};
