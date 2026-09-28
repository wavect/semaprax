use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::{
    compile_owned_authorize_v2, compile_owned_frame_helper_v2,
};
use std::path::Path;
use std::sync::Weak;

const SOURCE: &str = r#"
module owned.authorize;
@id("state") record State {
 @id("state.z") first: Bytes,
 @id("state.a") second: Bytes,
 @id("state.budget") budget: i64,
}
@id("observation") record Observation { @id("observation.budget") budget: i64, }
@id("proposal") record Proposal {
 @id("proposal.budget") budget: i64,
 @id("proposal.urgent") urgent: bool,
 @id("proposal.sequence") sequence: usize,
}
@id("decision") variant Decision {
 @id("decision.granted") Granted { @id("decision.seal") seal: Bytes, @id("decision.budget") budget: i64, },
 @id("decision.refused") Refused { @id("decision.code") code: i64, },
}
@id("park") fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
 let proposal = yield observation;
 state
}
@id("authorize") fn authorize(state: borrow State, budget: i64, urgent: bool, sequence: usize) -> Decision {
 let seal = [65u8, 90u8];
 if budget <= state.budget && sequence > 0usize {
  Decision::Granted { seal: bytes_copy(array_as_slice(seal)), budget: budget }
 } else { Decision::Refused { code: if urgent { 2 } else { 1 } } }
}
@id("main") fn main() -> i64 { 0 }
"#;
fn proof(source: &str) -> CheckedOwnedAuthorizeV2 {
    let p = hir::resolve(&crate::check(source, Path::new("owned-authorize.spx")).unwrap()).unwrap();
    let helper = compile_owned_frame_helper_v2(&p, &DeclarationId::new("park")).unwrap();
    compile_owned_authorize_v2(&helper, &DeclarationId::new("authorize")).unwrap()
}
fn input() -> OwnedFrameInput {
    OwnedFrameInput {
        declaration: DeclarationId::new("state"),
        fields: vec![
            OwnedFrameInputField {
                identity: DeclarationId::new("state.z"),
                value: OwnedFrameInputValue::Bytes(vec![]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("state.a"),
                value: OwnedFrameInputValue::Bytes(vec![0]),
            },
            OwnedFrameInputField {
                identity: DeclarationId::new("state.budget"),
                value: OwnedFrameInputValue::Scalar(ArgumentValue::Int(10)),
            },
        ],
    }
}
fn completed(p: &CheckedOwnedAuthorizeV2, budget: i64) -> CompletedOwnedAgentStateV2 {
    let state = admit_owned_agent_state_input(p.helper(), input())
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let prep = prepare_owned_copy_wait_v2(
        state,
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("observation"),
            fields: vec![ArgumentValue::Int(10)],
        },
    )
    .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let OwnedCopyWaitStepV2::Parked(parked) =
        begin_owned_copy_wait_v2(prep, &mut OwnedFrameBudget::new(100).unwrap())
            .unwrap_or_else(|_| panic!("PID"))
    else {
        panic!()
    };
    let OwnedCopyWaitStepV2::Terminal(t) = resume_owned_copy_wait_v2(
        parked,
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("proposal"),
            fields: vec![
                ArgumentValue::Int(budget),
                ArgumentValue::Bool(true),
                ArgumentValue::Usize(1),
            ],
        },
        &mut OwnedFrameBudget::new(100).unwrap(),
    )
    .unwrap_or_else(|_| panic!("PID")) else {
        panic!()
    };
    let OwnedCopyWaitSettledV2::Completed(state) =
        settle_owned_copy_wait_v2(t, || true, |_| panic!("success cleanup"))
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
    else {
        panic!()
    };
    state
}
fn state_weak(state: &CompletedOwnedAgentStateV2) -> Vec<Weak<[u8]>> {
    let Value::Record(r) = state.root.as_ref().unwrap() else {
        panic!()
    };
    ["state.a", "state.z"]
        .iter()
        .map(|id| {
            let Value::Bytes(b) = &r.fields[&DeclarationId::new(*id)] else {
                panic!()
            };
            Arc::downgrade(&b.bytes)
        })
        .collect()
}
fn seal_weak(staged: &StagedOwnedAuthorizeV2) -> Option<Weak<[u8]>> {
    let Value::Variant(v) = staged.decision.as_ref()? else {
        panic!()
    };
    match v.fields.get(staged.plan.seal())? {
        Value::Bytes(b) => Some(Arc::downgrade(&b.bytes)),
        _ => panic!(),
    }
}

