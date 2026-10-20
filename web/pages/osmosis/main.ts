import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot, ScatterPlot } from "../../src/lib/plot";
import { Tools } from "../../src/lib/interact";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const cIn = $<HTMLInputElement>("c");
const tIn = $<HTMLInputElement>("t");
const attract = $<HTMLInputElement>("attract");

let base: { imp: Float64Array; steps: number } | null = null;
let soluteCount = 0;
let box = [1, 1];
let piNow = NaN;
let cNow = NaN;
const recorded: { x: number; y: number; e: number }[] = [];

const plot = new ScatterPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "solute concentration c (per σ²)",
  yLabel: "Π / kT (points) and van 't Hoff (line)",
  point: () => cssVar("--accent"),
  line: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const temperature = () => Number(tIn.value);

async function readout(sim: DiscSim) {
  const l = (await sim.ledger(1)) as { impulse: Float64Array };
  if (!base) {
    base = { imp: l.impulse.slice(), steps: host.elapsed };
    return;
  }
  const t = (host.elapsed - base.steps) * sim.dt();
  if (t <= 0) return;
  const d = (w: number) => l.impulse[w] - base!.imp[w];
  const pi = (d(5) - d(4)) / (t * box[1]);
  const pl = d(0) / (t * box[1]);
  const pr = d(1) / (t * box[1]);
  piNow = pi;
  cNow = soluteCount / (box[1] * (box[0] / 2 - 0.5));
  $("pi").textContent = pi.toFixed(4);
  $("vth").textContent = (cNow * temperature()).toFixed(4);
  $("pl").textContent = pl.toFixed(3);
  $("pr").textContent = pr.toFixed(3);
  $("status").textContent = `Averaging over ${t.toFixed(0)} τ. Right wall minus left wall: ${(pr - pl).toFixed(4)}; push on the membrane: ${pi.toFixed(4)}.`;
}

function overlay(sim: DiscSim) {
  const c = cssColor("--accent");
  const r: number[] = [];
  const x = box[0] / 2;
  for (let y = 0; y < box[1]; y += 2) r.push(x - 0.15, y, x + 0.15, y + 1.2, c[0], c[1], c[2], 0.9);
  sim.set_overlay(new Float32Array(r));
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "osmosis",
  options: () => ({ n: 16000, density: 0.5, solute: Number(cIn.value), temperature: temperature(), attraction: attract.checked ? 1 : 0, seed: 8 }),
  names: ["water", "solute"],
  colors: ["--species-2", "--cream"],
  stepsPerFrame: () => 40,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    box = Array.from(sim.box_size());
    const h = (await sim.histogram()) as { marked: Uint32Array };
    soluteCount = h.marked.reduce((a, b) => a + b, 0);
    base = null;
    overlay(sim);
  },
});

