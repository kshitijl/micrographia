//! Wall collisions shared by the engines: exact mirrors for hard walls, and the bath
//! distribution for thermal walls.

use crate::system::System;
use gpu_prims::threefry;

/// Walls are numbered `2 * axis + side` (side 0 at 0, side 1 at L); the membrane's two faces
/// are 4 (hit from the low side) and 5 (from the high side); slot 6 books the bulk heat bath
/// (Andersen thermostat) as if it were a wall.
pub const WALLS: usize = 7;
pub const MEMBRANE_LOW: usize = 4;
pub const MEMBRANE_HIGH: usize = 5;
pub const BATH: usize = 6;

/// Did a step from `old` to `new` cross a membrane at `w`? Returns the mirrored coordinate and
/// the face hit (exact; mirrors about w − ½).
pub fn cross_membrane(old: i64, new: i64, w: i64) -> Option<(i64, usize)> {
    match (old < w, new < w) {
        (true, false) => Some((2 * w - 1 - new, MEMBRANE_LOW)),
        (false, true) => Some((2 * w - 1 - new, MEMBRANE_HIGH)),
        _ => None,
    }
}

/// Mirror a coordinate that left `[0, l)` back inside. The mirrors are about −½ and l − ½
/// position units, so each is an exact involution on integers.
pub fn mirror(x: i64, l: i64) -> (i64, Option<usize>) {
    if x < 0 {
        (-1 - x, Some(0))
    } else if x >= l {
        (2 * l - 1 - x, Some(1))
    } else {
        (x, None)
    }
}

/// Energy and momentum exchanged with each wall, accumulated in fixed point exactly as the
/// GPU does (so totals do not depend on the order of collisions). Totals over the whole wall;
/// [`CompartmentLedger`] splits them by compartment.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ledger {
    /// Energy given to the system by each wall (ε).
    pub energy: [f64; WALLS],
    /// Normal momentum the system gave to each wall (m σ / τ); divided by elapsed time and wall
    /// length this is the pressure.
    pub impulse: [f64; WALLS],
    pub hits: [u64; WALLS],
}

/// The ledger per wall and per compartment along the wall (index `[wall][compartment]`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompartmentLedger {
    pub energy: Vec<Vec<f64>>,
    pub impulse: Vec<Vec<f64>>,
    pub hits: Vec<Vec<u64>>,
}

impl CompartmentLedger {
    pub fn new(per_wall: usize) -> Self {
        Self { energy: vec![vec![0.0; per_wall]; WALLS], impulse: vec![vec![0.0; per_wall]; WALLS], hits: vec![vec![0; per_wall]; WALLS] }
    }

    pub fn totals(&self) -> Ledger {
        let mut l = Ledger::default();
        for w in 0..WALLS {
            l.energy[w] = self.energy[w].iter().sum();
            l.impulse[w] = self.impulse[w].iter().sum();
            l.hits[w] = self.hits[w].iter().sum();
        }
        l
    }
}

/// Mirror a coordinate that crossed an internal compartment wall back into compartment `k`
/// (compartment side 2^`shift`).
pub fn mirror_compartment(x: i64, k: i64, shift: u32) -> i64 {
    let lo = k << shift;
    let hi = (k + 1) << shift;
    if x < lo { 2 * lo - 1 - x } else if x >= hi { 2 * hi - 1 - x } else { x }
}

/// Fixed-point scale of ledger entries: 2⁻²⁴ ε (or m σ/τ).
pub const LEDGER_SCALE: f64 = (1u64 << 24) as f64;

/// RNG streams for wall draws.
pub const STREAM_WALL: u32 = 0x5741_4c4c;

/// A thermal wall's new velocity (σ/τ) for a particle of mass `m`: the normal component from
/// the Maxwell flux distribution, pointing into the box (`inward` = ±1), the tangential one
/// Gaussian. Counter-based on (particle id, step).
pub fn thermal_velocity(sys: &System, id: u32, clock: u32, t: f64, m: f64, inward: f64) -> (f64, f64) {
    let key = [sys.seed as u32, (sys.seed >> 32) as u32 ^ STREAM_WALL];
    let r = threefry::threefry2x32([id, clock], key);
    let r2 = threefry::threefry2x32([id, clock], [key[0], key[1] ^ 1]);
    let u1 = 1.0 - threefry::to_unit_f64(r);
    let (u2, u3) = (1.0 - threefry::to_unit_f64([r2[0], 0]), threefry::to_unit_f64([r2[1], 0]));
    let sd = (t / m).sqrt();
    let normal = inward * sd * (-2.0 * u1.ln()).sqrt();
    let tangential = sd * (-2.0 * u2.ln()).sqrt() * (std::f64::consts::TAU * u3).cos();
    (normal, tangential)
}

/// The soft wall: WCA in the distance d (σ) of a particle centre from the wall, with σ = ε = 1.
pub const SOFT_RC: f64 = 1.122_462_048_309_373; // 2^(1/6)

/// (force away from the wall, energy) at distance `d` from a soft wall, in f64.
pub fn soft_wall(d: f64) -> (f64, f64) {
    if d >= SOFT_RC {
        return (0.0, 0.0);
    }
    let s6 = 1.0 / d.powi(6);
    (24.0 * s6 * (2.0 * s6 - 1.0) / d, 4.0 * s6 * (s6 - 1.0) + 1.0)
}

/// RNG stream for the Andersen thermostat.
pub const STREAM_THERMO: u32 = 0x5448_524d;

/// Andersen thermostat: Some(new velocity in σ/τ, relative to the bath's drift) if particle
/// `id` (species `s`, at x coordinate `x`) is redrawn this step.
pub fn andersen(sys: &System, id: u32, clock: u32, s: u32, x: i64) -> Option<(f64, f64)> {
    let th = sys.thermostat?;
    if !th.acts_on(s, x) {
        return None;
    }
    let m = sys.masses[s as usize];
    let key = [sys.seed as u32, (sys.seed >> 32) as u32 ^ STREAM_THERMO];
    let r = threefry::threefry2x32([id, clock], key);
    if threefry::to_unit_f64([r[0], 0]) >= th.rate * sys.dt {
        return None;
    }
    let r2 = threefry::threefry2x32([id, clock], [key[0], key[1] ^ 1]);
    let u1 = 1.0 - threefry::to_unit_f64([r[1], 0]);
    let u2 = threefry::to_unit_f64([r2[0], 0]);
    let sd = (th.t / m).sqrt();
    let rad = (-2.0 * u1.ln()).sqrt();
    let a = std::f64::consts::TAU * u2;
    Some((sd * rad * a.cos(), sd * rad * a.sin()))
}

/// Fixed-point scale of the per-step piston force sum: 2⁻¹⁶ ε/σ per unit.
pub const PISTON_FORCE_SCALE: f64 = 65536.0;

/// The integer kick on a free piston for a net upward force (ε/σ): Δv·Δt = F Δt² / M.
pub fn piston_kick(sys: &System, net_force: f64, mass: f64) -> i32 {
    (net_force * sys.dt * sys.dt / mass * crate::ONE as f64).round() as i32
}
