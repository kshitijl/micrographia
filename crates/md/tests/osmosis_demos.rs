//! The osmosis page's two demos: a free membrane that moves, and an osmometer whose level rises.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// With 50% of the right side's sites solute, the membrane moves from 48 σ to ~33 σ in 500 τ.
#[test]
#[ignore]
fn free_membrane_moves() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let f: f64 = std::env::var("SOLUTE").map_or(0.5, |v| v.parse().unwrap());
    let sc = build("osmosis_free", &Options::default().with("solute", f)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let mut line = String::new();
    for _ in 0..10 {
        e.step(10000);
        let s = block_on(e.read_state());
        let xs: Vec<f64> = (0..s.n()).filter(|&i| s.species[i] == 2).map(|i| fixed::to_f64(s.pos[i][0])).collect();
        let solute_left = (0..s.n()).filter(|&i| s.species[i] == 1 && fixed::to_f64(s.pos[i][0]) < xs.iter().cloned().fold(f64::MAX, f64::min)).count();
        line += &format!(" {:.1}", xs.iter().sum::<f64>() / xs.len() as f64);
        if solute_left > 0 {
            line += &format!("(leak {solute_left})");
        }
    }
    eprintln!("membrane x every 50 τ:{line}");
}

/// The surface height of the liquid in x ∈ [x0, x1): the top of the density profile (1 σ bins,
/// smoothed over 3), where it falls through 0.35 (vapour ~0.01, liquid ~0.7); and the liquid's
/// density, averaged over the lower half of the column.
pub fn surface(s: &md_core::State, x0: f64, x1: f64, h: f64) -> (f64, f64) {
    let bins = h as usize;
    let mut c = vec![0.0; bins];
    for i in 0..s.n() {
        let (x, y) = (fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1]));
        if s.species[i] != 2 && x >= x0 && x < x1 && y >= 0.0 && y < h {
            c[y as usize] += 1.0 / (x1 - x0);
        }
    }
    let sm: Vec<f64> = (0..bins).map(|k| (c[k.saturating_sub(1)] + c[k] + c[(k + 1).min(bins - 1)]) / 3.0).collect();
    let top = (0..bins).rev().find(|&k| sm[k] > 0.35).unwrap_or(0);
    // Interpolate within the bin where it crosses.
    let y = top as f64 + 0.5 + if top + 1 < bins { (sm[top] - 0.35) / (sm[top] - sm[top + 1]).max(1e-9) } else { 0.0 };
    let rho = sm[2..(top / 2).max(3)].iter().sum::<f64>() / ((top / 2).max(3) - 2) as f64;
    (y, rho)
}

/// Measured with the defaults (16 σ arms, g = 0.006, 30% solute, LJ cut at 4 σ, kT 0.5, walls
/// wetted at ε 0.7, an ideal solution), seeds 1 and 22: Δh rises to 8–17 σ within ~800 τ and then
/// jitters between ~4 and ~16 σ, about half of van 't Hoff's ~20 σ on average. The 2D surface of a
/// liquid 16 σ wide is ragged (films climb the walls and the membrane), so a single level reading
/// is uncertain by several σ. Things that made it worse: the usual 2.5 σ cutoff (the liquid at
/// kT 0.45 is too close to its critical point), bare hard walls (the liquid dewets into a blob),
/// fully wetting walls and a self-repelling solute (both gave ~1.5× van 't Hoff), weak gravity and
/// wide arms (thousands of τ). A clean measurement needs a bigger system, or 3D.
#[test]
#[ignore]
fn osmometer_rises() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let g: f64 = std::env::var("G").map_or(0.006, |v| v.parse().unwrap());
    let f: f64 = std::env::var("SOLUTE").map_or(0.3, |v| v.parse().unwrap());
    let width: f64 = std::env::var("WIDTH").map_or(32.0, |v| v.parse().unwrap());
    let wet: f64 = std::env::var("WET").map_or(0.7, |v| v.parse().unwrap());
    let t: f64 = std::env::var("T").map_or(0.5, |v| v.parse().unwrap());
    let seed: f64 = std::env::var("SEED").map_or(1.0, |v| v.parse().unwrap());
    let sc = build("osmometer", &Options::default().with("gravity", g).with("solute", f).with("width", width).with("wetting", wet).with("temperature", t).with("seed", seed)).unwrap();
    let w = fixed::to_f64(sc.sys.box_len[0]);
    let h = fixed::to_f64(sc.sys.box_len[1]);
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    for k in 0..10 {
        e.step(40000);
        let s = block_on(e.read_state());
        let (hl, rl) = surface(&s, 2.0, w / 2.0 - 1.0, h);
        let (hr, rr) = surface(&s, w / 2.0 + 1.0, w - 2.0, h);
        let sol = (0..s.n()).filter(|&i| s.species[i] == 1 && fixed::to_f64(s.pos[i][1]) < hr + 3.0).count() as f64;
        let c = sol / (hr * (w / 2.0 - 1.0));
        let pred = c * t / (0.5 * (rl + rr) * g);
        eprintln!("{} τ: surface left {hl:.1}, right {hr:.1}, Δh {:.1}; van 't Hoff Δh {pred:.1}; densities {rl:.3} {rr:.3}", (k + 1) * 200, hr - hl);
    }
}

