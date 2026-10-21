//! Three dimensions: the system description, state and initial conditions for the 3D engines.
//!
//! The same units, fixed-point positions and leapfrog as in 2D, with a smaller set of features
//! for now: pair potentials between up to four species, periodic or walled axes (hard, soft or
//! heat-bath walls), gravity, an Andersen heat bath and pinned species. No compartments,
//! membranes, pistons or bonds yet.

use crate::fixed::{self, ONE, UNIT};
use crate::pair::Pair;
use crate::system::{Wall, MAX_SPECIES};
use gpu_prims::threefry;

#[derive(Clone, Debug)]
pub struct System3 {
    /// Box lengths in position units.
    pub box_len: [i64; 3],
    pub dt: f64,
    pub masses: Vec<f64>,
    /// Interaction of each species pair, row-major `[a * ns + b]`, symmetric.
    pub pairs: Vec<Pair>,
    /// Per axis: `None` for periodic, or the walls at 0 and at L.
    pub walls: [Option<[Wall; 2]>; 3],
    /// Uniform acceleration (σ/τ²) on every particle.
    pub gravity: [f64; 3],
    /// Seed for stochastic boundaries.
    pub seed: u64,
    /// A heat bath throughout the volume (Andersen).
    pub thermostat: Option<Bath>,
    /// Bit s set if species s is pinned (no forces, no gravity, ignored by baths).
    pub pinned: u32,
}

/// Andersen heat bath: each step, each particle of the species in `species` (a bitmask) has
/// probability `rate · Δt` of having its velocity redrawn at temperature `t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bath {
    pub t: f64,
    pub rate: f64,
    pub species: u32,
}

/// Walls are numbered `2 * axis + side`; slot 6 books the bulk heat bath, as in 2D.
pub const WALLS3: usize = 7;
pub const BATH3: usize = 6;

impl System3 {
    /// A periodic system in which every species pair interacts through `pair`.
    pub fn new(box_len: [i64; 3], dt: f64, pair: Pair, masses: Vec<f64>) -> Self {
        assert!(!masses.is_empty() && masses.len() <= MAX_SPECIES);
        let ns = masses.len();
        assert!(pair.rc < 7.9, "cutoff must be below 8 σ");
        Self { box_len, dt, pairs: vec![pair; ns * ns], masses, walls: [None; 3], gravity: [0.0; 3], seed: 1, thermostat: None, pinned: 0 }
    }

    pub fn species(&self) -> usize {
        self.masses.len()
    }

    /// Set the interaction between species `a` and `b` (both orders).
    pub fn with_pair(mut self, a: u32, b: u32, pair: Pair) -> Self {
        assert!(pair.rc < 7.9, "cutoff must be below 8 σ");
        let ns = self.species();
        self.pairs[a as usize * ns + b as usize] = pair;
        self.pairs[b as usize * ns + a as usize] = pair;
        self
    }

    pub fn with_walls(mut self, axis: usize, lo: Wall, hi: Wall) -> Self {
        self.walls[axis] = Some([lo, hi]);
        self
    }

    pub fn with_gravity(mut self, g: [f64; 3]) -> Self {
        self.gravity = g;
        self
    }

    /// A heat bath on every species at temperature `t`, `rate` collisions per particle per τ.
    pub fn with_thermostat(mut self, t: f64, rate: f64) -> Self {
        let species = self.thermostat.map_or(u32::MAX, |b| b.species);
        self.thermostat = Some(Bath { t, rate, species });
        self
    }

