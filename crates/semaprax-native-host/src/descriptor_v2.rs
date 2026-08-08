//! Strict decoder for the canonical callable native descriptor v2.
//!
//! This module describes admission metadata only. It does not load a module,
//! expose a raw symbol, or open the gated public resource backend.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "callable descriptor v2 is staged behind the native resource gate"
    )
)]

use std::collections::HashSet;

use sha2::{Digest, Sha256};

pub(crate) const MAGIC: &[u8; 8] = b"SPXNABI2";
pub(crate) const VERSION: u32 = 2;
pub(crate) const HEADER_SIZE: u32 = 20;
pub(crate) const FINGERPRINT_BYTES: usize = 32;
pub(crate) const CALL_ABI_TAG: u32 = 1;
pub(crate) const REQUIRED_OBLIGATIONS: u32 = 0x0f;
pub(crate) const OWNED_PAYLOAD_WIRE_KIND: u32 = 1;

const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_SYMBOL_BYTES: usize = 1024;
const MAX_CALL_WIRE_BYTES: u32 = 1024 * 1024;
const MAX_EVENT_COUNT: u32 = 65_536;
const MAX_DICTIONARY_BYTES: u32 = 1024 * 1024;
const MAX_DICTIONARY_ENTRIES: u32 = 65_536;
const MIN_SCALAR_PARAMETER_BYTES: usize = 4 + 4 + 4 + 1 + 4;
const MIN_RESULT_BYTES: usize = 4;

const SCHEMA_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-callable-descriptor-schema.v2\0";
const TARGET_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-callable-target.v2\0";
const PHYSICAL_MODULE_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-callable-physical-module.v2\0";
const REQUEST_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-request-schema.v1\0";
const RESPONSE_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-response-schema.v1\0";
const CALL_ABI_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-callable-c-abi.v1\0";
const CALL_CONTRACT_DOMAIN: &[u8] = b"semaprax.native-callable-contract.v2\0";
const SYMBOL_SEED_DOMAIN: &[u8] = b"semaprax.native-callable-symbol-seed.v2\0";
pub(crate) const GETTER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-getter.v2\0";
pub(crate) const CALLABLE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-entry.v2\0";

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED_RESOURCE: u32 = 2;
const SCALAR_I64: u32 = 1;
const SCALAR_BOOL: u32 = 2;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

const REQUEST_FIXED_BYTES: u32 = 20 + 32 + 8 + 4;
const REQUEST_I64_BYTES: u32 = 4 + 4 + 8;
const REQUEST_BOOL_BYTES: u32 = 4 + 4 + 4;
const REQUEST_OWNER_BYTES: u32 = 4 + 4 + 4 + 8;
const RESPONSE_FIXED_BYTES: u32 = 20 + 32 + 8 + 4 + 4;
const RESPONSE_FAILURE_PAYLOAD_BYTES: u32 = 4;
const RESPONSE_SCALAR_PAYLOAD_BYTES: u32 = 4 + 8;
const RESPONSE_OWNER_PAYLOAD_BYTES: u32 = 4 + 4;
const RESPONSE_EVENT_ORDINAL_BYTES: u32 = 4;

/// Every independently domain-separated fingerprint carried by descriptor v2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fingerprints {
    pub(crate) schema: [u8; FINGERPRINT_BYTES],
    pub(crate) target: [u8; FINGERPRINT_BYTES],
    pub(crate) semantic_module: [u8; FINGERPRINT_BYTES],
    pub(crate) physical_module: [u8; FINGERPRINT_BYTES],
    pub(crate) function_template: [u8; FINGERPRINT_BYTES],
    pub(crate) execution_cleanup: [u8; FINGERPRINT_BYTES],
    pub(crate) event_dictionary: [u8; FINGERPRINT_BYTES],
    pub(crate) request_schema: [u8; FINGERPRINT_BYTES],
    pub(crate) response_schema: [u8; FINGERPRINT_BYTES],
    pub(crate) call_abi: [u8; FINGERPRINT_BYTES],
    pub(crate) call_contract: [u8; FINGERPRINT_BYTES],
}

