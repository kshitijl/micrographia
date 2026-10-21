//! The systems behind the pages, built from a name and numeric options. Pure data (no GPU),
//! so the same scenario runs in the browser, in native tests and on the CPU engines.

use md_core::{fixed, init, Pair, State, System, Wall};
use std::collections::HashMap;

pub mod three;

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
    /// Rescale to `temperature`, then turn particles of species 0 beyond `x_min` (σ) into
    /// species 1, each with probability `fraction` (by id and `seed`), keeping their positions.
    Solute { temperature: f64, x_min: f64, fraction: f64, seed: u64 },
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
            Finish::Solute { temperature, x_min, fraction, seed } => {
                // Rescale the moving particles only (pinned ones must keep zero velocity, and
                // walls and gravity mean there is no momentum to keep at zero).
                let moving: Vec<usize> = (0..state.n()).filter(|&i| sys.pinned & (1 << state.species[i]) == 0).collect();
                let ke: f64 = moving
                    .iter()
                    .map(|&i| 0.5 * sys.masses[state.species[i] as usize] * (sys.velocity(state.vel[i][0]).powi(2) + sys.velocity(state.vel[i][1]).powi(2)))
                    .sum();
                let f = (temperature / (ke / moving.len() as f64)).sqrt();
                for &i in &moving {
                    state.vel[i] = state.vel[i].map(|v| (v as f64 * f).round() as i32);
                }
                for i in 0..state.n() {
                    if state.species[i] == 0 && fixed::to_f64(state.pos[i][0]) > x_min && gpu_prims::threefry::uniform(seed, 6, i as u32) < fraction {
                        state.species[i] = 1;
                    }
                }
            }
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

pub const NAMES: &[&str] = &["cream", "corner", "boltzmann", "heavy", "oilwater", "heatflow", "osmosis", "compress", "rubber", "atmosphere", "carnot", "boiling", "lipids", "balloon", "smell", "binding", "fep", "cell", "buffer", "mask", "dna", "demon", "ensemble", "osmosis_free", "osmometer", "flask", "emulsion", "enthalpy", "humid"];

pub fn build(name: &str, o: &Options) -> Result<Scenario, String> {
    let sc = build_unchecked(name, o)?;
    // Positions live in the half-open box [0, L) on each axis; one placed on the far edge sits
    // outside it (a wall bead there was once bounced by the wall at 100 σ/τ).
    for (i, p) in sc.state.pos.iter().enumerate() {
        for a in 0..2 {
            if p[a] < 0 || p[a] >= sc.sys.box_len[a] {
                return Err(format!("scenario {name}: particle {i} starts outside the box on axis {a} ({})", fixed::to_f64(p[a])));
            }
        }
    }
    Ok(sc)
}

