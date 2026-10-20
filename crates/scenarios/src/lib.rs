//! The systems behind the pages, built from a name and numeric options. Pure data (no GPU),
//! so the same scenario runs in the browser, in native tests and on the CPU engines.

use md_core::{fixed, init, Pair, State, System, Wall};
use std::collections::HashMap;

/// Numeric options with defaults.
#[derive(Clone, Debug, Default)]
pub struct Options(pub HashMap<String, f64>);

impl Options {
    pub fn get(&self, key: &str, default: f64) -> f64 {
        self.0.get(key).copied().unwrap_or(default)
    }

    pub fn with(mut self, key: &str, value: f64) -> Self {
        self.0.insert(key.to_string(), value);
        self
    }
}

/// What to do after the warm-up run (on whichever engine runs it), before the page starts.
#[derive(Clone, Debug, PartialEq)]
pub enum Finish {
    None,
    /// Rescale to this temperature, then mark particles within `radius` σ of `center` as
    /// species `species`.
    MarkDisc { temperature: f64, center: [f64; 2], radius: f64, species: u32 },
    /// Rescale to this temperature.
    Temperature(f64),
}

impl Finish {
    pub fn apply(&self, state: &mut State, sys: &System) {
        match *self {
            Finish::None => {}
            Finish::MarkDisc { temperature, center, radius, species } => {
                init::set_temperature(state, sys, temperature);
                init::mark_disc(state, sys, center, radius, species);
            }
            Finish::Temperature(t) => init::set_temperature(state, sys, t),
        }
    }
}

/// A scenario: the system, its initial state, a warm-up and what the page should know.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub sys: System,
    pub state: State,
    /// Steps to run before `finish` and the page's start (e.g. to melt the initial lattice).
    pub warmup: usize,
    pub finish: Finish,
    /// The species the page's histogram tracks (e.g. the cream).
    pub marked: u32,
    /// Histogram grid for the page's observer.
    pub grid: [u32; 2],
    /// Display radius of each species (σ).
    pub radii: Vec<f64>,
}

pub const NAMES: &[&str] = &["cream", "corner", "boltzmann", "heavy", "oilwater", "heatflow"];

pub fn build(name: &str, o: &Options) -> Result<Scenario, String> {
    match name {
        "cream" => Ok(cream(o)),
        "corner" => Ok(corner(o)),
        "boltzmann" => Ok(boltzmann(o)),
        "heavy" => Ok(heavy(o)),
        "oilwater" => Ok(oilwater(o)),
        "heatflow" => Ok(heatflow(o)),
        _ => Err(format!("unknown scenario {name}")),
    }
}

fn grid_for(n: usize, b: [f64; 2], per_bin: f64) -> [u32; 2] {
    let bins = n as f64 / per_bin;
    let gy = ((bins * b[1] / b[0]).sqrt().round() as u32).max(2);
    let gx = ((gy as f64 * b[0] / b[1]).round() as u32).max(2);
    [gx, gy]
}

/// Cream in coffee: a WCA fluid with a disc of marked particles in the middle.
/// Options: n, density, temperature, seed, warmup, blob (radius / shorter side), aspect.
pub fn cream(o: &Options) -> Scenario {
    let n = o.get("n", 8000.0) as usize;
    let t = o.get("temperature", 1.0);
    let sys = System::new(init::box_for(n, o.get("density", 0.5), o.get("aspect", 4.0 / 3.0)), init::DT, Pair::wca(), vec![1.0, 1.0]);
    let state = init::fluid(&sys, n, t, o.get("seed", 1.0) as u64);
    let b = sys.box_f64();
    Scenario {
        finish: Finish::MarkDisc { temperature: t, center: [b[0] / 2.0, b[1] / 2.0], radius: o.get("blob", 0.2) * b[0].min(b[1]), species: 1 },
        warmup: o.get("warmup", 1000.0) as usize,
        marked: 1,
        grid: grid_for(n, b, 64.0),
        radii: vec![0.5, 0.5],
        sys,
        state,
    }
}