impl Fingerprints {
    fn iter(&self) -> impl Iterator<Item = &[u8; FINGERPRINT_BYTES]> {
        [
            &self.schema,
            &self.target,
            &self.semantic_module,
            &self.physical_module,
            &self.function_template,
            &self.execution_cleanup,
            &self.event_dictionary,
            &self.request_schema,
            &self.response_schema,
            &self.call_abi,
            &self.call_contract,
        ]
        .into_iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarKind {
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Parameter {
    Scalar {
        index: usize,
        value: String,
        kind: ScalarKind,
    },
    Owned {
        index: usize,
        value: String,
        owner_ordinal: usize,
        resource: String,
        lifecycle: String,
        payload_wire_kind: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultShape {
    ScalarI64,
    OwnedInput {
        parameter_index: usize,
        owner_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Capacities {
    pub(crate) max_request_bytes: u32,
    pub(crate) max_response_bytes: u32,
    pub(crate) max_event_count: u32,
    pub(crate) dictionary_bytes: u32,
    pub(crate) dictionary_entries: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) target: String,
    pub(crate) fingerprints: Fingerprints,
    pub(crate) module: String,
    pub(crate) function: String,
    pub(crate) getter_symbol: String,
    pub(crate) callable_symbol: String,
    pub(crate) call_abi_tag: u32,
    pub(crate) obligations: u32,
    pub(crate) capacities: Capacities,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) result: ResultShape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorError {
    Malformed,
    UnsupportedSchema,
    WrongTarget,
    NonCanonical,
}

impl Descriptor {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, DescriptorError> {
        if bytes.len() > MAX_DESCRIPTOR_BYTES {
            return Err(DescriptorError::Malformed);
        }
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(MAGIC.len())? != MAGIC
            || reader.u32()? != VERSION
            || reader.u32()? != HEADER_SIZE
        {
            return Err(DescriptorError::UnsupportedSchema);
        }
        let declared = reader.usize()?;
        if declared != bytes.len() {
            return Err(DescriptorError::Malformed);
        }

        let target = reader.text(MAX_TEXT_BYTES)?;
        if target != current_target_tag() {
            return Err(DescriptorError::WrongTarget);
        }
        let fingerprints = Fingerprints {
            schema: reader.fingerprint()?,
            target: reader.fingerprint()?,
            semantic_module: reader.fingerprint()?,
            physical_module: reader.fingerprint()?,
            function_template: reader.fingerprint()?,
            execution_cleanup: reader.fingerprint()?,
            event_dictionary: reader.fingerprint()?,
            request_schema: reader.fingerprint()?,
            response_schema: reader.fingerprint()?,
            call_abi: reader.fingerprint()?,
            call_contract: reader.fingerprint()?,
        };
        if fingerprints.schema != schema_fingerprint() {
            return Err(DescriptorError::UnsupportedSchema);
        }
        if fingerprints.target != target_fingerprint(target.as_bytes()) {
            return Err(DescriptorError::WrongTarget);
        }
        if fingerprints
            .iter()
            .any(|fingerprint| *fingerprint == [0; 32])
        {
            return Err(DescriptorError::NonCanonical);
        }

        let module = reader.text(MAX_TEXT_BYTES)?;
        let function = reader.text(MAX_TEXT_BYTES)?;
        if fingerprints.physical_module
            != physical_module_fingerprint(
                &fingerprints.schema,
                &fingerprints.target,
                &fingerprints.semantic_module,
                module.as_bytes(),
            )
        {
            return Err(DescriptorError::NonCanonical);
        }

        let getter_symbol = reader.text(MAX_SYMBOL_BYTES)?;
        let callable_symbol = reader.text(MAX_SYMBOL_BYTES)?;
        if !is_c_symbol(&getter_symbol)
            || !is_c_symbol(&callable_symbol)
            || getter_symbol == callable_symbol
        {
            return Err(DescriptorError::NonCanonical);
        }

        let call_abi_tag = reader.u32()?;
        if call_abi_tag != CALL_ABI_TAG {
            return Err(DescriptorError::UnsupportedSchema);
        }
        let obligations = reader.u32()?;
        if obligations != REQUIRED_OBLIGATIONS {
            return Err(DescriptorError::NonCanonical);
        }
        if fingerprints.request_schema != request_schema_fingerprint()
            || fingerprints.response_schema != response_schema_fingerprint()
            || fingerprints.call_abi != call_abi_fingerprint()
        {
            return Err(DescriptorError::UnsupportedSchema);
        }

        let capacities = Capacities {
            max_request_bytes: reader.u32()?,
            max_response_bytes: reader.u32()?,
            max_event_count: reader.u32()?,
            dictionary_bytes: reader.u32()?,
            dictionary_entries: reader.u32()?,
        };
        if capacities.max_request_bytes == 0
            || capacities.max_request_bytes > MAX_CALL_WIRE_BYTES
            || capacities.max_response_bytes == 0
            || capacities.max_response_bytes > MAX_CALL_WIRE_BYTES
            || capacities.max_event_count == 0
            || capacities.max_event_count > MAX_EVENT_COUNT
            || capacities.dictionary_bytes == 0
            || capacities.dictionary_bytes > MAX_DICTIONARY_BYTES
            || capacities.dictionary_entries == 0
            || capacities.dictionary_entries > MAX_DICTIONARY_ENTRIES
        {
            return Err(DescriptorError::NonCanonical);
        }

        let parameter_count = reader.usize()?;
        let remaining = bytes
            .len()
            .checked_sub(reader.offset)
            .ok_or(DescriptorError::Malformed)?;
        let maximum_structural_count =
            remaining.saturating_sub(MIN_RESULT_BYTES) / MIN_SCALAR_PARAMETER_BYTES;
        if parameter_count > maximum_structural_count {
            return Err(DescriptorError::NonCanonical);
        }
        let mut parameters = Vec::with_capacity(parameter_count);
        let mut values = HashSet::with_capacity(parameter_count);
        let mut next_owner = 0_usize;
        for expected_index in 0..parameter_count {
            let tag = reader.u32()?;
            let index = reader.usize()?;
            if index != expected_index {
                return Err(DescriptorError::NonCanonical);
            }
            let value = reader.text(MAX_TEXT_BYTES)?;
            if !values.insert(value.clone()) {
                return Err(DescriptorError::NonCanonical);
            }
            match tag {
                PARAMETER_SCALAR => {
                    let kind = match reader.u32()? {
                        SCALAR_I64 => ScalarKind::I64,
                        SCALAR_BOOL => ScalarKind::Bool,
                        _ => return Err(DescriptorError::NonCanonical),
                    };
                    parameters.push(Parameter::Scalar { index, value, kind });
                }
                PARAMETER_OWNED_RESOURCE => {
                    let owner_ordinal = reader.usize()?;
                    if owner_ordinal != next_owner {
                        return Err(DescriptorError::NonCanonical);
                    }
                    next_owner = next_owner
                        .checked_add(1)
                        .ok_or(DescriptorError::Malformed)?;
                    let resource = reader.text(MAX_TEXT_BYTES)?;
                    let lifecycle = reader.text(MAX_TEXT_BYTES)?;
                    let payload_wire_kind = reader.u32()?;
                    if payload_wire_kind != OWNED_PAYLOAD_WIRE_KIND {
                        return Err(DescriptorError::UnsupportedSchema);
                    }
                    parameters.push(Parameter::Owned {
                        index,
                        value,
                        owner_ordinal,
                        resource,
                        lifecycle,
                        payload_wire_kind,
                    });
                }
                _ => return Err(DescriptorError::NonCanonical),
            }
        }

        let result = match reader.u32()? {
            RESULT_SCALAR_I64 => ResultShape::ScalarI64,
            RESULT_OWNED_INPUT => {
                let parameter_index = reader.usize()?;
                let value = reader.text(MAX_TEXT_BYTES)?;
                let owner_ordinal = reader.usize()?;
                let Some(Parameter::Owned {
                    index,
                    value: expected_value,
                    owner_ordinal: expected_ordinal,
                    ..
                }) = parameters.get(parameter_index)
                else {
                    return Err(DescriptorError::NonCanonical);
                };
                if *index != parameter_index
                    || *expected_value != value
                    || *expected_ordinal != owner_ordinal
                {
                    return Err(DescriptorError::NonCanonical);
                }
                ResultShape::OwnedInput {
                    parameter_index,
                    owner_ordinal,
                }
            }
            _ => return Err(DescriptorError::NonCanonical),
        };
        if reader.offset != bytes.len() {
            return Err(DescriptorError::Malformed);
        }

        if capacities.max_request_bytes != request_capacity(&parameters)?
            || capacities.max_response_bytes
                != response_capacity(&result, capacities.max_event_count)?
        {
            return Err(DescriptorError::NonCanonical);
        }

        let expected_contract = call_contract_fingerprint(
            &target,
            &fingerprints,
            &module,
            &function,
            &capacities,
            &parameters,
            &result,
        );
        if fingerprints.call_contract != expected_contract {
            return Err(DescriptorError::NonCanonical);
        }

        let (expected_getter, expected_callable) = derive_symbols(&fingerprints);
        if getter_symbol != expected_getter || callable_symbol != expected_callable {
            return Err(DescriptorError::NonCanonical);
        }

        Ok(Self {
            target,
            fingerprints,
            module,
            function,
            getter_symbol,
            callable_symbol,
            call_abi_tag,
            obligations,
            capacities,
            parameters,
            result,
        })
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], DescriptorError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DescriptorError::Malformed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DescriptorError::Malformed)?;
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, DescriptorError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| DescriptorError::Malformed)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn usize(&mut self) -> Result<usize, DescriptorError> {
        usize::try_from(self.u32()?).map_err(|_| DescriptorError::Malformed)
    }

    fn text(&mut self, max: usize) -> Result<String, DescriptorError> {
        let length = self.usize()?;
        if length > max {
            return Err(DescriptorError::NonCanonical);
        }
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes).map_err(|_| DescriptorError::Malformed)?;
        if value.is_empty() || value.contains('\0') {
            return Err(DescriptorError::NonCanonical);
        }
        Ok(value.to_owned())
    }

    fn fingerprint(&mut self) -> Result<[u8; FINGERPRINT_BYTES], DescriptorError> {
        self.take(FINGERPRINT_BYTES)?
            .try_into()
            .map_err(|_| DescriptorError::Malformed)
    }
}

pub(crate) fn current_target_tag() -> String {
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
        "unknown"
    };
    let object_format = if cfg!(windows) {
        "coff"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "macho"
    } else {
        "elf"
    };
    let call = if cfg!(windows) {
        "callable-cdecl"
    } else {
        "callable-c"
    };
    format!(
        "{}-{}-{environment}-{object_format}-ptr{}-{endian}-{call}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        usize::BITS
    )
}

pub(crate) fn schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, MAGIC);
    for value in [
        VERSION,
        HEADER_SIZE,
        CALL_ABI_TAG,
        REQUIRED_OBLIGATIONS,
        PARAMETER_SCALAR,
        PARAMETER_OWNED_RESOURCE,
        SCALAR_I64,
        SCALAR_BOOL,
        OWNED_PAYLOAD_WIRE_KIND,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
    ] {
        hash_u32(&mut hasher, value);
    }
    hash_field(
        &mut hasher,
        b"target;11-fingerprints;module;function;getter;callable;abi;obligations;request-cap;response-cap;max-events;dictionary-bytes;dictionary-entries;ordered-parameters;result",
    );
    hasher.finalize().into()
}

