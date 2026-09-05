//! Owned and borrowed data ABI: byte records and variants, borrowed strings,
//! owned UTF-8 and usize products, across the interpreter, native and Wasm
//! backends.

// `owned_npm_publication` is shared with the `project` harness and reaches
// `full_toolchain` through `crate::`, so this root provides it too. Both are
// Windows-only there, and `full_toolchain` reaches `native_rust_cargo` via
// `super::`, which resolves here at the crate root.
#[cfg(windows)]
#[path = "support/full_toolchain.rs"]
mod full_toolchain;
#[cfg(windows)]
#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;
#[path = "support/owned_npm_publication.rs"]
mod owned_npm_publication;

#[path = "owned_data/borrowed_str.rs"]
mod borrowed_str;
#[path = "owned_data/borrowed_str_native.rs"]
mod borrowed_str_native;
#[path = "owned_data/browser_fixture.rs"]
mod browser_fixture;
#[path = "owned_data/byte_record_algebra.rs"]
mod byte_record_algebra;
#[path = "owned_data/byte_record_interpreter.rs"]
mod byte_record_interpreter;
#[path = "owned_data/byte_record_native.rs"]
mod byte_record_native;
#[path = "owned_data/byte_record_wasm.rs"]
mod byte_record_wasm;
#[path = "owned_data/byte_variant_cleanup_graph.rs"]
mod byte_variant_cleanup_graph;
#[path = "owned_data/byte_variant_frontend_hir.rs"]
mod byte_variant_frontend_hir;
#[path = "owned_data/byte_variant_interpreter.rs"]
mod byte_variant_interpreter;
#[path = "owned_data/byte_variant_native.rs"]
mod byte_variant_native;
#[path = "owned_data/byte_variant_wasm.rs"]
mod byte_variant_wasm;
#[path = "owned_data/concrete_generic_owned_record_update.rs"]
mod concrete_generic_owned_record_update;
#[path = "owned_data/generic_owned_function_runtime.rs"]
mod generic_owned_function_runtime;
#[path = "owned_data/interpreter.rs"]
mod interpreter;
#[path = "owned_data/nested_generic_owned_record_frontend_hir.rs"]
mod nested_generic_owned_record_frontend_hir;
#[path = "owned_data/nested_generic_owned_record_runtime.rs"]
mod nested_generic_owned_record_runtime;
#[path = "owned_data/nested_owned_record_frontend_hir.rs"]
mod nested_owned_record_frontend_hir;
#[path = "owned_data/nested_owned_record_runtime.rs"]
mod nested_owned_record_runtime;
#[path = "owned_data/nested_owned_record_update_frontend_hir.rs"]
mod nested_owned_record_update_frontend_hir;
#[path = "owned_data/projected_bytes_borrowed_call_native.rs"]
mod projected_bytes_borrowed_call_native;
#[path = "owned_data/public_utf8_api.rs"]
mod public_utf8_api;
#[path = "owned_data/useful_data_usize.rs"]
mod useful_data_usize;
#[path = "owned_data/usize_mul.rs"]
mod usize_mul;
