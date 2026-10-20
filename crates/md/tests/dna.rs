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

/// Melting curves for strands of 1, 4 and 8 beads, each temperature from a fresh start (the
/// unpaired lattice). Measured: single beads 2–16% paired at all temperatures; 4 beads melt at
/// kT ≈ 0.72 and 8 beads at ≈ 0.95 (the page, swept from hot to cold: 0.73, 0.94). Effective
/// van 't Hoff energies from the slopes of ln K against 1/T: 1.2, 5.2 and 8 ε.
#[test]
#[ignore]
fn melting_curves() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for eps in [1.5] {
        for t in [0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3] {
            let o = Options::default().with("eps", eps).with("temperature", t).with("columns", 12.0).with("rows", 4.0).with("lengths", 148.0);
            let sc = build("dna", &o).unwrap();
            let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
            e.step(40000);
            let a = paired(&mut e, 16.0, 12, 4, 3, 50, 1000);
            let b = paired(&mut e, 16.0, 12, 4, 3, 50, 1000);
            eprintln!("eps {eps} T {t}: paired (lengths 1, 4, 8) {:.2} {:.2} {:.2}, later {:.2} {:.2} {:.2}", a[0], a[1], a[2], b[0], b[1], b[2]);
        }
    }
}

/// How many beads of each length-8 A strand are paired, as a histogram over boxes and
/// snapshots, swept from hot to cold as on the page. Near the melting temperature (kT 0.9–1.0):
/// a peak at none, a dip at one or two, and a broad hump of mostly zipped strands with frayed
/// ends; not the sharp all-or-none of real DNA, which needs stacking. Stiffer strands
/// (STIFFNESS=50) melt higher (~1.1) but no more sharply.
#[test]
#[ignore]
fn all_or_none() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let k = std::env::var("STIFFNESS").map_or(10.0, |v| v.parse().unwrap());
    let o = Options::default().with("stiffness", k).with("temperature", 1.3).with("columns", 12.0).with("rows", 4.0).with("lengths", 148.0);
    let sc = build("dna", &o).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let side = 16.0;
    for t in [1.3, 1.2, 1.1, 1.0, 0.9, 0.8, 0.7, 0.6, 0.5] {
        e.update_system(e.sys.clone().with_thermostat(t, 0.5));
        e.step(20000);
        let mut hist = [0usize; 9];
        let mut share = [0.0; 3];
        for _ in 0..50 {
            e.step(1000);
            let s = block_on(e.read_state());
            let q: Vec<[f64; 2]> = s.pos.iter().map(|x| [fixed::to_f64(x[0]), fixed::to_f64(x[1])]).collect();
            let mut beads: Vec<[Vec<usize>; 2]> = vec![[vec![], vec![]]; 12 * 12];
            for i in 0..s.n() {
                beads[(q[i][1] / side) as usize * 12 + (q[i][0] / side) as usize][s.species[i] as usize].push(i);
            }
            for (b, [a, bb]) in beads.iter().enumerate() {
                let close = a.iter().filter(|&&i| bb.iter().any(|&j| (q[i][0] - q[j][0]).powi(2) + (q[i][1] - q[j][1]).powi(2) < 2.25)).count();
                let band = b / 48;
                if 2 * close > a.len() {
                    share[band] += 1.0 / (50.0 * 48.0);
                }
                if band == 2 {
                    hist[close] += 1;
                }
            }
        }
        eprintln!("T {t}: paired {:.2} {:.2} {:.2}; length 8 histogram {hist:?}", share[0], share[1], share[2]);
    }
}