pub(crate) fn target_fingerprint(target: &[u8]) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, target);
    hasher.finalize().into()
}

pub(crate) fn physical_module_fingerprint(
    schema: &[u8; FINGERPRINT_BYTES],
    target: &[u8; FINGERPRINT_BYTES],
    semantic_module: &[u8; FINGERPRINT_BYTES],
    module: &[u8],
) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(PHYSICAL_MODULE_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, schema);
    hash_field(&mut hasher, target);
    hash_field(&mut hasher, semantic_module);
    hash_field(&mut hasher, module);
    hasher.finalize().into()
}

pub(crate) fn request_schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(REQUEST_SCHEMA_DOMAIN);
    for value in [
        REQUEST_FIXED_BYTES,
        REQUEST_I64_BYTES,
        REQUEST_BOOL_BYTES,
        REQUEST_OWNER_BYTES,
        PARAMETER_SCALAR,
        PARAMETER_OWNED_RESOURCE,
        SCALAR_I64,
        SCALAR_BOOL,
    ] {
        hash_u32(&mut hasher, value);
    }
    hash_field(
        &mut hasher,
        b"SPXNREQ1;u32le-envelope;call-contract-fingerprint;invocation-u64le;ordered-indexed-args;opaque-owner-payload-u64le;bool-canonical-0-or-1",
    );
    hasher.finalize().into()
}

