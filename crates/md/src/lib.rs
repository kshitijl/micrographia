//! GPU MD engines. For now: `gpu-int-f32` (integer positions, f32 forces), 2D, NVE, with a
//! cell list rebuilt every step.

pub mod checks;
pub mod engine;
pub mod observe;

pub use engine::GpuEngine;

pub const FIXED64_WGSL: &str = gpu_prims::prims::FIXED64_WGSL;
pub const MD_WGSL: &str = include_str!("../shaders/md.wgsl");
pub const OBSERVE_WGSL: &str = include_str!("../shaders/observe.wgsl");

pub fn md_source() -> String {
    format!("{FIXED64_WGSL}\n{MD_WGSL}")
}

pub fn observe_source() -> String {
    format!("{FIXED64_WGSL}\n{OBSERVE_WGSL}")
}

/// Every shader source in this crate, for offline validation.
pub fn shader_sources() -> Vec<(&'static str, String)> {
    vec![("md", md_source()), ("observe", observe_source())]
}
