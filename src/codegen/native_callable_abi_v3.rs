//! Canonical, metadata-only callable native adapter descriptor v3.
//!
//! This encoder is intentionally independent from `SPXNABI2` and
//! `SPXNPRF1`: it consumes validated compiler facts and the target-neutral
//! settlement derivation directly, and embeds neither predecessor artifact.
//! The result names future entry points but emits no provider, opens no image,
//! reserves no invocation, and grants no physical-finalizer authority.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable-v3 remains unpublished metadata")
)]

use std::collections::HashMap;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::native_settlement::{
    NativeSettlementCertificate, SettlementOutcome, SettlementProgressAction,
    SettlementResourceState,
};
use crate::trace_path_certificate::TracePathOutcome;

use super::native_callable_wire_v3::{
    self as runtime_wire, ACTION_EVIDENCE_BYTES, ACTION_SCHEMA_STATEMENT, CALL_ABI_STATEMENT,
    CANDIDATE_RECEIPT_SCHEMA_STATEMENT, COMMITTED_RECEIPT_SCHEMA_STATEMENT, DECISION_BYTES,
    DECISION_SCHEMA_STATEMENT, EXECUTE_RESPONSE_SCHEMA_STATEMENT, FRAME_SCHEMA_STATEMENT,
    HOST_RECEIPT_BYTES, MAX_WIRE_BYTES, REQUEST_BOOL_BYTES, REQUEST_FIXED_BYTES, REQUEST_I64_BYTES,
    REQUEST_OWNER_BYTES, REQUEST_SCHEMA_STATEMENT,
};
use super::native_host_contract::{
    NativeAdapterParameterProjection, NativeAdapterResultProjection, NativeHostScalarKind,
};
use super::{
    native_callable_execution, native_cleanup, native_cleanup_emit, native_host_contract,
    native_resource, native_runtime, native_settlement_derivation, native_trace_runtime,
    native_value, NATIVE_SCALAR_RUNTIME_C,
};

const MAGIC: &[u8; 8] = b"SPXNABI3";
const VERSION: u32 = 3;
const HEADER_SIZE: u32 = 20;
const FINGERPRINT_BYTES: usize = 32;
const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;
const MAX_EVENT_COUNT: u32 = 65_536;
const MAX_DICTIONARY_BYTES: u32 = 1024 * 1024;
const MAX_DICTIONARY_ENTRIES: u32 = 65_536;
const MAX_RESOURCE_COUNT: u32 = 4_096;
const MAX_CHECKPOINT_COUNT: u32 = 65_536;
const MAX_GRAPH_WORK_UNITS: u32 = 1_000_000;
const MAX_INSTANCE_RESERVED_BYTES: u32 = 64 * 1024 * 1024;

const LINKAGE_DYNAMIC_IMAGE: u32 = 1;
const LINKAGE_IOS_STATIC_REGISTRATION: u32 = 2;
const CALL_ABI_TAG: u32 = 3;
const CALL_OBLIGATIONS: u32 = 0x03ff;
const ACTIVE_FRAME_LIMIT: u32 = 256;
const QUARANTINED_FRAME_LIMIT: u32 = 64;

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED_RESOURCE: u32 = 2;
const SCALAR_I64: u32 = 1;
const SCALAR_BOOL: u32 = 2;
const OWNED_PAYLOAD_WIRE_KIND: u32 = 1;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

const GRAPH_VERSION: u32 = 3;
const STATE_LIVE: u32 = 1;
const STATE_PROVISIONAL_RESULT: u32 = 2;
const STATE_FINALIZING: u32 = 3;
const STATE_DEAD: u32 = 4;
const STATE_PUBLISHED: u32 = 5;
const OUTCOME_NONE: u32 = 0;
const OUTCOME_SCALAR_SUCCESS: u32 = 1;
const OUTCOME_SEMANTIC_FAILURE: u32 = 2;
const OUTCOME_OWNED_SUCCESS: u32 = 3;
const ACTION_FINALIZE: u32 = 1;
const ACTION_STAGE_OWNED_RESULT: u32 = 2;
const ACTION_CERTIFY_OUTCOME: u32 = 3;

const DESCRIPTOR_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-descriptor-schema.v3\0";
const TARGET_DOMAIN: &[u8] = b"semaprax.native-callable-target.v3\0";
const PHYSICAL_MODULE_DOMAIN: &[u8] = b"semaprax.native-callable-physical-module.v3\0";
const SETTLEMENT_GRAPH_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-graph.v3\0";
const TRACE_EVIDENCE_DOMAIN: &[u8] = b"semaprax.native-recovery-trace-evidence.v1\0";
const REQUEST_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-request-schema.v3\0";
const EXECUTE_RESPONSE_SCHEMA_DOMAIN: &[u8] =
    b"semaprax.native-callable-execute-response-schema.v3\0";
const FRAME_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-frame-schema.v3\0";
const DECISION_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-decision-schema.v3\0";
const ACTION_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-action-schema.v3\0";
const CANDIDATE_RECEIPT_SCHEMA_DOMAIN: &[u8] =
    b"semaprax.native-callable-candidate-receipt-schema.v3\0";
const COMMITTED_RECEIPT_SCHEMA_DOMAIN: &[u8] =
    b"semaprax.native-callable-committed-receipt-schema.v3\0";
const CALL_ABI_DOMAIN: &[u8] = b"semaprax.native-callable-c-abi.v3\0";
const CALL_CONTRACT_DOMAIN: &[u8] = b"semaprax.native-callable-contract.v3\0";
const SYMBOL_SEED_DOMAIN: &[u8] = b"semaprax.native-callable-symbol-seed.v3\0";
const GETTER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-getter.v3\0";
const EXECUTE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-execute.v3\0";
const SETTLE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-settle.v3\0";

// Literal descriptor metadata remains in this module. The runtime schema and
// ABI statements are frozen alongside their canonical compiler encoders.
const DESCRIPTOR_SCHEMA_STATEMENT: &[u8] = b"SPXNABI3;u32le;header=20;sequential-no-offsets-no-trailing;target;linkage-profile;19-fingerprints;module;function;getter;execute;settle;abi-tag;obligations;15-capacities;signature;graph-len;graph";

