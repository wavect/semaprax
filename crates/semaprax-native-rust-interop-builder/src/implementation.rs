// Private Native Rust Interoperability v1 preparation and static-bundle lane.
// Compiled only by the unpublished interop crate during private A+B.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::platform;

use crate::ast::{ParamMode, Program, Type};
use crate::diagnostic::{quote_json, Diagnostic};

mod artifacts;
mod capacity;
mod exact_replay;

use artifacts::{
    c_expression_shape, generate_c, generate_header, generate_rust_artifacts, render_descriptor,
    replay_descriptor, replay_generated_exact, replay_limits_exact, C_EXPRESSION_FRAME_BYTES,
    REPLAY_C_EXPRESSION_FRAME_BYTES,
};
#[cfg(test)]
use artifacts::{
    capability_digest, generate_c_into, generate_header_with_limit,
    generate_rust_artifacts_with_limit, render_descriptor_with_limit, replay_capabilities_digest,
    replay_generated, replay_spec_bytes_exact, replay_symbol_hash,
};
use exact_replay::ExactReplay;

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
const SOURCE_DOMAIN: &[u8] = b"semaprax.native-rust-interop.source-revision.v1\0";
const HIR_DOMAIN: &[u8] = b"semaprax.native-rust-interop.hir-digest.v1\0";
const SPEC_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-interop.spec-digest.v1\0";
const DESCRIPTOR_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-interop.descriptor-digest.v1\0";
const CALL_DOMAIN: &[u8] = b"semaprax.native-rust-interop.call-contract.v1\0";
const BUNDLE_DIGEST_DOMAIN: &[u8] = b"semaprax.native-rust-interop.bundle-digest.v1\0";
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

