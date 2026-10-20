//! Deterministic initial conditions (counter-based random numbers, so any engine can
//! regenerate them).

use crate::fixed::{self, ONE};
use crate::system::{State, System};
use gpu_prims::threefry;

/// RNG stream ids, so different uses of one seed never overlap.
pub const STREAM_VELOCITY: u32 = 1;
pub const STREAM_JITTER: u32 = 2;
pub const STREAM_DIRECTION: u32 = 3;

/// Side lengths of a box holding `n` particles at number density `density` with the given
/// aspect ratio (x / y).
pub fn box_for(n: usize, density: f64, aspect: f64) -> [i64; 2] {
    let area = n as f64 / density;
    let ly = (area / aspect).sqrt();
    [fixed::to_fixed(ly * aspect), fixed::to_fixed(ly)]
}

/// The first `n` sites of a square-ish lattice filling the box, with a small random jitter.
pub fn lattice(n: usize, box_len: [i64; 2], jitter: f64, seed: u64) -> Vec<[i64; 2]> {
    let b = [fixed::to_f64(box_len[0]), fixed::to_f64(box_len[1])];
    let nx = ((n as f64 * b[0] / b[1]).sqrt().ceil() as usize).max(1);
    let ny = n.div_ceil(nx);
    let a = [b[0] / nx as f64, b[1] / ny as f64];
    (0..n)
        .map(|k| {
            let (i, j) = (k % nx, k / nx);
            let jx = jitter * (threefry::uniform(seed, STREAM_JITTER, 2 * k as u32) - 0.5);
            let jy = jitter * (threefry::uniform(seed, STREAM_JITTER, 2 * k as u32 + 1) - 0.5);
            [
                fixed::wrap(fixed::to_fixed((i as f64 + 0.5) * a[0] + jx), box_len[0]),
                fixed::wrap(fixed::to_fixed((j as f64 + 0.5) * a[1] + jy), box_len[1]),
            ]
        })
        .collect()
}

/// Maxwell–Boltzmann velocities at temperature `t`, with the total velocity sum set to
/// exactly zero (zero momentum when masses are equal).
pub fn maxwell(sys: &System, species: &[u32], t: f64, seed: u64) -> Vec<[i32; 2]> {
    let mut vel: Vec<[i32; 2]> = species
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let sd = (t / sys.masses[s as usize]).sqrt();
            let i = i as u32;
            [
                sys.to_step_displacement(sd * threefry::normal(seed, STREAM_VELOCITY, 2 * i)),
                sys.to_step_displacement(sd * threefry::normal(seed, STREAM_VELOCITY, 2 * i + 1)),
            ]
        })
        .collect();
    zero_velocity_sum(&mut vel);
    vel
}

/// Every particle at the same speed (mean kinetic energy `t` per particle in 2D) in a random
/// direction: far from Maxwell–Boltzmann, for relaxation tests.
pub fn fixed_speed(sys: &System, species: &[u32], t: f64, seed: u64) -> Vec<[i32; 2]> {
    let mut vel: Vec<[i32; 2]> = species
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let speed = (2.0 * t / sys.masses[s as usize]).sqrt();
            let th = std::f64::consts::TAU * threefry::uniform(seed, STREAM_DIRECTION, i as u32);
            [sys.to_step_displacement(speed * th.cos()), sys.to_step_displacement(speed * th.sin())]
        })
        .collect();
    zero_velocity_sum(&mut vel);
    vel
}

/// Subtract the mean so the integer sum is exactly zero in each axis.
pub fn zero_velocity_sum(vel: &mut [[i32; 2]]) {
    let n = vel.len() as i64;
    if n == 0 {
        return;
    }
    for a in 0..2 {
        let sum: i64 = vel.iter().map(|v| v[a] as i64).sum();
        let q = sum.div_euclid(n);
        let r = sum.rem_euclid(n);
        for (i, v) in vel.iter_mut().enumerate() {
            v[a] -= (q + ((i as i64) < r) as i64) as i32;
        }
    }
}

/// Rescale velocities so the (half-step) kinetic temperature is `t`, keeping the sum zero.
pub fn set_temperature(state: &mut State, sys: &System, t: f64) {
    let dof = (state.n() as f64 - 1.0).max(1.0);
    let now = state.half_step_kinetic(sys) / dof;
    if now <= 0.0 {
        return;
    }
    let f = (t / now).sqrt();
    for v in state.vel.iter_mut() {
        *v = [(v[0] as f64 * f).round() as i32, (v[1] as f64 * f).round() as i32];
    }
    zero_velocity_sum(&mut state.vel);
}

