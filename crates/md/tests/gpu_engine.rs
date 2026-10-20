//! `gpu-int-f32` against the CPU configurations, and its exactness properties.

use gpu_prims::{Gpu, Prims};
use md::{ForceMode, GpuEngine};
use md_core::{init, Pair, State, System};
use md_ref::{FloatEngine, IntEngine};
use pollster::block_on;

fn prims() -> Prims {
    Prims::new(&block_on(Gpu::new()).expect("a GPU adapter is required"))
}

fn fluid(n: usize, density: f64, pair: Pair, seed: u64) -> (System, State) {
    let sys = System::new(init::box_for(n, density, 1.0), init::DT, pair, vec![1.0]);
    let s = init::fluid(&sys, n, 1.0, seed);
    (sys, s)
}

/// A state that has been run for a while, so particles are in fluid-like contact.
fn equilibrated(n: usize, density: f64, pair: Pair, seed: u64) -> (System, State) {
    let (sys, s) = fluid(n, density, pair, seed);
    let mut e = IntEngine::<f32>::new(sys.clone(), s);
    e.step(200);
    (sys, e.state)
}

#[test]
fn kicks_match_the_cpu_twin() {
    let p = prims();
    for pair in [Pair::wca(), Pair::lj(2.5)] {
        let (sys, s) = equilibrated(3000, 0.7, pair, 1);
        let gpu = GpuEngine::new(&p, sys.clone(), &s);
        let (kg, peg) = block_on(gpu.kicks());
        let (k32, pe32) = IntEngine::<f32>::new(sys.clone(), s.clone()).kicks();
        let (k64, _) = IntEngine::<f64>::new(sys, s).kicks();
        let mut identical = 0;
        let mut max_diff = 0;
        let (mut num, mut den) = (0.0, 0.0);
        for i in 0..kg.len() {
            if kg[i] == k32[i] {
                identical += 1;
            }
            for a in 0..2 {
                max_diff = max_diff.max((kg[i][a] - k32[i][a]).abs());
                num += ((kg[i][a] - k64[i][a]) as f64).powi(2);
                den += (k64[i][a] as f64).powi(2);
            }
        }
        let rel64 = (num / den).sqrt();
        eprintln!(
            "rc={:.3}: GPU vs CPU f32 twin: {identical}/{} kicks identical, max diff {max_diff} units; \
             rms rel. diff vs f64 {rel64:.2e}; PE {peg:.5} vs {pe32:.5}",
            pair.rc,
            kg.len()
        );
        assert!(rel64 < 1e-5);
        assert!(identical as f64 > 0.5 * kg.len() as f64);
        assert!(((peg - pe32) / pe32).abs() < 1e-5);
    }
}

#[test]
fn run_to_run_identical() {
    let p = prims();
    let (sys, s) = fluid(5000, 0.6, Pair::wca(), 2);
    let mut a = GpuEngine::new(&p, sys.clone(), &s);
    let mut b = GpuEngine::new(&p, sys, &s);
    a.step(1000);
    b.step(1000);
    let (sa, sb) = (block_on(a.read_state()), block_on(b.read_state()));
    assert_ne!(sa, s);
    assert_eq!(sa.hash(), sb.hash());
}

#[test]
fn reversal_is_exact() {
    let p = prims();
    let (sys, s0) = fluid(5000, 0.6, Pair::wca(), 3);
    for mode in [ForceMode::CellList, ForceMode::verlet_for(&sys, s0.n())] {
        let mut e = GpuEngine::with_mode(&p, sys.clone(), &s0, mode);
        e.step(2000);
        e.reverse();
        e.step(2000);
        e.reverse();
        let s = block_on(e.read_state());
        assert_eq!(s.hash(), s0.hash(), "{mode:?}: state after forward + reverse differs from the start");
        assert_eq!(e.steps, 0);
    }
}

#[test]
fn one_bit_flip_breaks_reversal() {
    let p = prims();
    let (sys, s0) = fluid(5000, 0.6, Pair::wca(), 4);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(3000);
    e.flip_bit(17, 0, 0);
    e.reverse();
    e.step(3000);
    e.reverse();
    let s = block_on(e.read_state());
    let (differ, max) = s.position_difference(&s0, &sys);
    eprintln!("GPU, one bit flipped: {differ}/5000 particles differ, max {max:.2} σ");
    assert!(differ > 4000);
}

#[test]
fn storage_order_does_not_matter() {
    let p = prims();
    let (sys, s0) = fluid(4000, 0.6, Pair::wca(), 5);
    let mut rng = gpu_prims::checks::SplitMix(5);
    let mut perm: Vec<usize> = (0..s0.n()).collect();
    for i in (1..perm.len()).rev() {
        perm.swap(i, rng.below(i as u32 + 1) as usize);
    }
    let mut a = GpuEngine::new(&p, sys.clone(), &s0);
    let mut b = GpuEngine::new(&p, sys, &s0.permuted(&perm));
    a.step(1000);
    b.step(1000);
    let sa = block_on(a.read_state());
    let sb = block_on(b.read_state()).unpermuted(&perm);
    assert_eq!(sa.hash(), sb.hash());
}

