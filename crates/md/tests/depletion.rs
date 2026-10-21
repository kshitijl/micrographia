//! The depletion force (the `depletion` scenario): two big disks held still near contact are
//! pushed together by the small disks around them, with the Asakura–Oosawa force
//! 2 n kT √((R + r)² − d²/4) (2D, ideal small disks), stronger when hotter: an entropic force.
//! (These molecules are soft: hot ones squash closer, so R + r, where the WCA energy is kT, shrinks
//! a little as kT grows, and the force grows a little less than ∝ T.) The force on each big disk is
//! summed from the small disks' WCA pushes.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

/// WCA force magnitude at distance r for diameter s.
fn wca(r: f64, s: f64) -> f64 {
    if r >= s * 2f64.powf(1.0 / 6.0) {
        return 0.0;
    }
    let x6 = (s / r).powi(6);
    24.0 * x6 * (2.0 * x6 - 1.0) / r
}

/// Mean attraction (the x force on the left disk, + towards the right one, and minus that on the
/// right one, halved), and the small disks' kT.
fn measure(e: &mut GpuEngine, samples: usize) -> (f64, f64) {
    let (mut f, mut t) = (0.0, 0.0);
    for _ in 0..samples {
        e.step(50);
        let s = block_on(e.read_state());
        let b = e.sys.box_f64();
        let c = [0, 1].map(|i| [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])]);
        let mut fx = [0.0; 2];
        let (mut ke, mut m) = (0.0, 0);
        for i in 2..s.n() {
            let p = [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])];
            for (k, q) in c.iter().enumerate() {
                let mut dx = q[0] - p[0];
                let mut dy = q[1] - p[1];
                dx -= b[0] * (dx / b[0]).round();
                dy -= b[1] * (dy / b[1]).round();
                let r = (dx * dx + dy * dy).sqrt();
                fx[k] += wca(r, 3.5) * dx / r;
            }
            ke += 0.5 * (e.sys.velocity(s.vel[i][0]).powi(2) + e.sys.velocity(s.vel[i][1]).powi(2));
            m += 1;
        }
        f += 0.5 * (fx[0] - fx[1]) / samples as f64;
        t += ke / m as f64 / samples as f64;
    }
    (f, t)
}

/// Force (with its standard error over 10 blocks) and kT, at centre separation `sep`, after
/// scaling every speed by `heat`.
fn force_at(p: &Prims, sep: f64, heat: f64) -> (f64, f64, f64, f64) {
    let sc = scenarios::build("depletion", &Options::default().with("separation", sep)).unwrap();
    let mut e = GpuEngine::new(p, sc.sys.clone(), &sc.state);
    let b = e.sys.box_f64();
    let n = (sc.state.n() - 2) as f64 / (b[0] * b[1] - 2.0 * std::f64::consts::PI * 3.5 * 3.5);
    e.kick_region([-1.0, -1.0], [b[0] + 1.0, b[1] + 1.0], heat, [0.0, 0.0]);
    e.step(4000);
    let blocks: Vec<(f64, f64)> = (0..10).map(|_| measure(&mut e, 300)).collect();
    let f = blocks.iter().map(|x| x.0).sum::<f64>() / 10.0;
    let t = blocks.iter().map(|x| x.1).sum::<f64>() / 10.0;
    let se = (blocks.iter().map(|x| (x.0 - f).powi(2)).sum::<f64>() / 90.0).sqrt();
    // R + r where the small–big WCA energy is kT: (σ/r)⁶ = (1 + √kT)/2.
    let rr = 3.5 / ((1.0 + t.sqrt()) / 2.0).powf(1.0 / 6.0);
    let ao = if sep < 2.0 * rr { 2.0 * n * t * (rr * rr - sep * sep / 4.0).sqrt() } else { 0.0 };
    (f, se, t, ao)
}

#[test]
fn depletion_force_is_entropic() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let mut out = vec![];
    for (sep, heat) in [(6.3, 1.0), (6.3, 2f64.sqrt()), (6.8, 1.0), (8.5, 1.0)] {
        let (f, se, t, ao) = force_at(&p, sep, heat);
        eprintln!("depletion: d {sep}: kT {t:.3}: force {f:.3} ± {se:.3} (Asakura–Oosawa {ao:.3})");
        out.push((f, se, t, ao));
    }
    // Near contact: about AO's force, hot and cold; stronger when hotter; nothing beyond the range.
    for o in &out[..2] {
        assert!((o.0 - o.3).abs() < 0.25 * o.3 + 3.0 * o.1);
    }
    assert!(out[1].0 > 1.2 * out[0].0);
    assert!(out[3].0.abs() < 3.0 * out[3].1 + 0.05);
}
