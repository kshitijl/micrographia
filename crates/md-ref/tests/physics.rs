//! CPU engine tests: exactness properties of the integer engine, agreement between
//! configurations, and basic physics (energy conservation, equipartition, Maxwell–Boltzmann).

use md_core::{fixed, init, Pair, State, System};
use md_ref::{FloatEngine, IntEngine};

fn wca_system(n: usize, density: f64, masses: Vec<f64>) -> System {
    System::new(init::box_for(n, density, 1.0), init::DT, Pair::wca(), masses)
}

fn fluid(n: usize, density: f64, t: f64, seed: u64) -> (System, State) {
    let sys = wca_system(n, density, vec![1.0]);
    let s = init::fluid(&sys, n, t, seed);
    (sys, s)
}

fn reversal_is_exact<F: md_ref::Real>() {
    let (sys, s0) = fluid(400, 0.6, 1.0, 1);
    let mut e = IntEngine::<F>::new(sys, s0.clone());
    e.step(1500);
    assert_ne!(e.state, s0);
    e.reverse();
    e.step(1500);
    e.reverse();
    assert_eq!(e.state, s0, "{}: reversal did not restore the initial state", F::NAME);
    assert_eq!(e.steps, 0);
}

#[test]
fn reversal_is_exact_f32() {
    reversal_is_exact::<f32>();
}

#[test]
fn reversal_is_exact_f64() {
    reversal_is_exact::<f64>();
}

#[test]
fn one_bit_flip_breaks_reversal() {
    let (sys, s0) = fluid(400, 0.6, 1.0, 2);
    let mut e = IntEngine::<f32>::new(sys.clone(), s0.clone());
    e.step(3000);
    e.state.flip_bit(0, 0, 0);
    e.reverse();
    e.step(3000);
    e.reverse();
    let (differ, max) = e.state.position_difference(&s0, &sys);
    eprintln!("after a one-bit flip: {differ} of 400 particles differ, by up to {max:.3} σ");
    assert!(differ > 300 && max > 0.5, "chaos should amplify a 2^-32 σ perturbation");
}

#[test]
fn storage_order_does_not_matter() {
    let (sys, s0) = fluid(500, 0.6, 1.0, 3);
    let perm: Vec<usize> = {
        let mut p: Vec<usize> = (0..s0.n()).collect();
        let mut rng = gpu_prims::checks::SplitMix(11);
        for i in (1..p.len()).rev() {
            p.swap(i, rng.below(i as u32 + 1) as usize);
        }
        p
    };
    let mut a = IntEngine::<f32>::new(sys.clone(), s0.clone());
    let mut b = IntEngine::<f32>::new(sys, s0.permuted(&perm));
    a.step(500);
    b.step(500);
    assert_eq!(b.state.unpermuted(&perm), a.state);
}

#[test]
fn momentum_is_exactly_conserved() {
    let (sys, s0) = fluid(500, 0.6, 1.0, 4);
    assert_eq!(s0.velocity_sum(), [0, 0]);
    let mut e = IntEngine::<f32>::new(sys, s0);
    for _ in 0..10 {
        e.step(100);
        assert_eq!(e.state.velocity_sum(), [0, 0]);
    }
}

/// Standard deviation of the total energy over a run.
fn energy_fluctuation(dt: f64, steps: usize, sample_every: usize) -> f64 {
    let n = 400;
    // WCA: force and energy are both continuous at the cutoff, so the error is pure integrator error.
    let sys = System::new(init::box_for(n, 0.6, 1.0), dt, Pair::wca(), vec![1.0]);
    let s0 = init::fluid(&sys, n, 1.0, 5);
    let mut e = FloatEngine::<f64>::from_state(sys.clone(), &s0);
    // Equilibrate at the coarse step so both runs start from comparable states.
    e.step(steps / 4);
    let mut es = Vec::new();
    for _ in 0..steps / sample_every {
        e.step(sample_every);
        es.push(e.energy().total());
    }
    let mean = es.iter().sum::<f64>() / es.len() as f64;
    (es.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / es.len() as f64).sqrt()
}

#[test]
fn energy_error_scales_as_dt_squared() {
    let coarse = energy_fluctuation(0.004, 2000, 10);
    let fine = energy_fluctuation(0.002, 4000, 20);
    let ratio = coarse / fine;
    eprintln!("energy fluctuation: Δt=0.004 → {coarse:.3e}, Δt=0.002 → {fine:.3e}, ratio {ratio:.2}");
    assert!((3.0..5.5).contains(&ratio), "ratio {ratio}, expected ≈ 4");
}

