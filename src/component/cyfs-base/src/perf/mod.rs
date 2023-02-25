mod perf;

pub use perf::*;

#[cfg(not(feature = "perf"))]
mod dummy;

#[cfg(not(feature = "perf"))]
pub use dummy::*;

#[cfg(feature = "perf")]
mod auxi;

#[cfg(feature = "perf")]
pub use auxi::*;