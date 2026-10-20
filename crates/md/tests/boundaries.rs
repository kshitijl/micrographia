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

#[test]
fn compartments_on_the_gpu() {
    let p = prims();
    let side = 8i64 << 32;
    let sys = System::new([32 * side, 16 * side], init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, 3)
        .with_compartments(1, 3);
    let s0 = init::compartment_fluid(&sys, 8, 1.0, 9);
    let home: Vec<[i64; 2]> = s0.pos.iter().map(|&q| sys.compartment_of(q)).collect();
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s0.n())] {
        let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, mode);
        // Kicks first: compartments must cut interactions exactly as on the CPU.
        let (kg, _) = block_on(e.kicks());
        let (k64, _) = IntEngine::<f64>::new(sys.clone(), s0.clone()).kicks();
        let worst = kg.iter().zip(&k64).map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs())).max().unwrap();
        assert!(worst < 64, "{mode:?}: kick difference {worst}");
        e.step(3000);
        let s = block_on(e.read_state());
        for (i, q) in s.pos.iter().enumerate() {
            assert_eq!(sys.compartment_of(*q), home[i], "{mode:?}: particle {i} left its compartment");
        }
        e.reverse();
        e.step(3000);
        e.reverse();
        assert_eq!(block_on(e.read_state()), s0, "{mode:?}");
    }
}

/// Rows of compartments between a hot and a cold wall: one ledger slot per row, summing to the
/// wall totals.
#[test]
fn per_compartment_ledger() {
    let p = prims();
    let side = 16i64 << 32;
    let rows = 8;
    let sys = System::new([2 * side, rows * side], init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Thermal(2.0), Wall::Thermal(1.0))
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(1, 4);
    let s0 = init::compartment_fluid(&sys, 30, 1.5, 10);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(20000);
    let l = block_on(e.buf.read_compartment_ledger(rows as usize));
    let t = l.totals();
    eprintln!("heat from the hot wall per row: {:?}", l.energy[0].iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>());
    assert!(l.energy[0].iter().all(|&q| q > 0.0));
    let total: f64 = l.energy[0].iter().sum();
    assert!((total - t.energy[0]).abs() < 1e-6);
    assert!(l.hits[2].iter().skip(1).all(|&h| h == 0), "only row 0 touches the bottom wall");
}

#[test]
fn andersen_thermostat_on_the_gpu() {
    let p = prims();
    let n = 5000;
    let sys = System::new(init::box_for(n, 0.6, 1.0), init::DT, Pair::lj(2.5), vec![1.0]).with_thermostat(0.8, 1.0);
    let s0 = init::fluid(&sys, n, 2.0, 12);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(3000);
    let mut t = 0.0;
    for _ in 0..20 {
        e.step(100);
        t += block_on(e.read_state()).half_step_kinetic(&sys) / n as f64 / 20.0;
    }
    eprintln!("GPU Andersen at T = 0.8: {t:.3}");
    assert!((t - 0.8).abs() < 0.04);
}