fn build_unchecked(name: &str, o: &Options) -> Result<Scenario, String> {
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
        "carnot" => Ok(carnot(o)),
        "boiling" => Ok(boiling(o)),
        "lipids" => Ok(lipids(o)),
        "balloon" => Ok(balloon(o)),
        "smell" => Ok(smell(o)),
        "binding" => Ok(binding(o)),
        "fep" => Ok(fep(o)),
        "cell" => Ok(cell(o)),
        "buffer" => Ok(buffer(o)),
        "mask" => Ok(mask(o)),
        "dna" => Ok(dna(o)),
        "demon" => Ok(demon(o)),
        "ensemble" => Ok(ensemble(o)),
        "osmosis_free" => Ok(osmosis_free(o)),
        "osmometer" => Ok(osmometer(o)),
        "flask" => Ok(flask(o)),
        "emulsion" => Ok(emulsion(o)),
        "enthalpy" => Ok(enthalpy(o)),
        "humid" => Ok(humid(o)),
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
/// half. Tiny molecules (size 0.25 σ, the default) in boxes of 8 σ keep the gas nearly ideal;
/// larger ones show the excluded-area correction to 2^-N. Options: per_box, boxes_x, boxes_y,
/// size, seed.
pub fn corner(o: &Options) -> Scenario {
    let per_box = o.get("per_box", 6.0) as usize;
    let (bx, by) = (o.get("boxes_x", 32.0) as i64, o.get("boxes_y", 24.0) as i64);
    let side = 8i64 << 32;
    let size = o.get("size", 0.25);
    let small = Pair { eps: 1.0, sigma: size, rc: size * 2f64.powf(1.0 / 6.0) };
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
/// themselves, held at a temperature by a heat bath acting throughout (Andersen), between a soft
/// floor and ceiling. Options: n, density, temperature, cross (oil–water attraction ε_AB),
/// oil_mass, gravity, bath_rate (a gentle bath keeps the flow that lets oil rise), seed.
pub fn oilwater(o: &Options) -> Scenario {
    let n = o.get("n", 12000.0) as usize;
    let t = o.get("temperature", 1.0);
    let lj = |eps: f64| Pair { eps, sigma: 1.0, rc: 2.5 };
    // A floor and a ceiling (soft walls), so that gravity can be turned on; oil (species 1) is a
    // little lighter than water.
    let sys = System::new(init::box_for(n, o.get("density", 0.7), o.get("aspect", 4.0 / 3.0)), init::DT, lj(1.0), vec![1.0, o.get("oil_mass", 0.8)])
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_pair(0, 1, lj(o.get("cross", 0.5)))
        .with_gravity([0.0, -o.get("gravity", 0.0)])
        .with_thermostat(t, o.get("bath_rate", 0.5));
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
    // Insulating walls are hard planes, the same geometry as thermal ones (the page switches).
    let side = if thermal > 0.0 { Wall::Thermal(thermal) } else { Wall::Hard };
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
/// page can measure the bond's tension), 3 right anchor (pinned, can be moved; or, with a gauge,
/// a free handle on a spring to a post, appended after each chain). With a gauge the species are
/// 0 beads, 1 the fixed end, 2 the end a page moves (pinned), 3 the handles; the handle is at the
/// right end, or the left with `gauge_side` −1. Options: chains, length (beads), stretch (end
/// separation / contour length), temperature, gauge (spring stiffness, 0 for pinned ends),
/// gauge_side, handle_mass, bath_rate, harmonic (1: stiff harmonic bonds), seed.
pub fn rubber(o: &Options) -> Scenario {
    let chains = o.get("chains", 64.0) as usize;
    let length = o.get("length", 40.0) as usize;
    let stretch = o.get("stretch", 0.5);
    let t = o.get("temperature", 1.0);
    let bond = 0.97;
    let contour = (length - 1) as f64 * bond;
    let width = ((contour * 1.1 + 6.0) / 8.0).ceil() * 8.0;
    let row = 16i64 << 32;
    // With a gauge, each chain's right end is a free, heavy handle tied by a zero-length spring of
    // stiffness `gauge` to a fixed post (species 1, like the left anchors) at its starting
    // position: the chains' pull stretches the springs, and you can see it. Without one the
    // right ends are pinned too.
    let gauge = o.get("gauge", 0.0);
    // With a gauge on the left (`gauge_side` −1) the handle and post are at the left end instead.
    let left = o.get("gauge_side", 1.0) < 0.0;
    let mut sys = System::new([fixed::to_fixed(width), chains as i64 * row], init::DT, Pair::wca(), vec![1.0, 1.0, 1.0, if gauge > 0.0 { o.get("handle_mass", 20.0) } else { 1.0 }])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(1, 4)
        .with_pinned(1)
        .with_thermostat(t, o.get("bath_rate", 1.0));
    if gauge > 0.0 {
        // Species 1: the fixed end (left anchors, or left posts); 2: the end the page moves (right
        // posts, or right anchors), pinned; 3: the handles, free, each on a spring to its post.
        sys = sys.with_pinned(2).with_pair(1, 3, Pair::none()).with_pair(2, 3, Pair::none());
    } else {
        sys = sys.with_pinned(3);
    }
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
            species.push(match (gauge > 0.0, left, k) {
                (false, _, 0) => 1,
                (false, _, k) if k == length - 1 => 3,
                (false, _, k) if k == length - 2 => 2,
                (true, false, 0) => 1,
                (true, false, k) if k == length - 1 => 3,
                (true, true, 0) => 3,
                (true, true, k) if k == length - 1 => 2,
                _ => 0,
            });
            if k > 0 {
                bonds.push([id - 1, id, 0]);
            }
        }
        if gauge > 0.0 {
            // The post, on top of its handle, joined by a zero-length spring.
            let first = pos.len() - length;
            let handle = if left { first } else { pos.len() - 1 } as u32;
            pos.push(pos[handle as usize]);
            species.push(if left { 1 } else { 2 });
            bonds.push([handle, pos.len() as u32 - 1, 1]);
        }
    }
    // Kremer–Grest (FENE) bonds by default; `harmonic` 1 uses stiff harmonic springs instead,
    // which cannot blow up when a chain is pulled faster than it can follow (FENE's force
    // diverges at 1.5 σ).
    let chain_bond = if o.get("harmonic", 0.0) > 0.0 { md_core::BondType::Harmonic { k: 100.0, r0: 1.0 } } else { md_core::BondType::kremer_grest() };
    let sys = sys.with_bonds(vec![chain_bond, md_core::BondType::Harmonic { k: gauge.max(1e-9), r0: 0.0 }], bonds);
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
/// either a bulk heat bath at `temperature` or none (isolated, the default). `g` is set so that
/// the light gas's scale height kT/(m g) is `scale` (a fraction of the column height) times
/// 1/`gravity`. Options: n, heavy (mass), heavy_share, scale, gravity (relative), temperature,
/// bath (1 or 0), start (see [`AtmosphereStart`]), dt, seed.
pub fn atmosphere(o: &Options) -> Scenario {
    let n = o.get("n", 6000.0) as usize;
    let t = o.get("temperature", 1.0);
    let small = Pair { eps: 1.0, sigma: 0.4, rc: 0.4 * 2f64.powf(1.0 / 6.0) };
    let aspect = o.get("aspect", 4.0 / 3.0);
    // Dilute: density 0.05 on average.
    let bl = init::box_for(n, 0.05, aspect);
    let h = fixed::to_f64(bl[1]);
    // Gravity in units of the default, which gives light molecules a scale height of 0.35 of the box.
    let g = o.get("gravity", 1.0) * t / (o.get("scale", 0.35) * h);
    let mut sys = System::new(bl, o.get("dt", init::DT), small, vec![1.0, o.get("heavy", 4.0)]).with_walls(1, Wall::Soft, Wall::Soft).with_gravity([0.0, -g]);
    if o.get("bath", 0.0) > 0.0 {
        sys = sys.with_thermostat(t, 0.2);
    }
    let seed = o.get("seed", 1.0) as u64;
    let share = o.get("heavy_share", 0.3);
    let species: Vec<u32> = (0..n).map(|i| (gpu_prims::threefry::uniform(seed, 4, i as u32) < share) as u32).collect();
    // The soft floor and ceiling act within 1.12 σ of a molecule's centre, whatever its size.
    let start = AtmosphereStart::from_code(o.get("start", 1.0)).unwrap_or(AtmosphereStart::Uniform);
    let sample = |g: f64| init::barometric(&sys.clone().with_gravity([0.0, -g]), &species, t, 0.5, 1.1, seed);
    let state = match start {
        AtmosphereStart::Equilibrium => sample(g),
        AtmosphereStart::Uniform => sample(0.0),
        // Squeezed into the bottom (or, mirrored, the top) tenth or so.
        AtmosphereStart::Bottom => sample(10.0 * t / h),
        AtmosphereStart::Top => {
            let mut s = sample(10.0 * t / h);
            for p in s.pos.iter_mut() {
                p[1] = bl[1] - p[1];
            }
            s
        }
    };
    let warmup = if start == AtmosphereStart::Equilibrium { 2000.0 } else { 0.0 };
    Scenario { warmup: o.get("warmup", warmup) as usize, finish: Finish::None, marked: 1, grid: [1, 40], radii: vec![0.5, 0.7], sys, state }
}

/// How the `atmosphere` gas starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AtmosphereStart {
    /// Already in the barometric distribution.
    Equilibrium,
    /// Spread evenly from floor to ceiling.
    Uniform,
    /// Squeezed against the floor.
    Bottom,
    /// Squeezed against the ceiling.
    Top,
}

impl AtmosphereStart {
    /// From the page's option code (0–3).
    pub fn from_code(c: f64) -> Result<Self, String> {
        Ok(match c as i64 {
            0 => Self::Equilibrium,
            1 => Self::Uniform,
            2 => Self::Bottom,
            3 => Self::Top,
            _ => return Err(format!("unknown atmosphere start {c}")),
        })
    }
}
/// A heat-engine cylinder: small molecules (σ = 0.5, nearly ideal) under a fixed-speed piston,
/// hard (insulating) walls, and a bulk heat bath that the page switches between hot, cold and
/// off: thermal contact through the walls alone is too slow for cycles of practical length.
/// Options: n, width (σ), start (piston height, σ), top (box height, σ), hot, contact (bath
/// rate, 1/τ), seed.
pub fn carnot(o: &Options) -> Scenario {
    let n = o.get("n", 800.0) as usize;
    let (width, start, top) = (o.get("width", 64.0), o.get("start", 48.0), o.get("top", 160.0));
    let th = o.get("hot", 1.5);
    let small = Pair { eps: 1.0, sigma: 0.5, rc: 0.5 * 2f64.powf(1.0 / 6.0) };
    let sys = System::new([fixed::to_fixed(width), fixed::to_fixed(top)], init::DT, small, vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_piston(md_core::PistonDrive::Speed(0.0), start, 4.0)
        .with_thermostat(th, o.get("contact", 0.5));
    // Molecules start below the piston.
    let lower = System::new([fixed::to_fixed(width), fixed::to_fixed(start)], init::DT, small, vec![1.0]).with_walls(0, Wall::Soft, Wall::Soft).with_walls(1, Wall::Soft, Wall::Soft);
    let mut state = init::fluid(&lower, n, th, o.get("seed", 1.0) as u64);
    state.vel = init::maxwell(&sys, &state.species, th, o.get("seed", 1.0) as u64);
    Scenario { warmup: o.get("warmup", 4000.0) as usize, finish: Finish::None, marked: 0, grid: [4, 4], radii: vec![0.6], sys, state }
}

/// A pool of Lennard-Jones liquid (rc 2.5) with its vapour above, held at a temperature by a
/// bulk bath, or heated from below by a hot plate (`plate`, a heat-bath floor) or a hot layer
/// (`layer`, a bath in the bottom 3 σ), with an insulating lid; weak gravity keeps the liquid at the bottom (the vapour's scale height is
/// far taller than the box). The vapour starts near saturation (`prefill`, default 1). Options: n,
/// width, height (σ), temperature, plate, layer, prefill, seed.
pub fn boiling(o: &Options) -> Scenario {
    let n = o.get("n", 6000.0) as usize;
    let (w, h) = (o.get("width", 200.0), o.get("height", 160.0));
    let t = o.get("temperature", 0.42);
    // Heating, one of: `plate` > 0, a hot plate (the floor is a heat-bath wall at that
    // temperature); `layer` > 0, a hot layer (a heat bath acting only in the bottom 3 σ, the
    // liquid's first few layers); otherwise a bulk bath at `temperature`. Both plates sit under an
    // insulating lid. (On the plate the liquid, which does not wet a purely repulsive wall, can
    // hold a thin vapour gap above it; the layer heats the liquid directly.)
    let (plate, layer) = (o.get("plate", 0.0), o.get("layer", 0.0));
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::lj(2.5), vec![1.0]).with_gravity([0.0, -0.002]);
    let sys = if plate > 0.0 {
        sys.with_walls(1, Wall::Thermal(plate), Wall::Soft)
    } else if layer > 0.0 {
        sys.with_walls(1, Wall::Soft, Wall::Soft).with_thermostat(layer, 1.0).with_bath_layer(0.0, 3.0)
    } else {
        sys.with_walls(1, Wall::Soft, Wall::Soft).with_thermostat(t, 0.5)
    };
    // The liquid starts as a slab at density 0.75 on the floor.
    let depth = n as f64 / (0.75 * w);
    let pool = System::new([fixed::to_fixed(w), fixed::to_fixed(depth + 2.0)], init::DT, Pair::lj(2.5), vec![1.0]).with_walls(1, Wall::Soft, Wall::Soft);
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::fluid(&pool, n, t, seed);
    // The vapour starts near saturation (option `prefill`, default on): evaporation in 2D is so
    // slow that a vapour space started empty is still filling after 1,000 τ (research §18.8).
    // Saturated vapour density from the Gibbs ensemble and MD (crates/mc/tests/gibbs.rs), as a
    // Clausius–Clapeyron fit: 0.023 at kT 0.40, 0.058 at 0.44.
    if o.get("prefill", 1.0) > 0.0 {
        let rho_v = 0.023 * (-4.07 * (1.0 / t.clamp(0.3, 0.47) - 1.0 / 0.40)).exp();
        let (y0, y1) = (depth + 3.0, h - 2.0);
        let count = (rho_v * w * (y1 - y0)).round() as usize;
        let mut placed: Vec<[f64; 2]> = vec![];
        let mut k = 0u32;
        while placed.len() < count && k < 100 * count as u32 + 100 {
            let x = gpu_prims::threefry::uniform(seed, 31, 2 * k) * w;
            let y = y0 + gpu_prims::threefry::uniform(seed, 31, 2 * k + 1) * (y1 - y0);
            k += 1;
            if placed.iter().any(|q| (q[0] - x).powi(2) + (q[1] - y).powi(2) < 1.44) {
                continue;
            }
            placed.push([x, y]);
        }
        let vel = init::maxwell(&sys, &vec![0; placed.len()], t, seed + 7);
        for (q, v) in placed.iter().zip(vel) {
            state.pos.push([fixed::to_fixed(q[0]), fixed::to_fixed(q[1])]);
            state.vel.push(v);
            state.species.push(0);
        }
    }
    Scenario { warmup: o.get("warmup", 10000.0) as usize, finish: Finish::None, marked: 0, grid: [1, 80], radii: vec![0.5], sys, state }
}

/// Amphiphiles in an implicit solvent (a bulk bath): each "lipid" is a head bead (species 0)
/// and `tail` tail beads (species 1) in a line, bonded by Kremer–Grest springs plus harmonic
/// springs between next-nearest beads that keep it fairly straight. Tails attract each other (LJ, standing
/// in for the hydrophobic effect); heads only repel, with an adjustable size standing in for the
/// heads' charge. Options: lipids, density (beads per σ²), temperature, attraction (tail–tail
/// ε), head (head σ, up to 1.6), seed.
pub fn lipids(o: &Options) -> Scenario {
    let count = o.get("lipids", 1000.0) as usize;
    let tail = o.get("tail", 3.0) as usize;
    let len = tail + 1;
    let n = len * count;
    let t = o.get("temperature", 1.0);
    let head = o.get("head", 1.0).min(1.6);
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let bl = init::box_for(n, o.get("density", 0.25), o.get("aspect", 4.0 / 3.0));
    // Head–head starts as WCA with the largest allowed head (so the cutoff never grows), then the
    // real head size is set.
    let mut sys = System::new(bl, init::DT, wca(1.6), vec![1.0, 1.0])
        .with_pair(0, 1, wca(0.5 * (head + 1.0)))
        .with_pair(1, 1, Pair { eps: o.get("attraction", 1.0), sigma: 1.0, rc: 2.5 })
        .with_thermostat(t, 0.5);
    sys = sys.with_pair(0, 0, Pair { eps: 1.0, sigma: head, rc: 1.6 * 2f64.powf(1.0 / 6.0) });
    // Place straight lipids at random positions and orientations without overlaps.
    let b = sys.box_f64();
    let seed = o.get("seed", 1.0) as u64;
    let mut pos: Vec<[f64; 2]> = Vec::with_capacity(n);
    let mut draw = 0u32;
    let mut bonds = Vec::new();
    let clear = |pos: &Vec<[f64; 2]>, q: [f64; 2]| {
        pos.iter().all(|p| {
            let mut dx = (p[0] - q[0]).abs();
            dx = dx.min(b[0] - dx);
            let mut dy = (p[1] - q[1]).abs();
            dy = dy.min(b[1] - dy);
            dx * dx + dy * dy > 1.1 * 1.1
        })
    };
    for l in 0..count {
        loop {
            let u = |k: u32| gpu_prims::threefry::uniform(seed, 7, 3 * draw + k);
            let (x, y, a) = (u(0) * b[0], u(1) * b[1], u(2) * std::f64::consts::TAU);
            draw += 1;
            let beads: Vec<[f64; 2]> = (0..len).map(|k| [(x + 0.97 * k as f64 * a.cos()).rem_euclid(b[0]), (y + 0.97 * k as f64 * a.sin()).rem_euclid(b[1])]).collect();
            if beads.iter().all(|&q| clear(&pos, q)) {
                pos.extend(beads);
                let id = (len * l) as u32;
                for k in 0..len as u32 - 1 {
                    bonds.push([id + k, id + k + 1, 0]);
                    if k + 2 < len as u32 {
                        bonds.push([id + k, id + k + 2, 1]);
                    }
                }
                break;
            }
        }
    }
    let sys = sys.with_bonds(vec![md_core::BondType::kremer_grest(), md_core::BondType::Harmonic { k: 10.0, r0: 1.94 }], bonds);
    let species: Vec<u32> = (0..n).map(|i| (i % len != 0) as u32).collect();
    let vel = init::maxwell(&sys, &species, t, seed);
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: [8, 8], radii: vec![0.5 * head, 0.5], sys, state }
}

