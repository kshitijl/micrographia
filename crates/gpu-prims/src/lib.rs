//! GPU primitives shared by every engine: reduce, scan, counting sort, compaction, two-word
//! 64-bit integers and the Threefry counter-based RNG, each with a CPU twin and self-checks.

pub mod checks;
pub mod context;
pub mod fixed64;
pub mod prims;
pub mod threefry;
pub mod timing;
pub mod util;

pub use context::{Gpu, GpuError};
pub use prims::Prims;
