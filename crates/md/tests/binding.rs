//! Tuning and checks for the binding and FEP scenarios.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// For ligands A (species 1) and B (3): the ratio of time spent bound (within 0.8 σ of the
/// pocket's centre) to time spent far (beyond 4 σ), over `frames` snapshots.
pub fn occupancy(e: &mut GpuEngine, side: f64, frames: usize, every: usize) -> (f64, f64) {
    let (mut a, mut b, mut na, mut nb) = (0.0, 0.0, 0.0, 0.0);
    for _ in 0..frames {
        e.step(every);
        let s = block_on(e.read_state());
        for i in 0..s.n() {
            let sp = s.species[i];
            if sp == 1 || sp == 3 {
                let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
                let (cx, cy) = (((x / side).floor() + 0.5) * side, ((y / side).floor() + 0.5) * side);
                let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
                let (bound, far) = ((r < 0.8) as u32 as f64, (r > 4.0) as u32 as f64);
                if sp == 1 {
                    na += far;
                    a += bound;
                } else {
                    nb += far;
                    b += bound;
                }
            }
        }
    }
    (a / na, b / nb)
}

#[test]
#[ignore]
fn tune_binding() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (ea, eb, seed) in [(1.2, 1.5, 1.0), (1.2, 1.5, 2.0), (1.2, 1.5, 3.0)] {
        let sc = build("binding", &Options::default().with("eps_a", ea).with("eps_b", eb).with("eps_ls_b", 0.3).with("boxes_x", 12.0).with("boxes_y", 12.0).with("seed", seed)).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        e.step(10000);
        let (ka, kb) = occupancy(&mut e, 16.0, 600, 500);
        let ddg = -(kb / ka).ln();
        eprintln!("seed {seed} eps {ea}/{eb}: bound/far A {ka:.3}, B {kb:.3}; ΔΔG(B − A) = {ddg:.2} kT");
    }
}

/// LJ shape with ε = 1, truncated and shifted at 2.5 σ.
fn phi(r2: f64) -> f64 {
    if r2 >= 6.25 {
        return 0.0;
    }
    let s6 = 1.0 / (r2 * r2 * r2);
    let c6 = 1.0 / 6.25f64.powi(3);
    4.0 * s6 * (s6 - 1.0) - 4.0 * c6 * (c6 - 1.0)
}

#[test]
#[ignore]
fn fep_matches_direct_counting() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let (ea, eb) = (1.2, 1.5);
    let (sa, sb) = (0.5, 0.3);
    let sc = build("fep", &Options::default().with("eps_lp", ea).with("eps_ls", sa).with("tether", 20.0)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(5000);
    let windows = 11;
    let mut fwd = [vec![], vec![]];
    let mut dudl = [vec![], vec![]];
    for w in 0..windows {
        let lam = w as f64 / (windows - 1) as f64;
        let eps = ea + lam * (eb - ea);
        let ls = sa + lam * (sb - sa);
        let sys = e.sys.clone().with_pair(1, 2, md_core::Pair { eps, sigma: 1.0, rc: 2.5 }).with_pair(0, 1, md_core::Pair { eps: ls, sigma: 1.0, rc: 2.5 });
        e.update_system(sys);
        e.step(2000);
        let mut samples = [vec![], vec![]];
        for _ in 0..300 {
            e.step(100);
            let s = block_on(e.read_state());
            for leg in 0..2 {
                let lig = (0..s.n()).find(|&i| s.species[i] == 1 && ((fixed::to_f64(s.pos[i][0]) < 16.0) == (leg == 0))).unwrap();
                let lp = [fixed::to_f64(s.pos[lig][0]), fixed::to_f64(s.pos[lig][1])];
                let (mut pocket, mut solvent) = (0.0, 0.0);
                for i in 0..s.n() {
                    let q = [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])];
                    // Same compartment only.
                    if (q[0] < 16.0) != (leg == 0) {
                        continue;
                    }
                    let r2 = (q[0] - lp[0]).powi(2) + (q[1] - lp[1]).powi(2);
                    match s.species[i] {
                        2 => pocket += phi(r2),
                        0 => solvent += phi(r2),
                        _ => {}
                    }
                }
                samples[leg].push(pocket * (eb - ea) + solvent * (sb - sa)); // ∂U/∂λ
            }
        }
        for leg in 0..2 {
            let mean = samples[leg].iter().sum::<f64>() / samples[leg].len() as f64;
            dudl[leg].push(mean);
            if w + 1 < windows {
                let dl = 1.0 / (windows - 1) as f64;
                let avg = samples[leg].iter().map(|x| (-x * dl).exp()).sum::<f64>() / samples[leg].len() as f64;
                fwd[leg].push(-avg.ln());
            }
        }
    }
    for leg in 0..2 {
        let ti: f64 = (0..windows - 1).map(|k| 0.5 * (dudl[leg][k] + dudl[leg][k + 1]) / (windows - 1) as f64).sum();
        let fep: f64 = fwd[leg].iter().sum();
        eprintln!("{} leg: ΔG(A→B) by TI {ti:.2} kT, by FEP {fep:.2} kT", if leg == 0 { "bound" } else { "free" });
    }
}

