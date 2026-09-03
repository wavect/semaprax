//! Semantic-image v5 protocol regressions outside the transport surface.
//!
//! One harness binary for the remaining v5 image subjects: candidate reads,
//! draft and hole navigation, retained subjects, source review, and the
//! constructor families. Each module below was its own integration test
//! binary, and every one statically linked the compiler, so the family cost
//! thirteen executables to express one subject. The modules stay independent:
//! each owns a distinct fixture root and asserts only over its own session.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "image_v5/candidate_analysis_coverage.rs"]
mod candidate_analysis_coverage;
#[path = "image_v5/candidate_dependency_navigation.rs"]
mod candidate_dependency_navigation;
#[path = "image_v5/candidate_function_facets.rs"]
mod candidate_function_facets;
#[path = "image_v5/candidate_impact_navigation.rs"]
mod candidate_impact_navigation;
#[path = "image_v5/candidate_merge_preview.rs"]
mod candidate_merge_preview;
#[path = "image_v5/draft_expression_catalog.rs"]
mod draft_expression_catalog;
#[path = "image_v5/function_instances.rs"]
mod function_instances;
#[path = "image_v5/hole_fill_suggestions.rs"]
mod hole_fill_suggestions;
#[path = "image_v5/hole_navigation.rs"]
mod hole_navigation;
#[path = "image_v5/literal_constructors.rs"]
mod literal_constructors;
#[path = "image_v5/retained_subjects.rs"]
mod retained_subjects;
#[path = "image_v5/source_review.rs"]
mod source_review;
#[path = "image_v5/string_builtin_constructors.rs"]
mod string_builtin_constructors;
