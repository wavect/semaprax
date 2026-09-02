use super::*;
use semaprax::codegen::emit_native_adapter_admission;
use semaprax::hir::{self, DeclarationId};
use std::path::Path;

const SOURCE: &str = r#"module test.host_descriptor;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.scalar-mix")
fn scalar_mix(value: own Token, delta: i64, condition: bool) -> i64 {
0
}

@id("token.select-second")
fn select_second(first: own Token, second: own Token) -> Token { second }

@id("test.main")
fn main() -> i64 { 0 }
"#;

struct WireOffsets {
    physical_module: usize,
    function_template: usize,
    owned_tag: usize,
    owned_index: usize,
    owned_ordinal: usize,
    resource_length: usize,
    resource_bytes: usize,
    scalar_kind: usize,
    result_tag: usize,
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

fn canonical_wire() -> (Vec<u8>, WireOffsets) {
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    push_u32(&mut output, VERSION);
    push_u32(&mut output, HEADER_SIZE);
    let declared_length = push_u32(&mut output, 0);
    let target = current_target_tag();
    push_text(&mut output, &target);
    output.extend_from_slice(&schema_fingerprint());
    output.extend_from_slice(&fingerprint_target(target.as_bytes()));
    let physical_module = output.len();
    output.extend_from_slice(&[0x11; FINGERPRINT_BYTES]);
    let function_template = output.len();
    output.extend_from_slice(&[0x22; FINGERPRINT_BYTES]);
    push_text(&mut output, "test.module");
    push_text(&mut output, "test.function");
    push_u32(&mut output, 2);

    let owned_tag = push_u32(&mut output, PARAMETER_OWNED_RESOURCE);
    let owned_index = push_u32(&mut output, 0);
    push_text(&mut output, "token.value");
    let owned_ordinal = push_u32(&mut output, 0);
    let (resource_length, resource_bytes) = push_text(&mut output, "token.type");
    push_text(&mut output, "token.drop");

    push_u32(&mut output, PARAMETER_SCALAR);
    push_u32(&mut output, 1);
    push_text(&mut output, "delta.value");
    let scalar_kind = push_u32(&mut output, SCALAR_I64);

    let result_tag = push_u32(&mut output, RESULT_OWNED_INPUT);
    let result_parameter = push_u32(&mut output, 0);
    let (_, result_value) = push_text(&mut output, "token.value");
    let result_ordinal = push_u32(&mut output, 0);

    let length = u32::try_from(output.len()).unwrap();
    output[declared_length..declared_length + 4].copy_from_slice(&length.to_le_bytes());
    (
        output,
        WireOffsets {
            physical_module,
            function_template,
            owned_tag,
            owned_index,
            owned_ordinal,
            resource_length,
            resource_bytes,
            scalar_kind,
            result_tag,
            result_parameter,
            result_value,
            result_ordinal,
        },
    )
}

fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn artifact(function: &str) -> semaprax::codegen::NativeAdapterAdmissionArtifact {
    let parsed = semaprax::parse(SOURCE, Path::new("host-descriptor-unit.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    emit_native_adapter_admission(&resolved, &DeclarationId::new(function), "descriptor.h").unwrap()
}

#[test]
fn compiler_artifacts_round_trip_scalar_and_multi_owner_shapes_exactly() {
    let scalar = artifact("token.scalar-mix");
    let scalar_descriptor = Descriptor::parse(scalar.descriptor()).unwrap();
    assert_eq!(
        scalar_descriptor.scalar_kinds(),
        [ScalarKind::I64, ScalarKind::Bool]
    );
    assert_eq!(scalar_descriptor.parameters.len(), 3);
    assert_eq!(scalar_descriptor.owner_requirements().len(), 1);
    assert_eq!(scalar_descriptor.getter_symbol(), scalar.getter_symbol());

    let multi_owner = artifact("token.select-second");
    let multi_owner_descriptor = Descriptor::parse(multi_owner.descriptor()).unwrap();
    assert!(multi_owner_descriptor.scalar_kinds().is_empty());
    assert_eq!(multi_owner_descriptor.owner_requirements().len(), 2);
    assert_eq!(
        multi_owner_descriptor.result,
        ResultShape::OwnedInput {
            parameter_index: 1,
            owner_ordinal: 1,
        }
    );
    assert_eq!(
        multi_owner_descriptor.getter_symbol(),
        multi_owner.getter_symbol()
    );
}

#[test]
fn every_body_discriminant_index_and_fingerprint_is_checked() {
    let (canonical, offsets) = canonical_wire();
    Descriptor::parse(&canonical).unwrap();

    let mut cases = Vec::new();
    let mut physical_zero = canonical.clone();
    physical_zero[offsets.physical_module..offsets.physical_module + FINGERPRINT_BYTES].fill(0);
    cases.push(physical_zero);
    let mut function_zero = canonical.clone();
    function_zero[offsets.function_template..offsets.function_template + FINGERPRINT_BYTES].fill(0);
    cases.push(function_zero);
    for (offset, value) in [
        (offsets.owned_tag, 99),
        (offsets.owned_index, 1),
        (offsets.owned_ordinal, 1),
        (offsets.scalar_kind, 99),
        (offsets.result_tag, 99),
        (offsets.result_parameter, 1),
        (offsets.result_ordinal, 1),
    ] {
        let mut hostile = canonical.clone();
        replace_u32(&mut hostile, offset, value);
        cases.push(hostile);
    }
    let mut wrong_result_value = canonical.clone();
    wrong_result_value[offsets.result_value] ^= 1;
    cases.push(wrong_result_value);

    for hostile in cases {
        assert_eq!(
            Descriptor::parse(&hostile),
            Err(DescriptorError::NonCanonical)
        );
    }
}

#[test]
fn identity_lengths_nuls_and_utf8_fail_closed() {
    let (canonical, offsets) = canonical_wire();

    let mut empty = canonical.clone();
    replace_u32(&mut empty, offsets.resource_length, 0);
    assert_eq!(
        Descriptor::parse(&empty),
        Err(DescriptorError::NonCanonical)
    );

    let mut nul = canonical.clone();
    nul[offsets.resource_bytes] = 0;
    assert_eq!(Descriptor::parse(&nul), Err(DescriptorError::NonCanonical));

    let mut invalid_utf8 = canonical;
    invalid_utf8[offsets.resource_bytes] = 0xff;
    assert_eq!(
        Descriptor::parse(&invalid_utf8),
        Err(DescriptorError::Malformed)
    );
}
