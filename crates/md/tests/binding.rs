//! Tuning and checks for the binding and FEP scenarios.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// Fraction of time ligands of species A (1) and B (3) spend within 1.5 σ of their pocket's
/// centre, over `frames` snapshots.
pub fn occupancy(e: &mut GpuEngine, side: f64, frames: usize, every: usize) -> (f64, f64) {
    let (mut a, mut b, mut na, mut nb) = (0.0, 0.0, 0.0, 0.0);
    for _ in 0..frames {
        e.step(every);
        let s = block_on(e.read_state());
        for i in 0..s.n() {
            let sp = s.species[i];
            if sp == 1 || sp == 3 {
                let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
                let (cx, cy) = (((x / side).floor() + 0.5) * side, ((y / side).floor() + 0.5) * side);
                let bound = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() < 0.8;
                if sp == 1 {
                    na += 1.0;
                    a += bound as u32 as f64;
                } else {
                    nb += 1.0;
                    b += bound as u32 as f64;
                }
            }
        }
    }
    (a / na, b / nb)
}

#[test]
#[ignore]
fn tune_binding() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (ea, eb) in [(0.6, 0.9), (0.8, 1.1), (0.6, 0.6)] {
        let sc = build("binding", &Options::default().with("eps_a", ea).with("eps_b", eb).with("boxes_x", 6.0).with("boxes_y", 6.0)).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        e.step(10000);
        let (fa, fb) = occupancy(&mut e, 16.0, 200, 500);
        let ddg = -((fb / (1.0 - fb)) / (fa / (1.0 - fa))).ln();
        eprintln!("eps {ea}/{eb}: bound A {:.3}, B {:.3}; ΔΔG(B − A) = {ddg:.2} kT", fa, fb);
    }
}
