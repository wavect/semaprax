//! Cleanup, resource-model, and backend-equivalence regressions.
//!
//! One harness binary for the cleanup contract and the executable evidence that
//! depends on it: the inventory and plan boundaries, the settlement and
//! concurrency proof models that read those plans, and the generated-code
//! suites that compile and run a program on every claiming backend to show the
//! same observable behavior. Each module below was its own integration test
//! binary, and every one statically linked the whole compiler, so seventeen
//! executables expressed one subject.
//!
//! The modules stay independent. Those that write temporaries derive each path
//! from a distinct literal prefix — `semaprax-aggregate-*`,
//! `semaprax-generic-function-*`, `semaprax-generic-record-*`,
//! `semaprax-generic-variant-*`, `semaprax-option-try-*`,
//! `semaprax-record-pattern-*`, `semaprax-shared-loan-*`,
//! `semaprax-try-*`, and `semaprax-variant-*` — so sharing one process id does
//! not make two modules derive the same fixture path.
//!
//! `scalar_status_backend_equivalence` is deliberately not a module here: CI
//! names it with `--test scalar_status_backend_equivalence`, so it must remain
//! its own binary.
//!
//! Every module drops the affix its former file name shared with this harness,
//! except `byte_data_capacity_v1`: it loads `src/byte_data_capacity.rs` as its
//! own child module, and the shorter name would make that child share its
//! parent's name, which `clippy::module_inception` rejects.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "cleanup_backends/arc_zones_model.rs"]
mod arc_zones_model;
#[path = "cleanup_backends/byte_data_capacity_v1.rs"]
mod byte_data_capacity_v1;
#[path = "cleanup_backends/bytes_cleanup.rs"]
mod bytes_cleanup;
#[path = "cleanup_backends/executable_aggregate.rs"]
mod executable_aggregate;
#[path = "cleanup_backends/executable_generic_function.rs"]
mod executable_generic_function;
#[path = "cleanup_backends/executable_generic_record.rs"]
mod executable_generic_record;
#[path = "cleanup_backends/executable_record_pattern.rs"]
mod executable_record_pattern;
#[path = "cleanup_backends/executable_try.rs"]
mod executable_try;
#[path = "cleanup_backends/executable_variant.rs"]
mod executable_variant;
#[path = "cleanup_backends/executor.rs"]
mod executor;
#[path = "cleanup_backends/inventory.rs"]
mod inventory;
#[path = "cleanup_backends/plan.rs"]
mod plan;
#[path = "cleanup_backends/scoped_tasks_model.rs"]
mod scoped_tasks_model;
#[path = "cleanup_backends/shared_loan_graph.rs"]
mod shared_loan_graph;
#[path = "cleanup_backends/shared_loan_hir.rs"]
mod shared_loan_hir;
#[path = "cleanup_backends/shared_loan_runtime.rs"]
mod shared_loan_runtime;
#[path = "cleanup_backends/shared_loan_source.rs"]
mod shared_loan_source;
