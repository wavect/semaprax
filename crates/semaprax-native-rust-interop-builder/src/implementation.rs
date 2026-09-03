// Private Native Rust Interoperability v1 preparation and static-bundle lane.
// Compiled only by the unpublished interop crate during private A+B.
//
// This file is the module index and the shared vocabulary: the bounded limits,
// the specification and subject shapes, the post-HIR fact types, the opaque
// phase-A and phase-B fact carriers, and the two public entry points. The work
// itself lives in the submodules declared below, which divide it by concern --
// capacity proofs, canonical input, HIR analysis, artifact projection, the
// builder ledger, toolchain authentication, manifest projection, and the
// staged phase-B build that alone holds physical platform authority.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::platform;

use crate::ast::Program;
use crate::diagnostic::{quote_json, Diagnostic};

mod artifacts;
mod authority;
mod bundle_facts;
mod canonical_input;
mod capacity;
mod disposal;
mod exact_replay;
mod facts_capacity;
mod harness;
mod hir_analysis;
mod ledger;
mod manifest;
mod observability;
mod phase_a;
mod phase_b;
mod platform_stage;
mod stages;
mod toolchain;

use authority::*;
use bundle_facts::*;
use disposal::*;
use facts_capacity::*;
use harness::*;
use ledger::*;
use manifest::*;
use observability::*;
use phase_b::*;
use platform_stage::*;
use stages::*;
use toolchain::*;

use artifacts::{
    c_expression_shape, generate_c, generate_header, generate_rust_artifacts, render_descriptor,
    render_descriptor_for_subject, replay_descriptor, replay_descriptor_for_subject,
    replay_generated_exact, replay_limits_exact, C_EXPRESSION_FRAME_BYTES,
    REPLAY_C_EXPRESSION_FRAME_BYTES,
};
#[cfg(test)]
use artifacts::{
    capability_digest, generate_c_into, generate_header_with_limit,
    generate_rust_artifacts_with_limit, render_descriptor_with_limit, replay_capabilities_digest,
    replay_generated, replay_spec_bytes_exact, replay_symbol_hash,
};
use canonical_input::*;
use exact_replay::ExactReplay;
use hir_analysis::*;
use phase_a::{prepare_native_rust_interop_bounded, prepare_project_native_rust_interop_bounded};
#[cfg(test)]
use phase_a::{
    prepare_native_rust_interop_with_test_limit, validate_native_unit_discard_bindings,
    validate_selected_scalar_closure,
};

use capacity::{
    ast_child, hir_owned_capacity, hir_pre_resolve_capacity, scan_ast_capacity,
    validate_native_rust_expression_budget_for_closure,
    validate_native_rust_source_expression_budget,
};
#[cfg(test)]
use capacity::{
    cleanup_function_exit_events, cleanup_parameter_finalizer_events, cleanup_source_exit_events,
    declaration_dag_expansion, generic_function_instance_identity_upper,
    hir_capacity_terms_for_test, validate_native_rust_expression_budget, HirPreResolveCapacity,
};

use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedImport, ResolvedImportFailure, ResolvedImportResultKind, ResolvedProgram,
    ResolvedStatement, ResolvedType,
};

const SPEC_SCHEMA: &str = "semaprax.native-rust-interop-spec.v1";
const DESCRIPTOR_SCHEMA: &str = "semaprax.native-rust-interop-descriptor.v1";
const BUNDLE_SCHEMA: &str = "semaprax.native-rust-interop-bundle.v1";
const PROJECT_SUBJECT_SCHEMA: &str = "semaprax.project-native-rust-subject.v1";
const PROJECT_SCHEMA: &str = "semaprax.project.v1";
const PROJECT_GRAPH_SCHEMA: &str = "semaprax.project-semantic-graph.v1";
const PROJECT_DESCRIPTOR_SCHEMA: &str = "semaprax.project-native-rust-interop-descriptor.v1";
const PROJECT_BUNDLE_SCHEMA: &str = "semaprax.project-native-rust-interop-bundle.v1";
const SOURCE_DOMAIN: &[u8] = b"semaprax.native-rust-interop.source-revision.v1\0";
const PROJECT_SUBJECT_DOMAIN: &[u8] = b"semaprax.project-native-rust-interop.subject.v1\0";
const HIR_DOMAIN: &[u8] = b"semaprax.native-rust-interop.hir-digest.v1\0";
const SPEC_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-interop.spec-digest.v1\0";
const DESCRIPTOR_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-interop.descriptor-digest.v1\0";
const PROJECT_DESCRIPTOR_DIGEST_DOMAIN: &[u8] =
    b"semaprax.project-native-rust-interop.descriptor-digest.v1\0";
