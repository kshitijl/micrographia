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

pub const NAMES: &[&str] = &["cream", "corner", "boltzmann", "heavy", "oilwater", "heatflow", "osmosis", "compress", "rubber", "atmosphere"];

pub fn build(name: &str, o: &Options) -> Result<Scenario, String> {
    match name {
        "cream" => Ok(cream(o)),
        "corner" => Ok(corner(o)),
        "boltzmann" => Ok(boltzmann(o)),
        "heavy" => Ok(heavy(o)),
        "oilwater" => Ok(oilwater(o)),
        "heatflow" => Ok(heatflow(o)),
        "osmosis" => Ok(osmosis(o)),
        "compress" => Ok(compress(o)),
        "rubber" => Ok(rubber(o)),
        "atmosphere" => Ok(atmosphere(o)),
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

/// Solvent (0) everywhere and solute (1) on the right, separated by a membrane in the middle
/// that only solvent crosses; soft walls at both ends, a bulk heat bath. Options: n, density,
/// solute (fraction of right-half molecules), temperature, attraction (solute–solvent LJ ε; 0
/// for purely repulsive), seed.
pub fn osmosis(o: &Options) -> Scenario {
    let n = o.get("n", 16000.0) as usize;
    let t = o.get("temperature", 1.0);
    let attraction = o.get("attraction", 0.0);
    let mut sys = System::new(init::box_for(n, o.get("density", 0.5), o.get("aspect", 2.0)), init::DT, Pair::wca(), vec![1.0, 1.0])
        .with_walls(0, Wall::Soft, Wall::Soft)
        .with_thermostat(t, 0.5);
    if attraction > 0.0 {
        sys = sys.with_pair(0, 1, Pair { eps: attraction, sigma: 1.0, rc: 2.5 });
    }
    let b = sys.box_f64();
    let sys = sys.with_membrane(0, b[0] / 2.0, &[1]);
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::fluid(&sys, n, t, seed);
    let f = o.get("solute", 0.08);
    for i in 0..n {
        if fixed::to_f64(state.pos[i][0]) > b[0] / 2.0 + 1.0 && gpu_prims::threefry::uniform(seed, 3, i as u32) < f {
            state.species[i] = 1;
        }
    }
    Scenario { warmup: o.get("warmup", 4000.0) as usize, finish: Finish::None, marked: 1, grid: [32, 1], radii: vec![0.5, 0.5], sys, state }
}

/// A dilute gas of small molecules (σ = 0.5, nearly ideal) in a cylinder closed by a piston
/// that starts halfway up. The box is 4:3; the piston runs between 15% and 100% of its height.
/// Walls are insulating (soft) unless `thermal` > 0, which makes the side and bottom walls a
/// heat bath at that temperature. Options: n, density (initial), thermal, seed.
pub fn compress(o: &Options) -> Scenario {
    let n = o.get("n", 3000.0) as usize;
    let area0 = n as f64 / o.get("density", 0.05);
    let width = (8.0 * area0 / 3.0).sqrt();
    let height = 0.75 * width;
    let small = Pair { eps: 1.0, sigma: 0.5, rc: 0.5 * 2f64.powf(1.0 / 6.0) };
    let thermal = o.get("thermal", 0.0);
    let side = if thermal > 0.0 { Wall::Thermal(thermal) } else { Wall::Soft };
    let sys = System::new([fixed::to_fixed(width), fixed::to_fixed(height)], init::DT, small, vec![1.0])
        .with_walls(0, side, side)
        .with_walls(1, side, Wall::Hard)
        .with_piston(md_core::PistonDrive::Speed(0.0), height / 2.0, 0.15 * height);
    let mut state = init::fluid(&sys, n, 1.0, o.get("seed", 1.0) as u64);
    for p in state.pos.iter_mut() {
        p[1] /= 2;
    }
    Scenario { warmup: 0, finish: Finish::None, marked: 0, grid: [8, 8], radii: vec![0.6], sys, state }
}

/// Rubber as an entropic spring: one bead–spring (Kremer–Grest) chain per compartment, its two
/// end beads pinned at a set separation, in an implicit solvent (a bulk heat bath). Species: 0
/// bead, 1 left anchor (pinned), 2 the bead bonded to the right anchor (an ordinary bead, so the
/// page can measure the bond's tension), 3 right anchor (pinned, can be moved). Options:
/// chains, length (beads), stretch (anchor separation / contour length), temperature, seed.
pub fn rubber(o: &Options) -> Scenario {
    let chains = o.get("chains", 64.0) as usize;
    let length = o.get("length", 40.0) as usize;
    let stretch = o.get("stretch", 0.5);
    let t = o.get("temperature", 1.0);
    let bond = 0.97;
    let contour = (length - 1) as f64 * bond;
    let width = ((contour * 1.1 + 6.0) / 8.0).ceil() * 8.0;
    let row = 16i64 << 32;
    let sys = System::new([fixed::to_fixed(width), chains as i64 * row], init::DT, Pair::wca(), vec![1.0, 1.0, 1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(1, 4)
        .with_pinned(1)
        .with_pinned(3)
        .with_thermostat(t, 1.0);
    let sep = stretch * contour;
    let x0 = 0.5 * (width - sep);
    // Lay each chain out as a serpentine: m vertical strands at least 1.1 σ apart, joined by
    // short horizontal steps, whose total length is the contour; beads then sit one bond length
    // apart along it, and strands never come closer than a bead diameter.
    let m = ((sep / 1.1).floor() as usize).max(1);
    let d = sep / m as f64;
    let h = ((contour - sep) / m as f64).max(0.0);
    assert!(h <= 12.0, "stretch {stretch} too small for a 16 σ row");
    // Half a strand up from the left anchor, full strands alternating, half a strand back down
    // to the right anchor: total m·h vertical plus sep horizontal.
    let mut y = h / 2.0;
    let mut corners = vec![(0.0f64, 0.0f64), (0.0, y)];
    for k in 1..=m {
        let x = k as f64 * d;
        corners.push((x, y));
        if k < m {
            y = -y;
            corners.push((x, y));
        }
    }
    corners.push((sep, 0.0));
    // Walk along the polyline, placing each bead one bond length (straight-line distance) after
    // the previous one, so beads around corners are not squeezed. The last bead (the right
    // anchor) lands near (sep, 0).
    let mut dense = Vec::new();
    for w in corners.windows(2) {
        let l = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
        let k = (l / 0.002).ceil().max(1.0) as usize;
        for i in 0..k {
            let f = i as f64 / k as f64;
            dense.push((w[0].0 + f * (w[1].0 - w[0].0), w[0].1 + f * (w[1].1 - w[0].1)));
        }
    }
    dense.push(*corners.last().unwrap());
    let mut curve = vec![dense[0]];
    for &q in &dense {
        let p = *curve.last().unwrap();
        if ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt() >= bond && curve.len() < length {
            curve.push(q);
        }
    }
    while curve.len() < length {
        let p = *curve.last().unwrap();
        curve.push((p.0 + bond, p.1));
    }
    let mut pos = Vec::new();
    let mut species = Vec::new();
    let mut bonds = Vec::new();
    for c in 0..chains {
        let y = (c as f64 + 0.5) * 16.0;
        for (k, &(cx, cy)) in curve.iter().enumerate() {
            let id = pos.len() as u32;
            pos.push([fixed::to_fixed(x0 + cx), fixed::to_fixed(y + cy)]);
            species.push(match k {
                0 => 1,
                k if k == length - 1 => 3,
                k if k == length - 2 => 2,
                _ => 0,
            });
            if k > 0 {
                bonds.push([id - 1, id, 0]);
            }
        }
    }
    let sys = sys.with_bonds(vec![md_core::BondType::kremer_grest()], bonds);
    let mut vel = init::maxwell(&sys, &species, t, o.get("seed", 1.0) as u64);
    for (v, s) in vel.iter_mut().zip(&species) {
        if sys.is_pinned(*s) {
            *v = [0, 0];
        }
    }
    let state = State { pos, vel, species };
    Scenario { warmup: o.get("warmup", 4000.0) as usize, finish: Finish::None, marked: 0, grid: [8, 8], radii: vec![0.5, 0.7, 0.5, 0.7], sys, state }
}

/// A column of gas under gravity: light (species 0, m = 1) and heavy (species 1, m = heavy)
/// small molecules (σ = 0.4, nearly ideal), soft floor and ceiling, periodic sideways, and
/// either a bulk heat bath at `temperature` or none (isolated). `g` is set so that the light
/// gas's scale height kT/(m g) is `scale` (a fraction of the column height). Options: n,
/// heavy (mass), heavy_share, scale, temperature, bath (1 or 0), seed.
pub fn atmosphere(o: &Options) -> Scenario {
    let n = o.get("n", 6000.0) as usize;
    let t = o.get("temperature", 1.0);
    let small = Pair { eps: 1.0, sigma: 0.4, rc: 0.4 * 2f64.powf(1.0 / 6.0) };
    let aspect = o.get("aspect", 4.0 / 3.0);
    // Dilute: density 0.05 on average.
    let bl = init::box_for(n, 0.05, aspect);
    let h = fixed::to_f64(bl[1]);
    let g = t / (o.get("scale", 0.35) * h);
    let mut sys = System::new(bl, init::DT, small, vec![1.0, o.get("heavy", 4.0)]).with_walls(1, Wall::Soft, Wall::Soft).with_gravity([0.0, -g]);
    if o.get("bath", 1.0) > 0.0 {
        sys = sys.with_thermostat(t, 0.2);
    }
    let seed = o.get("seed", 1.0) as u64;
    let share = o.get("heavy_share", 0.3);
    let species: Vec<u32> = (0..n).map(|i| (gpu_prims::threefry::uniform(seed, 4, i as u32) < share) as u32).collect();
    // Start in equilibrium: falling into shape would take hundreds of τ.
    // The soft floor and ceiling act within 1.12 σ of a molecule's centre, whatever its size.
    let state = init::barometric(&sys, &species, t, 0.5, 1.1, seed);
    Scenario { warmup: o.get("warmup", 2000.0) as usize, finish: Finish::None, marked: 1, grid: [1, 40], radii: vec![0.5, 0.7], sys, state }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rubber_starts_without_overlaps_at_any_stretch() {
        for stretch in [0.15, 0.3, 0.5, 0.8, 0.95] {
            let sc = rubber(&Options::default().with("chains", 2.0).with("stretch", stretch));
            let e = md_ref::IntEngine::<f64>::new(sc.sys.clone(), sc.state.clone());
            let (kicks, pe) = e.kicks();
            let worst = kicks.iter().map(|k| k[0].abs().max(k[1].abs())).max().unwrap();
            // A kick of 2^24 units per step corresponds to a force of ~160 ε/σ.
            assert!(worst < 1 << 24, "stretch {stretch}: kick {worst}, energy {pe}");
        }
    }

    #[test]
    fn every_scenario_builds_and_runs_briefly_on_the_cpu() {
        for name in NAMES {
            let mut o = Options::default();
            // Small versions, so the CPU engine finishes quickly.
            for (k, v) in [("n", 1500.0), ("boxes_x", 8.0), ("boxes_y", 6.0), ("columns", 16.0), ("chains", 8.0)] {
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
