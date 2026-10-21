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
  energy?: false | { work?: (sim: DiscSim) => Promise<number>; workLabel?: string };
}

/**
 * Owns one DiscSim on a canvas: creation, the run loop, theme colours and the "no WebGPU"
 * message.
 *
 * Two ways to run:
 * - **Slow motion** (the default): each animation frame runs the page's `stepsPerFrame`, then
 *   waits for the page's readout before the next frame.
 * - **Full speed**: the simulation runs underneath as fast as the GPU allows. A pump keeps two
 *   batches of steps in flight, each sized to about 10 ms of GPU work. Every animation frame
 *   draws straight from the GPU buffers (no readback). A few times a second the pump pauses
 *   briefly while the page reads out and acts (its `afterSteps`), so page logic never races the
 *   steps.
 */
export class SimHost {
  sim: DiscSim | null = null;
  running = false;
  /** Steps run since creation, in either direction. */
  elapsed = 0;
  /** Full speed (see the class comment) rather than slow motion. */
  fullSpeed = false;
  private frameBusy = false;
  private paused = false;
  // Full speed: batches in flight, the batch size (steps), and the sampling pause.
  private inflight = 0;
  private batch = 200;
  private sampling = false;
  private lastSample = 0;
  private lastDone = 0;
  // Speed readout: steps counted over a window.
  private rate = { t0: 0, steps0: 0 };
  private speedLabel: HTMLElement | null = null;
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
    this.addSpeedControl();
    this.ready = this.init();
  }

  /** Whether anything is in flight (a frame's readout, batches, a sample). */
  private get busy() {
    return this.frameBusy || this.inflight > 0 || this.sampling;
  }

  /** "Slow motion / Full speed" buttons and a live speed readout, under the page's buttons. */
  private addSpeedControl() {
    const panel = this.cfg.canvas.closest(".lab")?.querySelector("aside.panel");
    if (!panel) return;
    const box = document.createElement("div");
    box.className = "speedmode";
    const slow = document.createElement("button");
    const fast = document.createElement("button");
    slow.textContent = "Slow motion";
    fast.textContent = "Full speed";
    const label = document.createElement("span");
    label.className = "speedlabel";
    const show = () => {
      slow.classList.toggle("on", !this.fullSpeed);
      fast.classList.toggle("on", this.fullSpeed);
    };
    slow.onclick = () => {
      this.fullSpeed = false;
      show();
    };
    fast.onclick = () => {
      this.fullSpeed = true;
      show();
      this.pump();
    };
    show();
    box.append(slow, fast, label);
    this.speedLabel = label;
    const buttons = panel.querySelector(".buttons");
    if (buttons) buttons.after(box);
    else panel.prepend(box);
  }

  /** Full speed: keep two batches in flight; size them to ~10 ms of GPU work each. */
  private pump() {
    const sim = this.sim;
    while (sim && this.sim === sim && this.running && this.fullSpeed && !this.paused && !this.sampling && !this.frameBusy && this.inflight < 2) {
      const k = Math.round(this.batch);
      const t0 = performance.now();
      sim.step(k);
      this.elapsed += k;
      this.inflight++;
      (sim.sync() as Promise<void>).then(() => {
        this.inflight--;
        const now = performance.now();
        // With two batches queued, the time between completions is the GPU's time for one.
        const dt = now - Math.max(t0, this.lastDone);
        this.lastDone = now;
        if (dt < 7) this.batch = Math.min(200000, this.batch * 1.4);
        else if (dt > 14) this.batch = Math.max(20, this.batch / 1.4);
        this.pump();
      });
    }
  }

  /** Full speed: pause the pump, let the page read out and act, resume. */
  private async sample(sim: DiscSim) {
    this.sampling = true;
    while (this.inflight > 0) await new Promise((r) => setTimeout(r, 1));
    try {
      if (this.sim === sim) {
        await this.cfg.afterSteps?.(sim);
        if (this.energy) await this.energy.sample(sim);
      }
    } finally {
      this.sampling = false;
      this.pump();
    }
  }

  /** Steps per second and τ per second, over the last half second or so. */
  private updateSpeedLabel(sim: DiscSim) {
    const now = performance.now();
    if (!this.speedLabel || now - this.rate.t0 < 500) return;
    const rate = ((this.elapsed - this.rate.steps0) / (now - this.rate.t0)) * 1000;
    this.rate = { t0: now, steps0: this.elapsed };
    this.speedLabel.textContent = this.running ? `${Math.round(rate).toLocaleString()} steps/s · ${(rate * sim.dt()).toFixed(rate * sim.dt() < 10 ? 1 : 0)} τ/s` : "";
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
    this.rate = { t0: performance.now(), steps0: 0 };
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

  /** Rewrite the ensemble line and the "About this simulation" section, after a change to the
   * system (a wall made a heat bath, say). */
  refreshNotes() {
    if (!this.sim) return;
    showAbout(this.sim, this.cfg.names, this.cfg.label);
    this.energy?.setNote(this.sim.energy_note());
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
    this.updateSpeedLabel(sim);
    if (this.fullSpeed) {
      if (this.running) {
        this.pump();
        const now = performance.now();
        if (!this.sampling && now - this.lastSample > 200) {
          this.lastSample = now;
          void this.sample(sim);
        }
      }
      sim.render();
      return;
    }
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
      this.frameBusy = true;
      wait.finally(() => (this.frameBusy = false));
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