#[cfg(test)]
thread_local! {
    static CANONICAL_FORMAT_PASS_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static HIR_RESOLVE_PASS_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static HIR_POST_RESOLVE_PHASE_COUNT: std::cell::Cell<[usize; 4]> = const { std::cell::Cell::new([0; 4]) };
    static HIR_POST_RESOLVE_CAPACITY_HIGH_WATER: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
    static POST_HIR_FACTS_ENTRY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static POST_HIR_FACTS_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static POST_HIR_FACTS_SCRATCH_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static POST_HIR_AUTHORITY_TRANSFER_TERMS: std::cell::Cell<[usize; 5]> = const { std::cell::Cell::new([0; 5]) };
    static POST_HIR_RENDER_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static POST_HIR_REPLAY_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CLOSURE_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RESOLVED_DISPOSE_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RESOLVED_DISPOSE_COMPLETIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RESOLVED_DISPOSE_CAPACITIES: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
    static PREPARE_FAILURE_INJECTION: std::cell::Cell<Option<PrepareFailurePoint>> = const { std::cell::Cell::new(None) };
    static CREATE_AUTH_DISAGREEMENT: std::cell::Cell<Option<CreateAuthDisagreement>> = const { std::cell::Cell::new(None) };
    static CREATE_AUTH_DISCARD_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_EFFECT_STARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_NATIVE_STAGE_ARENA_SETS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PREPARED_CARRIER_IDENTITIES: std::cell::Cell<[usize; 7]> = const { std::cell::Cell::new([0; 7]) };
    static PHASE_B_LOCAL_FAILURE_INJECTION: std::cell::Cell<Option<PhaseBLocalError>> = const { std::cell::Cell::new(None) };
    static PHASE_B_DISCARD_FAILURE_AFTER_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static PHASE_B_DISCARD_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_OVERSIZE_MANIFEST_INJECTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_OUTPUT_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_TOOL_HOLDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_TOOL_PROCESSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PROCESS_ARENA_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PROCESS_ARENA_BUDGET_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PROCESS_ARENA_DROP_ORDER: std::cell::Cell<[u8; 2]> = const { std::cell::Cell::new([0; 2]) };
    static PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_INVALID_TOOL_ENV_INJECTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_BUILD_INVOCATION_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_BUILD_INVOCATION_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_LINK_COPY_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_LINK_COPY_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_INVENTORY_EXACT_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_INVENTORY_EXACT_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PUBLISH_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PUBLISH_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_PUBLISH_FAILURE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static PHASE_B_OBJECT_AUTHORITY_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_OBJECT_AUTHORITY_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_OBJECT_AUTHORITY_LIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_OBJECT_BYTES_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_OBJECT_DROP_ORDER: std::cell::Cell<[u8; 2]> = const { std::cell::Cell::new([0; 2]) };
    static PHASE_B_OBJECT_DROP_ORDER_LENGTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_MANIFEST_PLAN_CAPACITY: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_MANIFEST_BYTES) };
    static PHASE_B_MANIFEST_ARENA_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_MANIFEST_ARENA_GROWTHS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_MANIFEST_AUTHORITY_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_MANIFEST_AUTHORITY_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_MANIFEST_AUTHORITY_LIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static PHASE_B_MANIFEST_BYTES_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PHASE_B_MANIFEST_DROP_ORDER: std::cell::Cell<[u8; 2]> = const { std::cell::Cell::new([0; 2]) };
    static PHASE_B_MANIFEST_DROP_ORDER_LENGTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrepareFailurePoint {
    Closure,
    Facts,
    Render,
    Replay,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreateAuthDisagreement {
    Clean,
    Substituted,
}

#[cfg(test)]
fn inject_prepare_failure(point: PrepareFailurePoint) -> Result<(), Diagnostic> {
    if PREPARE_FAILURE_INJECTION.with(std::cell::Cell::get) == Some(point) {
        Err(b107("injected private preparation failure"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
fn note_canonical_format_pass() {
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn note_hir_resolve_pass() {
    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn note_hir_post_resolve_phase(index: usize) {
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| {
        let mut values = counts.get();
        values[index] += 1;
        counts.set(values);
    });
}

#[cfg(test)]
fn note_hir_post_resolve_capacity(index: usize, bytes: usize) {
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| {
        let mut values = water.get();
        values[index] = values[index].max(bytes);
        water.set(values);
    });
}

#[cfg(test)]
fn note_post_hir_facts_entry() {
    POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn note_post_hir_facts_capacity(bytes: usize) {
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn note_post_hir_facts_scratch(bytes: usize) {
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn note_post_hir_render_capacity(bytes: usize) {
    POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn note_post_hir_replay_capacity(bytes: usize) {
    POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(not(test))]
fn note_hir_post_resolve_phase(_index: usize) {}

#[cfg(not(test))]
fn note_hir_post_resolve_capacity(_index: usize, _bytes: usize) {}

#[cfg(not(test))]
fn note_hir_resolve_pass() {}

#[cfg(test)]
fn reset_closure_capacity_high_water() {
    CLOSURE_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
fn closure_capacity_high_water() -> usize {
    CLOSURE_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_closure_capacity_high_water(bytes: usize) {
    CLOSURE_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn note_resolved_dispose_high_water(len: usize) {
    RESOLVED_DISPOSE_HIGH_WATER.with(|water| water.set(water.get().max(len)));
}

#[cfg(not(test))]
fn note_resolved_dispose_high_water(_len: usize) {}

#[cfg(test)]
fn note_resolved_dispose_completion() {
    RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
fn note_resolved_dispose_capacity(index: usize, capacity: usize) {
    RESOLVED_DISPOSE_CAPACITIES.with(|capacities| {
        let mut values = capacities.get();
        values[index] = capacity;
        capacities.set(values);
    });
}

#[cfg(not(test))]
fn note_resolved_dispose_capacity(_index: usize, _capacity: usize) {}

#[cfg(not(test))]
fn note_resolved_dispose_completion() {}

#[cfg(not(test))]
fn note_canonical_format_pass() {}
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
    source_revision: String,
    target: Target,
    exports: Vec<String>,
    imports: Vec<String>,
    capabilities: Vec<String>,
}

#[derive(Clone, Eq, PartialEq)]
struct Target {
    triple: String,
    pointer_width: u32,
    endian: String,
    panic_strategy: String,
    thread_policy: String,
}

fn checked_spec_owned_capacity(spec: &Spec) -> Option<usize> {
    std::mem::size_of::<Spec>()
        .checked_add(spec.module.capacity())
        .and_then(|bytes| bytes.checked_add(spec.source_revision.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.triple.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.endian.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.thread_policy.capacity()))
        .and_then(|bytes| {
            [&spec.exports, &spec.imports, &spec.capabilities]
                .into_iter()
                .try_fold(bytes, |bytes, values| {
                    bytes
                        .checked_add(
                            values
                                .capacity()
                                .checked_mul(std::mem::size_of::<String>())?,
                        )
                        .and_then(|bytes| {
                            values
                                .iter()
                                .try_fold(bytes, |bytes, value| bytes.checked_add(value.capacity()))
                        })
                })
        })
}

fn prepared_spec_transfer_capacity(spec: &Spec) -> Option<usize> {
    spec.source_revision
        .capacity()
        .checked_add(spec.target.triple.capacity())
        .and_then(|bytes| bytes.checked_add(spec.target.endian.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.thread_policy.capacity()))
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

#[derive(Clone, Copy)]
struct PostHirFactsCapacity {
    retained_upper: usize,
    facts_scratch_upper: usize,
    render_scratch_upper: usize,
    replay_scratch_upper: usize,
    traversal_pending_capacity: usize,
}

impl PostHirFactsCapacity {
    fn scratch_upper(self) -> usize {
        self.facts_scratch_upper
            .max(self.render_scratch_upper)
            .max(self.replay_scratch_upper)
    }

    fn complete(self) -> Option<usize> {
        self.retained_upper.checked_add(self.scratch_upper())
    }
}

fn checked_btree_allocation_upper<K, V>(len: usize) -> Option<usize> {
    len.checked_mul(
        std::mem::size_of::<(K, V)>().checked_add(std::mem::size_of::<BTreeMap<K, V>>())?,
    )
}

#[cfg(test)]
fn checked_owned_string_vec(values: &[String], capacity: usize) -> Option<usize> {
    values.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<String>())?,
        |bytes, value| bytes.checked_add(value.capacity()),
    )
}

#[cfg(test)]
fn checked_owned_string_pairs(values: &Vec<(String, String)>) -> Option<usize> {
    values.iter().try_fold(
        values
            .capacity()
            .checked_mul(std::mem::size_of::<(String, String)>())?,
        |bytes, (left, right)| {
            bytes
                .checked_add(left.capacity())?
                .checked_add(right.capacity())
        },
    )
}

#[cfg(test)]
fn checked_u16_vec(values: &Vec<u16>) -> Option<usize> {
    values.capacity().checked_mul(std::mem::size_of::<u16>())
}

#[cfg(test)]
fn note_post_hir_facts_live(_baseline: usize, scratch: usize) {
    note_post_hir_facts_scratch(scratch);
    note_post_hir_facts_capacity(_baseline.saturating_add(scratch));
}

#[cfg(test)]
fn checked_owned_string_set(values: &BTreeSet<String>) -> Option<usize> {
    values.iter().try_fold(
        checked_btree_allocation_upper::<String, ()>(values.len())?,
        |bytes, value| bytes.checked_add(value.capacity()),
    )
}

#[cfg(test)]
fn checked_json_value_owned(value: &Value) -> Option<usize> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Some(0),
        Value::String(value) => Some(value.capacity()),
        Value::Array(values) => values.iter().try_fold(
            values
                .capacity()
                .checked_mul(std::mem::size_of::<Value>())?,
            |bytes, value| bytes.checked_add(checked_json_value_owned(value)?),
        ),
        Value::Object(values) => values.iter().try_fold(
            checked_btree_allocation_upper::<String, Value>(values.len())?,
            |bytes, (key, value)| {
                bytes
                    .checked_add(key.capacity())?
                    .checked_add(checked_json_value_owned(value)?)
            },
        ),
    }
}

#[cfg(test)]
fn checked_json_string_payload(value: &Value) -> Option<usize> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => Some(0),
        Value::String(value) => Some(value.capacity()),
        Value::Array(values) => values.iter().try_fold(0usize, |bytes, value| {
            bytes.checked_add(checked_json_string_payload(value)?)
        }),
        Value::Object(values) => values.iter().try_fold(0usize, |bytes, (key, value)| {
            bytes
                .checked_add(key.capacity())?
                .checked_add(checked_json_string_payload(value)?)
        }),
    }
}

fn post_hir_facts_capacity(
    _source_bytes: usize,
    spec_bytes: usize,
    resolved: &ResolvedProgram,
    closure: &[&ResolvedFunction],
    spec: &Spec,
) -> Result<PostHirFactsCapacity, Diagnostic> {
    let selected = closure.len().max(1);
    let exports = spec.exports.len();
    let imports = spec.imports.len();
    let capabilities = spec.capabilities.len();
    let resolved_import_count = resolved
        .interfaces
        .iter()
        .try_fold(0usize, |count, interface| {
            count.checked_add(interface.imports.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let digest_text_capacity = "sha256:"
        .len()
        .checked_add(64)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let parameter_slots = closure
        .iter()
        .try_fold(0usize, |count, function| {
            count.checked_add(function.params.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // This census executes before the post-HIR reservation. Traverse borrowed
    // imports directly: no Vec or map may be materialized until `complete()`
    // has been admitted.
    let (
        import_parameter_slots,
        import_retained_payload,
        import_effect_entries,
        selected_import_id_bytes,
        selected_import_effect_bytes,
    ) = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| spec.imports.iter().any(|id| id == import.id.as_str()))
        .try_fold((0usize, 0usize, 0usize, 0usize, 0usize), |state, import| {
            let parameter_names = import
                .parameters
                .iter()
                .try_fold(0usize, |total, parameter| {
                    total.checked_add(parameter.name.capacity())
                })?;
            let effects = import
                .effects
                .iter()
                .try_fold(0usize, |total, effect| total.checked_add(effect.capacity()))?;
            let failure = match &import.failure {
                ResolvedImportFailure::Infallible => 0,
                ResolvedImportFailure::Status { domain_id, .. } => domain_id.len(),
            };
            let parameter_backing = import
                .parameters
                .len()
                .checked_mul(std::mem::size_of::<ParameterFact>())?;
            let effect_backing = import
                .effects
                .len()
                .checked_mul(std::mem::size_of::<String>())?
                .checked_mul(2)?;
            let retained = import
                .id
                .as_str()
                .len()
                .checked_add(import.interface.as_str().len())?
                .checked_add(import.import_key.capacity())?
                .checked_add("import_".len().checked_add(64)?)?
                .checked_add("spxnr1_i_".len().checked_add(64)?)?
                .checked_add(parameter_backing)?
                .checked_add(parameter_names)?
                .checked_add(effect_backing)?
                .checked_add(effects.checked_mul(2)?)?
                .checked_add(failure)?
                .checked_add(digest_text_capacity)?;
            Some((
                state.0.checked_add(import.parameters.len())?,
                state.1.checked_add(retained)?,
                state.2.checked_add(import.effects.len())?,
                state.3.checked_add(import.id.as_str().len())?,
                state.4.checked_add(effects)?,
            ))
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let import_effect_conversion_scratch = resolved
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .filter(|import| spec.imports.iter().any(|id| id == import.id.as_str()))
        .try_fold(0usize, |maximum, import| {
            let effect_payload = import
                .effects
                .iter()
                .try_fold(0usize, |bytes, effect| bytes.checked_add(effect.capacity()))?;
            let parameter_payload = import
                .parameters
                .iter()
                .try_fold(0usize, |bytes, parameter| {
                    bytes.checked_add(parameter.name.capacity())
                })?;
            let failure_payload = match &import.failure {
                ResolvedImportFailure::Infallible => 0,
                ResolvedImportFailure::Status { domain_id, .. } => domain_id.as_str().len(),
            };
            let scratch = checked_btree_allocation_upper::<String, ()>(import.effects.len())?
                .checked_add(effect_payload)?
                .checked_add(
                    import
                        .effects
                        .len()
                        .checked_mul(std::mem::size_of::<String>())?,
                )?
                .checked_add(effect_payload)?
                .checked_add(
                    import
                        .parameters
                        .len()
                        .checked_mul(std::mem::size_of::<ParameterFact>())?,
                )?
                .checked_add(parameter_payload)?
                .checked_add(failure_payload)?
                .checked_add(digest_text_capacity)?;
            Some(maximum.max(scratch))
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let checked_string_bytes = |values: &[String]| {
        values
            .iter()
            .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    };
    let import_id_bytes = checked_string_bytes(&spec.imports)?;
    let capability_bytes = checked_string_bytes(&spec.capabilities)?;
    let closure_id_bytes = closure
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes.checked_add(function.id.as_str().len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let export_retained_payload = spec
        .exports
        .iter()
        .try_fold(0usize, |bytes, id| {
            let function = closure
                .iter()
                .find(|function| function.id.as_str() == id)
                .copied()?;
            let parameter_payload = function
                .params
                .iter()
                .try_fold(0usize, |payload, parameter| {
                    payload.checked_add(parameter.name.capacity())
                })?;
            let parameter_backing = function
                .params
                .len()
                .checked_mul(std::mem::size_of::<ParameterFact>())?;
            let effect_payload = function
                .effects
                .iter()
                .try_fold(0usize, |payload, effect| {
                    payload.checked_add(effect.capacity())
                })?;
            let effect_backing = function
                .effects
                .len()
                .checked_mul(std::mem::size_of::<String>())?;
            let capability_backing = capabilities.checked_mul(std::mem::size_of::<String>())?;
            let required_import_backing = imports.checked_mul(std::mem::size_of::<String>())?;
            let status_ordinal_backing = imports
                .checked_add(3)?
                .checked_mul(std::mem::size_of::<u16>())?;
            let retained = id
                .len()
                .checked_add("export_".len().checked_add(64)?)?
                .checked_add("spxnr1_e_".len().checked_add(64)?)?
                .checked_add(parameter_backing)?
                .checked_add(parameter_payload)?
                .checked_add(effect_backing)?
                .checked_add(effect_payload)?
                .checked_add(capability_backing)?
                .checked_add(capability_bytes)?
                .checked_add(required_import_backing)?
                .checked_add(import_id_bytes)?
                .checked_add(status_ordinal_backing)?
                .checked_add(digest_text_capacity)?;
            bytes.checked_add(retained)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let target_retained_payload = spec
        .target
        .triple
        .capacity()
        .checked_add(spec.target.endian.capacity())
        .and_then(|bytes| bytes.checked_add(spec.target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(spec.target.thread_policy.capacity()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let final_digest_payload = digest_text_capacity
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_upper = exports
        .checked_mul(std::mem::size_of::<ExportFact>())
        .and_then(|bytes| {
            bytes.checked_add(imports.checked_mul(std::mem::size_of::<ImportFact>())?)
        })
        .and_then(|bytes| bytes.checked_add(import_retained_payload))
        .and_then(|bytes| bytes.checked_add(export_retained_payload))
        .and_then(|bytes| {
            bytes.checked_add(closure.len().checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(closure_id_bytes))
        .and_then(|bytes| bytes.checked_add(spec.source_revision.capacity()))
        .and_then(|bytes| bytes.checked_add(final_digest_payload))
        .and_then(|bytes| bytes.checked_add(target_retained_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let (maximum_c_nodes, maximum_c_depth, maximum_parameter_owned) = closure
        .iter()
        .try_fold((1usize, 1usize, 0usize), |maximum, function| {
            let function_shape = function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .try_fold((1usize, 1usize), |current, expression| {
                    let (nodes, depth) = c_expression_shape(expression).ok()?;
                    Some((current.0.max(nodes), current.1.max(depth)))
                })?;
            let parameter_owned = function
                .params
                .iter()
                .try_fold(0usize, |bytes, parameter| {
                    bytes.checked_add(parameter.name.len())
                })?
                .checked_add(
                    function
                        .params
                        .len()
                        .checked_mul(std::mem::size_of::<ParameterFact>())?,
                )?;
            Some((
                maximum.0.max(function_shape.0),
                maximum.1.max(function_shape.1),
                maximum.2.max(parameter_owned),
            ))
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let selected_effects_backing =
        checked_btree_allocation_upper::<&str, ()>(import_effect_entries)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let source_function_backing =
        checked_btree_allocation_upper::<&str, &ResolvedFunction>(resolved.functions.len())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolved_import_backing = resolved_import_count
        .checked_mul(std::mem::size_of::<(&str, &ResolvedImport)>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let by_function_backing =
        checked_btree_allocation_upper::<&str, &ResolvedFunction>(closure.len())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let selection_scratch = selected_effects_backing
        .checked_add(source_function_backing)
        .and_then(|bytes| bytes.checked_add(resolved_import_backing))
        .and_then(|bytes| bytes.checked_add(by_function_backing))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

    let traversal_calls = traversal_call_site_census(closure)?;
    let traversal_pending_capacity = traversal_calls
        .function_sites
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let current_id_payload = closure
        .iter()
        .map(|function| function.id.as_str().len())
        .max()
        .unwrap_or(0);
    let pending_backing = traversal_pending_capacity
        .checked_mul(std::mem::size_of::<String>())
        .and_then(|bytes| bytes.checked_add(traversal_calls.function_id_bytes))
        .and_then(|bytes| bytes.checked_add(current_id_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let visited_backing = checked_btree_allocation_upper::<String, ()>(closure.len())
        .and_then(|bytes| bytes.checked_add(closure_id_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let direct_function_backing =
        checked_btree_allocation_upper::<DeclarationId, ()>(traversal_calls.function_sites)
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(traversal_calls.function_id_bytes.checked_mul(2)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let direct_import_backing =
        checked_btree_allocation_upper::<DeclarationId, ()>(traversal_calls.import_sites)
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(traversal_calls.import_id_bytes.checked_mul(2)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let transitive_import_backing = checked_btree_allocation_upper::<String, ()>(imports)
        .and_then(|bytes| bytes.checked_add(selected_import_id_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let traversal_scratch = pending_backing
        .checked_add(visited_backing)
        .and_then(|bytes| bytes.checked_add(direct_function_backing))
        .and_then(|bytes| bytes.checked_add(direct_import_backing))
        .and_then(|bytes| bytes.checked_add(transitive_import_backing))
        .and_then(|bytes| bytes.checked_add(current_id_payload))
        .and_then(|bytes| {
            bytes.checked_add(
                (MAX_SEMANTIC_EXPRESSION_DEPTH + 1)
                    .checked_mul(std::mem::size_of::<(&ResolvedExpr, usize)>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_cross_product_entry = digest_text_capacity
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<(String, String)>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_cross_product = exports
        .checked_mul(imports)
        .and_then(|rows| rows.checked_mul(facts_cross_product_entry))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let export_construction_scratch = export_retained_payload
        .checked_add(facts_cross_product)
        .and_then(|bytes| bytes.checked_add(import_retained_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_general_scratch = selection_scratch
        .checked_add(traversal_scratch)
        .and_then(|bytes| bytes.checked_add(export_construction_scratch))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let import_phase_scratch = selection_scratch
        .checked_add(import_effect_conversion_scratch)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Status-domain canonicalization owns the complete BTreeSet while the
    // exact-capacity Vec is filled. Charge both container allocations and a
    // conservative copy of every bounded key payload; the actual conversion
    // moves each String, so this is an upper rather than an amortized claim.
    let status_payload = imports
        .checked_mul(MAX_IDENTIFIER_BYTES)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let selected_capabilities_owned = import_effect_entries
        .checked_mul(std::mem::size_of::<String>())
        .and_then(|bytes| bytes.checked_add(selected_import_effect_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_set_vec_scratch = checked_btree_allocation_upper::<String, ()>(imports)
        .and_then(|bytes| bytes.checked_add(status_payload))
        .and_then(|bytes| bytes.checked_add(imports.checked_mul(std::mem::size_of::<String>())?))
        .and_then(|bytes| bytes.checked_add(status_payload))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_conversion_scratch = status_set_vec_scratch
        .checked_add(selection_scratch)
        .and_then(|bytes| bytes.checked_add(selected_capabilities_owned))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let fingerprint_action_scratch = FINGERPRINT_ACTION_SLOTS
        .checked_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let fingerprint_scratch = fingerprint_action_scratch
        .checked_add(fingerprint_type_scratch_upper(closure)?)
        .and_then(|bytes| bytes.checked_add(digest_text_capacity))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_scratch_upper = facts_general_scratch
        .max(import_phase_scratch)
        .max(status_conversion_scratch)
        .max(fingerprint_scratch);
    // Artifact outputs have independent retained reservations. These terms
    // authorize only simultaneously live renderer/replay scratch: final sink
    // plus branch/argument fragments for C, and the descriptor JSON DOM plus
    // exact-replay hash/escape temporaries. Fixed output maxima are admission
    // limits, not empirical multipliers.
    // The shared C generator/replay machine has one continuation per semantic
    // ancestor, one result slot per depth, and one flat argument slot per
    // expression node. A fixed line arena and the disjoint live value payload
    // each have the generated-C byte ceiling; neither can grow geometrically.
    let c_machine_scratch = maximum_c_depth
        .checked_add(1)
        .and_then(|slots| {
            slots.checked_mul(C_EXPRESSION_FRAME_BYTES.max(REPLAY_C_EXPRESSION_FRAME_BYTES))
        })
        .and_then(|bytes| {
            bytes.checked_add(
                maximum_c_depth
                    .checked_add(1)?
                    .checked_mul(std::mem::size_of::<String>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(maximum_c_nodes.checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(MAX_GENERATED_C_BYTES))
        .and_then(|bytes| bytes.checked_add(MAX_GENERATED_C_BYTES))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Persistent generator locals that coexist with the expression machine:
    // exact selected parameter facts, two borrowed import indexes, and the
    // bounded capability/parameter/hash strings. Final output is excluded.
    let c_outer_scratch = maximum_parameter_owned
        .checked_add(
            checked_btree_allocation_upper::<&String, ()>(imports)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(checked_btree_allocation_upper::<&str, usize>(imports)?)
        })
        .and_then(|bytes| bytes.checked_add(MAX_IDENTIFIER_BYTES.checked_mul(12)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let render_entries = selected
        .checked_add(exports)
        .and_then(|entries| entries.checked_add(imports))
        .and_then(|entries| entries.checked_add(capabilities))
        .and_then(|entries| entries.checked_add(parameter_slots))
        .and_then(|entries| entries.checked_add(import_parameter_slots))
        .and_then(|entries| entries.checked_add(imports.checked_add(4)?))
        .and_then(|entries| entries.checked_add(exports.checked_mul(imports.checked_add(3)?)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_collection_bytes = render_entries
        .checked_mul(
            std::mem::size_of::<String>()
                .checked_add(std::mem::size_of::<Value>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<Value>>()))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Descriptor row fragments coexist with their joined row strings before
    // the separately reserved final descriptor sink is materialized.
    let descriptor_render_scratch = MAX_DESCRIPTOR_BYTES
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(descriptor_collection_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The final generated-C output has its own retained reservation. One
    // MAX_C term here authorizes only the transient line payload; lines are
    // drained directly into the output so a second joined copy never exists.
    let c_render_scratch = c_machine_scratch
        .checked_add(c_outer_scratch)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Safe Rust keeps the quoted capability row plus at most one parameter or
    // argument row. Private FFI keeps its 32 digest-byte strings and import
    // table rows, then at most one callback/argument pair. Charge these Vec
    // headers fieldwise; MAX_RUST below covers only their joined/string
    // payloads, never either separately retained final sink.
    let safe_rust_vec_headers = capabilities
        .checked_add(MAX_PARAMETERS)
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<String>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let private_ffi_vec_headers = 32usize
        .checked_add(imports)
        .and_then(|entries| {
            entries.checked_add(
                MAX_PARAMETERS
                    .checked_mul(2)?
                    .max(imports.checked_add(MAX_PARAMETERS)?),
            )
        })
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<String>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let rust_render_scratch = MAX_GENERATED_RUST_BYTES
        .checked_add(safe_rust_vec_headers.max(private_ffi_vec_headers))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let render_scratch_upper = descriptor_render_scratch
        .max(c_render_scratch)
        .max(rust_render_scratch);
    // Descriptor replay owns one serde_json DOM plus independent expected
    // status/limit collections. Charge every schema-derived object entry as a
    // separately allocated BTree node, every admitted array Value slot at a
    // geometric two-times capacity upper, and the status Set→Vec overlap.
    // The two descriptor-byte terms cover decoded key/string payload capacity
    // and exact-replay escape/number temporaries; the final artifact is held by
    // its independent retained reservation.
    let descriptor_object_entries = 56usize
        .checked_add(
            exports
                .checked_mul(12)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|entries| entries.checked_add(imports.checked_mul(17)?))
        .and_then(|entries| {
            entries.checked_add(
                parameter_slots
                    .checked_add(import_parameter_slots)?
                    .checked_mul(3)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_array_values = imports
        .checked_add(4)
        .and_then(|values| values.checked_add(exports))
        .and_then(|values| values.checked_add(imports))
        .and_then(|values| values.checked_add(parameter_slots))
        .and_then(|values| values.checked_add(import_parameter_slots))
        .and_then(|values| values.checked_add(exports.checked_mul(3)?))
        .and_then(|values| values.checked_add(exports.checked_mul(capabilities.checked_mul(2)?)?))
        .and_then(|values| values.checked_add(exports.checked_mul(imports.checked_mul(2)?)?))
        .and_then(|values| values.checked_add(imports.checked_mul(capabilities.checked_mul(2)?)?))
        .and_then(|values| values.checked_add(NONCLAIMS.len()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_dom_backing =
        checked_btree_allocation_upper::<String, Value>(descriptor_object_entries)
            .and_then(|bytes| {
                bytes.checked_add(
                    descriptor_array_values
                        .checked_mul(2)?
                        .checked_mul(std::mem::size_of::<Value>())?,
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let expected_status_backing = imports
        .checked_add(4)
        .and_then(|entries| entries.checked_mul(std::mem::size_of::<(u64, &str)>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Locked serde_json 1.0.151 owns one reusable `Deserializer::scratch`
    // Vec<u8> (`src/de.rs`) whose string/number paths clear and reuse the same
    // buffer (`src/read.rs`). Decoded bytes cannot exceed the admitted input;
    // geometric Vec capacity is therefore at most twice that input. Returned
    // DOM string/key payload is a distinct at-most-input term. Exact replay's
    // escape/hash temporary is separate and begins only after parsing ends.
    let descriptor_dom_string_payload = MAX_DESCRIPTOR_BYTES;
    let serde_parser_vec_scratch = MAX_DESCRIPTOR_BYTES
        .checked_mul(2)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exact_descriptor_replay_temp = MAX_DESCRIPTOR_BYTES;
    let descriptor_parse_scratch = descriptor_dom_backing
        .checked_add(descriptor_dom_string_payload)
        .and_then(|bytes| bytes.checked_add(serde_parser_vec_scratch))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_validation_scratch = descriptor_dom_backing
        .checked_add(descriptor_dom_string_payload)
        .and_then(|bytes| bytes.checked_add(status_set_vec_scratch))
        .and_then(|bytes| bytes.checked_add(expected_status_backing))
        .and_then(|bytes| bytes.checked_add(exact_descriptor_replay_temp))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let descriptor_replay_scratch = descriptor_parse_scratch.max(descriptor_validation_scratch);
    let c_replay_scratch = c_machine_scratch
        .checked_add(c_outer_scratch)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exact_replay_scratch = spec_bytes
        .checked_add(
            MAX_IDENTIFIER_BYTES
                .checked_mul(4)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let replay_scratch_upper = descriptor_replay_scratch
        .max(c_replay_scratch)
        .max(exact_replay_scratch);
    Ok(PostHirFactsCapacity {
        retained_upper,
        facts_scratch_upper,
        render_scratch_upper,
        replay_scratch_upper,
        traversal_pending_capacity,
    })
}

fn string_vec_owned_capacity(values: &[String], capacity: usize) -> usize {
    capacity * std::mem::size_of::<String>() + values.iter().map(String::capacity).sum::<usize>()
}

fn parameter_facts_owned_capacity(values: &[ParameterFact], capacity: usize) -> usize {
    capacity * std::mem::size_of::<ParameterFact>()
        + values
            .iter()
            .map(|value| value.name.capacity())
            .sum::<usize>()
}

fn string_vec_owned_capacity_checked(values: &[String], capacity: usize) -> Option<usize> {
    values.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<String>())?,
        |bytes, value| bytes.checked_add(value.capacity()),
    )
}

fn parameter_facts_owned_capacity_checked(
    values: &[ParameterFact],
    capacity: usize,
) -> Option<usize> {
    values.iter().try_fold(
        capacity.checked_mul(std::mem::size_of::<ParameterFact>())?,
        |bytes, value| bytes.checked_add(value.name.capacity()),
    )
}

fn borrowed_string_set_owned_capacity(values: &BTreeSet<&str>) -> usize {
    values.len() * (std::mem::size_of::<(&str, ())>() + std::mem::size_of::<BTreeMap<&str, ()>>())
}

fn owned_string_set_owned_capacity(values: &BTreeSet<String>) -> usize {
    values.len()
        * (std::mem::size_of::<(String, ())>() + std::mem::size_of::<BTreeMap<String, ()>>())
        + values.iter().map(String::capacity).sum::<usize>()
}

#[cfg(test)]
fn borrowed_map_owned_capacity<K, V>(len: usize) -> usize {
    btree_allocation_upper::<K, V>(len)
}

#[cfg(test)]
fn post_hir_selection_scratch_capacity(
    selected_effects: &BTreeSet<&str>,
    source_functions: &BTreeMap<&str, &crate::ast::Function>,
    resolved_imports: &Vec<(&str, &ResolvedImport)>,
) -> usize {
    borrowed_string_set_owned_capacity(selected_effects)
        .saturating_add(borrowed_map_owned_capacity::<&str, &crate::ast::Function>(
            source_functions.len(),
        ))
        .saturating_add(
            resolved_imports.capacity() * std::mem::size_of::<(&str, &ResolvedImport)>(),
        )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn post_hir_live_facts_capacity(
    export_facts: &Vec<ExportFact>,
    import_facts: &Vec<ImportFact>,
    selected_effects: &BTreeSet<&str>,
    source_functions: &BTreeMap<&str, &crate::ast::Function>,
    resolved_imports: &Vec<(&str, &ResolvedImport)>,
    selected_capabilities: &Vec<String>,
    status_domains: &Vec<String>,
    ordinals: &BTreeMap<&str, u16>,
    by_function: &BTreeMap<&str, &ResolvedFunction>,
) -> usize {
    post_hir_facts_owned_capacity(export_facts, import_facts)
        .saturating_add(post_hir_selection_scratch_capacity(
            selected_effects,
            source_functions,
            resolved_imports,
        ))
        .saturating_add(string_vec_owned_capacity(
            selected_capabilities,
            selected_capabilities.capacity(),
        ))
        .saturating_add(string_vec_owned_capacity(
            status_domains,
            status_domains.capacity(),
        ))
        .saturating_add(borrowed_map_owned_capacity::<&str, u16>(ordinals.len()))
        .saturating_add(borrowed_map_owned_capacity::<&str, &ResolvedFunction>(
            by_function.len(),
        ))
}

fn post_hir_facts_owned_capacity(exports: &Vec<ExportFact>, imports: &Vec<ImportFact>) -> usize {
    let export_bytes = exports.iter().map(|fact| {
        fact.id.capacity()
            + fact.rust_method.capacity()
            + fact.c_symbol.capacity()
            + parameter_facts_owned_capacity(&fact.parameters, fact.parameters.capacity())
            + string_vec_owned_capacity(&fact.effects, fact.effects.capacity())
            + string_vec_owned_capacity(&fact.capabilities, fact.capabilities.capacity())
            + string_vec_owned_capacity(&fact.required_imports, fact.required_imports.capacity())
            + fact.status_domain_ordinals.capacity() * std::mem::size_of::<u16>()
            + fact.call_contract_digest.capacity()
    });
    let import_bytes = imports.iter().map(|fact| {
        fact.id.capacity()
            + fact.interface.capacity()
            + fact.import_key.capacity()
            + fact.rust_method.capacity()
            + fact.c_field.capacity()
            + parameter_facts_owned_capacity(&fact.parameters, fact.parameters.capacity())
            + string_vec_owned_capacity(&fact.effects, fact.effects.capacity())
            + string_vec_owned_capacity(&fact.capabilities, fact.capabilities.capacity())
            + fact.failure.as_ref().map_or(0, String::capacity)
            + fact.call_contract_digest.capacity()
    });
    exports.capacity() * std::mem::size_of::<ExportFact>()
        + imports.capacity() * std::mem::size_of::<ImportFact>()
        + export_bytes.sum::<usize>()
        + import_bytes.sum::<usize>()
}

fn post_hir_facts_owned_capacity_checked(
    exports: &Vec<ExportFact>,
    imports: &Vec<ImportFact>,
) -> Option<usize> {
    let mut bytes = exports
        .capacity()
        .checked_mul(std::mem::size_of::<ExportFact>())?
        .checked_add(
            imports
                .capacity()
                .checked_mul(std::mem::size_of::<ImportFact>())?,
        )?;
    for fact in exports {
        bytes = bytes
            .checked_add(fact.id.capacity())?
            .checked_add(fact.rust_method.capacity())?
            .checked_add(fact.c_symbol.capacity())?
            .checked_add(parameter_facts_owned_capacity_checked(
                &fact.parameters,
                fact.parameters.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.effects,
                fact.effects.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.capabilities,
                fact.capabilities.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.required_imports,
                fact.required_imports.capacity(),
            )?)?
            .checked_add(
                fact.status_domain_ordinals
                    .capacity()
                    .checked_mul(std::mem::size_of::<u16>())?,
            )?
            .checked_add(fact.call_contract_digest.capacity())?;
    }
    for fact in imports {
        bytes = bytes
            .checked_add(fact.id.capacity())?
            .checked_add(fact.interface.capacity())?
            .checked_add(fact.import_key.capacity())?
            .checked_add(fact.rust_method.capacity())?
            .checked_add(fact.c_field.capacity())?
            .checked_add(parameter_facts_owned_capacity_checked(
                &fact.parameters,
                fact.parameters.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.effects,
                fact.effects.capacity(),
            )?)?
            .checked_add(string_vec_owned_capacity_checked(
                &fact.capabilities,
                fact.capabilities.capacity(),
            )?)?
            .checked_add(fact.failure.as_ref().map_or(0, String::capacity))?
            .checked_add(fact.call_contract_digest.capacity())?;
    }
    Some(bytes)
}

fn string_slice_owned_capacity(values: &[String]) -> usize {
    std::mem::size_of_val(values) + values.iter().map(String::capacity).sum::<usize>()
}

fn spec_owned_capacity(spec: &Spec) -> usize {
    spec.module.capacity()
        + spec.source_revision.capacity()
        + spec.target.triple.capacity()
        + spec.target.endian.capacity()
        + spec.target.panic_strategy.capacity()
        + spec.target.thread_policy.capacity()
        + string_slice_owned_capacity(&spec.exports)
        + string_slice_owned_capacity(&spec.imports)
        + string_slice_owned_capacity(&spec.capabilities)
}

/// Opaque private phase-A facts. Fields intentionally have no getters before C.
pub(crate) struct PreparedNativeRustInterop {
    canonical_spec: String,
    spec_digest: String,
    descriptor: String,
    descriptor_digest: String,
    source_revision: String,
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
    pub(crate) fn source_revision(&self) -> &str {
        &self.source_revision
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
}

struct PendingBundleFacts {
    output_directory: PathBuf,
    object_path: PathBuf,
    descriptor_path: PathBuf,
    manifest_path: PathBuf,
    manifest_digest: String,
}

impl PendingBundleFacts {
    fn new(output: &Path, object_name: &'static str) -> Result<Self, Diagnostic> {
        use std::path::Component;

        let parent = output.parent().ok_or_else(platform_publication_error)?;
        let output_name = output.file_name().ok_or_else(platform_publication_error)?;
        let mut components = Path::new(output_name).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || output.strip_prefix(parent).ok() != Some(Path::new(output_name))
        {
            return Err(platform_publication_error());
        }

        let output_bytes = output.as_os_str().len();
        let child_capacity = |name: &str| exact_child_path_capacity(output, name.len());
        let object_capacity = child_capacity(object_name)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let descriptor_capacity = child_capacity("descriptor.json")
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let manifest_capacity = child_capacity("semaprax.native-rust-interop.json")
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let retained = output_bytes
            .checked_add(object_capacity)
            .and_then(|bytes| bytes.checked_add(descriptor_capacity))
            .and_then(|bytes| bytes.checked_add(manifest_capacity))
            .and_then(|bytes| bytes.checked_add(SHA256_TEXT_BYTES))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let authority = reserve_temporary_exact(retained)?;

        let output_directory = exact_path_copy(output, output_bytes)?;
        let object_path = exact_child_path(output, object_name, object_capacity)?;
        let descriptor_path = exact_child_path(output, "descriptor.json", descriptor_capacity)?;
        let manifest_path = exact_child_path(
            output,
            "semaprax.native-rust-interop.json",
            manifest_capacity,
        )?;
        let manifest_digest = String::with_capacity(SHA256_TEXT_BYTES);
        if manifest_digest.capacity() != SHA256_TEXT_BYTES {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        authority.retain(retained)?;
        Ok(Self {
            output_directory,
            object_path,
            descriptor_path,
            manifest_path,
            manifest_digest,
        })
    }

    fn bind_manifest_digest(&mut self, manifest: &[u8]) -> Result<(), PhaseBLocalError> {
        if !self.manifest_digest.is_empty() || self.manifest_digest.capacity() != SHA256_TEXT_BYTES
        {
            return Err(PhaseBLocalError::Replay);
        }
        self.manifest_digest.push_str("sha256:");
        let mut hasher = Sha256::new();
        hasher.update(BUNDLE_DIGEST_DOMAIN);
        hasher.update(manifest);
        let digest = hasher.finalize();
        for byte in digest {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            self.manifest_digest
                .push(char::from(HEX[usize::from(byte >> 4)]));
            self.manifest_digest
                .push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        if self.manifest_digest.len() != SHA256_TEXT_BYTES
            || self.manifest_digest.capacity() != SHA256_TEXT_BYTES
        {
            return Err(PhaseBLocalError::Replay);
        }
        Ok(())
    }

    fn finish(self) -> NativeRustInteropBundleFacts {
        NativeRustInteropBundleFacts {
            output_directory: self.output_directory,
            object_path: self.object_path,
            descriptor_path: self.descriptor_path,
            manifest_path: self.manifest_path,
            manifest_digest: self.manifest_digest,
        }
    }
}

fn exact_path_copy(path: &Path, capacity: usize) -> Result<PathBuf, Diagnostic> {
    if path.as_os_str().len() != capacity {
        return Err(platform_publication_error());
    }
    let mut output = OsString::with_capacity(capacity);
    output.push(path.as_os_str());
    let output = PathBuf::from(output);
    if output != path || output.capacity() != capacity {
        return Err(platform_publication_error());
    }
    Ok(output)
}

fn exact_child_path_capacity(parent: &Path, child_bytes: usize) -> Option<usize> {
    parent
        .as_os_str()
        .len()
        .checked_add(usize::from(path_needs_separator(parent)))?
        .checked_add(child_bytes)
}

fn path_needs_separator(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};

        let mut components = path.components();
        if matches!(
            components.next(),
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        ) && components.next().is_none()
        {
            return false;
        }
    }
    let Some(last) = path.as_os_str().as_encoded_bytes().last().copied() else {
        return false;
    };
    last != b'/' && (!cfg!(windows) || last != b'\\')
}

fn fill_exact_child_path(output: &mut PathBuf, parent: &Path, name: &OsStr) -> bool {
    if exact_child_path_capacity(parent, name.len())
        .is_none_or(|required| required > output.capacity())
    {
        return false;
    }
    let mut storage = std::mem::take(output).into_os_string();
    storage.clear();
    storage.push(parent.as_os_str());
    if path_needs_separator(parent) {
        storage.push(std::path::MAIN_SEPARATOR_STR);
    }
    storage.push(name);
    *output = PathBuf::from(storage);
    true
}

fn exact_child_path_matches(output: &Path, parent: &Path, name: &OsStr) -> bool {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(value)) if value == name)
        || components.next().is_some()
    {
        return false;
    }
    let separator = usize::from(path_needs_separator(parent));
    let output = output.as_os_str().as_encoded_bytes();
    let parent = parent.as_os_str().as_encoded_bytes();
    let name = name.as_encoded_bytes();
    output.len() == parent.len() + separator + name.len()
        && output.starts_with(parent)
        && (separator == 0 || output.get(parent.len()) == Some(&(std::path::MAIN_SEPARATOR as u8)))
        && &output[parent.len() + separator..] == name
}

fn exact_child_path(parent: &Path, name: &str, capacity: usize) -> Result<PathBuf, Diagnostic> {
    if exact_child_path_capacity(parent, name.len()) != Some(capacity) {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let mut storage = OsString::with_capacity(capacity);
    storage.push(parent.as_os_str());
    if path_needs_separator(parent) {
        storage.push(std::path::MAIN_SEPARATOR_STR);
    }
    storage.push(name);
    let output = PathBuf::from(storage);
    if output.capacity() != capacity || !exact_child_path_matches(&output, parent, OsStr::new(name))
    {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    Ok(output)
}

impl NativeRustInteropBundleFacts {
    pub(crate) fn output_directory(&self) -> &Path {
        &self.output_directory
    }
    pub(crate) fn object_path(&self) -> &Path {
        &self.object_path
    }
    pub(crate) fn descriptor_path(&self) -> &Path {
        &self.descriptor_path
    }
    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }
    pub(crate) fn manifest_digest(&self) -> &str {
        &self.manifest_digest
    }
}

fn b106() -> Diagnostic {
    Diagnostic::io(
        "SPX-B106",
        "Native Rust Interop specification is not canonical semaprax.native-rust-interop-spec.v1 JSON",
    )
}

fn b107(reason: &'static str) -> Diagnostic {
    Diagnostic::io(
        "SPX-B107",
        format!("Native Rust Interop declaration set is unsupported: {reason}"),
    )
}

fn b108() -> Diagnostic {
    Diagnostic::io(
        "SPX-B108",
        "Native Rust Interop descriptor disagrees with validated source and HIR",
    )
}

fn b109(field: &'static str, maximum: usize) -> Diagnostic {
    Diagnostic::io(
        "SPX-B109",
        format!("Native Rust Interop {field} exceeds {maximum}"),
    )
}

fn b110() -> Diagnostic {
    Diagnostic::io(
        "SPX-B110",
        "Native Rust Interop target or toolchain is unsupported",
    )
}

fn b111() -> Diagnostic {
    Diagnostic::io(
        "SPX-B111",
        "Native Rust Interop generated artifact replay failed",
    )
}

fn debit(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }
}

fn reserve_temporary_exact(maximum: usize) -> Result<TemporaryBudget, Diagnostic> {
    let remaining = crate::bounded_output::remaining_active().unwrap_or(MAX_BUILDER_BYTES);
    if maximum > remaining {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    debit(maximum)?;
    Ok(TemporaryBudget { reserved: maximum })
}

struct TemporaryBudget {
    reserved: usize,
}

enum ResolvedDisposeFrame {
    ExprBox(Box<ResolvedExpr>),
    Exprs(Vec<ResolvedExpr>),
    Statements(Vec<ResolvedStatement>),
    Fields(Vec<crate::hir::ResolvedFieldInitializer>),
    Arms(Vec<crate::hir::ResolvedMatchArm>),
    RecordPatternFields(Vec<crate::hir::ResolvedRecordMatchPatternField>),
    VariantPatternFields(Vec<crate::hir::ResolvedMatchPatternField>),
    Type(ResolvedType),
    Types(Vec<ResolvedType>),
    Shape(semaprax::cleanup::FieldLivenessShape),
    Shapes(Vec<semaprax::cleanup::FieldLiveness>),
}

const _: () = assert!(std::mem::size_of::<ResolvedDisposeFrame>() == 56);

struct ResolvedProgramOwner {
    program: Option<ResolvedProgram>,
    frames: Vec<ResolvedDisposeFrame>,
}

impl ResolvedProgramOwner {
    fn new(program: ResolvedProgram, frames: Vec<ResolvedDisposeFrame>, capacity: usize) -> Self {
        if frames.capacity() != capacity || !frames.is_empty() {
            std::process::abort();
        }
        note_resolved_dispose_capacity(0, capacity);
        Self {
            program: Some(program),
            frames,
        }
    }

    fn program(&self) -> &ResolvedProgram {
        self.program.as_ref().expect("resolved program retained")
    }
}

fn disposal_push(frames: &mut Vec<ResolvedDisposeFrame>, frame: ResolvedDisposeFrame) {
    if frames.len() == frames.capacity() {
        // The owner is created only after the admitted-depth census reserved
        // this fixed workspace. Exhaustion is an internal invariant failure;
        // aborting avoids both recursive fallback and allocation during Drop.
        std::process::abort();
    }
    frames.push(frame);
    note_resolved_dispose_high_water(frames.len());
}

impl Drop for ResolvedProgramOwner {
    fn drop(&mut self) {
        let Some(program) = self.program.take() else {
            return;
        };
        let ResolvedProgram {
            module,
            permits,
            entrypoint,
            declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        } = program;
        // Scalars, strings, declarations, and non-recursive declaration
        // containers may drop directly after every recursive HIR tree has
        // been moved into the preallocated disposal machine.
        for interface in interfaces {
            for import in interface.imports {
                for parameter in import.parameters {
                    disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(parameter.ty));
                    drain_disposal_frames(&mut self.frames, None);
                }
            }
        }
        drop((module, permits, entrypoint, declarations));
        for declaration in types {
            match declaration.kind {
                crate::hir::ResolvedTypeDeclarationKind::Resource { .. } => {}
                crate::hir::ResolvedTypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(field.ty));
                        drain_disposal_frames(&mut self.frames, None);
                    }
                }
                crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        for field in case.fields {
                            disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(field.ty));
                            drain_disposal_frames(&mut self.frames, None);
                        }
                    }
                }
            }
        }
        for template in function_templates {
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Type(template.return_type),
            );
            drain_disposal_frames(&mut self.frames, None);
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Exprs(template.requires),
            );
            drain_disposal_frames(&mut self.frames, None);
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Exprs(template.ensures),
            );
            drain_disposal_frames(&mut self.frames, None);
            for parameter in template.params {
                disposal_push(&mut self.frames, ResolvedDisposeFrame::Type(parameter.ty));
                drain_disposal_frames(&mut self.frames, None);
            }
            drain_disposal_frames(&mut self.frames, Some(template.body));
        }
        for function in functions {
            push_function_for_disposal(&mut self.frames, function);
        }
        for instance in function_instances {
            disposal_push(
                &mut self.frames,
                ResolvedDisposeFrame::Types(instance.type_arguments),
            );
            drain_disposal_frames(&mut self.frames, None);
            push_function_for_disposal(&mut self.frames, instance.function);
        }
        drain_disposal_frames(&mut self.frames, None);
        note_resolved_dispose_capacity(1, self.frames.capacity());
        note_resolved_dispose_completion();
    }
}

fn push_function_for_disposal(frames: &mut Vec<ResolvedDisposeFrame>, function: ResolvedFunction) {
    disposal_push(frames, ResolvedDisposeFrame::Type(function.return_type));
    drain_disposal_frames(frames, None);
    disposal_push(frames, ResolvedDisposeFrame::Exprs(function.requires));
    drain_disposal_frames(frames, None);
    disposal_push(frames, ResolvedDisposeFrame::Exprs(function.ensures));
    drain_disposal_frames(frames, None);
    for parameter in function.params {
        disposal_push(frames, ResolvedDisposeFrame::Type(parameter.ty));
        drain_disposal_frames(frames, None);
    }
    for slot in function.cleanup.slots {
        disposal_push(frames, ResolvedDisposeFrame::Type(slot.ty));
        drain_disposal_frames(frames, None);
        disposal_push(frames, ResolvedDisposeFrame::Shape(slot.shape));
        drain_disposal_frames(frames, None);
    }
    for slot in function.cleanup_plan.slots {
        disposal_push(frames, ResolvedDisposeFrame::Type(slot.ty));
        drain_disposal_frames(frames, None);
        disposal_push(
            frames,
            ResolvedDisposeFrame::Shape(slot.field_liveness_shape),
        );
        drain_disposal_frames(frames, None);
    }
    for block in function.cleanup_plan.blocks {
        for transition in block.transitions {
            if let crate::cleanup_plan::CleanupTransition::StageCopyResult { source } = transition {
                match source {
                    crate::cleanup_plan::StagedCopyResultSource::Body { instance, .. } => {
                        disposal_push(frames, ResolvedDisposeFrame::Type(instance));
                        drain_disposal_frames(frames, None);
                    }
                    crate::cleanup_plan::StagedCopyResultSource::TryResidual {
                        source_instance,
                        target_instance,
                        ..
                    }
                    | crate::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                        source_instance,
                        target_instance,
                        ..
                    } => {
                        disposal_push(frames, ResolvedDisposeFrame::Type(source_instance));
                        drain_disposal_frames(frames, None);
                        disposal_push(frames, ResolvedDisposeFrame::Type(target_instance));
                        drain_disposal_frames(frames, None);
                    }
                }
            }
        }
    }
    drain_disposal_frames(frames, Some(function.body));
}

fn drain_disposal_frames(
    frames: &mut Vec<ResolvedDisposeFrame>,
    mut pending_expression: Option<ResolvedExpr>,
) {
    loop {
        if let Some(expression) = pending_expression.take() {
            disposal_push(frames, ResolvedDisposeFrame::Type(expression.ty));
            match expression.kind {
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::Place(_) => {}
                ResolvedExprKind::Call {
                    type_arguments,
                    args,
                    ..
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::Types(type_arguments));
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(args));
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(call.args));
                }
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Project { base: value, .. } => {
                    pending_expression = Some(*value);
                }
                ResolvedExprKind::Binary { left, right, .. } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(right));
                    pending_expression = Some(*left);
                }
                ResolvedExprKind::Block { statements, tail } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(tail));
                    disposal_push(frames, ResolvedDisposeFrame::Statements(statements));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(else_branch));
                    disposal_push(frames, ResolvedDisposeFrame::ExprBox(then_branch));
                    pending_expression = Some(*condition);
                }
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    disposal_push(frames, ResolvedDisposeFrame::Fields(fields));
                }
                ResolvedExprKind::Match { scrutinee, arms } => {
                    disposal_push(frames, ResolvedDisposeFrame::Arms(arms));
                    pending_expression = Some(*scrutinee);
                }
                ResolvedExprKind::Try {
                    operand,
                    residual_type,
                    ..
                }
                | ResolvedExprKind::TryOption {
                    operand,
                    residual_type,
                    ..
                } => {
                    disposal_push(frames, ResolvedDisposeFrame::Type(residual_type));
                    pending_expression = Some(*operand);
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    disposal_push(frames, ResolvedDisposeFrame::Fields(fields));
                    pending_expression = Some(*base);
                }
            }
            continue;
        }
        let Some(frame) = frames.pop() else { break };
        match frame {
            ResolvedDisposeFrame::ExprBox(expression) => pending_expression = Some(*expression),
            ResolvedDisposeFrame::Exprs(mut expressions) => {
                if let Some(expression) = expressions.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Exprs(expressions));
                    pending_expression = Some(expression);
                }
            }
            ResolvedDisposeFrame::Statements(mut statements) => {
                if let Some(statement) = statements.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Statements(statements));
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    disposal_push(frames, ResolvedDisposeFrame::Type(binding.ty));
                    pending_expression = Some(value);
                }
            }
            ResolvedDisposeFrame::Fields(mut fields) => {
                if let Some(field) = fields.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Fields(fields));
                    pending_expression = Some(field.value);
                }
            }
            ResolvedDisposeFrame::Arms(mut arms) => {
                if let Some(arm) = arms.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Arms(arms));
                    dispose_match_pattern(frames, arm.pattern);
                    pending_expression = Some(arm.value);
                }
            }
            ResolvedDisposeFrame::RecordPatternFields(mut fields) => {
                if let Some(field) = fields.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::RecordPatternFields(fields));
                    match field.pattern {
                        crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                            disposal_push(frames, ResolvedDisposeFrame::Type(binding.ty));
                        }
                        crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
                        crate::hir::ResolvedRecordMatchFieldPattern::Record {
                            instance,
                            fields,
                            ..
                        } => {
                            disposal_push(frames, ResolvedDisposeFrame::Type(instance));
                            disposal_push(
                                frames,
                                ResolvedDisposeFrame::RecordPatternFields(fields),
                            );
                        }
                    }
                }
            }
            ResolvedDisposeFrame::VariantPatternFields(mut fields) => {
                if let Some(field) = fields.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::VariantPatternFields(fields));
                    disposal_push(frames, ResolvedDisposeFrame::Type(field.binding.ty));
                }
            }
            ResolvedDisposeFrame::Type(ty) => {
                if let ResolvedType::Nominal { arguments, .. } = ty {
                    disposal_push(frames, ResolvedDisposeFrame::Types(arguments));
                }
            }
            ResolvedDisposeFrame::Types(mut types) => {
                if let Some(ty) = types.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Types(types));
                    disposal_push(frames, ResolvedDisposeFrame::Type(ty));
                }
            }
            ResolvedDisposeFrame::Shape(shape) => {
                if let semaprax::cleanup::FieldLivenessShape::Record { fields, .. } = shape {
                    disposal_push(frames, ResolvedDisposeFrame::Shapes(fields));
                }
            }
            ResolvedDisposeFrame::Shapes(mut shapes) => {
                if let Some(field) = shapes.pop() {
                    disposal_push(frames, ResolvedDisposeFrame::Shapes(shapes));
                    disposal_push(frames, ResolvedDisposeFrame::Shape(field.shape));
                }
            }
        }
    }
}

fn dispose_match_pattern(
    frames: &mut Vec<ResolvedDisposeFrame>,
    pattern: crate::hir::ResolvedMatchPattern,
) {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => {}
        crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
            disposal_push(frames, ResolvedDisposeFrame::VariantPatternFields(fields));
        }
        crate::hir::ResolvedMatchPattern::Record {
            instance, fields, ..
        } => {
            disposal_push(frames, ResolvedDisposeFrame::Type(instance));
            disposal_push(frames, ResolvedDisposeFrame::RecordPatternFields(fields));
        }
    }
}

impl TemporaryBudget {
    fn maximum(&self) -> usize {
        self.reserved
    }

    fn retain(mut self, actual: usize) -> Result<(), Diagnostic> {
        if actual > self.reserved {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        crate::bounded_output::release_active(self.reserved - actual);
        self.reserved = 0;
        Ok(())
    }

    fn shrink_held(&mut self, actual: usize) -> Result<(), Diagnostic> {
        if actual > self.reserved {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        crate::bounded_output::release_active(self.reserved - actual);
        self.reserved = actual;
        Ok(())
    }

    fn check(&self, actual: usize) -> Result<(), Diagnostic> {
        if actual > self.reserved {
            Err(b109("max_builder_bytes", MAX_BUILDER_BYTES))
        } else {
            Ok(())
        }
    }
}

impl Drop for TemporaryBudget {
    fn drop(&mut self) {
        crate::bounded_output::release_active(self.reserved);
    }
}

fn debit_source(source: &str) -> Result<(), Diagnostic> {
    debit(source.len())
}

#[cfg(test)]
thread_local! {
    static TEST_TARGET_OVERRIDE: std::cell::RefCell<Option<Target>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn with_test_target<T>(target: Target, run: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_TARGET_OVERRIDE.with(|slot| *slot.borrow_mut() = None);
        }
    }
    TEST_TARGET_OVERRIDE.with(|slot| {
        assert!(slot.borrow().is_none(), "test target override nested");
        *slot.borrow_mut() = Some(target);
    });
    let reset = Reset;
    let result = run();
    drop(reset);
    result
}

fn current_target() -> Option<Target> {
    #[cfg(test)]
    if let Some(target) = TEST_TARGET_OVERRIDE.with(|slot| slot.borrow().clone()) {
        return Some(target);
    }
    let triple = if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "aarch64", target_os = "linux")) {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(all(target_arch = "x86_64", target_os = "macos")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_arch = "aarch64", target_os = "macos")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_arch = "aarch64", target_os = "windows")) {
        "aarch64-pc-windows-msvc"
    } else {
        return None;
    };
    Some(Target {
        triple: triple.to_owned(),
        pointer_width: 64,
        endian: "little".to_owned(),
        panic_strategy: "unwind".to_owned(),
        thread_policy: "same_thread".to_owned(),
    })
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn raw_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn identifier_gate(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES || value.contains('\0') {
        Err(b109("max_identifier_bytes", MAX_IDENTIFIER_BYTES))
    } else {
        Ok(())
    }
}

fn identifier_audit(program: &Program, spec: &Spec) -> Result<(), Diagnostic> {
    identifier_gate(&program.module)?;
    if spec.capabilities.len() > MAX_EFFECTS {
        return Err(b109("max_effects", MAX_EFFECTS));
    }
    for value in spec
        .exports
        .iter()
        .chain(&spec.imports)
        .chain(&spec.capabilities)
    {
        identifier_gate(value)?;
    }
    Ok(())
}

fn full_hash(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

fn frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn framed_digest<'a>(domain: &[u8], fields: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for field in fields {
        frame(&mut hasher, field);
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn json_depth(bytes: &[u8]) -> Result<usize, Diagnostic> {
    let mut depth = 0_usize;
    let mut maximum = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    for byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match *byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth = depth.checked_add(1).ok_or_else(b106)?;
                maximum = maximum.max(depth);
            }
            b'}' | b']' => depth = depth.checked_sub(1).ok_or_else(b106)?,
            _ => {}
        }
    }
    if quoted || depth != 0 {
        return Err(b106());
    }
    Ok(maximum)
}

fn maximum_spec_strings() -> Result<usize, Diagnostic> {
    MAX_EXPORTS
        .checked_add(MAX_IMPORTS)
        .and_then(|count| count.checked_add(MAX_EFFECTS))
        .and_then(|count| count.checked_add(NONCLAIMS.len()))
        .and_then(|count| count.checked_add(64))
        .ok_or_else(b106)
}

struct CountingSink {
    bytes: usize,
    maximum: usize,
    overflowed: bool,
}

impl std::fmt::Write for CountingSink {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let Some(bytes) = self.bytes.checked_add(value.len()) else {
            self.overflowed = true;
            return Ok(());
        };
        if bytes > self.maximum {
            self.overflowed = true;
        } else {
            self.bytes = bytes;
        }
        Ok(())
    }
}

fn count_exact_artifact<F>(
    field: &'static str,
    maximum: usize,
    render: &mut F,
) -> Result<usize, Diagnostic>
where
    F: FnMut(&mut dyn std::fmt::Write) -> Result<(), Diagnostic>,
{
    let mut counter = CountingSink {
        bytes: 0,
        maximum,
        overflowed: false,
    };
    render(&mut counter)?;
    if counter.overflowed {
        return Err(b109(field, maximum));
    }
    Ok(counter.bytes)
}

fn render_counted_artifact<F>(
    field: &'static str,
    maximum: usize,
    exact_bytes: usize,
    render: &mut F,
) -> Result<String, Diagnostic>
where
    F: FnMut(&mut dyn std::fmt::Write) -> Result<(), Diagnostic>,
{
    #[cfg(test)]
    EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT.with(|count| count.set(count.get() + 1));
    let mut output = String::with_capacity(exact_bytes);
    let initial_capacity = output.capacity();
    if initial_capacity != exact_bytes {
        return Err(b109(field, maximum));
    }
    render(&mut output)?;
    if output.len() != exact_bytes || output.capacity() != initial_capacity {
        return Err(b109(field, maximum));
    }
    Ok(output)
}

fn render_exact_artifact<F>(
    field: &'static str,
    maximum: usize,
    mut render: F,
) -> Result<String, Diagnostic>
where
    F: FnMut(&mut dyn std::fmt::Write) -> Result<(), Diagnostic>,
{
    let exact_bytes = count_exact_artifact(field, maximum, &mut render)?;
    render_counted_artifact(field, maximum, exact_bytes, &mut render)
}

fn canonical_format_scratch_capacity(
    program: &Program,
) -> Result<crate::private_format::PrivateScratchCapacity, Diagnostic> {
    let mut expression_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let expressions = program.functions.iter().flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    });
    let expression_depth = scan_ast_capacity(expressions, program, false, &mut expression_stack)?
        .max_depth
        .max(1);
    let mut type_depth = 1usize;
    for expression in program.functions.iter().flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    }) {
        type_depth = type_depth.max(ast_expression_type_depth(expression)?);
    }
    for function in &program.functions {
        type_depth = type_depth.max(ast_type_depth(&function.return_type)?);
        for parameter in &function.params {
            type_depth = type_depth.max(ast_type_depth(&parameter.ty)?);
        }
    }
    for interface in &program.interfaces {
        for import in &interface.imports {
            for parameter in &import.params {
                type_depth = type_depth.max(ast_type_depth(&parameter.ty)?);
            }
        }
    }
    for declaration in &program.types {
        match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { .. } => {}
            crate::ast::TypeDeclarationKind::Record { fields } => {
                for field in fields {
                    type_depth = type_depth.max(ast_type_depth(&field.ty)?);
                }
            }
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        type_depth = type_depth.max(ast_type_depth(&field.ty)?);
                    }
                }
            }
        }
    }
    let mut pattern_depth = 1usize;
    for function in &program.functions {
        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        for expression in roots {
            pattern_depth = pattern_depth.max(ast_pattern_depth(expression)?);
        }
    }
    crate::private_format::private_scratch_capacity(expression_depth, type_depth, pattern_depth)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn ast_expression_type_depth(root: &crate::ast::Expr) -> Result<usize, Diagnostic> {
    let mut expressions = [None; MAX_FORMAT_NESTING];
    expressions[0] = Some((root, 0usize));
    let mut len = 1usize;
    let mut maximum = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = expressions[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next == 0 {
            match &expression.kind {
                crate::ast::ExprKind::Call { type_arguments, .. } => {
                    for ty in type_arguments {
                        maximum = maximum.max(ast_type_depth(ty)?);
                    }
                }
                crate::ast::ExprKind::ConstructRecord { type_arguments, .. }
                | crate::ast::ExprKind::ConstructVariant { type_arguments, .. } => {
                    for ty in type_arguments {
                        maximum = maximum.max(ast_type_depth(ty)?);
                    }
                }
                _ => {}
            }
        }
        if let Some(child) = ast_child(expression, next) {
            if len + 2 > expressions.len() {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            expressions[len] = Some((expression, next + 1));
            expressions[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    Ok(maximum)
}

fn ast_type_depth(root: &crate::ast::Type) -> Result<usize, Diagnostic> {
    let mut stack: [Option<(&crate::ast::Type, usize, usize)>; MAX_FORMAT_NESTING] =
        [None; MAX_FORMAT_NESTING];
    stack[0] = Some((root, 1, 0));
    let mut len = 1usize;
    let mut maximum = 1usize;
    while len != 0 {
        len -= 1;
        let (ty, depth, next_child) = stack[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        maximum = maximum.max(depth);
        if let crate::ast::Type::Named { arguments, .. } = ty {
            if let Some(argument) = arguments.get(next_child) {
                if len + 2 > stack.len() {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                stack[len] = Some((ty, depth, next_child + 1));
                stack[len + 1] = Some((argument, depth + 1, 0));
                len += 2;
            }
        }
    }
    Ok(maximum)
}

fn ast_pattern_depth(root: &crate::ast::Expr) -> Result<usize, Diagnostic> {
    let mut expressions = [None; MAX_FORMAT_NESTING];
    expressions[0] = Some((root, 0usize));
    let mut expression_len = 1usize;
    let mut maximum = 1usize;
    while expression_len != 0 {
        expression_len -= 1;
        let (expression, next) = expressions[expression_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next == 0 {
            if let crate::ast::ExprKind::Match { arms, .. } = &expression.kind {
                for arm in arms {
                    maximum = maximum.max(match_pattern_depth(&arm.pattern)?);
                }
            }
        }
        if let Some(child) = ast_child(expression, next) {
            if expression_len + 2 > expressions.len() {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            expressions[expression_len] = Some((expression, next + 1));
            expressions[expression_len + 1] = Some((child, 0));
            expression_len += 2;
        }
    }
    Ok(maximum)
}

fn match_pattern_depth(pattern: &crate::ast::MatchPattern) -> Result<usize, Diagnostic> {
    let crate::ast::MatchPattern::Record { fields, .. } = pattern else {
        return Ok(1);
    };
    let mut stack: [Option<(&[crate::ast::RecordMatchPatternField], usize, usize)>;
        MAX_FORMAT_NESTING] = [None; MAX_FORMAT_NESTING];
    stack[0] = Some((fields, 1, 0));
    let mut len = 1usize;
    let mut maximum = 1usize;
    while len != 0 {
        len -= 1;
        let (fields, depth, next_child) = stack[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        maximum = maximum.max(depth);
        if let Some(field) = fields.get(next_child) {
            if let crate::ast::RecordMatchFieldPattern::Record { fields: nested, .. } =
                &field.pattern
            {
                if len + 2 > stack.len() {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                stack[len] = Some((fields, depth, next_child + 1));
                stack[len + 1] = Some((nested, depth + 1, 0));
                len += 2;
            } else if next_child + 1 < fields.len() {
                stack[len] = Some((fields, depth, next_child + 1));
                len += 1;
            }
        }
    }
    Ok(maximum)
}

fn canonical_source_bounded(program: &Program) -> Result<String, Diagnostic> {
    let scratch_bytes = canonical_format_scratch_capacity(program)?;
    let scratch_budget = reserve_temporary_exact(scratch_bytes.bytes())?;
    // Pass one establishes the exact final capacity while its frame scratch is
    // already authorized. Pass two holds the same scratch and exact String.
    let mut counter = CountingSink {
        bytes: 0,
        maximum: MAX_SOURCE_BYTES,
        overflowed: false,
    };
    note_canonical_format_pass();
    crate::private_format::write_canonical_with_scratch(program, &mut counter, scratch_bytes);
    if counter.overflowed {
        return Err(b109("max_source_bytes", MAX_SOURCE_BYTES));
    }
    let budget = reserve_temporary_exact(counter.bytes)?;
    let mut source = String::with_capacity(counter.bytes);
    note_canonical_format_pass();
    crate::private_format::write_canonical_with_scratch(program, &mut source, scratch_bytes);
    if source.len() != counter.bytes || source.capacity() != counter.bytes {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    budget.retain(source.capacity())?;
    drop(scratch_budget);
    Ok(source)
}

/// A single-pass parser for the exact canonical Spec shape.  It admits every
/// container, member, scalar, and array element before allocating the decoded
/// value, so hostile generic JSON never reaches a serde DOM.
struct SpecCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> SpecCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), Diagnostic> {
        let end = self.offset.checked_add(expected.len()).ok_or_else(b106)?;
        if self.bytes.get(self.offset..end) != Some(expected) {
            return Err(b106());
        }
        self.offset = end;
        Ok(())
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        let start = self.offset;
        self.expect(b"\"")?;
        let mut escaped = false;
        loop {
            let byte = *self.bytes.get(self.offset).ok_or_else(b106)?;
            self.offset = self.offset.checked_add(1).ok_or_else(b106)?;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                break;
            } else if byte < 0x20 {
                return Err(b106());
            }
        }
        let value: String =
            serde_json::from_slice(&self.bytes[start..self.offset]).map_err(|_| b106())?;
        if value.contains('\0') {
            return Err(b106());
        }
        Ok(value)
    }

    fn string_array(
        &mut self,
        maximum: usize,
        field: &'static str,
    ) -> Result<Vec<String>, Diagnostic> {
        self.expect(b"[")?;
        let mut values = Vec::new();
        if self.bytes.get(self.offset) == Some(&b']') {
            self.offset += 1;
            return Ok(values);
        }
        loop {
            if values.len() == maximum {
                return Err(b109(field, maximum));
            }
            values.push(self.string()?);
            match self.bytes.get(self.offset) {
                Some(b',') => self.offset += 1,
                Some(b']') => {
                    self.offset += 1;
                    return Ok(values);
                }
                _ => return Err(b106()),
            }
        }
    }

    fn finish(self) -> Result<(), Diagnostic> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(b106())
        }
    }
}

fn parse_spec(program: &Program, bytes: &[u8]) -> Result<Spec, Diagnostic> {
    let source = canonical_source_bounded(program)?;
    parse_spec_with_source(program, bytes, &source)
}

fn parse_spec_with_source(
    program: &Program,
    bytes: &[u8],
    source: &str,
) -> Result<Spec, Diagnostic> {
    let (spec, authority) = parse_spec_with_source_authority(program, bytes, source)?;
    let retained = authority.maximum();
    authority.retain(retained)?;
    Ok(spec)
}

fn parse_spec_with_source_authority(
    program: &Program,
    bytes: &[u8],
    source: &str,
) -> Result<(Spec, TemporaryBudget), Diagnostic> {
    if bytes.len() > MAX_SPEC_BYTES {
        return Err(b109("max_spec_bytes", MAX_SPEC_BYTES));
    }
    if json_depth(bytes)? > MAX_JSON_DEPTH {
        return Err(b109("max_json_depth", MAX_JSON_DEPTH));
    }
    let container_overhead = maximum_spec_strings()?
        .checked_mul(256)
        .and_then(|bytes| bytes.checked_add(65_536))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The exact-shape cursor can own at most one decoded copy of every input
    // string plus the three bounded vectors.  Reserve that complete capacity
    // before decoding the first string.
    let spec_upper = bytes
        .len()
        .checked_add(container_overhead)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Spec>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let spec_budget = reserve_temporary_exact(spec_upper)?;
    let mut cursor = SpecCursor::new(bytes);
    cursor.expect(b"{\"schema\":")?;
    if cursor.string()? != SPEC_SCHEMA {
        return Err(b106());
    }
    cursor.expect(b",\"module\":")?;
    let module = cursor.string()?;
    cursor.expect(b",\"source_revision\":")?;
    let source_revision = cursor.string()?;
    cursor.expect(b",\"target\":{\"triple\":")?;
    let triple = cursor.string()?;
    cursor.expect(b",\"pointer_width\":")?;
    let pointer_width = if cursor.bytes[cursor.offset..].starts_with(b"64") {
        cursor.offset += 2;
        64
    } else {
        return Err(b106());
    };
    cursor.expect(b",\"endian\":")?;
    let endian = cursor.string()?;
    cursor.expect(b",\"panic_strategy\":")?;
    let panic_strategy = cursor.string()?;
    cursor.expect(b",\"thread_policy\":")?;
    let thread_policy = cursor.string()?;
    cursor.expect(b"},\"exports\":")?;
    let exports = cursor.string_array(MAX_EXPORTS, "max_exports")?;
    cursor.expect(b",\"imports\":")?;
    let imports = cursor.string_array(MAX_IMPORTS, "max_imports")?;
    cursor.expect(b",\"capabilities\":")?;
    let capabilities = cursor.string_array(MAX_EFFECTS, "max_effects")?;
    cursor.expect(b",\"limits\":")?;
    cursor.expect(limits_json().as_bytes())?;
    cursor.expect(b",\"nonclaims\":[")?;
    for (index, expected) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        if cursor.string()? != *expected {
            return Err(b106());
        }
    }
    cursor.expect(b"]}\n")?;
    cursor.finish()?;
    let target = Target {
        triple,
        pointer_width,
        endian,
        panic_strategy,
        thread_policy,
    };
    let spec = Spec {
        module,
        source_revision,
        target,
        exports,
        imports,
        capabilities,
    };
    if spec.exports.is_empty()
        || !sorted_unique(&spec.exports)
        || !sorted_unique(&spec.imports)
        || !sorted_unique(&spec.capabilities)
    {
        return Err(b106());
    }
    let canonical_budget = reserve_temporary_exact(MAX_SPEC_BYTES)?;
    let canonical = render_spec(&spec);
    canonical_budget.check(canonical.capacity())?;
    if canonical.as_bytes() != bytes {
        return Err(b106());
    }
    drop(canonical);
    drop(canonical_budget);
    let spec_owned = checked_spec_owned_capacity(&spec)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for (actual, field, maximum) in [
        (spec.exports.len(), "max_exports", MAX_EXPORTS),
        (spec.imports.len(), "max_imports", MAX_IMPORTS),
        (spec.capabilities.len(), "max_effects", MAX_EFFECTS),
    ] {
        if actual > maximum {
            return Err(b109(field, maximum));
        }
    }
    identifier_gate(&spec.module)?;
    for value in spec
        .exports
        .iter()
        .chain(&spec.imports)
        .chain(&spec.capabilities)
    {
        identifier_gate(value)?;
    }
    if current_target().as_ref() != Some(&spec.target) {
        return Err(b107("target profile mismatch"));
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(b109("max_source_bytes", MAX_SOURCE_BYTES));
    }
    if spec.module != program.module
        || spec.source_revision != domain_digest(SOURCE_DOMAIN, source.as_bytes())
    {
        return Err(b107("selected identity missing"));
    }
    // Keep the complete decode reservation live through target construction,
    // source-digest materialization, and validation; only then transfer the
    // exact retained Spec capacity into the invocation-wide ledger.
    let mut spec_budget = spec_budget;
    spec_budget.shrink_held(spec_owned)?;
    Ok((spec, spec_budget))
}

fn limits_json() -> String {
    format!(
        "{{\"max_exports\":{MAX_EXPORTS},\"max_imports\":{MAX_IMPORTS},\"max_parameters\":{MAX_PARAMETERS},\"max_closure_functions\":{MAX_CLOSURE_FUNCTIONS},\"max_status_domains\":{MAX_STATUS_DOMAINS},\"max_effects\":{MAX_EFFECTS},\"max_identifier_bytes\":{MAX_IDENTIFIER_BYTES},\"max_source_bytes\":{MAX_SOURCE_BYTES},\"max_spec_bytes\":{MAX_SPEC_BYTES},\"max_descriptor_bytes\":{MAX_DESCRIPTOR_BYTES},\"max_generated_c_bytes\":{MAX_GENERATED_C_BYTES},\"max_generated_header_bytes\":{MAX_GENERATED_HEADER_BYTES},\"max_generated_rust_bytes\":{MAX_GENERATED_RUST_BYTES},\"max_manifest_bytes\":{MAX_MANIFEST_BYTES},\"max_builder_bytes\":{MAX_BUILDER_BYTES},\"max_json_depth\":{MAX_JSON_DEPTH},\"max_semantic_expression_depth\":{MAX_SEMANTIC_EXPRESSION_DEPTH},\"max_call_depth\":{MAX_CALL_DEPTH},\"max_calls_per_bridge\":{MAX_CALLS_PER_BRIDGE},\"max_unexpected_inventory_entries\":0}}"
    )
}

fn render_string_array(values: &[String]) -> String {
    let mut output = String::new();
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_json_string(&mut output, value).expect("writing JSON cannot fail");
    }
    output
}

fn nonclaims_json() -> String {
    let mut output = String::new();
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_json_string(&mut output, value).expect("writing JSON cannot fail");
    }
    output
}

fn write_limits_json(output: &mut impl std::fmt::Write) -> std::fmt::Result {
    output.write_char('{')?;
    for (index, (name, value)) in LIMIT_ROWS.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, name)?;
        output.write_char(':')?;
        write_usize_decimal(output, *value)?;
    }
    output.write_char('}')
}

fn target_json(target: &Target) -> String {
    format!(
        "{{\"triple\":{},\"pointer_width\":{},\"endian\":{},\"panic_strategy\":{},\"thread_policy\":{}}}",
        quote_json(&target.triple),
        target.pointer_width,
        quote_json(&target.endian),
        quote_json(&target.panic_strategy),
        quote_json(&target.thread_policy)
    )
}

fn write_json_string(output: &mut impl std::fmt::Write, value: &str) -> std::fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\u{08}' => output.write_str("\\b")?,
            '\u{0c}' => output.write_str("\\f")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            character if character <= '\u{1f}' => {
                write!(output, "\\u{:04x}", u32::from(character))?
            }
            character => output.write_char(character)?,
        }
    }
    output.write_char('"')
}

fn write_spec_string_array(
    output: &mut impl std::fmt::Write,
    values: &[String],
) -> std::fmt::Result {
    output.write_char('[')?;
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, value)?;
    }
    output.write_char(']')
}

fn write_spec(spec: &Spec, output: &mut impl std::fmt::Write) -> std::fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json_string(output, SPEC_SCHEMA)?;
    output.write_str(",\"module\":")?;
    write_json_string(output, &spec.module)?;
    output.write_str(",\"source_revision\":")?;
    write_json_string(output, &spec.source_revision)?;
    output.write_str(",\"target\":{\"triple\":")?;
    write_json_string(output, &spec.target.triple)?;
    write!(output, ",\"pointer_width\":{}", spec.target.pointer_width)?;
    output.write_str(",\"endian\":")?;
    write_json_string(output, &spec.target.endian)?;
    output.write_str(",\"panic_strategy\":")?;
    write_json_string(output, &spec.target.panic_strategy)?;
    output.write_str(",\"thread_policy\":")?;
    write_json_string(output, &spec.target.thread_policy)?;
    output.write_str("},\"exports\":")?;
    write_spec_string_array(output, &spec.exports)?;
    output.write_str(",\"imports\":")?;
    write_spec_string_array(output, &spec.imports)?;
    output.write_str(",\"capabilities\":")?;
    write_spec_string_array(output, &spec.capabilities)?;
    output.write_str(",\"limits\":")?;
    write_limits_json(output)?;
    output.write_str(",\"nonclaims\":[")?;
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, value)?;
    }
    output.write_str("]}\n")
}

fn render_spec(spec: &Spec) -> String {
    let mut counter = CountingSink {
        bytes: 0,
        maximum: MAX_SPEC_BYTES,
        overflowed: false,
    };
    write_spec(spec, &mut counter).expect("counting Spec output cannot fail");
    if counter.overflowed {
        return String::new();
    }
    let mut output = String::with_capacity(counter.bytes);
    write_spec(spec, &mut output).expect("writing Spec output cannot fail");
    output
}

fn scalar_type(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

fn source_scalar_type(ty: &Type) -> Option<ScalarType> {
    match ty {
        Type::I64 => Some(ScalarType::I64),
        Type::Bool => Some(ScalarType::Bool),
        Type::I32 | Type::Char | Type::U8 | Type::F32 | Type::F64 | Type::Named { .. } => None,
    }
}

fn scalar_text(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "unit",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
    }
}

fn c_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "void",
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
    }
}

fn rust_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
    }
}

fn rust_ffi_wire_type(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::Unit => "()",
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
    }
}

#[allow(clippy::too_many_arguments)]
fn call_digest(
    direction: &str,
    id: &str,
    parameters: &[ParameterFact],
    result: ScalarType,
    effects: &[String],
    capabilities: &[String],
    required_imports: &[String],
    required_import_contracts: &[(String, String)],
    failure: &str,
    _capacity_baseline: usize,
    target: &Target,
) -> Result<String, Diagnostic> {
    let parameter_values = parameters
        .iter()
        .map(|parameter| format!("{}:{}:value", parameter.name, scalar_text(parameter.ty)))
        .collect::<Vec<_>>();
    let params = parameter_values.join("\0");
    let target = target_json(target);
    let effects = effects.join("\0");
    let capabilities = capabilities.join("\0");
    #[cfg(test)]
    {
        let scratch = checked_owned_string_vec(&parameter_values, parameter_values.capacity())
            .and_then(|bytes| bytes.checked_add(params.capacity()))
            .and_then(|bytes| bytes.checked_add(target.capacity()))
            .and_then(|bytes| bytes.checked_add(effects.capacity()))
            .and_then(|bytes| bytes.checked_add(capabilities.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        note_post_hir_facts_live(_capacity_baseline, scratch);
    }
    let abi = "1\0C\0u64-domain16-code32-class8-retry1-reserved7\0u8-0-or-1\0signed-two-complement-i64\0SPXNRCTX1\0SPXNRIMP1\0caller-owned-uninitialized-success-only\0none-across-boundary\0caught-before-ffi-return\0same-thread\0rejected";
    let mut hasher = Sha256::new();
    hasher.update(CALL_DOMAIN);
    for value in [
        direction.as_bytes(),
        id.as_bytes(),
        params.as_bytes(),
        scalar_text(result).as_bytes(),
        effects.as_bytes(),
        capabilities.as_bytes(),
        failure.as_bytes(),
        target.as_bytes(),
        abi.as_bytes(),
    ] {
        frame(&mut hasher, value);
    }
    hash_count(&mut hasher, "required-imports", required_imports.len());
    for import in required_imports {
        frame(&mut hasher, import.as_bytes());
    }
    hash_count(
        &mut hasher,
        "required-import-contracts",
        required_import_contracts.len(),
    );
    for (id, digest) in required_import_contracts {
        frame(&mut hasher, id.as_bytes());
        frame(&mut hasher, digest.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    {
        let scratch = checked_owned_string_vec(&parameter_values, parameter_values.capacity())
            .and_then(|bytes| bytes.checked_add(params.capacity()))
            .and_then(|bytes| bytes.checked_add(target.capacity()))
            .and_then(|bytes| bytes.checked_add(effects.capacity()))
            .and_then(|bytes| bytes.checked_add(capabilities.capacity()))
            .and_then(|bytes| bytes.checked_add(digest.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        note_post_hir_facts_live(_capacity_baseline, scratch);
    }
    Ok(digest)
}

fn visit_calls(
    expression: &ResolvedExpr,
    functions: &mut BTreeSet<DeclarationId>,
    imports: &mut BTreeSet<DeclarationId>,
    _capacity_baseline: usize,
    _scratch_baseline: usize,
) -> Result<(), Diagnostic> {
    fn child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
        resolved_call_child(expression, index)
    }
    let mut frames = Vec::with_capacity(MAX_SEMANTIC_EXPRESSION_DEPTH + 1);
    frames.push((expression, 0usize));
    while let Some((expression, next)) = frames.pop() {
        if next == 0 {
            match &expression.kind {
                ResolvedExprKind::Call { callee, .. } => {
                    functions.insert(callee.clone());
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    imports.insert(call.import.clone());
                }
                _ => {}
            }
        }
        if let Some(child) = child(expression, next) {
            if frames.len() + 2 > frames.capacity() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            frames.push((expression, next + 1));
            frames.push((child, 0));
        }
        #[cfg(test)]
        {
            let scratch = frames.capacity() * std::mem::size_of::<(&ResolvedExpr, usize)>()
                + declaration_set_capacity(functions)
                + declaration_set_capacity(imports);
            note_post_hir_facts_scratch(_scratch_baseline.saturating_add(scratch));
            note_post_hir_facts_capacity(_capacity_baseline.saturating_add(scratch));
        }
    }
    Ok(())
}

fn resolved_call_child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.get(index),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. } => (index == 0).then_some(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            [left.as_ref(), right.as_ref()].get(index).copied()
        }
        ResolvedExprKind::Block { statements, tail } => statements
            .get(index)
            .map(|statement| {
                let ResolvedStatement::Let { value, .. } = statement;
                value
            })
            .or_else(|| (index == statements.len()).then_some(tail)),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied(),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            if index == 0 {
                Some(scrutinee)
            } else {
                arms.get(index - 1).map(|arm| &arm.value)
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            if index == 0 {
                Some(base)
            } else {
                fields.get(index - 1).map(|field| &field.value)
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::Place(_) => None,
    }
}

#[derive(Clone, Copy, Default)]
struct TraversalCallSiteCensus {
    function_sites: usize,
    function_id_bytes: usize,
    import_sites: usize,
    import_id_bytes: usize,
}

fn expression_call_site_census(root: &ResolvedExpr) -> Result<TraversalCallSiteCensus, Diagnostic> {
    let mut frames = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut frame_len = 1usize;
    frames[0] = Some((root, 0usize));
    let mut census = TraversalCallSiteCensus::default();
    while frame_len > 0 {
        let (expression, next) = frames[frame_len - 1]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        frame_len -= 1;
        if next == 0 {
            match &expression.kind {
                ResolvedExprKind::Call { callee, .. } => {
                    census.function_sites = census
                        .function_sites
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    census.function_id_bytes = census
                        .function_id_bytes
                        .checked_add(callee.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    census.import_sites = census
                        .import_sites
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    census.import_id_bytes = census
                        .import_id_bytes
                        .checked_add(call.import.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                _ => {}
            }
        }
        if let Some(child) = resolved_call_child(expression, next) {
            if frame_len + 2 > frames.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            frames[frame_len] = Some((expression, next + 1));
            frames[frame_len + 1] = Some((child, 0));
            frame_len += 2;
        }
    }
    Ok(census)
}

fn traversal_call_site_census(
    closure: &[&ResolvedFunction],
) -> Result<TraversalCallSiteCensus, Diagnostic> {
    closure
        .iter()
        .try_fold(TraversalCallSiteCensus::default(), |mut total, function| {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                let current = expression_call_site_census(expression)?;
                total.function_sites = total
                    .function_sites
                    .checked_add(current.function_sites)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                total.function_id_bytes = total
                    .function_id_bytes
                    .checked_add(current.function_id_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                total.import_sites = total
                    .import_sites
                    .checked_add(current.import_sites)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                total.import_id_bytes = total
                    .import_id_bytes
                    .checked_add(current.import_id_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            Ok(total)
        })
}

fn direct_calls(
    function: &ResolvedFunction,
    _capacity_baseline: usize,
    _scratch_baseline: usize,
) -> Result<(BTreeSet<DeclarationId>, BTreeSet<DeclarationId>), Diagnostic> {
    let mut functions = BTreeSet::new();
    let mut imports = BTreeSet::new();
    for contract in &function.requires {
        let mut contract_functions = BTreeSet::new();
        let mut contract_imports = BTreeSet::new();
        #[cfg(test)]
        let nested_baseline = _capacity_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_baseline = 0;
        #[cfg(test)]
        let nested_scratch = _scratch_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_scratch = 0;
        visit_calls(
            contract,
            &mut contract_functions,
            &mut contract_imports,
            nested_baseline,
            nested_scratch,
        )?;
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _scratch_baseline
                .saturating_add(declaration_set_capacity(&functions))
                .saturating_add(declaration_set_capacity(&imports))
                .saturating_add(declaration_set_capacity(&contract_functions))
                .saturating_add(declaration_set_capacity(&contract_imports)),
        );
        if !contract_imports.is_empty() {
            imports.insert(DeclarationId::new("\0native-rust-contract-call".to_owned()));
        }
        functions.extend(contract_functions);
    }
    visit_calls(
        &function.body,
        &mut functions,
        &mut imports,
        _capacity_baseline,
        _scratch_baseline,
    )?;
    for contract in &function.ensures {
        let mut contract_functions = BTreeSet::new();
        let mut contract_imports = BTreeSet::new();
        #[cfg(test)]
        let nested_baseline = _capacity_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_baseline = 0;
        #[cfg(test)]
        let nested_scratch = _scratch_baseline
            + declaration_set_capacity(&functions)
            + declaration_set_capacity(&imports);
        #[cfg(not(test))]
        let nested_scratch = 0;
        visit_calls(
            contract,
            &mut contract_functions,
            &mut contract_imports,
            nested_baseline,
            nested_scratch,
        )?;
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _scratch_baseline
                .saturating_add(declaration_set_capacity(&functions))
                .saturating_add(declaration_set_capacity(&imports))
                .saturating_add(declaration_set_capacity(&contract_functions))
                .saturating_add(declaration_set_capacity(&contract_imports)),
        );
        if !contract_imports.is_empty() {
            imports.insert(DeclarationId::new("\0native-rust-contract-call".to_owned()));
        }
        functions.extend(contract_functions);
    }
    Ok((functions, imports))
}

#[cfg(test)]
fn btree_allocation_upper<K, V>(len: usize) -> usize {
    // A BTree allocation contains inline key/value slots plus links and node
    // metadata. Charging one complete map header per live entry is a
    // conservative upper for the separately allocated node/link storage: a
    // non-root node always contains multiple entries, while a singleton root
    // needs only one header.
    len.saturating_mul(
        std::mem::size_of::<(K, V)>().saturating_add(std::mem::size_of::<BTreeMap<K, V>>()),
    )
}

#[cfg(test)]
fn declaration_set_capacity(set: &BTreeSet<DeclarationId>) -> usize {
    btree_allocation_upper::<DeclarationId, ()>(set.len())
        .saturating_add(set.iter().map(|id| id.as_str().len()).sum::<usize>())
}

struct SelectedClosureFrame {
    id: String,
    calls: Vec<String>,
    next: usize,
    longest: usize,
}

const _: () = assert!(std::mem::size_of::<SelectedClosureFrame>() == 64);

#[cfg(test)]
fn selected_closure_live_capacity(
    by_id: &BTreeMap<&str, &ResolvedFunction>,
    state: &BTreeMap<String, u8>,
    depths: &BTreeMap<String, usize>,
    closure: &Vec<&ResolvedFunction>,
    reached_imports: &BTreeSet<String>,
    stack: &Vec<SelectedClosureFrame>,
    pending: &Option<String>,
) -> usize {
    let stack_bytes = stack.iter().fold(
        stack.capacity() * std::mem::size_of::<SelectedClosureFrame>(),
        |bytes, frame| {
            bytes
                .saturating_add(frame.id.capacity())
                .saturating_add(frame.calls.capacity() * std::mem::size_of::<String>())
                .saturating_add(frame.calls.iter().map(String::capacity).sum::<usize>())
        },
    );
    let map_bytes = btree_allocation_upper::<&str, &ResolvedFunction>(by_id.len())
        .saturating_add(btree_allocation_upper::<String, u8>(state.len()))
        .saturating_add(state.keys().map(String::capacity).sum::<usize>())
        .saturating_add(btree_allocation_upper::<String, usize>(depths.len()))
        .saturating_add(depths.keys().map(String::capacity).sum::<usize>())
        .saturating_add(btree_allocation_upper::<String, ()>(reached_imports.len()))
        .saturating_add(reached_imports.iter().map(String::capacity).sum::<usize>());
    let closure_bytes = closure.capacity() * std::mem::size_of::<&ResolvedFunction>();
    stack_bytes
        .saturating_add(map_bytes)
        .saturating_add(closure_bytes)
        .saturating_add(pending.as_ref().map_or(0, String::capacity))
}

fn contract_reaches_native_import(
    function: &ResolvedFunction,
    by_id: &BTreeMap<&str, &ResolvedFunction>,
    #[cfg(test)] retained_outer_bytes: usize,
) -> Result<bool, Diagnostic> {
    let mut pending = BTreeSet::new();
    for contract in function.requires.iter().chain(&function.ensures) {
        let mut calls = BTreeSet::new();
        let mut imports = BTreeSet::new();
        visit_calls(
            contract,
            &mut calls,
            &mut imports,
            #[cfg(test)]
            retained_outer_bytes.saturating_add(declaration_set_capacity(&pending)),
            #[cfg(not(test))]
            0,
            0,
        )?;
        #[cfg(test)]
        note_closure_capacity_high_water(
            retained_outer_bytes
                .saturating_add(declaration_set_capacity(&pending))
                .saturating_add(declaration_set_capacity(&calls))
                .saturating_add(declaration_set_capacity(&imports)),
        );
        if !imports.is_empty() {
            return Ok(true);
        }
        pending.extend(calls);
    }
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop_first() {
        if !visited.insert(id.clone()) {
            continue;
        }
        let helper = by_id
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        let (calls, imports) = direct_calls(helper, 0, 0)?;
        #[cfg(test)]
        note_closure_capacity_high_water(
            retained_outer_bytes
                .saturating_add(declaration_set_capacity(&pending))
                .saturating_add(declaration_set_capacity(&visited))
                .saturating_add(declaration_set_capacity(&calls))
                .saturating_add(declaration_set_capacity(&imports))
                .saturating_add(id.as_str().len()),
        );
        if !imports.is_empty() {
            return Ok(true);
        }
        pending.extend(calls);
    }
    Ok(false)
}

fn selected_closure<'a>(
    resolved: &'a ResolvedProgram,
    selected: &[String],
) -> Result<(Vec<&'a ResolvedFunction>, BTreeSet<String>), Diagnostic> {
    note_hir_post_resolve_phase(0);
    let by_id = resolved
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut state = BTreeMap::<String, u8>::new();
    let mut depths = BTreeMap::<String, usize>::new();
    let mut closure = Vec::new();
    let mut reached_imports = BTreeSet::new();

    for root in selected {
        if state.get(root).copied() == Some(2) {
            continue;
        }
        let mut stack = Vec::<SelectedClosureFrame>::new();
        let mut pending = Some(root.clone());
        loop {
            #[cfg(test)]
            note_closure_capacity_high_water(selected_closure_live_capacity(
                &by_id,
                &state,
                &depths,
                &closure,
                &reached_imports,
                &stack,
                &pending,
            ));
            if let Some(id) = pending.take() {
                match state.get(&id).copied() {
                    Some(1) => return Err(b107("selected closure is cyclic")),
                    Some(2) => {
                        let child_depth = *depths
                            .get(&id)
                            .ok_or_else(|| b107("selected identity missing"))?;
                        if let Some(parent) = stack.last_mut() {
                            parent.longest = parent.longest.max(
                                child_depth
                                    .checked_add(1)
                                    .ok_or_else(|| b109("max_call_depth", MAX_CALL_DEPTH))?,
                            );
                            if parent.longest > MAX_CALL_DEPTH {
                                return Err(b109("max_call_depth", MAX_CALL_DEPTH));
                            }
                            continue;
                        }
                        break;
                    }
                    _ => {}
                }
                let function = by_id
                    .get(id.as_str())
                    .ok_or_else(|| b107("selected identity missing"))?;
                if contract_reaches_native_import(
                    function,
                    &by_id,
                    #[cfg(test)]
                    selected_closure_live_capacity(
                        &by_id,
                        &state,
                        &depths,
                        &closure,
                        &reached_imports,
                        &stack,
                        &pending,
                    ),
                )? {
                    return Err(b107("effect or capability mismatch"));
                }
                if state.len() >= MAX_CLOSURE_FUNCTIONS {
                    return Err(b109("max_closure_functions", MAX_CLOSURE_FUNCTIONS));
                }
                state.insert(id.clone(), 1);
                let (calls, imports) = direct_calls(function, 0, 0)?;
                #[cfg(test)]
                note_closure_capacity_high_water(
                    selected_closure_live_capacity(
                        &by_id,
                        &state,
                        &depths,
                        &closure,
                        &reached_imports,
                        &stack,
                        &pending,
                    )
                    .saturating_add(declaration_set_capacity(&calls))
                    .saturating_add(declaration_set_capacity(&imports)),
                );
                if imports.iter().any(|id| id.as_str().starts_with('\0')) {
                    return Err(b107("effect or capability mismatch"));
                }
                reached_imports.extend(imports.into_iter().map(|id| id.as_str().to_owned()));
                let mut call_ids = Vec::with_capacity(calls.len());
                let mut remaining_calls = calls;
                while let Some(id) = remaining_calls.pop_first() {
                    call_ids.push(id.as_str().to_owned());
                    #[cfg(test)]
                    note_closure_capacity_high_water(
                        selected_closure_live_capacity(
                            &by_id,
                            &state,
                            &depths,
                            &closure,
                            &reached_imports,
                            &stack,
                            &pending,
                        )
                        .saturating_add(declaration_set_capacity(&remaining_calls))
                        .saturating_add(
                            call_ids.capacity() * std::mem::size_of::<String>()
                                + call_ids.iter().map(String::capacity).sum::<usize>(),
                        ),
                    );
                }
                stack.push(SelectedClosureFrame {
                    id,
                    calls: call_ids,
                    next: 0,
                    longest: 1,
                });
                if stack.len() > MAX_CALL_DEPTH {
                    return Err(b109("max_call_depth", MAX_CALL_DEPTH));
                }
            }

            let Some(frame) = stack.last_mut() else { break };
            if let Some(call) = frame.calls.get(frame.next).cloned() {
                frame.next += 1;
                pending = Some(call);
                continue;
            }
            let frame = stack.pop().expect("checked nonempty");
            let function = by_id
                .get(frame.id.as_str())
                .ok_or_else(|| b107("selected identity missing"))?;
            state.insert(frame.id.clone(), 2);
            depths.insert(frame.id, frame.longest);
            closure.push(*function);
            if let Some(parent) = stack.last_mut() {
                parent.longest = parent.longest.max(
                    frame
                        .longest
                        .checked_add(1)
                        .ok_or_else(|| b109("max_call_depth", MAX_CALL_DEPTH))?,
                );
                if parent.longest > MAX_CALL_DEPTH {
                    return Err(b109("max_call_depth", MAX_CALL_DEPTH));
                }
            } else {
                break;
            }
        }
    }
    closure.sort_by(|left, right| left.id.cmp(&right.id));
    Ok((closure, reached_imports))
}

fn transitive_imports(
    root: &ResolvedFunction,
    functions: &BTreeMap<&str, &ResolvedFunction>,
    pending_capacity: usize,
    _capacity_baseline: usize,
) -> Result<BTreeSet<String>, Diagnostic> {
    let mut pending = Vec::with_capacity(pending_capacity);
    pending.push(root.id.as_str().to_owned());
    let mut visited = BTreeSet::new();
    let mut imports = BTreeSet::new();
    while let Some(id) = pending.pop() {
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            checked_owned_string_vec(&pending, pending.capacity())
                .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&visited)))
                .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&imports)))
                .and_then(|bytes| bytes.checked_add(id.capacity()))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        if !visited.insert(id.clone()) {
            continue;
        }
        let function = functions
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        #[cfg(test)]
        let traversal_scratch = checked_owned_string_vec(&pending, pending.capacity())
            .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&visited)))
            .and_then(|bytes| bytes.checked_add(owned_string_set_owned_capacity(&imports)))
            .and_then(|bytes| bytes.checked_add(id.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        #[cfg(not(test))]
        let traversal_scratch = 0;
        #[cfg(test)]
        note_post_hir_facts_live(_capacity_baseline, traversal_scratch);
        #[cfg(test)]
        let traversal_baseline = _capacity_baseline.saturating_add(traversal_scratch);
        #[cfg(not(test))]
        let traversal_baseline = 0;
        let (calls, reached) = direct_calls(function, traversal_baseline, traversal_scratch)?;
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            traversal_scratch
                .saturating_add(declaration_set_capacity(&calls))
                .saturating_add(declaration_set_capacity(&reached)),
        );
        if reached.iter().any(|id| id.as_str().starts_with('\0')) {
            return Err(b107("effect or capability mismatch"));
        }
        imports.extend(reached.into_iter().map(|id| id.as_str().to_owned()));
        pending.extend(calls.into_iter().map(|id| id.as_str().to_owned()));
    }
    Ok(imports)
}

fn parameter_facts(function: &ResolvedFunction) -> Result<Vec<ParameterFact>, Diagnostic> {
    if function.params.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    let mut facts = Vec::with_capacity(function.params.len());
    for parameter in &function.params {
        if parameter.ownership != OwnershipMode::Value
            || parameter.name.len() > MAX_IDENTIFIER_BYTES
        {
            return Err(b107("scalar value signature required"));
        }
        facts.push(ParameterFact {
            name: parameter.name.clone(),
            ty: scalar_type(&parameter.ty)
                .filter(|ty| *ty != ScalarType::Unit)
                .ok_or_else(|| b107("scalar value signature required"))?,
        });
    }
    Ok(facts)
}

fn import_parameter_facts(import: &ResolvedImport) -> Result<Vec<ParameterFact>, Diagnostic> {
    if import.parameters.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    let mut facts = Vec::with_capacity(import.parameters.len());
    for parameter in &import.parameters {
        if parameter.ownership != OwnershipMode::Value
            || parameter.consumes_on_failure
            || parameter.name.len() > MAX_IDENTIFIER_BYTES
        {
            return Err(b107("scalar value signature required"));
        }
        facts.push(ParameterFact {
            name: parameter.name.clone(),
            ty: scalar_type(&parameter.ty)
                .filter(|ty| *ty != ScalarType::Unit)
                .ok_or_else(|| b107("scalar value signature required"))?,
        });
    }
    Ok(facts)
}

#[derive(Clone, Copy, Debug)]
struct TypeIdentityMetrics {
    nodes: usize,
    all_key_bytes: usize,
    root_bytes: usize,
    maximum_encoded_bytes: usize,
}

enum TypeIdentityFrame<'a> {
    Enter(&'a ResolvedType),
    Finish(&'a DeclarationId, usize, usize, usize),
}

#[derive(Clone, Copy)]
enum TypeIdentityMetricFrame<'a> {
    Enter(&'a ResolvedType, usize),
    Finish(&'a DeclarationId, usize),
}

fn decimal_bytes(mut value: usize) -> usize {
    let mut bytes = 1usize;
    while value >= 10 {
        value /= 10;
        bytes += 1;
    }
    bytes
}

fn type_identity_metrics(
    ty: &ResolvedType,
    initial_depth: usize,
) -> Result<TypeIdentityMetrics, Diagnostic> {
    let leaf = |root_bytes| TypeIdentityMetrics {
        nodes: 1,
        all_key_bytes: root_bytes,
        root_bytes,
        maximum_encoded_bytes: 0,
    };
    let mut frames = [None; FINGERPRINT_ACTION_SLOTS];
    let mut frame_len = 1usize;
    frames[0] = Some(TypeIdentityMetricFrame::Enter(ty, initial_depth));
    let mut results = [None; FINGERPRINT_ACTION_SLOTS];
    let mut result_len = 0usize;
    let mut work = 0usize;
    while frame_len > 0 {
        frame_len -= 1;
        let frame = frames[frame_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match frame {
            TypeIdentityMetricFrame::Enter(ty, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                work = work
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if work > FINGERPRINT_ACTION_SLOTS {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                let metric = match ty {
                    ResolvedType::Unit => Some(leaf("unit".len())),
                    ResolvedType::I64 => Some(leaf("i64".len())),
                    ResolvedType::I32 => Some(leaf("i32".len())),
                    ResolvedType::Char => Some(leaf("char".len())),
                    ResolvedType::U8 => Some(leaf("u8".len())),
                    ResolvedType::F32 => Some(leaf("f32".len())),
                    ResolvedType::F64 => Some(leaf("f64".len())),
                    ResolvedType::Bool => Some(leaf("bool".len())),
                    ResolvedType::TypeParameter { owner, index } => {
                        let owner_bytes = owner.as_str().len();
                        let root_bytes = "parameter:"
                            .len()
                            .checked_add(decimal_bytes(owner_bytes))
                            .and_then(|bytes| bytes.checked_add(1))
                            .and_then(|bytes| bytes.checked_add(owner_bytes))
                            .and_then(|bytes| bytes.checked_add(1))
                            .and_then(|bytes| bytes.checked_add(decimal_bytes(*index as usize)))
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        Some(leaf(root_bytes))
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } => {
                        if frame_len
                            .checked_add(arguments.len())
                            .and_then(|len| len.checked_add(1))
                            .is_none_or(|len| len > frames.len())
                        {
                            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                        }
                        frames[frame_len] = Some(TypeIdentityMetricFrame::Finish(
                            declaration,
                            arguments.len(),
                        ));
                        frame_len += 1;
                        for argument in arguments.iter().rev() {
                            frames[frame_len] =
                                Some(TypeIdentityMetricFrame::Enter(argument, depth + 1));
                            frame_len += 1;
                        }
                        None
                    }
                };
                if let Some(metric) = metric {
                    let slot = results
                        .get_mut(result_len)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    *slot = Some(metric);
                    result_len += 1;
                }
            }
            TypeIdentityMetricFrame::Finish(declaration, count) => {
                let split = result_len
                    .checked_sub(count)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let mut nodes = 1usize;
                let mut all_key_bytes = 0usize;
                let mut encoded_bytes = 0usize;
                let mut maximum_encoded_bytes = 0usize;
                for slot in &mut results[split..result_len] {
                    let child = slot
                        .take()
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    nodes = nodes
                        .checked_add(child.nodes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    all_key_bytes = all_key_bytes
                        .checked_add(child.all_key_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    encoded_bytes = encoded_bytes
                        .checked_add(decimal_bytes(child.root_bytes))
                        .and_then(|bytes| bytes.checked_add(1))
                        .and_then(|bytes| bytes.checked_add(child.root_bytes))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    maximum_encoded_bytes = maximum_encoded_bytes.max(child.maximum_encoded_bytes);
                }
                let declaration_bytes = declaration.as_str().len();
                let root_bytes = "nominal:"
                    .len()
                    .checked_add(decimal_bytes(declaration_bytes))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(declaration_bytes))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(decimal_bytes(count)))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(encoded_bytes))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                result_len = split;
                results[result_len] = Some(TypeIdentityMetrics {
                    nodes,
                    all_key_bytes: all_key_bytes
                        .checked_add(root_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    root_bytes,
                    maximum_encoded_bytes: maximum_encoded_bytes.max(encoded_bytes),
                });
                result_len += 1;
            }
        }
    }
    if result_len != 1 {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    results[0]
        .take()
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn type_identity_scratch_upper(ty: &ResolvedType) -> Result<usize, Diagnostic> {
    let metrics = type_identity_metrics(ty, 1)?;
    metrics
        .nodes
        .checked_mul(std::mem::size_of::<TypeIdentityFrame<'_>>())
        .and_then(|bytes| {
            bytes.checked_add(metrics.nodes.checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(metrics.all_key_bytes))
        .and_then(|bytes| bytes.checked_add(metrics.maximum_encoded_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn fingerprint_type_identity(
    ty: &ResolvedType,
    _capacity_baseline: usize,
    _outer_scratch: usize,
) -> Result<String, Diagnostic> {
    let metrics = type_identity_metrics(ty, 1)?;
    let mut frames = Vec::with_capacity(metrics.nodes);
    let mut keys = Vec::<String>::with_capacity(metrics.nodes);
    frames.push(TypeIdentityFrame::Enter(ty));
    while let Some(frame) = frames.pop() {
        match frame {
            TypeIdentityFrame::Enter(ty) => match ty {
                ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool => {
                    let text = match ty {
                        ResolvedType::Unit => "unit",
                        ResolvedType::I64 => "i64",
                        ResolvedType::I32 => "i32",
                        ResolvedType::Char => "char",
                        ResolvedType::U8 => "u8",
                        ResolvedType::F32 => "f32",
                        ResolvedType::F64 => "f64",
                        ResolvedType::Bool => "bool",
                        _ => unreachable!(),
                    };
                    let mut key = String::with_capacity(text.len());
                    key.push_str(text);
                    keys.push(key);
                }
                ResolvedType::TypeParameter { owner, index } => {
                    let key_bytes = type_identity_metrics(ty, 1)?.root_bytes;
                    let mut key = String::with_capacity(key_bytes);
                    write!(key, "parameter:{}:{}:{index}", owner.as_str().len(), owner)
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    keys.push(key);
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    let node = type_identity_metrics(ty, 1)?;
                    let encoded_bytes = arguments
                        .iter()
                        .try_fold(0usize, |bytes, argument| {
                            let child_bytes = type_identity_metrics(argument, 1).ok()?.root_bytes;
                            bytes
                                .checked_add(decimal_bytes(child_bytes))?
                                .checked_add(1)?
                                .checked_add(child_bytes)
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    frames.push(TypeIdentityFrame::Finish(
                        declaration,
                        arguments.len(),
                        encoded_bytes,
                        node.root_bytes,
                    ));
                    frames.extend(arguments.iter().rev().map(TypeIdentityFrame::Enter));
                }
            },
            TypeIdentityFrame::Finish(declaration, count, encoded_bytes, result_bytes) => {
                let split = keys
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let mut encoded = String::with_capacity(encoded_bytes);
                for key in &keys[split..] {
                    write!(encoded, "{}:{key}", key.len())
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let mut result = String::with_capacity(result_bytes);
                write!(
                    result,
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    count,
                    encoded
                )
                .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    _outer_scratch
                        .saturating_add(
                            frames
                                .capacity()
                                .saturating_mul(std::mem::size_of::<TypeIdentityFrame<'_>>()),
                        )
                        .saturating_add(
                            keys.capacity()
                                .saturating_mul(std::mem::size_of::<String>()),
                        )
                        .saturating_add(keys.iter().map(String::capacity).sum::<usize>())
                        .saturating_add(encoded.capacity())
                        .saturating_add(result.capacity()),
                );
                keys.truncate(split);
                keys.push(result);
            }
        }
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _outer_scratch
                .saturating_add(
                    frames
                        .capacity()
                        .saturating_mul(std::mem::size_of::<TypeIdentityFrame<'_>>()),
                )
                .saturating_add(
                    keys.capacity()
                        .saturating_mul(std::mem::size_of::<String>()),
                )
                .saturating_add(keys.iter().map(String::capacity).sum::<usize>()),
        );
    }
    if keys.len() != 1 {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    Ok(keys.pop().expect("one checked type identity"))
}

fn fingerprint_binding_type_scratch(
    binding: &crate::hir::ResolvedBinding,
) -> Result<usize, Diagnostic> {
    type_identity_scratch_upper(&binding.ty)
}

fn fingerprint_record_pattern_types_scratch(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
) -> Result<usize, Diagnostic> {
    fields.iter().try_fold(0usize, |maximum, field| {
        let current = match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                fingerprint_binding_type_scratch(binding)?
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => 0,
            crate::hir::ResolvedRecordMatchFieldPattern::Record {
                instance, fields, ..
            } => type_identity_scratch_upper(instance)?
                .max(fingerprint_record_pattern_types_scratch(fields)?),
        };
        Ok(maximum.max(current))
    })
}

fn fingerprint_pattern_types_scratch(
    pattern: &crate::hir::ResolvedMatchPattern,
) -> Result<usize, Diagnostic> {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => Ok(0),
        crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
            fields.iter().try_fold(0usize, |maximum, field| {
                Ok(maximum.max(fingerprint_binding_type_scratch(&field.binding)?))
            })
        }
        crate::hir::ResolvedMatchPattern::Record {
            instance, fields, ..
        } => Ok(type_identity_scratch_upper(instance)?
            .max(fingerprint_record_pattern_types_scratch(fields)?)),
    }
}

fn fingerprint_expression_types_scratch(
    expression: &ResolvedExpr,
    depth: usize,
) -> Result<usize, Diagnostic> {
    #[derive(Clone, Copy)]
    enum Frame<'a> {
        Expr(&'a ResolvedExpr, usize),
        Exprs(&'a [ResolvedExpr], usize, usize),
        Statements(&'a [ResolvedStatement], usize, usize),
        Fields(&'a [crate::hir::ResolvedFieldInitializer], usize, usize),
        Arms(&'a [crate::hir::ResolvedMatchArm], usize, usize),
    }
    fn push<'a>(
        stack: &mut [Option<Frame<'a>>],
        stack_len: &mut usize,
        frame: Frame<'a>,
    ) -> Result<(), Diagnostic> {
        let slot = stack.get_mut(*stack_len).ok_or_else(|| {
            b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            )
        })?;
        *slot = Some(frame);
        *stack_len += 1;
        Ok(())
    }

    let mut stack = [None; FINGERPRINT_ACTION_SLOTS];
    let mut stack_len = 0usize;
    push(&mut stack, &mut stack_len, Frame::Expr(expression, depth))?;
    let mut maximum = 0usize;
    while stack_len > 0 {
        stack_len -= 1;
        let frame = stack[stack_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match frame {
            Frame::Expr(expression, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    )
                })?;
                maximum = maximum.max(type_identity_scratch_upper(&expression.ty)?);
                match &expression.kind {
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::Place(_) => {}
                    ResolvedExprKind::Call {
                        type_arguments,
                        args,
                        ..
                    } => {
                        for ty in type_arguments {
                            maximum = maximum.max(type_identity_scratch_upper(ty)?);
                        }
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Exprs(args, 0, child_depth),
                        )?;
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(&call.args, 0, child_depth),
                    )?,
                    ResolvedExprKind::Unary { value, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(value, child_depth))?
                    }
                    ResolvedExprKind::Binary { left, right, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(right, child_depth))?;
                        push(&mut stack, &mut stack_len, Frame::Expr(left, child_depth))?;
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(tail, child_depth))?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Statements(statements, 0, child_depth),
                        )?;
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(else_branch, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(then_branch, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(condition, child_depth),
                        )?;
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. } => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Fields(fields, 0, child_depth),
                    )?,
                    ResolvedExprKind::Match { scrutinee, arms } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Arms(arms, 0, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(scrutinee, child_depth),
                        )?;
                    }
                    ResolvedExprKind::Try {
                        operand,
                        residual_type,
                        ..
                    }
                    | ResolvedExprKind::TryOption {
                        operand,
                        residual_type,
                        ..
                    } => {
                        maximum = maximum.max(type_identity_scratch_upper(residual_type)?);
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(operand, child_depth),
                        )?;
                    }
                    ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Fields(fields, 0, child_depth),
                        )?;
                        push(&mut stack, &mut stack_len, Frame::Expr(base, child_depth))?;
                    }
                    ResolvedExprKind::Project { base, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(base, child_depth))?
                    }
                }
            }
            Frame::Exprs(expressions, index, depth) => {
                if let Some(expression) = expressions.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(expressions, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(expression, depth))?;
                }
            }
            Frame::Statements(statements, index, depth) => {
                if let Some(statement) = statements.get(index) {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    maximum = maximum.max(fingerprint_binding_type_scratch(binding)?);
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Statements(statements, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(value, depth))?;
                }
            }
            Frame::Fields(fields, index, depth) => {
                if let Some(field) = fields.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Fields(fields, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(&field.value, depth))?;
                }
            }
            Frame::Arms(arms, index, depth) => {
                if let Some(arm) = arms.get(index) {
                    maximum = maximum.max(fingerprint_pattern_types_scratch(&arm.pattern)?);
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Arms(arms, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(&arm.value, depth))?;
                }
            }
        }
    }
    Ok(maximum)
}

fn fingerprint_type_scratch_upper(closure: &[&ResolvedFunction]) -> Result<usize, Diagnostic> {
    closure.iter().try_fold(0usize, |mut maximum, function| {
        maximum = maximum.max(type_identity_scratch_upper(&function.return_type)?);
        for parameter in &function.params {
            maximum = maximum.max(type_identity_scratch_upper(&parameter.ty)?);
        }
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            maximum = maximum.max(fingerprint_expression_types_scratch(expression, 1)?);
        }
        Ok(maximum)
    })
}

fn hir_fingerprint(
    closure: &[&ResolvedFunction],
    imports: &[ImportFact],
    _capacity_baseline: usize,
) -> Result<String, Diagnostic> {
    let mut hasher = Sha256::new();
    hasher.update(HIR_DOMAIN);
    hash_count(&mut hasher, "functions", closure.len());
    for function in closure {
        frame(&mut hasher, b"function");
        frame(&mut hasher, function.id.as_str().as_bytes());
        frame(&mut hasher, function.name.as_bytes());
        frame(&mut hasher, function.result_id.as_str().as_bytes());
        frame(&mut hasher, function.body.id.as_str().as_bytes());
        let return_identity =
            fingerprint_type_identity(&function.return_type, _capacity_baseline, 0)?;
        #[cfg(test)]
        note_post_hir_facts_live(_capacity_baseline, return_identity.capacity());
        frame(&mut hasher, return_identity.as_bytes());
        hash_count(&mut hasher, "effects", function.effects.len());
        for effect in &function.effects {
            frame(&mut hasher, effect.as_bytes());
        }
        hash_count(&mut hasher, "parameters", function.params.len());
        for parameter in &function.params {
            frame(&mut hasher, parameter.id.as_str().as_bytes());
            frame(&mut hasher, parameter.name.as_bytes());
            frame(
                &mut hasher,
                match parameter.ownership {
                    OwnershipMode::Value => b"value",
                    OwnershipMode::Own => b"own",
                    OwnershipMode::Borrow => b"borrow",
                    OwnershipMode::Shared => b"shared",
                },
            );
            let parameter_identity =
                fingerprint_type_identity(&parameter.ty, _capacity_baseline, 0)?;
            #[cfg(test)]
            note_post_hir_facts_live(_capacity_baseline, parameter_identity.capacity());
            frame(&mut hasher, parameter_identity.as_bytes());
        }
        hash_count(&mut hasher, "requires", function.requires.len());
        for requirement in &function.requires {
            hash_expr(&mut hasher, requirement, _capacity_baseline)?;
        }
        frame(&mut hasher, b"body");
        hash_expr(&mut hasher, &function.body, _capacity_baseline)?;
        hash_count(&mut hasher, "ensures", function.ensures.len());
        for guarantee in &function.ensures {
            hash_expr(&mut hasher, guarantee, _capacity_baseline)?;
        }
    }
    hash_count(&mut hasher, "imports", imports.len());
    for import in imports {
        frame(&mut hasher, b"import");
        frame(&mut hasher, import.id.as_bytes());
        frame(&mut hasher, import.interface.as_bytes());
        frame(&mut hasher, import.import_key.as_bytes());
        frame(&mut hasher, scalar_text(import.result).as_bytes());
        hash_count(&mut hasher, "import-parameters", import.parameters.len());
        for parameter in &import.parameters {
            frame(&mut hasher, parameter.name.as_bytes());
            frame(&mut hasher, scalar_text(parameter.ty).as_bytes());
        }
        hash_count(&mut hasher, "import-effects", import.effects.len());
        for effect in &import.effects {
            frame(&mut hasher, effect.as_bytes());
        }
        frame(
            &mut hasher,
            import.failure.as_deref().unwrap_or("infallible").as_bytes(),
        );
        frame(&mut hasher, import.call_contract_digest.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    note_post_hir_facts_live(_capacity_baseline, digest.capacity());
    Ok(digest)
}

fn hash_count(hasher: &mut Sha256, label: &str, count: usize) {
    frame(hasher, label.as_bytes());
    frame(
        hasher,
        &u64::try_from(count).unwrap_or(u64::MAX).to_be_bytes(),
    );
}

enum HirFingerprintAction<'a> {
    Expr(&'a ResolvedExpr, usize),
    Exprs(&'a [ResolvedExpr], usize, usize),
    Statement(&'a ResolvedStatement, usize),
    Statements(&'a [ResolvedStatement], usize, usize),
    Field(&'a crate::hir::ResolvedFieldInitializer, usize),
    Fields(&'a [crate::hir::ResolvedFieldInitializer], usize, usize),
    Pattern(&'a crate::hir::ResolvedMatchPattern),
    RecordPatternField(&'a crate::hir::ResolvedRecordMatchPatternField),
    RecordPatternFields(&'a [crate::hir::ResolvedRecordMatchPatternField], usize),
    Arms(&'a [crate::hir::ResolvedMatchArm], usize, usize),
    TryIds([&'a DeclarationId; 5], usize),
    OptionIds([&'a DeclarationId; 4], usize),
    Bytes(&'a [u8]),
    Type(&'a ResolvedType),
}

fn hash_expr(
    hasher: &mut Sha256,
    expression: &ResolvedExpr,
    _capacity_baseline: usize,
) -> Result<(), Diagnostic> {
    let ownership = |ownership| match ownership {
        OwnershipMode::Value => b"value".as_slice(),
        OwnershipMode::Own => b"own".as_slice(),
        OwnershipMode::Borrow => b"borrow".as_slice(),
        OwnershipMode::Shared => b"shared".as_slice(),
    };
    let mut actions = Vec::with_capacity(MAX_SEMANTIC_EXPRESSION_DEPTH * 4 + 8);
    actions.push(HirFingerprintAction::Expr(expression, 1));
    while let Some(action) = actions.pop() {
        if actions.len() + 4 > actions.capacity() {
            return Err(b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            ));
        }
        match action {
            HirFingerprintAction::Bytes(value) => frame(hasher, value),
            HirFingerprintAction::Type(ty) => {
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let identity = fingerprint_type_identity(ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(identity.capacity()),
                );
                frame(hasher, identity.as_bytes());
            }
            HirFingerprintAction::Statement(statement, depth) => {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                frame(hasher, b"let");
                frame(hasher, binding.id.as_str().as_bytes());
                frame(hasher, binding.name.as_bytes());
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let binding_identity =
                    fingerprint_type_identity(&binding.ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(binding_identity.capacity()),
                );
                frame(hasher, binding_identity.as_bytes());
                frame(hasher, ownership(binding.ownership));
                actions.push(HirFingerprintAction::Expr(value, depth));
            }
            HirFingerprintAction::Statements(statements, index, depth) => {
                if let Some(statement) = statements.get(index) {
                    actions.push(HirFingerprintAction::Statements(
                        statements,
                        index + 1,
                        depth,
                    ));
                    actions.push(HirFingerprintAction::Statement(statement, depth));
                }
            }
            HirFingerprintAction::Exprs(expressions, index, depth) => {
                if let Some(expression) = expressions.get(index) {
                    actions.push(HirFingerprintAction::Exprs(expressions, index + 1, depth));
                    actions.push(HirFingerprintAction::Expr(expression, depth));
                }
            }
            HirFingerprintAction::TryIds(ids, index) => {
                if let Some(id) = ids.get(index) {
                    actions.push(HirFingerprintAction::TryIds(ids, index + 1));
                    actions.push(HirFingerprintAction::Bytes(id.as_str().as_bytes()));
                }
            }
            HirFingerprintAction::OptionIds(ids, index) => {
                if let Some(id) = ids.get(index) {
                    actions.push(HirFingerprintAction::OptionIds(ids, index + 1));
                    actions.push(HirFingerprintAction::Bytes(id.as_str().as_bytes()));
                }
            }
            HirFingerprintAction::Field(field, depth) => {
                frame(hasher, field.field.as_str().as_bytes());
                actions.push(HirFingerprintAction::Expr(&field.value, depth));
            }
            HirFingerprintAction::Fields(fields, index, depth) => {
                if index == 0 {
                    hash_count(hasher, "fields", fields.len());
                }
                if let Some(field) = fields.get(index) {
                    actions.push(HirFingerprintAction::Fields(fields, index + 1, depth));
                    actions.push(HirFingerprintAction::Field(field, depth));
                }
            }
            HirFingerprintAction::Arms(arms, index, depth) => {
                if index == 0 {
                    hash_count(hasher, "arms", arms.len());
                }
                if let Some(arm) = arms.get(index) {
                    actions.push(HirFingerprintAction::Arms(arms, index + 1, depth));
                    actions.push(HirFingerprintAction::Expr(&arm.value, depth));
                    actions.push(HirFingerprintAction::Pattern(&arm.pattern));
                }
            }
            HirFingerprintAction::RecordPatternFields(fields, index) => {
                if let Some(field) = fields.get(index) {
                    actions.push(HirFingerprintAction::RecordPatternFields(fields, index + 1));
                    actions.push(HirFingerprintAction::RecordPatternField(field));
                }
            }
            HirFingerprintAction::RecordPatternField(field) => {
                frame(hasher, field.field.as_str().as_bytes());
                match &field.pattern {
                    crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                        frame(hasher, b"binding");
                        hash_binding(
                            hasher,
                            binding,
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                        )?;
                    }
                    crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                        frame(hasher, b"wildcard");
                    }
                    crate::hir::ResolvedRecordMatchFieldPattern::Record {
                        record,
                        instance,
                        fields,
                    } => {
                        frame(hasher, b"record");
                        frame(hasher, record.as_str().as_bytes());
                        let action_bytes = actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                        let instance_identity =
                            fingerprint_type_identity(instance, _capacity_baseline, action_bytes)?;
                        #[cfg(test)]
                        note_post_hir_facts_live(
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                .saturating_add(instance_identity.capacity()),
                        );
                        frame(hasher, instance_identity.as_bytes());
                        hash_count(hasher, "record-pattern-fields", fields.len());
                        actions.push(HirFingerprintAction::RecordPatternFields(fields, 0));
                    }
                }
            }
            HirFingerprintAction::Pattern(pattern) => match pattern {
                crate::hir::ResolvedMatchPattern::Wildcard => frame(hasher, b"wildcard"),
                crate::hir::ResolvedMatchPattern::Variant {
                    variant,
                    case,
                    fields,
                } => {
                    frame(hasher, b"variant");
                    frame(hasher, variant.as_str().as_bytes());
                    frame(hasher, case.as_str().as_bytes());
                    hash_count(hasher, "variant-pattern-fields", fields.len());
                    for field in fields {
                        frame(hasher, field.field.as_str().as_bytes());
                        hash_binding(
                            hasher,
                            &field.binding,
                            _capacity_baseline,
                            actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
                        )?;
                    }
                }
                crate::hir::ResolvedMatchPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    frame(hasher, b"record");
                    frame(hasher, record.as_str().as_bytes());
                    let action_bytes = actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                    let instance_identity =
                        fingerprint_type_identity(instance, _capacity_baseline, action_bytes)?;
                    #[cfg(test)]
                    note_post_hir_facts_live(
                        _capacity_baseline,
                        actions
                            .capacity()
                            .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                            .saturating_add(instance_identity.capacity()),
                    );
                    frame(hasher, instance_identity.as_bytes());
                    hash_count(hasher, "record-pattern-fields", fields.len());
                    actions.push(HirFingerprintAction::RecordPatternFields(fields, 0));
                }
            },
            HirFingerprintAction::Expr(expression, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    )
                })?;
                frame(hasher, b"expression");
                frame(hasher, expression.id.as_str().as_bytes());
                let action_bytes = actions
                    .capacity()
                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                let identity =
                    fingerprint_type_identity(&expression.ty, _capacity_baseline, action_bytes)?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    actions
                        .capacity()
                        .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                        .saturating_add(identity.capacity()),
                );
                frame(hasher, identity.as_bytes());
                frame(hasher, ownership(expression.ownership));
                match &expression.kind {
                    ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_) => {
                        // Non-i64 scalar signatures are outside the scalar
                        // native boundary; admission rejects them first.
                        return Err(b107("scalar value signature required"));
                    }
                    ResolvedExprKind::Int(value) => {
                        frame(hasher, b"int");
                        frame(hasher, &value.to_be_bytes());
                    }
                    ResolvedExprKind::Bool(value) => {
                        frame(hasher, b"bool");
                        frame(hasher, &[*value as u8]);
                    }
                    ResolvedExprKind::Place(place) => {
                        frame(hasher, b"place");
                        frame(hasher, place.root.as_str().as_bytes());
                        hash_count(hasher, "projections", place.projections.len());
                        for projection in &place.projections {
                            match projection {
                                crate::hir::PlaceProjection::Field(field) => {
                                    frame(hasher, b"field");
                                    frame(hasher, field.as_str().as_bytes());
                                }
                                crate::hir::PlaceProjection::VariantField { case, field } => {
                                    frame(hasher, b"variant-field");
                                    frame(hasher, case.as_str().as_bytes());
                                    frame(hasher, field.as_str().as_bytes());
                                }
                            }
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        type_arguments,
                        instance,
                        args,
                    } => {
                        frame(hasher, b"call");
                        frame(hasher, callee.as_str().as_bytes());
                        hash_count(hasher, "type-arguments", type_arguments.len());
                        for argument in type_arguments {
                            let action_bytes = actions
                                .capacity()
                                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>());
                            let argument_identity = fingerprint_type_identity(
                                argument,
                                _capacity_baseline,
                                action_bytes,
                            )?;
                            #[cfg(test)]
                            note_post_hir_facts_live(
                                _capacity_baseline,
                                actions
                                    .capacity()
                                    .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>())
                                    .saturating_add(argument_identity.capacity()),
                            );
                            frame(hasher, argument_identity.as_bytes());
                        }
                        frame(
                            hasher,
                            instance
                                .as_ref()
                                .map_or(b"".as_slice(), |value| value.as_str().as_bytes()),
                        );
                        hash_count(hasher, "arguments", args.len());
                        actions.push(HirFingerprintAction::Exprs(args, 0, child_depth));
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => {
                        frame(hasher, b"native-rust-import");
                        frame(hasher, call.expression.as_str().as_bytes());
                        frame(hasher, call.import.as_str().as_bytes());
                        frame(
                            hasher,
                            match call.result {
                                ResolvedImportResultKind::Unit => b"unit",
                                ResolvedImportResultKind::I64 => b"i64",
                                ResolvedImportResultKind::Bool => b"bool",
                            },
                        );
                        hash_count(hasher, "arguments", call.args.len());
                        actions.push(HirFingerprintAction::Exprs(&call.args, 0, child_depth));
                    }
                    ResolvedExprKind::Unary { op, value } => {
                        frame(
                            hasher,
                            match op {
                                crate::ast::UnaryOp::Neg => b"unary-neg",
                                crate::ast::UnaryOp::Not => b"unary-not",
                            },
                        );
                        actions.push(HirFingerprintAction::Expr(value, child_depth));
                    }
                    ResolvedExprKind::Binary { op, left, right } => {
                        frame(
                            hasher,
                            match op {
                                crate::ast::BinaryOp::Add => b"binary-add",
                                crate::ast::BinaryOp::Sub => b"binary-sub",
                                crate::ast::BinaryOp::Mul => b"binary-mul",
                                crate::ast::BinaryOp::Div => b"binary-div",
                                crate::ast::BinaryOp::Rem => b"binary-rem",
                                crate::ast::BinaryOp::Eq => b"binary-eq",
                                crate::ast::BinaryOp::Ne => b"binary-ne",
                                crate::ast::BinaryOp::Lt => b"binary-lt",
                                crate::ast::BinaryOp::Le => b"binary-le",
                                crate::ast::BinaryOp::Gt => b"binary-gt",
                                crate::ast::BinaryOp::Ge => b"binary-ge",
                                crate::ast::BinaryOp::And => b"binary-and",
                                crate::ast::BinaryOp::Or => b"binary-or",
                            },
                        );
                        actions.push(HirFingerprintAction::Expr(right, child_depth));
                        actions.push(HirFingerprintAction::Expr(left, child_depth));
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        frame(hasher, b"block");
                        hash_count(hasher, "statements", statements.len());
                        actions.push(HirFingerprintAction::Expr(tail, child_depth));
                        actions.push(HirFingerprintAction::Statements(statements, 0, child_depth));
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        frame(hasher, b"if");
                        actions.push(HirFingerprintAction::Expr(else_branch, child_depth));
                        actions.push(HirFingerprintAction::Expr(then_branch, child_depth));
                        actions.push(HirFingerprintAction::Expr(condition, child_depth));
                    }
                    ResolvedExprKind::ConstructRecord { record, fields } => {
                        frame(hasher, b"construct-record");
                        frame(hasher, record.as_str().as_bytes());
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                    }
                    ResolvedExprKind::ConstructVariant {
                        variant,
                        case,
                        fields,
                    } => {
                        frame(hasher, b"construct-variant");
                        frame(hasher, variant.as_str().as_bytes());
                        frame(hasher, case.as_str().as_bytes());
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                    }
                    ResolvedExprKind::Match { scrutinee, arms } => {
                        frame(hasher, b"match");
                        actions.push(HirFingerprintAction::Arms(arms, 0, child_depth));
                        actions.push(HirFingerprintAction::Expr(scrutinee, child_depth));
                    }
                    ResolvedExprKind::Try {
                        operand,
                        result,
                        ok_case,
                        ok_field,
                        err_case,
                        err_field,
                        residual_type,
                    } => {
                        frame(hasher, b"try");
                        let ids = [result, ok_case, ok_field, err_case, err_field];
                        actions.push(HirFingerprintAction::Type(residual_type));
                        actions.push(HirFingerprintAction::TryIds(ids, 0));
                        actions.push(HirFingerprintAction::Expr(operand, child_depth));
                    }
                    ResolvedExprKind::TryOption {
                        operand,
                        option,
                        some_case,
                        some_field,
                        none_case,
                        residual_type,
                    } => {
                        frame(hasher, b"try-option");
                        let ids = [option, some_case, some_field, none_case];
                        actions.push(HirFingerprintAction::Type(residual_type));
                        actions.push(HirFingerprintAction::OptionIds(ids, 0));
                        actions.push(HirFingerprintAction::Expr(operand, child_depth));
                    }
                    ResolvedExprKind::UpdateRecord {
                        base,
                        record,
                        fields,
                    } => {
                        frame(hasher, b"update-record");
                        actions.push(HirFingerprintAction::Fields(fields, 0, child_depth));
                        actions.push(HirFingerprintAction::Bytes(record.as_str().as_bytes()));
                        actions.push(HirFingerprintAction::Expr(base, child_depth));
                    }
                    ResolvedExprKind::Project { base, field } => {
                        frame(hasher, b"project");
                        actions.push(HirFingerprintAction::Bytes(field.as_str().as_bytes()));
                        actions.push(HirFingerprintAction::Expr(base, child_depth));
                    }
                }
            }
        }
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            actions
                .capacity()
                .saturating_mul(std::mem::size_of::<HirFingerprintAction<'_>>()),
        );
    }
    Ok(())
}

fn hash_binding(
    hasher: &mut Sha256,
    binding: &crate::hir::ResolvedBinding,
    _capacity_baseline: usize,
    _action_bytes: usize,
) -> Result<(), Diagnostic> {
    frame(hasher, binding.id.as_str().as_bytes());
    frame(hasher, binding.name.as_bytes());
    let binding_identity =
        fingerprint_type_identity(&binding.ty, _capacity_baseline, _action_bytes)?;
    #[cfg(test)]
    note_post_hir_facts_live(
        _capacity_baseline,
        _action_bytes.saturating_add(binding_identity.capacity()),
    );
    frame(hasher, binding_identity.as_bytes());
    frame(
        hasher,
        match binding.ownership {
            OwnershipMode::Value => b"value",
            OwnershipMode::Own => b"own",
            OwnershipMode::Borrow => b"borrow",
            OwnershipMode::Shared => b"shared",
        },
    );
    Ok(())
}

/// Pure private phase-A preparation. It performs no filesystem, process, or
/// network operation.
pub(crate) fn prepare_native_rust_interop(
    program: &Program,
    spec_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Vec<Diagnostic>> {
    let result = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        prepare_native_rust_interop_bounded(program, spec_bytes)
    });
    if result.1 {
        return Err(vec![b109("max_builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.0.map_err(|error| vec![error])
}

#[cfg(test)]
fn prepare_native_rust_interop_with_test_limit(
    program: &Program,
    spec_bytes: &[u8],
    limit: usize,
) -> Result<PreparedNativeRustInterop, Vec<Diagnostic>> {
    assert!(limit <= MAX_BUILDER_BYTES);
    let (result, overflowed) = crate::bounded_output::with_limit(limit, || {
        prepare_native_rust_interop_bounded(program, spec_bytes)
    });
    if overflowed {
        return Err(vec![b109("max_builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.map_err(|error| vec![error])
}

fn prepare_native_rust_interop_bounded(
    program: &Program,
    spec_bytes: &[u8],
) -> Result<PreparedNativeRustInterop, Diagnostic> {
    validate_native_rust_source_expression_budget(program)?;
    debit(spec_bytes.len())?;
    let canonical_source = canonical_source_bounded(program)?;
    let (spec, spec_authority) =
        parse_spec_with_source_authority(program, spec_bytes, &canonical_source)?;
    #[cfg(test)]
    let spec_transfer_allocations = (
        spec.source_revision.as_ptr(),
        spec.target.triple.as_ptr(),
        spec.target.endian.as_ptr(),
        spec.target.panic_strategy.as_ptr(),
        spec.target.thread_policy.as_ptr(),
    );
    identifier_audit(program, &spec)?;
    let canonical_spec_budget = reserve_temporary_exact(MAX_SPEC_BYTES)?;
    let canonical_spec = render_spec(&spec);
    canonical_spec_budget.retain(canonical_spec.capacity())?;
    let mut hir_scan_stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let hir_capacity =
        hir_pre_resolve_capacity(program, canonical_source.len(), &mut hir_scan_stack)?;
    let mut hir_budget = reserve_temporary_exact(
        hir_capacity
            .complete()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    )?;
    let dispose_frames = Vec::with_capacity(hir_capacity.disposal_frames);
    note_hir_resolve_pass();
    let resolved_owner = ResolvedProgramOwner::new(
        hir::resolve(program).map_err(|_| b107("selected identity missing"))?,
        dispose_frames,
        hir_capacity.disposal_frames,
    );
    let resolved = resolved_owner.program();
    let (closure, reached_imports) = selected_closure(resolved, &spec.exports)?;
    validate_native_rust_expression_budget_for_closure(&closure, true)?;
    validate_selected_scalar_closure(&closure)?;
    validate_native_unit_discard_bindings(&closure)?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Closure)?;
    // Keep the complete reservation through the post-resolution closure and
    // validation phases: their maps, DFS stacks, and pending vectors are part
    // of `scratch_upper`. Only after every such phase settles may the shared
    // sequential scratch be released while the conservative retained HIR and
    // selected-function clone ceiling remain authorized.
    // `DeclarationIndex` is intentionally opaque across the crate boundary.
    // Its maps contain only declaration identities/type facts derived from
    // canonical source; charge a separate source-derived upper while every
    // public ResolvedProgram field and selected clone is exact-censused.
    let declaration_index_upper = hir_capacity.declaration_index_upper;
    let actual_hir_retained = hir_owned_capacity(resolved)?
        .checked_add(declaration_index_upper)
        .and_then(|bytes| {
            bytes.checked_add(
                hir_capacity
                    .disposal_frames
                    .checked_mul(std::mem::size_of::<ResolvedDisposeFrame>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if actual_hir_retained > hir_capacity.retained_upper {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    hir_budget.shrink_held(actual_hir_retained)?;
    let facts_capacity = post_hir_facts_capacity(
        canonical_source.len(),
        canonical_spec.len(),
        resolved,
        &closure,
        &spec,
    )?;
    let spec_transfer_capacity = prepared_spec_transfer_capacity(&spec)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The source revision and target strings are still owned by `spec` and
    // therefore already covered by `spec_authority`. Reserve only the new
    // facts topology here; the existing authority is narrowed and retained
    // when those exact allocations move into Prepared below.
    let facts_complete_without_spec_transfer = facts_capacity
        .complete()
        .and_then(|complete| complete.checked_sub(spec_transfer_capacity))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let facts_budget = reserve_temporary_exact(facts_complete_without_spec_transfer)?;
    #[cfg(test)]
    note_post_hir_facts_entry();
    if reached_imports != spec.imports.iter().cloned().collect() {
        return Err(b107("unselected import reached"));
    }
    let mut selected_effects = BTreeSet::new();
    for function in &closure {
        identifier_gate(function.id.as_str())?;
        identifier_gate(&function.name)?;
        if function.effects.len() > MAX_EFFECTS {
            return Err(b109("max_effects", MAX_EFFECTS));
        }
        for effect in &function.effects {
            identifier_gate(effect)?;
            selected_effects.insert(effect.as_str());
        }
    }
    let source_functions = program
        .functions
        .iter()
        .map(|function| (function.stable_id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    for id in &spec.exports {
        let function = source_functions
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        if !function.explicit_id
            || function.name == "main"
            || !function.type_parameters.is_empty()
            || function.params.len() > MAX_PARAMETERS
            || function.params.iter().any(|parameter| {
                parameter.mode != ParamMode::Value || source_scalar_type(&parameter.ty).is_none()
            })
            || source_scalar_type(&function.return_type).is_none()
        {
            return Err(b107(if !function.explicit_id {
                "explicit persistent ID required"
            } else {
                "scalar value signature required"
            }));
        }
    }

    let resolved_import_count = resolved
        .interfaces
        .iter()
        .try_fold(0usize, |count, interface| {
            count.checked_add(interface.imports.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut resolved_imports = Vec::with_capacity(resolved_import_count);
    resolved_imports.extend(resolved.interfaces.iter().flat_map(|interface| {
        interface
            .imports
            .iter()
            .map(move |import| (interface.id.as_str(), import))
    }));
    #[cfg(test)]
    note_post_hir_facts_capacity(post_hir_selection_scratch_capacity(
        &selected_effects,
        &source_functions,
        &resolved_imports,
    ));
    #[cfg(test)]
    note_post_hir_facts_scratch(post_hir_selection_scratch_capacity(
        &selected_effects,
        &source_functions,
        &resolved_imports,
    ));
    let mut import_facts = Vec::with_capacity(spec.imports.len());
    for id in &spec.imports {
        let (interface, import) = resolved_imports
            .iter()
            .find(|(_, import)| import.id.as_str() == id)
            .copied()
            .ok_or_else(|| b107("selected identity missing"))?;
        if !import.native_rust {
            return Err(b107("selected identity missing"));
        }
        identifier_gate(interface)?;
        identifier_gate(import.id.as_str())?;
        identifier_gate(&import.name)?;
        if import.effects.len() > MAX_EFFECTS {
            return Err(b109("max_effects", MAX_EFFECTS));
        }
        for effect in &import.effects {
            identifier_gate(effect)?;
            selected_effects.insert(effect.as_str());
        }
        let parameters = import_parameter_facts(import)?;
        let result = match import.result.kind {
            ResolvedImportResultKind::Unit => ScalarType::Unit,
            ResolvedImportResultKind::I64 => ScalarType::I64,
            ResolvedImportResultKind::Bool => ScalarType::Bool,
        };
        let failure = match &import.failure {
            ResolvedImportFailure::Infallible => None,
            ResolvedImportFailure::Status { domain_id, .. } => Some(domain_id.clone()),
        };
        let hash = full_hash(id);
        let effect_set = import.effects.iter().cloned().collect::<BTreeSet<_>>();
        #[cfg(test)]
        let import_effect_baseline = post_hir_facts_owned_capacity(&Vec::new(), &import_facts);
        #[cfg(test)]
        let import_effect_outer_scratch = post_hir_selection_scratch_capacity(
            &selected_effects,
            &source_functions,
            &resolved_imports,
        );
        #[cfg(test)]
        let import_effect_locals =
            parameter_facts_owned_capacity(&parameters, parameters.capacity())
                .saturating_add(failure.as_ref().map_or(0, String::capacity))
                .saturating_add(hash.capacity());
        #[cfg(test)]
        note_post_hir_facts_live(
            import_effect_baseline,
            import_effect_outer_scratch
                .saturating_add(import_effect_locals)
                .saturating_add(
                    checked_owned_string_set(&effect_set)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                ),
        );
        let mut effects = Vec::with_capacity(effect_set.len());
        let mut remaining_effects = effect_set;
        while let Some(effect) = remaining_effects.pop_first() {
            effects.push(effect);
            #[cfg(test)]
            note_post_hir_facts_live(
                import_effect_baseline,
                import_effect_outer_scratch
                    .saturating_add(import_effect_locals)
                    .saturating_add(
                        checked_owned_string_set(&remaining_effects)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(&effects, effects.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    ),
            );
        }
        import_facts.push(ImportFact {
            id: id.clone(),
            interface: interface.to_owned(),
            import_key: import.import_key.clone(),
            rust_method: format!("import_{hash}"),
            c_field: format!("spxnr1_i_{hash}"),
            parameters,
            result,
            effects: effects.clone(),
            capabilities: effects,
            failure,
            call_contract_digest: String::new(),
        });
        #[cfg(test)]
        note_post_hir_facts_live(
            post_hir_facts_owned_capacity(&Vec::new(), &import_facts),
            post_hir_selection_scratch_capacity(
                &selected_effects,
                &source_functions,
                &resolved_imports,
            )
            .saturating_add(hash.capacity()),
        );
    }
    import_facts.sort_by(|left, right| left.id.cmp(&right.id));
    if selected_effects.len() > MAX_EFFECTS {
        return Err(b109("max_effects", MAX_EFFECTS));
    }
    let selected_capability_set = import_facts
        .iter()
        .flat_map(|import| import.capabilities.iter().cloned())
        .collect::<BTreeSet<_>>();
    #[cfg(test)]
    let selected_capability_baseline = post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
        .saturating_add(post_hir_selection_scratch_capacity(
            &selected_effects,
            &source_functions,
            &resolved_imports,
        ));
    #[cfg(test)]
    let selected_capability_set_owned = checked_owned_string_set(&selected_capability_set)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_facts_live(selected_capability_baseline, selected_capability_set_owned);
    let mut selected_capabilities = Vec::with_capacity(selected_capability_set.len());
    for capability in selected_capability_set {
        selected_capabilities.push(capability);
        #[cfg(test)]
        note_post_hir_facts_live(
            selected_capability_baseline,
            selected_capability_set_owned.saturating_add(
                checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            ),
        );
    }
    if selected_capabilities != spec.capabilities {
        return Err(b107("effect or capability mismatch"));
    }

    let status_domain_set = import_facts
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>();
    #[cfg(test)]
    let status_domain_set_owned = checked_owned_string_set(&status_domain_set)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let status_phase_outer_scratch = post_hir_selection_scratch_capacity(
        &selected_effects,
        &source_functions,
        &resolved_imports,
    )
    .checked_add(
        checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let status_domain_conversion_baseline =
        post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
            .checked_add(status_phase_outer_scratch)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_facts_capacity(
        status_domain_conversion_baseline
            .checked_add(status_domain_set_owned)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    #[cfg(test)]
    note_post_hir_facts_scratch(
        status_phase_outer_scratch
            .checked_add(status_domain_set_owned)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    let mut status_domains = Vec::with_capacity(status_domain_set.len());
    for domain in status_domain_set {
        status_domains.push(domain);
        #[cfg(test)]
        {
            let status_domain_conversion_scratch = status_domain_set_owned
                .checked_add(
                    checked_owned_string_vec(&status_domains, status_domains.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            note_post_hir_facts_scratch(
                status_phase_outer_scratch
                    .checked_add(status_domain_conversion_scratch)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            );
            note_post_hir_facts_capacity(
                status_domain_conversion_baseline
                    .checked_add(status_domain_conversion_scratch)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            );
        }
    }
    if status_domains
        .len()
        .checked_add(4)
        .is_none_or(|count| count > MAX_STATUS_DOMAINS)
    {
        return Err(b109("max_status_domains", MAX_STATUS_DOMAINS));
    }
    let ordinals = status_domains
        .iter()
        .enumerate()
        .map(|(index, domain)| {
            (
                domain.as_str(),
                u16::try_from(index + 1).unwrap_or(u16::MAX),
            )
        })
        .collect::<BTreeMap<_, _>>();
    #[cfg(test)]
    note_post_hir_facts_capacity(
        post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
            + post_hir_selection_scratch_capacity(
                &selected_effects,
                &source_functions,
                &resolved_imports,
            )
            + checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + checked_owned_string_vec(&status_domains, status_domains.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + borrowed_map_owned_capacity::<&str, u16>(ordinals.len()),
    );
    #[allow(clippy::needless_range_loop)]
    for index in 0..import_facts.len() {
        #[cfg(test)]
        let import_digest_baseline = post_hir_facts_owned_capacity(&Vec::new(), &import_facts)
            + post_hir_selection_scratch_capacity(
                &selected_effects,
                &source_functions,
                &resolved_imports,
            )
            + checked_owned_string_vec(&selected_capabilities, selected_capabilities.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + checked_owned_string_vec(&status_domains, status_domains.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            + borrowed_map_owned_capacity::<&str, u16>(ordinals.len())
            + owned_string_set_owned_capacity(&reached_imports);
        #[cfg(not(test))]
        let import_digest_baseline = 0;
        let import = &mut import_facts[index];
        let failure = import.failure.as_ref().map_or_else(
            || "infallible".to_owned(),
            |domain| {
                format!(
                    "{}:{domain}",
                    ordinals.get(domain.as_str()).copied().unwrap_or(u16::MAX)
                )
            },
        );
        import.call_contract_digest = call_digest(
            "import",
            &import.id,
            &import.parameters,
            import.result,
            &import.effects,
            &import.capabilities,
            &[],
            &[],
            &failure,
            import_digest_baseline + failure.capacity(),
            &spec.target,
        )?;
    }

    let by_function = closure
        .iter()
        .map(|function| (function.id.as_str(), *function))
        .collect::<BTreeMap<_, _>>();
    for function in &closure {
        #[cfg(test)]
        let traversal_baseline = post_hir_live_facts_capacity(
            &Vec::new(),
            &import_facts,
            &selected_effects,
            &source_functions,
            &resolved_imports,
            &selected_capabilities,
            &status_domains,
            &ordinals,
            &by_function,
        );
        #[cfg(not(test))]
        let traversal_baseline = 0;
        let reachable = transitive_imports(
            function,
            &by_function,
            facts_capacity.traversal_pending_capacity,
            traversal_baseline,
        )?;
        let reachable_effects = reachable
            .iter()
            .filter_map(|id| import_facts.iter().find(|import| import.id == id.as_str()))
            .flat_map(|import| import.effects.iter().cloned())
            .collect::<BTreeSet<_>>();
        let declared = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        #[cfg(test)]
        note_post_hir_facts_live(
            traversal_baseline,
            owned_string_set_owned_capacity(&reachable)
                .saturating_add(owned_string_set_owned_capacity(&reachable_effects))
                .saturating_add(owned_string_set_owned_capacity(&declared)),
        );
        if declared != reachable_effects {
            return Err(b107("effect or capability mismatch"));
        }
    }
    let mut export_facts = Vec::with_capacity(spec.exports.len());
    for id in &spec.exports {
        let function = by_function
            .get(id.as_str())
            .ok_or_else(|| b107("selected identity missing"))?;
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type)
            .filter(|ty| *ty != ScalarType::Unit)
            .ok_or_else(|| b107("scalar value signature required"))?;
        #[cfg(test)]
        let traversal_baseline = post_hir_live_facts_capacity(
            &export_facts,
            &import_facts,
            &selected_effects,
            &source_functions,
            &resolved_imports,
            &selected_capabilities,
            &status_domains,
            &ordinals,
            &by_function,
        );
        #[cfg(not(test))]
        let traversal_baseline = 0;
        let reachable_imports = transitive_imports(
            function,
            &by_function,
            facts_capacity.traversal_pending_capacity,
            traversal_baseline,
        )?;
        let capabilities = spec.capabilities.clone();
        let mut required_imports = Vec::with_capacity(import_facts.len());
        required_imports.extend(import_facts.iter().map(|import| import.id.clone()));
        let mut required_import_contracts = Vec::with_capacity(import_facts.len());
        required_import_contracts.extend(
            import_facts
                .iter()
                .map(|import| (import.id.clone(), import.call_contract_digest.clone())),
        );
        #[cfg(test)]
        let export_prefix_baseline = traversal_baseline
            .saturating_add(parameter_facts_owned_capacity(
                &parameters,
                parameters.capacity(),
            ))
            .saturating_add(owned_string_set_owned_capacity(&reachable_imports))
            .saturating_add(
                checked_owned_string_vec(&capabilities, capabilities.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(
                checked_owned_string_vec(&required_imports, required_imports.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(
                checked_owned_string_pairs(&required_import_contracts)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            );
        let effect_set = function.effects.iter().cloned().collect::<BTreeSet<_>>();
        #[cfg(test)]
        let effect_set_owned = checked_owned_string_set(&effect_set)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        #[cfg(test)]
        note_post_hir_facts_live(export_prefix_baseline, effect_set_owned);
        let mut effects = Vec::with_capacity(effect_set.len());
        for effect in effect_set {
            effects.push(effect);
            #[cfg(test)]
            note_post_hir_facts_live(
                export_prefix_baseline,
                effect_set_owned.saturating_add(
                    checked_owned_string_vec(&effects, effects.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                ),
            );
        }
        let status_domain_ordinal_set = reachable_imports
            .iter()
            .filter_map(|id| import_facts.iter().find(|import| import.id == id.as_str()))
            .filter_map(|import| import.failure.as_deref())
            .filter_map(|domain| ordinals.get(domain).copied())
            .collect::<BTreeSet<_>>();
        #[cfg(test)]
        let status_domain_ordinal_set_owned =
            btree_allocation_upper::<u16, ()>(status_domain_ordinal_set.len());
        #[cfg(test)]
        let status_ordinal_baseline = export_prefix_baseline.saturating_add(
            checked_owned_string_vec(&effects, effects.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        #[cfg(test)]
        note_post_hir_facts_live(status_ordinal_baseline, status_domain_ordinal_set_owned);
        let mut status_domain_ordinals = Vec::with_capacity(
            status_domain_ordinal_set
                .len()
                .checked_add(3)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        for ordinal in status_domain_ordinal_set {
            status_domain_ordinals.push(ordinal);
            #[cfg(test)]
            note_post_hir_facts_live(
                status_ordinal_baseline,
                status_domain_ordinal_set_owned.saturating_add(
                    checked_u16_vec(&status_domain_ordinals)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                ),
            );
        }
        status_domain_ordinals.extend([65_533, 65_534, 65_535]);
        status_domain_ordinals.sort_unstable();
        let status_contract_values = status_domain_ordinals
            .iter()
            .map(|ordinal| match *ordinal {
                65_533 => Ok::<_, Diagnostic>("65533:semaprax.native-rust-semantics.v1".to_owned()),
                65_534 => Ok("65534:semaprax.native-rust-host.v1".to_owned()),
                65_535 => Ok("65535:semaprax.native-rust-adapter.v1".to_owned()),
                _ => {
                    let domain = status_domains
                        .get(usize::from(*ordinal).saturating_sub(1))
                        .ok_or_else(b111)?;
                    Ok(format!("{ordinal}:{domain}"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        #[cfg(test)]
        let status_contract_baseline = status_ordinal_baseline.saturating_add(
            checked_u16_vec(&status_domain_ordinals)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        #[cfg(test)]
        note_post_hir_facts_live(
            status_contract_baseline,
            checked_owned_string_vec(&status_contract_values, status_contract_values.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        let status_contract = status_contract_values.join(";");
        #[cfg(test)]
        note_post_hir_facts_live(
            status_contract_baseline,
            checked_owned_string_vec(&status_contract_values, status_contract_values.capacity())
                .and_then(|bytes| bytes.checked_add(status_contract.capacity()))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
        let hash = full_hash(id);
        #[cfg(test)]
        let export_digest_baseline = status_contract_baseline
            .saturating_add(
                checked_owned_string_vec(
                    &status_contract_values,
                    status_contract_values.capacity(),
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(status_contract.capacity())
            .saturating_add(hash.capacity());
        #[cfg(not(test))]
        let export_digest_baseline = 0;
        let call_contract_digest = call_digest(
            "export",
            id,
            &parameters,
            result,
            &effects,
            &capabilities,
            &required_imports,
            &required_import_contracts,
            &status_contract,
            export_digest_baseline,
            &spec.target,
        )?;
        export_facts.push(ExportFact {
            id: id.clone(),
            rust_method: format!("export_{hash}"),
            c_symbol: format!("spxnr1_e_{hash}"),
            parameters: parameters.clone(),
            result,
            effects: effects.clone(),
            capabilities: capabilities.clone(),
            required_imports: required_imports.clone(),
            status_domain_ordinals,
            call_contract_digest,
        });
        #[cfg(test)]
        {
            let export_clone_overlap_scratch =
                parameter_facts_owned_capacity(&parameters, parameters.capacity())
                    .saturating_add(owned_string_set_owned_capacity(&reachable_imports))
                    .saturating_add(
                        checked_owned_string_vec(&capabilities, capabilities.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(&required_imports, required_imports.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_pairs(&required_import_contracts)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(&effects, effects.capacity())
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(
                        checked_owned_string_vec(
                            &status_contract_values,
                            status_contract_values.capacity(),
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .saturating_add(status_contract.capacity())
                    .saturating_add(hash.capacity());
            note_post_hir_facts_live(
                post_hir_live_facts_capacity(
                    &export_facts,
                    &import_facts,
                    &selected_effects,
                    &source_functions,
                    &resolved_imports,
                    &selected_capabilities,
                    &status_domains,
                    &ordinals,
                    &by_function,
                ),
                export_clone_overlap_scratch,
            );
        }
    }
    export_facts.sort_by(|left, right| left.id.cmp(&right.id));
    #[cfg(test)]
    note_post_hir_facts_capacity(post_hir_facts_owned_capacity(&export_facts, &import_facts));
    drop(by_function);
    drop(ordinals);
    drop(selected_capabilities);
    drop(resolved_imports);
    drop(source_functions);
    drop(selected_effects);
    drop(reached_imports);
    status_domains.shrink_to_fit();
    #[cfg(test)]
    let fingerprint_baseline = post_hir_facts_owned_capacity(&export_facts, &import_facts)
        + string_vec_owned_capacity(&status_domains, status_domains.capacity());
    #[cfg(not(test))]
    let fingerprint_baseline = 0;
    let hir_digest = hir_fingerprint(&closure, &import_facts, fingerprint_baseline)?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Facts)?;
    let descriptor_budget = reserve_temporary_exact(MAX_DESCRIPTOR_BYTES)?;
    let descriptor = render_descriptor(
        &spec,
        &hir_digest,
        &status_domains,
        &export_facts,
        &import_facts,
    )?;
    descriptor_budget.retain(descriptor.capacity())?;
    if descriptor.len() > MAX_DESCRIPTOR_BYTES {
        return Err(b109("max_descriptor_bytes", MAX_DESCRIPTOR_BYTES));
    }
    replay_descriptor(
        &descriptor,
        &spec,
        &hir_digest,
        &export_facts,
        &import_facts,
    )?;
    let header_budget = reserve_temporary_exact(MAX_GENERATED_HEADER_BYTES)?;
    let generated_header = generate_header(&export_facts, &import_facts)?;
    header_budget.retain(generated_header.capacity())?;
    let c_budget = reserve_temporary_exact(MAX_GENERATED_C_BYTES)?;
    let generated_c = generate_c(&spec, &closure, &export_facts, &import_facts)?;
    c_budget.retain(generated_c.capacity())?;
    let rust_budget = reserve_temporary_exact(MAX_GENERATED_RUST_BYTES)?;
    let (generated_rust, private_ffi_source) =
        generate_rust_artifacts(&spec, &export_facts, &import_facts)?;
    let rust_capacity = generated_rust
        .capacity()
        .checked_add(private_ffi_source.capacity())
        .ok_or_else(|| b109("max_generated_rust_bytes", MAX_GENERATED_RUST_BYTES))?;
    rust_budget.retain(rust_capacity)?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Render)?;
    for (field, bytes, maximum) in [
        (
            "max_generated_c_bytes",
            generated_c.len(),
            MAX_GENERATED_C_BYTES,
        ),
        (
            "max_generated_header_bytes",
            generated_header.len(),
            MAX_GENERATED_HEADER_BYTES,
        ),
    ] {
        if bytes > maximum {
            return Err(b109(field, maximum));
        }
    }
    replay_generated_exact(
        &spec,
        &closure,
        &export_facts,
        &import_facts,
        &generated_header,
        &generated_c,
        &generated_rust,
        &private_ffi_source,
    )?;
    #[cfg(test)]
    inject_prepare_failure(PrepareFailurePoint::Replay)?;
    drop(status_domains);
    let mut closure_ids = Vec::with_capacity(closure.len());
    closure_ids.extend(
        closure
            .iter()
            .map(|function| function.id.as_str().to_owned()),
    );
    let spec_digest = domain_digest(SPEC_DIGEST_DOMAIN, canonical_spec.as_bytes());
    let descriptor_digest = domain_digest(DESCRIPTOR_DIGEST_DOMAIN, descriptor.as_bytes());
    let spec_authority_bytes = checked_spec_owned_capacity(&spec)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if spec_authority.maximum() != spec_authority_bytes {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let closure_id_bytes = closure_ids
        .iter()
        .try_fold(0usize, |bytes, id| bytes.checked_add(id.capacity()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let persistent_without_spec_transfer =
        post_hir_facts_owned_capacity_checked(&export_facts, &import_facts)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            .checked_add(
                closure_ids
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_add(closure_id_bytes))
            .and_then(|bytes| bytes.checked_add(hir_digest.capacity()))
            .and_then(|bytes| bytes.checked_add(spec_digest.capacity()))
            .and_then(|bytes| bytes.checked_add(descriptor_digest.capacity()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let persistent_facts = persistent_without_spec_transfer
        .checked_add(spec_transfer_capacity)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    POST_HIR_AUTHORITY_TRANSFER_TERMS.with(|terms| {
        terms.set([
            facts_capacity.complete().expect("checked facts capacity"),
            spec_transfer_capacity,
            facts_complete_without_spec_transfer,
            persistent_without_spec_transfer,
            persistent_facts,
        ]);
    });
    if persistent_facts > facts_capacity.retained_upper {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    if spec_transfer_capacity > spec_authority_bytes {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let retained_without_spec_transfer_upper = facts_capacity
        .retained_upper
        .checked_sub(spec_transfer_capacity)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if persistent_without_spec_transfer > retained_without_spec_transfer_upper {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    #[cfg(test)]
    let ledger_before_transfer = crate::bounded_output::remaining_active()
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let facts_reserved_before_transfer = facts_budget.maximum();
    let Spec {
        module,
        source_revision,
        target,
        exports: spec_exports,
        imports: spec_imports,
        capabilities: spec_capabilities,
    } = spec;
    #[cfg(test)]
    assert_eq!(
        spec_transfer_allocations,
        (
            source_revision.as_ptr(),
            target.triple.as_ptr(),
            target.endian.as_ptr(),
            target.panic_strategy.as_ptr(),
            target.thread_policy.as_ptr(),
        ),
        "Spec source/target allocations must move into Prepared without clones",
    );
    let moved_transfer_capacity = source_revision
        .capacity()
        .checked_add(target.triple.capacity())
        .and_then(|bytes| bytes.checked_add(target.endian.capacity()))
        .and_then(|bytes| bytes.checked_add(target.panic_strategy.capacity()))
        .and_then(|bytes| bytes.checked_add(target.thread_policy.capacity()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if moved_transfer_capacity != spec_transfer_capacity {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    // Destroy every non-transferred Spec allocation before narrowing its
    // authority; the five moved allocations remain continuously covered.
    drop((module, spec_exports, spec_imports, spec_capabilities));
    #[cfg(test)]
    let expected_remaining_after_transfer = ledger_before_transfer
        .checked_add(
            spec_authority_bytes
                .checked_sub(spec_transfer_capacity)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                facts_reserved_before_transfer.checked_sub(persistent_without_spec_transfer)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    spec_authority.retain(spec_transfer_capacity)?;
    facts_budget.retain(persistent_without_spec_transfer)?;
    #[cfg(test)]
    assert_eq!(
        crate::bounded_output::remaining_active(),
        Some(expected_remaining_after_transfer),
        "Spec authority must release exactly once before Prepared facts retain",
    );
    drop(closure);
    let _ = resolved;
    drop(resolved_owner);
    drop(hir_budget);
    Ok(PreparedNativeRustInterop {
        spec_digest,
        canonical_spec,
        descriptor_digest,
        descriptor,
        source_revision,
        hir_digest,
        target,
        exports: export_facts,
        imports: import_facts,
        closure: closure_ids,
        generated_c,
        generated_header,
        generated_rust,
        private_ffi_source,
    })
}

fn validate_selected_scalar_closure(functions: &[&ResolvedFunction]) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(2);
    let mut pending = Vec::new();
    for function in functions {
        if function.params.len() > MAX_PARAMETERS
            || function.params.iter().any(|parameter| {
                parameter.ownership != hir::OwnershipMode::Value
                    || scalar_type(&parameter.ty).is_none()
            })
            || scalar_type(&function.return_type).is_none()
            || !function.cleanup.slots.is_empty()
            || !function.cleanup.flags.is_empty()
            || !function.cleanup_plan.slots.is_empty()
        {
            return Err(b107("scalar value signature required"));
        }
        pending.extend(function.requires.iter());
        pending.push(&function.body);
        pending.extend(function.ensures.iter());
    }
    while let Some(expression) = pending.pop() {
        note_hir_post_resolve_capacity(
            1,
            pending.capacity() * std::mem::size_of::<&ResolvedExpr>(),
        );
        let direct_unit_import = expression.ty == ResolvedType::Unit
            && matches!(expression.kind, ResolvedExprKind::NativeRustImportCall(_));
        if scalar_type(&expression.ty).is_none() && !direct_unit_import {
            return Err(b107("scalar value signature required"));
        }
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::Place(place)
                if place.projections.is_empty()
                    && expression.ownership == hir::OwnershipMode::Value => {}
            ResolvedExprKind::Call {
                type_arguments,
                instance,
                args,
                ..
            } if type_arguments.is_empty() && instance.is_none() => pending.extend(args),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    let unit_discard = binding.ty == ResolvedType::Unit
                        && value.ty == ResolvedType::Unit
                        && matches!(value.kind, ResolvedExprKind::NativeRustImportCall(_));
                    if binding.ownership != hir::OwnershipMode::Value
                        || (scalar_type(&binding.ty).is_none() && !unit_discard)
                    {
                        return Err(b107("scalar value signature required"));
                    }
                    pending.push(value);
                }
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::Call { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Place(_) => {
                return Err(b107("scalar value signature required"));
            }
        }
    }
    Ok(())
}

fn validate_native_unit_discard_bindings(
    functions: &[&ResolvedFunction],
) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(3);
    for function in functions {
        let mut discarded = BTreeSet::<hir::ValueId>::new();
        let mut pending = vec![(&function.body, false)];
        while let Some((expression, direct_let_rhs)) = pending.pop() {
            note_hir_post_resolve_capacity(
                2,
                pending.capacity() * std::mem::size_of::<(&ResolvedExpr, bool)>()
                    + discarded.len()
                        * (std::mem::size_of::<hir::ValueId>()
                            + std::mem::size_of::<BTreeSet<hir::ValueId>>())
                    + discarded.iter().map(|id| id.as_str().len()).sum::<usize>(),
            );
            if expression.ty == ResolvedType::Unit && !direct_let_rhs {
                return Err(b107("scalar value signature required"));
            }
            match &expression.kind {
                ResolvedExprKind::Block { statements, tail } => {
                    for statement in statements {
                        let ResolvedStatement::Let { binding, value, .. } = statement;
                        if value.ty == ResolvedType::Unit {
                            if !matches!(value.kind, ResolvedExprKind::NativeRustImportCall(_))
                                || binding.ty != ResolvedType::Unit
                                || !discarded.insert(binding.id.clone())
                            {
                                return Err(b107("scalar value signature required"));
                            }
                            pending.push((value, true));
                        } else {
                            pending.push((value, false));
                        }
                    }
                    pending.push((tail, false));
                }
                ResolvedExprKind::Place(place) if discarded.contains(&place.root) => {
                    return Err(b107("scalar value signature required"));
                }
                ResolvedExprKind::Call { args, .. } => {
                    pending.extend(args.iter().map(|child| (child, false)));
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    pending.extend(call.args.iter().map(|child| (child, false)));
                }
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Try { operand: value, .. }
                | ResolvedExprKind::TryOption { operand: value, .. }
                | ResolvedExprKind::Project { base: value, .. } => pending.push((value, false)),
                ResolvedExprKind::Binary { left, right, .. } => {
                    pending.push((left, false));
                    pending.push((right, false));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push((condition, false));
                    pending.push((then_branch, false));
                    pending.push((else_branch, false));
                }
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    pending.extend(fields.iter().map(|field| (&field.value, false)));
                }
                ResolvedExprKind::Match { scrutinee, arms } => {
                    pending.push((scrutinee, false));
                    pending.extend(arms.iter().map(|arm| (&arm.value, false)));
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    pending.push((base, false));
                    pending.extend(fields.iter().map(|field| (&field.value, false)));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

// Deterministic artifact generation and independent exact replay are isolated
// in `implementation/artifacts.rs`; no physical Phase-B authority crosses it.
#[cfg(test)]
struct TestTool {
    path: PathBuf,
}

#[cfg(test)]
fn same_file_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    if left.len() != right.len()
        || left.is_file() != right.is_file()
        || left.modified().ok() != right.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
fn configured_tool(variable: &str) -> Result<TestTool, Diagnostic> {
    if let Some(value) = std::env::var_os(variable) {
        let path = std::fs::canonicalize(value).map_err(|_| b110())?;
        return Ok(TestTool { path });
    }
    let name = if variable == "RUSTC" {
        if cfg!(windows) {
            "rustc.exe"
        } else {
            "rustc"
        }
    } else if cfg!(windows) {
        "clang.exe"
    } else {
        "clang"
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            let candidate = directory.join(name);
            let Ok(path) = std::fs::canonicalize(candidate) else {
                continue;
            };
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_file() && !metadata.file_type().is_symlink() {
                return Ok(TestTool { path });
            }
        }
    }
    Err(b110())
}

#[cfg(test)]
fn bind_test_tool_environment(command: &mut std::process::Command) {
    #[cfg(windows)]
    for variable in ["INCLUDE", "LIB"] {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
fn bind_test_rust_linker(command: &mut std::process::Command, _clang: &TestTool) {
    #[cfg(windows)]
    let linker = {
        let configured =
            std::env::var_os("SEMAPRAX_LINKER").expect("configured absolute Windows linker");
        let configured = PathBuf::from(configured);
        assert!(configured.is_absolute(), "Windows linker must be absolute");
        let linker = std::fs::canonicalize(configured).expect("canonical Windows linker");
        let metadata = std::fs::symlink_metadata(&linker).expect("stat canonical Windows linker");
        assert!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Windows linker must be a regular non-symlink file"
        );
        linker
    };
    #[cfg(not(windows))]
    let linker = _clang.path.clone();
    command
        .arg("-C")
        .arg(format!("linker={}", linker.display()));
    #[cfg(target_os = "linux")]
    command.args(["-C", "link-arg=--ld-path=/usr/bin/ld"]);
}

struct RustcVersion {
    storage: String,
    boundaries: [usize; 5],
}

impl RustcVersion {
    fn prepared() -> Result<Self, PhaseBLocalError> {
        let storage = String::with_capacity(PHASE_B_TOOL_VERSION_CAPACITY);
        if storage.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(Self {
            storage,
            boundaries: [0; 5],
        })
    }

    fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    fn field(&self, index: usize) -> &str {
        &self.storage[self.boundaries[index]..self.boundaries[index + 1]]
    }

    fn release(&self) -> &str {
        self.field(0)
    }

    fn commit_hash(&self) -> &str {
        self.field(1)
    }

    fn host(&self) -> &str {
        self.field(2)
    }

    fn llvm_version(&self) -> &str {
        self.field(3)
    }

    fn store(&mut self, values: [&str; 4]) -> Result<(), PhaseBLocalError> {
        if self.capacity() != PHASE_B_TOOL_VERSION_CAPACITY
            || !self.storage.is_empty()
            || self.boundaries != [0; 5]
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        let total = values
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()));
        if total.is_none_or(|total| total > self.capacity()) {
            return Err(PhaseBLocalError::Unsupported);
        }
        for (index, value) in values.into_iter().enumerate() {
            self.storage.push_str(value);
            self.boundaries[index + 1] = self.storage.len();
        }
        if self.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(())
    }

    #[cfg(test)]
    fn from_fields(values: [&str; 4]) -> Self {
        let mut version = Self::prepared().unwrap();
        version.store(values).unwrap();
        version
    }
}

struct FrozenToolEnvironment {
    clang: Option<OsString>,
    rustc: Option<OsString>,
    linker: Option<OsString>,
    vctools: Option<OsString>,
    path: Option<OsString>,
    sanitizer: Option<OsString>,
    include: Option<OsString>,
    libraries: Option<OsString>,
    budget: TemporaryBudget,
}

struct AuthorizedProcessArena {
    arena: Option<platform::PreparedProcessArena>,
    budget: Option<TemporaryBudget>,
}

impl AuthorizedProcessArena {
    fn new(arena: platform::PreparedProcessArena, budget: TemporaryBudget) -> Self {
        Self {
            arena: Some(arena),
            budget: Some(budget),
        }
    }

    fn arena(&self) -> Result<&platform::PreparedProcessArena, PhaseBLocalError> {
        self.arena.as_ref().ok_or(PhaseBLocalError::BuilderBudget)
    }

    fn arena_mut(&mut self) -> Result<&mut platform::PreparedProcessArena, PhaseBLocalError> {
        self.arena.as_mut().ok_or(PhaseBLocalError::BuilderBudget)
    }

    fn authorized_capacity(&self) -> Result<usize, PhaseBLocalError> {
        self.budget
            .as_ref()
            .map(TemporaryBudget::maximum)
            .ok_or(PhaseBLocalError::BuilderBudget)
    }
}

impl Drop for AuthorizedProcessArena {
    fn drop(&mut self) {
        if let Some(arena) = self.arena.take() {
            drop(arena);
            #[cfg(test)]
            {
                PHASE_B_PROCESS_ARENA_DROPS.with(|drops| drops.set(drops.get() + 1));
                note_phase_b_process_arena_drop(1);
            }
        }
        if let Some(budget) = self.budget.take() {
            drop(budget);
            #[cfg(test)]
            {
                PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(|drops| drops.set(drops.get() + 1));
                note_phase_b_process_arena_drop(2);
            }
        }
    }
}

struct PreparedToolchainPlan {
    environment: FrozenToolEnvironment,
    path_budget: TemporaryBudget,
    linker_resolver: Option<platform::PreparedToolResolver>,
    linker_resolver_budget: Option<TemporaryBudget>,
    discovery_output_budget: TemporaryBudget,
    direct_sysroot_output_budget: TemporaryBudget,
    rustc_output_budget: TemporaryBudget,
    clang_output_budget: TemporaryBudget,
    command_budget: TemporaryBudget,
    clang_resolver: platform::PreparedToolResolver,
    rustc_resolver: platform::PreparedToolResolver,
    discovery_invocation: platform::PreparedSysrootInvocation,
    direct_sysroot_invocation: platform::PreparedSysrootInvocation,
    rustc_invocation: platform::PreparedRustcVersionInvocation,
    clang_invocation: platform::PreparedVersionInvocation,
    process_arena: AuthorizedProcessArena,
    rustc_version: RustcVersion,
}

struct ToolchainFacts {
    rustc: platform::HeldDirectRustc,
    clang: platform::HeldTool,
    linker: Option<platform::HeldTool>,
    process_arena: Option<AuthorizedProcessArena>,
    rustc_version: RustcVersion,
    clang_version: String,
}

fn freeze_tool_environment() -> Result<FrozenToolEnvironment, PhaseBLocalError> {
    #[cfg(test)]
    let invalid_tool_environment = PHASE_B_INVALID_TOOL_ENV_INJECTION.with(std::cell::Cell::get);
    #[cfg(not(test))]
    let invalid_tool_environment = false;
    let clang = if invalid_tool_environment {
        Some(OsString::from("__semaprax_missing_clang__"))
    } else {
        std::env::var_os("CLANG")
    };
    let rustc = if invalid_tool_environment {
        Some(OsString::from("__semaprax_missing_rustc__"))
    } else {
        std::env::var_os("RUSTC")
    };
    let linker = if cfg!(windows) {
        std::env::var_os("SEMAPRAX_LINKER")
    } else {
        None
    };
    let vctools = if cfg!(windows) {
        std::env::var_os("SEMAPRAX_VCTOOLS")
    } else {
        None
    };
    let path = std::env::var_os("PATH");
    let sanitizer = std::env::var_os("SEMAPRAX_REQUIRE_NATIVE_RUST_INTEROP_SANITIZERS");
    let include = if cfg!(windows) {
        std::env::var_os("INCLUDE")
    } else {
        None
    };
    let libraries = if cfg!(windows) {
        std::env::var_os("LIB")
    } else {
        None
    };
    let capacity = [
        &clang, &rustc, &linker, &vctools, &path, &sanitizer, &include, &libraries,
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        total.checked_add(value.as_ref().map_or(0, OsString::capacity))
    })
    .ok_or(PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(capacity)?;
    Ok(FrozenToolEnvironment {
        clang,
        rustc,
        linker,
        vctools,
        path,
        sanitizer,
        include,
        libraries,
        budget,
    })
}

fn prepare_toolchain_plan() -> Result<PreparedToolchainPlan, PhaseBLocalError> {
    let mut environment = freeze_tool_environment()?;
    let process_arena = prepare_process_arena_authorized(
        environment.include.as_deref(),
        environment.libraries.as_deref(),
    )?;
    let include = environment.include.take();
    let libraries = environment.libraries.take();
    drop(include);
    drop(libraries);
    let retained_environment = [
        &environment.clang,
        &environment.rustc,
        &environment.linker,
        &environment.vctools,
        &environment.path,
        &environment.sanitizer,
    ]
    .into_iter()
    .try_fold(0usize, |total, value| {
        total.checked_add(value.as_ref().map_or(0, OsString::capacity))
    })
    .ok_or(PhaseBLocalError::BuilderBudget)?;
    shrink_phase_b(&mut environment.budget, retained_environment)?;
    let clang_name = if cfg!(windows) { "clang.exe" } else { "clang" };
    let path_budget = reserve_phase_b(PHASE_B_TOOL_RESOLVER_CAPACITY)?;
    let discovery_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let direct_sysroot_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let rustc_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let clang_output_budget = reserve_phase_b(PHASE_B_TOOL_VERSION_CAPACITY)?;
    let command_budget = reserve_phase_b(
        PHASE_B_VERSION_COMMAND_CAPACITY
            .checked_mul(4)
            .ok_or(PhaseBLocalError::BuilderBudget)?,
    )?;
    let clang_resolver = platform::prepare_tool_resolver(clang_name, PHASE_B_TOOL_PATH_CAPACITY)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let resolver_owned = platform::prepared_tool_resolver_owned_capacity(&clang_resolver);
    if resolver_owned > path_budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let rustc_name = if cfg!(windows) { "rustc.exe" } else { "rustc" };
    let rustc_resolver = platform::prepare_tool_resolver(rustc_name, PHASE_B_TOOL_PATH_CAPACITY)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let rustc_resolver_owned = platform::prepared_tool_resolver_owned_capacity(&rustc_resolver);
    if rustc_resolver_owned > path_budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let (linker_resolver_budget, linker_resolver) = if cfg!(windows) {
        let budget = reserve_phase_b(PHASE_B_TOOL_RESOLVER_CAPACITY)?;
        let resolver = platform::prepare_tool_resolver("link.exe", PHASE_B_TOOL_PATH_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
        if platform::prepared_tool_resolver_owned_capacity(&resolver) > budget.maximum() {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        (Some(budget), Some(resolver))
    } else {
        (None, None)
    };
    let discovery_invocation = platform::prepare_sysroot_invocation(PHASE_B_TOOL_VERSION_CAPACITY)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let direct_sysroot_invocation =
        platform::prepare_sysroot_invocation(PHASE_B_TOOL_VERSION_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let rustc_invocation =
        platform::prepare_rustc_version_invocation(PHASE_B_TOOL_VERSION_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let clang_invocation =
        platform::prepare_version_invocation("--version", PHASE_B_TOOL_VERSION_CAPACITY)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let command_owned = platform::prepared_sysroot_owned_capacity(&discovery_invocation)
        .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
        .and_then(|discovery| {
            platform::prepared_sysroot_owned_capacity(&direct_sysroot_invocation)
                .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
                .and_then(|direct| discovery.checked_add(direct))
        })
        .and_then(|total| {
            platform::prepared_rustc_version_owned_capacity(&rustc_invocation)
                .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
                .and_then(|rustc| total.checked_add(rustc))
        })
        .and_then(|total| {
            platform::prepared_version_owned_capacity(&clang_invocation)
                .checked_sub(PHASE_B_TOOL_VERSION_CAPACITY)
                .and_then(|clang| total.checked_add(clang))
        })
        .ok_or(PhaseBLocalError::BuilderBudget)?;
    if command_owned > command_budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let persistent = PHASE_B_TOOL_VERSION_CAPACITY;
    let persistent_budget = reserve_phase_b(persistent)?;
    let rustc_version = RustcVersion::prepared()?;
    if rustc_version.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    retain_phase_b(persistent_budget, rustc_version.capacity())?;
    Ok(PreparedToolchainPlan {
        environment,
        path_budget,
        linker_resolver_budget,
        discovery_output_budget,
        direct_sysroot_output_budget,
        rustc_output_budget,
        clang_output_budget,
        command_budget,
        clang_resolver,
        rustc_resolver,
        linker_resolver,
        discovery_invocation,
        direct_sysroot_invocation,
        rustc_invocation,
        clang_invocation,
        process_arena,
        rustc_version,
    })
}

fn authenticate_toolchain(
    plan: PreparedToolchainPlan,
    target: &Target,
    cwd: &platform::HeldDirectory,
) -> Result<ToolchainFacts, PhaseBLocalError> {
    let PreparedToolchainPlan {
        environment,
        path_budget,
        linker_resolver_budget,
        discovery_output_budget,
        direct_sysroot_output_budget,
        rustc_output_budget,
        clang_output_budget,
        command_budget,
        clang_resolver,
        rustc_resolver,
        linker_resolver,
        discovery_invocation,
        direct_sysroot_invocation,
        rustc_invocation,
        clang_invocation,
        mut process_arena,
        mut rustc_version,
        ..
    } = plan;
    let FrozenToolEnvironment {
        clang: configured_clang,
        rustc: configured_rustc,
        linker: configured_linker,
        vctools: configured_vctools,
        path,
        sanitizer,
        include,
        libraries,
        budget: environment_budget,
    } = environment;
    match sanitizer {
        None => {}
        Some(value) if value == "1" && cfg!(target_os = "linux") => {}
        Some(_) => return Err(PhaseBLocalError::Unsupported),
    }
    if cfg!(windows)
        && !valid_windows_link_environment(
            configured_linker.as_deref(),
            configured_vctools.as_deref(),
        )
    {
        return Err(PhaseBLocalError::Unsupported);
    }
    let (clang, _clang_resolver) = platform::resolve_and_hold_tool_reusing_prepared(
        clang_resolver,
        configured_clang.as_deref(),
        path.as_deref(),
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    let linker = if cfg!(windows) {
        let configured = configured_linker
            .as_deref()
            .filter(|path| std::path::Path::new(path).is_absolute())
            .ok_or(PhaseBLocalError::Unsupported)?;
        let resolver = linker_resolver.ok_or(PhaseBLocalError::BuilderBudget)?;
        let (linker, _) =
            platform::resolve_and_hold_tool_reusing_prepared(resolver, Some(configured), None)
                .map_err(|_| PhaseBLocalError::Unsupported)?;
        if std::path::Path::new(platform::tool_path(&linker)).as_os_str() != configured {
            return Err(PhaseBLocalError::Unsupported);
        }
        #[cfg(test)]
        PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
        Some(linker)
    } else {
        if configured_linker.is_some()
            || configured_vctools.is_some()
            || linker_resolver.is_some()
            || linker_resolver_budget.is_some()
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        None
    };
    let (rustc, rustc_resolver) = if let Some(rustc) = configured_rustc.as_deref() {
        if std::path::Path::new(rustc).is_absolute() {
            platform::resolve_and_hold_tool_reusing_prepared(
                rustc_resolver,
                Some(rustc),
                path.as_deref(),
            )
            .map_err(|_| PhaseBLocalError::Unsupported)?
        } else {
            platform::resolve_and_hold_tool_reusing_prepared(rustc_resolver, None, path.as_deref())
                .map_err(|_| PhaseBLocalError::Unsupported)?
        }
    } else {
        platform::resolve_and_hold_tool_reusing_prepared(rustc_resolver, None, path.as_deref())
            .map_err(|_| PhaseBLocalError::Unsupported)?
    };
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    let configured_rustc = platform::tool_path(&rustc).to_owned();
    let discovery =
        platform::hold_rustc_discovery_prepared(rustc_resolver, OsStr::new(&configured_rustc))
            .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    drop(rustc);
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    drop(configured_clang);
    drop(configured_rustc);
    drop(configured_linker);
    drop(configured_vctools);
    drop(include);
    drop(libraries);
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let discovery_sysroot = platform::rustc_discovery_output_prepared(
        &discovery,
        cwd,
        discovery_invocation,
        process_arena.arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    if discovery_sysroot.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let mut rustc = platform::hold_direct_rustc_prepared(discovery, discovery_sysroot.bytes())
        .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    PHASE_B_TOOL_HOLDS.with(|count| count.set(count.get().saturating_add(1)));
    drop(discovery_sysroot);
    drop(discovery_output_budget);
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let direct_sysroot = platform::direct_rustc_output_prepared(
        &rustc,
        cwd,
        direct_sysroot_invocation,
        process_arena.arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    if direct_sysroot.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    platform::direct_rustc_reproduces_sysroot(&mut rustc, direct_sysroot.bytes())
        .map_err(|_| PhaseBLocalError::Unsupported)?;
    #[cfg(test)]
    if PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION.with(std::cell::Cell::get) {
        return Err(PhaseBLocalError::Unsupported);
    }
    drop(direct_sysroot);
    drop(direct_sysroot_output_budget);
    drop(path);
    drop(environment_budget);
    retain_phase_b(path_budget, platform::tool_path_capacity(&clang))?;
    if let Some(linker) = linker.as_ref() {
        retain_phase_b(
            linker_resolver_budget.ok_or(PhaseBLocalError::BuilderBudget)?,
            platform::tool_path_capacity(linker),
        )?;
    }
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let rustc_text = platform::direct_rustc_version_prepared(
        &rustc,
        cwd,
        rustc_invocation,
        process_arena.arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Unsupported)?;
    if rustc_text.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let rustc_bytes = rustc_text.into_bytes();
    let rustc_text = std::str::from_utf8(&rustc_bytes)
        .map_err(|_| PhaseBLocalError::Unsupported)?
        .trim();
    parse_rustc_version(rustc_text, &mut rustc_version)?;
    drop(rustc_bytes);
    drop(rustc_output_budget);
    if rustc_version.host() != target.triple {
        return Err(PhaseBLocalError::Unsupported);
    }
    #[cfg(test)]
    PHASE_B_TOOL_PROCESSES.with(|count| count.set(count.get().saturating_add(1)));
    let clang_text =
        platform::tool_version_prepared(&clang, cwd, clang_invocation, process_arena.arena_mut()?)
            .map_err(|_| PhaseBLocalError::Unsupported)?;
    if clang_text.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let mut clang_version =
        String::from_utf8(clang_text.into_bytes()).map_err(|_| PhaseBLocalError::Unsupported)?;
    let trimmed = clang_version.trim();
    let start = trimmed.as_ptr() as usize - clang_version.as_ptr() as usize;
    let end = start + trimmed.len();
    clang_version.truncate(end);
    clang_version.drain(..start);
    if clang_version.capacity() != PHASE_B_TOOL_VERSION_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    retain_phase_b(clang_output_budget, clang_version.capacity())?;
    drop(command_budget);
    if clang_version.is_empty() {
        return Err(PhaseBLocalError::Unsupported);
    }
    if platform::prepared_process_arena_remaining(process_arena.arena()?)
        != PHASE_B_PROCESS_INVOCATIONS - 4
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    Ok(ToolchainFacts {
        rustc,
        clang,
        linker,
        process_arena: Some(process_arena),
        rustc_version,
        clang_version,
    })
}

fn planned_sanitizers(plan: &PreparedToolchainPlan) -> bool {
    cfg!(target_os = "linux")
        && plan.environment.sanitizer.as_deref() == Some(std::ffi::OsStr::new("1"))
}

fn planned_linker(plan: &PreparedToolchainPlan) -> Option<&OsStr> {
    if cfg!(windows) {
        if valid_windows_link_environment(
            plan.environment.linker.as_deref(),
            plan.environment.vctools.as_deref(),
        ) {
            plan.environment.linker.as_deref()
        } else {
            Some(OsStr::new(PHASE_B_MISSING_WINDOWS_LINKER))
        }
    } else {
        None
    }
}

fn planned_vctools(plan: &PreparedToolchainPlan) -> Option<&OsStr> {
    if cfg!(windows) {
        if valid_windows_link_environment(
            plan.environment.linker.as_deref(),
            plan.environment.vctools.as_deref(),
        ) {
            plan.environment.vctools.as_deref()
        } else {
            Some(OsStr::new(PHASE_B_MISSING_WINDOWS_VCTOOLS))
        }
    } else {
        None
    }
}

fn valid_windows_link_environment(linker: Option<&OsStr>, vctools: Option<&OsStr>) -> bool {
    let Some(linker) = linker.map(std::path::Path::new) else {
        return false;
    };
    let Some(vctools) = vctools.map(std::path::Path::new) else {
        return false;
    };
    linker.is_absolute()
        && vctools.is_absolute()
        && linker.strip_prefix(vctools).ok()
            == Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
}

fn parse_rustc_version(source: &str, output: &mut RustcVersion) -> Result<(), PhaseBLocalError> {
    if output.capacity() != PHASE_B_TOOL_VERSION_CAPACITY
        || !output.storage.is_empty()
        || output.boundaries != [0; 5]
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let mut lines = source.lines();
    let header = lines
        .next()
        .filter(|line| line.starts_with("rustc ") && line.len() > 6)
        .ok_or(PhaseBLocalError::Unsupported)?;
    let mut values = [None; 4];
    let mut binary_seen = false;
    let mut date_seen = false;
    for line in lines {
        let (key, value) = line.split_once(": ").ok_or(PhaseBLocalError::Unsupported)?;
        let slot = match key {
            "release" => Some(0),
            "commit-hash" => Some(1),
            "host" => Some(2),
            "LLVM version" => Some(3),
            "binary" if !binary_seen => {
                binary_seen = true;
                None
            }
            "commit-date" if !date_seen => {
                date_seen = true;
                None
            }
            _ => return Err(PhaseBLocalError::Unsupported),
        };
        if let Some(slot) = slot {
            if values[slot].replace(value).is_some() {
                return Err(PhaseBLocalError::Unsupported);
            }
        }
    }
    let [Some(release), Some(commit_hash), Some(host), Some(llvm_version)] = values else {
        return Err(PhaseBLocalError::Unsupported);
    };
    if release.is_empty()
        || !header.contains(release)
        || commit_hash.len() < 7
        || !commit_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        || host.is_empty()
        || llvm_version.is_empty()
        || [release, commit_hash, host, llvm_version]
            .iter()
            .any(|value| value.len() > PHASE_B_TOOL_VERSION_CAPACITY)
    {
        return Err(PhaseBLocalError::Unsupported);
    }
    output.store([release, commit_hash, host, llvm_version])
}

/// Canonical manifest row order is a wire contract.  The platform object is
/// always the third row; no caller may infer or sort this order dynamically.
const fn canonical_manifest_file_names() -> [&'static str; 6] {
    [
        "descriptor.json",
        "module.c",
        if cfg!(windows) {
            "module.obj"
        } else {
            "module.o"
        },
        "semaprax_native_rust_interop.h",
        "semaprax_native_rust_interop.rs",
        "semaprax_native_rust_interop_ffi.rs",
    ]
}

fn write_raw_digest_json(output: &mut impl std::fmt::Write, bytes: &[u8]) -> std::fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.write_str("\"sha256:")?;
    for byte in Sha256::digest(bytes) {
        let pair = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]];
        output.write_str(std::str::from_utf8(&pair).map_err(|_| std::fmt::Error)?)?;
    }
    output.write_char('"')
}

fn write_usize_decimal(output: &mut impl std::fmt::Write, mut value: usize) -> std::fmt::Result {
    let mut bytes = [0_u8; 20];
    let mut start = bytes.len();
    loop {
        start -= 1;
        bytes[start] = b'0' + u8::try_from(value % 10).map_err(|_| std::fmt::Error)?;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    output.write_str(std::str::from_utf8(&bytes[start..]).map_err(|_| std::fmt::Error)?)
}

fn write_manifest_file_row(
    output: &mut impl std::fmt::Write,
    path: &str,
    bytes: &[u8],
) -> std::fmt::Result {
    output.write_str("{\"path\":")?;
    write_json_string(output, path)?;
    output.write_str(",\"sha256\":")?;
    write_raw_digest_json(output, bytes)?;
    output.write_str(",\"bytes\":")?;
    write_usize_decimal(output, bytes.len())?;
    output.write_char('}')
}

fn write_manifest(
    output: &mut impl std::fmt::Write,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8])],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> std::fmt::Result {
    output.write_str("{\"schema\":")?;
    write_json_string(output, BUNDLE_SCHEMA)?;
    output.write_str(",\"descriptor\":{\"schema\":")?;
    write_json_string(output, DESCRIPTOR_SCHEMA)?;
    output.write_str(",\"digest\":")?;
    write_json_string(output, &prepared.descriptor_digest)?;
    output.write_str(",\"bytes\":")?;
    write_usize_decimal(output, prepared.descriptor.len())?;
    output.write_str("},\"files\":[")?;
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_manifest_file_row(output, path, bytes)?;
    }
    output.write_str("],\"toolchain\":{\"rustc_release\":")?;
    write_json_string(output, rustc.release())?;
    output.write_str(",\"rustc_commit_hash\":")?;
    write_json_string(output, rustc.commit_hash())?;
    output.write_str(",\"host\":")?;
    write_json_string(output, rustc.host())?;
    output.write_str(",\"llvm_version\":")?;
    write_json_string(output, rustc.llvm_version())?;
    output.write_str(",\"clang_path\":")?;
    write_json_string(output, clang_path)?;
    output.write_str(",\"clang_version\":")?;
    write_json_string(output, clang_version)?;
    output.write_str(",\"target\":")?;
    write_json_string(output, target)?;
    output.write_str("},\"limits\":")?;
    write_limits_json(output)?;
    output.write_str(",\"nonclaims\":[")?;
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.write_char(',')?;
        }
        write_json_string(output, nonclaim)?;
    }
    output.write_str("]}\n")
}

#[cfg(test)]
fn render_manifest(
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8])],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> String {
    let mut count = CountingSink {
        bytes: 0,
        maximum: MAX_MANIFEST_BYTES,
        overflowed: false,
    };
    write_manifest(
        &mut count,
        prepared,
        files,
        clang_path,
        clang_version,
        rustc,
        target,
    )
    .expect("manifest count cannot fail");
    assert!(!count.overflowed);
    let mut output = String::with_capacity(count.bytes);
    write_manifest(
        &mut output,
        prepared,
        files,
        clang_path,
        clang_version,
        rustc,
        target,
    )
    .expect("String writing cannot fail");
    assert_eq!(output.capacity(), count.bytes);
    output
}

fn replay_manifest_bytes_exact(
    source: &str,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8])],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> bool {
    let mut exact = ExactReplay::new(source);
    exact.text("{\"schema\":");
    exact.json(BUNDLE_SCHEMA);
    exact.text(",\"descriptor\":{\"schema\":");
    exact.json(DESCRIPTOR_SCHEMA);
    exact.text(",\"digest\":");
    exact.json(&prepared.descriptor_digest);
    exact.text(",\"bytes\":");
    exact.usize_noalloc(prepared.descriptor.len());
    exact.text("},\"files\":[");
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            exact.text(",");
        }
        exact.text("{\"path\":");
        exact.json(path);
        exact.text(",\"sha256\":");
        exact.raw_digest_json_noalloc(bytes);
        exact.text(",\"bytes\":");
        exact.usize_noalloc(bytes.len());
        exact.text("}");
    }
    exact.text("],\"toolchain\":{\"rustc_release\":");
    exact.json(rustc.release());
    exact.text(",\"rustc_commit_hash\":");
    exact.json(rustc.commit_hash());
    exact.text(",\"host\":");
    exact.json(rustc.host());
    exact.text(",\"llvm_version\":");
    exact.json(rustc.llvm_version());
    exact.text(",\"clang_path\":");
    exact.json(clang_path);
    exact.text(",\"clang_version\":");
    exact.json(clang_version);
    exact.text(",\"target\":");
    exact.json(target);
    exact.text("},\"limits\":");
    replay_limits_exact(&mut exact);
    exact.text(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            exact.text(",");
        }
        exact.json(nonclaim);
    }
    exact.text("]}\n");
    exact.finish()
}

/// Independently consumes the fixed manifest JSON grammar without a DOM or
/// decoded-string allocation.  The exact replay above binds canonical bytes;
/// this cursor separately validates decoded values and the complete member,
/// type, cardinality, depth, and trailing-byte shape.
struct ManifestCursor<'a> {
    source: &'a str,
    offset: usize,
    work: usize,
    maximum_work: usize,
}

impl<'a> ManifestCursor<'a> {
    fn new(source: &'a str) -> Result<Self, PhaseBLocalError> {
        Ok(Self {
            source,
            offset: 0,
            work: 0,
            maximum_work: source
                .len()
                .checked_mul(2)
                .ok_or(PhaseBLocalError::Replay)?,
        })
    }

    fn bytes(&self) -> &'a [u8] {
        self.source.as_bytes()
    }

    fn advance(&mut self, bytes: usize) -> Result<(), PhaseBLocalError> {
        self.offset = self
            .offset
            .checked_add(bytes)
            .ok_or(PhaseBLocalError::Replay)?;
        self.work = self
            .work
            .checked_add(bytes)
            .ok_or(PhaseBLocalError::Replay)?;
        if self.offset > self.source.len() || self.work > self.maximum_work {
            return Err(PhaseBLocalError::Replay);
        }
        Ok(())
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), PhaseBLocalError> {
        let end = self
            .offset
            .checked_add(expected.len())
            .ok_or(PhaseBLocalError::Replay)?;
        if self.bytes().get(self.offset..end) != Some(expected) {
            return Err(PhaseBLocalError::Replay);
        }
        self.advance(expected.len())
    }

    fn hex_quad(&mut self) -> Result<u16, PhaseBLocalError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let byte = *self
                .bytes()
                .get(self.offset)
                .ok_or(PhaseBLocalError::Replay)?;
            self.advance(1)?;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a') + 10,
                b'A'..=b'F' => u16::from(byte - b'A') + 10,
                _ => return Err(PhaseBLocalError::Replay),
            };
            value = value
                .checked_mul(16)
                .and_then(|value| value.checked_add(digit))
                .ok_or(PhaseBLocalError::Replay)?;
        }
        Ok(value)
    }

    fn json_character(&mut self) -> Result<char, PhaseBLocalError> {
        let byte = *self
            .bytes()
            .get(self.offset)
            .ok_or(PhaseBLocalError::Replay)?;
        if byte == b'"' || byte < 0x20 {
            return Err(PhaseBLocalError::Replay);
        }
        if byte != b'\\' {
            let character = self
                .source
                .get(self.offset..)
                .ok_or(PhaseBLocalError::Replay)?
                .chars()
                .next()
                .ok_or(PhaseBLocalError::Replay)?;
            self.advance(character.len_utf8())?;
            return Ok(character);
        }
        self.advance(1)?;
        let escape = *self
            .bytes()
            .get(self.offset)
            .ok_or(PhaseBLocalError::Replay)?;
        self.advance(1)?;
        match escape {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{08}'),
            b'f' => Ok('\u{0c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => {
                let first = self.hex_quad()?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    self.expect(b"\\u")?;
                    let second = self.hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(PhaseBLocalError::Replay);
                    }
                    0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(PhaseBLocalError::Replay);
                } else {
                    u32::from(first)
                };
                char::from_u32(scalar).ok_or(PhaseBLocalError::Replay)
            }
            _ => Err(PhaseBLocalError::Replay),
        }
    }

    fn string_eq(&mut self, expected: &str) -> Result<(), PhaseBLocalError> {
        self.expect(b"\"")?;
        for expected in expected.chars() {
            if self.json_character()? != expected {
                return Err(PhaseBLocalError::Replay);
            }
        }
        self.expect(b"\"")
    }

    fn usize_eq(&mut self, expected: usize) -> Result<(), PhaseBLocalError> {
        let first = *self
            .bytes()
            .get(self.offset)
            .ok_or(PhaseBLocalError::Replay)?;
        if !first.is_ascii_digit() {
            return Err(PhaseBLocalError::Replay);
        }
        let mut value = 0_usize;
        let mut digits = 0_usize;
        while let Some(byte @ b'0'..=b'9') = self.bytes().get(self.offset).copied() {
            if digits == 1 && first == b'0' {
                return Err(PhaseBLocalError::Replay);
            }
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(usize::from(byte - b'0')))
                .ok_or(PhaseBLocalError::Replay)?;
            self.advance(1)?;
            digits += 1;
        }
        if value == expected {
            Ok(())
        } else {
            Err(PhaseBLocalError::Replay)
        }
    }

    fn raw_digest_eq(&mut self, bytes: &[u8]) -> Result<(), PhaseBLocalError> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.expect(b"\"sha256:")?;
        for byte in Sha256::digest(bytes) {
            self.expect(&[HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]])?;
        }
        self.expect(b"\"")
    }

    fn finish(self) -> Result<usize, PhaseBLocalError> {
        if self.offset == self.source.len() && self.work <= self.maximum_work {
            Ok(self.work)
        } else {
            Err(PhaseBLocalError::Replay)
        }
    }
}

fn replay_manifest_semantic(
    source: &str,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8]); 6],
    clang_path: &str,
    clang_version: &str,
    rustc: &RustcVersion,
    target: &str,
) -> Result<usize, PhaseBLocalError> {
    let mut cursor = ManifestCursor::new(source)?;
    cursor.expect(b"{\"schema\":")?;
    cursor.string_eq(BUNDLE_SCHEMA)?;
    cursor.expect(b",\"descriptor\":{\"schema\":")?;
    cursor.string_eq(DESCRIPTOR_SCHEMA)?;
    cursor.expect(b",\"digest\":")?;
    cursor.string_eq(&prepared.descriptor_digest)?;
    cursor.expect(b",\"bytes\":")?;
    cursor.usize_eq(prepared.descriptor.len())?;
    cursor.expect(b"},\"files\":[")?;
    for (index, (path, bytes)) in files.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        cursor.expect(b"{\"path\":")?;
        cursor.string_eq(path)?;
        cursor.expect(b",\"sha256\":")?;
        cursor.raw_digest_eq(bytes)?;
        cursor.expect(b",\"bytes\":")?;
        cursor.usize_eq(bytes.len())?;
        cursor.expect(b"}")?;
    }
    cursor.expect(b"],\"toolchain\":{\"rustc_release\":")?;
    cursor.string_eq(rustc.release())?;
    cursor.expect(b",\"rustc_commit_hash\":")?;
    cursor.string_eq(rustc.commit_hash())?;
    cursor.expect(b",\"host\":")?;
    cursor.string_eq(rustc.host())?;
    cursor.expect(b",\"llvm_version\":")?;
    cursor.string_eq(rustc.llvm_version())?;
    cursor.expect(b",\"clang_path\":")?;
    cursor.string_eq(clang_path)?;
    cursor.expect(b",\"clang_version\":")?;
    cursor.string_eq(clang_version)?;
    cursor.expect(b",\"target\":")?;
    cursor.string_eq(target)?;
    cursor.expect(b"},\"limits\":{")?;
    for (index, (name, value)) in LIMIT_ROWS.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        cursor.string_eq(name)?;
        cursor.expect(b":")?;
        cursor.usize_eq(*value)?;
    }
    cursor.expect(b"},\"nonclaims\":[")?;
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            cursor.expect(b",")?;
        }
        cursor.string_eq(nonclaim)?;
    }
    cursor.expect(b"]}\n")?;
    cursor.finish()
}

fn replay_manifest(
    source: &str,
    prepared: &PreparedNativeRustInterop,
    files: &[(&str, &[u8]); 6],
    tools: &ToolchainFacts,
) -> Result<(), PhaseBLocalError> {
    if !replay_manifest_bytes_exact(
        source,
        prepared,
        files,
        platform::tool_path(&tools.clang),
        &tools.clang_version,
        &tools.rustc_version,
        &prepared.target.triple,
    ) {
        return Err(PhaseBLocalError::Replay);
    }
    replay_manifest_semantic(
        source,
        prepared,
        files,
        platform::tool_path(&tools.clang),
        &tools.clang_version,
        &tools.rustc_version,
        &prepared.target.triple,
    )
    .map(|_| ())
}

fn render_rust_harness(
    output: &mut impl std::fmt::Write,
    prepared: &PreparedNativeRustInterop,
) -> std::fmt::Result {
    output.write_str(
        "#[path=\"semaprax_native_rust_interop.rs\"]mod semaprax_native_rust_interop;\nuse semaprax_native_rust_interop::*;\nstruct Host;\nimpl NativeRustImports for Host{\n",
    )?;
    for import in &prepared.imports {
        write!(
            output,
            "fn {}(&mut self{}",
            import.rust_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
        )?;
        for (index, parameter) in import.parameters.iter().enumerate() {
            if index != 0 {
                output.write_str(", ")?;
            }
            write!(output, "_arg_{index}: {}", rust_type(parameter.ty))?;
        }
        write!(
            output,
            ")->NativeRustImportResult<{}>{{{} }}\n",
            rust_type(import.result),
            match import.result {
                ScalarType::Unit => "NativeRustImportResult::Success(())",
                ScalarType::Bool => "NativeRustImportResult::Success(false)",
                ScalarType::I64 => "NativeRustImportResult::Success(0)",
            }
        )?;
    }
    output.write_str("}\n#[no_mangle]pub extern \"C\" fn spxnr1_rust_harness_run()->i32{let code=core::num::NonZeroU32::new(1).unwrap();")?;
    if !prepared.imports.is_empty() {
        output.write_str("let _=NativeRustImportResult::<()>::Status{code,class:NativeRustStatusClass::Import,retryable:false};let _=NativeRustImportResult::<()>::HostFailure;")?;
    }
    output.write_str("let probe=NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class:NativeRustStatusClass::Semantic,retryable:false};if let NativeRustCallError::Semantic{domain_id,code,class,retryable}=probe{let _=(domain_id,code,class,retryable);}let caps=match NativeRustCapabilities::new(&[")?;
    let mut previous = None;
    let mut first = true;
    loop {
        let mut selected = None;
        for capability in prepared
            .imports
            .iter()
            .flat_map(|import| &import.capabilities)
            .map(String::as_str)
        {
            if previous.is_none_or(|prior| capability > prior)
                && selected.is_none_or(|current| capability < current)
            {
                selected = Some(capability);
            }
        }
        let Some(capability) = selected else {
            break;
        };
        if !first {
            output.write_char(',')?;
        }
        write_json_string(output, capability)?;
        previous = Some(capability);
        first = false;
    }
    output.write_str(
        "]){Ok(value)=>value,Err(_)=>return 2};let mut bridge=NativeRustBridge::new(Host,caps);",
    )?;
    for export in &prepared.exports {
        write!(output, "let _closed_result=bridge.{}(", export.rust_method)?;
        for (index, parameter) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.write_char(',')?;
            }
            output.write_str(match parameter.ty {
                ScalarType::I64 => "0",
                ScalarType::Bool => "false",
                ScalarType::Unit => "()",
            })?;
        }
        output.write_str(");")?;
    }
    output.write_str("0}\n")
}

#[derive(Default)]
struct HarnessCount {
    length: usize,
}

impl std::fmt::Write for HarnessCount {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.length = self
            .length
            .checked_add(value.len())
            .ok_or(std::fmt::Error)?;
        Ok(())
    }
}

fn prepare_rust_harness(
    prepared: &PreparedNativeRustInterop,
) -> Result<(String, TemporaryBudget), PhaseBLocalError> {
    let mut count = HarnessCount::default();
    render_rust_harness(&mut count, prepared).map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(count.length)?;
    let mut output = String::with_capacity(count.length);
    if output.capacity() != count.length {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    render_rust_harness(&mut output, prepared).map_err(|_| PhaseBLocalError::BuilderBudget)?;
    if output.len() != count.length || output.capacity() != count.length {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    Ok((output, budget))
}

const PHASE_B_INVOCATION_ARGUMENT_CAPACITY: usize = 16_384;

struct PreparedBuildInvocations {
    c_o0: (platform::PreparedCCompileInvocation, TemporaryBudget),
    c_o2: (platform::PreparedCCompileInvocation, TemporaryBudget),
    rust: (platform::PreparedRustCompileInvocation, TemporaryBudget),
    c_main: (platform::PreparedCCompileInvocation, TemporaryBudget),
    link_o0: (platform::PreparedLinkInvocation, TemporaryBudget),
    run_o0: (platform::PreparedRunInvocation, TemporaryBudget),
    link_o2: (platform::PreparedLinkInvocation, TemporaryBudget),
    run_o2: (platform::PreparedRunInvocation, TemporaryBudget),
}

fn prepare_invocation<T>(
    maximum: usize,
    prepare: impl FnOnce() -> Result<T, platform::Error>,
    capacity: impl FnOnce(&T) -> usize,
) -> Result<(T, TemporaryBudget), PhaseBLocalError> {
    let mut budget = reserve_phase_b(maximum)?;
    let invocation = prepare().map_err(|error| match error {
        platform::Error::OutputLimit => PhaseBLocalError::BuilderBudget,
        platform::Error::Invalid
        | platform::Error::Unsupported
        | platform::Error::Exists
        | platform::Error::Changed
        | platform::Error::Spawn
        | platform::Error::Exit => PhaseBLocalError::Unsupported,
    })?;
    let owned = capacity(&invocation);
    if owned > budget.maximum() {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    shrink_phase_b(&mut budget, owned)?;
    #[cfg(test)]
    PHASE_B_BUILD_INVOCATION_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((invocation, budget))
}

fn consume_invocation<T>(plan: (T, TemporaryBudget)) -> (T, TemporaryBudget) {
    #[cfg(test)]
    PHASE_B_BUILD_INVOCATION_CONSUMPTIONS.with(|count| count.set(count.get().saturating_add(1)));
    plan
}

fn prepare_build_invocations(
    prepared: &PreparedNativeRustInterop,
    sanitizers: bool,
    linker: Option<&OsStr>,
    vctools: Option<&OsStr>,
) -> Result<PreparedBuildInvocations, PhaseBLocalError> {
    let c_maximum = MAX_GENERATED_C_BYTES
        .checked_add(PHASE_B_INVOCATION_ARGUMENT_CAPACITY)
        .ok_or(PhaseBLocalError::BuilderBudget)?;
    let command_maximum = PHASE_B_INVOCATION_ARGUMENT_CAPACITY;
    let link_command_maximum = if cfg!(windows) {
        command_maximum
            .checked_add(
                PHASE_B_TOOL_RESOLVER_CAPACITY
                    .checked_mul(2)
                    .ok_or(PhaseBLocalError::BuilderBudget)?,
            )
            .ok_or(PhaseBLocalError::BuilderBudget)?
    } else {
        command_maximum
    };
    let staticlib_name = if cfg!(windows) {
        "semaprax_bridge.lib"
    } else {
        "libsemaprax_bridge.a"
    };
    let link_o0_name = if cfg!(windows) {
        "__semaprax_native_rust_link_O0.exe"
    } else {
        "__semaprax_native_rust_link_O0"
    };
    let link_o2_name = if cfg!(windows) {
        "__semaprax_native_rust_link_O2.exe"
    } else {
        "__semaprax_native_rust_link_O2"
    };
    Ok(PreparedBuildInvocations {
        c_o0: prepare_invocation(
            c_maximum,
            || {
                platform::prepare_c_compile_invocation(
                    &prepared.target.triple,
                    "module.c".as_ref(),
                    0,
                    sanitizers,
                    MAX_GENERATED_C_BYTES,
                )
            },
            platform::prepared_c_compile_owned_capacity,
        )?,
        c_o2: prepare_invocation(
            c_maximum,
            || {
                platform::prepare_c_compile_invocation(
                    &prepared.target.triple,
                    "module.c".as_ref(),
                    2,
                    sanitizers,
                    MAX_GENERATED_C_BYTES,
                )
            },
            platform::prepared_c_compile_owned_capacity,
        )?,
        rust: prepare_invocation(
            command_maximum,
            || {
                platform::prepare_rust_compile_invocation(
                    &prepared.target.triple,
                    "__semaprax_native_rust_link.rs".as_ref(),
                    staticlib_name.as_ref(),
                )
            },
            platform::prepared_rust_compile_owned_capacity,
        )?,
        c_main: prepare_invocation(
            c_maximum,
            || {
                platform::prepare_c_compile_invocation(
                    &prepared.target.triple,
                    "__semaprax_native_rust_main.c".as_ref(),
                    2,
                    sanitizers,
                    MAX_GENERATED_C_BYTES,
                )
            },
            platform::prepared_c_compile_owned_capacity,
        )?,
        link_o0: prepare_invocation(
            link_command_maximum,
            || {
                platform::prepare_link_invocation(
                    &prepared.target.triple,
                    linker,
                    vctools,
                    "__semaprax_native_rust_main.o".as_ref(),
                    "module_O0.o".as_ref(),
                    staticlib_name.as_ref(),
                    link_o0_name.as_ref(),
                    sanitizers,
                )
            },
            platform::prepared_link_owned_capacity,
        )?,
        run_o0: prepare_invocation(
            command_maximum,
            platform::prepare_run_invocation,
            platform::prepared_run_owned_capacity,
        )?,
        link_o2: prepare_invocation(
            link_command_maximum,
            || {
                platform::prepare_link_invocation(
                    &prepared.target.triple,
                    linker,
                    vctools,
                    "__semaprax_native_rust_main.o".as_ref(),
                    "module_O2.o".as_ref(),
                    staticlib_name.as_ref(),
                    link_o2_name.as_ref(),
                    sanitizers,
                )
            },
            platform::prepared_link_owned_capacity,
        )?,
        run_o2: prepare_invocation(
            command_maximum,
            platform::prepare_run_invocation,
            platform::prepared_run_owned_capacity,
        )?,
    })
}

/// Private phase-B static bundle construction. The output directory is
/// create-new and never merged with existing content.
const PHASE_B_PUBLICATION_MESSAGE: &str = "Native Rust Interop output publication failed";
const PHASE_B_COMPILE_MESSAGE: &str = "Native Rust Interop Clang compilation failed";
const PHASE_B_LINK_MESSAGE: &str = "Native Rust Interop Rust compilation or link failed";
const PHASE_B_UNSUPPORTED_MESSAGE: &str = "Native Rust Interop target or toolchain is unsupported";
const PHASE_B_REPLAY_MESSAGE: &str = "Native Rust Interop generated artifact replay failed";
const PHASE_B_BUILDER_BUDGET_MESSAGE: &str =
    "Native Rust Interop max_builder_bytes exceeds 33554432";
const PHASE_B_MANIFEST_BUDGET_MESSAGE: &str =
    "Native Rust Interop max_manifest_bytes exceeds 1048576";
const PHASE_B_TOOL_VERSION_CAPACITY: usize = 65_536;
const PHASE_B_TOOL_PATH_CAPACITY: usize = 32_768;
const PHASE_B_MISSING_WINDOWS_LINKER: &str =
    r"C:\__semaprax_missing_vctools__\bin\Hostx64\x64\link.exe";
const PHASE_B_MISSING_WINDOWS_VCTOOLS: &str = r"C:\__semaprax_missing_vctools__";
const PHASE_B_VERSION_COMMAND_CAPACITY: usize = 256;
const PHASE_B_TOOL_RESOLVER_CAPACITY: usize = PHASE_B_TOOL_PATH_CAPACITY * 7 + 256;
const PHASE_B_PROCESS_INVOCATIONS: usize = 12;
#[cfg(windows)]
const PHASE_B_PROCESS_ARENA_MAX_CAPACITY: usize = 1_245_188;
#[cfg(unix)]
const PHASE_B_PROCESS_ARENA_MAX_CAPACITY: usize = 0;

fn prepare_process_arena_authorized(
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<AuthorizedProcessArena, PhaseBLocalError> {
    if cfg!(windows) && (include.is_none() || libraries.is_none()) {
        return Err(PhaseBLocalError::Unsupported);
    }
    let plan = platform::prepare_process_arena_plan_with_environment(
        PHASE_B_PROCESS_INVOCATIONS,
        include,
        libraries,
    )
    .map_err(|error| match error {
        platform::Error::OutputLimit => PhaseBLocalError::BuilderBudget,
        platform::Error::Invalid
        | platform::Error::Unsupported
        | platform::Error::Exists
        | platform::Error::Changed
        | platform::Error::Spawn
        | platform::Error::Exit => PhaseBLocalError::Unsupported,
    })?;
    let required = platform::prepared_process_arena_plan_capacity(&plan);
    if required > PHASE_B_PROCESS_ARENA_MAX_CAPACITY {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let budget = reserve_phase_b(required)?;
    let arena = platform::materialize_process_arena_with_environment(plan, include, libraries)
        .map_err(|error| match error {
            platform::Error::OutputLimit => PhaseBLocalError::BuilderBudget,
            platform::Error::Invalid
            | platform::Error::Unsupported
            | platform::Error::Exists
            | platform::Error::Changed
            | platform::Error::Spawn
            | platform::Error::Exit => PhaseBLocalError::Unsupported,
        })?;
    if platform::prepared_process_arena_owned_capacity(&arena) != required {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    Ok(AuthorizedProcessArena::new(arena, budget))
}

#[cfg(test)]
fn note_phase_b_process_arena_drop(value: u8) {
    PHASE_B_PROCESS_ARENA_DROP_ORDER.with(|order| {
        PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH.with(|length| {
            let index = length.get();
            if index < 2 {
                let mut values = order.get();
                values[index] = value;
                order.set(values);
                length.set(index + 1);
            }
        });
    });
}

#[cfg(test)]
fn reset_phase_b_process_arena_drop_observer() {
    PHASE_B_PROCESS_ARENA_DROPS.with(|drops| drops.set(0));
    PHASE_B_PROCESS_ARENA_BUDGET_DROPS.with(|drops| drops.set(0));
    PHASE_B_PROCESS_ARENA_DROP_ORDER.with(|order| order.set([0; 2]));
    PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH.with(|length| length.set(0));
}

#[cfg(test)]
fn reset_phase_b_error_materialization_observer() {
    PHASE_B_EFFECT_STARTED.with(|started| started.set(false));
    PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(|count| count.set(0));
}

#[cfg(test)]
fn reset_phase_b_native_stage_arena_observer() {
    PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(|count| count.set(0));
    PHASE_B_NATIVE_STAGE_ARENA_SETS.with(|count| count.set(0));
    PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(|count| count.set(0));
}

#[cfg(test)]
fn reset_phase_b_object_authority_observer() {
    assert!(!PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get));
    let prior_length = PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
    assert!(prior_length == 0 || prior_length == 2);
    PHASE_B_OBJECT_AUTHORITY_TRANSFERS.with(|count| count.set(0));
    PHASE_B_OBJECT_AUTHORITY_DROPS.with(|count| count.set(0));
    PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS.with(|count| count.set(0));
    PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS.with(|count| count.set(0));
    PHASE_B_OBJECT_BYTES_DROPS.with(|count| count.set(0));
    PHASE_B_OBJECT_DROP_ORDER.with(|order| order.set([0; 2]));
    PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(0));
}

#[cfg(test)]
fn assert_phase_b_object_drop_order(expected: usize) {
    assert_eq!(
        PHASE_B_OBJECT_BYTES_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_OBJECT_AUTHORITY_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get),
        expected.saturating_mul(2),
    );
    assert_eq!(
        PHASE_B_OBJECT_DROP_ORDER.with(std::cell::Cell::get),
        if expected == 0 { [0, 0] } else { [1, 2] },
    );
    assert!(!PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get));
}

#[cfg(test)]
fn reset_phase_b_manifest_authority_observer() {
    assert!(!PHASE_B_MANIFEST_AUTHORITY_LIVE.with(std::cell::Cell::get));
    let prior_length = PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
    assert!(prior_length == 0 || prior_length == 2);
    PHASE_B_MANIFEST_PLAN_CAPACITY.with(|capacity| capacity.set(MAX_MANIFEST_BYTES));
    PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(|count| count.set(0));
    PHASE_B_MANIFEST_ARENA_GROWTHS.with(|count| count.set(0));
    PHASE_B_MANIFEST_AUTHORITY_TRANSFERS.with(|count| count.set(0));
    PHASE_B_MANIFEST_AUTHORITY_DROPS.with(|count| count.set(0));
    PHASE_B_MANIFEST_BYTES_DROPS.with(|count| count.set(0));
    PHASE_B_MANIFEST_DROP_ORDER.with(|order| order.set([0; 2]));
    PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(0));
}

#[cfg(test)]
fn assert_phase_b_manifest_drop_order(expected: usize) {
    assert_eq!(
        PHASE_B_MANIFEST_BYTES_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_MANIFEST_AUTHORITY_DROPS.with(std::cell::Cell::get),
        expected
    );
    assert_eq!(
        PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get),
        expected.saturating_mul(2)
    );
    assert_eq!(
        PHASE_B_MANIFEST_DROP_ORDER.with(std::cell::Cell::get),
        if expected == 0 { [0, 0] } else { [1, 2] }
    );
    assert!(!PHASE_B_MANIFEST_AUTHORITY_LIVE.with(std::cell::Cell::get));
}

#[cfg(not(test))]
fn reset_phase_b_error_materialization_observer() {}

#[cfg(test)]
fn mark_phase_b_effect_started() {
    PHASE_B_EFFECT_STARTED.with(|started| started.set(true));
}

#[cfg(not(test))]
fn mark_phase_b_effect_started() {}

#[cfg(test)]
fn observe_phase_b_error_materialization() {
    if PHASE_B_EFFECT_STARTED.with(std::cell::Cell::get) {
        PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
    }
}

#[cfg(not(test))]
fn observe_phase_b_error_materialization() {}

#[allow(
    clippy::vec_init_then_push,
    reason = "the one-element public diagnostic carrier requires an observed exact capacity"
)]
fn diagnostic_vector(error: Diagnostic) -> Vec<Diagnostic> {
    observe_phase_b_error_materialization();
    let mut errors = Vec::with_capacity(1);
    errors.push(error);
    errors
}

struct BundleBuildSuccess {
    facts: NativeRustInteropBundleFacts,
    overflow: Vec<Diagnostic>,
}

enum BundleBuildError {
    Diagnostic(Diagnostic),
    Prepared {
        selected: Vec<Diagnostic>,
        overflow: Option<Vec<Diagnostic>>,
    },
}

impl From<Diagnostic> for BundleBuildError {
    fn from(error: Diagnostic) -> Self {
        Self::Diagnostic(error)
    }
}

impl BundleBuildError {
    fn into_diagnostics(self, overflowed: bool) -> Vec<Diagnostic> {
        match self {
            Self::Diagnostic(error) => {
                if overflowed {
                    diagnostic_vector(b109("max_builder_bytes", MAX_BUILDER_BYTES))
                } else {
                    diagnostic_vector(error)
                }
            }
            Self::Prepared { selected, overflow } => {
                if overflowed {
                    overflow.unwrap_or(selected)
                } else {
                    selected
                }
            }
        }
    }
}

struct StickyDiagnosticCarrier {
    errors: Option<Vec<Diagnostic>>,
}

impl StickyDiagnosticCarrier {
    #[allow(
        clippy::vec_init_then_push,
        reason = "the pre-effect sticky diagnostic carrier requires an observed exact capacity"
    )]
    fn prepare(code: &'static str, message: &'static str) -> Result<Self, Diagnostic> {
        let maximum = message
            .len()
            .checked_add(std::mem::size_of::<Diagnostic>())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let authority = reserve_temporary_exact(maximum)?;
        observe_phase_b_error_materialization();
        let diagnostic = Diagnostic::io(code, message);
        let mut errors = Vec::with_capacity(1);
        errors.push(diagnostic);
        let retained = errors[0]
            .message
            .capacity()
            .checked_add(
                errors
                    .capacity()
                    .checked_mul(std::mem::size_of::<Diagnostic>())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if errors.capacity() != 1 || retained > maximum {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        authority.retain(retained)?;
        Ok(Self {
            errors: Some(errors),
        })
    }

    fn take(&mut self) -> Vec<Diagnostic> {
        self.errors
            .take()
            .expect("sticky phase-B diagnostic is consumed once")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhaseBLocalError {
    BuilderBudget,
    ManifestBudget,
    Unsupported,
    Replay,
    Compile,
    Link,
    Publication,
}

struct PreparedManifestPlan {
    file_names: [&'static str; 6],
    manifest: AuthorizedManifest,
}

impl PreparedManifestPlan {
    fn prepare(object_name: &'static str) -> Result<Self, PhaseBLocalError> {
        let file_names = canonical_manifest_file_names();
        if object_name != file_names[2] {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        #[cfg(test)]
        let capacity = PHASE_B_MANIFEST_PLAN_CAPACITY.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let capacity = MAX_MANIFEST_BYTES;
        let authority = reserve_phase_b(capacity)?;
        let arena = String::with_capacity(capacity);
        #[cfg(test)]
        PHASE_B_MANIFEST_ARENA_ALLOCATIONS.with(|count| count.set(count.get().saturating_add(1)));
        if arena.capacity() != MAX_MANIFEST_BYTES {
            return Err(PhaseBLocalError::ManifestBudget);
        }
        Ok(Self {
            file_names,
            manifest: AuthorizedManifest::new(arena, authority)?,
        })
    }

    #[allow(clippy::too_many_arguments, reason = "manifest inputs remain explicit")]
    fn render(
        mut self,
        prepared: &PreparedNativeRustInterop,
        files: &[(&str, &[u8]); 6],
        clang_path: &str,
        clang_version: &str,
        rustc: &RustcVersion,
        target: &str,
    ) -> Result<AuthorizedManifest, PhaseBLocalError> {
        if files
            .iter()
            .zip(self.file_names)
            .any(|((actual, _), expected)| *actual != expected)
            || self.manifest.manifest.bytes.capacity() != MAX_MANIFEST_BYTES
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        self.manifest.check()?;
        let mut count = CountingSink {
            bytes: 0,
            maximum: MAX_MANIFEST_BYTES,
            overflowed: false,
        };
        write_manifest(
            &mut count,
            prepared,
            files,
            clang_path,
            clang_version,
            rustc,
            target,
        )
        .map_err(|_| PhaseBLocalError::ManifestBudget)?;
        #[cfg(test)]
        if PHASE_B_OVERSIZE_MANIFEST_INJECTION.with(std::cell::Cell::get) {
            count.overflowed = true;
        }
        if count.overflowed {
            return Err(PhaseBLocalError::ManifestBudget);
        }
        write_manifest(
            &mut self.manifest.manifest.bytes,
            prepared,
            files,
            clang_path,
            clang_version,
            rustc,
            target,
        )
        .map_err(|_| PhaseBLocalError::ManifestBudget)?;
        #[cfg(test)]
        if self.manifest.manifest.bytes.capacity() != MAX_MANIFEST_BYTES {
            PHASE_B_MANIFEST_ARENA_GROWTHS.with(|count| count.set(count.get().saturating_add(1)));
        }
        if self.manifest.manifest.bytes.len() != count.bytes
            || self.manifest.manifest.bytes.capacity() != MAX_MANIFEST_BYTES
        {
            return Err(PhaseBLocalError::ManifestBudget);
        }
        self.manifest.check()?;
        Ok(self.manifest)
    }
}

impl PhaseBLocalError {
    const fn index(self) -> usize {
        match self {
            Self::BuilderBudget => 0,
            Self::ManifestBudget => 1,
            Self::Unsupported => 2,
            Self::Replay => 3,
            Self::Compile => 4,
            Self::Link => 5,
            Self::Publication => 6,
        }
    }

    const fn diagnostic(self) -> (&'static str, &'static str) {
        match self {
            Self::BuilderBudget => ("SPX-B109", PHASE_B_BUILDER_BUDGET_MESSAGE),
            Self::ManifestBudget => ("SPX-B109", PHASE_B_MANIFEST_BUDGET_MESSAGE),
            Self::Unsupported => ("SPX-B110", PHASE_B_UNSUPPORTED_MESSAGE),
            Self::Replay => ("SPX-B111", PHASE_B_REPLAY_MESSAGE),
            Self::Compile => ("SPX-I230", PHASE_B_COMPILE_MESSAGE),
            Self::Link => ("SPX-I231", PHASE_B_LINK_MESSAGE),
            Self::Publication => ("SPX-I232", PHASE_B_PUBLICATION_MESSAGE),
        }
    }
}

fn debit_phase_b(bytes: usize) -> Result<(), PhaseBLocalError> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(PhaseBLocalError::BuilderBudget)
    }
}

fn reserve_phase_b(maximum: usize) -> Result<TemporaryBudget, PhaseBLocalError> {
    let remaining = crate::bounded_output::remaining_active().unwrap_or(MAX_BUILDER_BYTES);
    if maximum > remaining {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    debit_phase_b(maximum)?;
    Ok(TemporaryBudget { reserved: maximum })
}

fn shrink_phase_b(authority: &mut TemporaryBudget, actual: usize) -> Result<(), PhaseBLocalError> {
    if actual > authority.reserved {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    crate::bounded_output::release_active(authority.reserved - actual);
    authority.reserved = actual;
    Ok(())
}

fn retain_phase_b(mut authority: TemporaryBudget, actual: usize) -> Result<(), PhaseBLocalError> {
    if actual > authority.reserved {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    crate::bounded_output::release_active(authority.reserved - actual);
    authority.reserved = 0;
    Ok(())
}

struct PhaseBErrorCarriers {
    carriers: [StickyDiagnosticCarrier; 7],
}

fn finish_bounded_bundle(
    result: Result<BundleBuildSuccess, BundleBuildError>,
    overflowed: bool,
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    match result {
        Ok(success) if overflowed => Err(success.overflow),
        Ok(success) => Ok(success.facts),
        Err(error) => Err(error.into_diagnostics(overflowed)),
    }
}

impl PhaseBErrorCarriers {
    fn prepare() -> Result<Self, Diagnostic> {
        let kinds = [
            PhaseBLocalError::BuilderBudget,
            PhaseBLocalError::ManifestBudget,
            PhaseBLocalError::Unsupported,
            PhaseBLocalError::Replay,
            PhaseBLocalError::Compile,
            PhaseBLocalError::Link,
            PhaseBLocalError::Publication,
        ];
        let carriers = kinds.map(|kind| {
            let (code, message) = kind.diagnostic();
            StickyDiagnosticCarrier::prepare(code, message)
        });
        let [Ok(builder), Ok(manifest), Ok(unsupported), Ok(replay), Ok(compile), Ok(link), Ok(publication)] =
            carriers
        else {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        };
        let carriers = [
            builder,
            manifest,
            unsupported,
            replay,
            compile,
            link,
            publication,
        ];
        #[cfg(test)]
        PHASE_B_PREPARED_CARRIER_IDENTITIES.with(|identities| {
            identities.set(std::array::from_fn(|index| {
                carriers[index].errors.as_ref().expect("prepared")[0]
                    .message
                    .as_ptr() as usize
            }));
        });
        Ok(Self { carriers })
    }

    fn take(&mut self, kind: PhaseBLocalError) -> Vec<Diagnostic> {
        self.carriers[kind.index()].take()
    }

    fn error(&mut self, kind: PhaseBLocalError) -> BundleBuildError {
        let selected = self.take(kind);
        let overflow = if kind == PhaseBLocalError::BuilderBudget {
            None
        } else {
            Some(self.take(PhaseBLocalError::BuilderBudget))
        };
        BundleBuildError::Prepared { selected, overflow }
    }
}

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

#[cfg(test)]
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeRustBuildPoint {
    BeforeClang,
    BeforeRustLink,
    BeforeExecutableAuthentication,
    BeforeExecute,
    BeforeObjectRead,
    BeforeManifestPublish,
    BeforeBundlePublish,
}

type PublishDiscardInventory = platform::PreparedDiscardInventory<7>;
type RunDiscardInventory = platform::PreparedDiscardInventory<10>;

fn native_relative_name_capacity(name: &OsStr) -> Result<usize, Diagnostic> {
    let bytes = name
        .to_str()
        .filter(|name| name.is_ascii())
        .ok_or_else(platform_publication_error)?
        .len();
    if cfg!(windows) {
        bytes
            .checked_mul(2)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    } else {
        bytes
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }
}

fn prepare_discard_inventory<const N: usize>(
    names: [&'static OsStr; N],
) -> Result<platform::PreparedDiscardInventory<N>, Diagnostic> {
    let retained = names.iter().try_fold(0usize, |bytes, name| {
        bytes
            .checked_add(native_relative_name_capacity(name)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    })?;
    let authority = reserve_temporary_exact(retained)?;
    let inventory = platform::prepare_discard_inventory_bounded(names, retained)
        .map_err(|_| platform_publication_error())?;
    if platform::prepared_discard_inventory_owned_capacity(&inventory) != retained {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    authority.retain(retained)?;
    Ok(inventory)
}

fn prepare_publish_discard_inventory() -> Result<PublishDiscardInventory, Diagnostic> {
    prepare_discard_inventory([
        OsStr::new("descriptor.json"),
        OsStr::new("module.c"),
        OsStr::new("semaprax_native_rust_interop.h"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        OsStr::new(if cfg!(windows) {
            "module.obj"
        } else {
            "module.o"
        }),
        OsStr::new("semaprax.native-rust-interop.json"),
    ])
}

fn prepare_run_discard_inventory() -> Result<RunDiscardInventory, Diagnostic> {
    prepare_discard_inventory([
        OsStr::new("module_O0.o"),
        OsStr::new("__semaprax_native_rust_link.rs"),
        OsStr::new("semaprax_native_rust_interop.rs"),
        OsStr::new("semaprax_native_rust_interop_ffi.rs"),
        OsStr::new("module_O2.o"),
        OsStr::new(if cfg!(windows) {
            "semaprax_bridge.lib"
        } else {
            "libsemaprax_bridge.a"
        }),
        OsStr::new("__semaprax_native_rust_main.c"),
        OsStr::new("__semaprax_native_rust_main.o"),
        OsStr::new(if cfg!(windows) {
            "__semaprax_native_rust_link_O0.exe"
        } else {
            "__semaprax_native_rust_link_O0"
        }),
        OsStr::new(if cfg!(windows) {
            "__semaprax_native_rust_link_O2.exe"
        } else {
            "__semaprax_native_rust_link_O2"
        }),
    ])
}

struct PreparedLinkCopies {
    safe_rust: (platform::PreparedLinkOrCopy, TemporaryBudget),
    private_ffi: (platform::PreparedLinkOrCopy, TemporaryBudget),
    optimized_object: (platform::PreparedLinkOrCopy, TemporaryBudget),
}

fn prepare_link_copy<const S: usize, const D: usize>(
    source: &platform::PreparedDiscardInventory<S>,
    source_name: &'static str,
    destination: &platform::PreparedDiscardInventory<D>,
    destination_name: &'static str,
) -> Result<(platform::PreparedLinkOrCopy, TemporaryBudget), PhaseBLocalError> {
    let required = platform::link_or_copy_required_capacity(
        source,
        source_name,
        destination,
        destination_name,
    )
    .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(required)?;
    let prepared =
        platform::prepare_link_or_copy(source, source_name, destination, destination_name)
            .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    if platform::prepared_link_or_copy_owned_capacity(&prepared) != required {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    #[cfg(test)]
    PHASE_B_LINK_COPY_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((prepared, budget))
}

fn prepare_link_copies(
    publish: &PublishDiscardInventory,
    run: &RunDiscardInventory,
    object_name: &'static str,
) -> Result<PreparedLinkCopies, PhaseBLocalError> {
    let prepared = PreparedLinkCopies {
        safe_rust: prepare_link_copy(
            publish,
            "semaprax_native_rust_interop.rs",
            run,
            "semaprax_native_rust_interop.rs",
        )?,
        private_ffi: prepare_link_copy(
            publish,
            "semaprax_native_rust_interop_ffi.rs",
            run,
            "semaprax_native_rust_interop_ffi.rs",
        )?,
        optimized_object: prepare_link_copy(publish, object_name, run, "module_O2.o")?,
    };
    #[cfg(all(test, debug_assertions))]
    let prepared = {
        let mut prepared = prepared;
        if PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION.with(std::cell::Cell::get) {
            platform::inject_link_or_copy_failure_before_authentication(&mut prepared.safe_rust.0);
        }
        prepared
    };
    Ok(prepared)
}

fn consume_link_copy<const S: usize, const D: usize>(
    plan: (platform::PreparedLinkOrCopy, TemporaryBudget),
    source: &platform::PreparedDiscardInventory<S>,
    destination_directory: &HeldStage,
    destination: &mut platform::PreparedDiscardInventory<D>,
    source_bytes: &[u8],
) -> Result<(), PhaseBLocalError> {
    let (prepared, budget) = plan;
    #[cfg(test)]
    PHASE_B_LINK_COPY_CONSUMPTIONS.with(|count| count.set(count.get().saturating_add(1)));
    let result = platform::link_or_copy_new_prepared(
        prepared,
        source,
        destination_directory.authority.held(),
        destination,
        source_bytes,
    )
    .map_err(|_| PhaseBLocalError::Publication);
    drop(budget);
    result
}

fn prepare_publish_inventory_exact(
    publish: &PublishDiscardInventory,
) -> Result<(platform::PreparedInventoryExact<7>, TemporaryBudget), PhaseBLocalError> {
    let required = platform::inventory_exact_required_capacity(publish)
        .map_err(|_| PhaseBLocalError::BuilderBudget)?;
    let budget = reserve_phase_b(required)?;
    let prepared =
        platform::prepare_inventory_exact(publish).map_err(|_| PhaseBLocalError::BuilderBudget)?;
    if platform::prepared_inventory_exact_owned_capacity(&prepared) != required
        || platform::prepared_inventory_exact_remaining(&prepared) != 2
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    #[cfg(test)]
    PHASE_B_INVENTORY_EXACT_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((prepared, budget))
}

fn scan_publish_inventory_exact(
    prepared: &mut platform::PreparedInventoryExact<7>,
    stage: &HeldStage,
    publish: &PublishDiscardInventory,
) -> Result<(), PhaseBLocalError> {
    #[cfg(test)]
    PHASE_B_INVENTORY_EXACT_SCANS.with(|count| count.set(count.get().saturating_add(1)));
    platform::inventory_exact_prepared(prepared, stage.authority.held(), publish)
        .map_err(|_| PhaseBLocalError::Publication)
}

fn prepare_final_publish(
    output: &Path,
) -> Result<(platform::PreparedPublishDirectory, TemporaryBudget), PhaseBLocalError> {
    let output_name = output.file_name().ok_or(PhaseBLocalError::Publication)?;
    let required = platform::publish_directory_required_capacity(output_name)
        .map_err(|_| PhaseBLocalError::Publication)?;
    let budget = reserve_phase_b(required)?;
    let prepared = platform::prepare_publish_directory(output_name)
        .map_err(|_| PhaseBLocalError::Publication)?;
    if platform::prepared_publish_directory_owned_capacity(&prepared) != required
        || platform::prepared_publish_directory_remaining(&prepared) != 1
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    #[cfg(test)]
    PHASE_B_PUBLISH_PLANS.with(|count| count.set(count.get().saturating_add(1)));
    Ok((prepared, budget))
}

#[cfg(not(test))]
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy)]
enum NativeRustBuildPoint {
    BeforeClang,
    BeforeRustLink,
    BeforeExecutableAuthentication,
    BeforeExecute,
    BeforeObjectRead,
    BeforeManifestPublish,
    BeforeBundlePublish,
}

#[cfg(test)]
fn build_native_rust_interop_bundle_with_hook(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
    mut hook: impl FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
) -> Result<NativeRustInteropBundleFacts, Vec<Diagnostic>> {
    reset_phase_b_error_materialization_observer();
    let (result, overflowed) = crate::bounded_output::with_limit(MAX_BUILDER_BYTES, || {
        build_native_rust_interop_bundle_bounded(program, spec_bytes, output, &mut hook)
    });
    finish_bounded_bundle(result, overflowed)
}

struct PreparedPhaseB {
    prepared: PreparedNativeRustInterop,
    pending_facts: PendingBundleFacts,
    publish_slot: StageSlot,
    run_slot: StageSlot,
    publish_files: PublishDiscardInventory,
    run_files: RunDiscardInventory,
    parent_path: PathBuf,
    carriers: PhaseBErrorCarriers,
    toolchain_plan: PreparedToolchainPlan,
    harness_plan: (String, TemporaryBudget),
    build_invocations: PreparedBuildInvocations,
    manifest_plan: PreparedManifestPlan,
    link_copies: PreparedLinkCopies,
    inventory_exact: (platform::PreparedInventoryExact<7>, TemporaryBudget),
    final_publish: (platform::PreparedPublishDirectory, TemporaryBudget),
}

fn prepare_phase_b(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
) -> Result<PreparedPhaseB, BundleBuildError> {
    let prepared = prepare_native_rust_interop_bounded(program, spec_bytes)?;
    let object_name: &'static str = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    let parent = output.parent().ok_or_else(platform_publication_error)?;
    let pending_facts = PendingBundleFacts::new(output, object_name)?;
    let publish_slot = StageSlot::new(parent, &prepared.descriptor_digest, "publish")?;
    let run_slot = StageSlot::new(parent, &prepared.descriptor_digest, "run")?;
    let publish_files = prepare_publish_discard_inventory()?;
    let run_files = prepare_run_discard_inventory()?;
    #[cfg(all(test, debug_assertions))]
    let run_files = {
        let mut run_files = run_files;
        run_files.inject_discard_failure_after_delete(
            PHASE_B_DISCARD_FAILURE_AFTER_DELETE.with(std::cell::Cell::get),
        );
        run_files
    };
    let parent_capacity = parent.as_os_str().as_encoded_bytes().len();
    let parent_budget = reserve_temporary_exact(parent_capacity)?;
    let parent_path = exact_path_copy(parent, parent_capacity)?;
    parent_budget.retain(parent_capacity)?;
    let mut carriers = PhaseBErrorCarriers::prepare()?;
    let toolchain_plan = match prepare_toolchain_plan() {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let harness_plan = match prepare_rust_harness(&prepared) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let build_invocations = match prepare_build_invocations(
        &prepared,
        planned_sanitizers(&toolchain_plan),
        planned_linker(&toolchain_plan),
        planned_vctools(&toolchain_plan),
    ) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let manifest_plan = match PreparedManifestPlan::prepare(object_name) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let link_copies = match prepare_link_copies(&publish_files, &run_files, object_name) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let inventory_exact = match prepare_publish_inventory_exact(&publish_files) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    let final_publish = match prepare_final_publish(output) {
        Ok(plan) => plan,
        Err(error) => return Err(carriers.error(error)),
    };
    Ok(PreparedPhaseB {
        prepared,
        pending_facts,
        publish_slot,
        run_slot,
        publish_files,
        run_files,
        parent_path,
        carriers,
        toolchain_plan,
        harness_plan,
        build_invocations,
        manifest_plan,
        link_copies,
        inventory_exact,
        final_publish,
    })
}

fn build_native_rust_interop_bundle_bounded(
    program: &Program,
    spec_bytes: &[u8],
    output: &Path,
    hook: &mut dyn FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
) -> Result<BundleBuildSuccess, BundleBuildError> {
    let PreparedPhaseB {
        prepared,
        mut pending_facts,
        publish_slot,
        run_slot,
        mut publish_files,
        mut run_files,
        parent_path,
        mut carriers,
        toolchain_plan,
        harness_plan,
        build_invocations,
        manifest_plan,
        link_copies,
        inventory_exact,
        mut final_publish,
    } = prepare_phase_b(program, spec_bytes, output)?;

    mark_phase_b_effect_started();
    #[cfg(test)]
    PHASE_B_OUTPUT_PROBES.with(|count| count.set(count.get().saturating_add(1)));
    if output.exists() {
        return Err(carriers.error(PhaseBLocalError::Publication));
    }
    let parent_authority = match hold_stage(parent_path) {
        Ok(parent) => parent,
        Err(error) => return Err(carriers.error(error)),
    };
    if parent_authority.recheck_local().is_err() {
        return Err(carriers.error(PhaseBLocalError::Publication));
    }
    let stage = match create_stage(&parent_authority, publish_slot, &publish_files) {
        Ok(stage) => stage,
        Err(error) => return Err(carriers.error(error)),
    };
    let run_stage = match parent_authority
        .recheck_local()
        .and_then(|()| create_stage(&parent_authority, run_slot, &run_files))
    {
        Ok(run_stage) => run_stage,
        Err(error) => {
            let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
            return Err(carriers.error(error));
        }
    };
    let build = (|| {
        #[cfg(test)]
        if let Some(error) = PHASE_B_LOCAL_FAILURE_INJECTION.with(std::cell::Cell::get) {
            return Err(error);
        }
        let mut tools =
            authenticate_toolchain(toolchain_plan, &prepared.target, run_stage.authority.held())?;
        build_stage_platform(
            &prepared,
            &mut tools,
            &stage,
            &run_stage,
            harness_plan,
            build_invocations,
            link_copies,
            inventory_exact,
            manifest_plan,
            output,
            hook,
            &mut run_files,
            &mut publish_files,
        )
    })();
    let cleanup = discard_run_stage(&parent_authority, &run_stage, &run_files);
    let mut facts = match (build, cleanup) {
        (Err(error), _) => {
            let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
            return Err(carriers.error(error));
        }
        (Ok(_), Err(error)) => {
            let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
            return Err(carriers.error(error));
        }
        (Ok(facts), Ok(())) => facts,
    };
    if let Err(error) = facts.observe_object_authority_for_manifest() {
        let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
        return Err(carriers.error(error));
    }
    if let Err(error) = pending_facts.bind_manifest_digest(facts.manifest.as_bytes()) {
        let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
        return Err(carriers.error(error));
    }
    let bundle_facts = pending_facts.finish();
    let publication: Result<NativeRustInteropBundleFacts, PhaseBLocalError> = (|| {
        parent_authority.recheck_local()?;
        stage.recheck_local()?;
        hook(
            NativeRustBuildPoint::BeforeBundlePublish,
            &stage.path,
            &run_stage.path,
            output,
        );
        publish_stage_platform(
            &parent_authority,
            &stage,
            output,
            &prepared,
            &mut facts,
            &mut publish_files,
            &mut final_publish.0,
        )?;
        Ok(bundle_facts)
    })();
    if publication.is_err() {
        let _ = discard_run_stage(&parent_authority, &stage, &publish_files);
    }
    match publication {
        Ok(facts) => Ok(BundleBuildSuccess {
            facts,
            overflow: carriers.take(PhaseBLocalError::BuilderBudget),
        }),
        Err(error) => Err(carriers.error(error)),
    }
}

struct HeldStage {
    path: PathBuf,
    authority: crate::workspace::AuthenticatedDirectory,
    discard_name: Option<platform::PreparedStageName>,
}

struct StageSlot {
    purpose: &'static str,
    digest_prefix: [u8; 16],
    name: String,
    path: PathBuf,
    path_capacity: usize,
    native_name: platform::PreparedStageName,
}

impl StageSlot {
    fn new(parent: &Path, digest: &str, purpose: &'static str) -> Result<Self, Diagnostic> {
        let digest = Sha256::digest(digest.as_bytes());
        let mut digest_prefix = [0_u8; 16];
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for (index, byte) in digest.iter().take(8).copied().enumerate() {
            digest_prefix[index * 2] = HEX[usize::from(byte >> 4)];
            digest_prefix[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        let path_capacity = exact_child_path_capacity(parent, PHASE_B_STAGE_NAME_CAPACITY)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let retained = PHASE_B_STAGE_NAME_CAPACITY
            .checked_add(path_capacity)
            .and_then(|bytes| {
                bytes.checked_add(if cfg!(windows) {
                    PHASE_B_STAGE_NAME_CAPACITY.checked_mul(2)?
                } else {
                    PHASE_B_STAGE_NAME_CAPACITY.checked_add(1)?
                })
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let authority = reserve_temporary_exact(retained)?;
        let name = String::with_capacity(PHASE_B_STAGE_NAME_CAPACITY);
        let path = PathBuf::with_capacity(path_capacity);
        let native_name = platform::prepare_stage_name_arena(PHASE_B_STAGE_NAME_CAPACITY)
            .map_err(|_| platform_publication_error())?;
        #[cfg(test)]
        PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
        if name.capacity() != PHASE_B_STAGE_NAME_CAPACITY || path.capacity() != path_capacity {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        authority.retain(retained)?;
        Ok(Self {
            purpose,
            digest_prefix,
            name,
            path,
            path_capacity,
            native_name,
        })
    }

    fn prepare(&mut self, parent: &Path, nonce: u32) -> Result<(), PhaseBLocalError> {
        self.name.clear();
        write!(
            self.name,
            ".semaprax-native-rust-interop-{}-{}-{}-{nonce}",
            self.purpose,
            std::process::id(),
            std::str::from_utf8(&self.digest_prefix).map_err(|_| PhaseBLocalError::Publication)?,
        )
        .map_err(|_| PhaseBLocalError::Publication)?;
        if self.name.capacity() != PHASE_B_STAGE_NAME_CAPACITY {
            return Err(PhaseBLocalError::Publication);
        }
        if !fill_exact_child_path(&mut self.path, parent, self.name.as_ref())
            || self.path.capacity() != self.path_capacity
            || !exact_child_path_matches(&self.path, parent, self.name.as_ref())
        {
            return Err(PhaseBLocalError::Publication);
        }
        self.native_name
            .set(self.name.as_ref())
            .map_err(|_| PhaseBLocalError::Publication)?;
        #[cfg(test)]
        PHASE_B_NATIVE_STAGE_ARENA_SETS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
        Ok(())
    }
}

impl HeldStage {
    fn recheck_local(&self) -> Result<(), PhaseBLocalError> {
        self.authority
            .recheck()
            .map_err(|_| PhaseBLocalError::Publication)?;
        if !self.authority.same_directory_path(&self.path) {
            return Err(PhaseBLocalError::Publication);
        }
        Ok(())
    }

    fn recheck(&self) -> Result<(), Diagnostic> {
        self.recheck_local()
            .map_err(|_| platform_publication_error())
    }
}

fn hold_stage(path: PathBuf) -> Result<HeldStage, PhaseBLocalError> {
    let authority = crate::workspace::authenticate_directory_held(&path)
        .map_err(|_| PhaseBLocalError::Publication)?;
    Ok(HeldStage {
        path,
        authority,
        discard_name: None,
    })
}

fn create_stage<const N: usize>(
    parent: &HeldStage,
    mut slot: StageSlot,
    inventory: &platform::PreparedDiscardInventory<N>,
) -> Result<HeldStage, PhaseBLocalError> {
    for nonce in 0_u32..1024 {
        slot.prepare(&parent.path, nonce)?;
        #[cfg(test)]
        PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS.with(|count| {
            count.set(count.get().saturating_add(1));
        });
        match platform::create_directory_new_prepared(
            parent.authority.held(),
            &slot.native_name,
            0o700,
        ) {
            Ok(held) => {
                #[cfg(test)]
                let authentication_path = match CREATE_AUTH_DISAGREEMENT.with(std::cell::Cell::get)
                {
                    None => None,
                    Some(CreateAuthDisagreement::Clean) => Some(parent.path.clone()),
                    Some(CreateAuthDisagreement::Substituted) => {
                        let displaced = parent.path.join("auth-displaced");
                        std::fs::rename(&slot.path, &displaced)
                            .map_err(|_| PhaseBLocalError::Publication)?;
                        std::fs::create_dir(&slot.path)
                            .map_err(|_| PhaseBLocalError::Publication)?;
                        std::fs::write(slot.path.join("foreign-sentinel"), b"foreign")
                            .map_err(|_| PhaseBLocalError::Publication)?;
                        Some(slot.path.clone())
                    }
                };
                #[cfg(test)]
                let authentication_path = authentication_path.as_deref().unwrap_or(&slot.path);
                #[cfg(not(test))]
                let authentication_path = &slot.path;
                let authority = match crate::workspace::authenticate_created_directory(
                    authentication_path,
                    held,
                ) {
                    Ok(authority) => authority,
                    Err(crate::workspace::CreatedDirectoryAuthenticationError::Disagreement(
                        raw_child,
                    )) => {
                        #[cfg(test)]
                        CREATE_AUTH_DISCARD_ATTEMPTS.with(|attempts| {
                            attempts.set(attempts.get().saturating_add(1));
                        });
                        let _ = platform::discard_owned_stage_prepared(
                            parent.authority.held(),
                            &raw_child,
                            &slot.native_name,
                            inventory,
                        );
                        return Err(PhaseBLocalError::Publication);
                    }
                };
                return Ok(HeldStage {
                    path: slot.path,
                    authority,
                    discard_name: Some(slot.native_name),
                });
            }
            Err(platform::Error::Exists) => {}
            Err(_) => break,
        }
    }
    Err(PhaseBLocalError::Publication)
}

#[cfg(test)]
fn exact_inventory(directory: &Path, expected: &BTreeSet<&str>) -> Result<(), Diagnostic> {
    let metadata = std::fs::symlink_metadata(directory)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    let mut actual = BTreeSet::new();
    for entry in std::fs::read_dir(directory)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?
    {
        if actual.len() >= expected.len() {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        let entry = entry.map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        let name = entry.file_name().into_string().map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if !expected.contains(name.as_str()) || !crate::bounded_output::reserve_active(name.len()) {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        if !actual.insert(name) {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
    }
    if actual.iter().map(String::as_str).collect::<BTreeSet<_>>() != *expected {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn match_regular_file(path: &Path, expected: &[u8]) -> Result<(), Diagnostic> {
    let before_path = std::fs::symlink_metadata(path)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if !before_path.is_file() || before_path.file_type().is_symlink() {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(false);
    #[cfg(all(unix, target_os = "macos"))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0000_0100);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(0x0002_0000);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        options.custom_flags(0x0020_0000);
    }
    let mut file = options
        .open(path)
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    let before = file
        .metadata()
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if !before.is_file()
        || !same_file_metadata(&before_path, &before)
        || before.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX)
    {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    let mut offset = 0_usize;
    let mut buffer = [0_u8; 8192];
    loop {
        let length = std::io::Read::read(&mut file, &mut buffer).map_err(|_| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if length == 0 {
            break;
        }
        let end = offset.checked_add(length).ok_or_else(|| {
            Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed")
        })?;
        if expected.get(offset..end) != Some(&buffer[..length]) {
            return Err(Diagnostic::io(
                "SPX-I232",
                "Native Rust Interop output publication failed",
            ));
        }
        offset = end;
    }
    let after = file
        .metadata()
        .map_err(|_| Diagnostic::io("SPX-I232", "Native Rust Interop output publication failed"))?;
    if offset != expected.len() || !same_file_metadata(&before, &after) {
        return Err(Diagnostic::io(
            "SPX-I232",
            "Native Rust Interop output publication failed",
        ));
    }
    Ok(())
}

struct ObjectAuthority {
    budget: TemporaryBudget,
}

impl ObjectAuthority {
    fn new(mut budget: TemporaryBudget, object_capacity: usize) -> Result<Self, PhaseBLocalError> {
        shrink_phase_b(&mut budget, object_capacity)?;
        if budget.maximum() != object_capacity {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        #[cfg(test)]
        {
            if PHASE_B_OBJECT_AUTHORITY_LIVE.with(|live| live.replace(true)) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            // The detailed order trace is per authorized object.  Aggregate
            // transfer/drop counters intentionally remain cumulative so tests
            // can also prove that repeated builder invocations release once.
            PHASE_B_OBJECT_DROP_ORDER.with(|order| order.set([0, 0]));
            PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(0));
            PHASE_B_OBJECT_AUTHORITY_TRANSFERS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(Self { budget })
    }

    fn check(&self, object: &[u8], object_capacity: usize) -> Result<(), PhaseBLocalError> {
        if self.budget.maximum() != object_capacity || object.len() > object_capacity {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(())
    }
}

impl Drop for ObjectAuthority {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            let index = PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 1, "object bytes must drop before their authority");
            PHASE_B_OBJECT_DROP_ORDER.with(|order| {
                let mut values = order.get();
                assert_eq!(values[0], 1);
                values[index] = 2;
                order.set(values);
            });
            PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            assert!(PHASE_B_OBJECT_AUTHORITY_LIVE.with(|live| live.replace(false)));
            PHASE_B_OBJECT_AUTHORITY_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ObjectDropGuard;

impl Drop for ObjectDropGuard {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            assert!(PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get));
            let index = PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 0);
            PHASE_B_OBJECT_DROP_ORDER.with(|order| {
                let mut values = order.get();
                values[index] = 1;
                order.set(values);
            });
            PHASE_B_OBJECT_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            PHASE_B_OBJECT_BYTES_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ObjectBytes {
    bytes: Vec<u8>,
    drop_guard: ObjectDropGuard,
}

struct AuthorizedObject {
    object: ObjectBytes,
    authority: ObjectAuthority,
}

impl AuthorizedObject {
    fn new(bytes: Vec<u8>, budget: TemporaryBudget) -> Result<Self, PhaseBLocalError> {
        let authority = ObjectAuthority::new(budget, bytes.capacity())?;
        Ok(Self {
            object: ObjectBytes {
                bytes,
                drop_guard: ObjectDropGuard,
            },
            authority,
        })
    }

    fn as_slice(&self) -> &[u8] {
        &self.object.bytes
    }

    fn check(&self) -> Result<(), PhaseBLocalError> {
        let _ = &self.object.drop_guard;
        self.authority
            .check(self.as_slice(), self.object.bytes.capacity())
    }
}

struct ManifestAuthority {
    budget: TemporaryBudget,
}

impl ManifestAuthority {
    fn check(&self, manifest: &String) -> Result<(), PhaseBLocalError> {
        if self.budget.maximum() != manifest.capacity()
            || manifest.len() > self.budget.maximum()
            || manifest.capacity() != MAX_MANIFEST_BYTES
        {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        Ok(())
    }
}

impl Drop for ManifestAuthority {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            let index = PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 1, "manifest bytes must drop before their authority");
            PHASE_B_MANIFEST_DROP_ORDER.with(|order| {
                let mut values = order.get();
                assert_eq!(values[0], 1);
                values[index] = 2;
                order.set(values);
            });
            PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            assert!(PHASE_B_MANIFEST_AUTHORITY_LIVE.with(|live| live.replace(false)));
            PHASE_B_MANIFEST_AUTHORITY_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ManifestDropGuard;

impl Drop for ManifestDropGuard {
    fn drop(&mut self) {
        #[cfg(test)]
        {
            assert!(PHASE_B_MANIFEST_AUTHORITY_LIVE.with(std::cell::Cell::get));
            let index = PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(std::cell::Cell::get);
            assert_eq!(index, 0);
            PHASE_B_MANIFEST_DROP_ORDER.with(|order| {
                let mut values = order.get();
                values[index] = 1;
                order.set(values);
            });
            PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(index + 1));
            PHASE_B_MANIFEST_BYTES_DROPS.with(|count| count.set(count.get().saturating_add(1)));
        }
    }
}

struct ManifestBytes {
    bytes: String,
    drop_guard: ManifestDropGuard,
}

struct AuthorizedManifest {
    manifest: ManifestBytes,
    authority: ManifestAuthority,
}

impl AuthorizedManifest {
    fn new(bytes: String, budget: TemporaryBudget) -> Result<Self, PhaseBLocalError> {
        if bytes.capacity() != budget.maximum() || bytes.capacity() != MAX_MANIFEST_BYTES {
            return Err(PhaseBLocalError::BuilderBudget);
        }
        #[cfg(test)]
        {
            if PHASE_B_MANIFEST_AUTHORITY_LIVE.with(|live| live.replace(true)) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            PHASE_B_MANIFEST_DROP_ORDER.with(|order| order.set([0, 0]));
            PHASE_B_MANIFEST_DROP_ORDER_LENGTH.with(|length| length.set(0));
            PHASE_B_MANIFEST_AUTHORITY_TRANSFERS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(Self {
            manifest: ManifestBytes {
                bytes,
                drop_guard: ManifestDropGuard,
            },
            authority: ManifestAuthority { budget },
        })
    }

    fn as_str(&self) -> &str {
        &self.manifest.bytes
    }

    fn as_bytes(&self) -> &[u8] {
        self.as_str().as_bytes()
    }

    fn check(&self) -> Result<(), PhaseBLocalError> {
        let _ = &self.manifest.drop_guard;
        self.authority.check(&self.manifest.bytes)
    }
}

struct BuildStageFacts {
    object_name: &'static str,
    object: AuthorizedObject,
    manifest: AuthorizedManifest,
    inventory_exact: (platform::PreparedInventoryExact<7>, TemporaryBudget),
}

impl BuildStageFacts {
    fn observe_object_authority_for_manifest(&self) -> Result<(), PhaseBLocalError> {
        self.object.check()?;
        self.manifest.check()?;
        #[cfg(test)]
        {
            if !PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(())
    }

    fn observe_object_authority_for_publish(&self) -> Result<(), PhaseBLocalError> {
        self.object.check()?;
        self.manifest.check()?;
        #[cfg(test)]
        {
            if !PHASE_B_OBJECT_AUTHORITY_LIVE.with(std::cell::Cell::get) {
                return Err(PhaseBLocalError::BuilderBudget);
            }
            PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS
                .with(|count| count.set(count.get().saturating_add(1)));
        }
        Ok(())
    }
}

fn platform_publication_error() -> Diagnostic {
    observe_phase_b_error_materialization();
    Diagnostic::io("SPX-I232", PHASE_B_PUBLICATION_MESSAGE)
}

fn platform_compile_error() -> Diagnostic {
    Diagnostic::io("SPX-I230", "Native Rust Interop Clang compilation failed")
}

fn platform_link_error() -> Diagnostic {
    Diagnostic::io(
        "SPX-I231",
        "Native Rust Interop Rust compilation or link failed",
    )
}

fn map_link_error(_error: platform::Error) -> Diagnostic {
    #[cfg(test)]
    eprintln!("native rust platform link error: {_error:?}");
    platform_link_error()
}

fn write_platform_file<const N: usize>(
    directory: &HeldStage,
    files: &mut platform::PreparedDiscardInventory<N>,
    name: &str,
    bytes: &[u8],
) -> Result<(), PhaseBLocalError> {
    files
        .validate_next(name)
        .map_err(|_| PhaseBLocalError::Publication)?;
    directory.recheck_local()?;
    platform::write_file_new_prepared(directory.authority.held(), files, name, bytes, 0o600)
        .map_err(|_| PhaseBLocalError::Publication)
}

fn discard_run_stage<const N: usize>(
    parent: &HeldStage,
    stage: &HeldStage,
    files: &platform::PreparedDiscardInventory<N>,
) -> Result<(), PhaseBLocalError> {
    #[cfg(test)]
    PHASE_B_DISCARD_ATTEMPTS.with(|attempts| {
        attempts.set(attempts.get().saturating_add(1));
    });
    let stage_name = stage
        .discard_name
        .as_ref()
        .ok_or(PhaseBLocalError::Publication)?;
    platform::discard_owned_stage_prepared(
        parent.authority.held(),
        stage.authority.held(),
        stage_name,
        files,
    )
    .map_err(|_| PhaseBLocalError::Publication)
}

fn track_run_file<const N: usize>(
    files: &mut platform::PreparedDiscardInventory<N>,
    name: &str,
    file: platform::HeldRegularFile,
) -> Result<(), PhaseBLocalError> {
    files
        .attach(name, file)
        .map_err(|_| PhaseBLocalError::Publication)
}

fn recheck_tracked_files<const N: usize>(
    files: &platform::PreparedDiscardInventory<N>,
    required: &[&str],
) -> Result<(), PhaseBLocalError> {
    files
        .recheck(required)
        .map_err(|_| PhaseBLocalError::Publication)
}

const REQUIRED_NATIVE_RUST_SANITIZER_FLAGS: [&str; 2] =
    ["-fsanitize=address,undefined", "-fno-sanitize-recover=all"];

fn sanitizer_mode() -> Result<bool, PhaseBLocalError> {
    match std::env::var_os("SEMAPRAX_REQUIRE_NATIVE_RUST_INTEROP_SANITIZERS") {
        None => Ok(false),
        Some(value) if value == "1" && cfg!(target_os = "linux") => {
            let _ = REQUIRED_NATIVE_RUST_SANITIZER_FLAGS;
            Ok(true)
        }
        Some(_) => Err(PhaseBLocalError::Link),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "held publish and run inventories stay explicit"
)]
fn build_stage_platform(
    prepared: &PreparedNativeRustInterop,
    tools: &mut ToolchainFacts,
    stage: &HeldStage,
    run_stage: &HeldStage,
    harness_plan: (String, TemporaryBudget),
    build_invocations: PreparedBuildInvocations,
    link_copies: PreparedLinkCopies,
    mut inventory_exact: (platform::PreparedInventoryExact<7>, TemporaryBudget),
    manifest_plan: PreparedManifestPlan,
    output: &Path,
    hook: &mut dyn FnMut(NativeRustBuildPoint, &Path, &Path, &Path),
    run_files: &mut RunDiscardInventory,
    publish_files: &mut PublishDiscardInventory,
) -> Result<BuildStageFacts, PhaseBLocalError> {
    let PreparedBuildInvocations {
        c_o0,
        c_o2,
        rust,
        c_main,
        link_o0,
        run_o0,
        link_o2,
        run_o2,
    } = build_invocations;
    let PreparedLinkCopies {
        safe_rust,
        private_ffi,
        optimized_object,
    } = link_copies;
    for (name, bytes) in [
        ("descriptor.json", prepared.descriptor.as_bytes()),
        ("module.c", prepared.generated_c.as_bytes()),
        (
            "semaprax_native_rust_interop.h",
            prepared.generated_header.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop.rs",
            prepared.generated_rust.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop_ffi.rs",
            prepared.private_ffi_source.as_bytes(),
        ),
    ] {
        write_platform_file(stage, publish_files, name, bytes)?;
    }
    let object_name = if cfg!(windows) {
        "module.obj"
    } else {
        "module.o"
    };
    hook(
        NativeRustBuildPoint::BeforeClang,
        &stage.path,
        &run_stage.path,
        output,
    );
    stage.recheck_local()?;
    recheck_tracked_files(
        publish_files,
        &["module.c", "semaprax_native_rust_interop.h"],
    )?;
    let mut retained_object = None;
    for (optimization, invocation) in [(0_u8, c_o0), (2_u8, c_o2)] {
        let (invocation, invocation_budget) = consume_invocation(invocation);
        let output = platform::compile_c_tool_prepared(
            &tools.clang,
            stage.authority.held(),
            invocation,
            tools
                .process_arena
                .as_mut()
                .ok_or(PhaseBLocalError::BuilderBudget)?
                .arena_mut()?,
        )
        .map_err(|_| PhaseBLocalError::Compile)?;
        let object = output.into_bytes();
        if optimization == 0 {
            let name = "module_O0.o";
            write_platform_file(run_stage, run_files, name, &object)?;
            drop(object);
            drop(invocation_budget);
        } else {
            retained_object = Some((object, invocation_budget));
        }
    }
    let (object, object_invocation_budget) = retained_object.ok_or(PhaseBLocalError::Compile)?;
    let object = AuthorizedObject::new(object, object_invocation_budget)?;
    object.check()?;
    write_platform_file(stage, publish_files, object_name, object.as_slice())?;

    let (harness, harness_budget) = harness_plan;
    write_platform_file(
        run_stage,
        run_files,
        "__semaprax_native_rust_link.rs",
        harness.as_bytes(),
    )?;
    drop(harness);
    drop(harness_budget);
    consume_link_copy(
        safe_rust,
        publish_files,
        run_stage,
        run_files,
        prepared.generated_rust.as_bytes(),
    )?;
    consume_link_copy(
        private_ffi,
        publish_files,
        run_stage,
        run_files,
        prepared.private_ffi_source.as_bytes(),
    )?;
    consume_link_copy(
        optimized_object,
        publish_files,
        run_stage,
        run_files,
        object.as_slice(),
    )?;
    #[cfg(windows)]
    platform::transition_regular_file_to_external_read_prepared(
        stage.authority.held(),
        publish_files,
        object_name,
    )
    .map_err(|_| PhaseBLocalError::Publication)?;

    let staticlib_name = if cfg!(windows) {
        "semaprax_bridge.lib"
    } else {
        "libsemaprax_bridge.a"
    };
    hook(
        NativeRustBuildPoint::BeforeRustLink,
        &stage.path,
        &run_stage.path,
        output,
    );
    recheck_tracked_files(
        run_files,
        &[
            "__semaprax_native_rust_link.rs",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
            "module_O0.o",
            "module_O2.o",
        ],
    )?;
    let (rust, rust_invocation_budget) = consume_invocation(rust);
    let staticlib = platform::compile_rust_tool_prepared(
        &tools.rustc,
        run_stage.authority.held(),
        rust,
        tools
            .process_arena
            .as_mut()
            .ok_or(PhaseBLocalError::BuilderBudget)?
            .arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Link)?;
    drop(rust_invocation_budget);
    track_run_file(run_files, staticlib_name, staticlib)?;

    let c_harness = "extern int spxnr1_rust_harness_run(void);int main(void){return spxnr1_rust_harness_run();}\n";
    write_platform_file(
        run_stage,
        run_files,
        "__semaprax_native_rust_main.c",
        c_harness.as_bytes(),
    )?;
    let (c_main, c_main_invocation_budget) = consume_invocation(c_main);
    let harness_object = platform::compile_c_tool_prepared(
        &tools.clang,
        run_stage.authority.held(),
        c_main,
        tools
            .process_arena
            .as_mut()
            .ok_or(PhaseBLocalError::BuilderBudget)?
            .arena_mut()?,
    )
    .map_err(|_| PhaseBLocalError::Compile)?;
    write_platform_file(
        run_stage,
        run_files,
        "__semaprax_native_rust_main.o",
        harness_object.bytes(),
    )?;
    drop(harness_object);
    drop(c_main_invocation_budget);
    #[cfg(windows)]
    for name in ["__semaprax_native_rust_main.o", staticlib_name] {
        platform::transition_regular_file_to_external_read_prepared(
            run_stage.authority.held(),
            run_files,
            name,
        )
        .map_err(|_| PhaseBLocalError::Publication)?;
    }
    for (optimization, link_invocation, run_invocation) in
        [(0_u8, link_o0, run_o0), (2_u8, link_o2, run_o2)]
    {
        let c_object = if optimization == 0 {
            "module_O0.o"
        } else {
            "module_O2.o"
        };
        let executable_name = if cfg!(windows) {
            if optimization == 0 {
                "__semaprax_native_rust_link_O0.exe"
            } else {
                "__semaprax_native_rust_link_O2.exe"
            }
        } else if optimization == 0 {
            "__semaprax_native_rust_link_O0"
        } else {
            "__semaprax_native_rust_link_O2"
        };
        #[cfg(windows)]
        platform::transition_regular_file_to_external_read_prepared(
            run_stage.authority.held(),
            run_files,
            c_object,
        )
        .map_err(|_| PhaseBLocalError::Publication)?;
        hook(
            NativeRustBuildPoint::BeforeExecutableAuthentication,
            &stage.path,
            &run_stage.path,
            output,
        );
        recheck_tracked_files(
            run_files,
            &["__semaprax_native_rust_main.o", c_object, staticlib_name],
        )?;
        let (link_invocation, link_invocation_budget) = consume_invocation(link_invocation);
        let executable = platform::link_tool_prepared(
            &tools.clang,
            tools.linker.as_ref(),
            run_stage.authority.held(),
            link_invocation,
            tools
                .process_arena
                .as_mut()
                .ok_or(PhaseBLocalError::BuilderBudget)?
                .arena_mut()?,
        )
        .map_err(|_| PhaseBLocalError::Link)?;
        drop(link_invocation_budget);
        let executable_file = platform::executable_regular_file(&executable)
            .map_err(|_| PhaseBLocalError::Publication)?;
        track_run_file(run_files, executable_name, executable_file)?;
        hook(
            NativeRustBuildPoint::BeforeExecute,
            &stage.path,
            &run_stage.path,
            output,
        );
        let (run_invocation, run_invocation_budget) = consume_invocation(run_invocation);
        platform::execute_tool_prepared(
            &executable,
            run_stage.authority.held(),
            run_invocation,
            tools
                .process_arena
                .as_mut()
                .ok_or(PhaseBLocalError::BuilderBudget)?
                .arena_mut()?,
        )
        .map_err(|_| PhaseBLocalError::Link)?;
        drop(run_invocation_budget);
    }
    let process_arena = tools
        .process_arena
        .take()
        .ok_or(PhaseBLocalError::BuilderBudget)?;
    if platform::prepared_process_arena_remaining(process_arena.arena()?) != 0 {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    let process_arena_capacity = process_arena.authorized_capacity()?;
    if platform::prepared_process_arena_owned_capacity(process_arena.arena()?)
        != process_arena_capacity
    {
        return Err(PhaseBLocalError::BuilderBudget);
    }
    drop(process_arena);
    hook(
        NativeRustBuildPoint::BeforeObjectRead,
        &stage.path,
        &run_stage.path,
        output,
    );
    recheck_tracked_files(publish_files, &[object_name])?;

    let manifest_file_names = canonical_manifest_file_names();
    let files = [
        (manifest_file_names[0], prepared.descriptor.as_bytes()),
        (manifest_file_names[1], prepared.generated_c.as_bytes()),
        (manifest_file_names[2], object.as_slice()),
        (manifest_file_names[3], prepared.generated_header.as_bytes()),
        (manifest_file_names[4], prepared.generated_rust.as_bytes()),
        (
            manifest_file_names[5],
            prepared.private_ffi_source.as_bytes(),
        ),
    ];
    let manifest = manifest_plan.render(
        prepared,
        &files,
        platform::tool_path(&tools.clang),
        &tools.clang_version,
        &tools.rustc_version,
        &prepared.target.triple,
    )?;
    replay_manifest(manifest.as_str(), prepared, &files, tools)?;
    hook(
        NativeRustBuildPoint::BeforeManifestPublish,
        &stage.path,
        &run_stage.path,
        output,
    );
    recheck_tracked_files(
        publish_files,
        &[
            "descriptor.json",
            "module.c",
            object_name,
            "semaprax_native_rust_interop.h",
            "semaprax_native_rust_interop.rs",
            "semaprax_native_rust_interop_ffi.rs",
        ],
    )?;
    write_platform_file(
        stage,
        publish_files,
        "semaprax.native-rust-interop.json",
        manifest.as_bytes(),
    )?;
    scan_publish_inventory_exact(&mut inventory_exact.0, stage, publish_files)?;
    Ok(BuildStageFacts {
        object_name,
        object,
        manifest,
        inventory_exact,
    })
}

fn publish_stage_platform(
    parent: &HeldStage,
    stage: &HeldStage,
    output: &Path,
    prepared: &PreparedNativeRustInterop,
    facts: &mut BuildStageFacts,
    publish_files: &mut PublishDiscardInventory,
    final_publish: &mut platform::PreparedPublishDirectory,
) -> Result<(), PhaseBLocalError> {
    facts.observe_object_authority_for_publish()?;
    parent.recheck_local()?;
    stage.recheck_local()?;
    let mut comparison_scratch = [0_u8; platform::FILE_COMPARE_SCRATCH_BYTES];
    for (name, expected) in [
        ("descriptor.json", prepared.descriptor.as_bytes()),
        ("module.c", prepared.generated_c.as_bytes()),
        (
            "semaprax.native-rust-interop.json",
            facts.manifest.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop.h",
            prepared.generated_header.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop.rs",
            prepared.generated_rust.as_bytes(),
        ),
        (
            "semaprax_native_rust_interop_ffi.rs",
            prepared.private_ffi_source.as_bytes(),
        ),
        (facts.object_name, facts.object.as_slice()),
    ] {
        let held = publish_files
            .file(name)
            .map_err(|_| PhaseBLocalError::Publication)?;
        let matches = platform::compare_exact(held, expected, &mut comparison_scratch)
            .map_err(|_| PhaseBLocalError::Publication)?;
        if !matches {
            return Err(PhaseBLocalError::Publication);
        }
    }
    scan_publish_inventory_exact(&mut facts.inventory_exact.0, stage, publish_files)?;
    let output_name = output.file_name().ok_or(PhaseBLocalError::Publication)?;
    let stage_name = stage
        .discard_name
        .as_ref()
        .ok_or(PhaseBLocalError::Publication)?;
    #[cfg(all(test, debug_assertions))]
    if let Some(point) =
        PHASE_B_PUBLISH_FAILURE.with(|point| (point.get() != 0).then(|| point.get()))
    {
        platform::inject_publish_directory_failure(final_publish, point)
            .map_err(|_| PhaseBLocalError::Publication)?;
    }
    publish_files
        .settle_for_publish()
        .map_err(|_| PhaseBLocalError::Publication)?;
    let publication = platform::publish_directory_new_prepared(
        final_publish,
        parent.authority.held(),
        stage.authority.held(),
        stage_name,
        output_name,
    );
    #[cfg(test)]
    if platform::prepared_publish_directory_remaining(final_publish) == 0 {
        PHASE_B_PUBLISH_CONSUMPTIONS.with(|count| count.set(count.get().saturating_add(1)));
    }
    publication.map_err(|_| PhaseBLocalError::Publication)?;
    Ok(())
}

// Proof-only implementation tests are isolated in `implementation/tests.rs`; this file retains production authority.
#[cfg(test)]
#[path = "implementation/tests.rs"]
mod tests;
