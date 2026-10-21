import { BallSim, DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost, type HostedSim } from "../../src/lib/host";
import { Tools } from "../../src/lib/interact";
import { LinesPlot } from "../../src/lib/plot";

// Velocity space for a 2D and a 3D gas: Boltzmann's density of each state against the number
// of states at each speed or energy (the density of states).

/** What both simulations provide here. */
interface GasSim extends HostedSim {
  velocities(): Promise<unknown>;
  scale_velocities?(s: number): void;
}

interface Lab {
  dims: 2 | 3;
  /** The ring's radius and width in velocity space (σ/τ). */
  ring: { r: number; w: number };
  last: Float32Array | null;
}

function setup<S extends GasSim>(p: "A" | "B", dims: 2 | 3, host: SimHost<S>, scale: (sim: S, s: number) => void) {
  const lab: Lab = { dims, ring: { r: 1, w: 0.15 }, last: null };
  const vplane = $<HTMLCanvasElement>(`vplane${p}`);
  const eh = new LinesPlot($<HTMLCanvasElement>(`eh${p}`), { xLabel: "kinetic energy (ε)", yLabel: "share per unit energy", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
  const sh = new LinesPlot($<HTMLCanvasElement>(`sh${p}`), { xLabel: "speed (σ/τ)", yLabel: "share per unit speed", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
  /** The velocity range the plane shows (σ/τ, each way). */
  let vmax = 4;

  function draw() {
    const v = lab.last;
    if (!v) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    vplane.width = Math.round(vplane.clientWidth * dpr);
    vplane.height = Math.round(vplane.clientHeight * dpr);
    const g = vplane.getContext("2d")!;
    const [W, H] = [vplane.width, vplane.height];
    g.fillStyle = cssVar("--stage-bg");
    g.fillRect(0, 0, W, H);
    const s = W / (2 * vmax);
    const n = v.length / dims;
    g.fillStyle = cssVar("--species-2");
    const r = Math.max(1, 0.6 * dpr);
    for (let i = 0; i < n; i++) g.fillRect(W / 2 + s * v[dims * i] - r, H / 2 - s * v[dims * i + 1] - r, 2 * r, 2 * r);
    // Axes, and the ring.
    g.strokeStyle = cssVar("--muted");
    g.lineWidth = 1;
    g.beginPath();
    g.moveTo(0, H / 2);
    g.lineTo(W, H / 2);
    g.moveTo(W / 2, 0);
    g.lineTo(W / 2, H);
    g.stroke();
    g.strokeStyle = cssVar("--highlight");
    g.lineWidth = Math.max(2, s * lab.ring.w);
    g.globalAlpha = 0.45;
    g.beginPath();
    g.arc(W / 2, H / 2, s * lab.ring.r, 0, 2 * Math.PI);
    g.stroke();
    g.globalAlpha = 1;
    // The central patch of the same area, for comparison.
    const ringArea = 2 * Math.PI * lab.ring.r * lab.ring.w;
    g.strokeStyle = cssVar("--highlight");
    g.lineWidth = 1.5;
    g.beginPath();
    g.arc(W / 2, H / 2, s * Math.sqrt(ringArea / Math.PI), 0, 2 * Math.PI);
    g.stroke();
    g.fillStyle = cssVar("--muted");
    g.font = `${11 * dpr}px system-ui, sans-serif`;
    g.fillText("vx →", W - 40 * dpr, H / 2 - 6 * dpr);
    g.fillText("vy ↑", W / 2 + 6 * dpr, 14 * dpr);
  }

  async function readout(sim: S) {
    const v = (await sim.velocities()) as Float32Array;
    lab.last = v;
    const n = v.length / dims;
    const speeds = new Float32Array(n);
    let k = 0;
    for (let i = 0; i < n; i++) {
      let s2 = 0;
      for (let a = 0; a < dims; a++) s2 += v[dims * i + a] ** 2;
      speeds[i] = Math.sqrt(s2);
      k += 0.5 * s2;
    }
    const kt = k / n / (dims / 2);
    vmax = 4 * Math.sqrt(kt);
    // Histograms against the theory (m = 1).
    const binsE = 30;
    const eMax = 6 * kt;
    const he = new Array(binsE).fill(0);
    const vMaxH = 4 * Math.sqrt(kt);
    const hs = new Array(binsE).fill(0);
    for (let i = 0; i < n; i++) {
      const e = 0.5 * speeds[i] ** 2;
      if (e < eMax) he[Math.floor((e / eMax) * binsE)]++;
      if (speeds[i] < vMaxH) hs[Math.floor((speeds[i] / vMaxH) * binsE)]++;
    }
    const de = eMax / binsE;
    const dv = vMaxH / binsE;
    const eTheory = (e: number) => (dims === 2 ? Math.exp(-e / kt) / kt : (2 * Math.sqrt(e / Math.PI) / kt ** 1.5) * Math.exp(-e / kt));
    const sTheory = (u: number) => (dims === 2 ? (u / kt) * Math.exp((-u * u) / (2 * kt)) : Math.sqrt(2 / Math.PI) * (u * u / kt ** 1.5) * Math.exp((-u * u) / (2 * kt)));
    const line = (f: (x: number) => number, max: number) => Array.from({ length: 121 }, (_, j) => ({ x: (j / 120) * max, y: f((j / 120) * max) }));
    eh.set([
      { points: he.map((c, j) => ({ x: (j + 0.5) * de, y: c / (n * de) })), color: cssVar("--accent"), dots: true },
      { points: line(eTheory, eMax), color: cssVar("--muted") },
    ]);
    sh.set([
      { points: hs.map((c, j) => ({ x: (j + 0.5) * dv, y: c / (n * dv) })), color: cssVar("--species-2"), dots: true },
      { points: line(sTheory, vMaxH), color: cssVar("--muted") },
    ]);
    // The peak of the histogram, smoothed over five bins (the top of the speed curve is flat).
    const peak = (h: number[], w: number) => {
      const sm = h.map((_, j) => h.slice(Math.max(0, j - 2), j + 3).reduce((x, y) => x + y, 0) / (Math.min(h.length, j + 3) - Math.max(0, j - 2)));
      return (sm.indexOf(Math.max(...sm)) + 0.5) * w;
    };
    // Ring and centre counts, in the (vx, vy) plane (3D: of the projected velocities).
    const { r, w } = lab.ring;
    let [inRing, inCentre] = [0, 0];
    const ringArea = 2 * Math.PI * r * w;
    const rc = Math.sqrt(ringArea / Math.PI);
    for (let i = 0; i < n; i++) {
      const q = Math.hypot(v[dims * i], v[dims * i + 1]);
      if (Math.abs(q - r) < w / 2) inRing++;
      if (q < rc) inCentre++;
    }
    $(`t${p}`).textContent = kt.toFixed(3);
    $(`e${p}`).textContent = `${(k / n).toFixed(3)} = ${(k / n / kt).toFixed(2)} kT`;
    $(`me${p}`).textContent = `${peak(he, de).toFixed(2)} ε (theory ${dims === 2 ? "0" : `½kT = ${(0.5 * kt).toFixed(2)}`})`;
    $(`ms${p}`).textContent = `${peak(hs, dv).toFixed(2)} σ/τ (theory ${Math.sqrt((dims - 1) * kt).toFixed(2)})`;
    $(`dc${p}`).textContent = (inCentre / ringArea).toFixed(0);
    $(`dr${p}`).textContent = `${(inRing / ringArea).toFixed(0)}, ${inRing}`;
    draw();
  }

  // Click or drag on the plane to move the ring.
  let dragging = false;
  const toRing = (e: PointerEvent) => {
    const b = vplane.getBoundingClientRect();
    const [x, y] = [((e.clientX - b.left) / b.width - 0.5) * 2 * vmax, ((e.clientY - b.top) / b.height - 0.5) * 2 * vmax];
    lab.ring.r = Math.max(lab.ring.w / 2, Math.hypot(x, y));
    lab.ring.w = 0.06 * vmax;
    if (host.sim) void readout(host.sim);
  };
  vplane.addEventListener("pointerdown", (e) => {
    vplane.setPointerCapture(e.pointerId);
    dragging = true;
    toRing(e);
  });
  vplane.addEventListener("pointermove", (e) => dragging && toRing(e));
  vplane.addEventListener("pointerup", () => (dragging = false));

  $(`run${p}`).onclick = () => {
    host.running = !host.running;
    $(`run${p}`).textContent = host.running ? "Pause" : "Run";
  };
  const change = (s: number) => {
    if (!host.sim) return;
    scale(host.sim, s);
    if (!host.running) void host.exclusive(async (sim) => readout(sim));
  };
  $(`heat${p}`).onclick = () => change(Math.sqrt(1.2));
  $(`cool${p}`).onclick = () => change(Math.sqrt(0.8));
  return readout;
}

// ---- 1. 2D ----
let readA: (sim: DiscSim) => Promise<void> = async () => {};
let lastA = 0;
const hostA = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvasA"),
  overlay: $("overlayA"),
  scenario: "boltzmann",
  options: () => ({ n: 20000, density: 0.3, temperature: 1, seed: 2 }),
  names: ["molecules", "one marked molecule"],
  label: "2D gas",
  colors: ["--species-2", "--highlight"],
  stepsPerFrame: () => 20,
  afterSteps: async (sim) => {
    if (performance.now() - lastA < 150) return;
    lastA = performance.now();
    await readA(sim);
  },
  onCreate: (sim) => readA(sim),
});
readA = setup("A", 2, hostA, (sim, s) => {
  const [w, h] = Array.from(sim.box_size());
  sim.kick_region(-1, -1, w + 1, h + 1, s, 0, 0);
});
new Tools({ host: hostA, container: $("toolsA"), after: (sim) => readA(sim) });

// ---- 2. 3D ----
let readB: (sim: BallSim) => Promise<void> = async () => {};
let lastB = 0;
const camera = { yaw: 0.6, pitch: 0.35 };
const hostB = new SimHost<BallSim>({
  canvas: $<HTMLCanvasElement>("canvasB"),
  overlay: $("overlayB"),
  scenario: "gas3",
  create: (c, name, o) => BallSim.create(c, name, o),
  options: () => ({ n: 16000, density: 0.2, temperature: 1, seed: 3 }),
  names: ["molecules"],
  label: "3D gas",
  colors: ["--species-2"],
  stepsPerFrame: () => 20,
  afterSteps: async (sim) => {
    if (performance.now() - lastB < 150) return;
    lastB = performance.now();
    await readB(sim);
  },
  onCreate: async (sim) => {
    sim.set_camera(camera.yaw, camera.pitch, 1.3);
    sim.set_ramp_colors(new Float32Array([0.25, 0.45, 0.9, 1, 0.95, 0.3, 0.2, 1]));
    sim.color_by_speed(3.5);
    await readB(sim);
  },
});
readB = setup("B", 3, hostB, (sim, s) => sim.scale_velocities(s));
{
  const c = hostB.canvas;
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
    hostB.sim?.set_camera(camera.yaw, camera.pitch, 1.3);
  });
  c.addEventListener("pointerup", () => (drag = null));
}

declare global {
  interface Window {
    speedsTest?: { ready: Promise<void>; run(steps: number): Promise<{ a: string[]; b: string[] }> };
  }
}

window.speedsTest = {
  ready: Promise.all([hostA.ready, hostB.ready]).then(() => {}),
  /** Run both gases `steps` steps and return their readouts (average energy, peaks). */
  async run(steps: number) {
    const ids = (p: string) => ["e", "me", "ms"].map((k) => $(`${k}${p}`).textContent ?? "");
    await hostA.exclusive(async (sim) => {
      sim.step(steps);
      await readA(sim);
    });
    await hostB.exclusive(async (sim) => {
      sim.step(steps);
      await readB(sim);
    });
    return { a: ids("A"), b: ids("B") };
  },
};
