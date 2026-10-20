use crate::cells::{cell_of, CellList};
use crate::real::{PairConsts, Real};
use crate::Energy;
use md_core::walls::{self, CompartmentLedger, Ledger};
use md_core::{fixed, State, System, Wall};

/// The result of a force evaluation.
pub struct Forces {
    pub kicks: Vec<[i32; 2]>,
    pub potential: f64,
    /// Force of each particle on each soft wall (ε/σ), outward.
    pub wall_force: Vec<[f64; walls::WALLS]>,
}

/// `cpu-int-float`: integer positions and velocities, forces in `F`.
///
/// Each pair's kick `round(−f/r · c · d)` is converted to an integer before it is summed, so
/// the total is independent of summation order, and Newton's third law holds bit for bit when
/// masses are equal. The step is exactly reversible (with periodic or hard walls): see
/// [`IntEngine::reverse`].
pub struct IntEngine<F: Real> {
    pub sys: System,
    pub state: State,
    consts: Vec<PairConsts<F>>,
    kick_c: Vec<F>,
    gravity: [i32; 2],
    cells: [u32; 2],
    /// Net steps taken (decreases while running reversed).
    pub steps: i64,
    /// Steps taken in either direction; the counter for stochastic boundaries.
    pub clock: u32,
    /// Wall ledger totals, and per compartment along each wall.
    pub ledger: Ledger,
    pub compartment_ledger: CompartmentLedger,
}

impl<F: Real> IntEngine<F> {
    pub fn new(sys: System, state: State) -> Self {
        let consts = sys.pairs.iter().map(PairConsts::new).collect();
        let kick_c = (0..sys.species() as u32).map(|s| F::from_f64(sys.kick_coeff(s))).collect();
        let cells = sys.cells();
        let gravity = sys.gravity_kick();
        let per_wall = sys.compartment_counts().into_iter().max().unwrap();
        Self { sys, state, consts, kick_c, gravity, cells, steps: 0, clock: 0, ledger: Ledger::default(), compartment_ledger: CompartmentLedger::new(per_wall) }
    }

    fn cell_list(&self) -> CellList {
        let b = self.sys.box_f64();
        let s = &self.state;
        CellList::build(s.n(), self.cells, |i| {
            [cell_of(fixed::to_f64(s.pos[i][0]), b[0], self.cells[0]), cell_of(fixed::to_f64(s.pos[i][1]), b[1], self.cells[1])]
        })
    }

    fn separation(&self, a: i64, b: i64, axis: usize) -> i64 {
        if self.sys.periodic(axis) { fixed::min_image(a, b, self.sys.box_len[axis]) } else { b - a }
    }

    /// Integer kicks at the current positions, and the potential energy.
    pub fn kicks(&self) -> (Vec<[i32; 2]>, f64) {
        let f = self.forces();
        (f.kicks, f.potential)
    }

