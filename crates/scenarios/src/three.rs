//! 3D scenarios, for `md::GpuEngine3`.

use crate::Options;
use md_core::three::{self, State3, System3};
use md_core::{fixed, init, Pair, Wall};

#[derive(Clone, Debug)]
pub struct Scenario3 {
    pub sys: System3,
    pub state: State3,
    /// Display radius of each species (σ).
    pub radii: Vec<f64>,
}

pub const NAMES3: &[&str] = &["crystal3", "gas3", "freeze3", "brownian3", "suspension3"];

pub fn build3(name: &str, o: &Options) -> Result<Scenario3, String> {
    let sc = match name {
        "crystal3" => crystal3(o),
        "gas3" => gas3(o),
        "freeze3" => freeze3(o),
        "brownian3" => brownian3(o),
        "suspension3" => suspension3(o),
        _ => return Err(format!("unknown 3D scenario {name}")),
    };
    for (i, p) in sc.state.pos.iter().enumerate() {
        for a in 0..3 {
            if p[a] < 0 || p[a] >= sc.sys.box_len[a] {
                return Err(format!("scenario {name}: particle {i} starts outside the box on axis {a} ({})", fixed::to_f64(p[a])));
            }
        }
    }
    Ok(sc)
}

/// A Lennard-Jones crystal (FCC, the structure LJ atoms freeze into): a cube of about `n` atoms
/// (whole unit cells) at number density `density`, in a periodic box `room` times its volume
/// (1: the crystal fills the box; more: a block surrounded by space, with free surfaces, as a
/// real crystal has).
/// Options: n, density, room, temperature, seed.
pub fn crystal3(o: &Options) -> Scenario3 {
    let n = o.get("n", 8000.0) as usize;
    let density = o.get("density", 0.96);
    let room = o.get("room", 1.0).max(1.0);
    let t = o.get("temperature", 0.5);
    let seed = o.get("seed", 1.0) as u64;
    // Whole unit cells (a perfect crystal has no gaps): c per side, 4 atoms each.
    let c = ((n as f64 / 4.0).cbrt().round() as usize).max(2);
    let n = 4 * c * c * c;
    let side = (n as f64 / density).cbrt();
    let l = side * room.cbrt();
    let sys = System3::new([fixed::to_fixed(l); 3], init::DT, Pair::lj(2.5), vec![1.0]).with_seed(seed);
    let block_len = [fixed::to_fixed(side); 3];
    let cells = [c; 3];
    let offset = block_len.map(|b| (sys.box_len[0] - b) / 2);
    let pos: Vec<[i64; 3]> = three::fcc(n, block_len, cells, 0.02, seed).into_iter().map(|p| std::array::from_fn(|k| fixed::wrap(p[k] + offset[k], sys.box_len[k]))).collect();
    let species = vec![0; n];
    let vel = three::maxwell(&sys, &species, t, seed + 1);
    Scenario3 { sys, state: State3 { pos, vel, species }, radii: vec![0.5] }
}

/// A gas of repulsive (WCA) molecules in a periodic cube, at Maxwell's distribution.
/// Options: n, density, temperature, seed.
pub fn gas3(o: &Options) -> Scenario3 {
    let n = o.get("n", 8000.0) as usize;
    let t = o.get("temperature", 1.0);
    let seed = o.get("seed", 1.0) as u64;
    let sys = System3::new(three::cube_for(n, o.get("density", 0.2)), init::DT, Pair::wca(), vec![1.0]).with_seed(seed);
    let pos = three::fcc(n, sys.box_len, three::fcc_cells(n, sys.box_len), 0.1, seed);
    let species = vec![0; n];
    let vel = three::maxwell(&sys, &species, t, seed + 1);
    Scenario3 { sys, state: State3 { pos, vel, species }, radii: vec![0.5] }
}

