//! Cross-implementation tests between the compiler v3 encoder and independent host parser.

use semaprax::codegen::{
    emit_native_adapter_admission, emit_native_callable_admission,
    emit_native_callable_settlement_proof, emit_native_callable_v3_descriptor,
};
use semaprax::hir::DeclarationId;
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;
use sha2::{Digest, Sha256};

use crate::descriptor::Descriptor as DescriptorV1;
use crate::descriptor_v2::Descriptor as DescriptorV2;
use crate::descriptor_v3::{
    encode_descriptor, encode_graph, Action, Descriptor, Linkage, Outcome, Parameter,
    ResourceState, ResultShape, TraceOutcome,
};
use crate::settlement_proof::BoundSettlementProof;

fn compiler_descriptor() -> semaprax::codegen::NativeCallableV3DescriptorArtifact {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    emit_native_callable_v3_descriptor(&corpus.program, &DeclarationId::new("token.discard-two"))
        .unwrap()
}

#[test]
fn compiler_v3_descriptor_is_accepted_with_exact_bound_metadata() {
    let artifact = compiler_descriptor();
    let descriptor = Descriptor::parse(artifact.bytes()).unwrap();

    assert_eq!(descriptor.module, "test.owned_resource_corpus");
    assert_eq!(descriptor.function, "token.discard-two");
    assert_eq!(descriptor.getter_symbol, artifact.getter_symbol());
    assert_eq!(descriptor.execute_symbol, artifact.execute_symbol());
    assert_eq!(descriptor.settle_symbol, artifact.settle_symbol());
    assert_eq!(
        descriptor.fingerprints.call_contract,
        artifact.call_contract()
    );
    assert_eq!(descriptor.call_abi_tag, 3);
    assert_eq!(descriptor.obligations, 0x03ff);
    assert_eq!(
        descriptor.linkage,
        if cfg!(target_os = "ios") {
            Linkage::IosStatic
        } else {
            Linkage::Dynamic
        }
    );

    assert_eq!(descriptor.parameters.len(), 2);
    assert!(descriptor
        .parameters
        .iter()
        .enumerate()
        .all(|(ordinal, parameter)| matches!(
            parameter,
            Parameter::Owned {
                index,
                owner_ordinal,
                ..
            } if *index == ordinal && *owner_ordinal == ordinal
        )));
    assert_eq!(descriptor.result, ResultShape::ScalarI64);

    assert_eq!(descriptor.capacities.request, 144);
    assert_eq!(
        descriptor.capacities.execute_response,
        124 + 4 * descriptor.capacities.event_count
    );
    assert_eq!(descriptor.capacities.frame, 216);
    assert_eq!(descriptor.capacities.decision, 172);
    assert_eq!(descriptor.capacities.action_evidence, 188);
    assert_eq!(descriptor.capacities.candidate_receipt, 264);
    assert_eq!(descriptor.capacities.resource_count, 2);
    assert_eq!(descriptor.capacities.checkpoint_count, 4);
    assert_eq!(descriptor.capacities.graph_work_units, 8);
    assert_eq!(descriptor.capacities.active_frames, 256);
    assert_eq!(descriptor.capacities.quarantined_frames, 64);
    let per_active = descriptor.capacities.request
        + descriptor.capacities.execute_response
        + descriptor.capacities.frame
        + descriptor.capacities.decision
        + descriptor.capacities.action_evidence
        + descriptor.capacities.candidate_receipt;
    let per_quarantine = descriptor.capacities.frame + descriptor.capacities.candidate_receipt;
    assert_eq!(
        descriptor.capacities.instance_reserved_bytes,
        256 * per_active + 64 * per_quarantine
    );

    assert_eq!(descriptor.graph.function, descriptor.function);
    assert_eq!(descriptor.graph.resource_count, 2);
    assert_eq!(descriptor.graph.checkpoints.len(), 4);
    assert_eq!(descriptor.graph.starts, [1]);
    assert!(descriptor
        .graph
        .checkpoints
        .first()
        .unwrap()
        .resources
        .iter()
        .all(|state| *state == ResourceState::Live));
    assert!(descriptor
        .graph
        .checkpoints
        .iter()
        .any(|checkpoint| checkpoint.outcome == Some(Outcome::ScalarSuccess)));
    assert!(descriptor
        .graph
        .edges
        .iter()
        .any(|edge| matches!(&edge.action, Action::Finalize(_))));
    assert!(descriptor
        .graph
        .edges
        .iter()
        .any(|edge| matches!(&edge.action, Action::CertifyOutcome(_))));
    assert_eq!(
        hex(&Sha256::digest(encode_graph(&descriptor.graph).unwrap())),
        "0da4af442f926506e2dcfc71fd0a6895dd3f48223922f06bae4f2ac9cf67a380"
    );
}

