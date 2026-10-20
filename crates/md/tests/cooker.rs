//! Boiling at constant pressure: a Lennard-Jones pool under a free piston, heated from below.
//!
//! A first look, not yet a page. With 2000 molecules, a thermal floor at kT = 0.8 and piston
//! loads of 0.2 and 0.6 ε/σ (pressures 0.002 and 0.006), the liquid warms from 0.40 to ~0.43–0.44
//! over 1000 τ while the piston rises, but there is no clean plateau and the two loads barely
//! differ. The 2D LJ liquid range is narrow (triple ~0.41, critical ~0.46), heating from the
//! floor sets up a gradient through the pool, and per-bin "vapour" counts are dominated by the
//! interface. Next to try: uniform (bulk) heating at constant power, and a fluid with a wider
//! liquid range.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::{fixed, init, Pair, PistonDrive, System, Wall};
use pollster::block_on;

/// (liquid temperature, vapour temperature, molecules in the vapour) from density bins 2 σ tall.
fn phases(e: &GpuEngine, w: f64) -> (f64, f64, f64) {
    let s = block_on(e.read_state());
    let dt = e.sys.dt;
    let bins = 400;
    let mut count = vec![0.0; bins];
    let mut ke = vec![0.0; bins];
    for i in 0..s.n() {
        let y = fixed::to_f64(s.pos[i][1]);
        let b = ((y / 2.0) as usize).min(bins - 1);
        let v = s.vel[i].map(|x| fixed::to_f64(x as i64) / dt);
        count[b] += 1.0;
        ke[b] += 0.5 * (v[0] * v[0] + v[1] * v[1]);
    }
    let (mut tl, mut nl, mut tv, mut nv) = (0.0, 0.0, 0.0, 0.0);
    for b in 0..bins {
        if count[b] / (2.0 * w) > 0.4 {
            tl += ke[b];
            nl += count[b];
        } else {
            tv += ke[b];
            nv += count[b];
        }
    }
    (tl / nl.max(1.0), tv / nv.max(1.0), nv)
}

#[test]
#[ignore]
fn plateau() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let (w, h, n) = (100.0, 600.0, 2000usize);
    let depth = n as f64 / (0.75 * w);
    for force in [0.2, 0.6] {
        let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::lj(2.5), vec![1.0])
            .with_walls(1, Wall::Thermal(0.8), Wall::Hard)
            .with_gravity([0.0, -0.002])
            .with_piston(PistonDrive::Free { mass: 50.0, force }, depth + 8.0, depth);
        let pool = System::new([fixed::to_fixed(w), fixed::to_fixed(depth + 2.0)], init::DT, Pair::lj(2.5), vec![1.0]).with_walls(1, Wall::Soft, Wall::Soft);
        let state = init::fluid(&pool, n, 0.38, 1);
        let mut e = GpuEngine::new(&p, sys, &state);
        for k in 0..40 {
            e.step(5000);
            let (tl, tv, nv) = phases(&e, w);
            let pis = block_on(e.read_piston());
            eprintln!("F {force} t {:5.0}: liquid T {tl:.3}, vapour T {tv:.3}, {nv:4.0} in vapour, piston {:.1}", (k + 1) as f64 * 5000.0 * e.sys.dt, pis[0]);
        }
    }
}
