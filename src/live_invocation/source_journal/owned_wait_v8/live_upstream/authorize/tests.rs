//! Actual SDK lineage, physical roots and real fixed-adapter ACK windows.
use super::super::model::tests::completed_test_actor;
use super::*;
use std::sync::Arc;
struct Clock;
impl crate::live_invocation::InvocationClock for Clock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for Clock {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_authorize_same_root_granted_and_refused_have_true_refs_and_exact_charges() {
    for refused in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let (runtime, execution) = journal.context().test_runtime_execution();
            let budget = if refused {
                runtime.owned_wait_task_v8(execution).unwrap().budget + 1
            } else {
                3
            };
            let completed = completed_test_actor(&journal, &cancel, &Clock, budget);
            let transfer_wait = completed.wait.clone();
            let transfer_state = completed.owner.checked_facts(execution.wait()).unwrap();
            let transfer_proposal = completed.proposal.ordinary_digest().to_owned();
            let old = completed.owner.test_weak();
            let prior = completed.session.fold_for_live_test();
            let staged = authorize_live_actor_v8(completed)
                .unwrap_or_else(|failed| panic!("actual authorize {:?}", failed.error));
            assert_eq!(
                (
                    staged.transfer,
                    staged.reservation,
                    staged.staged,
                    staged.session.sequence()
                ),
                (17, 18, 19, 20)
            );
            let binding = execution.wait();
            // Real retained registration and Created projection have exactly
            // one generation; the ACTUAL selected Transfer is fold-admitted.
            let created = &journal.context().fold().created;
            let generation = wire::generation_digest_from_created(created).unwrap();
            assert_eq!(generation, staged.held.generation());
            let journal_model::OwnedBodyV8::OwnedRunCreated {
                scope,
                execution: created_execution,
                binding: created_binding,
                store_identity,
                limits,
                ..
            } = created
            else {
                panic!("Created");
            };
            let wrong_generation = wire::recipe_digest(wire::RecipeV8::Generation,
                &serde_json::json!({"scope":scope,"execution":created_execution,"binding":created_binding,"store_identity":store_identity,"limits":limits})).unwrap();
            assert_ne!(generation, wrong_generation);
            assert_eq!(
                staged.transfer_digest,
                wire::recipe_digest(
                    wire::RecipeV8::Transfer,
                    &serde_json::json!({"scope":scope,"generation":generation,"turn":0,"attempt":0,
                    "wait":transfer_wait,"from":binding.helper().function().id.as_str(),
                    "to":binding.authorize().function().id.as_str(),
                    "state_digest":wire::record_argument_digest(&transfer_state),
                    "proposal_digest":transfer_proposal})
                )
                .unwrap()
            );
            let (_, decision) = staged.owner.checked_facts(binding).unwrap();
            assert_eq!(
                decision["declaration"],
                binding.authorize().decision().as_str()
            );
            assert_eq!(
                decision["case"],
                if refused {
                    binding.authorize().refused().as_str()
                } else {
                    binding.authorize().granted().as_str()
                }
            );
            assert!(staged.owner.consumed() > 0);
            assert!(staged.owner.consumed() <= execution.evaluation_fuel() as u64);
            let leaves = staged.owner.test_weak();
            assert_eq!(leaves.len(), old.len() + usize::from(!refused));
            for leaf in &old {
                assert_eq!(leaf.strong_count(), 1);
                assert!(leaves.iter().any(|new| std::sync::Weak::ptr_eq(new, leaf)));
            }
            if !refused {
                let seal = leaves
                    .iter()
                    .find(|new| old.iter().all(|old| !std::sync::Weak::ptr_eq(old, new)))
                    .unwrap();
                assert_eq!(seal.strong_count(), 1);
            }
            let folded = staged.session.fold_for_live_test();
            assert_eq!(
                folded.tail,
                if refused {
                    fold::TailV8::PendingRefusal
                } else {
                    fold::TailV8::PendingReady
                }
            );
            assert_eq!(folded.stages, 3);
            assert_eq!(
                folded.reserved_total,
                5 * execution.evaluation_fuel() as u64
            );
            assert_eq!(
                folded.consumed_recorded,
                prior.consumed_recorded + staged.owner.consumed()
            );
            staged
                .held
                .validate_prefix(
                    staged.session.sequence(),
                    staged.session.acknowledged_bytes(),
                )
                .unwrap();
            drop(staged);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        });
    }
}

