//! The cloud chamber's α particle (scenario cloud): fired at 10 σ/τ through the air, it meets
//! molecules only through a tiny hard core; at a time step of 0.001 τ those sharp collisions
//! still conserve energy.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use pollster::block_on;
use scenarios::Options;

#[test]
fn alpha_collisions_conserve_energy() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = scenarios::build("cloud", &Options::default()).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.set_species(&[0], 1);
    e.set_species_velocity(1, [10.0, 0.3]);
    let energy = |e: &GpuEngine| {
        let s = block_on(e.read_state());
        let (_, pe) = block_on(e.buf.read_kicks());
        s.half_step_kinetic(&e.sys) + pe.iter().map(|&x| x as f64).sum::<f64>()
    };
    let e0 = energy(&e);
    let mut path = vec![];
    for _ in 0..20 {
        e.step(1000);
        let s = block_on(e.read_state());
        path.push(format!("({:.0},{:.0})", md_core::fixed::to_f64(s.pos[0][0]), md_core::fixed::to_f64(s.pos[0][1])));
    }
    let e1 = energy(&e);
    eprintln!("cloud: energy {e0:.2} → {e1:.2}; α path {}", path.join(" "));
    assert!((e1 - e0).abs() < 1e-3 * e0.abs().max(50.0));
}
