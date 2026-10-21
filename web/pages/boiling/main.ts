import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { side, warmth } from "../../src/lib/profiles";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";
import "./freeenergy";

const BINS = 80;
const tIn = $<HTMLInputElement>("t");
let box = [200, 160];
let sumV = 0;
let sumL = 0;
let frames = 0;
let lastP = NaN;
const points: { x: number; y: number }[] = [];

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "1 / T",
  yLabel: "ln (vapour pressure)",
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const temperature = () => Number(tIn.value);
const heating = () => $<HTMLSelectElement>("heat").value as "plate" | "layer" | "bath";
/** Heated from below (a plate or a layer), rather than by a bath throughout. */
const plate = () => heating() !== "bath";
const plateIn = $<HTMLInputElement>("plateT");
const plateT = () => Number(plateIn.value);
// Speed colours are scaled to kT = 0.42 (the middle of the temperature range): a typical molecule
// there is cream, and the ramp saturates at twice its rms speed.
const V_MAX = 2 * Math.sqrt(2 * 0.42);

/** Colour by speed now, by energy averaged over ~2 τ (kinetic energy is ½v², so the same
 * order), or plain. */
function applyColoring(sim: DiscSim) {
  const mode = $<HTMLSelectElement>("colorby").value;
  if (mode === "plain") {
    sim.color_by_speed(0);
    sim.smooth_speed_colors(0);
    return;
  }
  sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--highlight"), ...cssColor("--cream")]));
  sim.color_by_speed(V_MAX);
  sim.smooth_speed_colors(mode === "smooth" ? 2 : 0);
}

// ---- Profiles beside the pot: density and temperature at each height, averaged over frames. ----
const SIDE_BINS = 32;
let sideN = new Float64Array(SIDE_BINS);
let sideE = new Float64Array(SIDE_BINS);
let sideFresh = true;

function drawSides(sim: DiscSim, pos: Float32Array, ke: Float32Array) {
  const [w, h] = [box[0], box[1]];
  const dh = h / SIDE_BINS;
  const [n, e] = [new Float64Array(SIDE_BINS), new Float64Array(SIDE_BINS)];
  for (let i = 0; i < ke.length; i++) {
    const b = Math.max(0, Math.min(SIDE_BINS - 1, Math.floor(pos[2 * i + 1] / dh)));
    n[b]++;
    e[b] += ke[i];
  }
  const a = sideFresh ? 1 : 0.1;
  sideFresh = false;
  for (let b = 0; b < SIDE_BINS; b++) {
    sideN[b] += a * (n[b] - sideN[b]);
    sideE[b] += a * (e[b] - sideE[b]);
  }
  const rho = Array.from(sideN, (c) => c / (dh * w));
  const t = Array.from(sideE, (x, b) => (sideN[b] > 3 ? x / sideN[b] : 0));
  const tAvg = ke.reduce((x, y) => x + y, 0) / ke.length;
  // Line the bars up with the box inside the drawn view (which also shows the plate and the lid).
  const v = sim.view();
  const span: [number, number] = [(0 - v[1]) / (v[3] - v[1]), (h - v[1]) / (v[3] - v[1])];
  side("sideRho", { values: [rho], colors: [cssVar("--species-2")], hi: 0.9, label: (x) => x.toFixed(1), span });
  side("sideT", {
    values: [t],
    colors: [(b) => warmth(t[b] / tAvg, 0.25)],
    dashed: plate() ? new Array(SIDE_BINS).fill(plateT()) : undefined,
    hi: Math.max(plate() ? plateT() : 0, ...t, 0.1) * 1.15,
    label: (x) => x.toFixed(2),
    span,
  });
}

/** The hot plate under the floor (its glow follows its temperature) and the insulating lid, drawn
 * just outside the box; nothing extra with the bulk heat bath. */
function drawPlate(sim: DiscSim) {
  const [w, h] = [box[0], box[1]];
  if (!plate()) {
    sim.set_overlay(new Float32Array([]));
    sim.set_view(0, 0, w, h);
    return;
  }
  const t = Math.max(0, Math.min(1, (plateT() - 0.3) / 0.9));
  const [cool, hot] = [[0.45, 0.08, 0.04], [1.0, 0.85, 0.35]];
  const glow = cool.map((c, i) => c + (hot[i] - c) * t);
  const lid = cssColor("--muted");
  sim.set_view(0, -3, w, h + 2);
  sim.set_overlay(new Float32Array([0, -3, w, 0, glow[0], glow[1], glow[2], 1, 0, h, w, h + 2, lid[0], lid[1], lid[2], 1]));
  $("platev").textContent = plateT().toFixed(2);
}

