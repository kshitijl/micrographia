import { DiscSim, loadWasm, webgpuProblem } from "./wasm";
import { cssColor, onThemeChange } from "./theme";

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

  constructor(private cfg: HostConfig) {
    this.ready = this.init();
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

  async create() {
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
      // Wait for the page's readout, or at least for the GPU, before stepping again.
      const after = this.cfg.afterSteps?.(sim);
      const wait = after instanceof Promise ? after : (sim.sync() as Promise<void>);
      this.busy = true;
      wait.finally(() => (this.busy = false));
    }
    sim.render();
  }
}

/** A tiny DOM helper. */
export const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

/** Read a CSS custom property. */
export const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
