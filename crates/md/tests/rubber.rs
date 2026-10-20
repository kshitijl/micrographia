//! The entropic spring: tension of pinned bead–spring chains against temperature.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

fn tension(t: f64, eq: usize, frames: usize) -> (f64, f64) {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = build("rubber", &Options::default().with("chains", 64.0).with("stretch", 0.5).with("temperature", t)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(eq);
    let (mut f, mut ke, mut nke) = (0.0, 0.0, 0.0);
    for _ in 0..frames {
        e.step(100);
        let s = block_on(e.read_state());
        let n = s.n();
        let mut anchors = vec![];
        let mut beads = vec![];
        for i in 0..n {
            match s.species[i] {
                3 => anchors.push(s.pos[i]),
                2 => beads.push(s.pos[i]),
                _ => {}
            }
            if !sc.sys.is_pinned(s.species[i]) {
                let vx = sc.sys.velocity(s.vel[i][0]);
                let vy = sc.sys.velocity(s.vel[i][1]);
                ke += 0.5 * (vx * vx + vy * vy);
                nke += 1.0;
            }
        }
        for (a, b) in anchors.iter().zip(&beads) {
            let dx = fixed::to_f64(a[0] - b[0]);
            let dy = fixed::to_f64(a[1] - b[1]);
            let r2 = dx * dx + dy * dy;
            let mut fr = -30.0 / (1.0 - (r2 / 2.25).min(0.999999));
            if r2 < 1.2599 {
                let s6 = 1.0 / (r2 * r2 * r2);
                fr += 24.0 * s6 * (2.0 * s6 - 1.0) / r2;
            }
            f += -fr * dx / anchors.len() as f64;
        }
    }
    (f / frames as f64, ke / nke)
}

#[test]
#[ignore]
fn tension_against_temperature() {
    for (t, eq) in [(0.5, 20000), (1.0, 20000), (2.0, 20000), (2.0, 60000)] {
        let (f, tm) = tension(t, eq, 200);
        eprintln!("T = {t} (measured {tm:.3}), equilibrated {eq} steps: tension {f:.3}, tension/T {:.3}", f / tm);
    }
}

/// With a spring gauge on the right ends: how far the springs stretch at kT 1 and 2.
#[test]
#[ignore]
fn spring_gauge() {
    let p = gpu_prims::Prims::new(&pollster::block_on(gpu_prims::Gpu::new()).unwrap());
    for t in [1.0, 2.0] {
        let o = scenarios::Options::default().with("chains", 64.0).with("length", 40.0).with("stretch", 0.5).with("temperature", t).with("gauge", 0.3).with("seed", 10.0);
        let sc = scenarios::build("rubber", &o).unwrap();
        let mut e = md::GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = String::new();
        for k in 0..6 {
            e.step(10000);
            let s = pollster::block_on(e.read_state());
            // Each chain ends: ..., handle (species 3), post (species 1).
            let mut d = 0.0;
            let mut n = 0.0;
            for i in 0..s.n() {
                if s.species[i] == 3 {
                    d += md_core::fixed::to_f64(s.pos[i + 1][0] - s.pos[i][0]);
                    n += 1.0;
                }
            }
            line += &format!(" {}τ: {:.2}σ", (k + 1) * 50, d / n);
        }
        eprintln!("kT {t}: spring stretch{line}");
    }
}
