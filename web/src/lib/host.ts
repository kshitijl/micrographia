import { DiscSim, loadWasm, webgpuProblem } from "./wasm";
import { cssColor, onThemeChange } from "./theme";
import { EnergyPanel } from "./interact";

export type Options = Record<string, number>;

export interface HostConfig {
  canvas: HTMLCanvasElement;
  overlay: HTMLElement;
  scenario: string;
  /** Scenario options, read at every (re)creation. */
  options: () => Options;
  /** CSS custom properties for species 0..3 (missing ones fall back to defaults). */
  colors: string[];
  /** Steps per animation frame while running. */
  stepsPerFrame: () => number;
  /**
   * Called after each batch of steps with the sim. Return a promise to make the host wait for
   * it before stepping again (keeps the GPU queue short and readouts in sync).
   */
  afterSteps?: (sim: DiscSim) => Promise<void> | void;
  /** Called once a sim exists (and after every reset). */
  onCreate?: (sim: DiscSim) => Promise<void> | void;
  /** The species' names, for the "About this simulation" section. */
  names?: string[];
  /** Which simulation this is, if the page has more than one (for its "About" section). */
  label?: string;
  /**
   * The total-energy and entropy plots, added to the simulation's panel unless `false`. `work`
   * returns the work done on the system so far, if something (a piston) does work on it.
   */
  energy?: false | { work?: (sim: DiscSim) => Promise<number> };
}

/**
 * Owns one DiscSim on a canvas: creation, the run loop (with backpressure), theme colours and
 * the "no WebGPU" message.
 */
export class SimHost {
  sim: DiscSim | null = null;
  running = false;
  /** Steps run since creation, in either direction. */
  elapsed = 0;
  private busy = false;
  private paused = false;
  readonly ready: Promise<void>;
  /** The energy and entropy plots (unless the page turned them off). */
  energy: EnergyPanel | null = null;
  private lastEnergy = 0;

  constructor(private cfg: HostConfig) {
    if (cfg.energy !== false) {
      const panel = cfg.canvas.closest(".lab")?.querySelector("aside.panel");
      if (panel) {
        const box = document.createElement("div");
        box.className = "energy-panel";
        panel.appendChild(box);
        this.energy = new EnergyPanel(box, cfg.energy ?? {});
      }
    }
    this.ready = this.init();
  }

  get canvas(): HTMLCanvasElement {
    return this.cfg.canvas;
  }

  private async init() {
    const problem = await webgpuProblem();
    if (problem) {
      this.cfg.overlay.textContent = problem;
      throw new Error(problem);
    }
    await loadWasm();
    await this.create();
    onThemeChange(() => this.applyColors());
    new ResizeObserver(() => this.resize()).observe(this.cfg.canvas);
    requestAnimationFrame(() => this.frame());
  }

  private sizeCanvas() {
    const c = this.cfg.canvas;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    c.width = Math.max(1, Math.round(c.clientWidth * dpr));
    c.height = Math.max(1, Math.round(c.clientHeight * dpr));
  }

  private resize() {
    if (!this.sim) return;
    this.sizeCanvas();
    this.sim.resize(this.cfg.canvas.width, this.cfg.canvas.height);
  }

  /** Build (or rebuild) the simulation. Calls run one at a time, and each waits for the frame in
   * flight, so the old simulation is never freed while a readout is still using it. */
  create(): Promise<void> {
    const next = this.creating.catch(() => {}).then(() => this.build());
    this.creating = next;
    return next;
  }
  private creating: Promise<void> = Promise.resolve();

  private async build() {
    const paused = this.paused;
    this.paused = true;
    while (this.busy) await new Promise((r) => setTimeout(r, 5));
    this.paused = paused;
    this.sim?.free();
    this.sim = null;
    this.running = false;
    this.elapsed = 0;
    this.cfg.overlay.style.display = "grid";
    this.cfg.overlay.textContent = "Starting the GPU…";
    this.sizeCanvas();
    const opts = { aspect: this.cfg.canvas.width / this.cfg.canvas.height, ...this.cfg.options() };
    const sim = await DiscSim.create(this.cfg.canvas, this.cfg.scenario, opts);
    this.sim = sim;
    this.applyColors();
    this.cfg.overlay.style.display = "none";
    await this.cfg.onCreate?.(sim);
    showAbout(sim, this.cfg.names, this.cfg.label);
    if (this.energy) {
      this.energy.clear();
      this.energy.setNote(sim.energy_note());
      await this.energy.sample(sim);
    }
    sim.render();
  }

  applyColors() {
    if (!this.sim) return;
    const defaults = ["--coffee", "--cream", "--species-2", "--species-3"];
    const c: number[] = [];
    for (let s = 0; s < 4; s++) c.push(...cssColor(this.cfg.colors[s] ?? defaults[s]));
    c.push(...cssColor("--highlight"), ...cssColor("--stage-bg"));
    this.sim.set_colors(new Float32Array(c));
  }

  /** Run `f` with the animation loop stopped (for tests and one-off measurements). */
  async exclusive<T>(f: (sim: DiscSim) => Promise<T>): Promise<T> {
    await this.ready;
    this.paused = true;
    while (this.busy) await new Promise((r) => setTimeout(r, 5));
    try {
      return await f(this.sim!);
    } finally {
      this.paused = false;
    }
  }

  private frame() {
    requestAnimationFrame(() => this.frame());
    const sim = this.sim;
    if (!sim || this.paused) return;
    if (this.running && !this.busy) {
      const k = Math.max(1, Math.round(this.cfg.stepsPerFrame()));
      sim.step(k);
      this.elapsed += k;
      // Wait for the page's readout, or at least for the GPU, before stepping again. The energy
      // plots are sampled a few times a second.
      const after = this.cfg.afterSteps?.(sim);
      const now = performance.now();
      const energy = this.energy && now - this.lastEnergy > 150 ? ((this.lastEnergy = now), this.energy.sample(sim)) : null;
      const wait = Promise.all([after instanceof Promise ? after : (sim.sync() as Promise<void>), energy]);
      this.busy = true;
      wait.finally(() => (this.busy = false));
    }
    sim.render();
  }
}

/**
 * Write (or rewrite) the page's "About this simulation" section at the end of its notes: the
 * method, forces, boundaries and ensemble, as the engine describes the system it is running.
 */
export function showAbout(sim: DiscSim, names: string[] = [], label = "") {
  const notes = document.querySelector("article.notes");
  if (!notes) return;
  const id = `about-sim${label ? `-${label.replace(/\W+/g, "-")}` : ""}`;
  let sec = document.getElementById(id);
  if (!sec) {
    sec = document.createElement("section");
    sec.id = id;
    sec.className = "about-sim";
    notes.appendChild(sec);
  }
  const esc = (t: string) => t.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const lines = sim.describe(names) as string[];
  sec.innerHTML = `<h2>About this simulation${label ? `: ${esc(label)}` : ""}</h2><ul>${lines.map((l) => `<li>${esc(l)}</li>`).join("")}</ul>`;
}

/** A tiny DOM helper. */
export const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

/** Read a CSS custom property. */
export const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
