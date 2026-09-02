//! Canonical callable native adapter descriptor v2.
//!
//! Descriptor v1 is intentionally descriptor-only. This independent v2 wire
//! binds an exact C callable, its bounded request/response transport, the
//! compiler-emitted cleanup implementation, and the semantic event dictionary.
//! It remains private groundwork behind `SPX-B104`: encoding a descriptor does
//! not make native resource execution public or safe.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "callable native resources remain gated by SPX-B104"
    )
)]

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::native_host_contract::{
    self, NativeAdapterParameterProjection, NativeAdapterResultProjection,
    NativeHostContractTemplate, NativeHostScalarKind,
};

const MAGIC: &[u8; 8] = b"SPXNABI2";
const VERSION: u32 = 2;
const HEADER_SIZE: u32 = 20;
const FINGERPRINT_BYTES: usize = 32;
const CALL_ABI_TAG: u32 = 1;
const CALL_OBLIGATIONS: u32 = 0x0f;
const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;
const MAX_CALL_WIRE_BYTES: u32 = 1024 * 1024;
const MAX_EVENT_COUNT: u32 = 65_536;
const MAX_DICTIONARY_BYTES: u32 = 1024 * 1024;
const MAX_DICTIONARY_ENTRIES: u32 = 65_536;

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED_RESOURCE: u32 = 2;
const SCALAR_I64: u32 = 1;
const SCALAR_BOOL: u32 = 2;
const OWNED_PAYLOAD_WIRE_KIND: u32 = 1;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;
pub(super) const RESPONSE_OUTCOME_SUCCESS: u32 = 1;
pub(super) const RESPONSE_OUTCOME_FAILURE: u32 = 2;
pub(super) const CALL_RESULT_COMPLETE: u32 = 0;
pub(super) const CALL_RESULT_INVALID_REQUEST: u32 = 1;
pub(super) const CALL_RESULT_RESPONSE_CAPACITY: u32 = 2;
pub(super) const CALL_RESULT_INTERNAL_FAILURE: u32 = 3;

// Request v1: 20-byte envelope, contract fingerprint, invocation id, count.
const REQUEST_FIXED_BYTES: u32 = 20 + 32 + 8 + 4;
const REQUEST_I64_BYTES: u32 = 4 + 4 + 8;
const REQUEST_BOOL_BYTES: u32 = 4 + 4 + 4;
// Owned requests carry only a signature index, owner ordinal, and opaque payload.
const REQUEST_OWNER_BYTES: u32 = 4 + 4 + 4 + 8;
// Response v1: envelope, contract fingerprint, invocation id, outcome, event count.
const RESPONSE_FIXED_BYTES: u32 = 20 + 32 + 8 + 4 + 4;
const RESPONSE_FAILURE_PAYLOAD_BYTES: u32 = 4;
const RESPONSE_SCALAR_PAYLOAD_BYTES: u32 = 4 + 8;
const RESPONSE_OWNER_PAYLOAD_BYTES: u32 = 4 + 4;
const RESPONSE_EVENT_ORDINAL_BYTES: u32 = 4;

const SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-descriptor-schema.v2\0";
const TARGET_DOMAIN: &[u8] = b"semaprax.native-callable-target.v2\0";
const PHYSICAL_MODULE_DOMAIN: &[u8] = b"semaprax.native-callable-physical-module.v2\0";
const REQUEST_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-request-schema.v1\0";
const RESPONSE_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-response-schema.v1\0";
const CALL_ABI_DOMAIN: &[u8] = b"semaprax.native-callable-c-abi.v1\0";
const CALL_CONTRACT_DOMAIN: &[u8] = b"semaprax.native-callable-contract.v2\0";
const SYMBOL_SEED_DOMAIN: &[u8] = b"semaprax.native-callable-symbol-seed.v2\0";
const GETTER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-getter.v2\0";
const CALLABLE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-entry.v2\0";

/// Compiler facts that are not recoverable from the admitted signature.
///
/// Capacities, transport fingerprints, the C ABI fingerprint, call contract,
/// and symbols are deliberately absent: the encoder derives them itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableSemantics {
    execution_cleanup_fingerprint: [u8; FINGERPRINT_BYTES],
    event_dictionary_fingerprint: [u8; FINGERPRINT_BYTES],
    trace_path_certificate_fingerprint: [u8; FINGERPRINT_BYTES],
    dictionary_bytes: u32,
    dictionary_entries: u32,
    max_event_count: u32,
}

