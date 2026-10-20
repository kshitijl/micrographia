//! Engine self-checks that run both natively and in the browser (where the shaders go through
//! the browser's compiler rather than naga).

use crate::GpuEngine;
use gpu_prims::checks::{CheckResult, SplitMix};
use gpu_prims::Prims;
use md_core::{init, Pair, State, System};
use md_ref::IntEngine;

pub const NAMES: &[&str] = &["md_kicks_vs_cpu", "md_run_to_run", "md_reversal", "md_permutation", "md_momentum"];

pub async fn run(name: &str, prims: &Prims) -> CheckResult {
    match name {
        "md_kicks_vs_cpu" => kicks_vs_cpu(prims).await,
        "md_run_to_run" => run_to_run(prims).await,
        "md_reversal" => reversal(prims).await,
        "md_permutation" => permutation(prims).await,
        "md_momentum" => momentum(prims).await,
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
