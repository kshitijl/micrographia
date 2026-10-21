//! Evaporative cooling (the `sweat` scenario): an insulated pool under a warm ceiling cools below
//! the air's temperature while it evaporates into dry air (vapour carried off near the ceiling,
//! as a wind would), and hardly at all into air that is nearly saturated.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::Options;

/// (pool kT, air kT) after `blocks` × 1000 steps, keeping the air at `humidity` by turning water
/// molecules above `renew` into air (dry) or air into water (humid) to hold the vapour there.
fn run(p: &Prims, humidity: f64, blocks: usize) -> Vec<(f64, f64, usize)> {
    let sc = scenarios::build("sweat", &Options::default().with("humidity", humidity)).unwrap();
    let mut e = GpuEngine::new(p, sc.sys.clone(), &sc.state);
    let (w, h) = (120.0, 100.0);
    let sat = scenarios::saturated_vapour_density(0.44);
    let renew = h - 20.0;
    let mut out = vec![];
    for b in 0..blocks {
        e.step(1000);
        let s = block_on(e.read_state());
        let y = |i: usize| fixed::to_f64(s.pos[i][1]);
        let sys = e.sys.clone();
        let ke = |i: usize| 0.5 * (sys.velocity(s.vel[i][0]).powi(2) + sys.velocity(s.vel[i][1]).powi(2));
        // Renew the air near the ceiling to the set humidity.
        let water: Vec<u32> = (0..s.n()).filter(|&i| s.species[i] == 0 && y(i) > renew).map(|i| i as u32).collect();
        let air: Vec<u32> = (0..s.n()).filter(|&i| s.species[i] == 1 && y(i) > renew).map(|i| i as u32).collect();
        let want = (humidity * sat * w * (h - renew)).round() as usize;
        if water.len() > want {
            e.set_species(&water[..water.len() - want], 1);
        } else if water.len() < want && air.len() > want - water.len() {
            e.set_species(&air[..want - water.len()], 0);
        }
        let pool: Vec<usize> = (0..s.n()).filter(|&i| s.species[i] == 0 && y(i) < 13.0).collect();
        let high: Vec<usize> = (0..s.n()).filter(|&i| s.species[i] == 1 && y(i) > 30.0).collect();
        let tp = pool.iter().map(|&i| ke(i)).sum::<f64>() / pool.len() as f64;
        let ta = high.iter().map(|&i| ke(i)).sum::<f64>() / high.len() as f64;
        if b % 10 == 9 {
            out.push((tp, ta, pool.len()));
        }
    }
    out
}

#[test]
#[ignore = "an experiment: evaporative cooling, dry and humid"]
fn evaporation_cools_the_pool() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    for hum in [0.0, 0.5, 1.0] {
        let r = run(&p, hum, 200);
        eprintln!("sweat: humidity {hum}: {}", r.iter().map(|(tp, ta, n)| format!("{tp:.3}/{ta:.3}/{n}")).collect::<Vec<_>>().join(" "));
    }
}