function clear() {
  sumV = 0;
  sumL = 0;
  frames = 0;
}

function fit(): { slope: number; intercept: number } | null {
  if (points.length < 2) return null;
  const n = points.length;
  const mx = points.reduce((a, p) => a + p.x, 0) / n;
  const my = points.reduce((a, p) => a + p.y, 0) / n;
  const slope = points.reduce((a, p) => a + (p.x - mx) * (p.y - my), 0) / points.reduce((a, p) => a + (p.x - mx) ** 2, 0);
  return { slope, intercept: my - slope * mx };
}

function redraw() {
  const f = fit();
  const series = [{ points: points.slice(), color: cssVar("--accent"), dots: true }];
  if (f) {
    const xs = points.map((p) => p.x);
    const [a, b] = [Math.min(...xs), Math.max(...xs)];
    series.push({ points: [{ x: a, y: f.intercept + f.slope * a }, { x: b, y: f.intercept + f.slope * b }], color: cssVar("--plot-back"), dots: false });
    $("lat").textContent = `${(-f.slope).toFixed(2)} ε`;
  }
  plot.set(series);
}

async function readout(sim: DiscSim) {
  const h = (await sim.histogram()) as { all: Uint32Array };
  const dh = box[1] / BINS;
  const rho = Array.from(h.all, (c) => c / (dh * box[0]));
  // The surface: the first bin (from the bottom) where the density drops below half the liquid's.
  const liquidRho = rho.slice(3, 8).reduce((a, b) => a + b, 0) / 5;
  let surface = rho.findIndex((r, k) => k > 3 && r < 0.5 * liquidRho);
  if (surface < 0) surface = BINS - 1;
  // Vapour: from 6 σ above the surface to 6 σ below the ceiling.
  const lo = Math.min(BINS - 2, surface + Math.ceil(6 / dh));
  const hi = BINS - Math.ceil(6 / dh);
  let nv = 0;
  for (let k = lo; k < hi; k++) nv += h.all[k];
  const vapour = nv / ((hi - lo) * dh * box[0]);
  sumV += vapour;
  sumL += liquidRho;
  frames++;
  const rv = sumV / frames;
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  drawSides(sim, pos, ke);
  const tMeasured = ke.reduce((a, b) => a + b, 0) / ke.length;
  // With the hot plate the temperature is whatever the molecules have; with the bath, the set one.
  lastP = rv * (plate() ? tMeasured : temperature());
  $("tm").textContent = tMeasured.toFixed(3);
  $("nv").textContent = String(nv);
  $("pv").textContent = lastP.toExponential(2);
  $("rl").textContent = (sumL / frames).toFixed(3);
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "boiling",
  // The box grows with the number of molecules, keeping the pool's depth and the vapour space.
  options: () => {
    const n = Number($<HTMLSelectElement>("nmol").value);
    const f = Math.sqrt(n / 6000);
    return { n, width: 200 * f, height: 160 * f, temperature: temperature(), plate: heating() === "plate" ? plateT() : 0, layer: heating() === "layer" ? plateT() : 0, seed: 13 };
  },
  names: ["molecules"],
  colors: ["--species-2"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    box = Array.from(sim.box_size());
    $("platerow").hidden = !plate();
    $("tlabel").textContent = plate() ? "Starting temperature" : "Temperature";
    applyColoring(sim);
    drawPlate(sim);
    sideFresh = true;
    clear();
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("record").onclick = () => {
  if (frames < 20 || !(lastP > 0)) return;
  points.push({ x: 1 / temperature(), y: Math.log(lastP) });
  redraw();
  $("status").textContent = `${points.length} points recorded. Change the temperature, wait, and record again.`;
};
tIn.oninput = () => ($("tv").textContent = temperature().toFixed(3));
tIn.onchange = () => {
  if (!plate()) host.sim?.set_thermostat(temperature(), 0.5);
  clear();
};
plateIn.oninput = () => {
  if (!host.sim) return;
  if (heating() === "plate") host.sim.set_wall(2, plateT());
  else host.sim.set_thermostat(plateT(), 1.0); // the layer is a bath confined to the bottom 3 σ
  drawPlate(host.sim);
  if (!host.running) host.sim.render();
};
$("colorby").onchange = () => {
  if (!host.sim) return;
  applyColoring(host.sim);
  if (!host.running) host.sim.render();
};
function restart() {
  const run = host.running;
  void host.create().then(() => (host.running = run));
}
$("heat").onchange = restart;
$("nmol").onchange = restart;

// ---- 2. Latent heat: a droplet in a sealed flask, heated in steps. ----
// The flask conserves energy except for the heat the page adds, which is exact: each addition
// scales every velocity by s, adding K (s² − 1). The heat then ends up as kinetic energy (the
// temperature) or as potential energy (molecules pulled out of the liquid, against their
// attraction): the latent heat. Measured natively (crates/md/tests/flask.rs): ~3 ε per molecule
// to evaporate it all; heated too fast, the liquid overheats because evaporation is slow.
const powerL = $<HTMLInputElement>("powerL");
const speedL = $<HTMLInputElement>("speedL");
const SETTLE_L = 20; // τ
let heatL = 0; // ε, in total
let startL: { k: number; u: number } | null = null;
let burstPending = 0;
let lastSteps = 0;
const tLine: { x: number; y: number }[] = [];
const kLine: { x: number; y: number }[] = [];
const uLine: { x: number; y: number }[] = [];
const qLine: { x: number; y: number }[] = [];
const plotTL = new LinesPlot($<HTMLCanvasElement>("plotTL"), { xLabel: "heat added per molecule (ε)", yLabel: "kT", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const plotEL = new LinesPlot($<HTMLCanvasElement>("plotEL"), { xLabel: "heat added per molecule (ε)", yLabel: "change per molecule (ε)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** Fraction of molecules in the vapour: fewer than two neighbours within 1.5 σ. */
function vapourFraction(pos: Float32Array) {
  const n = pos.length / 2;
  const cell = 1.5;
  const grid = new Map<number, number[]>();
  const key = (cx: number, cy: number) => cx * 100003 + cy;
  for (let i = 0; i < n; i++) {
    const k = key(Math.floor(pos[2 * i] / cell), Math.floor(pos[2 * i + 1] / cell));
    if (!grid.has(k)) grid.set(k, []);
    grid.get(k)!.push(i);
  }
  let vap = 0;
  for (let i = 0; i < n; i++) {
    const [x, y] = [pos[2 * i], pos[2 * i + 1]];
    const [cx, cy] = [Math.floor(x / cell), Math.floor(y / cell)];
    let nb = 0;
    for (let dx = -1; dx <= 1; dx++)
      for (let dy = -1; dy <= 1; dy++)
        for (const j of grid.get(key(cx + dx, cy + dy)) ?? []) if (j !== i && (pos[2 * j] - x) ** 2 + (pos[2 * j + 1] - y) ** 2 < cell * cell) nb++;
    if (nb < 2) vap++;
  }
  return vap / n;
}

async function readoutL(sim: DiscSim) {
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  const n = ke.length;
  const k = ke.reduce((a, b) => a + b, 0);
  const u = (await sim.potential_energy()) as number;
  // The droplet starts on a lattice and relaxes for the first ~20 τ (releasing potential energy,
  // and the forces only exist from the first step): the books and the heater start after that.
  const steps = sim.steps();
  const settled = steps * sim.dt() > SETTLE_L;
  if (settled) startL ??= { k, u };
  const dtRun = (steps - lastSteps) * sim.dt();
  lastSteps = steps;
  const dq = settled ? Number(powerL.value) * 1e-4 * n * Math.max(0, dtRun) + burstPending : 0;
  if (settled) burstPending = 0;
  if (dq > 0) {
    const [w, h] = Array.from(sim.box_size());
    const scale = Math.sqrt(1 + dq / k);
    sim.kick_region(-1, -1, w + 1, h + 1, scale, 0, 0);
    heatL += k * (scale * scale - 1);
  }
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  const vap = vapourFraction(pos);
  const q = heatL / n;
  if (!startL) {
    $("tL").textContent = (k / n).toFixed(3);
    $("vapL").textContent = `${Math.round(100 * vap)}%`;
    $("heatL").textContent = "settling…";
    return;
  }
  const [dk, du] = [(k - startL.k) / n, (u - startL.u) / n];
  if (!tLine.length || q - tLine[tLine.length - 1].x > 0.005 || dq === 0) {
    tLine.push({ x: q, y: k / n });
    kLine.push({ x: q, y: dk });
    uLine.push({ x: q, y: du });
    qLine.push({ x: q, y: q });
    if (tLine.length > 4000) [tLine, kLine, uLine, qLine].forEach((l) => l.splice(0, 1));
  }
  plotTL.set([{ points: tLine, color: cssVar("--accent") }]);
  plotEL.set([
    { points: kLine, color: cssVar("--highlight") },
    { points: uLine, color: cssVar("--plot-back") },
    { points: qLine, color: cssVar("--muted"), dashed: true },
  ]);
  $("heatL").textContent = `${q.toFixed(2)} ε`;
  $("tL").textContent = (k / n).toFixed(3);
  $("vapL").textContent = `${Math.round(100 * vap)}%`;
  $("apartL").textContent = q > 0.01 ? `${Math.round((100 * du) / q)}%` : "–";
}

const hostL = new SimHost({
  canvas: $<HTMLCanvasElement>("canvasL"),
  overlay: $("overlayL"),
  scenario: "flask",
  options: () => ({ n: Number($<HTMLSelectElement>("nmolL").value), temperature: 0.36, seed: 3 }),
  names: ["molecules"],
  label: "latent heat",
  colors: ["--species-2"],
  stepsPerFrame: () => Number(speedL.value),
  afterSteps: (sim) => readoutL(sim),
  energy: { work: async () => heatL, workLabel: "total minus the heat you added" },
  onCreate: async (sim) => {
    heatL = 0;
    startL = null;
    burstPending = 0;
    lastSteps = sim.steps();
    [tLine, kLine, uLine, qLine].forEach((l) => (l.length = 0));
    sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--highlight"), ...cssColor("--cream")]));
    sim.color_by_speed(V_MAX);
    sim.smooth_speed_colors(2);
    await readoutL(sim);
  },
});
$("runL").onclick = () => {
  hostL.running = !hostL.running;
  $("runL").textContent = hostL.running ? "Pause" : "Run";
};
$("resetL").onclick = () => void hostL.create().then(() => ($("runL").textContent = "Run"));
$("nmolL").onchange = () => void hostL.create().then(() => ($("runL").textContent = "Run"));
$("burstL").onclick = () => {
  burstPending += 0.1 * (hostL.sim?.n() ?? 0);
  if (!hostL.running && hostL.sim) void hostL.exclusive((sim) => readoutL(sim)).then(() => hostL.sim?.render());
};
const showPower = () => ($("powerLv").textContent = Number(powerL.value) === 0 ? "off" : `${(Number(powerL.value) * 1e-4).toFixed(4)} ε per molecule per τ`);
powerL.oninput = showPower;
showPower();
speedL.oninput = () => ($("speedLv").textContent = `${speedL.value} steps/frame`);

declare global {
  interface Window {
    boilingTest?: { ready: Promise<void>; measure(t: number, eq: number, frames: number): Promise<{ p: number; liquid: number }>; record(): number };
    latentTest?: { heat(frames: number, steps: number): Promise<{ q: number; t: number; vap: number; apart: number }[]> };
  }
}

window.boilingTest = {
  ready: host.ready,
  async measure(t: number, eq: number, n: number) {
    await host.ready;
    // The vapour-pressure measurement uses the bulk heat bath (the page starts on the hot plate),
    // and a pot started at this temperature, so its vapour starts near saturation (evaporation
    // and condensation are too slow to change it much in a test's time; that the pre-filled
    // density is the equilibrium one is checked natively, crates/md/tests/coexistence.rs).
    $<HTMLSelectElement>("heat").value = "bath";
    tIn.value = String(t);
    await host.create();
    return host.exclusive(async (sim) => {
      sim.set_thermostat(t, 0.5);
      for (let k = 0; k < eq / 1000; k++) sim.step(1000);
      clear();
      for (let k = 0; k < n; k++) {
        sim.step(200);
        await readout(sim);
      }
      points.push({ x: 1 / t, y: Math.log(lastP) });
      redraw();
      return { p: lastP, liquid: sumL / frames };
    });
  },
  record() {
    const f = fit();
    return f ? -f.slope : NaN;
  },
};

window.latentTest = {
  /** Run the flask `frames` frames of `steps` steps with the heater as set; report every 20th. */
  async heat(frames: number, steps: number) {
    await hostL.ready;
    await hostL.create();
    return hostL.exclusive(async (sim) => {
      const out: { q: number; t: number; vap: number; apart: number }[] = [];
      for (let f = 0; f < frames; f++) {
        sim.step(steps);
        await readoutL(sim);
        if (f % 20 === 19)
          out.push({ q: Number($("heatL").textContent!.split(" ")[0]), t: Number($("tL").textContent), vap: parseInt($("vapL").textContent!), apart: parseInt($("apartL").textContent!) });
      }
      return out;
    });
  },
};
