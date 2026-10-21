//! 3D melting and freezing of a Lennard-Jones crystal (the `crystal3` scenario).

use gpu_prims::{Gpu, Prims};
use md::GpuEngine3;
use md_core::order::solid_bonds;
use md_core::three::State3;
use pollster::block_on;
use scenarios::Options;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

fn crystalline(s: &State3, e: &GpuEngine3) -> f64 {
    let b = solid_bonds(&s.pos, e.sys.box_len, [true; 3], 1.5);
    b.iter().filter(|&&c| c >= 7).count() as f64 / s.n() as f64
}

/// Rescale all velocities so the kinetic temperature is `t` (a heater or a fridge).
fn set_t(e: &mut GpuEngine3, t: f64) {
    let n = e.n() as f64;
    let now = block_on(e.read_kinetic_total()) / (1.5 * n);
    let b = e.sys.box_f64();
    e.kick_region([-1.0; 3], [b[0] + 1.0, b[1] + 1.0, b[2] + 1.0], (t / now).sqrt(), [0.0; 3]);
}

#[test]
#[ignore = "an experiment: prints how a crystal block melts and refreezes"]
fn block_melts_and_refreezes() {
    let p = prims();
    let o = Options::default().with("n", 8000.0).with("room", 3.0).with("temperature", 0.5);
    let sc = scenarios::three::build3("crystal3", &o).unwrap();
    let mut e = GpuEngine3::new(&p, sc.sys.clone(), &sc.state);
    let n = e.n() as f64;
    let report = |e: &GpuEngine3, label: &str| {
        let s = block_on(e.read_state());
        let t = block_on(e.read_kinetic_total()) / (1.5 * n);
        eprintln!("{label:>12} t = {:6.0} τ  kT = {t:.3}  crystalline {:.2}", e.steps as f64 * e.sys.dt, crystalline(&s, e));
    };
    report(&e, "start");
    for phase in [("cold", None, 6), ("hot 0.9", Some(0.9), 8), ("cool 0.42", Some(0.42), 30)] {
        if let Some(t) = phase.1 {
            set_t(&mut e, t);
        }
        for _ in 0..phase.2 {
            e.step(4000);
            if let Some(t) = phase.1 {
                // Hold the temperature (the latent heat would otherwise move it).
                set_t(&mut e, t);
            }
            report(&e, phase.0);
        }
    }
}
