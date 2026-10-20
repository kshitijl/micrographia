import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import type { DiscSim } from "../../src/lib/wasm";

const uIn = $<HTMLInputElement>("u");
const thermal = $<HTMLInputElement>("thermal");
let width = 1;
let path: { a: number; t: number }[] = [];
let start: { a: number; t: number } | null = null;
let heat0 = 0;
let lastT = NaN;
let lastA = NaN;

const speed = () => Number(uIn.value);

function drawPlot() {
  const c = $<HTMLCanvasElement>("plot");
  const dpr = window.devicePixelRatio || 1;
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const W = c.clientWidth;
  const H = c.clientHeight;
  g.clearRect(0, 0, W, H);
  if (!start || path.length < 2) return;
  const pad = { l: 36, r: 8, t: 14, b: 22 };
  const aMax = Math.max(...path.map((p) => p.a), start.a) * 1.05;
  const aMin = Math.min(...path.map((p) => p.a)) * 0.9;
  const tMax = Math.max(...path.map((p) => p.t), start.t * 1.2) * 1.1;
  const x = (a: number) => pad.l + ((a - aMin) / (aMax - aMin)) * (W - pad.l - pad.r);
  const y = (t: number) => H - pad.b - (t / tMax) * (H - pad.t - pad.b);
  g.strokeStyle = cssVar("--rule");
  g.beginPath();
  g.moveTo(pad.l, pad.t);
  g.lineTo(pad.l, H - pad.b);
  g.lineTo(W - pad.r, H - pad.b);
  g.stroke();
  // Adiabat through the start: T = T0 A0 / A.
  g.strokeStyle = cssVar("--plot-back");
  g.lineWidth = 2;
  g.beginPath();
  for (let k = 0; k <= 60; k++) {
    const a = aMin + ((aMax - aMin) * k) / 60;
    const t = (start.t * start.a) / a;
    if (k === 0) g.moveTo(x(a), y(Math.min(t, tMax)));
    else g.lineTo(x(a), y(Math.min(t, tMax)));
  }
  g.stroke();
  g.strokeStyle = cssVar("--accent");
  g.beginPath();
  path.forEach((p, k) => (k === 0 ? g.moveTo(x(p.a), y(p.t)) : g.lineTo(x(p.a), y(p.t))));
  g.stroke();
  g.lineWidth = 1;
  g.fillStyle = cssVar("--muted");
  g.font = "11px system-ui, sans-serif";
  g.fillText("temperature", pad.l + 4, 10);
  g.textAlign = "right";
  g.fillText("area (σ²)", W - pad.r, H - 4);
}

async function readout(sim: DiscSim) {
  const [h, , , workByGas] = Array.from((await sim.piston()) as Float64Array);
  const r = (await sim.observe()) as { temperature: number };
  const l = (await sim.ledger(1)) as { energy: Float64Array };
  const heat = l.energy[0] + l.energy[1] + l.energy[2] - heat0;
  const a = width * (h - 1);
  lastT = r.temperature;
  lastA = a;
  start ??= { a, t: r.temperature };
  if (!path.length || Math.abs(path[path.length - 1].a - a) > 1 || path.length % 20 === 0) path.push({ a, t: r.temperature });
  if (path.length > 2000) path.shift();
  $("temp").textContent = r.temperature.toFixed(3);
  $("area").textContent = a.toFixed(0);
  $("work").textContent = `${(-workByGas).toFixed(0)} ε`;
  $("heat").textContent = `${heat.toFixed(0)} ε`;
  const c = cssColor("--accent");
  sim.set_overlay(new Float32Array([0, h, width, h + 2.5, c[0], c[1], c[2], 1]));
  drawPlot();
}

const host = new SimHost({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "compress",
  options: () => ({ n: 3000, density: 0.05, thermal: thermal.checked ? 1 : 0, seed: 9 }),
  colors: ["--cream"],
  stepsPerFrame: () => 40,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    width = sim.box_size()[0];
    path = [];
    start = null;
    heat0 = 0;
    sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3")]));
    sim.color_by_speed(3);
    await readout(sim);
  },
});

function move(sign: number) {
  if (!host.sim) return;
  host.sim.set_piston_speed(sign * speed());
  host.running = true;
}

$("down").onclick = () => move(-1);
$("up").onclick = () => move(1);
$("stop").onclick = () => host.sim?.set_piston_speed(0);
$("reset").onclick = () => void host.create().then(() => (host.running = true));
uIn.oninput = () => ($("uv").textContent = `${speed().toFixed(2)} σ/τ`);
thermal.onchange = () => {
  host.sim?.set_walls_thermal(thermal.checked ? 1 : 0);
  start = null;
  path = [];
};
void host.ready.then(() => (host.running = true));

declare global {
  interface Window {
    compressTest?: { ready: Promise<void>; run(u: number, steps: number): Promise<{ t0: number; a0: number; t1: number; a1: number }> };
  }
}

window.compressTest = {
  ready: host.ready,
  async run(u: number, steps: number) {
    await host.ready;
    return host.exclusive(async (sim) => {
      await readout(sim);
      const t0 = lastT;
      const a0 = lastA;
      sim.set_piston_speed(u);
      for (let k = 0; k < steps / 1000; k++) sim.step(1000);
      sim.set_piston_speed(0);
      sim.step(200);
      await readout(sim);
      return { t0, a0, t1: lastT, a1: lastA };
    });
  },
};
