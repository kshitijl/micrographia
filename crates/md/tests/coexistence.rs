//! Liquid–vapour coexistence of the 2D Lennard-Jones fluid from a slab simulation.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::{fixed, init, Pair, State, System};
use pollster::block_on;

/// A dense slab across the middle of a tall periodic box; returns (liquid, vapour) densities.
pub fn slab(p: &Prims, rc: f64, t: f64, steps: usize) -> (f64, f64, Vec<f64>) {
    let (w, h) = (60.0, 160.0);
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::lj(rc), vec![1.0]).with_thermostat(t, 0.5);
    // Liquid at density 0.7 between h/2 ± 20.
    let slab = System::new([fixed::to_fixed(w), fixed::to_fixed(40.0)], init::DT, Pair::lj(rc), vec![1.0]);
    let n = (0.7 * w * 40.0) as usize;
    let mut s: State = init::fluid(&slab, n, t, 1);
    for q in s.pos.iter_mut() {
        q[1] += fixed::to_fixed(h / 2.0 - 20.0);
    }
    let mut e = GpuEngine::new(p, sys.clone(), &s);
    e.step(steps / 2);
    let bins = 80;
    let mut prof = vec![0f64; bins];
    let frames = 100;
    for _ in 0..frames {
        e.step(steps / 2 / frames);
        let st = block_on(e.read_state());
        // Centre on the slab's centre of mass (it wanders) using the circular mean.
        let (mut cs, mut sn) = (0.0, 0.0);
        for q in &st.pos {
            let a = fixed::to_f64(q[1]) / h * std::f64::consts::TAU;
            cs += a.cos();
            sn += a.sin();
        }
        let centre = (sn.atan2(cs) / std::f64::consts::TAU).rem_euclid(1.0) * h;
        for q in &st.pos {
            let y = (fixed::to_f64(q[1]) - centre + 1.5 * h).rem_euclid(h);
            prof[((y / h * bins as f64) as usize).min(bins - 1)] += 1.0;
        }
    }
    let area = w * h / bins as f64 * frames as f64;
    let rho: Vec<f64> = prof.iter().map(|c| c / area).collect();
    // Liquid: the central bins; vapour: the bins farthest from the slab.
    let liquid = rho[bins / 2 - 4..bins / 2 + 4].iter().sum::<f64>() / 8.0;
    let vapour = (rho[..6].iter().sum::<f64>() + rho[bins - 6..].iter().sum::<f64>()) / 12.0;
    (liquid, vapour, rho)
}

#[test]
#[ignore]
fn coexistence_curve() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for rc in [2.5, 4.0] {
        for t in [0.38, 0.40, 0.42, 0.44, 0.46] {
            let (l, v, _) = slab(&p, rc, t, 200_000);
            eprintln!("rc {rc}, T {t}: liquid {l:.3}, vapour {v:.4}");
        }
    }
}

/// Does a uniform gas at density `rho` and temperature `t` stay a gas, or condense? Returns the
/// fraction of molecules with at least two neighbours within 1.5 σ, over time.
#[test]
#[ignore]
fn does_the_gas_condense() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for (rho, t) in [(0.024, 0.40), (0.006, 0.40), (0.06, 0.44)] {
        let n = 8000;
        let sys = System::new(init::box_for(n, rho, 1.0), init::DT, Pair::lj(2.5), vec![1.0]).with_thermostat(t, 0.5);
        let s = init::fluid(&sys, n, t, 3);
        let mut e = GpuEngine::new(&p, sys.clone(), &s);
        let l = sys.box_f64();
        let mut line = vec![];
        for _ in 0..8 {
            e.step(25_000);
            let st = block_on(e.read_state());
            let pos: Vec<[f64; 2]> = st.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
            let cell = 1.5;
            let nc = [(l[0] / cell) as i64, (l[1] / cell) as i64];
            let mut grid = std::collections::HashMap::<(i64, i64), Vec<usize>>::new();
            for (i, q) in pos.iter().enumerate() {
                grid.entry(((q[0] / l[0] * nc[0] as f64) as i64, (q[1] / l[1] * nc[1] as f64) as i64)).or_default().push(i);
            }
            let mut bound = 0;
            for (i, q) in pos.iter().enumerate() {
                let c = ((q[0] / l[0] * nc[0] as f64) as i64, (q[1] / l[1] * nc[1] as f64) as i64);
                let mut k = 0;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if let Some(v) = grid.get(&((c.0 + dx).rem_euclid(nc[0]), (c.1 + dy).rem_euclid(nc[1]))) {
                            for &j in v {
                                if j != i {
                                    let mut d = [pos[j][0] - q[0], pos[j][1] - q[1]];
                                    for a in 0..2 {
                                        d[a] -= l[a] * (d[a] / l[a]).round();
                                    }
                                    if d[0] * d[0] + d[1] * d[1] < cell * cell {
                                        k += 1;
                                    }
                                }
                            }
                        }
                    }
                }
                if k >= 2 {
                    bound += 1;
                }
            }
            line.push(format!("{:.2}", bound as f64 / n as f64));
        }
        eprintln!("ρ {rho}, T {t}: share with ≥2 neighbours every 125 τ: {}", line.join(" "));
    }
}

