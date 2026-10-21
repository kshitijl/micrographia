import { $, cssVar } from "../../src/lib/host";
import { cssColor } from "../../src/lib/theme";
import { LinesPlot } from "../../src/lib/plot";
import { FARADAY, formatN, record } from "../../src/lib/perrin";

// Millikan's oil drop, as Langevin dynamics in SI units: overdamped (the drop reaches its
// terminal speed in microseconds), so each step moves it by (forces / 6πηa) dt plus a random
// Brownian step √(2D dt).

const E_CHARGE = 1.602e-19; // C
const KT = 1.381e-23 * 293; // J
const ETA = 1.82e-5; // Pa s, air at 20 °C
const RHO = 920; // kg/m³, oil (minus air's 1.2)
const G = 9.81;
const GAP = 8e-3; // m between the plates
const VIEW = 1e-3; // m: the microscope's field, top to bottom
const LINES = [0.25e-3, 0.75e-3]; // the reticle lines (m from the bottom of the view)
const DT = 0.002; // s

let seed = 11;
const rand = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const gauss = () => Math.sqrt(-2 * Math.log(1 - rand())) * Math.cos(2 * Math.PI * rand());

const volts = $<HTMLInputElement>("volts");
const scope = $<HTMLCanvasElement>("scope");
const hist = new LinesPlot($<HTMLCanvasElement>("hist"), { xLabel: "charge (10⁻¹⁹ C)", yLabel: "drops measured", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

let a = 1e-6; // radius
let n = 3; // charges (in e)
let y = 0.9e-3; // height in the view (m)
let x = 0.5;
let field = false;
let running = false;
let t = 0;
const trail: number[][] = [];
// Timing between the reticle lines: when the drop crosses one line, start; the other, stop.
let crossing: { from: number; t: number; field: boolean } | null = null;
let vFall = NaN;
let vRise = NaN;
const charges: number[] = [];

function newDrop() {
  a = (0.6 + 0.8 * rand()) * 1e-6;
  n = 1 + Math.floor(rand() * 8);
  y = 0.9e-3;
  field = false;
  vFall = vRise = NaN;
  crossing = null;
  trail.length = 0;
  labels();
}

const efield = () => Number(volts.value) / GAP;
function step() {
  const zeta = 6 * Math.PI * ETA * a;
  const weight = (4 / 3) * Math.PI * a ** 3 * RHO * G;
  const force = (field ? n * E_CHARGE * efield() : 0) - weight;
  const d = KT / zeta;
  const yOld = y;
  y += (force / zeta) * DT + Math.sqrt(2 * d * DT) * gauss();
  x += (Math.sqrt(2 * d * DT) * gauss()) / VIEW;
  x = Math.min(0.95, Math.max(0.05, x));
  // Keep it in view (in the real apparatus the plates are far beyond the view).
  y = Math.min(VIEW, Math.max(0, y));
  t += DT;
  // Timings: crossing one reticle line starts a run, crossing the other ends it.
  for (const [k, line] of LINES.entries()) {
    if ((yOld - line) * (y - line) < 0) {
      if (crossing && crossing.from !== k && crossing.field === field) {
        const v = Math.abs(LINES[1] - LINES[0]) / (t - crossing.t);
        if (field) vRise = v;
        else vFall = v;
        crossing = null;
        measure();
      } else crossing = { from: k, t, field };
    }
  }
}

/** After a fall and a rise: the radius from the fall, the charge from both. */
/** The charge they are all whole multiples of (10⁻¹⁹ C): start from the smallest, or a half, a
 * third or a quarter of it; refine (e = Σq / Σn, with each n the nearest whole number of e's)
 * from the smaller drops up, as a noisy smallest drop would throw the big ones' n off; take the
 * largest that fits every drop within 0.3 e. */
function elementary(qs: number[]) {
  if (qs.length < 3) return NaN;
  const sorted = [...qs].sort((a, b) => a - b);
  for (let k = 1; k <= 4; k++) {
    let e = sorted[0] / k;
    for (let m = 1; m <= sorted.length; m++) {
      const some = sorted.slice(0, m);
      e = some.reduce((a, q) => a + q, 0) / some.reduce((a, q) => a + Math.max(1, Math.round(q / e)), 0);
    }
    if (qs.every((q) => Math.abs(q / e - Math.round(q / e)) < 0.3)) return e;
  }
  return NaN;
}

function measure() {
  if (!Number.isFinite(vFall) || !Number.isFinite(vRise)) {
    labels();
    return;
  }
  const r = Math.sqrt((9 * ETA * vFall) / (2 * RHO * G));
  const q = (6 * Math.PI * ETA * r * (vFall + vRise)) / efield();
  charges.push(q / 1e-19);
  vRise = NaN; // the next pair
  labels(r, q);
  drawHist();
  const e = elementary(charges);
  $("e").textContent = Number.isFinite(e) ? `${e.toFixed(3)} × 10⁻¹⁹ C; N = F/e = ${formatN(FARADAY / (e * 1e-19))}` : charges.length < 3 ? "measure 3 drops" : "no whole multiples yet";
  if (Number.isFinite(e)) record("millikan", FARADAY / (e * 1e-19), `e from ${charges.length} drops, and the faraday`);
}

function labels(r?: number, q?: number) {
  $("vf").textContent = Number.isFinite(vFall) ? `${(vFall * 1e6).toFixed(0)} µm/s` : "–";
  $("vr").textContent = Number.isFinite(vRise) ? `${(vRise * 1e6).toFixed(0)} µm/s` : "–";
  if (r !== undefined) $("rad").textContent = `${(r * 1e6).toFixed(2)} µm`;
  if (q !== undefined) $("q").textContent = `${(q / 1e-19).toFixed(2)} × 10⁻¹⁹ C`;
  $("field").textContent = field ? "Field off (let it fall)" : "Field on (lift it)";
}

function drawHist() {
  const bin = 0.2;
  const h = new Map<number, number>();
  for (const q of charges) {
    const k = Math.round(q / bin);
    h.set(k, (h.get(k) ?? 0) + 1);
  }
  const max = Math.max(14, ...charges.map((q) => q + 1));
  const pts = Array.from({ length: Math.ceil(max / bin) }, (_, k) => ({ x: k * bin, y: h.get(k) ?? 0 }));
  const lines = Array.from({ length: 9 }, (_, m) => ({ points: [{ x: (m + 1) * 1.602, y: 0 }, { x: (m + 1) * 1.602, y: 0.3 }], color: cssVar("--muted"), dashed: true }));
  hist.set([{ points: pts, color: cssVar("--accent") }, ...lines]);
}

function draw() {
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  scope.width = Math.round(scope.clientWidth * dpr);
  scope.height = Math.round(scope.clientHeight * dpr);
  const g = scope.getContext("2d")!;
  const [W, H] = [scope.width, scope.height];
  // A dark field with a bright drop, as in the microscope.
  g.fillStyle = "#0b0a09";
  g.fillRect(0, 0, W, H);
  g.strokeStyle = "rgba(255,255,255,0.35)";
  g.lineWidth = dpr;
  for (const line of LINES) {
    const py = H - (line / VIEW) * H;
    g.beginPath();
    g.moveTo(0, py);
    g.lineTo(W, py);
    g.stroke();
  }
  const py = H - (y / VIEW) * H;
  trail.push([x * W, py]);
  if (trail.length > 200) trail.shift();
  g.strokeStyle = "rgba(243,228,198,0.25)";
  g.beginPath();
  trail.forEach(([tx, ty], k) => (k === 0 ? g.moveTo(tx, ty) : g.lineTo(tx, ty)));
  g.stroke();
  const c = cssColor("--cream");
  g.fillStyle = `rgb(${c.slice(0, 3).map((v) => Math.round(255 * v)).join(",")})`;
  g.beginPath();
  g.arc(x * W, py, Math.max(3 * dpr, (a / VIEW) * H * 3), 0, 2 * Math.PI);
  g.fill();
  if (field) {
    g.fillStyle = "rgba(229,72,58,0.8)";
    g.font = `${12 * dpr}px system-ui, sans-serif`;
    g.fillText(`field on: ${(efield() / 1000).toFixed(0)} kV/m`, 10 * dpr, 18 * dpr);
  }
}

function frame() {
  if (running) {
    // Real time: 1/60 s of drop per frame.
    for (let k = 0; k < Math.round(1 / 60 / DT); k++) step();
    draw();
  }
  requestAnimationFrame(frame);
}

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("field").onclick = () => {
  field = !field;
  crossing = null;
  labels();
};
scope.addEventListener("pointerdown", () => $("field").click());
$("xray").onclick = () => {
  const dn = (rand() < 0.5 ? -1 : 1) * (rand() < 0.7 ? 1 : 2);
  n = Math.max(1, n + dn);
  vFall = vRise = NaN;
  labels();
};
$("newdrop").onclick = () => {
  newDrop();
  draw();
};
volts.oninput = () => ($("vv").textContent = `${Number(volts.value).toLocaleString()} V`);
newDrop();
draw();
drawHist();
new ResizeObserver(draw).observe(scope);
requestAnimationFrame(frame);

declare global {
  interface Window {
    millikanTest?: { measure(drops: number): number[] };
  }
}

window.millikanTest = {
  /** Measure `drops` drops automatically (a fall and a rise each), returning the charges (10⁻¹⁹ C). */
  measure(drops: number) {
    volts.value = "8000";
    charges.length = 0;
    for (let d = 0; d < drops; d++) {
      newDrop();
      // Fall through the lines, then rise through them.
      field = false;
      y = 0.95e-3;
      while (!Number.isFinite(vFall) && t < 1e9) step();
      field = true;
      crossing = null;
      y = 0.05e-3;
      let guard = 0;
      while (charges.length <= d && guard++ < 1e6) step();
    }
    drawHist();
    return charges.slice();
  },
};
