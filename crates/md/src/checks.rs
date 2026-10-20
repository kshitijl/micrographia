//! Engine self-checks that run both natively and in the browser (where the shaders go through
//! the browser's compiler rather than naga).

use crate::{Arith, ForceMode, GpuEngine};
use gpu_prims::checks::{CheckResult, SplitMix};
use gpu_prims::Prims;
use md_core::{init, Pair, State, System, Wall};
use md_ref::IntEngine;

pub const NAMES: &[&str] = &["md_kicks_vs_cpu", "md_run_to_run", "md_reversal", "md_permutation", "md_momentum", "md_verlet_vs_cells", "md_walls_reversal", "md_int_golden"];

pub async fn run(name: &str, prims: &Prims) -> CheckResult {
    match name {
        "md_kicks_vs_cpu" => kicks_vs_cpu(prims).await,
        "md_run_to_run" => run_to_run(prims).await,
        "md_reversal" => reversal(prims).await,
        "md_permutation" => permutation(prims).await,
        "md_momentum" => momentum(prims).await,
        "md_verlet_vs_cells" => verlet_vs_cells(prims).await,
        "md_walls_reversal" => walls_reversal(prims).await,
        "md_int_golden" => int_golden(prims).await,
        _ => Err(format!("unknown check {name}")),
    }
}

fn fluid(n: usize, pair: Pair, seed: u64) -> (System, State) {
    let sys = System::new(init::box_for(n, 0.6, 1.0), init::DT, pair, vec![1.0]);
    let s = init::fluid(&sys, n, 1.0, seed);
    (sys, s)
}

/// The reference state for golden hashes: 3000 WCA particles after 1000 steps.
pub fn golden_setup() -> (System, State, usize) {
    let (sys, s) = fluid(3000, Pair::wca(), 2026);
    (sys, s, 1000)
}

pub async fn kicks_vs_cpu(prims: &Prims) -> CheckResult {
    let (sys, s) = fluid(3000, Pair::lj(2.5), 1);
    let mut cpu = IntEngine::<f32>::new(sys.clone(), s);
    cpu.step(100);
    let gpu = GpuEngine::new(prims, sys.clone(), &cpu.state);
    let (kg, _) = gpu.kicks().await;
    let (k64, _) = IntEngine::<f64>::new(sys, cpu.state.clone()).kicks();
    let (k32, _) = cpu.kicks();
    let identical = kg.iter().zip(&k32).filter(|(a, b)| a == b).count();
    let (mut num, mut den) = (0.0, 0.0);
    for (a, b) in kg.iter().zip(&k64) {
        for ax in 0..2 {
            num += ((a[ax] - b[ax]) as f64).powi(2);
            den += (b[ax] as f64).powi(2);
        }
    }
    let rel = (num / den).sqrt();
    if rel > 1e-5 {
        return Err(format!("rms relative kick error vs f64 {rel:.2e}"));
    }
    Ok(format!("{identical}/{} kicks identical to the CPU f32 twin; rms error vs f64 {rel:.1e}", kg.len()))
}

pub async fn run_to_run(prims: &Prims) -> CheckResult {
    let (sys, s, steps) = golden_setup();
    let mut a = GpuEngine::new(prims, sys.clone(), &s);
    let mut b = GpuEngine::new(prims, sys, &s);
    a.step(steps);
    b.step(steps);
    let (ha, hb) = (a.read_state().await.hash(), b.read_state().await.hash());
    if ha != hb {
        return Err(format!("hashes differ: {ha:016x} vs {hb:016x}"));
    }
    Ok(format!("state hash after {steps} steps: {ha:016x}"))
}

pub async fn reversal(prims: &Prims) -> CheckResult {
    let (sys, s0) = fluid(4000, Pair::wca(), 3);
    let mut e = GpuEngine::new(prims, sys, &s0);
    e.step(1500);
    e.reverse();
    e.step(1500);
    e.reverse();
    let s = e.read_state().await;
    if s != s0 {
        return Err("forward 1500, reverse, 1500, reverse did not restore the state".into());
    }
    Ok("1500 steps forward and back restore every bit".into())
}