/// The classic U-tube (`channel` > 0): the arms meet only through a passage at the bottom.
/// Measured (24 σ arms, 90 σ deep, a 30 σ passage, g 0.008, 30% solute, ~3300 molecules): with
/// no heat bath Δh reaches ~11 σ by 1000 τ and then wanders between 12 and 21 σ, against van 't
/// Hoff's ~16 σ. With the Andersen bath it is ~2 σ at 1000 τ and ~4 σ at 2000 τ: redrawing
/// velocities destroys the momentum the flow through the passage needs. (Earlier numbers were
/// spoiled by a wall bead placed on the box's top edge, which the wall bounced at 100 σ/τ.)
/// Env: WIDTH, HEIGHT, DEPTH, CHANNEL, G, SOLUTE, BATH (0 = constant energy), STEPS (per report).
#[test]
#[ignore]
fn utube_rises() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let env = |k: &str, d: f64| std::env::var(k).map_or(d, |v| v.parse().unwrap());
    let (g, f, t) = (env("G", 0.006), env("SOLUTE", 0.3), 0.5);
    let o = Options::default()
        .with("gravity", g)
        .with("solute", f)
        .with("width", env("WIDTH", 64.0))
        .with("height", env("HEIGHT", 140.0))
        .with("depth", env("DEPTH", 60.0))
        .with("channel", env("CHANNEL", 12.0))
        .with("bath", env("BATH", 1.0));
    let sc = build("osmometer", &o).unwrap();
    let w = fixed::to_f64(sc.sys.box_len[0]);
    let h = fixed::to_f64(sc.sys.box_len[1]);
    eprintln!("{} molecules", sc.state.n());
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let steps = env("STEPS", 40000.0) as usize;
    let start = std::time::Instant::now();
    for k in 0..env("REPORTS", 10.0) as usize {
        e.step(steps);
        let s = block_on(e.read_state());
        let (hl, rl) = surface(&s, 2.0, w / 2.0 - 1.0, h);
        let (hr, rr) = surface(&s, w / 2.0 + 1.0, w - 2.0, h);
        let sol = (0..s.n()).filter(|&i| s.species[i] == 1 && fixed::to_f64(s.pos[i][1]) < hr + 3.0).count() as f64;
        let c = sol / (hr * (w / 2.0 - 1.0));
        let pred = c * t / (0.5 * (rl + rr) * g);
        let ke = |i: usize| 0.5 * (sc.sys.velocity(s.vel[i][0]).powi(2) + sc.sys.velocity(s.vel[i][1]).powi(2));
        let fast = (0..s.n()).max_by(|&a, &b| ke(a).total_cmp(&ke(b))).unwrap();
        let mobile = (0..s.n()).filter(|&i| s.species[i] != 2).count() as f64;
        eprintln!(
            "  kT {:.3}; fastest: id {fast} species {} at ({:.1}, {:.1}), KE {:.1}",
            s.half_step_kinetic(&sc.sys) / mobile,
            s.species[fast],
            fixed::to_f64(s.pos[fast][0]),
            fixed::to_f64(s.pos[fast][1]),
            ke(fast)
        );
        eprintln!(
            "{:.0} τ ({:.0} s): left {hl:.1}, right {hr:.1}, Δh {:.1}; van 't Hoff {pred:.1}; liquid densities {rl:.3} {rr:.3}",
            ((k + 1) * steps) as f64 * sc.sys.dt,
            start.elapsed().as_secs_f64(),
            hr - hl
        );
    }
}