#[test]
fn integer_and_float_engines_agree_initially() {
    let (sys, s0) = fluid(400, 0.6, 1.0, 6);
    let mut i64e = IntEngine::<f64>::new(sys.clone(), s0.clone());
    let mut f64e = FloatEngine::<f64>::from_state(sys.clone(), &s0);
    i64e.step(20);
    f64e.step(20);
    let b = sys.box_f64();
    let mut worst = 0.0f64;
    for (p, q) in i64e.state.pos.iter().zip(f64e.positions_f64()) {
        for a in 0..2 {
            let mut d = (fixed::to_f64(p[a]) - q[a]).abs();
            d = d.min(b[a] - d);
            worst = worst.max(d);
        }
    }
    eprintln!("cpu-int-float<f64> vs cpu-float<f64> after 20 steps: max |Δx| = {worst:.2e} σ");
    assert!(worst < 1e-7, "worst {worst}");
}

#[test]
fn f32_kicks_match_f64_kicks() {
    let (sys, s0) = fluid(1000, 0.7, 1.0, 7);
    let (k32, pe32) = IntEngine::<f32>::new(sys.clone(), s0.clone()).kicks();
    let (k64, pe64) = IntEngine::<f64>::new(sys, s0).kicks();
    let mut num = 0.0;
    let mut den = 0.0;
    for (a, b) in k32.iter().zip(&k64) {
        for ax in 0..2 {
            num += ((a[ax] - b[ax]) as f64).powi(2);
            den += (b[ax] as f64).powi(2);
        }
    }
    let rel = (num / den).sqrt();
    eprintln!("f32 vs f64 kicks: rms relative difference {rel:.2e}; PE {pe32:.6} vs {pe64:.6}");
    assert!(rel < 1e-5, "rel {rel}");
    assert!(((pe32 - pe64) / pe64).abs() < 1e-5);
}

#[test]
fn equipartition_between_light_and_heavy() {
    let n = 800;
    let sys = wca_system(n, 0.5, vec![1.0, 10.0]);
    let mut s0 = init::fluid(&sys, n, 1.0, 8);
    for i in (0..n).step_by(2) {
        s0.species[i] = 1;
    }
    s0.vel = init::maxwell(&sys, &s0.species, 1.0, 8);
    let mut e = IntEngine::<f32>::new(sys.clone(), s0);
    e.step(3000);
    let mut ke = [0.0f64; 2];
    let samples = 60;
    for _ in 0..samples {
        e.step(50);
        let v = e.velocities();
        for i in 0..n {
            let s = e.state.species[i] as usize;
            ke[s] += 0.5 * sys.masses[s] * (v[i][0] * v[i][0] + v[i][1] * v[i][1]);
        }
    }
    let per = [ke[0] / (samples * n / 2) as f64, ke[1] / (samples * n / 2) as f64];
    eprintln!("mean kinetic energy per particle: light {:.4}, heavy (10×) {:.4}", per[0], per[1]);
    assert!((per[0] / per[1] - 1.0).abs() < 0.06, "{per:?}");
}

#[test]
fn equal_speeds_relax_to_maxwell_boltzmann() {
    let n = 1000;
    let sys = wca_system(n, 0.5, vec![1.0]);
    let mut s0 = init::fluid(&sys, n, 1.0, 9);
    s0.vel = init::fixed_speed(&sys, &s0.species, 1.0, 9);
    let mut e = IntEngine::<f32>::new(sys, s0);
    let kurtosis = |e: &IntEngine<f32>| {
        let vx: Vec<f64> = e.velocities().iter().flat_map(|v| [v[0], v[1]]).collect();
        let m2 = vx.iter().map(|v| v * v).sum::<f64>() / vx.len() as f64;
        let m4 = vx.iter().map(|v| v.powi(4)).sum::<f64>() / vx.len() as f64;
        m4 / (m2 * m2)
    };
    let k0 = kurtosis(&e);
    e.step(2000);
    let mut k = 0.0;
    for _ in 0..20 {
        e.step(50);
        k += kurtosis(&e) / 20.0;
    }
    eprintln!("velocity-component kurtosis: start {k0:.3} (equal speeds: 1.5), after relaxation {k:.3} (Gaussian: 3)");
    assert!((k0 - 1.5).abs() < 0.1);
    assert!((k - 3.0).abs() < 0.15, "kurtosis {k}");
}
