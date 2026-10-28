//! WebAssembly backend and package regressions.
//!
//! One harness binary for the whole Wasm-facing test surface: the browser and
//! owned packages, the scalar and text export profiles, the internal-string
//! profiles and their web consumers, and the ordinary arithmetic lanes. Each
//! module below was its own integration test binary, and every one statically
//! linked the compiler, so the family cost nine executables to express one
//! subject. The modules stay independent: each owns a distinct fixture root
//! and asserts only over its own session.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly, and a relocated body reaches shared fixtures through
//! `../`.

#[path = "wasm/internal_strings_v1.rs"]
mod internal_strings_v1;
#[path = "wasm/internal_strings_web_v1.rs"]
mod internal_strings_web_v1;
#[path = "wasm/law_runtime_structured.rs"]
mod law_runtime_structured;
#[path = "wasm/legacy_string_admission_v1.rs"]
mod legacy_string_admission_v1;
#[path = "wasm/legacy_vec_admission_v1.rs"]
mod legacy_vec_admission_v1;
#[path = "wasm/owned.rs"]
mod owned;
#[path = "wasm/readme_web_package_commands.rs"]
mod readme_web_package_commands;
#[path = "wasm/scalar_browser_ci_contract.rs"]
mod scalar_browser_ci_contract;
#[path = "wasm/scalar_exports_v1.rs"]
mod scalar_exports_v1;
#[path = "wasm/scalar_local_layout.rs"]
mod scalar_local_layout;
#[path = "wasm/text_exports_v1.rs"]
mod text_exports_v1;
#[path = "wasm/usize_multiplication_v1.rs"]
mod usize_multiplication_v1;
#[path = "wasm/web_package.rs"]
mod web_package;

// Share the exact current-source custody receiver with the library test crate.
use semaprax::digest_hex;
#[path = "../src/source_reflows_merge/main_e177_final_inputs.rs"]
pub(crate) mod main_e177_final_inputs;
#[path = "../src/source_reflows_merge/next_big_batch_inputs.rs"]
pub(crate) mod next_big_batch_inputs;
mod source_reflows_merge {
    pub(crate) use crate::{main_e177_final_inputs, next_big_batch_inputs};
}

// Use the same real allocator/registry as the library and workspace test images.
#[allow(dead_code, unused_imports)]
#[path = "../src/test_allocation_observer/backing.rs"]
mod test_allocation_observer;
