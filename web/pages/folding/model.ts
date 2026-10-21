// A lattice protein (the HP model, Lau and Dill 1989, with charges): a chain of residues on a
// square lattice, one per site, each step to a neighbouring site. Hydrophobic residues (H) that
// touch without being bonded gain −1 ε; acidic (D, charge −1 above its pKa) and basic (K, +1 below
// its) residues interact by a screened Coulomb energy, ℓ e^(−r/λ)/r, λ the Debye length (shorter in
// salty water). Monte Carlo: end moves, corner flips, crankshafts and pivots, Metropolis at kT.

export type Residue = "H" | "P" | "D" | "K";

/** A 24-residue chain: a hydrophobic pattern that packs into a core, with six acidic and six
 * basic residues between, which pair up on the outside at neutral pH. */
export const SEQUENCE: Residue[] = [..."DHKHDHKHHDKHHDHKHHKDHDKH"] as Residue[];

/** pKa of the acidic (Asp/Glu-like) and basic (Lys-like) side chains. */
export const PKA = { D: 4.0, K: 10.5 };

export interface Conditions {
  kT: number;
  pH: number;
  /** Debye length (lattice units): long in pure water, short in salt. */
  debye: number;
  /** Strength of a unit charge pair at distance 1 (ε). */
  bjerrum: number;
}

/** Mean charge of each residue at this pH (Henderson–Hasselbalch). */
export function charges(pH: number) {
  return SEQUENCE.map((r) => (r === "D" ? -1 / (1 + 10 ** (PKA.D - pH)) : r === "K" ? 1 / (1 + 10 ** (pH - PKA.K)) : 0));
}

export class Chain {
  x: Int32Array;
  y: Int32Array;
  readonly n = SEQUENCE.length;
  private seed: number;
  q: number[] = charges(7);
  /** A pull on residue `pulled` towards (px, py), strength `force` (ε per site), or none. */
  pull: { i: number; x: number; y: number; force: number } | null = null;

  constructor(seed = 1) {
    this.seed = seed;
    // Start stretched out.
    this.x = Int32Array.from({ length: this.n }, (_, i) => i);
    this.y = new Int32Array(this.n);
  }

