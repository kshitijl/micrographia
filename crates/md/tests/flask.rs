//! The latent-heat flask: heat a droplet in steps and record temperature and vapour fraction.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// Fraction of molecules with fewer than 2 neighbours within 1.5 σ (vapour).
fn vapour_fraction(s: &md_core::State) -> f64 {
    let n = s.n();
    let p: Vec<[f64; 2]> = s.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
    let cell = 1.5;
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, q) in p.iter().enumerate() {
        grid.entry(((q[0] / cell) as i64, (q[1] / cell) as i64)).or_default().push(i);
    }
    let mut vap = 0;
    for (i, q) in p.iter().enumerate() {
        let (cx, cy) = ((q[0] / cell) as i64, (q[1] / cell) as i64);
        let mut nb = 0;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = grid.get(&(cx + dx, cy + dy)) {
                    for &j in v {
                        if j != i && (p[j][0] - q[0]).powi(2) + (p[j][1] - q[1]).powi(2) < cell * cell {
                            nb += 1;
                        }
                    }
                }
            }
        }
        vap += (nb < 2) as usize;
    }
    vap as f64 / n as f64
}

#[test]
#[ignore]
fn heating_curve() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let env = |k: &str, d: f64| std::env::var(k).map_or(d, |v| v.parse().unwrap());
    let n = env("N", 1000.0);
    let sc = build("flask", &Options::default().with("n", n)).unwrap();
    let side = fixed::to_f64(sc.sys.box_len[0]);
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let dq = env("DQ", 0.1); // heat per molecule per step
    let mut added = 0.0;
    for k in 0..env("STEPS", 40.0) as usize {
        if k > 0 {
            let s = block_on(e.read_state());
            let kin = s.half_step_kinetic(&sc.sys);
            let scale = (1.0 + dq * n / kin).sqrt();
            e.kick_region([-1.0, -1.0], [side + 1.0, side + 1.0], scale, [0.0, 0.0]);
            added += dq;
        }
        e.step(env("WAIT", 10000.0) as usize);
        let s = block_on(e.read_state());
        let t = s.half_step_kinetic(&sc.sys) / n;
        eprintln!("heat {added:.2} per molecule: kT {t:.3}, vapour {:.2}", vapour_fraction(&s));
    }
}

/// Debug: the boiling scenario at several sizes; closest starting pair and kT after warm-up.
#[test]
#[ignore]
fn boiling_sizes() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for n in [6000.0, 24000.0, 48000.0] {
        let f = (n / 6000.0f64).sqrt();
        let o = Options::default().with("n", n).with("width", 200.0 * f).with("height", 160.0 * f).with("temperature", 0.42).with("plate", 0.6);
        let sc = build("boiling", &o).unwrap();
        let pos: Vec<[f64; 2]> = sc.state.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
        let w = 200.0 * f;
        let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
        for (i, q) in pos.iter().enumerate() {
            grid.entry(((q[0] / 2.0) as i64, (q[1] / 2.0) as i64)).or_default().push(i);
        }
        let gx = (w / 2.0) as i64 + 1;
        let mut dmin = f64::MAX;
        for (i, q) in pos.iter().enumerate() {
            let (cx, cy) = ((q[0] / 2.0) as i64, (q[1] / 2.0) as i64);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(v) = grid.get(&((cx + dx).rem_euclid(gx), cy + dy)) {
                        for &j in v {
                            if j != i {
                                let mut ddx = pos[j][0] - q[0];
                                ddx -= w * (ddx / w).round();
                                dmin = dmin.min((ddx * ddx + (pos[j][1] - q[1]).powi(2)).sqrt());
                            }
                        }
                    }
                }
            }
        }
        eprintln!("n {n}: closest starting pair {dmin:.3} σ");
        let mut ys: Vec<f64> = pos.iter().map(|q| q[1]).collect();
        ys.sort_by(f64::total_cmp);
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let s0 = block_on(e.read_state());
        let t0 = s0.half_step_kinetic(&sc.sys) / n;
        e.step(sc.warmup);
        let s = block_on(e.read_state());
        let t = s.half_step_kinetic(&sc.sys) / n;
        eprintln!("n {n}: box {:.0}×{:.0}, y range {:.1}..{:.1}, kT start {t0:.3}, after warm-up {} steps {t:.3}", 200.0 * f, 160.0 * f, ys[0], ys[ys.len() - 1], sc.warmup);
    }
}

/// Regression: a big pool under a big vapour space (average density far below the liquid's)
/// must not overflow the Verlet lists, which used to be sized from the average density.
#[test]
fn big_boiling_pool_lists_do_not_overflow() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let n = 24000.0;
    let o = Options::default().with("n", n).with("width", 400.0).with("height", 320.0).with("temperature", 0.42).with("warmup", 0.0);
    let sc = build("boiling", &o).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(2000);
    let (_, overflow) = block_on(e.buf.read_list_stats());
    let t = block_on(e.read_state()).half_step_kinetic(&sc.sys) / n;
    assert!(!overflow, "neighbour lists overflowed");
    assert!(t < 0.6, "kT {t} after 2000 steps");
}