/// Metadata only. Possession of these bytes or names confers no native-call
/// or physical-finalizer authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableV3Descriptor {
    pub(super) bytes: Vec<u8>,
    pub(super) getter_symbol: String,
    pub(super) execute_symbol: String,
    pub(super) settle_symbol: String,
    pub(super) call_contract: [u8; FINGERPRINT_BYTES],
    pub(super) recovery_contract: [u8; FINGERPRINT_BYTES],
    pub(super) settlement_graph: [u8; FINGERPRINT_BYTES],
    pub(super) trace_path_certificate: [u8; FINGERPRINT_BYTES],
    pub(super) request_bytes: u32,
    pub(super) maximum_events: u32,
    pub(super) dictionary_entries: u32,
    pub(super) resource_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalParameter {
    index: u32,
    value: String,
    kind: CanonicalParameterKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CanonicalParameterKind {
    Scalar(u32),
    Owned {
        ordinal: u32,
        resource: String,
        lifecycle: String,
        payload_wire_kind: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CanonicalResult {
    ScalarI64,
    Owned {
        parameter_index: u32,
        value: String,
        owner_ordinal: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Capacities {
    request: u32,
    execute_response: u32,
    frame: u32,
    decision: u32,
    action_evidence: u32,
    candidate_receipt: u32,
    event_count: u32,
    dictionary_bytes: u32,
    dictionary_entries: u32,
    resource_count: u32,
    checkpoint_count: u32,
    graph_work_units: u32,
    active_frames: u32,
    quarantined_frames: u32,
    instance_reserved_bytes: u32,
}

impl Capacities {
    fn words(&self) -> [u32; 15] {
        [
            self.request,
            self.execute_response,
            self.frame,
            self.decision,
            self.action_evidence,
            self.candidate_receipt,
            self.event_count,
            self.dictionary_bytes,
            self.dictionary_entries,
            self.resource_count,
            self.checkpoint_count,
            self.graph_work_units,
            self.active_frames,
            self.quarantined_frames,
            self.instance_reserved_bytes,
        ]
    }
}

pub(super) fn derive(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeCallableV3Descriptor, Diagnostic> {
    derive_for_target(
        program,
        function_id,
        &physical_target_tag()?,
        physical_linkage_profile(),
    )
}

fn derive_for_target(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    target: &str,
    linkage_profile: u32,
) -> Result<NativeCallableV3Descriptor, Diagnostic> {
    validate_target_profile(target, linkage_profile)?;
    crate::hir::validate(program)?;

    let resource_abi = native_resource::build_resource_abi(program)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| &candidate.id == function_id)
        .ok_or_else(|| v3_error("function is not in the validated program"))?;
    let cleanup = native_cleanup::classify(program, function)?;
    let mut values =
        native_value::plan(program, function, &cleanup, &resource_abi, &HashMap::new())?;
    let dictionary = crate::semantic_trace::build_semantic_event_dictionary(program, function_id)?;
    let trace = crate::trace_path_certificate::build_trace_path_certificate(
        program,
        function,
        &dictionary,
    )?;
    values.cleanup_bindings.semantic_events = Some(dictionary.clone());
    let declarations = native_value::emit_declarations(&values);
    let cleanup_body = native_cleanup_emit::emit_with_block_prologues(
        &cleanup,
        &values.cleanup_bindings,
        |block, output| {
            output.push_str(&native_value::emit_block_prologue(&values, block));
            Ok(())
        },
    )?;
    let mut status_runtime = String::new();
    native_runtime::emit_status_runtime(&mut status_runtime);
    let mut trace_runtime = String::new();
    native_trace_runtime::emit_trace_runtime(&mut trace_runtime);
    let execution = native_callable_execution::plan(
        program,
        function,
        &cleanup,
        &values,
        &resource_abi,
        &dictionary,
        declarations.clone(),
        cleanup_body.clone(),
    )?;
    let (normalized_execution_projection, codec_profile_fingerprint) =
        execution.normalized_projection()?;
    let execution_cleanup = super::native_callable_execution_cleanup_fingerprint(&[
        resource_abi.declarations.as_bytes(),
        status_runtime.as_bytes(),
        trace_runtime.as_bytes(),
        NATIVE_SCALAR_RUNTIME_C.as_bytes(),
        declarations.as_bytes(),
        cleanup_body.as_bytes(),
        &codec_profile_fingerprint,
        normalized_execution_projection.as_bytes(),
    ]);

    let template = native_host_contract::derive_from_admitted(
        program,
        function_id,
        &resource_abi,
        &cleanup,
        &values,
    )?;
    let projection = native_host_contract::project_for_callable_abi(&template);
    let semantic_module = decode_fingerprint(
        &projection.module_abi_fingerprint,
        "semantic module ABI fingerprint",
    )?;
    let function_template = decode_fingerprint(
        &projection.function_template_fingerprint,
        "function-template fingerprint",
    )?;
    let (parameters, result) = canonical_signature(&projection.parameters, &projection.result)?;

    let settlement = native_settlement_derivation::derive_native_settlement(program, function_id)?;
    if settlement.trace_certificate_fingerprint() != trace.fingerprint() {
        return Err(v3_error(
            "execution and settlement derivations disagree on the trace certificate",
        ));
    }
    let graph = encode_graph(
        &settlement,
        execution_cleanup,
        settlement.trace_certificate_fingerprint(),
        MAX_DESCRIPTOR_BYTES,
    )?;

    let schema = framed_fingerprint(DESCRIPTOR_SCHEMA_DOMAIN, DESCRIPTOR_SCHEMA_STATEMENT);
    let target_fingerprint = framed_fingerprint(TARGET_DOMAIN, target.as_bytes());
    let physical_module = physical_module_fingerprint(
        &schema,
        &target_fingerprint,
        &semantic_module,
        projection.module.as_bytes(),
        linkage_profile,
    );
    let settlement_graph = framed_fingerprint(SETTLEMENT_GRAPH_DOMAIN, &graph);
    let request_schema = framed_fingerprint(REQUEST_SCHEMA_DOMAIN, REQUEST_SCHEMA_STATEMENT);
    let execute_response_schema = framed_fingerprint(
        EXECUTE_RESPONSE_SCHEMA_DOMAIN,
        EXECUTE_RESPONSE_SCHEMA_STATEMENT,
    );
    let frame_schema = framed_fingerprint(FRAME_SCHEMA_DOMAIN, FRAME_SCHEMA_STATEMENT);
    let decision_schema = framed_fingerprint(DECISION_SCHEMA_DOMAIN, DECISION_SCHEMA_STATEMENT);
    let action_schema = framed_fingerprint(ACTION_SCHEMA_DOMAIN, ACTION_SCHEMA_STATEMENT);
    let candidate_receipt_schema = framed_fingerprint(
        CANDIDATE_RECEIPT_SCHEMA_DOMAIN,
        CANDIDATE_RECEIPT_SCHEMA_STATEMENT,
    );
    let committed_receipt_schema = framed_fingerprint(
        COMMITTED_RECEIPT_SCHEMA_DOMAIN,
        COMMITTED_RECEIPT_SCHEMA_STATEMENT,
    );
    let call_abi = framed_fingerprint(CALL_ABI_DOMAIN, CALL_ABI_STATEMENT);
    let capacities = derive_capacities(
        &parameters,
        values.required_event_capacity,
        dictionary.canonical_json().len(),
        dictionary.entries().len(),
        settlement.certificate(),
    )?;
    let pre_contract_fingerprints = [
        schema,
        target_fingerprint,
        semantic_module,
        physical_module,
        function_template,
        execution_cleanup,
        dictionary.fingerprint(),
        trace.fingerprint(),
        settlement.recovery_contract_fingerprint(),
        settlement_graph,
        request_schema,
        execute_response_schema,
        frame_schema,
        decision_schema,
        action_schema,
        candidate_receipt_schema,
        committed_receipt_schema,
        call_abi,
    ];
    let call_contract = call_contract_fingerprint(
        target,
        &pre_contract_fingerprints,
        &projection.module,
        &projection.function,
        linkage_profile,
        &capacities,
        &parameters,
        &result,
    );
    let seed = symbol_seed(&[
        physical_module,
        function_template,
        settlement.recovery_contract_fingerprint(),
        settlement_graph,
        request_schema,
        execute_response_schema,
        frame_schema,
        decision_schema,
        action_schema,
        candidate_receipt_schema,
        committed_receipt_schema,
        call_abi,
        call_contract,
    ]);
    let getter_symbol = exact_symbol(&seed, GETTER_SYMBOL_DOMAIN, "descriptor_v3");
    let execute_symbol = exact_symbol(&seed, EXECUTE_SYMBOL_DOMAIN, "execute_v3");
    let settle_symbol = exact_symbol(&seed, SETTLE_SYMBOL_DOMAIN, "settle_v3");
    if getter_symbol == execute_symbol
        || getter_symbol == settle_symbol
        || execute_symbol == settle_symbol
    {
        return Err(v3_error(
            "derived callable-v3 symbols are not pairwise distinct",
        ));
    }

    let mut writer = WireWriter::new(MAX_DESCRIPTOR_BYTES);
    writer.raw(MAGIC)?;
    writer.u32(VERSION)?;
    writer.u32(HEADER_SIZE)?;
    let total_offset = writer.bytes.len();
    writer.u32(0)?;
    writer.text(target, "physical target tag")?;
    writer.u32(linkage_profile)?;
    for fingerprint in pre_contract_fingerprints
        .into_iter()
        .chain(std::iter::once(call_contract))
    {
        require_initialized(&fingerprint, "callable-v3 fingerprint")?;
        writer.raw(&fingerprint)?;
    }
    writer.text(&projection.module, "module identity")?;
    writer.text(&projection.function, "function identity")?;
    writer.text(&getter_symbol, "descriptor getter symbol")?;
    writer.text(&execute_symbol, "execute symbol")?;
    writer.text(&settle_symbol, "settle symbol")?;
    writer.u32(CALL_ABI_TAG)?;
    writer.u32(CALL_OBLIGATIONS)?;
    for capacity in capacities.words() {
        writer.u32(capacity)?;
    }
    encode_signature(&mut writer, &parameters, &result)?;
    writer.u32(wire_u32(graph.len(), "settlement graph byte length")?)?;
    writer.raw(&graph)?;
    let total = wire_u32(writer.bytes.len(), "callable-v3 descriptor byte length")?;
    writer.bytes[total_offset..total_offset + 4].copy_from_slice(&total.to_le_bytes());

    Ok(NativeCallableV3Descriptor {
        bytes: writer.bytes,
        getter_symbol,
        execute_symbol,
        settle_symbol,
        call_contract,
        recovery_contract: settlement.recovery_contract_fingerprint(),
        settlement_graph,
        trace_path_certificate: trace.fingerprint(),
        request_bytes: capacities.request,
        maximum_events: capacities.event_count,
        dictionary_entries: capacities.dictionary_entries,
        resource_count: capacities.resource_count,
    })
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
pub(super) fn derive_ios_static_for_target(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    target: &str,
) -> Result<NativeCallableV3Descriptor, Diagnostic> {
    derive_for_target(
        program,
        function_id,
        target,
        LINKAGE_IOS_STATIC_REGISTRATION,
    )
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
pub(super) fn derive_dynamic_for_target(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    target: &str,
) -> Result<NativeCallableV3Descriptor, Diagnostic> {
    derive_for_target(program, function_id, target, LINKAGE_DYNAMIC_IMAGE)
}

fn canonical_signature(
    projected: &[NativeAdapterParameterProjection],
    projected_result: &NativeAdapterResultProjection,
) -> Result<(Vec<CanonicalParameter>, CanonicalResult), Diagnostic> {
    let mut parameters = Vec::with_capacity(projected.len());
    let mut next_owner = 0_usize;
    for (expected_index, parameter) in projected.iter().enumerate() {
        let canonical = match parameter {
            NativeAdapterParameterProjection::Scalar {
                parameter_index,
                value_id,
                kind,
            } => {
                require_index(*parameter_index, expected_index, "parameter")?;
                CanonicalParameter {
                    index: wire_u32(*parameter_index, "parameter index")?,
                    value: checked_text(value_id.as_str(), "parameter value identity")?,
                    kind: CanonicalParameterKind::Scalar(match kind {
                        NativeHostScalarKind::I64 => SCALAR_I64,
                        NativeHostScalarKind::Bool => SCALAR_BOOL,
                    }),
                }
            }
            NativeAdapterParameterProjection::OwnedResource {
                parameter_index,
                value_id,
                owner_ordinal,
                resource_type,
                lifecycle,
            } => {
                require_index(*parameter_index, expected_index, "parameter")?;
                require_index(*owner_ordinal, next_owner, "owner ordinal")?;
                next_owner += 1;
                CanonicalParameter {
                    index: wire_u32(*parameter_index, "parameter index")?,
                    value: checked_text(value_id.as_str(), "parameter value identity")?,
                    kind: CanonicalParameterKind::Owned {
                        ordinal: wire_u32(*owner_ordinal, "owner ordinal")?,
                        resource: checked_text(resource_type, "resource identity")?,
                        lifecycle: checked_text(lifecycle, "lifecycle identity")?,
                        payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
                    },
                }
            }
        };
        parameters.push(canonical);
    }
    let result = match projected_result {
        NativeAdapterResultProjection::ScalarI64 => CanonicalResult::ScalarI64,
        NativeAdapterResultProjection::OwnedInput {
            parameter_index,
            value_id,
            owner_ordinal,
        } => {
            let parameter_index = wire_u32(*parameter_index, "owned-result parameter index")?;
            let value = checked_text(value_id.as_str(), "owned-result value identity")?;
            let owner_ordinal = wire_u32(*owner_ordinal, "owned-result owner ordinal")?;
            match parameters.get(parameter_index as usize) {
                Some(CanonicalParameter {
                    index,
                    value: admitted_value,
                    kind: CanonicalParameterKind::Owned { ordinal, .. },
                }) if *index == parameter_index
                    && admitted_value == &value
                    && *ordinal == owner_ordinal => {}
                _ => {
                    return Err(v3_error(
                        "owned result does not exactly select an admitted owned parameter",
                    ));
                }
            }
            CanonicalResult::Owned {
                parameter_index,
                value,
                owner_ordinal,
            }
        }
    };
    Ok((parameters, result))
}

fn derive_capacities(
    parameters: &[CanonicalParameter],
    event_count: u32,
    dictionary_bytes: usize,
    dictionary_entries: usize,
    certificate: &NativeSettlementCertificate,
) -> Result<Capacities, Diagnostic> {
    let mut request = REQUEST_FIXED_BYTES;
    for parameter in parameters {
        let increment = match parameter.kind {
            CanonicalParameterKind::Scalar(SCALAR_I64) => REQUEST_I64_BYTES,
            CanonicalParameterKind::Scalar(SCALAR_BOOL) => REQUEST_BOOL_BYTES,
            CanonicalParameterKind::Owned { .. } => REQUEST_OWNER_BYTES,
            CanonicalParameterKind::Scalar(_) => {
                return Err(v3_error("unknown scalar wire kind"));
            }
        };
        request = request
            .checked_add(increment)
            .ok_or_else(|| v3_error("request capacity overflow"))?;
    }
    let execute_response = runtime_wire::execute_response_capacity(event_count)
        .map_err(|_| v3_error("execute-response capacity overflow"))?;
    let resource_count = wire_u32(certificate.resource_count(), "settlement resource count")?;
    let checkpoint_count = wire_u32(
        certificate.checkpoints().len(),
        "settlement checkpoint count",
    )?;
    if resource_count == 0 || resource_count > MAX_RESOURCE_COUNT {
        return Err(v3_error(
            "settlement resource count is outside the v3 bound",
        ));
    }
    if checkpoint_count == 0 || checkpoint_count > MAX_CHECKPOINT_COUNT {
        return Err(v3_error(
            "settlement checkpoint count is outside the v3 bound",
        ));
    }
    let graph_work_units = resource_count
        .checked_mul(checkpoint_count)
        .filter(|work| *work <= MAX_GRAPH_WORK_UNITS)
        .ok_or_else(|| v3_error("settlement graph work exceeds the v3 bound"))?;
    let frame = runtime_wire::frame_capacity(resource_count)
        .map_err(|_| v3_error("frame capacity overflow"))?;
    let candidate_receipt = runtime_wire::candidate_receipt_capacity(resource_count)
        .map_err(|_| v3_error("candidate-receipt capacity overflow"))?;
    for (label, capacity) in [
        ("request", request),
        ("execute response", execute_response),
        ("frame", frame),
        ("decision", DECISION_BYTES),
        ("action evidence", ACTION_EVIDENCE_BYTES),
        ("candidate receipt", candidate_receipt),
    ] {
        if capacity == 0 || capacity > MAX_WIRE_BYTES {
            return Err(v3_error(format!(
                "{label} capacity exceeds the {MAX_WIRE_BYTES}-byte v3 wire bound"
            )));
        }
    }
    let per_active = [
        request,
        execute_response,
        frame,
        DECISION_BYTES,
        ACTION_EVIDENCE_BYTES,
        candidate_receipt,
        HOST_RECEIPT_BYTES,
    ]
    .into_iter()
    .try_fold(0_u32, |sum, value| sum.checked_add(value))
    .ok_or_else(|| v3_error("active-frame reserve capacity overflow"))?;
    let retained_frames = ACTIVE_FRAME_LIMIT
        .checked_add(QUARANTINED_FRAME_LIMIT)
        .ok_or_else(|| v3_error("retained-frame count overflow"))?;
    let instance_reserved_bytes = retained_frames
        .checked_mul(per_active)
        .filter(|reserve| *reserve <= MAX_INSTANCE_RESERVED_BYTES)
        .ok_or_else(|| v3_error("instance reserve exceeds the 64-MiB v3 bound"))?;
    let dictionary_bytes = wire_u32(dictionary_bytes, "event dictionary byte length")?;
    let dictionary_entries = wire_u32(dictionary_entries, "event dictionary entry count")?;
    if event_count == 0 || dictionary_bytes == 0 || dictionary_entries == 0 {
        return Err(v3_error("event and dictionary capacities must be nonzero"));
    }
    if event_count > MAX_EVENT_COUNT
        || dictionary_bytes > MAX_DICTIONARY_BYTES
        || dictionary_entries > MAX_DICTIONARY_ENTRIES
    {
        return Err(v3_error(
            "event or dictionary capacity exceeds the callable-v3 bound",
        ));
    }
    Ok(Capacities {
        request,
        execute_response,
        frame,
        decision: DECISION_BYTES,
        action_evidence: ACTION_EVIDENCE_BYTES,
        candidate_receipt,
        event_count,
        dictionary_bytes,
        dictionary_entries,
        resource_count,
        checkpoint_count,
        graph_work_units,
        active_frames: ACTIVE_FRAME_LIMIT,
        quarantined_frames: QUARANTINED_FRAME_LIMIT,
        instance_reserved_bytes,
    })
}

fn encode_signature(
    writer: &mut WireWriter,
    parameters: &[CanonicalParameter],
    result: &CanonicalResult,
) -> Result<(), Diagnostic> {
    writer.u32(wire_u32(parameters.len(), "parameter count")?)?;
    for parameter in parameters {
        match &parameter.kind {
            CanonicalParameterKind::Scalar(kind) => {
                writer.u32(PARAMETER_SCALAR)?;
                writer.u32(parameter.index)?;
                writer.text(&parameter.value, "parameter value identity")?;
                writer.u32(*kind)?;
            }
            CanonicalParameterKind::Owned {
                ordinal,
                resource,
                lifecycle,
                payload_wire_kind,
            } => {
                writer.u32(PARAMETER_OWNED_RESOURCE)?;
                writer.u32(parameter.index)?;
                writer.text(&parameter.value, "parameter value identity")?;
                writer.u32(*ordinal)?;
                writer.text(resource, "resource identity")?;
                writer.text(lifecycle, "lifecycle identity")?;
                writer.u32(*payload_wire_kind)?;
            }
        }
    }
    match result {
        CanonicalResult::ScalarI64 => writer.u32(RESULT_SCALAR_I64)?,
        CanonicalResult::Owned {
            parameter_index,
            value,
            owner_ordinal,
        } => {
            writer.u32(RESULT_OWNED_INPUT)?;
            writer.u32(*parameter_index)?;
            writer.text(value, "owned-result value identity")?;
            writer.u32(*owner_ordinal)?;
        }
    }
    Ok(())
}

fn hash_signature(
    hasher: &mut Sha256,
    parameters: &[CanonicalParameter],
    result: &CanonicalResult,
) {
    hash_u32(
        hasher,
        u32::try_from(parameters.len()).expect("validated parameter count fits u32"),
    );
    for parameter in parameters {
        match &parameter.kind {
            CanonicalParameterKind::Scalar(kind) => {
                hash_u32(hasher, PARAMETER_SCALAR);
                hash_u32(hasher, parameter.index);
                hash_field(hasher, parameter.value.as_bytes());
                hash_u32(hasher, *kind);
            }
            CanonicalParameterKind::Owned {
                ordinal,
                resource,
                lifecycle,
                payload_wire_kind,
            } => {
                hash_u32(hasher, PARAMETER_OWNED_RESOURCE);
                hash_u32(hasher, parameter.index);
                hash_field(hasher, parameter.value.as_bytes());
                hash_u32(hasher, *ordinal);
                hash_field(hasher, resource.as_bytes());
                hash_field(hasher, lifecycle.as_bytes());
                hash_u32(hasher, *payload_wire_kind);
            }
        }
    }
    match result {
        CanonicalResult::ScalarI64 => hash_u32(hasher, RESULT_SCALAR_I64),
        CanonicalResult::Owned {
            parameter_index,
            value,
            owner_ordinal,
        } => {
            hash_u32(hasher, RESULT_OWNED_INPUT);
            hash_u32(hasher, *parameter_index);
            hash_field(hasher, value.as_bytes());
            hash_u32(hasher, *owner_ordinal);
        }
    }
}

fn encode_graph(
    derivation: &native_settlement_derivation::NativeSettlementDerivation,
    execution_cleanup: [u8; FINGERPRINT_BYTES],
    trace_certificate: [u8; FINGERPRINT_BYTES],
    byte_budget: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let certificate = derivation.certificate();
    for (fingerprint, label) in [
        (certificate.recovery_contract(), "recovery contract"),
        (execution_cleanup, "execution cleanup"),
        (trace_certificate, "trace certificate"),
    ] {
        require_initialized(&fingerprint, label)?;
    }
    let mut graph = WireWriter::new(byte_budget);
    graph.u32(GRAPH_VERSION)?;
    graph.text(
        certificate.function().as_str(),
        "settlement function identity",
    )?;
    graph.raw(&certificate.recovery_contract())?;
    graph.raw(&execution_cleanup)?;
    graph.raw(&trace_certificate)?;
    graph.u32(wire_u32(
        certificate.resource_count(),
        "settlement resource count",
    )?)?;
    graph.u32(wire_u32(
        certificate.checkpoints().len(),
        "settlement checkpoint count",
    )?)?;
    for checkpoint in certificate.checkpoints() {
        graph.u32(checkpoint.checkpoint())?;
        graph.u32(wire_u32(
            checkpoint.resources().len(),
            "checkpoint resource-state count",
        )?)?;
        for state in checkpoint.resources() {
            graph.u32(match state {
                SettlementResourceState::Live => STATE_LIVE,
                SettlementResourceState::ProvisionalResult => STATE_PROVISIONAL_RESULT,
                SettlementResourceState::Finalizing => STATE_FINALIZING,
                SettlementResourceState::Dead => STATE_DEAD,
                SettlementResourceState::Published => STATE_PUBLISHED,
            })?;
        }
        match checkpoint.normal_outcome() {
            None => graph.u32(OUTCOME_NONE)?,
            Some(SettlementOutcome::ScalarSuccess) => graph.u32(OUTCOME_SCALAR_SUCCESS)?,
            Some(SettlementOutcome::SemanticFailure) => graph.u32(OUTCOME_SEMANTIC_FAILURE)?,
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal }) => {
                graph.u32(OUTCOME_OWNED_SUCCESS)?;
                graph.u32(owner_ordinal)?;
            }
        }
        push_ordinals(&mut graph, checkpoint.abort_cleanup_order())?;
        push_ordinals(&mut graph, checkpoint.accept_cleanup_order())?;
    }
    push_ordinals(&mut graph, certificate.start_checkpoints())?;
    graph.u32(wire_u32(
        certificate.progress_edges().len(),
        "settlement progress-edge count",
    )?)?;
    for edge in certificate.progress_edges() {
        graph.u32(edge.from())?;
        graph.u32(edge.to())?;
        match edge.action() {
            SettlementProgressAction::Finalize { owner_ordinal } => {
                graph.u32(ACTION_FINALIZE)?;
                graph.u32(owner_ordinal)?;
            }
            SettlementProgressAction::StageOwnedResult { owner_ordinal } => {
                graph.u32(ACTION_STAGE_OWNED_RESULT)?;
                graph.u32(owner_ordinal)?;
            }
            SettlementProgressAction::CertifyOutcome { trace_evidence } => {
                graph.u32(ACTION_CERTIFY_OUTCOME)?;
                graph.raw(&trace_evidence)?;
                let witness = derivation
                    .trace_evidence_witness(&trace_evidence)
                    .ok_or_else(|| v3_error("certify-outcome edge has no trace witness"))?;
                if trace_evidence_fingerprint(
                    trace_certificate,
                    witness.ordinals(),
                    witness.outcome(),
                ) != trace_evidence
                {
                    return Err(v3_error(
                        "certify-outcome trace witness does not reproduce its digest",
                    ));
                }
                graph.u32(wire_u32(
                    witness.ordinals().len(),
                    "trace witness ordinal count",
                )?)?;
                for ordinal in witness.ordinals() {
                    graph.u32(*ordinal)?;
                }
                match witness.outcome() {
                    TracePathOutcome::ScalarSuccess => graph.u32(1)?,
                    TracePathOutcome::OwnedSuccess => graph.u32(2)?,
                    TracePathOutcome::Failure { selected_ordinal } => {
                        graph.u32(3)?;
                        graph.u32(selected_ordinal)?;
                    }
                }
            }
        }
    }
    Ok(graph.bytes)
}

fn trace_evidence_fingerprint(
    trace_certificate: [u8; 32],
    ordinals: &[u32],
    outcome: TracePathOutcome,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TRACE_EVIDENCE_DOMAIN);
    hasher.update(trace_certificate);
    hasher.update((ordinals.len() as u64).to_le_bytes());
    for ordinal in ordinals {
        hasher.update(ordinal.to_le_bytes());
    }
    match outcome {
        TracePathOutcome::ScalarSuccess => hasher.update([1]),
        TracePathOutcome::OwnedSuccess => hasher.update([2]),
        TracePathOutcome::Failure { selected_ordinal } => {
            hasher.update([3]);
            hasher.update(selected_ordinal.to_le_bytes());
        }
    }
    hasher.finalize().into()
}

fn push_ordinals(writer: &mut WireWriter, ordinals: &[u32]) -> Result<(), Diagnostic> {
    writer.u32(wire_u32(ordinals.len(), "settlement ordinal count")?)?;
    for ordinal in ordinals {
        writer.u32(*ordinal)?;
    }
    Ok(())
}

fn physical_module_fingerprint(
    schema: &[u8; 32],
    target: &[u8; 32],
    semantic_module: &[u8; 32],
    module: &[u8],
    linkage_profile: u32,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PHYSICAL_MODULE_DOMAIN);
    for field in [schema.as_slice(), target, semantic_module, module] {
        hash_field(&mut hasher, field);
    }
    hash_u32(&mut hasher, linkage_profile);
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn call_contract_fingerprint(
    target: &str,
    fingerprints: &[[u8; 32]; 18],
    module: &str,
    function: &str,
    linkage_profile: u32,
    capacities: &Capacities,
    parameters: &[CanonicalParameter],
    result: &CanonicalResult,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CALL_CONTRACT_DOMAIN);
    hash_field(&mut hasher, target.as_bytes());
    for fingerprint in fingerprints {
        hash_field(&mut hasher, fingerprint);
    }
    hash_field(&mut hasher, module.as_bytes());
    hash_field(&mut hasher, function.as_bytes());
    for word in [linkage_profile, CALL_ABI_TAG, CALL_OBLIGATIONS]
        .into_iter()
        .chain(capacities.words())
    {
        hash_u32(&mut hasher, word);
    }
    hash_signature(&mut hasher, parameters, result);
    hasher.finalize().into()
}

fn symbol_seed(fingerprints: &[[u8; 32]; 13]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SYMBOL_SEED_DOMAIN);
    for fingerprint in fingerprints {
        hash_field(&mut hasher, fingerprint);
    }
    hasher.finalize().into()
}

fn exact_symbol(seed: &[u8; 32], domain: &[u8], suffix: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hash_field(&mut hasher, seed);
    let digest = hasher.finalize();
    let mut symbol = String::from("spx_");
    for byte in &digest[..24] {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    write!(symbol, "_{suffix}").expect("writing to a string cannot fail");
    symbol
}

fn framed_fingerprint(domain: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_be_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_le_bytes());
}

fn physical_linkage_profile() -> u32 {
    if cfg!(target_os = "ios") {
        LINKAGE_IOS_STATIC_REGISTRATION
    } else {
        LINKAGE_DYNAMIC_IMAGE
    }
}

fn physical_target_tag() -> Result<String, Diagnostic> {
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    let environment = if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "apple"
    } else {
        return Err(v3_error("physical target environment is unknown"));
    };
    let object = if cfg!(windows) {
        "coff"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "macho"
    } else if cfg!(any(target_os = "linux", target_os = "android")) {
        "elf"
    } else {
        return Err(v3_error("physical target object format is unknown"));
    };
    if cfg!(target_os = "ios") {
        let variant = if cfg!(target_abi = "macabi") {
            "catalyst"
        } else if cfg!(target_abi = "sim") {
            "simulator"
        } else {
            "device"
        };
        Ok(format!(
            "{}-ios-{variant}-{environment}-{object}-ptr{}-{endian}-callable-v3",
            std::env::consts::ARCH,
            usize::BITS,
        ))
    } else {
        Ok(format!(
            "{}-{}-{environment}-{object}-ptr{}-{endian}-callable-v3",
            std::env::consts::ARCH,
            std::env::consts::OS,
            usize::BITS,
        ))
    }
}

fn validate_target_profile(target: &str, linkage_profile: u32) -> Result<(), Diagnostic> {
    validate_text(target, "physical target tag")?;
    let ios = target.split('-').any(|component| component == "ios");
    let dynamic_os = target
        .split('-')
        .any(|component| matches!(component, "linux" | "macos" | "windows" | "android"));
    match linkage_profile {
        LINKAGE_DYNAMIC_IMAGE if dynamic_os && !ios => Ok(()),
        LINKAGE_IOS_STATIC_REGISTRATION if ios => Ok(()),
        LINKAGE_DYNAMIC_IMAGE | LINKAGE_IOS_STATIC_REGISTRATION => Err(v3_error(
            "target and callable-v3 linkage profile are incompatible",
        )),
        _ => Err(v3_error("unknown callable-v3 linkage profile")),
    }
}

fn require_index(actual: usize, expected: usize, label: &str) -> Result<(), Diagnostic> {
    if actual != expected {
        return Err(v3_error(format!(
            "noncanonical {label} {actual}; expected {expected}"
        )));
    }
    Ok(())
}

fn decode_fingerprint(value: &str, label: &str) -> Result<[u8; 32], Diagnostic> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(v3_error(format!(
            "{label} is not canonical lowercase SHA-256"
        )));
    }
    let mut decoded = [0_u8; 32];
    for (index, byte) in decoded.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| v3_error(format!("{label} contains invalid hexadecimal")))?;
    }
    require_initialized(&decoded, label)?;
    Ok(decoded)
}

