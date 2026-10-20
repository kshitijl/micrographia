import { $, cssVar, SimHost } from "../../src/lib/host";
import { TimePlot } from "../../src/lib/plot";
import { Spoon } from "../../src/lib/interact";
import type { DiscSim } from "../../src/lib/wasm";

const cross = $<HTMLInputElement>("cross");
const temp = $<HTMLInputElement>("t");
const plot = new TimePlot($<HTMLCanvasElement>("plot"), {
  yMin: 0,
  yMax: 1,
  yLabel: "mixed",
  xLabel: "time (τ)",
  colors: () => [cssVar("--plot-forward")],
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
let last = 1;

async function readout(sim: DiscSim) {
  const r = (await sim.observe()) as { segregation: number; temperature: number };
  const mixed = 1 - r.segregation;
  last = mixed;
  $("mix").textContent = `${Math.round(100 * mixed)}%`;
  $("temp").textContent = r.temperature.toFixed(3);
  plot.add(host.elapsed * sim.dt(), mixed);
  plot.draw();
  const c = Number(cross.value);
  $("status").textContent =
    c >= 1
      ? "No energy reason to separate: entropy wins at every temperature."
      : mixed < 0.6
        ? "Separating: the energy of like–like contacts beats the entropy of mixing."
        : "Mixed: at this temperature entropy wins (or the domains are still forming).";
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "oilwater",
  options: () => ({ n: 8000, density: 0.7, temperature: Number(temp.value), cross: Number(cross.value), gravity: Number($<HTMLInputElement>("g").value), oil_mass: 0.5, bath_rate: 0.02, seed: 6 }),
  names: ["water", "oil"],
  colors: ["--species-2", "--cream"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: () => plot.clear(),
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("remix").onclick = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
cross.oninput = () => {
  $("crossv").textContent = Number(cross.value).toFixed(2);
  host.sim?.set_attraction(0, 1, Number(cross.value));
};
// 100 steps per frame at ~60 frames a second: about 30 τ per real second.
new Spoon(host, $<HTMLButtonElement>("spoon"), { size: 10, taupersecond: () => (host.running ? 100 * 60 * 0.005 : 1) });
$<HTMLInputElement>("g").oninput = () => {
  const g = Number($<HTMLInputElement>("g").value);
  $("gv").textContent = g === 0 ? "off" : g.toFixed(3);
  host.sim?.set_gravity(0, -g);
};
temp.oninput = () => {
  $("tv").textContent = Number(temp.value).toFixed(2);
  host.sim?.set_thermostat(Number(temp.value), 0.02);
};

declare global {
  interface Window {
    oilTest?: {
      ready: Promise<void>;
      run(steps: number, cross: number, t: number): Promise<number>;
      /** Share of the oil in the top half after `steps` with gravity g. */
      oilOnTop(steps: number, g: number): Promise<number>;
    };
  }
}

window.oilTest = {
  ready: host.ready,
  async run(steps: number, c: number, t: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      sim.set_attraction(0, 1, c);
      sim.set_thermostat(t, 0.02);
      for (let k = 0; k < steps / 500; k++) sim.step(500);
      await readout(sim);
      return last;
    });
  },
  async oilOnTop(steps: number, g: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      sim.set_gravity(0, -g);
      for (let k = 0; k < steps / 500; k++) sim.step(500);
      const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
      const h = Array.from(sim.box_size())[1];
      let [top, all] = [0, 0];
      for (let i = 0; i < species.length; i++) {
        if (species[i] !== 1) continue;
        all++;
        if (pos[2 * i + 1] > h / 2) top++;
      }
      return top / all;
    });
  },
};
