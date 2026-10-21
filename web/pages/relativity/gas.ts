// A relativistic gas of hard disks in 2D, in a periodic box, in units where c = 1 and the rest
// mass m = 1 (so energies are in mc², speeds in c). Each disk has momentum p, energy
// E = √(1 + p²) and velocity p/E < 1. Time steps move the disks; two that touch and are
// approaching collide: boost both into their centre-of-momentum frame, reverse their momenta's
// component along the line of centres there, and boost back. That keeps total energy and
// momentum exactly (up to rounding), whatever the speeds.

export class RelGas {
  readonly n: number;
  readonly L: number;
  readonly x: Float64Array;
  readonly y: Float64Array;
  readonly px: Float64Array;
  readonly py: Float64Array;
  /** How far each disk has travelled across the periodic box (whole box lengths), for unwrapping. */
  readonly wrapX: Int32Array;
  /** For the pressure (virial): Σ (p·v) dt over the steps, Σ r·Δp over the collisions, and the
   * time they cover. */
  private pvDt = 0;
  private rDp = 0;
  private virialTime = 0;
  private seed: number;
  private head: Int32Array;
  private next: Int32Array;
  private cells: number;

  /** `n` disks (n even) of diameter 1 in a box `L` wide, each with energy `e` (mc²), in
   * opposite pairs so that the total momentum is zero. */
  constructor(n: number, L: number, e: number, seed = 1) {
    this.n = n;
    this.L = L;
    this.seed = seed;
    [this.x, this.y, this.px, this.py] = [new Float64Array(n), new Float64Array(n), new Float64Array(n), new Float64Array(n)];
    this.wrapX = new Int32Array(n);
    const side = Math.ceil(Math.sqrt(n));
    const p = Math.sqrt(e * e - 1);
    for (let i = 0; i < n; i++) {
      this.x[i] = ((i % side) + 0.5) * (L / side);
      this.y[i] = (Math.floor(i / side) + 0.5) * (L / side);
      if (i % 2 === 0) {
        const a = 2 * Math.PI * this.rand();
        this.px[i] = p * Math.cos(a);
        this.py[i] = p * Math.sin(a);
      } else {
        this.px[i] = -this.px[i - 1];
        this.py[i] = -this.py[i - 1];
      }
    }
    this.cells = Math.floor(L);
    this.head = new Int32Array(this.cells * this.cells);
    this.next = new Int32Array(n);
  }