pub(crate) fn response_schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(RESPONSE_SCHEMA_DOMAIN);
    for value in [
        RESPONSE_FIXED_BYTES,
        RESPONSE_FAILURE_PAYLOAD_BYTES,
        RESPONSE_SCALAR_PAYLOAD_BYTES,
        RESPONSE_OWNER_PAYLOAD_BYTES,
        RESPONSE_EVENT_ORDINAL_BYTES,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
    ] {
        hash_u32(&mut hasher, value);
    }
    hash_field(
        &mut hasher,
        b"SPXNRSP1;u32le-envelope;call-contract-fingerprint;invocation-u64le;success-or-failure;result-or-selected-failure-ordinal;semantic-event-ordinals",
    );
    hasher.finalize().into()
}

pub(crate) fn call_abi_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(CALL_ABI_FINGERPRINT_DOMAIN);
    hash_u32(&mut hasher, CALL_ABI_TAG);
    hash_u32(&mut hasher, REQUIRED_OBLIGATIONS);
    hash_field(
        &mut hasher,
        b"extern-C-u32(const-u8-request,u32-request-len,u8-response,u32-response-cap);windows-cdecl;no-unwind;no-longjmp;no-retained-pointers;no-callbacks;one-shot",
    );
    hasher.finalize().into()
}

fn request_capacity(parameters: &[Parameter]) -> Result<u32, DescriptorError> {
    let mut total = REQUEST_FIXED_BYTES;
    for parameter in parameters {
        let bytes = match parameter {
            Parameter::Scalar {
                kind: ScalarKind::I64,
                ..
            } => REQUEST_I64_BYTES,
            Parameter::Scalar {
                kind: ScalarKind::Bool,
                ..
            } => REQUEST_BOOL_BYTES,
            Parameter::Owned { .. } => REQUEST_OWNER_BYTES,
        };
        total = total
            .checked_add(bytes)
            .ok_or(DescriptorError::NonCanonical)?;
    }
    require_wire_capacity(total)
}

fn response_capacity(result: &ResultShape, max_event_count: u32) -> Result<u32, DescriptorError> {
    let success = match result {
        ResultShape::ScalarI64 => RESPONSE_SCALAR_PAYLOAD_BYTES,
        ResultShape::OwnedInput { .. } => RESPONSE_OWNER_PAYLOAD_BYTES,
    };
    let outcome = success.max(RESPONSE_FAILURE_PAYLOAD_BYTES);
    let events = max_event_count
        .checked_mul(RESPONSE_EVENT_ORDINAL_BYTES)
        .ok_or(DescriptorError::NonCanonical)?;
    let total = RESPONSE_FIXED_BYTES
        .checked_add(outcome)
        .and_then(|bytes| bytes.checked_add(events))
        .ok_or(DescriptorError::NonCanonical)?;
    require_wire_capacity(total)
}

