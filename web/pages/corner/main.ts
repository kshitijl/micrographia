import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const BOXES = [32, 24];
const SIDE = 8; // σ per box
const nInput = $<HTMLInputElement>("n");
const mode = $<HTMLSelectElement>("mode");
const sizeSel = $<HTMLSelectElement>("size");
const size = () => Number(sizeSel.value);

/**
 * The area one molecule denies another: ∫ (1 − e^(−U(r)/kT)) 2πr dr for the WCA repulsion of
 * diameter d at kT = 1 (the boxes stay near their starting temperature).
 */
function excludedArea(d: number): number {
  const rc = d * 2 ** (1 / 6);
  const steps = 4000;
  let b = 0;
  for (let k = 0; k < steps; k++) {
    const r = ((k + 0.5) / steps) * rc;
    const s6 = (d / r) ** 6;
    const u = 4 * s6 * (s6 - 1) + 1;
    b += (1 - Math.exp(-u)) * 2 * Math.PI * r * (rc / steps);
  }
  return b;
}

let samples = 0;
let events = 0;
let hist: number[] = [];

const bars = new BarPlot($<HTMLCanvasElement>("hist"), {
  labels: [],
  yLabel: "share of boxes",
  xLabel: "molecules in the left half",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

const perBox = () => Number(nInput.value);

function binomial(n: number): number[] {
  const out: number[] = [];
  let c = 1;
  for (let k = 0; k <= n; k++) {
    out.push(c / 2 ** n);
    c = (c * (n - k)) / (k + 1);
  }
  return out;
}

function predicted(): number {
  return mode.value === "half" ? 2 ** -perBox() : 4 ** -perBox();
}

/** With the molecules' size, to second order: × e^(−N(N−1) b (1/f − 1) / 2A) for a region f. */
function corrected(): number {
  const n = perBox();
  const f = mode.value === "half" ? 1 / 2 : 1 / 4;
  return predicted() * Math.exp((-n * (n - 1) * excludedArea(size()) * (1 / f - 1)) / (2 * SIDE * SIDE));
}

function fmtShare(x: number): string {
  if (x === 0) return "0";
  if (x >= 0.01) return `${(100 * x).toFixed(1)}%`;
  return `1 in ${Math.round(1 / x).toLocaleString()}`;
}

function resetStats() {
  samples = 0;
  events = 0;
  hist = new Array(perBox() + 1).fill(0);
  $("pred").textContent = fmtShare(predicted());
  $("pred2").textContent = fmtShare(corrected());
  $("meas").textContent = "–";
  $("samples").textContent = "0";
  $("events").textContent = "0";
}

function gridLines(): number[] {
  const line = cssColor("--box-line");
  const r: number[] = [];
  const [W, H] = [BOXES[0] * SIDE, BOXES[1] * SIDE];
  const t = 0.12;
  for (let i = 0; i <= BOXES[0]; i++) r.push(i * SIDE - t, 0, i * SIDE + t, H, ...line);
  for (let j = 0; j <= BOXES[1]; j++) r.push(0, j * SIDE - t, W, j * SIDE + t, ...line);
  return r;
}

async function readout(sim: DiscSim) {
  const h = (await sim.histogram()) as { all: Uint32Array };
  const gx = 2 * BOXES[0];
  const n = perBox();
  const lit: number[] = [];
  const hit = cssColor("--box-hit");
  for (let j = 0; j < BOXES[1]; j++) {
    for (let i = 0; i < BOXES[0]; i++) {
      const q = (x: number, y: number) => h.all[(2 * j + y) * gx + 2 * i + x];
      const left = q(0, 0) + q(0, 1);
      hist[left] = (hist[left] ?? 0) + 1;
      const all = mode.value === "half" ? left === n : q(0, 0) === n;
      if (all) {
        events++;
        lit.push(i * SIDE, j * SIDE, (i + 0.5) * SIDE, (j + (mode.value === "half" ? 1 : 0.5)) * SIDE, ...hit);
      }
    }
  }
  samples += BOXES[0] * BOXES[1];
  sim.set_overlay(new Float32Array([...gridLines(), ...lit]));
  const share = events / samples;
  $("meas").textContent = fmtShare(share);
  $("samples").textContent = samples.toLocaleString();
  $("events").textContent = events.toLocaleString();
  const total = hist.reduce((a, b) => a + b, 0);
  bars.set(
    hist.map((c) => c / total),
    binomial(n),
    hist.map((_, k) => String(k)),
  );
  const ratio = share / predicted();
  const ratio2 = share / corrected();
  $("status").textContent =
    events < 5
      ? "Counting…"
      : `Measured share is ${ratio.toFixed(2)}× the ideal-gas odds ${mode.value === "half" ? "2^−N" : "4^−N"}, and ${ratio2.toFixed(2)}× the odds counting the molecules' size. Add a molecule and the odds ${mode.value === "half" ? "halve" : "quarter"}.`;
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "corner",
  options: () => ({ per_box: perBox(), boxes_x: BOXES[0], boxes_y: BOXES[1], size: size(), seed: 3 }),
  colors: ["--cream"],
  // About one box-crossing time between snapshots, so they are roughly independent.
  stepsPerFrame: () => 200,
  afterSteps: (sim) => readout(sim),
  onCreate: (sim) => {
    resetStats();
    sim.set_overlay(new Float32Array(gridLines()));
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("reset").onclick = () => void host.create();
nInput.oninput = () => ($("nv").textContent = nInput.value);
nInput.onchange = () => void host.create().then(() => host.sim && (host.running = $("run").textContent === "Pause"));
mode.onchange = () => resetStats();
sizeSel.onchange = () => void host.create().then(() => host.sim && (host.running = $("run").textContent === "Pause"));
$("nboxes").textContent = String(BOXES[0] * BOXES[1]);

declare global {
  interface Window {
    cornerTest?: {
      ready: Promise<void>;
      /** Optionally recreate with a molecule size and count per box first. */
      run(frames: number, size?: number, n?: number): Promise<{ samples: number; events: number; predicted: number; corrected: number }>;
    };
  }
}

window.cornerTest = {
  ready: host.ready,
  async run(frames: number, s?: number, n?: number) {
    await host.ready;
    if (s !== undefined || n !== undefined) {
      if (s !== undefined) sizeSel.value = String(s);
      if (n !== undefined) nInput.value = String(n);
      await host.create();
    }
    await host.exclusive(async (sim) => {
      for (let f = 0; f < frames; f++) {
        sim.step(200);
        await readout(sim);
      }
    });
    return { samples, events, predicted: predicted(), corrected: corrected() };
  },
};
