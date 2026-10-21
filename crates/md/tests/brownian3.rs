//! A Brownian grain in 3D (the `brownian3` scenario): diffusion, friction and Einstein's
//! relation D = kT / friction.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine3;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

#[test]
#[ignore = "an experiment: the grain's diffusion and friction"]
fn einstein_relation() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let size: f64 = std::env::var("SIZE").ok().and_then(|s| s.parse().ok()).unwrap_or(5.0);
    let n: f64 = std::env::var("N").ok().and_then(|s| s.parse().ok()).unwrap_or(16000.0);
    let sc = scenarios::three::build3("brownian3", &Options::default().with("size", size).with("n", n)).unwrap();
    let t0 = std::time::Instant::now();
    let mut e = GpuEngine3::new(&p, sc.sys.clone(), &sc.state);
    let l = sc.sys.box_f64();
    let m = sc.sys.masses[1];
    e.step(4000);
    // Diffusion: the grain's unwrapped path, sampled every 0.5 τ.
    let mut path: Vec<[f64; 3]> = vec![];
    let mut last: Option<[f64; 3]> = None;
    let mut acc = [0.0; 3];
    for _ in 0..4000 {
        e.step(100);
        let s = block_on(e.read_state());
        let q = s.pos[0].map(fixed::to_f64);
        if let Some(prev) = last {
            for a in 0..3 {
                let mut d = q[a] - prev[a];
                d -= l[a] * (d / l[a]).round();
                acc[a] += d;
            }
        }
        last = Some(q);
        path.push(acc);
    }
    let dt = 0.5;
    for lag in [10usize, 40, 160] {
        let mut s = 0.0;
        let k = path.len() - lag;
        for i in 0..k {
            s += (0..3).map(|a| (path[i + lag][a] - path[i][a]).powi(2)).sum::<f64>();
        }
        let msd = s / k as f64;
        eprintln!("lag {:.0} τ: MSD {msd:.3} σ², D = MSD/6t = {:.4}", lag as f64 * dt, msd / (6.0 * lag as f64 * dt));
    }
    // Friction: throw the grain at 1 σ/τ and fit the decay of its speed (averaged over throws).
    let mut decay = vec![0.0; 40];
    for _ in 0..20 {
        e.set_species_velocity(1, [1.0, 0.0, 0.0]);
        for k in 0..40 {
            e.step(20);
            let s = block_on(e.read_state());
            decay[k] += e.sys.velocity(s.vel[0][0]) / 20.0;
        }
        e.step(2000);
    }
    eprintln!("thrown at 1 σ/τ: v(t) every 0.1 τ: {}", decay.iter().map(|v| format!("{v:.3}")).collect::<Vec<_>>().join(" "));
    let mut integral = 0.0;
    for v in &decay {
        integral += v * 0.1;
    }
    eprintln!("grain mass {m:.1}; ∫v/v₀ dt = {integral:.3} τ; ζ = {:.1}; ({:.0} s)", m / integral, t0.elapsed().as_secs_f64());
}