impl NativeCallableSemantics {
    pub(super) fn new(
        execution_cleanup_fingerprint: [u8; FINGERPRINT_BYTES],
        event_dictionary_fingerprint: [u8; FINGERPRINT_BYTES],
        trace_path_certificate_fingerprint: [u8; FINGERPRINT_BYTES],
        dictionary_bytes: usize,
        dictionary_entries: usize,
        max_event_count: usize,
    ) -> Result<Self, Diagnostic> {
        require_initialized(
            &execution_cleanup_fingerprint,
            "execution/cleanup fingerprint",
        )?;
        require_initialized(
            &event_dictionary_fingerprint,
            "event-dictionary fingerprint",
        )?;
        require_initialized(
            &trace_path_certificate_fingerprint,
            "trace-path certificate fingerprint",
        )?;
        let dictionary_bytes = wire_u32(dictionary_bytes, "event-dictionary byte length")?;
        let dictionary_entries = wire_u32(dictionary_entries, "event-dictionary entry count")?;
        let max_event_count = wire_u32(max_event_count, "maximum event count")?;
        if dictionary_bytes == 0 || dictionary_entries == 0 || max_event_count == 0 {
            return Err(callable_error(
                "event dictionary bytes, entries, and maximum trace events must be nonzero",
            ));
        }
        if dictionary_bytes > MAX_DICTIONARY_BYTES {
            return Err(callable_error(format!(
                "event dictionary exceeds the {MAX_DICTIONARY_BYTES}-byte limit"
            )));
        }
        if dictionary_entries > MAX_DICTIONARY_ENTRIES {
            return Err(callable_error(format!(
                "event dictionary exceeds the {MAX_DICTIONARY_ENTRIES}-entry limit"
            )));
        }
        if max_event_count > MAX_EVENT_COUNT {
            return Err(callable_error(format!(
                "maximum event count exceeds the {MAX_EVENT_COUNT}-event limit"
            )));
        }
        Ok(Self {
            execution_cleanup_fingerprint,
            event_dictionary_fingerprint,
            trace_path_certificate_fingerprint,
            dictionary_bytes,
            dictionary_entries,
            max_event_count,
        })
    }
}

/// Exact descriptor bytes and the only two symbols admitted from its image.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableDescriptor {
    pub(super) bytes: Vec<u8>,
    pub(super) getter_symbol: String,
    pub(super) callable_symbol: String,
    pub(super) call_contract: [u8; FINGERPRINT_BYTES],
    pub(super) max_request_bytes: u32,
    pub(super) max_response_bytes: u32,
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

/// Derive v2 only from a sealed, admitted host template and compiler-produced
/// execution/dictionary facts.
#[allow(
    dead_code,
    reason = "production callable emission is the next SPX-B104 gate"
)]
pub(super) fn derive(
    template: &NativeHostContractTemplate,
    semantics: &NativeCallableSemantics,
) -> Result<NativeCallableDescriptor, Diagnostic> {
    derive_for_target(template, semantics, &physical_target_tag()?)
}

/// Emit the immutable descriptor blob and its exact descriptor-v2 getter.
/// The fragment is appended to the callable provider translation unit after
/// that unit has defined `SPX_PROVIDER_API` and `SPX_PROVIDER_CALL`.
pub(super) fn emit_getter_source(descriptor: &NativeCallableDescriptor) -> String {
    let bytes_symbol = format!("{}_bytes", descriptor.getter_symbol);
    let mut output = String::new();
    writeln!(output, "static const uint8_t {bytes_symbol}[] = {{").expect("writing cannot fail");
    for chunk in descriptor.bytes.chunks(12) {
        output.push_str("    ");
        for byte in chunk {
            write!(output, "0x{byte:02x}, ").expect("writing cannot fail");
        }
        output.push('\n');
    }
    output.push_str("};\n");
    writeln!(
        output,
        "SPX_PROVIDER_API const uint8_t *SPX_PROVIDER_CALL {}(void) {{ return {bytes_symbol}; }}",
        descriptor.getter_symbol
    )
    .expect("writing cannot fail");
    output
}

