//! Gibbs-ensemble Monte Carlo in 2D (Panagiotopoulos, Mol. Phys. 61, 813, 1987; Frenkel & Smit,
//! *Understanding Molecular Simulation*, §8.3 and Algorithms 18–20).
//!
//! Two periodic square boxes at one temperature, with the total number of molecules and total
//! area fixed. Three kinds of move, each accepted with the Metropolis rule for the Gibbs
//! ensemble: a molecule moves within its box (equilibrium inside each box), area passes from one
//! box to the other (equal pressures), and a molecule moves from one box to a random place in
//! the other (equal chemical potentials). Below the critical point the boxes settle as one
//! liquid and one vapour, at the coexisting densities, with no interface between them.

/// A small fast generator (SplitMix64); the method needs many random numbers, not
/// reproducibility across engines.
#[derive(Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.uniform() * n as f64) as usize
    }
}

/// The pair potential: Lennard-Jones cut and shifted at `rc` (the molecular-dynamics pages'
/// model, so the coexistence curves can be compared).
#[derive(Clone, Copy, Debug)]
pub struct Lj {
    pub rc: f64,
    shift: f64,
}

impl Lj {
    pub fn new(rc: f64) -> Self {
        let s6 = rc.powi(-6);
        Self { rc, shift: 4.0 * s6 * (s6 - 1.0) }
    }

    #[inline]
    pub fn energy(&self, r2: f64) -> f64 {
        if r2 >= self.rc * self.rc {
            return 0.0;
        }
        let s6 = 1.0 / (r2 * r2 * r2);
        4.0 * s6 * (s6 - 1.0) - self.shift
    }

    /// r · dU/dr, for the virial pressure.
    #[inline]
    pub fn r_du(&self, r2: f64) -> f64 {
        if r2 >= self.rc * self.rc {
            return 0.0;
        }
        let s6 = 1.0 / (r2 * r2 * r2);
        -24.0 * s6 * (2.0 * s6 - 1.0)
    }
}

/// One periodic square box with a cell list.
#[derive(Clone, Debug)]
pub struct GBox {
    pub side: f64,
    pub pos: Vec<[f64; 2]>,
    /// Cells per side (side ≥ rc each), and the molecules in each.
    ncell: usize,
    cells: Vec<Vec<usize>>,
}

impl GBox {
    fn new(side: f64, pos: Vec<[f64; 2]>, rc: f64) -> Self {
        let mut b = Self { side, pos, ncell: 1, cells: vec![] };
        b.rebuild(rc);
        b
    }

    pub fn area(&self) -> f64 {
        self.side * self.side
    }

    pub fn n(&self) -> usize {
        self.pos.len()
    }

    pub fn density(&self) -> f64 {
        self.n() as f64 / self.area()
    }

    fn rebuild(&mut self, rc: f64) {
        self.ncell = ((self.side / rc).floor() as usize).max(1);
        self.cells = vec![vec![]; self.ncell * self.ncell];
        for i in 0..self.pos.len() {
            let c = self.cell_of(self.pos[i]);
            self.cells[c].push(i);
        }
    }

    fn cell_of(&self, p: [f64; 2]) -> usize {
        let k = |x: f64| ((x / self.side * self.ncell as f64) as usize).min(self.ncell - 1);
        k(p[1]) * self.ncell + k(p[0])
    }

