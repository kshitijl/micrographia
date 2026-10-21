import { GasSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";

// The lattice gas at the critical density, and the light it would scatter: S(k) at wavelengths
// L/m for m = 1..12 (along x and y, averaged), and white light's scattered colour ∝ k⁴ S(k) at
// "blue", "green" and "red" wavelengths L/6, L/5 and L/4 (ratio 1.5, like 450 and 680 nm).

const SIDE = 258;
const M = [1, 2, 3, 4, 5, 6, 8, 10, 12];
const RGB_M = { r: 4, g: 5, b: 6 };
const tIn = $<HTMLInputElement>("t");
const plot = new LinesPlot($<HTMLCanvasElement>("sk"), { xLabel: "wavelength (sites)", yLabel: "S(k)", log: true, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
/** S(k) per m, averaged over recent readouts. */
const sAvg = new Map<number, number>();
let lastRead = -Infinity;

/** S(k) for k = 2π m / L along x and along y (averaged): |Σ (n − ρ) e^{ik·r}|² / sites. */
function structure(occ: Uint8Array) {
  const n = occ.length;
  let rho = 0;
  for (let i = 0; i < n; i++) rho += occ[i];
  rho /= n;
  // Column and row sums are all a wave along x or y needs.
  const col = new Float64Array(SIDE);
  const row = new Float64Array(SIDE);
  for (let y = 0; y < SIDE; y++)
    for (let x = 0; x < SIDE; x++) {
      const d = occ[y * SIDE + x] - rho;
      col[x] += d;
      row[y] += d;
    }
  const out = new Map<number, number>();
  for (const m of M) {
    let [cr, ci, rr, ri] = [0, 0, 0, 0];
    for (let j = 0; j < SIDE; j++) {
      const a = (2 * Math.PI * m * j) / SIDE;
      cr += col[j] * Math.cos(a);
      ci += col[j] * Math.sin(a);
      rr += row[j] * Math.cos(a);
      ri += row[j] * Math.sin(a);
    }
    // Normalised by an ideal (uncorrelated) lattice gas's value, ρ(1 − ρ).
    out.set(m, (0.5 * (cr * cr + ci * ci + rr * rr + ri * ri)) / n / (rho * (1 - rho)));
  }
  return out;
}

async function readout(sim: GasSim, force = false) {
  const now = performance.now();
  if (!force && now - lastRead < 250) return;
  lastRead = now;
  const occ = (await sim.sites()) as Uint8Array;
  const s = structure(occ);
  for (const [m, v] of s) sAvg.set(m, sAvg.has(m) ? 0.85 * sAvg.get(m)! + 0.15 * v : v);
  const pts = M.map((m) => ({ x: SIDE / m, y: sAvg.get(m)! }));
  plot.set([{ points: pts, color: cssVar("--accent"), dots: true }]);
  // White light in: each colour scattered ∝ k⁴ S(k); against an ideal gas (S = 1) at "blue".
  const I = (m: number) => m ** 4 * sAvg.get(m)!;
  const [r, g, b] = [I(RGB_M.r), I(RGB_M.g), I(RGB_M.b)];
  const max = Math.max(r, g, b);
  const c = [r, g, b].map((v) => Math.round(255 * Math.pow(v / max, 0.6)));
  $("swatch").style.background = `rgb(${c.join(",")})`;
  const total = (r + g + b) / (RGB_M.r ** 4 + RGB_M.g ** 4 + RGB_M.b ** 4);
  $("bright").textContent = `×${total.toFixed(total < 10 ? 1 : 0)}`;
  $("sb").textContent = sAvg.get(RGB_M.b)!.toFixed(2);
  $("sr").textContent = sAvg.get(RGB_M.r)!.toFixed(2);
}

const host = new SimHost<GasSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "gas",
  create: (c, name, o) => GasSim.create(c, name, o),
  options: () => ({ w: SIDE, h: SIDE, periodic: 1, temperature: Number(tIn.value), start: 2, fill: 0.5, seed: 4 }),
  names: [],
  colors: ["--species-2"],
  stepsPerFrame: () => 4,
  afterSteps: (sim) => readout(sim),
  energy: false,
  stepName: "sweeps",
  onCreate: async (sim) => {
    sAvg.clear();
    await readout(sim, true);
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
tIn.oninput = () => {
  $("tv").textContent = Number(tIn.value).toFixed(3);
  host.sim?.set_temperature(Number(tIn.value));
};

declare global {
  interface Window {
    opalescenceTest?: { ready: Promise<void>; measure(t: number, sweeps: number): Promise<{ blue: number; red: number; bright: number }> };
  }
}

window.opalescenceTest = {
  ready: host.ready,
  /** Start at kT `t`, run `sweeps` (averaging S(k) over the second half): S at the blue and red
   * scales, and the brightness against an ideal gas. */
  async measure(t: number, sweeps: number) {
    tIn.value = String(t);
    await host.create();
    return host.exclusive(async (sim) => {
      sim.step(sweeps / 2);
      sAvg.clear();
      const acc = new Map<number, number>();
      const k = 20;
      for (let j = 0; j < k; j++) {
        sim.step(sweeps / 2 / k);
        const s = structure((await sim.sites()) as Uint8Array);
        for (const [m, v] of s) acc.set(m, (acc.get(m) ?? 0) + v / k);
      }
      for (const [m, v] of acc) sAvg.set(m, v);
      const I = (m: number) => m ** 4 * sAvg.get(m)!;
      const bright = (I(4) + I(5) + I(6)) / (4 ** 4 + 5 ** 4 + 6 ** 4);
      sim.render();
      return { blue: sAvg.get(6)!, red: sAvg.get(4)!, bright };
    });
  },
};