pub async fn permutation(prims: &Prims) -> CheckResult {
    let (sys, s0) = fluid(3000, Pair::wca(), 4);
    let mut rng = SplitMix(4);
    let mut perm: Vec<usize> = (0..s0.n()).collect();
    for i in (1..perm.len()).rev() {
        perm.swap(i, rng.below(i as u32 + 1) as usize);
    }
    let mut a = GpuEngine::new(prims, sys.clone(), &s0);
    let mut b = GpuEngine::new(prims, sys, &s0.permuted(&perm));
    a.step(500);
    b.step(500);
    if a.read_state().await != b.read_state().await.unpermuted(&perm) {
        return Err("shuffling storage order changed the trajectory".into());
    }
    Ok("shuffled storage order gives the identical trajectory".into())
}

pub async fn momentum(prims: &Prims) -> CheckResult {
    let (sys, s0) = fluid(3000, Pair::lj(2.5), 5);
    let mut e = GpuEngine::new(prims, sys, &s0);
    e.step(1000);
    let p = e.read_state().await.velocity_sum();
    if p != [0, 0] {
        return Err(format!("integer momentum drifted to {p:?}"));
    }
    Ok("integer momentum exactly zero after 1000 steps".into())
}

pub async fn verlet_vs_cells(prims: &Prims) -> CheckResult {
    let (sys, s0) = fluid(5000, Pair::lj(2.5), 6);
    let mut a = GpuEngine::with_mode(prims, sys.clone(), &s0, ForceMode::CellList);
    let mut b = GpuEngine::with_mode(prims, sys.clone(), &s0, ForceMode::verlet_for(&sys, s0.n()));
    a.step(1000);
    b.step(1000);
    let (rebuilds, overflow) = b.buf.read_list_stats().await;
    if overflow {
        return Err("neighbour list overflowed".into());
    }
    if a.read_state().await != b.read_state().await {
        return Err("Verlet-list and cell-list trajectories differ".into());
    }
    Ok(format!("identical trajectories over 1000 steps ({rebuilds} list rebuilds, decided on the GPU)"))
}

/// Hard and soft walls, gravity and a two-species pair table, reversed exactly.
pub async fn walls_reversal(prims: &Prims) -> CheckResult {
    let sys = System::new(init::box_for(3000, 0.4, 1.0), init::DT, Pair::lj(2.5), vec![1.0, 3.0])
        .with_pair(0, 1, Pair { eps: 0.5, sigma: 1.2, rc: 3.0 })
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_gravity([0.0, -0.05]);
    let mut s0 = init::fluid(&sys, 3000, 1.0, 7);
    for i in (0..3000).step_by(4) {
        s0.species[i] = 1;
    }
    let mut e = GpuEngine::new(prims, sys, &s0);
    e.step(1500);
    let hits: u64 = e.read_ledger().await.hits.iter().sum();
    e.reverse();
    e.step(1500);
    e.reverse();
    if e.read_state().await != s0 {
        return Err("walls + gravity + mixture: reversal did not restore the state".into());
    }
    Ok(format!("exact reversal with walls ({hits} hits), gravity and a binary mixture"))
}

/// The setup for the all-integer golden hash: a binary LJ mixture of 3000 particles.
pub fn golden_int_setup() -> (System, State) {
    let sys = System::new(init::box_for(3000, 0.6, 1.0), init::DT, Pair::lj(2.5), vec![1.0, 2.0]).with_pair(0, 1, Pair { eps: 0.7, sigma: 1.05, rc: 2.5 });
    let mut s = init::fluid(&sys, 3000, 1.0, 2026);
    for i in (0..3000).step_by(4) {
        s.species[i] = 1;
    }
    s.vel = init::maxwell(&sys, &s.species, 1.0, 2026);
    (sys, s)
}

/// State hash of `golden_int_setup` after 1000 steps with all-integer forces, on any device.
pub const GOLDEN_INT_HASH: u64 = 0x3aae_ff68_0a2f_0278;

/// All-integer forces give the same bits on every device: compare with the golden hash.
pub async fn int_golden(prims: &Prims) -> CheckResult {
    let (sys, s0) = golden_int_setup();
    let mut e = GpuEngine::with_options(prims, sys, &s0, ForceMode::CellList, Arith::Int);
    e.step(1000);
    let h = e.read_state().await.hash();
    if h != GOLDEN_INT_HASH {
        return Err(format!("all-integer hash {h:016x}, golden {GOLDEN_INT_HASH:016x}"));
    }
    Ok(format!("all-integer forces reproduce the golden hash {h:016x}"))
}