#[test]
fn owned_frame_v2_authorize_real_checked_decision_preserves_state_and_matches_ordinary_charges() {
    let p = proof(SOURCE);
    for (budget, case) in [(5, "decision.granted"), (11, "decision.refused")] {
        let state = completed(&p, budget);
        let backing = state_weak(&state);
        let mut fuel = OwnedFrameBudget::new(100).unwrap();
        let staged = stage_owned_authorize_v2(state, &p, &mut fuel)
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
        assert!(staged.failure().is_none());
        assert!(backing.iter().all(|w| w.strong_count() == 1));
        let Value::Variant(v) = staged.decision.as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(v.case.as_str(), case);
        let seal = seal_weak(&staged);
        if let Some(seal) = &seal {
            assert_eq!(&*seal.upgrade().unwrap(), &[65, 90]);
            assert_eq!(seal.strong_count(), 1);
        }
        // Independent ordinary route is an oracle only; the staged route never
        // converts its live State to RetainedValue or re-admits a copied root.
        use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue};
        let args = [
            RetainedValue::Record(RetainedRecord {
                record: DeclarationId::new("state"),
                fields: vec![
                    RetainedField {
                        field: DeclarationId::new("state.z"),
                        value: RetainedValue::Bytes(vec![]),
                    },
                    RetainedField {
                        field: DeclarationId::new("state.a"),
                        value: RetainedValue::Bytes(vec![0]),
                    },
                    RetainedField {
                        field: DeclarationId::new("state.budget"),
                        value: RetainedValue::I64(10),
                    },
                ],
            }),
            RetainedValue::I64(budget),
            RetainedValue::Bool(true),
            RetainedValue::Usize(1),
        ];
        let prepared =
            crate::interpreter::prepare_retained_call(p.helper().program(), "authorize").unwrap();
        let ordinary =
            crate::interpreter::evaluate_retained_call(p.helper().program(), &prepared, &args, 100)
                .unwrap();
        assert_eq!(fuel.consumed(), ordinary.steps_used, "{case}");
        let OwnedAuthorizeSettledV2::Ready(ready) =
            settle_owned_authorize_v2(staged, || true, |_| panic!("empty success vector"))
                .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
        else {
            panic!()
        };
        assert!(backing.iter().all(|w| w.strong_count() == 1));
        drop(ready);
        assert!(backing.iter().all(|w| w.upgrade().is_none()));
        if let Some(seal) = seal {
            assert!(seal.upgrade().is_none());
        }
    }
}

