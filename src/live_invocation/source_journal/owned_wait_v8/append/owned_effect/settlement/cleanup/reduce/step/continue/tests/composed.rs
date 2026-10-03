//! Exercise the production composition with actual initialized first-turn
//! ownership, one SDK dispatch and one physical target on its second turn.
use super::super::run::finish_second_turn_v8;
use super::*;

fn exercise(fault: Option<(usize, &'static str)>, cancel_before: bool, three_turns: bool) {
    with_moved_profile(three_turns, false, |journal, moved, weak, cancel, _| {
        let (_, execution) = journal.context().test_runtime_execution();
        let model = execution.model();
        let starts = Rc::new(Cell::new(0));
        let response = model_document(journal.context());
        let factory_starts = Rc::clone(&starts);
        let mut factory = move || -> Box<dyn ProviderAdapter> {
            let mut capabilities = base_capabilities("owned-wait-inert-test", true);
            capabilities.max_request_bytes = 65_536;
            Box::new(DispatchProbe {
                starts: Rc::clone(&factory_starts),
                polls: vec![
                    AdapterPoll::Event(AdapterEvent::Delta(response.clone())),
                    AdapterPoll::Event(AdapterEvent::Completed),
                    AdapterPoll::Settled(AdapterSettlement {
                        response_bytes: response.clone(),
                        usage: usage(2, 3, 1),
                    }),
                ]
                .into(),
                capabilities,
            })
        };
        let mut adapter = StreamingSourceProposalAdapter::new_bound_checkpointed(
            &mut factory,
            AdapterInvocationCapability::grant("composed second-turn test"),
            execution.wait().lifecycle().proposal_schema(),
            model.clone(),
            model.invocation_capability(),
            SourceProposalPolicy {
                deployment_binding: model.digest(),
                response_limit: execution.ordinary().response_limit(),
                reservation_units: execution.ordinary().reservation_units(),
            },
        )
        .unwrap_or_else(|_| panic!("checked model adapter"));
        let before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
        let sequence = journal.begin_session().unwrap().sequence();
        if let Some((offset, _)) = fault {
            journal
                .lease
                .try_borrow_mut()
                .unwrap()
                .test_fail_before_write(sequence + offset);
        }
        if cancel_before {
            cancel.cancel();
        }
        let mut host = ActualEffectProbe { calls: 0 };
        let releases = Cell::new(0);
        let result = finish_second_turn_v8(moved, &mut adapter, &mut host, |_| {
            releases.set(releases.get() + 1);
        });
        if cancel_before || three_turns || fault.is_some() {
            let failure = result
                .err()
                .expect("composed run refuses incomplete authority");
            assert_eq!(
                failure.phase(),
                fault.map_or("admission", |(_, phase)| phase)
            );
            assert!(
                weak.iter().any(|root| root.strong_count() == 1),
                "quarantine retains the physical State or Report"
            );
            assert!(journal.begin_session().is_err());
            assert!(journal.hold().is_err());
            assert!(journal.terminal_evidence().is_err());
            let persisted = journal
                .lease
                .try_borrow()
                .unwrap()
                .test_persisted_snapshot()
                .unwrap();
            if let Some((offset, _)) = fault {
                assert!(persisted.starts_with(&before));
                assert_eq!(
                    persisted.iter().filter(|b| **b == b'\n').count(),
                    sequence + offset - 1
                );
                assert_eq!(starts.get(), 1);
                assert_eq!(host.calls, usize::from(offset > 20));
            } else {
                assert_eq!(
                    persisted, before,
                    "unsupported or cancelled admission appends nothing"
                );
                assert_eq!(starts.get(), 0);
                assert_eq!(host.calls, 0);
                assert_eq!(releases.get(), 0);
            }
            assert!(
                journal.prospective_reduce.borrow().is_some(),
                "failed composition cannot retire the inherited Reduce hold"
            );
            let released = releases.get();
            drop(failure);
            assert!(journal.prospective_reduce.borrow().is_some());
            assert!(
                journal.begin_session().is_err(),
                "failure Drop stays quarantined"
            );
            assert!(weak.iter().all(|root| root.upgrade().is_none()));
            assert_eq!(
                releases.get(),
                released,
                "dropping quarantine cannot retry a finalizer"
            );
            false
        } else {
            let delivered =
                result.unwrap_or_else(|failure| panic!("composed phase {}", failure.phase()));
            let ContinuedRunOutcomeV8::Complete(delivered) = delivered else {
                panic!("successful fixture cannot select failed Observe");
            };
            assert_eq!(starts.get(), 1);
            assert_eq!(host.calls, 1);
            assert_eq!(
                releases.get(),
                2,
                "Decision and non-result Step cleanup each run once"
            );
            assert!(
                journal.prospective_reduce.borrow().is_none(),
                "actual terminal Report consumption settles the inherited registry"
            );
            journal
                .hold()
                .expect("normal success Drop must preserve the registered store guard");
            assert_eq!(journal.begin_session().unwrap().sequence(), sequence + 32);
            assert!(
                journal.begin_fresh_session().is_err(),
                "retirement cannot reinitialize a completed history"
            );
            assert_eq!(delivered["kind"], "complete");
            assert!(delivered["report"]["fields"].as_array().is_some());
            let terminal = journal.terminal_evidence().unwrap();
            assert_eq!(
                delivered["terminal_evidence"].as_str().map(str::as_bytes),
                Some(terminal.evidence())
            );
            assert!(
                weak.iter().all(|root| root.upgrade().is_none()),
                "only copied terminal projection escapes"
            );
            true // The existing fixture closes and reopens the actual store.
        }
    });
}

#[test]
fn owned_composed_second_turn_reaches_authenticated_report_once() {
    exercise(None, false, false);
}

#[test]
fn owned_composed_second_turn_model_settlement_fault_quarantines_parked_owner() {
    exercise(Some((9, "model-dispatch")), false, false);
}

#[test]
fn owned_composed_second_turn_terminal_fault_retains_report_without_delivery() {
    exercise(Some((32, "terminal")), false, false);
}

#[test]
fn owned_composed_second_turn_cancelled_admission_has_no_effects() {
    exercise(None, true, false);
}

#[test]
fn owned_composed_second_turn_unsupported_iteration_profile_has_no_effects() {
    exercise(None, false, true);
}
