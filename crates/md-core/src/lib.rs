//! Definitions shared by every MD engine configuration (see `docs/architecture.md`):
//! the fixed-point units, the system description, the state, initial conditions, the pair
//! potential and the state hash.
//!
//! Units are reduced Lennard-Jones units: lengths in σ, energies in ε, masses in m, time in
//! τ = σ √(m/ε), and k_B = 1.

pub mod fixed;
pub mod init;
pub mod pair;
pub mod system;

pub use fixed::{FRAC_BITS, ONE};
pub use pair::Pair;
pub use system::{State, System};
