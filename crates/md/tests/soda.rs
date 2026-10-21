//! Gas solubility (the `soda` scenario): CO₂ that the water attracts splits between the pool and
//! the headspace; the share dissolved falls when warmer (Henry's constant falls with T).

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

/// (dissolved per σ² of pool, in the headspace per σ²), averaged over the last half.
pub fn split(p: &Prims, t: f64, solvation: f64, steps: usize) -> (f64, f64) {
    let o = Options::default().with("temperature", t).with("solvation", solvation);
    let sc = scenarios::build("soda", &o).unwrap();
    let mut e = GpuEngine::new(p, sc.sys.clone(), &sc.state);
    let (w, h, depth) = (100.0, 80.0, 20.0);
    let (mut cl, mut cg, mut k) = (0.0, 0.0, 0);
    for b in 0..steps / 1000 {
        e.step(1000);
        if b < steps / 2000 {
            continue;
        }
        let s = block_on(e.read_state());
        // The surface: where the water's density falls to half, from a coarse profile.
        let mut prof = vec![0.0; 80];
        for i in 0..s.n() {
            if s.species[i] == 0 {
                let y = fixed::to_f64(s.pos[i][1]) as usize;
                if y < 80 {
                    prof[y] += 1.0 / w;
                }
            }
        }
        if b == steps / 1000 - 1 {
            eprintln!("  profile: {}", prof.iter().step_by(4).map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" "));
        }
        let surf = (6..78).find(|&y| prof[y] < 0.35 && prof[y + 1] < 0.35).unwrap_or(depth as usize) as f64;
        let (mut nl, mut ng) = (0, 0);
        for i in 0..s.n() {
            if s.species[i] != 1 {
                continue;
            }
            let y = fixed::to_f64(s.pos[i][1]);
            if y < surf - 2.0 {
                nl += 1;
            } else if y > surf + 4.0 {
                ng += 1;
            }
        }
        cl += nl as f64 / (w * (surf - 2.0));
        cg += ng as f64 / (w * (h - surf - 5.5));
        k += 1;
    }
    (cl / k as f64, cg / k as f64)
}

#[test]
#[ignore = "an experiment: gas solubility against temperature"]
fn warm_water_holds_less_gas() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let solv: f64 = std::env::var("SOLV").ok().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    for t in [0.40, 0.425, 0.45] {
        let (cl, cg) = split(&p, t, solv, 200_000);
        eprintln!("soda: solvation {solv}, kT {t}: dissolved {cl:.4}, gas {cg:.4}: ratio {:.2}", cl / cg);
    }
}
