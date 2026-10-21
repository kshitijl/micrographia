//! Classical heat capacities (the `heatcap` scenario), per molecule, in 2D: single atoms 1k,
//! two-atom molecules on springs 2.5k (2 of translation, 1/2 of rotation, 1 of vibration:
//! kinetic and potential), a crystal 2k per atom. Equipartition, whatever the spring's stiffness.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use pollster::block_on;
use scenarios::Options;

/// (ΔE / ΔT) per molecule between two temperatures, from heating by velocity scaling.
fn capacity(p: &Prims, kind: f64, k: f64) -> (f64, f64) {
    let o = Options::default().with("kind", kind).with("k", k);
    let sc = scenarios::build("heatcap", &o).unwrap();
    let mut e = GpuEngine::new(p, sc.sys.clone(), &sc.state);
    let atoms = e.n() as f64;
    let molecules = if kind == 1.0 { atoms / 2.0 } else { atoms };
    let measure = |e: &mut GpuEngine| {
        let (mut t, mut u) = (0.0, 0.0);
        for _ in 0..50 {
            e.step(200);
            let s = block_on(e.read_state());
            let ke = s.half_step_kinetic(&e.sys);
            t += ke / atoms / 50.0;
            let (_, pe) = block_on(e.buf.read_kicks());
            u += (ke + pe.iter().map(|&x| x as f64).sum::<f64>()) / molecules / 50.0;
        }
        (t, u)
    };
    e.step(20000);
    let (t0, u0) = measure(&mut e);
    let b = e.sys.box_f64();
    e.kick_region([-1.0, -1.0], [b[0] + 1.0, b[1] + 1.0], 1.3, [0.0, 0.0]);
    e.step(20000);
    let (t1, u1) = measure(&mut e);
    ((u1 - u0) / (t1 - t0), t1)
}

#[test]
fn equipartition_heat_capacities() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (kind, k, want, name) in [(0.0, 200.0, 1.0, "atoms"), (1.0, 200.0, 2.5, "molecules, k 200"), (1.0, 1000.0, 2.5, "molecules, k 1000"), (2.0, 200.0, 2.0, "crystal")] {
        let (c, t) = capacity(&p, kind, k);
        eprintln!("heatcap: {name}: {c:.3} k per molecule (equipartition {want}) at kT {t:.3}");
        assert!((c - want).abs() < 0.12 * want, "{name}");
    }
}