/// Debug: where the U-tube's potential energy sits, by species.
#[test]
#[ignore]
fn utube_energy_breakdown() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let o = Options::default().with("width", 50.0).with("height", 150.0).with("depth", 90.0).with("channel", 30.0).with("gravity", 0.008).with("bath", 0.0);
    let sc = build("osmometer", &o).unwrap();
    let e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.compute_forces();
    let (_, pe) = block_on(e.buf.read_kicks());
    let s = block_on(e.read_state());
    let mut by = [0.0f64; 3];
    for (k, &u) in pe.iter().enumerate() {
        by[s.species[k] as usize] += u as f64; // storage order == id order before any sort
    }
    let worst = (0..pe.len()).max_by(|&a, &b| pe[a].total_cmp(&pe[b])).unwrap();
    eprintln!("PE by species {by:?}; worst slot {worst}: {} species {} at ({:.2}, {:.2})", pe[worst], s.species[worst], fixed::to_f64(s.pos[worst][0]), fixed::to_f64(s.pos[worst][1]));
}

/// The U-tube's energy books with no heat bath, after settling as pure water for `SETTLE` τ:
/// changes in m g h, kinetic and interaction energy (each averaged over 100 readings 0.5 τ
/// apart), with SOLUTE (0.3) and without (0, the control). Env: SOLUTE, SETTLE, REPORTS.
#[test]
#[ignore]
fn utube_energy_books() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let env = |k: &str, d: f64| std::env::var(k).map_or(d, |v| v.parse().unwrap());
    let g = 0.008;
    let o = Options::default()
        .with("width", 50.0)
        .with("height", 150.0)
        .with("depth", 90.0)
        .with("channel", 30.0)
        .with("gravity", g)
        .with("bath", 0.0)
        .with("solute", env("SOLUTE", 0.3))
        .with("settle", env("SETTLE", 200.0));
    let sc = build("osmometer", &o).unwrap();
    let w = fixed::to_f64(sc.sys.box_len[0]);
    let h = fixed::to_f64(sc.sys.box_len[1]);
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(sc.warmup);
    let mut s = block_on(e.read_state());
    sc.finish.apply(&mut s, &sc.sys);
    e.upload(&s);
    let mobile: Vec<usize> = (0..s.n()).filter(|&i| s.species[i] != 2).collect();
    let mut first: Option<[f64; 3]> = None;
    for k in 0..env("REPORTS", 10.0) as usize {
        let mut acc = [0.0; 3];
        let mut dh = 0.0;
        for _ in 0..100 {
            e.step(100);
            let (_, pe) = block_on(e.buf.read_kicks());
            let s = block_on(e.read_state());
            let lift: f64 = mobile.iter().map(|&i| g * fixed::to_f64(s.pos[i][1])).sum();
            let kin = s.half_step_kinetic(&sc.sys);
            acc[0] += lift / 100.0;
            acc[1] += kin / 100.0;
            acc[2] += pe.iter().map(|&u| u as f64).sum::<f64>() / 100.0;
            dh += (surface(&s, w / 2.0 + 1.0, w - 2.0, h).0 - surface(&s, 2.0, w / 2.0 - 1.0, h).0) / 100.0;
        }
        let f = *first.get_or_insert(acc);
        eprintln!(
            "{} τ: Δh {dh:.1}; lift {:+.1}, heat {:+.1}, interactions {:+.1}, total {:+.1}; kT {:.4}",
            (k + 1) * 50,
            acc[0] - f[0],
            acc[1] - f[1],
            acc[2] - f[2],
            acc[0] + acc[1] + acc[2] - f[0] - f[1] - f[2],
            acc[1] / mobile.len() as f64
        );
    }
}