  rand() {
    this.seed = (this.seed + 0x6d2b79f5) | 0;
    let t = Math.imul(this.seed ^ (this.seed >>> 15), 1 | this.seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  energyOf(i: number) {
    return Math.sqrt(1 + this.px[i] * this.px[i] + this.py[i] * this.py[i]);
  }

  /** Total energy and momentum. */
  totals() {
    let [e, px, py] = [0, 0, 0];
    for (let i = 0; i < this.n; i++) {
      e += this.energyOf(i);
      px += this.px[i];
      py += this.py[i];
    }
    return { e, px, py };
  }

  /** Advance by `steps` steps of `dt`, with a force `f` (along x) on every disk. */
  step(steps: number, dt: number, f = 0) {
    const { n, L, x, y, px, py } = this;
    for (let s = 0; s < steps; s++) {
      for (let i = 0; i < n; i++) {
        px[i] += f * dt;
        const e = Math.sqrt(1 + px[i] * px[i] + py[i] * py[i]);
        this.pvDt += ((px[i] * px[i] + py[i] * py[i]) / e) * dt;
        x[i] += (px[i] / e) * dt;
        y[i] += (py[i] / e) * dt;
        if (x[i] >= L) {
          x[i] -= L;
          this.wrapX[i]++;
        } else if (x[i] < 0) {
          x[i] += L;
          this.wrapX[i]--;
        }
        if (y[i] >= L) y[i] -= L;
        else if (y[i] < 0) y[i] += L;
      }
      this.collide();
      this.virialTime += dt;
    }
  }

  /** Pressure times area since the last call (2D virial: pA = ½ (⟨Σ p·v⟩ + Σ r·Δp / time)),
   * for a gas at rest. */
  pressureArea() {
    const pa = (this.pvDt + this.rDp) / (2 * this.virialTime);
    this.pvDt = this.rDp = this.virialTime = 0;
    return pa;
  }

  private collide() {
    const { n, L, x, y, cells, head, next } = this;
    const size = L / cells;
    head.fill(-1);
    for (let i = 0; i < n; i++) {
      const c = Math.min(cells - 1, Math.floor(x[i] / size)) * cells + Math.min(cells - 1, Math.floor(y[i] / size));
      next[i] = head[c];
      head[c] = i;
    }
    for (let i = 0; i < n; i++) {
      const cx = Math.min(cells - 1, Math.floor(x[i] / size));
      const cy = Math.min(cells - 1, Math.floor(y[i] / size));
      for (let dx = -1; dx <= 1; dx++)
        for (let dy = -1; dy <= 1; dy++) {
          let j = head[((cx + dx + cells) % cells) * cells + ((cy + dy + cells) % cells)];
          for (; j >= 0; j = next[j]) {
            if (j <= i) continue;
            let rx = x[i] - x[j];
            let ry = y[i] - y[j];
            rx -= L * Math.round(rx / L);
            ry -= L * Math.round(ry / L);
            const r2 = rx * rx + ry * ry;
            if (r2 >= 1 || r2 === 0) continue;
            const ei = this.energyOf(i);
            const ej = this.energyOf(j);
            const vx = this.px[i] / ei - this.px[j] / ej;
            const vy = this.py[i] / ei - this.py[j] / ej;
            if (vx * rx + vy * ry >= 0) continue;
            const r = Math.sqrt(r2);
            const [qx, qy] = [this.px[i], this.py[i]];
            this.bounce(i, j, ei, ej, rx / r, ry / r);
            this.rDp += rx * (this.px[i] - qx) + ry * (this.py[i] - qy);
          }
        }
    }
  }

  /** The collision: in the centre-of-momentum frame, reverse the momenta along (nx, ny). */
  private bounce(i: number, j: number, ei: number, ej: number, nx: number, ny: number) {
    const { px, py } = this;
    const E = ei + ej;
    const bx = (px[i] + px[j]) / E;
    const by = (py[i] + py[j]) / E;
    const b2 = bx * bx + by * by;
    const g = 1 / Math.sqrt(1 - b2);
    // Boost particle i into the CM frame (j's momentum there is the opposite).
    const bp = bx * px[i] + by * py[i];
    const k = (b2 > 1e-30 ? ((g - 1) * bp) / b2 : 0) - g * ei;
    let qx = px[i] + k * bx;
    let qy = py[i] + k * by;
    const eStar = g * (ei - bp);
    // Reverse the component along the line of centres.
    const qn = qx * nx + qy * ny;
    qx -= 2 * qn * nx;
    qy -= 2 * qn * ny;
    // Boost both back (by −β): i has (eStar, q), j has (eStar_j, −q).
    const back = (ex: number, ax: number, ay: number) => {
      const bq = bx * ax + by * ay;
      const kk = (b2 > 1e-30 ? ((g - 1) * bq) / b2 : 0) + g * ex;
      return [ax + kk * bx, ay + kk * by];
    };
    const eStarJ = E / g - eStar;
    [px[i], py[i]] = back(eStar, qx, qy);
    [px[j], py[j]] = back(eStarJ, -qx, -qy);
  }

  /** Scale every momentum by s. */
  scale(s: number) {
    for (let i = 0; i < this.n; i++) {
      this.px[i] *= s;
      this.py[i] *= s;
    }
  }

  /** Add heat q (mc², in all) by scaling every momentum by the same factor (found by
   * bisection). Meant for a gas at rest: it scales any drift too. */
  heat(q: number) {
    const e0 = this.totals().e;
    const e = (s: number) => {
      let t = 0;
      for (let i = 0; i < this.n; i++) t += Math.sqrt(1 + s * s * (this.px[i] ** 2 + this.py[i] ** 2));
      return t;
    };
    let [lo, hi] = [0, 1];
    while (e(hi) < e0 + q) hi *= 2;
    for (let k = 0; k < 60; k++) {
      const m = 0.5 * (lo + hi);
      if (e(m) < e0 + q) lo = m;
      else hi = m;
    }
    this.scale(0.5 * (lo + hi));
  }

  /** The kinetic thermometer: kT = ⟨p·v⟩ / 2 (equipartition in its general form, ⟨p ∂E/∂p⟩ = kT
   * per direction, which holds at any speed). */
  kT() {
    let s = 0;
    for (let i = 0; i < this.n; i++) s += (this.px[i] ** 2 + this.py[i] ** 2) / this.energyOf(i);
    return s / (2 * this.n);
  }
}

/** Mean energy per disk (mc²) in equilibrium at kT (2D Maxwell–Jüttner): 1 + kT + kT²/(1 + kT). */
export const juttnerEnergy = (t: number) => 1 + t + (t * t) / (1 + t);
