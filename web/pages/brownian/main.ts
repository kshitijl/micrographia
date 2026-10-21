import { BallSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// A Brownian grain in 3D (scenario brownian3: a sphere 5 σ across, as dense as the WCA liquid
// around it). Measured natively (crates/md/tests/brownian3.rs): D 0.0169 over 20 τ; a throw's
// ∫ v/v₀ dt over 4 τ is 0.77 τ, so ζ = m/0.77 = 59; Dζ = 1.0 = kT.

/** The grain's id. */
const GRAIN = 0;
/** The window for D, and for the throws. */
const LAG = 20;
const THROW_SAMPLES = 40;
const THROW_EVERY = 20; // steps (0.1 τ)
const DTS = [0.25, 0.5, 1, 2, 4, 8];

const dtIn = $<HTMLInputElement>("dt");
const liquid = $<HTMLInputElement>("liquid");
const msdPlot = new LinesPlot($<HTMLCanvasElement>("msd"), { xLabel: "time between looks (τ)", yLabel: "⟨r²⟩ (σ²)", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const decayPlot = new LinesPlot($<HTMLCanvasElement>("decay"), { xLabel: "time after the throw (τ)", yLabel: "v / v₀", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const scope = $<HTMLCanvasElement>("scope");
const camera = { yaw: 0.5, pitch: 0.3, distance: 0.9 };

/** The grain's path, unwrapped: (time τ, x, y, z). */
let path: number[][] = [];
let last: number[] | null = null;
let acc = [0, 0, 0];
/** Throws so far, and the sum of their v/v₀ curves. */
let throws = 0;
const decaySum = new Float64Array(THROW_SAMPLES);
let mass = 1;
let box = [1, 1, 1];

function msdAt(lag: number) {
  // Pairs of looks about `lag` apart.
  let [s, k] = [0, 0];
  let j = 0;
  for (let i = 0; i < path.length; i++) {
    while (j < path.length && path[j][0] < path[i][0] + lag) j++;
    if (j >= path.length) break;
    s += (path[j][1] - path[i][1]) ** 2 + (path[j][2] - path[i][2]) ** 2 + (path[j][3] - path[i][3]) ** 2;
    k++;
  }
  return k > 5 ? s / k : NaN;
}

function drawScope() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  scope.width = Math.round(scope.clientWidth * dpr);
  scope.height = Math.round(scope.clientHeight * dpr);
  const g = scope.getContext("2d")!;
  const [W, H] = [scope.width, scope.height];
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, W, H);
  if (!path.length) return;
  // Dots every Δt, the latest at the centre, a field 16 σ across.
  const dt = DTS[Number(dtIn.value)];
  const s = W / 16;
  const [cx, cy] = [path[path.length - 1][1], path[path.length - 1][2]];
  const pts: number[][] = [];
  let next = path[0][0];
  for (const p of path) if (p[0] >= next) {
    pts.push(p);
    next = p[0] + dt;
  }
  g.strokeStyle = cssVar("--cream");
  g.globalAlpha = 0.8;
  g.lineWidth = dpr;
  g.beginPath();
  pts.forEach((p, k) => {
    const [x, y] = [W / 2 + s * (p[1] - cx), H / 2 - s * (p[2] - cy)];
    if (k === 0) g.moveTo(x, y);
    else g.lineTo(x, y);
  });
  g.stroke();
  g.fillStyle = cssVar("--species-3");
  for (const p of pts) g.fillRect(W / 2 + s * (p[1] - cx) - 1.5 * dpr, H / 2 - s * (p[2] - cy) - 1.5 * dpr, 3 * dpr, 3 * dpr);
  g.globalAlpha = 1;
  // A scale bar: 1 σ.
  g.fillStyle = cssVar("--muted");
  g.fillRect(10 * dpr, H - 14 * dpr, s, 2 * dpr);
  g.font = `${11 * dpr}px system-ui, sans-serif`;
  g.fillText("1 σ", 10 * dpr, H - 20 * dpr);
}

function show(sim: BallSim, pos: Float32Array) {
  // The grain in red; the liquid only near it, unless asked.
  const n = pos.length / 3;
  const v = new Float32Array(n);
  const g = [pos[3 * GRAIN], pos[3 * GRAIN + 1], pos[3 * GRAIN + 2]];
  const r2 = 5.2 ** 2;
  for (let i = 0; i < n; i++) {
    if (i === GRAIN) {
      v[i] = 1;
      continue;
    }
    let d2 = 0;
    for (let a = 0; a < 3; a++) {
      let d = Math.abs(pos[3 * i + a] - g[a]);
      d = Math.min(d, box[a] - d);
      d2 += d * d;
    }
    v[i] = liquid.checked || d2 < r2 ? 0.5 : -1;
  }
  sim.color_by_values(v);
}

/** Add the grain's position to its path (unwrapped). */
function track(sim: BallSim, q: number[]) {
  if (last) for (let a = 0; a < 3; a++) {
    let d = q[a] - last[a];
    d -= box[a] * Math.round(d / box[a]);
    acc[a] += d;
  }
  last = q;
  path.push([sim.steps() * sim.dt(), ...acc]);
  if (path.length > 20000) path.splice(0, 1);
}

async function readout(sim: BallSim) {
  const { pos } = (await sim.all_positions()) as { pos: Float32Array };
  track(sim, [pos[3 * GRAIN], pos[3 * GRAIN + 1], pos[3 * GRAIN + 2]]);
  show(sim, pos);
  drawScope();
  const lags = [1, 2, 5, 10, 20, 40];
  msdPlot.set([{ points: lags.map((l) => ({ x: l, y: msdAt(l) })).filter((p) => Number.isFinite(p.y)), color: cssVar("--accent"), dots: true }]);
  const m = msdAt(LAG);
  const d = m / (6 * LAG);
  $("d").textContent = Number.isFinite(d) ? `${d.toFixed(4)} σ²/τ` : "watching…";
  const { ke } = (await sim.kinetic_energies()) as { ke: Float32Array };
  let k = 0;
  for (let i = 0; i < ke.length; i++) if (i !== GRAIN) k += ke[i];
  $("tm").textContent = ((2 / 3) * (k / (ke.length - 1))).toFixed(3);
  const zeta = friction();
  $("zeta").textContent = Number.isFinite(zeta) ? `${zeta.toFixed(0)} (from ${throws} throws)` : "throw it";
  $("kt").textContent = Number.isFinite(zeta) && Number.isFinite(d) ? (d * zeta).toFixed(2) : "–";
}

/** ζ = m / ∫ v/v₀ dt over the throws' window. */
function friction() {
  if (!throws) return NaN;
  let s = 0;
  for (let k = 0; k < THROW_SAMPLES; k++) s += decaySum[k] / throws;
  return mass / (s * THROW_EVERY * 0.005);
}

async function throwGrain(sim: BallSim) {
  // A random direction; 1 σ/τ.
  const u = [Math.random() - 0.5, Math.random() - 0.5, Math.random() - 0.5];
  const l = Math.hypot(...u);
  const dir = u.map((x) => x / l);
  sim.set_species_velocity(1, dir[0], dir[1], dir[2]);
  for (let k = 0; k < THROW_SAMPLES; k++) {
    sim.step(THROW_EVERY);
    const v = (await sim.velocities()) as Float32Array;
    decaySum[k] += v[3 * GRAIN] * dir[0] + v[3 * GRAIN + 1] * dir[1] + v[3 * GRAIN + 2] * dir[2];
  }
  throws++;
  decayPlot.set([{ points: Array.from(decaySum, (v, k) => ({ x: (k + 1) * THROW_EVERY * 0.005, y: v / throws })), color: cssVar("--species-3") }]);
  // The throw disturbed the path: start it again from here.
  last = null;
}

const host = new SimHost<BallSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "brownian3",
  create: (c, name, o) => BallSim.create(c, name, o),
  options: () => ({ n: 16000, density: 0.7, size: 5, temperature: 1, seed: 1 }),
  names: ["liquid", "the grain"],
  colors: ["--species-2", "--species-3"],
  stepsPerFrame: () => 20,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    path = [];
    last = null;
    acc = [0, 0, 0];
    throws = 0;
    decaySum.fill(0);
    mass = Array.from(sim.masses())[1];
    box = Array.from(sim.box_size());
    sim.set_camera(camera.yaw, camera.pitch, camera.distance);
    sim.set_ramp_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--species-3"), ...cssColor("--species-2")]));
    await readout(sim);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("throw").onclick = () => {
  if (host.sim) void host.exclusive(async (sim) => {
    await throwGrain(sim);
    await readout(sim);
  });
};
$("clear").onclick = () => {
  path = [];
  last = null;
  drawScope();
};
const showDt = () => ($("dtv").textContent = `${DTS[Number(dtIn.value)]} τ`);
dtIn.oninput = () => {
  showDt();
  drawScope();
};
showDt();
liquid.onchange = () => host.sim && void readout(host.sim);
new ResizeObserver(drawScope).observe(scope);
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
}

declare global {
  interface Window {
    brownianTest?: { ready: Promise<void>; measure(steps: number, throwsN: number): Promise<{ d: number; zeta: number; kt: number }> };
  }
}

window.brownianTest = {
  ready: host.ready,
  /** Follow the grain for `steps` (a look every 100 steps), throw it `throwsN` times; D, ζ, Dζ. */
  async measure(steps: number, throwsN: number) {
    return host.exclusive(async (sim) => {
      // Only the grain's position at each look (the page's full readout is slower).
      for (let k = 0; k < steps / 100; k++) {
        sim.step(100);
        track(sim, Array.from((await sim.positions_of(new Uint32Array([GRAIN]))) as Float64Array));
      }
      const d = msdAt(LAG) / (6 * LAG);
      for (let k = 0; k < throwsN; k++) {
        await throwGrain(sim);
        sim.step(1000);
      }
      const zeta = friction();
      sim.render();
      return { d, zeta, kt: d * zeta };
    });
  },
};
