use std::fmt::Write as _;
use std::num::NonZeroU64;
use std::path::Path;

use crate::conformance::{TraceOutcome, TraceResult};
use crate::native_settlement::{
    AdapterAbortReason, SettlementDecision, SettlementError, SettlementOutcome,
};
use crate::owned_resource_corpus::build_owned_resource_corpus_v1;
use crate::owned_resource_corpus::OWNED_RESOURCE_CORPUS_SOURCE_V1;

use super::*;

#[test]
fn authoritative_corpus_derives_deterministically_and_settles_every_checkpoint() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    assert_eq!(corpus.cases.len(), 14);
    let mut functions = BTreeSet::new();
    let aborts = [
        AdapterAbortReason::PhysicalResult(7),
        AdapterAbortReason::MalformedResponse,
        AdapterAbortReason::TraceRejected,
        AdapterAbortReason::HostUnwind,
    ];
    let mut invocation = 1_u64;

    for case in &corpus.cases {
        let function = corpus
            .program
            .functions
            .iter()
            .find(|function| function.id.as_str() == case.function_id)
            .unwrap();
        let first = derive_native_settlement(&corpus.program, &function.id).unwrap();
        let second = derive_native_settlement(&corpus.program, &function.id).unwrap();
        assert_eq!(first, second, "{}", case.scenario_id);
        assert_eq!(
            first.certificate().recovery_contract(),
            first.recovery_contract_fingerprint()
        );
        assert!(first
            .recovery_contract_fingerprint()
            .iter()
            .any(|byte| *byte != 0));

        let expected = match &case.reference.outcome {
            TraceOutcome::Failure { .. } => SettlementOutcome::SemanticFailure,
            TraceOutcome::Success {
                result: TraceResult::Owned { .. },
            } => SettlementOutcome::OwnedSuccess {
                owner_ordinal: u32::try_from(case.expected_owned_result_ordinal.unwrap()).unwrap(),
            },
            TraceOutcome::Success { .. } => SettlementOutcome::ScalarSuccess,
        };
        assert!(
            first
                .certificate()
                .checkpoints()
                .iter()
                .any(|checkpoint| checkpoint.normal_outcome() == Some(expected)),
            "missing accepted outcome for {}",
            case.scenario_id
        );

        if functions.insert(case.function_id) {
            for checkpoint in first.certificate().checkpoints() {
                for reason in aborts {
                    let nonce = NonZeroU64::new(invocation).unwrap();
                    invocation += 1;
                    let mut frame = first
                        .certificate()
                        .prepare_frame(nonce, checkpoint.checkpoint())
                        .unwrap();
                    let decision = SettlementDecision::Abort(reason);
                    let application = first.certificate().settle(&mut frame, decision).unwrap();
                    first
                        .certificate()
                        .validate_receipt(nonce, application.receipt())
                        .unwrap();
                    let receipt = application.receipt().clone();
                    let replay = first.certificate().settle(&mut frame, decision).unwrap();
                    assert_eq!(replay.receipt(), &receipt);
                    assert!(replay.performed_actions().is_empty());
                    assert_eq!(
                        first.certificate().settle(
                            &mut frame,
                            SettlementDecision::Abort(AdapterAbortReason::MalformedResponse)
                        ),
                        if reason == AdapterAbortReason::MalformedResponse {
                            Ok(replay)
                        } else {
                            Err(SettlementError::ConflictingTerminalDecision)
                        }
                    );
                }
                if let Some(outcome) = checkpoint.normal_outcome() {
                    let nonce = NonZeroU64::new(invocation).unwrap();
                    invocation += 1;
                    let mut frame = first
                        .certificate()
                        .prepare_frame(nonce, checkpoint.checkpoint())
                        .unwrap();
                    let accepted = first
                        .certificate()
                        .settle(&mut frame, SettlementDecision::Accept(outcome))
                        .unwrap();
                    first
                        .certificate()
                        .validate_receipt(nonce, accepted.receipt())
                        .unwrap();
                }
            }
        }
    }
}

