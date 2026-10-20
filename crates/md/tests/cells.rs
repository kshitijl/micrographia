//! Several model cells: how far their membrane springs stretch, averaged over 2 τ, in pure
//! water, saline and brine (to set a threshold for bursting).

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

#[test]
#[ignore]
fn membrane_stretch() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let beads = 44usize;
    let k: f64 = std::env::var("K").map_or(20.0, |v| v.parse().unwrap());
    let bend: f64 = std::env::var("BEND").map_or(10.0, |v| v.parse().unwrap());
    let inside: f64 = std::env::var("INSIDE").map_or(0.35, |v| v.parse().unwrap());
    let rest: f64 = std::env::var("REST").map_or(1.0, |v| v.parse().unwrap());
    for outside in [0.0, inside, 0.9] {
        let o = Options::default().with("cells_x", 2.0).with("cells_y", 2.0).with("width", 48.0).with("inside", inside).with("outside", outside).with("stiffness", k).with("bend", bend).with("rest", rest).with("seed", 19.0);
        let sc = build("cell", &o).unwrap();
        let r0 = rest * std::f64::consts::TAU * 10.0 / beads as f64;
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut avg = vec![0.0; 4 * beads];
        let mut window = 0;
        let (mut worst, mut mean_late) = (0.0f64, 0.0);
        let mut area = 0.0;
        let mut na = 0.0;
        let mut late = 0.0;
        for f in 0..1500 {
            e.step(40); // 0.2 τ
            let s = block_on(e.read_positions());
            for c in 0..4 {
                for k in 0..beads {
                    let (a, b) = (s[c * beads + k], s[c * beads + (k + 1) % beads]);
                    let d = (fixed::to_f64(a[0] - b[0]).powi(2) + fixed::to_f64(a[1] - b[1]).powi(2)).sqrt();
                    avg[c * beads + k] += d / 10.0;
                }
            }
            if f > 500 {
                for c in 0..4 {
                    let mut a2 = 0.0;
                    for k in 0..beads {
                        let (p, q) = (s[c * beads + k], s[c * beads + (k + 1) % beads]);
                        a2 += fixed::to_f64(p[0]) * fixed::to_f64(q[1]) - fixed::to_f64(q[0]) * fixed::to_f64(p[1]);
                    }
                    area += a2.abs() / 2.0;
                    na += 1.0;
                }
            }
            window += 1;
            if window == 10 {
                // A 2 τ average of each spring's length.
                for x in avg.iter_mut() {
                    if f > 500 {
                        worst = worst.max(*x / r0);
                        mean_late += *x / r0;
                        late += 1.0;
                    }
                    *x = 0.0;
                }
                window = 0;
            }
        }
        eprintln!("k {k} bend {bend} rest {rest} inside {inside} outside {outside}: area {:.0}; 2 τ-averaged spring length / rest after 100 τ: mean {:.3}, max {worst:.3}", area / na, mean_late / late);
    }
}

/// Big stiff cells (the saline page's second panel): the membrane tension, k·(mean spring length
/// − rest), averaged over the whole ring and over `WINDOW` τ, in pure water, saline and brine.
/// Env: K, RADIUS, BEADS, WIDTH, INSIDE, WINDOW, FRAMES (of 0.5 τ).
#[test]
#[ignore]
fn big_cell_tension() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let env = |k: &str, d: f64| std::env::var(k).map_or(d, |v| v.parse().unwrap());
    let (k, radius, beads, inside) = (env("K", 150.0), env("RADIUS", 25.0), env("BEADS", 110.0) as usize, env("INSIDE", 0.35));
    let window = (env("WINDOW", 10.0) * 2.0) as usize;
    let rest = env("REST", 1.0);
    for outside in [0.0, inside, env("BRINE", 0.9)] {
        let o = Options::default()
            .with("rest", rest)
            .with("cells_x", 1.0)
            .with("cells_y", 1.0)
            .with("width", env("WIDTH", 72.0))
            .with("radius", radius)
            .with("beads", beads as f64)
            .with("inside", inside)
            .with("outside", outside)
            .with("stiffness", k)
            .with("bend", env("BEND", 0.5 * k))
            .with("bath", env("BATH", 1.0))
            .with("seed", 19.0);
        let sc = build("cell", &o).unwrap();
        let r0 = rest * std::f64::consts::TAU * radius / beads as f64;
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = String::new();
        let (mut sum, mut n) = (0.0, 0);
        for f in 0..env("FRAMES", 400.0) as usize {
            e.step(100); // 0.5 τ
            let s = block_on(e.read_positions());
            for b in 0..beads {
                let (a, c) = (s[b], s[(b + 1) % beads]);
                sum += (fixed::to_f64(a[0] - c[0]).powi(2) + fixed::to_f64(a[1] - c[1]).powi(2)).sqrt();
                n += 1;
            }
            if (f + 1) % window == 0 {
                line += &format!(" {:.2}", k * (sum / n as f64 - r0));
                (sum, n) = (0.0, 0);
            }
        }
        eprintln!("outside {outside}: tension every {} τ:{line}", window / 2);
    }
}
