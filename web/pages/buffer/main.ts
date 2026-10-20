import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const SIDE = 32;
const AREA = SIDE * SIDE;
const COLS = 12;
const BUFFERED = 6; // rows with buffer anions (the bottom ones)
const WATER = 3;
const ANIONS = 40;
const STEP = 8; // protons added per column
const EVERY = 500; // steps between snapshots
const MIDDLE = 3; // the column the readouts compare (24 protons)
/** A proton within this distance of an anion is bound (the well's minimum is at 0.56 σ). */
const BIND = 0.8;
const epsIn = $<HTMLInputElement>("eps");

/** Sums over snapshots, per column: free protons (buffered), free anions, bound pairs, free protons (water). */
let sums: number[][] = [];
let frames = 0;
let settle = 0;

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "protons added per box",
  yLabel: "−log₁₀ [H⁺]",
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

function reset() {
  sums = Array.from({ length: COLS }, () => [0, 0, 0, 0]);
  frames = 0;
}

/** Per-column averages per box: [free H (buffer), free A, HA, free H (water)]. */
function means(): number[][] {
  return sums.map((s) => [s[0] / (frames * BUFFERED), s[1] / (frames * BUFFERED), s[2] / (frames * BUFFERED), s[3] / (frames * WATER)]);
}

/** K = [H][A]/[HA] (σ⁻²) per column, where all three are present. */
function ks(): number[] {
  return means().map(([h, a, ha]) => (h > 0 && a > 0 && ha > 0 ? (h * a) / ha / AREA : NaN));
}

/** The column's K values up to the equivalence point, and their median. */
function kMedian(): number {
  const k = ks().slice(1, ANIONS / STEP + 1).filter(Number.isFinite).sort((x, y) => x - y);
  return k.length ? k[Math.floor(k.length / 2)] : NaN;
}

/** Free protons per box predicted by mass action with constant K, for n added. */
function predicted(n: number, k: number): number {
  const kv = k * AREA;
  const b = ANIONS - n + kv;
  return (-b + Math.sqrt(b * b + 4 * kv * n)) / 2;
}

const ph = (count: number) => -Math.log10(count / AREA);

function draw() {
  const m = means();
  const k = kMedian();
  const cols = m.map((_, c) => c).filter((c) => c > 0);
  const theory = Number.isFinite(k) ? Array.from({ length: 81 }, (_, j) => ({ x: j + 1, y: ph(predicted(j + 1, k)) })).filter((p) => p.x <= (COLS - 1) * STEP) : [];
  plot.set([
    { points: theory, color: cssVar("--muted"), dashed: true },
    { points: cols.map((c) => ({ x: c * STEP, y: ph(m[c][3]) })), color: cssVar("--plot-back"), dots: true },
    { points: cols.map((c) => ({ x: c * STEP, y: ph(m[c][0]) })), color: cssVar("--accent"), dots: true },
  ]);
}

function gridLines(): number[] {
  const line = cssColor("--box-line");
  const r: number[] = [];
  const [W, H] = [COLS * SIDE, (BUFFERED + WATER) * SIDE];
  const t = 0.3;
  for (let i = 0; i <= COLS; i++) r.push(i * SIDE - t, 0, i * SIDE + t, H, ...line);
  for (let j = 0; j <= BUFFERED + WATER; j++) r.push(0, j * SIDE - t, W, j * SIDE + t, ...line);
  // A heavier line between the buffered rows and the water rows.
  r.push(0, BUFFERED * SIDE - 0.4, W, BUFFERED * SIDE + 0.4, ...cssColor("--muted"));
  return r;
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  if (settle > 0) {
    settle--;
    $("status").textContent = "Settling after the change…";
    return;
  }
  // Anions by box.
  const anions: number[][] = Array.from({ length: COLS * (BUFFERED + WATER) }, () => []);
  const boxOf = (i: number) => Math.floor(pos[2 * i + 1] / SIDE) * COLS + Math.floor(pos[2 * i] / SIDE);
  for (let i = 0; i < species.length; i++) if (species[i] === 1) anions[boxOf(i)].push(i);
  const bound = new Set<number>();
  for (let i = 0; i < species.length; i++) {
    if (species[i] !== 0) continue;
    const b = boxOf(i);
    const col = b % COLS;
    if (Math.floor(b / COLS) >= BUFFERED) {
      sums[col][3]++;
      continue;
    }
    let hit = -1;
    for (const a of anions[b]) {
      if ((pos[2 * a] - pos[2 * i]) ** 2 + (pos[2 * a + 1] - pos[2 * i + 1]) ** 2 < BIND * BIND) {
        hit = a;
        break;
      }
    }
    if (hit < 0) sums[col][0]++;
    else bound.add(hit);
  }
  for (let b = 0; b < COLS * BUFFERED; b++) {
    const ha = anions[b].filter((a) => bound.has(a)).length;
    sums[b % COLS][1] += anions[b].length - ha;
    sums[b % COLS][2] += ha;
  }
  frames++;
  const m = means();
  $("hw").textContent = m[MIDDLE][3].toFixed(1);
  $("hb").textContent = m[MIDDLE][0].toFixed(1);
  const k = kMedian();
  $("k").textContent = Number.isFinite(k) ? k.toFixed(4) : "–";
  $("frames").textContent = String(frames);
  $("status").textContent =
    frames < 10
      ? "Counting…"
      : `With ${MIDDLE * STEP} protons per box, ${Math.round((100 * (MIDDLE * STEP - m[MIDDLE][0])) / (MIDDLE * STEP))}% are held by the buffer: ${(m[MIDDLE][3] / m[MIDDLE][0]).toFixed(1)}× fewer free protons than in water.`;
  draw();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "buffer",
  options: () => ({ eps: Number(epsIn.value), columns: COLS, buffered_rows: BUFFERED, water_rows: WATER, anions: ANIONS, step: STEP, warmup: 20000, seed: 29 }),
  names: ["protons", "buffer anions"],
  colors: ["--cream", "--accent"],
  stepsPerFrame: () => EVERY,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    reset();
    sim.set_overlay(new Float32Array(gridLines()));
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
epsIn.oninput = () => ($("epsv").textContent = Number(epsIn.value).toFixed(1));
epsIn.onchange = () =>
  void host.exclusive(async (sim) => {
    sim.set_attraction(0, 1, Number(epsIn.value));
    reset();
    settle = 40;
  });

declare global {
  interface Window {
    bufferTest?: { ready: Promise<void>; run(frames: number): Promise<{ ks: number[]; free: number[][]; k: number; predicted: number[] }> };
  }
}

window.bufferTest = {
  ready: host.ready,
  async run(n: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      sim.step(20000);
      reset();
      settle = 0;
      for (let f = 0; f < n; f++) {
        sim.step(EVERY);
        await readout(sim);
        if (f % 10 === 0) sim.render();
      }
      const k = kMedian();
      return { ks: ks(), free: means(), k, predicted: means().map((_, c) => predicted(c * STEP, k)) };
    });
  },
};