#[test]
fn owned_wait_live_authorize_rejects_unprojected_generation_before_transfer_write() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let completed = completed_test_actor(&journal, &cancel, &Clock, 3);
        let binding = journal.context().test_runtime_execution().1.wait();
        let state = completed.owner.checked_facts(binding).unwrap();
        let weak = completed.owner.test_weak();
        let digest = completed.proposal.ordinary_digest().to_owned();
        let journal_model::OwnedBodyV8::OwnedRunCreated {
            scope,
            execution,
            binding: binding_id,
            store_identity,
            limits,
            ..
        } = &journal.context().fold().created
        else {
            panic!("Created");
        };
        let correct =
            wire::generation_digest_from_created(&journal.context().fold().created).unwrap();
        assert_eq!(correct, completed.held.generation());
        let wrong = wire::recipe_digest(wire::RecipeV8::Generation, &serde_json::json!({
            "scope":scope,"execution":execution,"binding":binding_id,"store_identity":store_identity,"limits":limits,
        })).unwrap();
        assert_ne!(correct, wrong);
        let selected = |generation| {
            let transfer_digest = wire::recipe_digest(wire::RecipeV8::Transfer, &serde_json::json!({
                "scope":scope,"generation":generation,"turn":0,"attempt":0,"wait":completed.wait,
                "from":binding.helper().function().id.as_str(),"to":binding.authorize().function().id.as_str(),
                "state_digest":wire::record_argument_digest(&state),"proposal_digest":digest,
            })).unwrap();
            EntryV8::Owned(journal_model::OwnedBodyV8::OwnedStateTransferReserved {
                turn: 0,
                attempt: 0,
                wait: completed.wait.clone(),
                from: binding.helper().function().id.as_str().into(),
                to: binding.authorize().function().id.as_str().into(),
                state_digest: wire::record_argument_digest(&state),
                proposal_digest: digest.clone(),
                transfer_digest,
            })
        };
        let session = match journal.begin_session().unwrap().append(EntryV8::Ordinary(
            SourceJournalEntry::ProposalAdmitted {
                turn: 0,
                attempt: 0,
                proposal_digest: digest.clone(),
            },
        )) {
            Ok(session) => session,
            Err(_) => panic!("actual admitted Proposal"),
        };
        let bytes = session.acknowledged_bytes();
        let failure = session
            .append(selected(&wrong))
            .err()
            .expect("old generation refused");
        assert_eq!(append_failure_error(&failure), SourceJournalError::Order);
        let super::super::super::append::AppendFailureV8::CandidateRefused {
            session, error, ..
        } = failure
        else {
            panic!("pure candidate refusal, no physical uncertainty");
        };
        assert_eq!(error, SourceJournalError::Order);
        assert_eq!(
            (session.sequence(), session.acknowledged_bytes()),
            (16, bytes)
        );
        completed.held.validate_guard().unwrap();
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        let session = match session.append(selected(&correct)) {
            Ok(session) => session,
            Err(_) => panic!("single canonical generation admitted"),
        };
        assert_eq!(session.sequence(), 17);
        assert!(weak.iter().all(|w| w.strong_count() == 1));
        drop(completed);
        assert!(weak.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_wait_live_authorize_generation_projection_refuses_scope_substitution() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let original = &journal.context().fold().created;
        for edit in [0, 1, 2, 3] {
            let mut created = original.clone();
            let journal_model::OwnedBodyV8::OwnedRunCreated { scope, .. } = &mut created else {
                panic!("Created");
            };
            match edit {
                0 => {
                    let invocation = scope.as_object_mut().unwrap().remove("invocation").unwrap();
                    scope["invocation_id"] = invocation;
                }
                1 => scope["program_root"] = false.into(),
                2 => scope["policy_epoch"] = (-1).into(),
                3 => scope["extra"] = 0.into(),
                _ => unreachable!(),
            }
            assert_eq!(
                wire::generation_digest_from_created(&created),
                Err(SourceJournalError::Malformed)
            );
        }
        assert_eq!(journal.begin_session().unwrap().sequence(), 0);
        journal.hold().unwrap();
    });
}
fn weak(owner: &LiveAuthorizeFailureOwnerV8) -> Vec<std::sync::Weak<[u8]>> {
    match owner {
        LiveAuthorizeFailureOwnerV8::Resumed(x) => x.test_weak(),
        LiveAuthorizeFailureOwnerV8::Transferred(x) => x.test_weak(),
        LiveAuthorizeFailureOwnerV8::Staged(x) => x.test_weak(),
        LiveAuthorizeFailureOwnerV8::Transfer(x) => match x {
            LiveStateTransferOutcomeV8::Moved(x) | LiveStateTransferOutcomeV8::GuardLost(x) => {
                x.test_weak()
            }
            LiveStateTransferOutcomeV8::Refused(x) => x.test_weak(),
        },
        LiveAuthorizeFailureOwnerV8::Authorize(x) => match x {
            LiveAuthorizeOutcomeV8::Refused(x) => x.test_weak(),
            LiveAuthorizeOutcomeV8::Staged(x)
            | LiveAuthorizeOutcomeV8::Failed(x)
            | LiveAuthorizeOutcomeV8::GuardLost(x) => x.test_weak(),
        },
    }
}
#[test]
fn owned_wait_live_authorize_each_ack_fault_keeps_one_owner_and_failed_reservation_never_evaluates()
{
    for append in 16..=20 {
        for persisted in [false, true] {
            CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
                |context, mut lease, key| {
                    let context = context.with_initialization(&lease).unwrap();
                    if persisted {
                        lease.test_fail_after_write(append);
                    } else {
                        lease.test_fail_before_write(append);
                    }
                    let journal =
                        SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
                    let cancel = crate::agent_runtime::AgentCancellation::new();
                    let completed = completed_test_actor(&journal, &cancel, &Clock, 3);
                    let old = completed.owner.test_weak();
                    let failed = authorize_live_actor_v8(completed)
                        .err()
                        .expect("faulted ACK");
                    assert_eq!(failed.error, SourceJournalError::Uncertain);
                    let leaves = weak(&failed.owner);
                    assert_eq!(leaves.len(), old.len() + usize::from(append == 20));
                    assert!(old.iter().all(|w| w.strong_count() == 1));
                    if append == 20 {
                        let LiveAuthorizeFailureOwnerV8::Staged(x) = &failed.owner else {
                            panic!("only Staged ACK follows evaluator");
                        };
                        assert!(x.consumed() > 0);
                    } else {
                        assert!(!matches!(
                            &failed.owner,
                            LiveAuthorizeFailureOwnerV8::Staged(_)
                                | LiveAuthorizeFailureOwnerV8::Authorize(_)
                        ));
                    }
                    assert_eq!(
                        failed.held.validate_guard(),
                        Err(SourceJournalError::Poisoned)
                    );
                    drop(failed);
                    assert!(leaves.iter().all(|w| w.upgrade().is_none()));
                },
            );
        }
    }
}
struct DeadlineClock<'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    stop: usize,
}
impl crate::live_invocation::InvocationClock for DeadlineClock<'_> {
    fn now_millis(&self) -> i64 {
        if self.journal.begin_session().unwrap().sequence() >= self.stop {
            1000
        } else {
            1
        }
    }
}
impl SourceInvocationClock for DeadlineClock<'_> {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_authorize_deadline_before_and_after_reservation_has_zero_source_entry() {
    for stop in [18, 19] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let clock = DeadlineClock {
                journal: &journal,
                stop,
            };
            let completed = completed_test_actor(&journal, &cancel, &clock, 3);
            let old = completed.owner.test_weak();
            let failed = authorize_live_actor_v8(completed).err().expect("expired");
            assert_eq!(failed.error, SourceJournalError::Time);
            assert!(matches!(
                &failed.owner,
                LiveAuthorizeFailureOwnerV8::Transferred(_)
            ));
            assert_eq!(weak(&failed.owner).len(), old.len());
            let session = journal.begin_session().unwrap();
            assert_eq!(session.sequence(), stop);
            let folded = session.fold_for_live_test();
            let f = journal
                .context()
                .test_runtime_execution()
                .1
                .evaluation_fuel() as u64;
            assert_eq!(
                folded.reserved_total,
                if stop == 19 { 5 * f } else { 4 * f }
            );
            assert_eq!(folded.stages, if stop == 19 { 3 } else { 2 });
            assert!(old.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(old.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
#[test]
fn owned_wait_live_authorize_cancelled_completed_retains_root_and_writes_nothing() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let completed = completed_test_actor(&journal, &cancel, &Clock, 3);
        let old = completed.owner.test_weak();
        cancel.cancel();
        let failed = authorize_live_actor_v8(completed).err().expect("cancelled");
        assert_eq!(journal.begin_session().unwrap().sequence(), 15);
        assert_eq!(weak(&failed.owner).len(), old.len());
        assert!(old.iter().all(|w| w.strong_count() == 1));
        drop(failed);
        assert!(old.iter().all(|w| w.upgrade().is_none()));
    });
}

