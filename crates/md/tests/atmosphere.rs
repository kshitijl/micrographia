//! An isolated gas column under gravity, from starts far from equilibrium.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{build, Options};

/// (temperature in the bottom and top fifths, fitted log-density slope of the light gas against
/// −m g / kT, kT overall) from one snapshot.
fn profile(e: &GpuEngine) -> (f64, f64, f64, f64) {
    let s = block_on(e.read_state());
    let h = fixed::to_f64(e.sys.box_len[1]);
    let g = -e.sys.gravity[1];
    let bins = 10;
    let mut c = vec![0.0f64; bins];
    let (mut kb, mut nb, mut kt, mut nt, mut k_all) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..s.n() {
        let y = fixed::to_f64(s.pos[i][1]);
        let m = e.sys.masses[s.species[i] as usize];
        let v = s.vel[i].map(|x| e.sys.velocity(x));
        let ke = 0.5 * m * (v[0] * v[0] + v[1] * v[1]);
        k_all += ke;
        if y < h / 5.0 {
            kb += ke;
            nb += 1.0;
        } else if y > 0.6 * h && y < 0.8 * h {
            kt += ke;
            nt += 1.0;
        }
        if s.species[i] == 0 {
            c[((y / h * bins as f64) as usize).min(bins - 1)] += 1.0;
        }
    }
    let kt_all = k_all / s.n() as f64;
    // Least-squares slope of ln(count) against height over bins 1..8 (away from floor and ceiling).
    let pts: Vec<(f64, f64)> = (1..bins - 1).filter(|&b| c[b] > 5.0).map(|b| ((b as f64 + 0.5) * h / bins as f64, c[b].ln())).collect();
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64, pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64);
    let slope = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / pts.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
    (kb / nb, kt / nt, slope / (-g / kt_all), kt_all)
}

#[test]
#[ignore]
fn relaxes_from_any_start() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let dt: f64 = std::env::var("DT").map_or(0.0025, |v| v.parse().unwrap());
    for (start, name) in [(1.0, "uniform"), (2.0, "bottom"), (3.0, "top")] {
        let sc = build("atmosphere", &Options::default().with("start", start).with("dt", dt)).unwrap();
        let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
        let mut line = String::new();
        for k in 0..8 {
            e.step((100.0 / dt) as usize);
            let (tb, tt, slope, kt) = profile(&e);
            line += &format!(" | {} τ: kT {kt:.2}, bottom {tb:.2} top {tt:.2}, slope/predicted {slope:.2}", (k + 1) * 100);
        }
        eprintln!("{name}:{line}");
    }
}
