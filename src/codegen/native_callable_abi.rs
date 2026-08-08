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
    dictionary_bytes: u32,
    dictionary_entries: u32,
    max_event_count: u32,
}

impl NativeCallableSemantics {
    pub(super) fn new(
        execution_cleanup_fingerprint: [u8; FINGERPRINT_BYTES],
        event_dictionary_fingerprint: [u8; FINGERPRINT_BYTES],
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
        b"target;11-fingerprints;module;function;getter;callable;abi;obligations;request-cap;response-cap;max-events;dictionary-bytes;dictionary-entries;ordered-parameters;result",
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
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use crate::hir::{self, DeclarationId, ExpressionId, ResolvedFunction, ResolvedProgram};
    use crate::parse;

    use super::super::{native_cleanup, native_host_contract, native_resource, native_value};
    use super::*;

    const SOURCE: &str = r#"module test.native_callable;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("other.type")
resource Other { @id("other.drop") drop trivial; }

@id("token.mixed")
fn mixed(first: own Token, count: i64, enabled: bool, second: own Other) -> i64 { 0 }

@id("token.identity")
fn identity(count: i64, value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ParsedDescriptor {
        target: String,
        fingerprints: [[u8; 32]; 11],
        module: String,
        function: String,
        getter: String,
        callable: String,
        abi_tag: u32,
        obligations: u32,
        max_request: u32,
        max_response: u32,
        max_events: u32,
        dictionary_bytes: u32,
        dictionary_entries: u32,
        parameters: Vec<CanonicalParameter>,
        result: CanonicalResult,
        field_offsets: Vec<usize>,
    }

    struct Reader<'a> {
        bytes: &'a [u8],
        offset: usize,
        field_offsets: Vec<usize>,
    }

    impl<'a> Reader<'a> {
        fn field(&mut self) {
            self.field_offsets.push(self.offset);
        }

        fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
            let end = self
                .offset
                .checked_add(length)
                .ok_or_else(|| "descriptor offset overflow".to_owned())?;
            let value = self
                .bytes
                .get(self.offset..end)
                .ok_or_else(|| "truncated descriptor".to_owned())?;
            self.offset = end;
            Ok(value)
        }

        fn u32(&mut self) -> Result<u32, String> {
            self.field();
            let bytes: [u8; 4] = self
                .take(4)?
                .try_into()
                .map_err(|_| "invalid u32 width".to_owned())?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn fingerprint(&mut self) -> Result<[u8; 32], String> {
            self.field();
            self.take(32)?
                .try_into()
                .map_err(|_| "invalid fingerprint width".to_owned())
        }

        fn text(&mut self) -> Result<String, String> {
            let length = usize::try_from(self.u32()?).map_err(|_| "length overflow".to_owned())?;
            if length == 0 {
                return Err("empty text".to_owned());
            }
            self.field();
            let offset = self.offset;
            let bytes = self.take(length)?;
            if bytes.contains(&0) {
                return Err("text contains NUL".to_owned());
            }
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| format!("text at {offset} is not UTF-8"))
        }
    }

