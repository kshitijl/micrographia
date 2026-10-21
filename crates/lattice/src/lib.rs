//! Lattice models of phase transitions on the GPU, for comparison with molecular dynamics:
//!
//! - [`gas::LatticeGas`]: a 2D lattice gas (molecules on the sites of a square grid, attracting
//!   their nearest neighbours) under Kawasaki Monte Carlo, which moves molecules one hop at a
//!   time and so conserves their number. Equivalent to the Ising model, so its liquid–gas
//!   coexistence curve and critical point (kT_c = 0.567 ε) are known exactly (Onsager).
//! - [`lbm::ShanChen`]: a Shan–Chen lattice Boltzmann fluid with a Carnahan–Starling equation of
//!   state: liquid and vapour with flow (bubbles rise, drops merge), and optionally heat, so a
//!   pool on a hot plate boils.

pub mod gas;
pub mod lbm;

pub const GAS_WGSL: &str = include_str!("../shaders/gas.wgsl");
pub const LBM_WGSL: &str = include_str!("../shaders/lbm.wgsl");

pub fn gas_source() -> String {
    format!("{}\n{GAS_WGSL}", gpu_prims::prims::THREEFRY_WGSL)
}

/// Every shader source in this crate, for offline validation.
pub fn shader_sources() -> Vec<(&'static str, String)> {
    vec![("gas", gas_source()), ("lbm", LBM_WGSL.to_string())]
}