/// Freezing with salt: a Lennard-Jones crystal slab on the floor (y = 0; FCC, its (100) face up,
/// `cells` unit cells across and `layers` deep), its liquid above it (`liquid` σ deep), then
/// empty space up to the ceiling, so the pressure is close to zero. Species 1 is the solute
/// ("salt"): a little bigger against the solvent (σ = 1.1), so it does not fit into the crystal,
/// attracted to the solvent more strongly than the solvent to itself (ε = 1.5: it stays in the
/// liquid rather than evaporating, as salt does), and purely repulsive among itself. A heat bath holds the temperature (`rate` collisions per
/// atom per τ): this experiment needs a fixed temperature. Options: cells, layers, liquid,
/// temperature, solute (share of the liquid's atoms made solute at the start), rate, seed.
pub fn freeze3(o: &Options) -> Scenario3 {
    let cells = o.get("cells", 10.0) as usize;
    let layers = o.get("layers", 8.0) as usize;
    let t = o.get("temperature", 0.6);
    let seed = o.get("seed", 1.0) as u64;
    let a = (4.0 / 0.96f64).cbrt();
    let side = a * cells as f64;
    let crystal_h = a * layers as f64;
    let liquid_h = o.get("liquid", 14.0);
    let z0 = 0.6;
    let height = z0 + crystal_h + liquid_h + o.get("space", 20.0);
    let lj = Pair::lj(2.5);
    // y is up (as the renderer draws it): the floor and the ceiling are on y.
    let sys = System3::new([fixed::to_fixed(side), fixed::to_fixed(height), fixed::to_fixed(side)], init::DT, lj, vec![1.0, 1.0])
        .with_walls(1, Wall::Hard, Wall::Hard)
        .with_pair(0, 1, Pair { eps: 1.5, sigma: 1.1, rc: 2.5 })
        .with_pair(1, 1, Pair { eps: 1.0, sigma: 1.1, rc: 1.1 * 2f64.powf(1.0 / 6.0) })
        .with_thermostat(t, o.get("rate", 0.2))
        .with_seed(seed);
    let block = [fixed::to_fixed(side), fixed::to_fixed(side), fixed::to_fixed(crystal_h)];
    let mut pos: Vec<[i64; 3]> = three::fcc(4 * cells * cells * layers, block, [cells, cells, layers], 0.02, seed).into_iter().map(|p| [p[0], p[1], p[2] + fixed::to_fixed(z0)]).collect();
    // The liquid: a jittered simple cubic lattice at density ~0.85.
    let b = (1.0f64 / 0.85).cbrt();
    let nl = (side / b).floor() as usize;
    let bl = side / nl as f64;
    let nz = (liquid_h / b).floor() as usize;
    let zl = z0 + crystal_h + 0.5;
    let mut k = 0u32;
    for iz in 0..nz {
        for iy in 0..nl {
            for ix in 0..nl {
                let j = |q: u32| 0.1 * (gpu_prims::threefry::uniform(seed, 41, 3 * k + q) - 0.5);
                pos.push([fixed::to_fixed((ix as f64 + 0.5) * bl + j(0)), fixed::to_fixed((iy as f64 + 0.5) * bl + j(1)), fixed::to_fixed(zl + (iz as f64 + 0.5) * b + j(2))]);
                k += 1;
            }
        }
    }
    let n_crystal = 4 * cells * cells * layers;
    let share = o.get("solute", 0.0);
    let species: Vec<u32> = (0..pos.len()).map(|i| (i >= n_crystal && gpu_prims::threefry::uniform(seed, 42, i as u32) < share) as u32).collect();
    // Built with z up; turn it so y is up.
    let pos = pos.into_iter().map(|p| [p[0], p[2], p[1]]).collect();
    let vel = three::maxwell(&sys, &species, t, seed + 1);
    Scenario3 { sys, state: State3 { pos, vel, species }, radii: vec![0.5, 0.55] }
}

