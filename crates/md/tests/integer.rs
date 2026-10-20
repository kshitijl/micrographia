//! `gpu-int-int` against `cpu-int-int`: all-integer forces must agree bit for bit.

use gpu_prims::{Gpu, Prims};
use md::{Arith, ForceMode, GpuEngine};
use md_core::{init, Pair, State, System};
use md_ref::IntEngine;
use pollster::block_on;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

fn mixture(n: usize, seed: u64) -> (System, State) {
    let sys = System::new(init::box_for(n, 0.6, 1.0), init::DT, Pair::lj(2.5), vec![1.0, 3.0]).with_pair(0, 1, Pair { eps: 0.6, sigma: 1.1, rc: 2.5 });
    let mut s = init::fluid(&sys, n, 1.0, seed);
    for i in (0..n).step_by(3) {
        s.species[i] = 1;
    }
    s.vel = init::maxwell(&sys, &s.species, 1.0, seed);
    (sys, s)
}

#[test]
fn integer_kicks_are_identical_on_gpu_and_cpu() {
    let p = prims();
    let (sys, s0) = mixture(4000, 1);
    let mut cpu = IntEngine::<f64>::integer(sys.clone(), s0);
    cpu.step(200);
    let s = cpu.state.clone();
    let (kc, _) = cpu.kicks();
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s.n())] {
        let g = GpuEngine::with_options(&p, sys.clone(), &s, mode, Arith::Int);
        let (kg, _) = block_on(g.kicks());
        let same = kg.iter().zip(&kc).filter(|(a, b)| a == b).count();
        assert_eq!(same, kg.len(), "{mode:?}: {} of {} integer kicks differ", kg.len() - same, kg.len());
    }
    // And close to the f64 forces.
    let (k64, _) = IntEngine::<f64>::new(sys, s).kicks();
    let (mut num, mut den) = (0.0, 0.0);
    for (a, b) in kc.iter().zip(&k64) {
        for ax in 0..2 {
            num += ((a[ax] - b[ax]) as f64).powi(2);
            den += (b[ax] as f64).powi(2);
        }
    }
    eprintln!("integer kicks vs f64: rms relative difference {:.1e}", (num / den).sqrt());
    assert!((num / den).sqrt() < 1e-5);
}

#[test]
fn integer_trajectories_are_identical_on_gpu_and_cpu() {
    let p = prims();
    let (sys, s0) = mixture(3000, 2);
    let mut cpu = IntEngine::<f64>::integer(sys.clone(), s0.clone());
    let mut gpu = GpuEngine::with_options(&p, sys.clone(), &s0, ForceMode::CellList, Arith::Int);
    cpu.step(1000);
    gpu.step(1000);
    assert_eq!(block_on(gpu.read_state()).hash(), cpu.state.hash(), "gpu-int-int and cpu-int-int diverged");
}

#[test]
fn integer_dynamics_reverses_and_conserves_energy() {
    let p = prims();
    let (sys, s0) = mixture(6000, 3);
    let mut e = GpuEngine::with_options(&p, sys.clone(), &s0, ForceMode::verlet_for(&sys, s0.n()), Arith::Int);
    e.step(2000);
    e.reverse();
    e.step(2000);
    e.reverse();
    assert_eq!(block_on(e.read_state()), s0);
    // Energy drift, from the CPU f64 evaluation of GPU states.
    let energy = |s: &State| IntEngine::<f64>::new(sys.clone(), s.clone()).energy().total() / s.n() as f64;
    e.step(500);
    let e0 = energy(&block_on(e.read_state()));
    let mut worst = 0.0f64;
    for _ in 0..10 {
        e.step(500);
        worst = worst.max((energy(&block_on(e.read_state())) - e0).abs());
    }
    eprintln!("integer forces: energy per particle {e0:.4}, max deviation over 5000 steps {worst:.1e}");
    assert!(worst < 3e-3);
}

/// The golden hash (`md::checks::GOLDEN_INT_HASH`): the same 1000 steps must give this state
/// hash on every device, browser and CPU. If it changes, the integer force path changed.
#[test]
fn golden_hash() {
    let (sys, s0) = md::checks::golden_int_setup();
    let mut cpu = IntEngine::<f64>::integer(sys.clone(), s0.clone());
    cpu.step(1000);
    let h = cpu.state.hash();
    eprintln!("golden cpu-int-int hash after 1000 steps: {h:016x}");
    let p = prims();
    let mut gpu = GpuEngine::with_options(&p, sys, &s0, ForceMode::CellList, Arith::Int);
    gpu.step(1000);
    assert_eq!(block_on(gpu.read_state()).hash(), h);
    assert_eq!(h, md::checks::GOLDEN_INT_HASH, "the golden hash changed");
}
