//! GPU walls, gravity and species-pair tables against the CPU twin.

use gpu_prims::{Gpu, Prims};
use md::{ForceMode, GpuEngine};
use md_core::{init, Pair, State, System, Wall};
use md_ref::IntEngine;
use pollster::block_on;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

/// A binary mixture (different sizes and attractions) in a box with soft walls in y and
/// gravity.
fn mixture(n: usize, seed: u64) -> (System, State) {
    let big = Pair { eps: 1.0, sigma: 1.4, rc: 1.4 * 2.5 };
    let cross = Pair { eps: 0.5, sigma: 1.2, rc: 1.2 * 2.5 };
    let sys = System::new(init::box_for(n, 0.4, 1.5), init::DT, Pair::lj(2.5), vec![1.0, 2.0])
        .with_pair(1, 1, big)
        .with_pair(0, 1, cross)
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_gravity([0.0, -0.05]);
    let mut s = init::fluid(&sys, n, 1.0, seed);
    for i in (0..n).step_by(3) {
        s.species[i] = 1;
    }
    s.vel = init::maxwell(&sys, &s.species, 1.0, seed);
    (sys, s)
}

#[test]
fn mixture_and_wall_kicks_match_the_cpu_twin() {
    let p = prims();
    let (sys, s) = mixture(4000, 1);
    let mut cpu = IntEngine::<f32>::new(sys.clone(), s);
    cpu.step(300);
    let s = cpu.state.clone();
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s.n())] {
        let gpu = GpuEngine::with_mode(&p, sys.clone(), &s, mode);
        let (kg, _) = block_on(gpu.kicks());
        let (k32, _) = cpu.kicks();
        let (k64, _) = IntEngine::<f64>::new(sys.clone(), s.clone()).kicks();
        let identical = kg.iter().zip(&k32).filter(|(a, b)| a == b).count();
        let (mut num, mut den) = (0.0, 0.0);
        for (a, b) in kg.iter().zip(&k64) {
            for ax in 0..2 {
                num += ((a[ax] - b[ax]) as f64).powi(2);
                den += (b[ax] as f64).powi(2);
            }
        }
        let rel = (num / den).sqrt();
        eprintln!("{mode:?}: {identical}/{} kicks identical to the CPU f32 twin; rms error vs f64 {rel:.1e}", kg.len());
        assert!(rel < 1e-5);
    }
}

#[test]
fn walls_and_gravity_are_exactly_reversible_on_the_gpu() {
    let p = prims();
    let (sys, s0) = mixture(5000, 2);
    let sys = sys.with_walls(0, Wall::Hard, Wall::Hard);
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s0.n())] {
        let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, mode);
        e.step(2000);
        let hits: u64 = block_on(e.read_ledger()).hits.iter().sum();
        e.reverse();
        e.step(2000);
        e.reverse();
        assert_eq!(block_on(e.read_state()), s0, "{mode:?}");
        assert!(hits > 100, "{mode:?}: only {hits} wall hits");
    }
}

#[test]
fn verlet_matches_cells_with_walls_gravity_and_mixtures() {
    let p = prims();
    let (sys, s0) = mixture(6000, 3);
    let mut a = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    let mut b = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::verlet_for(&sys, s0.n()));
    a.step(2000);
    b.step(2000);
    assert_eq!(block_on(a.read_state()), block_on(b.read_state()));
    let (la, lb) = (block_on(a.read_ledger()), block_on(b.read_ledger()));
    assert_eq!(la.impulse, lb.impulse, "soft-wall impulse should be bit-identical too");
}

/// Thermal walls bring the gas to their temperature, on the GPU and on the CPU alike.
#[test]
fn thermal_walls_on_the_gpu() {
    let p = prims();
    let n = 150;
    let sys = System::new(init::box_for(n, 0.1, 1.0), init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(1.5), Wall::Thermal(1.5))
        .with_walls(1, Wall::Thermal(1.5), Wall::Thermal(1.5));
    let s0 = init::fluid(&sys, n, 0.5, 4);
    let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    e.step(40000);
    let mut t = 0.0;
    for _ in 0..100 {
        e.step(200);
        let s = block_on(e.read_state());
        t += s.half_step_kinetic(&sys) / n as f64 / 100.0;
    }
    let heat: f64 = block_on(e.read_ledger()).energy.iter().sum();
    eprintln!("GPU: start T = 0.5, walls at 1.5: final T = {t:.3}; heat from the walls {heat:.1} ε");
    assert!((t - 1.5).abs() < 0.08, "T = {t}");
}

/// Heat flow between a hot and a cold wall: GPU and CPU agree within statistics, and the
/// ledger balances in the steady state.
#[test]
fn heat_flow_matches_the_cpu() {
    let p = prims();
    let n = 150;
    let sys = System::new(init::box_for(n, 0.1, 1.0), init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(2.0), Wall::Thermal(1.0))
        .with_walls(1, Wall::Hard, Wall::Hard);
    let s0 = init::fluid(&sys, n, 1.5, 6);
    let mut g = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    let mut c = IntEngine::<f32>::new(sys, s0);
    g.step(40000);
    c.step(40000);
    let (g0, c0) = (block_on(g.read_ledger()), c.ledger);
    g.step(200000);
    c.step(200000);
    let gl = block_on(g.read_ledger());
    let flux = |hot: f64| hot / (200000.0 * init::DT);
    let (gh, gc) = (gl.energy[0] - g0.energy[0], gl.energy[1] - g0.energy[1]);
    let (ch, cc) = (c.ledger.energy[0] - c0.energy[0], c.ledger.energy[1] - c0.energy[1]);
    eprintln!("heat flux hot→cold: GPU {:.3} ε/τ (cold wall {:.3}), CPU {:.3} ε/τ (cold wall {:.3})", flux(gh), flux(-gc), flux(ch), flux(-cc));
    assert!(((gh + gc) / gh).abs() < 0.1);
    assert!((gh / ch - 1.0).abs() < 0.15);
}