fn derive_for_target(
    template: &NativeHostContractTemplate,
    semantics: &NativeCallableSemantics,
    target: &str,
) -> Result<NativeCallableDescriptor, Diagnostic> {
    validate_text(target, "physical target tag")?;
    let projection = native_host_contract::project_for_callable_abi(template);
    let semantic_module = decode_fingerprint(
        &projection.module_abi_fingerprint,
        "semantic module ABI fingerprint",
    )?;
    let function_template = decode_fingerprint(
        &projection.function_template_fingerprint,
        "function-template fingerprint",
    )?;
    let (parameters, result) = canonical_signature(&projection.parameters, &projection.result)?;

    let schema = schema_fingerprint();
    let target_fingerprint = target_fingerprint(target.as_bytes());
    let physical_module = physical_module_fingerprint(
        &schema,
        &target_fingerprint,
        &semantic_module,
        projection.module.as_bytes(),
    );
    let request_schema = request_schema_fingerprint();
    let response_schema = response_schema_fingerprint();
    let call_abi = call_abi_fingerprint();
    let max_request_bytes = request_capacity(&parameters)?;
    let max_response_bytes = response_capacity(&result, semantics.max_event_count)?;
    let call_contract = call_contract_fingerprint(
        target,
        &schema,
        &target_fingerprint,
        &semantic_module,
        &physical_module,
        &function_template,
        &semantics.execution_cleanup_fingerprint,
        &semantics.event_dictionary_fingerprint,
        &semantics.trace_path_certificate_fingerprint,
        &request_schema,
        &response_schema,
        &call_abi,
        &projection.module,
        &projection.function,
        max_request_bytes,
        max_response_bytes,
        semantics,
        &parameters,
        &result,
    );
    let symbol_seed = symbol_seed(
        &physical_module,
        &function_template,
        &semantics.execution_cleanup_fingerprint,
        &semantics.event_dictionary_fingerprint,
        &semantics.trace_path_certificate_fingerprint,
        &request_schema,
        &response_schema,
        &call_abi,
        &call_contract,
    );
    let getter_symbol = exact_symbol(&symbol_seed, GETTER_SYMBOL_DOMAIN, "descriptor_v2");
    let callable_symbol = exact_symbol(&symbol_seed, CALLABLE_SYMBOL_DOMAIN, "call_v2");

    let mut writer = WireWriter::new();
    writer.raw(MAGIC);
    writer.u32(VERSION);
    writer.u32(HEADER_SIZE);
    let total_length_offset = writer.bytes.len();
    writer.u32(0);
    writer.text(target, "physical target tag")?;
    for fingerprint in [
        schema,
        target_fingerprint,
        semantic_module,
        physical_module,
        function_template,
        semantics.execution_cleanup_fingerprint,
        semantics.event_dictionary_fingerprint,
        semantics.trace_path_certificate_fingerprint,
        request_schema,
        response_schema,
        call_abi,
        call_contract,
    ] {
        writer.raw(&fingerprint);
    }
    writer.text(&projection.module, "module identity")?;
    writer.text(&projection.function, "function identity")?;
    writer.text(&getter_symbol, "descriptor getter symbol")?;
    writer.text(&callable_symbol, "callable symbol")?;
    writer.u32(CALL_ABI_TAG);
    writer.u32(CALL_OBLIGATIONS);
    writer.u32(max_request_bytes);
    writer.u32(max_response_bytes);
    writer.u32(semantics.max_event_count);
    writer.u32(semantics.dictionary_bytes);
    writer.u32(semantics.dictionary_entries);
    encode_signature(&mut writer, &parameters, &result)?;

    let total_length = descriptor_length(writer.bytes.len())?;
    writer.bytes[total_length_offset..total_length_offset + 4]
        .copy_from_slice(&total_length.to_le_bytes());
    Ok(NativeCallableDescriptor {
        bytes: writer.bytes,
        getter_symbol,
        callable_symbol,
        call_contract,
        max_request_bytes,
        max_response_bytes,
    })
}

