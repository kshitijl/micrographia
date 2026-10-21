import { BallSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// A crystal slab under its liquid (scenario freeze3), with salt poured in by hand. Measured
// natively (crates/md/tests/freeze3.rs): pure, the slab grows at kT ≤ 0.60 and melts at 0.64;
// with 15% salt it stops part way (53% of the solvent crystalline at 0.52, against 94% pure).

const tIn = $<HTMLInputElement>("t");
const cut = $<HTMLInputElement>("cut");
const onlyCrystal = $<HTMLInputElement>("onlycrystal");
const plot = new LinesPlot($<HTMLCanvasElement>("plot"), { xLabel: "time (τ)", yLabel: "solvent in the crystal", yMin: 0, yMax: 1, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
/** Atoms turned into salt per pour. */
const POUR = 150;
/** Neighbours within this distance count for the crystal test. */
const R_CUT = 1.5;
let line: { x: number; y: number }[] = [];
let prev: { x: number; y: number }[] = [];
let lastTest = -Infinity;
let bonds: Uint8Array | null = null;
let species: Uint32Array | null = null;
const camera = { yaw: 0.5, pitch: 0.25, distance: 1.25 };

function show(sim: BallSim) {
  if (!bonds || !species) return;
  const v = new Float32Array(bonds.length);
  for (let i = 0; i < v.length; i++) {
    if (species[i] === 1) v[i] = 1;
    else if (bonds[i] >= 7) v[i] = 0;
    else v[i] = onlyCrystal.checked ? -1 : 0.5;
  }
  sim.color_by_values(v);
}

async function readout(sim: BallSim) {
  const now = performance.now();
  if (now - lastTest < 700 && bonds) return;
  lastTest = now;
  const b = await sim.crystal_bonds(R_CUT);
  const all = await sim.all_positions();
  const kin = await sim.kinetic_energies();
  bonds = b as Uint8Array;
  species = (all as { species: Uint32Array }).species;
  const ke = (kin as { ke: Float32Array }).ke;
  let [solvent, frozen, salt, liquid] = [0, 0, 0, 0];
  for (let i = 0; i < bonds.length; i++) {
    if (species[i] === 1) {
      salt++;
      liquid++;
    } else {
      solvent++;
      if (bonds[i] >= 7) frozen++;
      else liquid++;
    }
  }
  show(sim);
  const time = sim.steps() * sim.dt();
  line.push({ x: time, y: frozen / solvent });
  plot.set([
    { points: prev, color: cssVar("--muted") },
    { points: line, color: cssVar("--accent") },
  ]);
  $("xtal").textContent = `${Math.round((100 * frozen) / solvent)}%`;
  $("conc").textContent = salt ? `${((100 * salt) / liquid).toFixed(1)}% of its atoms` : "none";
  $("tm").textContent = ((2 / 3) * (ke.reduce((a, x) => a + x, 0) / ke.length)).toFixed(3);
  $("time").textContent = `${time.toFixed(0)} τ`;
}

const host = new SimHost<BallSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "freeze3",
  create: (c, name, o) => BallSim.create(c, name, o),
  options: () => ({ temperature: Number(tIn.value), seed: 1 }),
  names: ["solvent", "salt"],
  colors: ["--species-2", "--species-3"],
  stepsPerFrame: () => 20,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    if (line.length) prev = line;
    line = [];
    bonds = null;
    sim.set_camera(camera.yaw, camera.pitch, camera.distance);
    sim.set_cut(Number(cut.value));
    sim.set_ramp_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3"), ...cssColor("--cream")]));
    await readout(sim);
  },
});

/** Turn up to `count` liquid solvent atoms that have room (no neighbour within 1 σ, so a
 * salt atom, 1.1 σ against the solvent, does not land on top of one) into salt. */
