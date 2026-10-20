//! A filter of fibres in a fan-driven air flow: which aerosol sizes get through?

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// (captures, clean passes through the filter, mean air speed) over `frames` snapshots.
/// An aerosol is captured once it sits within 1.3 × contact of a fibre for 5 snapshots in a
/// row; it passes when it gets from upstream of the filter to downstream of it without being
/// captured (going back and forth across the downstream edge counts once).
pub fn filtration(e: &mut GpuEngine, o: &Options, frames: usize, every: usize) -> (f64, f64, f64) {
    let s0 = block_on(e.read_state());
    let (rf, size, gap, rows) = (o.get("fiber", 2.0), o.get("size", 1.0), o.get("gap", 8.0), o.get("rows", 3.0));
    let w = fixed::to_f64(e.sys.box_len[0]);
    let pitch = 2.0 * rf + gap;
    let (f0, f1) = (w / 2.0 - rf - size, w / 2.0 + (rows - 1.0) * pitch * 0.866 + rf + size);
    let fibers: Vec<[f64; 2]> = (0..s0.n()).filter(|&i| s0.species[i] == 1).map(|i| [fixed::to_f64(s0.pos[i][0]), fixed::to_f64(s0.pos[i][1])]).collect();
    let touch = 1.3 * (rf + size / 2.0);
    let ids: Vec<usize> = (0..s0.n()).filter(|&i| s0.species[i] == 2).collect();
    let mut stuck = vec![0usize; ids.len()];
    let mut captured = vec![false; ids.len()];
    // Whether each aerosol was last seen upstream of the filter (it starts there).
    let mut upstream = vec![true; ids.len()];
    let (mut caps, mut passes, mut speed) = (0.0, 0.0, 0.0);
    for _ in 0..frames {
        e.step(every);
        let s = block_on(e.read_state());
        let (mut sv, mut c) = (0.0, 0.0);
        for i in 0..s.n() {
            if s.species[i] == 0 {
                sv += e.sys.velocity(s.vel[i][0]);
                c += 1.0;
            }
        }
        speed += sv / c / frames as f64;
        for (k, &i) in ids.iter().enumerate() {
            if captured[k] {
                continue;
            }
            let q = [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])];
            let on = fibers.iter().any(|f| (f[0] - q[0]).powi(2) + (f[1] - q[1]).powi(2) < touch * touch);
            stuck[k] = if on { stuck[k] + 1 } else { 0 };
            if stuck[k] >= 5 {
                captured[k] = true;
                caps += 1.0;
                continue;
            }
            if q[0] < f0 {
                upstream[k] = true;
            } else if q[0] >= f1 && upstream[k] {
                passes += 1.0;
                upstream[k] = false;
            }
        }
    }
    (caps, passes, speed)
}

/// Single-pass efficiency by size, with two rows of fibres (4 σ across, 8 σ gaps), as on the
/// `mask` page. Measured (seed 1): 0.41, 0.36, 0.40, 0.76 for sizes 0.3, 1.2, 2.4, 4.8 σ; the
/// page's seed gives 0.36, 0.40, 0.44, 0.76. So particles 7–27× smaller than the gaps are caught
/// on ~40% of passes and the largest on ~3/4; the small–medium difference is within the noise.
/// Four rows: 0.67, 0.62, 1.00 for 0.3, 1.2, 4.8. A deeper well (ε 24) changes little.
#[test]
#[ignore]
fn efficiency_by_size() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let mut eff = vec![];
    for size in [0.3, 1.2, 2.4, 4.8] {
        let aerosols = if size > 3.0 { 30.0 } else if size > 2.0 { 60.0 } else { 120.0 };
        let o = Options::default().with("size", size).with("rows", 2.0).with("aerosols", aerosols).with("fan", 1.0);
        let sc = build("mask", &o).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let (c, ps, u) = filtration(&mut e, &o, 150, 1000);
        eprintln!("size {size}: {c} captured, {ps} passed, efficiency {:.2}, air speed {u:.3}", c / (c + ps));
        eff.push(c / (c + ps));
    }
    assert!(eff[..3].iter().all(|&e| e > 0.2));
    assert!(eff[3] > eff[1] + 0.2);
}