#[test]
fn owned_frame_v2_authorize_failures_retain_real_partial_and_provisional_cleanup_roots() {
    for mode in ["requires", "ensures", "fuel", "cancel", "partial"] {
        let source = match mode {
            "requires" => SOURCE.replace("-> Decision {", "-> Decision requires false {"),
            "ensures" => SOURCE.replace("-> Decision {", "-> Decision ensures false {"),
            _ => SOURCE.to_owned(),
        };
        let p = proof(&source);
        let limits: Vec<_> = if mode == "partial" {
            (1..40).collect()
        } else {
            vec![if mode == "fuel" { 1 } else { 100 }]
        };
        let mut observed_partial = false;
        for limit in limits {
            let state = completed(&p, 5);
            let backing = state_weak(&state);
            let mut fuel = OwnedFrameBudget::new(limit).unwrap();
            if mode == "cancel" {
                fuel.cancel();
            }
            let staged = stage_owned_authorize_v2(state, &p, &mut fuel)
                .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
            let seal = seal_weak(&staged);
            if mode == "partial"
                && (seal.is_none() || staged.provisional || staged.failure().is_none())
            {
                continue;
            }
            assert!(staged.failure().is_some());
            assert!(backing.iter().all(|w| w.strong_count() == 1));
            if mode == "ensures" {
                assert!(staged.provisional);
                assert!(seal.is_some());
            }
            if mode == "partial" {
                observed_partial = true;
                assert_eq!(staged.failure(), Some(&OwnedFrameFailure::FuelExhausted));
            }
            let selected = staged.failure.clone().unwrap();
            let mut after = Vec::new();
            let result = settle_owned_authorize_v2(
                staged,
                || true,
                |a| {
                    after.push((
                        a.source.clone(),
                        seal.as_ref().is_none_or(|s| s.upgrade().is_none()),
                        backing[0].upgrade().is_none(),
                        backing[1].upgrade().is_none(),
                    ));
                },
            )
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
            let OwnedAuthorizeSettledV2::Failed {
                failure,
                decision_operations,
                state_receipt,
                observations_succeeded,
            } = result
            else {
                panic!()
            };
            assert_eq!(failure, selected);
            assert!(observations_succeeded);
            assert_eq!(
                state_receipt.operations,
                p.helper().liveness().result_disposal
            );
            assert_eq!(decision_operations.len(), usize::from(seal.is_some()));
            if seal.is_some() {
                assert_eq!(
                    &after[0].0,
                    &(if mode == "partial" {
                        &p.partial_disposal()[0].source
                    } else {
                        &p.disposal()[0].source
                    })
                );
                assert_eq!((after[0].1, after[0].2, after[0].3), (true, false, false));
                if mode == "partial" {
                    assert!(decision_operations[0].active_case.is_none());
                } else {
                    assert_eq!(
                        decision_operations[0].active_case.as_ref().unwrap().case,
                        *p.granted()
                    );
                }
            }
            assert!(backing.iter().all(|w| w.upgrade().is_none()));
            if mode == "partial" {
                break;
            }
        }
        if mode == "partial" {
            assert!(observed_partial, "actual fuel window after seal transfer");
        }
    }
}

#[test]
fn owned_frame_v2_authorize_rejects_wrong_binding_and_alias_before_source_or_cleanup() {
    let p = proof(SOURCE);
    let other = proof(SOURCE);
    let state = completed(&p, 5);
    let backing = state_weak(&state);
    let mut fuel = OwnedFrameBudget::new(100).unwrap();
    let rejected = stage_owned_authorize_v2(state, &other, &mut fuel)
        .err()
        .expect("distinct retained proof");
    assert_eq!(fuel.consumed(), 0);
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    let mut state = rejected.state;
    let ResumableChannelValue::Record { fields, .. } = &mut state.proposal else {
        panic!()
    };
    fields[0] = ArgumentValue::Bool(true);
    let rejected = stage_owned_authorize_v2(state, &p, &mut fuel)
        .err()
        .expect("wrong Proposal leaf");
    assert_eq!(fuel.consumed(), 0);
    drop(rejected);
    assert!(backing.iter().all(|w| w.upgrade().is_none()));
    let p = proof(&SOURCE.replace("-> Decision {", "-> Decision ensures false {"));
    let staged = stage_owned_authorize_v2(completed(&p, 5), &p, &mut fuel)
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let alias = match staged.decision.as_ref().unwrap() {
        Value::Variant(v) => v.clone(),
        _ => panic!(),
    };
    let mut calls = 0;
    let rejected = settle_owned_authorize_v2(staged, || true, |_| calls += 1)
        .err()
        .expect("alias");
    assert_eq!(calls, 0);
    drop(alias);
    let authority = std::cell::Cell::new(true);
    let seal = seal_weak(&rejected.staged).unwrap();
    let backing = state_weak(&rejected.staged.state);
    let rejected = settle_owned_authorize_v2(
        rejected.staged,
        || authority.get(),
        |_| {
            calls += 1;
            authority.set(false);
        },
    )
    .err()
    .expect("lost guard after seal");
    assert_eq!(calls, 1);
    assert!(seal.upgrade().is_none());
    assert!(backing.iter().all(|w| w.strong_count() == 1));
    let mut repeated = 0;
    assert!(settle_owned_authorize_v2(rejected.staged, || true, |_| repeated += 1).is_err());
    assert_eq!(repeated, 0);
}

