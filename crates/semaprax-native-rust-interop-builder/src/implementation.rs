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
// Pinned by module-local assertions beside the private iterative enums.
const HIR_RESOLVER_FRAME_BYTES: usize = 552;
const HIR_VALIDATOR_FRAME_BYTES: usize = 288;
const SOURCE_VERIFIER_FRAME_BYTES: usize = 320;
const SOURCE_VARIANT_MATCH_STATE_BYTES: usize = 312;
const CLEANUP_INVENTORY_SHAPE_FRAME_BYTES: usize = 40;
const CLEANUP_INVENTORY_EXPR_FRAME_BYTES: usize = 24;
const CLEANUP_LOWER_FRAME_BYTES: usize = 344;
const CLEANUP_EVAL_RESULT_BYTES: usize = 128;
const CALL_INDEX_FRAME_BYTES: usize = 16;
const C_EXPRESSION_FRAME_BYTES: usize = std::mem::size_of::<CExpressionFrame<'static>>();
const REPLAY_C_EXPRESSION_FRAME_BYTES: usize =
    std::mem::size_of::<ReplayCExpressionFrame<'static>>();
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
        Type::Named { .. } => None,
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
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => None,
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
                ResolvedType::Unit | ResolvedType::I64 | ResolvedType::Bool => {
                    let text = match ty {
                        ResolvedType::Unit => "unit",
                        ResolvedType::I64 => "i64",
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
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
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

fn validate_native_rust_source_expression_budget(program: &Program) -> Result<(), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for function in &program.functions {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut stack_len = 1;
            stack[0] = Some((root, 1_usize, 0_usize));
            while stack_len != 0 {
                stack_len -= 1;
                let (expression, depth, next_child) = stack[stack_len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child == 0 {
                    debit(std::mem::size_of::<&crate::ast::Expr>())?;
                    if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                }
                if let Some(child) = ast_child(expression, next_child) {
                    if stack_len + 2 > stack.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    stack[stack_len] = Some((expression, depth, next_child + 1));
                    stack[stack_len + 1] = Some((child, depth + 1, 0));
                    stack_len += 2;
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct AstCapacityStats {
    nodes: usize,
    cumulative_depth: usize,
    generic_calls: usize,
    max_depth: usize,
    max_match_arms: usize,
    max_indexed_children: usize,
    depth_arm_product_sum: usize,
    depth_width_product_sum: usize,
    local_bindings: usize,
    pattern_bindings: usize,
    binding_name_bytes: usize,
    binding_depth_sum: usize,
    max_index_digits: usize,
}

fn ast_child(expression: &crate::ast::Expr, index: usize) -> Option<&crate::ast::Expr> {
    match &expression.kind {
        crate::ast::ExprKind::Call { args, .. } => args.get(index),
        crate::ast::ExprKind::Unary { value, .. }
        | crate::ast::ExprKind::Try { operand: value }
        | crate::ast::ExprKind::Project { base: value, .. } => (index == 0).then_some(value),
        crate::ast::ExprKind::Binary { left, right, .. } => {
            [left.as_ref(), right.as_ref()].get(index).copied()
        }
        crate::ast::ExprKind::Block { statements, tail } => statements
            .get(index)
            .map(|statement| {
                let crate::ast::Statement::Let { value, .. } = statement;
                value
            })
            .or_else(|| (index == statements.len()).then_some(tail)),
        crate::ast::ExprKind::If {
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
        crate::ast::ExprKind::ConstructRecord { fields, .. }
        | crate::ast::ExprKind::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        crate::ast::ExprKind::Match { scrutinee, arms } => {
            if index == 0 {
                Some(scrutinee)
            } else {
                arms.get(index - 1).map(|arm| &arm.value)
            }
        }
        crate::ast::ExprKind::UpdateRecord { base, fields } => {
            if index == 0 {
                Some(base)
            } else {
                fields.get(index - 1).map(|field| &field.value)
            }
        }
        crate::ast::ExprKind::Int(_)
        | crate::ast::ExprKind::Bool(_)
        | crate::ast::ExprKind::Var(_) => None,
    }
}

fn ast_child_identity_path_increment(
    expression: &crate::ast::Expr,
    child_index: usize,
    program: &Program,
) -> usize {
    match &expression.kind {
        crate::ast::ExprKind::Call { name, .. } => {
            let prefix = if program
                .interfaces
                .iter()
                .any(|interface| interface.imports.iter().any(|import| import.name == *name))
            {
                ".native-rust-arg."
            } else {
                ".arg."
            };
            prefix.len() + decimal_digits(child_index)
        }
        crate::ast::ExprKind::Unary { .. } => ".value".len(),
        crate::ast::ExprKind::Binary { .. } => {
            if child_index == 0 { ".left" } else { ".right" }.len()
        }
        crate::ast::ExprKind::Block { statements, .. } => {
            if child_index < statements.len() {
                ".s".len() + decimal_digits(child_index) + ".value".len()
            } else {
                ".tail".len()
            }
        }
        crate::ast::ExprKind::If { .. } => [".condition", ".then", ".else"]
            .get(child_index)
            .map_or(0, |segment| segment.len()),
        crate::ast::ExprKind::ConstructRecord { .. }
        | crate::ast::ExprKind::ConstructVariant { .. } => {
            ".field.".len() + decimal_digits(child_index) + ".value".len()
        }
        crate::ast::ExprKind::Match { .. } => {
            if child_index == 0 {
                ".scrutinee".len()
            } else {
                ".arm.".len() + decimal_digits(child_index - 1) + ".value".len()
            }
        }
        crate::ast::ExprKind::UpdateRecord { .. } => {
            if child_index == 0 {
                ".base".len()
            } else {
                ".field.".len() + decimal_digits(child_index - 1) + ".value".len()
            }
        }
        crate::ast::ExprKind::Try { .. } => ".operand".len(),
        crate::ast::ExprKind::Project { .. } => ".base".len(),
        crate::ast::ExprKind::Int(_)
        | crate::ast::ExprKind::Bool(_)
        | crate::ast::ExprKind::Var(_) => 0,
    }
}

fn ast_root_identity_path_len(function: &crate::ast::Function, root_index: usize) -> usize {
    match root_index.cmp(&function.requires.len()) {
        std::cmp::Ordering::Less => "requires.".len() + decimal_digits(root_index),
        std::cmp::Ordering::Equal => "body".len(),
        std::cmp::Ordering::Greater => {
            "ensures.".len() + decimal_digits(root_index - function.requires.len() - 1)
        }
    }
}

fn ast_type_identity_key_len(program: &Program, root: &crate::ast::Type) -> Option<usize> {
    #[derive(Clone, Copy)]
    enum Frame<'a> {
        Enter(&'a crate::ast::Type),
        Finish(&'a crate::ast::TypeDeclaration, usize),
    }

    let mut frames = [None; MAX_FORMAT_NESTING * 2];
    let mut results = [0usize; MAX_FORMAT_NESTING];
    frames[0] = Some(Frame::Enter(root));
    let mut frame_len = 1usize;
    let mut result_len = 0usize;
    while frame_len != 0 {
        frame_len -= 1;
        match frames[frame_len].take()? {
            Frame::Enter(crate::ast::Type::I64) => {
                results[result_len] = "i64".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Bool) => {
                results[result_len] = "bool".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Named { name, arguments }) => {
                let declaration = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *name)?;
                if frame_len.checked_add(arguments.len())?.checked_add(1)? > frames.len() {
                    return None;
                }
                frames[frame_len] = Some(Frame::Finish(declaration, arguments.len()));
                frame_len += 1;
                for argument in arguments.iter().rev() {
                    frames[frame_len] = Some(Frame::Enter(argument));
                    frame_len += 1;
                }
            }
            Frame::Finish(declaration, argument_count) => {
                let start = result_len.checked_sub(argument_count)?;
                let encoded_arguments =
                    results[start..result_len]
                        .iter()
                        .try_fold(0usize, |bytes, key_len| {
                            bytes
                                .checked_add(decimal_digits(*key_len))?
                                .checked_add(1)?
                                .checked_add(*key_len)
                        })?;
                result_len = start;
                let declaration_len = declaration.stable_id.len();
                let key_len = "nominal:"
                    .len()
                    .checked_add(decimal_digits(declaration_len))?
                    .checked_add(1)?
                    .checked_add(declaration_len)?
                    .checked_add(1)?
                    .checked_add(decimal_digits(argument_count))?
                    .checked_add(1)?
                    .checked_add(encoded_arguments)?;
                results[result_len] = key_len;
                result_len = result_len.checked_add(1)?;
            }
        }
    }
    (result_len == 1).then_some(results[0])
}

fn function_instance_identity_len(
    program: &Program,
    function: &crate::ast::Function,
    type_arguments: &[crate::ast::Type],
) -> Option<usize> {
    if type_arguments.len() != function.type_parameters.len() {
        return None;
    }
    let encoded_arguments = type_arguments.iter().try_fold(0usize, |bytes, ty| {
        let key_len = ast_type_identity_key_len(program, ty)?;
        bytes
            .checked_add(decimal_digits(key_len))?
            .checked_add(1)?
            .checked_add(key_len)
    })?;
    "semaprax.function-instance.v1:"
        .len()
        .checked_add(decimal_digits(function.stable_id.len()))?
        .checked_add(1)?
        .checked_add(function.stable_id.len())?
        .checked_add(1)?
        .checked_add(decimal_digits(type_arguments.len()))?
        .checked_add(1)?
        .checked_add(encoded_arguments)
}

fn generic_function_instance_identity_upper(
    program: &Program,
    function: &crate::ast::Function,
) -> Option<usize> {
    if function.type_parameters.is_empty() {
        return Some(0);
    }
    let mut maximum = 0usize;
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for caller in program
        .functions
        .iter()
        .filter(|caller| caller.type_parameters.is_empty())
    {
        for root in caller
            .requires
            .iter()
            .chain(std::iter::once(&caller.body))
            .chain(&caller.ensures)
        {
            let mut len = 1usize;
            traversal[0] = Some((root, 0usize, 0usize));
            while len != 0 {
                len -= 1;
                let (expression, next_child, _) = traversal[len].take()?;
                if next_child == 0 {
                    if let crate::ast::ExprKind::Call {
                        name,
                        type_arguments,
                        ..
                    } = &expression.kind
                    {
                        if *name == function.name {
                            if let Some(identity_len) =
                                function_instance_identity_len(program, function, type_arguments)
                            {
                                maximum = maximum.max(identity_len);
                            }
                        }
                    }
                }
                if let Some(child) = ast_child(expression, next_child) {
                    if len + 2 > traversal.len() {
                        return None;
                    }
                    traversal[len] = Some((expression, next_child + 1, 0));
                    traversal[len + 1] = Some((child, 0, 0));
                    len += 2;
                }
            }
        }
    }
    Some(maximum)
}

fn scoped_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    kind_len: usize,
    path_len: usize,
) -> Option<usize> {
    let monomorphic = "declaration:"
        .len()
        .checked_add(decimal_digits(function.stable_id.len()))?
        .checked_add(1)?
        .checked_add(function.stable_id.len())?
        .checked_add(1)?
        .checked_add(kind_len)?
        .checked_add(1)?
        .checked_add(decimal_digits(path_len))?
        .checked_add(1)?
        .checked_add(path_len)?;
    if function.type_parameters.is_empty() {
        return Some(monomorphic);
    }
    if generic_instance_identity_len == 0 {
        return Some(monomorphic);
    }
    let owner_len = "semaprax.function-execution.v1:generic:"
        .len()
        .checked_add(decimal_digits(generic_instance_identity_len))?
        .checked_add(1)?
        .checked_add(generic_instance_identity_len)?;
    let generic = "function-execution:"
        .len()
        .checked_add(decimal_digits(owner_len))?
        .checked_add(1)?
        .checked_add(owner_len)?
        .checked_add(1)?
        .checked_add(kind_len)?
        .checked_add(1)?
        .checked_add(decimal_digits(path_len))?
        .checked_add(1)?
        .checked_add(path_len)?;
    Some(monomorphic.max(generic))
}

fn scoped_value_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    path_len: usize,
) -> Option<usize> {
    scoped_identity_upper(
        function,
        generic_instance_identity_len,
        "value:result".len().max("value:local".len()),
        path_len,
    )
}

fn scoped_expression_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    path_len: usize,
) -> Option<usize> {
    scoped_identity_upper(
        function,
        generic_instance_identity_len,
        "expression".len(),
        path_len,
    )
}

fn scan_ast_capacity<'a>(
    roots: impl IntoIterator<Item = &'a crate::ast::Expr>,
    program: &Program,
    count_generic_calls: bool,
    stack: &mut [Option<(&'a crate::ast::Expr, usize, usize)>; MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<AstCapacityStats, Diagnostic> {
    let mut stats = AstCapacityStats::default();
    for root in roots {
        let mut stack_len = 1;
        stack[0] = Some((root, 1, 0));
        while stack_len != 0 {
            stack_len -= 1;
            let (expression, depth, next_child) = stack[stack_len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if next_child == 0 {
                stats.nodes = stats
                    .nodes
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                stats.cumulative_depth = stats
                    .cumulative_depth
                    .checked_add(depth)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                stats.max_depth = stats.max_depth.max(depth);
                let indexed_children = match &expression.kind {
                    crate::ast::ExprKind::Call { args, .. } => args.len(),
                    crate::ast::ExprKind::Block { statements, .. } => statements.len() + 1,
                    crate::ast::ExprKind::ConstructRecord { fields, .. }
                    | crate::ast::ExprKind::ConstructVariant { fields, .. } => fields.len(),
                    crate::ast::ExprKind::Match { arms, .. } => {
                        stats.max_match_arms = stats.max_match_arms.max(arms.len());
                        arms.len() + 1
                    }
                    crate::ast::ExprKind::UpdateRecord { fields, .. } => fields.len() + 1,
                    crate::ast::ExprKind::If { .. } => 3,
                    crate::ast::ExprKind::Binary { .. } => 2,
                    crate::ast::ExprKind::Unary { .. }
                    | crate::ast::ExprKind::Try { .. }
                    | crate::ast::ExprKind::Project { .. } => 1,
                    crate::ast::ExprKind::Int(_)
                    | crate::ast::ExprKind::Bool(_)
                    | crate::ast::ExprKind::Var(_) => 0,
                };
                stats.max_indexed_children = stats.max_indexed_children.max(indexed_children);
                stats.max_index_digits = stats
                    .max_index_digits
                    .max(decimal_digits(indexed_children.saturating_sub(1)));
                if let crate::ast::ExprKind::Block { statements, .. } = &expression.kind {
                    stats.local_bindings = stats
                        .local_bindings
                        .checked_add(statements.len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stats.binding_name_bytes = statements
                        .iter()
                        .try_fold(stats.binding_name_bytes, |bytes, statement| {
                            let crate::ast::Statement::Let { name, .. } = statement;
                            bytes.checked_add(name.len())
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stats.binding_depth_sum = stats
                        .binding_depth_sum
                        .checked_add(
                            depth
                                .checked_mul(statements.len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                stats.depth_width_product_sum = stats
                    .depth_width_product_sum
                    .checked_add(
                        depth
                            .checked_mul(indexed_children)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if let crate::ast::ExprKind::Match { arms, .. } = &expression.kind {
                    for arm in arms {
                        let (bindings, names) = ast_pattern_binding_stats(&arm.pattern)?;
                        stats.pattern_bindings = stats
                            .pattern_bindings
                            .checked_add(bindings)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        stats.binding_name_bytes = stats
                            .binding_name_bytes
                            .checked_add(names)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        stats.binding_depth_sum = stats
                            .binding_depth_sum
                            .checked_add(
                                depth
                                    .checked_mul(bindings)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        stats.max_index_digits = stats
                            .max_index_digits
                            .max(ast_pattern_index_digits(&arm.pattern)?);
                    }
                    stats.depth_arm_product_sum = stats
                        .depth_arm_product_sum
                        .checked_add(
                            depth
                                .checked_mul(arms.len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                if let crate::ast::ExprKind::Call {
                    name,
                    type_arguments,
                    ..
                } = &expression.kind
                {
                    if count_generic_calls
                        && !type_arguments.is_empty()
                        && program.functions.iter().any(|function| {
                            !function.type_parameters.is_empty() && function.name == *name
                        })
                    {
                        stats.generic_calls = stats
                            .generic_calls
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
            }
            if let Some(child) = ast_child(expression, next_child) {
                if stack_len + 2 > stack.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                stack[stack_len] = Some((expression, depth, next_child + 1));
                stack[stack_len + 1] = Some((child, depth + 1, 0));
                stack_len += 2;
            }
        }
    }
    Ok(stats)
}

fn ast_pattern_index_digits(pattern: &crate::ast::MatchPattern) -> Result<usize, Diagnostic> {
    let crate::ast::MatchPattern::Record { fields, .. } = pattern else {
        return Ok(match pattern {
            crate::ast::MatchPattern::Variant { fields, .. } => {
                decimal_digits(fields.len().saturating_sub(1))
            }
            _ => 1,
        });
    };
    let mut pending: [Option<(&[crate::ast::RecordMatchPatternField], usize)>; MAX_FORMAT_NESTING] =
        [None; MAX_FORMAT_NESTING];
    pending[0] = Some((fields, 0));
    let mut len = 1;
    let mut digits = 1;
    while len != 0 {
        len -= 1;
        let (fields, next) = pending[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        digits = digits.max(decimal_digits(fields.len().saturating_sub(1)));
        let Some(field) = fields.get(next) else {
            continue;
        };
        pending[len] = Some((fields, next + 1));
        len += 1;
        if let crate::ast::RecordMatchFieldPattern::Record { fields, .. } = &field.pattern {
            if len == pending.len() {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            pending[len] = Some((fields, 0));
            len += 1;
        }
    }
    Ok(digits)
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn ast_pattern_binding_stats(
    pattern: &crate::ast::MatchPattern,
) -> Result<(usize, usize), Diagnostic> {
    match pattern {
        crate::ast::MatchPattern::Wildcard { .. } => Ok((0, 0)),
        crate::ast::MatchPattern::Variant { fields, .. } => Ok((
            fields.len(),
            fields.iter().map(|field| field.binding.len()).sum(),
        )),
        crate::ast::MatchPattern::Record { fields, .. } => {
            let mut pending: [Option<(&[crate::ast::RecordMatchPatternField], usize)>;
                MAX_FORMAT_NESTING] = [None; MAX_FORMAT_NESTING];
            pending[0] = Some((fields, 0));
            let mut len = 1;
            let mut count = 0usize;
            let mut names = 0usize;
            while len != 0 {
                len -= 1;
                let (fields, next) = pending[len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let Some(field) = fields.get(next) else {
                    continue;
                };
                pending[len] = Some((fields, next + 1));
                len += 1;
                match &field.pattern {
                    crate::ast::RecordMatchFieldPattern::Binding { .. } => {
                        count = count
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        if let crate::ast::RecordMatchFieldPattern::Binding { name, .. } =
                            &field.pattern
                        {
                            names = names
                                .checked_add(name.len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                    }
                    crate::ast::RecordMatchFieldPattern::Record { fields, .. } => {
                        if len == pending.len() {
                            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                        }
                        pending[len] = Some((fields, 0));
                        len += 1;
                    }
                    crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {}
                }
            }
            Ok((count, names))
        }
    }
}

fn declaration_field_type(
    declaration: &crate::ast::TypeDeclaration,
    mut index: usize,
) -> Option<&crate::ast::Type> {
    match &declaration.kind {
        crate::ast::TypeDeclarationKind::Resource { .. } => None,
        crate::ast::TypeDeclarationKind::Record { fields } => {
            fields.get(index).map(|field| &field.ty)
        }
        crate::ast::TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                if index < case.fields.len() {
                    return Some(&case.fields[index].ty);
                }
                index -= case.fields.len();
            }
            None
        }
    }
}

fn declaration_field_identity_bytes(
    declaration: &crate::ast::TypeDeclaration,
    mut index: usize,
) -> Option<usize> {
    match &declaration.kind {
        crate::ast::TypeDeclarationKind::Resource { .. } => None,
        crate::ast::TypeDeclarationKind::Record { fields } => {
            fields.get(index).map(|field| field.stable_id.len())
        }
        crate::ast::TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                if index < case.fields.len() {
                    return case
                        .stable_id
                        .len()
                        .checked_add(case.fields[index].stable_id.len());
                }
                index -= case.fields.len();
            }
            None
        }
    }
}

fn ast_resource_leaf_count(
    root: &crate::ast::Type,
    program: &Program,
) -> Result<usize, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a crate::ast::Type, usize),
        Children(&'a crate::ast::TypeDeclaration, usize, usize, usize),
        Add(&'a crate::ast::TypeDeclaration, usize, usize, usize),
    }
    let mut frames: [Option<Frame<'_>>; MAX_FORMAT_NESTING] = std::array::from_fn(|_| None);
    let mut ancestors: [Option<&str>; MAX_FORMAT_NESTING] = [None; MAX_FORMAT_NESTING];
    let mut values = [0usize; MAX_FORMAT_NESTING];
    frames[0] = Some(Frame::Enter(root, 0));
    let (mut frame_len, mut value_len) = (1usize, 0usize);
    while frame_len != 0 {
        frame_len -= 1;
        match frames[frame_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        {
            Frame::Enter(crate::ast::Type::I64 | crate::ast::Type::Bool, _) => {
                values[value_len] = 0;
                value_len += 1;
            }
            Frame::Enter(crate::ast::Type::Named { name, .. }, depth) => {
                let Some(declaration) = program.types.iter().find(|value| value.name == *name)
                else {
                    values[value_len] = 0;
                    value_len += 1;
                    continue;
                };
                if ancestors[..depth].contains(&Some(name.as_str())) {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                ancestors[depth] = Some(name);
                if matches!(
                    declaration.kind,
                    crate::ast::TypeDeclarationKind::Resource { .. }
                ) {
                    values[value_len] = 1;
                    value_len += 1;
                    ancestors[depth] = None;
                } else {
                    frames[frame_len] = Some(Frame::Children(declaration, 0, 0, depth));
                    frame_len += 1;
                }
            }
            Frame::Children(declaration, index, total, depth) => {
                if let Some(child) = declaration_field_type(declaration, index) {
                    if frame_len + 2 > frames.len() || depth + 1 >= ancestors.len() {
                        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                    }
                    frames[frame_len] = Some(Frame::Add(declaration, index + 1, total, depth));
                    frames[frame_len + 1] = Some(Frame::Enter(child, depth + 1));
                    frame_len += 2;
                } else {
                    ancestors[depth] = None;
                    values[value_len] = total;
                    value_len += 1;
                }
            }
            Frame::Add(declaration, index, total, depth) => {
                value_len = value_len
                    .checked_sub(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let total = total
                    .checked_add(values[value_len])
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if total > MAX_BUILDER_BYTES {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                frames[frame_len] = Some(Frame::Children(declaration, index, total, depth));
                frame_len += 1;
            }
        }
    }
    (value_len == 1)
        .then_some(values[0])
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn maximum_resource_leaf_count(program: &Program) -> Result<usize, Diagnostic> {
    let mut maximum = 1usize;
    for declaration in &program.types {
        let leaves = match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { .. } => 1,
            crate::ast::TypeDeclarationKind::Record { fields } => {
                fields
                    .iter()
                    .try_fold(0usize, |total, field| -> Result<usize, Diagnostic> {
                        total
                            .checked_add(ast_resource_leaf_count(&field.ty, program)?)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                    })?
            }
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                cases
                    .iter()
                    .try_fold(0usize, |total, case| -> Result<usize, Diagnostic> {
                        case.fields.iter().try_fold(
                            total,
                            |total, field| -> Result<usize, Diagnostic> {
                                total
                                    .checked_add(ast_resource_leaf_count(&field.ty, program)?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                            },
                        )
                    })?
            }
        };
        maximum = maximum.max(leaves);
    }
    Ok(maximum)
}

#[derive(Clone, Copy, Debug)]
struct DeclarationDagExpansion {
    maximum_resource_leaves: usize,
    maximum_type_occurrences: usize,
    maximum_shape_fields: usize,
    maximum_projection_segments: usize,
    maximum_shape_identity_bytes: usize,
    maximum_lifecycle_identity_bytes: usize,
    maximum_projection_identity_bytes: usize,
    cleanup_retained: CleanupRetainedStats,
}

#[derive(Clone, Copy, Default)]
struct CleanupTypeFacts {
    leaves: usize,
    occurrences: usize,
    shape_fields: usize,
    projection_segments: usize,
    shape_ids: usize,
    lifecycle_ids: usize,
    projection_ids: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct CleanupRetainedStats {
    roots: usize,
    occurrences: usize,
    shape_fields: usize,
    leaves: usize,
    projection_segments: usize,
    shape_ids: usize,
    lifecycle_ids: usize,
    projection_ids: usize,
    finalizer_copies: usize,
    finalizer_projection_segments: usize,
    finalizer_lifecycle_ids: usize,
    finalizer_projection_ids: usize,
    place_copies: usize,
    place_projection_segments: usize,
    place_projection_ids: usize,
    call_arguments: usize,
    call_argument_owned_bytes: usize,
    parent_local_epochs: usize,
    parent_local_zero_lifetime_transfers: usize,
    parent_local_partial_fields: usize,
    parent_local_finalizer_copies: usize,
    parent_local_finalizer_projection_segments: usize,
    parent_local_finalizer_lifecycle_ids: usize,
    parent_local_finalizer_projection_ids: usize,
    parent_local_finalizer_storage_bytes: usize,
    parent_local_projection_epochs: usize,
    parent_local_projection_exit_groups: usize,
    parent_local_projection_finalizer_copies: usize,
    parent_local_projection_finalizer_projection_segments: usize,
    parent_local_projection_finalizer_lifecycle_ids: usize,
    parent_local_projection_finalizer_projection_ids: usize,
    parent_local_projection_finalizer_storage_bytes: usize,
    parent_local_update_prefix_fields: usize,
    parent_local_update_prefix_exit_groups: usize,
    parent_local_update_prefix_finalizer_copies: usize,
    parent_local_update_prefix_finalizer_projection_segments: usize,
    parent_local_update_prefix_finalizer_lifecycle_ids: usize,
    parent_local_update_prefix_finalizer_projection_ids: usize,
    parent_local_update_prefix_finalizer_storage_bytes: usize,
    ordinary_slot_payload_bytes: usize,
    ordinary_place_storage_bytes: usize,
    ordinary_finalizer_storage_bytes: usize,
    staged_results: usize,
    variant_edges: usize,
    stage_identity_and_type_bytes: usize,
    variant_identity_bytes: usize,
    fallback_roots: usize,
    exit_events: usize,
}

impl CleanupRetainedStats {
    fn add_root(&mut self, facts: CleanupTypeFacts) -> Option<()> {
        if facts.leaves == 0 {
            return Some(());
        }
        self.roots = self.roots.checked_add(1)?;
        self.occurrences = self.occurrences.checked_add(facts.occurrences)?;
        self.shape_fields = self.shape_fields.checked_add(facts.shape_fields)?;
        self.leaves = self.leaves.checked_add(facts.leaves)?;
        self.projection_segments = self
            .projection_segments
            .checked_add(facts.projection_segments)?;
        self.shape_ids = self.shape_ids.checked_add(facts.shape_ids)?;
        self.lifecycle_ids = self.lifecycle_ids.checked_add(facts.lifecycle_ids)?;
        self.projection_ids = self.projection_ids.checked_add(facts.projection_ids)?;
        Some(())
    }

    fn merge(&mut self, other: Self) -> Option<()> {
        self.roots = self.roots.checked_add(other.roots)?;
        self.occurrences = self.occurrences.checked_add(other.occurrences)?;
        self.shape_fields = self.shape_fields.checked_add(other.shape_fields)?;
        self.leaves = self.leaves.checked_add(other.leaves)?;
        self.projection_segments = self
            .projection_segments
            .checked_add(other.projection_segments)?;
        self.shape_ids = self.shape_ids.checked_add(other.shape_ids)?;
        self.lifecycle_ids = self.lifecycle_ids.checked_add(other.lifecycle_ids)?;
        self.projection_ids = self.projection_ids.checked_add(other.projection_ids)?;
        self.finalizer_copies = self.finalizer_copies.checked_add(other.finalizer_copies)?;
        self.finalizer_projection_segments = self
            .finalizer_projection_segments
            .checked_add(other.finalizer_projection_segments)?;
        self.finalizer_lifecycle_ids = self
            .finalizer_lifecycle_ids
            .checked_add(other.finalizer_lifecycle_ids)?;
        self.finalizer_projection_ids = self
            .finalizer_projection_ids
            .checked_add(other.finalizer_projection_ids)?;
        self.place_copies = self.place_copies.checked_add(other.place_copies)?;
        self.place_projection_segments = self
            .place_projection_segments
            .checked_add(other.place_projection_segments)?;
        self.place_projection_ids = self
            .place_projection_ids
            .checked_add(other.place_projection_ids)?;
        self.call_arguments = self.call_arguments.checked_add(other.call_arguments)?;
        self.call_argument_owned_bytes = self
            .call_argument_owned_bytes
            .checked_add(other.call_argument_owned_bytes)?;
        self.parent_local_epochs = self
            .parent_local_epochs
            .checked_add(other.parent_local_epochs)?;
        self.parent_local_zero_lifetime_transfers = self
            .parent_local_zero_lifetime_transfers
            .checked_add(other.parent_local_zero_lifetime_transfers)?;
        self.parent_local_partial_fields = self
            .parent_local_partial_fields
            .checked_add(other.parent_local_partial_fields)?;
        self.parent_local_finalizer_copies = self
            .parent_local_finalizer_copies
            .checked_add(other.parent_local_finalizer_copies)?;
        self.parent_local_finalizer_projection_segments = self
            .parent_local_finalizer_projection_segments
            .checked_add(other.parent_local_finalizer_projection_segments)?;
        self.parent_local_finalizer_lifecycle_ids = self
            .parent_local_finalizer_lifecycle_ids
            .checked_add(other.parent_local_finalizer_lifecycle_ids)?;
        self.parent_local_finalizer_projection_ids = self
            .parent_local_finalizer_projection_ids
            .checked_add(other.parent_local_finalizer_projection_ids)?;
        self.parent_local_finalizer_storage_bytes = self
            .parent_local_finalizer_storage_bytes
            .checked_add(other.parent_local_finalizer_storage_bytes)?;
        self.parent_local_projection_epochs = self
            .parent_local_projection_epochs
            .checked_add(other.parent_local_projection_epochs)?;
        self.parent_local_projection_exit_groups = self
            .parent_local_projection_exit_groups
            .checked_add(other.parent_local_projection_exit_groups)?;
        self.parent_local_projection_finalizer_copies = self
            .parent_local_projection_finalizer_copies
            .checked_add(other.parent_local_projection_finalizer_copies)?;
        self.parent_local_projection_finalizer_projection_segments = self
            .parent_local_projection_finalizer_projection_segments
            .checked_add(other.parent_local_projection_finalizer_projection_segments)?;
        self.parent_local_projection_finalizer_lifecycle_ids = self
            .parent_local_projection_finalizer_lifecycle_ids
            .checked_add(other.parent_local_projection_finalizer_lifecycle_ids)?;
        self.parent_local_projection_finalizer_projection_ids = self
            .parent_local_projection_finalizer_projection_ids
            .checked_add(other.parent_local_projection_finalizer_projection_ids)?;
        self.parent_local_projection_finalizer_storage_bytes = self
            .parent_local_projection_finalizer_storage_bytes
            .checked_add(other.parent_local_projection_finalizer_storage_bytes)?;
        self.parent_local_update_prefix_fields = self
            .parent_local_update_prefix_fields
            .checked_add(other.parent_local_update_prefix_fields)?;
        self.parent_local_update_prefix_exit_groups =
            self.parent_local_update_prefix_exit_groups
                .checked_add(other.parent_local_update_prefix_exit_groups)?;
        self.parent_local_update_prefix_finalizer_copies = self
            .parent_local_update_prefix_finalizer_copies
            .checked_add(other.parent_local_update_prefix_finalizer_copies)?;
        self.parent_local_update_prefix_finalizer_projection_segments = self
            .parent_local_update_prefix_finalizer_projection_segments
            .checked_add(other.parent_local_update_prefix_finalizer_projection_segments)?;
        self.parent_local_update_prefix_finalizer_lifecycle_ids = self
            .parent_local_update_prefix_finalizer_lifecycle_ids
            .checked_add(other.parent_local_update_prefix_finalizer_lifecycle_ids)?;
        self.parent_local_update_prefix_finalizer_projection_ids = self
            .parent_local_update_prefix_finalizer_projection_ids
            .checked_add(other.parent_local_update_prefix_finalizer_projection_ids)?;
        self.parent_local_update_prefix_finalizer_storage_bytes = self
            .parent_local_update_prefix_finalizer_storage_bytes
            .checked_add(other.parent_local_update_prefix_finalizer_storage_bytes)?;
        self.ordinary_slot_payload_bytes = self
            .ordinary_slot_payload_bytes
            .checked_add(other.ordinary_slot_payload_bytes)?;
        self.ordinary_place_storage_bytes = self
            .ordinary_place_storage_bytes
            .checked_add(other.ordinary_place_storage_bytes)?;
        self.ordinary_finalizer_storage_bytes = self
            .ordinary_finalizer_storage_bytes
            .checked_add(other.ordinary_finalizer_storage_bytes)?;
        self.staged_results = self.staged_results.checked_add(other.staged_results)?;
        self.variant_edges = self.variant_edges.checked_add(other.variant_edges)?;
        self.stage_identity_and_type_bytes = self
            .stage_identity_and_type_bytes
            .checked_add(other.stage_identity_and_type_bytes)?;
        self.variant_identity_bytes = self
            .variant_identity_bytes
            .checked_add(other.variant_identity_bytes)?;
        self.fallback_roots = self.fallback_roots.checked_add(other.fallback_roots)?;
        self.exit_events = self.exit_events.checked_add(other.exit_events)?;
        Some(())
    }

    fn scaled(self, multiplier: usize) -> Option<Self> {
        Some(Self {
            roots: self.roots.checked_mul(multiplier)?,
            occurrences: self.occurrences.checked_mul(multiplier)?,
            shape_fields: self.shape_fields.checked_mul(multiplier)?,
            leaves: self.leaves.checked_mul(multiplier)?,
            projection_segments: self.projection_segments.checked_mul(multiplier)?,
            shape_ids: self.shape_ids.checked_mul(multiplier)?,
            lifecycle_ids: self.lifecycle_ids.checked_mul(multiplier)?,
            projection_ids: self.projection_ids.checked_mul(multiplier)?,
            finalizer_copies: self.finalizer_copies.checked_mul(multiplier)?,
            finalizer_projection_segments: self
                .finalizer_projection_segments
                .checked_mul(multiplier)?,
            finalizer_lifecycle_ids: self.finalizer_lifecycle_ids.checked_mul(multiplier)?,
            finalizer_projection_ids: self.finalizer_projection_ids.checked_mul(multiplier)?,
            place_copies: self.place_copies.checked_mul(multiplier)?,
            place_projection_segments: self.place_projection_segments.checked_mul(multiplier)?,
            place_projection_ids: self.place_projection_ids.checked_mul(multiplier)?,
            call_arguments: self.call_arguments.checked_mul(multiplier)?,
            call_argument_owned_bytes: self.call_argument_owned_bytes.checked_mul(multiplier)?,
            parent_local_epochs: self.parent_local_epochs.checked_mul(multiplier)?,
            parent_local_zero_lifetime_transfers: self
                .parent_local_zero_lifetime_transfers
                .checked_mul(multiplier)?,
            parent_local_partial_fields: self
                .parent_local_partial_fields
                .checked_mul(multiplier)?,
            parent_local_finalizer_copies: self
                .parent_local_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_finalizer_projection_segments: self
                .parent_local_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_finalizer_lifecycle_ids: self
                .parent_local_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_finalizer_projection_ids: self
                .parent_local_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_finalizer_storage_bytes: self
                .parent_local_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            parent_local_projection_epochs: self
                .parent_local_projection_epochs
                .checked_mul(multiplier)?,
            parent_local_projection_exit_groups: self
                .parent_local_projection_exit_groups
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_copies: self
                .parent_local_projection_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_projection_segments: self
                .parent_local_projection_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_lifecycle_ids: self
                .parent_local_projection_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_projection_ids: self
                .parent_local_projection_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_storage_bytes: self
                .parent_local_projection_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            parent_local_update_prefix_fields: self
                .parent_local_update_prefix_fields
                .checked_mul(multiplier)?,
            parent_local_update_prefix_exit_groups: self
                .parent_local_update_prefix_exit_groups
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_copies: self
                .parent_local_update_prefix_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_projection_segments: self
                .parent_local_update_prefix_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_lifecycle_ids: self
                .parent_local_update_prefix_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_projection_ids: self
                .parent_local_update_prefix_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_storage_bytes: self
                .parent_local_update_prefix_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            ordinary_slot_payload_bytes: self
                .ordinary_slot_payload_bytes
                .checked_mul(multiplier)?,
            ordinary_place_storage_bytes: self
                .ordinary_place_storage_bytes
                .checked_mul(multiplier)?,
            ordinary_finalizer_storage_bytes: self
                .ordinary_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            staged_results: self.staged_results.checked_mul(multiplier)?,
            variant_edges: self.variant_edges.checked_mul(multiplier)?,
            stage_identity_and_type_bytes: self
                .stage_identity_and_type_bytes
                .checked_mul(multiplier)?,
            variant_identity_bytes: self.variant_identity_bytes.checked_mul(multiplier)?,
            fallback_roots: self.fallback_roots.checked_mul(multiplier)?,
            exit_events: self.exit_events.checked_mul(multiplier)?,
        })
    }
}

fn retained_vec_capacity_extra(logical_entries: usize, container_upper: usize) -> Option<usize> {
    if logical_entries == 0 {
        return Some(0);
    }
    let nonempty_containers = container_upper.min(logical_entries);
    nonempty_containers
        .checked_mul(8)
        .and_then(|capacity| capacity.checked_add(logical_entries.checked_mul(2)?))
        .and_then(|capacity| capacity.checked_sub(logical_entries))
}

fn declaration_dag_expansion(
    program: &Program,
    generic_instance_upper: usize,
) -> Result<DeclarationDagExpansion, Diagnostic> {
    fn add_child(
        parent: &mut CleanupTypeFacts,
        child: CleanupTypeFacts,
        edge_ids: usize,
    ) -> Option<()> {
        parent.leaves = parent.leaves.checked_add(child.leaves)?;
        parent.occurrences = parent.occurrences.checked_add(child.occurrences)?;
        parent.shape_fields = parent
            .shape_fields
            .checked_add(1)?
            .checked_add(child.shape_fields)?;
        parent.projection_segments = parent
            .projection_segments
            .checked_add(child.projection_segments)?
            .checked_add(child.leaves)?;
        parent.shape_ids = parent
            .shape_ids
            .checked_add(edge_ids)?
            .checked_add(child.shape_ids)?;
        parent.lifecycle_ids = parent.lifecycle_ids.checked_add(child.lifecycle_ids)?;
        parent.projection_ids = parent
            .projection_ids
            .checked_add(child.projection_ids)?
            .checked_add(child.leaves.checked_mul(edge_ids)?)?;
        Some(())
    }

    let mut cleanup_node_count = 0usize;
    let mut cleanup_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for function in &program.functions {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut len = 1usize;
            cleanup_scan[0] = Some((root, 0usize, 0usize));
            while len != 0 {
                len -= 1;
                let (expression, next_child, _) = cleanup_scan[len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child == 0 {
                    cleanup_node_count = cleanup_node_count
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                if let Some(child) = ast_child(expression, next_child) {
                    if len + 2 > cleanup_scan.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    cleanup_scan[len] = Some((expression, next_child + 1, 0));
                    cleanup_scan[len + 1] = Some((child, 0, 0));
                    len += 2;
                }
            }
        }
    }
    let cleanup_node_capacity = cleanup_node_count.max(1);
    let count = program.types.len().max(1);
    let table_bytes = count
        .checked_mul(
            std::mem::size_of::<u8>()
                + std::mem::size_of::<CleanupTypeFacts>()
                + std::mem::size_of::<Option<(usize, usize, CleanupTypeFacts)>>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup_node_capacity.checked_mul(std::mem::size_of::<CleanupTypeKey>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let _table_budget = reserve_temporary_exact(table_bytes)?;
    let mut state = Vec::with_capacity(count);
    let mut facts = Vec::with_capacity(count);
    let mut stack: Vec<Option<(usize, usize, CleanupTypeFacts)>> = Vec::with_capacity(count);
    state.resize(count, 0u8);
    facts.resize(count, CleanupTypeFacts::default());
    stack.resize(count, None);
    if state.capacity() != count || facts.capacity() != count || stack.capacity() != count {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let mut maximum_resource_leaves = 0usize;
    let mut maximum_type_occurrences = 1usize;
    let mut maximum_shape_fields = 0usize;
    let mut maximum_projection_segments = 0usize;
    let mut maximum_shape_identity_bytes = 0usize;
    let mut maximum_lifecycle_identity_bytes = 0usize;
    let mut maximum_projection_identity_bytes = 0usize;
    for root in 0..program.types.len() {
        if state[root] == 2 {
            continue;
        }
        stack[0] = Some((
            root,
            0,
            CleanupTypeFacts {
                occurrences: 1,
                shape_ids: program.types[root].stable_id.len(),
                ..CleanupTypeFacts::default()
            },
        ));
        state[root] = 1;
        let mut len = 1usize;
        while len != 0 {
            len -= 1;
            let (index, next, total) = stack[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let declaration = &program.types[index];
            if matches!(
                declaration.kind,
                crate::ast::TypeDeclarationKind::Resource { .. }
            ) {
                let lifecycle_bytes = match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Resource { lifecycles } => lifecycles
                        .iter()
                        .filter_map(|lifecycle| lifecycle.stable_id.as_deref())
                        .try_fold(0usize, |bytes, id| bytes.checked_add(id.len()))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    _ => unreachable!(),
                };
                facts[index] = CleanupTypeFacts {
                    leaves: 1,
                    occurrences: 1,
                    shape_ids: lifecycle_bytes,
                    lifecycle_ids: lifecycle_bytes,
                    projection_ids: 0,
                    ..CleanupTypeFacts::default()
                };
                maximum_resource_leaves = maximum_resource_leaves.max(1);
                maximum_type_occurrences = maximum_type_occurrences.max(1);
                maximum_shape_identity_bytes = maximum_shape_identity_bytes.max(lifecycle_bytes);
                maximum_lifecycle_identity_bytes =
                    maximum_lifecycle_identity_bytes.max(lifecycle_bytes);
                state[index] = 2;
                if let Some(parent) = len.checked_sub(1).and_then(|parent| stack[parent].as_mut()) {
                    let parent_decl = &program.types[parent.0];
                    let edge = declaration_field_identity_bytes(parent_decl, parent.1 - 1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, facts[index], edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                continue;
            }
            let Some(child) = declaration_field_type(declaration, next) else {
                facts[index] = total;
                state[index] = 2;
                maximum_resource_leaves = maximum_resource_leaves.max(total.leaves);
                maximum_type_occurrences = maximum_type_occurrences.max(total.occurrences);
                maximum_shape_fields = maximum_shape_fields.max(total.shape_fields);
                maximum_projection_segments =
                    maximum_projection_segments.max(total.projection_segments);
                maximum_shape_identity_bytes = maximum_shape_identity_bytes.max(total.shape_ids);
                maximum_lifecycle_identity_bytes =
                    maximum_lifecycle_identity_bytes.max(total.lifecycle_ids);
                maximum_projection_identity_bytes =
                    maximum_projection_identity_bytes.max(total.projection_ids);
                if let Some(parent) = len.checked_sub(1).and_then(|parent| stack[parent].as_mut()) {
                    let parent_decl = &program.types[parent.0];
                    let edge = declaration_field_identity_bytes(parent_decl, parent.1 - 1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, total, edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                continue;
            };
            stack[len] = Some((index, next + 1, total));
            len += 1;
            let crate::ast::Type::Named { name, .. } = child else {
                let parent = stack[len - 1].as_mut().expect("parent retained");
                parent.2.occurrences = parent
                    .2
                    .occurrences
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_fields = parent
                    .2
                    .shape_fields
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_ids = parent
                    .2
                    .shape_ids
                    .checked_add(
                        declaration_field_identity_bytes(declaration, next)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                continue;
            };
            let Some(child_index) = program.types.iter().position(|value| value.name == *name)
            else {
                let parent = stack[len - 1].as_mut().expect("parent retained");
                parent.2.occurrences = parent
                    .2
                    .occurrences
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_fields = parent
                    .2
                    .shape_fields
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_ids = parent
                    .2
                    .shape_ids
                    .checked_add(
                        declaration_field_identity_bytes(declaration, next)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                continue;
            };
            match state[child_index] {
                2 => {
                    let parent = stack[len - 1].as_mut().expect("parent retained");
                    let edge = declaration_field_identity_bytes(declaration, next)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, facts[child_index], edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                1 => return Err(b107("selected identity missing")),
                _ => {
                    if len == stack.len() {
                        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                    }
                    state[child_index] = 1;
                    stack[len] = Some((
                        child_index,
                        0,
                        CleanupTypeFacts {
                            occurrences: 1,
                            shape_ids: program.types[child_index].stable_id.len(),
                            ..CleanupTypeFacts::default()
                        },
                    ));
                    len += 1;
                }
            }
        }
    }
    let cleanup_retained = cleanup_retained_stats(
        program,
        &facts,
        cleanup_node_capacity,
        generic_instance_upper,
    )?;
    Ok(DeclarationDagExpansion {
        maximum_resource_leaves,
        maximum_type_occurrences,
        maximum_shape_fields,
        maximum_projection_segments,
        maximum_shape_identity_bytes,
        maximum_lifecycle_identity_bytes,
        maximum_projection_identity_bytes,
        cleanup_retained,
    })
}

#[derive(Clone, Copy)]
enum CleanupTypeKey {
    Scalar,
    Declaration(usize),
    Unknown,
}

fn cleanup_source_exit_events(expression: &crate::ast::Expr) -> usize {
    match &expression.kind {
        crate::ast::ExprKind::Call { .. }
        | crate::ast::ExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            ..
        }
        | crate::ast::ExprKind::Binary {
            op:
                crate::ast::BinaryOp::Add
                | crate::ast::BinaryOp::Sub
                | crate::ast::BinaryOp::Mul
                | crate::ast::BinaryOp::Div
                | crate::ast::BinaryOp::Rem,
            ..
        }
        | crate::ast::ExprKind::Block { .. }
        | crate::ast::ExprKind::Try { .. }
        | crate::ast::ExprKind::UpdateRecord { .. } => 1,
        // If, lazy boolean, and Match are lowered in their active region.
        // Their authored Block children, when present, own the corresponding
        // lexical scope exits and are counted independently above.
        _ => 0,
    }
}

fn cleanup_source_failure_events(expression: &crate::ast::Expr) -> usize {
    match &expression.kind {
        crate::ast::ExprKind::Call { .. }
        | crate::ast::ExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            ..
        }
        | crate::ast::ExprKind::Binary {
            op:
                crate::ast::BinaryOp::Add
                | crate::ast::BinaryOp::Sub
                | crate::ast::BinaryOp::Mul
                | crate::ast::BinaryOp::Div
                | crate::ast::BinaryOp::Rem,
            ..
        }
        | crate::ast::ExprKind::Try { .. } => 1,
        _ => 0,
    }
}

fn cleanup_function_exit_events<'a>(
    function: &'a crate::ast::Function,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = function
        .requires
        .len()
        .checked_add(function.ensures.len())
        .and_then(|contracts| contracts.checked_mul(2))
        .and_then(|events| events.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        let mut len = 1usize;
        traversal[0] = Some((root, 0, 0));
        while len != 0 {
            len -= 1;
            let (expression, next_child, _) = traversal[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if next_child == 0 {
                // lower_root_body reuses the function's root region instead
                // of creating an authored Block region for the outer body.
                if !std::ptr::eq(expression, &function.body) {
                    events = events
                        .checked_add(cleanup_source_exit_events(expression))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            if let Some(child) = ast_child(expression, next_child) {
                if len + 2 > traversal.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                traversal[len] = Some((expression, next_child + 1, 0));
                traversal[len + 1] = Some((child, 0, 0));
                len += 2;
            }
        }
    }
    Ok(events)
}

fn cleanup_expression_exit_events<'a>(
    root: &'a crate::ast::Expr,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0 {
            events = events
                .checked_add(cleanup_source_exit_events(expression))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        if let Some(child) = ast_child(expression, next_child) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, next_child + 1, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_expression_failure_events<'a>(
    root: &'a crate::ast::Expr,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0 {
            events = events
                .checked_add(cleanup_source_failure_events(expression))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        if let Some(child) = ast_child(expression, next_child) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, next_child + 1, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_expression_call_events<'a>(
    root: &'a crate::ast::Expr,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0 {
            if let crate::ast::ExprKind::Call { name, .. } = &expression.kind {
                if !program
                    .interfaces
                    .iter()
                    .any(|interface| interface.imports.iter().any(|import| import.name == *name))
                {
                    events = events
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
        }
        if let Some(child) = ast_child(expression, next_child) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, next_child + 1, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_expression_boolean_branch_events<'a>(
    root: &'a crate::ast::Expr,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0
            && matches!(
                expression.kind,
                crate::ast::ExprKind::If { .. }
                    | crate::ast::ExprKind::Binary {
                        op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                        ..
                    }
            )
        {
            events = events
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        if let Some(child) = ast_child(expression, next_child) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, next_child + 1, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_plan_variable_identity_bytes(
    function: &crate::ast::Function,
    program: &Program,
    cleanup_path_copies: usize,
) -> Result<(usize, usize), Diagnostic> {
    fn child_path_increment(
        expression: &crate::ast::Expr,
        child_index: usize,
        program: &Program,
    ) -> usize {
        match &expression.kind {
            crate::ast::ExprKind::Call { name, .. } => {
                let prefix =
                    if program.interfaces.iter().any(|interface| {
                        interface.imports.iter().any(|import| import.name == *name)
                    }) {
                        ".native-rust-arg."
                    } else {
                        ".arg."
                    };
                prefix.len() + decimal_digits(child_index)
            }
            crate::ast::ExprKind::Unary { .. } => ".value".len(),
            crate::ast::ExprKind::Binary { .. } => {
                if child_index == 0 { ".left" } else { ".right" }.len()
            }
            crate::ast::ExprKind::Block { statements, .. } => {
                if child_index < statements.len() {
                    ".s".len() + decimal_digits(child_index) + ".value".len()
                } else {
                    ".tail".len()
                }
            }
            crate::ast::ExprKind::If { .. } => [".condition", ".then", ".else"]
                .get(child_index)
                .map_or(0, |segment| segment.len()),
            crate::ast::ExprKind::ConstructRecord { .. }
            | crate::ast::ExprKind::ConstructVariant { .. } => {
                ".field.".len() + decimal_digits(child_index) + ".value".len()
            }
            crate::ast::ExprKind::Match { .. } => {
                if child_index == 0 {
                    ".scrutinee".len()
                } else {
                    ".arm.".len() + decimal_digits(child_index - 1) + ".value".len()
                }
            }
            crate::ast::ExprKind::UpdateRecord { .. } => {
                if child_index == 0 {
                    ".base".len()
                } else {
                    ".field.".len() + decimal_digits(child_index - 1) + ".value".len()
                }
            }
            crate::ast::ExprKind::Try { .. } => ".operand".len(),
            crate::ast::ExprKind::Project { .. } => ".base".len(),
            crate::ast::ExprKind::Int(_)
            | crate::ast::ExprKind::Bool(_)
            | crate::ast::ExprKind::Var(_) => 0,
        }
    }

    let generic_instance_identity_len = generic_function_instance_identity_upper(program, function)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut bytes = 0usize;
    let mut all_expression_bytes = 0usize;
    for (root_index, (root, contract)) in function
        .requires
        .iter()
        .map(|root| (root, true))
        .chain(std::iter::once((&function.body, false)))
        .chain(function.ensures.iter().map(|root| (root, true)))
        .enumerate()
    {
        let path_len = match root_index.cmp(&function.requires.len()) {
            std::cmp::Ordering::Less => "requires.".len() + decimal_digits(root_index),
            std::cmp::Ordering::Equal => "body".len(),
            std::cmp::Ordering::Greater => {
                "ensures.".len() + decimal_digits(root_index - function.requires.len() - 1)
            }
        };
        let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let mut len = 1usize;
        traversal[0] = Some((root, path_len, 0));
        while len != 0 {
            len -= 1;
            let (expression, path_len, next_child) = traversal[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if next_child == 0 {
                let mut copies = usize::from(contract && std::ptr::eq(expression, root))
                    .checked_mul(5)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                match &expression.kind {
                    crate::ast::ExprKind::Call { name, .. } => {
                        if !program.interfaces.iter().any(|interface| {
                            interface.imports.iter().any(|import| import.name == *name)
                        }) {
                            // StatusSource, two status edges, SelectFailure,
                            // ReturnFailure, and CallCommit.
                            copies = copies
                                .checked_add(6)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                    }
                    crate::ast::ExprKind::Unary {
                        op: crate::ast::UnaryOp::Neg,
                        ..
                    }
                    | crate::ast::ExprKind::Binary {
                        op:
                            crate::ast::BinaryOp::Add
                            | crate::ast::BinaryOp::Sub
                            | crate::ast::BinaryOp::Mul
                            | crate::ast::BinaryOp::Div
                            | crate::ast::BinaryOp::Rem,
                        ..
                    } => {
                        copies = copies
                            .checked_add(5)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                    crate::ast::ExprKind::If { .. }
                    | crate::ast::ExprKind::Binary {
                        op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                        ..
                    } => {
                        copies = copies
                            .checked_add(2)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                    _ => {}
                }
                if std::ptr::eq(expression, &function.body) {
                    copies = copies
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let uncovered = copies
                    .checked_sub(copies.min(cleanup_path_copies))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let identity_bytes = scoped_expression_identity_upper(
                    function,
                    generic_instance_identity_len,
                    path_len,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                all_expression_bytes = all_expression_bytes
                    .checked_add(identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                bytes = bytes
                    .checked_add(
                        uncovered
                            .checked_mul(identity_bytes)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            if let Some(child) = ast_child(expression, next_child) {
                if len + 2 > traversal.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_path_len = path_len
                    .checked_add(child_path_increment(expression, next_child, program))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                traversal[len] = Some((expression, path_len, next_child + 1));
                traversal[len + 1] = Some((child, child_path_len, 0));
                len += 2;
            }
        }
    }
    Ok((all_expression_bytes, bytes))
}

fn cleanup_function_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = function
        .requires
        .len()
        .checked_add(function.ensures.len())
        .and_then(|events| events.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        events = events
            .checked_add(cleanup_expression_failure_events(root, traversal)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(events)
}

fn cleanup_function_region_depth<'a>(
    function: &'a crate::ast::Function,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut maximum = 1usize;
    for (root, contract_region) in function
        .requires
        .iter()
        .map(|root| (root, true))
        .chain(std::iter::once((&function.body, false)))
        .chain(function.ensures.iter().map(|root| (root, true)))
    {
        let root_region = 1usize
            .checked_add(usize::from(contract_region))
            .and_then(|depth| {
                depth.checked_add(usize::from(
                    !std::ptr::eq(root, &function.body)
                        && matches!(
                            root.kind,
                            crate::ast::ExprKind::Block { .. }
                                | crate::ast::ExprKind::UpdateRecord { .. }
                        ),
                ))
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        maximum = maximum.max(root_region);
        let mut len = 1usize;
        traversal[0] = Some((root, 0, root_region));
        while len != 0 {
            len -= 1;
            let (expression, next_child, region_depth) = traversal[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if let Some(child) = ast_child(expression, next_child) {
                if len + 2 > traversal.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = region_depth
                    .checked_add(usize::from(matches!(
                        child.kind,
                        crate::ast::ExprKind::Block { .. }
                            | crate::ast::ExprKind::UpdateRecord { .. }
                    )))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                maximum = maximum.max(child_depth);
                traversal[len] = Some((expression, next_child + 1, region_depth));
                traversal[len + 1] = Some((child, 0, child_depth));
                len += 2;
            }
        }
    }
    Ok(maximum)
}

#[derive(Clone, Copy, Default)]
struct CleanupBindingFlow {
    failure_finalizers: usize,
    live_after: bool,
}

fn cleanup_binding_flow<'a>(
    root: &'a crate::ast::Expr,
    binding: &str,
    consumes_result: bool,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<CleanupBindingFlow, Diagnostic> {
    let mut consumes = [false; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut flows = [CleanupBindingFlow::default(); MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut branch_live = [false; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut stack_len = 1usize;
    traversal[0] = Some((root, 0, 0));
    consumes[0] = consumes_result;
    flows[0].live_after = true;
    let mut returned: Option<CleanupBindingFlow> = None;
    while stack_len != 0 {
        let frame_index = stack_len - 1;
        let consume = consumes[frame_index];
        let (expression, next_child, _) =
            traversal[frame_index].ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        if let Some(child) = returned.take() {
            let child_index = next_child
                .checked_sub(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let flow = &mut flows[frame_index];
            let sequence = |flow: &mut CleanupBindingFlow,
                            child: CleanupBindingFlow|
             -> Result<(), Diagnostic> {
                if flow.live_after {
                    flow.failure_finalizers = flow
                        .failure_finalizers
                        .checked_add(child.failure_finalizers)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    flow.live_after = child.live_after;
                }
                Ok(())
            };
            match &expression.kind {
                crate::ast::ExprKind::If { .. } | crate::ast::ExprKind::Match { .. }
                    if child_index != 0 =>
                {
                    if flow.live_after {
                        flow.failure_finalizers = flow
                            .failure_finalizers
                            .checked_add(child.failure_finalizers)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        branch_live[frame_index] |= child.live_after;
                    }
                }
                crate::ast::ExprKind::Binary {
                    op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                    ..
                } if child_index == 1 => {
                    if flow.live_after {
                        flow.failure_finalizers = flow
                            .failure_finalizers
                            .checked_add(child.failure_finalizers)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        // The lazy short-circuit path retains the binding even
                        // if the right operand consumes it.
                    }
                }
                _ => sequence(flow, child)?,
            }
        }

        if let Some(child) = ast_child(expression, next_child) {
            if stack_len == traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            let child_consumes = match &expression.kind {
                crate::ast::ExprKind::Call { name, .. } => program
                    .functions
                    .iter()
                    .find(|function| function.name == *name)
                    .and_then(|function| function.params.get(next_child))
                    .is_some_and(|parameter| parameter.mode == crate::ast::ParamMode::Own),
                crate::ast::ExprKind::Block { statements, .. } => {
                    next_child < statements.len() || consume
                }
                crate::ast::ExprKind::If { .. } => next_child != 0 && consume,
                crate::ast::ExprKind::ConstructRecord { .. }
                | crate::ast::ExprKind::ConstructVariant { .. }
                | crate::ast::ExprKind::UpdateRecord { .. } => true,
                crate::ast::ExprKind::Match { .. } => next_child == 0 || consume,
                crate::ast::ExprKind::Try { .. } => true,
                crate::ast::ExprKind::Project { .. }
                | crate::ast::ExprKind::Unary { .. }
                | crate::ast::ExprKind::Binary { .. } => false,
                crate::ast::ExprKind::Int(_)
                | crate::ast::ExprKind::Bool(_)
                | crate::ast::ExprKind::Var(_) => false,
            };
            traversal[frame_index] = Some((expression, next_child + 1, 0));
            traversal[stack_len] = Some((child, 0, 0));
            consumes[stack_len] = child_consumes;
            flows[stack_len] = CleanupBindingFlow {
                failure_finalizers: 0,
                live_after: true,
            };
            branch_live[stack_len] = false;
            stack_len += 1;
            continue;
        }

        let mut flow = flows[frame_index];
        match &expression.kind {
            crate::ast::ExprKind::If { .. } | crate::ast::ExprKind::Match { .. }
                if flow.live_after =>
            {
                flow.live_after = branch_live[frame_index];
            }
            _ => {}
        }
        if flow.live_after {
            flow.failure_finalizers = flow
                .failure_finalizers
                .checked_add(cleanup_source_failure_events(expression))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if consume
                && matches!(&expression.kind, crate::ast::ExprKind::Var(name) if name == binding)
            {
                flow.live_after = false;
            }
        }
        traversal[frame_index] = None;
        stack_len -= 1;
        returned = Some(flow);
    }
    returned.ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_block_binding_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    block: &'a crate::ast::Expr,
    next_child: usize,
    binding: &str,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut live = true;
    let mut child_index = next_child;
    while let Some(child) = ast_child(block, child_index) {
        if live {
            let flow = cleanup_binding_flow(child, binding, true, program, traversal)?;
            events = events
                .checked_add(flow.failure_finalizers)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            live = flow.live_after;
        }
        child_index = child_index
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    if live && std::ptr::eq(block, &function.body) {
        for ensure in &function.ensures {
            let flow = cleanup_binding_flow(ensure, binding, false, program, traversal)?;
            events = events
                .checked_add(flow.failure_finalizers)
                .and_then(|events| events.checked_add(1))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            live = flow.live_after;
            if !live {
                break;
            }
        }
    }
    events
        .checked_add(usize::from(live))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_parameter_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    binding: &str,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut live = true;
    for require in &function.requires {
        let flow = cleanup_binding_flow(require, binding, false, program, traversal)?;
        events = events
            .checked_add(flow.failure_finalizers)
            .and_then(|events| events.checked_add(usize::from(live)))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        live &= flow.live_after;
    }
    if matches!(function.body.kind, crate::ast::ExprKind::Block { .. }) {
        return events
            .checked_add(cleanup_block_binding_finalizer_events(
                function,
                &function.body,
                0,
                binding,
                program,
                traversal,
            )?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    if live {
        let flow = cleanup_binding_flow(&function.body, binding, true, program, traversal)?;
        events = events
            .checked_add(flow.failure_finalizers)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        live = flow.live_after;
    }
    for ensure in &function.ensures {
        if live {
            let flow = cleanup_binding_flow(ensure, binding, false, program, traversal)?;
            events = events
                .checked_add(flow.failure_finalizers)
                .and_then(|events| events.checked_add(1))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            live = flow.live_after;
        }
    }
    events
        .checked_add(usize::from(live))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_parent_local_remaining_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    root: &'a crate::ast::Expr,
    traversal: &[Option<(&'a crate::ast::Expr, usize, usize)>],
    stack_len: usize,
    event_traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    for (ancestor, next_child, _) in traversal[..stack_len].iter().rev().flatten().copied() {
        let active_child = next_child
            .checked_sub(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut add_later_child = |child_index: usize| -> Result<(), Diagnostic> {
            if let Some(child) = ast_child(ancestor, child_index) {
                events = events
                    .checked_add(cleanup_expression_failure_events(child, event_traversal)?)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            Ok(())
        };
        match &ancestor.kind {
            crate::ast::ExprKind::If { .. } => {
                if active_child == 0 {
                    add_later_child(1)?;
                    add_later_child(2)?;
                }
            }
            crate::ast::ExprKind::Match { arms, .. } => {
                if active_child == 0 {
                    for arm_index in 0..arms.len() {
                        add_later_child(arm_index + 1)?;
                    }
                }
            }
            crate::ast::ExprKind::Binary {
                op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                ..
            } => {
                if active_child == 0 {
                    add_later_child(1)?;
                }
            }
            _ => {
                let mut child_index = next_child;
                while ast_child(ancestor, child_index).is_some() {
                    add_later_child(child_index)?;
                    child_index = child_index
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
        }
        events = events
            .checked_add(cleanup_source_failure_events(ancestor))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if matches!(
            ancestor.kind,
            crate::ast::ExprKind::Block { .. } | crate::ast::ExprKind::UpdateRecord { .. }
        ) {
            if matches!(ancestor.kind, crate::ast::ExprKind::Block { .. })
                && std::ptr::eq(ancestor, &function.body)
            {
                for ensure in &function.ensures {
                    events = events
                        .checked_add(cleanup_expression_failure_events(ensure, event_traversal)?)
                        .and_then(|events| events.checked_add(1))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            return events
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
    }
    if std::ptr::eq(root, &function.body) {
        for ensure in &function.ensures {
            events = events
                .checked_add(cleanup_expression_failure_events(ensure, event_traversal)?)
                .and_then(|events| events.checked_add(1))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
    }
    events
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_retained_stats(
    program: &Program,
    declaration_facts: &[CleanupTypeFacts],
    node_capacity: usize,
    generic_instance_upper: usize,
) -> Result<CleanupRetainedStats, Diagnostic> {
    fn key_for_type(program: &Program, ty: &crate::ast::Type) -> CleanupTypeKey {
        match ty {
            crate::ast::Type::I64 | crate::ast::Type::Bool => CleanupTypeKey::Scalar,
            crate::ast::Type::Named { name, .. } => {
                if let Some(index) = program
                    .types
                    .iter()
                    .position(|declaration| declaration.name == *name)
                {
                    CleanupTypeKey::Declaration(index)
                } else if matches!(name.as_str(), "Option" | "Result")
                    || program.types.iter().any(|declaration| {
                        declaration
                            .type_parameters
                            .iter()
                            .any(|parameter| parameter.name == *name)
                    })
                    || program.functions.iter().any(|function| {
                        function
                            .type_parameters
                            .iter()
                            .any(|parameter| parameter.name == *name)
                    })
                {
                    // Prelude Option/Result and admitted direct generic
                    // arguments are Copy-only at this boundary.
                    CleanupTypeKey::Scalar
                } else {
                    CleanupTypeKey::Unknown
                }
            }
        }
    }

    fn pattern_binding_key(
        program: &Program,
        pattern: &crate::ast::MatchPattern,
        name: &str,
    ) -> Result<Option<CleanupTypeKey>, Diagnostic> {
        Ok(match pattern {
            crate::ast::MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                ..
            } => {
                let Some(declaration) = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *type_name)
                else {
                    return Ok(None);
                };
                let crate::ast::TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                    return Ok(None);
                };
                let Some(case) = cases.iter().find(|case| case.name == *case_name) else {
                    return Ok(None);
                };
                fields.iter().find_map(|binding| {
                    (binding.binding == name).then(|| {
                        case.fields
                            .iter()
                            .find(|field| field.name == binding.name)
                            .map(|field| key_for_type(program, &field.ty))
                    })?
                })
            }
            crate::ast::MatchPattern::Record {
                type_name, fields, ..
            } => {
                let Some(declaration) = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *type_name)
                else {
                    return Ok(None);
                };
                let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
                let mut len = 1usize;
                stack[0] = Some((declaration, fields.as_slice(), 0usize, 1usize));
                let mut found = None;
                while len != 0 {
                    len -= 1;
                    let (declaration, fields, index, depth) = stack[len]
                        .take()
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let crate::ast::TypeDeclarationKind::Record {
                        fields: declarations,
                    } = &declaration.kind
                    else {
                        continue;
                    };
                    let Some(field) = fields.get(index) else {
                        continue;
                    };
                    let Some(declaration_field) = declarations
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                    else {
                        continue;
                    };
                    match &field.pattern {
                        crate::ast::RecordMatchFieldPattern::Binding { name: binding, .. }
                            if binding == name =>
                        {
                            found = Some(key_for_type(program, &declaration_field.ty));
                            break;
                        }
                        crate::ast::RecordMatchFieldPattern::Record {
                            type_name,
                            fields: child_fields,
                            ..
                        } => {
                            let Some(child) = program
                                .types
                                .iter()
                                .find(|candidate| candidate.name == *type_name)
                            else {
                                continue;
                            };
                            let child_depth = depth.checked_add(1).ok_or_else(|| {
                                b109(
                                    "max_semantic_expression_depth",
                                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                                )
                            })?;
                            if child_depth > MAX_SEMANTIC_EXPRESSION_DEPTH || len + 2 > stack.len()
                            {
                                return Err(b109(
                                    "max_semantic_expression_depth",
                                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                                ));
                            }
                            stack[len] = Some((declaration, fields, index + 1, depth));
                            stack[len + 1] = Some((child, child_fields.as_slice(), 0, child_depth));
                            len += 2;
                        }
                        _ => {
                            stack[len] = Some((declaration, fields, index + 1, depth));
                            len += 1;
                        }
                    }
                }
                found
            }
            crate::ast::MatchPattern::Wildcard { .. } => None,
        })
    }

    fn facts_for_key(
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
    ) -> CleanupTypeFacts {
        match key {
            CleanupTypeKey::Scalar => CleanupTypeFacts::default(),
            CleanupTypeKey::Declaration(index) => declaration_facts[index],
            CleanupTypeKey::Unknown => fallback,
        }
    }

    fn add_root(
        target: &mut CleanupRetainedStats,
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
        storage_identity_bytes: usize,
        resolved_type_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if matches!(key, CleanupTypeKey::Unknown) {
            target.fallback_roots = target
                .fallback_roots
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let facts = facts_for_key(key, declaration_facts, fallback);
        if facts.leaves != 0 {
            target.ordinary_slot_payload_bytes = target
                .ordinary_slot_payload_bytes
                .checked_add(
                    storage_identity_bytes
                        .checked_add(resolved_type_bytes)
                        .and_then(|bytes| bytes.checked_mul(2))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            target.ordinary_place_storage_bytes = target
                .ordinary_place_storage_bytes
                .checked_add(
                    storage_identity_bytes
                        // Initialize, Transfer source/destination, and the
                        // region's raw StorageId each own the full identity.
                        .checked_mul(4)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        target
            .add_root(facts)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }

    fn add_finalizer_upper(
        target: &mut CleanupRetainedStats,
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
        exits_after_initialization: usize,
        storage_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        let facts = facts_for_key(key, declaration_facts, fallback);
        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(
                facts
                    .leaves
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(
                facts
                    .projection_segments
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(
                facts
                    .lifecycle_ids
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(
                facts
                    .projection_ids
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(
                storage_identity_bytes
                    .checked_mul(facts.leaves)
                    .and_then(|bytes| bytes.checked_mul(exits_after_initialization))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_record_prefix(
        target: &mut CleanupRetainedStats,
        facts: CleanupTypeFacts,
        later_failure_events: usize,
        storage_identity_bytes: usize,
        field_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if facts.leaves == 0 || later_failure_events == 0 {
            return Ok(());
        }
        let finalizer_copies = facts
            .leaves
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = facts
            .projection_segments
            .checked_add(facts.leaves)
            .and_then(|segments| segments.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = facts
            .lifecycle_ids
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = facts
            .projection_ids
            .checked_add(
                facts
                    .leaves
                    .checked_mul(field_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_partial_fields = target
            .parent_local_partial_fields
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_copies = target
            .parent_local_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_projection_segments = target
            .parent_local_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_lifecycle_ids = target
            .parent_local_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_projection_ids = target
            .parent_local_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_storage_bytes = target
            .parent_local_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_update_prefix(
        target: &mut CleanupRetainedStats,
        facts: CleanupTypeFacts,
        later_failure_events: usize,
        storage_identity_bytes: usize,
        field_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if facts.leaves == 0 || later_failure_events == 0 {
            return Ok(());
        }
        let finalizer_copies = facts
            .leaves
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = facts
            .projection_segments
            .checked_add(facts.leaves)
            .and_then(|segments| segments.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = facts
            .lifecycle_ids
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = facts
            .projection_ids
            .checked_add(
                facts
                    .leaves
                    .checked_mul(field_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_update_prefix_fields = target
            .parent_local_update_prefix_fields
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_exit_groups = target
            .parent_local_update_prefix_exit_groups
            .checked_add(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_copies = target
            .parent_local_update_prefix_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_projection_segments = target
            .parent_local_update_prefix_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_lifecycle_ids = target
            .parent_local_update_prefix_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_projection_ids = target
            .parent_local_update_prefix_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_storage_bytes = target
            .parent_local_update_prefix_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_projection_residual(
        target: &mut CleanupRetainedStats,
        residual: CleanupTypeFacts,
        remaining_events: usize,
        storage_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if residual.leaves == 0 || remaining_events == 0 {
            return Ok(());
        }
        let finalizer_copies = residual
            .leaves
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = residual
            .projection_segments
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = residual
            .lifecycle_ids
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = residual
            .projection_ids
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_projection_epochs = target
            .parent_local_projection_epochs
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_exit_groups = target
            .parent_local_projection_exit_groups
            .checked_add(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_copies = target
            .parent_local_projection_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_projection_segments = target
            .parent_local_projection_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_lifecycle_ids = target
            .parent_local_projection_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_projection_ids = target
            .parent_local_projection_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_storage_bytes = target
            .parent_local_projection_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn variable_key(
        program: &Program,
        function: &crate::ast::Function,
        name: &str,
        traversal: &[Option<(&crate::ast::Expr, usize, usize)>],
        stack_len: usize,
        results: &[CleanupTypeKey],
    ) -> Result<CleanupTypeKey, Diagnostic> {
        for (ancestor, next_child, result_start) in
            traversal[..stack_len].iter().rev().flatten().copied()
        {
            match &ancestor.kind {
                crate::ast::ExprKind::Block { statements, .. } => {
                    let active_child = next_child.saturating_sub(1);
                    let completed_statements = active_child.min(statements.len());
                    for index in (0..completed_statements).rev() {
                        let crate::ast::Statement::Let { name: binding, .. } = &statements[index];
                        if binding == name {
                            return Ok(results
                                .get(result_start + index)
                                .copied()
                                .unwrap_or(CleanupTypeKey::Unknown));
                        }
                    }
                }
                crate::ast::ExprKind::Match { arms, .. } => {
                    let active_child = next_child.saturating_sub(1);
                    if let Some(arm_index) = active_child.checked_sub(1) {
                        if let Some(key) = arms
                            .get(arm_index)
                            .map(|arm| pattern_binding_key(program, &arm.pattern, name))
                            .transpose()?
                            .flatten()
                        {
                            return Ok(key);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(function
            .params
            .iter()
            .rev()
            .find(|parameter| parameter.name == name)
            .map(|parameter| key_for_type(program, &parameter.ty))
            .unwrap_or(CleanupTypeKey::Unknown))
    }

    let fallback =
        declaration_facts
            .iter()
            .copied()
            .fold(CleanupTypeFacts::default(), |maximum, facts| {
                CleanupTypeFacts {
                    leaves: maximum.leaves.max(facts.leaves),
                    occurrences: maximum.occurrences.max(facts.occurrences),
                    shape_fields: maximum.shape_fields.max(facts.shape_fields),
                    projection_segments: maximum.projection_segments.max(facts.projection_segments),
                    shape_ids: maximum.shape_ids.max(facts.shape_ids),
                    lifecycle_ids: maximum.lifecycle_ids.max(facts.lifecycle_ids),
                    projection_ids: maximum.projection_ids.max(facts.projection_ids),
                }
            });
    // Staged Result/Option records retain compiler-owned identities even in a
    // program with no user resource declarations. Keep this list adjacent to
    // the source prelude contract; tests below bind its exact spellings.
    let prelude_identity_bytes = crate::private_capacity_contract::PRELUDE_CAPACITY_IDENTITIES
        .into_iter()
        .map(str::len)
        .max()
        .expect("private prelude identities are nonempty");
    let authored_identity_bytes = program.types.iter().fold(0usize, |maximum, declaration| {
        let maximum = maximum.max(declaration.stable_id.len());
        match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { lifecycles } => {
                lifecycles.iter().fold(maximum, |maximum, lifecycle| {
                    maximum.max(lifecycle.stable_id.as_deref().map(str::len).unwrap_or(0))
                })
            }
            crate::ast::TypeDeclarationKind::Record { fields } => fields
                .iter()
                .fold(maximum, |maximum, field| maximum.max(field.stable_id.len())),
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                cases.iter().fold(maximum, |maximum, case| {
                    case.fields
                        .iter()
                        .fold(maximum.max(case.stable_id.len()), |maximum, field| {
                            maximum.max(field.stable_id.len())
                        })
                })
            }
        }
    });
    let maximum_declaration_identity_bytes = authored_identity_bytes.max(prelude_identity_bytes);
    let maximum_type_arguments = program
        .types
        .iter()
        .map(|declaration| declaration.type_parameters.len())
        .max()
        .unwrap_or(0)
        .max(2);
    let maximum_resolved_type_owned_bytes = maximum_declaration_identity_bytes
        .checked_add(
            maximum_type_arguments
                .checked_mul(std::mem::size_of::<ResolvedType>())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut total = CleanupRetainedStats::default();
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut event_traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];

    for function in &program.functions {
        let generic_instance_identity_len =
            generic_function_instance_identity_upper(program, function)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let function_roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let function_node_total =
            scan_ast_capacity(function_roots, program, false, &mut traversal)?.nodes;
        let path_segment_bytes = 32usize
            .checked_add(decimal_digits(function_node_total))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let value_storage_identity_bytes_for_path = |path_len: usize| {
            scoped_value_identity_upper(function, generic_instance_identity_len, path_len)
        };
        let expression_storage_identity_bytes_for_path = |path_len: usize| {
            scoped_expression_identity_upper(function, generic_instance_identity_len, path_len)
        };
        let type_bytes_for_key = |key: CleanupTypeKey| match key {
            CleanupTypeKey::Scalar => Some(0),
            CleanupTypeKey::Declaration(index) => program.types[index].stable_id.len().checked_add(
                program.types[index]
                    .type_parameters
                    .len()
                    .checked_mul(std::mem::size_of::<ResolvedType>())?,
            ),
            CleanupTypeKey::Unknown => Some(maximum_resolved_type_owned_bytes),
        };
        let function_exit_upper = cleanup_function_exit_events(function, &mut traversal)?;
        // These are exactly the source forms that can ask the lowerer for
        // an exit: operation failure, postfix residual, authored/update
        // scope, contract false/scope, and final success.
        let mut function_stats = CleanupRetainedStats {
            exit_events: function_exit_upper,
            ..CleanupRetainedStats::default()
        };
        let mut function_nodes = 0usize;
        let mut owned_parameters = 0usize;
        let mut has_try = false;
        for (parameter_index, parameter) in function.params.iter().enumerate() {
            if parameter.mode == crate::ast::ParamMode::Own {
                let key = key_for_type(program, &parameter.ty);
                let storage_identity_bytes =
                    value_storage_identity_bytes_for_path(decimal_digits(parameter_index))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_root(
                    &mut function_stats,
                    key,
                    declaration_facts,
                    fallback,
                    storage_identity_bytes,
                    type_bytes_for_key(key)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )?;
                add_finalizer_upper(
                    &mut function_stats,
                    key,
                    declaration_facts,
                    fallback,
                    cleanup_parameter_finalizer_events(
                        function,
                        &parameter.name,
                        program,
                        &mut event_traversal,
                    )?,
                    storage_identity_bytes,
                )?;
                function_stats.ordinary_place_storage_bytes = function_stats
                    .ordinary_place_storage_bytes
                    .checked_add(storage_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                owned_parameters = owned_parameters
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
        }

        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let mut traversal_path_lengths = [0usize; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        for (root_index, root) in roots.enumerate() {
            let mut stack_len = 1usize;
            traversal[0] = Some((root, 0usize, 0usize));
            traversal_path_lengths[0] = ast_root_identity_path_len(function, root_index);
            let mut results = Vec::<CleanupTypeKey>::with_capacity(node_capacity);
            if results.capacity() != node_capacity {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            while stack_len != 0 {
                stack_len -= 1;
                let expression_path_len = traversal_path_lengths[stack_len];
                let (expression, next_child, result_start) = traversal[stack_len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child != 0 {
                    if let crate::ast::ExprKind::Block { statements, .. } = &expression.kind {
                        let previous = next_child - 1;
                        if previous < statements.len() {
                            let crate::ast::Statement::Let { name, .. } = &statements[previous];
                            let key = results
                                .last()
                                .copied()
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let storage_identity_bytes = value_storage_identity_bytes_for_path(
                                expression_path_len
                                    .checked_add(".s".len())
                                    .and_then(|bytes| bytes.checked_add(decimal_digits(previous)))
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            add_root(
                                &mut function_stats,
                                key,
                                declaration_facts,
                                fallback,
                                storage_identity_bytes,
                                type_bytes_for_key(key)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )?;
                            if facts_for_key(key, declaration_facts, fallback).leaves != 0 {
                                function_stats.parent_local_epochs = function_stats
                                    .parent_local_epochs
                                    .checked_add(1)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            }
                            let remaining = cleanup_block_binding_finalizer_events(
                                function,
                                expression,
                                next_child,
                                name,
                                program,
                                &mut event_traversal,
                            )?;
                            add_finalizer_upper(
                                &mut function_stats,
                                key,
                                declaration_facts,
                                fallback,
                                remaining,
                                storage_identity_bytes,
                            )?;
                        }
                    }
                }
                if next_child == 0 {
                    function_nodes = function_nodes
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    match &expression.kind {
                        crate::ast::ExprKind::Call { args, .. } => {
                            if let crate::ast::ExprKind::Call { name, .. } = &expression.kind {
                                if let Some(candidate) = program
                                    .functions
                                    .iter()
                                    .find(|candidate| candidate.name == *name)
                                {
                                    for (argument_index, parameter) in
                                        candidate.params.iter().take(args.len()).enumerate()
                                    {
                                        let key = key_for_type(program, &parameter.ty);
                                        if parameter.mode != crate::ast::ParamMode::Own
                                            || facts_for_key(key, declaration_facts, fallback)
                                                .leaves
                                                == 0
                                        {
                                            continue;
                                        }
                                        // The caller retains a distinct
                                        // CallArgument epoch in addition to
                                        // the argument expression temporary.
                                        add_root(
                                            &mut function_stats,
                                            key,
                                            declaration_facts,
                                            fallback,
                                            0,
                                            0,
                                        )?;
                                        let later_argument_events = args[argument_index + 1..]
                                            .iter()
                                            .try_fold(0usize, |events, argument| {
                                                events
                                                    .checked_add(cleanup_expression_failure_events(
                                                        argument,
                                                        &mut event_traversal,
                                                    )?)
                                                    .ok_or_else(|| {
                                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                                    })
                                            })?;
                                        let argument_identity_bytes = function
                                            .stable_id
                                            .len()
                                            .checked_add(
                                                stack_len
                                                    .checked_add(2)
                                                    .and_then(|depth| {
                                                        depth.checked_mul(path_segment_bytes)
                                                    })
                                                    .ok_or_else(|| {
                                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                                    })?,
                                            )
                                            .and_then(|bytes| bytes.checked_mul(2))
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        let argument_facts =
                                            facts_for_key(key, declaration_facts, fallback);
                                        // Four paired CallArgument StorageId
                                        // copies coexist (slot, region,
                                        // Transfer destination, CallCommit
                                        // source). Transfer::at and
                                        // CallCommit::call add two single
                                        // expression IDs, equal to one more
                                        // paired upper.
                                        let fixed_storage_copies = argument_identity_bytes
                                            .checked_mul(5)
                                            .and_then(|bytes| {
                                                bytes.checked_add(maximum_resolved_type_owned_bytes)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        let failure_storage_copies = argument_identity_bytes
                                            .checked_mul(argument_facts.leaves)
                                            .and_then(|bytes| {
                                                bytes.checked_mul(later_argument_events)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        function_stats.call_argument_owned_bytes = function_stats
                                            .call_argument_owned_bytes
                                            .checked_add(fixed_storage_copies)
                                            .and_then(|bytes| {
                                                bytes.checked_add(failure_storage_copies)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        add_finalizer_upper(
                                            &mut function_stats,
                                            key,
                                            declaration_facts,
                                            fallback,
                                            later_argument_events,
                                            0,
                                        )?;
                                        function_stats.call_arguments = function_stats
                                            .call_arguments
                                            .checked_add(1)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        function_stats.parent_local_epochs = function_stats
                                            .parent_local_epochs
                                            .checked_add(1)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                    }
                                }
                            }
                        }
                        crate::ast::ExprKind::Match { arms, .. } => {
                            function_stats.variant_edges =
                                function_stats
                                    .variant_edges
                                    .checked_add(arms.len().checked_mul(2).ok_or_else(|| {
                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                    })?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        crate::ast::ExprKind::Try { .. } => {
                            has_try = true;
                            function_stats.staged_results = function_stats
                                .staged_results
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            function_stats.variant_edges = function_stats
                                .variant_edges
                                .checked_add(2)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        _ => {}
                    }
                }
                if let Some(child) = ast_child(expression, next_child) {
                    if stack_len + 2 > traversal.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    traversal[stack_len] = Some((expression, next_child + 1, result_start));
                    traversal_path_lengths[stack_len] = expression_path_len;
                    traversal[stack_len + 1] = Some((child, 0, results.len()));
                    traversal_path_lengths[stack_len + 1] = expression_path_len
                        .checked_add(ast_child_identity_path_increment(
                            expression, next_child, program,
                        ))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stack_len += 2;
                    continue;
                }

                let children = &results[result_start..];
                let key = match &expression.kind {
                    crate::ast::ExprKind::Int(_) | crate::ast::ExprKind::Bool(_) => {
                        CleanupTypeKey::Scalar
                    }
                    crate::ast::ExprKind::Var(name) => {
                        variable_key(program, function, name, &traversal, stack_len, &results)?
                    }
                    crate::ast::ExprKind::Call { name, .. } => program
                        .functions
                        .iter()
                        .find(|candidate| candidate.name == *name)
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::Unary { .. } | crate::ast::ExprKind::Binary { .. } => {
                        CleanupTypeKey::Scalar
                    }
                    crate::ast::ExprKind::Block { .. } => {
                        children.last().copied().unwrap_or(CleanupTypeKey::Scalar)
                    }
                    crate::ast::ExprKind::If { .. } => {
                        children.get(1).copied().unwrap_or(CleanupTypeKey::Unknown)
                    }
                    crate::ast::ExprKind::ConstructRecord {
                        type_name, fields, ..
                    } => {
                        let declaration_index = program
                            .types
                            .iter()
                            .position(|declaration| declaration.name == *type_name);
                        if let Some(declaration_index) = declaration_index {
                            let declaration = &program.types[declaration_index];
                            let declared_fields = match &declaration.kind {
                                crate::ast::TypeDeclarationKind::Record { fields } => fields,
                                _ => {
                                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                                }
                            };
                            let storage_identity_bytes =
                                expression_storage_identity_bytes_for_path(expression_path_len)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            for (field_index, initializer) in fields.iter().enumerate() {
                                let field_key = children
                                    .get(field_index)
                                    .copied()
                                    .unwrap_or(CleanupTypeKey::Unknown);
                                let facts = facts_for_key(field_key, declaration_facts, fallback);
                                if facts.leaves == 0 {
                                    continue;
                                }
                                let later_failure_events = fields[field_index + 1..]
                                    .iter()
                                    .try_fold(0usize, |events, later| {
                                        events
                                            .checked_add(cleanup_expression_failure_events(
                                                &later.value,
                                                &mut event_traversal,
                                            )?)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })
                                    })?;
                                let field_identity_bytes = declared_fields
                                    .iter()
                                    .find(|field| field.name == initializer.name)
                                    .map(|field| field.stable_id.len())
                                    .unwrap_or(maximum_declaration_identity_bytes);
                                add_parent_local_record_prefix(
                                    &mut function_stats,
                                    facts,
                                    later_failure_events,
                                    storage_identity_bytes,
                                    field_identity_bytes,
                                )?;
                            }
                            CleanupTypeKey::Declaration(declaration_index)
                        } else {
                            CleanupTypeKey::Unknown
                        }
                    }
                    crate::ast::ExprKind::ConstructVariant { type_name, .. } => program
                        .types
                        .iter()
                        .position(|declaration| declaration.name == *type_name)
                        .map(CleanupTypeKey::Declaration)
                        .unwrap_or_else(|| {
                            if matches!(type_name.as_str(), "Option" | "Result") {
                                CleanupTypeKey::Scalar
                            } else {
                                CleanupTypeKey::Unknown
                            }
                        }),
                    crate::ast::ExprKind::Match { arms, .. } => {
                        if let Some(arm) = arms.first() {
                            if let crate::ast::ExprKind::Var(name) = &arm.value.kind {
                                pattern_binding_key(program, &arm.pattern, name)?
                                    .unwrap_or(CleanupTypeKey::Unknown)
                            } else {
                                children.get(1).copied().unwrap_or(CleanupTypeKey::Unknown)
                            }
                        } else {
                            CleanupTypeKey::Unknown
                        }
                    }
                    crate::ast::ExprKind::Try { .. } => CleanupTypeKey::Scalar,
                    crate::ast::ExprKind::UpdateRecord { fields, .. } => {
                        let base = children.first().copied().unwrap_or(CleanupTypeKey::Unknown);
                        let destination_storage_identity_bytes =
                            expression_storage_identity_bytes_for_path(expression_path_len)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        for (field_index, initializer) in fields.iter().enumerate() {
                            let replacement_key = children
                                .get(field_index + 1)
                                .copied()
                                .unwrap_or(CleanupTypeKey::Unknown);
                            let replacement_facts =
                                facts_for_key(replacement_key, declaration_facts, fallback);
                            if replacement_facts.leaves == 0 {
                                continue;
                            }
                            let later_failure_events = fields[field_index + 1..].iter().try_fold(
                                0usize,
                                |events, later| {
                                    events
                                        .checked_add(cleanup_expression_failure_events(
                                            &later.value,
                                            &mut event_traversal,
                                        )?)
                                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                                },
                            )?;
                            let field_identity_bytes = match base {
                                CleanupTypeKey::Declaration(index) => {
                                    match &program.types[index].kind {
                                        crate::ast::TypeDeclarationKind::Record { fields } => {
                                            fields
                                                .iter()
                                                .find(|field| field.name == initializer.name)
                                                .map(|field| field.stable_id.len())
                                                .unwrap_or(maximum_declaration_identity_bytes)
                                        }
                                        _ => maximum_declaration_identity_bytes,
                                    }
                                }
                                _ => maximum_declaration_identity_bytes,
                            };
                            add_parent_local_update_prefix(
                                &mut function_stats,
                                replacement_facts,
                                later_failure_events,
                                destination_storage_identity_bytes,
                                field_identity_bytes,
                            )?;
                        }
                        let storage_identity_bytes = expression_storage_identity_bytes_for_path(
                            expression_path_len
                                .checked_add(".base".len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        add_root(
                            &mut function_stats,
                            base,
                            declaration_facts,
                            fallback,
                            storage_identity_bytes,
                            type_bytes_for_key(base)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )?;
                        let staged_base_exits = fields.iter().try_fold(
                            1usize,
                            |events, field| -> Result<usize, Diagnostic> {
                                events
                                    .checked_add(cleanup_expression_failure_events(
                                        &field.value,
                                        &mut event_traversal,
                                    )?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                            },
                        )?;
                        add_finalizer_upper(
                            &mut function_stats,
                            base,
                            declaration_facts,
                            fallback,
                            staged_base_exits,
                            storage_identity_bytes,
                        )?;
                        if facts_for_key(base, declaration_facts, fallback).leaves != 0 {
                            function_stats.parent_local_epochs = function_stats
                                .parent_local_epochs
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        base
                    }
                    crate::ast::ExprKind::Project { base, field, .. } => {
                        let base_key = children.first().copied().unwrap_or(CleanupTypeKey::Unknown);
                        let selected = match base_key {
                            CleanupTypeKey::Declaration(index) => {
                                let declaration = &program.types[index];
                                match &declaration.kind {
                                    crate::ast::TypeDeclarationKind::Record { fields } => fields
                                        .iter()
                                        .find(|candidate| candidate.name == *field)
                                        .map(|candidate| key_for_type(program, &candidate.ty)),
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                        .unwrap_or(CleanupTypeKey::Unknown);
                        if !matches!(base.kind, crate::ast::ExprKind::Var(_)) {
                            let base_facts = facts_for_key(base_key, declaration_facts, fallback);
                            let residual = if let CleanupTypeKey::Declaration(index) = base_key {
                                let selected_facts =
                                    facts_for_key(selected, declaration_facts, fallback);
                                let field_identity_bytes = match &program.types[index].kind {
                                    crate::ast::TypeDeclarationKind::Record { fields } => fields
                                        .iter()
                                        .find(|candidate| candidate.name == *field)
                                        .map(|candidate| candidate.stable_id.len())
                                        .unwrap_or(maximum_declaration_identity_bytes),
                                    _ => maximum_declaration_identity_bytes,
                                };
                                let selected_projection_segments = selected_facts
                                    .projection_segments
                                    .checked_add(selected_facts.leaves)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                                let selected_projection_ids = selected_facts
                                    .projection_ids
                                    .checked_add(
                                        selected_facts
                                            .leaves
                                            .checked_mul(field_identity_bytes)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?,
                                    )
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                                if selected_facts.leaves <= base_facts.leaves
                                    && selected_projection_segments
                                        <= base_facts.projection_segments
                                    && selected_facts.lifecycle_ids <= base_facts.lifecycle_ids
                                    && selected_projection_ids <= base_facts.projection_ids
                                {
                                    CleanupTypeFacts {
                                        leaves: base_facts.leaves - selected_facts.leaves,
                                        projection_segments: base_facts.projection_segments
                                            - selected_projection_segments,
                                        lifecycle_ids: base_facts.lifecycle_ids
                                            - selected_facts.lifecycle_ids,
                                        projection_ids: base_facts.projection_ids
                                            - selected_projection_ids,
                                        ..CleanupTypeFacts::default()
                                    }
                                } else {
                                    // Generic field substitution is not yet
                                    // materialized in this source census.
                                    // Keeping the complete base is the exact
                                    // admitted fallback, never a subtraction
                                    // from unrelated declaration facts.
                                    base_facts
                                }
                            } else {
                                // A valid unresolved generic projection may
                                // still instantiate to the maximum admitted
                                // resource aggregate. Retain the whole fallback
                                // rather than assuming which field transferred.
                                base_facts
                            };
                            let base_path_len = expression_path_len
                                .checked_add(ast_child_identity_path_increment(
                                    expression, 0, program,
                                ))
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let storage_identity_bytes =
                                expression_storage_identity_bytes_for_path(base_path_len)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let remaining_events = cleanup_parent_local_remaining_finalizer_events(
                                function,
                                root,
                                &traversal,
                                stack_len,
                                &mut event_traversal,
                            )?;
                            add_parent_local_projection_residual(
                                &mut function_stats,
                                residual,
                                remaining_events,
                                storage_identity_bytes,
                            )?;
                        }
                        selected
                    }
                };
                if !matches!(expression.kind, crate::ast::ExprKind::Var(_)) {
                    let storage_identity_bytes =
                        expression_storage_identity_bytes_for_path(expression_path_len)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_root(
                        &mut function_stats,
                        key,
                        declaration_facts,
                        fallback,
                        storage_identity_bytes,
                        type_bytes_for_key(key)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )?;
                    if facts_for_key(key, declaration_facts, fallback).leaves != 0 {
                        function_stats.parent_local_epochs = function_stats
                            .parent_local_epochs
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        function_stats.parent_local_zero_lifetime_transfers = function_stats
                            .parent_local_zero_lifetime_transfers
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
                results.truncate(result_start);
                results.push(key);
            }
            if results.len() != 1 {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
        }
        if function_nodes != function_node_total {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }

        let result_key = key_for_type(program, &function.return_type);
        add_root(
            &mut function_stats,
            result_key,
            declaration_facts,
            fallback,
            0,
            type_bytes_for_key(result_key)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )?;
        if facts_for_key(result_key, declaration_facts, fallback).leaves != 0 {
            function_stats.parent_local_epochs = function_stats
                .parent_local_epochs
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        if facts_for_key(result_key, declaration_facts, fallback).leaves != 0 {
            function_stats.ordinary_slot_payload_bytes = function_stats
                .ordinary_slot_payload_bytes
                .checked_add(
                    value_storage_identity_bytes_for_path(0)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let result_finalizer_events =
            function
                .ensures
                .iter()
                .try_fold(function.ensures.len(), |events, ensure| {
                    events
                        .checked_add(cleanup_expression_failure_events(
                            ensure,
                            &mut event_traversal,
                        )?)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                })?;
        add_finalizer_upper(
            &mut function_stats,
            result_key,
            declaration_facts,
            fallback,
            result_finalizer_events,
            0,
        )?;
        if has_try {
            // The plan retains one Body staging source in addition to every
            // residual source materialized by a postfix `?`.
            function_stats.staged_results = function_stats
                .staged_results
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let expression_identity_bytes = function
            .stable_id
            .len()
            .checked_add(
                function_nodes
                    .checked_mul(path_segment_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_add(fallback.shape_ids))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let staged_owned_bytes = expression_identity_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(maximum_resolved_type_owned_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(maximum_declaration_identity_bytes.checked_mul(5)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        function_stats.stage_identity_and_type_bytes = function_stats
            .staged_results
            .checked_mul(staged_owned_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        function_stats.variant_identity_bytes = function_stats
            .variant_edges
            .checked_mul(
                expression_identity_bytes
                    .checked_add(maximum_declaration_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if function_stats.leaves != 0 {
            // Each cleanup storage epoch is initialized once and transferred
            // at most once; its inventory/plan slot is accounted separately.
            // CallCommit argument sources are additional projected places.
            let root_transition_copies = function_stats
                .roots
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let projected_place_copies = root_transition_copies
                .checked_add(function_stats.call_arguments)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_copies = function_stats
                .roots
                .checked_add(projected_place_copies)
                .and_then(|value| value.checked_add(owned_parameters))
                .and_then(|value| value.checked_add(1))
                .and_then(|value| value.checked_add(function_stats.finalizer_copies))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_projection_segments = function_stats
                .projection_segments
                .checked_mul(2)
                .and_then(|segments| {
                    segments.checked_add(
                        fallback
                            .projection_segments
                            .checked_mul(function_stats.call_arguments)?,
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_projection_ids = function_stats
                .projection_ids
                .checked_mul(2)
                .and_then(|bytes| {
                    bytes.checked_add(
                        fallback
                            .projection_ids
                            .checked_mul(function_stats.call_arguments)?,
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }

        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            generic_instance_upper
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        total
            .merge(
                function_stats
                    .scaled(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(total)
}

#[derive(Clone, Copy)]
struct HirPreResolveCapacity {
    retained_upper: usize,
    scratch_upper: usize,
    declaration_index_upper: usize,
    cleanup_retained_upper: usize,
    cleanup_authority_upper: usize,
    cleanup_exit_events_upper: usize,
    cleanup_fallback_roots: usize,
    cleanup_call_argument_owned_upper: usize,
    cleanup_plan_structural_upper: usize,
    #[cfg(test)]
    cleanup_parent_local_lifetime_upper: usize,
    #[cfg(test)]
    cleanup_parent_local_projection_lifetime_upper: usize,
    #[cfg(test)]
    cleanup_parent_local_update_prefix_lifetime_upper: usize,
    #[cfg(test)]
    cleanup_proof: CleanupCapacityProofTerms,
    phase_peaks: [usize; 8],
    disposal_frames: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct CleanupCapacityProofTerms {
    stats: CleanupRetainedStats,
    inventory_slot_capacity_entries: usize,
    inventory_flag_capacity_entries: usize,
    inventory_entry_capacity_entries: usize,
    plan_slot_capacity_entries: usize,
    plan_entry_capacity_entries: usize,
    shape_field_capacity_entries: usize,
    flag_projection_capacity_entries: usize,
    place_projection_capacity_entries: usize,
    finalizer_projection_capacity_entries: usize,
    finalizer_capacity_entries: usize,
    block_capacity_entries: usize,
    edge_capacity_entries: usize,
    region_capacity_entries: usize,
    exit_capacity_entries: usize,
    status_capacity_entries: usize,
    transition_capacity_entries: usize,
    branch_edge_capacity_entries: usize,
    region_slot_capacity_entries: usize,
    exit_region_capacity_entries: usize,
    status_case_capacity_entries: usize,
}

impl HirPreResolveCapacity {
    fn complete(self) -> Option<usize> {
        self.retained_upper.checked_add(self.scratch_upper)
    }

    #[cfg(test)]
    fn phase_peaks(self) -> [usize; 8] {
        self.phase_peaks
    }
}

#[cfg(test)]
fn hir_capacity_terms_for_test(
    program: &Program,
    source_bytes: usize,
) -> Result<(usize, usize, usize), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(program, source_bytes, &mut stack)?;
    Ok((
        capacity.retained_upper,
        capacity.scratch_upper,
        capacity.cleanup_retained_upper,
    ))
}

fn hir_pre_resolve_capacity<'a>(
    program: &'a Program,
    source_bytes: usize,
    stack: &mut [Option<(&'a crate::ast::Expr, usize, usize)>; MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<HirPreResolveCapacity, Diagnostic> {
    let all_roots = program.functions.iter().flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    });
    let stats = scan_ast_capacity(all_roots, program, false, stack)?;
    let contract_index_digits = program.functions.iter().fold(1usize, |digits, function| {
        digits
            .max(decimal_digits(function.requires.len().saturating_sub(1)))
            .max(decimal_digits(function.ensures.len().saturating_sub(1)))
            .max(decimal_digits(function.params.len().saturating_sub(1)))
    });
    let monomorphic_roots = program
        .functions
        .iter()
        .filter(|function| function.type_parameters.is_empty())
        .flat_map(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
        });
    let reachable_generic_calls =
        scan_ast_capacity(monomorphic_roots, program, true, stack)?.generic_calls;
    let mut largest_template = AstCapacityStats::default();
    for function in &program.functions {
        if function.type_parameters.is_empty() {
            continue;
        }
        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let template = scan_ast_capacity(roots, program, false, stack)?;
        largest_template.nodes = largest_template.nodes.max(template.nodes);
        largest_template.cumulative_depth = largest_template
            .cumulative_depth
            .max(template.cumulative_depth);
        largest_template.max_depth = largest_template.max_depth.max(template.max_depth);
        largest_template.max_match_arms =
            largest_template.max_match_arms.max(template.max_match_arms);
        largest_template.max_indexed_children = largest_template
            .max_indexed_children
            .max(template.max_indexed_children);
        largest_template.depth_arm_product_sum = largest_template
            .depth_arm_product_sum
            .max(template.depth_arm_product_sum);
        largest_template.depth_width_product_sum = largest_template
            .depth_width_product_sum
            .max(template.depth_width_product_sum);
        largest_template.local_bindings =
            largest_template.local_bindings.max(template.local_bindings);
        largest_template.pattern_bindings = largest_template
            .pattern_bindings
            .max(template.pattern_bindings);
        largest_template.binding_name_bytes = largest_template
            .binding_name_bytes
            .max(template.binding_name_bytes);
        largest_template.binding_depth_sum = largest_template
            .binding_depth_sum
            .max(template.binding_depth_sum);
        largest_template.max_index_digits = largest_template
            .max_index_digits
            .max(template.max_index_digits);
    }
    let declarations = program
        .types
        .len()
        .checked_add(program.interfaces.len())
        .and_then(|value| value.checked_add(program.functions.len()))
        .and_then(|value| {
            program
                .interfaces
                .iter()
                .try_fold(value, |value, interface| {
                    value.checked_add(interface.imports.len())
                })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let nested_declarations = program
        .types
        .iter()
        .try_fold(declarations, |count, declaration| {
            let count = count.checked_add(declaration.type_parameters.len())?;
            match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { lifecycles } => {
                    count.checked_add(lifecycles.len())
                }
                crate::ast::TypeDeclarationKind::Record { fields } => {
                    count.checked_add(fields.len())
                }
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .try_fold(count.checked_add(cases.len())?, |count, case| {
                        count.checked_add(case.fields.len())
                    }),
            }
        })
        .and_then(|count| {
            program.functions.iter().try_fold(count, |count, function| {
                count
                    .checked_add(function.type_parameters.len())?
                    .checked_add(function.params.len())
            })
        })
        .and_then(|count| {
            program
                .interfaces
                .iter()
                .try_fold(count, |count, interface| {
                    interface.imports.iter().try_fold(count, |count, import| {
                        count.checked_add(import.params.len())
                    })
                })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The longest indexed segment is `.arm.<i>.binding.<j>`; derive its digit
    // widths from the widest admitted authored node instead of assuming a
    // machine-usize textual width. Resolved
    // expression identity, value identity, cleanup inventory, cleanup plan,
    // and validation/index ownership can retain at most six path-bearing
    // copies. Fixed node/declaration terms cover enum/vector/BTree node bodies.
    let maximum_index_digits = stats.max_index_digits.max(contract_index_digits);
    let indexed_path_segment_bytes = 15usize
        .checked_add(
            maximum_index_digits
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_node_inline = std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>()
        .checked_add(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_node_inline = std::mem::size_of::<ResolvedExpr>()
        .checked_add(cleanup_node_inline)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let type_expansion = declaration_dag_expansion(program, reachable_generic_calls)?;
    let maximum_resource_leaves = type_expansion.maximum_resource_leaves;
    let disposal_frames = stats
        .max_depth
        .checked_mul(4)
        .and_then(|frames| {
            frames.checked_add(type_expansion.maximum_type_occurrences.checked_mul(2)?)
        })
        .and_then(|frames| frames.checked_add(16))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let path_copy_upper = 1usize
        .checked_add(maximum_resource_leaves.min(5))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_path_copies = maximum_resource_leaves.min(5);
    let mut exact_expression_identity_bytes = 0usize;
    let mut cleanup_plan_uncovered_identity_bytes = 0usize;
    for function in &program.functions {
        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            reachable_generic_calls
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        let (function_expression_bytes, function_plan_bytes) =
            cleanup_plan_variable_identity_bytes(function, program, cleanup_path_copies)?;
        exact_expression_identity_bytes = exact_expression_identity_bytes
            .checked_add(
                function_expression_bytes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_plan_uncovered_identity_bytes = cleanup_plan_uncovered_identity_bytes
            .checked_add(
                function_plan_bytes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let node_bytes = stats
        .nodes
        .checked_mul(retained_node_inline)
        .and_then(|bytes| {
            bytes.checked_add(exact_expression_identity_bytes.checked_mul(path_copy_upper)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Peak iterative resolver/validator/cleanup scratch. The declaration
    // census is a conservative upper for simultaneously live bindings/flags.
    // Branch continuations retain at most depth copies; Match retains one
    // FlowState per authored arm. Indexed child vectors/commit lists are
    // bounded by the widest authored node.
    let parameter_bindings = program
        .functions
        .iter()
        .try_fold(0usize, |count, function| {
            count.checked_add(function.params.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let maximum_declared_fields = program
        .types
        .iter()
        .try_fold(0usize, |total, declaration| {
            let fields = match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { .. } => 1,
                crate::ast::TypeDeclarationKind::Record { fields } => fields.len(),
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))?,
            };
            total.checked_add(fields)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let binding_slots = parameter_bindings
        .checked_add(stats.local_bindings)
        .and_then(|width| width.checked_add(stats.pattern_bindings))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // A binding of an aggregate can contribute one ownership/partial-place
    // fact per resource leaf. The declaration-field sum is a no-allocation
    // upper for an acyclic declaration graph, while the declaration verifier
    // rejects cycles before semantic admission.
    let live_state_width = binding_slots
        .checked_mul(maximum_resource_leaves)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let branch_scope_copies = stats
        .depth_arm_product_sum
        .checked_add(stats.max_depth)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let parameter_name_bytes = program
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            function.params.iter().try_fold(bytes, |bytes, parameter| {
                bytes.checked_add(parameter.name.len())
            })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let binding_identity_bytes = stats
        .binding_name_bytes
        .checked_add(parameter_name_bytes)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .binding_depth_sum
                    .checked_mul(indexed_path_segment_bytes)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let scope_entry_inline =
        std::mem::size_of::<(crate::hir::ValueId, ResolvedType, OwnershipMode)>();
    let scope_payload_bytes = live_state_width
        .checked_mul(scope_entry_inline)
        .and_then(|bytes| {
            bytes.checked_add(binding_identity_bytes.checked_mul(maximum_declared_fields)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let indexed_result_bytes = std::mem::size_of::<ResolvedExpr>()
        .checked_add(std::mem::size_of::<ResolvedStatement>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<crate::hir::ResolvedFieldInitializer>())
        })
        .and_then(|bytes| bytes.checked_add(CLEANUP_EVAL_RESULT_BYTES))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_work_bytes = std::mem::size_of::<crate::hir::ResolvedTypeDeclaration>()
        .checked_add(std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<
                crate::hir::ResolvedVariantCaseDeclaration,
            >())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let source_phase = stats
        .max_depth
        .checked_mul(SOURCE_VERIFIER_FRAME_BYTES)
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolver_phase = stats
        .max_depth
        .checked_mul(HIR_RESOLVER_FRAME_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(stats.max_indexed_children.max(1))?
                    .checked_mul(indexed_result_bytes)?,
            )
        })
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let validator_phase = stats
        .max_depth
        .checked_mul(HIR_VALIDATOR_FRAME_BYTES)
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_phase = maximum_resource_leaves
        .checked_mul(
            CLEANUP_INVENTORY_SHAPE_FRAME_BYTES
                + std::mem::size_of::<DeclarationId>()
                + std::mem::size_of::<semaprax::cleanup::FieldLivenessShape>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(CLEANUP_INVENTORY_EXPR_FRAME_BYTES)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_entry_bytes = std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>()
        + std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>()
        + std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>()
        + std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>()
        + std::mem::size_of::<semaprax::cleanup_plan::StatusSource>();
    let cleanup_phase = stats
        .max_depth
        .checked_mul(CLEANUP_LOWER_FRAME_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(stats.max_indexed_children.max(1))?
                    .checked_mul(CLEANUP_EVAL_RESULT_BYTES)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .nodes
                    .checked_mul(maximum_resource_leaves)?
                    .checked_mul(plan_entry_bytes)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let call_index_phase = stats
        .max_depth
        .checked_mul(CALL_INDEX_FRAME_BYTES)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let closure_identity_entries = program
        .functions
        .len()
        .checked_add(program.interfaces.len())
        .and_then(|entries| entries.checked_add(nested_declarations))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let closure_btree_entry_overhead = std::mem::size_of::<BTreeMap<String, usize>>();
    let closure_reference_headers = program
        .functions
        .len()
        .checked_mul(std::mem::size_of::<&ResolvedFunction>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let closure_phase = Some(closure_reference_headers)
        // The selected closure borrows functions from the live resolved
        // program. Only the sorted reference vector is retained; expression
        // and cleanup trees are neither cloned nor separately dropped.
        // by_id, state, depths, reached-imports, pending/visited/direct-call
        // sets, and contract traversal sets can overlap. One separately
        // allocated BTree node per identity plus the full authored source as
        // every key payload is conservative for each of the nine containers.
        .and_then(|bytes| {
            bytes.checked_add(
                closure_identity_entries
                    .checked_mul(
                        std::mem::size_of::<(String, usize)>()
                            .checked_add(closure_btree_entry_overhead)?,
                    )?
                    .checked_mul(9)?,
            )
        })
        .and_then(|bytes| bytes.checked_add(source_bytes.checked_mul(9)?))
        // DFS retains one ID and one indexed direct-call vector per depth.
        .and_then(|bytes| {
            bytes.checked_add(
                MAX_CALL_DEPTH.checked_mul(
                    std::mem::size_of::<SelectedClosureFrame>()
                        .checked_add(indexed_path_segment_bytes)?,
                )?,
            )
        })
        // While converting a direct-call set into the frame Vec, both
        // container backings and all ID strings coexist.
        .and_then(|bytes| {
            bytes.checked_add(
                closure_identity_entries.checked_mul(
                    std::mem::size_of::<String>()
                        .checked_add(std::mem::size_of::<DeclarationId>())?
                        .checked_add(closure_btree_entry_overhead)?,
                )?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let frame_machine_scratch = source_phase
        .max(resolver_phase)
        .max(validator_phase)
        .max(inventory_phase)
        .max(cleanup_phase)
        .max(call_index_phase)
        .max(closure_phase)
        .checked_add(
            nested_declarations
                .checked_mul(declaration_work_bytes)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_phase_overlap = nested_declarations
        .checked_mul(declaration_work_bytes)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Each distinct reachable specialization may clone and resolve one whole
    // template while the resolved template remains live. Count every call
    // site (even duplicate instances) against the largest template, which is
    // conservative without allocating a pre-resolution identity set.
    let specialization_bytes = largest_template
        .nodes
        .checked_mul(
            retained_node_inline
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .cumulative_depth
                    .checked_mul(indexed_path_segment_bytes.checked_mul(2 * 6)?)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .max_depth
                    .checked_mul(HIR_RESOLVER_FRAME_BYTES)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .depth_arm_product_sum
                    .checked_add(largest_template.max_depth)?
                    .checked_mul(live_state_width)?
                    .checked_mul(scope_entry_inline)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .max_depth
                    .checked_mul(largest_template.max_indexed_children.max(1))?
                    .checked_mul(indexed_result_bytes)?,
            )
        })
        .and_then(|bytes| bytes.checked_mul(reachable_generic_calls))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // TypeFacts layout keys recursively embed each child key. The fixed
    // per-occurrence syntax consists of four decimal lengths/separators plus
    let type_fact_layout_upper = crate::private_capacity_contract::type_facts_layout_upper(
        source_bytes,
        program.types.len(),
        type_expansion.maximum_type_occurrences,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let type_facts_frame_bytes = std::mem::size_of::<(
        ResolvedType,
        String,
        DeclarationId,
        crate::hir::DeclarationKind,
        usize,
    )>();
    let type_facts_scratch = type_expansion
        .maximum_type_occurrences
        .checked_mul(type_facts_frame_bytes)
        .and_then(|bytes| bytes.checked_add(type_fact_layout_upper.checked_mul(2)?))
        .and_then(|bytes| {
            bytes.checked_add(
                program
                    .types
                    .len()
                    .checked_mul(std::mem::size_of::<(String, crate::hir::TypeFacts)>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_index_upper = crate::private_capacity_contract::declaration_index_upper(
        source_bytes,
        program.types.len(),
        program.interfaces.len(),
        program.functions.len(),
        type_fact_layout_upper,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The declaration-DAG pass also performs a typed source-flow census while
    // its exact temporary memo is still authorized. Unlike the former
    // `all roots * largest type * all nodes` product, every persistent shape,
    // flag and projection below is charged against the authored type of the
    // storage root that can create it. Plan places and exits are separate
    // copies because they coexist with the inventory and plan-slot shapes.
    let cleanup = type_expansion.cleanup_retained;
    let cleanup_function_instance_upper = program
        .functions
        .iter()
        .try_fold(0usize, |instances, function| {
            let multiplicity = if function.type_parameters.is_empty() {
                1
            } else {
                reachable_generic_calls.checked_add(1)?
            };
            instances.checked_add(multiplicity)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let flag_capacity_extra =
        retained_vec_capacity_extra(cleanup.leaves, cleanup_function_instance_upper)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let shape_field_capacity_extra = retained_vec_capacity_extra(
        cleanup.shape_fields,
        cleanup.occurrences.min(cleanup.shape_fields),
    )
    .and_then(|extra| extra.checked_mul(2))
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let flag_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.projection_segments,
        cleanup.leaves.min(cleanup.projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let place_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.place_projection_segments,
        cleanup.place_copies.min(cleanup.place_projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let finalizer_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.finalizer_projection_segments,
        cleanup
            .finalizer_copies
            .min(cleanup.finalizer_projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let finalizer_capacity_extra = retained_vec_capacity_extra(
        cleanup.finalizer_copies,
        cleanup.exit_events.min(cleanup.finalizer_copies),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let entry_state_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_slot_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_slot_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_entry_capacity_entries = cleanup
        .roots
        .checked_add(entry_state_capacity_extra)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_parent_local_lifetime_upper = cleanup
        .parent_local_finalizer_copies
        .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .parent_local_finalizer_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_lifecycle_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_projection_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_storage_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_parent_local_projection_lifetime_upper = {
        let action_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_projection_finalizer_copies,
            cleanup
                .parent_local_projection_exit_groups
                .min(cleanup.parent_local_projection_finalizer_copies),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_projection_finalizer_projection_segments,
            cleanup
                .parent_local_projection_finalizer_copies
                .min(cleanup.parent_local_projection_finalizer_projection_segments),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup
            .parent_local_projection_finalizer_copies
            .checked_add(action_capacity_extra)
            .and_then(|entries| {
                entries.checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    cleanup
                        .parent_local_projection_finalizer_projection_segments
                        .checked_add(projection_capacity_extra)?
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_lifecycle_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_projection_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_storage_bytes)
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
    };
    #[cfg(test)]
    let cleanup_parent_local_update_prefix_lifetime_upper = {
        let action_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_update_prefix_finalizer_copies,
            cleanup
                .parent_local_update_prefix_exit_groups
                .min(cleanup.parent_local_update_prefix_finalizer_copies),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_update_prefix_finalizer_projection_segments,
            cleanup
                .parent_local_update_prefix_finalizer_copies
                .min(cleanup.parent_local_update_prefix_finalizer_projection_segments),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup
            .parent_local_update_prefix_finalizer_copies
            .checked_add(action_capacity_extra)
            .and_then(|entries| {
                entries.checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    cleanup
                        .parent_local_update_prefix_finalizer_projection_segments
                        .checked_add(projection_capacity_extra)?
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_lifecycle_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_projection_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_storage_bytes)
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
    };
    let cleanup_retained_upper = cleanup
        .roots
        .checked_mul(
            std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>()
                + std::mem::size_of::<semaprax::cleanup_plan::CleanupSlot>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .occurrences
                    .checked_mul(2)?
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLivenessShape>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .shape_fields
                    .checked_mul(2)?
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLiveness>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.shape_ids.checked_mul(2)?))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .leaves
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.lifecycle_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .place_copies
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupPlace>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .place_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.place_projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .finalizer_copies
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .finalizer_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.finalizer_lifecycle_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.finalizer_projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(cleanup.staged_results.checked_mul(std::mem::size_of::<
                semaprax::cleanup_plan::StagedCopyResultSource,
            >())?)
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .variant_edges
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::EdgeCondition>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.stage_identity_and_type_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.variant_identity_bytes))
        .and_then(|bytes| {
            bytes.checked_add(cleanup.call_arguments.checked_mul(std::mem::size_of::<
                semaprax::cleanup_plan::CallArgumentTransfer,
            >())?)
        })
        .and_then(|bytes| bytes.checked_add(cleanup.call_argument_owned_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_slot_payload_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_place_storage_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_finalizer_storage_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                flag_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                shape_field_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLiveness>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                flag_projection_capacity_extra.checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                place_projection_capacity_extra.checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                finalizer_projection_capacity_extra
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                finalizer_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                entry_state_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupPlace>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                inventory_slot_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                plan_slot_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupSlot>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                inventory_entry_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupStorageId>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut cleanup_structural_nodes = 0usize;
    let mut cleanup_structural_depth = 0usize;
    let mut cleanup_failure_events = 0usize;
    let mut cleanup_call_events = 0usize;
    let mut cleanup_boolean_branch_events = 0usize;
    let mut cleanup_contracts = 0usize;
    let mut cleanup_function_instances = 0usize;
    let cleanup_expression_identity_bytes = exact_expression_identity_bytes;
    for function in &program.functions {
        let function_stats = scan_ast_capacity(
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures),
            program,
            false,
            stack,
        )?;
        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            reachable_generic_calls
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        cleanup_structural_nodes = cleanup_structural_nodes
            .checked_add(
                function_stats
                    .nodes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_structural_depth =
            cleanup_structural_depth.max(cleanup_function_region_depth(function, stack)?);
        cleanup_function_instances = cleanup_function_instances
            .checked_add(multiplicity)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_contracts = cleanup_contracts
            .checked_add(
                function
                    .requires
                    .len()
                    .checked_add(function.ensures.len())
                    .and_then(|contracts| contracts.checked_mul(multiplicity))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_failure_events = cleanup_failure_events
            .checked_add(
                cleanup_function_finalizer_events(function, stack)?
                    .checked_sub(1)
                    .and_then(|events| events.checked_mul(multiplicity))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut function_call_events = 0usize;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            function_call_events = function_call_events
                .checked_add(cleanup_expression_call_events(root, program, stack)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        cleanup_call_events = cleanup_call_events
            .checked_add(
                function_call_events
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut function_boolean_branch_events = 0usize;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            function_boolean_branch_events = function_boolean_branch_events
                .checked_add(cleanup_expression_boolean_branch_events(root, stack)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        cleanup_boolean_branch_events = cleanup_boolean_branch_events
            .checked_add(
                function_boolean_branch_events
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let cleanup_structural_upper = cleanup_structural_nodes
        .checked_mul(cleanup_node_inline)
        .and_then(|bytes| {
            bytes.checked_add(cleanup_expression_identity_bytes.checked_mul(cleanup_path_copies)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Retained CleanupPlan container backing and identity payloads are
    // distinct from inventory/slot shapes above. Derive each family from the
    // source events that can create it. Four headers per logical entry covers
    // the current target's minimum-capacity floor for independently allocated
    // small Vecs as well as geometric growth.
    let transition_entries = cleanup
        .occurrences
        .checked_mul(2)
        // Every failing status path owns one SelectFailure transition. Only
        // ordinary calls additionally own CallCommit; checked arithmetic and
        // contract-false paths do not. Native Rust imports have neither.
        .and_then(|entries| entries.checked_add(cleanup_failure_events))
        .and_then(|entries| entries.checked_add(cleanup_call_events))
        .and_then(|entries| entries.checked_add(cleanup.call_arguments))
        .and_then(|entries| entries.checked_add(cleanup.staged_results))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_callee_identity_bytes = program
        .functions
        .iter()
        .map(|function| function.stable_id.len())
        .chain(program.interfaces.iter().flat_map(|interface| {
            interface
                .imports
                .iter()
                .map(|import| import.stable_id.len())
        }))
        .max()
        .unwrap_or(0);
    let expression_identity_fixed_bytes = "function-execution:"
        .len()
        .checked_add("semaprax.function-execution.v1:generic:".len())
        .and_then(|bytes| bytes.checked_add("declaration:".len()))
        .and_then(|bytes| bytes.checked_add(":expression:".len()))
        .and_then(|bytes| bytes.checked_add(decimal_digits(source_bytes).checked_mul(4)?))
        .and_then(|bytes| bytes.checked_add(8))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_block_headers = cleanup_structural_nodes
        .checked_mul(2)
        .and_then(|entries| entries.checked_add(cleanup_contracts))
        .and_then(|entries| entries.checked_add(cleanup_function_instances))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_edge_headers = cleanup_structural_nodes
        .checked_mul(3)
        .and_then(|entries| entries.checked_add(cleanup_contracts.checked_mul(2)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_region_headers = cleanup_contracts
        .checked_add(cleanup_function_instances)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_exit_headers = cleanup
        .exit_events
        .checked_sub(cleanup.exit_events.min(cleanup_structural_nodes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let block_entries = cleanup_structural_nodes
        .checked_add(extra_block_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let edge_entries = cleanup_structural_nodes
        .checked_add(extra_edge_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_entries = cleanup_structural_nodes
        .checked_add(extra_region_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_entries = cleanup_structural_nodes
        .checked_add(extra_exit_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let block_capacity_extra =
        retained_vec_capacity_extra(block_entries, cleanup_function_instances.min(block_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let edge_capacity_extra =
        retained_vec_capacity_extra(edge_entries, cleanup_function_instances.min(edge_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_capacity_extra = retained_vec_capacity_extra(
        region_entries,
        cleanup_function_instances.min(region_entries),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_capacity_extra =
        retained_vec_capacity_extra(exit_entries, cleanup_function_instances.min(exit_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_capacity_extra = retained_vec_capacity_extra(
        cleanup_failure_events,
        cleanup_function_instances.min(cleanup_failure_events),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_expression_identity_copies = cleanup
        .occurrences
        .checked_mul(2)
        .and_then(|copies| copies.checked_add(cleanup_failure_events.checked_mul(5)?))
        .and_then(|copies| copies.checked_add(cleanup_call_events))
        .and_then(|copies| copies.checked_add(cleanup_boolean_branch_events.checked_mul(2)?))
        .and_then(|copies| copies.checked_add(cleanup_function_instances))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let transition_capacity_entries = transition_entries
        .checked_add(
            retained_vec_capacity_extra(transition_entries, block_entries.min(transition_entries))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let branch_edge_entries = cleanup_structural_nodes
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let branch_edge_capacity_entries = branch_edge_entries
        .checked_add(
            retained_vec_capacity_extra(
                branch_edge_entries,
                block_entries.min(branch_edge_entries),
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_slot_capacity_entries = cleanup
        .roots
        .checked_add(
            retained_vec_capacity_extra(cleanup.roots, region_entries.min(cleanup.roots))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_region_entries = cleanup
        .exit_events
        .checked_mul(cleanup_structural_depth.max(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_region_capacity_entries = exit_region_entries
        .checked_add(
            retained_vec_capacity_extra(exit_region_entries, exit_entries.min(exit_region_entries))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_case_capacity_entries = cleanup_failure_events
        .checked_add(
            retained_vec_capacity_extra(cleanup_failure_events, cleanup_failure_events)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_plan_structural_upper = transition_capacity_entries
        .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupTransition>())
        .and_then(|bytes| {
            bytes.checked_add(
                branch_edge_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::EdgeId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_block_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_edge_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_region_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_exit_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                region_slot_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StorageId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                exit_region_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegionId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup_failure_events
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusSource>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                status_case_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusCase>())?,
            )
        })
        // The full path payload has one source-derived copy in
        // cleanup_structural_upper. Each status/edge/continuation clone also
        // owns the fixed scoped-identity framing around that path.
        .and_then(|bytes| {
            bytes.checked_add(
                plan_expression_identity_copies.checked_mul(expression_identity_fixed_bytes)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(cleanup_failure_events.checked_mul(cleanup_callee_identity_bytes)?)
        })
        .and_then(|bytes| bytes.checked_add(cleanup_plan_uncovered_identity_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                block_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                edge_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                region_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                exit_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                status_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusSource>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_authority_upper = cleanup_retained_upper
        .checked_add(cleanup_structural_upper)
        .and_then(|bytes| bytes.checked_add(cleanup_plan_structural_upper))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_proof =
        CleanupCapacityProofTerms {
            stats: cleanup,
            inventory_slot_capacity_entries: cleanup
                .roots
                .checked_add(inventory_slot_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_flag_capacity_entries: cleanup
                .leaves
                .checked_add(flag_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_entry_capacity_entries,
            plan_slot_capacity_entries: cleanup
                .roots
                .checked_add(plan_slot_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            plan_entry_capacity_entries: cleanup
                .roots
                .checked_add(entry_state_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            shape_field_capacity_entries: cleanup
                .shape_fields
                .checked_mul(2)
                .and_then(|entries| entries.checked_add(shape_field_capacity_extra))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            flag_projection_capacity_entries: cleanup
                .projection_segments
                .checked_add(flag_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            place_projection_capacity_entries: cleanup
                .place_projection_segments
                .checked_add(place_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            finalizer_projection_capacity_entries: cleanup
                .finalizer_projection_segments
                .checked_add(finalizer_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            finalizer_capacity_entries: cleanup
                .finalizer_copies
                .checked_add(finalizer_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            block_capacity_entries: block_entries
                .checked_add(block_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            edge_capacity_entries: edge_entries
                .checked_add(edge_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            region_capacity_entries: region_entries
                .checked_add(region_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            exit_capacity_entries: exit_entries
                .checked_add(exit_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            status_capacity_entries: cleanup_failure_events
                .checked_add(status_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            transition_capacity_entries,
            branch_edge_capacity_entries,
            region_slot_capacity_entries,
            exit_region_capacity_entries,
            status_case_capacity_entries,
        };
    let disposal_workspace_bytes = disposal_frames
        .checked_mul(std::mem::size_of::<ResolvedDisposeFrame>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolved_function_headers = program
        .functions
        .len()
        .checked_add(reachable_generic_calls)
        .and_then(|functions| functions.checked_mul(std::mem::size_of::<ResolvedFunction>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_upper = source_bytes
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(node_bytes))
        .and_then(|bytes| bytes.checked_add(specialization_bytes))
        .and_then(|bytes| {
            bytes.checked_add(nested_declarations.checked_mul(declaration_work_bytes)?)
        })
        .and_then(|bytes| bytes.checked_add(declaration_index_upper))
        .and_then(|bytes| bytes.checked_add(cleanup_retained_upper))
        .and_then(|bytes| bytes.checked_add(cleanup_plan_structural_upper))
        .and_then(|bytes| bytes.checked_add(disposal_workspace_bytes))
        .and_then(|bytes| bytes.checked_add(resolved_function_headers))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(HirPreResolveCapacity {
        retained_upper,
        scratch_upper: frame_machine_scratch.max(type_facts_scratch),
        declaration_index_upper,
        cleanup_retained_upper,
        cleanup_authority_upper,
        cleanup_exit_events_upper: cleanup.exit_events,
        cleanup_fallback_roots: cleanup.fallback_roots,
        cleanup_call_argument_owned_upper: cleanup.call_argument_owned_bytes,
        cleanup_plan_structural_upper,
        #[cfg(test)]
        cleanup_parent_local_lifetime_upper,
        #[cfg(test)]
        cleanup_parent_local_projection_lifetime_upper,
        #[cfg(test)]
        cleanup_parent_local_update_prefix_lifetime_upper,
        #[cfg(test)]
        cleanup_proof,
        phase_peaks: [
            source_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            resolver_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            validator_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            cleanup_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            call_index_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            closure_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            type_facts_scratch,
        ],
        disposal_frames,
    })
}

fn hir_type_owned_capacity(ty: &ResolvedType) -> Option<usize> {
    match ty {
        ResolvedType::Unit | ResolvedType::I64 | ResolvedType::Bool => Some(0),
        ResolvedType::TypeParameter { owner, .. } => Some(owner.as_str().len()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => arguments
            .iter()
            .try_fold(declaration.as_str().len(), |bytes, argument| {
                bytes.checked_add(hir_type_owned_capacity(argument)?)
            })?
            .checked_add(arguments.capacity() * std::mem::size_of::<ResolvedType>()),
    }
}

fn add_capacity(total: &mut usize, capacity: usize, element: usize) -> Result<(), Diagnostic> {
    *total = total
        .checked_add(
            capacity
                .checked_mul(element)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(())
}

fn hir_binding_owned_capacity(binding: &crate::hir::ResolvedBinding) -> Option<usize> {
    binding
        .id
        .as_str()
        .len()
        .checked_add(binding.name.capacity())?
        .checked_add(hir_type_owned_capacity(&binding.ty)?)
}

fn hir_match_pattern_owned_capacity(pattern: &crate::hir::ResolvedMatchPattern) -> Option<usize> {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => Some(0),
        crate::hir::ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => fields
            .iter()
            .try_fold(
                variant.as_str().len().checked_add(case.as_str().len())?,
                |bytes, field| {
                    bytes
                        .checked_add(field.field.as_str().len())?
                        .checked_add(hir_binding_owned_capacity(&field.binding)?)
                },
            )?
            .checked_add(
                fields.capacity() * std::mem::size_of::<crate::hir::ResolvedMatchPatternField>(),
            ),
        crate::hir::ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => fields
            .iter()
            .try_fold(
                record
                    .as_str()
                    .len()
                    .checked_add(hir_type_owned_capacity(instance)?)?,
                |bytes, field| {
                    bytes
                        .checked_add(field.field.as_str().len())?
                        .checked_add(hir_record_pattern_field_owned_capacity(&field.pattern)?)
                },
            )?
            .checked_add(
                fields.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedRecordMatchPatternField>(),
            ),
    }
}

fn hir_record_pattern_field_owned_capacity(
    pattern: &crate::hir::ResolvedRecordMatchFieldPattern,
) -> Option<usize> {
    match pattern {
        crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
            hir_binding_owned_capacity(binding)
        }
        crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => Some(0),
        crate::hir::ResolvedRecordMatchFieldPattern::Record {
            record,
            instance,
            fields,
        } => fields
            .iter()
            .try_fold(
                record
                    .as_str()
                    .len()
                    .checked_add(hir_type_owned_capacity(instance)?)?,
                |bytes, field| {
                    bytes
                        .checked_add(field.field.as_str().len())?
                        .checked_add(hir_record_pattern_field_owned_capacity(&field.pattern)?)
                },
            )?
            .checked_add(
                fields.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedRecordMatchPatternField>(),
            ),
    }
}

fn hir_expr_owned_capacity(expression: &ResolvedExpr) -> Result<usize, Diagnostic> {
    let mut total = 0_usize;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        total = total
            .checked_add(std::mem::size_of::<ResolvedExpr>())
            .and_then(|bytes| bytes.checked_add(expression.id.as_str().len()))
            .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&expression.ty)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match &expression.kind {
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                total = total
                    .checked_add(callee.as_str().len())
                    .and_then(|bytes| {
                        bytes.checked_add(
                            type_arguments.capacity() * std::mem::size_of::<ResolvedType>(),
                        )
                    })
                    .and_then(|bytes| {
                        instance.as_ref().map_or(Some(bytes), |instance| {
                            bytes.checked_add(instance.as_str().len())
                        })
                    })
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for ty in type_arguments {
                    let ty_bytes = hir_type_owned_capacity(ty)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    total = total
                        .checked_add(ty_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                add_capacity(
                    &mut total,
                    args.capacity(),
                    std::mem::size_of::<ResolvedExpr>(),
                )?;
                pending.extend(args);
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                total = total
                    .checked_add(call.expression.as_str().len())
                    .and_then(|bytes| bytes.checked_add(call.import.as_str().len()))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    call.args.capacity(),
                    std::mem::size_of::<ResolvedExpr>(),
                )?;
                pending.extend(&call.args);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                for id in [result, ok_case, ok_field, err_case, err_field] {
                    total = total
                        .checked_add(id.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                total = total
                    .checked_add(
                        hir_type_owned_capacity(residual_type)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                pending.push(operand);
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                for id in [option, some_case, some_field, none_case] {
                    total = total
                        .checked_add(id.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                total = total
                    .checked_add(
                        hir_type_owned_capacity(residual_type)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                pending.push(operand);
            }
            ResolvedExprKind::Project { base, field } => {
                total = total
                    .checked_add(field.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                pending.push(base);
            }
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                add_capacity(
                    &mut total,
                    statements.capacity(),
                    std::mem::size_of::<ResolvedStatement>(),
                )?;
                for statement in statements {
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    total = total
                        .checked_add(binding.id.as_str().len())
                        .and_then(|bytes| bytes.checked_add(binding.name.capacity()))
                        .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&binding.ty)?))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
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
            ResolvedExprKind::ConstructRecord { record, fields } => {
                total = total
                    .checked_add(record.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    fields.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedFieldInitializer>(),
                )?;
                for field in fields {
                    total = total
                        .checked_add(field.field.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                total = total
                    .checked_add(variant.as_str().len())
                    .and_then(|bytes| bytes.checked_add(case.as_str().len()))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    fields.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedFieldInitializer>(),
                )?;
                for field in fields {
                    total = total
                        .checked_add(field.field.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                add_capacity(
                    &mut total,
                    arms.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedMatchArm>(),
                )?;
                for arm in arms {
                    total = total
                        .checked_add(
                            hir_match_pattern_owned_capacity(&arm.pattern)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.push(scrutinee);
                pending.extend(arms.iter().map(|arm| &arm.value));
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                total = total
                    .checked_add(record.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    fields.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedFieldInitializer>(),
                )?;
                for field in fields {
                    total = total
                        .checked_add(field.field.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Place(place) => {
                total = total
                    .checked_add(place.root.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    place.projections.capacity(),
                    std::mem::size_of::<crate::hir::PlaceProjection>(),
                )?;
                for projection in &place.projections {
                    total = total
                        .checked_add(match projection {
                            crate::hir::PlaceProjection::Field(field) => field.as_str().len(),
                            crate::hir::PlaceProjection::VariantField { case, field } => {
                                case.as_str().len().saturating_add(field.as_str().len())
                            }
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
        }
    }
    Ok(total)
}

fn hir_function_owned_capacity(function: &ResolvedFunction) -> Result<usize, Diagnostic> {
    let mut total = std::mem::size_of::<ResolvedFunction>()
        .checked_add(function.id.as_str().len())
        .and_then(|bytes| bytes.checked_add(function.result_id.as_str().len()))
        .and_then(|bytes| bytes.checked_add(function.name.capacity()))
        .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&function.return_type)?))
        .and_then(|bytes| {
            bytes.checked_add(
                function.params.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(function.effects.capacity() * std::mem::size_of::<String>())
        })
        .and_then(|bytes| {
            bytes.checked_add(function.requires.capacity() * std::mem::size_of::<ResolvedExpr>())
        })
        .and_then(|bytes| {
            bytes.checked_add(function.ensures.capacity() * std::mem::size_of::<ResolvedExpr>())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for parameter in &function.params {
        total = total
            .checked_add(parameter.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(parameter.name.capacity()))
            .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&parameter.ty)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for effect in &function.effects {
        total = total
            .checked_add(effect.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        total = total
            .checked_add(hir_expr_owned_capacity(expression)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    total = total
        .checked_add(
            crate::private_capacity_contract::cleanup_inventory_owned_capacity(&function.cleanup)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                crate::private_capacity_contract::cleanup_plan_owned_capacity(
                    &function.cleanup_plan,
                )?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(total)
}

fn hir_owned_capacity(resolved: &ResolvedProgram) -> Result<usize, Diagnostic> {
    // `declaration_index_upper` separately owns the opaque index's inline
    // header and heap payload. Avoid charging its inline bytes twice here.
    let mut total = (std::mem::size_of::<ResolvedProgram>()
        - std::mem::size_of::<crate::hir::DeclarationIndex>())
    .checked_add(resolved.module.capacity())
    .and_then(|bytes| bytes.checked_add(resolved.entrypoint.as_str().len()))
    .and_then(|bytes| {
        bytes.checked_add(resolved.permits.capacity() * std::mem::size_of::<String>())
    })
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for permit in &resolved.permits {
        total = total
            .checked_add(permit.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    total = total
        .checked_add(resolved.functions.capacity() * std::mem::size_of::<ResolvedFunction>())
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.interfaces.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedInterface>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.types.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedTypeDeclaration>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.function_templates.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedFunctionTemplate>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.function_instances.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedFunctionInstance>(),
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for interface in &resolved.interfaces {
        total = total
            .checked_add(interface.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(interface.name.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(interface.permits.capacity() * std::mem::size_of::<String>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    interface.imports.capacity()
                        * std::mem::size_of::<crate::hir::ResolvedImport>(),
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for permit in &interface.permits {
            total = total
                .checked_add(permit.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for import in &interface.imports {
            total = total
                .checked_add(import.id.as_str().len())
                .and_then(|bytes| bytes.checked_add(import.interface.as_str().len()))
                .and_then(|bytes| bytes.checked_add(import.name.capacity()))
                .and_then(|bytes| bytes.checked_add(import.import_key.capacity()))
                .and_then(|bytes| {
                    bytes.checked_add(
                        import.parameters.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedImportParameter>(),
                    )
                })
                .and_then(|bytes| {
                    bytes.checked_add(import.effects.capacity() * std::mem::size_of::<String>())
                })
                .and_then(|bytes| {
                    bytes.checked_add(
                        import.required_authority.capacity() * std::mem::size_of::<String>(),
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            for parameter in &import.parameters {
                total = total
                    .checked_add(parameter.name.capacity())
                    .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&parameter.ty)?))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            for value in import.effects.iter().chain(&import.required_authority) {
                total = total
                    .checked_add(value.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            if let ResolvedImportFailure::Status { domain_id, .. } = &import.failure {
                total = total
                    .checked_add(domain_id.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
        }
    }
    for function in &resolved.functions {
        // The outer function vector already accounts for each inline struct;
        // add only its recursively owned payload.
        let whole_function = hir_function_owned_capacity(function)?;
        total = total
            .checked_add(whole_function.saturating_sub(std::mem::size_of::<ResolvedFunction>()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for declaration in &resolved.types {
        total = total
            .checked_add(declaration.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(declaration.name.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(
                    declaration.type_parameters.capacity()
                        * std::mem::size_of::<crate::hir::ResolvedTypeParameterDeclaration>(),
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for parameter in &declaration.type_parameters {
            total = total
                .checked_add(parameter.name.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        match &declaration.kind {
            crate::hir::ResolvedTypeDeclarationKind::Resource { drop } => {
                total = total
                    .checked_add(drop.id.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if let crate::hir::ResolvedResourceDropKind::Imported { import, import_key } =
                    &drop.kind
                {
                    total = total
                        .checked_add(import.as_str().len())
                        .and_then(|bytes| bytes.checked_add(import_key.capacity()))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Record { fields } => {
                total = total
                    .checked_add(
                        fields.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>(),
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for field in fields {
                    total = total
                        .checked_add(field.id.as_str().len())
                        .and_then(|bytes| bytes.checked_add(field.name.capacity()))
                        .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&field.ty)?))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                total = total
                    .checked_add(
                        cases.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedVariantCaseDeclaration>(),
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for case in cases {
                    total = total
                        .checked_add(case.id.as_str().len())
                        .and_then(|bytes| bytes.checked_add(case.name.capacity()))
                        .and_then(|bytes| {
                            bytes.checked_add(
                                case.fields.capacity()
                                    * std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>(),
                            )
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    for field in &case.fields {
                        total = total
                            .checked_add(field.id.as_str().len())
                            .and_then(|bytes| bytes.checked_add(field.name.capacity()))
                            .and_then(|bytes| {
                                bytes.checked_add(hir_type_owned_capacity(&field.ty)?)
                            })
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
            }
        }
    }
    for template in &resolved.function_templates {
        total = total
            .checked_add(template.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(template.result_id.as_str().len()))
            .and_then(|bytes| bytes.checked_add(template.name.capacity()))
            .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&template.return_type)?))
            .and_then(|bytes| {
                bytes.checked_add(
                    template.type_parameters.capacity()
                        * std::mem::size_of::<crate::hir::ResolvedTypeParameterDeclaration>(),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    template.params.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>(),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(template.effects.capacity() * std::mem::size_of::<String>())
            })
            .and_then(|bytes| {
                bytes
                    .checked_add(template.requires.capacity() * std::mem::size_of::<ResolvedExpr>())
            })
            .and_then(|bytes| {
                bytes.checked_add(template.ensures.capacity() * std::mem::size_of::<ResolvedExpr>())
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for parameter in &template.type_parameters {
            total = total
                .checked_add(parameter.name.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for parameter in &template.params {
            total = total
                .checked_add(parameter.id.as_str().len())
                .and_then(|bytes| bytes.checked_add(parameter.name.capacity()))
                .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&parameter.ty)?))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for effect in &template.effects {
            total = total
                .checked_add(effect.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            total = total
                .checked_add(hir_expr_owned_capacity(expression)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
    }
    for instance in &resolved.function_instances {
        total = total
            .checked_add(instance.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(instance.template.as_str().len()))
            .and_then(|bytes| {
                bytes.checked_add(
                    instance.type_arguments.capacity() * std::mem::size_of::<ResolvedType>(),
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for ty in &instance.type_arguments {
            total = total
                .checked_add(
                    hir_type_owned_capacity(ty)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        total = total
            .checked_add(hir_function_owned_capacity(&instance.function)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(total)
}

#[cfg(test)]
fn validate_native_rust_expression_budget(resolved: &ResolvedProgram) -> Result<(), Diagnostic> {
    let functions = resolved.functions.iter().collect::<Vec<_>>();
    validate_native_rust_expression_budget_for_closure(&functions, false)
}

fn validate_native_rust_expression_budget_for_closure(
    functions: &[&ResolvedFunction],
    preauthorized: bool,
) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(1);
    let mut pending = Vec::new();
    for function in functions {
        pending.extend(
            function
                .requires
                .iter()
                .map(|expression| (expression, 1_usize)),
        );
        pending.push((&function.body, 1));
        pending.extend(
            function
                .ensures
                .iter()
                .map(|expression| (expression, 1_usize)),
        );
    }
    let mut visited = 0_usize;
    while let Some((expression, depth)) = pending.pop() {
        note_hir_post_resolve_capacity(
            0,
            pending.capacity() * std::mem::size_of::<(&ResolvedExpr, usize)>(),
        );
        visited = visited
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if !preauthorized {
            debit(std::mem::size_of::<&ResolvedExpr>())?;
        }
        if visited > MAX_SOURCE_BYTES {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
            return Err(b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            ));
        }
        let child_depth = depth
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                pending.extend(args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                pending.extend(call.args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. } => pending.push((value, child_depth)),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push((left, child_depth));
                pending.push((right, child_depth));
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let ResolvedStatement::Let { value, .. } = statement;
                    pending.push((value, child_depth));
                }
                pending.push((tail, child_depth));
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push((condition, child_depth));
                pending.push((then_branch, child_depth));
                pending.push((else_branch, child_depth));
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| (&field.value, child_depth)));
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                pending.push((scrutinee, child_depth));
                pending.extend(arms.iter().map(|arm| (&arm.value, child_depth)));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push((base, child_depth));
                pending.extend(fields.iter().map(|field| (&field.value, child_depth)));
            }
            ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        }
    }
    Ok(())
}

fn parameter_json(parameter: &ParameterFact) -> String {
    format!(
        "{{\"name\":{},\"type\":{},\"mode\":\"value\"}}",
        quote_json(&parameter.name),
        quote_json(scalar_text(parameter.ty))
    )
}

fn result_json(result: ScalarType) -> String {
    format!(
        "{{\"type\":{},\"out_slot\":{}}}",
        quote_json(scalar_text(result)),
        result != ScalarType::Unit
    )
}

struct ExactReplay<'a> {
    source: &'a [u8],
    position: usize,
    failed: bool,
}

impl<'a> ExactReplay<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            position: 0,
            failed: false,
        }
    }

    fn text(&mut self, expected: &str) {
        let end = self.position.checked_add(expected.len());
        let mismatch = end.is_none_or(|end| end > self.source.len())
            || end.is_some_and(|end| &self.source[self.position..end] != expected.as_bytes());
        if self.failed || mismatch {
            self.failed = true;
            return;
        }
        self.position = end.unwrap_or(self.position);
    }

    fn json(&mut self, value: &str) {
        self.text("\"");
        for character in value.chars() {
            match character {
                '\"' => self.text("\\\""),
                '\\' => self.text("\\\\"),
                '\u{08}' => self.text("\\b"),
                '\t' => self.text("\\t"),
                '\n' => self.text("\\n"),
                '\u{0c}' => self.text("\\f"),
                '\r' => self.text("\\r"),
                character if character <= '\u{1f}' => {
                    let code = u32::from(character);
                    let hex = b"0123456789abcdef";
                    let escaped = [
                        b'\\',
                        b'u',
                        hex[((code >> 12) & 0xf) as usize],
                        hex[((code >> 8) & 0xf) as usize],
                        hex[((code >> 4) & 0xf) as usize],
                        hex[(code & 0xf) as usize],
                    ];
                    self.text(std::str::from_utf8(&escaped).unwrap_or(""));
                }
                character => {
                    let mut encoded = [0_u8; 4];
                    self.text(character.encode_utf8(&mut encoded));
                }
            }
        }
        self.text("\"");
    }

    fn number(&mut self, value: impl std::fmt::Display) {
        let rendered = value.to_string();
        #[cfg(test)]
        note_post_hir_replay_capacity(rendered.capacity());
        self.text(&rendered);
    }

    fn usize_noalloc(&mut self, mut value: usize) {
        let mut bytes = [0_u8; 20];
        let mut start = bytes.len();
        loop {
            start -= 1;
            bytes[start] = b'0' + u8::try_from(value % 10).unwrap_or(0);
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.text(std::str::from_utf8(&bytes[start..]).unwrap_or(""));
    }

    fn raw_digest_json_noalloc(&mut self, bytes: &[u8]) {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        self.text("\"sha256:");
        for byte in Sha256::digest(bytes) {
            let pair = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]];
            self.text(std::str::from_utf8(&pair).unwrap_or(""));
        }
        self.text("\"");
    }

    fn finish(self) -> bool {
        !self.failed && self.position == self.source.len()
    }
}

fn replay_limits_exact(replay: &mut ExactReplay<'_>) {
    replay.text("{");
    for (index, (name, value)) in LIMIT_ROWS.into_iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(name);
        replay.text(":");
        replay.usize_noalloc(value);
    }
    replay.text("}");
}

fn replay_spec_bytes_exact(source: &str, spec: &Spec) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("{\"schema\":");
    replay.json(SPEC_SCHEMA);
    replay.text(",\"module\":");
    replay.json(&spec.module);
    replay.text(",\"source_revision\":");
    replay.json(&spec.source_revision);
    replay.text(",\"target\":{\"triple\":");
    replay.json(&spec.target.triple);
    replay.text(",\"pointer_width\":");
    replay.number(spec.target.pointer_width);
    replay.text(",\"endian\":");
    replay.json(&spec.target.endian);
    replay.text(",\"panic_strategy\":");
    replay.json(&spec.target.panic_strategy);
    replay.text(",\"thread_policy\":");
    replay.json(&spec.target.thread_policy);
    replay.text("},\"exports\":[");
    replay_strings_exact(&mut replay, &spec.exports);
    replay.text("],\"imports\":[");
    replay_strings_exact(&mut replay, &spec.imports);
    replay.text("],\"capabilities\":[");
    replay_strings_exact(&mut replay, &spec.capabilities);
    replay.text("],\"limits\":");
    replay_limits_exact(&mut replay);
    replay.text(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(nonclaim);
    }
    replay.text("]}\n");
    replay.finish()
}

fn replay_parameter_exact(replay: &mut ExactReplay<'_>, parameter: &ParameterFact) {
    replay.text("{\"name\":");
    replay.json(&parameter.name);
    replay.text(",\"type\":");
    replay.json(scalar_text(parameter.ty));
    replay.text(",\"mode\":\"value\"}");
}

fn replay_result_exact(replay: &mut ExactReplay<'_>, result: ScalarType) {
    replay.text("{\"type\":");
    replay.json(scalar_text(result));
    replay.text(",\"out_slot\":");
    replay.text(if result == ScalarType::Unit {
        "false"
    } else {
        "true"
    });
    replay.text("}");
}

fn replay_strings_exact(replay: &mut ExactReplay<'_>, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(value);
    }
}

fn replay_descriptor_bytes_exact(
    source: &str,
    spec: &Spec,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("{\"schema\":");
    replay.json(DESCRIPTOR_SCHEMA);
    replay.text(",\"module\":");
    replay.json(&spec.module);
    replay.text(",\"source_revision\":");
    replay.json(&spec.source_revision);
    replay.text(",\"hir_digest\":");
    replay.json(hir_digest);
    replay.text(",\"target\":{\"triple\":");
    replay.json(&spec.target.triple);
    replay.text(",\"pointer_width\":");
    replay.number(spec.target.pointer_width);
    replay.text(",\"endian\":");
    replay.json(&spec.target.endian);
    replay.text(",\"panic_strategy\":");
    replay.json(&spec.target.panic_strategy);
    replay.text(",\"thread_policy\":");
    replay.json(&spec.target.thread_policy);
    replay.text("},\"status_domains\":[{\"ordinal\":0,\"domain_id\":\"success\"}");
    for (index, domain) in status_domains.iter().enumerate() {
        replay.text(",{\"ordinal\":");
        replay.number(index + 1);
        replay.text(",\"domain_id\":");
        replay.json(domain);
        replay.text("}");
    }
    replay.text(r#",{"ordinal":65533,"domain_id":"semaprax.native-rust-semantics.v1"},{"ordinal":65534,"domain_id":"semaprax.native-rust-host.v1"},{"ordinal":65535,"domain_id":"semaprax.native-rust-adapter.v1"}],"abi":{"version":1,"calling_convention":"C","status_word":"u64-domain16-code32-class8-retry1-reserved7","bool":"u8-0-or-1","i64":"signed-two-complement-i64","context":"SPXNRCTX1","imports_table":"SPXNRIMP1","result":"caller-owned-uninitialized-success-only","allocator":"none-across-boundary","unwind":"caught-before-ffi-return","threading":"same-thread","reentrancy":"rejected"},"exports":["#);
    for (index, export) in exports.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.text("{\"id\":");
        replay.json(&export.id);
        replay.text(",\"rust_method\":");
        replay.json(&export.rust_method);
        replay.text(",\"c_symbol\":");
        replay.json(&export.c_symbol);
        replay.text(",\"parameters\":[");
        for (parameter_index, parameter) in export.parameters.iter().enumerate() {
            if parameter_index != 0 {
                replay.text(",");
            }
            replay_parameter_exact(&mut replay, parameter);
        }
        replay.text("],\"result\":");
        replay_result_exact(&mut replay, export.result);
        replay.text(",\"effects\":[");
        replay_strings_exact(&mut replay, &export.effects);
        replay.text("],\"capabilities\":[");
        replay_strings_exact(&mut replay, &export.capabilities);
        replay.text("],\"required_imports\":[");
        replay_strings_exact(&mut replay, &export.required_imports);
        replay.text("],\"status_domain_ordinals\":[");
        for (ordinal_index, ordinal) in export.status_domain_ordinals.iter().enumerate() {
            if ordinal_index != 0 {
                replay.text(",");
            }
            replay.number(ordinal);
        }
        replay.text("],\"call_contract_digest\":");
        replay.json(&export.call_contract_digest);
        replay.text("}");
    }
    replay.text("],\"imports\":[");
    for (index, import) in imports.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.text("{\"id\":");
        replay.json(&import.id);
        replay.text(",\"interface\":");
        replay.json(&import.interface);
        replay.text(",\"import_key\":");
        replay.json(&import.import_key);
        replay.text(",\"rust_method\":");
        replay.json(&import.rust_method);
        replay.text(",\"c_field\":");
        replay.json(&import.c_field);
        replay.text(",\"parameters\":[");
        for (parameter_index, parameter) in import.parameters.iter().enumerate() {
            if parameter_index != 0 {
                replay.text(",");
            }
            replay_parameter_exact(&mut replay, parameter);
        }
        replay.text("],\"result\":");
        replay_result_exact(&mut replay, import.result);
        replay.text(",\"effects\":[");
        replay_strings_exact(&mut replay, &import.effects);
        replay.text("],\"capabilities\":[");
        replay_strings_exact(&mut replay, &import.capabilities);
        replay.text("],\"failure\":{\"kind\":");
        if let Some(domain) = &import.failure {
            replay.text("\"status\",\"domain_id\":");
            replay.json(domain);
        } else {
            replay.text("\"infallible\"");
        }
        replay.text("},\"call_contract_digest\":");
        replay.json(&import.call_contract_digest);
        replay.text("}");
    }
    replay.text("],\"limits\":");
    replay_limits_exact(&mut replay);
    replay.text(",\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(nonclaim);
    }
    replay.text("]}\n");
    replay.finish()
}

fn render_descriptor_with_limit(
    spec: &Spec,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<String, Diagnostic> {
    let mut statuses = vec!["{\"ordinal\":0,\"domain_id\":\"success\"}".to_owned()];
    statuses.extend(status_domains.iter().enumerate().map(|(index, domain)| {
        format!(
            "{{\"ordinal\":{},\"domain_id\":{}}}",
            index + 1,
            quote_json(domain)
        )
    }));
    statuses
        .push("{\"ordinal\":65533,\"domain_id\":\"semaprax.native-rust-semantics.v1\"}".to_owned());
    statuses.push("{\"ordinal\":65534,\"domain_id\":\"semaprax.native-rust-host.v1\"}".to_owned());
    statuses
        .push("{\"ordinal\":65535,\"domain_id\":\"semaprax.native-rust-adapter.v1\"}".to_owned());
    #[cfg(test)]
    let status_scratch = checked_owned_string_vec(&statuses, statuses.capacity())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut export_row_values = Vec::with_capacity(exports.len());
    for export in exports {
        let id = quote_json(&export.id);
        let rust_method = quote_json(&export.rust_method);
        let c_symbol = quote_json(&export.c_symbol);
        let parameter_values = export
            .parameters
            .iter()
            .map(parameter_json)
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let effects = render_string_array(&export.effects);
        let capabilities = render_string_array(&export.capabilities);
        let required_imports = render_string_array(&export.required_imports);
        let ordinal_values = export
            .status_domain_ordinals
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>();
        let ordinals = ordinal_values.join(",");
        let result = result_json(export.result);
        let call_contract_digest = quote_json(&export.call_contract_digest);
        let row = format!(
                "{{\"id\":{},\"rust_method\":{},\"c_symbol\":{},\"parameters\":[{}],\"result\":{},\"effects\":[{}],\"capabilities\":[{}],\"required_imports\":[{}],\"status_domain_ordinals\":[{}],\"call_contract_digest\":{}}}",
                id,
                rust_method,
                c_symbol,
                parameters,
                result,
                effects,
                capabilities,
                required_imports,
                ordinals,
                call_contract_digest
            );
        #[cfg(test)]
        note_post_hir_render_capacity(
            status_scratch
                .saturating_add(
                    checked_owned_string_vec(&export_row_values, export_row_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(
                    checked_owned_string_vec(&parameter_values, parameter_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(
                    checked_owned_string_vec(&ordinal_values, ordinal_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(id.capacity())
                .saturating_add(rust_method.capacity())
                .saturating_add(c_symbol.capacity())
                .saturating_add(parameters.capacity())
                .saturating_add(effects.capacity())
                .saturating_add(capabilities.capacity())
                .saturating_add(required_imports.capacity())
                .saturating_add(ordinals.capacity())
                .saturating_add(result.capacity())
                .saturating_add(call_contract_digest.capacity())
                .saturating_add(row.capacity()),
        );
        export_row_values.push(row);
    }
    let export_rows = export_row_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        status_scratch
            .saturating_add(
                checked_owned_string_vec(&export_row_values, export_row_values.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(export_rows.capacity()),
    );
    drop(export_row_values);
    let mut import_row_values = Vec::with_capacity(imports.len());
    for import in imports {
        let id = quote_json(&import.id);
        let interface = quote_json(&import.interface);
        let import_key = quote_json(&import.import_key);
        let rust_method = quote_json(&import.rust_method);
        let c_field = quote_json(&import.c_field);
        let parameter_values = import
            .parameters
            .iter()
            .map(parameter_json)
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let effects = render_string_array(&import.effects);
        let capabilities = render_string_array(&import.capabilities);
        let failure = import.failure.as_ref().map_or_else(
            || "{\"kind\":\"infallible\"}".to_owned(),
            |domain| {
                format!(
                    "{{\"kind\":\"status\",\"domain_id\":{}}}",
                    quote_json(domain)
                )
            },
        );
        let result = result_json(import.result);
        let call_contract_digest = quote_json(&import.call_contract_digest);
        let row = format!(
                "{{\"id\":{},\"interface\":{},\"import_key\":{},\"rust_method\":{},\"c_field\":{},\"parameters\":[{}],\"result\":{},\"effects\":[{}],\"capabilities\":[{}],\"failure\":{},\"call_contract_digest\":{}}}",
                id,
                interface,
                import_key,
                rust_method,
                c_field,
                parameters,
                result,
                effects,
                capabilities,
                failure,
                call_contract_digest
            );
        #[cfg(test)]
        note_post_hir_render_capacity(
            status_scratch
                .saturating_add(export_rows.capacity())
                .saturating_add(
                    checked_owned_string_vec(&import_row_values, import_row_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(
                    checked_owned_string_vec(&parameter_values, parameter_values.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .saturating_add(id.capacity())
                .saturating_add(interface.capacity())
                .saturating_add(import_key.capacity())
                .saturating_add(rust_method.capacity())
                .saturating_add(c_field.capacity())
                .saturating_add(parameters.capacity())
                .saturating_add(effects.capacity())
                .saturating_add(capabilities.capacity())
                .saturating_add(failure.capacity())
                .saturating_add(result.capacity())
                .saturating_add(call_contract_digest.capacity())
                .saturating_add(row.capacity()),
        );
        import_row_values.push(row);
    }
    let import_rows = import_row_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        status_scratch
            .saturating_add(export_rows.capacity())
            .saturating_add(
                checked_owned_string_vec(&import_row_values, import_row_values.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .saturating_add(import_rows.capacity()),
    );
    drop(import_row_values);
    let schema = quote_json(DESCRIPTOR_SCHEMA);
    let module = quote_json(&spec.module);
    let source_revision = quote_json(&spec.source_revision);
    let hir = quote_json(hir_digest);
    let target = target_json(&spec.target);
    let status_rows = statuses.join(",");
    let limits = limits_json();
    let nonclaims = nonclaims_json();
    #[cfg(test)]
    note_post_hir_render_capacity(
        checked_owned_string_vec(&statuses, statuses.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
            .saturating_add(export_rows.capacity())
            .saturating_add(import_rows.capacity())
            .saturating_add(schema.capacity())
            .saturating_add(module.capacity())
            .saturating_add(source_revision.capacity())
            .saturating_add(hir.capacity())
            .saturating_add(target.capacity())
            .saturating_add(status_rows.capacity())
            .saturating_add(limits.capacity())
            .saturating_add(nonclaims.capacity()),
    );
    render_exact_artifact("max_descriptor_bytes", maximum, |sink| {
        write!(
            sink,
            "{{\"schema\":{},\"module\":{},\"source_revision\":{},\"hir_digest\":{},\"target\":{},\"status_domains\":[{}],\"abi\":{{\"version\":1,\"calling_convention\":\"C\",\"status_word\":\"u64-domain16-code32-class8-retry1-reserved7\",\"bool\":\"u8-0-or-1\",\"i64\":\"signed-two-complement-i64\",\"context\":\"SPXNRCTX1\",\"imports_table\":\"SPXNRIMP1\",\"result\":\"caller-owned-uninitialized-success-only\",\"allocator\":\"none-across-boundary\",\"unwind\":\"caught-before-ffi-return\",\"threading\":\"same-thread\",\"reentrancy\":\"rejected\"}},\"exports\":[{}],\"imports\":[{}],\"limits\":{},\"nonclaims\":[{}]}}\n",
            schema,
            module,
            source_revision,
            hir,
            target,
            status_rows,
            export_rows,
            import_rows,
            limits,
            nonclaims
        )
        .map_err(|_| b109("max_descriptor_bytes", MAX_DESCRIPTOR_BYTES))
    })
}

fn render_descriptor(
    spec: &Spec,
    hir_digest: &str,
    status_domains: &[String],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_descriptor_with_limit(
        spec,
        hir_digest,
        status_domains,
        exports,
        imports,
        MAX_DESCRIPTOR_BYTES,
    )
}

fn replay_descriptor(
    source: &str,
    spec: &Spec,
    hir_digest: &str,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let status_domain_set = imports
        .iter()
        .filter_map(|import| import.failure.clone())
        .collect::<BTreeSet<_>>();
    #[cfg(test)]
    let status_domain_set_owned = checked_owned_string_set(&status_domain_set)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_replay_capacity(status_domain_set_owned);
    let mut status_domains = Vec::with_capacity(status_domain_set.len());
    for domain in status_domain_set {
        status_domains.push(domain);
        #[cfg(test)]
        note_post_hir_replay_capacity(
            status_domain_set_owned
                .checked_add(
                    checked_owned_string_vec(&status_domains, status_domains.capacity())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        );
    }
    if !replay_descriptor_bytes_exact(source, spec, hir_digest, &status_domains, exports, imports) {
        return Err(b108());
    }
    if !source.ends_with('\n') {
        return Err(b108());
    }
    let value: Value = serde_json::from_str(source).map_err(|_| b108())?;
    #[cfg(test)]
    let descriptor_dom_owned = checked_json_value_owned(&value)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let status_domains_owned = checked_owned_string_vec(&status_domains, status_domains.capacity())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    note_post_hir_replay_capacity(
        descriptor_dom_owned
            .checked_add(status_domains_owned)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    let row = value.as_object().ok_or_else(b108)?;
    if row.len() != 11
        || row.get("schema").and_then(Value::as_str) != Some(DESCRIPTOR_SCHEMA)
        || row.get("module").and_then(Value::as_str) != Some(&spec.module)
        || row.get("source_revision").and_then(Value::as_str) != Some(&spec.source_revision)
        || row.get("hir_digest").and_then(Value::as_str) != Some(hir_digest)
        || row.get("exports").and_then(Value::as_array).map(Vec::len) != Some(exports.len())
        || row.get("imports").and_then(Value::as_array).map(Vec::len) != Some(imports.len())
    {
        return Err(b108());
    }
    let target = row
        .get("target")
        .and_then(Value::as_object)
        .ok_or_else(b108)?;
    if target.len() != 5
        || target.get("triple").and_then(Value::as_str) != Some(&spec.target.triple)
        || target.get("pointer_width").and_then(Value::as_u64)
            != Some(u64::from(spec.target.pointer_width))
        || target.get("endian").and_then(Value::as_str) != Some(&spec.target.endian)
        || target.get("panic_strategy").and_then(Value::as_str) != Some(&spec.target.panic_strategy)
        || target.get("thread_policy").and_then(Value::as_str) != Some(&spec.target.thread_policy)
    {
        return Err(b108());
    }
    let expected_statuses = std::iter::once((0_u64, "success"))
        .chain(status_domains.iter().enumerate().map(|(index, domain)| {
            (
                u64::try_from(index + 1).unwrap_or(u64::MAX),
                domain.as_str(),
            )
        }))
        .chain([
            (65_533, "semaprax.native-rust-semantics.v1"),
            (65_534, "semaprax.native-rust-host.v1"),
            (65_535, "semaprax.native-rust-adapter.v1"),
        ])
        .collect::<Vec<_>>();
    #[cfg(test)]
    note_post_hir_replay_capacity(
        descriptor_dom_owned
            .checked_add(status_domains_owned)
            .and_then(|bytes| {
                bytes.checked_add(
                    expected_statuses
                        .capacity()
                        .checked_mul(std::mem::size_of::<(u64, &str)>())?,
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
    );
    let statuses = row
        .get("status_domains")
        .and_then(Value::as_array)
        .ok_or_else(b108)?;
    if statuses.len() != expected_statuses.len()
        || statuses
            .iter()
            .zip(&expected_statuses)
            .any(|(value, expected)| {
                value.as_object().is_none_or(|object| {
                    object.len() != 2
                        || object.get("ordinal").and_then(Value::as_u64) != Some(expected.0)
                        || object.get("domain_id").and_then(Value::as_str) != Some(expected.1)
                })
            })
    {
        return Err(b108());
    }
    let abi = row.get("abi").and_then(Value::as_object).ok_or_else(b108)?;
    for (key, expected) in [
        ("calling_convention", "C"),
        ("status_word", "u64-domain16-code32-class8-retry1-reserved7"),
        ("bool", "u8-0-or-1"),
        ("i64", "signed-two-complement-i64"),
        ("context", "SPXNRCTX1"),
        ("imports_table", "SPXNRIMP1"),
        ("result", "caller-owned-uninitialized-success-only"),
        ("allocator", "none-across-boundary"),
        ("unwind", "caught-before-ffi-return"),
        ("threading", "same-thread"),
        ("reentrancy", "rejected"),
    ] {
        if abi.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(b108());
        }
    }
    if abi.len() != 12 || abi.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(b108());
    }
    validate_descriptor_exports(row.get("exports").ok_or_else(b108)?, exports)?;
    validate_descriptor_imports(row.get("imports").ok_or_else(b108)?, imports)?;
    let limits = row
        .get("limits")
        .and_then(Value::as_object)
        .ok_or_else(b108)?;
    if limits.len() != LIMIT_ROWS.len()
        || LIMIT_ROWS.iter().any(|(name, expected)| {
            limits.get(*name).and_then(Value::as_u64) != u64::try_from(*expected).ok()
        })
        || row
            .get("nonclaims")
            .and_then(Value::as_array)
            .is_none_or(|values| {
                values.len() != NONCLAIMS.len()
                    || values
                        .iter()
                        .zip(NONCLAIMS)
                        .any(|(value, expected)| value.as_str() != Some(*expected))
            })
    {
        return Err(b108());
    }
    Ok(())
}

fn validate_parameter_values(value: &Value, expected: &[ParameterFact]) -> Result<(), Diagnostic> {
    let values = value.as_array().ok_or_else(b108)?;
    if values.len() != expected.len() {
        return Err(b108());
    }
    for (value, expected) in values.iter().zip(expected) {
        let row = value.as_object().ok_or_else(b108)?;
        if row.len() != 3
            || row.get("name").and_then(Value::as_str) != Some(&expected.name)
            || row.get("type").and_then(Value::as_str) != Some(scalar_text(expected.ty))
            || row.get("mode").and_then(Value::as_str) != Some("value")
        {
            return Err(b108());
        }
    }
    Ok(())
}

fn validate_result_value(value: &Value, expected: ScalarType) -> Result<(), Diagnostic> {
    let row = value.as_object().ok_or_else(b108)?;
    if row.len() != 2
        || row.get("type").and_then(Value::as_str) != Some(scalar_text(expected))
        || row.get("out_slot").and_then(Value::as_bool) != Some(expected != ScalarType::Unit)
    {
        return Err(b108());
    }
    Ok(())
}

fn strings_equal(value: Option<&Value>, expected: &[String]) -> bool {
    value.and_then(Value::as_array).is_some_and(|values| {
        values.len() == expected.len()
            && values
                .iter()
                .zip(expected)
                .all(|(value, expected)| value.as_str() == Some(expected))
    })
}

fn validate_descriptor_exports(value: &Value, expected: &[ExportFact]) -> Result<(), Diagnostic> {
    let rows = value.as_array().ok_or_else(b108)?;
    if rows.len() != expected.len() {
        return Err(b108());
    }
    for (value, expected) in rows.iter().zip(expected) {
        let row = value.as_object().ok_or_else(b108)?;
        validate_parameter_values(
            row.get("parameters").ok_or_else(b108)?,
            &expected.parameters,
        )?;
        validate_result_value(row.get("result").ok_or_else(b108)?, expected.result)?;
        if row.len() != 10
            || row.get("id").and_then(Value::as_str) != Some(&expected.id)
            || row.get("rust_method").and_then(Value::as_str) != Some(&expected.rust_method)
            || row.get("c_symbol").and_then(Value::as_str) != Some(&expected.c_symbol)
            || !strings_equal(row.get("effects"), &expected.effects)
            || !strings_equal(row.get("capabilities"), &expected.capabilities)
            || !strings_equal(row.get("required_imports"), &expected.required_imports)
            || row
                .get("status_domain_ordinals")
                .and_then(Value::as_array)
                .is_none_or(|values| {
                    values.len() != expected.status_domain_ordinals.len()
                        || values
                            .iter()
                            .zip(&expected.status_domain_ordinals)
                            .any(|(value, expected)| value.as_u64() != Some(u64::from(*expected)))
                })
            || row.get("call_contract_digest").and_then(Value::as_str)
                != Some(&expected.call_contract_digest)
        {
            return Err(b108());
        }
    }
    Ok(())
}

fn validate_descriptor_imports(value: &Value, expected: &[ImportFact]) -> Result<(), Diagnostic> {
    let rows = value.as_array().ok_or_else(b108)?;
    if rows.len() != expected.len() {
        return Err(b108());
    }
    for (value, expected) in rows.iter().zip(expected) {
        let row = value.as_object().ok_or_else(b108)?;
        validate_parameter_values(
            row.get("parameters").ok_or_else(b108)?,
            &expected.parameters,
        )?;
        validate_result_value(row.get("result").ok_or_else(b108)?, expected.result)?;
        let failure = row
            .get("failure")
            .and_then(Value::as_object)
            .ok_or_else(b108)?;
        let valid_failure = expected.failure.as_ref().map_or_else(
            || {
                failure.len() == 1
                    && failure.get("kind").and_then(Value::as_str) == Some("infallible")
            },
            |domain| {
                failure.len() == 2
                    && failure.get("kind").and_then(Value::as_str) == Some("status")
                    && failure.get("domain_id").and_then(Value::as_str) == Some(domain)
            },
        );
        if row.len() != 11
            || row.get("id").and_then(Value::as_str) != Some(&expected.id)
            || row.get("interface").and_then(Value::as_str) != Some(&expected.interface)
            || row.get("import_key").and_then(Value::as_str) != Some(&expected.import_key)
            || row.get("rust_method").and_then(Value::as_str) != Some(&expected.rust_method)
            || row.get("c_field").and_then(Value::as_str) != Some(&expected.c_field)
            || !strings_equal(row.get("effects"), &expected.effects)
            || !strings_equal(row.get("capabilities"), &expected.capabilities)
            || !valid_failure
            || row.get("call_contract_digest").and_then(Value::as_str)
                != Some(&expected.call_contract_digest)
        {
            return Err(b108());
        }
    }
    Ok(())
}

fn c_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("{} arg_{index}", c_type(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

fn generate_header_with_limit(
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<String, Diagnostic> {
    let mut import_rows = Vec::with_capacity(imports.len());
    for import in imports {
        let params = c_parameters(&import.parameters);
        let out = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", {} *result_out", c_type(import.result))
        };
        let row = format!(
            " spxnr_status_v1 (*{})(void *userdata{}{}{});",
            import.c_field,
            if params.is_empty() { "" } else { ", " },
            params,
            out
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&import_rows)
                .saturating_add(params.capacity())
                .saturating_add(out.capacity())
                .saturating_add(row.capacity()),
        );
        import_rows.push(row);
    }
    let mut export_rows = Vec::with_capacity(exports.len());
    for export in exports {
        let params = c_parameters(&export.parameters);
        let out = if export.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", {} *result_out", c_type(export.result))
        };
        let row = format!(
            "spxnr_status_v1 {}(const spxnr_context_v1 *ctx{}{}{});\n",
            export.c_symbol,
            if params.is_empty() { "" } else { ", " },
            params,
            out
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&import_rows)
                .saturating_add(string_slice_owned_capacity(&export_rows))
                .saturating_add(params.capacity())
                .saturating_add(out.capacity())
                .saturating_add(row.capacity()),
        );
        export_rows.push(row);
    }
    render_exact_artifact("max_generated_header_bytes", maximum, |sink| {
        sink.write_str(
                "#ifndef SEMAPRAX_NATIVE_RUST_INTEROP_H\n#define SEMAPRAX_NATIVE_RUST_INTEROP_H\n#include <stdint.h>\n#include <stddef.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint64_t spxnr_status_v1;\ntypedef struct spxnr_imports_v1 spxnr_imports_v1;\ntypedef struct { uint32_t abi_version; uint32_t size; void *userdata; const spxnr_imports_v1 *imports; uint8_t capabilities_digest[32]; uint32_t call_depth; uint32_t reserved; } spxnr_context_v1;\nstruct spxnr_imports_v1 { uint32_t abi_version; uint32_t size;",
            )
            .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        for row in &import_rows {
            sink.write_str(row)
                .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        }
        sink.write_str(" };\n")
            .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        for row in &export_rows {
            sink.write_str(row)
                .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))?;
        }
        sink.write_str("#ifdef __cplusplus\n}\n#endif\n#endif\n")
            .map_err(|_| b109("max_generated_header_bytes", MAX_GENERATED_HEADER_BYTES))
    })
}

fn generate_header(exports: &[ExportFact], imports: &[ImportFact]) -> Result<String, Diagnostic> {
    generate_header_with_limit(exports, imports, MAX_GENERATED_HEADER_BYTES)
}

#[derive(Clone, Copy)]
enum CExpressionMode {
    Generate,
    Replay,
}

enum CExpressionFrame<'a> {
    Enter(&'a ResolvedExpr),
    Unary(crate::ast::UnaryOp),
    BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    BinaryRight(crate::ast::BinaryOp, String),
    LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    LazyRight(String),
    Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType),
    IfThen(&'a ResolvedExpr, Option<String>),
    IfElse(Option<String>),
    NativeArgs(&'a crate::hir::ResolvedNativeRustImportCall, usize, usize),
    CallArgs(&'a str, &'a [ResolvedExpr], &'a ResolvedType, usize, usize),
}

// Intentionally separate from `CExpressionFrame`: exact replay must not share
// the generator's scheduling state or traversal implementation.
enum ReplayCExpressionFrame<'a> {
    Evaluate(&'a ResolvedExpr),
    FinishUnary(crate::ast::UnaryOp),
    FinishBinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    FinishBinary(crate::ast::BinaryOp, String),
    FinishLazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    FinishLazy(String),
    ContinueBlock(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishBinding(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType),
    FinishThen(&'a ResolvedExpr, Option<String>),
    FinishElse(Option<String>),
    ContinueNative(&'a crate::hir::ResolvedNativeRustImportCall, usize, usize),
    ContinueCall(&'a str, &'a [ResolvedExpr], &'a ResolvedType, usize, usize),
}

/// One fixed backing allocation owns every generated statement byte for one C
/// expression. The final C artifact has a separate reservation; this arena is
/// transient scratch and cannot grow geometrically past the admitted artifact
/// ceiling before the final-size gate observes it.
struct CExpressionLineArena {
    bytes: Box<[u8]>,
    len: usize,
}

impl CExpressionLineArena {
    fn new() -> Self {
        Self {
            bytes: vec![0; MAX_GENERATED_C_BYTES].into_boxed_slice(),
            len: 0,
        }
    }

    fn as_str(&self) -> Result<&str, Diagnostic> {
        std::str::from_utf8(&self.bytes[..self.len]).map_err(|_| b111())
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    #[cfg(test)]
    fn retained_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl std::fmt::Write for CExpressionLineArena {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(std::fmt::Error)?;
        let destination = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        destination.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn c_expression_hash(mode: CExpressionMode, value: &str) -> String {
    match mode {
        CExpressionMode::Generate => full_hash(value),
        CExpressionMode::Replay => replay_symbol_hash(value),
    }
}

fn c_expression_scalar(mode: CExpressionMode, value: ScalarType) -> &'static str {
    match mode {
        CExpressionMode::Generate => c_type(value),
        CExpressionMode::Replay => replay_c_scalar(value),
    }
}

fn c_expression_resolved_scalar(mode: CExpressionMode, value: &ResolvedType) -> Option<ScalarType> {
    match mode {
        CExpressionMode::Generate => scalar_type(value),
        CExpressionMode::Replay => replay_resolved_scalar(value),
    }
}

#[cfg(any())]
fn take_c_lines(lines: &mut Vec<String>) -> String {
    let bytes = lines.iter().map(String::len).sum();
    let mut joined = String::with_capacity(bytes);
    for line in lines.drain(..) {
        joined.push_str(&line);
    }
    joined
}

#[cfg(any())]
fn append_c_lines(output: &mut String, lines: &mut Vec<String>) {
    for line in lines.drain(..) {
        output.push_str(&line);
    }
}

#[cfg(any())]
fn move_root_c_lines(lines: &mut Vec<String>, contexts: &mut [Vec<String>]) {
    let mut root = std::mem::take(&mut contexts[0]);
    if lines.is_empty() {
        std::mem::swap(lines, &mut root);
    } else {
        lines.append(&mut root);
    }
}

fn c_expression_child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.get(index),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
        ResolvedExprKind::Unary { value, .. } => (index == 0).then_some(value),
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
        _ => None,
    }
}

fn c_expression_shape(expression: &ResolvedExpr) -> Result<(usize, usize), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    stack[0] = Some((expression, 0usize, 1usize));
    let mut stack_len = 1usize;
    let mut nodes = 0usize;
    let mut depth = 1usize;
    while stack_len > 0 {
        let (node, next_child, node_depth) = stack[stack_len - 1].take().ok_or_else(b111)?;
        stack_len -= 1;
        if next_child == 0 {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            depth = depth.max(node_depth);
        }
        if let Some(child) = c_expression_child(node, next_child) {
            if stack_len + 2 > stack.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            stack[stack_len] = Some((node, next_child + 1, node_depth));
            stack[stack_len + 1] = Some((child, 0, node_depth + 1));
            stack_len += 2;
        }
    }
    Ok((nodes, depth))
}

fn c_expression_frame_payload(frame: &CExpressionFrame<'_>) -> usize {
    match frame {
        CExpressionFrame::BinaryRight(_, value) | CExpressionFrame::LazyRight(value) => {
            value.capacity()
        }
        CExpressionFrame::IfThen(_, value) | CExpressionFrame::IfElse(value) => {
            value.as_ref().map_or(0, String::capacity)
        }
        _ => 0,
    }
}

fn c_expression_live_string_payload(
    current: &CExpressionFrame<'_>,
    frames: &[CExpressionFrame<'_>],
    values: &[String],
    arguments: &[String],
) -> Option<usize> {
    frames
        .iter()
        .try_fold(c_expression_frame_payload(current), |bytes, frame| {
            bytes.checked_add(c_expression_frame_payload(frame))
        })?
        .checked_add(
            values
                .iter()
                .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))?,
        )?
        .checked_add(
            arguments
                .iter()
                .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))?,
        )
}

#[allow(clippy::ptr_arg)] // Exact Vec capacities are part of the scratch proof.
fn note_c_expression_scratch(
    mode: CExpressionMode,
    current: &CExpressionFrame<'_>,
    frames: &Vec<CExpressionFrame<'_>>,
    values: &Vec<String>,
    arguments: &Vec<String>,
    lines: &CExpressionLineArena,
) -> Result<(), Diagnostic> {
    #[cfg(not(test))]
    let _ = mode;
    #[cfg(not(test))]
    let _ = lines;
    let string_payload = c_expression_live_string_payload(current, frames, values, arguments)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if string_payload > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    #[cfg(test)]
    {
        let working = frames
            .capacity()
            .saturating_mul(C_EXPRESSION_FRAME_BYTES)
            .saturating_add(
                values
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                arguments
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(lines.retained_bytes())
            .saturating_add(string_payload);
        match mode {
            CExpressionMode::Generate => note_post_hir_render_capacity(working),
            CExpressionMode::Replay => note_post_hir_replay_capacity(working),
        }
    }
    Ok(())
}

fn write_c_expression_arguments(
    lines: &mut CExpressionLineArena,
    arguments: &[String],
    separator: &str,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
            lines
                .write_str(separator)
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
        }
        lines
            .write_str(argument)
            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
    }
    Ok(())
}

fn c_expression_linear(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    let mode = CExpressionMode::Generate;
    let (node_count, depth) = c_expression_shape(expression)?;
    let frame_capacity = depth
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::<String>::with_capacity(frame_capacity);
    let mut arguments = Vec::<String>::with_capacity(node_count);
    frames.push(CExpressionFrame::Enter(expression));
    while let Some(frame) = frames.pop() {
        note_c_expression_scratch(mode, &frame, &frames, &values, &arguments, lines)?;
        match frame {
            CExpressionFrame::Enter(expression) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => values.push(
                    format!("v_{}", c_expression_hash(mode, place.root.as_str())),
                ),
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(CExpressionFrame::NativeArgs(call, 0, arguments.len()));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(CExpressionFrame::Unary(*op));
                    frames.push(CExpressionFrame::Enter(value));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(CExpressionFrame::LazyLeft(*op, right));
                    frames.push(CExpressionFrame::Enter(left));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(CExpressionFrame::BinaryLeft(*op, right));
                    frames.push(CExpressionFrame::Enter(left));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(CExpressionFrame::Block(statements, 0, tail));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = c_expression_resolved_scalar(mode, &expression.ty).ok_or_else(b111)?;
                    frames.push(CExpressionFrame::IfCondition(then_branch, else_branch, ty));
                    frames.push(CExpressionFrame::Enter(condition));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(CExpressionFrame::CallArgs(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        arguments.len(),
                    ));
                }
                ResolvedExprKind::ConstructRecord { .. }
                | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::UpdateRecord { .. }
                | ResolvedExprKind::Project { .. }
                | ResolvedExprKind::Place(_) => {
                    return Err(b107("scalar value signature required"));
                }
            },
            CExpressionFrame::Unary(op) => {
                let value = values.pop().ok_or_else(b111)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(lines, "if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    crate::ast::UnaryOp::Not => values.push(format!("(!({value}))")),
                }
            }
            CExpressionFrame::BinaryLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(CExpressionFrame::BinaryRight(op, left));
                frames.push(CExpressionFrame::Enter(right));
            }
            CExpressionFrame::BinaryRight(op, left) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "int64_t {name};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    match op {
                        crate::ast::BinaryOp::Add => write!(lines, "if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => write!(lines, "if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => write!(lines, "if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    }
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            CExpressionFrame::LazyLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                write!(
                    lines,
                    "uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);if({}){{",
                    if op == crate::ast::BinaryOp::And {
                        name.clone()
                    } else {
                        format!("!{name}")
                    }
                )
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::LazyRight(name));
                frames.push(CExpressionFrame::Enter(right));
            }
            CExpressionFrame::LazyRight(name) => {
                let right = values.pop().ok_or_else(b111)?;
                write!(lines, " {name}=({right})?UINT8_C(1):UINT8_C(0);}}")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                values.push(name);
            }
            CExpressionFrame::Block(statements, index, tail) => {
                if let Some(ResolvedStatement::Let { value, .. }) = statements.get(index) {
                    frames.push(CExpressionFrame::BlockLet(statements, index, tail));
                    frames.push(CExpressionFrame::Enter(value));
                } else {
                    frames.push(CExpressionFrame::Enter(tail));
                }
            }
            CExpressionFrame::BlockLet(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index];
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "{} v_{} = {value};",
                        c_expression_scalar(mode, ty),
                        c_expression_hash(mode, binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(CExpressionFrame::Block(statements, index + 1, tail));
            }
            CExpressionFrame::IfCondition(then_branch, else_branch, ty) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "{} {name};", c_expression_scalar(mode, ty))
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    Some(name)
                };
                write!(lines, "if({condition}){{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::IfThen(else_branch, name));
                frames.push(CExpressionFrame::Enter(then_branch));
            }
            CExpressionFrame::IfThen(else_branch, name) => {
                let then_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = &name {
                    write!(lines, "{name}={then_value};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                lines
                    .write_str("}else{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::IfElse(name));
                frames.push(CExpressionFrame::Enter(else_branch));
            }
            CExpressionFrame::IfElse(name) => {
                let else_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = name {
                    write!(lines, "{name}={else_value};}}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    lines
                        .write_str("}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push("INT64_C(0)".to_owned());
                }
            }
            CExpressionFrame::NativeArgs(call, index, start) => {
                if index < call.args.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(CExpressionFrame::NativeArgs(call, index + 1, start));
                    frames.push(CExpressionFrame::Enter(&call.args[index]));
                } else {
                    if !call.args.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = format!("tmp_{}", *temporary_count);
                    if import.result != ScalarType::Unit {
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};",
                            c_expression_scalar(mode, import.result)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(
                        lines,
                        "status = ctx->imports->{}(ctx->userdata",
                        import.c_field
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if start != arguments.len() {
                        lines
                            .write_str(", ")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        write_c_expression_arguments(lines, &arguments[start..], ", ")?;
                    }
                    if import.result != ScalarType::Unit {
                        write!(lines, ", &{name}")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(lines, "); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.rust_method)
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if import.result == ScalarType::Bool {
                        write!(lines, "if ({name} > UINT8_C(1)) return spxnr_adapter(4);")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    arguments.truncate(start);
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            CExpressionFrame::CallArgs(callee, source, ty, index, start) => {
                if index < source.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(CExpressionFrame::CallArgs(
                        callee,
                        source,
                        ty,
                        index + 1,
                        start,
                    ));
                    frames.push(CExpressionFrame::Enter(&source[index]));
                } else {
                    if !source.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        write!(
                            lines,
                            "status=spxnr1_f_{}(ctx",
                            c_expression_hash(mode, callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start != arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            write_c_expression_arguments(lines, &arguments[start..], ",")?;
                        }
                        lines
                            .write_str(");if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};status=spxnr1_f_{}(ctx",
                            c_expression_scalar(
                                mode,
                                c_expression_resolved_scalar(mode, ty).ok_or_else(b111)?
                            ),
                            c_expression_hash(mode, callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start != arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            write_c_expression_arguments(lines, &arguments[start..], ",")?;
                        }
                        write!(lines, ",&{name});if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    arguments.truncate(start);
                }
            }
        }
    }
    let terminal = CExpressionFrame::Enter(expression);
    note_c_expression_scratch(mode, &terminal, &frames, &values, &arguments, lines)?;
    if values.len() != 1 || !arguments.is_empty() {
        return Err(b111());
    }
    let result = values.pop().ok_or_else(b111)?;
    if result.capacity() > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    Ok(result)
}

#[cfg(any())]
fn c_context_line_slots(expression: &ResolvedExpr) -> Result<usize, Diagnostic> {
    // A line is owned by exactly one active context. Branch results are
    // collapsed to one String before being appended to their parent, and the
    // drained child Vec is released immediately. Child contexts therefore do
    // not reserve their whole subtree: across all live contexts their logical
    // line count is at most 3N. Vec geometric growth is below twice logical
    // length, so 6N String slots bounds all context backings simultaneously.
    c_expression_shape(expression)?
        .0
        .checked_mul(6)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

#[cfg(any())]
fn c_expr_iterative(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    mut temporary_names: Option<&mut Vec<String>>,
    lines: &mut Vec<String>,
    mode: CExpressionMode,
) -> Result<String, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr, usize),
        Unary(crate::ast::UnaryOp, usize),
        BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        BinaryRight(crate::ast::BinaryOp, String, usize),
        LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        LazyRight(crate::ast::BinaryOp, String, String, usize, usize),
        Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType, usize),
        IfThen(String, &'a ResolvedExpr, Option<String>, usize, usize),
        IfElse(String, Option<String>, String, usize, usize, usize),
        NativeArgs(
            &'a crate::hir::ResolvedNativeRustImportCall,
            usize,
            Vec<String>,
            usize,
        ),
        CallArgs(
            &'a str,
            &'a [ResolvedExpr],
            &'a ResolvedType,
            usize,
            Vec<String>,
            usize,
        ),
    }
    const _: () = assert!(std::mem::size_of::<Frame<'static>>() == C_EXPRESSION_FRAME_BYTES);

    let allocate_temporary =
        |temporary_count: &mut usize, temporary_names: &mut Option<&mut Vec<String>>| {
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            if let Some(names) = temporary_names.as_deref_mut() {
                names.push(name.clone());
            }
            name
        };
    let (node_count, depth) = c_expression_shape(expression)?;
    let line_capacity = node_count
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if lines.capacity() < line_capacity {
        lines
            .try_reserve_exact(line_capacity - lines.capacity())
            .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let frame_capacity = node_count
        .checked_mul(2)
        .and_then(|slots| slots.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    frames.push(Frame::Enter(expression, 0));
    let mut results = Vec::<String>::with_capacity(depth + 1);
    let mut contexts = Vec::with_capacity(node_count + 1);
    contexts.push(Vec::<String>::with_capacity(node_count.saturating_mul(3)));
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let frame_owned = frames
                .iter()
                .map(|frame| match frame {
                    Frame::BinaryRight(_, value, _)
                    | Frame::LazyRight(_, value, _, _, _)
                    | Frame::IfThen(value, _, _, _, _)
                    | Frame::IfElse(value, _, _, _, _, _) => value.capacity(),
                    Frame::NativeArgs(_, _, values, _) | Frame::CallArgs(_, _, _, _, values, _) => {
                        values.capacity() * std::mem::size_of::<String>()
                            + values.iter().map(String::capacity).sum::<usize>()
                    }
                    _ => 0,
                })
                .sum::<usize>();
            let result_owned = results.capacity() * std::mem::size_of::<String>()
                + results.iter().map(String::capacity).sum::<usize>();
            let context_owned = contexts.capacity() * std::mem::size_of::<Vec<String>>()
                + contexts
                    .iter()
                    .map(|context| {
                        context.capacity() * std::mem::size_of::<String>()
                            + context.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>();
            let caller_lines = lines.capacity() * std::mem::size_of::<String>()
                + lines.iter().map(String::capacity).sum::<usize>();
            let persistent_temporaries = temporary_names.as_deref().map_or(0, |names| {
                names.capacity() * std::mem::size_of::<String>()
                    + names.iter().map(String::capacity).sum::<usize>()
            });
            let working = frames.capacity() * std::mem::size_of::<Frame<'_>>()
                + frame_owned
                + result_owned
                + context_owned
                + caller_lines
                + persistent_temporaries;
            match mode {
                CExpressionMode::Generate => note_post_hir_render_capacity(working),
                CExpressionMode::Replay => note_post_hir_replay_capacity(working),
            }
        }
        match frame {
            Frame::Enter(expression, context) => match &expression.kind {
                ResolvedExprKind::Int(value) => results.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    results.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => results.push(
                    format!("v_{}", c_expression_hash(mode, place.root.as_str())),
                ),
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(Frame::NativeArgs(
                        call,
                        0,
                        Vec::with_capacity(call.args.len()),
                        context,
                    ));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(Frame::Unary(*op, context));
                    frames.push(Frame::Enter(value, context));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(Frame::LazyLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(Frame::BinaryLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(Frame::Block(statements, 0, tail, context));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = c_expression_resolved_scalar(mode, &expression.ty).ok_or_else(b111)?;
                    frames.push(Frame::IfCondition(then_branch, else_branch, ty, context));
                    frames.push(Frame::Enter(condition, context));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(Frame::CallArgs(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        Vec::with_capacity(args.len()),
                        context,
                    ));
                }
                ResolvedExprKind::ConstructRecord { .. }
                | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::UpdateRecord { .. }
                | ResolvedExprKind::Project { .. }
                | ResolvedExprKind::Place(_) => {
                    return Err(b107("scalar value signature required"));
                }
            },
            Frame::Unary(op, context) => {
                let value = results.pop().ok_or_else(b111)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let name = allocate_temporary(temporary_count, &mut temporary_names);
                        contexts[context].push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                        results.push(name);
                    }
                    crate::ast::UnaryOp::Not => results.push(format!("(!({value}))")),
                }
            }
            Frame::BinaryLeft(op, right, context) => {
                let left = results.pop().ok_or_else(b111)?;
                frames.push(Frame::BinaryRight(op, left, context));
                frames.push(Frame::Enter(right, context));
            }
            Frame::BinaryRight(op, left, context) => {
                let right = results.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = allocate_temporary(temporary_count, &mut temporary_names);
                    contexts[context].push(format!("int64_t {name};"));
                    contexts[context].push(match op {
                        crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    });
                    results.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    results.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            Frame::LazyLeft(op, right, context) => {
                let left = results.pop().ok_or_else(b111)?;
                let name = allocate_temporary(temporary_count, &mut temporary_names);
                contexts[context].push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
                let branch = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::LazyRight(op, name, left, context, branch));
                frames.push(Frame::Enter(right, branch));
            }
            Frame::LazyRight(op, name, _left, context, branch) => {
                let right = results.pop().ok_or_else(b111)?;
                let condition = if op == crate::ast::BinaryOp::And {
                    name.clone()
                } else {
                    format!("!{name}")
                };
                let branch_lines = take_c_lines(&mut contexts[branch]);
                contexts[branch] = Vec::new();
                contexts[context].push(format!(
                    "if({condition}){{{branch_lines} {name}=({right})?UINT8_C(1):UINT8_C(0);}}"
                ));
                results.push(name);
            }
            Frame::Block(statements, index, tail, context) => {
                if index == statements.len() {
                    frames.push(Frame::Enter(tail, context));
                } else {
                    let ResolvedStatement::Let { value, .. } = &statements[index];
                    frames.push(Frame::BlockLet(statements, index, tail, context));
                    frames.push(Frame::Enter(value, context));
                }
            }
            Frame::BlockLet(statements, index, tail, context) => {
                let value = results.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index];
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "{} v_{} = {value};",
                        c_expression_scalar(mode, ty),
                        c_expression_hash(mode, binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::IfCondition(then_branch, else_branch, ty, context) => {
                let condition = results.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = allocate_temporary(temporary_count, &mut temporary_names);
                    contexts[context].push(format!("{} {name};", c_expression_scalar(mode, ty)));
                    Some(name)
                };
                let then_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfThen(
                    condition,
                    else_branch,
                    name,
                    context,
                    then_context,
                ));
                frames.push(Frame::Enter(then_branch, then_context));
            }
            Frame::IfThen(condition, else_branch, name, context, then_context) => {
                let then_value = results.pop().ok_or_else(b111)?;
                let else_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfElse(
                    condition,
                    name,
                    then_value,
                    context,
                    then_context,
                    else_context,
                ));
                frames.push(Frame::Enter(else_branch, else_context));
            }
            Frame::IfElse(condition, name, then_value, context, then_context, else_context) => {
                let else_value = results.pop().ok_or_else(b111)?;
                let then_lines = take_c_lines(&mut contexts[then_context]);
                let else_lines = take_c_lines(&mut contexts[else_context]);
                contexts[then_context] = Vec::new();
                contexts[else_context] = Vec::new();
                if let Some(name) = name {
                    contexts[context].push(format!("if({condition}){{{then_lines}{name}={then_value};}}else{{{else_lines}{name}={else_value};}}"));
                    results.push(name);
                } else {
                    contexts[context].push(format!(
                        "if({condition}){{{then_lines}}}else{{{else_lines}}}"
                    ));
                    results.push("INT64_C(0)".to_owned());
                }
            }
            Frame::NativeArgs(call, index, mut args, context) => {
                if index < call.args.len() {
                    if index > 0 {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::NativeArgs(call, index + 1, args, context));
                    frames.push(Frame::Enter(&call.args[index], context));
                } else {
                    if !call.args.is_empty() {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = if import.result == ScalarType::Unit {
                        format!("tmp_{}", *temporary_count)
                    } else {
                        allocate_temporary(temporary_count, &mut temporary_names)
                    };
                    if import.result != ScalarType::Unit {
                        contexts[context].push(format!(
                            "{} {name};",
                            c_expression_scalar(mode, import.result)
                        ));
                    }
                    contexts[context].push(format!("status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.c_field, if args.is_empty() { "" } else { ", " }, args.join(", "), if import.result == ScalarType::Unit { String::new() } else { format!(", &{name}") }, import.rust_method));
                    if import.result == ScalarType::Bool {
                        contexts[context]
                            .push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
                    }
                    results.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            Frame::CallArgs(callee, call_args, ty, index, mut args, context) => {
                if index < call_args.len() {
                    if index > 0 {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::CallArgs(
                        callee,
                        call_args,
                        ty,
                        index + 1,
                        args,
                        context,
                    ));
                    frames.push(Frame::Enter(&call_args[index], context));
                } else {
                    if !call_args.is_empty() {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        contexts[context].push(format!(
                            "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                            c_expression_hash(mode, callee),
                            if args.is_empty() { "" } else { ", " },
                            args.join(",")
                        ));
                        results.push("INT64_C(0)".to_owned());
                    } else {
                        let name = allocate_temporary(temporary_count, &mut temporary_names);
                        let scalar = c_expression_resolved_scalar(mode, ty).ok_or_else(b111)?;
                        contexts[context].push(format!("{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;", c_expression_scalar(mode, scalar), c_expression_hash(mode, callee), if args.is_empty() { "" } else { ", " }, args.join(",")));
                        results.push(name);
                    }
                }
            }
        }
    }
    if results.len() != 1 {
        return Err(b111());
    }
    move_root_c_lines(lines, &mut contexts);
    results.pop().ok_or_else(b111)
}

#[cfg(any())]
fn c_expr(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporaries: &mut Vec<String>,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    let mut count = temporaries.len();
    c_expr_iterative(
        expression,
        imports,
        &mut count,
        Some(temporaries),
        lines,
        CExpressionMode::Generate,
    )
}

fn c_expr(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    c_expression_linear(expression, imports, temporary_count, lines)
}

fn generate_c_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let capability_digest = capability_digest(&spec.capabilities);
    let capability_hex = capability_digest.strip_prefix("sha256:").ok_or_else(b111)?;
    let bytes = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &capability_hex[index..index + 2]))
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "#include \"semaprax_native_rust_interop.h\"\n#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n#include <limits.h>\nstatic const uint8_t spxnr_capabilities[32] = {{{bytes}}};\nstatic spxnr_status_v1 spxnr_adapter(uint32_t code){{return (((uint64_t)65535)<<48)|(((uint64_t)4)<<32)|code;}}\nstatic spxnr_status_v1 spxnr_validate(const spxnr_context_v1 *ctx){{if(!ctx||((uintptr_t)ctx%_Alignof(spxnr_context_v1))!=0)return spxnr_adapter(1);if(ctx->abi_version!=1||ctx->size!=sizeof(*ctx)||ctx->reserved!=0)return spxnr_adapter(1);if(!ctx->imports||((uintptr_t)ctx->imports%_Alignof(spxnr_imports_v1))!=0)return spxnr_adapter(2);if(ctx->imports->abi_version!=1||ctx->imports->size!=sizeof(*ctx->imports))return spxnr_adapter(2);if(memcmp(ctx->capabilities_digest,spxnr_capabilities,32)!=0)return spxnr_adapter(3);if(ctx->call_depth>=32)return spxnr_adapter(7);return 0;}}\n"
    )
    .unwrap();
    if !imports.is_empty() {
        output.write_str("static int spxnr_status_canonical(spxnr_status_v1 status){if(status==0)return 1;uint32_t code=(uint32_t)status;uint8_t class_=(uint8_t)(status>>32);uint8_t retry=(uint8_t)((status>>40)&1);uint8_t reserved=(uint8_t)((status>>41)&0x7f);uint16_t domain=(uint16_t)(status>>48);if(code==0||reserved!=0||domain==0)return 0;if(domain==65533)return retry==0&&((class_==1&&code>=1&&code<=6)||(class_==2&&code>=1&&code<=2));").unwrap();
    }
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .collect::<BTreeSet<_>>();
    for (index, _) in domains.iter().enumerate() {
        write!(output, "if(domain=={})return class_==3;", index + 1).unwrap();
    }
    if !imports.is_empty() {
        output.write_str("if(domain==65534)return class_==4&&retry==0&&code>=1&&code<=2;if(domain==65535)return class_==4&&retry==0&&code>=1&&code<=8;return 0;}\n").unwrap();
    }
    let domain_ordinals = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    for import in imports {
        let custom = import
            .failure
            .as_deref()
            .and_then(|domain| domain_ordinals.get(domain).copied());
        write!(
            output,
            "static int spxnr_status_for_{}(spxnr_status_v1 status){{if(!spxnr_status_canonical(status))return 0;uint16_t domain=(uint16_t)(status>>48);return domain==65534||domain==65535{};}}\n",
            import.rust_method,
            custom.map_or_else(String::new, |ordinal| format!("||domain=={ordinal}"))
        )
        .unwrap();
        write!(output,"static spxnr_status_v1 spxnr_validate_{}(const spxnr_context_v1 *ctx){{return ctx->imports->{}?0:spxnr_adapter(2);}}\n",import.rust_method,import.c_field).unwrap();
    }
    for function in closure {
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type).ok_or_else(b111)?;
        let params = c_parameters(&parameters);
        write!(
            output,
            "static spxnr_status_v1 spxnr1_f_{}(const spxnr_context_v1 *ctx{}{}{});\n",
            full_hash(function.id.as_str()),
            if params.is_empty() { "" } else { ", " },
            params,
            if result == ScalarType::Unit {
                String::new()
            } else {
                format!(", {} *result_out", c_type(result))
            }
        )
        .unwrap();
    }
    for function in closure {
        let parameters = parameter_facts(function)?;
        let result = scalar_type(&function.return_type).ok_or_else(b111)?;
        let params = c_parameters(&parameters);
        write!(output,"static spxnr_status_v1 spxnr1_f_{}(const spxnr_context_v1 *ctx{}{}{} ){{spxnr_status_v1 status=0;(void)ctx;",full_hash(function.id.as_str()),if params.is_empty(){""}else{", "},params,if result==ScalarType::Unit{String::new()}else{format!(", {} *result_out",c_type(result))}).unwrap();
        for index in 0..parameters.len() {
            write!(output, "(void)arg_{index};").unwrap();
        }
        for (index, (parameter, resolved)) in parameters.iter().zip(&function.params).enumerate() {
            write!(
                output,
                "{} v_{}=arg_{};",
                c_type(parameter.ty),
                full_hash(resolved.id.as_str()),
                index
            )
            .unwrap();
        }
        let mut temporary_count = 0usize;
        let mut lines = CExpressionLineArena::new();
        for requirement in &function.requires {
            lines.clear();
            let value = c_expr(requirement, imports, &mut temporary_count, &mut lines)?;
            output.write_str(lines.as_str()?).unwrap();
            write!(
                output,
                "if(!({value}))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(1);"
            )
            .unwrap();
        }
        lines.clear();
        let value = c_expr(&function.body, imports, &mut temporary_count, &mut lines)?;
        output.write_str(lines.as_str()?).unwrap();
        if result != ScalarType::Unit {
            write!(
                output,
                "{} v_{}={value};",
                c_type(result),
                full_hash(function.result_id.as_str())
            )
            .unwrap();
        }
        for guarantee in &function.ensures {
            lines.clear();
            let value = c_expr(guarantee, imports, &mut temporary_count, &mut lines)?;
            output.write_str(lines.as_str()?).unwrap();
            write!(
                output,
                "if(!({value}))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(2);"
            )
            .unwrap();
        }
        if result != ScalarType::Unit {
            write!(
                output,
                "*result_out=v_{};",
                full_hash(function.result_id.as_str())
            )
            .unwrap();
        }
        output.write_str("return status;}\n").unwrap();
    }
    for export in exports {
        let params = c_parameters(&export.parameters);
        write!(output, "spxnr_status_v1 {}(const spxnr_context_v1 *ctx{}{}{} ){{spxnr_status_v1 status=spxnr_validate(ctx);if(status!=0)return status;", export.c_symbol, if params.is_empty(){""}else{", "}, params, if export.result==ScalarType::Unit{String::new()}else{format!(", {} *result_out",c_type(export.result))}).unwrap();
        for import in imports {
            write!(
                output,
                "status=spxnr_validate_{}(ctx);if(status!=0)return status;",
                import.rust_method
            )
            .unwrap();
        }
        if export.result != ScalarType::Unit {
            write!(
                output,
                "if(!result_out||((uintptr_t)result_out%_Alignof({}))!=0)return spxnr_adapter(5);",
                c_type(export.result)
            )
            .unwrap();
        }
        for (index, parameter) in export.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                write!(output, "if(arg_{index}>1)return spxnr_adapter(4);").unwrap();
            }
        }
        output
            .write_str("spxnr_context_v1 local=*ctx;local.call_depth=ctx->call_depth+1;")
            .unwrap();
        write!(
            output,
            "status=spxnr1_f_{}(&local{}{}{});",
            full_hash(&export.id),
            if export.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            (0..export.parameters.len())
                .map(|index| format!("arg_{index}"))
                .collect::<Vec<_>>()
                .join(","),
            if export.result == ScalarType::Unit {
                String::new()
            } else {
                ", result_out".to_owned()
            }
        )
        .unwrap();
        output.write_str("return status;}\n").unwrap();
    }
    Ok(())
}

fn generate_c(
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<String, Diagnostic> {
    render_exact_artifact("max_generated_c_bytes", MAX_GENERATED_C_BYTES, |sink| {
        generate_c_into(sink, spec, closure, exports, imports)
    })
}

fn capability_digest(capabilities: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITIES_DOMAIN);
    for capability in capabilities {
        frame(&mut hasher, capability.as_bytes());
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn rust_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("arg_{index}: {}", rust_type(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

fn generate_safe_rust_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    output.write_str("mod api{#![forbid(unsafe_code)]\nuse core::num::NonZeroU32;\n#[repr(u8)] #[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum NativeRustStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n").unwrap();
    if !imports.is_empty() {
        output.write_str("pub enum NativeRustImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailure}\n").unwrap();
    }
    output.write_str("pub enum NativeRustCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\npub struct NativeRustAdmissionError;\n").unwrap();
    output.write_str("pub trait NativeRustImports{").unwrap();
    for import in imports {
        write!(
            output,
            "fn {}(&mut self{}{})->NativeRustImportResult<{}>;",
            import.rust_method,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            rust_parameters(&import.parameters),
            rust_type(import.result)
        )
        .unwrap();
    }
    output.write_str("}\n").unwrap();
    let capability_values = spec
        .capabilities
        .iter()
        .map(|value| quote_json(value))
        .collect::<Vec<_>>();
    let capabilities = capability_values.join(",");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&capability_values).saturating_add(capabilities.capacity()),
    );
    write!(
        output,
        "const EXPECTED_CAPABILITIES:&[&str]=&[{}];\n",
        capabilities
    )
    .unwrap();
    output.write_str("pub struct NativeRustCapabilities{digest:[u8;32]} impl NativeRustCapabilities{pub fn new(values:&[&str])->Result<Self,NativeRustAdmissionError>{if values!=EXPECTED_CAPABILITIES{return Err(NativeRustAdmissionError)}Ok(Self{digest:super::ffi::capabilities_digest()})}}\n").unwrap();
    output.write_str("struct ActiveGuard<'a>{active:&'a mut bool}impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}\npub struct NativeRustBridge<H:NativeRustImports>{host:H,capabilities:NativeRustCapabilities,owner:std::thread::ThreadId,active:bool,calls:u32,_not_send_sync:core::marker::PhantomData<*mut ()>} impl<H:NativeRustImports> NativeRustBridge<H>{pub fn new(host:H,capabilities:NativeRustCapabilities)->Self{Self{host,capabilities,owner:std::thread::current().id(),active:false,calls:0,_not_send_sync:core::marker::PhantomData}}\n").unwrap();
    for export in exports {
        let parameters = rust_parameters(&export.parameters);
        let argument_values = (0..export.parameters.len())
            .map(|index| format!("arg_{index}"))
            .collect::<Vec<_>>();
        let arguments = argument_values.join(", ");
        #[cfg(test)]
        note_post_hir_render_capacity(
            parameters
                .capacity()
                .saturating_add(string_slice_owned_capacity(&argument_values))
                .saturating_add(arguments.capacity()),
        );
        write!(output,"pub fn {}(&mut self{}{})->Result<{},NativeRustCallError>{{if self.owner!=std::thread::current().id()||core::mem::replace(&mut self.active,true){{return Err(NativeRustCallError::AdapterRejected)}}let _active_guard=ActiveGuard{{active:&mut self.active}};super::ffi::{}(&mut self.host,&mut self.calls,self.capabilities.digest{}{})}}\n",export.rust_method,if export.parameters.is_empty(){""}else{", "},parameters,rust_type(export.result),export.rust_method,if export.parameters.is_empty(){""}else{", "},arguments).unwrap();
    }
    output
        .write_str(
            "}\n}\n#[path=\"semaprax_native_rust_interop_ffi.rs\"]mod ffi;\npub use api::*;\n",
        )
        .unwrap();
    Ok(())
}

fn generate_private_ffi_into(
    output: &mut dyn std::fmt::Write,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(), Diagnostic> {
    let digest = capability_digest(&spec.capabilities);
    let hex = digest.strip_prefix("sha256:").unwrap_or("");
    let byte_values = (0..64)
        .step_by(2)
        .map(|index| format!("0x{}", &hex[index..index + 2]))
        .collect::<Vec<_>>();
    let bytes = byte_values.join(",");
    let mut import_table_values = Vec::with_capacity(imports.len());
    for import in imports {
        let parameter_values = import
            .parameters
            .iter()
            .map(|parameter| match parameter.ty {
                ScalarType::I64 => "i64".to_owned(),
                ScalarType::Bool => "u8".to_owned(),
                ScalarType::Unit => "()".to_owned(),
            })
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let result = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", *mut {}", rust_ffi_wire_type(import.result))
        };
        let row = format!(
            "{}:unsafe extern \"C\" fn(*mut c_void{}{}{})->u64,",
            import.c_field,
            if import.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            parameters,
            result
        );
        #[cfg(test)]
        note_post_hir_render_capacity(
            string_slice_owned_capacity(&byte_values)
                .saturating_add(bytes.capacity())
                .saturating_add(string_slice_owned_capacity(&import_table_values))
                .saturating_add(string_slice_owned_capacity(&parameter_values))
                .saturating_add(parameters.capacity())
                .saturating_add(result.capacity())
                .saturating_add(row.capacity()),
        );
        import_table_values.push(row);
    }
    let import_table = import_table_values.join("");
    #[cfg(test)]
    note_post_hir_render_capacity(
        string_slice_owned_capacity(&byte_values)
            .saturating_add(bytes.capacity())
            .saturating_add(string_slice_owned_capacity(&import_table_values))
            .saturating_add(import_table.capacity()),
    );
    write!(output, "#![allow(unsafe_code)]\nuse super::api::*;\nuse core::ffi::c_void;\n#[repr(C)]struct Imports{{abi_version:u32,size:u32,{import_table} }}\n#[repr(C)]struct Context{{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}}\n").unwrap();
    if !imports.is_empty() {
        output
            .write_str("struct Frame<H>{host:*mut H,calls:*mut u32}\n")
            .unwrap();
    }
    write!(
        output,
        "pub(super) fn capabilities_digest()->[u8;32]{{[{bytes}]}}\n"
    )
    .unwrap();
    #[cfg(test)]
    let ffi_prefix_scratch = digest
        .capacity()
        .saturating_add(string_slice_owned_capacity(&byte_values))
        .saturating_add(bytes.capacity())
        .saturating_add(string_slice_owned_capacity(&import_table_values))
        .saturating_add(import_table.capacity());
    if !imports.is_empty() {
        output.write_str("fn adapter(code:u32)->u64{((65535u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|u64::from(code)}\n").unwrap();
    }
    output.write_str("fn decode_status(status:u64)->NativeRustCallError{let code=(status&0xffff_ffff)as u32;let class=((status>>32)&0xff)as u8;let retryable=((status>>40)&1)!=0;let reserved=(status>>41)&0x7f;let domain=(status>>48)as u16;let Some(code)=core::num::NonZeroU32::new(code)else{return NativeRustCallError::AdapterRejected};let class=match class{1=>NativeRustStatusClass::Semantic,2=>NativeRustStatusClass::Contract,3=>NativeRustStatusClass::Import,4=>NativeRustStatusClass::Adapter,_=>return NativeRustCallError::AdapterRejected};if reserved!=0||domain==0{return NativeRustCallError::AdapterRejected}match domain{65533=>{let valid=!retryable&&match class{NativeRustStatusClass::Semantic=>(1..=6).contains(&code.get()),NativeRustStatusClass::Contract=>(1..=2).contains(&code.get()),_=>false};if !valid{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class,retryable}},").unwrap();
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    for (index, domain) in domains.iter().enumerate() {
        write!(output,"{}=>{{if class!=NativeRustStatusClass::Import{{return NativeRustCallError::AdapterRejected}}NativeRustCallError::Semantic{{domain_id:{},code,class,retryable}}}},",index+1,quote_json(domain)).unwrap();
    }
    output.write_str("65534=>if class==NativeRustStatusClass::Adapter&&!retryable{match code.get(){1=>NativeRustCallError::HostPanicked,2=>NativeRustCallError::HostFailed,_=>NativeRustCallError::AdapterRejected}}else{NativeRustCallError::AdapterRejected},65535=>if class==NativeRustStatusClass::Adapter&&!retryable&&(1..=8).contains(&code.get()){NativeRustCallError::AdapterRejected}else{NativeRustCallError::AdapterRejected},_=>NativeRustCallError::AdapterRejected}}\n").unwrap();
    for import in imports {
        let parameter_declaration_values = import
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                format!(
                    "arg_{index}:{}",
                    match p.ty {
                        ScalarType::I64 => "i64",
                        ScalarType::Bool => "u8",
                        ScalarType::Unit => "()",
                    }
                )
            })
            .collect::<Vec<_>>();
        let parameter_declarations = parameter_declaration_values.join(",");
        let result_declaration = if import.result == ScalarType::Unit {
            String::new()
        } else {
            format!(
                ", result_out:*mut {}",
                match import.result {
                    ScalarType::I64 => "i64",
                    ScalarType::Bool => "u8",
                    ScalarType::Unit => "()",
                }
            )
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&parameter_declaration_values))
                .saturating_add(parameter_declarations.capacity())
                .saturating_add(result_declaration.capacity()),
        );
        write!(output,"unsafe extern \"C\" fn cb_{}<H:NativeRustImports>(userdata:*mut c_void{}{}{}) -> u64{{if userdata.is_null(){{return adapter(1);}}",import.rust_method,if import.parameters.is_empty(){""}else{", "},parameter_declarations,result_declaration).unwrap();
        for (index, parameter) in import.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                write!(output, "if arg_{index}>1{{return adapter(4);}}").unwrap();
            }
        }
        if import.result != ScalarType::Unit {
            write!(output,"if result_out.is_null()||(result_out as usize)%core::mem::align_of::<{}>()!=0{{return adapter(5);}}",rust_type(import.result)).unwrap();
        }
        let call_argument_values = import
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if p.ty == ScalarType::Bool {
                    format!("arg_{index}!=0")
                } else {
                    format!("arg_{index}")
                }
            })
            .collect::<Vec<_>>();
        let call_arguments = call_argument_values.join(",");
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&call_argument_values))
                .saturating_add(call_arguments.capacity()),
        );
        write!(output,"if (userdata as usize)%core::mem::align_of::<Frame<H>>()!=0{{return adapter(1);}}let frame=&mut*(userdata as *mut Frame<H>);if frame.host.is_null()||frame.calls.is_null()||*frame.calls>=4096{{return adapter(7);}}*frame.calls+=1;let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{{let host=&mut *frame.host;host.{}({})}}));match run{{Err(payload)=>{{core::mem::forget(payload);((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|1}},Ok(NativeRustImportResult::HostFailure)=>((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|2,",import.rust_method,call_arguments).unwrap();
        let ordinal = import
            .failure
            .as_ref()
            .and_then(|domain| domains.iter().position(|value| value == domain))
            .map(|index| index + 1);
        if let Some(ordinal) = ordinal {
            write!(output,"Ok(NativeRustImportResult::Status{{code,class,retryable}})=>if class==NativeRustStatusClass::Import{{(({}u64)<<48)|((class as u64)<<32)|((retryable as u64)<<40)|u64::from(code.get())}}else{{adapter(3)}},",ordinal).unwrap();
        } else {
            output.write_str("Ok(NativeRustImportResult::Status{code,class,retryable})=>{let _=(code,class,retryable);adapter(3)},").unwrap();
        }
        if import.result == ScalarType::Unit {
            output
                .write_str("Ok(NativeRustImportResult::Success(()))=>0}}}\n")
                .unwrap();
        } else {
            write!(
                output,
                "Ok(NativeRustImportResult::Success(value))=>{{*result_out={};0}}",
                if import.result == ScalarType::Bool {
                    "u8::from(value)"
                } else {
                    "value"
                }
            )
            .unwrap();
            output.write_str("}}\n").unwrap();
        }
    }
    for export in exports {
        let parameter_values = export
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| format!("arg_{index}:{}", rust_ffi_wire_type(parameter.ty)))
            .collect::<Vec<_>>();
        let parameters = parameter_values.join(",");
        let result = if export.result == ScalarType::Unit {
            String::new()
        } else {
            format!(", result_out:*mut {}", rust_ffi_wire_type(export.result))
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(string_slice_owned_capacity(&parameter_values))
                .saturating_add(parameters.capacity())
                .saturating_add(result.capacity()),
        );
        write!(
            output,
            "extern \"C\"{{fn {}(ctx:*const Context{}{}{})->u64;}}\n",
            export.c_symbol,
            if export.parameters.is_empty() {
                ""
            } else {
                ", "
            },
            parameters,
            result
        )
        .unwrap();
    }
    for export in exports {
        let result_slot = match export.result {
            ScalarType::Unit => String::new(),
            ScalarType::I64 => "let mut result=core::mem::MaybeUninit::<i64>::uninit();".to_owned(),
            ScalarType::Bool => "let mut result=core::mem::MaybeUninit::<u8>::uninit();".to_owned(),
        };
        let publish = match export.result {
            ScalarType::Unit => "Ok(())",
            ScalarType::I64 => "Ok(result.assume_init())",
            ScalarType::Bool => "let value=result.assume_init();if value>1{return Err(NativeRustCallError::AdapterRejected)}Ok(value!=0)",
        };
        let parameters = rust_parameters(&export.parameters);
        let callback_values = imports
            .iter()
            .map(|import| format!("{}:cb_{}::<H>,", import.c_field, import.rust_method))
            .collect::<Vec<_>>();
        let callbacks = callback_values.join("");
        let argument_values = export
            .parameters
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if p.ty == ScalarType::Bool {
                    format!("u8::from(arg_{index})")
                } else {
                    format!("arg_{index}")
                }
            })
            .collect::<Vec<_>>();
        let arguments = argument_values.join(",");
        let result_argument = if export.result == ScalarType::Unit {
            String::new()
        } else {
            ", result.as_mut_ptr()".to_owned()
        };
        #[cfg(test)]
        note_post_hir_render_capacity(
            ffi_prefix_scratch
                .saturating_add(owned_string_set_owned_capacity(&domains))
                .saturating_add(
                    parameters
                        .capacity()
                        .saturating_add(string_slice_owned_capacity(&callback_values))
                        .saturating_add(callbacks.capacity())
                        .saturating_add(result_slot.capacity())
                        .saturating_add(string_slice_owned_capacity(&argument_values))
                        .saturating_add(arguments.capacity())
                        .saturating_add(result_argument.capacity()),
                ),
        );
        let frame = if imports.is_empty() {
            "let _=host;let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};"
        } else {
            "let mut frame=Frame{host:host as *mut H,calls:calls as *mut u32};let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:&mut frame as *mut Frame<H> as *mut c_void,imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};"
        };
        write!(output,"pub(super) fn {}<H:NativeRustImports>(host:&mut H,calls:&mut u32,digest:[u8;32]{}{})->Result<{},NativeRustCallError>{{unsafe{{if *calls>=4096{{return Err(NativeRustCallError::AdapterRejected)}}*calls+=1;let table=Imports{{abi_version:1,size:core::mem::size_of::<Imports>() as u32,{}}};{}{}let status={}(&ctx{}{}{});if status!=0{{return Err(decode_status(status))}}{} }}}}\n",export.rust_method,if export.parameters.is_empty(){""}else{", "},parameters,rust_type(export.result),callbacks,frame,result_slot,export.c_symbol,if export.parameters.is_empty(){""}else{", "},arguments,result_argument,publish).unwrap();
    }
    Ok(())
}

fn generate_rust_artifacts_with_limit(
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
    maximum: usize,
) -> Result<(String, String), Diagnostic> {
    let mut render_safe =
        |sink: &mut dyn std::fmt::Write| generate_safe_rust_into(sink, spec, exports, imports);
    let mut render_ffi =
        |sink: &mut dyn std::fmt::Write| generate_private_ffi_into(sink, spec, exports, imports);
    let safe_bytes = count_exact_artifact("max_generated_rust_bytes", maximum, &mut render_safe)?;
    let ffi_bytes = count_exact_artifact("max_generated_rust_bytes", maximum, &mut render_ffi)?;
    let combined_bytes = safe_bytes
        .checked_add(ffi_bytes)
        .ok_or_else(|| b109("max_generated_rust_bytes", maximum))?;
    if combined_bytes > maximum {
        return Err(b109("max_generated_rust_bytes", maximum));
    }
    let safe = render_counted_artifact(
        "max_generated_rust_bytes",
        maximum,
        safe_bytes,
        &mut render_safe,
    )?;
    let ffi = render_counted_artifact(
        "max_generated_rust_bytes",
        maximum,
        ffi_bytes,
        &mut render_ffi,
    )?;
    Ok((safe, ffi))
}

fn generate_rust_artifacts(
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<(String, String), Diagnostic> {
    generate_rust_artifacts_with_limit(spec, exports, imports, MAX_GENERATED_RUST_BYTES)
}

fn replay_generated(header: &str, c: &str, rust: &str, ffi: &str) -> Result<(), Diagnostic> {
    if !header.starts_with("#ifndef ")
        || !header.ends_with("#endif\n")
        || !c.starts_with("#include \"semaprax_native_rust_interop.h\"")
        || !rust.starts_with("mod api{#![forbid(unsafe_code)]\n")
        || rust.contains("unsafe {")
        || !ffi.starts_with("#![allow(unsafe_code)]\n")
    {
        return Err(b111());
    }
    Ok(())
}

fn replay_header_exact(source: &str, exports: &[ExportFact], imports: &[ImportFact]) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("#ifndef SEMAPRAX_NATIVE_RUST_INTEROP_H\n#define SEMAPRAX_NATIVE_RUST_INTEROP_H\n#include <stdint.h>\n#include <stddef.h>\n#ifdef __cplusplus\nextern \"C\" {\n#endif\ntypedef uint64_t spxnr_status_v1;\ntypedef struct spxnr_imports_v1 spxnr_imports_v1;\ntypedef struct { uint32_t abi_version; uint32_t size; void *userdata; const spxnr_imports_v1 *imports; uint8_t capabilities_digest[32]; uint32_t call_depth; uint32_t reserved; } spxnr_context_v1;\nstruct spxnr_imports_v1 { uint32_t abi_version; uint32_t size;");
    for import in imports {
        replay.text(" spxnr_status_v1 (*");
        replay.text(&import.c_field);
        replay.text(")(void *userdata");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(", ");
            replay.text(c_type(parameter.ty));
            replay.text(" arg_");
            replay.number(index);
        }
        if import.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(c_type(import.result));
            replay.text(" *result_out");
        }
        replay.text(");");
    }
    replay.text(" };\n");
    for export in exports {
        replay.text("spxnr_status_v1 ");
        replay.text(&export.c_symbol);
        replay.text("(const spxnr_context_v1 *ctx");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(", ");
            replay.text(c_type(parameter.ty));
            replay.text(" arg_");
            replay.number(index);
        }
        if export.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(c_type(export.result));
            replay.text(" *result_out");
        }
        replay.text(");\n");
    }
    replay.text("#ifdef __cplusplus\n}\n#endif\n#endif\n");
    replay.finish()
}

fn replay_rust_scalar(replay: &mut ExactReplay<'_>, ty: ScalarType) {
    replay.text(match ty {
        ScalarType::I64 => "i64",
        ScalarType::Bool => "bool",
        ScalarType::Unit => "()",
    });
}

fn replay_rust_parameters(replay: &mut ExactReplay<'_>, parameters: &[ParameterFact]) {
    for (index, parameter) in parameters.iter().enumerate() {
        if index != 0 {
            replay.text(", ");
        }
        replay.text("arg_");
        replay.number(index);
        replay.text(": ");
        replay_rust_scalar(replay, parameter.ty);
    }
}

fn replay_safe_rust_exact(
    source: &str,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("mod api{#![forbid(unsafe_code)]\nuse core::num::NonZeroU32;\n#[repr(u8)] #[derive(Clone,Copy,Debug,Eq,PartialEq)] pub enum NativeRustStatusClass{Semantic=1,Contract=2,Import=3,Adapter=4}\n");
    if !imports.is_empty() {
        replay.text("pub enum NativeRustImportResult<T>{Success(T),Status{code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailure}\n");
    }
    replay.text("pub enum NativeRustCallError{Semantic{domain_id:&'static str,code:NonZeroU32,class:NativeRustStatusClass,retryable:bool},HostFailed,HostPanicked,AdapterRejected}\npub struct NativeRustAdmissionError;\n");
    replay.text("pub trait NativeRustImports{");
    for import in imports {
        replay.text("fn ");
        replay.text(&import.rust_method);
        replay.text("(&mut self");
        if !import.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &import.parameters);
        }
        replay.text(")->NativeRustImportResult<");
        replay_rust_scalar(&mut replay, import.result);
        replay.text(">;");
    }
    replay.text("}\nconst EXPECTED_CAPABILITIES:&[&str]=&[");
    for (index, capability) in spec.capabilities.iter().enumerate() {
        if index != 0 {
            replay.text(",");
        }
        replay.json(capability);
    }
    replay.text("];\n");
    replay.text("pub struct NativeRustCapabilities{digest:[u8;32]} impl NativeRustCapabilities{pub fn new(values:&[&str])->Result<Self,NativeRustAdmissionError>{if values!=EXPECTED_CAPABILITIES{return Err(NativeRustAdmissionError)}Ok(Self{digest:super::ffi::capabilities_digest()})}}\n");
    replay.text("struct ActiveGuard<'a>{active:&'a mut bool}impl Drop for ActiveGuard<'_>{fn drop(&mut self){*self.active=false;}}\npub struct NativeRustBridge<H:NativeRustImports>{host:H,capabilities:NativeRustCapabilities,owner:std::thread::ThreadId,active:bool,calls:u32,_not_send_sync:core::marker::PhantomData<*mut ()>} impl<H:NativeRustImports> NativeRustBridge<H>{pub fn new(host:H,capabilities:NativeRustCapabilities)->Self{Self{host,capabilities,owner:std::thread::current().id(),active:false,calls:0,_not_send_sync:core::marker::PhantomData}}\n");
    for export in exports {
        replay.text("pub fn ");
        replay.text(&export.rust_method);
        replay.text("(&mut self");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &export.parameters);
        }
        replay.text(")->Result<");
        replay_rust_scalar(&mut replay, export.result);
        replay.text(",NativeRustCallError>{if self.owner!=std::thread::current().id()||core::mem::replace(&mut self.active,true){return Err(NativeRustCallError::AdapterRejected)}let _active_guard=ActiveGuard{active:&mut self.active};super::ffi::");
        replay.text(&export.rust_method);
        replay.text("(&mut self.host,&mut self.calls,self.capabilities.digest");
        for index in 0..export.parameters.len() {
            replay.text(", arg_");
            replay.number(index);
        }
        replay.text(")}\n");
    }
    replay.text("}\n}\n#[path=\"semaprax_native_rust_interop_ffi.rs\"]mod ffi;\npub use api::*;\n");
    replay.finish()
}

fn replay_ffi_wire_scalar(replay: &mut ExactReplay<'_>, ty: ScalarType) {
    replay.text(match ty {
        ScalarType::I64 => "i64",
        ScalarType::Bool => "u8",
        ScalarType::Unit => "()",
    });
}

fn replay_private_ffi_exact(
    source: &str,
    spec: &Spec,
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> bool {
    let mut replay = ExactReplay::new(source);
    replay.text("#![allow(unsafe_code)]\nuse super::api::*;\nuse core::ffi::c_void;\n#[repr(C)]struct Imports{abi_version:u32,size:u32,");
    for import in imports {
        replay.text(&import.c_field);
        replay.text(":unsafe extern \"C\" fn(*mut c_void");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", " } else { "," });
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if import.result != ScalarType::Unit {
            replay.text(", *mut ");
            replay_ffi_wire_scalar(&mut replay, import.result);
        }
        replay.text(")->u64,");
    }
    replay.text(" }\n#[repr(C)]struct Context{abi_version:u32,size:u32,userdata:*mut c_void,imports:*const Imports,capabilities_digest:[u8;32],call_depth:u32,reserved:u32}\n");
    if !imports.is_empty() {
        replay.text("struct Frame<H>{host:*mut H,calls:*mut u32}\n");
    }
    replay.text("pub(super) fn capabilities_digest()->[u8;32]{[");
    let digest = replay_capabilities_digest(&spec.capabilities);
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    if hex.len() != 64 {
        return false;
    }
    for index in (0..64).step_by(2) {
        if index != 0 {
            replay.text(",");
        }
        replay.text("0x");
        replay.text(&hex[index..index + 2]);
    }
    replay.text("]}\n");
    if !imports.is_empty() {
        replay.text("fn adapter(code:u32)->u64{((65535u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|u64::from(code)}\n");
    }
    replay.text("fn decode_status(status:u64)->NativeRustCallError{let code=(status&0xffff_ffff)as u32;let class=((status>>32)&0xff)as u8;let retryable=((status>>40)&1)!=0;let reserved=(status>>41)&0x7f;let domain=(status>>48)as u16;let Some(code)=core::num::NonZeroU32::new(code)else{return NativeRustCallError::AdapterRejected};let class=match class{1=>NativeRustStatusClass::Semantic,2=>NativeRustStatusClass::Contract,3=>NativeRustStatusClass::Import,4=>NativeRustStatusClass::Adapter,_=>return NativeRustCallError::AdapterRejected};if reserved!=0||domain==0{return NativeRustCallError::AdapterRejected}match domain{65533=>{let valid=!retryable&&match class{NativeRustStatusClass::Semantic=>(1..=6).contains(&code.get()),NativeRustStatusClass::Contract=>(1..=2).contains(&code.get()),_=>false};if !valid{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:\"semaprax.native-rust-semantics.v1\",code,class,retryable}},");
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .cloned()
        .collect::<BTreeSet<_>>();
    for (index, domain) in domains.iter().enumerate() {
        replay.number(index + 1);
        replay.text("=>{if class!=NativeRustStatusClass::Import{return NativeRustCallError::AdapterRejected}NativeRustCallError::Semantic{domain_id:");
        replay.json(domain);
        replay.text(",code,class,retryable}},");
    }
    replay.text("65534=>if class==NativeRustStatusClass::Adapter&&!retryable{match code.get(){1=>NativeRustCallError::HostPanicked,2=>NativeRustCallError::HostFailed,_=>NativeRustCallError::AdapterRejected}}else{NativeRustCallError::AdapterRejected},65535=>if class==NativeRustStatusClass::Adapter&&!retryable&&(1..=8).contains(&code.get()){NativeRustCallError::AdapterRejected}else{NativeRustCallError::AdapterRejected},_=>NativeRustCallError::AdapterRejected}}\n");
    for import in imports {
        replay.text("unsafe extern \"C\" fn cb_");
        replay.text(&import.rust_method);
        replay.text("<H:NativeRustImports>(userdata:*mut c_void");
        for (index, parameter) in import.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", arg_" } else { ",arg_" });
            replay.number(index);
            replay.text(":");
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if import.result != ScalarType::Unit {
            replay.text(", result_out:*mut ");
            replay_ffi_wire_scalar(&mut replay, import.result);
        }
        replay.text(") -> u64{if userdata.is_null(){return adapter(1);}");
        for (index, parameter) in import.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                replay.text("if arg_");
                replay.number(index);
                replay.text(">1{return adapter(4);}");
            }
        }
        if import.result != ScalarType::Unit {
            replay.text("if result_out.is_null()||(result_out as usize)%core::mem::align_of::<");
            replay_rust_scalar(&mut replay, import.result);
            replay.text(">()!=0{return adapter(5);}");
        }
        replay.text("if (userdata as usize)%core::mem::align_of::<Frame<H>>()!=0{return adapter(1);}let frame=&mut*(userdata as *mut Frame<H>);if frame.host.is_null()||frame.calls.is_null()||*frame.calls>=4096{return adapter(7);}*frame.calls+=1;let run=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{let host=&mut *frame.host;host.");
        replay.text(&import.rust_method);
        replay.text("(");
        for (index, parameter) in import.parameters.iter().enumerate() {
            if index != 0 {
                replay.text(",");
            }
            replay.text("arg_");
            replay.number(index);
            if parameter.ty == ScalarType::Bool {
                replay.text("!=0");
            }
        }
        replay.text(")}));match run{Err(payload)=>{core::mem::forget(payload);((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|1},Ok(NativeRustImportResult::HostFailure)=>((65534u64)<<48)|((NativeRustStatusClass::Adapter as u64)<<32)|2,");
        if let Some(domain) = &import.failure {
            let Some(ordinal) = domains.iter().position(|value| value == domain) else {
                return false;
            };
            replay.text("Ok(NativeRustImportResult::Status{code,class,retryable})=>if class==NativeRustStatusClass::Import{((");
            replay.number(ordinal + 1);
            replay.text("u64)<<48)|((class as u64)<<32)|((retryable as u64)<<40)|u64::from(code.get())}else{adapter(3)},");
        } else {
            replay.text("Ok(NativeRustImportResult::Status{code,class,retryable})=>{let _=(code,class,retryable);adapter(3)},");
        }
        if import.result == ScalarType::Unit {
            replay.text("Ok(NativeRustImportResult::Success(()))=>0}}}\n");
        } else {
            replay.text("Ok(NativeRustImportResult::Success(value))=>{*result_out=");
            if import.result == ScalarType::Bool {
                replay.text("u8::from(value)");
            } else {
                replay.text("value");
            }
            replay.text(";0}}}\n");
        }
    }
    for export in exports {
        replay.text("extern \"C\"{fn ");
        replay.text(&export.c_symbol);
        replay.text("(ctx:*const Context");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", arg_" } else { ",arg_" });
            replay.number(index);
            replay.text(":");
            replay_ffi_wire_scalar(&mut replay, parameter.ty);
        }
        if export.result != ScalarType::Unit {
            replay.text(", result_out:*mut ");
            replay_ffi_wire_scalar(&mut replay, export.result);
        }
        replay.text(")->u64;}\n");
    }
    for export in exports {
        replay.text("pub(super) fn ");
        replay.text(&export.rust_method);
        replay.text("<H:NativeRustImports>(host:&mut H,calls:&mut u32,digest:[u8;32]");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay_rust_parameters(&mut replay, &export.parameters);
        }
        replay.text(")->Result<");
        replay_rust_scalar(&mut replay, export.result);
        replay.text(",NativeRustCallError>{unsafe{if *calls>=4096{return Err(NativeRustCallError::AdapterRejected)}*calls+=1;let table=Imports{abi_version:1,size:core::mem::size_of::<Imports>() as u32,");
        for import in imports {
            replay.text(&import.c_field);
            replay.text(":cb_");
            replay.text(&import.rust_method);
            replay.text("::<H>,");
        }
        replay.text("};");
        if imports.is_empty() {
            replay.text("let _=host;let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:core::ptr::null_mut(),imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};");
        } else {
            replay.text("let mut frame=Frame{host:host as *mut H,calls:calls as *mut u32};let ctx=Context{abi_version:1,size:core::mem::size_of::<Context>() as u32,userdata:&mut frame as *mut Frame<H> as *mut c_void,imports:&table,capabilities_digest:digest,call_depth:0,reserved:0};");
        }
        match export.result {
            ScalarType::Unit => {}
            ScalarType::I64 => {
                replay.text("let mut result=core::mem::MaybeUninit::<i64>::uninit();")
            }
            ScalarType::Bool => {
                replay.text("let mut result=core::mem::MaybeUninit::<u8>::uninit();")
            }
        }
        replay.text("let status=");
        replay.text(&export.c_symbol);
        replay.text("(&ctx");
        for (index, parameter) in export.parameters.iter().enumerate() {
            replay.text(if index == 0 { ", " } else { "," });
            if parameter.ty == ScalarType::Bool {
                replay.text("u8::from(");
            }
            replay.text("arg_");
            replay.number(index);
            if parameter.ty == ScalarType::Bool {
                replay.text(")");
            }
        }
        if export.result != ScalarType::Unit {
            replay.text(", result.as_mut_ptr()");
        }
        replay.text(");if status!=0{return Err(decode_status(status))}");
        match export.result {
            ScalarType::Unit => replay.text("Ok(())"),
            ScalarType::I64 => replay.text("Ok(result.assume_init())"),
            ScalarType::Bool => replay.text("let value=result.assume_init();if value>1{return Err(NativeRustCallError::AdapterRejected)}Ok(value!=0)"),
        }
        replay.text(" }}\n");
    }
    replay.finish()
}

fn replay_c_scalar(ty: ScalarType) -> &'static str {
    match ty {
        ScalarType::I64 => "int64_t",
        ScalarType::Bool => "uint8_t",
        ScalarType::Unit => "void",
    }
}

fn replay_symbol_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(encoded, "{byte:02x}").unwrap();
    }
    #[cfg(test)]
    note_post_hir_replay_capacity(encoded.capacity());
    encoded
}

fn replay_capabilities_digest(capabilities: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CAPABILITIES_DOMAIN);
    for capability in capabilities {
        hasher.update(
            u64::try_from(capability.len())
                .unwrap_or(u64::MAX)
                .to_be_bytes(),
        );
        hasher.update(capability.as_bytes());
    }
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    #[cfg(test)]
    note_post_hir_replay_capacity(digest.capacity());
    digest
}

fn replay_resolved_scalar(ty: &ResolvedType) -> Option<ScalarType> {
    match ty {
        ResolvedType::Unit => Some(ScalarType::Unit),
        ResolvedType::I64 => Some(ScalarType::I64),
        ResolvedType::Bool => Some(ScalarType::Bool),
        _ => None,
    }
}

fn replay_parameter_facts(function: &ResolvedFunction) -> Result<Vec<ParameterFact>, Diagnostic> {
    if function.params.len() > MAX_PARAMETERS {
        return Err(b109("max_parameters", MAX_PARAMETERS));
    }
    function
        .params
        .iter()
        .map(|parameter| {
            if parameter.ownership != OwnershipMode::Value
                || parameter.name.len() > MAX_IDENTIFIER_BYTES
            {
                return Err(b107("scalar value signature required"));
            }
            Ok(ParameterFact {
                name: parameter.name.clone(),
                ty: replay_resolved_scalar(&parameter.ty)
                    .filter(|ty| *ty != ScalarType::Unit)
                    .ok_or_else(|| b107("scalar value signature required"))?,
            })
        })
        .collect()
}

fn replay_c_parameters(parameters: &[ParameterFact]) -> String {
    let values = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| format!("{} arg_{index}", replay_c_scalar(parameter.ty)))
        .collect::<Vec<_>>();
    let joined = values.join(", ");
    #[cfg(test)]
    note_post_hir_replay_capacity(
        string_slice_owned_capacity(&values).saturating_add(joined.capacity()),
    );
    joined
}

#[cfg(any())]
fn replay_c_expression(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr, usize),
        Unary(crate::ast::UnaryOp, usize),
        BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        BinaryRight(crate::ast::BinaryOp, String, usize),
        LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        LazyRight(crate::ast::BinaryOp, String, usize, usize),
        Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType, usize),
        IfThen(String, &'a ResolvedExpr, Option<String>, usize, usize),
        IfElse(String, Option<String>, String, usize, usize, usize),
        NativeArgs(
            &'a crate::hir::ResolvedNativeRustImportCall,
            usize,
            Vec<String>,
            usize,
        ),
        CallArgs(
            &'a str,
            &'a [ResolvedExpr],
            &'a ResolvedType,
            usize,
            Vec<String>,
            usize,
        ),
    }
    const _: () = assert!(std::mem::size_of::<Frame<'static>>() == C_EXPRESSION_FRAME_BYTES);
    let next_temporary = |count: &mut usize| {
        let value = format!("tmp_{}", *count);
        *count += 1;
        value
    };
    let (node_count, depth) = c_expression_shape(expression)?;
    let line_capacity = node_count
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if lines.capacity() < line_capacity {
        lines
            .try_reserve_exact(line_capacity - lines.capacity())
            .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let frame_capacity = node_count
        .checked_mul(2)
        .and_then(|slots| slots.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::<String>::with_capacity(depth + 1);
    let mut contexts = Vec::<Vec<String>>::with_capacity(node_count + 1);
    contexts.push(Vec::with_capacity(line_capacity));
    frames.push(Frame::Enter(expression, 0));
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let frame_payload = |frame: &Frame<'_>| match frame {
                Frame::BinaryRight(_, value, _)
                | Frame::LazyRight(_, value, _, _)
                | Frame::IfThen(value, _, _, _, _)
                | Frame::IfElse(value, _, _, _, _, _) => value.capacity(),
                Frame::NativeArgs(_, _, values, _) | Frame::CallArgs(_, _, _, _, values, _) => {
                    values.capacity() * std::mem::size_of::<String>()
                        + values.iter().map(String::capacity).sum::<usize>()
                }
                _ => 0,
            };
            let frame_owned = frames.iter().map(&frame_payload).sum::<usize>();
            let owned = frames.capacity() * std::mem::size_of::<Frame<'_>>()
                + frame_owned
                + frame_payload(&frame)
                + values.capacity() * std::mem::size_of::<String>()
                + values.iter().map(String::capacity).sum::<usize>()
                + contexts.capacity() * std::mem::size_of::<Vec<String>>()
                + contexts
                    .iter()
                    .map(|context| {
                        context.capacity() * std::mem::size_of::<String>()
                            + context.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>()
                + lines.capacity() * std::mem::size_of::<String>()
                + lines.iter().map(String::capacity).sum::<usize>();
            note_post_hir_replay_capacity(owned);
        }
        match frame {
            Frame::Enter(expression, context) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                    values.push(format!("v_{}", replay_symbol_hash(place.root.as_str())))
                }
                ResolvedExprKind::NativeRustImportCall(call) => frames.push(Frame::NativeArgs(
                    call,
                    0,
                    Vec::with_capacity(call.args.len()),
                    context,
                )),
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(Frame::Unary(*op, context));
                    frames.push(Frame::Enter(value, context));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(Frame::LazyLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(Frame::BinaryLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(Frame::Block(statements, 0, tail, context));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = replay_resolved_scalar(&expression.ty).ok_or_else(b111)?;
                    frames.push(Frame::IfCondition(then_branch, else_branch, ty, context));
                    frames.push(Frame::Enter(condition, context));
                }
                ResolvedExprKind::Call { callee, args, .. } => frames.push(Frame::CallArgs(
                    callee.as_str(),
                    args,
                    &expression.ty,
                    0,
                    Vec::with_capacity(args.len()),
                    context,
                )),
                _ => return Err(b107("scalar value signature required")),
            },
            Frame::Unary(op, context) => {
                let value = values.pop().ok_or_else(b111)?;
                if op == crate::ast::UnaryOp::Not {
                    values.push(format!("(!({value}))"));
                } else {
                    let name = next_temporary(temporary_count);
                    contexts[context].push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                    values.push(name);
                }
            }
            Frame::BinaryLeft(op, right, context) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(Frame::BinaryRight(op, left, context));
                frames.push(Frame::Enter(right, context));
            }
            Frame::BinaryRight(op, left, context) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = next_temporary(temporary_count);
                    contexts[context].push(format!("int64_t {name};"));
                    contexts[context].push(match op {
                        crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    });
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            Frame::LazyLeft(op, right, context) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = next_temporary(temporary_count);
                contexts[context].push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
                let branch = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::LazyRight(op, name, context, branch));
                frames.push(Frame::Enter(right, branch));
            }
            Frame::LazyRight(op, name, context, branch) => {
                let right = values.pop().ok_or_else(b111)?;
                let branch_lines = take_c_lines(&mut contexts[branch]);
                contexts[branch] = Vec::new();
                let condition = if op == crate::ast::BinaryOp::And {
                    name.clone()
                } else {
                    format!("!{name}")
                };
                contexts[context].push(format!(
                    "if({condition}){{{branch_lines} {name}=({right})?UINT8_C(1):UINT8_C(0);}}"
                ));
                values.push(name);
            }
            Frame::Block(statements, index, tail, context) => {
                if let Some(ResolvedStatement::Let { value, .. }) = statements.get(index) {
                    frames.push(Frame::BlockLet(statements, index, tail, context));
                    frames.push(Frame::Enter(value, context));
                } else {
                    frames.push(Frame::Enter(tail, context));
                }
            }
            Frame::BlockLet(statements, index, tail, context) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index];
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::IfCondition(then_branch, else_branch, ty, context) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = (ty != ScalarType::Unit).then(|| next_temporary(temporary_count));
                if let Some(name) = &name {
                    contexts[context].push(format!("{} {name};", replay_c_scalar(ty)));
                }
                let then_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfThen(
                    condition,
                    else_branch,
                    name,
                    context,
                    then_context,
                ));
                frames.push(Frame::Enter(then_branch, then_context));
            }
            Frame::IfThen(condition, else_branch, name, context, then_context) => {
                let then_value = values.pop().ok_or_else(b111)?;
                let else_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfElse(
                    condition,
                    name,
                    then_value,
                    context,
                    then_context,
                    else_context,
                ));
                frames.push(Frame::Enter(else_branch, else_context));
            }
            Frame::IfElse(condition, name, then_value, context, then_context, else_context) => {
                let else_value = values.pop().ok_or_else(b111)?;
                let then_lines = take_c_lines(&mut contexts[then_context]);
                let else_lines = take_c_lines(&mut contexts[else_context]);
                contexts[then_context] = Vec::new();
                contexts[else_context] = Vec::new();
                if let Some(name) = name {
                    contexts[context].push(format!("if({condition}){{{then_lines}{name}={then_value};}}else{{{else_lines}{name}={else_value};}}"));
                    values.push(name);
                } else {
                    contexts[context].push(format!(
                        "if({condition}){{{then_lines}}}else{{{else_lines}}}"
                    ));
                    values.push("INT64_C(0)".to_owned());
                }
            }
            Frame::NativeArgs(call, index, mut args, context) => {
                if index < call.args.len() {
                    if index > 0 {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::NativeArgs(call, index + 1, args, context));
                    frames.push(Frame::Enter(&call.args[index], context));
                } else {
                    if !call.args.is_empty() {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = if import.result == ScalarType::Unit {
                        format!("tmp_{}", *temporary_count)
                    } else {
                        next_temporary(temporary_count)
                    };
                    if import.result != ScalarType::Unit {
                        contexts[context]
                            .push(format!("{} {name};", replay_c_scalar(import.result)));
                    }
                    contexts[context].push(format!("status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.c_field, if args.is_empty() { "" } else { ", " }, args.join(", "), if import.result == ScalarType::Unit { String::new() } else { format!(", &{name}") }, import.rust_method));
                    if import.result == ScalarType::Bool {
                        contexts[context]
                            .push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
                    }
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            Frame::CallArgs(callee, args_source, ty, index, mut args, context) => {
                if index < args_source.len() {
                    if index > 0 {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::CallArgs(
                        callee,
                        args_source,
                        ty,
                        index + 1,
                        args,
                        context,
                    ));
                    frames.push(Frame::Enter(&args_source[index], context));
                } else {
                    if !args_source.is_empty() {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        contexts[context].push(format!(
                            "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                            replay_symbol_hash(callee),
                            if args.is_empty() { "" } else { ", " },
                            args.join(",")
                        ));
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = next_temporary(temporary_count);
                        contexts[context].push(format!("{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;", replay_c_scalar(replay_resolved_scalar(ty).ok_or_else(b111)?), replay_symbol_hash(callee), if args.is_empty() { "" } else { ", " }, args.join(",")));
                        values.push(name);
                    }
                }
            }
        }
    }
    if values.len() != 1 {
        return Err(b111());
    }
    move_root_c_lines(lines, &mut contexts);
    values.pop().ok_or_else(b111)
}

fn replay_c_expression_child(expression: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::Call { args, .. } => args.get(index),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
        ResolvedExprKind::Unary { value, .. } => (index == 0).then_some(value),
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
        _ => None,
    }
}

fn replay_c_expression_shape(expression: &ResolvedExpr) -> Result<(usize, usize), Diagnostic> {
    let mut pending = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    pending[0] = Some((expression, 0usize, 1usize));
    let mut pending_len = 1usize;
    let mut nodes = 0usize;
    let mut maximum_depth = 1usize;
    while pending_len != 0 {
        let (node, child_index, node_depth) = pending[pending_len - 1].take().ok_or_else(b111)?;
        pending_len -= 1;
        if child_index == 0 {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            maximum_depth = maximum_depth.max(node_depth);
        }
        if let Some(child) = replay_c_expression_child(node, child_index) {
            if pending_len + 2 > pending.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            pending[pending_len] = Some((node, child_index + 1, node_depth));
            pending[pending_len + 1] = Some((child, 0, node_depth + 1));
            pending_len += 2;
        }
    }
    Ok((nodes, maximum_depth))
}

fn replay_c_frame_payload(frame: &ReplayCExpressionFrame<'_>) -> usize {
    match frame {
        ReplayCExpressionFrame::FinishBinary(_, value)
        | ReplayCExpressionFrame::FinishLazy(value) => value.capacity(),
        ReplayCExpressionFrame::FinishThen(_, value)
        | ReplayCExpressionFrame::FinishElse(value) => value.as_ref().map_or(0, String::capacity),
        _ => 0,
    }
}

#[allow(clippy::ptr_arg)] // Exact Vec capacities are part of the scratch proof.
fn note_replay_c_expression_scratch(
    current: &ReplayCExpressionFrame<'_>,
    frames: &Vec<ReplayCExpressionFrame<'_>>,
    values: &Vec<String>,
    arguments: &Vec<String>,
    lines: &CExpressionLineArena,
) -> Result<(), Diagnostic> {
    #[cfg(not(test))]
    let _ = lines;
    let mut string_payload = replay_c_frame_payload(current);
    for frame in frames {
        string_payload = string_payload
            .checked_add(replay_c_frame_payload(frame))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for value in values.iter().chain(arguments) {
        string_payload = string_payload
            .checked_add(value.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    if string_payload > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    #[cfg(test)]
    note_post_hir_replay_capacity(
        frames
            .capacity()
            .saturating_mul(REPLAY_C_EXPRESSION_FRAME_BYTES)
            .saturating_add(
                values
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                arguments
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(lines.retained_bytes())
            .saturating_add(string_payload),
    );
    Ok(())
}

fn replay_write_c_arguments(
    lines: &mut CExpressionLineArena,
    arguments: &[String],
    separator: &str,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            lines
                .write_str(separator)
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
        }
        lines
            .write_str(argument)
            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
    }
    Ok(())
}

fn replay_c_expression_linear_independent(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    let (node_count, depth) = replay_c_expression_shape(expression)?;
    let capacity = depth
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(capacity);
    let mut values = Vec::<String>::with_capacity(capacity);
    let mut arguments = Vec::<String>::with_capacity(node_count);
    frames.push(ReplayCExpressionFrame::Evaluate(expression));
    while let Some(frame) = frames.pop() {
        note_replay_c_expression_scratch(&frame, &frames, &values, &arguments, lines)?;
        match frame {
            ReplayCExpressionFrame::Evaluate(expression) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                    values.push(format!("v_{}", replay_symbol_hash(place.root.as_str())))
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(ReplayCExpressionFrame::ContinueNative(
                        call,
                        0,
                        arguments.len(),
                    ));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(ReplayCExpressionFrame::FinishUnary(*op));
                    frames.push(ReplayCExpressionFrame::Evaluate(value));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(ReplayCExpressionFrame::FinishLazyLeft(*op, right));
                    frames.push(ReplayCExpressionFrame::Evaluate(left));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(ReplayCExpressionFrame::FinishBinaryLeft(*op, right));
                    frames.push(ReplayCExpressionFrame::Evaluate(left));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(ReplayCExpressionFrame::ContinueBlock(statements, 0, tail));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = replay_resolved_scalar(&expression.ty).ok_or_else(b111)?;
                    frames.push(ReplayCExpressionFrame::FinishCondition(
                        then_branch,
                        else_branch,
                        ty,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(condition));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(ReplayCExpressionFrame::ContinueCall(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        arguments.len(),
                    ));
                }
                _ => return Err(b107("scalar value signature required")),
            },
            ReplayCExpressionFrame::FinishUnary(op) => {
                let value = values.pop().ok_or_else(b111)?;
                if op == crate::ast::UnaryOp::Not {
                    values.push(format!("(!({value}))"));
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                }
            }
            ReplayCExpressionFrame::FinishBinaryLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(ReplayCExpressionFrame::FinishBinary(op, left));
                frames.push(ReplayCExpressionFrame::Evaluate(right));
            }
            ReplayCExpressionFrame::FinishBinary(op, left) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "int64_t {name};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    match op {
                        crate::ast::BinaryOp::Add => write!(lines, "if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => write!(lines, "if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => write!(lines, "if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    }
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            ReplayCExpressionFrame::FinishLazyLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                write!(
                    lines,
                    "uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);if({}){{",
                    if op == crate::ast::BinaryOp::And {
                        name.clone()
                    } else {
                        format!("!{name}")
                    }
                )
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishLazy(name));
                frames.push(ReplayCExpressionFrame::Evaluate(right));
            }
            ReplayCExpressionFrame::FinishLazy(name) => {
                let right = values.pop().ok_or_else(b111)?;
                write!(lines, " {name}=({right})?UINT8_C(1):UINT8_C(0);}}")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                values.push(name);
            }
            ReplayCExpressionFrame::ContinueBlock(statements, index, tail) => {
                if let Some(ResolvedStatement::Let { value, .. }) = statements.get(index) {
                    frames.push(ReplayCExpressionFrame::FinishBinding(
                        statements, index, tail,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(value));
                } else {
                    frames.push(ReplayCExpressionFrame::Evaluate(tail));
                }
            }
            ReplayCExpressionFrame::FinishBinding(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index];
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(ReplayCExpressionFrame::ContinueBlock(
                    statements,
                    index + 1,
                    tail,
                ));
            }
            ReplayCExpressionFrame::FinishCondition(then_branch, else_branch, ty) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "{} {name};", replay_c_scalar(ty))
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    Some(name)
                };
                write!(lines, "if({condition}){{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishThen(else_branch, name));
                frames.push(ReplayCExpressionFrame::Evaluate(then_branch));
            }
            ReplayCExpressionFrame::FinishThen(else_branch, name) => {
                let then_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = &name {
                    write!(lines, "{name}={then_value};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                lines
                    .write_str("}else{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishElse(name));
                frames.push(ReplayCExpressionFrame::Evaluate(else_branch));
            }
            ReplayCExpressionFrame::FinishElse(name) => {
                let else_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = name {
                    write!(lines, "{name}={else_value};}}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    lines
                        .write_str("}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push("INT64_C(0)".to_owned());
                }
            }
            ReplayCExpressionFrame::ContinueNative(call, index, start) => {
                if index < call.args.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(ReplayCExpressionFrame::ContinueNative(
                        call,
                        index + 1,
                        start,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(&call.args[index]));
                } else {
                    if !call.args.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = format!("tmp_{}", *temporary_count);
                    if import.result != ScalarType::Unit {
                        *temporary_count += 1;
                        write!(lines, "{} {name};", replay_c_scalar(import.result))
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(
                        lines,
                        "status = ctx->imports->{}(ctx->userdata",
                        import.c_field
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if start < arguments.len() {
                        lines
                            .write_str(", ")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        replay_write_c_arguments(lines, &arguments[start..], ", ")?;
                    }
                    if import.result != ScalarType::Unit {
                        write!(lines, ", &{name}")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(lines, "); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.rust_method)
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if import.result == ScalarType::Bool {
                        write!(lines, "if ({name} > UINT8_C(1)) return spxnr_adapter(4);")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    arguments.truncate(start);
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            ReplayCExpressionFrame::ContinueCall(callee, source, ty, index, start) => {
                if index < source.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(ReplayCExpressionFrame::ContinueCall(
                        callee,
                        source,
                        ty,
                        index + 1,
                        start,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(&source[index]));
                } else {
                    if !source.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        write!(lines, "status=spxnr1_f_{}(ctx", replay_symbol_hash(callee))
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start < arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            replay_write_c_arguments(lines, &arguments[start..], ",")?;
                        }
                        lines
                            .write_str(");if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};status=spxnr1_f_{}(ctx",
                            replay_c_scalar(replay_resolved_scalar(ty).ok_or_else(b111)?),
                            replay_symbol_hash(callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start < arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            replay_write_c_arguments(lines, &arguments[start..], ",")?;
                        }
                        write!(lines, ",&{name});if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    arguments.truncate(start);
                }
            }
        }
    }
    let terminal = ReplayCExpressionFrame::Evaluate(expression);
    note_replay_c_expression_scratch(&terminal, &frames, &values, &arguments, lines)?;
    if values.len() != 1 || !arguments.is_empty() {
        return Err(b111());
    }
    values.pop().ok_or_else(b111)
}

fn replay_c_expression(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    replay_c_expression_linear_independent(expression, imports, temporary_count, lines)
}

// Kept out of every build: the iterative generator above is the sole replay
// evaluator. This source reference makes authored formatting changes easy to
// audit while preventing a recursive production route from reappearing.
#[cfg(any())]
fn replay_c_expression_recursive_reference(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(if *value == i64::MIN {
            "INT64_MIN".to_owned()
        } else {
            format!("INT64_C({value})")
        }),
        ResolvedExprKind::Bool(value) => Ok(if *value {
            "UINT8_C(1)".to_owned()
        } else {
            "UINT8_C(0)".to_owned()
        }),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => {
            Ok(format!("v_{}", replay_symbol_hash(place.root.as_str())))
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let import = imports
                .iter()
                .find(|item| item.id == call.import.as_str())
                .ok_or_else(b111)?;
            let args = call
                .args
                .iter()
                .map(|arg| replay_c_expression(arg, imports, temporary_count, lines))
                .collect::<Result<Vec<_>, _>>()?;
            let name = format!("tmp_{}", *temporary_count);
            if import.result != ScalarType::Unit {
                lines.push(format!("{} {name};", replay_c_scalar(import.result)));
                *temporary_count += 1;
            }
            lines.push(format!(
                "status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}",
                import.c_field,
                if args.is_empty() { "" } else { ", " },
                args.join(", "),
                if import.result == ScalarType::Unit {
                    String::new()
                } else {
                    format!(", &{name}")
                },
                import.rust_method,
            ));
            if import.result == ScalarType::Bool {
                lines.push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
            }
            Ok(if import.result == ScalarType::Unit {
                "INT64_C(0)".to_owned()
            } else {
                name
            })
        }
        ResolvedExprKind::Unary { op, value } => {
            let value = replay_c_expression(value, imports, temporary_count, lines)?;
            match op {
                crate::ast::UnaryOp::Neg => {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    lines.push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                    Ok(name)
                }
                crate::ast::UnaryOp::Not => Ok(format!("(!({value}))")),
            }
        }
        ResolvedExprKind::Binary {
            op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
            left,
            right,
        } => {
            let left = replay_c_expression(left, imports, temporary_count, lines)?;
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
            let mut branch_lines = Vec::new();
            let right = replay_c_expression(right, imports, temporary_count, &mut branch_lines)?;
            let condition = if matches!(
                expression.kind,
                ResolvedExprKind::Binary {
                    op: crate::ast::BinaryOp::And,
                    ..
                }
            ) {
                name.clone()
            } else {
                format!("!{name}")
            };
            lines.push(format!(
                "if({condition}){{{} {name}=({right})?UINT8_C(1):UINT8_C(0);}}",
                branch_lines.join("")
            ));
            Ok(name)
        }
        ResolvedExprKind::Binary { op, left, right } => {
            let left = replay_c_expression(left, imports, temporary_count, lines)?;
            let right = replay_c_expression(right, imports, temporary_count, lines)?;
            if matches!(
                op,
                crate::ast::BinaryOp::Add
                    | crate::ast::BinaryOp::Sub
                    | crate::ast::BinaryOp::Mul
                    | crate::ast::BinaryOp::Div
                    | crate::ast::BinaryOp::Rem
            ) {
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                lines.push(format!("int64_t {name};"));
                lines.push(match op {
                    crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                    crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                    crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                    crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                    crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                    _ => unreachable!(),
                });
                return Ok(name);
            }
            let operator = match op {
                crate::ast::BinaryOp::Add => "+",
                crate::ast::BinaryOp::Sub => "-",
                crate::ast::BinaryOp::Mul => "*",
                crate::ast::BinaryOp::Div => "/",
                crate::ast::BinaryOp::Rem => "%",
                crate::ast::BinaryOp::Eq => "==",
                crate::ast::BinaryOp::Ne => "!=",
                crate::ast::BinaryOp::Lt => "<",
                crate::ast::BinaryOp::Le => "<=",
                crate::ast::BinaryOp::Gt => ">",
                crate::ast::BinaryOp::Ge => ">=",
                crate::ast::BinaryOp::And => "&&",
                crate::ast::BinaryOp::Or => "||",
            };
            Ok(format!("(({left}) {operator} ({right}))"))
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                let value = replay_c_expression(value, imports, temporary_count, lines)?;
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    lines.push(format!(
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
            }
            replay_c_expression(tail, imports, temporary_count, lines)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = replay_c_expression(condition, imports, temporary_count, lines)?;
            if replay_resolved_scalar(&expression.ty) == Some(ScalarType::Unit) {
                let mut then_lines = Vec::new();
                let _ =
                    replay_c_expression(then_branch, imports, temporary_count, &mut then_lines)?;
                let mut else_lines = Vec::new();
                let _ =
                    replay_c_expression(else_branch, imports, temporary_count, &mut else_lines)?;
                lines.push(format!(
                    "if({condition}){{{}}}else{{{}}}",
                    then_lines.join(""),
                    else_lines.join("")
                ));
                return Ok("INT64_C(0)".to_owned());
            }
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!(
                "{} {name};",
                replay_c_scalar(replay_resolved_scalar(&expression.ty).ok_or_else(b111)?)
            ));
            let mut then_lines = Vec::new();
            let then_value =
                replay_c_expression(then_branch, imports, temporary_count, &mut then_lines)?;
            let mut else_lines = Vec::new();
            let else_value =
                replay_c_expression(else_branch, imports, temporary_count, &mut else_lines)?;
            lines.push(format!(
                "if({condition}){{{}{name}={then_value};}}else{{{}{name}={else_value};}}",
                then_lines.join(""),
                else_lines.join("")
            ));
            Ok(name)
        }
        ResolvedExprKind::Call { callee, args, .. } => {
            let args = args
                .iter()
                .map(|arg| replay_c_expression(arg, imports, temporary_count, lines))
                .collect::<Result<Vec<_>, _>>()?;
            if expression.ty == ResolvedType::Unit {
                lines.push(format!(
                    "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                    replay_symbol_hash(callee.as_str()),
                    if args.is_empty() { "" } else { ", " },
                    args.join(",")
                ));
                return Ok("INT64_C(0)".to_owned());
            }
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!(
                "{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;",
                replay_c_scalar(replay_resolved_scalar(&expression.ty).ok_or_else(b111)?),
                replay_symbol_hash(callee.as_str()),
                if args.is_empty() { "" } else { ", " },
                args.join(",")
            ));
            Ok(name)
        }
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Place(_) => Err(b107("scalar value signature required")),
    }
}

fn replay_c_exact(
    source: &str,
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<bool, Diagnostic> {
    let mut replay = ExactReplay::new(source);
    replay.text("#include \"semaprax_native_rust_interop.h\"\n#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n#include <limits.h>\nstatic const uint8_t spxnr_capabilities[32] = {");
    let digest = replay_capabilities_digest(&spec.capabilities);
    let hex = digest.strip_prefix("sha256:").ok_or_else(b111)?;
    if hex.len() != 64 {
        return Err(b111());
    }
    for index in (0..64).step_by(2) {
        if index != 0 {
            replay.text(",");
        }
        replay.text("0x");
        replay.text(&hex[index..index + 2]);
    }
    replay.text("};\nstatic spxnr_status_v1 spxnr_adapter(uint32_t code){return (((uint64_t)65535)<<48)|(((uint64_t)4)<<32)|code;}\nstatic spxnr_status_v1 spxnr_validate(const spxnr_context_v1 *ctx){if(!ctx||((uintptr_t)ctx%_Alignof(spxnr_context_v1))!=0)return spxnr_adapter(1);if(ctx->abi_version!=1||ctx->size!=sizeof(*ctx)||ctx->reserved!=0)return spxnr_adapter(1);if(!ctx->imports||((uintptr_t)ctx->imports%_Alignof(spxnr_imports_v1))!=0)return spxnr_adapter(2);if(ctx->imports->abi_version!=1||ctx->imports->size!=sizeof(*ctx->imports))return spxnr_adapter(2);if(memcmp(ctx->capabilities_digest,spxnr_capabilities,32)!=0)return spxnr_adapter(3);if(ctx->call_depth>=32)return spxnr_adapter(7);return 0;}\n");
    if !imports.is_empty() {
        replay.text("static int spxnr_status_canonical(spxnr_status_v1 status){if(status==0)return 1;uint32_t code=(uint32_t)status;uint8_t class_=(uint8_t)(status>>32);uint8_t retry=(uint8_t)((status>>40)&1);uint8_t reserved=(uint8_t)((status>>41)&0x7f);uint16_t domain=(uint16_t)(status>>48);if(code==0||reserved!=0||domain==0)return 0;if(domain==65533)return retry==0&&((class_==1&&code>=1&&code<=6)||(class_==2&&code>=1&&code<=2));");
    }
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .collect::<BTreeSet<_>>();
    for (index, _) in domains.iter().enumerate() {
        replay.text("if(domain==");
        replay.number(index + 1);
        replay.text(")return class_==3;");
    }
    if !imports.is_empty() {
        replay.text("if(domain==65534)return class_==4&&retry==0&&code>=1&&code<=2;if(domain==65535)return class_==4&&retry==0&&code>=1&&code<=8;return 0;}\n");
    }
    let ordinals = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    for import in imports {
        replay.text("static int spxnr_status_for_");
        replay.text(&import.rust_method);
        replay.text("(spxnr_status_v1 status){if(!spxnr_status_canonical(status))return 0;uint16_t domain=(uint16_t)(status>>48);return domain==65534||domain==65535");
        if let Some(ordinal) = import
            .failure
            .as_deref()
            .and_then(|domain| ordinals.get(domain).copied())
        {
            replay.text("||domain==");
            replay.number(ordinal);
        }
        replay.text(";}\nstatic spxnr_status_v1 spxnr_validate_");
        replay.text(&import.rust_method);
        replay.text("(const spxnr_context_v1 *ctx){return ctx->imports->");
        replay.text(&import.c_field);
        replay.text("?0:spxnr_adapter(2);}\n");
    }
    for function in closure {
        let parameters = replay_parameter_facts(function)?;
        let result = replay_resolved_scalar(&function.return_type).ok_or_else(b111)?;
        replay.text("static spxnr_status_v1 spxnr1_f_");
        replay.text(&replay_symbol_hash(function.id.as_str()));
        replay.text("(const spxnr_context_v1 *ctx");
        if !parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&parameters));
        }
        if result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(result));
            replay.text(" *result_out");
        }
        replay.text(");\n");
    }
    for function in closure {
        let parameters = replay_parameter_facts(function)?;
        let result = replay_resolved_scalar(&function.return_type).ok_or_else(b111)?;
        replay.text("static spxnr_status_v1 spxnr1_f_");
        replay.text(&replay_symbol_hash(function.id.as_str()));
        replay.text("(const spxnr_context_v1 *ctx");
        if !parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&parameters));
        }
        if result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(result));
            replay.text(" *result_out");
        }
        replay.text(" ){spxnr_status_v1 status=0;(void)ctx;");
        for index in 0..parameters.len() {
            replay.text("(void)arg_");
            replay.number(index);
            replay.text(";");
        }
        for (index, (parameter, resolved)) in parameters.iter().zip(&function.params).enumerate() {
            replay.text(replay_c_scalar(parameter.ty));
            replay.text(" v_");
            replay.text(&replay_symbol_hash(resolved.id.as_str()));
            replay.text("=arg_");
            replay.number(index);
            replay.text(";");
        }
        let mut temporary_count = 0;
        let mut lines = CExpressionLineArena::new();
        for requirement in &function.requires {
            lines.clear();
            let value =
                replay_c_expression(requirement, imports, &mut temporary_count, &mut lines)?;
            replay.text(lines.as_str()?);
            replay.text("if(!(");
            replay.text(&value);
            replay.text("))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(1);");
        }
        lines.clear();
        let value = replay_c_expression(&function.body, imports, &mut temporary_count, &mut lines)?;
        replay.text(lines.as_str()?);
        if result != ScalarType::Unit {
            replay.text(replay_c_scalar(result));
            replay.text(" v_");
            replay.text(&replay_symbol_hash(function.result_id.as_str()));
            replay.text("=");
            replay.text(&value);
            replay.text(";");
        }
        for guarantee in &function.ensures {
            lines.clear();
            let value = replay_c_expression(guarantee, imports, &mut temporary_count, &mut lines)?;
            replay.text(lines.as_str()?);
            replay.text("if(!(");
            replay.text(&value);
            replay.text("))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(2);");
        }
        if result != ScalarType::Unit {
            replay.text("*result_out=v_");
            replay.text(&replay_symbol_hash(function.result_id.as_str()));
            replay.text(";");
        }
        replay.text("return status;}\n");
    }
    for export in exports {
        replay.text("spxnr_status_v1 ");
        replay.text(&export.c_symbol);
        replay.text("(const spxnr_context_v1 *ctx");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&export.parameters));
        }
        if export.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(export.result));
            replay.text(" *result_out");
        }
        replay.text(" ){spxnr_status_v1 status=spxnr_validate(ctx);if(status!=0)return status;");
        for import in imports {
            replay.text("status=spxnr_validate_");
            replay.text(&import.rust_method);
            replay.text("(ctx);if(status!=0)return status;");
        }
        if export.result != ScalarType::Unit {
            replay.text("if(!result_out||((uintptr_t)result_out%_Alignof(");
            replay.text(replay_c_scalar(export.result));
            replay.text("))!=0)return spxnr_adapter(5);");
        }
        for (index, parameter) in export.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                replay.text("if(arg_");
                replay.number(index);
                replay.text(">1)return spxnr_adapter(4);");
            }
        }
        replay.text(
            "spxnr_context_v1 local=*ctx;local.call_depth=ctx->call_depth+1;status=spxnr1_f_",
        );
        replay.text(&replay_symbol_hash(&export.id));
        replay.text("(&local");
        for index in 0..export.parameters.len() {
            replay.text(if index == 0 { ", " } else { "," });
            replay.text("arg_");
            replay.number(index);
        }
        if export.result != ScalarType::Unit {
            replay.text(", result_out");
        }
        replay.text(");return status;}\n");
    }
    Ok(replay.finish())
}

#[allow(clippy::too_many_arguments)]
fn replay_generated_exact(
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
    header: &str,
    c: &str,
    rust: &str,
    ffi: &str,
) -> Result<(), Diagnostic> {
    if !replay_header_exact(header, exports, imports) {
        return Err(b111());
    }
    if !replay_safe_rust_exact(rust, spec, exports, imports)
        || !replay_private_ffi_exact(ffi, spec, exports, imports)
        || !replay_c_exact(c, spec, closure, exports, imports)?
    {
        return Err(b111());
    }
    replay_generated(header, c, rust, ffi)
}

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
