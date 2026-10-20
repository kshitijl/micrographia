//! DNA melting: strand pairs in many boxes, melting curves by strand length.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// Per length band: the share of boxes whose strands are paired (more than half the beads of
/// the A strand within 1.5 σ of a B bead), averaged over `frames` snapshots.
pub fn paired(e: &mut GpuEngine, side: f64, cols: usize, rows: usize, bands: usize, frames: usize, every: usize) -> Vec<f64> {
    let mut out = vec![0.0; bands];
    for _ in 0..frames {
        e.step(every);
        let s = block_on(e.read_state());
        let p: Vec<[f64; 2]> = s.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
        let nbox = cols * rows * bands;
        let mut beads: Vec<[Vec<usize>; 2]> = vec![[vec![], vec![]]; nbox];
        for i in 0..s.n() {
            let b = (p[i][1] / side) as usize * cols + (p[i][0] / side) as usize;
            beads[b][s.species[i] as usize].push(i);
        }
        for (b, [a, bb]) in beads.iter().enumerate() {
            let close = a.iter().filter(|&&i| bb.iter().any(|&j| (p[i][0] - p[j][0]).powi(2) + (p[i][1] - p[j][1]).powi(2) < 1.5 * 1.5)).count();
            if 2 * close > a.len() {
                out[b / (cols * rows)] += 1.0 / (frames * cols * rows) as f64;
            }
        }
    }
    out
}

#[test]
#[ignore]
fn melting_curves() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for eps in [1.5] {
        for t in [0.3, 0.4, 0.5, 0.6, 0.7, 0.8] {
            let o = Options::default().with("eps", eps).with("temperature", t).with("columns", 12.0).with("rows", 4.0).with("lengths", 248.0);
            let sc = build("dna", &o).unwrap();
            let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
            e.step(20000);
            let a = paired(&mut e, 16.0, 12, 4, 3, 50, 1000);
            let b = paired(&mut e, 16.0, 12, 4, 3, 50, 1000);
            eprintln!("eps {eps} T {t}: paired (lengths 2, 4, 8) {:.2} {:.2} {:.2}, later {:.2} {:.2} {:.2}", a[0], a[1], a[2], b[0], b[1], b[2]);
        }
    }
}
