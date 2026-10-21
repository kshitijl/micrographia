//! Gibbs-ensemble Monte Carlo of the 2D LJ fluid (cut and shifted at 2.5 σ).

use mc::gibbs::Gibbs;

/// Average (vapour, liquid) densities and pressures over `cycles` after `equilibrate`.
fn coexistence(t: f64, n: usize, equilibrate: usize, cycles: usize) -> ((f64, f64), (f64, f64)) {
    let mut g = Gibbs::new(n, 0.3, t, 2.5, 7);
    g.run(equilibrate);
    let (mut lo, mut hi, mut plo, mut phi, mut k) = (0.0, 0.0, 0.0, 0.0, 0.0);
    // Block averages of the pressure difference, for an error bar.
    let mut blocks = vec![];
    let mut block = (0.0, 0.0);
    for s in 0..cycles / 10 {
        g.run(10);
        let m = g.measure();
        let (v, l) = if m[0].0 < m[1].0 { (m[0], m[1]) } else { (m[1], m[0]) };
        lo += v.0;
        hi += l.0;
        plo += v.1;
        phi += l.1;
        k += 1.0;
        block.0 += l.1 - v.1;
        block.1 += 1.0;
        if (s + 1) % (cycles / 100).max(1) == 0 {
            blocks.push(block.0 / block.1);
            block = (0.0, 0.0);
        }
    }
    let mean = blocks.iter().sum::<f64>() / blocks.len() as f64;
    let err = (blocks.iter().map(|b| (b - mean).powi(2)).sum::<f64>() / (blocks.len() as f64 * (blocks.len() as f64 - 1.0))).sqrt();
    eprintln!("T {t}: liquid − vapour pressure {mean:.5} ± {err:.5} ({} blocks)", blocks.len());
    let s = g.stats;
    eprintln!(
        "T {t}: acceptance moves {:.2}, areas {:.2}, transfers {:.3}",
        s.moves.1 as f64 / s.moves.0 as f64,
        s.areas.1 as f64 / s.areas.0 as f64,
        s.transfers.1 as f64 / s.transfers.0 as f64
    );
    ((lo / k, hi / k), (plo / k, phi / k))
}

/// Molecular dynamics of the same model, a liquid slab with its vapour pre-filled near
/// saturation, held 1,000 τ (crates/md/tests/coexistence.rs, slab_with_prefilled_vapour):
/// (T, vapour, liquid). Started with empty vapour space, the MD vapour takes far longer than
/// 1,000 τ to fill (research §18.8).
const MD: [(f64, f64, f64); 2] = [(0.40, 0.023, 0.742), (0.44, 0.058, 0.68)];

#[test]
fn boxes_split_into_liquid_and_vapour_matching_molecular_dynamics() {
    for (t, md_v, md_l) in MD {
        let t0 = std::time::Instant::now();
        let ((v, l), (pv, pl)) = coexistence(t, 400, 3000, 30000);
        eprintln!("T {t}: vapour {v:.4} (MD {md_v}), liquid {l:.4} (MD {md_l}); pressures {pv:.5} and {pl:.5} ({:.1} s)", t0.elapsed().as_secs_f64());
        assert!((pv - pl).abs() < 0.01, "equal pressures");
        // Near the critical point (0.44) the liquid box's density fluctuates widely.
        let tol = if t < 0.42 { 0.01 } else { 0.05 };
        assert!((l - md_l).abs() < tol, "liquid");
        assert!((v - md_v).abs() < 0.2 * md_v, "vapour");
    }
}

/// Measured: N 400: 0.060 / 0.649; N 800: 0.139 / 0.554; N 1600: 0.109 / 0.622 at T 0.44.
/// Bigger boxes are worse: they have room for a drop in the vapour box or a bubble in the
/// liquid, and the means then mix the phases.
#[test]
#[ignore = "an experiment: GEMC sizes and run lengths"]
fn sizes() {
    for (n, t) in [(400, 0.44), (800, 0.44), (1600, 0.44)] {
        let t0 = std::time::Instant::now();
        let ((v, l), (pv, pl)) = coexistence(t, n, 5000, 20000);
        eprintln!("N {n}, T {t}: vapour {v:.4}, liquid {l:.4}; pressures {pv:.5} / {pl:.5} ({:.0} s)", t0.elapsed().as_secs_f64());
    }
}

/// The two most common box densities (the peaks of the histogram of both boxes' densities, one
/// below and one above `split`): robust against samples where a box briefly holds both phases.
fn peaks(samples: &[f64], split: f64) -> (f64, f64) {
    let bin = 0.005;
    let mut h = std::collections::HashMap::<i64, usize>::new();
    for &s in samples {
        *h.entry((s / bin).floor() as i64).or_default() += 1;
    }
    let best = |below: bool| {
        // Smooth over 3 bins, then take the highest.
        let (mut kmax, mut cmax) = (0i64, 0usize);
        for (&k, _) in &h {
            let c: usize = (k - 1..=k + 1).map(|j| *h.get(&j).unwrap_or(&0)).sum();
            if ((k as f64 + 0.5) * bin < split) == below && c > cmax {
                (kmax, cmax) = (k, c);
            }
        }
        (kmax as f64 + 0.5) * bin
    };
    (best(true), best(false))
}

/// Measured (vapour / liquid peaks): T 0.40: 0.0225 / 0.7475 for N 200 and 400; T 0.44:
/// 0.0525 / 0.7075 (N 200), 0.0575 / 0.6525 (N 400), 0.0675 / 0.6925 (N 800, whose boxes hold
/// both phases 44% of the time).
#[test]
#[ignore = "an experiment: GEMC densities from histogram peaks"]
fn histogram_peaks() {
    for (n, t) in [(200, 0.40), (400, 0.40), (200, 0.44), (400, 0.44), (800, 0.44)] {
        let mut g = Gibbs::new(n, 0.3, t, 2.5, 7);
        g.run(5000);
        let mut samples = vec![];
        let mut mid = 0usize;
        for _ in 0..4000 {
            g.run(5);
            let m = g.measure();
            for b in m {
                samples.push(b.0);
                if b.0 > 0.15 && b.0 < 0.5 {
                    mid += 1;
                }
            }
        }
        let (v, l) = peaks(&samples, 0.3);
        eprintln!("N {n}, T {t}: peaks vapour {v:.4}, liquid {l:.4}; samples between 0.15 and 0.5: {:.0}%", 100.0 * mid as f64 / samples.len() as f64);
    }
}
