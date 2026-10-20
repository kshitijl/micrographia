/** A small time-series plot on a canvas, with segments coloured by series. */
export interface Point {
  t: number;
  y: number;
  series: number;
}

export class TimePlot {
  private points: Point[] = [];
  private markers: { t: number; label: string }[] = [];

  constructor(
    private canvas: HTMLCanvasElement,
    private opts: { yMin: number; yMax: number; yLabel: string; xLabel: string; colors: () => string[]; ink: () => string; rule: () => string },
  ) {
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  clear(): void {
    this.points = [];
    this.markers = [];
    this.draw();
  }

  add(t: number, y: number, series = 0): void {
    this.points.push({ t, y, series });
  }

  mark(t: number, label: string): void {
    this.markers.push({ t, label });
  }

  draw(): void {
    const c = this.canvas;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(c.clientWidth * dpr));
    const h = Math.max(1, Math.round(c.clientHeight * dpr));
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
    }
    const g = c.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    const W = c.clientWidth;
    const H = c.clientHeight;
    g.clearRect(0, 0, W, H);
    const pad = { l: 34, r: 8, t: 8, b: 22 };
    const tMax = Math.max(1, ...this.points.map((p) => p.t), ...this.markers.map((m) => m.t));
    const x = (t: number) => pad.l + (t / tMax) * (W - pad.l - pad.r);
    const { yMin, yMax } = this.opts;
    const y = (v: number) => pad.t + (1 - (v - yMin) / (yMax - yMin)) * (H - pad.t - pad.b);
    g.strokeStyle = this.opts.rule();
    g.lineWidth = 1;
    g.beginPath();
    g.moveTo(pad.l, pad.t);
    g.lineTo(pad.l, H - pad.b);
    g.lineTo(W - pad.r, H - pad.b);
    g.stroke();
    g.fillStyle = this.opts.ink();
    g.font = "11px system-ui, sans-serif";
    g.textAlign = "right";
    g.textBaseline = "middle";
    g.fillText(String(yMax), pad.l - 4, y(yMax));
    g.fillText(String(yMin), pad.l - 4, y(yMin));
    g.textAlign = "left";
    g.textBaseline = "top";
    g.fillText(this.opts.yLabel, pad.l + 4, pad.t);
    g.textAlign = "right";
    g.textBaseline = "bottom";
    g.fillText(this.opts.xLabel, W - pad.r, H - 2);
    for (const m of this.markers) {
      g.strokeStyle = this.opts.rule();
      g.setLineDash([3, 3]);
      g.beginPath();
      g.moveTo(x(m.t), pad.t);
      g.lineTo(x(m.t), H - pad.b);
      g.stroke();
      g.setLineDash([]);
    }
    const colors = this.opts.colors();
    g.lineWidth = 2;
    for (let i = 1; i < this.points.length; i++) {
      const a = this.points[i - 1];
      const b = this.points[i];
      g.strokeStyle = colors[b.series % colors.length];
      g.beginPath();
      g.moveTo(x(a.t), y(a.y));
      g.lineTo(x(b.t), y(b.y));
      g.stroke();
    }
  }
}

export interface BarOpts {
  labels: string[];
  yLabel: string;
  xLabel?: string;
  color: () => string;
  theoryColor: () => string;
  ink: () => string;
  rule: () => string;
  /** Logarithmic y axis (values must be positive to show). */
  log?: boolean;
  yMax?: number;
}

function prepare(canvas: HTMLCanvasElement) {
  const dpr = window.devicePixelRatio || 1;
  const w = Math.max(1, Math.round(canvas.clientWidth * dpr));
  const h = Math.max(1, Math.round(canvas.clientHeight * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
  }
  const g = canvas.getContext("2d")!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);
  return { g, W: canvas.clientWidth, H: canvas.clientHeight };
}

/** Bars (measured) with optional theory markers, linear or log y. */
export class BarPlot {
  private values: number[] = [];
  private theory: number[] | null = null;

  constructor(
    private canvas: HTMLCanvasElement,
    private opts: BarOpts,
  ) {
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  configure(opts: Partial<BarOpts>) {
    Object.assign(this.opts, opts);
  }

  set(values: number[], theory: number[] | null = null, labels?: string[]) {
    this.values = values;
    this.theory = theory;
    if (labels) this.opts.labels = labels;
    this.draw();
  }

  draw() {
    const { g, W, H } = prepare(this.canvas);
    const pad = { l: 40, r: 8, t: 14, b: 22 };
    const n = Math.max(1, this.values.length);
    const all = [...this.values, ...(this.theory ?? [])].filter((v) => Number.isFinite(v));
    const log = !!this.opts.log;
    const pos = all.filter((v) => v > 0);
    const yMax = this.opts.yMax ?? Math.max(1e-12, ...all) * 1.1;
    const yMin = log ? Math.max(Math.min(...pos, yMax) / 3, yMax * 1e-7) : 0;
    const y = (v: number) => {
      const f = log ? (Math.log(Math.max(v, yMin)) - Math.log(yMin)) / (Math.log(yMax) - Math.log(yMin)) : v / yMax;
      return H - pad.b - Math.max(0, Math.min(1, f)) * (H - pad.t - pad.b);
    };
    const bw = (W - pad.l - pad.r) / n;
    g.strokeStyle = this.opts.rule();
    g.beginPath();
    g.moveTo(pad.l, pad.t);
    g.lineTo(pad.l, H - pad.b);
    g.lineTo(W - pad.r, H - pad.b);
    g.stroke();
    g.fillStyle = this.opts.color();
    this.values.forEach((v, i) => {
      if (!(v > 0)) return;
      const top = y(v);
      g.fillRect(pad.l + i * bw + bw * 0.12, top, bw * 0.76, H - pad.b - top);
    });
    if (this.theory) {
      g.strokeStyle = this.opts.theoryColor();
      g.lineWidth = 2;
      this.theory.forEach((v, i) => {
        if (!(v > 0)) return;
        const yy = y(v);
        g.beginPath();
        g.moveTo(pad.l + i * bw + bw * 0.05, yy);
        g.lineTo(pad.l + (i + 1) * bw - bw * 0.05, yy);
        g.stroke();
      });
      g.lineWidth = 1;
    }
    g.fillStyle = this.opts.ink();
    g.font = "11px system-ui, sans-serif";
    g.textAlign = "center";
    g.textBaseline = "top";
    const every = Math.ceil(n / 16);
    this.opts.labels.forEach((l, i) => {
      if (i % every === 0) g.fillText(l, pad.l + (i + 0.5) * bw, H - pad.b + 4);
    });
    g.textAlign = "left";
    g.fillText(this.opts.yLabel + (log ? " (log scale)" : ""), pad.l + 4, 0);
    if (this.opts.xLabel) {
      g.textAlign = "right";
      g.textBaseline = "bottom";
      g.fillText(this.opts.xLabel, W - pad.r, H);
    }
  }
}
