//! The emulsion panel: how fast the oil–water boundary shrinks, with and without soap.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// Water–oil neighbour pairs (within 1.5 σ) per molecule, and the number of oil clusters (oil and
/// tail beads joined within 1.5 σ) of at least 10 members.
fn boundary(s: &md_core::State, b: [f64; 2]) -> (f64, usize) {
    let p: Vec<[f64; 2]> = s.pos.iter().map(|q| [fixed::to_f64(q[0]), fixed::to_f64(q[1])]).collect();
    let cell = 1.5;
    let (gx, gy) = ((b[0] / cell) as i64, (b[1] / cell) as i64);
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (i, q) in p.iter().enumerate() {
        grid.entry((((q[0] / cell) as i64).rem_euclid(gx), ((q[1] / cell) as i64).rem_euclid(gy))).or_default().push(i);
    }
    let oily = |sp: u32| sp == 1 || sp == 3;
    let mut parent: Vec<usize> = (0..p.len()).collect();
    fn find(parent: &mut Vec<usize>, mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut contacts = 0usize;
    for (i, q) in p.iter().enumerate() {
        let (cx, cy) = ((q[0] / cell) as i64, (q[1] / cell) as i64);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = grid.get(&((cx + dx).rem_euclid(gx), (cy + dy).rem_euclid(gy))) {
                    for &j in v {
                        if j <= i {
                            continue;
                        }
                        let mut ddx = p[j][0] - q[0];
                        ddx -= b[0] * (ddx / b[0]).round();
                        let mut ddy = p[j][1] - q[1];
                        ddy -= b[1] * (ddy / b[1]).round();
                        if ddx * ddx + ddy * ddy < cell * cell {
                            let (a, c) = (s.species[i], s.species[j]);
                            if (a == 0 && c == 1) || (a == 1 && c == 0) {
                                contacts += 1;
                            }
                            if oily(a) && oily(c) {
                                let (ra, rc) = (find(&mut parent, i), find(&mut parent, j));
                                parent[ra] = rc;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut sizes: std::collections::HashMap<usize, usize> = Default::default();
    for i in 0..p.len() {
        if oily(s.species[i]) {
            let r = find(&mut parent, i);
            *sizes.entry(r).or_default() += 1;
        }
    }
    (contacts as f64 / p.len() as f64, sizes.values().filter(|&&c| c >= 10).count())
}

#[test]
#[ignore]
fn soap_stops_coarsening() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let env = |k: &str, d: f64| std::env::var(k).map_or(d, |v| v.parse().unwrap());
    for soap in [0.0, 0.1, 0.2] {
        let o = Options::default().with("n", env("N", 8000.0)).with("soap", soap).with("cross", env("CROSS", 0.3)).with("temperature", env("T", 0.7)).with("soap_cross", env("SOAPX", 0.0)).with("oil", env("OIL", 0.25));
        let sc = build("emulsion", &o).unwrap();
        let b = sc.sys.box_f64();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = String::new();
        for k in 0..env("REPORTS", 8.0) as usize {
            e.step(env("STEPS", 20000.0) as usize);
            let s = block_on(e.read_state());
            let (c, drops) = boundary(&s, b);
            let t = s.half_step_kinetic(&sc.sys) / s.n() as f64;
            line += &format!(" [{} τ: {c:.3}, {drops} drops, kT {t:.2}]", (k + 1) * env("STEPS", 20000.0) as usize / 200);
        }
        eprintln!("soap {soap}:{line}");
    }
}