    /// Integer kicks (pairs and soft walls), the potential energy, and the force on each soft
    /// wall (ε/σ, pushing outward).
    pub fn forces(&self) -> Forces {
        let s = &self.state;
        let b = self.sys.box_f64();
        let ns = self.sys.species();
        let list = self.cell_list();
        let mut kicks = vec![[0i32; 2]; s.n()];
        let mut pe = 0.0f64;
        let mut wall_force = vec![[0.0f64; walls::WALLS]; s.n()];
        for i in 0..s.n() {
            let pi = s.pos[i];
            let si = s.species[i] as usize;
            let c = self.kick_c[si];
            let cell = [cell_of(fixed::to_f64(pi[0]), b[0], self.cells[0]), cell_of(fixed::to_f64(pi[1]), b[1], self.cells[1])];
            let (mut kx, mut ky) = (0i32, 0i32);
            let mut pe_i = F::from_f64(0.0);
            list.for_neighbors(cell, |j| {
                if j == i {
                    return;
                }
                let pj = s.pos[j];
                if self.sys.compartment_of(pi) != self.sys.compartment_of(pj) {
                    return;
                }
                let dx = self.separation(pi[0], pj[0], 0);
                let dy = self.separation(pi[1], pj[1], 1);
                let (Some(fx), Some(fy)) = (F::sep(dx), F::sep(dy)) else { return };
                if let Some((fr, u)) = self.consts[si * ns + s.species[j] as usize].eval(fx, fy) {
                    let a = -fr * c;
                    kx = kx.wrapping_add((a * fx).round_even().to_i32());
                    ky = ky.wrapping_add((a * fy).round_even().to_i32());
                    pe_i += u;
                }
            });
            let mut wall_pe = 0.0;
            for a in 0..2 {
                let Some(ws) = self.sys.walls[a] else { continue };
                for side in 0..2 {
                    if ws[side] != Wall::Soft {
                        continue;
                    }
                    // Distance from the wall, exactly as the GPU computes it.
                    let d = if side == 0 { pi[a] } else { self.sys.box_len[a] - pi[a] };
                    let Some(df) = F::sep(d) else { continue };
                    if df < F::from_f64(walls::SOFT_RC) {
                        let one = F::from_f64(1.0);
                        let s2 = one / (df * df);
                        let s6 = s2 * s2 * s2;
                        let f = F::from_f64(24.0) * s6 * (F::from_f64(2.0) * s6 - one) / df;
                        let k = (f * c).round_even().to_i32();
                        let k = if side == 0 { k } else { k.wrapping_neg() };
                        if a == 0 {
                            kx = kx.wrapping_add(k);
                        } else {
                            ky = ky.wrapping_add(k);
                        }
                        wall_force[i][2 * a + side] += f.to_f64();
                        wall_pe += (F::from_f64(4.0) * s6 * (s6 - one) + one).to_f64();
                    }
                }
            }
            kicks[i] = [kx, ky];
            pe += 0.5 * pe_i.to_f64() + wall_pe;
        }
        Forces { kicks, potential: pe, wall_force }
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            let Forces { kicks, wall_force, .. } = self.forces();
            for (i, wf) in wall_force.iter().enumerate() {
                for w in 0..walls::WALLS {
                    if wf[w] != 0.0 {
                        let c = self.ledger_compartment(i, w / 2);
                        self.ledger.impulse[w] += wf[w] * self.sys.dt;
                        self.compartment_ledger.impulse[w][c] += wf[w] * self.sys.dt;
                    }
                }
            }
            let l = self.sys.box_len;
            for i in 0..self.state.n() {
                let mut v = self.state.vel[i];
                let mut x = self.state.pos[i];
                let home = self.sys.compartment_of(x);
                for a in 0..2 {
                    v[a] = v[a].wrapping_add(kicks[i][a]).wrapping_add(self.gravity[a]);
                }
                let m = self.sys.masses[self.state.species[i] as usize];
                if let Some((vx, vy)) = walls::andersen(&self.sys, i as u32, self.clock, m) {
                    v = [self.sys.to_step_displacement(vx), self.sys.to_step_displacement(vy)];
                }
                for a in 0..2 {
                    x[a] += v[a] as i64;
                }
                for a in 0..2 {
                    match self.sys.walls[a] {
                        None => x[a] = fixed::wrap(x[a], l[a]),
                        Some(ws) => {
                            let (m, side) = walls::mirror(x[a], l[a]);
                            if let Some(side) = side {
                                x[a] = m;
                                self.collide(i, a, side, ws[side], &mut v);
                            } else if self.sys.compartments[a] != 0 {
                                let shift = self.sys.compartments[a];
                                if x[a] >> shift != home[a] {
                                    x[a] = walls::mirror_compartment(x[a], home[a], shift);
                                    v[a] = v[a].wrapping_neg();
                                }
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

    /// A particle crossed wall `side` of `axis`: update its velocity and the ledger.
    fn collide(&mut self, i: usize, axis: usize, side: usize, wall: Wall, v: &mut [i32; 2]) {
        let m = self.sys.masses[self.state.species[i] as usize];
        let w = 2 * axis + side;
        let before = [self.sys.velocity(v[0]), self.sys.velocity(v[1])];
        match wall {
            // A particle can reach a soft wall's plane only at very high speed; reflect it.
            Wall::Hard | Wall::Soft => v[axis] = v[axis].wrapping_neg(),
            Wall::Thermal(t) => {
                let inward = if side == 0 { 1.0 } else { -1.0 };
                let (vn, vt) = walls::thermal_velocity(&self.sys, i as u32, self.clock, t, m, inward);
                v[axis] = self.sys.to_step_displacement(vn);
                v[1 - axis] = self.sys.to_step_displacement(vt);
            }
        }
        let after = [self.sys.velocity(v[0]), self.sys.velocity(v[1])];
        let ke = |u: [f64; 2]| 0.5 * m * (u[0] * u[0] + u[1] * u[1]);
        let (de, dp) = (ke(after) - ke(before), (m * (before[axis] - after[axis])).abs());
        let c = self.ledger_compartment(i, axis);
        self.ledger.energy[w] += de;
        self.ledger.impulse[w] += dp;
        self.ledger.hits[w] += 1;
        self.compartment_ledger.energy[w][c] += de;
        self.compartment_ledger.impulse[w][c] += dp;
        self.compartment_ledger.hits[w][c] += 1;
    }

    /// The ledger slot of particle i at a wall of `axis`: its compartment along the other axis.
    fn ledger_compartment(&self, i: usize, axis: usize) -> usize {
        self.sys.compartment_of(self.state.pos[i])[1 - axis] as usize
    }

    /// Time reversal: `v ← −(v + kick(x) + g)`. Running n steps, reversing, running n steps and
    /// reversing again restores the initial state bit for bit (when every wall is hard).
    pub fn reverse(&mut self) {
        let (kicks, _) = self.kicks();
        for (v, k) in self.state.vel.iter_mut().zip(&kicks) {
            for a in 0..2 {
                v[a] = v[a].wrapping_add(k[a]).wrapping_add(self.gravity[a]).wrapping_neg();
            }
        }
        self.steps = -self.steps;
    }

    /// On-step velocities `v_n = (v_{n−1/2} + v_{n+1/2}) / 2` in σ/τ (ignoring wall hits during
    /// the next half step).
    pub fn velocities(&self) -> Vec<[f64; 2]> {
        let (kicks, _) = self.kicks();
        let s = &self.state;
        (0..s.n())
            .map(|i| {
                let v = |a: usize| {
                    let next = s.vel[i][a].wrapping_add(kicks[i][a]).wrapping_add(self.gravity[a]);
                    0.5 * (self.sys.velocity(s.vel[i][a]) + self.sys.velocity(next))
                };
                [v(0), v(1)]
            })
            .collect()
    }

    /// Kinetic energy at the current (on-step) time and the potential energy (pair energy
    /// plus gravitational energy relative to the origin).
    pub fn energy(&self) -> Energy {
        let v = self.velocities();
        let (_, mut potential) = self.kicks();
        let s = &self.state;
        let mut kinetic = 0.0;
        for i in 0..s.n() {
            let m = self.sys.masses[s.species[i] as usize];
            kinetic += 0.5 * m * (v[i][0] * v[i][0] + v[i][1] * v[i][1]);
            for a in 0..2 {
                potential -= m * self.sys.gravity[a] * fixed::to_f64(s.pos[i][a]);
            }
        }
        Energy { kinetic, potential }
    }
}