fn require_wire_capacity(bytes: u32) -> Result<u32, DescriptorError> {
    if bytes == 0 || bytes > MAX_CALL_WIRE_BYTES {
        Err(DescriptorError::NonCanonical)
    } else {
        Ok(bytes)
    }
}

#[allow(clippy::too_many_arguments)]
fn call_contract_fingerprint(
    target: &str,
    fingerprints: &Fingerprints,
    module: &str,
    function: &str,
    capacities: &Capacities,
    parameters: &[Parameter],
    result: &ResultShape,
) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(CALL_CONTRACT_DOMAIN);
    for bytes in [
        target.as_bytes(),
        fingerprints.schema.as_slice(),
        &fingerprints.target,
        &fingerprints.semantic_module,
        &fingerprints.physical_module,
        &fingerprints.function_template,
        &fingerprints.execution_cleanup,
        &fingerprints.event_dictionary,
        &fingerprints.request_schema,
        &fingerprints.response_schema,
        &fingerprints.call_abi,
        module.as_bytes(),
        function.as_bytes(),
    ] {
        hash_field(&mut hasher, bytes);
    }
    for value in [
        CALL_ABI_TAG,
        REQUIRED_OBLIGATIONS,
        capacities.max_request_bytes,
        capacities.max_response_bytes,
        capacities.max_event_count,
        capacities.dictionary_bytes,
        capacities.dictionary_entries,
    ] {
        hash_u32(&mut hasher, value);
    }
    hash_signature(&mut hasher, parameters, result);
    hasher.finalize().into()
}

fn hash_signature(hasher: &mut Sha256, parameters: &[Parameter], result: &ResultShape) {
    hash_u32(
        hasher,
        u32::try_from(parameters.len()).expect("decoded parameter count came from u32"),
    );
    for parameter in parameters {
        match parameter {
            Parameter::Scalar { index, value, kind } => {
                hash_u32(hasher, PARAMETER_SCALAR);
                hash_u32(
                    hasher,
                    u32::try_from(*index).expect("decoded parameter index came from u32"),
                );
                hash_field(hasher, value.as_bytes());
                hash_u32(
                    hasher,
                    match kind {
                        ScalarKind::I64 => SCALAR_I64,
                        ScalarKind::Bool => SCALAR_BOOL,
                    },
                );
            }
            Parameter::Owned {
                index,
                value,
                owner_ordinal,
                resource,
                lifecycle,
                payload_wire_kind,
            } => {
                hash_u32(hasher, PARAMETER_OWNED_RESOURCE);
                hash_u32(
                    hasher,
                    u32::try_from(*index).expect("decoded parameter index came from u32"),
                );
                hash_field(hasher, value.as_bytes());
                hash_u32(
                    hasher,
                    u32::try_from(*owner_ordinal).expect("decoded owner ordinal came from u32"),
                );
                hash_field(hasher, resource.as_bytes());
                hash_field(hasher, lifecycle.as_bytes());
                hash_u32(hasher, *payload_wire_kind);
            }
        }
    }
    match result {
        ResultShape::ScalarI64 => hash_u32(hasher, RESULT_SCALAR_I64),
        ResultShape::OwnedInput {
            parameter_index,
            owner_ordinal,
        } => {
            hash_u32(hasher, RESULT_OWNED_INPUT);
            hash_u32(
                hasher,
                u32::try_from(*parameter_index)
                    .expect("decoded result parameter index came from u32"),
            );
            let value = match &parameters[*parameter_index] {
                Parameter::Owned { value, .. } => value,
                Parameter::Scalar { .. } => {
                    unreachable!("owned result was validated against an owned parameter")
                }
            };
            hash_field(hasher, value.as_bytes());
            hash_u32(
                hasher,
                u32::try_from(*owner_ordinal).expect("decoded result owner ordinal came from u32"),
            );
        }
    }
}

pub(crate) fn derive_symbols(fingerprints: &Fingerprints) -> (String, String) {
    let mut seed_hasher = Sha256::new();
    seed_hasher.update(SYMBOL_SEED_DOMAIN);
    for fingerprint in [
        &fingerprints.physical_module,
        &fingerprints.function_template,
        &fingerprints.execution_cleanup,
        &fingerprints.event_dictionary,
        &fingerprints.request_schema,
        &fingerprints.response_schema,
        &fingerprints.call_abi,
        &fingerprints.call_contract,
    ] {
        hash_field(&mut seed_hasher, fingerprint);
    }
    let seed: [u8; FINGERPRINT_BYTES] = seed_hasher.finalize().into();
    (
        derive_symbol(GETTER_SYMBOL_DOMAIN, &seed, "descriptor_v2"),
        derive_symbol(CALLABLE_SYMBOL_DOMAIN, &seed, "call_v2"),
    )
}

