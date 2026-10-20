//! CPU reference engines (see "Engine configurations" in `docs/architecture.md`):
//!
//! - [`IntEngine`] (`cpu-int-float`): integer positions and velocities, float forces in f32 or
//!   f64. With f32 it evaluates forces with the same operations, in the same order, as the GPU
//!   kernel.
//! - [`FloatEngine`] (`cpu-float`): conventional float MD in f32 or f64. The f64 build is the
//!   reference for observables and integrator error.
//!
//! Both use the same leapfrog: `v_{n+1/2} = v_{n-1/2} + a(x_n) Δt`, `x_{n+1} = x_n + v_{n+1/2} Δt`.

mod cells;
mod float_engine;
mod int_engine;
mod real;

pub use cells::CellList;
pub use float_engine::FloatEngine;
pub use int_engine::{Forces, IntEngine};
pub use real::Real;

/// Energies per configuration, in ε.
#[derive(Clone, Copy, Debug, Default)]
pub struct Energy {
    pub kinetic: f64,
    pub potential: f64,
}

impl Energy {
    pub fn total(&self) -> f64 {
        self.kinetic + self.potential
    }
}
