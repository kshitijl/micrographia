import { $, cssVar } from "../../src/lib/host";
import { LinesPlot } from "../../src/lib/plot";
import { Chain, charges, SEQUENCE, type Conditions } from "./model";

// A lattice protein (model.ts). Checked in Node, annealed then averaged over 20,000 sweeps:
// at kT 0.3 and pH 7 it is compact (Rg 2.04, 8.8 oily contacts); at kT 0.6, 4.4 contacts; at
// pH 1 it swells (Rg 2.73), and with salt (Debye length 0.5) comes back (2.40).

const tIn = $<HTMLInputElement>("t");
const phIn = $<HTMLInputElement>("ph");
const saltIn = $<HTMLInputElement>("salt");
const BJERRUM = 2;
const SWEEPS_PER_FRAME = 30;

let chain = new Chain(1);
let running = false;
let e = 0;
let cooking: { until: number; back: number } | null = null;
/** Melting curve: contacts summed per temperature bin (0.05 wide). */
const melt = new Map<number, { s: number; n: number }>();
let line: { x: number; y: number }[] = [];
let eline: { x: number; y: number }[] = [];
let sweeps = 0;

const meltPlot = new LinesPlot($<HTMLCanvasElement>("melt"), { xLabel: "temperature (kT, ε)", yLabel: "oily contacts", yMin: 0, ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });
const timePlot = new LinesPlot($<HTMLCanvasElement>("time"), { xLabel: "Monte Carlo sweeps", yLabel: "contacts; energy", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") });

/** The Debye length from the salt slider: 5 sites in pure water down to 0.5 in brine. */
const debye = () => 5 * 0.1 ** Number(saltIn.value);
function conditions(): Conditions {
  return { kT: Number(tIn.value), pH: Number(phIn.value), debye: debye(), bjerrum: BJERRUM };
}

function sweep(c: Conditions) {
  for (let k = 0; k < chain.n; k++) e = chain.move(c, e).e;
  sweeps++;
}

function readout() {
  const c = conditions();
  const { contacts } = chain.energy({ ...c });
  const b = Math.round(c.kT / 0.05);
  const m = melt.get(b) ?? { s: 0, n: 0 };
  melt.set(b, { s: m.s + contacts, n: m.n + 1 });
  line.push({ x: sweeps, y: contacts });
  eline.push({ x: sweeps, y: e });
  if (line.length > 1500) [line, eline].forEach((l) => l.shift());
  const pts = [...melt].filter(([, v]) => v.n > 20).map(([k, v]) => ({ x: k * 0.05, y: v.s / v.n })).sort((a, b) => a.x - b.x);
  meltPlot.set([{ points: pts, color: cssVar("--accent"), dots: true }]);
  timePlot.set([
    { points: line, color: cssVar("--accent") },
    { points: eline, color: cssVar("--muted") },
  ]);
  $("c").textContent = String(contacts);
  $("rg").textContent = chain.rg().toFixed(2);
  const net = chain.q.reduce((a, x) => a + x, 0);
  $("q").textContent = `${net >= 0 ? "+" : ""}${net.toFixed(1)}`;
  $("state").textContent = contacts >= 7 ? "folded" : contacts >= 4 ? "partly folded" : "unfolded";
  draw();
}

function draw() {
  const cv = $<HTMLCanvasElement>("chain");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  cv.width = Math.round(cv.clientWidth * dpr);
  cv.height = Math.round(cv.clientHeight * dpr);
  const g = cv.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const [W, H] = [cv.clientWidth, cv.clientHeight];
  g.fillStyle = cssVar("--stage-bg");
  g.fillRect(0, 0, W, H);
  const { x, y, n } = chain;
  const [mx, my] = [x.reduce((a, v) => a + v, 0) / n, y.reduce((a, v) => a + v, 0) / n];
  const s = Math.min(W, H) / 16;
  const px = (i: number) => W / 2 + (x[i] - mx) * s;
  const py = (i: number) => H / 2 - (y[i] - my) * s;
  view = { mx, my, s, W, H };
  // Oily contacts.
  g.strokeStyle = cssVar("--accent");
  g.setLineDash([3, 3]);
  g.lineWidth = 2;
  for (let i = 0; i < n; i++)
    for (let j = i + 2; j < n; j++)
      if (SEQUENCE[i] === "H" && SEQUENCE[j] === "H" && Math.abs(x[i] - x[j]) + Math.abs(y[i] - y[j]) === 1) {
        g.beginPath();
        g.moveTo(px(i), py(i));
        g.lineTo(px(j), py(j));
        g.stroke();
      }
  g.setLineDash([]);
  // The backbone.
  g.strokeStyle = cssVar("--cream");
  g.lineWidth = 3;
  g.beginPath();
  for (let i = 0; i < n; i++) (i ? g.lineTo : g.moveTo).call(g, px(i), py(i));
  g.stroke();
  for (let i = 0; i < n; i++) {
    const r = SEQUENCE[i];
    const col = r === "H" ? cssVar("--accent") : r === "D" ? cssVar("--highlight") : r === "K" ? cssVar("--species-2") : cssVar("--muted");
    g.globalAlpha = r === "D" || r === "K" ? 0.35 + 0.65 * Math.abs(chain.q[i]) : 1;
    g.fillStyle = col;
    g.beginPath();
    g.arc(px(i), py(i), s * 0.32, 0, 2 * Math.PI);
    g.fill();
    g.globalAlpha = 1;
    if ((r === "D" || r === "K") && Math.abs(chain.q[i]) > 0.5) {
      g.fillStyle = "#fff";
      g.font = `${Math.round(s * 0.4)}px system-ui, sans-serif`;
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(r === "D" ? "−" : "+", px(i), py(i));
    }
  }
  if (chain.pull) {
    const p = chain.pull;
    g.strokeStyle = cssVar("--highlight");
    g.lineWidth = 1.5;
    g.beginPath();
    g.moveTo(px(p.i), py(p.i));
    g.lineTo(W / 2 + (p.x - mx) * s, H / 2 - (p.y - my) * s);
    g.stroke();
  }
}
let view = { mx: 0, my: 0, s: 1, W: 1, H: 1 };

function frame() {
  if (running) {
    if (cooking && sweeps >= cooking.until) {
      tIn.value = String(cooking.back);
      cooking = null;
      labels();
    }
    chain.q = charges(Number(phIn.value));
    const c = conditions();
    e = chain.energy(c).energy;
    for (let k = 0; k < SWEEPS_PER_FRAME; k++) sweep(c);
    readout();
  }
  requestAnimationFrame(frame);
}

const labels = () => {
  const t = Number(tIn.value);
  $("tv").textContent = `kT ${t.toFixed(2)}${t < 0.32 ? " (body heat)" : t < 0.5 ? " (fever, and beyond)" : " (cooking)"}`;
  const ph = Number(phIn.value);
  $("phv").textContent = `${ph.toFixed(1)}${ph < 3.5 ? " (vinegar, stomach)" : ph > 11 ? " (soap, bleach)" : ph > 6.5 && ph < 7.5 ? " (neutral)" : ""}`;
  $("sv").textContent = `Debye length ${debye().toFixed(1)} sites`;
};
for (const el of [tIn, phIn, saltIn]) el.addEventListener("input", labels);
labels();

$("run").onclick = () => {
  running = !running;
  $("run").textContent = running ? "Pause" : "Run";
};
$("cook").onclick = () => {
  cooking = { until: sweeps + 3000, back: cooking?.back ?? Number(tIn.value) };
  tIn.value = "1.1";
  labels();
  if (!running) $("run").click();
};
$("reset").onclick = () => {
  chain = new Chain(Math.floor(Math.random() * 1e9));
  sweeps = 0;
  line = [];
  eline = [];
  readout();
};
{
  const cv = $<HTMLCanvasElement>("chain");
  const toLattice = (ev: PointerEvent) => {
    const r = cv.getBoundingClientRect();
    return { x: view.mx + (ev.clientX - r.left - view.W / 2) / view.s, y: view.my - (ev.clientY - r.top - view.H / 2) / view.s };
  };
  cv.addEventListener("pointerdown", (ev) => {
    const p = toLattice(ev);
    let best = 0;
    for (let i = 1; i < chain.n; i++) if (Math.hypot(chain.x[i] - p.x, chain.y[i] - p.y) < Math.hypot(chain.x[best] - p.x, chain.y[best] - p.y)) best = i;
    cv.setPointerCapture(ev.pointerId);
    chain.pull = { i: best, x: p.x, y: p.y, force: 1.5 };
    if (!running) $("run").click();
  });
  cv.addEventListener("pointermove", (ev) => {
    if (!chain.pull) return;
    const p = toLattice(ev);
    chain.pull.x = p.x;
    chain.pull.y = p.y;
  });
  const off = () => (chain.pull = null);
  cv.addEventListener("pointerup", off);
  cv.addEventListener("pointercancel", off);
}
readout();
new ResizeObserver(draw).observe($("chain"));
requestAnimationFrame(frame);

declare global {
  interface Window {
    foldingTest?: { run(kT: number, pH: number, salt: number, sweeps: number): { contacts: number; rg: number } };
  }
}

window.foldingTest = {
  /** Start stretched, anneal down to kT over half the sweeps, then average over as many again. */
  run(kT, pH, salt, n) {
    tIn.value = String(kT);
    phIn.value = String(pH);
    saltIn.value = String(salt);
    labels();
    chain = new Chain(1);
    chain.q = charges(pH);
    const c = conditions();
    e = chain.energy(c).energy;
    for (let s = 0; s < n; s++) {
      const t = s < n / 2 ? Math.max(kT, 1.5 - ((1.5 - kT) * s) / (n / 2)) : kT;
      e = chain.energy({ ...c, kT: t }).energy;
      sweep({ ...c, kT: t });
    }
    e = chain.energy(c).energy;
    let [con, rg, m] = [0, 0, 0];
    for (let s = 0; s < n; s++) {
      sweep(c);
      if (s % 10 === 0) {
        con += chain.energy(c).contacts;
        rg += chain.rg();
        m++;
      }
    }
    readout();
    return { contacts: con / m, rg: rg / m };
  },
};