fn derive_symbol(domain: &[u8], seed: &[u8; FINGERPRINT_BYTES], suffix: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hash_field(&mut hasher, seed);
    let digest = hasher.finalize();
    let mut symbol = String::with_capacity(4 + 48 + suffix.len());
    symbol.push_str("spx_");
    for byte in &digest[..24] {
        use std::fmt::Write as _;
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.push('_');
    symbol.push_str(suffix);
    symbol
}

fn is_c_symbol(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Offsets {
        declared: usize,
        target_bytes: usize,
        physical_module: usize,
        execution_cleanup: usize,
        getter_bytes: usize,
        callable_bytes: usize,
        call_abi_tag: usize,
        obligations: usize,
        max_request: usize,
        dictionary_entries: usize,
        parameter_count: usize,
        first_index: usize,
        first_payload_kind: usize,
        second_kind: usize,
        result_parameter: usize,
        result_value: usize,
        result_ordinal: usize,
    }

    fn push_u32(output: &mut Vec<u8>, value: u32) -> usize {
        let offset = output.len();
        output.extend_from_slice(&value.to_le_bytes());
        offset
    }

    fn push_text(output: &mut Vec<u8>, value: &str) -> (usize, usize) {
        let length = push_u32(output, value.len().try_into().unwrap());
        let bytes = output.len();
        output.extend_from_slice(value.as_bytes());
        (length, bytes)
    }

    fn fixture_fingerprints(target: &str, module: &str) -> Fingerprints {
        let schema = schema_fingerprint();
        let target_fingerprint = target_fingerprint(target.as_bytes());
        let semantic_module = [0x31; FINGERPRINT_BYTES];
        Fingerprints {
            schema,
            target: target_fingerprint,
            semantic_module,
            physical_module: physical_module_fingerprint(
                &schema,
                &target_fingerprint,
                &semantic_module,
                module.as_bytes(),
            ),
            function_template: [0x32; FINGERPRINT_BYTES],
            execution_cleanup: [0x33; FINGERPRINT_BYTES],
            event_dictionary: [0x34; FINGERPRINT_BYTES],
            request_schema: request_schema_fingerprint(),
            response_schema: response_schema_fingerprint(),
            call_abi: call_abi_fingerprint(),
            call_contract: [0; FINGERPRINT_BYTES],
        }
    }

    fn canonical_wire() -> (Vec<u8>, Offsets) {
        canonical_wire_with_limits(16, 2048, 8)
    }

    fn canonical_wire_with_limits(
        max_event_count: u32,
        dictionary_bytes: u32,
        dictionary_entries: u32,
    ) -> (Vec<u8>, Offsets) {
        let target = current_target_tag();
        let module = "test.module";
        let function = "test.select";
        let parameters = vec![
            Parameter::Owned {
                index: 0,
                value: "token.value".to_owned(),
                owner_ordinal: 0,
                resource: "token.type".to_owned(),
                lifecycle: "token.drop".to_owned(),
                payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
            },
            Parameter::Scalar {
                index: 1,
                value: "delta.value".to_owned(),
                kind: ScalarKind::I64,
            },
        ];
        let result = ResultShape::OwnedInput {
            parameter_index: 0,
            owner_ordinal: 0,
        };
        let capacities = Capacities {
            max_request_bytes: request_capacity(&parameters).unwrap(),
            max_response_bytes: response_capacity(&result, max_event_count).unwrap(),
            max_event_count,
            dictionary_bytes,
            dictionary_entries,
        };
        let mut fingerprints = fixture_fingerprints(&target, module);
        fingerprints.call_contract = call_contract_fingerprint(
            &target,
            &fingerprints,
            module,
            function,
            &capacities,
            &parameters,
            &result,
        );
        let (getter, callable) = derive_symbols(&fingerprints);

        let mut output = Vec::new();
        output.extend_from_slice(MAGIC);
        push_u32(&mut output, VERSION);
        push_u32(&mut output, HEADER_SIZE);
        let declared = push_u32(&mut output, 0);
        let (_, target_bytes) = push_text(&mut output, &target);
        output.extend_from_slice(&fingerprints.schema);
        output.extend_from_slice(&fingerprints.target);
        output.extend_from_slice(&fingerprints.semantic_module);
        let physical_module = output.len();
        output.extend_from_slice(&fingerprints.physical_module);
        output.extend_from_slice(&fingerprints.function_template);
        let execution_cleanup = output.len();
        output.extend_from_slice(&fingerprints.execution_cleanup);
        output.extend_from_slice(&fingerprints.event_dictionary);
        output.extend_from_slice(&fingerprints.request_schema);
        output.extend_from_slice(&fingerprints.response_schema);
        output.extend_from_slice(&fingerprints.call_abi);
        output.extend_from_slice(&fingerprints.call_contract);
        push_text(&mut output, module);
        push_text(&mut output, function);
        let (_, getter_bytes) = push_text(&mut output, &getter);
        let (_, callable_bytes) = push_text(&mut output, &callable);
        let call_abi_tag = push_u32(&mut output, CALL_ABI_TAG);
        let obligations = push_u32(&mut output, REQUIRED_OBLIGATIONS);
        let max_request = push_u32(&mut output, capacities.max_request_bytes);
        push_u32(&mut output, capacities.max_response_bytes);
        push_u32(&mut output, capacities.max_event_count);
        push_u32(&mut output, capacities.dictionary_bytes);
        let dictionary_entries = push_u32(&mut output, capacities.dictionary_entries);
        let parameter_count = push_u32(&mut output, 2);

        push_u32(&mut output, PARAMETER_OWNED_RESOURCE);
        let first_index = push_u32(&mut output, 0);
        push_text(&mut output, "token.value");
        push_u32(&mut output, 0);
        push_text(&mut output, "token.type");
        push_text(&mut output, "token.drop");
        let first_payload_kind = push_u32(&mut output, OWNED_PAYLOAD_WIRE_KIND);

        push_u32(&mut output, PARAMETER_SCALAR);
        push_u32(&mut output, 1);
        push_text(&mut output, "delta.value");
        let second_kind = push_u32(&mut output, SCALAR_I64);

        push_u32(&mut output, RESULT_OWNED_INPUT);
        let result_parameter = push_u32(&mut output, 0);
        let (_, result_value) = push_text(&mut output, "token.value");
        let result_ordinal = push_u32(&mut output, 0);
        let total = u32::try_from(output.len()).unwrap();
        replace_u32(&mut output, declared, total);
        (
            output,
            Offsets {
                declared,
                target_bytes,
                physical_module,
                execution_cleanup,
                getter_bytes,
                callable_bytes,
                call_abi_tag,
                obligations,
                max_request,
                dictionary_entries,
                parameter_count,
                first_index,
                first_payload_kind,
                second_kind,
                result_parameter,
                result_value,
                result_ordinal,
            },
        )
    }

    fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn canonical_callable_descriptor_round_trips_every_bound_field() {
        let (wire, _) = canonical_wire();
        let descriptor = Descriptor::parse(&wire).unwrap();
        assert_eq!(descriptor.target, current_target_tag());
        assert_eq!(descriptor.module, "test.module");
        assert_eq!(descriptor.function, "test.select");
        assert_eq!(descriptor.call_abi_tag, CALL_ABI_TAG);
        assert_eq!(descriptor.obligations, REQUIRED_OBLIGATIONS);
        assert_eq!(descriptor.capacities.max_request_bytes, 100);
        assert_eq!(descriptor.capacities.max_response_bytes, 140);
        assert_eq!(descriptor.capacities.max_event_count, 16);
        assert_eq!(descriptor.capacities.dictionary_bytes, 2048);
        assert_eq!(descriptor.capacities.dictionary_entries, 8);
        assert_eq!(descriptor.parameters.len(), 2);
        assert!(matches!(
            descriptor.parameters[0],
            Parameter::Owned {
                owner_ordinal: 0,
                payload_wire_kind: OWNED_PAYLOAD_WIRE_KIND,
                ..
            }
        ));
        assert_eq!(
            descriptor.result,
            ResultShape::OwnedInput {
                parameter_index: 0,
                owner_ordinal: 0,
            }
        );
    }

    #[test]
    fn envelope_target_and_trailing_data_fail_closed() {
        let (wire, offsets) = canonical_wire();

        let mut wrong_magic = wire.clone();
        wrong_magic[7] = b'9';
        assert_eq!(
            Descriptor::parse(&wrong_magic),
            Err(DescriptorError::UnsupportedSchema)
        );
        let mut wrong_target = wire.clone();
        wrong_target[offsets.target_bytes] ^= 1;
        assert_eq!(
            Descriptor::parse(&wrong_target),
            Err(DescriptorError::WrongTarget)
        );
        let mut wrong_length = wire.clone();
        replace_u32(&mut wrong_length, offsets.declared, 20);
        assert_eq!(
            Descriptor::parse(&wrong_length),
            Err(DescriptorError::Malformed)
        );
        let mut trailing = wire;
        trailing.push(0);
        assert_eq!(
            Descriptor::parse(&trailing),
            Err(DescriptorError::Malformed)
        );
        let trailing_length = u32::try_from(trailing.len()).unwrap();
        replace_u32(&mut trailing, offsets.declared, trailing_length);
        assert_eq!(
            Descriptor::parse(&trailing),
            Err(DescriptorError::Malformed)
        );
    }

    #[test]
    fn fingerprints_symbols_abi_and_obligations_are_exact() {
        let (wire, offsets) = canonical_wire();
        for offset in [offsets.physical_module, offsets.execution_cleanup] {
            let mut hostile = wire.clone();
            hostile[offset] ^= 1;
            assert_eq!(
                Descriptor::parse(&hostile),
                Err(DescriptorError::NonCanonical)
            );
        }
        for offset in [offsets.getter_bytes, offsets.callable_bytes] {
            let mut hostile = wire.clone();
            hostile[offset] = b'x';
            assert_eq!(
                Descriptor::parse(&hostile),
                Err(DescriptorError::NonCanonical)
            );
        }
        let mut abi = wire.clone();
        replace_u32(&mut abi, offsets.call_abi_tag, 2);
        assert_eq!(
            Descriptor::parse(&abi),
            Err(DescriptorError::UnsupportedSchema)
        );
        let mut obligations = wire;
        replace_u32(&mut obligations, offsets.obligations, 0x07);
        assert_eq!(
            Descriptor::parse(&obligations),
            Err(DescriptorError::NonCanonical)
        );
    }

    #[test]
    fn capacities_and_counts_are_bounded_before_allocation() {
        let (wire, offsets) = canonical_wire();
        for (offset, value) in [
            (offsets.max_request, 0),
            (offsets.max_request, MAX_CALL_WIRE_BYTES + 1),
            (offsets.dictionary_entries, 0),
            (offsets.parameter_count, u32::MAX),
        ] {
            let mut hostile = wire.clone();
            replace_u32(&mut hostile, offset, value);
            assert_eq!(
                Descriptor::parse(&hostile),
                Err(DescriptorError::NonCanonical)
            );
        }
    }

    #[test]
    fn protocol_limits_accept_the_boundary_and_reject_one_over() {
        let (boundary, _) = canonical_wire_with_limits(
            MAX_EVENT_COUNT,
            MAX_DICTIONARY_BYTES,
            MAX_DICTIONARY_ENTRIES,
        );
        let parsed = Descriptor::parse(&boundary).unwrap();
        assert_eq!(parsed.capacities.max_event_count, MAX_EVENT_COUNT);
        assert_eq!(parsed.capacities.dictionary_bytes, MAX_DICTIONARY_BYTES);
        assert_eq!(parsed.capacities.dictionary_entries, MAX_DICTIONARY_ENTRIES);

        for limits in [
            (
                MAX_EVENT_COUNT + 1,
                MAX_DICTIONARY_BYTES,
                MAX_DICTIONARY_ENTRIES,
            ),
            (
                MAX_EVENT_COUNT,
                MAX_DICTIONARY_BYTES + 1,
                MAX_DICTIONARY_ENTRIES,
            ),
            (
                MAX_EVENT_COUNT,
                MAX_DICTIONARY_BYTES,
                MAX_DICTIONARY_ENTRIES + 1,
            ),
        ] {
            let (hostile, _) = canonical_wire_with_limits(limits.0, limits.1, limits.2);
            assert_eq!(
                Descriptor::parse(&hostile),
                Err(DescriptorError::NonCanonical)
            );
        }

        assert_eq!(
            Descriptor::parse(&vec![0; MAX_DESCRIPTOR_BYTES]),
            Err(DescriptorError::UnsupportedSchema)
        );
        assert_eq!(
            Descriptor::parse(&vec![0; MAX_DESCRIPTOR_BYTES + 1]),
            Err(DescriptorError::Malformed)
        );
    }

    #[test]
    fn parameter_and_owned_result_mappings_are_canonical() {
        let (wire, offsets) = canonical_wire();
        for (offset, value) in [
            (offsets.first_index, 1),
            (offsets.first_payload_kind, 2),
            (offsets.second_kind, 99),
            (offsets.result_parameter, 1),
            (offsets.result_ordinal, 1),
        ] {
            let mut hostile = wire.clone();
            replace_u32(&mut hostile, offset, value);
            assert!(matches!(
                Descriptor::parse(&hostile),
                Err(DescriptorError::NonCanonical | DescriptorError::UnsupportedSchema)
            ));
        }
        let mut wrong_value = wire;
        wrong_value[offsets.result_value] ^= 1;
        assert_eq!(
            Descriptor::parse(&wrong_value),
            Err(DescriptorError::NonCanonical)
        );
    }

    #[test]
    fn truncated_invalid_utf8_nul_and_unknown_tags_are_rejected() {
        let (wire, offsets) = canonical_wire();
        for length in 0..wire.len() {
            assert!(
                Descriptor::parse(&wire[..length]).is_err(),
                "accepted truncated prefix of length {length}"
            );
        }
        let mut invalid_utf8 = wire.clone();
        invalid_utf8[offsets.target_bytes] = 0xff;
        assert_eq!(
            Descriptor::parse(&invalid_utf8),
            Err(DescriptorError::Malformed)
        );
        let mut nul_symbol = wire.clone();
        nul_symbol[offsets.getter_bytes] = 0;
        assert_eq!(
            Descriptor::parse(&nul_symbol),
            Err(DescriptorError::NonCanonical)
        );
        let mut unknown_parameter = wire;
        replace_u32(&mut unknown_parameter, offsets.first_index - 4, 99);
        assert_eq!(
            Descriptor::parse(&unknown_parameter),
            Err(DescriptorError::NonCanonical)
        );
    }

    #[test]
    fn every_encoded_byte_is_structural_or_authenticated() {
        let (wire, _) = canonical_wire();
        Descriptor::parse(&wire).unwrap();
        for offset in 0..wire.len() {
            let mut hostile = wire.clone();
            hostile[offset] ^= 1;
            assert!(
                Descriptor::parse(&hostile).is_err(),
                "accepted single-byte mutation at offset {offset}"
            );
        }
    }
}