    /// Energy of a molecule at `p` with every molecule in the box except `skip`.
    fn energy_at(&self, p: [f64; 2], skip: usize, lj: &Lj) -> f64 {
        let l = self.side;
        let mut u = 0.0;
        let mut each = |j: usize| {
            if j == skip {
                return;
            }
            let mut dx = self.pos[j][0] - p[0];
            let mut dy = self.pos[j][1] - p[1];
            dx -= l * (dx / l).round();
            dy -= l * (dy / l).round();
            u += lj.energy(dx * dx + dy * dy);
        };
        if self.ncell < 3 {
            (0..self.pos.len()).for_each(&mut each);
        } else {
            let n = self.ncell as i64;
            let c = self.cell_of(p);
            let (cx, cy) = ((c % self.ncell) as i64, (c / self.ncell) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let k = (((cy + dy).rem_euclid(n)) * n + (cx + dx).rem_euclid(n)) as usize;
                    for &j in &self.cells[k] {
                        each(j);
                    }
                }
            }
        }
        u
    }

    /// Total energy and virial Σ r·dU/dr, with every position scaled by `s` (for trial areas).
    fn totals(&self, lj: &Lj, s: f64) -> (f64, f64) {
        let scaled = GBox::new(self.side * s, self.pos.iter().map(|p| [p[0] * s, p[1] * s]).collect(), lj.rc);
        let l = scaled.side;
        let (mut u, mut w) = (0.0, 0.0);
        for i in 0..scaled.n() {
            let p = scaled.pos[i];
            let mut each = |j: usize| {
                if j <= i {
                    return;
                }
                let mut dx = scaled.pos[j][0] - p[0];
                let mut dy = scaled.pos[j][1] - p[1];
                dx -= l * (dx / l).round();
                dy -= l * (dy / l).round();
                let r2 = dx * dx + dy * dy;
                u += lj.energy(r2);
                w += lj.r_du(r2);
            };
            if scaled.ncell < 3 {
                (0..scaled.n()).for_each(&mut each);
            } else {
                let n = scaled.ncell as i64;
                let c = scaled.cell_of(p);
                let (cx, cy) = ((c % scaled.ncell) as i64, (c / scaled.ncell) as i64);
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let k = (((cy + dy).rem_euclid(n)) * n + (cx + dx).rem_euclid(n)) as usize;
                        for &j in &scaled.cells[k] {
                            each(j);
                        }
                    }
                }
            }
        }
        (u, w)
    }

    fn move_to(&mut self, i: usize, p: [f64; 2]) {
        let (a, b) = (self.cell_of(self.pos[i]), self.cell_of(p));
        if a != b {
            let k = self.cells[a].iter().position(|&j| j == i).unwrap();
            self.cells[a].swap_remove(k);
            self.cells[b].push(i);
        }
        self.pos[i] = p;
    }

    fn insert(&mut self, p: [f64; 2]) {
        let c = self.cell_of(p);
        self.cells[c].push(self.pos.len());
        self.pos.push(p);
    }

    /// Remove molecule i (the last one takes its index).
    fn remove(&mut self, i: usize) {
        let last = self.pos.len() - 1;
        let ci = self.cell_of(self.pos[i]);
        let k = self.cells[ci].iter().position(|&j| j == i).unwrap();
        self.cells[ci].swap_remove(k);
        if i != last {
            let cl = self.cell_of(self.pos[last]);
            let k = self.cells[cl].iter().position(|&j| j == last).unwrap();
            self.cells[cl][k] = i;
            self.pos[i] = self.pos[last];
        }
        self.pos.pop();
    }
}

/// Cycles of step-size tuning after a start or a change of temperature.
pub const TUNE_CYCLES: u64 = 300;

/// Acceptance counts: (tried, accepted) per kind of move.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub moves: (u64, u64),
    pub areas: (u64, u64),
    pub transfers: (u64, u64),
}

pub struct Gibbs {
    pub t: f64,
    pub lj: Lj,
    pub boxes: [GBox; 2],
    pub rng: Rng,
    /// Largest displacement per box (σ), and largest change of ln(A₁/A₂); tuned towards ~40%
    /// acceptance as it runs.
    pub max_move: [f64; 2],
    pub max_dln: f64,
    pub stats: Stats,
    pub cycles: u64,
    /// Cycles left in which step sizes are still tuned (after a start or a change of
    /// temperature); afterwards they stay fixed, so the moves keep detailed balance exactly.
    pub tuning: u64,
}

impl Gibbs {
    /// `n` molecules at overall density `density`, split equally between two equal boxes, on
    /// jittered square lattices, at temperature `t`.
    pub fn new(n: usize, density: f64, t: f64, rc: f64, seed: u64) -> Self {
        let mut rng = Rng(seed);
        let side = (0.5 * n as f64 / density).sqrt();
        let lj = Lj::new(rc);
        let make = |rng: &mut Rng, count: usize| {
            let k = (count as f64).sqrt().ceil() as usize;
            let a = side / k as f64;
            let pos = (0..count).map(|q| [((q % k) as f64 + 0.5 + 0.2 * (rng.uniform() - 0.5)) * a, ((q / k) as f64 + 0.5 + 0.2 * (rng.uniform() - 0.5)) * a]).collect();
            GBox::new(side, pos, rc)
        };
        let b0 = make(&mut rng, n / 2);
        let b1 = make(&mut rng, n - n / 2);
        Self { t, lj, boxes: [b0, b1], rng, max_move: [0.2, 0.2], max_dln: 0.02, stats: Stats::default(), cycles: 0, tuning: TUNE_CYCLES }
    }

    /// Change the temperature (and tune the step sizes again for a while).
    pub fn set_temperature(&mut self, t: f64) {
        self.t = t;
        self.tuning = TUNE_CYCLES;
    }

    pub fn n(&self) -> usize {
        self.boxes[0].n() + self.boxes[1].n()
    }

    fn displace(&mut self) {
        let b = self.rng.below(2);
        if self.boxes[b].n() == 0 {
            return;
        }
        let i = self.rng.below(self.boxes[b].n());
        let bx = &self.boxes[b];
        let p = bx.pos[i];
        let d = self.max_move[b];
        let l = bx.side;
        let q = [(p[0] + d * (2.0 * self.rng.uniform() - 1.0)).rem_euclid(l), (p[1] + d * (2.0 * self.rng.uniform() - 1.0)).rem_euclid(l)];
        let du = bx.energy_at(q, i, &self.lj) - bx.energy_at(p, i, &self.lj);
        self.stats.moves.0 += 1;
        if du <= 0.0 || self.rng.uniform() < (-du / self.t).exp() {
            self.boxes[b].move_to(i, q);
            self.stats.moves.1 += 1;
        }
    }

