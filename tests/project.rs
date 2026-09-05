//! Project regressions: manifests, scaffolding, transports, and the owned-value
//! product lanes.
//!
//! One harness binary for the project subject outside the candidate surface,
//! which has its own harness in `tests/project_candidate.rs`. Each module below
//! was its own integration test binary, and every one statically linked the
//! whole compiler, so the family cost fifty-five executables to express one
//! subject. The modules stay independent: each owns a distinct temporary
//! fixture root and asserts only over its own project.
//!
//! Eight project files are deliberately not modules here:
//!
//!   - `project_cli_v1`, `project_manifest_v1`,
//!     `project_product_acceptance_v1`, `project_product_acceptance_ci_contract`,
//!     `project_graph_operational_workflow_v1`,
//!     `project_graph_operational_git_workflow_v1` and
//!     `project_candidate_publication_v1` are named with `--test` by CI or by a
//!     driver under `scripts/`, so each must remain its own binary.
//!   - `project_native_rust_owned_utf8_v1` is read as *text* by
//!     `tests/public_native_rust_sdk_ci_contract.rs`, which asserts over its
//!     source at a fixed path.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

// Support modules used by more than one module of this harness are declared once
// here. Loading the same file as a module twice in one crate compiles it twice and
// yields two unrelated sets of types; modules refer to these as `crate::<name>`.
#[path = "support/flat_record_product.rs"]
mod flat_record_product;
#[path = "support/full_toolchain.rs"]
mod full_toolchain;
#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;
#[path = "support/owned_mixed_arity_product.rs"]
mod owned_mixed_arity_product;
#[path = "support/owned_npm_publication.rs"]
mod owned_npm_publication;
#[path = "support/owned_result_product.rs"]
mod owned_result_product;
#[path = "support/owned_tuple_product.rs"]
mod owned_tuple_product;

