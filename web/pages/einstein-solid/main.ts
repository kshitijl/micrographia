import { $, cssVar } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";

// Two Einstein solids: N oscillators each, energy in whole quanta ε. A step picks two atoms at
// random (in the same block, or across the contact when the blocks touch) and moves a quantum
// from the first to the second if it has one: a symmetric proposal, so every arrangement of the
// quanta is equally likely in the long run (the microcanonical ensemble).

const SIDE = 30;
const N = SIDE * SIDE;
let blocks = [new Int32Array(N), new Int32Array(N)];
let quanta = [0, 0];
let touching = true;
let running = false;
/** Heat-capacity measurements on the left block: (kT/ε, quanta per atom). */
let cPoints: { t: number; e: number }[] = [];

let seed = 7;
const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);

function reset() {
  blocks = [new Int32Array(N), new Int32Array(N)];
  quanta = [0, 0];
  cPoints = [];
  addHeat(0, 300);
}

function addHeat(b: number, q: number) {
  for (let k = 0; k < q; k++) blocks[b][Math.floor(rand() * N)]++;
  quanta[b] += q;
}

function steps(k: number) {
  for (let s = 0; s < k; s++) {
    const bi = rand() < 0.5 ? 0 : 1;
    // Across the contact a quarter of the time, when the blocks touch.
    const bj = touching && rand() < 0.25 ? 1 - bi : bi;
    const i = Math.floor(rand() * N);
    if (blocks[bi][i] === 0) continue;
    const j = Math.floor(rand() * N);
    blocks[bi][i]--;
    blocks[bj][j]++;
    quanta[bi]--;
    quanta[bj]++;
  }
}

/** ln Ω = ln C(q + N − 1, q), by log-gamma (Stirling's series). */
function lnGamma(x: number): number {
  if (x < 7) {
    let p = 1;
    while (x < 7) p *= x++;
    return lnGamma(x) - Math.log(p);
  }
  return (x - 0.5) * Math.log(x) - x + 0.5 * Math.log(2 * Math.PI) + 1 / (12 * x) - 1 / (360 * x ** 3);
}
const lnOmega = (q: number) => lnGamma(q + N) - lnGamma(q + 1) - lnGamma(N);
/** kT/ε from the mean quanta per atom (Bose–Einstein: ⟨n⟩ = 1/(e^(ε/kT) − 1)). */
const temperature = (q: number) => (q <= 0 ? 0 : 1 / Math.log(1 + N / q));
/** Einstein's heat capacity per atom (in k) at kT/ε = t. */
const einstein = (t: number) => {
  const x = 1 / t;
  return (x * x * Math.exp(x)) / (Math.exp(x) - 1) ** 2;
};

