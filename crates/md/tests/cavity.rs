//! Cavitation (the `cavity` scenario): a liquid held between wetting walls, stretched by pulling
//! the lid up, pulls back on it (negative pressure, tension) until it tears.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

fn lj(r: f64) -> f64 {
    if r >= 2.5 {
        return 0.0;
    }
    let x6 = r.powi(-6);
    24.0 * x6 * (2.0 * x6 - 1.0) / r
}

/// The liquid's pull on the lid per unit width (+: pulling it down, tension), and the lid's height.
pub fn pull(e: &GpuEngine, w: f64) -> (f64, f64) {
    let s = block_on(e.read_state());
    let lid: Vec<[f64; 2]> = (0..s.n()).filter(|&i| s.species[i] == 3).map(|i| [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])]).collect();
    let y_lid = lid[0][1];
    let mut fy = 0.0;
    for i in 0..s.n() {
        if s.species[i] != 0 {
            continue;
        }
        let p = [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])];
        if (p[1] - y_lid).abs() > 2.5 {
            continue;
        }
        for q in &lid {
            let mut dx = q[0] - p[0];
            dx -= w * (dx / w).round();
            let dy = q[1] - p[1];
            let r = (dx * dx + dy * dy).sqrt();
            fy += lj(r) * dy / r; // force on the lid bead, y: + is up (pushed); a pull is negative
        }
    }
    (-fy / w, y_lid)
}

#[test]
#[ignore = "an experiment: stretching a liquid until it tears"]
fn liquid_under_tension_tears() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let speed: f64 = std::env::var("SPEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0.02);
    let t: f64 = std::env::var("T").ok().and_then(|s| s.parse().ok()).unwrap_or(0.40);
    let rho: f64 = std::env::var("RHO").ok().and_then(|s| s.parse().ok()).unwrap_or(0.75);
    let wet: f64 = std::env::var("WET").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let sc = scenarios::build("cavity", &Options::default().with("temperature", t).with("density", rho).with("wet", wet)).unwrap();
    eprintln!("wet {wet}");
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let w = 60.0;
    e.step(10000);
    let mut line = vec![];
    let avg = |e: &mut GpuEngine| {
        let (mut f, mut y) = (0.0, 0.0);
        for _ in 0..40 {
            e.step(50);
            let (a, b) = pull(e, w);
            f += a / 40.0;
            y += b / 40.0;
        }
        (f, y)
    };
    let (f0, y0) = avg(&mut e);
    line.push(format!("{:.1}:{f0:.3}", y0));
    e.set_species_velocity(3, [0.0, speed]);
    let blocks: usize = std::env::var("BLOCKS").ok().and_then(|s| s.parse().ok()).unwrap_or(40);
    for _ in 0..blocks {
        e.step(4000);
        let (f, y) = avg(&mut e);
        line.push(format!("{:.1}:{f:.3}", y - y0));
    }
    eprintln!("cavity: kT {t}, density {rho}, speed {speed}: (stretch: tension) {}", line.join(" "));
    let s = block_on(e.read_state());
    let mut prof = vec![0usize; 64];
    for i in 0..s.n() {
        if s.species[i] == 0 {
            prof[(fixed::to_f64(s.pos[i][1]) as usize).min(63)] += 1;
        }
    }
    eprintln!("profile (per σ of height, ÷ width): {}", prof.iter().map(|&c| format!("{:.2}", c as f64 / w)).collect::<Vec<_>>().join(" "));
}
