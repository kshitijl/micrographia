//! Buffers: protons binding to anions in many boxes, one titration curve per run.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// A proton within this distance of an anion is bound to it (the well's minimum is at 0.56 σ).
const BIND: f64 = 0.8;

/// Per column, averaged over boxes and snapshots: (free protons, free anions, bound pairs) per
/// buffered box, and free protons per water box.
pub fn titration(e: &mut GpuEngine, side: f64, cols: usize, rb: usize, frames: usize, every: usize) -> (Vec<[f64; 3]>, Vec<f64>) {
    let mut buf = vec![[0.0; 3]; cols];
    let mut water = vec![0.0; cols];
    for _ in 0..frames {
        e.step(every);
        let s = block_on(e.read_state());
        let p: Vec<[f64; 2]> = s.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
        let boxof = |q: [f64; 2]| ((q[0] / side) as usize, (q[1] / side) as usize);
        let mut anions: Vec<Vec<[f64; 2]>> = vec![vec![]; cols * 64];
        for i in 0..s.n() {
            if s.species[i] == 1 {
                let (bx, by) = boxof(p[i]);
                anions[by * cols + bx].push(p[i]);
            }
        }
        let mut taken: Vec<Vec<bool>> = anions.iter().map(|a| vec![false; a.len()]).collect();
        for i in 0..s.n() {
            if s.species[i] != 0 {
                continue;
            }
            let (bx, by) = boxof(p[i]);
            let b = by * cols + bx;
            let hit = anions[b].iter().position(|a| (a[0] - p[i][0]).powi(2) + (a[1] - p[i][1]).powi(2) < BIND * BIND);
            match (by < rb, hit) {
                (false, _) => water[bx] += 1.0,
                (true, None) => buf[bx][0] += 1.0,
                (true, Some(k)) => taken[b][k] = true,
            }
        }
        for (b, t) in taken.iter().enumerate() {
            let (bx, by) = (b % cols, b / cols);
            if by < rb {
                let bound = t.iter().filter(|&&x| x).count() as f64;
                buf[bx][1] += t.len() as f64 - bound;
                buf[bx][2] += bound;
            }
        }
    }
    let rw = e.sys.compartment_counts()[1] as usize - rb;
    (buf.iter().map(|c| c.map(|x| x / (frames * rb) as f64)).collect(), water.iter().map(|x| x / (frames * rw.max(1)) as f64).collect())
}

/// The law of mass action: [H][A]/[HA] is the same in every column up to the equivalence point
/// (40 protons per box), while free protons vary tenfold. The layout of the `buffer` page.
/// Measured: K ≈ 0.0025 σ⁻², drifting down by ~30% past equivalence, where free protons crowd.
#[test]
#[ignore]
fn mass_action() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let o = Options::default().with("columns", 12.0).with("buffered_rows", 6.0).with("water_rows", 3.0).with("eps", 8.0).with("seed", 29.0);
    let sc = build("buffer", &o).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(40000);
    let area = 32.0 * 32.0;
    let (buf, water) = titration(&mut e, 32.0, 12, 6, 300, 500);
    let mut ks = vec![];
    for (c, (b, w)) in buf.iter().zip(&water).enumerate().skip(1) {
        let k = b[0] * b[1] / b[2] / area;
        eprintln!("{:3} protons: free H {:6.2}, free A {:5.2}, HA {:5.2}, K {:.4}, −log₁₀[H] {:.2} (water {:.2})", c * 8, b[0], b[1], b[2], k, -(b[0] / area).log10(), -(w / area).log10());
        ks.push(k);
    }
    let k = &ks[..5];
    let spread = k.iter().cloned().fold(0.0, f64::max) / k.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(spread < 1.6, "K varies {spread:.2}× up to equivalence");
    assert!(buf[3][0] < 0.25 * water[3]);
}

/// How many anions hold two protons, and how many protons sit in the well's tail (0.8–1 σ).
/// The first designs failed here: with a wider well (σ 0.75, cut at 2 σ) and weaker repulsions
/// a proton bridged two anions and anions held two protons, and K drifted 3.5× across columns.
#[test]
#[ignore]
fn double_binding() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = build("buffer", &Options::default().with("eps", 8.0)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(40000);
    let s = block_on(e.read_state());
    let side = 32.0;
    let pos: Vec<[f64; 2]> = s.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
    let mut per_col = vec![[0usize; 3]; 11];
    for a in 0..s.n() {
        if s.species[a] != 1 {
            continue;
        }
        let col = (pos[a][0] / side) as usize;
        let (mut near, mut tail) = (0, 0);
        for h in 0..s.n() {
            if s.species[h] == 0 {
                let d = ((pos[a][0] - pos[h][0]).powi(2) + (pos[a][1] - pos[h][1]).powi(2)).sqrt();
                if d < BIND {
                    near += 1;
                } else if d < 1.0 {
                    tail += 1;
                }
            }
        }
        per_col[col][0] += (near >= 2) as usize;
        per_col[col][1] += (near == 1) as usize;
        per_col[col][2] += tail;
    }
    for (c, x) in per_col.iter().enumerate() {
        eprintln!("col {c}: {} doubly bound, {} singly, {} protons in tails", x[0], x[1], x[2]);
        // Up to the equivalence point (column 5) fewer than 1 in 10 bound anions hold two; past it,
        // with protons crowding, more do (a source of K's drift there).
        assert!(c > 5 || x[0] * 10 <= x[1].max(10), "column {c}: too many anions hold two protons");
    }
}
