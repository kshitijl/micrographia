import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import type { DiscSim } from "../../src/lib/wasm";

const draughtIn = $<HTMLInputElement>("draught");
let width = 300;
let x0: Float64Array | null = null; // unwrapped positions
let prev: Float32Array | null = null;
let startMean = 0;
let startVar = 0;
let series: { t: number; sd: number; drift: number }[] = [];
let lastD = NaN;

const plot = new LinesPlot($<HTMLCanvasElement>("plot"), {
  xLabel: "time (τ)",
  yLabel: "σ",
  yMin: 0,
  ink: () => cssVar("--muted"),
  rule: () => cssVar("--rule"),
});

async function readout(sim: DiscSim) {
  const pos = (await sim.positions(1)) as Float32Array;
  const n = pos.length / 2;
  if (!x0 || !prev) {
    x0 = Float64Array.from({ length: n }, (_, i) => pos[2 * i]);
    prev = pos.slice();
  } else {
    for (let i = 0; i < n; i++) {
      let d = pos[2 * i] - prev[2 * i];
      if (d > width / 2) d -= width;
      if (d < -width / 2) d += width;
      x0[i] += d;
    }
    prev = pos.slice();
  }
  const mean = x0.reduce((a, b) => a + b, 0) / n;
  const variance = x0.reduce((a, b) => a + (b - mean) ** 2, 0) / n;
  const t = host.elapsed * sim.dt();
  if (!series.length) {
    startMean = mean;
    startVar = variance;
  }
  series.push({ t, sd: Math.sqrt(variance), drift: mean - startMean });
  if (t > 20) lastD = (variance - startVar) / (2 * t);
  $("time").textContent = `${t.toFixed(0)} τ`;
  $("spread").textContent = `${Math.sqrt(variance).toFixed(1)} σ`;
  $("drift").textContent = `${(mean - startMean).toFixed(1)} σ`;
  $("d").textContent = Number.isFinite(lastD) ? `${lastD.toFixed(2)} σ²/τ` : "–";
  plot.set([
    { points: series.map((s) => ({ x: s.t, y: s.sd })), color: cssVar("--accent") },
    { points: series.map((s) => ({ x: s.t, y: s.drift })), color: cssVar("--plot-back") },
  ]);
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "smell",
  options: () => ({ n: 2400, width: 300, height: 40, blob: 10, draught: Number(draughtIn.value), seed: 16 }),
  colors: ["--species-2", "--cream"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    width = sim.box_size()[0];
    x0 = null;
    prev = null;
    series = [];
    lastD = NaN;
    sim.set_view(0, 0, 120, 40);
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("reset").onclick = () => {
  const run = host.running;
  void host.create().then(() => (host.running = run));
};
draughtIn.oninput = () => ($("dv").textContent = Number(draughtIn.value) === 0 ? "off" : `push ${Number(draughtIn.value).toFixed(4)}`);
draughtIn.onchange = () => host.sim?.set_gravity(Number(draughtIn.value), 0);

declare global {
  interface Window {
    smellTest?: { ready: Promise<void>; run(frames: number, draught: number): Promise<{ d: number; drift: number; sd: number; t: number }> };
  }
}

window.smellTest = {
  ready: host.ready,
  async run(frames: number, draught: number) {
    await host.ready;
    draughtIn.value = String(draught);
    await host.create();
    return host.exclusive(async (sim) => {
      for (let k = 0; k < frames; k++) {
        sim.step(100);
        host.elapsed += 100;
        await readout(sim);
      }
      const last = series[series.length - 1];
      return { d: lastD, drift: last.drift, sd: last.sd, t: last.t };
    });
  },
};