    pub fn with_pinned(mut self, s: u32) -> Self {
        self.pinned |= 1 << s;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn is_pinned(&self, s: u32) -> bool {
        self.pinned & (1 << s) != 0
    }

    pub fn pair(&self, a: u32, b: u32) -> &Pair {
        &self.pairs[a as usize * self.species() + b as usize]
    }

    pub fn rc_max(&self) -> f64 {
        self.pairs.iter().map(|p| p.rc).fold(0.0, f64::max)
    }

    pub fn periodic(&self, axis: usize) -> bool {
        self.walls[axis].is_none()
    }

    /// True if nothing stochastic happens: the dynamics is then exactly reversible.
    pub fn deterministic(&self) -> bool {
        self.thermostat.is_none() && self.walls.iter().flatten().flatten().all(|w| !matches!(w, Wall::Thermal(_)))
    }

    pub fn has_soft_walls(&self) -> bool {
        self.walls.iter().flatten().flatten().any(|w| *w == Wall::Soft)
    }

    /// The integer gravity kick per step (the same for every species).
    pub fn gravity_kick(&self) -> [i32; 3] {
        self.gravity.map(|g| (g * self.dt * self.dt * ONE as f64).round() as i32)
    }

    pub fn box_f64(&self) -> [f64; 3] {
        self.box_len.map(fixed::to_f64)
    }

    pub fn volume(&self) -> f64 {
        self.box_f64().iter().product()
    }

    /// Force (ε/σ) → integer kick (position units per step) for a species: F Δt² / m.
    pub fn kick_coeff(&self, species: u32) -> f64 {
        if self.is_pinned(species) {
            return 0.0;
        }
        self.dt * self.dt / self.masses[species as usize] * ONE as f64
    }

    /// Velocity (σ/τ) of an integer per-step displacement.
    pub fn velocity(&self, v: i32) -> f64 {
        v as f64 * UNIT / self.dt
    }

    pub fn to_step_displacement(&self, v: f64) -> i32 {
        (v * self.dt * ONE as f64).round() as i32
    }

    /// Cells per axis for a cell list of radius `r` (side ≥ r plus a margin for rounding).
    pub fn cells_for(&self, r: f64) -> [u32; 3] {
        let b = self.box_f64();
        let margin = 1e-3 + 1e-6 * b.iter().cloned().fold(0.0, f64::max);
        let c = b.map(|l| (l / (r + margin)).floor() as u32);
        assert!(c.iter().all(|&k| k >= 3), "box too small for radius {r}: {c:?} cells");
        c
    }

    /// The ensemble in plain words, for the total-energy plot.
    pub fn energy_note(&self) -> String {
        let bath_walls = self.walls.iter().flatten().flatten().any(|w| matches!(w, Wall::Thermal(_)));
        match (self.thermostat.is_some(), bath_walls) {
            (true, _) => "A heat bath (constant temperature, the canonical ensemble) adds and removes energy.".into(),
            (false, true) => "Heat-bath walls let energy in and out.".into(),
            _ => "Constant energy (the microcanonical ensemble): nothing adds or removes energy, so the total stays flat.".into(),
        }
    }

    /// Bullet points describing the method, forces, boundaries and ensemble (as in 2D).
    pub fn describe(&self, n: usize, names: &[String]) -> Vec<String> {
        let ns = self.species();
        let name = |s: usize| names.get(s).cloned().unwrap_or_else(|| format!("species {s}"));
        let mut out = vec![format!(
            "Method: molecular dynamics in 3D. {n} particles follow Newton's laws, integrated with the leapfrog (velocity Verlet) scheme at a time step of {} τ. Positions and velocities are exact integers (2⁻³² σ), so every run can be reversed bit for bit; forces are computed in 32-bit floats on the GPU.",
            crate::describe::num(self.dt)
        )];
        out.push("Units: σ is a molecule's diameter, ε the depth of the attraction, and τ = σ√(m/ε) the time unit; temperatures are given as kT in units of ε.".into());
        let mut groups: Vec<(String, Vec<String>)> = vec![];
        for a in 0..ns {
            for b in a..ns {
                if self.is_pinned(a as u32) && self.is_pinned(b as u32) {
                    continue;
                }
                let phrase = crate::describe::pair_phrase(self.pair(a as u32, b as u32));
                let who = format!("{}–{}", name(a), name(b));
                match groups.iter_mut().find(|(p, _)| *p == phrase) {
                    Some((_, list)) => list.push(who),
                    None => groups.push((phrase, vec![who])),
                }
            }
        }
        if groups.len() == 1 {
            out.push(format!("Forces: between every pair of molecules, {}.", groups[0].0));
        } else {
            let parts: Vec<String> = groups.iter().map(|(p, who)| format!("{p} ({})", who.join(", "))).collect();
            out.push(format!("Forces: {}.", parts.join("; ")));
        }
        let moving: Vec<usize> = (0..ns).filter(|&s| !self.is_pinned(s as u32)).collect();
        if moving.iter().any(|&s| self.masses[s] != self.masses[moving[0]]) {
            let m: Vec<String> = moving.iter().map(|&s| format!("{} {}", name(s), crate::describe::num(self.masses[s]))).collect();
            out.push(format!("Masses: {}.", m.join(", ")));
        }
        let b = self.box_f64().map(crate::describe::num);
        out.push(format!("Box: {} σ × {} σ × {} σ.", b[0], b[1], b[2]));
        for (axis, both) in [(0, "Left and right"), (1, "Bottom and top"), (2, "Front and back")] {
            out.push(match &self.walls[axis] {
                None => format!("{both}: periodic (a molecule leaving one side comes back on the other)."),
                Some([lo, hi]) if lo == hi => format!("{both} walls: {}.", crate::describe::wall_phrase(lo)),
                Some([lo, hi]) => format!("{both} walls: {} and {}.", crate::describe::wall_phrase(lo), crate::describe::wall_phrase(hi)),
            });
        }
        if self.gravity != [0.0; 3] {
            let g = self.gravity.map(crate::describe::num);
            out.push(format!("Gravity: a constant force m·g with g = ({}, {}, {}) σ/τ².", g[0], g[1], g[2]));
        }
        out.push(match &self.thermostat {
            Some(t) => format!(
                "Heat bath (Andersen thermostat) at kT = {}: each molecule has its velocity replaced by a random one at that temperature about {} times per τ. Energy is not conserved: the bath adds and removes it.",
                crate::describe::num(t.t),
                crate::describe::num(t.rate)
            ),
            None if !self.deterministic() => "Ensemble: energy enters and leaves only through the heat-bath walls; otherwise it is conserved.".into(),
            None => "Ensemble: no heat bath; the total energy is conserved (constant N, V, E), apart from tiny integration errors.".into(),
        });
        out
    }
}

/// A heat-bath wall's new velocity (σ/τ) for a particle of mass `m`: the normal component from
/// the Maxwell flux distribution, pointing into the box (`inward` = ±1), and two Gaussian
/// tangential ones. Counter-based on (particle id, step); the GPU draws the same numbers in f32.
pub fn thermal_velocity(sys: &System3, id: u32, clock: u32, t: f64, m: f64, inward: f64) -> (f64, [f64; 2]) {
    let key = [sys.seed as u32, (sys.seed >> 32) as u32 ^ crate::walls::STREAM_WALL];
    let r = threefry::threefry2x32([id, clock], key);
    let r2 = threefry::threefry2x32([id, clock], [key[0], key[1] ^ 1]);
    let u1 = 1.0 - threefry::to_unit_f64(r);
    let (u2, u3) = (1.0 - threefry::to_unit_f64([r2[0], 0]), threefry::to_unit_f64([r2[1], 0]));
    let sd = (t / m).sqrt();
    let normal = inward * sd * (-2.0 * u1.ln()).sqrt();
    let rad = sd * (-2.0 * u2.ln()).sqrt();
    let a = std::f64::consts::TAU * u3;
    (normal, [rad * a.cos(), rad * a.sin()])
}

/// Andersen heat bath: Some(new velocity in σ/τ) if particle `id` of species `s` is redrawn
/// this step.
pub fn andersen(sys: &System3, id: u32, clock: u32, s: u32) -> Option<[f64; 3]> {
    let th = sys.thermostat?;
    if th.species & (1 << s) == 0 || sys.is_pinned(s) {
        return None;
    }
    let key = [sys.seed as u32, (sys.seed >> 32) as u32 ^ crate::walls::STREAM_THERMO];
    let r = threefry::threefry2x32([id, clock], key);
    if threefry::to_unit_f64([r[0], 0]) >= th.rate * sys.dt {
        return None;
    }
    let r2 = threefry::threefry2x32([id, clock], [key[0], key[1] ^ 1]);
    let r3 = threefry::threefry2x32([id, clock], [key[0], key[1] ^ 2]);
    let sd = (th.t / sys.masses[s as usize]).sqrt();
    let rad = (-2.0 * (1.0 - threefry::to_unit_f64([r[1], 0])).ln()).sqrt();
    let a = std::f64::consts::TAU * threefry::to_unit_f64([r2[0], 0]);
    let rad3 = (-2.0 * (1.0 - threefry::to_unit_f64([r3[0], 0])).ln()).sqrt();
    let a3 = std::f64::consts::TAU * threefry::to_unit_f64([r3[1], 0]);
    Some([sd * rad * a.cos(), sd * rad * a.sin(), sd * rad3 * a3.cos()])
}

/// Positions, half-step velocities (displacement per step) and species, in 3D.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State3 {
    pub pos: Vec<[i64; 3]>,
    pub vel: Vec<[i32; 3]>,
    pub species: Vec<u32>,
}