fn descriptor_length(length: usize) -> Result<u32, Diagnostic> {
    if length > MAX_DESCRIPTOR_BYTES {
        return Err(callable_error(format!(
            "callable descriptor exceeds the {MAX_DESCRIPTOR_BYTES}-byte limit"
        )));
    }
    wire_u32(length, "callable descriptor byte length")
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
            let index = wire_u32(*parameter_index, "owned-result parameter index")?;
            let value = checked_text(value_id.as_str(), "owned-result value identity")?;
            let ordinal = wire_u32(*owner_ordinal, "owned-result owner ordinal")?;
            match parameters.get(*parameter_index) {
                Some(CanonicalParameter {
                    index: admitted_index,
                    value: admitted_value,
                    kind:
                        CanonicalParameterKind::Owned {
                            ordinal: admitted_ordinal,
                            ..
                        },
                }) if *admitted_index == index
                    && admitted_value == &value
                    && *admitted_ordinal == ordinal => {}
                _ => {
                    return Err(callable_error(
                        "owned result does not exactly select an admitted owned parameter",
                    ))
                }
            }
            CanonicalResult::Owned {
                parameter_index: index,
                value,
                owner_ordinal: ordinal,
            }
        }
    };
    Ok((parameters, result))
}

fn encode_signature(
    writer: &mut WireWriter,
    parameters: &[CanonicalParameter],
    result: &CanonicalResult,
) -> Result<(), Diagnostic> {
    writer.u32(wire_u32(parameters.len(), "parameter count")?);
    for parameter in parameters {
        match &parameter.kind {
            CanonicalParameterKind::Scalar(kind) => {
                writer.u32(PARAMETER_SCALAR);
                writer.u32(parameter.index);
                writer.text(&parameter.value, "parameter value identity")?;
                writer.u32(*kind);
            }
            CanonicalParameterKind::Owned {
                ordinal,
                resource,
                lifecycle,
                payload_wire_kind,
            } => {
                writer.u32(PARAMETER_OWNED_RESOURCE);
                writer.u32(parameter.index);
                writer.text(&parameter.value, "parameter value identity")?;
                writer.u32(*ordinal);
                writer.text(resource, "resource identity")?;
                writer.text(lifecycle, "lifecycle identity")?;
                writer.u32(*payload_wire_kind);
            }
        }
    }
    match result {
        CanonicalResult::ScalarI64 => writer.u32(RESULT_SCALAR_I64),
        CanonicalResult::Owned {
            parameter_index,
            value,
            owner_ordinal,
        } => {
            writer.u32(RESULT_OWNED_INPUT);
            writer.u32(*parameter_index);
            writer.text(value, "owned-result value identity")?;
            writer.u32(*owner_ordinal);
        }
    }
    Ok(())
}

fn request_capacity(parameters: &[CanonicalParameter]) -> Result<u32, Diagnostic> {
    let mut total = REQUEST_FIXED_BYTES;
    for parameter in parameters {
        let bytes = match parameter.kind {
            CanonicalParameterKind::Scalar(SCALAR_I64) => REQUEST_I64_BYTES,
            CanonicalParameterKind::Scalar(SCALAR_BOOL) => REQUEST_BOOL_BYTES,
            CanonicalParameterKind::Scalar(_) => {
                return Err(callable_error("unknown canonical scalar wire kind"))
            }
            CanonicalParameterKind::Owned { .. } => REQUEST_OWNER_BYTES,
        };
        total = total
            .checked_add(bytes)
            .ok_or_else(|| callable_error("request capacity overflows u32"))?;
    }
    require_wire_capacity(total, "request")
}

fn response_capacity(result: &CanonicalResult, max_event_count: u32) -> Result<u32, Diagnostic> {
    let success = match result {
        CanonicalResult::ScalarI64 => RESPONSE_SCALAR_PAYLOAD_BYTES,
        CanonicalResult::Owned { .. } => RESPONSE_OWNER_PAYLOAD_BYTES,
    };
    let outcome = success.max(RESPONSE_FAILURE_PAYLOAD_BYTES);
    let events = max_event_count
        .checked_mul(RESPONSE_EVENT_ORDINAL_BYTES)
        .ok_or_else(|| callable_error("response event capacity overflows u32"))?;
    let total = RESPONSE_FIXED_BYTES
        .checked_add(outcome)
        .and_then(|bytes| bytes.checked_add(events))
        .ok_or_else(|| callable_error("response capacity overflows u32"))?;
    require_wire_capacity(total, "response")
}

fn require_wire_capacity(bytes: u32, context: &str) -> Result<u32, Diagnostic> {
    if bytes == 0 || bytes > MAX_CALL_WIRE_BYTES {
        return Err(callable_error(format!(
            "derived {context} capacity {bytes} exceeds the {MAX_CALL_WIRE_BYTES}-byte call-wire limit"
        )));
    }
    Ok(bytes)
}

