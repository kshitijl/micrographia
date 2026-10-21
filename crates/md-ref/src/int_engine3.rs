//! `cpu-int-float` in 3D: the CPU twin of `md::GpuEngine3`.

use crate::real::{PairConsts, Real};
use crate::Energy;
use md_core::three::{self, State3, System3, BATH3, WALLS3};
use md_core::walls::{self, SOFT_RC};
use md_core::{fixed, Wall};

/// Integer positions and velocities, forces in `F`, in 3D. As in 2D, each pair's kick is rounded
/// to an integer before it is summed, so the result does not depend on summation order and the
/// step is exactly reversible (with periodic or hard walls).
pub struct IntEngine3<F: Real> {
    pub sys: System3,
    pub state: State3,
    consts: Vec<PairConsts<F>>,
    kick_c: Vec<F>,
    gravity: [i32; 3],
    cells: [u32; 3],
    pub steps: i64,
    /// Steps taken in either direction; the counter for stochastic boundaries.
    pub clock: u32,
    /// Energy given by each wall and the bath (slot 6), impulse given to each wall, and hits.
    pub energy: [f64; WALLS3],
    pub impulse: [f64; WALLS3],
    pub hits: [u64; WALLS3],
}

impl<F: Real> IntEngine3<F> {
    pub fn new(sys: System3, state: State3) -> Self {
        let consts = sys.pairs.iter().map(PairConsts::new).collect();
        let kick_c = (0..sys.species() as u32).map(|s| F::from_f64(sys.kick_coeff(s))).collect();
        let cells = sys.cells_for(sys.rc_max());
        let gravity = sys.gravity_kick();
        Self { sys, state, consts, kick_c, gravity, cells, steps: 0, clock: 0, energy: [0.0; WALLS3], impulse: [0.0; WALLS3], hits: [0; WALLS3] }
    }

    fn cell_of(&self, p: [i64; 3]) -> [u32; 3] {
        let b = self.sys.box_f64();
        std::array::from_fn(|a| crate::cells::cell_of(fixed::to_f64(p[a]), b[a], self.cells[a]))
    }

    fn separation(&self, a: i64, b: i64, axis: usize) -> i64 {
        if self.sys.periodic(axis) { fixed::min_image(a, b, self.sys.box_len[axis]) } else { b - a }
    }

    /// Integer kicks (pairs and soft walls), the potential energy, and each particle's force on
    /// the soft walls (ε/σ, outward, per wall).
    pub fn forces(&self) -> (Vec<[i32; 3]>, f64, Vec<[f64; 6]>) {
        let s = &self.state;
        let ns = self.sys.species();
        let [nx, ny, nz] = self.cells;
        let mut start = vec![0usize; (nx * ny * nz) as usize + 1];
        let keys: Vec<usize> = (0..s.n())
            .map(|i| {
                let c = self.cell_of(s.pos[i]);
                ((c[2] * ny + c[1]) * nx + c[0]) as usize
            })
            .collect();
        for &k in &keys {
            start[k + 1] += 1;
        }
        for c in 0..start.len() - 1 {
            start[c + 1] += start[c];
        }
        let mut fill = start.clone();
        let mut items = vec![0usize; s.n()];
        for (i, &k) in keys.iter().enumerate() {
            items[fill[k]] = i;
            fill[k] += 1;
        }
        let mut kicks = vec![[0i32; 3]; s.n()];
        let mut wall_force = vec![[0.0f64; 6]; s.n()];
        let mut pe = 0.0;
        for i in 0..s.n() {
            let pi = s.pos[i];
            let si = s.species[i] as usize;
            let c = self.kick_c[si];
            let cc = self.cell_of(pi);
            let mut k = [0i32; 3];
            let mut pe_i = F::from_f64(0.0);
            for oz in 0..3 {
                let z = (cc[2] + nz + oz - 1) % nz;
                for oy in 0..3 {
                    let y = (cc[1] + ny + oy - 1) % ny;
                    for ox in 0..3 {
                        let x = (cc[0] + nx + ox - 1) % nx;
                        let cell = ((z * ny + y) * nx + x) as usize;
                        for &j in &items[start[cell]..start[cell + 1]] {
                            if j == i {
                                continue;
                            }
                            let pj = s.pos[j];
                            let d: [i64; 3] = std::array::from_fn(|a| self.separation(pi[a], pj[a], a));
                            let (Some(fx), Some(fy), Some(fz)) = (F::sep(d[0]), F::sep(d[1]), F::sep(d[2])) else { continue };
                            if let Some((fr, u)) = self.consts[si * ns + s.species[j] as usize].eval3(fx, fy, fz) {
                                let a = -fr * c;
                                k[0] = k[0].wrapping_add((a * fx).round_even().to_i32());
                                k[1] = k[1].wrapping_add((a * fy).round_even().to_i32());
                                k[2] = k[2].wrapping_add((a * fz).round_even().to_i32());
                                pe_i += u;
                            }
                        }
                    }
                }
            }
            let mut wall_pe = 0.0;
            for a in 0..3 {
                let Some(ws) = self.sys.walls[a] else { continue };
                for side in 0..2 {
                    if ws[side] != Wall::Soft {
                        continue;
                    }
                    let d = if side == 0 { pi[a] } else { self.sys.box_len[a] - pi[a] };
                    let Some(df) = F::sep(d) else { continue };
                    if df < F::from_f64(SOFT_RC) {
                        let one = F::from_f64(1.0);
                        let s2 = one / (df * df);
                        let s6 = s2 * s2 * s2;
                        let f = F::from_f64(24.0) * s6 * (F::from_f64(2.0) * s6 - one) / df;
                        let kw = (f * c).round_even().to_i32();
                        k[a] = k[a].wrapping_add(if side == 0 { kw } else { kw.wrapping_neg() });
                        wall_force[i][2 * a + side] += f.to_f64();
                        wall_pe += (F::from_f64(4.0) * s6 * (s6 - one) + one).to_f64();
                    }
                }
            }
            kicks[i] = k;
            pe += 0.5 * pe_i.to_f64() + wall_pe;
        }
        (kicks, pe, wall_force)
    }