impl State3 {
    pub fn n(&self) -> usize {
        self.pos.len()
    }

    /// FNV-1a over every bit of positions, velocities and species.
    pub fn hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        for i in 0..self.n() {
            for a in 0..3 {
                eat(&self.pos[i][a].to_le_bytes());
                eat(&self.vel[i][a].to_le_bytes());
            }
            eat(&self.species[i].to_le_bytes());
        }
        h
    }

    /// Kinetic energy from the stored half-step velocities.
    pub fn half_step_kinetic(&self, sys: &System3) -> f64 {
        (0..self.n())
            .filter(|&i| !sys.is_pinned(self.species[i]))
            .map(|i| {
                let m = sys.masses[self.species[i] as usize];
                0.5 * m * self.vel[i].iter().map(|&v| sys.velocity(v).powi(2)).sum::<f64>()
            })
            .sum()
    }
}

/// A cubic box holding `n` particles at number density `density`.
pub fn cube_for(n: usize, density: f64) -> [i64; 3] {
    let l = (n as f64 / density).cbrt();
    [fixed::to_fixed(l); 3]
}

/// The first `n` sites of a face-centred cubic lattice of `cells` unit cells per axis filling
/// `box_len`, with a small random jitter. FCC is the crystal Lennard-Jones atoms freeze into.
pub fn fcc(n: usize, box_len: [i64; 3], cells: [usize; 3], jitter: f64, seed: u64) -> Vec<[i64; 3]> {
    let b = box_len.map(fixed::to_f64);
    let a = [b[0] / cells[0] as f64, b[1] / cells[1] as f64, b[2] / cells[2] as f64];
    let basis = [[0.25, 0.25, 0.25], [0.75, 0.75, 0.25], [0.75, 0.25, 0.75], [0.25, 0.75, 0.75]];
    let mut out = Vec::with_capacity(n);
    'fill: for k in 0..cells[2] {
        for j in 0..cells[1] {
            for i in 0..cells[0] {
                for s in &basis {
                    if out.len() == n {
                        break 'fill;
                    }
                    let id = out.len() as u32;
                    let cell = [i as f64, j as f64, k as f64];
                    let p: [i64; 3] = std::array::from_fn(|ax| {
                        let jit = jitter * (threefry::uniform(seed, crate::init::STREAM_JITTER, 3 * id + ax as u32) - 0.5);
                        fixed::wrap(fixed::to_fixed((cell[ax] + s[ax]) * a[ax] + jit), box_len[ax])
                    });
                    out.push(p);
                }
            }
        }
    }
    out
}

