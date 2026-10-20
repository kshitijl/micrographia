use crate::cells::{cell_of, CellList};
use crate::real::{PairConsts, Real};
use crate::Energy;
use md_core::{fixed, State, System};

/// `cpu-float`: conventional MD with float positions and velocities (σ, σ/τ) in `F`.
pub struct FloatEngine<F: Real> {
    pub sys: System,
    pub pos: Vec<[F; 2]>,
    /// Half-step velocities `v_{n−1/2}`.
    pub vel: Vec<[F; 2]>,
    pub species: Vec<u32>,
    consts: PairConsts<F>,
    box_len: [F; 2],
    inv_mass: Vec<F>,
    dt: F,
    cells: [u32; 2],
}

impl<F: Real> FloatEngine<F> {
    /// Start from the same initial condition as the integer engines.
    pub fn from_state(sys: System, state: &State) -> Self {
        let b = sys.box_f64();
        let pos = state.pos.iter().map(|p| [F::from_f64(fixed::to_f64(p[0])), F::from_f64(fixed::to_f64(p[1]))]).collect();
        let vel = state.vel.iter().map(|v| [F::from_f64(sys.velocity(v[0])), F::from_f64(sys.velocity(v[1]))]).collect();
        Self {
            consts: PairConsts::new(&sys.pair),
            box_len: [F::from_f64(b[0]), F::from_f64(b[1])],
            inv_mass: sys.masses.iter().map(|m| F::from_f64(1.0 / m)).collect(),
            dt: F::from_f64(sys.dt),
            cells: sys.cells(),
            species: state.species.clone(),
            pos,
            vel,
            sys,
        }
    }

    fn min_image(&self, d: F, a: usize) -> F {
        let l = self.box_len[a];
        let half = l * F::from_f64(0.5);
        if d >= half {
            d - l
        } else if d < -half {
            d + l
        } else {
            d
        }
    }

    /// Forces (ε/σ) at the current positions, and the potential energy.
    pub fn forces(&self) -> (Vec<[F; 2]>, f64) {
        let b = self.sys.box_f64();
        let cell = |p: [F; 2]| [cell_of(p[0].to_f64(), b[0], self.cells[0]), cell_of(p[1].to_f64(), b[1], self.cells[1])];
        let list = CellList::build(self.pos.len(), self.cells, |i| cell(self.pos[i]));
        let zero = F::from_f64(0.0);
        let mut forces = vec![[zero; 2]; self.pos.len()];
        let mut pe = 0.0;
        for i in 0..self.pos.len() {
            let pi = self.pos[i];
            let (mut fx_i, mut fy_i, mut pe_i) = (zero, zero, zero);
            list.for_neighbors(cell(pi), |j| {
                if j == i {
                    return;
                }
                let dx = self.min_image(self.pos[j][0] - pi[0], 0);
                let dy = self.min_image(self.pos[j][1] - pi[1], 1);
                if let Some((fr, u)) = self.consts.eval(dx, dy) {
                    fx_i += -fr * dx;
                    fy_i += -fr * dy;
                    pe_i += u;
                }
            });
            forces[i] = [fx_i, fy_i];
            pe += 0.5 * pe_i.to_f64();
        }
        (forces, pe)
    }

    fn wrap(&self, x: F, a: usize) -> F {
        let l = self.box_len[a];
        x - l * (x / l).floor()
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            let (f, _) = self.forces();
            for i in 0..self.pos.len() {
                let k = self.inv_mass[self.species[i] as usize] * self.dt;
                for a in 0..2 {
                    self.vel[i][a] += f[i][a] * k;
                    let x = self.pos[i][a] + self.vel[i][a] * self.dt;
                    self.pos[i][a] = self.wrap(x, a);
                }
            }
        }
    }

    /// `v ← −(v + a Δt)`, the float analogue of the integer reversal (exact only in exact
    /// arithmetic).
    pub fn reverse(&mut self) {
        let (f, _) = self.forces();
        for i in 0..self.pos.len() {
            let k = self.inv_mass[self.species[i] as usize] * self.dt;
            for a in 0..2 {
                self.vel[i][a] = -(self.vel[i][a] + f[i][a] * k);
            }
        }
    }

    pub fn energy(&self) -> Energy {
        let (f, potential) = self.forces();
        let mut kinetic = 0.0;
        for i in 0..self.pos.len() {
            let m = self.sys.masses[self.species[i] as usize];
            let k = (self.inv_mass[self.species[i] as usize] * self.dt).to_f64();
            for a in 0..2 {
                let v = self.vel[i][a].to_f64() + 0.5 * f[i][a].to_f64() * k;
                kinetic += 0.5 * m * v * v;
            }
        }
        Energy { kinetic, potential }
    }

    pub fn positions_f64(&self) -> Vec<[f64; 2]> {
        self.pos.iter().map(|p| [p[0].to_f64(), p[1].to_f64()]).collect()
    }
}
