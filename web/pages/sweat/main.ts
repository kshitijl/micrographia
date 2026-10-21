import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

// Evaporative cooling (scenario sweat): an insulated pool of LJ water under air and a warm
// ceiling (kT 0.44). Measured natively (crates/md/tests/sweat.rs): the pool settles at kT ≈ 0.375
// with dry air renewed near the ceiling, 0.42 at 50% humidity, 0.44 (the air's) when saturated.

const W = 120;
const H = 100;
const DEPTH = 14;
const T_DAY = 0.44;
/** The renewal band's lower edge: calm air, and with the fan on (the dry air comes closer). */
const CALM = H - 20;
const FAN = DEPTH + 12;
const RENEW_EVERY = 1000; // steps (5 τ)

const sat = (t: number) => 0.023 * Math.exp(-4.07 * (1 / t - 1 / 0.4));
const humIn = $<HTMLInputElement>("hum");
let fan = false;
let lastRenew = 0;
/** Net evaporation: water turned into air in the band (minus air turned into water). */
let carried = 0;
let carriedLine: { t: number; n: number }[] = [];
let lineW: { x: number; y: number }[] = [];
let lineA: { x: number; y: number }[] = [];
const recent: { w: number; a: number }[] = [];

const plot = new LinesPlot($<HTMLCanvasElement>("temps"), { xLabel: "time (τ)", yLabel: "kT", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

const band = () => (fan ? FAN : CALM);

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const v = (await sim.velocities()) as Float32Array;
  const ke = (i: number) => 0.5 * (v[2 * i] ** 2 + v[2 * i + 1] ** 2);
  let [kw, nw, ka, na] = [0, 0, 0, 0];
  for (let i = 0; i < species.length; i++) {
    const y = pos[2 * i + 1];
    if (species[i] === 0 && y < DEPTH - 1) {
      kw += ke(i);
      nw++;
    } else if (species[i] === 1 && y > DEPTH + 4 && y < DEPTH + 30) {
      ka += ke(i);
      na++;
    }
  }
  const [tw, ta] = [kw / nw, ka / na];
  recent.push({ w: tw, a: ta });
  if (recent.length > 60) recent.shift();
  const steps = sim.steps();
  if (steps - lastRenew >= RENEW_EVERY) {
    lastRenew = steps;
    renew(sim, pos, species);
  }
  const t = steps * sim.dt();
  lineW.push({ x: t, y: tw });
  lineA.push({ x: t, y: ta });
  if (lineW.length > 3000) [lineW, lineA].forEach((l) => l.shift());
  show(t);
  sim.color_by_speed(1.3);
}

/** The wind: hold the vapour in the band at the set humidity. */
function renew(sim: DiscSim, pos: Float32Array, species: Uint32Array) {
  const y0 = band();
  const water: number[] = [];
  const air: number[] = [];
  for (let i = 0; i < species.length; i++)
    if (pos[2 * i + 1] > y0) (species[i] === 0 ? water : air).push(i);
  const want = Math.round(Number(humIn.value) * sat(T_DAY) * W * (H - y0));
  if (water.length > want) {
    sim.set_species(new Uint32Array(water.slice(0, water.length - want)), 1);
    carried += water.length - want;
  } else if (water.length < want) {
    const k = Math.min(air.length, want - water.length);
    sim.set_species(new Uint32Array(air.slice(0, k)), 0);
    carried -= k;
  }
}

function show(t: number) {
  const avg = (f: (r: { w: number; a: number }) => number) => recent.reduce((s, r) => s + f(r), 0) / recent.length;
  const [tw, ta] = [avg((r) => r.w), avg((r) => r.a)];
  plot.set([
    { points: lineW.length ? [{ x: lineW[0].x, y: T_DAY }, { x: t, y: T_DAY }] : [], color: cssVar("--muted"), dashed: true },
    { points: lineA, color: cssVar("--highlight") },
    { points: lineW, color: cssVar("--species-2") },
  ]);
  $("tw").textContent = tw.toFixed(3);
  $("ta").textContent = ta.toFixed(3);
  $("dt").textContent = `${(100 * (1 - tw / T_DAY)).toFixed(0)}%`;
  carriedLine.push({ t, n: carried });
  while (carriedLine.length > 2 && t - carriedLine[0].t > 100) carriedLine.shift();
  const span = t - carriedLine[0].t;
  $("ev").textContent = span > 20 ? ((100 * (carried - carriedLine[0].n)) / span).toFixed(0) : "–";
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "sweat",
  options: () => ({ width: W, height: H, depth: DEPTH, humidity: Number(humIn.value), temperature: T_DAY, seed: 2 }),
  names: ["water", "air"],
  colors: ["--species-2", "--muted"],
  stepsPerFrame: () => 50,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    lineW = [];
    lineA = [];
    recent.length = 0;
    carried = 0;
    carriedLine = [];
    lastRenew = sim.steps();
    overlay(sim);
    await readout(sim);
  },
});

function overlay(sim: DiscSim) {
  const c = cssColor("--plot-back");
  sim.set_overlay(new Float32Array([0, band(), W, band() + 0.4, c[0], c[1], c[2], 0.7]));
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("fan").onclick = () => {
  fan = !fan;
  $("fan").textContent = fan ? "Turn the fan off" : "Turn the fan on";
  if (host.sim) overlay(host.sim);
};
humIn.oninput = () => ($("hv").textContent = `${Math.round(100 * Number(humIn.value))}%`);

declare global {
  interface Window {
    sweatTest?: { ready: Promise<void>; run(humidity: number, steps: number): Promise<{ water: number; air: number }> };
  }
}

window.sweatTest = {
  ready: host.ready,
  /** Start again at this humidity, run, and average the water's and air's kT over the second half. */
  async run(humidity: number, steps: number) {
    humIn.value = String(humidity);
    await host.create();
    return host.exclusive(async (sim) => {
      let [w, a, k] = [0, 0, 0];
      for (let s = 0; s < steps / 200; s++) {
        sim.step(200);
        await readout(sim);
        if (s >= steps / 400) {
          w += recent[recent.length - 1].w;
          a += recent[recent.length - 1].a;
          k++;
        }
      }
      sim.render();
      return { water: w / k, air: a / k };
    });
  },
};
