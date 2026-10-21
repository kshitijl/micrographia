// Part 3: why different phases win at different temperatures. G = H − T S per molecule for a
// solid, a liquid and a gas (constant H and S: a sketch), with the gas's entropy falling with
// pressure; and the entropy books of system and surroundings when one molecule changes phase.
import { $, cssVar } from "../../src/lib/host";
import { onThemeChange } from "../../src/lib/theme";

// Per molecule, in the simulation's units (k = 1), with the energies of the 2D Lennard-Jones
// substance of parts 1–2 (latent heat of evaporation ~3 ε) but a wider liquid range, like water's
// (melting at kT 0.25, boiling at 0.45 at the pot's pressure; the 2D LJ liquid's own range is only
// ~0.40–0.46). H and S are taken as constant, which is good enough for the picture.
const H = { solid: -3.4, liquid: -3.1, gas: 0 };
const S0 = { solid: 0, liquid: 0.3 / 0.25, gas: 0.3 / 0.25 + 3.1 / 0.45 };
type Phase = keyof typeof H;
const PHASES: Phase[] = ["solid", "liquid", "gas"];
const COLOR: Record<Phase, string> = { solid: "--species-2", liquid: "--plot-back", gas: "--highlight" };
const [T_LO, T_HI] = [0.1, 0.7];

const tIn = $<HTMLInputElement>("gT");
const pIn = $<HTMLInputElement>("gP");
const plot = $<HTMLCanvasElement>("gplot");
const bars = $<HTMLCanvasElement>("gbars");

/** The gas's entropy at a pressure e^lnp times the pot's: less room, less entropy. */
const S = (ph: Phase) => (ph === "gas" ? S0.gas - Number(pIn.value) : S0[ph]);
const G = (ph: Phase, t: number) => H[ph] - t * S(ph);
const winner = (t: number) => PHASES.reduce((a, b) => (G(b, t) < G(a, t) ? b : a));
/** Where two phases' lines cross. */
const crossing = (a: Phase, b: Phase) => (H[b] - H[a]) / (S(b) - S(a));

function fit(c: HTMLCanvasElement) {
  const dpr = window.devicePixelRatio || 1;
  const [W, Hh] = [c.clientWidth, c.clientHeight];
  if (c.width !== Math.round(W * dpr)) {
    c.width = Math.round(W * dpr);
    c.height = Math.round(Hh * dpr);
  }
  const g = c.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, W, Hh);
  return { g, W, H: Hh };
}

function drawG(t: number) {
  const { g, W, H: Hh } = fit(plot);
  const pad = { l: 44, r: 10, t: 12, b: 30 };
  const [gLo, gHi] = [-6.4, -0.4];
  const x = (tt: number) => pad.l + ((tt - T_LO) / (T_HI - T_LO)) * (W - pad.l - pad.r);
  const y = (gg: number) => pad.t + (1 - (gg - gLo) / (gHi - gLo)) * (Hh - pad.t - pad.b);
  g.strokeStyle = cssVar("--rule");
  g.beginPath();
  g.moveTo(pad.l, pad.t);
  g.lineTo(pad.l, Hh - pad.b);
  g.lineTo(W - pad.r, Hh - pad.b);
  g.stroke();
  g.fillStyle = cssVar("--muted");
  g.font = "12px system-ui, sans-serif";
  g.fillText("G per molecule (ε)", pad.l + 4, pad.t + 10);
  g.textAlign = "right";
  g.fillText("temperature kT →", W - pad.r, Hh - 8);
  g.textAlign = "left";
  // Each phase's line, thin; the winning stretch of each, thick.
  for (const ph of PHASES) {
    g.strokeStyle = cssVar(COLOR[ph]);
    for (const thick of [false, true]) {
      g.lineWidth = thick ? 5 : 1.5;
      g.globalAlpha = thick ? 1 : 0.6;
      g.beginPath();
      let pen = false;
      for (let k = 0; k <= 300; k++) {
        const tt = T_LO + ((T_HI - T_LO) * k) / 300;
        const on = !thick || winner(tt) === ph;
        if (on) {
          if (!pen) g.moveTo(x(tt), y(G(ph, tt)));
          else g.lineTo(x(tt), y(G(ph, tt)));
        }
        pen = on;
      }
      g.stroke();
    }
    g.globalAlpha = 1;
    g.fillStyle = cssVar(COLOR[ph]);
    const slope = S(ph) === 0 ? "0" : `−${S(ph).toFixed(2)}`;
    g.fillText(`${ph} (slope −S = ${slope})`, x(T_LO) + 6, y(G(ph, T_LO)) + (ph === "solid" ? 16 : -6));
  }
  // Melting and boiling points, and the current temperature.
  g.lineWidth = 1;
  g.setLineDash([4, 4]);
  for (const [a, b, name, row] of [["solid", "liquid", "melts", 0], ["liquid", "gas", "boils", 1]] as const) {
    const tc = crossing(a, b);
    if (tc > T_LO && tc < T_HI && winner(tc + 1e-6) !== a) {
      g.strokeStyle = cssVar("--muted");
      g.beginPath();
      g.moveTo(x(tc), pad.t);
      g.lineTo(x(tc), Hh - pad.b);
      g.stroke();
      g.fillStyle = cssVar("--muted");
      g.fillText(`${name} at ${tc.toFixed(2)}`, x(tc) + 4, Hh - pad.b - 6 - 16 * row);
    }
  }
  g.setLineDash([]);
  g.strokeStyle = cssVar("--text");
  g.lineWidth = 2;
  g.beginPath();
  g.moveTo(x(t), pad.t);
  g.lineTo(x(t), Hh - pad.b);
  g.stroke();
  return { x, pad, W };
}