/// A "balloon" (the left quarter of the box) holding equal numbers of a light gas (species 0,
/// m = 1, helium) and a heavy one (species 1, m = heavy, air), behind a wall with small pores;
/// outside is empty. Dilute, so molecules effuse through the pores. Options: n, heavy, pore
/// (width σ), seed.
pub fn balloon(o: &Options) -> Scenario {
    // `scale` (a whole number) enlarges the whole box, keeping the gas as thin: a leak obeys Graham's
    // law only while the mean free path is longer than the pores (effusion); a denser gas flows
    // through them instead (measured: at 4× the density the ratio fell from 2.7 to 1.7).
    let k = o.get("scale", 1.0).round().max(1.0) as i64;
    let n = o.get("n", 1000.0 * (k * k) as f64) as usize;
    let small = Pair { eps: 1.0, sigma: 0.3, rc: 0.3 * 2f64.powf(1.0 / 6.0) };
    let masses = vec![1.0, o.get("heavy", 29.0 / 4.0)];
    let sys = System::new([(128 * k) << 32, (96 * k) << 32], init::DT, small, masses.clone())
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_membrane(0, 32.0 * k as f64, &[0, 1])
        .with_pores(4, o.get("pore", 0.6));
    let inside = System::new([(32 * k - 1) << 32, (96 * k) << 32], init::DT, small, masses).with_walls(0, Wall::Hard, Wall::Hard).with_walls(1, Wall::Hard, Wall::Hard);
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::fluid(&inside, n, 1.0, seed);
    for i in (1..n).step_by(2) {
        state.species[i] = 1;
    }
    state.vel = init::maxwell(&sys, &state.species, 1.0, seed);
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: [4, 1], radii: vec![0.5, 0.7], sys, state }
}

/// A channel of gas (periodic along x, thermal walls top and bottom) with a blob of marked
/// "smell" molecules (species 1, otherwise identical) at a quarter of the way along. A body
/// force along x (`draught`, σ/τ²) drives a flow; the thermal walls take away the heat it makes.
/// Options: n, density, width, height, blob (radius σ), draught, seed.
pub fn smell(o: &Options) -> Scenario {
    let n = o.get("n", 6000.0) as usize;
    let (w, h) = (o.get("width", 300.0), o.get("height", 40.0));
    let area = w * h;
    let _ = o.get("density", n as f64 / area);
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::wca(), vec![1.0, 1.0])
        .with_walls(1, Wall::Thermal(1.0), Wall::Thermal(1.0))
        .with_gravity([o.get("draught", 0.0), 0.0]);
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::fluid(&sys, n, 1.0, seed);
    let r = o.get("blob", 8.0);
    init::mark_disc(&mut state, &sys, [w / 4.0, h / 2.0], r, 1);
    // The scent can be drawn bigger than it is, to show up in a long corridor.
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: [60, 1], radii: vec![0.5, o.get("scent_radius", 0.5)], sys, state }
}

/// A cup-shaped binding pocket of five pinned beads on the lower half-circle of radius `r`
/// around (cx, cy), open at the top. With r ≈ 1.15 σ a ligand at the centre touches all five at
/// close to the Lennard-Jones minimum.
fn pocket(cx: f64, cy: f64, r: f64) -> Vec<[f64; 2]> {
    (0..5)
        .map(|k| {
            let a = std::f64::consts::PI * (1.0 + k as f64 / 4.0);
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect()
}

/// Solvent positions on a lattice inside the region [x0, x0 + w] × [y0, y0 + h], skipping any
/// site within `clear` σ of the given points.
fn solvent_in(x0: f64, y0: f64, w: f64, h: f64, density: f64, avoid: &[[f64; 2]], clear: f64) -> Vec<[f64; 2]> {
    let a = (1.0 / density).sqrt();
    let (nx, ny) = (((w - 1.0) / a) as usize, ((h - 1.0) / a) as usize);
    let mut out = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let p = [x0 + 0.5 + (i as f64 + 0.5) * a, y0 + 0.5 + (j as f64 + 0.5) * a];
            if avoid.iter().all(|q| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) > clear * clear) {
                out.push(p);
            }
        }
    }
    out
}

/// Binding in many small boxes: each 2^log2_box σ compartment has a pocket (pinned species 2)
/// and one ligand, A (species 1) in half the boxes and B (species 3) in the rest, in a
/// Lennard-Jones solvent (species 0) held at a temperature. Ligands differ only in how strongly
/// they stick to the pocket (`eps_a`, `eps_b`) and, optionally, to the solvent (`eps_ls`,
/// `eps_ls_b`). Options: boxes_x, boxes_y, log2_box, density, temperature, eps_a, eps_b,
/// eps_ls, eps_ls_b, seed.
pub fn binding(o: &Options) -> Scenario {
    let (bx, by) = (o.get("boxes_x", 4.0) as usize, o.get("boxes_y", 4.0) as usize);
    let log2 = o.get("log2_box", 4.0) as u32;
    let side = (1u64 << log2) as f64;
    let t = o.get("temperature", 1.0);
    let lj = |eps: f64| Pair { eps, sigma: 1.0, rc: 2.5 };
    let wca = Pair::wca();
    let sys = System::new([fixed::to_fixed(bx as f64 * side), fixed::to_fixed(by as f64 * side)], init::DT, lj(0.5), vec![1.0, 1.0, 1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, log2)
        .with_compartments(1, log2)
        .with_pair(0, 1, lj(o.get("eps_ls", 0.5)))
        .with_pair(0, 3, lj(o.get("eps_ls_b", o.get("eps_ls", 0.5))))
        .with_pair(0, 2, wca)
        .with_pair(1, 2, lj(o.get("eps_a", 0.6)))
        .with_pair(3, 2, lj(o.get("eps_b", 0.9)))
        .with_pair(2, 2, wca)
        .with_pair(1, 3, wca)
        .with_pinned(2)
        .with_thermostat(t, 0.5);
    let density = o.get("density", 0.5);
    let mut pos = Vec::new();
    let mut species = Vec::new();
    for j in 0..by {
        for i in 0..bx {
            let (x0, y0) = (i as f64 * side, j as f64 * side);
            let (cx, cy) = (x0 + side / 2.0, y0 + side / 2.0);
            let cup = pocket(cx, cy, 1.15);
            // The ligand starts in the bulk, a quarter box away from the pocket.
            let lig = [x0 + side * 0.2, y0 + side * 0.8];
            let mut avoid = cup.clone();
            avoid.push(lig);
            avoid.push([cx, cy]);
            for q in &cup {
                pos.push(*q);
                species.push(2);
            }
            pos.push(lig);
            species.push(if (i + j) % 2 == 0 { 1 } else { 3 });
            for q in solvent_in(x0, y0, side, side, density, &avoid, 1.1) {
                pos.push(q);
                species.push(0);
            }
        }
    }
    let seed = o.get("seed", 1.0) as u64;
    let mut vel = init::maxwell(&sys, &species, t, seed);
    for (v, s) in vel.iter_mut().zip(&species) {
        if sys.is_pinned(*s) {
            *v = [0, 0];
        }
    }
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: o.get("warmup", 2000.0) as usize, finish: Finish::None, marked: 1, grid: [8, 8], radii: vec![0.5, 0.55, 0.5, 0.55], sys, state }
}