fn schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_DOMAIN);
    hash_field(&mut hasher, MAGIC);
    for word in [
        VERSION,
        HEADER_SIZE,
        CALL_ABI_TAG,
        CALL_OBLIGATIONS,
        PARAMETER_SCALAR,
        PARAMETER_OWNED_RESOURCE,
        SCALAR_I64,
        SCALAR_BOOL,
        OWNED_PAYLOAD_WIRE_KIND,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
    ] {
        hash_u32(&mut hasher, word);
    }
    hash_field(
        &mut hasher,
        b"target;12-fingerprints;module;function;getter;callable;abi;obligations;request-cap;response-cap;max-events;dictionary-bytes;dictionary-entries;ordered-parameters;result",
    );
    hasher.finalize().into()
}

fn request_schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(REQUEST_SCHEMA_DOMAIN);
    for word in [
        REQUEST_FIXED_BYTES,
        REQUEST_I64_BYTES,
        REQUEST_BOOL_BYTES,
        REQUEST_OWNER_BYTES,
        PARAMETER_SCALAR,
        PARAMETER_OWNED_RESOURCE,
        SCALAR_I64,
        SCALAR_BOOL,
    ] {
        hash_u32(&mut hasher, word);
    }
    hash_field(
        &mut hasher,
        b"SPXNREQ1;u32le-envelope;call-contract-fingerprint;invocation-u64le;ordered-indexed-args;opaque-owner-payload-u64le;bool-canonical-0-or-1",
    );
    hasher.finalize().into()
}

fn response_schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_SCHEMA_DOMAIN);
    for word in [
        RESPONSE_FIXED_BYTES,
        RESPONSE_FAILURE_PAYLOAD_BYTES,
        RESPONSE_SCALAR_PAYLOAD_BYTES,
        RESPONSE_OWNER_PAYLOAD_BYTES,
        RESPONSE_EVENT_ORDINAL_BYTES,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
        RESPONSE_OUTCOME_SUCCESS,
        RESPONSE_OUTCOME_FAILURE,
    ] {
        hash_u32(&mut hasher, word);
    }
    hash_field(
        &mut hasher,
        b"SPXNRSP1;u32le-envelope;call-contract-fingerprint;invocation-u64le;success-or-failure;result-or-selected-failure-ordinal;semantic-event-ordinals",
    );
    hasher.finalize().into()
}

fn call_abi_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(CALL_ABI_DOMAIN);
    hash_u32(&mut hasher, CALL_ABI_TAG);
    hash_u32(&mut hasher, CALL_OBLIGATIONS);
    for result in [
        CALL_RESULT_COMPLETE,
        CALL_RESULT_INVALID_REQUEST,
        CALL_RESULT_RESPONSE_CAPACITY,
        CALL_RESULT_INTERNAL_FAILURE,
    ] {
        hash_u32(&mut hasher, result);
    }
    hash_field(
        &mut hasher,
        b"extern-C-u32(const-u8-request,u32-request-len,u8-response,u32-response-cap);windows-cdecl;no-unwind;no-longjmp;no-retained-pointers;no-callbacks;one-shot",
    );
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn call_contract_fingerprint(
    target: &str,
    schema: &[u8; 32],
    target_fingerprint: &[u8; 32],
    semantic_module: &[u8; 32],
    physical_module: &[u8; 32],
    function_template: &[u8; 32],
    execution_cleanup: &[u8; 32],
    event_dictionary: &[u8; 32],
    trace_path_certificate: &[u8; 32],
    request_schema: &[u8; 32],
    response_schema: &[u8; 32],
    call_abi: &[u8; 32],
    module: &str,
    function: &str,
    max_request_bytes: u32,
    max_response_bytes: u32,
    semantics: &NativeCallableSemantics,
    parameters: &[CanonicalParameter],
    result: &CanonicalResult,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CALL_CONTRACT_DOMAIN);
    for bytes in [
        target.as_bytes(),
        schema,
        target_fingerprint,
        semantic_module,
        physical_module,
        function_template,
        execution_cleanup,
        event_dictionary,
        trace_path_certificate,
        request_schema,
        response_schema,
        call_abi,
        module.as_bytes(),
        function.as_bytes(),
    ] {
        hash_field(&mut hasher, bytes);
    }
    for word in [
        CALL_ABI_TAG,
        CALL_OBLIGATIONS,
        max_request_bytes,
        max_response_bytes,
        semantics.max_event_count,
        semantics.dictionary_bytes,
        semantics.dictionary_entries,
    ] {
        hash_u32(&mut hasher, word);
    }
    hash_signature(&mut hasher, parameters, result);
    hasher.finalize().into()
}

