//! Monte Carlo methods for particles (off-lattice).
//!
//! - [`gibbs::Gibbs`]: Gibbs-ensemble Monte Carlo (Panagiotopoulos 1987) for liquid–vapour
//!   coexistence of the 2D Lennard-Jones fluid the molecular-dynamics pages use (cut and shifted
//!   at 2.5 σ): two boxes with no interface between them, which exchange volume and molecules
//!   until they hold the coexisting liquid and vapour.
//!
//! These run on the CPU (in the browser, as WebAssembly). Gibbs-ensemble moves do not
//! parallelise at the sizes the method needs (a few hundred to a few thousand molecules): its
//! molecule transfers and volume changes are sequential, and each transfer is one test
//! molecule's energy, ~20 pair terms. On the GPU each would cost a ~9 µs dispatch, against
//! ~0.2 µs on one CPU core.

pub mod gibbs;