    fn parse_descriptor(bytes: &[u8]) -> Result<ParsedDescriptor, String> {
        let mut reader = Reader {
            bytes,
            offset: 0,
            field_offsets: Vec::new(),
        };
        reader.field();
        if reader.take(8)? != MAGIC {
            return Err("wrong magic".to_owned());
        }
        if reader.u32()? != VERSION {
            return Err("wrong version".to_owned());
        }
        if reader.u32()? != HEADER_SIZE {
            return Err("wrong header size".to_owned());
        }
        if usize::try_from(reader.u32()?).map_err(|_| "length overflow".to_owned())? != bytes.len()
        {
            return Err("inexact total length".to_owned());
        }
        let target = reader.text()?;
        let mut fingerprints = [[0_u8; 32]; 11];
        for fingerprint in &mut fingerprints {
            *fingerprint = reader.fingerprint()?;
        }
        let module = reader.text()?;
        let function = reader.text()?;
        let getter = reader.text()?;
        let callable = reader.text()?;
        let abi_tag = reader.u32()?;
        let obligations = reader.u32()?;
        let max_request = reader.u32()?;
        let max_response = reader.u32()?;
        let max_events = reader.u32()?;
        let dictionary_bytes = reader.u32()?;
        let dictionary_entries = reader.u32()?;
        let parameter_count =
            usize::try_from(reader.u32()?).map_err(|_| "parameter count overflow".to_owned())?;
        let mut parameters = Vec::with_capacity(parameter_count.min(1024));
        let mut next_owner = 0_u32;
        for expected in 0..parameter_count {
            let tag = reader.u32()?;
            let index = reader.u32()?;
            if index != u32::try_from(expected).map_err(|_| "too many parameters".to_owned())? {
                return Err("noncanonical parameter index".to_owned());
            }
            let value = reader.text()?;
            let kind = match tag {
                PARAMETER_SCALAR => {
                    let scalar = reader.u32()?;
                    if !matches!(scalar, SCALAR_I64 | SCALAR_BOOL) {
                        return Err("unknown scalar kind".to_owned());
                    }
                    CanonicalParameterKind::Scalar(scalar)
                }
                PARAMETER_OWNED_RESOURCE => {
                    let ordinal = reader.u32()?;
                    if ordinal != next_owner {
                        return Err("noncanonical owner ordinal".to_owned());
                    }
                    next_owner = next_owner.checked_add(1).ok_or("owner ordinal overflow")?;
                    let resource = reader.text()?;
                    let lifecycle = reader.text()?;
                    let payload_wire_kind = reader.u32()?;
                    if payload_wire_kind != OWNED_PAYLOAD_WIRE_KIND {
                        return Err("unknown owned payload wire kind".to_owned());
                    }
                    CanonicalParameterKind::Owned {
                        ordinal,
                        resource,
                        lifecycle,
                        payload_wire_kind,
                    }
                }
                _ => return Err("unknown parameter tag".to_owned()),
            };
            parameters.push(CanonicalParameter { index, value, kind });
        }
        let result = match reader.u32()? {
            RESULT_SCALAR_I64 => CanonicalResult::ScalarI64,
            RESULT_OWNED_INPUT => {
                let parameter_index = reader.u32()?;
                let value = reader.text()?;
                let owner_ordinal = reader.u32()?;
                match parameters.get(parameter_index as usize) {
                    Some(CanonicalParameter {
                        index,
                        value: admitted_value,
                        kind: CanonicalParameterKind::Owned { ordinal, .. },
                    }) if *index == parameter_index
                        && admitted_value == &value
                        && *ordinal == owner_ordinal => {}
                    _ => return Err("owned result is not an exact owned parameter".to_owned()),
                }
                CanonicalResult::Owned {
                    parameter_index,
                    value,
                    owner_ordinal,
                }
            }
            _ => return Err("unknown result tag".to_owned()),
        };
        if reader.offset != bytes.len() {
            return Err("trailing bytes".to_owned());
        }
        if abi_tag != CALL_ABI_TAG || obligations != CALL_OBLIGATIONS {
            return Err("unknown call ABI or obligations".to_owned());
        }
        if dictionary_bytes == 0 || dictionary_entries == 0 || max_events == 0 {
            return Err("invalid dictionary bounds".to_owned());
        }
        if fingerprints[0] != schema_fingerprint()
            || fingerprints[1] != target_fingerprint(target.as_bytes())
            || fingerprints[3]
                != physical_module_fingerprint(
                    &fingerprints[0],
                    &fingerprints[1],
                    &fingerprints[2],
                    module.as_bytes(),
                )
            || fingerprints[7] != request_schema_fingerprint()
            || fingerprints[8] != response_schema_fingerprint()
            || fingerprints[9] != call_abi_fingerprint()
        {
            return Err("derived fingerprint mismatch".to_owned());
        }
        let expected_request = request_capacity(&parameters).map_err(|error| error.to_string())?;
        let expected_response =
            response_capacity(&result, max_events).map_err(|error| error.to_string())?;
        if max_request != expected_request || max_response != expected_response {
            return Err("noncanonical call-wire capacity".to_owned());
        }
        let semantics = NativeCallableSemantics {
            execution_cleanup_fingerprint: fingerprints[5],
            event_dictionary_fingerprint: fingerprints[6],
            dictionary_bytes,
            dictionary_entries,
            max_event_count: max_events,
        };
        let expected_contract = call_contract_fingerprint(
            &target,
            &fingerprints[0],
            &fingerprints[1],
            &fingerprints[2],
            &fingerprints[3],
            &fingerprints[4],
            &fingerprints[5],
            &fingerprints[6],
            &fingerprints[7],
            &fingerprints[8],
            &fingerprints[9],
            &module,
            &function,
            max_request,
            max_response,
            &semantics,
            &parameters,
            &result,
        );
        if fingerprints[10] != expected_contract {
            return Err("call contract mismatch".to_owned());
        }
        let seed = symbol_seed(
            &fingerprints[3],
            &fingerprints[4],
            &fingerprints[5],
            &fingerprints[6],
            &fingerprints[7],
            &fingerprints[8],
            &fingerprints[9],
            &fingerprints[10],
        );
        if getter != exact_symbol(&seed, GETTER_SYMBOL_DOMAIN, "descriptor_v2")
            || callable != exact_symbol(&seed, CALLABLE_SYMBOL_DOMAIN, "call_v2")
            || getter == callable
        {
            return Err("exact symbol mismatch".to_owned());
        }
        Ok(ParsedDescriptor {
            target,
            fingerprints,
            module,
            function,
            getter,
            callable,
            abi_tag,
            obligations,
            max_request,
            max_response,
            max_events,
            dictionary_bytes,
            dictionary_entries,
            parameters,
            result,
            field_offsets: reader.field_offsets,
        })
    }

