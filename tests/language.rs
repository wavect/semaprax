//! Source-level language semantics: scalars, records, variants, generics,
//! patterns and matching, mutation, ownership, control flow, and the widened
//! Copy-scalar surface as it reaches the interop, interpreter, schema,
//! property-generation, and public Wasm scalar export projections.
//!
//! One harness binary for the language surface itself, as opposed to a
//! subsystem such as the graph, the HIR, or a backend. Each module below was
//! its own integration test binary, and every one statically linked the whole
//! compiler, so this family cost thirty-four executables to express one
//! subject.
//!
//! The modules stay independent. None loads a shared `tests/support` file, none
//! re-invokes the test binary, and each that writes fixtures derives them from a
//! distinct literal prefix plus the process id, so nothing collides now that
//! they share a process.
//!
//! Module names drop the `_v1` suffix, which marks a first and only revision;
//! `string_ops_v1` and `string_ops_v2` keep theirs because both revisions are
//! present and the bare stem would be ambiguous.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "language/character_scalars.rs"]
mod character_scalars;
#[path = "language/class_declarations.rs"]
mod class_declarations;
#[path = "language/class_inheritance.rs"]
mod class_inheritance;
#[path = "language/control_flow.rs"]
mod control_flow;
#[path = "language/explicit_mutation.rs"]
mod explicit_mutation;
#[path = "language/field_mutation.rs"]
mod field_mutation;
#[path = "language/floating_point_scalars.rs"]
mod floating_point_scalars;
#[path = "language/generic_functions.rs"]
mod generic_functions;
#[path = "language/generic_records.rs"]
mod generic_records;
#[path = "language/generic_variants.rs"]
mod generic_variants;
#[path = "language/i32_scalars.rs"]
mod i32_scalars;
#[path = "language/indexed_byte_loops_v2.rs"]
mod indexed_byte_loops_v2;
#[path = "language/interop_scalar_widen.rs"]
mod interop_scalar_widen;
#[path = "language/interpreter_scalar_widen.rs"]
mod interpreter_scalar_widen;
#[path = "language/match_mode_graph_v21.rs"]
mod match_mode_graph_v21;
#[path = "language/match_modes_syntax.rs"]
mod match_modes_syntax;
#[path = "language/option_try_semantics.rs"]
mod option_try_semantics;
#[path = "language/ownership.rs"]
mod ownership;
#[path = "language/ownership_control_flow.rs"]
mod ownership_control_flow;
#[path = "language/property_widen.rs"]
mod property_widen;
#[path = "language/record_patterns.rs"]
mod record_patterns;
#[path = "language/records_semantics.rs"]
mod records_semantics;
#[path = "language/records_syntax.rs"]
mod records_syntax;
#[path = "language/refutable_match.rs"]
mod refutable_match;
#[path = "language/resource_lifecycle.rs"]
mod resource_lifecycle;
#[path = "language/result_try_semantics.rs"]
mod result_try_semantics;
#[path = "language/schema_scalar_widen.rs"]
mod schema_scalar_widen;
#[path = "language/stable_id_nul.rs"]
mod stable_id_nul;
#[path = "language/string_ops_v1.rs"]
mod string_ops_v1;
#[path = "language/string_ops_v2.rs"]
mod string_ops_v2;
#[path = "language/string_scalars.rs"]
mod string_scalars;
#[path = "language/u8_scalars.rs"]
mod u8_scalars;
#[path = "language/variants_semantics.rs"]
mod variants_semantics;
#[path = "language/wasm_scalar_export_widen.rs"]
mod wasm_scalar_export_widen;
#[path = "language/while_loops.rs"]
mod while_loops;
