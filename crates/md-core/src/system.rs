//! The system description and the integer state shared by all engines.

use crate::fixed::{self, ONE, UNIT};
use crate::pair::Pair;

/// Up to this many species (the GPU keeps per-species constants in vec4s).
pub const MAX_SPECIES: usize = 4;

/// What happens at a wall (walls sit at 0 and L of a non-periodic axis and act on particle
/// centres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wall {
    /// Specular reflection: exact, and exactly time-reversible. Under an external force the
    /// leapfrog reflection gains or loses O(Δt) energy per hit; use `Soft` there.
    Hard,
    /// A smooth repulsive (WCA) wall acting on the distance of a particle centre from the wall:
    /// conserves energy to O(Δt²) like the pair forces, and is exactly reversible. The force on
    /// it is recorded in the ledger, giving the pressure.
    Soft,
    /// A heat bath at temperature T: a particle that hits it leaves with a velocity drawn from
    /// the bath's distribution (Maxwell flux for the normal component). The energy exchanged is
    /// booked exactly in the wall's ledger.
    Thermal(f64),
}

#[derive(Clone, Debug)]
pub struct System {
    /// Box lengths in position units.
    pub box_len: [i64; 2],
    pub dt: f64,
    /// Mass per species.
    pub masses: Vec<f64>,
    /// Interaction of each species pair, row-major `[a * ns + b]`, symmetric.
    pub pairs: Vec<Pair>,
    /// Per axis: `None` for periodic, or the walls at 0 and at L.
    pub walls: [Option<[Wall; 2]>; 2],
    /// Uniform acceleration (σ/τ²) on every particle.
    pub gravity: [f64; 2],
    /// Seed for stochastic boundaries.
    pub seed: u64,
    /// Per axis: 0 for none, or log2 of the compartment side in position units (≥ 32, i.e.
    /// a power-of-two number of σ). Compartments are separated by hard walls, and particles in
    /// different compartments do not interact: one simulation then holds many independent
    /// replicas. The axis must have walls (the outer boundary).
    pub compartments: [u32; 2],
    /// A heat bath acting throughout the volume (Andersen): each step, each particle has
    /// probability `rate · Δt` of having its velocity redrawn at temperature `t`.
    pub thermostat: Option<Thermostat>,
    /// A semi-permeable membrane: a thin wall across the box that reflects some species and
    /// lets the others through.
    pub membrane: Option<Membrane>,
    /// A piston: a soft wall that replaces the top (y-high) wall and moves.
    pub piston: Option<Piston>,
    /// Bond potentials, and the bonded pairs (particle ids a, b and a bond type index). Shared,
    /// so systems stay cheap to clone.
    pub bond_types: Vec<BondType>,
    pub bonds: std::sync::Arc<Vec<[u32; 3]>>,
    /// Bit s set if species s is pinned: it feels no forces, keeps its velocity (often zero),
    /// and is ignored by heat baths and temperature measurements.
    pub pinned: u32,
}

/// At most this many bonds per particle (the GPU keeps a fixed-size partner table).
pub const MAX_BONDS: usize = 4;
pub const MAX_BOND_TYPES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BondType {
    /// U = ½ k (r − r0)².
    Harmonic { k: f64, r0: f64 },
    /// FENE, U = −½ k R0² ln(1 − (r/R0)²) (Kremer–Grest uses k = 30, R0 = 1.5 with WCA).
    Fene { k: f64, r0: f64 },
}

impl BondType {
    /// The Kremer–Grest bead–spring bond (with WCA between all beads).
    pub fn kremer_grest() -> Self {
        BondType::Fene { k: 30.0, r0: 1.5 }
    }

