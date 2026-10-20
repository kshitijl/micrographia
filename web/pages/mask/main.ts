import { $, cssVar, SimHost } from "../../src/lib/host";
import { BarPlot } from "../../src/lib/plot";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const W = 128;
const H = 64;
const FIBER = 2;
const GAP = 8;
const ROWS = 2;
const EVERY = 1000; // steps between snapshots
const SIZES = [0.3, 1.2, 2.4, 4.8];
const TRACED = 4;
const sizeSel = $<HTMLSelectElement>("size");

const size = () => Number(sizeSel.value);
const aerosols = (s: number) => (s > 3 ? 30 : s > 2 ? 60 : 120);

/** Per size: captures and clean passes. */
const results = new Map<number, { caps: number; passes: number }>();
let fibers: [number, number][] = [];
let aerosolIds: number[] = [];
let stuck: number[] = [];
let captured: boolean[] = [];
let upstream: boolean[] = [];
let trails: [number, number][][] = [];
let sweeping = false;

const bars = new BarPlot($<HTMLCanvasElement>("plot"), {
  labels: [],
  yLabel: "caught per pass",
  xLabel: "size (σ)",
  color: () => cssVar("--accent"),
  theoryColor: () => cssVar("--plot-back"),
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

function drawBars() {
  bars.set(
    SIZES.map((s) => {
      const r = results.get(s);
      return r && r.caps + r.passes > 0 ? r.caps / (r.caps + r.passes) : 0;
    }),
    null,
    SIZES.map(String),
  );
}

function filterEdges(s: number): [number, number] {
  const pitch = 2 * FIBER + GAP;
  return [W / 2 - FIBER - s, W / 2 + (ROWS - 1) * pitch * 0.866 + FIBER + s];
}

function overlay(): number[] {
  const r: number[] = [0, 0, W / 8, H, ...cssColor("--box-hit").slice(0, 3), 0.12];
  const ink = cssColor("--cream");
  const d = 0.4;
  for (const t of trails) for (const [x, y] of t) r.push(x - d, y - d, x + d, y + d, ink[0], ink[1], ink[2], 0.85);
  return r;
}

async function readout(sim: DiscSim) {
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  const s = size();
  if (!fibers.length) {
    for (let i = 0; i < species.length; i++) {
      if (species[i] === 1) fibers.push([pos[2 * i], pos[2 * i + 1]]);
      if (species[i] === 2) aerosolIds.push(i);
    }
    stuck = aerosolIds.map(() => 0);
    captured = aerosolIds.map(() => false);
    upstream = aerosolIds.map(() => true);
    trails = Array.from({ length: TRACED }, () => []);
  }
  const r = results.get(s) ?? { caps: 0, passes: 0 };
  results.set(s, r);
  const [f0, f1] = filterEdges(s);
  const touch = 1.3 * (FIBER + s / 2);
  aerosolIds.forEach((i, k) => {
    const x = pos[2 * i];
    const y = pos[2 * i + 1];
    if (k < TRACED) {
      const t = trails[k];
      // Break the trail where the particle wraps round the periodic channel.
      if (t.length && Math.abs(t[t.length - 1][0] - x) > W / 2) t.length = 0;
      t.push([x, y]);
      if (t.length > 80) t.shift();
    }
    if (captured[k]) return;
    const on = fibers.some(([fx, fy]) => (fx - x) ** 2 + (fy - y) ** 2 < touch * touch);
    stuck[k] = on ? stuck[k] + 1 : 0;
    if (stuck[k] >= 5) {
      captured[k] = true;
      r.caps++;
      return;
    }
    if (x < f0) upstream[k] = true;
    else if (x >= f1 && upstream[k]) {
      r.passes++;
      upstream[k] = false;
    }
  });
  sim.set_overlay(new Float32Array(overlay()));
  $("caught").textContent = String(r.caps);
  $("passed").textContent = String(r.passes);
  const n = r.caps + r.passes;
  $("eff").textContent = n > 0 ? `${Math.round((100 * r.caps) / n)}%` : "–";
  drawBars();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "mask",
  options: () => ({ width: W, height: H, fiber: FIBER, gap: GAP, rows: ROWS, size: size(), aerosols: aerosols(size()), fan: 1, seed: 31 }),
  colors: ["--species-2", "--accent", "--cream"],
  stepsPerFrame: () => 250,
  // Count every fourth frame (one snapshot per EVERY steps).
  afterSteps: (() => {
    let f = 0;
    return (sim: DiscSim) => (++f % (EVERY / 250) === 0 ? readout(sim) : undefined);
  })(),
  onCreate: () => {
    fibers = [];
    aerosolIds = [];
    results.delete(size());
  },
});

async function sweep(frames = 150) {
  sweeping = true;
  host.running = false;
  $("run").textContent = "Run";
  for (const s of SIZES) {
    sizeSel.value = String(s);
    $("status").textContent = `Measuring ${s} σ particles…`;
    await host.create();
    await host.exclusive(async (sim) => {
      for (let f = 0; f < frames; f++) {
        sim.step(EVERY);
        await readout(sim);
        if (f % 3 === 0) sim.render();
      }
    });
  }
  const eff = SIZES.map((s) => {
    const r = results.get(s)!;
    return r.caps / Math.max(1, r.caps + r.passes);
  });
  $("status").textContent = `Caught per pass: ${SIZES.map((s, k) => `${s} σ ${Math.round(100 * eff[k])}%`).join(", ")}. A sieve with these gaps would catch none of them.`;
  sweeping = false;
  return eff;
}

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("sweep").onclick = () => {
  if (!sweeping) void sweep();
};
sizeSel.onchange = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};

declare global {
  interface Window {
    maskTest?: { ready: Promise<void>; sweep(frames: number): Promise<number[]> };
  }
}

window.maskTest = {
  ready: host.ready,
  async sweep(frames: number) {
    await host.ready;
    return sweep(frames);
  },
};
