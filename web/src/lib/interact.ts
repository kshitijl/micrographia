// Reach-in tools for simulation pages: drag a box on the simulation to select the molecules in
// it, then kick, freeze or shove them; and a panel plotting total energy and entropy over time.

import { cssVar, type SimHost } from "./host";
import { LinesPlot } from "./plot";
import type { DiscSim } from "./wasm";

/** A rectangle in simulation coordinates (σ), y upwards. */
export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** What a tool does to the selected molecules' velocities: v → v × scale + add (σ/τ). */
export interface Action {
  label: string;
  scale: number;
  add?: [number, number];
}

export const KICK: Action = { label: "Kick (3× faster)", scale: 3 };
export const FREEZE: Action = { label: "Freeze", scale: 0 };
export const SHOVE: Action = { label: "Shove →", scale: 1, add: [4, 0] };

export interface ToolsConfig {
  host: SimHost;
  /** Where the buttons go (a panel element). */
  container: HTMLElement;
  actions?: Action[];
  /** Called after an action, e.g. to take a reading while paused. */
  after?: (sim: DiscSim, action: Action) => void | Promise<void>;
  /** Called for a click without a drag, at the clicked point (σ). */
  onClick?: (x: number, y: number) => void;
}

/**
 * Drag on the simulation to select a rectangle; buttons act on the molecules inside it. The
 * selection stays until replaced, so the same region can be kicked repeatedly.
 */
export class Tools {
  rect: Rect | null = null;
  private box: HTMLDivElement;
  private buttons: HTMLButtonElement[] = [];
  private hint: HTMLDivElement;

  constructor(private cfg: ToolsConfig) {
    const canvas = cfg.host.canvas;
    const stage = canvas.parentElement!;
    this.box = document.createElement("div");
    this.box.className = "select-box";
    this.box.style.display = "none";
    stage.appendChild(this.box);

    const wrap = document.createElement("div");
    wrap.className = "tools";
    this.hint = document.createElement("div");
    this.hint.className = "sub";
    this.hint.textContent = "Drag on the simulation to select molecules, then:";
    wrap.appendChild(this.hint);
    const row = document.createElement("div");
    row.className = "buttons";
    for (const a of cfg.actions ?? [KICK, FREEZE, SHOVE]) {
      const b = document.createElement("button");
      b.textContent = a.label;
      b.disabled = true;
      b.onclick = () => void this.apply(a);
      row.appendChild(b);
      this.buttons.push(b);
    }
    const clear = document.createElement("button");
    clear.textContent = "Clear selection";
    clear.onclick = () => this.select(null);
    row.appendChild(clear);
    wrap.appendChild(row);
    cfg.container.appendChild(wrap);

    let start: [number, number] | null = null;
    canvas.style.touchAction = "none";
    canvas.addEventListener("pointerdown", (e) => {
      start = [e.clientX, e.clientY];
      canvas.setPointerCapture(e.pointerId);
    });
    canvas.addEventListener("pointermove", (e) => {
      if (start) this.drawBox(start, [e.clientX, e.clientY]);
    });
    canvas.addEventListener("pointerup", (e) => {
      if (!start) return;
      const end: [number, number] = [e.clientX, e.clientY];
      const moved = Math.hypot(end[0] - start[0], end[1] - start[1]);
      if (moved < 6) {
        const [x, y] = this.toSim(end);
        this.box.style.display = this.rect ? "block" : "none";
        if (this.rect) this.placeBox(this.rect);
        cfg.onClick?.(x, y);
      } else {
        const [a, b] = [this.toSim(start), this.toSim(end)];
        this.select({ x0: Math.min(a[0], b[0]), y0: Math.min(a[1], b[1]), x1: Math.max(a[0], b[0]), y1: Math.max(a[1], b[1]) });
      }
      start = null;
    });
    new ResizeObserver(() => this.rect && this.placeBox(this.rect)).observe(canvas);
  }

  /** Select a rectangle (σ), or clear the selection. */
  select(r: Rect | null) {
    this.rect = r;
    for (const b of this.buttons) b.disabled = !r;
    this.box.style.display = r ? "block" : "none";
    if (r) this.placeBox(r);
  }

  async apply(a: Action) {
    const sim = this.cfg.host.sim;
    if (!sim || !this.rect) return;
    const r = this.rect;
    sim.kick_region(r.x0, r.y0, r.x1, r.y1, a.scale, a.add?.[0] ?? 0, a.add?.[1] ?? 0);
    await this.cfg.after?.(sim, a);
    sim.render();
  }

  /** Client coordinates to simulation coordinates, through the drawn view. */
  private toSim([cx, cy]: [number, number]): [number, number] {
    const canvas = this.cfg.host.canvas;
    const r = canvas.getBoundingClientRect();
    const v = this.cfg.host.sim?.view() ?? [0, 0, 1, 1];
    return [v[0] + ((cx - r.left) / r.width) * (v[2] - v[0]), v[3] - ((cy - r.top) / r.height) * (v[3] - v[1])];
  }