  rand() {
    this.seed = (this.seed + 0x6d2b79f5) | 0;
    let t = Math.imul(this.seed ^ (this.seed >>> 15), 1 | this.seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  /** Hydrophobic contacts, and the total energy (ε). */
  energy(c: Conditions) {
    const { n, x, y, q } = this;
    let [contacts, el] = [0, 0];
    for (let i = 0; i < n; i++)
      for (let j = i + 2; j < n; j++) {
        const dx = x[i] - x[j];
        const dy = y[i] - y[j];
        const d2 = dx * dx + dy * dy;
        if (d2 === 1 && SEQUENCE[i] === "H" && SEQUENCE[j] === "H") contacts++;
        if (q[i] !== 0 && q[j] !== 0) {
          const r = Math.sqrt(d2);
          el += (c.bjerrum * q[i] * q[j] * Math.exp(-r / c.debye)) / r;
        }
      }
    let e = -contacts + el;
    if (this.pull) {
      const p = this.pull;
      e += p.force * Math.hypot(x[p.i] - p.x, y[p.i] - p.y);
    }
    return { contacts, energy: e };
  }

  private occupied(skip: number[]) {
    const s = new Set<number>();
    for (let i = 0; i < this.n; i++) if (!skip.includes(i)) s.add(this.x[i] * 65536 + this.y[i]);
    return s;
  }

  /** One attempted move; returns whether it was accepted. */
  move(c: Conditions, e0: number) {
    const { n, x, y } = this;
    const i = Math.floor(this.rand() * n);
    const ox = Int32Array.from(x);
    const oy = Int32Array.from(y);
    const kind = this.rand();
    let ok = false;
    if (kind < 0.15) ok = this.pivot(i);
    else if (i === 0 || i === n - 1) ok = this.endMove(i);
    else if (kind < 0.6) ok = this.corner(i);
    else ok = this.crankshaft(i);
    if (!ok) return { accepted: false, e: e0 };
    const e1 = this.energy(c).energy;
    if (e1 <= e0 || this.rand() < Math.exp(-(e1 - e0) / c.kT)) return { accepted: true, e: e1 };
    x.set(ox);
    y.set(oy);
    return { accepted: false, e: e0 };
  }

  private endMove(i: number) {
    const j = i === 0 ? 1 : this.n - 2;
    const d = [[1, 0], [-1, 0], [0, 1], [0, -1]][Math.floor(this.rand() * 4)];
    const [nx, ny] = [this.x[j] + d[0], this.y[j] + d[1]];
    if (this.occupied([i]).has(nx * 65536 + ny)) return false;
    this.x[i] = nx;
    this.y[i] = ny;
    return true;
  }

  /** i−1, i, i+1 make an L: move i to the opposite corner. */
  private corner(i: number) {
    const { x, y } = this;
    const [ax, ay, bx, by] = [x[i - 1], y[i - 1], x[i + 1], y[i + 1]];
    if (Math.abs(ax - bx) !== 1 || Math.abs(ay - by) !== 1) return false;
    const [nx, ny] = [ax + bx - x[i], ay + by - y[i]];
    if (this.occupied([i]).has(nx * 65536 + ny)) return false;
    x[i] = nx;
    y[i] = ny;
    return true;
  }

  /** i, i+1 make a U with i−1 and i+2: flip it to the other side. */
  private crankshaft(i: number) {
    const { x, y, n } = this;
    if (i + 2 >= n) return false;
    const [ax, ay, dx, dy] = [x[i - 1], y[i - 1], x[i + 2], y[i + 2]];
    if (Math.abs(ax - dx) + Math.abs(ay - dy) !== 1) return false;
    // The U's direction, from i−1 to i.
    const [ux, uy] = [x[i] - ax, y[i] - ay];
    if (x[i + 1] - dx !== ux || y[i + 1] - dy !== uy) return false;
    const occ = this.occupied([i, i + 1]);
    const [n1x, n1y, n2x, n2y] = [ax - ux, ay - uy, dx - ux, dy - uy];
    if (occ.has(n1x * 65536 + n1y) || occ.has(n2x * 65536 + n2y)) return false;
    [x[i], y[i], x[i + 1], y[i + 1]] = [n1x, n1y, n2x, n2y];
    return true;
  }

  /** Rotate the part of the chain after i (or before, at random) about residue i. */
  private pivot(i: number) {
    const { x, y, n } = this;
    const after = this.rand() < 0.5;
    const r = Math.floor(this.rand() * 3) + 1; // 90°, 180°, 270°
    const rot = (dx: number, dy: number): [number, number] => (r === 1 ? [-dy, dx] : r === 2 ? [-dx, -dy] : [dy, -dx]);
    const idx = after ? Array.from({ length: n - i - 1 }, (_, k) => i + 1 + k) : Array.from({ length: i }, (_, k) => k);
    if (!idx.length) return false;
    const moved = new Map<number, [number, number]>();
    for (const k of idx) {
      const [dx, dy] = rot(x[k] - x[i], y[k] - y[i]);
      moved.set(k, [x[i] + dx, y[i] + dy]);
    }
    const occ = this.occupied(idx);
    for (const [, [px, py]] of moved) if (occ.has(px * 65536 + py)) return false;
    for (const [k, [px, py]] of moved) {
      x[k] = px;
      y[k] = py;
    }
    return true;
  }

  /** Radius of gyration (lattice units). */
  rg() {
    const { n, x, y } = this;
    let [mx, my] = [0, 0];
    for (let i = 0; i < n; i++) [mx, my] = [mx + x[i] / n, my + y[i] / n];
    let s = 0;
    for (let i = 0; i < n; i++) s += (x[i] - mx) ** 2 + (y[i] - my) ** 2;
    return Math.sqrt(s / n);
  }
}
