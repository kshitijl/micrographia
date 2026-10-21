//! Lattice models of phase transitions on the GPU, for comparison with molecular dynamics:
//!
//! - [`gas::LatticeGas`]: a 2D lattice gas (molecules on the sites of a square grid, attracting
//!   their nearest neighbours) under Kawasaki Monte Carlo, which moves molecules one hop at a
//!   time and so conserves their number. Equivalent to the Ising model, so its liquid–gas
//!   coexistence curve and critical point (kT_c = 0.567 ε) are known exactly (Onsager).

pub mod gas;

pub const GAS_WGSL: &str = include_str!("../shaders/gas.wgsl");

pub fn gas_source() -> String {
    format!("{}\n{GAS_WGSL}", gpu_prims::prims::THREEFRY_WGSL)
}

/// Every shader source in this crate, for offline validation.
pub fn shader_sources() -> Vec<(&'static str, String)> {
    vec![("gas", gas_source())]
}