    /// Area passes between the boxes: a random walk in ln(A₁/A₂) (Frenkel & Smit, Algorithm 18).
    fn change_areas(&mut self) {
        let (a0, a1) = (self.boxes[0].area(), self.boxes[1].area());
        let total = a0 + a1;
        let ln = (a0 / a1).ln() + self.max_dln * (2.0 * self.rng.uniform() - 1.0);
        let n0 = total * ln.exp() / (1.0 + ln.exp());
        let n1 = total - n0;
        // Boxes must stay at least two cutoffs wide.
        if n0.sqrt() < 2.0 * self.lj.rc || n1.sqrt() < 2.0 * self.lj.rc {
            return;
        }
        let s = [(n0 / a0).sqrt(), (n1 / a1).sqrt()];
        let (u0, _) = self.boxes[0].totals(&self.lj, 1.0);
        let (u1, _) = self.boxes[1].totals(&self.lj, 1.0);
        let (v0, _) = self.boxes[0].totals(&self.lj, s[0]);
        let (v1, _) = self.boxes[1].totals(&self.lj, s[1]);
        let (k0, k1) = (self.boxes[0].n() as f64, self.boxes[1].n() as f64);
        let arg = -(v0 + v1 - u0 - u1) / self.t + (k0 + 1.0) * (n0 / a0).ln() + (k1 + 1.0) * (n1 / a1).ln();
        self.stats.areas.0 += 1;
        if arg >= 0.0 || self.rng.uniform() < arg.exp() {
            for b in 0..2 {
                let bx = &mut self.boxes[b];
                bx.side *= s[b];
                for p in bx.pos.iter_mut() {
                    p[0] = (p[0] * s[b]).min(bx.side * (1.0 - 1e-12));
                    p[1] = (p[1] * s[b]).min(bx.side * (1.0 - 1e-12));
                }
                bx.rebuild(self.lj.rc);
            }
            self.stats.areas.1 += 1;
        }
    }

    /// A molecule leaves a random box for a random place in the other (Algorithm 19).
    fn transfer(&mut self) {
        let from = self.rng.below(2);
        let to = 1 - from;
        let nf = self.boxes[from].n();
        if nf == 0 {
            return;
        }
        let i = self.rng.below(nf);
        let l = self.boxes[to].side;
        let q = [self.rng.uniform() * l, self.rng.uniform() * l];
        let u_in = self.boxes[to].energy_at(q, usize::MAX, &self.lj);
        let u_out = self.boxes[from].energy_at(self.boxes[from].pos[i], i, &self.lj);
        let nt = self.boxes[to].n() as f64;
        let ratio = (nf as f64 * self.boxes[to].area()) / ((nt + 1.0) * self.boxes[from].area()) * (-(u_in - u_out) / self.t).exp();
        self.stats.transfers.0 += 1;
        if ratio >= 1.0 || self.rng.uniform() < ratio {
            self.boxes[from].remove(i);
            self.boxes[to].insert(q);
            self.stats.transfers.1 += 1;
        }
    }

    /// `n` cycles. A cycle is N molecule moves, one area change and N/4 transfer attempts, in
    /// random order (Frenkel & Smit's recipe, with more transfers to equalise the chemical
    /// potentials sooner).
    pub fn run(&mut self, cycles: usize) {
        for _ in 0..cycles {
            let n = self.n();
            let k = n + 1 + n / 4;
            for _ in 0..k {
                let r = self.rng.below(k);
                if r < n {
                    self.displace();
                } else if r == n {
                    self.change_areas();
                } else {
                    self.transfer();
                }
            }
            self.cycles += 1;
            if self.tuning > 0 {
                self.tuning -= 1;
                if self.cycles % 10 == 0 {
                    self.tune();
                }
            }
        }
    }

    /// Nudge the step sizes towards ~40% acceptance.
    fn tune(&mut self) {
        let rate = |(t, a): (u64, u64)| if t == 0 { 0.4 } else { a as f64 / t as f64 };
        let m = rate(self.stats.moves);
        for b in 0..2 {
            self.max_move[b] = (self.max_move[b] * if m > 0.4 { 1.05 } else { 0.95 }).clamp(0.02, 0.5 * self.boxes[b].side);
        }
        let a = rate(self.stats.areas);
        self.max_dln = (self.max_dln * if a > 0.4 { 1.05 } else { 0.95 }).clamp(1e-4, 0.5);
    }

    /// (density, pressure) of each box. Pressure from the virial: ρT − (1/2A) Σ r·dU/dr over pairs.
    pub fn measure(&self) -> [(f64, f64); 2] {
        [0, 1].map(|b| {
            let bx = &self.boxes[b];
            let (_, w) = bx.totals(&self.lj, 1.0);
            let rho = bx.density();
            (rho, rho * self.t - w / (2.0 * bx.area()))
        })
    }
}
