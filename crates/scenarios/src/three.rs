//! 3D scenarios, for `md::GpuEngine3`.

use crate::Options;
use md_core::three::{self, State3, System3};
use md_core::{fixed, init, Pair};

#[derive(Clone, Debug)]
pub struct Scenario3 {
    pub sys: System3,
    pub state: State3,
    /// Display radius of each species (σ).
    pub radii: Vec<f64>,
}

pub const NAMES3: &[&str] = &["crystal3", "gas3"];

pub fn build3(name: &str, o: &Options) -> Result<Scenario3, String> {
    let sc = match name {
        "crystal3" => crystal3(o),
        "gas3" => gas3(o),
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