#[test]
fn corpus_progress_paths_pin_stage_finalize_and_terminal_order() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let derive = |id: &str| {
        derive_native_settlement(&corpus.program, &DeclarationId::new(id))
            .unwrap_or_else(|error| panic!("{id}: {error:?}"))
    };
    assert_eq!(
        outcome_path(
            &derive("token.discard-two"),
            SettlementOutcome::ScalarSuccess
        ),
        "LL-F1->LD-F0->DD-Cscalar->DD"
    );
    assert_eq!(
        outcome_path(
            &derive("token.identity"),
            SettlementOutcome::OwnedSuccess { owner_ordinal: 0 }
        ),
        "L-S0->P-Cowned0->P"
    );
    assert_eq!(
        outcome_path(
            &derive("token.choose-second"),
            SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }
        ),
        "LL-S1->LP-F0->DP-Cowned1->DP"
    );
    assert_eq!(
        outcome_path(
            &derive("token.ensures-false"),
            SettlementOutcome::SemanticFailure
        ),
        "L-S0->P-F0->D-Cfailure->D"
    );
}

#[test]
fn every_corpus_function_pins_its_exact_progress_graph_fingerprint() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let actual = [
        "token.discard",
        "token.discard-two",
        "token.requires",
        "token.checked",
        "token.identity",
        "token.choose-second",
        "token.ensures-false",
    ]
    .map(|function| {
        let derivation =
            derive_native_settlement(&corpus.program, &DeclarationId::new(function)).unwrap();
        (function, hex(&derivation.certificate().fingerprint()))
    });
    let expected = [
        (
            "token.discard",
            "fd57051978df7693945186549712344253d4e7406964187f14ceac29de19560c",
        ),
        (
            "token.discard-two",
            "cfa2ceb812a7dcd5253af3ad977ed5312b9263091655262e6578e9a7a43e8a43",
        ),
        (
            "token.requires",
            "35351c0b7e63e0086d1655e9c60e4c7da791938372bed60908a1c85c8ce57b41",
        ),
        (
            "token.checked",
            "77a3b7528ccab8345b73418250f07a17000160465288a2d11a955a74b9620be2",
        ),
        (
            "token.identity",
            "292b6ae3b8fd0ccec14f14a8975c36d0bcffd216ed0dbe36e97d1e182019a95f",
        ),
        (
            "token.choose-second",
            "a7672a5bd7a8bccf1b2143bd3c50884684e997a2dd4403493a4986274e8b7860",
        ),
        (
            "token.ensures-false",
            "83a5ebc30b24928b94a8020aacf146d54dd813be986449e1513a7f924d683f48",
        ),
    ];
    assert_eq!(
        actual.map(|(_, fingerprint)| fingerprint),
        expected.map(|(_, fingerprint)| fingerprint.to_owned())
    );
}

#[test]
fn recovery_contract_binds_canonical_semantic_module_identity() {
    let first = build_owned_resource_corpus_v1().unwrap().program;
    let renamed_source = OWNED_RESOURCE_CORPUS_SOURCE_V1.replacen(
        "module test.owned_resource_corpus;",
        "module test.owned_resource_corpus_renamed;",
        1,
    );
    let parsed = crate::parse(
        &renamed_source,
        Path::new("owned-resource-corpus-module-rename.spx"),
    )
    .unwrap();
    let renamed = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&first).unwrap();
    crate::hir::validate(&renamed).unwrap();

    let function = DeclarationId::new("token.discard-two");
    let first = derive_native_settlement(&first, &function).unwrap();
    let renamed = derive_native_settlement(&renamed, &function).unwrap();
    assert_ne!(
        first.recovery_contract_fingerprint(),
        renamed.recovery_contract_fingerprint()
    );
    assert_ne!(
        first.certificate().fingerprint(),
        renamed.certificate().fingerprint()
    );
}

