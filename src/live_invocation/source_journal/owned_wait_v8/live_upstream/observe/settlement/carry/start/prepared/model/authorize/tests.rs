//! Actual M Completed→five true ACKs→ordinary source authorization oracle.
use super::super::tests::test_completed;
use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord, RetainedValue as R};

fn entries() -> usize {
    crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_continued_authorize_entries_v8()
}
fn ack<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    owner: LiveOwnedContinuedAuthorizeAppendV8<'j>,
) -> LiveContinuedAuthorizationV8<'j> {
    journal
        .begin_session()
        .unwrap()
        .append_owned_continued_authorize(owner)
        .unwrap_or_else(|_| panic!("real A ACK"))
        .advance_continued_authorize()
        .unwrap_or_else(|_| panic!("actual A boundary"))
}
fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}
fn ordinary_evaluation(
    journal: &SourceOwnedWaitJournalV8,
    state: &serde_json::Value,
    proposal: &CheckedOwnedWaitProposalV8,
) -> crate::interpreter::retained_call::RetainedCallEvaluation {
    let fields = state["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let v = &f["value"];
            let value = if v["kind"] == "bytes" {
                R::Bytes(bytes(v["hex"].as_str().unwrap()))
            } else {
                match v["tag"].as_str().unwrap() {
                    "i64" => R::I64(v["value"].as_i64().unwrap()),
                    "i32" => R::I32(v["value"].as_i64().unwrap() as i32),
                    "bool" => R::Bool(v["value"].as_bool().unwrap()),
                    "u8" => R::U8(v["value"].as_u64().unwrap() as u8),
                    "usize" => R::Usize(v["value"].as_u64().unwrap()),
                    _ => panic!("actual fixture scalar"),
                }
            };
            RetainedField {
                field: DeclarationId::new(f["identity"].as_str().unwrap()),
                value,
            }
        })
        .collect();
    let mut args = vec![R::Record(RetainedRecord {
        record: DeclarationId::new(state["declaration"].as_str().unwrap()),
        fields,
    })];
    let ResumableChannelValue::Record { fields, .. } = proposal.carrier() else {
        panic!("checked flat K")
    };
    args.extend(fields.iter().map(|v| match v {
        crate::interpreter::ArgumentValue::Int(x) => R::I64(*x),
        crate::interpreter::ArgumentValue::Bool(x) => R::Bool(*x),
        crate::interpreter::ArgumentValue::Usize(x) => R::Usize(*x as u64),
        _ => panic!("actual authored Proposal scalar"),
    }));
    let binding = journal.context().ready_runtime().unwrap().1.wait();
    let plan = binding.authorize();
    let prepared = crate::interpreter::retained_call::prepare_retained_call(
        plan.helper().program(),
        plan.function().id.as_str(),
    )
    .unwrap();
    let evaluated = crate::interpreter::retained_call::evaluate_retained_call(
        plan.helper().program(),
        &prepared,
        &args,
        journal
            .context()
            .ready_runtime()
            .unwrap()
            .1
            .evaluation_fuel(),
    )
    .unwrap();
    evaluated
}
fn ordinary(
    journal: &SourceOwnedWaitJournalV8,
    state: &serde_json::Value,
    proposal: &CheckedOwnedWaitProposalV8,
) -> (serde_json::Value, u64) {
    let evaluated = ordinary_evaluation(journal, state, proposal);
    let crate::interpreter::retained_call::RetainedCallOutcome::Returned(R::Variant(v)) =
        evaluated.outcome
    else {
        panic!("ordinary full Decision")
    };
    let fields=v.fields.into_iter().map(|f|{
        let value=match f.value{R::Bytes(b)=>serde_json::json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(&b)}),R::I64(x)=>serde_json::json!({"tag":"i64","value":x}),_=>panic!("closed Decision leaf")};
        serde_json::json!({"identity":f.field.as_str(),"value":value})
    }).collect::<Vec<_>>();
    (
        serde_json::json!({"declaration":v.variant.as_str(),"case":v.case.as_str(),"fields":fields}),
        evaluated.steps_used as u64,
    )
}
#[test]
fn owned_continued_authorize_real_full_f_ack_matches_ordinary_granted_and_refused() {
    for granted in [true, false] {
        test_completed(
            |journal, completed, weak, ledger, state| {
                let expected = ordinary(journal, &state, completed.proposal.as_ref().unwrap());
                let initial = entries();
                let original = completed.session().sequence();
                let selected = completed
                    .prepare_authorize()
                    .unwrap_or_else(|_| panic!("actual Completed admission"));
                assert_eq!(entries(), initial);
                let mut owner = ack(journal, selected);
                for row in 1..5 {
                    let selected = owner.prepare_next().unwrap_or_else(|_| panic!("row {row}"));
                    if row == 3 {
                        assert!(
                            matches!(selected.selected(),EntryV8::Ordinary(SourceJournalEntry::StageReservation{turn:1,attempt:Some(0),role:SourceStageRole::Authorize,fuel})if *fuel==journal.context().ready_runtime().unwrap().1.evaluation_fuel())
                        );
                        assert_eq!(entries(), initial, "true full-F ACK precedes source entry");
                    }
                    owner = ack(journal, selected);
                    assert_eq!(*owner.completed.owner.accounting(), ledger);
                    assert_eq!(entries(), initial + usize::from(row >= 3));
                }
                assert_eq!(owner.current().sequence(), original + 5);
                let actual = owner
                    .actual()
                    .unwrap()
                    .owner
                    .staged_authorize_facts(owner.binding().unwrap())
                    .expect("full source failure=None, including Refused");
                assert_eq!(actual.0, state);
                assert_eq!(actual.1, expected.0);
                assert_eq!(actual.2, expected.1);
                assert_eq!(
                    actual.1["case"].as_str(),
                    Some(if granted {
                        "fixture.agent.type.decision.granted"
                    } else {
                        "fixture.agent.type.decision.refused"
                    })
                );
                let retained = owner.actual().unwrap().owner.test_authorize_weak();
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(retained.iter().all(|w| w.strong_count() == 1));
                if granted {
                    let seal = retained.last().unwrap();
                    assert!(
                        weak.iter().all(|w| !w.ptr_eq(seal)),
                        "new actual seal backing differs from State/history"
                    );
                }
                assert!(
                    owner.prepare_next().is_err(),
                    "full Staged is this packet's endpoint"
                );
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
            granted,
        );
    }
}
#[cfg(unix)]
#[test]
fn owned_continued_authorize_all_ack_faults_are_in_doubt_and_never_repeat_source() {
    for row in 0..5 {
        for mode in 0..4 {
            test_completed(
                |journal, completed, weak, ledger, _| {
                    let mut selected = completed
                        .prepare_authorize()
                        .unwrap_or_else(|_| panic!("true Completed"));
                    for _ in 0..row {
                        selected = ack(journal, selected)
                            .prepare_next()
                            .unwrap_or_else(|_| panic!("actual next A row"));
                    }
                    let before = entries();
                    let number = selected.sequence() + 1;
                    {
                        let mut lease = journal.test_observe_lease().borrow_mut();
                        match mode {
                            0 => lease.test_fail_before_write(number),
                            1 => lease.test_fail_after_write(number),
                            2 => lease.test_fail_before_sync(number),
                            _ => lease.test_fail_after_sync(number),
                        }
                    }
                    assert_eq!(*selected.owner.completed.owner.accounting(), ledger);
                    let failed = journal
                        .begin_session()
                        .unwrap()
                        .append_owned_continued_authorize(selected)
                        .err()
                        .expect("physical fault");
                    assert!(failed.test_is_in_doubt(), "actual row {row}, window {mode}");
                    assert_eq!(entries(), before);
                    assert!(weak.iter().any(|w| w.strong_count() == 1));
                    assert!(journal.hold().is_err());
                    assert!(journal.begin_session().is_err());
                    assert!(weak.iter().all(|w| w.upgrade().is_none()));
                },
                true,
            );
        }
    }
}

#[test]
fn owned_continued_authorize_failures_retain_actual_roots_and_exact_consumed() {
    use crate::execution_revision::typed::TestContinuedAuthorizeV8 as Mode;
    use crate::interpreter::resumable::owned_frame::OwnedFrameFailure;
    for mode in [Mode::Requires, Mode::Ensures, Mode::Arithmetic, Mode::Fuel] {
        super::super::tests::test_completed_profile(
            |journal, completed, weak, ledger, state| {
                let expected =
                    ordinary_evaluation(journal, &state, completed.proposal.as_ref().unwrap());
                let expected_failure = match expected.outcome {
                    crate::interpreter::retained_call::RetainedCallOutcome::LanguageFailure(
                        status,
                    ) => OwnedFrameFailure::Language(status),
                    crate::interpreter::retained_call::RetainedCallOutcome::FuelExhausted => {
                        OwnedFrameFailure::FuelExhausted
                    }
                    _ => panic!("actual authored failure"),
                };
                let count = entries();
                let mut owner = ack(
                    journal,
                    completed
                        .prepare_authorize()
                        .unwrap_or_else(|_| panic!("Completed")),
                );
                for _ in 0..2 {
                    owner = ack(
                        journal,
                        owner.prepare_next().unwrap_or_else(|_| panic!("transfer")),
                    );
                }
                let before = owner.current().sequence();
                let reservation = owner.prepare_next().unwrap_or_else(|_| panic!("full F"));
                assert_eq!(entries(), count);
                let failed = journal
                    .begin_session()
                    .unwrap()
                    .append_owned_continued_authorize(reservation)
                    .unwrap_or_else(|_| panic!("actual full-F ACK"))
                    .advance_continued_authorize()
                    .err()
                    .expect("sole evaluator failure");
                let LiveContinuedAuthorizeAcknowledgmentFailureV8::Entered(failed) = &failed else {
                    panic!("actual post-entry holder")
                };
                let actual = failed
                    .owner
                    .actual()
                    .unwrap()
                    .owner
                    .authorize_failure()
                    .expect("sticky selected source failure");
                assert_eq!(actual, (expected_failure, expected.steps_used as u64));
                assert_eq!(entries(), count + 1);
                assert_eq!(failed.owner.current().sequence(), before + 1);
                assert_eq!(*failed.owner.completed.owner.accounting(), ledger);
                assert!(
                    failed
                        .owner
                        .actual()
                        .unwrap()
                        .owner
                        .staged_authorize_facts(failed.owner.binding().unwrap())
                        .is_none(),
                    "partial/failure never full Staged"
                );
                let retained = failed.owner.actual().unwrap().owner.test_authorize_weak();
                assert!(weak.iter().any(|w| w.strong_count() == 1));
                assert!(retained.iter().all(|w| w.strong_count() == 1));
                if !matches!(mode, Mode::Requires) {
                    assert!(
                        retained.len() > 1,
                        "actual provisional/partial seal stays owned"
                    );
                }
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
            },
            true,
            Some(mode),
        );
    }
}

thread_local! {static AFTER_ENTRY:std::cell::Cell<usize>=const{std::cell::Cell::new(usize::MAX)};static EXPIRED:std::cell::Cell<i64>=const{std::cell::Cell::new(i64::MAX)};}
struct AfterClock;
impl crate::live_invocation::InvocationClock for AfterClock {
    fn now_millis(&self) -> i64 {
        if entries() > AFTER_ENTRY.with(std::cell::Cell::get) {
            panic!("actual post-evaluation clock callback");
        }
        1
    }
}
impl crate::live_invocation::SourceInvocationClock for AfterClock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
struct ExpiredClock;
impl crate::live_invocation::InvocationClock for ExpiredClock {
    fn now_millis(&self) -> i64 {
        EXPIRED.with(std::cell::Cell::get)
    }
}
impl crate::live_invocation::SourceInvocationClock for ExpiredClock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_continued_authorize_strict_before_and_actual_after_guards_keep_boundary_owner() {
    for mode in 0..3 {
        test_completed(
            |journal, completed, weak, ledger, _| {
                let count = entries();
                let mut owner = ack(
                    journal,
                    completed
                        .prepare_authorize()
                        .unwrap_or_else(|_| panic!("Completed")),
                );
                for _ in 0..2 {
                    owner = ack(
                        journal,
                        owner.prepare_next().unwrap_or_else(|_| panic!("transfer")),
                    );
                }
                if mode == 0 {
                    owner.actual().unwrap().owner.test_authorize_cancel();
                    let failure = owner.prepare_next().err().expect("cancel before source");
                    assert_eq!(entries(), count);
                    assert_eq!(*failure.owner.completed.owner.accounting(), ledger);
                    assert!(weak.iter().any(|w| w.strong_count() == 1));
                } else {
                    let ModelOwnerV8::Resumed(actual) = &mut owner.completed.owner else {
                        panic!("actual moved State")
                    };
                    if mode == 1 {
                        EXPIRED.with(|x| {
                            x.set(
                                journal
                                    .context()
                                    .ordinary()
                                    .deadline_millis()
                                    .checked_add(1)
                                    .unwrap(),
                            )
                        });
                        actual.owner.test_authorize_clock(&ExpiredClock);
                        let failed = owner.prepare_next().err().expect("deadline before source");
                        assert_eq!(entries(), count);
                        assert!(weak.iter().any(|w| w.strong_count() == 1));
                        drop(failed);
                    } else {
                        AFTER_ENTRY.with(|x| x.set(count));
                        actual.owner.test_authorize_clock(&AfterClock);
                        let reservation = owner
                            .prepare_next()
                            .unwrap_or_else(|_| panic!("before evaluator"));
                        let failed = journal
                            .begin_session()
                            .unwrap()
                            .append_owned_continued_authorize(reservation)
                            .unwrap_or_else(|_| panic!("full-F ACK"))
                            .advance_continued_authorize()
                            .err()
                            .expect("caught actual post-evaluation clock panic");
                        let LiveContinuedAuthorizeAcknowledgmentFailureV8::Entered(failed) =
                            &failed
                        else {
                            panic!("resulting staged owner")
                        };
                        assert_eq!(entries(), count + 1);
                        assert_eq!(*failed.owner.completed.owner.accounting(), ledger);
                        assert!(failed
                            .owner
                            .actual()
                            .unwrap()
                            .owner
                            .test_authorize_weak()
                            .iter()
                            .all(|w| w.strong_count() == 1));
                        AFTER_ENTRY.with(|x| x.set(usize::MAX));
                    }
                }
                assert!(journal.hold().is_err());
                assert!(journal.begin_session().is_err());
                assert!(weak.iter().all(|w| w.upgrade().is_none()));
            },
            true,
        );
    }
}