#[test]
fn owned_frame_v2_authorize_callback_panics_continue_each_actual_release_and_preserve_failure() {
    let p = proof(&SOURCE.replace("-> Decision {", "-> Decision ensures false {"));
    let state = completed(&p, 5);
    let backing = state_weak(&state);
    let staged = stage_owned_authorize_v2(state, &p, &mut OwnedFrameBudget::new(100).unwrap())
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let seal = seal_weak(&staged).unwrap();
    let selected = staged.failure.clone().unwrap();
    let mut observed = Vec::new();
    let settled = settle_owned_authorize_v2(
        staged,
        || true,
        |_| {
            observed.push((
                seal.upgrade().is_none(),
                backing[0].upgrade().is_none(),
                backing[1].upgrade().is_none(),
            ));
            panic!("observer, after physical release");
        },
    )
    .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let OwnedAuthorizeSettledV2::Failed {
        failure,
        observations_succeeded,
        decision_operations,
        state_receipt,
    } = settled
    else {
        panic!()
    };
    assert_eq!(failure, selected);
    assert!(!observations_succeeded);
    assert_eq!(decision_operations, p.disposal().to_vec());
    assert_eq!(
        state_receipt.operations,
        p.helper().liveness().result_disposal
    );
    assert_eq!(
        observed,
        [
            (true, false, false),
            (true, true, false),
            (true, true, true)
        ]
    );
}