#[test]
fn momentum_is_exactly_conserved() {
    let p = prims();
    let (sys, s0) = fluid(4000, 0.6, Pair::lj(2.5), 6);
    let mut e = GpuEngine::new(&p, sys, &s0);
    e.step(2000);
    assert_eq!(block_on(e.read_state()).velocity_sum(), [0, 0]);
}

#[test]
fn trajectory_tracks_the_cpu_twin() {
    let p = prims();
    let (sys, s0) = fluid(2000, 0.6, Pair::wca(), 7);
    let mut g = GpuEngine::new(&p, sys.clone(), &s0);
    let mut c = IntEngine::<f32>::new(sys.clone(), s0.clone());
    let mut f = FloatEngine::<f64>::from_state(sys.clone(), &s0);
    let mut first_divergence = None;
    let mut report = Vec::new();
    for block in 1..=20 {
        g.step(50);
        c.step(50);
        f.step(50);
        let sg = block_on(g.read_state());
        if first_divergence.is_none() && sg != c.state {
            first_divergence = Some(block * 50);
        }
        let b = sys.box_f64();
        let mut worst = 0.0f64;
        for (p, q) in sg.pos.iter().zip(f.positions_f64()) {
            for a in 0..2 {
                let mut d = (md_core::fixed::to_f64(p[a]) - q[a]).abs();
                d = d.min(b[a] - d);
                worst = worst.max(d);
            }
        }
        report.push(format!("{}:{worst:.1e}", block * 50));
    }
    eprintln!("GPU first differs from the CPU f32 twin at step {first_divergence:?}");
    eprintln!("GPU vs cpu-float<f64>, max |Δx| (σ) by step: {}", report.join(" "));
}

#[test]
fn energy_is_conserved() {
    let p = prims();
    let (sys, s0) = fluid(4000, 0.6, Pair::wca(), 8);
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.step(500);
    let energy = |s: &State| IntEngine::<f64>::new(sys.clone(), s.clone()).energy().total() / s.n() as f64;
    let e0 = energy(&block_on(e.read_state()));
    let mut worst = 0.0f64;
    for _ in 0..10 {
        e.step(1000);
        worst = worst.max((energy(&block_on(e.read_state())) - e0).abs());
    }
    eprintln!("energy per particle {e0:.5}; max deviation over 10 000 steps {worst:.2e}");
    assert!(worst < 2e-3, "worst {worst}");
}

/// Growth of a one-bit perturbation at the reversal page's parameters (density 0.5, T = 1).
/// Prints the largest position difference between two copies over time, and the fitted rate.
#[test]
fn lyapunov_growth_at_page_parameters() {
    let p = prims();
    let n = 8000;
    let sys = System::new(init::box_for(n, 0.5, 4.0 / 3.0), init::DT, Pair::wca(), vec![1.0]);
    let mut s0 = init::fluid(&sys, n, 1.0, 7);
    let mut warm = GpuEngine::new(&p, sys.clone(), &s0);
    warm.step(1000);
    s0 = block_on(warm.read_state());
    init::set_temperature(&mut s0, &sys, 1.0);
    let mut a = GpuEngine::new(&p, sys.clone(), &s0);
    let mut b = GpuEngine::new(&p, sys.clone(), &s0);
    b.flip_bit(0, 0, 0);
    let mut rows = Vec::new();
    let mut fit = Vec::new();
    for k in 1..=40 {
        a.step(100);
        b.step(100);
        let (sa, sb) = (block_on(a.read_state()), block_on(b.read_state()));
        let (differ, max) = sa.position_difference(&sb, &sys);
        let t = k as f64 * 100.0 * init::DT;
        if max > 0.0 && max < 0.05 {
            fit.push((t, max.ln()));
        }
        rows.push(format!("t={t:.1}: {differ} differ, max {max:.1e}"));
    }
    let nf = fit.len() as f64;
    let (mt, my) = (fit.iter().map(|f| f.0).sum::<f64>() / nf, fit.iter().map(|f| f.1).sum::<f64>() / nf);
    let slope = fit.iter().map(|f| (f.0 - mt) * (f.1 - my)).sum::<f64>() / fit.iter().map(|f| (f.0 - mt).powi(2)).sum::<f64>();
    eprintln!("{}", rows.join("\n"));
    eprintln!("growth rate of the largest deviation: {slope:.2} per τ (factor {:.1} per τ)", slope.exp());
}

