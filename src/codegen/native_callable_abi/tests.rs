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
    fingerprints: [[u8; 32]; 12],
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
    if usize::try_from(reader.u32()?).map_err(|_| "length overflow".to_owned())? != bytes.len() {
        return Err("inexact total length".to_owned());
    }
    let target = reader.text()?;
    let mut fingerprints = [[0_u8; 32]; 12];
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
        || fingerprints[8] != request_schema_fingerprint()
        || fingerprints[9] != response_schema_fingerprint()
        || fingerprints[10] != call_abi_fingerprint()
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
        trace_path_certificate_fingerprint: fingerprints[7],
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
        &fingerprints[10],
        &module,
        &function,
        max_request,
        max_response,
        &semantics,
        &parameters,
        &result,
    );
    if fingerprints[11] != expected_contract {
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
        &fingerprints[11],
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
    NativeCallableSemantics::new([0x31; 32], [0x57; 32], [0x79; 32], 409, 7, 19).unwrap()
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
    assert_eq!(parsed.fingerprints[8], request_schema_fingerprint());
    assert_eq!(parsed.fingerprints[9], response_schema_fingerprint());
    assert_eq!(parsed.fingerprints[10], call_abi_fingerprint());
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
            0x08, 0x75, 0x57, 0x45, 0x49, 0x01, 0xd1, 0xaa, 0xc4, 0x17, 0xc6, 0xb7, 0x6c, 0xe3,
            0x90, 0xdd, 0x71, 0xe1, 0xe2, 0x9a, 0xd4, 0x59, 0x30, 0xc8, 0x73, 0x08, 0x5b, 0x31,
            0xea, 0x62, 0xa1, 0x84,
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
        NativeCallableSemantics::new([0; 32], [1; 32], [2; 32], 1, 1, 1),
        NativeCallableSemantics::new([1; 32], [0; 32], [2; 32], 1, 1, 1),
        NativeCallableSemantics::new([1; 32], [2; 32], [0; 32], 1, 1, 1),
        NativeCallableSemantics::new([1; 32], [2; 32], [3; 32], 0, 1, 1),
        NativeCallableSemantics::new([1; 32], [2; 32], [3; 32], 1, 0, 1),
        NativeCallableSemantics::new([1; 32], [2; 32], [3; 32], 1, 1, 0),
    ] {
        assert!(invalid.is_err());
    }
}

#[test]
fn normative_size_and_count_boundaries_are_exact() {
    let boundary = NativeCallableSemantics::new(
        [1; 32],
        [2; 32],
        [3; 32],
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
        [3; 32],
        MAX_DICTIONARY_BYTES as usize + 1,
        1,
        1,
    )
    .is_err());
    assert!(NativeCallableSemantics::new(
        [1; 32],
        [2; 32],
        [3; 32],
        1,
        MAX_DICTIONARY_ENTRIES as usize + 1,
        1,
    )
    .is_err());
    assert!(NativeCallableSemantics::new(
        [1; 32],
        [2; 32],
        [3; 32],
        1,
        1,
        MAX_EVENT_COUNT as usize + 1,
    )
    .is_err());
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