fn require_initialized(fingerprint: &[u8; 32], label: &str) -> Result<(), Diagnostic> {
    if fingerprint.iter().all(|byte| *byte == 0) {
        return Err(v3_error(format!("{label} must be nonzero")));
    }
    Ok(())
}

fn checked_text(value: &str, label: &str) -> Result<String, Diagnostic> {
    validate_text(value, label)?;
    Ok(value.to_owned())
}

fn validate_text(value: &str, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.as_bytes().contains(&0) {
        return Err(v3_error(format!("{label} must be nonempty and NUL-free")));
    }
    wire_u32(value.len(), label)?;
    Ok(())
}

fn wire_u32(value: usize, label: &str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| v3_error(format!("{label} exceeds u32")))
}

struct WireWriter {
    bytes: Vec<u8>,
    byte_budget: usize,
}

impl WireWriter {
    fn new(byte_budget: usize) -> Self {
        Self {
            bytes: Vec::new(),
            byte_budget,
        }
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        let next = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| v3_error("wire byte length overflow"))?;
        if next > self.byte_budget {
            return Err(v3_error(format!(
                "wire bytes exceed the {}-byte budget",
                self.byte_budget
            )));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u32(&mut self, value: u32) -> Result<(), Diagnostic> {
        self.raw(&value.to_le_bytes())
    }

    fn text(&mut self, value: &str, label: &str) -> Result<(), Diagnostic> {
        validate_text(value, label)?;
        self.u32(wire_u32(value.len(), label)?)?;
        self.raw(value.as_bytes())
    }
}

fn v3_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-I106",
        format!("native callable descriptor v3: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    use crate::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(
            String::with_capacity(bytes.len() * 2),
            |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to a string cannot fail");
                output
            },
        )
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn read_text<'a>(bytes: &'a [u8], offset: &mut usize) -> &'a str {
        let length = read_u32(bytes, *offset) as usize;
        *offset += 4;
        let value = std::str::from_utf8(&bytes[*offset..*offset + length]).unwrap();
        *offset += length;
        value
    }

    fn descriptor_capacities(bytes: &[u8]) -> [u32; 15] {
        let mut offset = HEADER_SIZE as usize;
        let _target = read_text(bytes, &mut offset);
        offset += 4 + 19 * FINGERPRINT_BYTES;
        for _ in 0..5 {
            let _text = read_text(bytes, &mut offset);
        }
        offset += 8;
        std::array::from_fn(|index| read_u32(bytes, offset + index * 4))
    }

    #[test]
    fn all_fourteen_corpus_cases_derive_bounded_deterministic_v3_only_bytes() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        assert_eq!(corpus.cases.len(), 14);
        let mut exact_by_function = BTreeSet::new();
        for case in &corpus.cases {
            let function = DeclarationId::new(case.function_id);
            let settlement =
                native_settlement_derivation::derive_native_settlement(&corpus.program, &function)
                    .unwrap();
            let mut witness_count = 0;
            for edge in settlement.certificate().progress_edges() {
                if let SettlementProgressAction::CertifyOutcome { trace_evidence } = edge.action() {
                    let witness = settlement.trace_evidence_witness(&trace_evidence).unwrap();
                    assert_eq!(
                        trace_evidence_fingerprint(
                            settlement.trace_certificate_fingerprint(),
                            witness.ordinals(),
                            witness.outcome(),
                        ),
                        trace_evidence
                    );
                    witness_count += 1;
                }
            }
            assert!(witness_count > 0, "{}", case.scenario_id);
            let first = derive_for_target(
                &corpus.program,
                &function,
                "x86_64-linux-gnu-elf-ptr64-little-callable-v3",
                LINKAGE_DYNAMIC_IMAGE,
            )
            .unwrap();
            let second = derive_for_target(
                &corpus.program,
                &function,
                "x86_64-linux-gnu-elf-ptr64-little-callable-v3",
                LINKAGE_DYNAMIC_IMAGE,
            )
            .unwrap();
            assert_eq!(first, second, "{}", case.scenario_id);
            assert!(first.bytes.len() <= MAX_DESCRIPTOR_BYTES);
            assert_eq!(&first.bytes[..8], MAGIC);
            assert_eq!(read_u32(&first.bytes, 8), VERSION);
            assert_eq!(read_u32(&first.bytes, 12), HEADER_SIZE);
            assert_eq!(read_u32(&first.bytes, 16) as usize, first.bytes.len());
            assert!(!first.bytes.windows(8).any(|window| window == b"SPXNABI2"));
            assert!(!first.bytes.windows(8).any(|window| window == b"SPXNPRF1"));
            exact_by_function.insert((case.function_id, first.bytes));
        }
        assert_eq!(exact_by_function.len(), 7);
    }

    #[test]
    fn descriptor_has_a_stable_known_answer_and_pairwise_distinct_symbols() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let descriptor = derive_for_target(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
            "x86_64-linux-gnu-elf-ptr64-little-callable-v3",
            LINKAGE_DYNAMIC_IMAGE,
        )
        .unwrap();
        assert_ne!(descriptor.getter_symbol, descriptor.execute_symbol);
        assert_ne!(descriptor.getter_symbol, descriptor.settle_symbol);
        assert_ne!(descriptor.execute_symbol, descriptor.settle_symbol);
        assert_eq!(descriptor.bytes.len(), 1_722);
        assert_eq!(
            hex(&Sha256::digest(&descriptor.bytes)),
            "74b1e96c2d78ccd7d1ea08eec988674ab22bfa6d91b2de19bb41dee42251b44e"
        );
        assert_eq!(
            hex(&descriptor.call_contract),
            "c3ebe4ac69ba061c551305e260ebfa2f4af62be7d9a619227edd7625e8210b59"
        );
        let capacities = descriptor_capacities(&descriptor.bytes);
        let retained_per_frame = capacities[..6]
            .iter()
            .copied()
            .chain(std::iter::once(HOST_RECEIPT_BYTES))
            .sum::<u32>();
        assert_eq!(
            capacities[14],
            (ACTIVE_FRAME_LIMIT + QUARANTINED_FRAME_LIMIT) * retained_per_frame
        );
        assert!(capacities[14] <= MAX_INSTANCE_RESERVED_BYTES);
    }

    #[test]
    fn graph_is_v3_and_has_no_contract_or_symbol_cycle() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let descriptor = derive_for_target(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
            "x86_64-linux-gnu-elf-ptr64-little-callable-v3",
            LINKAGE_DYNAMIC_IMAGE,
        )
        .unwrap();
        let mut offset = HEADER_SIZE as usize;
        assert_eq!(
            read_text(&descriptor.bytes, &mut offset),
            "x86_64-linux-gnu-elf-ptr64-little-callable-v3"
        );
        offset += 4 + 19 * FINGERPRINT_BYTES;
        let _module = read_text(&descriptor.bytes, &mut offset);
        let _function = read_text(&descriptor.bytes, &mut offset);
        assert_eq!(
            read_text(&descriptor.bytes, &mut offset),
            descriptor.getter_symbol
        );
        assert_eq!(
            read_text(&descriptor.bytes, &mut offset),
            descriptor.execute_symbol
        );
        assert_eq!(
            read_text(&descriptor.bytes, &mut offset),
            descriptor.settle_symbol
        );
        offset += 4 + 4 + 15 * 4;
        let parameter_count = read_u32(&descriptor.bytes, offset) as usize;
        offset += 4;
        for _ in 0..parameter_count {
            let tag = read_u32(&descriptor.bytes, offset);
            offset += 8;
            let _value = read_text(&descriptor.bytes, &mut offset);
            match tag {
                PARAMETER_SCALAR => offset += 4,
                PARAMETER_OWNED_RESOURCE => {
                    offset += 4;
                    let _resource = read_text(&descriptor.bytes, &mut offset);
                    let _lifecycle = read_text(&descriptor.bytes, &mut offset);
                    offset += 4;
                }
                _ => panic!("unexpected parameter tag"),
            }
        }
        match read_u32(&descriptor.bytes, offset) {
            RESULT_SCALAR_I64 => offset += 4,
            RESULT_OWNED_INPUT => {
                offset += 8;
                let _value = read_text(&descriptor.bytes, &mut offset);
                offset += 4;
            }
            _ => panic!("unexpected result tag"),
        }
        let graph_len = read_u32(&descriptor.bytes, offset) as usize;
        offset += 4;
        assert_eq!(offset + graph_len, descriptor.bytes.len());
        let graph = &descriptor.bytes[offset..];
        assert_eq!(read_u32(graph, 0), GRAPH_VERSION);
        assert_eq!(
            hex(&Sha256::digest(graph)),
            "0da4af442f926506e2dcfc71fd0a6895dd3f48223922f06bae4f2ac9cf67a380"
        );
        assert!(!graph
            .windows(descriptor.call_contract.len())
            .any(|window| window == descriptor.call_contract));
        for symbol in [
            &descriptor.getter_symbol,
            &descriptor.execute_symbol,
            &descriptor.settle_symbol,
        ] {
            assert!(!graph
                .windows(symbol.len())
                .any(|window| window == symbol.as_bytes()));
        }
    }

    #[test]
    fn target_linkage_profiles_are_closed_and_ios_variants_remain_distinct() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let function = DeclarationId::new("token.discard-two");
        for target in [
            "aarch64-linux-gnu-elf-ptr64-little-callable-v3",
            "aarch64-macos-apple-macho-ptr64-little-callable-v3",
            "x86_64-windows-msvc-coff-ptr64-little-callable-v3",
            "aarch64-android-android-elf-ptr64-little-callable-v3",
        ] {
            derive_for_target(&corpus.program, &function, target, LINKAGE_DYNAMIC_IMAGE).unwrap();
        }
        let device = derive_for_target(
            &corpus.program,
            &function,
            "aarch64-ios-device-apple-macho-ptr64-little-callable-v3",
            LINKAGE_IOS_STATIC_REGISTRATION,
        )
        .unwrap();
        let simulator = derive_for_target(
            &corpus.program,
            &function,
            "aarch64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            LINKAGE_IOS_STATIC_REGISTRATION,
        )
        .unwrap();
        let catalyst = derive_for_target(
            &corpus.program,
            &function,
            "aarch64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
            LINKAGE_IOS_STATIC_REGISTRATION,
        )
        .unwrap();
        assert_ne!(device.bytes, simulator.bytes);
        assert_ne!(device.bytes, catalyst.bytes);
        assert_ne!(simulator.bytes, catalyst.bytes);
        assert!(derive_for_target(
            &corpus.program,
            &function,
            "aarch64-ios-device-apple-macho-ptr64-little-callable-v3",
            LINKAGE_DYNAMIC_IMAGE,
        )
        .is_err());
        assert!(derive_for_target(
            &corpus.program,
            &function,
            "aarch64-linux-gnu-elf-ptr64-little-callable-v3",
            LINKAGE_IOS_STATIC_REGISTRATION,
        )
        .is_err());
        assert!(derive_for_target(&corpus.program, &function, "aarch64-plan9", 99).is_err());
    }

    #[test]
    fn physical_target_source_pins_ios_device_simulator_and_catalyst_cfgs() {
        let source = include_str!("native_callable_abi_v3.rs");
        let macabi = source.find("cfg!(target_abi = \"macabi\")").unwrap();
        let simulator = source.find("cfg!(target_abi = \"sim\")").unwrap();
        assert!(macabi < simulator);
        assert!(source.contains("\"catalyst\""));
        assert!(source.contains("\"simulator\""));
        assert!(source.contains("\"device\""));

        let current = physical_target_tag().unwrap();
        if cfg!(target_os = "ios") {
            let expected = if cfg!(target_abi = "macabi") {
                "-ios-catalyst-"
            } else if cfg!(target_abi = "sim") {
                "-ios-simulator-"
            } else {
                "-ios-device-"
            };
            assert!(current.contains(expected));
        } else {
            assert!(!current.contains("-ios-"));
        }
    }

    #[test]
    fn schema_statements_pin_normative_eight_byte_runtime_magics() {
        for (statement, magic) in [
            (REQUEST_SCHEMA_STATEMENT, b"SPXNRQ03".as_slice()),
            (EXECUTE_RESPONSE_SCHEMA_STATEMENT, b"SPXNEX03".as_slice()),
            (FRAME_SCHEMA_STATEMENT, b"SPXNFR03".as_slice()),
            (DECISION_SCHEMA_STATEMENT, b"SPXNDC03".as_slice()),
            (ACTION_SCHEMA_STATEMENT, b"SPXNAC03".as_slice()),
            (CANDIDATE_RECEIPT_SCHEMA_STATEMENT, b"SPXNCR03".as_slice()),
            (COMMITTED_RECEIPT_SCHEMA_STATEMENT, b"SPXHRP03".as_slice()),
        ] {
            assert!(statement.starts_with(magic));
            assert_eq!(magic.len(), 8);
        }
        for obsolete in [
            b"SPXNREQ3".as_slice(),
            b"SPXNXRS3".as_slice(),
            b"SPXNFRM3".as_slice(),
            b"SPXNDEC3".as_slice(),
            b"SPXNACT3".as_slice(),
            b"SPXNCAN3".as_slice(),
            b"SPXNCOM3".as_slice(),
        ] {
            assert!(![
                REQUEST_SCHEMA_STATEMENT,
                EXECUTE_RESPONSE_SCHEMA_STATEMENT,
                FRAME_SCHEMA_STATEMENT,
                DECISION_SCHEMA_STATEMENT,
                ACTION_SCHEMA_STATEMENT,
                CANDIDATE_RECEIPT_SCHEMA_STATEMENT,
                COMMITTED_RECEIPT_SCHEMA_STATEMENT,
            ]
            .iter()
            .any(|statement| statement.starts_with(obsolete)));
        }
    }
}