#[path = "project/add_fetch_v1.rs"]
mod add_fetch_v1;
#[path = "project/agent_transport_rename.rs"]
mod agent_transport_rename;
#[path = "project/agent_transport_v2.rs"]
mod agent_transport_v2;
#[path = "project/agent_transport_v5.rs"]
mod agent_transport_v5;
#[path = "project/agent_transport_v6.rs"]
mod agent_transport_v6;
#[path = "project/agent_transport_v6_sdk.rs"]
mod agent_transport_v6_sdk;
#[path = "project/agent_workflow.rs"]
mod agent_workflow;
#[path = "project/backend_equivalence.rs"]
mod backend_equivalence;
#[path = "project/borrowed_bytes_call_interpreter.rs"]
mod borrowed_bytes_call_interpreter;
#[path = "project/class_project.rs"]
mod class_project;
#[path = "project/command_argument_borrow.rs"]
mod command_argument_borrow;
#[path = "project/cxx_owned_data_package.rs"]
mod cxx_owned_data_package;
#[path = "project/dependency_resolution_v1.rs"]
mod dependency_resolution_v1;
#[path = "project/developer_loop.rs"]
mod developer_loop;
#[path = "project/draft_expression_catalog.rs"]
mod draft_expression_catalog;
#[path = "project/draft_field_display_rebase.rs"]
mod draft_field_display_rebase;
#[path = "project/flat_owned_record_api.rs"]
mod flat_owned_record_api;
#[path = "project/flat_owned_record_interpreter.rs"]
mod flat_owned_record_interpreter;
#[path = "project/frontend_cache.rs"]
mod frontend_cache;
#[path = "project/hole_fill_suggestions.rs"]
mod hole_fill_suggestions;
#[path = "project/hole_navigation.rs"]
mod hole_navigation;
#[path = "project/language_command_native.rs"]
mod language_command_native;
#[path = "project/line_command_native.rs"]
mod line_command_native;
#[path = "project/manifest_hints.rs"]
mod manifest_hints;
#[path = "project/manifest_v10.rs"]
mod manifest_v10;
#[path = "project/manifest_v11.rs"]
mod manifest_v11;
#[path = "project/manifest_v12.rs"]
mod manifest_v12;
#[path = "project/manifest_v13.rs"]
mod manifest_v13;
#[path = "project/manifest_v4.rs"]
mod manifest_v4;
#[path = "project/manifest_v5.rs"]
mod manifest_v5;
#[path = "project/manifest_v6.rs"]
mod manifest_v6;
#[path = "project/manifest_v7.rs"]
mod manifest_v7;
#[path = "project/manifest_v8.rs"]
mod manifest_v8;
#[path = "project/native_publication.rs"]
mod native_publication;
#[path = "project/native_rust_owned_data.rs"]
mod native_rust_owned_data;
#[path = "project/native_rust_scalar_callback.rs"]
mod native_rust_scalar_callback;
#[path = "project/nested_owned_record_api.rs"]
mod nested_owned_record_api;
#[path = "project/nested_owned_record_native.rs"]
mod nested_owned_record_native;
#[path = "project/nested_owned_record_npm.rs"]
mod nested_owned_record_npm;
#[path = "project/new_cli.rs"]
mod new_cli;
#[path = "project/owned_bytes_npm.rs"]
mod owned_bytes_npm;
#[path = "project/owned_failure_fsm.rs"]
mod owned_failure_fsm;
#[path = "project/owned_inactive_cleanup.rs"]
mod owned_inactive_cleanup;
#[path = "project/owned_input_admission.rs"]
mod owned_input_admission;
#[path = "project/owned_mixed_arity.rs"]
mod owned_mixed_arity;
#[path = "project/owned_mixed_arity_interpreter.rs"]
mod owned_mixed_arity_interpreter;
#[path = "project/owned_record_field_addition.rs"]
mod owned_record_field_addition;
#[path = "project/owned_result_extrema.rs"]
mod owned_result_extrema;
#[path = "project/owned_tuple_npm.rs"]
mod owned_tuple_npm;
#[path = "project/owned_utf8_capacity.rs"]
mod owned_utf8_capacity;
#[path = "project/owned_utf8_interpreter.rs"]
mod owned_utf8_interpreter;
#[path = "project/owned_utf8_lifetimes.rs"]
mod owned_utf8_lifetimes;
#[path = "project/owned_utf8_npm.rs"]
mod owned_utf8_npm;
#[path = "project/package_manifest_v1.rs"]
mod package_manifest_v1;
#[path = "project/profile_admission.rs"]
mod profile_admission;
#[path = "project/project_local_aggregates.rs"]
mod project_local_aggregates;
#[path = "project/project_lock_v1.rs"]
mod project_lock_v1;
#[path = "project/resource_free_record_evolution.rs"]
mod resource_free_record_evolution;
#[path = "project/retained_owned_api.rs"]
mod retained_owned_api;
#[path = "project/scaffold.rs"]
mod scaffold;
#[path = "project/scaffold_cli.rs"]
mod scaffold_cli;
#[path = "project/scalar_wit_interface.rs"]
mod scalar_wit_interface;
#[path = "project/semantic_cache.rs"]
mod semantic_cache;
#[path = "project/semantic_image_cli.rs"]
mod semantic_image_cli;
#[path = "project/signature_argument_expressions.rs"]
mod signature_argument_expressions;
#[path = "project/signature_catalog.rs"]
mod signature_catalog;
#[path = "project/signature_named_copy.rs"]
mod signature_named_copy;
#[path = "project/signature_nominal_arguments.rs"]
mod signature_nominal_arguments;
#[path = "project/signature_nominal_rebase.rs"]
mod signature_nominal_rebase;
#[path = "project/signature_owned_values.rs"]
mod signature_owned_values;
#[path = "project/standard_library.rs"]
mod standard_library;
#[path = "project/v10_recipe_consumer.rs"]
mod v10_recipe_consumer;
#[path = "project/v1_ci_contract.rs"]
mod v1_ci_contract;
#[path = "project/v8_promotion_receipt.rs"]
mod v8_promotion_receipt;
#[path = "project/v9_recipe_consumer.rs"]
mod v9_recipe_consumer;
#[path = "project/v9_recipe_identity.rs"]
mod v9_recipe_identity;
