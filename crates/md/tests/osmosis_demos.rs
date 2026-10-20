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

/// The surface height of the liquid in x ∈ [x0, x1): the top of the density profile (1 σ bins,
/// smoothed over 3), where it falls through 0.35 (vapour ~0.01, liquid ~0.7); and the liquid's
/// density, averaged over the lower half of the column.
pub fn surface(s: &md_core::State, x0: f64, x1: f64, h: f64) -> (f64, f64) {
    let bins = h as usize;
    let mut c = vec![0.0; bins];
    for i in 0..s.n() {
        let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
        if s.species[i] != 2 && x >= x0 && x < x1 && y >= 0.0 && y < h {
            c[y as usize] += 1.0 / (x1 - x0);
        }
    }
    let sm: Vec<f64> = (0..bins).map(|k| (c[k.saturating_sub(1)] + c[k] + c[(k + 1).min(bins - 1)]) / 3.0).collect();
    let top = (0..bins).rev().find(|&k| sm[k] > 0.35).unwrap_or(0);
    // Interpolate within the bin where it crosses.
    let y = top as f64 + 0.5 + if top + 1 < bins { (sm[top] - 0.35) / (sm[top] - sm[top + 1]).max(1e-9) } else { 0.0 };
    let rho = sm[2..(top / 2).max(3)].iter().sum::<f64>() / ((top / 2).max(3) - 2) as f64;
    (y, rho)
}

/// Measured with the defaults (16 σ arms, g = 0.006, 30% solute, LJ cut at 4 σ, kT 0.5, walls
/// wetted at ε 0.7, an ideal solution), seeds 1 and 22: Δh rises to 8–17 σ within ~800 τ and then
/// jitters between ~4 and ~16 σ, about half of van 't Hoff's ~20 σ on average. The 2D surface of a
/// liquid 16 σ wide is ragged (films climb the walls and the membrane), so a single level reading
/// is uncertain by several σ. Things that made it worse: the usual 2.5 σ cutoff (the liquid at
/// kT 0.45 is too close to its critical point), bare hard walls (the liquid dewets into a blob),
/// fully wetting walls and a self-repelling solute (both gave ~1.5× van 't Hoff), weak gravity and
/// wide arms (thousands of τ). A clean measurement needs a bigger system, or 3D.
#[test]
#[ignore]
fn osmometer_rises() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let g: f64 = std::env::var("G").map_or(0.006, |v| v.parse().unwrap());
    let f: f64 = std::env::var("SOLUTE").map_or(0.3, |v| v.parse().unwrap());
    let width: f64 = std::env::var("WIDTH").map_or(32.0, |v| v.parse().unwrap());
    let wet: f64 = std::env::var("WET").map_or(0.7, |v| v.parse().unwrap());
    let t: f64 = std::env::var("T").map_or(0.5, |v| v.parse().unwrap());
    let seed: f64 = std::env::var("SEED").map_or(1.0, |v| v.parse().unwrap());
    let sc = build("osmometer", &Options::default().with("gravity", g).with("solute", f).with("width", width).with("wetting", wet).with("temperature", t).with("seed", seed)).unwrap();
    let w = fixed::to_f64(sc.sys.box_len[0]);
    let h = fixed::to_f64(sc.sys.box_len[1]);
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    for k in 0..10 {
        e.step(40000);
        let s = block_on(e.read_state());
        let (hl, rl) = surface(&s, 2.0, w / 2.0 - 1.0, h);
        let (hr, rr) = surface(&s, w / 2.0 + 1.0, w - 2.0, h);
        let sol = (0..s.n()).filter(|&i| s.species[i] == 1 && fixed::to_f64(s.pos[i][1]) < hr + 3.0).count() as f64;
        let c = sol / (hr * (w / 2.0 - 1.0));
        let pred = c * t / (0.5 * (rl + rr) * g);
        eprintln!("{} τ: surface left {hl:.1}, right {hr:.1}, Δh {:.1}; van 't Hoff Δh {pred:.1}; densities {rl:.3} {rr:.3}", (k + 1) * 200, hr - hl);
    }
}
