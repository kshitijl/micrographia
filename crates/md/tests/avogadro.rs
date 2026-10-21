//! Avogadro's hypothesis in the `avogadro` scenario: under the same weighted piston at the same
//! temperature, the same number of molecules fills the same volume, whether each is one atom or
//! two, light or heavy.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use pollster::block_on;
use scenarios::Options;

fn height(p: &Prims, atoms: bool) -> f64 {
    let o = Options::default().with("atoms", if atoms { 1.0 } else { 0.0 });
    let sc = scenarios::build("avogadro", &o).unwrap();
    let mut e = GpuEngine::new(p, sc.sys.clone(), &sc.state);
    e.step(60000);
    let mut h = 0.0;
    let mut trace = vec![];
    for k in 0..1600 {
        e.step(200);
        let y = block_on(e.read_piston())[0];
        h += y / 1600.0;
        if k % 80 == 0 {
            trace.push(format!("{y:.0}"));
        }
    }
    eprintln!("piston ({}): {}", if atoms { "atoms" } else { "molecules" }, trace.join(" "));
    h
}

#[test]
fn equal_numbers_of_molecules_fill_equal_volumes() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let (two, one) = (height(&p, false), height(&p, true));
    eprintln!("avogadro: piston at {two:.1} σ over 300 two-atom molecules, {one:.1} σ over 300 atoms (start 100)");
    assert!((two / one - 1.0).abs() < 0.06);
}
