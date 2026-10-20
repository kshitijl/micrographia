import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot, LinesPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const SIDE = 16;
const COLS = 16;
const ROWS = 4; // per length
const LENGTHS = [1, 4, 8];
const EVERY = 1000;
/** A bead is paired when a partner bead is within this distance. */
const PAIR = 1.5;
// Hot to cold: strands find each other faster when they also come apart easily.
const SWEEP = [1.3, 1.2, 1.1, 1.0, 0.9, 0.8, 0.7, 0.6, 0.5];
const tempIn = $<HTMLInputElement>("temp");
const temperature = () => Number(tempIn.value);

/** Per temperature: snapshots, and per length the paired box count; and the length-8 histogram. */
const data = new Map<number, { frames: number; paired: number[]; hist: number[] }>();
let settle = 0;
let sweeping = false;

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "temperature kT/ε",
  yLabel: "share paired",
  yMin: 0,
  yMax: 1,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});
const colors = () => [cssVar("--muted"), cssVar("--plot-back"), cssVar("--accent")];

// Readouts and legend, one per strand length.
$("readouts").innerHTML =
  LENGTHS.map((l) => `<div class="readout"><div class="k">paired, ${l === 1 ? "single beads" : `${l}-bead strands`}</div><div class="v" id="p${l}">–</div></div>`).join("") +
  `<div class="readout"><div class="k">melting temperatures</div><div class="v" id="tm">–</div></div>`;
function legend() {
  const c = colors();
  $("plotlegend").innerHTML = LENGTHS.map((l, k) => `<span><i style="background: ${c[k]}"></i>${l === 1 ? "single" : `${l} beads`}</span>`).join("");
}
legend();

const hist = new BarPlot($<HTMLCanvasElement>("hist"), {
  labels: [],
  yLabel: "8-bead strands",
  xLabel: "beads paired",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

/** Share paired per length at each measured temperature, sorted by temperature. */
function curves(): { t: number; share: number[] }[] {
  return [...data]
    .filter(([, d]) => d.frames > 0)
    .map(([t, d]) => ({ t, share: d.paired.map((p) => p / (d.frames * COLS * ROWS)) }))
    .sort((a, b) => a.t - b.t);
}

/** Where the curve for length index k crosses one half (linear interpolation), or NaN. */
function meltingT(k: number): number {
  const c = curves();
  for (let j = 0; j + 1 < c.length; j++) {
    const [a, b] = [c[j].share[k], c[j + 1].share[k]];
    if (a >= 0.5 && b < 0.5) return c[j].t + ((a - 0.5) / (a - b)) * (c[j + 1].t - c[j].t);
  }
  return NaN;
}

function draw() {
  const c = curves();
  const col = colors();
  plot.set(LENGTHS.map((_, k) => ({ points: c.map((p) => ({ x: p.t, y: p.share[k] })), color: col[k], dots: c.length < 3 })));
  const h = data.get(temperature())?.hist;
  if (h) {
    const total = h.reduce((x, y) => x + y, 0) || 1;
    hist.set(
      h.map((x) => x / total),
      null,
      h.map((_, k) => String(k)),
    );
  }
  const tm = LENGTHS.slice(1).map((_, k) => meltingT(k + 1));
  $("tm").textContent = tm.every(Number.isFinite) ? tm.map((t) => t.toFixed(2)).join(", ") : "–";
}

function gridLines(): number[] {
  const line = cssColor("--box-line");
  const r: number[] = [];
  const [W, H] = [COLS * SIDE, LENGTHS.length * ROWS * SIDE];
  const t = 0.12;
  for (let i = 0; i <= COLS; i++) r.push(i * SIDE - t, 0, i * SIDE + t, H, ...line);
  for (let j = 0; j <= LENGTHS.length * ROWS; j++) r.push(0, j * SIDE - t, W, j * SIDE + t, ...line);
  return r;
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const nbox = COLS * ROWS * LENGTHS.length;
  const beads: number[][][] = Array.from({ length: nbox }, () => [[], []]);
  for (let i = 0; i < species.length; i++) beads[Math.floor(pos[2 * i + 1] / SIDE) * COLS + Math.floor(pos[2 * i] / SIDE)][species[i]].push(i);
  const t = temperature();
  const d = data.get(t) ?? { frames: 0, paired: LENGTHS.map(() => 0), hist: new Array(9).fill(0) };
  const lit: number[] = gridLines();
  const hit = cssColor("--box-hit");
  beads.forEach(([a, b], box) => {
    const close = a.filter((i) => b.some((j) => (pos[2 * i] - pos[2 * j]) ** 2 + (pos[2 * i + 1] - pos[2 * j + 1]) ** 2 < PAIR * PAIR)).length;
    const band = Math.floor(box / (COLS * ROWS));
    const paired = 2 * close > a.length;
    if (settle === 0) {
      if (paired) d.paired[band]++;
      if (band === LENGTHS.length - 1) d.hist[close]++;
    }
    if (paired) {
      const [x, y] = [(box % COLS) * SIDE, Math.floor(box / COLS) * SIDE];
      lit.push(x + 0.3, y + 0.3, x + SIDE - 0.3, y + SIDE - 0.3, hit[0], hit[1], hit[2], 0.25);
    }
  });
  sim.set_overlay(new Float32Array(lit));
  if (settle > 0) {
    settle--;
    $("status").textContent = `Settling at kT = ${t.toFixed(2)} ε…`;
    return;
  }
  d.frames++;
  data.set(t, d);
  LENGTHS.forEach((l, k) => ($(`p${l}`).textContent = `${Math.round((100 * d.paired[k]) / (d.frames * COLS * ROWS))}%`));
  draw();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "dna",
  options: () => ({ columns: COLS, rows: ROWS, lengths: Number(LENGTHS.join("")), temperature: temperature(), eps: 1.5, seed: 37 }),
  names: ["A beads", "B beads"],
  colors: ["--accent", "--species-2"],
  stepsPerFrame: () => EVERY,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    data.clear();
    settle = 20;
    sim.set_overlay(new Float32Array(gridLines()));
    $("tv").textContent = temperature().toFixed(2);
  },
});