/// A fluid of `n` particles of species 0 at `density` and temperature `t`. On axes with walls
/// the lattice keeps 1 σ clear of them (inside a soft wall's range the force is huge).
pub fn fluid(sys: &System, n: usize, t: f64, seed: u64) -> State {
    let margin = 1.0;
    let inner = [0, 1].map(|a| if sys.periodic(a) { sys.box_len[a] } else { sys.box_len[a] - fixed::to_fixed(2.0 * margin) });
    let mut pos = lattice(n, inner, 0.1, seed);
    for p in pos.iter_mut() {
        for a in 0..2 {
            if !sys.periodic(a) {
                p[a] += fixed::to_fixed(margin);
            }
        }
    }
    let species = vec![0; n];
    let vel = maxwell(sys, &species, t, seed);
    State { pos, vel, species }
}

/// Exactly `per_box` particles in every compartment, on a small jittered lattice clear of the
/// compartment walls, with Maxwell velocities at temperature `t`. Particles are numbered box by
/// box (row-major over compartments).
pub fn compartment_fluid(sys: &System, per_box: usize, t: f64, seed: u64) -> State {
    let [cx, cy] = sys.compartment_counts();
    let side = [fixed::to_f64(sys.box_len[0]) / cx as f64, fixed::to_f64(sys.box_len[1]) / cy as f64];
    let nx = ((per_box as f64 * side[0] / side[1]).sqrt().ceil() as usize).max(1);
    let ny = per_box.div_ceil(nx);
    let margin = 0.6;
    let a = [(side[0] - 2.0 * margin) / nx as f64, (side[1] - 2.0 * margin) / ny as f64];
    let mut pos = Vec::with_capacity(per_box * cx * cy);
    for by in 0..cy {
        for bx in 0..cx {
            for k in 0..per_box {
                let id = pos.len() as u32;
                let jx = 0.2 * a[0] * (threefry::uniform(seed, STREAM_JITTER, 2 * id) - 0.5);
                let jy = 0.2 * a[1] * (threefry::uniform(seed, STREAM_JITTER, 2 * id + 1) - 0.5);
                let x = bx as f64 * side[0] + margin + ((k % nx) as f64 + 0.5) * a[0] + jx;
                let y = by as f64 * side[1] + margin + ((k / nx) as f64 + 0.5) * a[1] + jy;
                pos.push([fixed::to_fixed(x), fixed::to_fixed(y)]);
            }
        }
    }
    let species = vec![0; pos.len()];
    let vel = maxwell(sys, &species, t, seed);
    State { pos, vel, species }
}

/// Set the species of every particle within `radius` σ of `center` (σ).
pub fn mark_disc(state: &mut State, sys: &System, center: [f64; 2], radius: f64, species: u32) {
    let c = [fixed::to_fixed(center[0]), fixed::to_fixed(center[1])];
    let r2 = radius * radius;
    for i in 0..state.n() {
        let dx = fixed::to_f64(fixed::min_image(c[0], state.pos[i][0], sys.box_len[0]));
        let dy = fixed::to_f64(fixed::min_image(c[1], state.pos[i][1], sys.box_len[1]));
        if dx * dx + dy * dy < r2 {
            state.species[i] = species;
        }
    }
}

/// A reasonable default timestep for (W)LJ fluids near T = 1.
pub const DT: f64 = 0.005;

/// One σ, for callers building boxes by hand.
pub const SIGMA: i64 = ONE;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pair;

    #[test]
    fn zero_sum_is_exact() {
        let sys = System::new(box_for(1000, 0.5, 1.0), DT, Pair::wca(), vec![1.0]);
        let s = fluid(&sys, 1000, 1.3, 9);
        assert_eq!(s.velocity_sum(), [0, 0]);
        let t = s.half_step_kinetic(&sys) / (s.n() as f64 - 1.0);
        assert!((t - 1.3).abs() < 0.1, "T = {t}");
    }

    #[test]
    fn lattice_is_inside_box_and_spread() {
        let b = box_for(500, 0.7, 2.0);
        let p = lattice(500, b, 0.1, 1);
        assert!(p.iter().all(|q| (0..b[0]).contains(&q[0]) && (0..b[1]).contains(&q[1])));
    }
}