#[test]
fn owned_frame_v2_authorize_actual_source_graph_conditional_and_partial_flags_are_compiler_owned() {
    let checked = crate::check(SOURCE, "owned-authorize.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let again = crate::check(&canonical, "owned-authorize.spx").unwrap();
    assert_eq!(crate::format::canonical(&again), canonical);
    assert_eq!(
        crate::graph::to_json(&checked).unwrap(),
        crate::graph::to_json(&again).unwrap()
    );
    let p = proof(SOURCE);
    assert_eq!(p.disposal().len(), 1);
    assert_eq!(p.partial_disposal().len(), 1);
    assert_eq!(
        p.disposal()[0].source.storage,
        crate::cleanup_plan::StorageId::ProvisionalResult
    );
    assert!(matches!(
        p.partial_disposal()[0].source.storage,
        crate::cleanup_plan::StorageId::Temporary(_)
    ));
    assert_eq!(
        p.disposal()[0].active_case.as_ref().unwrap().case,
        *p.granted()
    );
    assert!(p.partial_disposal()[0].active_case.is_none());
    assert_ne!(
        p.partial_disposal()[0].guard_flag,
        p.disposal()[0].guard_flag
    );
    let graph: serde_json::Value =
        serde_json::from_str(&crate::graph::to_json(&checked).unwrap()).unwrap();
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "authorize")
        .unwrap();
    assert_eq!(node["params"][0]["ownership_mode"], "borrow");
    assert_eq!(
        node["return_type_id"],
        p.function().return_type.identity_key()
    );
    for action in [&p.disposal()[0], &p.partial_disposal()[0]] {
        let slots = node["cleanup"]["slots"].as_array().unwrap();
        let slot = slots
            .iter()
            .find(|slot| match &action.source.storage {
                crate::cleanup_plan::StorageId::ProvisionalResult => {
                    slot["storage"]["kind"] == "provisional_result"
                }
                crate::cleanup_plan::StorageId::Temporary(id) => {
                    slot["storage"]["kind"] == "temporary"
                        && slot["storage"]["expression"] == id.as_str()
                }
                _ => false,
            })
            .expect("exact queried storage in graph");
        let case = slot["field_liveness_shape"]["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["case"] == "decision.granted")
            .unwrap();
        assert_eq!(case["fields"][0]["field"], "decision.seal");
        assert_eq!(case["fields"][0]["shape"]["flag"], action.guard_flag.0);
        assert_eq!(
            case["fields"][0]["shape"]["lifecycle"],
            crate::cleanup::BYTES_DROP_LIFECYCLE_ID
        );
    }
    // The two queried flags come from distinct actual inventory slots, not a
    // copied result flag applied before the variant tag is sealed.
    let inventory = &p.function().cleanup;
    assert!(inventory
        .flags
        .iter()
        .any(|f| f.id == p.disposal()[0].guard_flag));
    assert!(inventory
        .flags
        .iter()
        .any(|f| f.id == p.partial_disposal()[0].guard_flag));
    let ensures = proof(&SOURCE.replace("-> Decision {", "-> Decision ensures false {"));
    let exit = ensures
        .function()
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                exit.continuation,
                crate::cleanup_plan::ExitContinuation::ReturnFailure { .. }
            ) && exit
                .finalize_in_order
                .iter()
                .any(|a| a.source.storage == crate::cleanup_plan::StorageId::ProvisionalResult)
        })
        .expect("ordinary postcondition failure vector");
    assert_eq!(ensures.disposal(), exit.finalize_in_order.as_slice());
    let arithmetic = proof(&SOURCE.replace("budget: budget }", "budget: budget / 0 }"));
    let exit = arithmetic
        .function()
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                exit.continuation,
                crate::cleanup_plan::ExitContinuation::ReturnFailure { .. }
            ) && exit
                .finalize_in_order
                .iter()
                .any(|a| a.source.storage == arithmetic.partial_disposal()[0].source.storage)
        })
        .expect("ordinary mid-constructor failure vector");
    assert_eq!(
        arithmetic.partial_disposal(),
        exit.finalize_in_order.as_slice()
    );
    let source = SOURCE.replace(
        "let seal = [65u8, 90u8];",
        "let extra = bytes_zeroed(1usize); let seal = [65u8, 90u8];",
    );
    let ordinary = hir::resolve(&crate::check(&source, "extra-owner.spx").unwrap()).unwrap();
    let authorize = ordinary
        .functions
        .iter()
        .find(|f| f.id.as_str() == "authorize")
        .unwrap();
    assert!(
        authorize.cleanup_plan.exits.iter().any(|exit| matches!(
            exit.continuation,
            crate::cleanup_plan::ExitContinuation::CommitResult { .. }
        ) && !exit.finalize_in_order.is_empty()),
        "ordinary extra owner creates real non-result cleanup"
    );
    let helper = compile_owned_frame_helper_v2(&ordinary, &DeclarationId::new("park")).unwrap();
    assert_eq!(
        compile_owned_authorize_v2(&helper, &DeclarationId::new("authorize"))
            .err()
            .unwrap()
            .code,
        "SPX-T303"
    );
    let mut hostile = p.function().clone();
    let slot = hostile
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|s| s.storage == crate::cleanup_plan::StorageId::ProvisionalResult)
        .unwrap();
    let crate::cleanup::FieldLivenessShape::Variant { cases, .. } = &mut slot.field_liveness_shape
    else {
        panic!()
    };
    let crate::cleanup::FieldLivenessShape::Leaf { flag, .. } = &mut cases[0].fields[0].shape
    else {
        panic!()
    };
    *flag = crate::cleanup::LivenessFlagId(u32::MAX);
    assert_eq!(
        crate::cleanup_plan::owned_authorize_result_disposal(
            &p.helper().program().declarations,
            &hostile
        )
        .err()
        .unwrap()
        .code,
        "SPX-T303"
    );
}
