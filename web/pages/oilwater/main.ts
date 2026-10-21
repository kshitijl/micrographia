import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot, TimePlot } from "../../src/lib/plot";
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

// ---- 2. Soap. ----
// Measured natively (crates/md/tests/emulsion.rs, a quarter oil): without soap the droplets
// (oily clusters of 10 or more) fall from ~63 to ~30 by 800 τ and keep merging; with plenty of
// soap they settle at ~65–70 from 400 τ on.
/** The soap reserve: dormant soap molecules (three bonded beads, all water until switched on),
 * as (head id) for each; and how many have been switched on. */
let dormant: number[] = [];
let added = 0;
/** For tests: start with this share of soap already on (null: none, the page's start). */
let testSoap: number | null = null;
/** Soap molecules the scenario holds in reserve: up to 30% of the sites. */
const RESERVE = 0.3;
/** Soap molecules switched on per click. */
const DOSE = 40;
let dropsNow: { x: number; y: number }[] = [];
let dropsPrev: { x: number; y: number }[] = [];
let frameS = 0;
const plotS = new LinesPlot($<HTMLCanvasElement>("plotS"), { xLabel: "time (τ)", yLabel: "oil droplets", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** Droplets (oil and tail beads joined within 1.5 σ, at least 10), soap molecules at a boundary
 * (head touching water and a tail touching oil), and water–oil contacts per molecule. */
function measure(pos: Float32Array, species: Uint32Array, box: number[]) {
  const n = species.length;
  const cell = 1.5;
  const [gx, gy] = [Math.floor(box[0] / cell), Math.floor(box[1] / cell)];
  const key = (x: number, y: number) => ((Math.floor(y / cell) % gy) + gy) % gy * gx + ((Math.floor(x / cell) % gx) + gx) % gx;
  const grid: number[][] = Array.from({ length: gx * gy }, () => []);
  for (let i = 0; i < n; i++) grid[key(pos[2 * i], pos[2 * i + 1])].push(i);
  const parent = Int32Array.from({ length: n }, (_, i) => i);
  const find = (i: number): number => {
    while (parent[i] !== i) i = parent[i] = parent[parent[i]];
    return i;
  };
  const oily = (s: number) => s === 1 || s === 3;
  const touchesWater = new Uint8Array(n);
  const touchesOil = new Uint8Array(n);
  let contacts = 0;
  for (let i = 0; i < n; i++) {
    const [x, y] = [pos[2 * i], pos[2 * i + 1]];
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (const j of grid[key(x + dx * cell, y + dy * cell)]) {
          if (j <= i) continue;
          let [ddx, ddy] = [pos[2 * j] - x, pos[2 * j + 1] - y];
          ddx -= box[0] * Math.round(ddx / box[0]);
          ddy -= box[1] * Math.round(ddy / box[1]);
          if (ddx * ddx + ddy * ddy >= cell * cell) continue;
          const [a, b] = [species[i], species[j]];
          if ((a === 0 && b === 1) || (a === 1 && b === 0)) contacts++;
          if (a === 0) touchesWater[j] = 1;
          if (b === 0) touchesWater[i] = 1;
          if (a === 1) touchesOil[j] = 1;
          if (b === 1) touchesOil[i] = 1;
          if (oily(a) && oily(b)) parent[find(i)] = find(j);
        }
  }
  const sizes = new Map<number, number>();
  for (let i = 0; i < n; i++) if (oily(species[i])) sizes.set(find(i), (sizes.get(find(i)) ?? 0) + 1);
  let drops = 0;
  for (const c of sizes.values()) if (c >= 10) drops++;
  // Soap molecules are heads followed by their two tails, in id order.
  let [soap, edge] = [0, 0];
  for (let i = 0; i < n; i++)
    if (species[i] === 2) {
      soap++;
      if (touchesWater[i] && (touchesOil[i + 1] || touchesOil[i + 2])) edge++;
    }
  return { drops, edge: soap ? edge / soap : NaN, contacts: contacts / n };
}

async function readoutS(sim: DiscSim) {
  if (frameS++ % 4 !== 0) return; // every fourth frame: the cluster count reads all positions
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const m = measure(pos, species, Array.from(sim.box_size()));
  dropsNow.push({ x: hostS.elapsed * sim.dt(), y: m.drops });
  plotS.set([
    { points: dropsPrev, color: cssVar("--muted") },
    { points: dropsNow, color: cssVar("--accent") },
  ]);
  $("dropsS").textContent = String(m.drops);
  $("edgeS").textContent = Number.isFinite(m.edge) ? `${Math.round(100 * m.edge)}%` : "no soap";
  $("contactS").textContent = m.contacts.toFixed(2);
}

const hostS = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasS"),
  overlay: $("overlayS"),
  scenario: "emulsion",
  options: () => (testSoap === null ? { n: 8000, soap: RESERVE, active: 0, oil: 0.25, cross: 0.3, temperature: 0.7, bath_rate: 0.02, seed: 4 } : { n: 8000, soap: testSoap, active: 1, oil: 0.25, cross: 0.3, temperature: 0.7, bath_rate: 0.02, seed: 4 }),
  names: ["water", "oil", "soap heads", "soap tails"],
  label: "soap",
  colors: ["--species-2", "--cream", "--highlight", "--good"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readoutS(sim),
  onCreate: async (sim) => {
    // Bond type 1 joins a soap molecule's head (id a) to its far tail bead (a + 2).
    const b = sim.bonds();
    dormant = [];
    const sp = ((await sim.all_positions()) as { species: Uint32Array }).species;
    for (let k = 0; k < b.length; k += 3) if (b[k + 2] === 1 && sp[b[k]] === 0) dormant.push(b[k]);
    added = 0;
    $("addedS").textContent = "none";
    if (dropsNow.length) dropsPrev = dropsNow;
    dropsNow = [];
    frameS = 0;
    await readoutS(sim);
  },
});
$("runS").onclick = () => {
  hostS.running = !hostS.running;
  $("runS").textContent = hostS.running ? "Pause" : "Run";
};
const reshake = () => {
  const run = hostS.running;
  void hostS.create().then(() => (hostS.running = run));
};
$("shakeS").onclick = reshake;

// The soap dropper: a click switches on the dormant soap molecules nearest it (heads to species 2,
// tails to 3), up to DOSE of them.
let dropping = false;
$("dropS").onclick = () => {
  dropping = !dropping;
  $("dropS").classList.toggle("on", dropping);
  hostS.canvas.style.cursor = dropping ? "crosshair" : "";
};
hostS.canvas.addEventListener("pointerdown", async (e) => {
  const sim = hostS.sim;
  if (!dropping || !sim || !dormant.length) return;
  const r = hostS.canvas.getBoundingClientRect();
  const [x0, y0, x1, y1] = Array.from(sim.view());
  const x = x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
  const y = y1 - ((e.clientY - r.top) / r.height) * (y1 - y0);
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  const [w, h] = Array.from(sim.box_size());
  const d2 = (id: number) => {
    let dx = Math.abs(pos[2 * id] - x);
    let dy = Math.abs(pos[2 * id + 1] - y);
    dx = Math.min(dx, w - dx);
    dy = Math.min(dy, h - dy);
    return dx * dx + dy * dy;
  };
  const near = dormant.map((id) => ({ id, d: d2(id) })).sort((a, b) => a.d - b.d).slice(0, DOSE).map((p) => p.id);
  const set = new Set(near);
  dormant = dormant.filter((id) => !set.has(id));
  if (hostS.sim !== sim) return;
  sim.set_species(new Uint32Array(near), 2);
  sim.set_species(new Uint32Array(near.flatMap((id) => [id + 1, id + 2])), 3);
  added += near.length;
  $("addedS").textContent = `${added} molecules (${((300 * added) / sim.n()).toFixed(1)}% of the liquid)`;
  if (!hostS.running) sim.render();
});
new Spoon(hostS, $<HTMLButtonElement>("spoonS"), { size: 10, taupersecond: () => (hostS.running ? 100 * 60 * 0.005 : 1) });

declare global {
  interface Window {
    oilTest?: {
      ready: Promise<void>;
      run(steps: number, cross: number, t: number): Promise<number>;
      /** Share of the oil in the top half after `steps` with gravity g. */
      oilOnTop(steps: number, g: number): Promise<number>;
      /** The soap panel: droplets (and soap at a boundary) every 100 τ for `steps`. */
      soap(soap: number, steps: number): Promise<{ drops: number; edge: number }[]>;
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
  async soap(soap: number, steps: number) {
    await hostS.ready;
    testSoap = soap;
    await hostS.create();
    testSoap = null;
    return hostS.exclusive(async (sim) => {
      const out: { drops: number; edge: number }[] = [];
      for (let k = 0; k < steps / 20000; k++) {
        sim.step(20000);
        const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
        const m = measure(pos, species, Array.from(sim.box_size()));
        out.push({ drops: m.drops, edge: m.edge });
      }
      return out;
    });
  },
};