/// The two legs of a relative binding free energy calculation, side by side: in the left box
/// the ligand (species 1) is tethered into a pocket (pinned species 2); in the right box an
/// identical ligand is tethered in bulk solvent. Each ligand is held by a harmonic bond to an
/// invisible pinned anchor (species 3, no interactions). The page morphs the ligand from A to B
/// by changing its attraction to the pocket and the solvent. Options: log2_box, density,
/// temperature, eps_lp, eps_ls, tether (spring constant), seed.
pub fn fep(o: &Options) -> Scenario {
    let log2 = o.get("log2_box", 4.0) as u32;
    let side = (1u64 << log2) as f64;
    let t = o.get("temperature", 1.0);
    let lj = |eps: f64| Pair { eps, sigma: 1.0, rc: 2.5 };
    let wca = Pair::wca();
    let none = Pair::none();
    let sys = System::new([fixed::to_fixed(2.0 * side), fixed::to_fixed(side)], init::DT, lj(0.5), vec![1.0, 1.0, 1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, log2)
        .with_pair(0, 1, lj(o.get("eps_ls", 0.5)))
        .with_pair(0, 2, wca)
        .with_pair(1, 2, lj(o.get("eps_lp", 1.0)))
        .with_pair(2, 2, wca)
        .with_pair(3, 0, none)
        .with_pair(3, 1, none)
        .with_pair(3, 2, none)
        .with_pair(3, 3, none)
        .with_pinned(2)
        .with_pinned(3)
        .with_thermostat(t, 0.5);
    let density = o.get("density", 0.5);
    let mut pos = Vec::new();
    let mut species = Vec::new();
    let mut bonds = Vec::new();
    for leg in 0..2 {
        let x0 = leg as f64 * side;
        let (cx, cy) = (x0 + side / 2.0, side / 2.0);
        let mut avoid = vec![[cx, cy]];
        if leg == 0 {
            for q in pocket(cx, cy, 1.15) {
                avoid.push(q);
                pos.push(q);
                species.push(2);
            }
        }
        // Anchor, then the ligand on it.
        pos.push([cx, cy]);
        species.push(3);
        let anchor = (pos.len() - 1) as u32;
        pos.push([cx, cy + 0.05]);
        species.push(1);
        bonds.push([anchor, anchor + 1, 0]);
        for q in solvent_in(x0, 0.0, side, side, density, &avoid, 1.1) {
            pos.push(q);
            species.push(0);
        }
    }
    let sys = sys.with_bonds(vec![md_core::BondType::Harmonic { k: o.get("tether", 5.0), r0: 0.0 }], bonds);
    let seed = o.get("seed", 1.0) as u64;
    let mut vel = init::maxwell(&sys, &species, t, seed);
    for (v, s) in vel.iter_mut().zip(&species) {
        if sys.is_pinned(*s) {
            *v = [0, 0];
        }
    }
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: o.get("warmup", 2000.0) as usize, finish: Finish::None, marked: 1, grid: [8, 8], radii: vec![0.5, 0.55, 0.5, 0.1], sys, state }
}

/// A model cell in a solution: a ring of bonded membrane beads (species 2) whose gaps let small
/// solvent molecules (species 0, σ = 0.5) through but not solute particles (species 1, "salt",
/// σ = 1). Inside, the solute share is `inside`; outside it is `outside` (0 for pure water). A
/// bulk heat bath keeps the temperature (`bath` 0: none, constant energy). With several cells (`cells_x` × `cells_y`, each in a
/// square of side `width`), the membrane beads come first in id order: cell c's beads are ids
/// c·beads … c·beads + beads − 1, in order round the ring. Options: cells_x, cells_y, width,
/// radius, beads, density, inside, outside, stiffness, bend, rest (spring rest length over the
/// starting spacing), bath, water (0: water interacts with nothing), seed.
pub fn cell(o: &Options) -> Scenario {
    // A grid of cells, each in its own square of side `width` σ.
    let (cx_n, cy_n) = (o.get("cells_x", 1.0) as usize, o.get("cells_y", 1.0) as usize);
    let side = o.get("width", 64.0);
    let (w, hgt) = (side * cx_n as f64, side * cy_n as f64);
    let radius = o.get("radius", 10.0);
    let beads = o.get("beads", 44.0) as usize;
    let small = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(hgt)], init::DT, small(0.5), vec![1.0, 1.0, 1.0])
        .with_pair(0, 1, small(0.75))
        .with_pair(1, 1, small(1.0))
        .with_pair(0, 2, small(0.5))
        .with_pair(1, 2, small(1.5))
        .with_pair(2, 2, small(1.0));
    let sys = if o.get("bath", 1.0) > 0.0 { sys.with_thermostat(1.0, 0.5) } else { sys };
    // `water` 0: the water feels nothing and nothing feels it (as if there were none).
    let sys = if o.get("water", 1.0) > 0.0 { sys } else { sys.with_pair(0, 0, Pair::none()).with_pair(0, 1, Pair::none()).with_pair(0, 2, Pair::none()) };
    let centres: Vec<[f64; 2]> = (0..cy_n).flat_map(|j| (0..cx_n).map(move |i| [(i as f64 + 0.5) * side, (j as f64 + 0.5) * side])).collect();
    let spacing = std::f64::consts::TAU * radius / beads as f64;
    // Membrane beads first: cell by cell, in order round each ring (ids c·beads + k).
    let mut pos: Vec<[f64; 2]> = vec![];
    let mut bonds: Vec<[u32; 3]> = vec![];
    let next = 2.0 * radius * (std::f64::consts::TAU / beads as f64).sin();
    for (ci, c) in centres.iter().enumerate() {
        let base = (ci * beads) as u32;
        for k in 0..beads {
            let a = std::f64::consts::TAU * k as f64 / beads as f64;
            pos.push([c[0] + radius * a.cos(), c[1] + radius * a.sin()]);
        }
        let n = beads as u32;
        bonds.extend((0..n).map(|k| [base + k, base + (k + 1) % n, 0]));
        // Bending stiffness: springs between next-nearest beads keep the ring smooth.
        bonds.extend((0..n).map(|k| [base + k, base + (k + 2) % n, 1]));
    }
    let mut species = vec![2u32; pos.len()];
    // Fluid on a lattice, away from the membranes; solute chosen at random by region.
    let density = o.get("density", 0.8);
    let a = (1.0 / density).sqrt();
    let (nx, ny) = ((w / a) as usize, (hgt / a) as usize);
    let seed = o.get("seed", 1.0) as u64;
    let (fin, fout) = (o.get("inside", 0.15), o.get("outside", 0.0));
    let mut draw = 0u32;
    for j in 0..ny {
        for i in 0..nx {
            let p = [(i as f64 + 0.5) * a, (j as f64 + 0.5) * a];
            let (r, _) = centres.iter().map(|c| (((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt(), 0)).fold((f64::MAX, 0), |m, x| if x.0 < m.0 { x } else { m });
            if (r - radius).abs() < 1.2 {
                continue;
            }
            let f = if r < radius { fin } else { fout };
            let u = gpu_prims::threefry::uniform(seed, 5, draw);
            draw += 1;
            // Solute is bigger: only place it on every other site to avoid overlaps.
            let solute = u < f && (i + j) % 2 == 0 && (r - radius).abs() > 1.8;
            pos.push(p);
            species.push(solute as u32);
        }
    }
    let k = o.get("stiffness", 20.0);
    let bend = o.get("bend", 0.5 * k);
    // `rest` < 1 makes the springs' rest length shorter than the starting spacing, so the ring
    // starts taut instead of slack.
    let rest = o.get("rest", 1.0);
    let sys = sys.with_bonds(vec![md_core::BondType::Harmonic { k, r0: rest * spacing }, md_core::BondType::Harmonic { k: bend, r0: rest * next }], bonds);
    let vel = init::maxwell(&sys, &species, 1.0, seed);
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    // Membrane beads are drawn larger than their size so the cell boundary reads as a line.
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: [8, 8], radii: vec![0.25, 0.5, 0.85], sys, state }
}

/// A droplet of Lennard-Jones liquid (rc 2.5) in a sealed square box with hard walls and no
/// gravity, for heating in steps: it warms, then evaporates (taking in latent heat), then the
/// vapour warms. Constant energy between the page's heat additions. Options: n, size (box side,
/// σ; the default leaves room for all of it to evaporate below the critical point), temperature,
/// density (of the droplet), seed.
pub fn flask(o: &Options) -> Scenario {
    let n = o.get("n", 1000.0) as usize;
    // At 0.002 per σ² on average, all of it can evaporate below the critical point (~0.46).
    let side = o.get("size", (n as f64 / 0.002).sqrt());
    let t = o.get("temperature", 0.36);
    let sys = System::new([fixed::to_fixed(side), fixed::to_fixed(side)], init::DT, Pair::lj(2.5), vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard);
    // A round droplet in the middle, on a square lattice at the liquid's density.
    let a = (1.0 / o.get("density", 0.7)).sqrt();
    let r = (n as f64 * a * a / std::f64::consts::PI).sqrt() + a;
    let c = side / 2.0;
    let k = (r / a).ceil() as i64;
    let mut sites: Vec<[f64; 2]> = vec![];
    for j in -k..=k {
        for i in -k..=k {
            let p = [c + i as f64 * a, c + j as f64 * a];
            sites.push(p);
        }
    }
    sites.sort_by(|p, q| ((p[0] - c).hypot(p[1] - c)).total_cmp(&(q[0] - c).hypot(q[1] - c)));
    sites.truncate(n);
    let species = vec![0u32; n];
    let vel = init::maxwell(&sys, &species, t, o.get("seed", 1.0) as u64);
    let state = State { pos: sites.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: 0, finish: Finish::None, marked: 0, grid: [8, 8], radii: vec![0.5], sys, state }
}