/// A liquid slab with its vapour space pre-filled at density `rho_v`: does the vapour condense
/// onto the slab (if it is supersaturated) or evaporate from it (if too thin)?
#[test]
#[ignore]
fn slab_with_prefilled_vapour() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let cases: Vec<(f64, f64)> = match std::env::var("CASES") {
        Ok(c) => c.split(';').map(|p| { let v: Vec<f64> = p.split(',').map(|x| x.parse().unwrap()).collect(); (v[0], v[1]) }).collect(),
        Err(_) => vec![(0.40, 0.024), (0.40, 0.0), (0.44, 0.059)],
    };
    for (t, rho_v) in cases {
        let (w, h) = (60.0, 160.0);
        let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::lj(2.5), vec![1.0]).with_thermostat(t, 0.5);
        let slab = System::new([fixed::to_fixed(w), fixed::to_fixed(40.0)], init::DT, Pair::lj(2.5), vec![1.0]);
        let mut s: State = init::fluid(&slab, (0.7 * w * 40.0) as usize, t, 1);
        for q in s.pos.iter_mut() {
            q[1] += fixed::to_fixed(h / 2.0 - 20.0);
        }
        // Vapour at random in the two outer regions, at least 1.5 σ from the slab and each other.
        let nv = (rho_v * w * (h - 46.0)) as usize;
        let mut rng = gpu_prims::checks::SplitMix(11);
        let mut placed: Vec<[f64; 2]> = vec![];
        while placed.len() < nv {
            let x = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * w;
            let y = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * h;
            if (y - h / 2.0).abs() < 23.0 || placed.iter().any(|q| (q[0] - x).powi(2) + (q[1] - y).powi(2) < 2.25) {
                continue;
            }
            placed.push([x, y]);
        }
        for q in &placed {
            s.pos.push([fixed::to_fixed(q[0]), fixed::to_fixed(q[1])]);
            s.vel.push([0, 0]);
            s.species.push(0);
        }
        md_core::init::set_temperature(&mut s, &sys, t);
        let mut e = GpuEngine::new(&p, sys.clone(), &s);
        let mut line = vec![];
        for _ in 0..10 {
            e.step(20_000);
            let st = block_on(e.read_state());
            // Vapour: molecules more than 30 σ from the slab's centre (it barely moves).
            let (mut cs, mut sn) = (0.0, 0.0);
            for q in &st.pos {
                let a = fixed::to_f64(q[1]) / h * std::f64::consts::TAU;
                cs += a.cos();
                sn += a.sin();
            }
            let c = (sn.atan2(cs) / std::f64::consts::TAU).rem_euclid(1.0) * h;
            let far = st.pos.iter().filter(|q| ((fixed::to_f64(q[1]) - c + 1.5 * h).rem_euclid(h) - h / 2.0).abs() > 30.0).count();
            let near = st.pos.iter().filter(|q| ((fixed::to_f64(q[1]) - c + 1.5 * h).rem_euclid(h) - h / 2.0).abs() < 8.0).count();
            line.push(format!("{:.4}/{:.3}", far as f64 / (w * (h - 60.0)), near as f64 / (w * 16.0)));
        }
        eprintln!("T {t}, vapour started at {rho_v}: vapour/liquid density every 100 τ: {}", line.join(" "));
    }
}
