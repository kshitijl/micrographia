//! The textbook ensemble: boxes behind diathermal walls relaxing to one Boltzmann distribution.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_ref::IntEngine;
use pollster::block_on;
use scenarios::{build, Options};

/// Boltzmann's H-function entropy per molecule of the pooled kinetic energies, −Σ p ln(p/ΔE)
/// (in 2D the density of states in kinetic energy is flat), from a histogram with bins ΔE.
pub fn entropy(ke: &[f64], de: f64) -> f64 {
    let mut h = std::collections::HashMap::new();
    for &e in ke {
        *h.entry((e / de) as i64).or_insert(0.0) += 1.0;
    }
    let n = ke.len() as f64;
    h.values().map(|&c| -(c / n) * ((c / n) / de).ln()).sum()
}

#[test]
#[ignore]
fn relaxation() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for start in 0..5 {
        let o = Options::default().with("start", start as f64);
        let sc = build("ensemble", &o).unwrap();
        let sys = sc.sys.clone();
        let e0 = IntEngine::<f64>::new(sys.clone(), sc.state.clone()).energy().total();
        let mut e = GpuEngine::new(&p, sys.clone(), &sc.state);
        let mut line = String::new();
        for t in [0, 1, 2, 5, 10, 20, 50, 100, 200] {
            let target = (t as f64 / sys.dt).round() as u64;
            let now = e.steps as u64;
            e.step((target - now) as usize);
            let s = block_on(e.read_state());
            let ke: Vec<f64> = s.vel.iter().map(|v| v.map(|x| sys.velocity(x))).map(|v| 0.5 * (v[0] * v[0] + v[1] * v[1])).collect();
            let mut boxes = vec![(0.0, 0.0); 48];
            for i in 0..s.n() {
                let c = sys.compartment_of(s.pos[i]);
                let b = (c[1] * 8 + c[0]) as usize;
                boxes[b].0 += ke[i];
                boxes[b].1 += 1.0;
            }
            let tb: Vec<f64> = boxes.iter().map(|(e, n)| e / n).collect();
            let spread = (tb.iter().map(|t| (t - 1.0) * (t - 1.0)).sum::<f64>() / 48.0).sqrt();
            line += &format!(" t{t}: S {:.3} sd(T) {spread:.2};", entropy(&ke, 0.05));
        }
        let e1 = IntEngine::<f64>::new(sys.clone(), block_on(e.read_state())).energy().total();
        eprintln!("start {start}:{line} E {e0:.1} → {e1:.1} (S max {:.3})", 1.0 + (1.0f64).ln());
    }
}

/// Total energy drift over 200 τ, diathermal against insulating walls (same start).
#[test]
#[ignore]
fn energy_drift() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (diathermal, start, dt) in [(true, 0.0, 0.005), (false, 0.0, 0.005), (true, 3.0, 0.005), (true, 3.0, 0.0025)] {
        let sc = build("ensemble", &Options::default().with("start", start).with("dt", dt)).unwrap();
        let mut sys = sc.sys.clone();
        if !diathermal {
            sys.compartment_walls = md_core::system::CompartmentWalls::Insulating;
        }
        let en = |s| IntEngine::<f64>::new(sys.clone(), s).energy().total();
        let e0 = en(sc.state.clone());
        let mut e = GpuEngine::new(&p, sys.clone(), &sc.state);
        let mut line = String::new();
        for _ in 0..4 {
            e.step((50.0 / dt) as usize);
            line += &format!(" {:.2}", en(block_on(e.read_state())));
        }
        eprintln!("diathermal {diathermal}, start {start}, dt {dt}: E {e0:.2} →{line} (every 50 τ)");
    }
}