/// Water (species 0), oil (1) and soap, freshly shaken together: each soap molecule is a
/// water-loving head (2) bonded to two oil-loving tail beads (3), a straight trimer. All are LJ
/// (rc 2.5) with ε = 1 between friends (water–water, water–head, oil–oil, oil–tail, tail–tail),
/// ε = `cross` between water and oil, and `soap_cross` (0: purely repulsive) for the soap's
/// dislikes (water–tail, oil–head, head–tail, head–head). Options: n (sites), soap (share of
/// the sites in soap molecules), oil (share of the rest that is oil), density, temperature, cross, soap_cross, bath_rate (a gentle
/// bath takes away the stirring heat), seed.
pub fn emulsion(o: &Options) -> Scenario {
    let n = o.get("n", 8000.0) as usize;
    let t = o.get("temperature", 0.7);
    let cross = o.get("cross", 0.3);
    let lj = |eps: f64| Pair { eps, sigma: 1.0, rc: 2.5 };
    let bl = init::box_for(n, o.get("density", 0.7), o.get("aspect", 4.0 / 3.0));
    let mut sys = System::new(bl, init::DT, lj(1.0), vec![1.0; 4]).with_thermostat(t, o.get("bath_rate", 0.02));
    // The soap's dislikes (head–oil, tail–water, head–tail, head–head) are `soap_cross`; at 0
    // they are purely repulsive (WCA), which makes it a strong surfactant.
    let soap_cross = o.get("soap_cross", 0.0);
    let wca = Pair { eps: 1.0, sigma: 1.0, rc: 2f64.powf(1.0 / 6.0) };
    let dislike = if soap_cross > 0.0 { lj(soap_cross) } else { wca };
    sys = sys.with_pair(0, 1, lj(cross));
    for (a, b, friends) in [(0, 2, true), (0, 3, false), (1, 2, false), (1, 3, true), (2, 2, false), (2, 3, false), (3, 3, true)] {
        sys = sys.with_pair(a, b, if friends { lj(1.0) } else { dislike });
    }
    // A square lattice of sites; soap molecules take three neighbouring sites in a row.
    let b = sys.box_f64();
    let nx = (n as f64 * b[0] / b[1]).sqrt().round() as usize;
    let ny = n.div_ceil(nx);
    let (ax, ay) = (b[0] / nx as f64, b[1] / ny as f64);
    let seed = o.get("seed", 1.0) as u64;
    let soap = o.get("soap", 0.1);
    // Share of the soap molecules that start as soap; the rest start "dormant", as three bonded
    // beads that are all water, for the page to switch on where the visitor drops soap (bonds
    // cannot be made at run time, but species can change).
    let active = o.get("active", 1.0);
    let oil = o.get("oil", 0.5); // share of the other sites that are oil
    let mut kind = vec![u8::MAX; nx * ny]; // 255 free
    let mut bonds = Vec::new();
    let mut order: Vec<usize> = vec![]; // site index per particle id
    let mut draw = 0u32;
    let target = (soap * (nx * ny) as f64 / 3.0) as usize;
    let mut tries = 0;
    while order.len() / 3 < target && tries < 100 * target + 100 {
        tries += 1;
        let u = |k: u32| gpu_prims::threefry::uniform(seed, 11, 2 * draw + k);
        let (i, j) = ((u(0) * nx as f64) as usize, (u(1) * ny as f64) as usize);
        draw += 1;
        let sites = [j * nx + i, j * nx + (i + 1) % nx, j * nx + (i + 2) % nx];
        if sites.iter().all(|&s| kind[s] == u8::MAX) {
            let id = order.len() as u32;
            let on = ((order.len() / 3) as f64) < active * target as f64;
            for (k, &s) in sites.iter().enumerate() {
                kind[s] = if !on { 0 } else if k == 0 { 2 } else { 3 };
                order.push(s);
            }
            bonds.extend([[id, id + 1, 0], [id + 1, id + 2, 0], [id, id + 2, 1]]);
        }
    }
    for s in 0..nx * ny {
        if kind[s] == u8::MAX {
            kind[s] = (gpu_prims::threefry::uniform(seed, 12, s as u32) < oil) as u8;
            order.push(s);
        }
    }
    let pos: Vec<[i64; 2]> = order.iter().map(|&s| [fixed::to_fixed(((s % nx) as f64 + 0.5) * ax), fixed::to_fixed(((s / nx) as f64 + 0.5) * ay)]).collect();
    let species: Vec<u32> = order.iter().map(|&s| kind[s] as u32).collect();
    let sys = if bonds.is_empty() { sys } else { sys.with_bonds(vec![md_core::BondType::kremer_grest(), md_core::BondType::Harmonic { k: 10.0, r0: 2.0 * ax.min(1.1) }], bonds) };
    let vel = init::maxwell(&sys, &species, t, seed);
    let state = State { pos, vel, species };
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: grid_for(n, b, 16.0), radii: vec![0.5, 0.5, 0.55, 0.5], sys, state }
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

    /// The U-tube's wall beads (pinned) stay exactly where they start.
    #[test]
    fn utube_walls_stay_put() {
        let sc = build("osmometer", &Options::default().with("width", 24.0).with("height", 60.0).with("depth", 30.0).with("channel", 10.0).with("bath", 0.0)).unwrap();
        let mut e = md_ref::IntEngine::<f32>::new(sc.sys.clone(), sc.state.clone());
        e.step(300);
        for i in 0..sc.state.n() {
            if sc.state.species[i] == 2 {
                assert_eq!(e.state.pos[i], sc.state.pos[i], "wall bead {i} moved");
            }
        }
    }

    #[test]
    fn every_scenario_builds_and_runs_briefly_on_the_cpu() {
        for name in NAMES {
            let mut o = Options::default();
            // Small versions, so the CPU engine finishes quickly.
            for (k, v) in [("n", 1500.0), ("boxes_x", 8.0), ("boxes_y", 6.0), ("columns", 16.0), ("chains", 8.0), ("lipids", 200.0), ("width", 40.0), ("radius", 8.0), ("aerosols", 10.0), ("rows", 2.0)] {
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

/// Acid and buffer: protons (species 0) and buffer anions (species 1) in many small boxes, in an
/// implicit solvent (a bulk bath). A proton binds to an anion in a short Lennard-Jones well
/// (minimum at 0.56 σ, cut at 1 σ). Protons repel each other over 1.35 σ, so an anion holds at
/// most one, and anions repel over 1.68 σ, so a proton cannot bridge two. Column c (from the left) of boxes
/// holds c × `step` protons; the bottom `buffered_rows` rows also hold `anions` anions each, the
/// `water_rows` rows above them none. Options: log2_box, columns, buffered_rows, water_rows,
/// anions, step, eps (binding well depth), temperature, seed.
pub fn buffer(o: &Options) -> Scenario {
    let log2 = o.get("log2_box", 5.0) as u32;
    let side = (1u64 << log2) as f64;
    let cols = o.get("columns", 11.0) as usize;
    let (rb, rw) = (o.get("buffered_rows", 8.0) as usize, o.get("water_rows", 4.0) as usize);
    let anions = o.get("anions", 40.0) as usize;
    let step = o.get("step", 8.0) as usize;
    let t = o.get("temperature", 1.0);
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let sys = System::new([fixed::to_fixed(cols as f64 * side), fixed::to_fixed((rb + rw) as f64 * side)], init::DT, wca(1.0), vec![1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, log2)
        .with_compartments(1, log2)
        .with_pair(0, 0, wca(1.2))
        .with_pair(0, 1, Pair { eps: o.get("eps", 6.0), sigma: 0.5, rc: 1.0 })
        .with_pair(1, 1, wca(1.5))
        .with_thermostat(t, 0.5);
    let seed = o.get("seed", 1.0) as u64;
    // Sites on a 2 σ lattice inside each box, shuffled; protons then anions take the first ones.
    let per_axis = ((side - 2.0) / 2.0) as usize + 1;
    let mut pos = Vec::new();
    let mut species = Vec::new();
    for j in 0..rb + rw {
        for i in 0..cols {
            let b = (j * cols + i) as u32;
            let mut sites: Vec<usize> = (0..per_axis * per_axis).collect();
            for k in (1..sites.len()).rev() {
                let r = (gpu_prims::threefry::uniform(seed, 11 + b, k as u32) * (k + 1) as f64) as usize;
                sites.swap(k, r.min(k));
            }
            let (np, na) = (i * step, if j < rb { anions } else { 0 });
            assert!(np + na <= sites.len(), "too many particles for a box of {side} σ");
            for (k, &site) in sites[..np + na].iter().enumerate() {
                let (sx, sy) = (site % per_axis, site / per_axis);
                pos.push([i as f64 * side + 1.0 + 2.0 * sx as f64, j as f64 * side + 1.0 + 2.0 * sy as f64]);
                species.push(if k < np { 0 } else { 1 });
            }
        }
    }
    let vel = init::maxwell(&sys, &species, t, seed);
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: o.get("warmup", 2000.0) as usize, finish: Finish::None, marked: 0, grid: [8, 8], radii: vec![0.6, 1.0], sys, state }
}

/// A mask: air (species 0) blown round a periodic channel by a fan through a filter of fixed
/// fibres (species 1, pinned discs of radius `fiber`), carrying aerosol particles (species 2,
/// diameter `size`, mass `density` × size², at least 1). Aerosols stick to a fibre they touch (a short
/// Lennard-Jones well); air only bounces. Walls top and bottom are frictionless, so the fan
/// works only against the filter. Options: width, height, air (density), fiber, gap (between
/// fibre surfaces across the flow), rows, size, density, aerosols, fan (drift σ/τ), stick (ε),
/// seed.
pub fn mask(o: &Options) -> Scenario {
    let (w, h) = (o.get("width", 128.0), o.get("height", 64.0));
    let rf = o.get("fiber", 2.0);
    let gap = o.get("gap", 8.0);
    let rows = o.get("rows", 3.0) as usize;
    let size = o.get("size", 1.0);
    // No lighter than an air molecule.
    let m_a = (o.get("density", 10.0) * size * size).max(1.0);
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let af = rf + size / 2.0;
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, wca(1.0), vec![1.0, 1.0, m_a])
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_pair(0, 1, wca(rf + 0.5))
        .with_pair(1, 1, wca(1.0))
        .with_pair(0, 2, wca((1.0 + size) / 2.0))
        .with_pair(2, 2, wca(size))
        .with_pair(1, 2, Pair { eps: o.get("stick", 12.0), sigma: af / 2f64.powf(1.0 / 6.0), rc: 1.5 * af / 2f64.powf(1.0 / 6.0) })
        .with_pinned(1)
        .with_thermostat(1.0, 2.0)
        .with_fan(1, 0.0, w / 8.0, o.get("fan", 1.0));
    // Fibres in staggered columns across the middle of the channel, 2 rf + gap apart.
    let pitch = 2.0 * rf + gap;
    let x0 = w / 2.0;
    assert!(x0 + (rows as f64 - 1.0) * pitch * 0.866 + rf + 1.0 < w, "{rows} rows of fibres do not fit in a channel {w} σ long");
    let mut fibers = Vec::new();
    for c in 0..rows {
        let x = x0 + c as f64 * pitch * 0.866;
        let shift = if c % 2 == 0 { 0.0 } else { pitch / 2.0 };
        let mut y = pitch / 2.0 + shift;
        while y < h - rf - 0.6 {
            fibers.push([x, y]);
            y += pitch;
        }
    }
    let seed = o.get("seed", 1.0) as u64;
    let aerosols = o.get("aerosols", 60.0) as usize;
    // Aerosols start upstream of the fan, spread over the channel; air fills the rest.
    let mut pos: Vec<[f64; 2]> = fibers.clone();
    let mut species = vec![1u32; fibers.len()];
    let clear = |pos: &Vec<[f64; 2]>, q: [f64; 2], r: f64| pos.iter().all(|p| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) > r * r);
    let mut draw = 0u32;
    while species.iter().filter(|&&s| s == 2).count() < aerosols {
        assert!(draw < 1_000_000, "no room for {aerosols} aerosols of size {size} upstream");
        let u = |k: u32| gpu_prims::threefry::uniform(seed, 13, 2 * draw + k);
        let q = [w / 8.0 + u(0) * (x0 - w / 8.0 - pitch), size + u(1) * (h - 2.0 * size)];
        draw += 1;
        if clear(&pos, q, size + 1.0) {
            pos.push(q);
            species.push(2);
        }
    }
    let a = (1.0 / o.get("air", 0.2)).sqrt();
    let (nx, ny) = ((w / a) as usize, ((h - 1.0) / a) as usize);
    for j in 0..ny {
        for i in 0..nx {
            let q = [(i as f64 + 0.5) * a, 0.5 + (j as f64 + 0.5) * (h - 1.0) / ny as f64];
            let near_fiber = fibers.iter().any(|f| (f[0] - q[0]).powi(2) + (f[1] - q[1]).powi(2) < (rf + 1.0).powi(2));
            let near_aerosol = pos.iter().zip(&species).any(|(p, &s)| s == 2 && (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) < (size / 2.0 + 1.0).powi(2));
            if !near_fiber && !near_aerosol {
                pos.push(q);
                species.push(0);
            }
        }
    }
    let mut vel = init::maxwell(&sys, &species, 1.0, seed);
    for (v, s) in vel.iter_mut().zip(&species) {
        if *s == 1 {
            *v = [0, 0];
        }
    }
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: o.get("warmup", 0.0) as usize, finish: Finish::None, marked: 2, grid: [8, 8], radii: vec![0.5, rf, size / 2.0], sys, state }
}

