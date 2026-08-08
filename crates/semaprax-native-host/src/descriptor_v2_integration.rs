use std::path::Path;

use semaprax::codegen::emit_native_callable_admission;
use semaprax::hir::{self, DeclarationId};
use sha2::{Digest, Sha256};

use crate::descriptor_v2::{Descriptor, Parameter, ResultShape, ScalarKind};

const SOURCE: &str = r#"module test.callable_v2_cross_crate;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.choose")
fn choose(value: own Token, enabled: bool, count: i64) -> Token
requires count >= 0
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn compiler_descriptor() -> semaprax::codegen::NativeCallableAdmissionArtifact {
    let parsed = semaprax::parse(SOURCE, Path::new("callable-v2-cross-crate.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    emit_native_callable_admission(&resolved, &DeclarationId::new("token.choose")).unwrap()
}

#[test]
fn compiler_descriptor_is_accepted_with_exact_signature_and_capacities() {
    let artifact = compiler_descriptor();
    let descriptor = Descriptor::parse(artifact.descriptor()).unwrap();

    assert_eq!(descriptor.module, "test.callable_v2_cross_crate");
    assert_eq!(descriptor.function, "token.choose");
    assert_eq!(descriptor.getter_symbol, artifact.getter_symbol());
    assert_eq!(descriptor.callable_symbol, artifact.callable_symbol());
    assert_eq!(descriptor.capacities.max_request_bytes, 112);
    assert_eq!(
        descriptor.capacities.max_request_bytes,
        artifact.max_request_bytes()
    );
    assert_eq!(
        descriptor.capacities.max_response_bytes,
        artifact.max_response_bytes()
    );
    assert_eq!(
        descriptor.capacities.dictionary_bytes as usize,
        artifact.event_dictionary().len()
    );
    let mut dictionary_hasher = Sha256::new();
    dictionary_hasher.update(b"semaprax.semantic-event-dictionary-fingerprint.v1\0");
    dictionary_hasher.update((artifact.event_dictionary().len() as u64).to_le_bytes());
    dictionary_hasher.update(artifact.event_dictionary().as_bytes());
    let dictionary_fingerprint: [u8; 32] = dictionary_hasher.finalize().into();
    assert_eq!(
        descriptor.fingerprints.event_dictionary,
        dictionary_fingerprint
    );
    assert_eq!(
        descriptor.fingerprints.trace_path_certificate,
        artifact.trace_path_certificate().fingerprint()
    );
    assert_eq!(descriptor.parameters.len(), 3);
    assert!(matches!(descriptor.parameters[0], Parameter::Owned { .. }));
    assert!(matches!(
        descriptor.parameters[1],
        Parameter::Scalar {
            kind: ScalarKind::Bool,
            ..
        }
    ));
    assert!(matches!(
        descriptor.parameters[2],
        Parameter::Scalar {
            kind: ScalarKind::I64,
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
fn every_single_byte_mutation_of_compiler_descriptor_fails_closed() {
    let canonical = compiler_descriptor().descriptor().to_vec();
    assert!(Descriptor::parse(&canonical).is_ok());

    for offset in 0..canonical.len() {
        let mut hostile = canonical.clone();
        hostile[offset] ^= 0x01;
        assert!(
            Descriptor::parse(&hostile).is_err(),
            "single-byte mutation at descriptor offset {offset} was accepted"
        );
    }
}

#[test]
fn every_truncation_and_all_trailing_bytes_fail_closed() {
    let canonical = compiler_descriptor().descriptor().to_vec();
    for length in 0..canonical.len() {
        assert!(Descriptor::parse(&canonical[..length]).is_err());
    }
    for trailing in [0_u8, 1, 0x7f, 0xff] {
        let mut hostile = canonical.clone();
        hostile.push(trailing);
        assert!(Descriptor::parse(&hostile).is_err());
    }
}