/** The entropy books when one molecule melts (solid → liquid) or evaporates (liquid → gas) at t:
 * the system gains ΔS, the surroundings lose ΔH/T (the heat it takes from them), the universe
 * gains the difference. */
function drawBars(t: number) {
  const { g, W, H: Hh } = fit(bars);
  const rows = [
    { name: "one molecule melts", from: "solid" as Phase, to: "liquid" as Phase },
    { name: "one molecule evaporates", from: "liquid" as Phase, to: "gas" as Phase },
  ];
  const scale = (W - 160) / 2 / 25; // ±25 k fits across
  const mid = 150 + (W - 160) / 2;
  g.font = "12px system-ui, sans-serif";
  g.textBaseline = "middle";
  rows.forEach((r, k) => {
    const dS = S(r.to) - S(r.from);
    const dSsurr = -(H[r.to] - H[r.from]) / t;
    const net = dS + dSsurr;
    const y0 = 14 + k * (Hh / 2);
    g.fillStyle = cssVar("--text");
    g.fillText(r.name, 0, y0);
    const items: [string, number, string][] = [
      ["system gains", dS, "--plot-back"],
      ["surroundings (−ΔH/T)", dSsurr, "--muted"],
      ["universe", net, net > 0 ? "--good" : "--bad"],
    ];
    items.forEach(([label, v, col], j) => {
      const yy = y0 + 20 + j * 22;
      g.fillStyle = cssVar("--muted");
      g.fillText(label, 0, yy);
      const w = Math.max(-mid + 150, Math.min(W - mid, v * scale));
      g.fillStyle = cssVar(col);
      g.fillRect(Math.min(mid, mid + w), yy - 8, Math.abs(w), 16);
      g.fillStyle = cssVar("--text");
      g.textAlign = v >= 0 ? "left" : "right";
      g.fillText(`${v >= 0 ? "+" : "−"}${Math.abs(v).toFixed(2)} k`, mid + w + (v >= 0 ? 4 : -4), yy);
      g.textAlign = "left";
    });
    g.strokeStyle = cssVar("--rule");
    g.beginPath();
    g.moveTo(mid, y0 + 8);
    g.lineTo(mid, y0 + 20 + 2 * 22 + 10);
    g.stroke();
  });
}

function draw() {
  const t = Number(tIn.value);
  const lnp = Number(pIn.value);
  $("gTv").textContent = t.toFixed(2);
  $("gPv").textContent = lnp === 0 ? "1× (the pot's)" : `${Math.exp(lnp).toFixed(2)}×`;
  drawG(t);
  drawBars(t);
  const w = winner(t);
  $("gPhase").textContent = w;
  const [tm, tb] = [crossing("solid", "liquid"), crossing("liquid", "gas")];
  $("gPoints").textContent = `${tm.toFixed(2)} / ${tb > tm ? tb.toFixed(2) : "–"}`;
  const L = H.gas - H.liquid;
  $("gStatus").textContent =
    `At kT = ${t.toFixed(2)}, a molecule that evaporates takes ${L.toFixed(1)} ε of heat from its surroundings, costing them ` +
    `${(L / t).toFixed(1)} k of entropy (heat ÷ temperature: the colder they are, the more each bit of heat is worth), ` +
    `while it gains ${(S("gas") - S("liquid")).toFixed(1)} k itself by flying free. ` +
    (L / t > S("gas") - S("liquid") ? "The surroundings lose more than it gains: it stays put, and the liquid (or solid) wins." : "It gains more than they lose: it goes, and the gas wins.");
}

tIn.oninput = draw;
pIn.oninput = draw;
// Drag across the plot to set the temperature.
let dragging = false;
const setFromPointer = (e: PointerEvent) => {
  const r = plot.getBoundingClientRect();
  const f = (e.clientX - r.left - 44) / (r.width - 54);
  tIn.value = String(Math.max(T_LO, Math.min(T_HI, T_LO + f * (T_HI - T_LO))));
  draw();
};
plot.addEventListener("pointerdown", (e) => {
  plot.setPointerCapture(e.pointerId);
  dragging = true;
  setFromPointer(e);
});
plot.addEventListener("pointermove", (e) => dragging && setFromPointer(e));
plot.addEventListener("pointerup", () => (dragging = false));
new ResizeObserver(draw).observe(plot);
onThemeChange(draw);
