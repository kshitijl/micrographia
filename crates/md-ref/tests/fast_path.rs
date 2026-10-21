//! The fast pair path (cell-sorted, half shell, batched) gives bit-identical trajectories to the
//! reference loop, for mixtures of masses and sizes, LJ and WCA, periodic boxes and walls,
//! bonds and gravity.

use md_core::{fixed, init, BondType, Pair, System, Wall};
use md_ref::IntEngine;

fn compare(name: &str, sys: System, state: md_core::State, steps: usize) {
    let mut a = IntEngine::<f32>::new(sys.clone(), state.clone());
    let mut b = IntEngine::<f32>::new(sys, state);
    b.fast = false;
    a.step(steps);
    b.step(steps);
    assert_eq!(a.state, b.state, "{name}: trajectories differ");
    let (ea, eb) = (a.kicks().1, b.kicks().1);
    assert!((ea - eb).abs() < 1e-4 * eb.abs().max(1.0), "{name}: potential {ea} against {eb}");
    eprintln!("fast path: {name}: identical after {steps} steps (potential {ea:.4})");
}

#[test]
fn fast_path_matches_the_reference() {
    // An LJ liquid, periodic.
    let sys = System::new(init::box_for(2000, 0.7, 1.0), init::DT, Pair::lj(2.5), vec![1.0]);
    let st = init::fluid(&sys, 2000, 0.8, 1);
    compare("LJ liquid", sys, st, 300);

    // A WCA mixture of three masses and two sizes, hard walls on x, soft on y, gravity.
    let wca = |s: f64| Pair { eps: 1.0, sigma: s, rc: s * 2f64.powf(1.0 / 6.0) };
    let sys = System::new(init::box_for(1500, 0.3, 1.5), init::DT, wca(1.0), vec![1.0, 4.0, 30.0])
        .with_pair(0, 2, wca(2.0))
        .with_pair(1, 2, wca(2.0))
        .with_pair(2, 2, wca(3.0))
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_gravity([0.0, -0.01]);
    let mut st = init::fluid(&sys, 1500, 1.0, 2);
    for (i, s) in st.species.iter_mut().enumerate() {
        *s = if i % 50 == 0 { 2 } else { (i % 2) as u32 };
    }
    // Give the big ones room: drop small ones that overlap them.
    let big: Vec<[i64; 2]> = st.pos.iter().zip(&st.species).filter(|(_, s)| **s == 2).map(|(p, _)| *p).collect();
    let keep: Vec<usize> = (0..st.n())
        .filter(|&i| st.species[i] == 2 || big.iter().all(|b| (0..2).map(|a| fixed::to_f64(b[a] - st.pos[i][a]).powi(2)).sum::<f64>() > 4.0))
        .collect();
    let st = md_core::State { pos: keep.iter().map(|&i| st.pos[i]).collect(), vel: keep.iter().map(|&i| st.vel[i]).collect(), species: keep.iter().map(|&i| st.species[i]).collect() };
    compare("WCA mixture with walls and gravity", sys, st, 300);

    // Chains (bonds) in an LJ solvent.
    let sys = System::new(init::box_for(1200, 0.5, 1.0), init::DT, Pair::lj(2.5), vec![1.0]);
    let st = init::fluid(&sys, 1200, 1.0, 3);
    let bonds: Vec<[u32; 3]> = (0..200).map(|k| [2 * k, 2 * k + 1, 0]).collect();
    let sys = sys.with_bonds(vec![BondType::Harmonic { k: 50.0, r0: 1.0 }], bonds);
    compare("bonded pairs in LJ", sys, st, 300);
}

/// The branch-free separation conversion is the GPU's, bit for bit, across the whole range.
#[test]
fn near_separation_is_exact() {
    use md_ref::Real;
    let lim = md_core::fixed::SEP_LIMIT;
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    for k in 0..2_000_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let d = match k % 4 {
            0 => (x % (2 * lim as u64)) as i64 - lim,
            1 => (x % 1024) as i64 - 512,
            2 => ((x % 64) as i64 - 32) << 32,
            _ => (x % (2 * lim as u64)) as i64 - lim + 1,
        };
        if d >= lim || d < -lim {
            continue;
        }
        let a = <f32 as Real>::sep_near(d);
        let b = md_core::fixed::sep_f32(d).unwrap();
        assert_eq!(a.to_bits(), b.to_bits(), "d = {d}");
    }
}