/// FCC unit cells per axis to hold at least `n` sites in a box of these proportions.
pub fn fcc_cells(n: usize, box_len: [i64; 3]) -> [usize; 3] {
    let b = box_len.map(fixed::to_f64);
    let a = (b[0] * b[1] * b[2] * 4.0 / n as f64).cbrt();
    let mut c = b.map(|l| ((l / a).round() as usize).max(1));
    while 4 * c[0] * c[1] * c[2] < n {
        let k = (0..3).min_by(|&x, &y| (b[x] / c[x] as f64).total_cmp(&(b[y] / c[y] as f64)).reverse()).unwrap();
        c[k] += 1;
    }
    c
}

/// Maxwell–Boltzmann velocities at temperature `t`, with the velocity sum exactly zero (pinned
/// species get zero).
pub fn maxwell(sys: &System3, species: &[u32], t: f64, seed: u64) -> Vec<[i32; 3]> {
    let mut vel: Vec<[i32; 3]> = species
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            if sys.is_pinned(s) {
                return [0; 3];
            }
            let sd = (t / sys.masses[s as usize]).sqrt();
            std::array::from_fn(|a| sys.to_step_displacement(sd * threefry::normal(seed, crate::init::STREAM_VELOCITY, 3 * i as u32 + a as u32)))
        })
        .collect();
    zero_velocity_sum(sys, species, &mut vel);
    vel
}