const CALL_DOMAIN: &[u8] = b"semaprax.native-rust-interop.call-contract.v1\0";
const BUNDLE_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-interop.bundle-digest.v1\0";
const PROJECT_BUNDLE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.project-native-rust-interop.bundle-digest.v1\0";
const CAPABILITIES_DOMAIN: &[u8] = b"semaprax.native-rust-interop.capabilities.v1\0";

const MAX_EXPORTS: usize = 32;
const MAX_IMPORTS: usize = 32;
const MAX_PARAMETERS: usize = 8;
const MAX_CLOSURE_FUNCTIONS: usize = 256;
const MAX_STATUS_DOMAINS: usize = 64;
const MAX_EFFECTS: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_SOURCE_BYTES: usize = 16_777_216;
const MAX_SPEC_BYTES: usize = 1_048_576;
const MAX_DESCRIPTOR_BYTES: usize = 1_048_576;
const MAX_GENERATED_C_BYTES: usize = 4_194_304;
const MAX_GENERATED_HEADER_BYTES: usize = 1_048_576;
const MAX_GENERATED_RUST_BYTES: usize = 4_194_304;
const MAX_MANIFEST_BYTES: usize = 1_048_576;
const MAX_BUILDER_BYTES: usize = 33_554_432;
const SHA256_TEXT_BYTES: usize = "sha256:".len() + 64;
const PHASE_B_STAGE_NAME_CAPACITY: usize = 96;
const FINGERPRINT_ACTION_SLOTS: usize = MAX_SEMANTIC_EXPRESSION_DEPTH * 4 + 8;
const MAX_FORMAT_NESTING: usize = MAX_SEMANTIC_EXPRESSION_DEPTH + 1;

