//! Liquid–vapour coexistence of the 2D Lennard-Jones fluid from a slab simulation.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::{fixed, init, Pair, State, System};
use pollster::block_on;

/// A dense slab across the middle of a tall periodic box; returns (liquid, vapour) densities.
pub fn slab(p: &Prims, rc: f64, t: f64, steps: usize) -> (f64, f64, Vec<f64>) {
    let (w, h) = (60.0, 160.0);
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::lj(rc), vec![1.0]).with_thermostat(t, 0.5);
    // Liquid at density 0.7 between h/2 ± 20.
    let slab = System::new([fixed::to_fixed(w), fixed::to_fixed(40.0)], init::DT, Pair::lj(rc), vec![1.0]);
    let n = (0.7 * w * 40.0) as usize;
    let mut s: State = init::fluid(&slab, n, t, 1);
    for q in s.pos.iter_mut() {
        q[1] += fixed::to_fixed(h / 2.0 - 20.0);
    }
    let mut e = GpuEngine::new(p, sys.clone(), &s);
    e.step(steps / 2);
    let bins = 80;
    let mut prof = vec![0f64; bins];
    let frames = 100;
    for _ in 0..frames {
        e.step(steps / 2 / frames);
        let st = block_on(e.read_state());
        // Centre on the slab's centre of mass (it wanders) using the circular mean.
        let (mut cs, mut sn) = (0.0, 0.0);
        for q in &st.pos {
            let a = fixed::to_f64(q[1]) / h * std::f64::consts::TAU;
            cs += a.cos();
            sn += a.sin();
        }
        let centre = (sn.atan2(cs) / std::f64::consts::TAU).rem_euclid(1.0) * h;
        for q in &st.pos {
            let y = (fixed::to_f64(q[1]) - centre + 1.5 * h).rem_euclid(h);
            prof[((y / h * bins as f64) as usize).min(bins - 1)] += 1.0;
        }
    }
    let area = w * h / bins as f64 * frames as f64;
    let rho: Vec<f64> = prof.iter().map(|c| c / area).collect();
    // Liquid: the central bins; vapour: the bins farthest from the slab.
    let liquid = rho[bins / 2 - 4..bins / 2 + 4].iter().sum::<f64>() / 8.0;
    let vapour = (rho[..6].iter().sum::<f64>() + rho[bins - 6..].iter().sum::<f64>()) / 12.0;
    (liquid, vapour, rho)
}

#[test]
#[ignore]
fn coexistence_curve() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for rc in [2.5, 4.0] {
        for t in [0.38, 0.40, 0.42, 0.44, 0.46] {
            let (l, v, _) = slab(&p, rc, t, 200_000);
            eprintln!("rc {rc}, T {t}: liquid {l:.3}, vapour {v:.4}");
        }
    }
}
