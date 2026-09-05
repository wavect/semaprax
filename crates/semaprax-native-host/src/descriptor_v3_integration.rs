//! Cross-implementation tests between the compiler v3 encoder and independent host parser.

use semaprax::codegen::{
    emit_native_adapter_admission, emit_native_callable_admission,
    emit_native_callable_settlement_proof, emit_native_callable_v3_descriptor,
    emit_private_native_callable_v3_ios_descriptor, PrivateNativeCallableV3IosTarget,
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
use semaprax_native_loader::IosStaticTarget;

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
        156 + 4 * descriptor.capacities.event_count
    );
    assert_eq!(descriptor.capacities.frame, 412);
    assert_eq!(descriptor.capacities.decision, 172);
    assert_eq!(descriptor.capacities.action_evidence, 196);
    assert_eq!(descriptor.capacities.candidate_receipt, 396);
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
        + descriptor.capacities.candidate_receipt
        + 524;
    assert_eq!(
        descriptor.capacities.instance_reserved_bytes,
        (256 + 64) * per_active
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
        "575d43d7710a8b248e85b5f0d1fded007aad77c4e6635972eef3ac8feafcdc09"
    );
}

#[test]
fn every_ios_static_descriptor_identity_matches_exactly_one_loader_target() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = DeclarationId::new("token.discard-two");
    let pairs = [
        (
            PrivateNativeCallableV3IosTarget::DeviceArm64,
            IosStaticTarget::DeviceArm64,
        ),
        (
            PrivateNativeCallableV3IosTarget::SimulatorArm64,
            IosStaticTarget::SimulatorArm64,
        ),
        (
            PrivateNativeCallableV3IosTarget::SimulatorX86_64,
            IosStaticTarget::SimulatorX86_64,
        ),
        (
            PrivateNativeCallableV3IosTarget::MacCatalystArm64,
            IosStaticTarget::MacCatalystArm64,
        ),
        (
            PrivateNativeCallableV3IosTarget::MacCatalystX86_64,
            IosStaticTarget::MacCatalystX86_64,
        ),
    ];
    let mut encoded = Vec::with_capacity(pairs.len());
    for (compiler_target, loader_target) in pairs {
        let artifact = emit_private_native_callable_v3_ios_descriptor(
            &corpus.program,
            &function,
            compiler_target,
        )
        .unwrap();
        let descriptor = Descriptor::parse_for_target(
            artifact.bytes(),
            loader_target.canonical_tag(),
            Linkage::IosStatic,
        )
        .unwrap();
        assert_eq!(descriptor.target, loader_target.canonical_tag());
        encoded.push((loader_target, artifact.bytes().to_vec()));
    }
    for (index, (target, bytes)) in encoded.iter().enumerate() {
        for (other_index, (other_target, other_bytes)) in encoded.iter().enumerate() {
            if index == other_index {
                continue;
            }
            assert_ne!(bytes, other_bytes);
            assert!(Descriptor::parse_for_target(
                bytes,
                other_target.canonical_tag(),
                Linkage::IosStatic,
            )
            .is_err());
            assert_ne!(target.canonical_tag(), other_target.canonical_tag());
        }
    }
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
            "d0d151ab9a9dd5bb3d7eb0de4711076f14fe9cc08ebfaaf0e4a2f2dcb5b838bd"
        );
        assert_eq!(
            hex(&artifact.call_contract()),
            "864428355449d9089d25bc3d583150a23f2504e3e56e6828bd1cda9f7d7eadcd"
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
