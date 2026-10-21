//! Heating at constant volume and at constant pressure (the `enthalpy` scenario): the heat goes
//! into the gas's energy, and at constant pressure also into lifting the load: Q = ΔU + pΔV = ΔH.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

fn kinetic(e: &GpuEngine) -> f64 {
    let s = block_on(e.read_state());
    s.half_step_kinetic(&e.sys)
}

/// Heat by `q` per molecule in `doses` steps; returns (ΔT per molecule of heat, Δh, final T).
fn heat(p: &Prims, locked: bool, q: f64, doses: usize) -> (f64, f64, f64, f64) {
    let o = Options::default().with("locked", if locked { 1.0 } else { 0.0 });
    let sc = scenarios::build("enthalpy", &o).unwrap();
    let mut e = GpuEngine::new(p, sc.sys.clone(), &sc.state);
    let n = e.n() as f64;
    let b = e.sys.box_f64();
    e.step(20000);
    let t_of = |e: &GpuEngine| kinetic(e) / n;
    let (mut t0, mut h0) = (0.0, 0.0);
    for _ in 0..100 {
        e.step(500);
        t0 += t_of(&e) / 100.0;
        h0 += block_on(e.read_piston())[0] / 100.0;
    }
    let mut added = 0.0;
    for _ in 0..doses {
        let k = kinetic(&e);
        let s = (1.0 + q * n / doses as f64 / k).sqrt();
        e.kick_region([-1.0, -1.0], [b[0] + 1.0, b[1] + 1.0], s, [0.0, 0.0]);
        added += k * (s * s - 1.0);
        e.step(500);
    }
    e.step(20000);
    let (mut t1, mut h1) = (0.0, 0.0);
    for _ in 0..100 {
        e.step(500);
        t1 += t_of(&e) / 100.0;
        h1 += block_on(e.read_piston())[0] / 100.0;
    }
    let _ = fixed::ONE;
    ((t1 - t0) / (added / n), h1 - h0, t1, added)
}

#[test]
fn heat_capacity_at_constant_pressure_is_twice_that_at_constant_volume() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let (dv, _, tv, qv) = heat(&p, true, 0.5, 400);
    let (dp, dh, tp, qp) = heat(&p, false, 0.5, 400);
    eprintln!("constant volume: ΔT per unit heat per molecule {dv:.3} (T → {tv:.3}, Q {qv:.1}); constant pressure: {dp:.3}, the lid rose {dh:.1} σ (T → {tp:.3}, Q {qp:.1})");
    // 2D, nearly ideal: c_V = 1, c_p = 2 per molecule.
    assert!((dv - 1.0).abs() < 0.1);
    assert!((dp - 0.5).abs() < 0.07);
    assert!(dh > 10.0);
}
