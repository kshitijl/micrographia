import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// Gay-Lussac's volumes (scenario avogadro): hydrogen and chlorine under a free weighted piston,
// heat-bath walls. Left, as Avogadro had it: molecules of two atoms (H₂, Cl₂), reacting by a
// light-started chain (Cl₂ + light → 2 Cl; Cl + H₂ → HCl + H; H + Cl₂ → HCl + Cl). Right, as
// Dalton had it: single atoms, H + Cl → HCl. Measured natively (crates/md/tests/avogadro.rs):
// 300 molecules of two atoms hold the piston at 105 σ, 300 atoms at 103 σ.
//
// The chemistry is the page's: at each readout, atoms in contact react by the rules below, as
// bonds made and broken (add_bond, remove_bond), each atom with room for one bond. Atoms in HCl
// are switched to species 2 and 3 (same masses and forces) so that they show.

const N = 150;
const W = 150;
const RHO = 0.02;
/** The piston's height at the start: 2N molecules, "2 volumes". */
const H0 = (2 * N) / (RHO * W);
const H = 0;
const CL = 1;
const element = (s: number) => s % 2;
/** Bond energies, eV: H–H, H–Cl, Cl–Cl. */
const BOND = [
  [4.52, 4.47],
  [4.47, 2.48],
];
/** Atoms this close (σ) at a readout are touching. */
const CONTACT = 1.25;
/** Heat released into the atoms that react, ε per eV of bond energy gained (a much scaled-down
 * explosion: really 1 eV is 39 kT at room temperature). */
const HEAT_PER_EV = 0.3;
/** A free atom takes another's partner if the new bond is no weaker than this (eV) below the
 * old: Cl + H₂ → HCl + H costs 0.05 eV, which collisions supply. */
const SLACK = 0.1;
/** Chance that two free atoms that meet join (they need a third body to carry off the energy). */
const RECOMBINE = 0.3;
/** The lid is in contact with the room too: at each readout its speed loses this share and gains
 * the matching random jiggle at kT (an Ornstein–Uhlenbeck step), so that its bounce dies down
 * without it acting as a cold wall (friction alone made the gas under it denser). */
const FRICTION = 0.05;
const LID_MASS = 5;
/** The lamp: Cl₂ molecules in its beam (this wide, σ) split with this chance per readout. */
const BEAM = 14;
const PHOTOLYSIS = 0.15;

