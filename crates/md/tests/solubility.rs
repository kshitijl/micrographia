//! Salt dissolving in water (the `solubility` scenario), and solubility against temperature.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

/// Dissolved ions: those with no opposite ion within 1.4 σ (not in contact with the crystal or a
/// pair).
fn dissolved(e: &GpuEngine) -> usize {
    let s = block_on(e.read_state());
    let b = e.sys.box_f64();
    let ions: Vec<(usize, [f64; 2])> = (0..s.n()).filter(|&i| s.species[i] > 0).map(|i| (i, [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])])).collect();
    ions.iter()
        .filter(|(i, p)| {
            !ions.iter().any(|(j, q)| {
                if s.species[*i] == s.species[*j] {
                    return false;
                }
                let mut d = [q[0] - p[0], q[1] - p[1]];
                for a in 0..2 {
                    d[a] -= b[a] * (d[a] / b[a]).round();
                }
                d[0] * d[0] + d[1] * d[1] < 1.96
            })
        })
        .count()
}

#[test]
#[ignore = "an experiment: how much salt dissolves at each temperature"]
fn solubility_against_temperature() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let salt: f64 = std::env::var("SALT").ok().and_then(|s| s.parse().ok()).unwrap_or(2.5);
    let hyd: f64 = std::env::var("HYD").ok().and_then(|s| s.parse().ok()).unwrap_or(1.3);
    for t in [0.7, 0.9, 1.1] {
        let o = Options::default().with("temperature", t).with("salt", salt).with("hydration", hyd);
        let sc = scenarios::build("solubility", &o).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = vec![];
        let rounds: usize = std::env::var("ROUNDS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
        for _ in 0..rounds {
            e.step(20000);
            line.push(dissolved(&e).to_string());
        }
        eprintln!("salt {salt}, hydration {hyd}, T {t}: dissolved ions (of 256) every 100 τ: {}", line.join(" "));
    }
}