/// Many small boxes of gas (compartments), for counting how often all molecules sit in one
/// half. Small molecules (σ = 0.5) in boxes of 8 σ keep the gas nearly ideal.
/// Options: per_box, boxes_x, boxes_y, seed.
pub fn corner(o: &Options) -> Scenario {
    let per_box = o.get("per_box", 6.0) as usize;
    let (bx, by) = (o.get("boxes_x", 32.0) as i64, o.get("boxes_y", 24.0) as i64);
    let side = 8i64 << 32;
    let small = Pair { eps: 1.0, sigma: 0.5, rc: 0.5 * 2f64.powf(1.0 / 6.0) };
    let sys = System::new([bx * side, by * side], init::DT, small, vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, 3)
        .with_compartments(1, 3);
    let state = init::compartment_fluid(&sys, per_box, 1.0, o.get("seed", 1.0) as u64);
    // Warm up for several box-crossing times so counting starts from well-shuffled boxes.
    Scenario { warmup: o.get("warmup", 4000.0) as usize, finish: Finish::None, marked: 0, grid: [2 * bx as u32, 2 * by as u32], radii: vec![0.5], sys, state }
}

/// An isolated WCA fluid, for the energy distribution of one molecule or a group.
/// Options: n, density, temperature, seed.
pub fn boltzmann(o: &Options) -> Scenario {
    let n = o.get("n", 20000.0) as usize;
    let t = o.get("temperature", 1.0);
    let sys = System::new(init::box_for(n, o.get("density", 0.5), o.get("aspect", 4.0 / 3.0)), init::DT, Pair::wca(), vec![1.0, 1.0]);
    let mut state = init::fluid(&sys, n, t, o.get("seed", 1.0) as u64);
    // Colour one molecule so the page can follow it.
    state.species[n / 2] = 1;
    Scenario { warmup: 2000, finish: Finish::Temperature(t), marked: 1, grid: grid_for(n, sys.box_f64(), 64.0), radii: vec![0.5, 0.5], sys, state }
}

/// Light and heavy molecules at the same temperature, plus a few large heavy "bowling balls".
/// Species: 0 light (m = 1), 1 heavy (m = heavy), 2 ball (m = ball_mass, σ = ball_size).
/// Options: n (molecules, besides the balls), density (of molecules in the free area), heavy,
/// balls, ball_size, ball_mass, seed.
pub fn heavy(o: &Options) -> Scenario {
    let n = o.get("n", 6000.0) as usize;
    let heavy = o.get("heavy", 16.0);
    let balls = o.get("balls", 3.0) as usize;
    let size = o.get("ball_size", 6.0);
    let ball_mass = o.get("ball_mass", 400.0);
    let density = o.get("density", 0.4);
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let mix = 0.5 * (1.0 + size);
    let ball_area = balls as f64 * std::f64::consts::PI * mix * mix;
    let area = n as f64 / density + ball_area;
    let aspect = o.get("aspect", 4.0 / 3.0);
    let ly = (area / aspect).sqrt();
    let box_len = [fixed::to_fixed(ly * aspect), fixed::to_fixed(ly)];
    let sys = System::new(box_len, init::DT, Pair::wca(), vec![1.0, heavy, ball_mass])
        .with_pair(0, 2, wca(mix))
        .with_pair(1, 2, wca(mix))
        .with_pair(2, 2, wca(size));
    let seed = o.get("seed", 1.0) as u64;
    let b = sys.box_f64();
    // Balls first, spread out; then molecules on lattice sites clear of every ball.
    let centers: Vec<[f64; 2]> = (0..balls).map(|k| [b[0] * (k as f64 + 0.5) / balls as f64, b[1] * (0.3 + 0.4 * (k % 2) as f64)]).collect();
    let clear = |p: [i64; 2]| {
        centers.iter().all(|c| {
            let dx = fixed::to_f64(fixed::min_image(fixed::to_fixed(c[0]), p[0], sys.box_len[0]));
            let dy = fixed::to_f64(fixed::min_image(fixed::to_fixed(c[1]), p[1], sys.box_len[1]));
            dx * dx + dy * dy > (mix * 1.1).powi(2)
        })
    };
    let mut sites = n;
    let pos = loop {
        let free: Vec<[i64; 2]> = init::lattice(sites, sys.box_len, 0.1, seed).into_iter().filter(|&p| clear(p)).collect();
        if free.len() >= n {
            break free;
        }
        sites += sites / 20 + 1;
    };
    let mut state = State { pos: pos[..n].to_vec(), vel: vec![], species: (0..n).map(|i| (i % 2) as u32).collect() };
    for c in &centers {
        state.pos.push([fixed::to_fixed(c[0]), fixed::to_fixed(c[1])]);
        state.species.push(2);
    }
    state.vel = init::maxwell(&sys, &state.species, 1.0, seed);
    let total = state.n();
    Scenario { warmup: 3000, finish: Finish::Temperature(1.0), marked: 2, grid: grid_for(total, b, 64.0), radii: vec![0.5, 0.5, 0.5 * size], sys, state }
}

