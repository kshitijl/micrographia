import { DiscSim } from "../../src/lib/wasm";
import { $, cssVar, SimHost } from "../../src/lib/host";
import { Spoon } from "../../src/lib/interact";

// Perrin's grains (scenario perrin): 30 grains, 3 σ across and 30 times a molecule's mass, in a
// thin gas under gravity set for a scale height of 12 σ. Measured natively (crates/md/tests/
// perrin.rs): they settle within ~1,000 τ.

const W = 160;
const H = 60;
const MASS = 30;
const SCALE = 12; // σ: the scale height at kT = 1 and gravity ×1
const BIN = 2; // σ
const FIT_FROM = 6; // σ: fit above the crowded bottom
// Perrin's units: his gamboge grains' effective mass (in water), g, room temperature, R.
const M_LAB = 8.3e-15; // g
const G_LAB = 981; // cm/s²
const T_LAB = 293; // K for kT = 1
const R_LAB = 8.314e7; // erg/(mol K)
const N_TRUE = 6.022e23;

const gIn = $<HTMLInputElement>("g");
const bins = new Float64Array(Math.ceil(H / BIN));
let samples = 0;
let lastRead = -Infinity;

const gravity = () => (Number(gIn.value) * 1) / (MASS * SCALE);

/** Least-squares fit of ln(count) against height, above FIT_FROM; returns the scale height. */
function fit() {
  const pts: [number, number, number][] = [];
  bins.forEach((c, k) => {
    const h = (k + 0.5) * BIN;
    if (h > FIT_FROM && c > 0) pts.push([h, Math.log(c), c]);
  });
  if (pts.length < 3) return NaN;
  let [sw, sx, sy, sxx, sxy] = [0, 0, 0, 0, 0];
  for (const [x, y, w] of pts) {
    sw += w;
    sx += w * x;
    sy += w * y;
    sxx += w * x * x;
    sxy += w * x * y;
  }
  const slope = (sw * sxy - sx * sy) / (sw * sxx - sx * sx);
  return slope < 0 ? -1 / slope : NaN;
}

function drawHist() {
  const c = $<HTMLCanvasElement>("hist");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  const [Wc, Hc] = [c.width, c.height];
  g.fillStyle = cssVar("--surface");
  g.fillRect(0, 0, Wc, Hc);
  const max = Math.max(...bins, 1);
  const lmax = Math.log(max);
  const decades = 5; // e-folds shown
  const x = (v: number) => (v <= 0 ? 0 : Math.max(0, ((Math.log(v) - (lmax - decades)) / decades) * Wc));
  const bh = Hc / bins.length;
  g.fillStyle = cssVar("--species-3");
  bins.forEach((v, k) => g.fillRect(0, Hc - (k + 1) * bh + 1, x(v), bh - 2));
  const s = fit();
  if (Number.isFinite(s)) {
    // The fitted exponential, through the counts' weighted middle.
    g.strokeStyle = cssVar("--ink") || cssVar("--muted");
    g.setLineDash([4 * dpr, 3 * dpr]);
    g.lineWidth = dpr;
    g.beginPath();
    let [sh, sl] = [0, 0];
    let wsum = 0;
    bins.forEach((v, k) => {
      const h = (k + 0.5) * BIN;
      if (h > FIT_FROM && v > 0) {
        sh += v * h;
        sl += v * Math.log(v);
        wsum += v;
      }
    });
    const [h0, l0] = [sh / wsum, sl / wsum];
    for (let k = 0; k < bins.length; k++) {
      const h = (k + 0.5) * BIN;
      const v = Math.exp(l0 - (h - h0) / s);
      const px = ((Math.log(v) - (lmax - decades)) / decades) * Wc;
      const py = Hc - (k + 0.5) * bh;
      if (k === 0) g.moveTo(px, py);
      else g.lineTo(px, py);
    }
    g.stroke();
    g.setLineDash([]);
  }
}