fn hash_signature(
    hasher: &mut Sha256,
    parameters: &[CanonicalParameter],
    result: &CanonicalResult,
) {
    hash_u32(
        hasher,
        u32::try_from(parameters.len()).expect("admitted count was checked"),
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

fn target_fingerprint(target: &[u8]) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_DOMAIN);
    hash_field(&mut hasher, target);
    hasher.finalize().into()
}

fn physical_module_fingerprint(
    schema: &[u8; 32],
    target: &[u8; 32],
    semantic_module: &[u8; 32],
    module: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PHYSICAL_MODULE_DOMAIN);
    for bytes in [schema.as_slice(), target, semantic_module, module] {
        hash_field(&mut hasher, bytes);
    }
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn symbol_seed(
    physical_module: &[u8; 32],
    function_template: &[u8; 32],
    execution_cleanup: &[u8; 32],
    event_dictionary: &[u8; 32],
    trace_path_certificate: &[u8; 32],
    request_schema: &[u8; 32],
    response_schema: &[u8; 32],
    call_abi: &[u8; 32],
    call_contract: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SYMBOL_SEED_DOMAIN);
    for bytes in [
        physical_module,
        function_template,
        execution_cleanup,
        event_dictionary,
        trace_path_certificate,
        request_schema,
        response_schema,
        call_abi,
        call_contract,
    ] {
        hash_field(&mut hasher, bytes);
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

#[allow(dead_code, reason = "used when production callable emission opens")]
fn physical_target_tag() -> Result<String, Diagnostic> {
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    let environment = if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "apple"
    } else {
        return Err(callable_error("physical target environment is unknown"));
    };
    let object_format = if cfg!(windows) {
        "coff"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "macho"
    } else if cfg!(unix) {
        "elf"
    } else {
        return Err(callable_error("physical target object format is unknown"));
    };
    let call = if cfg!(windows) {
        "callable-cdecl"
    } else {
        "callable-c"
    };
    Ok(format!(
        "{}-{}-{environment}-{object_format}-ptr{}-{endian}-{call}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        usize::BITS
    ))
}

fn require_index(actual: usize, expected: usize, context: &str) -> Result<(), Diagnostic> {
    if actual != expected {
        return Err(callable_error(format!(
            "noncanonical {context} {actual}; expected {expected}"
        )));
    }
    Ok(())
}

fn require_initialized(fingerprint: &[u8; 32], context: &str) -> Result<(), Diagnostic> {
    if fingerprint.iter().all(|byte| *byte == 0) {
        return Err(callable_error(format!("{context} is uninitialized")));
    }
    Ok(())
}

fn decode_fingerprint(value: &str, context: &str) -> Result<[u8; 32], Diagnostic> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(callable_error(format!(
            "{context} is not canonical lowercase SHA-256"
        )));
    }
    let mut decoded = [0_u8; 32];
    for (index, byte) in decoded.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| callable_error(format!("{context} contains invalid hexadecimal")))?;
    }
    require_initialized(&decoded, context)?;
    Ok(decoded)
}

fn checked_text(value: &str, context: &str) -> Result<String, Diagnostic> {
    validate_text(value, context)?;
    Ok(value.to_owned())
}

fn validate_text(value: &str, context: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.contains('\0') {
        return Err(callable_error(format!(
            "{context} is empty or contains NUL"
        )));
    }
    wire_u32(value.len(), context)?;
    Ok(())
}

fn wire_u32(value: usize, context: &str) -> Result<u32, Diagnostic> {
    u32::try_from(value)
        .map_err(|_| callable_error(format!("{context} exceeds the u32 wire limit")))
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_le_bytes());
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

struct WireWriter {
    bytes: Vec<u8>,
}

impl WireWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u32(&mut self, value: u32) {
        self.raw(&value.to_le_bytes());
    }

    fn text(&mut self, value: &str, context: &str) -> Result<(), Diagnostic> {
        validate_text(value, context)?;
        self.u32(wire_u32(value.len(), context)?);
        self.raw(value.as_bytes());
        Ok(())
    }
}

fn callable_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native callable descriptor v2: {}", message.into()),
    )
}

#[cfg(test)]
#[path = "native_callable_abi/tests.rs"]
mod tests;
