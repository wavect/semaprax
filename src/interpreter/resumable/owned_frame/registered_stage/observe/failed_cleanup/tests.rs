//! Actual compiler/engine capture tests; no journal ACK or source authority.
use super::*;
use crate::hir::DeclarationId;
use crate::resumable_effects::owned_frame::v2::{
    compile_owned_frame_helper_v2, compile_owned_observe_v2,
};
const SOURCE: &str = r#"
module capture.observe;
@id("state") record State { @id("first") first: Bytes, @id("second") second: Bytes, }
@id("observation") record Observation { @id("value") value: i64, }
@id("proposal") record Proposal { @id("proposal.value") value: i64, }
@id("helper") fn helper(state: own State, observation: Observation) -> State yields Observation -> Proposal {
 let proposal = yield observation;
 state
}
@id("observe") fn observe(state: borrow State) -> Observation ensures false { Observation { value: 0 } }
@id("main") fn main() -> i64 { 0 }
"#;
fn failed() -> (FailedOwnedObserveV2, Vec<std::sync::Weak<[u8]>>) {
    let ast = crate::check(SOURCE, "failed-observe-capture.spx").unwrap();
    let p = crate::hir::resolve(&ast).unwrap();
    let helper = compile_owned_frame_helper_v2(&p, &DeclarationId::new("helper")).unwrap();
    let observe = compile_owned_observe_v2(&helper, &DeclarationId::new("observe")).unwrap();
    let input = OwnedFrameInput {
        declaration: DeclarationId::new("state"),
        fields: ["first", "second"]
            .into_iter()
            .map(|id| OwnedFrameInputField {
                identity: DeclarationId::new(id),
                value: OwnedFrameInputValue::Bytes(vec![1]),
            })
            .collect(),
    };
    let argument = admit_owned_agent_state_input(&helper, input)
        .unwrap_or_else(|r| panic!("{:?}", r.diagnostic));
    let weak = super::super::super::super::snapshot::weak_leaves(argument.root.as_ref().unwrap());
    let OwnedObserveStepV2::Failed(failed) =
        observe_owned_agent_state_v2(argument, &observe, &mut OwnedFrameBudget::new(100).unwrap())
            .unwrap_or_else(|r| panic!("{:?}", r.diagnostic))
    else {
        panic!("genuine Ensures failure")
    };
    assert_eq!(weak.len(), 2);
    assert!(weak.iter().all(|w| w.strong_count() == 1));
    (failed, weak)
}
#[test]
fn failed_observe_capture_preserves_mixed_actual_outcomes_in_compiler_order() {
    for panicking in [0, 1] {
        let (failed, weak) = failed();
        let expected = failed.plan.liveness().failure_cleanup.clone();
        assert_eq!(expected.len(), 2);
        let selected = failed.failure.clone();
        let mut seen = Vec::new();
        let result = capture_failed_observe_cleanup_v8(
            failed,
            || true,
            |action| {
                let index = seen.len();
                seen.push(action.clone());
                assert_eq!(
                    weak.iter().filter(|w| w.upgrade().is_none()).count(),
                    index + 1,
                    "observer follows actual removal"
                );
                if index == panicking {
                    panic!("genuine observer panic");
                }
            },
        )
        .unwrap_or_else(|_| panic!("capture of actual releases"));
        assert_eq!(seen, expected);
        assert_eq!(result.settled.receipt.operations, expected);
        assert_eq!(result.settled.failure, selected);
        assert_eq!(
            result.outcomes(),
            if panicking == 0 {
                &[false, true]
            } else {
                &[true, false]
            }
        );
        assert!(!result.settled.observations_succeeded);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    }
}
#[test]
fn failed_observe_capture_authority_loss_retains_actual_partial_owner_and_forbids_retry() {
    let (failed, weak) = failed();
    let authority = Cell::new(true);
    let mut observed = 0;
    let rejection = capture_failed_observe_cleanup_v8(
        failed,
        || authority.get(),
        |_| {
            observed += 1;
            authority.set(false);
        },
    )
    .err()
    .expect("actual postrelease guard loss");
    let ActualFailedObserveCleanupRejectionV8::Owner(rejection) = rejection else {
        panic!("authority loss is not capture or observer failure")
    };
    assert_eq!(observed, 1);
    assert!(rejection.failed.settlement_started);
    assert_eq!(weak.iter().filter(|w| w.upgrade().is_none()).count(), 1);
    let mut retry = 0;
    let result = capture_failed_observe_cleanup_v8(rejection.failed, || true, |_| retry += 1);
    assert!(matches!(
        result,
        Err(ActualFailedObserveCleanupRejectionV8::Owner(_))
    ));
    assert_eq!(retry, 0);
}
#[test]
fn failed_observe_capture_current_guard_panic_is_not_an_observer_outcome() {
    let (failed, weak) = failed();
    let mut observations = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = capture_failed_observe_cleanup_v8(
            failed,
            || panic!("authority callback"),
            |_| observations += 1,
        );
    }));
    assert!(result.is_err());
    assert_eq!(observations, 0);
    assert!(weak.iter().all(|w| w.upgrade().is_none()));
}