/// Two kinds of LJ molecules ("water" 0 and "oil" 1) that attract each other more weakly than
/// themselves, held at a temperature by a heat bath acting throughout (Andersen). Options: n,
/// density, temperature, cross (oil–water attraction ε_AB), seed.
pub fn oilwater(o: &Options) -> Scenario {
    let n = o.get("n", 12000.0) as usize;
    let t = o.get("temperature", 1.0);
    let lj = |eps: f64| Pair { eps, sigma: 1.0, rc: 2.5 };
    let sys = System::new(init::box_for(n, o.get("density", 0.7), o.get("aspect", 4.0 / 3.0)), init::DT, lj(1.0), vec![1.0, 1.0])
        .with_pair(0, 1, lj(o.get("cross", 0.5)))
        .with_thermostat(t, 0.5);
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::fluid(&sys, n, t, seed);
    for i in 0..n {
        if gpu_prims::threefry::uniform(seed, 9, i as u32) < 0.5 {
            state.species[i] = 1;
        }
    }
    // Small bins (about 16 molecules) so that early domains register.
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: grid_for(n, sys.box_f64(), 16.0), radii: vec![0.5, 0.5], sys, state }
}

/// Columns of small boxes between a hot wall (bottom) and a cold wall (top), for the
/// statistics of the heat each column passes in a short time. Each column is 2^log2_width σ
/// wide and twice that tall. Options: columns, per_box, log2_width, hot, cold, seed.
pub fn heatflow(o: &Options) -> Scenario {
    let cols = o.get("columns", 128.0) as i64;
    let per_box = o.get("per_box", 12.0) as usize;
    let log2w = o.get("log2_width", 3.0) as u32;
    let side = 1i64 << (32 + log2w);
    let (th, tc) = (o.get("hot", 1.5), o.get("cold", 1.0));
    let sys = System::new([cols * side, 2 * side], init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Thermal(th), Wall::Thermal(tc))
        .with_compartments(0, log2w);
    let state = init::compartment_fluid(&sys, per_box, 0.5 * (th + tc), o.get("seed", 1.0) as u64);
    Scenario { warmup: o.get("warmup", 4000.0) as usize, finish: Finish::None, marked: 0, grid: [cols as u32, 2], radii: vec![0.5], sys, state }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_builds_and_runs_briefly_on_the_cpu() {
        for name in NAMES {
            let mut o = Options::default();
            // Small versions, so the CPU engine finishes quickly.
            for (k, v) in [("n", 1500.0), ("boxes_x", 8.0), ("boxes_y", 6.0), ("columns", 16.0)] {
                o = o.with(k, v);
            }
            let sc = build(name, &o).unwrap();
            let mut e = md_ref::IntEngine::<f32>::new(sc.sys.clone(), sc.state.clone());
            e.step(200);
            let mut s = e.state.clone();
            sc.finish.apply(&mut s, &sc.sys);
            let en = md_ref::IntEngine::<f64>::new(sc.sys.clone(), s.clone()).energy();
            assert!(en.total().is_finite(), "{name}: energy not finite");
            let t = s.half_step_kinetic(&sc.sys) / s.n() as f64;
            assert!(t > 0.2 && t < 5.0, "{name}: temperature {t} after a short run");
            // No blow-ups from overlapping initial positions.
            let vmax = s.vel.iter().map(|v| sc.sys.velocity(v[0]).abs().max(sc.sys.velocity(v[1]).abs())).fold(0.0, f64::max);
            assert!(vmax < 20.0, "{name}: a particle reached {vmax} σ/τ");
        }
    }
}