async function pour(sim: BallSim, count: number) {
  const { pos, species: sp } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const b = (await sim.crystal_bonds(R_CUT)) as Uint8Array;
  const [lx, ly, lz] = Array.from(sim.box_size());
  const n = sp.length;
  // A grid of 1.2 σ cells for the neighbour check.
  const c = 1.2;
  const [nx, ny, nz] = [Math.floor(lx / c), Math.floor(ly / c), Math.floor(lz / c)];
  const cell = (i: number) => {
    const [x, y, z] = [Math.min(nx - 1, Math.floor((pos[3 * i] / lx) * nx)), Math.min(ny - 1, Math.floor((pos[3 * i + 1] / ly) * ny)), Math.min(nz - 1, Math.floor((pos[3 * i + 2] / lz) * nz))];
    return [x, y, z];
  };
  const grid = new Map<number, number[]>();
  const key = (x: number, y: number, z: number) => (z * ny + y) * nx + x;
  for (let i = 0; i < n; i++) {
    const [x, y, z] = cell(i);
    const k = key(x, y, z);
    if (!grid.has(k)) grid.set(k, []);
    grid.get(k)!.push(i);
  }
  // y has walls; x and z are periodic.
  const room = (i: number) => {
    const [x, y, z] = cell(i);
    for (let dz = -1; dz <= 1; dz++)
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++) {
          if (y + dy < 0 || y + dy >= ny) continue;
          for (const j of grid.get(key((x + dx + nx) % nx, y + dy, (z + dz + nz) % nz)) ?? []) {
            if (j === i) continue;
            let [ddx, ddz] = [Math.abs(pos[3 * j] - pos[3 * i]), Math.abs(pos[3 * j + 2] - pos[3 * i + 2])];
            ddx = Math.min(ddx, lx - ddx);
            ddz = Math.min(ddz, lz - ddz);
            if (ddx * ddx + ddz * ddz + (pos[3 * j + 1] - pos[3 * i + 1]) ** 2 < 1.0) return false;
          }
        }
    return true;
  };
  // Liquid solvent atoms (few crystal bonds, with neighbours: not the vapour), in random order.
  const candidates: number[] = [];
  for (let i = 0; i < n; i++) if (sp[i] === 0 && b[i] < 4 && room(i)) candidates.push(i);
  for (let i = candidates.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [candidates[i], candidates[j]] = [candidates[j], candidates[i]];
  }
  const chosen = candidates.slice(0, count);
  if (host.sim !== sim || !chosen.length) return 0;
  sim.set_species(new Uint32Array(chosen), 1);
  await sim.sync();
  lastTest = -Infinity;
  return chosen.length;
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("salt").onclick = () => {
  if (!host.sim) return;
  void host.exclusive(async (sim) => {
    await pour(sim, POUR);
    await readout(sim);
    sim.render();
  });
};
$("reset").onclick = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
tIn.oninput = () => {
  $("tv").textContent = Number(tIn.value).toFixed(3);
  host.sim?.set_thermostat(Number(tIn.value), 0.2);
};
cut.oninput = () => {
  const f = Number(cut.value);
  $("cutv").textContent = f === 0 ? "none" : `${Math.round(100 * f)}%`;
  host.sim?.set_cut(f);
};
onlyCrystal.onchange = () => host.sim && show(host.sim);

// Drag to turn, scroll to zoom.
{
  const c = host.canvas;
  let drag: { x: number; y: number } | null = null;
  c.addEventListener("pointerdown", (e) => {
    c.setPointerCapture(e.pointerId);
    drag = { x: e.clientX, y: e.clientY };
  });
  c.addEventListener("pointermove", (e) => {
    if (!drag) return;
    camera.yaw -= (e.clientX - drag.x) * 0.008;
    camera.pitch = Math.max(-1.4, Math.min(1.4, camera.pitch + (e.clientY - drag.y) * 0.008));
    drag = { x: e.clientX, y: e.clientY };
    host.sim?.set_camera(camera.yaw, camera.pitch, camera.distance);
  });
  c.addEventListener("pointerup", () => (drag = null));
  c.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      camera.distance = Math.max(0.6, Math.min(3, camera.distance * Math.exp(e.deltaY * 0.001)));
      host.sim?.set_camera(camera.yaw, camera.pitch, camera.distance);
    },
    { passive: false },
  );
}

declare global {
  interface Window {
    freezingTest?: { ready: Promise<void>; run(t: number, pours: number, steps: number): Promise<{ crystal: number; salt: number; t: number }> };
  }
}

window.freezingTest = {
  ready: host.ready,
  /** Start again at kT `t`, pour `pours` times, run `steps`; the solvent's crystalline share. */
  async run(t: number, pours: number, steps: number) {
    tIn.value = String(t);
    await host.create();
    return host.exclusive(async (sim) => {
      let salt = 0;
      for (let k = 0; k < pours; k++) salt += await pour(sim, POUR);
      for (let k = 0; k < steps / 1000; k++) sim.step(1000);
      lastTest = -Infinity;
      await readout(sim);
      sim.render();
      return { crystal: Number(($("xtal").textContent ?? "0").replace("%", "")) / 100, salt, t: Number($("tm").textContent) };
    });
  },
};