    fn program() -> ResolvedProgram {
        let parsed = parse(SOURCE, Path::new("native-callable-v2.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == id)
            .unwrap()
    }

    fn template(program: &ResolvedProgram, id: &str) -> NativeHostContractTemplate {
        let function = function(program, id);
        let abi = native_resource::build_resource_abi(program).unwrap();
        let cleanup = native_cleanup::classify(program, function).unwrap();
        let values = native_value::plan(
            program,
            function,
            &cleanup,
            &abi,
            &HashMap::<ExpressionId, String>::new(),
        )
        .unwrap();
        native_host_contract::derive_from_admitted(
            program,
            &DeclarationId::new(id),
            &abi,
            &cleanup,
            &values,
        )
        .unwrap()
    }

    fn semantics() -> NativeCallableSemantics {
        NativeCallableSemantics::new([0x31; 32], [0x57; 32], 409, 7, 19).unwrap()
    }

    fn descriptor(id: &str) -> NativeCallableDescriptor {
        let program = program();
        derive_for_target(
            &template(&program, id),
            &semantics(),
            "x86_64-linux-gnu-elf-ptr64-little-callable-c",
        )
        .unwrap()
    }

    #[test]
    fn descriptor_round_trips_every_canonical_field_in_order() {
        let descriptor = descriptor("token.mixed");
        let parsed = parse_descriptor(&descriptor.bytes).unwrap();
        assert_eq!(
            parsed.target,
            "x86_64-linux-gnu-elf-ptr64-little-callable-c"
        );
        assert_eq!(parsed.module, "test.native_callable");
        assert_eq!(parsed.function, "token.mixed");
        assert_eq!(parsed.getter, descriptor.getter_symbol);
        assert_eq!(parsed.callable, descriptor.callable_symbol);
        assert_ne!(parsed.getter, parsed.callable);
        assert_eq!(parsed.abi_tag, 1);
        assert_eq!(parsed.obligations, 0x0f);
        assert_eq!(
            parsed.max_request,
            REQUEST_FIXED_BYTES + 2 * REQUEST_OWNER_BYTES + REQUEST_I64_BYTES + REQUEST_BOOL_BYTES
        );
        assert_eq!(
            parsed.max_response,
            RESPONSE_FIXED_BYTES + RESPONSE_SCALAR_PAYLOAD_BYTES + 19 * 4
        );
        assert_eq!(parsed.max_request, descriptor.max_request_bytes);
        assert_eq!(parsed.max_response, descriptor.max_response_bytes);
        assert_eq!(parsed.max_events, 19);
        assert_eq!(parsed.dictionary_bytes, 409);
        assert_eq!(parsed.dictionary_entries, 7);
        assert_eq!(parsed.fingerprints[0], schema_fingerprint());
        assert_eq!(parsed.fingerprints[7], request_schema_fingerprint());
        assert_eq!(parsed.fingerprints[8], response_schema_fingerprint());
        assert_eq!(parsed.fingerprints[9], call_abi_fingerprint());
        assert!(parsed
            .fingerprints
            .iter()
            .all(|value| value.iter().any(|byte| *byte != 0)));
        assert!(matches!(
            &parsed.parameters[..],
            [
                CanonicalParameter {
                    index: 0,
                    kind: CanonicalParameterKind::Owned {
                        ordinal: 0,
                        payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
                        ..
                    },
                    ..
                },
                CanonicalParameter {
                    index: 1,
                    kind: CanonicalParameterKind::Scalar(SCALAR_I64),
                    ..
                },
                CanonicalParameter {
                    index: 2,
                    kind: CanonicalParameterKind::Scalar(SCALAR_BOOL),
                    ..
                },
                CanonicalParameter {
                    index: 3,
                    kind: CanonicalParameterKind::Owned {
                        ordinal: 1,
                        payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
                        ..
                    },
                    ..
                },
            ]
        ));
        assert_eq!(parsed.result, CanonicalResult::ScalarI64);
        assert_eq!(
            u32::from_le_bytes(descriptor.bytes[16..20].try_into().unwrap()) as usize,
            descriptor.bytes.len()
        );
    }

    #[test]
    fn callable_wire_component_sizes_are_normative_known_answers() {
        assert_eq!(OWNED_PAYLOAD_WIRE_KIND, 1);
        assert_eq!(REQUEST_FIXED_BYTES, 64);
        assert_eq!(REQUEST_I64_BYTES, 16);
        assert_eq!(REQUEST_BOOL_BYTES, 12);
        assert_eq!(REQUEST_OWNER_BYTES, 20);
        assert_eq!(RESPONSE_FIXED_BYTES, 68);
        assert_eq!(RESPONSE_FAILURE_PAYLOAD_BYTES, 4);
        assert_eq!(RESPONSE_SCALAR_PAYLOAD_BYTES, 12);
        assert_eq!(RESPONSE_OWNER_PAYLOAD_BYTES, 8);
        assert_eq!(RESPONSE_EVENT_ORDINAL_BYTES, 4);
    }

    #[test]
    fn owned_result_mapping_and_capacity_are_exact() {
        let descriptor = descriptor("token.identity");
        let parsed = parse_descriptor(&descriptor.bytes).unwrap();
        assert!(matches!(
            parsed.result,
            CanonicalResult::Owned {
                parameter_index: 1,
                owner_ordinal: 0,
                ..
            }
        ));
        assert_eq!(
            parsed.max_request,
            REQUEST_FIXED_BYTES + REQUEST_I64_BYTES + REQUEST_OWNER_BYTES
        );
        assert_eq!(
            parsed.max_response,
            RESPONSE_FIXED_BYTES + RESPONSE_OWNER_PAYLOAD_BYTES + 19 * 4
        );
    }

    #[test]
    fn deterministic_known_target_encoding_is_byte_exact() {
        let first = descriptor("token.mixed");
        let second = descriptor("token.mixed");
        assert_eq!(first, second);
        let digest: [u8; 32] = Sha256::digest(&first.bytes).into();
        // This known target fixture makes accidental field reordering visible
        // even when the host running the test has a different physical ABI.
        assert_eq!(
            digest,
            [
                0x4e, 0x04, 0xe5, 0x09, 0x80, 0x5c, 0x5e, 0xe2, 0x31, 0x4e, 0x06, 0xec, 0x79, 0x6b,
                0x61, 0xfc, 0xbf, 0x87, 0x96, 0x08, 0x33, 0xf7, 0x53, 0x49, 0x95, 0xfa, 0xd3, 0x22,
                0xcd, 0xca, 0x3a, 0x47,
            ]
        );
    }

    #[test]
    fn parser_rejects_every_truncated_prefix_and_trailing_data() {
        let descriptor = descriptor("token.mixed");
        for length in 0..descriptor.bytes.len() {
            assert!(
                parse_descriptor(&descriptor.bytes[..length]).is_err(),
                "accepted prefix length {length}"
            );
        }
        let mut trailing = descriptor.bytes.clone();
        trailing.push(0);
        assert!(parse_descriptor(&trailing).is_err());
        let trailing_length = trailing.len() as u32;
        trailing[16..20].copy_from_slice(&trailing_length.to_le_bytes());
        assert!(parse_descriptor(&trailing).is_err());
    }

    #[test]
    fn every_encoded_field_is_authenticated_or_structurally_checked() {
        let descriptor = descriptor("token.mixed");
        let parsed = parse_descriptor(&descriptor.bytes).unwrap();
        for offset in parsed.field_offsets {
            let mut corrupted = descriptor.bytes.clone();
            corrupted[offset] ^= 0x01;
            assert!(
                parse_descriptor(&corrupted).is_err(),
                "accepted mutation at field offset {offset}"
            );
        }
    }

    #[test]
    fn semantic_fingerprints_and_dictionary_bounds_fail_closed() {
        for invalid in [
            NativeCallableSemantics::new([0; 32], [1; 32], 1, 1, 1),
            NativeCallableSemantics::new([1; 32], [0; 32], 1, 1, 1),
            NativeCallableSemantics::new([1; 32], [1; 32], 0, 1, 1),
            NativeCallableSemantics::new([1; 32], [1; 32], 1, 0, 1),
            NativeCallableSemantics::new([1; 32], [1; 32], 1, 1, 0),
        ] {
            assert!(invalid.is_err());
        }
    }

    #[test]
    fn normative_size_and_count_boundaries_are_exact() {
        let boundary = NativeCallableSemantics::new(
            [1; 32],
            [2; 32],
            MAX_DICTIONARY_BYTES as usize,
            MAX_DICTIONARY_ENTRIES as usize,
            MAX_EVENT_COUNT as usize,
        )
        .unwrap();
        let program = program();
        let descriptor =
            derive_for_target(&template(&program, "token.identity"), &boundary, "fixture").unwrap();
        assert_eq!(
            descriptor.max_response_bytes,
            RESPONSE_FIXED_BYTES
                + RESPONSE_OWNER_PAYLOAD_BYTES
                + MAX_EVENT_COUNT * RESPONSE_EVENT_ORDINAL_BYTES
        );
        assert!(descriptor.max_response_bytes <= MAX_CALL_WIRE_BYTES);
        assert_eq!(
            descriptor_length(MAX_DESCRIPTOR_BYTES).unwrap(),
            MAX_DESCRIPTOR_BYTES as u32
        );

        assert!(NativeCallableSemantics::new(
            [1; 32],
            [2; 32],
            MAX_DICTIONARY_BYTES as usize + 1,
            1,
            1,
        )
        .is_err());
        assert!(NativeCallableSemantics::new(
            [1; 32],
            [2; 32],
            1,
            MAX_DICTIONARY_ENTRIES as usize + 1,
            1,
        )
        .is_err());
        assert!(
            NativeCallableSemantics::new([1; 32], [2; 32], 1, 1, MAX_EVENT_COUNT as usize + 1,)
                .is_err()
        );
        assert!(descriptor_length(MAX_DESCRIPTOR_BYTES + 1).is_err());

        assert!(response_capacity(
            &CanonicalResult::Owned {
                parameter_index: 0,
                value: "value".to_owned(),
                owner_ordinal: 0,
            },
            u32::MAX
        )
        .is_err());
    }
}
