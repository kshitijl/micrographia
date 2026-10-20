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