/// DNA melting, coarse-grained: each box holds one strand of A beads (species 0) and one of B
/// beads (species 1), in an implicit solvent (a bulk bath). Only A–B attract (a Lennard-Jones
/// well standing in for base pairing); beads of a strand are joined by Kremer–Grest bonds and
/// stiffened by springs between next-nearest beads. Rows of boxes come in bands with strands of
/// each length in `lengths` (bottom to top). Options: log2_box, columns, rows (per length),
/// lengths (a number whose decimal digits are the lengths, e.g. 248), eps, stiffness (of the
/// next-nearest springs), temperature, seed.
pub fn dna(o: &Options) -> Scenario {
    let log2 = o.get("log2_box", 4.0) as u32;
    let side = (1u64 << log2) as f64;
    let cols = o.get("columns", 12.0) as usize;
    let rows = o.get("rows", 3.0) as usize;
    let lengths: Vec<usize> = (o.get("lengths", 248.0) as u64).to_string().chars().map(|c| c.to_digit(10).unwrap() as usize).collect();
    let t = o.get("temperature", 1.0);
    let wca = Pair::wca();
    let mut pos = Vec::new();
    let mut species = Vec::new();
    let mut bonds = Vec::new();
    for (band, &len) in lengths.iter().enumerate() {
        assert!(len >= 1 && (len as f64) < side - 3.0, "strands of {len} do not fit in {side} σ boxes");
        for j in 0..rows {
            for i in 0..cols {
                let (x0, y0) = (i as f64 * side, (band * rows + j) as f64 * side);
                for (strand, y) in [(0u32, y0 + side * 0.3), (1u32, y0 + side * 0.7)] {
                    let id = pos.len() as u32;
                    for k in 0..len {
                        pos.push([x0 + (side - (len - 1) as f64 * 0.97) / 2.0 + k as f64 * 0.97, y]);
                        species.push(strand);
                    }
                    for k in 0..len as u32 {
                        if k + 1 < len as u32 {
                            bonds.push([id + k, id + k + 1, 0]);
                        }
                        if k + 2 < len as u32 {
                            bonds.push([id + k, id + k + 2, 1]);
                        }
                    }
                }
            }
        }
    }
    let ny = lengths.len() * rows;
    let mut sys = System::new([fixed::to_fixed(cols as f64 * side), fixed::to_fixed(ny as f64 * side)], init::DT, wca, vec![1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, log2)
        .with_compartments(1, log2)
        .with_pair(0, 1, Pair { eps: o.get("eps", 1.5), sigma: 1.0, rc: 2.0 })
        .with_thermostat(t, 0.5);
    if !bonds.is_empty() {
        sys = sys.with_bonds(vec![md_core::BondType::kremer_grest(), md_core::BondType::Harmonic { k: o.get("stiffness", 10.0), r0: 1.94 }], bonds);
    }
    let seed = o.get("seed", 1.0) as u64;
    let vel = init::maxwell(&sys, &species, t, seed);
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: o.get("warmup", 0.0) as usize, finish: Finish::None, marked: 0, grid: [8, 8], radii: vec![0.5, 0.5], sys, state }
}