function restart() {
  const run = host.running;
  void host.create().then(() => (host.running = run));
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("reset").onclick = restart;
$("record").onclick = () => {
  if (!Number.isFinite(piNow)) return;
  recorded.push({ x: cNow, y: piNow / temperature(), e: 0 });
  plot.set(recorded, 1);
};
cIn.oninput = () => ($("cv").textContent = `${Math.round(100 * Number(cIn.value))}%`);
cIn.onchange = restart;
attract.onchange = restart;
tIn.oninput = () => ($("tv").textContent = Number(tIn.value).toFixed(2));
tIn.onchange = () => {
  host.sim?.set_thermostat(temperature(), 0.5);
  base = null;
};

declare global {
  interface Window {
    osmosisTest?: { ready: Promise<void>; measure(steps: number): Promise<{ pi: number; c: number; t: number }> };
  }
}

window.osmosisTest = {
  ready: host.ready,
  async measure(steps: number) {
    await host.ready;
    await host.exclusive(async (sim) => {
      base = null;
      await readout(sim);
      for (let k = 0; k < steps / 1000; k++) {
        sim.step(1000);
        host.elapsed += 1000;
      }
      await readout(sim);
    });
    return { pi: piNow, c: cNow, t: temperature() };
  },
};

// ---- 1. A membrane that can move. ----
const W_A = 96;
const H_A = 48;
let trackA: { x: number; y: number }[] = [];
const plotA = new LinesPlot($<HTMLCanvasElement>("plotA"), { xLabel: "time (τ)", yLabel: "membrane position (σ)", yMin: 0, yMax: W_A, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

async function readoutA(sim: DiscSim) {
  const beads = (await sim.positions(2)) as Float32Array;
  let x = 0;
  for (let k = 0; k < beads.length; k += 2) x += beads[k];
  x /= beads.length / 2;
  const t = sim.steps() * sim.dt();
  trackA.push({ x: t, y: x });
  plotA.set([{ points: trackA, color: cssVar("--accent") }]);
  $("xm").textContent = `${x.toFixed(1)} σ`;
  $("tA").textContent = t.toFixed(0);
}

const hostA = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasA"),
  overlay: $("overlayA"),
  scenario: "osmosis_free",
  options: () => ({ width: W_A, height: H_A, solute: 0.5, seed: 21 }),
  names: ["water", "solute", "membrane beads"],
  label: "a membrane that can move",
  colors: ["--species-2", "--cream", "--accent"],
  stepsPerFrame: () => 60,
  afterSteps: (sim) => readoutA(sim),
  onCreate: async (sim) => {
    trackA = [];
    await readoutA(sim);
  },
});
new Tools({ host: hostA, container: $("toolsA"), after: (sim) => readoutA(sim) });
$("runA").onclick = () => {
  hostA.running = !hostA.running;
  $("runA").textContent = hostA.running ? "Pause" : "Run";
};
$("resetA").onclick = () => void hostA.create().then(() => ($("runA").textContent = "Run"));

// ---- 2 and 3. Water climbing a tube (osmometers). ----
type Book = { lift: number; heat: number; bonds: number; n: number; kT: number };
type Levels = { l: number; r: number; pred: number; smooth: number };

/** An osmometer panel: the page's elements carry the suffix `s` (canvasB, hlB, …). `channel` > 0
 * is the classic U-tube, the arms meeting only through a passage that high at the bottom. */
type Geometry = { width: number; height: number; depth: number; channel: number; settle: number; view: [number, number, number, number] };

function tube(s: string, cfg: { geometry: () => Geometry; gravity: number; temperature: number; bath: boolean; seed: number; label: string; energy: boolean }) {
  const [G, T] = [cfg.gravity, cfg.temperature];
  let geo = cfg.geometry();
  let [W, H] = [geo.width, geo.height];
  let track: { x: number; y: number }[] = [];
  let preds: { x: number; y: number }[] = [];
  let raw: number[] = [];
  const out = {
    levels: { l: NaN, r: NaN, pred: NaN, smooth: NaN } as Levels,
    host: null as unknown as SimHost,
    readout: null as unknown as (sim: DiscSim) => Promise<void>,
    kT: [] as number[],
    book: { lift: 0, heat: 0, bonds: 0, n: 0, kT: NaN },
  };
  const plot = new LinesPlot($<HTMLCanvasElement>(`plot${s}`), { xLabel: "time (τ)", yLabel: "Δh (σ)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
  // Energy bookkeeping (constant energy only): changes since the start in the energy of the water's
  // height (m g h), the kinetic energy (heat) and the molecules' interactions, each smoothed over
  // the last ~50 readings.
  const eplot = cfg.energy ? new LinesPlot($<HTMLCanvasElement>(`energy${s}`), { xLabel: "time (τ)", yLabel: "change since the start (ε)", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") }) : null;
  let e0: number[] | null = null;
  let recent: number[][] = [];
  let ebook: { x: number; y: number }[][] = [[], [], [], []];

  /** The surface height of the liquid in x ∈ [x0, x1): the top of its density profile (1 σ bins,
   * smoothed over 3) where it falls through 0.35 (vapour ~0.01, liquid ~0.7); and the density in
   * the lower half of the column. As in crates/md/tests/osmosis_demos.rs. */
  function surface(pos: Float32Array, species: Uint32Array, x0: number, x1: number): [number, number] {
    const c = new Float64Array(H);
    for (let i = 0; i < species.length; i++) {
      const [x, y] = [pos[2 * i], pos[2 * i + 1]];
      if (species[i] !== 2 && x >= x0 && x < x1 && y >= 0 && y < H) c[Math.floor(y)] += 1 / (x1 - x0);
    }
    const sm = Array.from(c, (_, k) => (c[Math.max(0, k - 1)] + c[k] + c[Math.min(H - 1, k + 1)]) / 3);
    let top = H - 1;
    while (top > 0 && sm[top] <= 0.35) top--;
    const y = top + 0.5 + (top + 1 < H ? (sm[top] - 0.35) / Math.max(1e-9, sm[top] - sm[top + 1]) : 0);
    const lo = Math.max(3, Math.floor(top / 2));
    let rho = 0;
    for (let k = 2; k < lo; k++) rho += sm[k];
    return [y, rho / (lo - 2)];
  }

  /** Levels in each arm, the height difference (smoothed over the last ~100 readings) and van 't
   * Hoff's Δh = c kT / (ρ g). */
  async function readout(sim: DiscSim) {
    const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
    const half = W / 2;
    if (eplot && sim.steps() > 0) await energy(sim, pos, species, sim.steps() * sim.dt()); // forces exist from the first step
    const [hl, rl] = surface(pos, species, 2, half - 1);
    const [hr, rr] = surface(pos, species, half + 1, W - 2);
    let sol = 0;
    for (let i = 0; i < species.length; i++) if (species[i] === 1 && pos[2 * i + 1] < hr + 3) sol++;
    const c = sol / (hr * (half - 1));
    const pred = (c * T) / (0.5 * (rl + rr) * G);
    const t = sim.steps() * sim.dt();
    raw.push(hr - hl);
    if (raw.length > 100) raw.shift();
    const smooth = raw.reduce((a, b) => a + b, 0) / raw.length;
    out.levels = { l: hl, r: hr, pred, smooth };
    track.push({ x: t, y: smooth });
    preds.push({ x: t, y: pred });
    plot.set([
      { points: preds, color: cssVar("--muted"), dashed: true },
      { points: track, color: cssVar("--accent") },
    ]);
    $(`hl${s}`).textContent = `${hl.toFixed(1)} σ`;
    $(`hr${s}`).textContent = `${hr.toFixed(1)} σ`;
    $(`dh${s}`).textContent = `${smooth.toFixed(1)} σ`;
    $(`dhp${s}`).textContent = `${pred.toFixed(1)} σ`;
    // The membrane (only in the passage, for the U-tube), and a mark at each arm's level.
    const m = cssColor("--accent");
    const mark = cssColor("--highlight");
    const ov: number[] = [];
    for (let y = 0; y < (geo.channel > 0 ? geo.channel : H); y += 2) ov.push(half - 0.15, y, half + 0.15, y + 1.2, ...m);
    ov.push(0, hl - 0.15, half - 0.5, hl + 0.15, ...mark, half + 0.5, hr - 0.15, W, hr + 0.15, ...mark);
    sim.set_overlay(new Float32Array(ov));
  }
  out.readout = readout;

  async function energy(sim: DiscSim, pos: Float32Array, species: Uint32Array, t: number) {
    const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
    const u = (await sim.potential_energy()) as number; // includes m g h for every particle
    let [k, lift, all, n] = [0, 0, 0, 0];
    for (let i = 0; i < species.length; i++) {
      all += G * pos[2 * i + 1];
      if (species[i] === 2) continue; // the walls never move
      k += ke[i];
      lift += G * pos[2 * i + 1];
      n++;
    }
    const now = [lift, k, u - all];
    e0 ??= now;
    recent.push(now.map((v, j) => v - e0![j]));
    if (recent.length > 50) recent.shift();
    const avg = [0, 1, 2].map((j) => recent.reduce((a, r) => a + r[j], 0) / recent.length);
    [...avg, avg[0] + avg[1] + avg[2]].forEach((v, j) => ebook[j].push({ x: t, y: v }));
    eplot!.set([
      { points: ebook[0], color: cssVar("--accent") },
      { points: ebook[1], color: cssVar("--species-3") },
      { points: ebook[2], color: cssVar("--muted") },
      { points: ebook[3], color: cssVar("--text"), dashed: true },
    ]);
    const kT = k / n; // this reading
    out.kT.push(kT);
    if (out.kT.length > 200) out.kT.shift();
    const kTavg = out.kT.reduce((a, b) => a + b, 0) / out.kT.length;
    $(`kt${s}`).textContent = kTavg.toFixed(4);
    // The energy stored in the height difference: the arms (each ~W/2 − 2 wide) moved Δh/2 apart
    // from the common level, ρ g w (Δh/2)². Taken from the heat alone, it would cool the water by
    // that over n.
    const stored = 0.72 * G * (W / 2 - 2) * (out.levels.smooth / 2) ** 2;
    $(`ktd${s}`).textContent = Number.isFinite(stored) ? `−${(stored / n).toFixed(4)}` : "–";
    out.book = { lift: avg[0], heat: avg[1], bonds: avg[2], n, kT: kTavg };
  }

  out.host = new SimHost({
    canvas: $<HTMLCanvasElement>(`canvas${s}`),
    overlay: $(`overlay${s}`),
    scenario: "osmometer",
    options: () => {
      geo = cfg.geometry();
      [W, H] = [geo.width, geo.height];
      return { width: W, height: H, depth: geo.depth, gravity: G, temperature: T, solute: 0.3, channel: geo.channel, bath: cfg.bath ? 1 : 0, settle: geo.settle, seed: cfg.seed };
    },
    names: ["water", "solute", "wall beads"],
    label: cfg.label,
    colors: ["--species-2", "--cream", "--muted"],
    stepsPerFrame: () => Number($<HTMLInputElement>(`speed${s}`).value),
    afterSteps: (sim) => readout(sim),
    onCreate: async (sim) => {
      sim.set_view(...geo.view);
      track = [];
      preds = [];
      raw = [];
      e0 = null;
      recent = [];
      ebook = [[], [], [], []];
      out.kT = [];
      await readout(sim);
    },
  });
  const host = out.host;
  $(`run${s}`).onclick = () => {
    host.running = !host.running;
    $(`run${s}`).textContent = host.running ? "Pause" : "Run";
  };
  $(`reset${s}`).onclick = () => void host.create().then(() => ($(`run${s}`).textContent = "Run"));
  $<HTMLInputElement>(`speed${s}`).oninput = () => ($(`speed${s}v`).textContent = `${$<HTMLInputElement>(`speed${s}`).value} steps/frame`);
  return out;
}

// 2: the membrane runs the whole height between two narrow arms (with a heat bath).
const tubeB = tube("B", { geometry: () => ({ width: 32, height: 112, depth: 40, channel: 0, settle: 0, view: [0, 0, 32, 64] }), gravity: 0.006, temperature: 0.5, bath: true, seed: 22, label: "water climbing a tube", energy: false });
// 3: the classic U-tube. Measured natively (crates/md/tests/osmosis_demos.rs, utube_rises): Δh
// ~11 σ by 1000 τ, then 12–21 σ against van 't Hoff's ~16 σ, with no heat bath. The Andersen
// bath, which redraws velocities, slows the flow through the passage (~4 σ by 2000 τ).
// "Big" is ~10× the molecules: arms 89 σ wide and 240 σ deep, with an 80 σ passage (about the
// same passage height per arm width, so it fills in a similar time).
const sizeC = $<HTMLSelectElement>("sizeC");
const tubeC = tube("C", {
  geometry: () =>
    sizeC.value === "big"
      ? { width: 180, height: 300, depth: 240, channel: 80, settle: 300, view: [-14, 0, 194, 260] }
      : { width: 50, height: 150, depth: 90, channel: 30, settle: 200, view: [-15, 0, 65, 100] },
  gravity: 0.008,
  temperature: 0.5,
  // Each settles as pure water first (compacting and evaporating), so the energy plot shows
  // osmosis alone; set_view takes corners.
  bath: false,
  seed: 1,
  label: "the classic U-tube",
  energy: true,
});
sizeC.onchange = () => void tubeC.host.create().then(() => ($("runC").textContent = "Run"));

declare global {
  interface Window {
    osmosisDemos?: {
      ready: Promise<void>;
      membrane(steps: number): Promise<{ start: number; end: number }>;
      tube(steps: number, which?: "B" | "C", size?: "small" | "big"): Promise<{ dh: number[]; pred: number; book: Book; kT: number[]; books: Book[] }>;
    };
  }
}

window.osmosisDemos = {
  ready: Promise.all([hostA.ready, tubeB.host.ready, tubeC.host.ready]).then(() => undefined),
  async membrane(steps: number) {
    await hostA.ready;
    return hostA.exclusive(async (sim) => {
      const start = trackA[trackA.length - 1].y;
      for (let k = 0; k < steps / 2000; k++) {
        sim.step(2000);
        await readoutA(sim);
      }
      return { start, end: trackA[trackA.length - 1].y };
    });
  },
  async tube(steps: number, which: "B" | "C" = "B", size: "small" | "big" = "small") {
    const t = which === "B" ? tubeB : tubeC;
    await t.host.ready;
    if (which === "C" && sizeC.value !== size) {
      sizeC.value = size;
      await t.host.create();
    }
    const kT: number[] = [];
    const books: (typeof tubeC.book)[] = [];
    return t.host.exclusive(async (sim) => {
      const dh: number[] = [];
      for (let k = 0; k < steps / 2000; k++) {
        sim.step(2000);
        await t.readout(sim);
        dh.push(t.levels.smooth);
        kT.push(t.book.kT);
        books.push({ ...t.book });
      }
      return { dh, pred: t.levels.pred, book: t.book, kT, books };
    });
  },
};
