import { $, cssVar } from "../../src/lib/host";
import { forget, formatN, N_TRUE, recorded, type Phenomenon } from "../../src/lib/perrin";

/** Perrin's table (Atoms, §120), his values ×10²² converted to ×10²³; the page that measures
 * each, if there is one. */
const ROWS: { name: string; perrin: number; page?: string; key?: Phenomenon }[] = [
  { name: "Viscosity of gases (with van der Waals)", perrin: 6.2, page: "viscosity", key: "viscosity" },
  { name: "Brownian movement: distribution of grains", perrin: 6.83, page: "sedimentation", key: "sedimentation" },
  { name: "Brownian movement: displacements", perrin: 6.88, page: "brownian", key: "brownian" },
  { name: "Brownian movement: rotations", perrin: 6.5 },
  { name: "Brownian movement: diffusion", perrin: 6.9 },
  { name: "Critical opalescence", perrin: 7.5, page: "opalescence" },
  { name: "The blue of the sky", perrin: 6.0, page: "opalescence" },
  { name: "Black-body spectrum", perrin: 6.4, page: "light", key: "blackbody" },
  { name: "Charged spheres in a gas (Millikan)", perrin: 6.8, page: "millikan", key: "millikan" },
  { name: "Radioactivity: charges produced", perrin: 6.25 },
  { name: "Radioactivity: helium produced", perrin: 6.4 },
  { name: "Radioactivity: radium disappearing", perrin: 7.1, page: "counting", key: "counting" },
  { name: "Radioactivity: energy radiated", perrin: 6.0 },
];

function show() {
  const mine = recorded();
  const body = $("table").querySelector("tbody")!;
  body.innerHTML = "";
  for (const r of ROWS) {
    const tr = document.createElement("tr");
    const m = r.key ? mine[r.key] : undefined;
    const name = r.page ? `<a href="../${r.page}/index.html">${r.name}</a>` : r.name;
    const yours = m ? formatN(m.n) : r.key ? "not yet: go and measure it" : r.page ? "the page does not count yet" : "no page yet";
    tr.innerHTML = `<td>${name}</td><td class="num">${formatN(r.perrin * 1e23)}</td><td class="num">${yours}</td><td class="how">${m ? m.how : ""}</td>`;
    if (r.key) tr.dataset.key = r.key;
    body.appendChild(tr);
  }
  strip(mine);
}

/** All the values on one axis, from 3 to 12 × 10²³. */
function strip(mine: ReturnType<typeof recorded>) {
  const c = $<HTMLCanvasElement>("strip");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  c.width = Math.round(c.clientWidth * dpr);
  c.height = Math.round(c.clientHeight * dpr);
  const g = c.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  const [W, H] = [c.clientWidth, c.clientHeight];
  g.clearRect(0, 0, W, H);
  const [lo, hi] = [3e23, 1.2e24];
  const x = (n: number) => 20 + ((Math.log(n) - Math.log(lo)) / (Math.log(hi) - Math.log(lo))) * (W - 40);
  g.strokeStyle = cssVar("--rule");
  g.beginPath();
  g.moveTo(20, H - 24);
  g.lineTo(W - 20, H - 24);
  g.stroke();
  g.fillStyle = cssVar("--muted");
  g.font = "11px system-ui, sans-serif";
  g.textAlign = "center";
  for (const t of [3, 4, 5, 6, 8, 10, 12]) g.fillText(`${t} × 10²³`, x(t * 1e23), H - 8);
  g.strokeStyle = cssVar("--good");
  g.beginPath();
  g.moveTo(x(N_TRUE), 6);
  g.lineTo(x(N_TRUE), H - 24);
  g.stroke();
  const dot = (n: number, y: number, color: string) => {
    g.fillStyle = color;
    g.beginPath();
    g.arc(x(Math.min(hi, Math.max(lo, n))), y, 5, 0, 2 * Math.PI);
    g.fill();
  };
  for (const r of ROWS) dot(r.perrin * 1e23, (H - 24) * 0.33, cssVar("--muted"));
  for (const m of Object.values(mine)) if (m) dot(m.n, (H - 24) * 0.7, cssVar("--accent"));
}

$("forget").onclick = () => {
  forget();
  show();
};
new ResizeObserver(show).observe($("strip"));
window.addEventListener("storage", show);
show();