  private drawBox(a: [number, number], b: [number, number]) {
    const r = this.cfg.host.canvas.getBoundingClientRect();
    const s = this.box.style;
    s.display = "block";
    s.left = `${Math.min(a[0], b[0]) - r.left}px`;
    s.top = `${Math.min(a[1], b[1]) - r.top}px`;
    s.width = `${Math.abs(a[0] - b[0])}px`;
    s.height = `${Math.abs(a[1] - b[1])}px`;
  }

  private placeBox(rect: Rect) {
    const canvas = this.cfg.host.canvas;
    const v = this.cfg.host.sim?.view();
    if (!v) return;
    const [w, h] = [canvas.clientWidth, canvas.clientHeight];
    const px = (x: number) => ((x - v[0]) / (v[2] - v[0])) * w;
    const py = (y: number) => ((v[3] - y) / (v[3] - v[1])) * h;
    const s = this.box.style;
    s.left = `${px(rect.x0)}px`;
    s.top = `${py(rect.y1)}px`;
    s.width = `${px(rect.x1) - px(rect.x0)}px`;
    s.height = `${py(rect.y0) - py(rect.y1)}px`;
  }
}

/** Boltzmann's H-function entropy per molecule, −Σ p ln(p/ΔE), of kinetic energies (2D: the
 * density of states in kinetic energy is flat), with the Miller–Madow correction (m − 1)/2N. */
export function entropy(ke: ArrayLike<number>, de = 0.05): number {
  const counts = new Map<number, number>();
  for (let i = 0; i < ke.length; i++) {
    const b = Math.floor(ke[i] / de);
    counts.set(b, (counts.get(b) ?? 0) + 1);
  }
  const n = ke.length;
  let s = 0;
  for (const c of counts.values()) s -= (c / n) * Math.log(c / n / de);
  return s + (counts.size - 1) / (2 * n);
}

/**
 * Total energy and entropy over time, on two small plots added to `container`. Call `sample`
 * from the page's readout (it reads kinetic and potential energies from the GPU).
 */
export class EnergyPanel {
  t: number[] = [];
  total: number[] = [];
  s: number[] = [];
  smax: number[] = [];
  private energyPlot: LinesPlot;
  private entropyPlot: LinesPlot;

  constructor(container: HTMLElement, opts: { bath?: string } = {}) {
    const mk = (label: string, items: [string, string][]) => {
      const c = document.createElement("canvas");
      c.className = "plot";
      c.setAttribute("aria-label", label);
      container.appendChild(c);
      const legend = document.createElement("div");
      legend.className = "legend";
      legend.innerHTML = items.map(([color, text]) => `<span><i style="background: var(${color})"></i>${text}</span>`).join("");
      container.appendChild(legend);
      return c;
    };
    this.energyPlot = new LinesPlot(mk("Total energy per molecule over time", [["--accent", `total energy per molecule${opts.bath ? ` (${opts.bath})` : ""}`]]), {
      xLabel: "time (τ)",
      yLabel: "energy per molecule",
      yMin: 0,
      ink: () => cssVar("--muted"),
      rule: () => cssVar("--rule"),
    });
    this.entropyPlot = new LinesPlot(
      mk("Entropy per molecule over time, and its maximum for the current energy", [
        ["--accent", "entropy per molecule (k)"],
        ["--muted", "the most it can be"],
      ]),
      { xLabel: "time (τ)", yLabel: "entropy per molecule", ink: () => cssVar("--muted"), rule: () => cssVar("--rule") },
    );
  }

  clear() {
    this.t = [];
    this.total = [];
    this.s = [];
    this.smax = [];
    this.draw();
  }

  async sample(sim: DiscSim) {
    const [{ ke }, pe] = (await Promise.all([sim.kinetic_energies(), sim.potential_energy()])) as [{ ke: Float32Array }, number];
    let kin = 0;
    for (let i = 0; i < ke.length; i++) kin += ke[i];
    const n = ke.length;
    this.t.push(sim.steps() * sim.dt());
    this.total.push((kin + pe) / n);
    this.s.push(entropy(ke));
    this.smax.push(1 + Math.log(kin / n));
    // Keep the plots light.
    if (this.t.length > 2000) for (const a of [this.t, this.total, this.s, this.smax]) a.splice(0, 1);
    this.draw();
    return { ke, total: (kin + pe) / n };
  }

  private draw() {
    const pts = (ys: number[]) => ys.map((y, k) => ({ x: this.t[k], y }));
    this.energyPlot.set([{ points: pts(this.total), color: cssVar("--accent") }]);
    this.entropyPlot.set([
      { points: pts(this.smax), color: cssVar("--muted"), dashed: true },
      { points: pts(this.s), color: cssVar("--accent") },
    ]);
  }
}
