//! The entropic spring: tension of pinned bead–spring chains against temperature.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

fn tension(t: f64, eq: usize, frames: usize) -> (f64, f64) {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = build("rubber", &Options::default().with("chains", 64.0).with("stretch", 0.5).with("temperature", t)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(eq);
    let (mut f, mut ke, mut nke) = (0.0, 0.0, 0.0);
    for _ in 0..frames {
        e.step(100);
        let s = block_on(e.read_state());
        let n = s.n();
        let mut anchors = vec![];
        let mut beads = vec![];
        for i in 0..n {
            match s.species[i] {
                3 => anchors.push(s.pos[i]),
                2 => beads.push(s.pos[i]),
                _ => {}
            }
            if !sc.sys.is_pinned(s.species[i]) {
                let vx = sc.sys.velocity(s.vel[i][0]);
                let vy = sc.sys.velocity(s.vel[i][1]);
                ke += 0.5 * (vx * vx + vy * vy);
                nke += 1.0;
            }
        }
        for (a, b) in anchors.iter().zip(&beads) {
            let dx = fixed::to_f64(a[0] - b[0]);
            let dy = fixed::to_f64(a[1] - b[1]);
            let r2 = dx * dx + dy * dy;
            let mut fr = -30.0 / (1.0 - (r2 / 2.25).min(0.999999));
            if r2 < 1.2599 {
                let s6 = 1.0 / (r2 * r2 * r2);
                fr += 24.0 * s6 * (2.0 * s6 - 1.0) / r2;
            }
            f += -fr * dx / anchors.len() as f64;
        }
    }
    (f / frames as f64, ke / nke)
}

#[test]
#[ignore]
fn tension_against_temperature() {
    for (t, eq) in [(0.5, 20000), (1.0, 20000), (2.0, 20000), (2.0, 60000)] {
        let (f, tm) = tension(t, eq, 200);
        eprintln!("T = {t} (measured {tm:.3}), equilibrated {eq} steps: tension {f:.3}, tension/T {:.3}", f / tm);
    }
}

/// With a spring gauge on the right ends: how far the springs stretch at kT 1 and 2.
#[test]
#[ignore]
fn spring_gauge() {
    let p = gpu_prims::Prims::new(&pollster::block_on(gpu_prims::Gpu::new()).unwrap());
    for t in [1.0, 2.0] {
        let o = scenarios::Options::default().with("chains", 64.0).with("length", 40.0).with("stretch", 0.5).with("temperature", t).with("gauge", 0.12).with("handle_mass", 5.0).with("seed", 10.0);
        let sc = scenarios::build("rubber", &o).unwrap();
        let mut e = md::GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = String::new();
        for k in 0..6 {
            e.step(10000);
            let s = pollster::block_on(e.read_state());
            // Each chain ends: ..., handle (species 3), post (species 1).
            let mut d = 0.0;
            let mut n = 0.0;
            for i in 0..s.n() {
                if s.species[i] == 3 {
                    d += md_core::fixed::to_f64(s.pos[i + 1][0] - s.pos[i][0]);
                    n += 1.0;
                }
            }
            line += &format!(" {}τ: {:.2}σ", (k + 1) * 50, d / n);
        }
        eprintln!("kT {t}: spring stretch{line}");
    }
}

/// The rubber page's part 2 (spring on the left, right ends pulled): does anything blow up?
#[test]
#[ignore]
fn left_gauge_pull() {
    let p = gpu_prims::Prims::new(&pollster::block_on(gpu_prims::Gpu::new()).unwrap());
    let side: f64 = std::env::var("SIDE").map_or(-1.0, |v| v.parse().unwrap());
    let o = scenarios::Options::default().with("chains", 64.0).with("length", 40.0).with("stretch", 0.5).with("temperature", 1.0).with("gauge", std::env::var("GAUGE").map_or(0.07, |v| v.parse().unwrap())).with("handle_mass", std::env::var("MASS").map_or(20.0, |v| v.parse().unwrap())).with("gauge_side", side).with("harmonic", std::env::var("HARMONIC").map_or(0.0, |v| v.parse().unwrap())).with("seed", 12.0);
    let sc = scenarios::build("rubber", &o).unwrap();
    let mut e = md::GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    e.step(sc.warmup);
    let rate: f64 = std::env::var("RATE").map_or(0.0, |v| v.parse().unwrap());
    e.update_system(e.sys.clone().with_thermostat(1.0, rate));
    let v: f64 = std::env::var("V").map_or(0.3, |v| v.parse().unwrap());
    e.set_species_velocity(2, [v, 0.0]);
    // Move the end for 50 τ (6 σ at 0.12 σ/τ), then hold.
    for k in 0..40 {
        if k == 20 {
            e.set_species_velocity(2, [0.0, 0.0]);
        }
        e.step(500);
        let s = pollster::block_on(e.read_state());
        let (mut vmax, mut who) = (0.0f64, 0usize);
        for i in 0..s.n() {
            let v = s.vel[i].map(|x| e.sys.velocity(x));
            let sp = (v[0] * v[0] + v[1] * v[1]).sqrt();
            if sp > vmax {
                vmax = sp;
                who = i;
            }
        }
        eprintln!("{} τ: fastest particle {who} (species {}, chain position {}) at {vmax:.2} σ/τ, x {:.1}", (k + 1) as f64 * 2.5, s.species[who], who % 41, md_core::fixed::to_f64(s.pos[who][0]));
    }
}

/// The same gauge set-up on the CPU twin, insulated: does it blow up there too?
#[test]
#[ignore]
fn gauge_on_cpu() {
    let o = scenarios::Options::default().with("chains", 8.0).with("length", 40.0).with("stretch", 0.5).with("temperature", 1.0).with("gauge", 0.07).with("seed", 12.0);
    let sc = scenarios::build("rubber", &o).unwrap();
    let mut e = md_ref::IntEngine::<f32>::new(sc.sys.clone().with_thermostat(1.0, 0.0), sc.state.clone());
    for k in 0..10 {
        e.step(1000);
        let vmax = e.state.vel.iter().map(|v| v.map(|x| e.sys.velocity(x))).map(|v| (v[0] * v[0] + v[1] * v[1]).sqrt()).fold(0.0, f64::max);
        eprintln!("CPU {} τ: fastest {vmax:.2} σ/τ", (k + 1) * 5);
    }
}

/// GPU against the CPU twin for the gauge set-up: where do they part?
#[test]
#[ignore]
fn gauge_gpu_vs_cpu() {
    let p = gpu_prims::Prims::new(&pollster::block_on(gpu_prims::Gpu::new()).unwrap());
    let o = scenarios::Options::default().with("chains", 8.0).with("length", 40.0).with("stretch", 0.5).with("temperature", 1.0).with("gauge", 0.07).with("seed", 12.0);
    let sc = scenarios::build("rubber", &o).unwrap();
    let sys = sc.sys.clone().with_thermostat(1.0, 0.0);
    let mut g = md::GpuEngine::new(&p, sys.clone(), &sc.state);
    if std::env::var("NOSORT").is_ok() {
        g.sort_interval = 0;
    }
    eprintln!("sort interval {}", g.sort_interval);
    let mut c = md_ref::IntEngine::<f32>::new(sys.clone(), sc.state.clone());
    let ke = |s: &md_core::State| s.vel.iter().map(|v| v.map(|x| sys.velocity(x))).map(|v| v[0] * v[0] + v[1] * v[1]).sum::<f64>();
    for k in 0..200 {
        g.step(50);
        c.step(50);
        let gs = pollster::block_on(g.read_state());
        let (kg, kc) = (ke(&gs), ke(&c.state));
        if k % 20 == 0 || (kg - kc).abs() > 0.5 * kc {
            let worst = (0..gs.n()).max_by_key(|&i| (gs.vel[i][0] as i64 - c.state.vel[i][0] as i64).abs()).unwrap();
            eprintln!("step {}: KE GPU {kg:.1}, CPU {kc:.1}; most different particle {worst} species {} (chain pos {})", (k + 1) * 50, gs.species[worst], worst % 41);
            if (kg - kc).abs() > 0.5 * kc {
                break;
            }
        }
    }
}

/// Find the first particle to go fast on the GPU (gauge set-up), and what is around it.
#[test]
#[ignore]
fn gauge_first_runaway() {
    let p = gpu_prims::Prims::new(&pollster::block_on(gpu_prims::Gpu::new()).unwrap());
    let o = scenarios::Options::default().with("chains", 8.0).with("length", 40.0).with("stretch", 0.5).with("temperature", 1.0).with("gauge", 0.07).with("seed", 12.0).with("harmonic", std::env::var("HARMONIC").map_or(0.0, |v| v.parse().unwrap()));
    let sc = scenarios::build("rubber", &o).unwrap();
    let sys = sc.sys.clone().with_thermostat(1.0, 0.0);
    let mut g = md::GpuEngine::new(&p, sys.clone(), &sc.state);
    let f = |x: i64| md_core::fixed::to_f64(x);
    for k in 0..2000 {
        g.step(5);
        let s = pollster::block_on(g.read_state());
        let speed = |i: usize| s.vel[i].map(|x| sys.velocity(x)).iter().map(|v| v * v).sum::<f64>().sqrt();
        let w = (0..s.n()).max_by(|&a, &b| speed(a).total_cmp(&speed(b))).unwrap();
        if speed(w) > 8.0 {
            let chain = w / 41;
            eprintln!("step {}: particle {w} (species {}, chain {chain}, position {}) at {:.1} σ/τ, at ({:.3}, {:.3})", (k + 1) * 5, s.species[w], w % 41, speed(w), f(s.pos[w][0]), f(s.pos[w][1]));
            for j in chain * 41..chain * 41 + 41 {
                let d = ((f(s.pos[j][0]) - f(s.pos[w][0])).powi(2) + (f(s.pos[j][1]) - f(s.pos[w][1])).powi(2)).sqrt();
                if d < 2.0 && j != w {
                    eprintln!("  near: {j} (species {}, position {}) at {d:.3} σ, speed {:.1}", s.species[j], j % 41, speed(j));
                }
            }
            break;
        }
    }
}

/// Step by step, the handle that runs away (harmonic chains, chain 4: handle 203, post 204).
#[test]
#[ignore]
fn gauge_handle_trace() {
    let p = gpu_prims::Prims::new(&pollster::block_on(gpu_prims::Gpu::new()).unwrap());
    let o = scenarios::Options::default().with("chains", 8.0).with("length", 40.0).with("stretch", 0.5).with("temperature", 1.0).with("gauge", 0.07).with("seed", 12.0).with("harmonic", 1.0);
    let sc = scenarios::build("rubber", &o).unwrap();
    let sys = sc.sys.clone().with_thermostat(1.0, 0.0);
    let mut g = md::GpuEngine::new(&p, sys.clone(), &sc.state);
    g.step(4900);
    let f = |x: i64| md_core::fixed::to_f64(x);
    for k in 0..40 {
        g.step(1);
        let s = pollster::block_on(g.read_state());
        let (kicks, _) = pollster::block_on(g.kicks());
        let line: Vec<String> = [202usize, 203, 204]
            .iter()
            .map(|&i| format!("{i}: x ({:.3},{:.3}) v ({:.2},{:.2}) kick ({}, {})", f(s.pos[i][0]), f(s.pos[i][1]), sys.velocity(s.vel[i][0]), sys.velocity(s.vel[i][1]), kicks[i][0], kicks[i][1]))
            .collect();
        eprintln!("step {}: {}", 4901 + k, line.join(" | "));
    }
}
