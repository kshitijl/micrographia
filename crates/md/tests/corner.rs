//! How often are all of a box's molecules in its left half? Against 2^-N.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// Measured / 2^-N. With the old default size 0.5 σ: 0.81 at 6 per box, 0.50 at 10 (the
/// excluded-area correction e^(−N(N−1)b/2A), b ≈ πd², predicts 0.80 and 0.54). With 0.25 σ
/// (the new default): 6 per box, seeds 1–4: 1.01, 0.94, 0.92, 0.93 (predicted 0.955); the
/// seed-to-seed scatter is larger than counting suggests, since the dilute gas mixes slowly.
#[test]
#[ignore]
fn all_on_one_side() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (size, n, seed) in [(0.25, 6, 1.0), (0.25, 6, 2.0), (0.25, 6, 3.0), (0.25, 6, 4.0)] {
        let o = Options::default().with("per_box", n as f64).with("size", size).with("seed", seed);
        let sc = build("corner", &o).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        e.step(sc.warmup);
        let (bx, by) = (32usize, 24usize);
        let (mut hits, mut samples) = (0.0, 0.0);
        for _ in 0..400 {
            e.step(200);
            let s = block_on(e.read_state());
            let mut left = vec![0usize; bx * by];
            for q in &s.pos {
                let (x, y) = (fixed::to_f64(q[0]), fixed::to_f64(q[1]));
                let b = (y / 8.0) as usize * bx + (x / 8.0) as usize;
                if x % 8.0 < 4.0 {
                    left[b] += 1;
                }
            }
            hits += left.iter().filter(|&&c| c == n).count() as f64;
            samples += (bx * by) as f64;
        }
        let ratio = hits / samples / 0.5f64.powi(n as i32);
        eprintln!("size {size}, {n} per box, seed {seed}: {hits} events, measured/2^-N = {ratio:.2}");
    }
}