/// The reversal page's protocol: forward, nudge, reverse, back to t = 0. Prints how far from
/// the initial state the nudged run ends, for several forward durations.
#[test]
fn nudged_reversal_at_page_parameters() {
    let p = prims();
    let n = 8000;
    let sys = System::new(init::box_for(n, 0.5, 4.0 / 3.0), init::DT, Pair::wca(), vec![1.0]);
    let mut s0 = init::fluid(&sys, n, 1.0, 7);
    let mut warm = GpuEngine::new(&p, sys.clone(), &s0);
    warm.step(1000);
    s0 = block_on(warm.read_state());
    init::set_temperature(&mut s0, &sys, 1.0);
    for steps in [1000, 2000, 3000, 4000, 6000] {
        let mut e = GpuEngine::new(&p, sys.clone(), &s0);
        e.step(steps);
        e.flip_bit(0, 0, 0);
        e.reverse();
        e.step(steps);
        e.reverse();
        let (differ, max) = block_on(e.read_state()).position_difference(&s0, &sys);
        eprintln!("forward {:.0} τ then back: {differ} differ, max {max:.2e} σ", steps as f64 * init::DT);
    }
}

#[test]
fn spatial_reordering_does_not_change_the_trajectory() {
    let p = prims();
    let (sys, s0) = fluid(6000, 0.6, Pair::lj(2.5), 9);
    let mut a = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
    let mut b = GpuEngine::with_mode(&p, sys, &s0, ForceMode::CellList);
    a.sort_interval = 0;
    b.sort_interval = 7;
    a.step(1500);
    b.step(1500);
    assert_eq!(block_on(a.read_state()).hash(), block_on(b.read_state()).hash());
}

/// Same pairs, order-independent integer sums: the two force paths must agree bit for bit.
#[test]
fn cell_list_and_verlet_lists_give_identical_trajectories() {
    let p = prims();
    for (pair, density) in [(Pair::wca(), 0.6), (Pair::lj(2.5), 0.7)] {
        let (sys, s0) = fluid(6000, density, pair, 10);
        let mut a = GpuEngine::with_mode(&p, sys.clone(), &s0, ForceMode::CellList);
        let mode = ForceMode::verlet_for(&sys, s0.n());
        let mut b = GpuEngine::with_mode(&p, sys, &s0, mode);
        a.step(3000);
        b.step(3000);
        let (rebuilds, overflow) = block_on(b.buf.read_list_stats());
        eprintln!("rc={:.2}: {rebuilds} list rebuilds in 3000 steps", pair.rc);
        assert!(!overflow);
        assert!(rebuilds > 1 && rebuilds < 3000);
        assert_eq!(block_on(a.read_state()).hash(), block_on(b.read_state()).hash());
    }
}

/// Flipping the lowest bit for a whole species moves exactly those particles, by one unit.
#[test]
fn flip_species_bit_moves_exactly_that_species() {
    let p = prims();
    let sys = System::new(init::box_for(3000, 0.5, 1.0), init::DT, Pair::wca(), vec![1.0, 1.0, 1.0]);
    let mut s0 = init::fluid(&sys, 3000, 1.0, 9);
    for i in 0..3000 {
        s0.species[i] = (i % 3) as u32;
    }
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.flip_species_bit(0b101, 0, 0);
    let s = block_on(e.read_state());
    for i in 0..3000 {
        let moved = s.pos[i] != s0.pos[i];
        assert_eq!(moved, s0.species[i] != 1, "particle {i}");
        if moved {
            assert_eq!(s.pos[i][0] ^ s0.pos[i][0], 1);
            assert_eq!(s.pos[i][1], s0.pos[i][1]);
        }
    }
}

/// A region command touches exactly the particles inside the rectangle.
#[test]
fn kick_region_touches_only_the_region() {
    let p = prims();
    let (sys, s0) = fluid(3000, 0.5, Pair::wca(), 12);
    let b = sys.box_f64();
    let (lo, hi) = ([b[0] * 0.25, b[1] * 0.25], [b[0] * 0.5, b[1] * 0.75]);
    let inside = |q: [i64; 2]| {
        let (x, y) = (md_core::fixed::to_f64(q[0]), md_core::fixed::to_f64(q[1]));
        x >= lo[0] && x < hi[0] && y >= lo[1] && y < hi[1]
    };
    let mut e = GpuEngine::new(&p, sys.clone(), &s0);
    e.kick_region(lo, hi, 0.0, [0.0, 0.0]);
    let s = block_on(e.read_state());
    let mut frozen = 0;
    for i in 0..3000 {
        if inside(s0.pos[i]) {
            assert_eq!(s.vel[i], [0, 0]);
            frozen += 1;
        } else {
            assert_eq!(s.vel[i], s0.vel[i]);
        }
    }
    e.kick_region(lo, hi, 1.0, [2.0, 0.0]);
    let s = block_on(e.read_state());
    for i in 0..3000 {
        if inside(s0.pos[i]) {
            assert_eq!(s.vel[i], [sys.to_step_displacement(2.0), 0]);
        }
    }
    assert!(frozen > 300, "only {frozen} particles in the region");
}