    /// −U'(r)/r (so the force on a is f_over_r · (r_a − r_b)) and U, in f64.
    pub fn eval(&self, r2: f64) -> (f64, f64) {
        match *self {
            BondType::Harmonic { k, r0 } => {
                let r = r2.sqrt();
                (-k * (r - r0) / r, 0.5 * k * (r - r0) * (r - r0))
            }
            BondType::Fene { k, r0 } => {
                let x = (r2 / (r0 * r0)).min(0.999_999);
                (-k / (1.0 - x), -0.5 * k * r0 * r0 * (1.0 - x).ln())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PistonDrive {
    /// Moves at a fixed speed (σ/τ; positive opens the box).
    Speed(f64),
    /// Moves freely under the gas pressure and a constant external force (ε/σ, pushing down),
    /// with mass M.
    Free { mass: f64, force: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piston {
    pub drive: PistonDrive,
    /// Initial height (σ).
    pub start: f64,
    /// The piston stops at these heights (σ).
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Membrane {
    /// The membrane is perpendicular to this axis (which must have walls, not periodic).
    pub axis: usize,
    /// Position along the axis, in position units. Particles reflect about position − ½.
    pub position: i64,
    /// Bit s set if species s is blocked.
    pub blocks: u32,
    /// Pores: openings that let every species through, `pore_width` wide, one every
    /// 2^`pore_shift` position units along the membrane (0 for no pores).
    pub pore_shift: u32,
    pub pore_width: i64,
}

impl Membrane {
    pub fn blocks(&self, species: u32) -> bool {
        self.blocks & (1 << species) != 0
    }

    /// Is position `along` (the coordinate along the membrane) inside a pore?
    pub fn in_pore(&self, along: i64) -> bool {
        self.pore_shift != 0 && (along & ((1i64 << self.pore_shift) - 1)) < self.pore_width
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thermostat {
    pub t: f64,
    /// Collision rate per particle (1/τ).
    pub rate: f64,
}

/// Ledger slots per wall: one per compartment along the other axis.
pub const MAX_COMPARTMENTS: usize = 256;

impl System {
    /// A periodic system in which every species pair interacts through `pair`.
    pub fn new(box_len: [i64; 2], dt: f64, pair: Pair, masses: Vec<f64>) -> Self {
        assert!(!masses.is_empty() && masses.len() <= MAX_SPECIES);
        let ns = masses.len();
        let s = Self { box_len, dt, pairs: vec![pair; ns * ns], masses, walls: [None, None], gravity: [0.0, 0.0], seed: 1, compartments: [0, 0], thermostat: None, membrane: None, piston: None, bond_types: vec![], bonds: Default::default(), pinned: 0 };
        s.check();
        s
    }

    fn check(&self) {
        for p in &self.pairs {
            assert!(p.rc < 7.9, "cutoff must be below 8 σ");
        }
    }

    pub fn species(&self) -> usize {
        self.masses.len()
    }

    /// Set the interaction between species `a` and `b` (both orders).
    pub fn with_pair(mut self, a: u32, b: u32, pair: Pair) -> Self {
        let ns = self.species();
        self.pairs[a as usize * ns + b as usize] = pair;
        self.pairs[b as usize * ns + a as usize] = pair;
        self.check();
        self
    }

    pub fn with_walls(mut self, axis: usize, lo: Wall, hi: Wall) -> Self {
        self.walls[axis] = Some([lo, hi]);
        self
    }

    pub fn with_gravity(mut self, g: [f64; 2]) -> Self {
        self.gravity = g;
        self
    }

    /// Split `axis` into compartments of side 2^`log2_sigma` σ separated by hard walls. The box
    /// length along the axis must be a whole number of compartments, and the axis must have
    /// walls.
    pub fn with_compartments(mut self, axis: usize, log2_sigma: u32) -> Self {
        let shift = 32 + log2_sigma;
        assert!(self.walls[axis].is_some(), "compartments need walls on their axis");
        assert_eq!(self.box_len[axis] % (1i64 << shift), 0, "box must be a whole number of compartments");
        assert!(self.compartments_along(axis, shift) <= MAX_COMPARTMENTS);
        self.compartments[axis] = shift;
        self
    }

    fn compartments_along(&self, axis: usize, shift: u32) -> usize {
        (self.box_len[axis] >> shift) as usize
    }

    /// Number of compartments along each axis (1 if none).
    pub fn compartment_counts(&self) -> [usize; 2] {
        let c = |a: usize| if self.compartments[a] == 0 { 1 } else { self.compartments_along(a, self.compartments[a]) };
        [c(0), c(1)]
    }

    /// Compartment coordinates of a position.
    pub fn compartment_of(&self, p: [i64; 2]) -> [i64; 2] {
        let c = |a: usize| if self.compartments[a] == 0 { 0 } else { p[a] >> self.compartments[a] };
        [c(0), c(1)]
    }

    /// A membrane across `axis` at `position_sigma` σ blocking the species in `blocked`.
    pub fn with_membrane(mut self, axis: usize, position_sigma: f64, blocked: &[u32]) -> Self {
        assert!(self.walls[axis].is_some(), "a membrane needs walls on its axis");
        let blocks = blocked.iter().fold(0u32, |m, &s| m | 1 << s);
        self.membrane = Some(Membrane { axis, position: crate::fixed::to_fixed(position_sigma), blocks, pore_shift: 0, pore_width: 0 });
        self
    }

    /// Open pores `width` σ wide every 2^`log2_period` σ along the membrane.
    pub fn with_pores(mut self, log2_period: u32, width: f64) -> Self {
        let m = self.membrane.as_mut().expect("add a membrane first");
        m.pore_shift = 32 + log2_period;
        m.pore_width = crate::fixed::to_fixed(width);
        self
    }

    /// Replace the top wall by a piston starting at height `start` σ (y must have walls; the
    /// box height is the piston's upper limit).
    pub fn with_piston(mut self, drive: PistonDrive, start: f64, min: f64) -> Self {
        assert!(self.walls[1].is_some(), "a piston needs walls on y");
        let max = self.box_f64()[1];
        assert!(start <= max && min < start);
        self.piston = Some(Piston { drive, start, min, max });
        self
    }

    /// Bond types and bonded pairs `[id_a, id_b, type]`.
    pub fn with_bonds(mut self, types: Vec<BondType>, bonds: Vec<[u32; 3]>) -> Self {
        assert!(types.len() <= MAX_BOND_TYPES);
        let mut count = std::collections::HashMap::new();
        for b in &bonds {
            assert!((b[2] as usize) < types.len());
            for id in [b[0], b[1]] {
                let c = count.entry(id).or_insert(0usize);
                *c += 1;
                assert!(*c <= MAX_BONDS, "particle {id} has more than {MAX_BONDS} bonds");
            }
        }
        self.bond_types = types;
        self.bonds = std::sync::Arc::new(bonds);
        self
    }

    /// Pin species `s`: no forces act on it, and baths ignore it.
    pub fn with_pinned(mut self, s: u32) -> Self {
        self.pinned |= 1 << s;
        self
    }

    pub fn is_pinned(&self, s: u32) -> bool {
        self.pinned & (1 << s) != 0
    }

    /// Bond partners of every particle (by id), padded with `u32::MAX`, and the bond type.
    pub fn partner_table(&self, n: usize) -> Vec<[u32; 2 * MAX_BONDS]> {
        let mut t = vec![[u32::MAX; 2 * MAX_BONDS]; n];
        let mut used = vec![0usize; n];
        for b in self.bonds.iter() {
            for (me, other) in [(b[0], b[1]), (b[1], b[0])] {
                let k = used[me as usize];
                t[me as usize][2 * k] = other;
                t[me as usize][2 * k + 1] = b[2];
                used[me as usize] += 1;
            }
        }
        t
    }

    pub fn with_thermostat(mut self, t: f64, rate: f64) -> Self {
        self.thermostat = Some(Thermostat { t, rate });
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn pair(&self, a: u32, b: u32) -> &Pair {
        &self.pairs[a as usize * self.species() + b as usize]
    }

    /// The largest cutoff of any pair.
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

    /// The integer gravity kick per step (position units per step², the same for every
    /// species).
    pub fn gravity_kick(&self) -> [i32; 2] {
        let k = |g: f64| (g * self.dt * self.dt * ONE as f64).round() as i32;
        [k(self.gravity[0]), k(self.gravity[1])]
    }

    pub fn box_f64(&self) -> [f64; 2] {
        [fixed::to_f64(self.box_len[0]), fixed::to_f64(self.box_len[1])]
    }

    pub fn area(&self) -> f64 {
        let b = self.box_f64();
        b[0] * b[1]
    }

    /// Converts force (ε/σ) into an integer kick (position units per step) for a species:
    /// Δv·Δt = F Δt² / m.
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

    /// Cells per axis for a cell list with the largest cutoff.
    pub fn cells(&self) -> [u32; 2] {
        self.cells_for(self.rc_max())
    }

    /// Cells per axis for a cell list of radius `r`: as many as fit with side ≥ r plus a margin
    /// for float rounding of the cell assignment.
    pub fn cells_for(&self, r: f64) -> [u32; 2] {
        let b = self.box_f64();
        let margin = 1e-3 + 1e-6 * b[0].max(b[1]);
        let c = [(b[0] / (r + margin)).floor() as u32, (b[1] / (r + margin)).floor() as u32];
        assert!(c[0] >= 3 && c[1] >= 3, "box too small for radius {r}: {c:?} cells");
        c
    }
}

/// Positions, half-step velocities (displacement per step, `v_{n-1/2}`) and species.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub pos: Vec<[i64; 2]>,
    pub vel: Vec<[i32; 2]>,
    pub species: Vec<u32>,
}

impl State {
    pub fn n(&self) -> usize {
        self.pos.len()
    }

    /// FNV-1a over every bit of positions, velocities and species, in storage order.
    pub fn hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        for i in 0..self.n() {
            eat(&self.pos[i][0].to_le_bytes());
            eat(&self.pos[i][1].to_le_bytes());
            eat(&self.vel[i][0].to_le_bytes());
            eat(&self.vel[i][1].to_le_bytes());
            eat(&self.species[i].to_le_bytes());
        }
        h
    }

    /// Particle `perm[k]` of `self` becomes particle `k` of the result.
    pub fn permuted(&self, perm: &[usize]) -> State {
        State {
            pos: perm.iter().map(|&i| self.pos[i]).collect(),
            vel: perm.iter().map(|&i| self.vel[i]).collect(),
            species: perm.iter().map(|&i| self.species[i]).collect(),
        }
    }

    /// Undo `permuted(perm)`.
    pub fn unpermuted(&self, perm: &[usize]) -> State {
        let mut out = self.clone();
        for (k, &i) in perm.iter().enumerate() {
            out.pos[i] = self.pos[k];
            out.vel[i] = self.vel[k];
            out.species[i] = self.species[k];
        }
        out
    }

    /// Exact total of the integer velocities (momentum when all masses are equal).
    pub fn velocity_sum(&self) -> [i64; 2] {
        self.vel.iter().fold([0, 0], |a, v| [a[0] + v[0] as i64, a[1] + v[1] as i64])
    }

    /// Kinetic energy from the stored half-step velocities (off by O(Δt²) from the on-step
    /// value; engines provide the on-step value).
    pub fn half_step_kinetic(&self, sys: &System) -> f64 {
        (0..self.n())
            .map(|i| {
                let m = sys.masses[self.species[i] as usize];
                let vx = sys.velocity(self.vel[i][0]);
                let vy = sys.velocity(self.vel[i][1]);
                0.5 * m * (vx * vx + vy * vy)
            })
            .sum()
    }

    /// Flip one bit of one coordinate: the smallest possible perturbation.
    pub fn flip_bit(&mut self, particle: usize, axis: usize, bit: u32) {
        self.pos[particle][axis] ^= 1i64 << bit;
    }

    /// Particles whose positions differ from `other`, and the largest difference in σ.
    pub fn position_difference(&self, other: &State, sys: &System) -> (usize, f64) {
        let mut count = 0;
        let mut max = 0.0f64;
        for i in 0..self.n() {
            if self.pos[i] != other.pos[i] {
                count += 1;
                for a in 0..2 {
                    let d = fixed::min_image(self.pos[i][a], other.pos[i][a], sys.box_len[a]);
                    max = max.max(fixed::to_f64(d).abs());
                }
            }
        }
        (count, max)
    }
}
