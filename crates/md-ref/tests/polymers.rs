//! Bonds (Kremer–Grest bead–spring chains) in the CPU engine.

use md_core::{fixed, init, BondType, Pair, State, System};
use md_ref::IntEngine;

pub fn melt(chains: usize, length: usize, density: f64, seed: u64) -> (System, State) {
    let n = chains * length;
    let sys0 = System::new(init::box_for(n, density, 1.0), init::DT, Pair::wca(), vec![1.0]);
    let (pos, bonds) = init::chains(&sys0, chains, length, 0.97);
    let sys = sys0.with_bonds(vec![BondType::kremer_grest()], bonds);
    let species = vec![0; n];
    let vel = init::maxwell(&sys, &species, 1.0, seed);
    (sys, State { pos, vel, species })
}

#[test]
fn chains_are_exactly_reversible() {
    let (sys, s0) = melt(20, 20, 0.3, 1);
    let mut e = IntEngine::<f32>::new(sys, s0.clone());
    e.step(2000);
    e.reverse();
    e.step(2000);
    e.reverse();
    assert_eq!(e.state, s0);
}

#[test]
fn chains_hold_together_and_conserve_energy() {
    let (sys, s0) = melt(20, 20, 0.3, 2);
    let mut e = IntEngine::<f64>::new(sys.clone(), s0);
    e.step(500);
    let e0 = e.energy().total();
    let mut worst = 0.0f64;
    for _ in 0..10 {
        e.step(200);
        worst = worst.max((e.energy().total() - e0).abs() / 400.0);
    }
    // Mean bond length near the Kremer–Grest value (~0.97 σ).
    let mut sum = 0.0;
    for b in sys.bonds.iter() {
        let (p, q) = (e.state.pos[b[0] as usize], e.state.pos[b[1] as usize]);
        let dx = fixed::to_f64(fixed::min_image(p[0], q[0], sys.box_len[0]));
        let dy = fixed::to_f64(fixed::min_image(p[1], q[1], sys.box_len[1]));
        sum += (dx * dx + dy * dy).sqrt();
    }
    let mean = sum / sys.bonds.len() as f64;
    eprintln!("bead–spring chains: energy drift per bead {worst:.1e}, mean bond length {mean:.3}");
    assert!(worst < 2e-3);
    assert!((mean - 0.97).abs() < 0.03);
}