function setTemperature(sim: DiscSim, t: number) {
  tempIn.value = String(t);
  $("tv").textContent = t.toFixed(2);
  sim.set_thermostat(t, 0.5);
}

async function sweep(frames = 50, equil = 20000) {
  sweeping = true;
  host.running = false;
  $("run").textContent = "Run";
  await host.exclusive(async (sim) => {
    for (const t of SWEEP) {
      setTemperature(sim, t);
      data.delete(t);
      $("status").textContent = `Settling, then counting at kT = ${t.toFixed(2)} ε…`;
      sim.step(equil);
      settle = 0;
      for (let f = 0; f < frames; f++) {
        sim.step(EVERY);
        await readout(sim);
        if (f % 5 === 0) sim.render();
      }
    }
  });
  const tm = LENGTHS.slice(1).map((_, k) => meltingT(k + 1));
  $("status").textContent = `Single beads hardly ever pair. Strands melt at kT ≈ ${tm.map((t) => t.toFixed(2)).join(" (4 beads) and ")} (8 beads): longer strands hold to higher temperatures.`;
  sweeping = false;
  return { curves: curves(), tm, hist: data.get(0.9)?.hist ?? [] };
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("sweep").onclick = () => {
  if (!sweeping) void sweep();
};
tempIn.oninput = () => ($("tv").textContent = temperature().toFixed(2));
tempIn.onchange = () =>
  void host.exclusive(async (sim) => {
    setTemperature(sim, temperature());
    settle = 20;
  });

declare global {
  interface Window {
    dnaTest?: { ready: Promise<void>; sweep(frames: number): Promise<{ curves: { t: number; share: number[] }[]; tm: number[]; hist: number[] }> };
  }
}

window.dnaTest = {
  ready: host.ready,
  async sweep(frames: number) {
    await host.ready;
    return sweep(frames);
  },
};
