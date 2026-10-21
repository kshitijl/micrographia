//! Salt lowers the melting point: a crystal slab under its liquid (the `freeze3` scenario).

use gpu_prims::{Gpu, Prims};
use md::GpuEngine3;
use md_core::order::solid_bonds;
use pollster::block_on;
use scenarios::Options;

/// Crystalline share of the solvent atoms every `every` steps.
fn run(p: &Prims, t: f64, solute: f64, rounds: usize, every: usize) -> Vec<f64> {
    let o = Options::default().with("temperature", t).with("solute", solute);
    let sc = scenarios::three::build3("freeze3", &o).unwrap();
    let mut e = GpuEngine3::new(p, sc.sys.clone(), &sc.state);
    let mut out = vec![];
    for _ in 0..rounds {
        e.step(every);
        let s = block_on(e.read_state());
        let b = solid_bonds(&s.pos, e.sys.box_len, [true, false, true], 1.5);
        let solvent = (0..s.n()).filter(|&i| s.species[i] == 0).count();
        out.push((0..s.n()).filter(|&i| s.species[i] == 0 && b[i] >= 7).count() as f64 / solvent as f64);
    }
    out
}

#[test]
#[ignore = "an experiment: where the slab neither grows nor melts, with and without salt"]
fn melting_point_with_and_without_salt() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for solute in [0.0, 0.15] {
        for t in [0.52, 0.56, 0.60, 0.64] {
            let c = run(&p, t, solute, 8, 5000);
            eprintln!("solute {solute}, T {t}: crystal {}", c.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" "));
        }
    }
}

#[test]
fn salt_stays_salt() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = scenarios::three::build3("freeze3", &Options::default().with("temperature", 0.53)).unwrap();
    let mut e = GpuEngine3::new(&p, sc.sys.clone(), &sc.state);
    e.step(300);
    let ids: Vec<u32> = (4000..4300).collect();
    e.set_species(&ids, 1);
    e.step(1000);
    e.step(1000);
    let s = block_on(e.read_state());
    let salt = s.species.iter().filter(|&&x| x == 1).count();
    assert_eq!(salt, 300);
}
