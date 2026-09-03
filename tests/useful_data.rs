//! The useful-data product surface: whole example projects carried end to end
//! through the interpreter, the native backend, Wasm and the command lanes,
//! together with the bounded command-I/O and transcript contracts they rely on.
//!
//! One harness binary for the product-surface subject. Each module below was its
//! own integration test binary and every one statically linked the whole
//! compiler, so the family cost fourteen executables to express one subject. The
//! modules stay independent: each derives its own temporary fixture root from a
//! distinct literal prefix and asserts only over its own copy of the example
//! project it exercises.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "useful_data/aggregate_contract_failure_lanes.rs"]
mod aggregate_contract_failure_lanes;
#[path = "useful_data/arrays_bytes_frontend.rs"]
mod arrays_bytes_frontend;
#[path = "useful_data/bounded_language_command_io.rs"]
mod bounded_language_command_io;
#[path = "useful_data/bounded_stdout_transcript.rs"]
mod bounded_stdout_transcript;
#[path = "useful_data/command_v1.rs"]
mod command_v1;
#[path = "useful_data/command_v2.rs"]
mod command_v2;
#[path = "useful_data/config_validator_project.rs"]
mod config_validator_project;
#[path = "useful_data/interpreter.rs"]
mod interpreter;
#[path = "useful_data/language_command_io_native.rs"]
mod language_command_io_native;
#[path = "useful_data/line_command_io_native.rs"]
mod line_command_io_native;
#[path = "useful_data/line_filter_project_v7.rs"]
mod line_filter_project_v7;
#[path = "useful_data/native.rs"]
mod native;
#[path = "useful_data/project.rs"]
mod project;
#[path = "useful_data/wasm.rs"]
mod wasm;
