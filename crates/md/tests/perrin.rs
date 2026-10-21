//! Perrin's grains (the `perrin` scenario) settling into the exponential profile.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

#[test]
#[ignore = "an experiment: how fast the grains settle, and their scale height"]
fn grains_settle_exponentially() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = scenarios::build("perrin", &Options::default()).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    for round in 0..10 {
        let mut heights = vec![];
        for _ in 0..50 {
            e.step(2000);
            let s = block_on(e.read_state());
            for i in 0..s.n() {
                if s.species[i] == 1 {
                    heights.push(fixed::to_f64(s.pos[i][1]));
                }
            }
        }
        let mean = heights.iter().sum::<f64>() / heights.len() as f64;
        eprintln!("after {} τ: grains' mean height {mean:.1} σ (an exponential with scale height 12 above the floor's 2 σ: ~14)", (round + 1) * 500);
    }
}