#[test]
fn start_only_progress_walk_rejects_skip_duplicate_and_wrong_action_without_mutation() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let derivation =
        derive_native_settlement(&corpus.program, &DeclarationId::new("token.discard-two"))
            .unwrap();
    let certificate = derivation.certificate();
    assert_eq!(certificate.start_checkpoints(), [1]);
    let mut frame = certificate
        .prepare_start_frame(NonZeroU64::new(91).unwrap())
        .unwrap();
    let before = (frame.checkpoint(), frame.resources().to_vec());
    assert_eq!(
        certificate.advance_frame(
            &mut frame,
            SettlementProgressAction::Finalize { owner_ordinal: 0 }
        ),
        Err(SettlementError::ProgressActionNotAdmitted)
    );
    assert_eq!((frame.checkpoint(), frame.resources().to_vec()), before);

    certificate
        .advance_frame(
            &mut frame,
            SettlementProgressAction::Finalize { owner_ordinal: 1 },
        )
        .unwrap();
    let after_first = (frame.checkpoint(), frame.resources().to_vec());
    assert_eq!(
        certificate.advance_frame(
            &mut frame,
            SettlementProgressAction::Finalize { owner_ordinal: 1 }
        ),
        Err(SettlementError::ProgressActionNotAdmitted)
    );
    assert_eq!(
        (frame.checkpoint(), frame.resources().to_vec()),
        after_first
    );
    certificate
        .advance_frame(
            &mut frame,
            SettlementProgressAction::Finalize { owner_ordinal: 0 },
        )
        .unwrap();
    let certify = certificate
        .progress_edges()
        .iter()
        .find(|edge| edge.from() == frame.checkpoint())
        .unwrap()
        .action();
    certificate.advance_frame(&mut frame, certify).unwrap();
    assert_eq!(
        certificate
            .checkpoints()
            .iter()
            .find(|checkpoint| checkpoint.checkpoint() == frame.checkpoint())
            .unwrap()
            .normal_outcome(),
        Some(SettlementOutcome::ScalarSuccess)
    );
}

#[test]
fn recovery_contract_and_certificate_have_exact_known_answers() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = corpus
        .program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let derivation = derive_native_settlement(&corpus.program, &function.id).unwrap();
    assert_eq!(
        hex(&derivation.recovery_contract_fingerprint()),
        "fd1dcd495352f07810e98025b3f5ba104f3af75915d0c143b0d86ba6e0c217b9"
    );
    assert_eq!(derivation.certificate().canonical_json(), "{\"schema\":\"semaprax.native-settlement-certificate.v2\",\"function\":\"token.discard-two\",\"recovery_contract\":\"fd1dcd495352f07810e98025b3f5ba104f3af75915d0c143b0d86ba6e0c217b9\",\"resource_count\":2,\"checkpoints\":[{\"checkpoint\":1,\"resources\":[\"live\",\"live\"],\"normal_outcome\":null,\"abort_cleanup_order\":[1,0],\"accept_cleanup_order\":[]},{\"checkpoint\":2,\"resources\":[\"live\",\"dead\"],\"normal_outcome\":null,\"abort_cleanup_order\":[0],\"accept_cleanup_order\":[]},{\"checkpoint\":3,\"resources\":[\"dead\",\"dead\"],\"normal_outcome\":null,\"abort_cleanup_order\":[],\"accept_cleanup_order\":[]},{\"checkpoint\":4,\"resources\":[\"dead\",\"dead\"],\"normal_outcome\":{\"kind\":\"scalar_success\"},\"abort_cleanup_order\":[],\"accept_cleanup_order\":[]}],\"start_checkpoints\":[1],\"progress_edges\":[{\"from\":1,\"to\":2,\"action\":{\"kind\":\"finalize\",\"owner_ordinal\":1}},{\"from\":2,\"to\":3,\"action\":{\"kind\":\"finalize\",\"owner_ordinal\":0}},{\"from\":3,\"to\":4,\"action\":{\"kind\":\"certify_outcome\",\"trace_evidence\":\"cc43560bb15664722fb9432ef6a1fa9fe1d67d4774bc3d514624f8021f25e26e\"}}]}");
    assert_eq!(
        hex(&derivation.certificate().fingerprint()),
        "cfa2ceb812a7dcd5253af3ad977ed5312b9263091655262e6578e9a7a43e8a43"
    );

    let terminal = derivation
        .certificate()
        .checkpoints()
        .iter()
        .find(|checkpoint| checkpoint.normal_outcome() == Some(SettlementOutcome::ScalarSuccess))
        .unwrap();
    let invocation = NonZeroU64::new(19).unwrap();
    let mut frame = derivation
        .certificate()
        .prepare_frame(invocation, terminal.checkpoint())
        .unwrap();
    let receipt = derivation
        .certificate()
        .settle(
            &mut frame,
            SettlementDecision::Accept(SettlementOutcome::ScalarSuccess),
        )
        .unwrap();
    assert_eq!(receipt.receipt().canonical_json(), "{\"schema\":\"semaprax.native-settlement-receipt.v2\",\"function\":\"token.discard-two\",\"recovery_contract\":\"fd1dcd495352f07810e98025b3f5ba104f3af75915d0c143b0d86ba6e0c217b9\",\"certificate_fingerprint\":\"cfa2ceb812a7dcd5253af3ad977ed5312b9263091655262e6578e9a7a43e8a43\",\"invocation\":19,\"checkpoint\":4,\"decision\":{\"kind\":\"accept\",\"outcome\":{\"kind\":\"scalar_success\"}},\"actions\":[],\"dispositions\":[\"dead\",\"dead\"],\"active_finalizers\":0}");
    assert_eq!(
        hex(&receipt.receipt().fingerprint()),
        "f90c65be0d9a0bc69f141f50e47d5b0424bb445b0848e48b76eea9ee8058dba6"
    );
}

