//! Bonds on the GPU against the CPU twin.

use gpu_prims::{Gpu, Prims};
use md::{ForceMode, GpuEngine};
use md_core::{init, BondType, Pair, State, System};
use md_ref::IntEngine;
use pollster::block_on;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

fn melt(chains: usize, length: usize, density: f64, seed: u64) -> (System, State) {
    let n = chains * length;
    let sys0 = System::new(init::box_for(n, density, 1.0), init::DT, Pair::wca(), vec![1.0]);
    let (pos, bonds) = init::chains(&sys0, chains, length, 0.97);
    let sys = sys0.with_bonds(vec![BondType::kremer_grest(), BondType::Harmonic { k: 50.0, r0: 1.0 }], bonds);
    let species = vec![0; n];
    let vel = init::maxwell(&sys, &species, 1.0, seed);
    (sys, State { pos, vel, species })
}

#[test]
fn bond_kicks_match_the_cpu_twin() {
    let p = prims();
    let (sys, s0) = melt(100, 30, 0.4, 1);
    let mut cpu = IntEngine::<f32>::new(sys.clone(), s0);
    cpu.step(300);
    let s = cpu.state.clone();
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s.n())] {
        let g = GpuEngine::with_mode(&p, sys.clone(), &s, mode);
        let (kg, _) = block_on(g.kicks());
        let (k64, _) = IntEngine::<f64>::new(sys.clone(), s.clone()).kicks();
        let (mut num, mut den) = (0.0, 0.0);
        for (a, b) in kg.iter().zip(&k64) {
            for ax in 0..2 {
                num += ((a[ax] - b[ax]) as f64).powi(2);
                den += (b[ax] as f64).powi(2);
            }
        }
        let rel = (num / den).sqrt();
        eprintln!("{mode:?}: rms error of GPU kicks with bonds vs f64: {rel:.1e}");
        assert!(rel < 1e-5);
    }
}

#[test]
fn chains_on_the_gpu_reverse_exactly_with_reordering() {
    let p = prims();
    // Enough beads for Verlet lists, whose rebuilds reorder storage (and remap bond partners).
    let (sys, s0) = melt(800, 32, 0.4, 2);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    assert!(matches!(e.mode, ForceMode::Verlet { .. }));
    e.step(3000);
    let (rebuilds, overflow) = block_on(e.buf.read_list_stats());
    assert!(rebuilds > 2 && !overflow);
    e.reverse();
    e.step(3000);
    e.reverse();
    assert_eq!(block_on(e.read_state()), s0);
}

#[test]
fn cells_and_verlet_agree_with_bonds() {
    let p = prims();
    let (sys, s0) = melt(300, 40, 0.4, 3);
    let mut a = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    let mut b = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::verlet_for(&sys, s0.n()));
    a.step(2000);
    b.step(2000);
    assert_eq!(block_on(a.read_state()), block_on(b.read_state()));
}
