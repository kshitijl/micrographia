//! GPU MD engines. For now: `gpu-int-f32` (integer positions, f32 forces), 2D, NVE, with
//! Verlet neighbour lists (or a cell list rebuilt every step, for comparison).

pub mod checks;
pub mod engine;
pub mod observe;

pub use engine::{ForceMode, GpuEngine};

pub const FIXED64_WGSL: &str = gpu_prims::prims::FIXED64_WGSL;
pub const THREEFRY_WGSL: &str = gpu_prims::prims::THREEFRY_WGSL;
pub const PARAMS_WGSL: &str = include_str!("../shaders/params.wgsl");
pub const BOUNDARY_WGSL: &str = include_str!("../shaders/boundary.wgsl");
pub const MD_WGSL: &str = include_str!("../shaders/md.wgsl");
pub const REORDER_WGSL: &str = include_str!("../shaders/reorder.wgsl");
pub const NEIGHBORS_WGSL: &str = include_str!("../shaders/neighbors.wgsl");
pub const OBSERVE_WGSL: &str = include_str!("../shaders/observe.wgsl");

pub fn md_source() -> String {
    format!("{FIXED64_WGSL}\n{THREEFRY_WGSL}\n{PARAMS_WGSL}\n{BOUNDARY_WGSL}\n{MD_WGSL}")
}

pub fn reorder_source() -> String {
    format!("{FIXED64_WGSL}\n{PARAMS_WGSL}\n{REORDER_WGSL}")
}

pub fn neighbors_source() -> String {
    format!("{FIXED64_WGSL}\n{THREEFRY_WGSL}\n{PARAMS_WGSL}\n{BOUNDARY_WGSL}\n{NEIGHBORS_WGSL}")
}

pub fn observe_source() -> String {
    format!("{FIXED64_WGSL}\n{OBSERVE_WGSL}")
}

/// Every shader source in this crate, for offline validation.
pub fn shader_sources() -> Vec<(&'static str, String)> {
    vec![("md", md_source()), ("reorder", reorder_source()), ("neighbors", neighbors_source()), ("observe", observe_source())]
}