async function readout(sim: DiscSim, force = false) {
  const now = performance.now();
  if (!force && now - lastRead < 150) return;
  lastRead = now;
  const { pos, species } = (await sim.all_positions()) as { pos: Float32Array; species: Uint32Array };
  for (let i = 0; i < species.length; i++) if (species[i] === 1) bins[Math.min(bins.length - 1, Math.floor(pos[2 * i + 1] / BIN))]++;
  samples++;
  const { ke, species: sp } = (await sim.kinetic_energies()) as { ke: Float32Array; species: Uint32Array };
  let [k, n] = [0, 0];
  for (let i = 0; i < ke.length; i++) if (sp[i] === 0) {
    k += ke[i];
    n++;
  }
  const kt = k / n;
  drawHist();
  const s = fit();
  const ktGrains = MASS * gravity() * s;
  $("hs").textContent = Number.isFinite(s) ? `${s.toFixed(1)} σ (at this kT, kT/mg = ${(kt / (MASS * gravity())).toFixed(1)})` : "counting…";
  $("kt").textContent = Number.isFinite(s) ? ktGrains.toFixed(3) : "–";
  $("tm").textContent = kt.toFixed(3);
  if (Number.isFinite(s)) {
    // In Perrin's units: H in cm (the box scaled so that kT/mg at kT = 1 and gravity ×1 is his ~50 µm).
    const hLab = s * ((R_LAB * T_LAB) / (N_TRUE * M_LAB * G_LAB) / SCALE);
    const nav = (R_LAB * T_LAB * kt) / (M_LAB * G_LAB * Number(gIn.value) * hLab);
    $("nav").textContent = `${(nav / 1e23).toFixed(1)} × 10²³ (H = ${(hLab * 1e4).toFixed(0)} µm)`;
  }
}

const host = new SimHost<DiscSim>({
  canvas: $<HTMLCanvasElement>("canvas"),
  overlay: $("overlay"),
  scenario: "perrin",
  options: () => ({ width: W, box: H, n: 600, grains: 30, size: 3, mass: MASS, height: SCALE / Number(gIn.value), seed: 2 }),
  names: ["molecules", "grains"],
  colors: ["--species-2", "--species-3"],
  stepsPerFrame: () => 100,
  afterSteps: (sim) => readout(sim),
  onCreate: async (sim) => {
    bins.fill(0);
    samples = 0;
    await readout(sim, true);
  },
});

$("run").onclick = () => {
  host.running = !host.running;
  $("run").textContent = host.running ? "Pause" : "Run";
};
$("clear").onclick = () => {
  bins.fill(0);
  samples = 0;
};
gIn.oninput = () => {
  $("gv").textContent = `×${Number(gIn.value).toFixed(2)}`;
  host.sim?.set_gravity(0, -gravity());
  bins.fill(0);
};
const scale = (f: number) => {
  if (!host.sim) return;
  host.sim.kick_region(-1, -1, W + 1, H + 1, Math.sqrt(f), 0, 0);
  bins.fill(0);
};
$("heat").onclick = () => scale(1.1);
$("cool").onclick = () => scale(0.9);
new Spoon(host, $<HTMLButtonElement>("spoon"), { size: 12, taupersecond: () => (host.running ? 100 * 60 * 0.005 : 1) });
new ResizeObserver(drawHist).observe($("hist"));

declare global {
  interface Window {
    sedimentationTest?: { ready: Promise<void>; run(settle: number, count: number): Promise<{ scale: number; kt: number; thermometer: number }> };
  }
}

window.sedimentationTest = {
  ready: host.ready,
  /** Let the grains settle for `settle` steps, then count for `count` steps (a look every 1,000). */
  async run(settle: number, count: number) {
    return host.exclusive(async (sim) => {
      for (let k = 0; k < settle / 2000; k++) sim.step(2000);
      bins.fill(0);
      for (let k = 0; k < count / 1000; k++) {
        sim.step(1000);
        await readout(sim, true);
      }
      sim.render();
      return { scale: fit(), kt: Number($("kt").textContent), thermometer: Number($("tm").textContent) };
    });
  },
};
