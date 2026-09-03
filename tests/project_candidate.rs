//! Project-candidate regressions: the whole `ProjectCandidate` surface.
//!
//! One harness binary for the candidate subject. Each module below was its own
//! integration test binary, and every one statically linked the compiler, so
//! the family cost fifty-nine executables to express one subject. The modules
//! stay independent: each owns a distinct temporary fixture root and asserts
//! only over its own project.
//!
//! `tests/project_candidate_publication_v1.rs` is deliberately not a module
//! here — the graph-operational evidence scripts name it with `--test`, so it
//! must remain its own binary.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "project_candidate/aggregate_expressions.rs"]
mod aggregate_expressions;
#[path = "project_candidate/analysis_artifact_evidence.rs"]
mod analysis_artifact_evidence;
#[path = "project_candidate/analysis_coverage.rs"]
mod analysis_coverage;
#[path = "project_candidate/analysis_evidence.rs"]
mod analysis_evidence;
#[path = "project_candidate/analysis_runtime_evidence.rs"]
mod analysis_runtime_evidence;
#[path = "project_candidate/archive.rs"]
mod archive;
#[path = "project_candidate/artifact_delta.rs"]
mod artifact_delta;
#[path = "project_candidate/builtin_calls.rs"]
mod builtin_calls;
#[path = "project_candidate/candidates.rs"]
mod candidates;
#[path = "project_candidate/contract_delta.rs"]
mod contract_delta;
#[path = "project_candidate/contract_holes.rs"]
mod contract_holes;
#[path = "project_candidate/data_type_declarations.rs"]
mod data_type_declarations;
#[path = "project_candidate/declaration.rs"]
mod declaration;
#[path = "project_candidate/dependency_navigation.rs"]
mod dependency_navigation;
#[path = "project_candidate/diagnostics.rs"]
mod diagnostics;
#[path = "project_candidate/draft_archive.rs"]
mod draft_archive;
#[path = "project_candidate/draft_merge.rs"]
mod draft_merge;
#[path = "project_candidate/draft_rebase.rs"]
mod draft_rebase;
#[path = "project_candidate/draft_recovery.rs"]
mod draft_recovery;
#[path = "project_candidate/expression.rs"]
mod expression;
#[path = "project_candidate/expression_holes.rs"]
mod expression_holes;
#[path = "project_candidate/extraction.rs"]
mod extraction;
#[path = "project_candidate/field_borrow_repair.rs"]
mod field_borrow_repair;
#[path = "project_candidate/field_places.rs"]
mod field_places;
#[path = "project_candidate/generic_aggregate_expressions.rs"]
mod generic_aggregate_expressions;
#[path = "project_candidate/git_publication.rs"]
mod git_publication;
#[path = "project_candidate/holes.rs"]
mod holes;
#[path = "project_candidate/impact_navigation.rs"]
mod impact_navigation;
#[path = "project_candidate/interface.rs"]
mod interface;
#[path = "project_candidate/interface_delta.rs"]
mod interface_delta;
#[path = "project_candidate/interface_rebase.rs"]
mod interface_rebase;
#[path = "project_candidate/lexical_binding.rs"]
mod lexical_binding;
#[path = "project_candidate/lexical_binding_rebase.rs"]
mod lexical_binding_rebase;
#[path = "project_candidate/literal_constructors.rs"]
mod literal_constructors;
#[path = "project_candidate/match_expressions.rs"]
mod match_expressions;
#[path = "project_candidate/member_rename.rs"]
mod member_rename;
#[path = "project_candidate/merge_preview.rs"]
mod merge_preview;
#[path = "project_candidate/movement.rs"]
mod movement;
#[path = "project_candidate/nominal_declarations.rs"]
mod nominal_declarations;
#[path = "project_candidate/nominal_extraction.rs"]
mod nominal_extraction;
#[path = "project_candidate/nominal_movement.rs"]
mod nominal_movement;
#[path = "project_candidate/nominal_rename.rs"]
mod nominal_rename;
#[path = "project_candidate/owned_block_extraction.rs"]
mod owned_block_extraction;
#[path = "project_candidate/owned_declarations.rs"]
mod owned_declarations;
#[path = "project_candidate/owned_movement.rs"]
mod owned_movement;
#[path = "project_candidate/ownership_delta.rs"]
mod ownership_delta;
#[path = "project_candidate/package_consumer_replay.rs"]
mod package_consumer_replay;
#[path = "project_candidate/rebase.rs"]
mod rebase;
#[path = "project_candidate/record_field.rs"]
mod record_field;
#[path = "project_candidate/record_projection.rs"]
mod record_projection;
#[path = "project_candidate/record_update.rs"]
mod record_update;
#[path = "project_candidate/recovery.rs"]
mod recovery;
#[path = "project_candidate/scalar_literal_constructors.rs"]
mod scalar_literal_constructors;
#[path = "project_candidate/semantic_delta.rs"]
mod semantic_delta;
#[path = "project_candidate/signature_ownership.rs"]
mod signature_ownership;
#[path = "project_candidate/source_review.rs"]
mod source_review;
#[path = "project_candidate/string_builtin_calls.rs"]
mod string_builtin_calls;
#[path = "project_candidate/testing.rs"]
mod testing;
#[path = "project_candidate/type_declarations.rs"]
mod type_declarations;
