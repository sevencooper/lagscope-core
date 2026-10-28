//! The shared engine of the LagScope desktop app and the LagScope Pulse website: analysis
//! rules, scoring and the test protocol's data. See README.md for what belongs here and how
//! the two products use it. No platform code: builds for Windows, Linux and WebAssembly.

pub mod beat;
pub mod design;
pub mod quality;
pub mod stalls;
pub mod stream;