    pub fn kicks(&self) -> (Vec<[i32; 3]>, f64) {
        let (k, pe, _) = self.forces();
        (k, pe)
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            let (kicks, _, wall_force) = self.forces();
            for wf in &wall_force {
                for w in 0..6 {
                    self.impulse[w] += wf[w] * self.sys.dt;
                }
            }
            let l = self.sys.box_len;
            for i in 0..self.state.n() {
                let sp = self.state.species[i];
                let pinned = self.sys.is_pinned(sp);
                let mut v = self.state.vel[i];
                for a in 0..3 {
                    v[a] = v[a].wrapping_add(kicks[i][a]).wrapping_add(if pinned { 0 } else { self.gravity[a] });
                }
                let m = self.sys.masses[sp as usize];
                if let Some(u) = three::andersen(&self.sys, i as u32, self.clock, sp) {
                    let before: f64 = v.iter().map(|&x| self.sys.velocity(x).powi(2)).sum();
                    v = u.map(|x| self.sys.to_step_displacement(x));
                    let after: f64 = v.iter().map(|&x| self.sys.velocity(x).powi(2)).sum();
                    self.energy[BATH3] += 0.5 * m * (after - before);
                    self.hits[BATH3] += 1;
                }
                let mut x = self.state.pos[i];
                for a in 0..3 {
                    x[a] += v[a] as i64;
                }
                for a in 0..3 {
                    match self.sys.walls[a] {
                        None => x[a] = fixed::wrap(x[a], l[a]),
                        Some(ws) => {
                            let (mx, side) = walls::mirror(x[a], l[a]);
                            if let Some(side) = side {
                                x[a] = mx;
                                self.collide(i, a, side, ws[side], &mut v);
                            }
                        }
                    }
                }
                self.state.vel[i] = v;
                self.state.pos[i] = x;
            }
            self.steps += 1;
            self.clock = self.clock.wrapping_add(1);
        }
    }

    fn collide(&mut self, i: usize, axis: usize, side: usize, wall: Wall, v: &mut [i32; 3]) {
        let m = self.sys.masses[self.state.species[i] as usize];
        let w = 2 * axis + side;
        let before = v.map(|x| self.sys.velocity(x));
        match wall {
            Wall::Hard | Wall::Soft => v[axis] = v[axis].wrapping_neg(),
            Wall::Thermal(t) => {
                let inward = if side == 0 { 1.0 } else { -1.0 };
                let (vn, vt) = three::thermal_velocity(&self.sys, i as u32, self.clock, t, m, inward);
                let (t1, t2) = ((axis + 1) % 3, (axis + 2) % 3);
                v[axis] = self.sys.to_step_displacement(vn);
                v[t1] = self.sys.to_step_displacement(vt[0]);
                v[t2] = self.sys.to_step_displacement(vt[1]);
            }
        }
        let after = v.map(|x| self.sys.velocity(x));
        let ke = |u: [f64; 3]| 0.5 * m * (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]);
        self.energy[w] += ke(after) - ke(before);
        self.impulse[w] += (m * (before[axis] - after[axis])).abs();
        self.hits[w] += 1;
    }

    /// Time reversal: `v ← −(v + kick(x) + g)`.
    pub fn reverse(&mut self) {
        let (kicks, _) = self.kicks();
        for (i, (v, k)) in self.state.vel.iter_mut().zip(&kicks).enumerate() {
            let pinned = self.sys.is_pinned(self.state.species[i]);
            for a in 0..3 {
                let g = if pinned { 0 } else { self.gravity[a] };
                v[a] = v[a].wrapping_add(k[a]).wrapping_add(g).wrapping_neg();
            }
        }
        self.steps = -self.steps;
    }

    /// On-step kinetic energy and the potential energy (pairs, soft walls and gravity).
    pub fn energy(&self) -> Energy {
        let (kicks, mut potential) = self.kicks();
        let s = &self.state;
        let mut kinetic = 0.0;
        for i in 0..s.n() {
            if self.sys.is_pinned(s.species[i]) {
                continue;
            }
            let m = self.sys.masses[s.species[i] as usize];
            for a in 0..3 {
                let next = s.vel[i][a].wrapping_add(kicks[i][a]).wrapping_add(self.gravity[a]);
                let v = 0.5 * (self.sys.velocity(s.vel[i][a]) + self.sys.velocity(next));
                kinetic += 0.5 * m * v * v;
                potential -= m * self.sys.gravity[a] * fixed::to_f64(s.pos[i][a]);
            }
        }
        Energy { kinetic, potential }
    }
}