/// Subtract the mean (over moving particles) so the integer sum is exactly zero on each axis.
pub fn zero_velocity_sum(sys: &System3, species: &[u32], vel: &mut [[i32; 3]]) {
    let moving: Vec<usize> = (0..vel.len()).filter(|&i| !sys.is_pinned(species[i])).collect();
    let n = moving.len() as i64;
    if n == 0 {
        return;
    }
    for a in 0..3 {
        let sum: i64 = moving.iter().map(|&i| vel[i][a] as i64).sum();
        let (q, r) = (sum.div_euclid(n), sum.rem_euclid(n));
        for (k, &i) in moving.iter().enumerate() {
            vel[i][a] -= (q + ((k as i64) < r) as i64) as i32;
        }
    }
}

/// Rescale velocities so the (half-step) kinetic temperature is `t`, keeping the sum zero.
pub fn set_temperature(state: &mut State3, sys: &System3, t: f64) {
    let moving = (0..state.n()).filter(|&i| !sys.is_pinned(state.species[i])).count();
    let dof = 1.5 * (moving as f64 - 1.0).max(1.0);
    let now = state.half_step_kinetic(sys) / dof;
    if now <= 0.0 {
        return;
    }
    let f = (t / now).sqrt();
    for v in state.vel.iter_mut() {
        *v = v.map(|x| (x as f64 * f).round() as i32);
    }
    zero_velocity_sum(sys, &state.species, &mut state.vel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fcc_fills_the_box_without_overlaps() {
        let n = 4 * 5 * 5 * 5;
        let b = cube_for(n, 1.0);
        let c = fcc_cells(n, b);
        assert_eq!(c, [5, 5, 5]);
        let p = fcc(n, b, c, 0.0, 1);
        assert_eq!(p.len(), n);
        // Nearest neighbours at a/√2 = (4/ρ)^(1/3)/√2 ≈ 1.12 σ at ρ = 1.
        let l = b[0];
        let mut min = f64::MAX;
        for i in 0..50 {
            for j in 0..n {
                if i != j {
                    let d2: f64 = (0..3).map(|a| fixed::to_f64(fixed::min_image(p[i][a], p[j][a], l)).powi(2)).sum();
                    min = min.min(d2.sqrt());
                }
            }
        }
        assert!((min - (4.0f64).cbrt() / 2f64.sqrt()).abs() < 1e-6, "{min}");
    }

    #[test]
    fn maxwell_has_zero_momentum_and_the_right_temperature() {
        let n = 4000;
        let sys = System3::new(cube_for(n, 0.5), 0.005, Pair::wca(), vec![1.0]);
        let species = vec![0; n];
        let mut s = State3 { pos: fcc(n, sys.box_len, fcc_cells(n, sys.box_len), 0.0, 1), vel: maxwell(&sys, &species, 1.3, 2), species };
        for a in 0..3 {
            assert_eq!(s.vel.iter().map(|v| v[a] as i64).sum::<i64>(), 0);
        }
        let t = s.half_step_kinetic(&sys) / (1.5 * n as f64);
        assert!((t - 1.3).abs() < 0.05, "{t}");
        set_temperature(&mut s, &sys, 0.7);
        let t = s.half_step_kinetic(&sys) / (1.5 * (n as f64 - 1.0));
        assert!((t - 0.7).abs() < 1e-3, "{t}");
    }
}