/// Smoluchowski's trapdoor (Maxwell's demon without a demon): a wall across the middle of a box
/// of gas (species 0) with a pore every 16 σ, each closed by a door (species 1, a heavy disc)
/// tethered by a spring to an invisible pinned anchor (species 3) just right of the pore. Two
/// pinned stops (species 2) at the pore's edges keep the door from swinging left, so gas from
/// the left can push it open and gas from the right only pushes it shut. A bath acts on the
/// doors alone (at `door_t`); the gas is heated only by the end walls, if at all. Options: width, height, density,
/// door (diameter), pore (width), door_t, gas_t (temperature of the end walls; 0 for
/// insulating), door_mass, spring, seed.
pub fn demon(o: &Options) -> Scenario {
    let (w, h) = (o.get("width", 96.0), o.get("height", 64.0));
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let none = Pair::none();
    let (door, stop, pore) = (o.get("door", 2.5), 0.5, o.get("pore", 2.4));
    let xm = w / 2.0;
    // The end walls are heat baths for the gas at `gas_t` (0: insulating).
    let gt = o.get("gas_t", 0.0);
    let end = if gt > 0.0 { Wall::Thermal(gt) } else { Wall::Hard };
    let mut sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, wca(1.0), vec![1.0, o.get("door_mass", 10.0), 1.0, 1.0])
        .with_walls(0, end, end)
        .with_pair(0, 1, wca((1.0 + door) / 2.0))
        .with_pair(0, 2, wca((1.0 + stop) / 2.0))
        .with_pair(1, 1, wca(door))
        .with_pair(1, 2, wca((door + stop) / 2.0))
        .with_pair(2, 2, none);
    for s in 0..4 {
        sys = sys.with_pair(s, 3, none);
    }
    let sys = sys
        .with_membrane(0, xm, &[0, 1])
        .with_pores(4, pore)
        .with_pinned(2)
        .with_pinned(3)
        .with_thermostat(o.get("door_t", 1.0), 1.0)
        .with_fan(1 << 1, 0.0, w, 0.0);
    // Per pore: stops at its edges just left of the wall, the door resting against them.
    let contact = (door + stop) / 2.0 * 2f64.powf(1.0 / 6.0);
    let mut pos = Vec::new();
    let mut species = Vec::new();
    let mut bonds = Vec::new();
    let pores = (h / 16.0) as usize;
    for k in 0..pores {
        let yc = 16.0 * k as f64 + pore / 2.0;
        let sx = xm - 0.3;
        let dx = sx + (contact * contact - (pore / 2.0) * (pore / 2.0)).sqrt();
        for y in [yc - pore / 2.0, yc + pore / 2.0] {
            pos.push([sx, y]);
            species.push(2);
        }
        let id = pos.len() as u32;
        pos.push([dx, yc]);
        species.push(1);
        pos.push([dx, yc]);
        species.push(3);
        bonds.push([id, id + 1, 0]);
    }
    // Gas on a lattice on both sides, clear of the wall and the doors.
    let a = (1.0 / o.get("density", 0.1)).sqrt();
    let (nx, ny) = ((w / a) as usize, (h / a) as usize);
    for j in 0..ny {
        for i in 0..nx {
            let q = [(i as f64 + 0.5) * a, (j as f64 + 0.5) * a];
            // Symmetric about the wall, so both sides start with the same number.
            if (q[0] - xm).abs() > door + 1.5 {
                pos.push(q);
                species.push(0);
            }
        }
    }
    let sys = sys.with_bonds(vec![md_core::BondType::Harmonic { k: o.get("spring", 5.0), r0: 0.0 }], bonds);
    let seed = o.get("seed", 1.0) as u64;
    let mut vel = init::maxwell(&sys, &species, 1.0, seed);
    for (v, s) in vel.iter_mut().zip(&species) {
        if *s >= 2 {
            *v = [0, 0];
        }
    }
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: 0, finish: Finish::None, marked: 0, grid: [2, 1], radii: vec![0.5, door / 2.0, stop / 2.0, 0.0], sys, state }
}

/// How the `ensemble` boxes' molecules start: every molecule gets exactly the kinetic energy of
/// its group (in a random direction), scaled so that the average is kT = 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnsembleStart {
    /// Every molecule the same energy.
    Equal,
    /// In every box, half the molecules hot (1.8) and half cold (0.2).
    HalfHot,
    /// In every box, 5% very hot (10) and the rest just below average.
    FewHot,
    /// One box hot, the rest cold (0.6).
    OneBoxHot,
    /// The left half of the boxes hot (1.6), the right half cold (0.4).
    LeftHot,
}

impl EnsembleStart {
    /// From the page's option code (0–4).
    pub fn from_code(c: f64) -> Result<Self, String> {
        Ok(match c as i64 {
            0 => Self::Equal,
            1 => Self::HalfHot,
            2 => Self::FewHot,
            3 => Self::OneBoxHot,
            4 => Self::LeftHot,
            _ => return Err(format!("unknown ensemble start {c}")),
        })
    }
}