type Key = "A" | "D";
interface Side {
  dalton: boolean;
  /** Each atom's bond partner (−1: a free atom). */
  partner: Int32Array;
  species: Uint32Array;
  /** Dalton's atoms combine once a light has been shone. */
  lit: boolean;
  /** Where the lamp shines (σ), while it is held down. */
  beam: number | null;
  /** Split this many Cl₂ at the next readout (a flash). */
  flash: number;
  heights: number[];
  line: { x: number; y: number }[];
  h: number;
}
const sides: Record<Key, Side> = {
  A: { dalton: false, partner: new Int32Array(0), species: new Uint32Array(0), lit: false, beam: null, flash: 0, heights: [], line: [], h: H0 },
  D: { dalton: true, partner: new Int32Array(0), species: new Uint32Array(0), lit: false, beam: null, flash: 0, heights: [], line: [], h: H0 },
};

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), { xLabel: "time (τ)", yLabel: "volume (in volumes of the hydrogen)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

let seed = 3;
function rand() {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

function gauss() {
  return Math.sqrt(-2 * Math.log(1 - rand())) * Math.cos(2 * Math.PI * rand());
}

/** The species an atom should show: its element, +2 if it is in HCl. */
function shown(s: Side, i: number) {
  const p = s.partner[i];
  const e = element(s.species[i]);
  return p >= 0 && element(s.species[p]) !== e ? e + 2 : e;
}

/** Contacts: pairs of atoms (not bonded to each other) within CONTACT, in random order. */
function contacts(pos: Float32Array, s: Side) {
  const n = pos.length / 2;
  const cell = CONTACT;
  const grid = new Map<number, number[]>();
  const key = (cx: number, cy: number) => cx * 4099 + cy;
  for (let i = 0; i < n; i++) {
    const k = key(Math.floor(pos[2 * i] / cell), Math.floor(pos[2 * i + 1] / cell));
    if (!grid.has(k)) grid.set(k, []);
    grid.get(k)!.push(i);
  }
  const out: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    const [cx, cy] = [Math.floor(pos[2 * i] / cell), Math.floor(pos[2 * i + 1] / cell)];
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (const j of grid.get(key(cx + dx, cy + dy)) ?? []) {
          if (j <= i || s.partner[i] === j) continue;
          if ((pos[2 * j] - pos[2 * i]) ** 2 + (pos[2 * j + 1] - pos[2 * i + 1]) ** 2 < cell * cell) out.push([i, j]);
        }
  }
  for (let k = out.length - 1; k > 0; k--) {
    const m = Math.floor(rand() * (k + 1));
    [out[k], out[m]] = [out[m], out[k]];
  }
  return out;
}

/** React, split and recolour; returns the heat to release: [atom, atom, ε]. */
function chemistry(sim: DiscSim, s: Side, pos: Float32Array) {
  const n = s.partner.length;
  const busy = new Uint8Array(n);
  const heat: [number, number, number][] = [];
  const bond = (a: number, b: number) => BOND[element(s.species[a])][element(s.species[b])];
  const join = (a: number, b: number) => {
    sim.add_bond(a, b, 0);
    s.partner[a] = b;
    s.partner[b] = a;
  };
  const split = (a: number) => {
    const b = s.partner[a];
    sim.remove_bond(a, b);
    s.partner[a] = -1;
    s.partner[b] = -1;
    busy[a] = busy[b] = 1;
  };
  // Light: Cl₂ in the beam (or, in a flash, anywhere) splits into two free atoms.
  if (!s.dalton) {
    const cl2: number[] = [];
    for (let i = 0; i < n; i++) {
      const p = s.partner[i];
      if (p > i && element(s.species[i]) === CL && element(s.species[p]) === CL) cl2.push(i);
    }
    for (const i of cl2) {
      const inBeam = s.beam !== null && Math.abs(pos[2 * i] - s.beam) < BEAM / 2 && pos[2 * i + 1] < s.h;
      if (inBeam && rand() < PHOTOLYSIS) split(i);
    }
    for (let k = 0; k < s.flash && cl2.length; k++) {
      const i = cl2.splice(Math.floor(rand() * cl2.length), 1)[0];
      if (s.partner[i] >= 0) split(i);
    }
    s.flash = 0;
  }
  for (const [i, j] of contacts(pos, s)) {
    if (busy[i] || busy[j]) continue;
    const [pi, pj] = [s.partner[i], s.partner[j]];
    if (s.dalton) {
      // Dalton: two different free atoms combine.
      if (s.lit && pi < 0 && pj < 0 && element(s.species[i]) !== element(s.species[j])) {
        join(i, j);
        heat.push([i, j, HEAT_PER_EV * bond(i, j)]);
        busy[i] = busy[j] = 1;
      }
      continue;
    }
    if (pi < 0 && pj < 0) {
      if (rand() < RECOMBINE) {
        join(i, j);
        heat.push([i, j, HEAT_PER_EV * bond(i, j)]);
        busy[i] = busy[j] = 1;
      }
    } else if (pi < 0 || pj < 0) {
      // A free atom x meets y, bonded to z: x takes y if that bond is about as strong or stronger.
      const [x, y] = pi < 0 ? [i, j] : [j, i];
      const z = s.partner[y];
      const gain = bond(x, y) - bond(y, z);
      if (gain >= -SLACK) {
        sim.remove_bond(y, z);
        s.partner[z] = -1;
        join(x, y);
        if (gain > 0) heat.push([x, y, HEAT_PER_EV * gain]);
        busy[x] = busy[y] = busy[z] = 1;
      }
    }
  }
  // Recolour the atoms whose molecule changed.
  const moves: number[][] = [[], [], [], []];
  for (let i = 0; i < n; i++) {
    const t = shown(s, i);
    if (t !== s.species[i]) {
      moves[t].push(i);
      s.species[i] = t;
    }
  }
  moves.forEach((ids, t) => ids.length && sim.set_species(new Uint32Array(ids), t));
  return heat;
}

/** Give each reaction's heat to the atoms that reacted (a small box around them). */
async function release(sim: DiscSim, s: Side, pos: Float32Array, heat: [number, number, number][]) {
  if (!heat.length) return;
  const v = (await sim.velocities()) as Float32Array;
  const m = Array.from(sim.masses());
  for (const [a, b, q] of heat) {
    const ke = [a, b].reduce((k, i) => k + 0.5 * m[s.species[i]] * (v[2 * i] ** 2 + v[2 * i + 1] ** 2), 0);
    if (ke < 1e-3) continue;
    const [x, y] = [0.5 * (pos[2 * a] + pos[2 * b]), 0.5 * (pos[2 * a + 1] + pos[2 * b + 1])];
    sim.kick_region(x - 1.2, y - 1.2, x + 1.2, y + 1.2, Math.sqrt(1 + q / ke), 0, 0);
  }
}

function counts(s: Side) {
  const c = { h2: 0, cl2: 0, hcl: 0, free: 0 };
  for (let i = 0; i < s.partner.length; i++) {
    const p = s.partner[i];
    if (p < 0) c.free++;
    else if (p > i) {
      const [a, b] = [element(s.species[i]), element(s.species[p])];
      if (a !== b) c.hcl++;
      else if (a === H) c.h2++;
      else c.cl2++;
    }
  }
  return c;
}

function overlay(sim: DiscSim, s: Side, pos: Float32Array) {
  const lid = cssColor("--muted");
  const wt = cssColor("--accent");
  const glow = cssColor("--trace-back");
  const lamp = cssColor("--trace-4");
  const rects = [0, s.h, W, s.h + 2, lid[0], lid[1], lid[2], 1, W / 2 - 25, s.h + 2, W / 2 + 25, s.h + 10, wt[0], wt[1], wt[2], 1];
  if (s.beam !== null) rects.push(s.beam - BEAM / 2, 0, s.beam + BEAM / 2, s.h, lamp[0], lamp[1], lamp[2], 0.18);
  // Free atoms (radicals, which carry the chain) glow.
  if (!s.dalton) for (let i = 0; i < s.partner.length; i++) if (s.partner[i] < 0) rects.push(pos[2 * i] - 1.4, pos[2 * i + 1] - 1.4, pos[2 * i] + 1.4, pos[2 * i + 1] + 1.4, glow[0], glow[1], glow[2], 0.35);
  sim.set_overlay(new Float32Array(rects));
}

async function readout(key: Key, sim: DiscSim) {
  const s = sides[key];
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  const heat = chemistry(sim, s, pos);
  await release(sim, s, pos, heat);
  const [h, vp] = Array.from((await sim.piston()) as Float64Array);
  const keep = 1 - FRICTION;
  sim.set_piston_speed(keep * vp + Math.sqrt((1 - keep * keep) / LID_MASS) * gauss());
  s.h = h;
  s.heights.push(h);
  if (s.heights.length > 150) s.heights.shift();
  const t = sim.steps() * sim.dt();
  if (!s.line.length || t - s.line[s.line.length - 1].x > 1) s.line.push({ x: t, y: (2 * h) / H0 });
  if (s.line.length > 3000) s.line.shift();
  overlay(sim, s, pos);
  show();
}

/** The volume, in volumes of the hydrogen at the start, averaged over the last readouts. */
const volume = (s: Side) => (2 * s.heights.reduce((a, x) => a + x, 0)) / s.heights.length / H0;

function show() {
  const [a, d] = [sides.A, sides.D];
  const ref = (y: number, l: { x: number }[]) => (l.length ? [{ x: l[0].x, y }, { x: l[l.length - 1].x, y }] : []);
  plot.set([
    { points: ref(2, a.line), color: cssVar("--rule"), dashed: true },
    { points: ref(1, a.line), color: cssVar("--rule"), dashed: true },
    { points: a.line, color: cssVar("--accent") },
    { points: d.line, color: cssVar("--plot-back") },
  ]);
  for (const [k, s] of [["A", a], ["D", d]] as const) {
    if (!s.heights.length) continue;
    const c = counts(s);
    $(`vol${k}`).textContent = `${volume(s).toFixed(2)} volumes`;
    $(`mix${k}`).textContent = s.dalton ? `H ${c.free - countCl(s)}, Cl ${countCl(s)}, HCl ${c.hcl}` : `H₂ ${c.h2}, Cl₂ ${c.cl2}, HCl ${c.hcl}, free atoms ${c.free}`;
  }
}
function countCl(s: Side) {
  let k = 0;
  for (let i = 0; i < s.partner.length; i++) if (s.partner[i] < 0 && element(s.species[i]) === CL) k++;
  return k;
}

function makeHost(key: Key) {
  const s = sides[key];
  return new SimHost<DiscSim>({
    canvas: $<HTMLCanvasElement>(`canvas${key}`),
    overlay: $(`overlay${key}`),
    scenario: "avogadro",
    options: () => ({ n: N, width: W, density: RHO, temperature: 1, atoms: s.dalton ? 1 : 0, seed: 4 }),
    names: ["hydrogen", "chlorine", "hydrogen in HCl", "chlorine in HCl"],
    label: s.dalton ? "Dalton's atoms" : "Avogadro's molecules",
    colors: ["--trace-2", "--trace-3", "--trace-4", "--trace-1"],
    stepsPerFrame: () => 40,
    afterSteps: (sim) => readout(key, sim),
    onCreate: async (sim) => {
      const { species } = (await sim.all_positions()) as { species: Uint32Array };
      s.species = Uint32Array.from(species);
      s.partner = new Int32Array(species.length).fill(-1);
      const b = Array.from(sim.bonds());
      for (let k = 0; k < b.length; k += 3) {
        s.partner[b[k]] = b[k + 1];
        s.partner[b[k + 1]] = b[k];
      }
      Object.assign(s, { lit: false, beam: null, flash: 0, heights: [], line: [], h: H0 });
      const [w, h] = Array.from(sim.box_size());
      sim.set_view(-4, -2, w + 4, h);
      await readout(key, sim);
    },
  });
}
const hosts = { A: makeHost("A"), D: makeHost("D") };

$("run").onclick = () => {
  const on = !hosts.A.running;
  hosts.A.running = hosts.D.running = on;
  $("run").textContent = on ? "Pause" : "Run both";
};
$("flash").onclick = () => {
  // A flash: a fifth of the Cl₂ splits; Dalton's atoms may combine from now on.
  sides.A.flash = Math.round(N / 5);
  sides.A.lit = sides.D.lit = true;
};
$("reset").onclick = () => {
  void hosts.A.create();
  void hosts.D.create();
};
// The lamp: hold it over the left cylinder to shine a beam down through the gas.
{
  const c = hosts.A.canvas;
  const at = (e: PointerEvent) => {
    const sim = hosts.A.sim;
    if (!sim) return null;
    const r = c.getBoundingClientRect();
    const [x0, , x1] = Array.from(sim.view());
    return x0 + ((e.clientX - r.left) / r.width) * (x1 - x0);
  };
  c.addEventListener("pointerdown", (e) => {
    c.setPointerCapture(e.pointerId);
    sides.A.beam = at(e);
    sides.A.lit = sides.D.lit = true;
  });
  c.addEventListener("pointermove", (e) => {
    if (sides.A.beam !== null) sides.A.beam = at(e);
  });
  const off = () => (sides.A.beam = null);
  c.addEventListener("pointerup", off);
  c.addEventListener("pointercancel", off);
}

declare global {
  interface Window {
    avogadroTest?: {
      ready: Promise<void>;
      react(settle: number, steps: number, cool: number, average: number): Promise<Record<Key, { before: number; after: number; h2: number; cl2: number; hcl: number; free: number }>>;
    };
  }
}

window.avogadroTest = {
  ready: Promise.all([hosts.A.ready, hosts.D.ready]).then(() => {}),
  /** For both cylinders: settle for `settle` steps and average the volume over `average`; react
   * for `steps`, with a flash every 10,000 (as if the lamp were held on it); let it cool for
   * `cool`, and average the volume again; with what each is made of. */
  async react(settle: number, steps: number, cool: number, average: number) {
    const out = {} as Record<Key, { before: number; after: number; h2: number; cl2: number; hcl: number; free: number }>;
    for (const key of ["A", "D"] as const) {
      const host = hosts[key];
      const s = sides[key];
      out[key] = await host.exclusive(async (sim) => {
        const run = async (k: number) => {
          for (let q = 0; q < k / 40; q++) {
            sim.step(40);
            await readout(key, sim);
          }
        };
        const mean = async (k: number) => {
          s.heights = [];
          let [sum, m] = [0, 0];
          for (let q = 0; q < k / 200; q++) {
            await run(200);
            sum += s.h;
            m++;
          }
          return (2 * sum) / m / H0;
        };
        await run(settle);
        const before = await mean(average);
        s.lit = true;
        for (let k = 0; k < steps / 10000; k++) {
          s.flash = Math.round(N / 5);
          await run(10000);
        }
        await run(cool);
        const after = await mean(average);
        sim.render();
        return { before, after, ...counts(s) };
      });
    }
    return out;
  },
};