#[test]
fn owned_wait_live_authorize_terminal_answer_substitution_is_prewrite_and_preserves_owner() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
        let context = context.with_initialization(&lease).unwrap();
        let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
        let cancel = crate::agent_runtime::AgentCancellation::new();
        let mut completed = completed_test_actor(&journal, &cancel, &Clock, 3);
        let old = completed.owner.test_weak();
        let prior = completed.session.fold_for_live_test();
        completed.owner.test_substitute_answer();
        let failed = authorize_live_actor_v8(completed)
            .err()
            .expect("different answer");
        assert_eq!(failed.error, SourceJournalError::Binding);
        let session = journal.begin_session().unwrap();
        assert_eq!(session.sequence(), 15);
        assert_eq!(
            session.fold_for_live_test().reserved_total,
            prior.reserved_total
        );
        assert!(old.iter().all(|w| w.strong_count() == 1));
        drop(failed);
        assert!(old.iter().all(|w| w.upgrade().is_none()));
    });
}
struct FaultClock<'j> {
    journal: &'j SourceOwnedWaitJournalV8,
    cancel: &'j crate::agent_runtime::AgentCancellation,
    panic_at: Option<usize>,
    cancel_at: Option<usize>,
}
impl crate::live_invocation::InvocationClock for FaultClock<'_> {
    fn now_millis(&self) -> i64 {
        let sequence = self.journal.begin_session().unwrap().sequence();
        if self.panic_at.is_some_and(|n| sequence >= n) {
            panic!("actual authorize guard clock");
        }
        if self.cancel_at.is_some_and(|n| sequence >= n) {
            self.cancel.cancel();
        }
        1
    }
}
impl SourceInvocationClock for FaultClock<'_> {
    fn clock_domain(&self) -> &str {
        "owned.wait.test"
    }
}
#[test]
fn owned_wait_live_authorize_callback_panic_before_entry_and_cancel_after_staged_never_publish_permission(
) {
    for panic in [false, true] {
        CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, lease, key| {
            let context = context.with_initialization(&lease).unwrap();
            let journal = SourceOwnedWaitJournalV8::open(Arc::new(context), key, lease).unwrap();
            let cancel = crate::agent_runtime::AgentCancellation::new();
            let clock = FaultClock {
                journal: &journal,
                cancel: &cancel,
                panic_at: panic.then_some(19),
                cancel_at: (!panic).then_some(20),
            };
            let completed = completed_test_actor(&journal, &cancel, &clock, 3);
            let old = completed.owner.test_weak();
            let failed = authorize_live_actor_v8(completed)
                .err()
                .expect("guard fault");
            let leaves = weak(&failed.owner);
            if panic {
                assert_eq!(failed.error, SourceJournalError::Poisoned);
                assert!(matches!(
                    &failed.owner,
                    LiveAuthorizeFailureOwnerV8::Transferred(_)
                ));
                assert_eq!(leaves.len(), old.len());
                assert_eq!(
                    failed.held.validate_guard(),
                    Err(SourceJournalError::Poisoned)
                );
            } else {
                assert_eq!(failed.error, SourceJournalError::Binding);
                assert!(matches!(
                    &failed.owner,
                    LiveAuthorizeFailureOwnerV8::Staged(_)
                ));
                assert_eq!(leaves.len(), old.len() + 1);
                assert_eq!(journal.begin_session().unwrap().sequence(), 20);
            }
            assert!(old.iter().all(|w| w.strong_count() == 1));
            drop(failed);
            assert!(leaves.iter().all(|w| w.upgrade().is_none()));
        });
    }
}