/// The textbook ensemble: many boxes of gas behind diathermal walls (molecules stay in their box
/// but push on their neighbours through the walls), isolated as a whole, so the total energy is
/// fixed. Options: boxes_x, boxes_y, log2_box, per_box, start (see [`EnsembleStart`]), dt, seed.
pub fn ensemble(o: &Options) -> Scenario {
    let (bx, by) = (o.get("boxes_x", 8.0) as i64, o.get("boxes_y", 6.0) as i64);
    let log2 = o.get("log2_box", 4.0) as u32;
    let side = 1i64 << (32 + log2);
    let start = EnsembleStart::from_code(o.get("start", 0.0)).unwrap_or(EnsembleStart::Equal);
    let sys = System::new([bx * side, by * side], o.get("dt", init::DT), Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_compartments(0, log2)
        .with_compartments(1, log2)
        .with_diathermal_compartments();
    let per_box = o.get("per_box", 64.0) as usize;
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::compartment_fluid(&sys, per_box, 1.0, seed);
    let nbox = (bx * by) as f64;
    let energies: Vec<f64> = (0..state.n())
        .map(|i| {
            let [cx, cy] = sys.compartment_of(state.pos[i]);
            let u = gpu_prims::threefry::uniform(seed, 17, i as u32);
            match start {
                EnsembleStart::Equal => 1.0,
                EnsembleStart::HalfHot => if u < 0.5 { 1.8 } else { 0.2 },
                EnsembleStart::FewHot => if u < 0.05 { 10.0 } else { 0.5 },
                EnsembleStart::OneBoxHot => if (cx, cy) == (0, 0) { nbox - (nbox - 1.0) * 0.6 } else { 0.6 },
                EnsembleStart::LeftHot => if (cx as i64) < bx / 2 { 1.6 } else { 0.4 },
            }
        })
        .collect();
    // Scale so the average is exactly 1 (the random splits are not exactly half or 5%).
    let mean = energies.iter().sum::<f64>() / energies.len() as f64;
    for (i, e) in energies.iter().enumerate() {
        let speed = (2.0 * e / mean).sqrt();
        let a = std::f64::consts::TAU * gpu_prims::threefry::uniform(seed, 19, i as u32);
        state.vel[i] = [sys.to_step_displacement(speed * a.cos()), sys.to_step_displacement(speed * a.sin())];
    }
    Scenario { warmup: 0, finish: Finish::None, marked: 0, grid: [bx as u32, by as u32], radii: vec![0.5], sys, state }
}

/// A movable semi-permeable membrane: a straight chain of beads (species 2) across a box that is
/// periodic top to bottom, joined by springs (stiffened by next-nearest springs), with gaps that
/// let water (species 0, small) through but not solute (species 1). Pure water on the left,
/// solution on the right; the membrane is free to move. A heat bath holds the temperature.
/// Options: width, height, density, solute (share of sites on the right), stiffness, seed.
pub fn osmosis_free(o: &Options) -> Scenario {
    let (w, h) = (o.get("width", 96.0), o.get("height", 48.0));
    let small = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, small(0.5), vec![1.0, 1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_pair(0, 1, small(0.75))
        .with_pair(1, 1, small(1.0))
        .with_pair(0, 2, small(0.5))
        .with_pair(1, 2, small(1.5))
        .with_pair(2, 2, small(1.0))
        .with_thermostat(1.0, 0.5);
    let beads = (h / 1.43).round() as usize;
    let spacing = h / beads as f64;
    let xm = w / 2.0;
    let mut pos: Vec<[f64; 2]> = (0..beads).map(|k| [xm, (k as f64 + 0.5) * spacing]).collect();
    let mut species = vec![2u32; beads];
    let n = beads as u32;
    let mut bonds: Vec<[u32; 3]> = (0..n).map(|k| [k, (k + 1) % n, 0]).collect();
    bonds.extend((0..n).map(|k| [k, (k + 2) % n, 1]));
    let density = o.get("density", 0.8);
    let a = (1.0 / density).sqrt();
    let (nx, ny) = ((w / a) as usize, (h / a) as usize);
    let seed = o.get("seed", 1.0) as u64;
    let f = o.get("solute", 0.5);
    let mut draw = 0u32;
    for j in 0..ny {
        for i in 0..nx {
            let p = [(i as f64 + 0.5) * a, (j as f64 + 0.5) * h / ny as f64];
            if (p[0] - xm).abs() < 1.2 || p[0] < 0.4 || p[0] > w - 0.4 {
                continue;
            }
            let u = gpu_prims::threefry::uniform(seed, 5, draw);
            draw += 1;
            let solute = p[0] > xm + 1.8 && u < f && (i + j) % 2 == 0;
            pos.push(p);
            species.push(solute as u32);
        }
    }
    let k = o.get("stiffness", 20.0);
    let sys = sys.with_bonds(vec![md_core::BondType::Harmonic { k, r0: spacing }, md_core::BondType::Harmonic { k: 0.5 * k, r0: 2.0 * spacing }], bonds);
    let vel = init::maxwell(&sys, &species, 1.0, seed);
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    Scenario { warmup: 0, finish: Finish::None, marked: 1, grid: [8, 8], radii: vec![0.25, 0.5, 0.85], sys, state }
}

/// An osmometer (the U-tube): a Lennard-Jones liquid under gravity in two arms separated by a
/// fixed membrane down the middle that lets the solvent (species 0) through but not the solute
/// (species 1, in the right arm). Solute and solvent are identical apart from the membrane (an
/// ideal solution: a solute that repelled itself made the osmotic pressure ~1.5× van 't Hoff's). Vapour above the liquid. A heat bath holds the temperature.
/// Options: width (of the box; each arm is half), height, depth (initial liquid depth), solute
/// (share of the right arm's molecules), gravity, temperature (bath: 0 for none, constant
/// energy), cutoff, wetting (the wall beads' attraction), channel, settle, lattice, seed.
/// With `settle` > 0 (τ) the liquid first settles as pure solvent for that long (it compacts
/// under gravity and evaporates into the space above, which moves energy between heat and
/// potential energy); then it is brought back to `temperature`, and only then do `solute` of the
/// right arm's molecules become solute:
/// solute and solvent are identical apart from the membrane, so the change costs no energy and
/// what follows is osmosis alone.
/// With `channel` > 0 it is the classic U-tube: a wall of fixed beads divides the arms from the
/// top down to `channel` above the floor, so the arms meet only through the passage at the
/// bottom, where the membrane is.
pub fn osmometer(o: &Options) -> Scenario {
    // Narrow arms and fairly strong gravity: the level settles within ~1500 τ.
    let (w, h) = (o.get("width", 32.0), o.get("height", 112.0));
    let t = o.get("temperature", 0.5);
    // A longer cutoff than the usual 2.5 σ raises the critical point (towards 0.53 in 2D), so the
    // liquid at kT 0.5 is clear of it: denser, with a sharper surface.
    let rc = o.get("cutoff", 4.0);
    let lj = Pair::lj(rc);
    // The side walls are lined with fixed beads (species 2) that attract the liquid (a bit less than
    // it attracts itself), so it wets them with a nearly flat surface; bare hard walls repel it and
    // it pulls away into a blob.
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, lj, vec![1.0, 1.0, 1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_pair(2, 2, Pair::none())
        .with_pair(0, 2, Pair { eps: o.get("wetting", 0.7), sigma: 1.0, rc })
        .with_pair(1, 2, Pair { eps: o.get("wetting", 0.7), sigma: 1.0, rc })
        .with_pinned(2)
        .with_gravity([0.0, -o.get("gravity", 0.006)])
        .with_membrane(0, w / 2.0, &[1]);
    let sys = if o.get("bath", 1.0) > 0.0 { sys.with_thermostat(t, 0.5) } else { sys };
    let depth = o.get("depth", 40.0);
    // The liquid starts on a square lattice at density `lattice` (it compacts to ~0.75 under
    // gravity at kT 0.5, slowly; but a start at 0.76 nearly froze it, and the U-tube barely rose).
    let a = (1.0f64 / o.get("lattice", 0.72)).sqrt();
    let (nx, ny) = (((w - 2.8) / a) as usize, (depth / a) as usize);
    let seed = o.get("seed", 1.0) as u64;
    let f = o.get("solute", 0.3);
    let mut pos = vec![];
    let mut species = vec![];
    // Wall beads stay beyond the soft floor's and lid's range (1.12 σ): they are pinned, but the
    // walls' push on them still counts as (constant) energy, ~16,000 ε per bead at 0.5 σ.
    let mut y = 1.2;
    while y < h - 1.2 {
        for x in [0.3, w - 0.3] {
            pos.push([x, y]);
            species.push(2);
        }
        y += 1.0;
    }
    let channel = o.get("channel", 0.0);
    let settle = o.get("settle", 0.0);
    if channel > 0.0 {
        let mut y = channel;
        while y < h - 1.2 {
            pos.push([w / 2.0, y]);
            species.push(2);
            y += 0.8;
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let p = [1.4 + (i as f64 + 0.5) * a, 1.0 + (j as f64 + 0.5) * a];
            if p[0] > w - 1.4 {
                continue;
            }
            if (p[0] - w / 2.0).abs() < 0.7 {
                continue;
            }
            let u = gpu_prims::threefry::uniform(seed, 6, (j * nx + i) as u32);
            pos.push(p);
            species.push((settle == 0.0 && p[0] > w / 2.0 + 0.7 && u < f) as u32);
        }
    }
    let mut vel = init::maxwell(&sys, &species, t, seed);
    for (v, s) in vel.iter_mut().zip(&species) {
        if *s == 2 {
            *v = [0, 0];
        }
    }
    let state = State { pos: pos.iter().map(|p| [fixed::to_fixed(p[0]), fixed::to_fixed(p[1])]).collect(), vel, species };
    let (warmup, finish) = if settle > 0.0 { ((settle / sys.dt) as usize, Finish::Solute { temperature: t, x_min: w / 2.0 + 0.7, fraction: f, seed }) } else { (0, Finish::None) };
    Scenario { warmup, finish, marked: 1, grid: [2, 56], radii: vec![0.5, 0.5, 0.5], sys, state }
}

/// A gas under a free piston carrying a weight (constant pressure), or under the same piston
/// locked (`locked` = 1: a huge mass, so it does not move: constant volume), for heating in
/// counted amounts. Hard walls; no gravity on the gas. Options: n, width, density (at the start),
/// temperature, mass (the piston's), locked, seed. The load is set so the gas starts balanced.
pub fn enthalpy(o: &Options) -> Scenario {
    let n = o.get("n", 1500.0) as usize;
    let width = o.get("width", 100.0);
    let t = o.get("temperature", 1.0);
    let rho = o.get("density", 0.1);
    let h0 = n as f64 / (rho * width);
    let top = 4.0 * h0;
    // The pressure of the gas at the start (nearly ideal: ρT, plus the WCA's small excess),
    // times the width, is the load that balances it.
    let force = o.get("force", rho * t * (1.0 + 0.5 * std::f64::consts::PI * rho * 0.75) * width);
    let mass = if o.get("locked", 0.0) > 0.0 { 1e12 } else { o.get("mass", 5.0) };
    let sys = System::new([fixed::to_fixed(width), fixed::to_fixed(top)], init::DT, Pair::wca(), vec![1.0])
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_piston(md_core::PistonDrive::Free { mass, force }, h0, 4.0);
    let lower = System::new([fixed::to_fixed(width), fixed::to_fixed(h0)], init::DT, Pair::wca(), vec![1.0]).with_walls(0, Wall::Soft, Wall::Soft).with_walls(1, Wall::Soft, Wall::Soft);
    let seed = o.get("seed", 1.0) as u64;
    let mut state = init::fluid(&lower, n, t, seed);
    state.vel = init::maxwell(&sys, &state.species, t, seed);
    Scenario { warmup: 0, finish: Finish::None, marked: 0, grid: [4, 4], radii: vec![0.5], sys, state }
}

/// The saturated vapour density of the 2D LJ fluid (rc 2.5) at temperature `t`: a
/// Clausius–Clapeyron fit through the Gibbs-ensemble and MD values (0.023 at kT 0.40, 0.058 at
/// 0.44; crates/mc/tests/gibbs.rs).
pub fn saturated_vapour_density(t: f64) -> f64 {
    0.023 * (-4.07 * (1.0 / t.clamp(0.3, 0.47) - 1.0 / 0.40)).exp()
}

/// Humid air over water: a pool of LJ "water" (species 0) on the floor under "air" (species 1, a
/// gas that never condenses: purely repulsive), with the water's vapour in the air at
/// `humidity` × its saturated density. A gentle bath holds the temperature; weak gravity keeps
/// the pool down; hard walls left and right. Options: width, height, depth (of the pool, σ), air
/// (density), humidity,
/// temperature, seed.
pub fn humid(o: &Options) -> Scenario {
    let (w, h) = (o.get("width", 120.0), o.get("height", 100.0));
    let depth = o.get("depth", 14.0);
    let t = o.get("temperature", 0.42);
    let seed = o.get("seed", 1.0) as u64;
    let wca = Pair::wca();
    let sys = System::new([fixed::to_fixed(w), fixed::to_fixed(h)], init::DT, Pair::lj(2.5), vec![1.0, 1.0])
        .with_pair(0, 1, wca)
        .with_pair(1, 1, wca)
        .with_walls(0, Wall::Hard, Wall::Hard)
        .with_walls(1, Wall::Soft, Wall::Soft)
        .with_gravity([0.0, -0.002])
        .with_thermostat(t, o.get("rate", 0.1));
    let n_pool = (0.75 * w * depth) as usize;
    let pool = System::new([fixed::to_fixed(w), fixed::to_fixed(depth + 2.0)], init::DT, Pair::lj(2.5), vec![1.0]).with_walls(0, Wall::Soft, Wall::Soft).with_walls(1, Wall::Soft, Wall::Soft);
    let mut state = init::fluid(&pool, n_pool, t, seed);
    // Vapour and air at random above the pool, clear of each other.
    let (y0, y1) = (depth + 3.0, h - 1.5);
    let area = w * (y1 - y0);
    let nv = (o.get("humidity", 1.0) * saturated_vapour_density(t) * area).round() as usize;
    let na = (o.get("air", 0.05) * area).round() as usize;
    let mut placed: Vec<[f64; 2]> = vec![];
    let mut k = 0u32;
    while placed.len() < nv + na && k < 200 * (nv + na) as u32 + 100 {
        let x = 1.0 + gpu_prims::threefry::uniform(seed, 51, 2 * k) * (w - 2.0);
        let y = y0 + gpu_prims::threefry::uniform(seed, 51, 2 * k + 1) * (y1 - y0);
        k += 1;
        if placed.iter().any(|q| (q[0] - x).powi(2) + (q[1] - y).powi(2) < 1.44) {
            continue;
        }
        placed.push([x, y]);
    }
    let species: Vec<u32> = (0..placed.len()).map(|i| (i >= nv) as u32).collect();
    let vel = init::maxwell(&sys, &species, t, seed + 3);
    for ((q, v), s) in placed.iter().zip(vel).zip(species) {
        state.pos.push([fixed::to_fixed(q[0]), fixed::to_fixed(q[1])]);
        state.vel.push(v);
        state.species.push(s);
    }
    Scenario { warmup: 0, finish: Finish::None, marked: 0, grid: [1, 50], radii: vec![0.5, 0.5], sys, state }
}
