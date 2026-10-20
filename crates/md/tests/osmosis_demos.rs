//! The osmosis page's two demos: a free membrane that moves, and an osmometer whose level rises.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// With 50% of the right side's sites solute, the membrane moves from 48 σ to ~33 σ in 500 τ.
#[test]
#[ignore]
fn free_membrane_moves() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let f: f64 = std::env::var("SOLUTE").map_or(0.5, |v| v.parse().unwrap());
    let sc = build("osmosis_free", &Options::default().with("solute", f)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let mut line = String::new();
    for _ in 0..10 {
        e.step(10000);
        let s = block_on(e.read_state());
        let xs: Vec<f64> = (0..s.n()).filter(|&i| s.species[i] == 2).map(|i| fixed::to_f64(s.pos[i][0])).collect();
        let solute_left = (0..s.n()).filter(|&i| s.species[i] == 1 && fixed::to_f64(s.pos[i][0]) < xs.iter().cloned().fold(f64::MAX, f64::min)).count();
        line += &format!(" {:.1}", xs.iter().sum::<f64>() / xs.len() as f64);
        if solute_left > 0 {
            line += &format!("(leak {solute_left})");
        }
    }
    eprintln!("membrane x every 50 τ:{line}");
}

/// Measured with the defaults (16 σ arms, g = 0.006, 18% solute, LJ cut at 4 σ, kT 0.45): Δh
/// rises from 0 to ~10–11 σ by 1400 τ and holds at 10–12 σ against van 't Hoff's 10.9–11. With
/// the usual 2.5 σ cutoff the liquid is too close to its critical point (Tc ≈ 0.46) and the levels
/// creep and fluctuate; with weak gravity and wide arms it takes many thousands of τ.
#[test]
#[ignore]
fn osmometer_rises() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let g: f64 = std::env::var("G").map_or(0.006, |v| v.parse().unwrap());
    let f: f64 = std::env::var("SOLUTE").map_or(0.18, |v| v.parse().unwrap());
    let width: f64 = std::env::var("WIDTH").map_or(32.0, |v| v.parse().unwrap());
    let sc = build("osmometer", &Options::default().with("gravity", g).with("solute", f).with("width", width)).unwrap();
    let w = fixed::to_f64(sc.sys.box_len[0]);
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    for k in 0..15 {
        e.step(40000);
        let s = block_on(e.read_state());
        // Liquid level per arm: molecules below 80 σ per unit width, over the liquid density.
        let (mut l, mut r, mut sol, mut solh) = (0.0, 0.0, 0.0, 0.0);
        for i in 0..s.n() {
            let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
            if y > 90.0 {
                continue;
            }
            if x < w / 2.0 { l += 1.0 } else { r += 1.0 }
            if s.species[i] == 1 {
                sol += 1.0;
                solh += y;
            }
        }
        let rho = {
            let n = (0..s.n()).filter(|&i| {
                let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
                x > 4.0 && x < w / 2.0 - 4.0 && y > 10.0 && y < 25.0
            }).count();
            n as f64 / ((w / 2.0 - 8.0) * 15.0)
        };
        let (hl, hr) = (l / (rho * w / 2.0), r / (rho * w / 2.0));
        let c = sol / (hr * w / 2.0);
        let pred = c * 0.45 / (rho * g);
        eprintln!("{} τ: level left {hl:.1}, right {hr:.1}, Δh {:.1}; solute mean height {:.1}; van 't Hoff Δh {pred:.1}; liquid density {rho:.3}", (k + 1) * 200, hr - hl, solh / sol);
    }
}