#[test]
fn exact_member_and_hostile_cleanup_plans_fail_before_derivation() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let error =
        derive_native_settlement(&corpus.program, &DeclarationId::new("token.not-in-program"))
            .unwrap_err();
    assert_eq!(error.code, "SPX-I104");

    let mut reordered = corpus.program.clone();
    let function = reordered
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| exit.finalize_in_order.len() == 2)
        .unwrap();
    exit.finalize_in_order.swap(0, 1);
    let function = reordered
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    assert_eq!(
        derive_native_settlement(&reordered, &function.id)
            .unwrap_err()
            .code,
        "SPX-H006"
    );

    let mut duplicate = corpus.program.clone();
    let function = duplicate
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| exit.finalize_in_order.len() == 2)
        .unwrap();
    exit.finalize_in_order[1] = exit.finalize_in_order[0].clone();
    let function = duplicate
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    assert_eq!(
        derive_native_settlement(&duplicate, &function.id)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
        output
    })
}

fn outcome_path(derivation: &NativeSettlementDerivation, outcome: SettlementOutcome) -> String {
    let certificate = derivation.certificate();
    let terminal = certificate
        .checkpoints()
        .iter()
        .find(|checkpoint| checkpoint.normal_outcome() == Some(outcome))
        .unwrap()
        .checkpoint();
    let mut reverse = Vec::new();
    let mut current = terminal;
    while current != 1 {
        let edge = certificate
            .progress_edges()
            .iter()
            .find(|edge| edge.to() == current)
            .unwrap();
        reverse.push(*edge);
        current = edge.from();
    }
    reverse.reverse();
    let mut projection = state_letters(certificate.checkpoints()[0].resources());
    for edge in reverse {
        let action = match edge.action() {
            SettlementProgressAction::Finalize { owner_ordinal } => {
                format!("F{owner_ordinal}")
            }
            SettlementProgressAction::StageOwnedResult { owner_ordinal } => {
                format!("S{owner_ordinal}")
            }
            SettlementProgressAction::CertifyOutcome { .. } => match outcome {
                SettlementOutcome::ScalarSuccess => "Cscalar".to_owned(),
                SettlementOutcome::SemanticFailure => "Cfailure".to_owned(),
                SettlementOutcome::OwnedSuccess { owner_ordinal } => {
                    format!("Cowned{owner_ordinal}")
                }
            },
        };
        let states = certificate.checkpoints()[(edge.to() - 1) as usize].resources();
        projection.push_str(&format!("-{action}->{}", state_letters(states)));
    }
    projection
}

fn state_letters(states: &[SettlementResourceState]) -> String {
    states
        .iter()
        .map(|state| match state {
            SettlementResourceState::Live => 'L',
            SettlementResourceState::ProvisionalResult => 'P',
            SettlementResourceState::Dead => 'D',
            SettlementResourceState::Finalizing => 'F',
            SettlementResourceState::Published => 'U',
        })
        .collect()
}
