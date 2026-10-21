import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";
import { formatN, N_TRUE, record } from "../../src/lib/perrin";

// Two WCA gases with the same number of molecules at densities 0.02 and 0.08. "Stir" sets a
// shear wave v_x = U sin(2π y / L); its amplitude decays as e^(−ν k² t), ν = η/ρ, k = 2π/L.

const N = 4000;
const U = 0.4;
const BANDS = 48;
const profilePlot = new LinesPlot($<HTMLCanvasElement>("profile"), { xLabel: "height (share of the box)", yLabel: "sideways speed", yMin: -0.5, yMax: 0.5, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const profiles: Record<"A" | "B", { x: number; y: number }[]> = { A: [], B: [] };
const plot = new LinesPlot($<HTMLCanvasElement>("plot"), { xLabel: "time since the stir (τ)", yLabel: "swirl", log: true, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

interface Gas {
  density: number;
  stirAt: number | null; // steps at the stir
  line: { x: number; y: number }[];
  rate: number;
  host: SimHost<DiscSim> | null;
}
const gases: Record<"A" | "B", Gas> = {
  A: { density: 0.02, stirAt: null, line: [], rate: NaN, host: null },
  B: { density: 0.08, stirAt: null, line: [], rate: NaN, host: null },
};

/** The shear wave's amplitude: (2/N) Σ v_x sin(2π y/L). */
async function amplitude(sim: DiscSim) {
  const [{ pos }, v] = [(await sim.all_positions()) as { pos: Float32Array }, (await sim.velocities()) as Float32Array];
  const l = Array.from(sim.box_size())[1];
  let a = 0;
  const bins = 16;
  const [sum, cnt] = [new Float64Array(bins), new Float64Array(bins)];
  for (let i = 0; i < N; i++) {
    a += v[2 * i] * Math.sin((2 * Math.PI * pos[2 * i + 1]) / l);
    const b = Math.min(bins - 1, Math.floor((pos[2 * i + 1] / l) * bins));
    sum[b] += v[2 * i];
    cnt[b]++;
  }
  return { a: (2 * a) / N, profile: Array.from(sum, (s, b) => ({ x: (b + 0.5) / bins, y: s / Math.max(1, cnt[b]) })) };
}

/** Fit the decay rate: least squares of ln(amplitude) against time, while the swirl is clear. */
function fitRate(line: { x: number; y: number }[]) {
  const pts = line.filter((p) => p.y > 0.04);
  if (pts.length < 5) return NaN;
  let [n, sx, sy, sxx, sxy] = [0, 0, 0, 0, 0];
  for (const p of pts) {
    const y = Math.log(p.y);
    n++;
    sx += p.x;
    sy += y;
    sxx += p.x * p.x;
    sxy += p.x * y;
  }
  return -(n * sxy - sx * sy) / (n * sxx - sx * sx);
}

async function readout(key: "A" | "B", sim: DiscSim) {
  const g = gases[key];
  const { a, profile } = await amplitude(sim);
  profiles[key] = profile;
  profilePlot.set([
    { points: profiles.A, color: cssVar("--plot-back") },
    { points: profiles.B, color: cssVar("--accent") },
  ]);
  if (g.stirAt === null) return;
  const t = (sim.steps() - g.stirAt) * sim.dt();
  g.line.push({ x: t, y: Math.max(a, 1e-4) });
  g.rate = fitRate(g.line);
  show();
}

function show() {
  const [a, b] = [gases.A, gases.B];
  plot.set([
    { points: a.line, color: cssVar("--plot-back") },
    { points: b.line, color: cssVar("--accent") },
  ]);
  const eta = (g: Gas) => {
    const l = Math.sqrt(N / g.density);
    const k = (2 * Math.PI) / l;
    return (g.density * g.rate) / (k * k);
  };
  const vbar = Math.sqrt(Math.PI / 2); // mean speed in 2D at kT = m = 1
  const mfp = (g: Gas) => (2 * eta(g)) / (g.density * vbar);
  const size = (g: Gas) => 1 / (2 * Math.SQRT2 * g.density * mfp(g));
  const f = (x: number, d = 3) => (Number.isFinite(x) ? x.toFixed(d) : "–");
  $("eta").textContent = `${f(eta(a))}, ${f(eta(b))}`;
  $("rate").textContent = `${f(a.rate, 4)}, ${f(b.rate, 4)}`;
  $("mfp").textContent = `${f(mfp(a), 1)} σ, ${f(mfp(b), 1)} σ`;
  $("size").textContent = `${f(size(a), 2)} σ, ${f(size(b), 2)} σ`;
  // Loschmidt: a gramme-molecule of the liquid packs N molecules of size d, so N ∝ 1/d³; the lab is
  // scaled so that the true size, 1 σ, gives the true N.
  const d = [size(a), size(b)].filter(Number.isFinite);
  if (d.length) {
    const mean = d.reduce((s, x) => s + x, 0) / d.length;
    const n = N_TRUE / mean ** 3;
    $("nav").textContent = formatN(n);
    record("viscosity", n, `molecule size ${mean.toFixed(2)} σ from the viscosity`);
  }
}

function stir(key: "A" | "B") {
  const g = gases[key];
  const sim = g.host?.sim;
  if (!sim) return;
  const [w, l] = Array.from(sim.box_size());
  for (let k = 0; k < BANDS; k++) {
    const y0 = (k * l) / BANDS;
    const y1 = ((k + 1) * l) / BANDS;
    sim.kick_region(-1, y0, w + 1, y1, 1, U * Math.sin((2 * Math.PI * (k + 0.5)) / BANDS), 0);
  }
  g.stirAt = sim.steps();
  g.line = [];
  g.rate = NaN;
}

function makeHost(key: "A" | "B") {
  const g = gases[key];
  let last = -Infinity;
  g.host = new SimHost<DiscSim>({
    canvas: $<HTMLCanvasElement>(`canvas${key}`),
    overlay: $(`overlay${key}`),
    scenario: "boltzmann",
    options: () => ({ n: N, density: g.density, aspect: 1, temperature: 1, seed: key === "A" ? 5 : 6 }),
    names: ["molecules", "one marked molecule"],
    label: key === "A" ? "thin gas" : "dense gas",
    colors: ["--species-2", "--species-2"],
    stepsPerFrame: () => 50,
    afterSteps: async (sim) => {
      if (performance.now() - last < 200) return;
      last = performance.now();
      await readout(key, sim);
    },
    onCreate: (sim) => {
      // Colour by speed (the stirring is too gentle to see this way; the profile plot shows it).
      sim.set_speed_colors(new Float32Array([...cssColor("--species-2"), ...cssColor("--highlight"), ...cssColor("--cream")]));
      sim.color_by_speed(2.5);
    },
  });
}
makeHost("A");
makeHost("B");
const hosts = [gases.A.host!, gases.B.host!];

$("run").onclick = () => {
  const run = !hosts[0].running;
  hosts.forEach((h) => (h.running = run));
  $("run").textContent = run ? "Pause both" : "Run both";
};
$("stir").onclick = () => {
  stir("A");
  stir("B");
};

declare global {
  interface Window {
    viscosityTest?: { ready: Promise<void>; decay(steps: number): Promise<{ etaA: number; etaB: number; rateA: number; rateB: number }> };
  }
}

window.viscosityTest = {
  ready: Promise.all(hosts.map((h) => h.ready)).then(() => {}),
  /** Stir both and follow the decay for `steps` (a look every 500 steps); each gas's η. */
  async decay(steps: number) {
    for (const key of ["A", "B"] as const) {
      const host = gases[key].host!;
      await host.exclusive(async (sim) => {
        sim.step(2000);
        stir(key);
        for (let k = 0; k < steps / 500; k++) {
          sim.step(500);
          await readout(key, sim);
        }
      });
    }
    const eta = (g: Gas) => (g.density * g.rate) / ((2 * Math.PI) / Math.sqrt(N / g.density)) ** 2;
    return { etaA: eta(gases.A), etaB: eta(gases.B), rateA: gases.A.rate, rateB: gases.B.rate };
  },
};
