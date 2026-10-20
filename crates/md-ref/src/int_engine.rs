use crate::cells::{cell_of, CellList};
use crate::real::{PairConsts, Real};
use crate::Energy;
use md_core::{fixed, State, System};

/// `cpu-int-float`: integer positions and velocities, forces in `F`.
///
/// Each pair's kick `round(−f/r · c · d)` is converted to an integer before it is summed, so
/// the total is independent of summation order, and Newton's third law holds bit for bit when
/// masses are equal. The step is exactly reversible: see [`IntEngine::reverse`].
pub struct IntEngine<F: Real> {
    pub sys: System,
    pub state: State,
    consts: PairConsts<F>,
    kick_c: Vec<F>,
    cells: [u32; 2],
    /// Net steps taken (decreases while running reversed).
    pub steps: i64,
}

impl<F: Real> IntEngine<F> {
    pub fn new(sys: System, state: State) -> Self {
        let consts = PairConsts::new(&sys.pair);
        let kick_c = (0..sys.masses.len() as u32).map(|s| F::from_f64(sys.kick_coeff(s))).collect();
        let cells = sys.cells();
        Self { sys, state, consts, kick_c, cells, steps: 0 }
    }

    fn cell_list(&self) -> CellList {
        let b = self.sys.box_f64();
        let s = &self.state;
        CellList::build(s.n(), self.cells, |i| {
            [cell_of(fixed::to_f64(s.pos[i][0]), b[0], self.cells[0]), cell_of(fixed::to_f64(s.pos[i][1]), b[1], self.cells[1])]
        })
    }

    /// Integer kicks at the current positions, and the potential energy.
    pub fn kicks(&self) -> (Vec<[i32; 2]>, f64) {
        let s = &self.state;
        let b = self.sys.box_f64();
        let list = self.cell_list();
        let mut kicks = vec![[0i32; 2]; s.n()];
        let mut pe = 0.0f64;
        for i in 0..s.n() {
            let pi = s.pos[i];
            let c = self.kick_c[s.species[i] as usize];
            let cell = [cell_of(fixed::to_f64(pi[0]), b[0], self.cells[0]), cell_of(fixed::to_f64(pi[1]), b[1], self.cells[1])];
            let (mut kx, mut ky) = (0i32, 0i32);
            let mut pe_i = F::from_f64(0.0);
            list.for_neighbors(cell, |j| {
                if j == i {
                    return;
                }
                let pj = s.pos[j];
                let dx = fixed::min_image(pi[0], pj[0], self.sys.box_len[0]);
                let dy = fixed::min_image(pi[1], pj[1], self.sys.box_len[1]);
                let (Some(fx), Some(fy)) = (F::sep(dx), F::sep(dy)) else { return };
                if let Some((fr, u)) = self.consts.eval(fx, fy) {
                    let a = -fr * c;
                    kx = kx.wrapping_add((a * fx).round_even().to_i32());
                    ky = ky.wrapping_add((a * fy).round_even().to_i32());
                    pe_i += u;
                }
            });
            kicks[i] = [kx, ky];
            pe += 0.5 * pe_i.to_f64();
        }
        (kicks, pe)
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            let (kicks, _) = self.kicks();
            let l = self.sys.box_len;
            for i in 0..self.state.n() {
                let v = &mut self.state.vel[i];
                let x = &mut self.state.pos[i];
                for a in 0..2 {
                    v[a] = v[a].wrapping_add(kicks[i][a]);
                    x[a] = fixed::wrap(x[a] + v[a] as i64, l[a]);
                }
            }
            self.steps += 1;
        }
    }

    /// Time reversal: `v ← −(v + kick(x))`. Running n steps, reversing, running n steps and
    /// reversing again restores the initial state bit for bit.
    pub fn reverse(&mut self) {
        let (kicks, _) = self.kicks();
        for (v, k) in self.state.vel.iter_mut().zip(&kicks) {
            for a in 0..2 {
                v[a] = v[a].wrapping_add(k[a]).wrapping_neg();
            }
        }
        self.steps = -self.steps;
    }

    /// Kinetic energy at the current (on-step) time, using `v_n = (v_{n−1/2} + v_{n+1/2}) / 2`,
    /// and the potential energy.
    pub fn energy(&self) -> Energy {
        let (kicks, potential) = self.kicks();
        let s = &self.state;
        let mut kinetic = 0.0;
        for i in 0..s.n() {
            let m = self.sys.masses[s.species[i] as usize];
            for a in 0..2 {
                let v = 0.5 * (self.sys.velocity(s.vel[i][a]) + self.sys.velocity(s.vel[i][a].wrapping_add(kicks[i][a])));
                kinetic += 0.5 * m * v * v;
            }
        }
        Energy { kinetic, potential }
    }

    /// On-step velocities in σ/τ.
    pub fn velocities(&self) -> Vec<[f64; 2]> {
        let (kicks, _) = self.kicks();
        let s = &self.state;
        (0..s.n())
            .map(|i| {
                let v = |a: usize| 0.5 * (self.sys.velocity(s.vel[i][a]) + self.sys.velocity(s.vel[i][a].wrapping_add(kicks[i][a])));
                [v(0), v(1)]
            })
            .collect()
    }
}