#[test]
fn all_fourteen_authoritative_corpus_cases_parse_and_reencode_canonically() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    assert_eq!(corpus.cases.len(), 14);
    let mut witnessed_outcomes = std::collections::BTreeSet::new();
    for case in &corpus.cases {
        let artifact = emit_native_callable_v3_descriptor(
            &corpus.program,
            &DeclarationId::new(case.function_id),
        )
        .unwrap();
        let descriptor = Descriptor::parse(artifact.bytes()).unwrap();
        assert_eq!(descriptor.function, case.function_id);
        assert_eq!(encode_descriptor(&descriptor).unwrap(), artifact.bytes());
        for edge in &descriptor.graph.edges {
            if let Action::CertifyOutcome(evidence) = &edge.action {
                witnessed_outcomes.insert(match evidence.outcome {
                    TraceOutcome::ScalarSuccess => 1,
                    TraceOutcome::OwnedSuccess => 2,
                    TraceOutcome::Failure { .. } => 3,
                });
            }
        }
    }
    assert_eq!(
        witnessed_outcomes,
        std::collections::BTreeSet::from([1, 2, 3])
    );
}

#[test]
fn exact_linux_compiler_descriptor_known_answer_is_stable() {
    if cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu",
        target_pointer_width = "64",
        target_endian = "little"
    )) {
        let artifact = compiler_descriptor();
        assert_eq!(artifact.bytes().len(), 1_722);
        assert_eq!(
            hex(&Sha256::digest(artifact.bytes())),
            "53096cf416ba8fe1fb7ca694649c81fcc93d3b5cfe71cdf5413c01b8f04ab64e"
        );
        assert_eq!(
            hex(&artifact.call_contract()),
            "9b9c13fc2c5cf506bd99b0cdcec326f7394bd94665d373dec2861f467149e496"
        );
        assert!(Descriptor::parse(artifact.bytes()).is_ok());
    }
}

#[test]
fn compiler_v3_descriptor_rejects_every_truncation_trailing_and_byte_mutation() {
    let canonical = compiler_descriptor().bytes().to_vec();
    assert!(Descriptor::parse(&canonical).is_ok());
    for length in 0..canonical.len() {
        assert!(
            Descriptor::parse(&canonical[..length]).is_err(),
            "accepted compiler descriptor prefix length {length}"
        );
    }
    for trailing in [0_u8, 1, 0x7f, 0xff] {
        let mut hostile = canonical.clone();
        hostile.push(trailing);
        assert!(Descriptor::parse(&hostile).is_err());
    }
    for offset in 0..canonical.len() {
        let mut hostile = canonical.clone();
        hostile[offset] ^= 1;
        assert!(
            Descriptor::parse(&hostile).is_err(),
            "accepted compiler descriptor mutation at {offset}"
        );
    }
}

#[test]
fn all_four_artifact_families_reject_every_other_magic_without_fallback() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = DeclarationId::new("token.discard-two");
    let v1 = emit_native_adapter_admission(&corpus.program, &function, "descriptor-v1.h").unwrap();
    let v2 = emit_native_callable_admission(&corpus.program, &function).unwrap();
    let proof = emit_native_callable_settlement_proof(&corpus.program, &function).unwrap();
    let v3 = compiler_descriptor();

    assert!(DescriptorV1::parse(v1.descriptor()).is_ok());
    assert!(DescriptorV2::parse(v2.descriptor()).is_ok());
    assert!(BoundSettlementProof::parse(proof.bytes()).is_ok());
    assert!(Descriptor::parse(v3.bytes()).is_ok());

    for hostile in [v2.descriptor(), proof.bytes(), v3.bytes()] {
        assert!(DescriptorV1::parse(hostile).is_err());
    }
    for hostile in [v1.descriptor(), proof.bytes(), v3.bytes()] {
        assert!(DescriptorV2::parse(hostile).is_err());
    }
    for hostile in [v1.descriptor(), v2.descriptor(), v3.bytes()] {
        assert!(BoundSettlementProof::parse(hostile).is_err());
    }
    assert!(Descriptor::parse(v1.descriptor()).is_err());
    assert!(Descriptor::parse(v2.descriptor()).is_err());
    assert!(Descriptor::parse(proof.bytes()).is_err());
}

fn hex(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(value, "{byte:02x}").unwrap();
    }
    value
}
