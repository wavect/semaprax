//! Intermediate-representation regressions: HIR resolution and validation, the
//! semantic graph projection, verifier parity, and iterative formatting.
//!
//! One harness binary for the frontend's two agent-facing representations. Each
//! module below was its own integration test binary, and every one statically
//! linked the whole compiler, so thirteen executables expressed one subject.
//! The modules are pure in-process frontend assertions over their own source
//! strings; only `hir_native` touches the filesystem, and it owns the distinct
//! `semaprax-hir-escape-` temporary prefix.
//!
//! `hir_module_boundaries` is deliberately not a module here:
//! `tests/source_locked_contracts.rs` reads it as *text* at the fixed path
//! `tests/hir_module_boundaries.rs`, so it must stay a top-level file.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "ir/formatter_iterative.rs"]
mod formatter_iterative;
#[path = "ir/graph_generics.rs"]
mod graph_generics;
#[path = "ir/graph_lifecycle.rs"]
mod graph_lifecycle;
#[path = "ir/graph_records.rs"]
mod graph_records;
#[path = "ir/graph_result_try.rs"]
mod graph_result_try;
#[path = "ir/graph_variants.rs"]
mod graph_variants;
#[path = "ir/hir.rs"]
mod hir;
#[path = "ir/hir_native.rs"]
mod hir_native;
#[path = "ir/hir_records.rs"]
mod hir_records;
#[path = "ir/hir_validation.rs"]
mod hir_validation;
#[path = "ir/hir_variants.rs"]
mod hir_variants;
#[path = "ir/hir_wasm.rs"]
mod hir_wasm;
#[path = "ir/verifier_parity.rs"]
mod verifier_parity;
