import { $, cssVar } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";

// Haemoglobin as the MWC two-state model: each molecule tense (T) or relaxed (R), four sites
// each empty, O₂ or CO. Weights: L for T (empty), and per bound ligand p/K (K_R = 5, K_T = 500
// mmHg for O₂; CO 220 times tighter). P50 = 26.7 mmHg, Hill n = 2.85.

const L = 1000;
const K = { R: 5, T: 500 };
const M = 220; // CO's affinity over O₂'s
const N = 300;
const MOVES_PER_FRAME = 2;
type Site = 0 | 1 | 2; // empty, O₂, CO

interface Hb {
  relaxed: boolean;
  sites: Site[];
  /** Position round the loop: 0–0.5 in the lungs, 0.5–1 in the body. */
  phase: number;
  /** O₂ held on entering the body, for the delivery count. */
  entered: number;
}

let seed = 5;
function rand() {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

const altIn = $<HTMLInputElement>("alt");
const coIn = $<HTMLInputElement>("co");
let exercising = false;
let pure = false;
let running = false;
let hbs: Hb[] = Array.from({ length: N }, (_, k) => ({ relaxed: false, sites: [0, 0, 0, 0], phase: k / N, entered: 0 }));
/** Recent deliveries (O₂ per molecule per trip), the saturations measured leaving each place,
 * and CO's share of the seats leaving the lungs. */
let deliveries: number[] = [];
let satLungs: number[] = [];
let satBody: number[] = [];
let coLungs: number[] = [];

/** Alveolar O₂ (mmHg): 100 at sea level, falling with the air's pressure (scale height 8 km). */
const pLungs = () => (pure ? 600 : 100 * Math.exp(-Number(altIn.value) / 8000));
/** The body's O₂ (mmHg): 40 at rest, 20 exercising, and no more than the lungs' less a margin. */
const pBody = () => Math.min(exercising ? 20 : 40, 0.6 * pLungs());
/** CO's partial pressure (mmHg) from its concentration in the air (ppm of the dry alveolar 713 mmHg). */
const pCO = () => Number(coIn.value) * 1e-6 * 713 * Math.exp(-Number(altIn.value) / 8000);

/** Boltzmann weight of one site's occupant in a given shape. */
function weight(s: Site, relaxed: boolean, pO2: number, pco: number) {
  const k = relaxed ? K.R : K.T;
  return s === 0 ? 1 : s === 1 ? pO2 / k : (pco * M) / k;
}

/** CO comes and goes only in the lungs (it has no source or sink in the body), and slowly: its
 * moves are offered this often, compared with oxygen's (both ways alike, so the balance it
 * reaches is unchanged). */
const CO_RATE = 0.05;

function mc(h: Hb, pO2: number, pco: number, inLungs: boolean) {
  // A site: offer one of the other two occupants.
  const i = Math.floor(rand() * 4);
  const old = h.sites[i];
  const next = ((old + 1 + Math.floor(rand() * 2)) % 3) as Site;
  const co = old === 2 || next === 2;
  const r = co && (!inLungs || rand() >= CO_RATE) ? 0 : weight(next, h.relaxed, pO2, pco) / weight(old, h.relaxed, pO2, pco);
  if (r >= 1 || rand() < r) h.sites[i] = next;
  // The shape: T over R is L, times c = K_R/K_T for each bound ligand.
  const bound = h.sites.filter((s) => s !== 0).length;
  const tOverR = L * (K.R / K.T) ** bound;
  const ratio = h.relaxed ? tOverR : 1 / tOverR;
  if (ratio >= 1 || rand() < ratio) h.relaxed = !h.relaxed;
}

function step() {
  const [pl, pb, pco] = [pLungs(), pBody(), pCO()];
  for (const h of hbs) {
    const before = h.phase;
    h.phase = (h.phase + 0.004) % 1;
    // Leaving the lungs (entering the body), or leaving the body: count before it adapts.
    const o2 = h.sites.filter((s) => s === 1).length;
    if (before < 0.5 && h.phase >= 0.5) {
      h.entered = o2;
      satLungs.push(o2 / 4);
      coLungs.push(h.sites.filter((s) => s === 2).length / 4);
    } else if (before > h.phase) {
      deliveries.push(h.entered - o2);
      satBody.push(o2 / 4);
    }
    const inLungs = h.phase < 0.5;
    for (let k = 0; k < MOVES_PER_FRAME * 4; k++) mc(h, inLungs ? pl : pb, pco, inLungs);
  }
  for (const l of [deliveries, satLungs, satBody, coLungs]) if (l.length > 600) l.splice(0, l.length - 600);
}

/** The MWC O₂ saturation at pO₂ with CO at pco. */
function saturation(pO2: number, pco: number) {
  const a = (k: number) => pO2 / k;
  const b = (k: number) => (pco * M) / k;
  const zr = 1 + a(K.R) + b(K.R);
  const zt = 1 + a(K.T) + b(K.T);
  return (a(K.R) * zr ** 3 + L * a(K.T) * zt ** 3) / (zr ** 4 + L * zt ** 4);
}

const curve = new LinesPlot($<HTMLCanvasElement>("curve"), { xLabel: "oxygen pressure (mmHg)", yLabel: "share of sites carrying O₂", yMin: 0, yMax: 1, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const mean = (l: number[]) => (l.length ? l.reduce((a, x) => a + x, 0) / l.length : NaN);

function show() {
  const ps = Array.from({ length: 121 }, (_, k) => k);
  const pco = pCO();
  curve.set([
    { points: ps.map((p) => ({ x: p, y: saturation(p, 0) })), color: cssVar("--muted") },
    { points: ps.map((p) => ({ x: p, y: p / (p + 2.8) })), color: cssVar("--species-2") },
    { points: ps.map((p) => ({ x: p, y: saturation(p, pco) })), color: cssVar("--accent") },
    { points: [{ x: Math.min(120, pLungs()), y: mean(satLungs) }, { x: pBody(), y: mean(satBody) }].filter((q) => Number.isFinite(q.y)), color: cssVar("--highlight"), dots: true },
  ]);
  $("p").textContent = `${pLungs().toFixed(0)}, ${pBody().toFixed(0)}`;
  const f = (x: number) => (Number.isFinite(x) ? `${(100 * x).toFixed(0)}%` : "–");
  $("sat").textContent = `${f(mean(satLungs))}, ${f(mean(satBody))}`;
  const d = mean(deliveries);
  $("del").textContent = Number.isFinite(d) ? d.toFixed(2) : "–";
  $("cohb").textContent = f(mean(coLungs));
  draw();
}

function draw() {
  const c = $<HTMLCanvasElement>("loop");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const [W, H] = [c.clientWidth, c.clientHeight];
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, W, H);
  g.fillStyle = "rgba(120,170,255,0.08)";
  g.fillRect(0, 0, W / 2, H);
  g.fillStyle = "rgba(255,140,90,0.08)";
  g.fillRect(W / 2, 0, W / 2, H);
  g.fillStyle = cssVar("--cream");
  g.font = "13px system-ui, sans-serif";
  g.fillText("lungs", 12, 20);
  g.fillText("body", W - 46, 20);
  const colors = ["#4a1820", "#e0453a", "#ff8fb0"];
  const s = Math.min(W, H) / 40;
  hbs.forEach((h, k) => {
    // Round an oval: the top run is the lungs to body, the bottom run back.
    const a = 2 * Math.PI * h.phase;
    const lane = (k % 6) - 2.5;
    const [rx, ry] = [W * 0.4 + lane * s * 1.2, H * 0.36 + lane * s * 1.2];
    const [cx, cy] = [W / 2 - rx * Math.cos(a), H / 2 - ry * Math.sin(a)];
    h.sites.forEach((site, i) => {
      g.fillStyle = colors[site];
      const [dx, dy] = [(i % 2) * s * 0.9 - s * 0.45, Math.floor(i / 2) * s * 0.9 - s * 0.45];
      g.beginPath();
      if (h.relaxed) g.arc(cx + dx, cy + dy, s * 0.42, 0, 2 * Math.PI);
      else g.rect(cx + dx - s * 0.4, cy + dy - s * 0.4, s * 0.8, s * 0.8);
      g.fill();
    });
  });
}

function frame() {
  if (running) {
    step();
    show();
  }
  requestAnimationFrame(frame);
}

const labels = () => {
  const alt = Number(altIn.value);
  $("altv").textContent = alt === 0 ? "sea level" : `${alt.toLocaleString("en")} m${alt >= 8800 ? " (Everest)" : alt > 5000 ? " (high camp)" : ""}`;
  const co = Number(coIn.value);
  $("cov").textContent = co === 0 ? "none" : `${co} ppm${co >= 200 ? " (headache within hours, worse)" : co >= 50 ? " (an alarm sounds)" : ""}`;
};
for (const el of [altIn, coIn]) el.addEventListener("input", () => {
  labels();
  deliveries = [];
  satLungs = [];
  satBody = [];
  coLungs = [];
});
labels();
$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("exercise").onclick = () => {
  exercising = !exercising;
  $("exercise").textContent = exercising ? "Rest" : "Exercise";
  deliveries = [];
  satBody = [];
};
$("pure").onclick = () => {
  pure = !pure;
  $("pure").textContent = pure ? "Breathe air" : "Breathe pure oxygen";
  deliveries = [];
  satLungs = [];
  satBody = [];
};
show();
new ResizeObserver(draw).observe($("loop"));
requestAnimationFrame(frame);

declare global {
  interface Window {
    bloodTest?: { run(opts: { altitude?: number; co?: number; exercise?: boolean; pure?: boolean }, frames: number): { lungs: number; body: number; delivered: number; cohb: number } };
  }
}

window.bloodTest = {
  /** Set the conditions, run `frames` frames, and average over the second half. */
  run(o, frames) {
    altIn.value = String(o.altitude ?? 0);
    coIn.value = String(o.co ?? 0);
    exercising = !!o.exercise;
    pure = !!o.pure;
    labels();
    for (let k = 0; k < frames / 2; k++) step();
    deliveries = [];
    satLungs = [];
    satBody = [];
    coLungs = [];
    for (let k = 0; k < frames / 2; k++) step();
    show();
    return { lungs: mean(satLungs), body: mean(satBody), delivered: mean(deliveries), cohb: mean(coLungs) };
  },
};
