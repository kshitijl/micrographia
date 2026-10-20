//! The system description and the integer state shared by all engines.

use crate::fixed::{self, ONE, UNIT};
use crate::pair::Pair;

/// Up to this many species (the GPU keeps per-species constants in one vec4).
pub const MAX_SPECIES: usize = 4;

#[derive(Clone, Debug)]
pub struct System {
    /// Periodic box lengths in position units.
    pub box_len: [i64; 2],
    pub dt: f64,
    pub pair: Pair,
    /// Mass per species; species only differ by mass (and by colour on pages).
    pub masses: Vec<f64>,
}

impl System {
    pub fn new(box_len: [i64; 2], dt: f64, pair: Pair, masses: Vec<f64>) -> Self {
        assert!(!masses.is_empty() && masses.len() <= MAX_SPECIES);
        assert!(pair.rc < 7.9, "cutoff must be below 8 σ");
        Self { box_len, dt, pair, masses }
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
        self.dt * self.dt / self.masses[species as usize] * ONE as f64
    }

    /// Velocity (σ/τ) of an integer per-step displacement.
    pub fn velocity(&self, v: i32) -> f64 {
        v as f64 * UNIT / self.dt
    }

    pub fn to_step_displacement(&self, v: f64) -> i32 {
        (v * self.dt * ONE as f64).round() as i32
    }

    /// Cells per axis for a cell list with cutoff rc.
    pub fn cells(&self) -> [u32; 2] {
        self.cells_for(self.pair.rc)
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
