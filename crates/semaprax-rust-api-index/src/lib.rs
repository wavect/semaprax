#![forbid(unsafe_code)]

// Keep private toolchain consumers on the same replay implementation that is
// packaged with the public semaprax library.
#[path = "../../../src/rust_api_index/mod.rs"]
mod shared;

pub use shared::*;
