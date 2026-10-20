//! The model cell swells in pure water and shrinks in brine.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// Area enclosed by the membrane ring (the first `beads` particles), by the shoelace formula.
pub fn ring_area(s: &md_core::State, beads: usize) -> f64 {
    let p: Vec<[f64; 2]> = (0..beads).map(|i| [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])]).collect();
    let mut a = 0.0;
    for k in 0..beads {
        let (u, v) = (p[k], p[(k + 1) % beads]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    0.5 * a.abs()
}

#[test]
#[ignore]
fn swelling_and_shrinking() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (k, outside) in [(2.0, 0.0), (2.0, 0.35), (2.0, 0.9)] {
        let sc = build("cell", &Options::default().with("inside", 0.35).with("outside", outside).with("stiffness", k).with("bend", 20.0)).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let a0 = ring_area(&sc.state, 44);
        let mut line = vec![];
        for _ in 0..10 {
            e.step(10000);
            line.push(format!("{:.0}", ring_area(&block_on(e.read_state()), 44)));
        }
        eprintln!("k {k}, outside solute {outside}: area {a0:.0} → {}", line.join(" "));
    }
}