/// Escape rates of the ligands over `frames` snapshots `every` steps apart, one per minimum age
/// in `ages` (in snapshots): (escapes, time bound in τ). A ligand is caught within 0.8 σ of its
/// pocket and escapes when it next gets beyond 3 σ. Only episodes that last longer than the
/// minimum age count, and only their time past it: for exponentially distributed residence
/// times this gives the same rate, but leaves out ligands that pass through the pocket and leave
/// at once.
fn escapes(e: &mut GpuEngine, side: f64, frames: usize, every: usize, ages: &[usize]) -> Vec<(f64, f64)> {
    let mut since: Vec<Option<usize>> = vec![];
    let mut out = vec![(0.0, 0.0); ages.len()];
    for f in 0..frames {
        e.step(every);
        let s = block_on(e.read_state());
        let r: Vec<f64> = (0..s.n())
            .filter(|&i| s.species[i] == 1 || s.species[i] == 3)
            .map(|i| {
                let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
                let (cx, cy) = (((x / side).floor() + 0.5) * side, ((y / side).floor() + 0.5) * side);
                ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()
            })
            .collect();
        since.resize(r.len(), None);
        for (c, r) in since.iter_mut().zip(&r) {
            match *c {
                None if *r < 0.8 => *c = Some(f),
                Some(f0) if *r > 3.0 => {
                    for (o, &m) in out.iter_mut().zip(ages) {
                        if f - f0 > m {
                            o.0 += 1.0;
                        }
                    }
                    *c = None;
                }
                _ => {}
            }
            if let Some(f0) = *c {
                for (o, &m) in out.iter_mut().zip(ages) {
                    if f - f0 >= m {
                        o.1 += every as f64 * 0.005;
                    }
                }
            }
        }
    }
    out
}

/// Escape rate from the pocket against temperature: an Arrhenius plot, measured the way the
/// `arrhenius` page does it, sweeping one run of 768 boxes of 8 σ from hot to cold.
///
/// Two lessons from `escape_rate_drift`: from a start with every ligand free, the rate per unit
/// time bound is high at first and settles as occupancy reaches equilibrium (~500 τ at kT = 0.8,
/// ~100 τ at 1.6), so each temperature needs its own equilibration; and the bound time has more
/// scatter than Poisson counting suggests (long episodes), so many boxes beat long runs.
#[test]
#[ignore]
fn arrhenius() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let opts = Options::default().with("eps_a", 1.6).with("eps_b", 1.6).with("boxes_x", 32.0).with("boxes_y", 24.0).with("log2_box", 3.0).with("seed", 23.0);
    let sc = build("binding", &opts).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(sc.warmup);
    let mut pts = vec![];
    for (t, equil) in [(1.6, 10000), (1.25, 20000), (1.0, 40000), (0.8, 60000)] {
        e.update_system(e.sys.clone().with_thermostat(t, 0.5));
        e.step(equil);
        let (n, b) = escapes(&mut e, 8.0, 200, 200, &[0])[0];
        eprintln!("T {t}: {n} escapes, {:.0}% bound, k {:.4}", 100.0 * b / (768.0 * 200.0), n / b);
        pts.push((1.0 / t, (n / b).ln()));
    }
    let ea = -(pts[3].1 - pts[0].1) / (pts[3].0 - pts[0].0);
    eprintln!("Eₐ from the end points: {ea:.2} ε");
    assert!((4.0..7.0).contains(&ea));
}

/// How long the escape rate takes to settle at each temperature, from a start with every
/// ligand in the bulk: rates in successive blocks of 100 τ, in 768 boxes of 8 σ (as on the
/// page), counting episodes past a minimum age of 0 and 5 τ.
#[test]
#[ignore]
fn escape_rate_drift() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for t in [0.8, 1.0, 1.6] {
        let o = Options::default().with("eps_a", 1.6).with("eps_b", 1.6).with("boxes_x", 32.0).with("boxes_y", 24.0).with("log2_box", 3.0).with("temperature", t).with("seed", 23.0);
        let sc = build("binding", &o).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        e.step(2000);
        let blocks: Vec<String> = (0..8)
            .map(|_| {
                let r = escapes(&mut e, 8.0, 100, 200, &[0, 5]);
                let ks: Vec<String> = r.iter().map(|(n, b)| format!("{:.4}", n / b)).collect();
                format!("[{}] {:.0}%", ks.join(" "), 100.0 * r[0].1 / (768.0 * 100.0))
            })
            .collect();
        eprintln!("T {t}: {}", blocks.join(" "));
    }
}

/// Kinetic temperature of the solvent after retuning the thermostat, against a fresh start.
#[test]
#[ignore]
fn retuned_temperature() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let opts = |t: f64| Options::default().with("eps_a", 1.6).with("eps_b", 1.6).with("boxes_x", 16.0).with("boxes_y", 12.0).with("temperature", t).with("seed", 23.0);
    let kt = |e: &GpuEngine| {
        let s = block_on(e.read_state());
        let dt = e.sys.dt;
        let (mut sum, mut n) = (0.0, 0.0);
        for i in 0..s.n() {
            if s.species[i] == 2 {
                continue;
            }
            let v = s.vel[i].map(|x| fixed::to_f64(x as i64) / dt);
            sum += 0.5 * (v[0] * v[0] + v[1] * v[1]);
            n += 1.0;
        }
        sum / n
    };
    let sc = build("binding", &opts(1.0)).unwrap();
    let mut swept = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    swept.step(2000);
    for t in [1.6, 0.8] {
        swept.update_system(swept.sys.clone().with_thermostat(t, 0.5));
        swept.step(20000);
        let sc = build("binding", &opts(t)).unwrap();
        let mut fresh = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        fresh.step(22000);
        let (a, b) = (kt(&swept), kt(&fresh));
        eprintln!("T {t}: retuned kT {a:.3}, fresh kT {b:.3}");
        assert!((a / t - 1.0).abs() < 0.03 && (b / t - 1.0).abs() < 0.03);
    }
}