const MAX_JSON_DEPTH: usize = 8;
const MAX_SEMANTIC_EXPRESSION_DEPTH: usize = 512;
const MAX_CALL_DEPTH: usize = 32;
const MAX_CALLS_PER_BRIDGE: usize = 4_096;
const LIMIT_ROWS: [(&str, usize); 20] = [
    ("max_exports", MAX_EXPORTS),
    ("max_imports", MAX_IMPORTS),
    ("max_parameters", MAX_PARAMETERS),
    ("max_closure_functions", MAX_CLOSURE_FUNCTIONS),
    ("max_status_domains", MAX_STATUS_DOMAINS),
    ("max_effects", MAX_EFFECTS),
    ("max_identifier_bytes", MAX_IDENTIFIER_BYTES),
    ("max_source_bytes", MAX_SOURCE_BYTES),
    ("max_spec_bytes", MAX_SPEC_BYTES),
    ("max_descriptor_bytes", MAX_DESCRIPTOR_BYTES),
    ("max_generated_c_bytes", MAX_GENERATED_C_BYTES),
    ("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES),
    ("max_generated_rust_bytes", MAX_GENERATED_RUST_BYTES),
    ("max_manifest_bytes", MAX_MANIFEST_BYTES),
    ("max_builder_bytes", MAX_BUILDER_BYTES),
    ("max_json_depth", MAX_JSON_DEPTH),
    (
        "max_semantic_expression_depth",
        MAX_SEMANTIC_EXPRESSION_DEPTH,
    ),
    ("max_call_depth", MAX_CALL_DEPTH),
    ("max_calls_per_bridge", MAX_CALLS_PER_BRIDGE),
    ("max_unexpected_inventory_entries", 0),
];

const NONCLAIMS: &[&str] = &[
    "no_resource_owned_borrow_shared_or_aggregate_abi",
    "no_pointer_reference_slice_string_trait_object_or_generic_abi",
    "no_cross_boundary_allocator_or_deallocator",
    "no_wasm_component_or_canonical_abi_detour",
    "no_dynamic_loading_symbol_lookup_unload_or_hot_reload",
    "no_public_execution_or_spx_b104_change_in_private_ab",
    "no_callable_v2_v3_proof_bundle_or_loader_wire_change",
    "no_graph_schema_api_kat_or_semantic_projection_change",
    "no_agent_runtime_economic_workspace_or_patch_wire_change",
    "no_untrusted_native_code_sandbox_or_memory_safety",
    "no_same_uid_process_signal_or_task_port_isolation",
    "no_same_uid_active_filesystem_mutation_or_namespace_race_isolation",
    "no_same_user_process_handle_or_thread_resume_isolation",
    "no_abi_compatibility_outside_exact_descriptor_target_toolchain",
    "no_cross_target_cross_toolchain_or_cross_build_bundle_reuse",
    "no_panic_or_unwind_across_ffi",
    "no_abort_oom_stack_overflow_signal_seh_or_process_crash_recovery",
    "no_power_loss_durability_or_crash_atomicity",
    "no_async_reentrant_parallel_cross_thread_or_send_sync_bridge",
    "no_host_capability_provenance_or_os_authority",
    "no_ambient_effect_capability_or_callback_discovery",
    "no_host_error_text_panic_payload_secret_or_pointer_evidence",
    "no_exactly_once_external_effect",
    "no_exception_cpp_rust_unwind_translation",
    "no_dynamic_library_code_signing_supply_chain_or_linker_provenance",
    "no_dynamic_dependency_identity_or_filesystem_race_isolation",
    "no_c_cpp_objective_c_swift_kotlin_jni_or_other_ecosystem_binding",
    "no_stable_rust_abi_claim_beyond_generated_c_abi_wrapper",
    "no_public_cli_package_registry_or_build_script_network",
    "no_general_interop_or_production_readiness",
    "no_completion_matrix_status_promotion",
];

#[derive(Clone)]
struct Spec {
    module: String,
    source_revision: Option<String>,
    target: Target,
    exports: Vec<String>,
    imports: Vec<String>,
    capabilities: Vec<String>,
}

impl Spec {
    fn source_revision(&self) -> Option<&str> {
        self.source_revision.as_deref()
    }
}

#[derive(Clone)]
struct ProjectSubjectSource {
    path: String,
    source_graph_schema: String,
    source_revision: String,
    source_digest: String,
    bytes: usize,
}

#[derive(Clone)]
struct ProjectSubjectExport {
    stable_id: String,
    module: String,
    path: String,
}

struct ProjectSubject {
    name: String,
    manifest_bytes: usize,
    manifest_digest: String,
    manifest_canonical: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    entry_module: String,
    sources: Vec<ProjectSubjectSource>,
    exports: Vec<ProjectSubjectExport>,
    imports: Vec<String>,
    capabilities: Vec<String>,
}

#[derive(Clone, Copy)]
enum DescriptorSubject<'a> {
    SourceRevision(&'a str),
    ProjectSubjectDigest(&'a str),
}

impl<'a> DescriptorSubject<'a> {
    fn schema(self) -> &'static str {
        match self {
            Self::SourceRevision(_) => DESCRIPTOR_SCHEMA,
            Self::ProjectSubjectDigest(_) => PROJECT_DESCRIPTOR_SCHEMA,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::SourceRevision(_) => "source_revision",
            Self::ProjectSubjectDigest(_) => "project_subject_digest",
        }
    }

    fn value(self) -> &'a str {
        match self {
            Self::SourceRevision(value) | Self::ProjectSubjectDigest(value) => value,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct Target {
    triple: String,
    pointer_width: u32,
    endian: String,
    panic_strategy: String,
    thread_policy: String,
}

#[derive(Clone)]
struct ParameterFact {
    name: String,
    ty: ScalarType,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ScalarType {
    Unit,
    I64,
    Bool,
}

#[derive(Clone)]
struct ExportFact {
    id: String,
    rust_method: String,
    c_symbol: String,
    parameters: Vec<ParameterFact>,
    result: ScalarType,
    effects: Vec<String>,
    capabilities: Vec<String>,
    required_imports: Vec<String>,
    status_domain_ordinals: Vec<u16>,
    call_contract_digest: String,
}

#[derive(Clone)]
struct ImportFact {
    id: String,
    interface: String,
    import_key: String,
    rust_method: String,
    c_field: String,
    parameters: Vec<ParameterFact>,
    result: ScalarType,
    effects: Vec<String>,
    capabilities: Vec<String>,
    failure: Option<String>,
    call_contract_digest: String,
}

/// Opaque private phase-A facts. Fields intentionally have no getters before C.
pub(crate) struct PreparedNativeRustInterop {
    canonical_spec: String,
    spec_digest: String,
    descriptor: String,
    descriptor_digest: String,
    source_revision: Option<String>,
    project_subject_digest: Option<String>,
    hir_digest: String,
    target: Target,
    exports: Vec<ExportFact>,
    imports: Vec<ImportFact>,
    closure: Vec<String>,
    generated_c: String,
    generated_header: String,
    generated_rust: String,
    private_ffi_source: String,
}

impl PreparedNativeRustInterop {
    fn is_project(&self) -> bool {
        self.project_subject_digest.is_some()
    }
    fn descriptor_schema(&self) -> &'static str {
        if self.is_project() {
            PROJECT_DESCRIPTOR_SCHEMA
        } else {
            DESCRIPTOR_SCHEMA
        }
    }
    fn bundle_schema(&self) -> &'static str {
        if self.is_project() {
            PROJECT_BUNDLE_SCHEMA
        } else {
            BUNDLE_SCHEMA
        }
    }
    pub(crate) fn canonical_spec(&self) -> &str {
        &self.canonical_spec
    }
    pub(crate) fn spec_digest(&self) -> &str {
        &self.spec_digest
    }
    pub(crate) fn descriptor(&self) -> &str {
        &self.descriptor
    }
    pub(crate) fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
    pub(crate) fn source_revision(&self) -> Option<&str> {
        self.source_revision.as_deref()
    }
    pub(crate) fn project_subject_digest(&self) -> Option<&str> {
        self.project_subject_digest.as_deref()
    }
    pub(crate) fn hir_digest(&self) -> &str {
        &self.hir_digest
    }
    pub(crate) fn target_triple(&self) -> &str {
        &self.target.triple
    }
    pub(crate) fn generated_c(&self) -> &str {
        &self.generated_c
    }
    pub(crate) fn generated_header(&self) -> &str {
        &self.generated_header
    }
    pub(crate) fn generated_rust(&self) -> &str {
        &self.generated_rust
    }
    pub(crate) fn private_ffi_source(&self) -> &str {
        &self.private_ffi_source
    }
    pub(crate) fn closure(&self) -> &[String] {
        &self.closure
    }
}

/// Opaque private phase-B facts. No execution or loader handle escapes.
pub(crate) struct NativeRustInteropBundleFacts {
    output_directory: PathBuf,
    object_path: PathBuf,
    descriptor_path: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: String,
    descriptor_digest: String,
}

/// Pure private phase-A preparation. It performs no filesystem, process, or
/// network operation.
pub(crate) fn prepare_native_rust_interop(
    program: &Program,
    spec_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Vec<Diagnostic>> {
    phase_a::prepare_native_rust_interop(program, spec_bytes)
}

// Deterministic artifact generation and independent exact replay are isolated
// in `implementation/artifacts.rs`; no physical Phase-B authority crosses it.

pub(crate) fn build_native_rust_interop_bundle(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    reset_phase_b_error_materialization_observer();
    let (result, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        let mut hook = |_, _: &Path, _: &Path, _: &Path| {};
        build_native_rust_interop_bundle_bounded(program, spec_bytes, output, &mut hook)
    });
    finish_bounded_bundle(result, overflowed)
}

pub(crate) fn build_project_native_rust_interop_bundle(
    program: &ResolvedProgram,
    project_subject_bytes: &[u8],
    output: &Path,
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    reset_phase_b_error_materialization_observer();
    let (result, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        let prepared = prepare_project_native_rust_interop_bounded(program, project_subject_bytes)?;
        let phase = prepare_phase_b_from_prepared(prepared, output)?;
        let mut hook = |_, _: &Path, _: &Path, _: &Path| {};
        build_prepared_phase_b_bounded(phase, output, &mut hook)
    });
    finish_bounded_bundle(result, overflowed)
}

#[cfg(test)]
fn build_native_rust_interop_bundle_with_test_limit(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
    limit: usize,
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    assert!(limit <= MAX_BUILDER_BYTES);
    reset_phase_b_error_materialization_observer();
    let (result, overflowed) = crate::bounded_output::with_limit(limit, || {
        let mut hook = |_, _: &Path, _: &Path, _: &Path| {};
        build_native_rust_interop_bundle_bounded(program, spec_bytes, output, &mut hook)
    });
    finish_bounded_bundle(result, overflowed)
}

#[cfg(test)]
fn prepare_phase_b_with_test_limit(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
    limit: usize,
) -> Result<(), Vec<Diagnostic>> {
    assert!(limit <= MAX_BUILDER_BYTES);
    reset_phase_b_error_materialization_observer();
    let (result, overflowed) =
        crate::bounded_output::with_limit(limit, || prepare_phase_b(program, spec_bytes, output));
    match result {
        Ok(prepared) if overflowed => {
            drop(prepared);
            Err(diagnostic_vector(b109(
                "max_builder_bytes",
                MAX_BUILDER_BYTES,
            )))
        }
        Ok(prepared) => {
            drop(prepared);
            Ok(())
        }
        Err(error) => Err(error.into_diagnostics(overflowed)),
    }
}

// Proof-only implementation tests are isolated in `implementation/tests.rs`; this file retains production authority.
#[cfg(test)]
#[path = "implementation/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "implementation/target_tests.rs"]
mod target_tests;
