//! Humid air over water (the `humid` scenario): the vapour sits at saturation, and cooled, it
//! condenses until it is saturated at the new temperature.

use gpu_prims::{Gpu, Prims};
use md::GpuEngine;
use md_core::fixed;
use pollster::block_on;
use scenarios::{saturated_vapour_density, Options};

/// Vapour density: water molecules (species 0) more than 6 σ above the pool's surface (~17 σ)
/// with no neighbour within 1.5 σ... counted simply as free molecules in the air space.
fn vapour(e: &GpuEngine) -> f64 {
    let s = block_on(e.read_state());
    let b = e.sys.box_f64();
    let (y0, y1) = (24.0, b[1] - 4.0);
    let water: Vec<[f64; 2]> = (0..s.n()).filter(|&i| s.species[i] == 0).map(|i| [fixed::to_f64(s.pos[i][0]), fixed::to_f64(s.pos[i][1])]).collect();
    let free = water
        .iter()
        .filter(|p| p[1] > y0 && p[1] < y1)
        .filter(|p| {
            water.iter().filter(|q| {
                let mut dx = (q[0] - p[0]).abs();
                dx = dx.min(b[0] - dx);
                dx * dx + (q[1] - p[1]).powi(2) < 2.25
            }).count() <= 2
        })
        .count();
    free as f64 / (b[0] * (y1 - y0))
}

#[test]
#[ignore = "an experiment: does saturated vapour stay saturated, and condense when cooled?"]
fn cooled_vapour_condenses() {
    let p = Prims::new(&block_on(Gpu::new()).unwrap());
    let sc = scenarios::build("humid", &Options::default().with("temperature", 0.44)).unwrap();
    let mut e = GpuEngine::new(&p, sc.sys.clone(), &sc.state);
    let mut line = vec![];
    for _ in 0..4 {
        e.step(20000);
        line.push(format!("{:.4}", vapour(&e)));
    }
    eprintln!("kT 0.44 (saturated {:.4}): {}", saturated_vapour_density(0.44), line.join(" "));
    let sys = e.sys.clone().with_thermostat(0.40, 0.1);
    e.update_system(sys);
    let mut line = vec![];
    for _ in 0..10 {
        e.step(20000);
        line.push(format!("{:.4}", vapour(&e)));
    }
    eprintln!("cooled to 0.40 (saturated {:.4}): {}", saturated_vapour_density(0.40), line.join(" "));
}
