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
    // Leave about 44 px per label.
    const every = Math.max(1, Math.ceil(n / Math.max(1, (W - pad.l - pad.r) / 44)));
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

/** Points with error bars and a reference line through the origin. */
export class ScatterPlot {
  private pts: { x: number; y: number; e: number }[] = [];
  private slope = 0;

  constructor(
    private canvas: HTMLCanvasElement,
    private opts: { xLabel: string; yLabel: string; point: () => string; line: () => string; ink: () => string; rule: () => string },
  ) {
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  set(pts: { x: number; y: number; e: number }[], slope: number) {
    this.pts = pts;
    this.slope = slope;
    this.draw();
  }

  draw() {
    const { g, W, H } = prepare(this.canvas);
    const pad = { l: 36, r: 8, t: 14, b: 22 };
    const xMax = Math.max(1e-9, ...this.pts.map((p) => p.x)) * 1.05;
    const yMax = Math.max(1e-9, this.slope * xMax, ...this.pts.map((p) => p.y + p.e)) * 1.05;
    const yMin = Math.min(0, ...this.pts.map((p) => p.y - p.e));
    const x = (v: number) => pad.l + (v / xMax) * (W - pad.l - pad.r);
    const y = (v: number) => H - pad.b - ((v - yMin) / (yMax - yMin)) * (H - pad.t - pad.b);
    g.strokeStyle = this.opts.rule();
    g.beginPath();
    g.moveTo(pad.l, pad.t);
    g.lineTo(pad.l, H - pad.b);
    g.lineTo(W - pad.r, H - pad.b);
    g.stroke();
    g.strokeStyle = this.opts.line();
    g.lineWidth = 2;
    g.beginPath();
    g.moveTo(x(0), y(0));
    g.lineTo(x(xMax), y(this.slope * xMax));
    g.stroke();
    g.lineWidth = 1;
    g.strokeStyle = this.opts.point();
    g.fillStyle = this.opts.point();
    for (const p of this.pts) {
      g.beginPath();
      g.moveTo(x(p.x), y(p.y - p.e));
      g.lineTo(x(p.x), y(p.y + p.e));
      g.stroke();
      g.beginPath();
      g.arc(x(p.x), y(p.y), 3, 0, 2 * Math.PI);
      g.fill();
    }
    g.fillStyle = this.opts.ink();
    g.font = "11px system-ui, sans-serif";
    g.textAlign = "left";
    g.textBaseline = "top";
    g.fillText(this.opts.yLabel, pad.l + 4, 0);
    g.textAlign = "right";
    g.textBaseline = "bottom";
    g.fillText(this.opts.xLabel, W - pad.r, H);
  }
}

export interface Series {
  points: { x: number; y: number }[];
  color: string;
  dashed?: boolean;
  dots?: boolean;
}

/** Several x–y series, optionally with a logarithmic y axis. */
export class LinesPlot {
  private series: Series[] = [];

  constructor(
    private canvas: HTMLCanvasElement,
    private opts: { xLabel: string; yLabel: string; log?: boolean; ink: () => string; rule: () => string; yMin?: number; yMax?: number },
  ) {
    new ResizeObserver(() => this.draw()).observe(canvas);
  }

  set(series: Series[]) {
    this.series = series;
    this.draw();
  }

  draw() {
    const { g, W, H } = prepare(this.canvas);
    const pad = { l: 40, r: 8, t: 14, b: 22 };
    const pts = this.series.flatMap((s) => s.points).filter((p) => Number.isFinite(p.y) && (!this.opts.log || p.y > 0));
    if (!pts.length) return;
    const xMin = Math.min(...pts.map((p) => p.x));
    const xMax = Math.max(...pts.map((p) => p.x));
    const log = !!this.opts.log;
    const tf = (v: number) => (log ? Math.log(v) : v);
    // Default range: on a linear axis, from zero (or the data, if negative) to the data, plus 10%.
    const [dMin, dMax] = [Math.min(...pts.map((p) => p.y)), Math.max(...pts.map((p) => p.y))];
    const [lo, hi] = [Math.min(0, dMin), Math.max(0, dMax)];
    const margin = 0.1 * (hi - lo || 1);
    const yMax = this.opts.yMax ?? (log ? dMax * 1.5 : hi + (dMax > 0 ? margin : 0.25 * margin));
    const yMin = this.opts.yMin ?? (log ? dMin / 1.5 : lo - (dMin < 0 ? margin : 0));
    const x = (v: number) => pad.l + ((v - xMin) / (xMax - xMin || 1)) * (W - pad.l - pad.r);
    const y = (v: number) => H - pad.b - ((tf(Math.max(v, log ? yMin : -Infinity)) - tf(yMin)) / (tf(yMax) - tf(yMin))) * (H - pad.t - pad.b);
    g.strokeStyle = this.opts.rule();
    g.beginPath();
    g.moveTo(pad.l, pad.t);
    g.lineTo(pad.l, H - pad.b);
    g.lineTo(W - pad.r, H - pad.b);
    g.stroke();
    for (const s of this.series) {
      g.strokeStyle = s.color;
      g.fillStyle = s.color;
      g.lineWidth = 2;
      g.setLineDash(s.dashed ? [5, 4] : []);
      const ok = s.points.filter((p) => Number.isFinite(p.y) && (!log || p.y > 0));
      if (s.dots) {
        for (const p of ok) {
          g.beginPath();
          g.arc(x(p.x), y(p.y), 2.5, 0, 2 * Math.PI);
          g.fill();
        }
      } else {
        g.beginPath();
        ok.forEach((p, k) => (k === 0 ? g.moveTo(x(p.x), y(p.y)) : g.lineTo(x(p.x), y(p.y))));
        g.stroke();
      }
    }
    g.setLineDash([]);
    g.lineWidth = 1;
    g.fillStyle = this.opts.ink();
    g.font = "11px system-ui, sans-serif";
    g.textAlign = "left";
    g.textBaseline = "top";
    g.fillText(this.opts.yLabel + (log ? " (log scale)" : ""), pad.l + 4, 0);
    g.textAlign = "right";
    g.textBaseline = "bottom";
    g.fillText(this.opts.xLabel, W - pad.r, H);
  }
}
