//! Where the CPU engine's time goes: forces against a whole step. `cargo run --release -p md-ref
//! --example profile_fast [N]`.
use md_core::{init, Pair, System};
use md_ref::IntEngine;
use std::time::Instant;

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(16000);
    let sys = System::new(init::box_for(n, 0.7, 1.0), init::DT, Pair::lj(2.5), vec![1.0]);
    let mut e = IntEngine::<f32>::new(sys.clone(), init::fluid(&sys, n, 1.0, 1));
    e.step(5);
    let reps = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or((4_000_000 / n).max(10));
    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(e.forces());
    }
    let f = t.elapsed().as_secs_f64() / reps as f64;
    let t = Instant::now();
    e.step(reps);
    let s = t.elapsed().as_secs_f64() / reps as f64;
    println!("N {n}: forces {:.1} ns/particle, step {:.1} ns/particle", f * 1e9 / n as f64, s * 1e9 / n as f64);
}