const grid = $<HTMLCanvasElement>("grid");
const dist = new LinesPlot($<HTMLCanvasElement>("dist"), { xLabel: "quanta held, n", yLabel: "atoms", log: true, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const cv = new LinesPlot($<HTMLCanvasElement>("cv"), { xLabel: "kT/ε", yLabel: "heat capacity per atom (k)", yMin: 0, yMax: 1.1, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const curve = Array.from({ length: 100 }, (_, k) => ({ x: 0.05 + 0.03 * k, y: einstein(0.05 + 0.03 * k) }));

function draw() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  grid.width = Math.round(grid.clientWidth * dpr);
  grid.height = Math.round(grid.clientHeight * dpr);
  const g = grid.getContext("2d")!;
  const [W, H] = [grid.width, grid.height];
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, W, H);
  const cell = Math.min((W * 0.46) / SIDE, (H * 0.92) / SIDE);
  const cols = [cssColor("--species-2"), cssColor("--species-3")];
  for (let b = 0; b < 2; b++) {
    const x0 = b === 0 ? W * 0.02 : W * 0.98 - cell * SIDE;
    const y0 = (H - cell * SIDE) / 2;
    for (let i = 0; i < N; i++) {
      const n = blocks[b][i];
      const f = Math.min(1, n / 4);
      const c = cols[b];
      g.fillStyle = `rgb(${[0, 1, 2].map((k) => Math.round(255 * (0.12 + (c[k] - 0.12) * f))).join(",")})`;
      g.fillRect(x0 + (i % SIDE) * cell, y0 + Math.floor(i / SIDE) * cell, cell - 0.5, cell - 0.5);
    }
  }
  if (touching) {
    g.fillStyle = cssVar("--muted");
    g.fillRect(W * 0.02 + cell * SIDE, H * 0.4, W * 0.96 - 2 * cell * SIDE, H * 0.2);
  }
  // Distributions.
  const hist = (b: number) => {
    const h: number[] = [];
    for (const n of blocks[b]) h[n] = (h[n] ?? 0) + 1;
    return Array.from(h, (v, n) => ({ x: n, y: v ?? 0 })).filter((p) => p.y > 0);
  };
  dist.set([
    { points: hist(0), color: cssVar("--species-2"), dots: true },
    { points: hist(1), color: cssVar("--species-3"), dots: true },
  ]);
  cv.set([
    { points: curve, color: cssVar("--muted") },
    { points: measuredC(), color: cssVar("--accent"), dots: true },
  ]);
  const [t0, t1] = [temperature(quanta[0]), temperature(quanta[1])];
  const [s0, s1] = [lnOmega(quanta[0]), lnOmega(quanta[1])];
  $("q").textContent = `${quanta[0]}, ${quanta[1]}`;
  $("t").textContent = `${t0.toFixed(3)}, ${t1.toFixed(3)}`;
  $("s").textContent = `${s0.toFixed(0)}, ${s1.toFixed(0)}, ${(s0 + s1).toFixed(0)}`;
  $("c").textContent = t0 > 0 ? `${einstein(t0).toFixed(3)} (Einstein)` : "–";
}

/** Heat capacity from successive heatings of the left block alone: Δ(quanta per atom)/Δ(kT/ε). */
function measuredC() {
  const out: { x: number; y: number }[] = [];
  for (let k = 1; k < cPoints.length; k++) {
    const [a, b] = [cPoints[k - 1], cPoints[k]];
    if (b.t > a.t) out.push({ x: 0.5 * (a.t + b.t), y: (b.e - a.e) / (b.t - a.t) });
  }
  return out;
}

grid.addEventListener("pointerdown", (e) => {
  const r = grid.getBoundingClientRect();
  const b = e.clientX - r.left < r.width / 2 ? 0 : 1;
  addHeat(b, Number($<HTMLInputElement>("dq").value));
  if (b === 0 && !touching) {
    // A measurement: the left block's temperature (from its mean quanta) after each heating.
    if (!cPoints.length) cPoints.push({ t: 0, e: 0 });
    cPoints.push({ t: temperature(quanta[0]), e: quanta[0] / N });
  }
  draw();
});

function frame() {
  if (running) {
    steps(40000);
    draw();
  }
  requestAnimationFrame(frame);
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("contact").onclick = () => {
  touching = !touching;
  $("contact").textContent = touching ? "Separate the blocks" : "Put them in contact";
  if (!touching) cPoints = [{ t: temperature(quanta[0]), e: quanta[0] / N }];
  draw();
};
$("reset").onclick = () => {
  reset();
  draw();
};
const dqIn = $<HTMLInputElement>("dq");
dqIn.oninput = () => ($("dqv").textContent = `${dqIn.value} quanta`);
reset();
draw();
new ResizeObserver(draw).observe(grid);
requestAnimationFrame(frame);

declare global {
  interface Window {
    einsteinTest?: { relax(steps: number): { t: number[]; s: number[]; slope: number } };
  }
}

window.einsteinTest = {
  /** Start again, put 1,200 quanta into the left block, run `steps` hops; temperatures, entropies,
   * and the slope of ln(atoms with n quanta) against n on the left (−ε/kT for Boltzmann). */
  relax(k: number) {
    touching = true;
    reset();
    blocks[0].fill(0);
    quanta = [0, 0];
    addHeat(0, 1200);
    steps(k);
    draw();
    const h: number[] = [0, 0, 0];
    for (const n of blocks[0]) if (n < 3) h[n]++;
    return { t: [temperature(quanta[0]), temperature(quanta[1])], s: [lnOmega(quanta[0]), lnOmega(quanta[1])], slope: Math.log(h[2] / h[0]) / 2 };
  },
};