/// A Brownian grain: one big sphere (species 1, diameter `size` σ, as dense as the liquid, so
/// mass ∝ its volume) in a liquid of repulsive (WCA) molecules at `density`, periodic, at
/// Maxwell's distribution. Options: n, density, size, temperature, seed.
pub fn brownian3(o: &Options) -> Scenario3 {
    let n = o.get("n", 16000.0) as usize;
    let rho = o.get("density", 0.7);
    let size = o.get("size", 5.0);
    let t = o.get("temperature", 1.0);
    let seed = o.get("seed", 1.0) as u64;
    let mass = rho * std::f64::consts::PI / 6.0 * size.powi(3);
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let box_len = three::cube_for(n, rho);
    let sys = System3::new(box_len, init::DT, wca(1.0), vec![1.0, mass]).with_pair(0, 1, wca(0.5 * (size + 1.0))).with_pair(1, 1, wca(size)).with_seed(seed);
    // A lattice of solvent with the grain at the centre: remove the molecules where it goes.
    let all = three::fcc(n, box_len, three::fcc_cells(n, box_len), 0.1, seed);
    let c = box_len.map(|l| l / 2);
    let clear = fixed::to_fixed(0.5 * (size + 1.0) * 1.02);
    let mut pos = vec![c];
    pos.extend(all.into_iter().filter(|p| {
        let d2: f64 = (0..3).map(|a| fixed::to_f64(fixed::min_image(p[a], c[a], box_len[a])).powi(2)).sum();
        d2 > fixed::to_f64(clear).powi(2)
    }));
    let mut species = vec![0; pos.len()];
    species[0] = 1;
    let vel = three::maxwell(&sys, &species, t, seed + 1);
    Scenario3 { sys, state: State3 { pos, vel, species }, radii: vec![0.5, 0.5 * size] }
}

/// A suspension: `grains` big spheres (species 1, diameter `size` σ, as dense as the liquid) on a
/// loose cubic lattice in a liquid of repulsive (WCA) molecules at `density`, periodic, at
/// Maxwell's distribution; molecules where a grain goes are removed. Options: n (molecules before
/// the grains displace some), density, grains, size, temperature, seed.
pub fn suspension3(o: &Options) -> Scenario3 {
    let n = o.get("n", 16000.0) as usize;
    let rho = o.get("density", 0.7);
    let size = o.get("size", 3.0);
    let k = o.get("grains", 20.0) as usize;
    let t = o.get("temperature", 1.0);
    let seed = o.get("seed", 1.0) as u64;
    let mass = rho * std::f64::consts::PI / 6.0 * size.powi(3);
    let wca = |sigma: f64| Pair { eps: 1.0, sigma, rc: sigma * 2f64.powf(1.0 / 6.0) };
    let box_len = three::cube_for(n, rho);
    let sys = System3::new(box_len, init::DT, wca(1.0), vec![1.0, mass]).with_pair(0, 1, wca(0.5 * (size + 1.0))).with_pair(1, 1, wca(size)).with_seed(seed);
    let all = three::fcc(n, box_len, three::fcc_cells(n, box_len), 0.1, seed);
    // Grains on the first k sites of an m × m × m lattice (m³ ≥ k), shuffled a little.
    let m = (k as f64).cbrt().ceil().max(1.0) as usize;
    let l = box_len.map(fixed::to_f64);
    let grains: Vec<[i64; 3]> = (0..k)
        .map(|q| {
            let ijk = [q % m, (q / m) % m, q / (m * m)];
            std::array::from_fn(|a| fixed::to_fixed((ijk[a] as f64 + 0.5 + 0.1 * (gpu_prims::threefry::uniform(seed, 97, (3 * q + a) as u32) - 0.5)) * l[a] / m as f64))
        })
        .collect();
    let clear = (0.5 * (size + 1.0) * o.get("clear", 1.02)).powi(2);
    let mut pos = grains.clone();
    pos.extend(all.into_iter().filter(|p| {
        grains.iter().all(|g| (0..3).map(|a| fixed::to_f64(fixed::min_image(p[a], g[a], box_len[a])).powi(2)).sum::<f64>() > clear)
    }));
    let mut species = vec![0; pos.len()];
    species[..k].fill(1);
    let vel = three::maxwell(&sys, &species, t, seed + 1);
    Scenario3 { sys, state: State3 { pos, vel, species }, radii: vec![0.5, 0.5 * size] }
}
