//! The actual turn-two Complete owner reaches a private Report only after ACK.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    LiveContinuedStagedStepV8, LiveContinuedTerminalDriverFailureV8, LiveContinuedTerminalPhaseV8,
};

pub(super) fn run<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    staged: LiveContinuedStagedStepV8<'j>,
    weak: &[std::sync::Weak<[u8]>],
    fault: u8,
) {
    let session = journal.begin_session().unwrap();
    let sequence = session.sequence();
    let (fuel, stages, turn, attempt, _) = session.inventory.step_reduce_facts().unwrap();
    assert_eq!((turn, attempt), (2, 0));
    let before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
    let offset = match fault {
        18 | 21 => None,
        19 => Some((2, LiveContinuedTerminalPhaseV8::CleanupSettled)),
        20 => Some((6, LiveContinuedTerminalPhaseV8::Terminal)),
        _ => unreachable!("bounded terminal driver fault"),
    };
    if let Some((offset, _)) = offset {
        journal
            .lease
            .try_borrow_mut()
            .unwrap()
            .test_fail_before_write(sequence + offset);
    }
    assert!(journal.terminal_evidence().is_err());
    let releases = Cell::new(0);
    let input = crate::live_invocation::source_journal::SourceTerminalEvidenceInput {
        completed_stages: stages,
        omitted_stage_rows: stages,
        stage_rows: Vec::new(),
        checked_run_evidence: None,
    };
    if fault == 21 {
        let claimed = staged
            .finish_complete_report(journal, |_| releases.set(releases.get() + 1), input)
            .unwrap_or_else(|failure| panic!("{}", failure_detail(&failure)));
        assert!(
            journal.prospective_reduce.borrow().is_some(),
            "claim alone does not retire the hold"
        );
        journal.quarantine();
        let (owner, error) = claimed
            .into_delivery_projection()
            .err()
            .expect("lost live authority must refuse delivery");
        assert_eq!(
            error,
            crate::live_invocation::source_journal::SourceJournalError::Poisoned
        );
        assert!(weak.iter().any(|root| root.strong_count() == 1));
        assert_eq!(releases.get(), 1);
        assert!(
            journal.prospective_reduce.borrow().is_some(),
            "failed projection retains hold obligation"
        );
        drop(owner);
        assert!(journal.begin_session().is_err());
        assert!(weak.iter().all(|root| root.upgrade().is_none()));
        assert_eq!(releases.get(), 1, "refusal cannot retry cleanup");
        return;
    }
    let result =
        staged.finish_complete_projection(journal, |_| releases.set(releases.get() + 1), input);
    if let Some((offset, expected_phase)) = offset {
        let failure = result
            .err()
            .expect("physical prewrite must refuse the closure");
        assert!(
            weak.iter().any(|root| root.strong_count() == 1),
            "a refused closure retains an original physical Report leaf"
        );
        assert!(
            matches!(&failure, LiveContinuedTerminalDriverFailureV8::Append { phase, .. } if *phase == expected_phase),
            "expected {expected_phase:?} prewrite refusal: {}",
            failure_detail(&failure)
        );
        assert_eq!(
            releases.get(),
            1,
            "the canonical non-result cleanup runs once"
        );
        assert!(journal.begin_session().is_err());
        assert!(journal.hold().is_err());
        assert!(
            journal.terminal_evidence().is_err(),
            "a preterminal prefix cannot become delivery evidence"
        );
        let persisted = journal
            .lease
            .try_borrow()
            .unwrap()
            .test_persisted_snapshot()
            .unwrap();
        assert!(persisted.starts_with(&before));
        assert_eq!(
            persisted.iter().filter(|byte| **byte == b'\n').count(),
            before.iter().filter(|byte| **byte == b'\n').count() + offset - 1,
            "the refused row adds no persisted bytes or authentication row"
        );
        drop(failure);
    } else {
        let delivered = result.unwrap_or_else(|failure| panic!("{}", failure_detail(&failure)));
        assert_eq!(
            releases.get(),
            1,
            "the canonical non-result cleanup runs once"
        );
        assert!(
            journal.prospective_reduce.borrow().is_none(),
            "later lineage settles the same inherited hold"
        );
        let after = journal.begin_session().unwrap();
        assert_eq!(after.sequence(), sequence + 6);
        let (after_fuel, after_stages, after_turn, after_attempt, row) =
            after.inventory.step_reduce_facts().unwrap();
        assert_eq!(
            (after_fuel, after_stages, after_turn, after_attempt),
            (fuel, stages, 2, 0),
            "terminal closure does not renew spent funding"
        );
        assert!(
            matches!(row, EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot {
            turn: Some(2), status: crate::live_invocation::source_journal::SourceTerminalStatus::Complete,
            committed_stage_fuel, stages: actual_stages, carrier: Some(_), ..
        }) if *committed_stage_fuel == fuel && *actual_stages == stages)
        );
        assert_eq!(delivered["kind"], "complete");
        assert!(delivered["report"]["fields"].as_array().is_some());
        let recovered = journal.terminal_evidence().unwrap();
        assert_eq!(
            recovered.status(),
            crate::live_invocation::source_journal::SourceTerminalStatus::Complete
        );
        assert!(recovered.carrier().is_some());
        assert!(!recovered.evidence().is_empty());
        assert_eq!(
            delivered["terminal_evidence"].as_str().map(str::as_bytes),
            Some(recovered.evidence()),
            "delivery retains the exact authenticated evidence bytes"
        );
        assert!(
            serde_json::to_vec(&delivered).unwrap().len()
                <= crate::live_invocation::source_journal::MAX_SOURCE_CARRIER_BYTES
                    + 2 * crate::live_invocation::source_journal::MAX_SOURCE_TERMINAL_EVIDENCE_BYTES
                    + 128
        );
        let final_bytes = journal.lease.try_borrow_mut().unwrap().read().unwrap();
        assert_eq!(
            journal.lease.try_borrow_mut().unwrap().read().unwrap(),
            final_bytes
        );
        assert!(
            weak.iter().all(|root| root.upgrade().is_none()),
            "the consuming delivery boundary leaves no Report owner behind"
        );
    }
    assert_eq!(
        releases.get(),
        1,
        "dropping the retained boundary never retries cleanup"
    );
}

fn failure_detail(failure: &LiveContinuedTerminalDriverFailureV8<'_>) -> String {
    match failure {
        LiveContinuedTerminalDriverFailureV8::Select {
            phase,
            error,
            owner,
        } => {
            format!("terminal closure {phase:?}: {error:?}; (staged present, cleanup ACK present, nominal case) = {:?}", owner.test_terminal_admission_shape())
        }
        LiveContinuedTerminalDriverFailureV8::Session { phase, error, .. } => {
            format!("terminal closure {phase:?}: {error:?}")
        }
        LiveContinuedTerminalDriverFailureV8::Append { phase, .. } => {
            format!("terminal closure {phase:?}: physical append refused")
        }
        LiveContinuedTerminalDriverFailureV8::Advance { phase, .. } => {
            format!("terminal closure {phase:?}: ACK advancement refused")
        }
        LiveContinuedTerminalDriverFailureV8::Shape { phase, .. } => {
            format!("terminal closure {phase:?}: unexpected owner shape")
        }
        LiveContinuedTerminalDriverFailureV8::Delivery { error, .. } => {
            format!("terminal delivery projection refused: {error:?}")
        }
        LiveContinuedTerminalDriverFailureV8::Release(_) => "physical Step cleanup refused".into(),
        LiveContinuedTerminalDriverFailureV8::Move(_) => "physical Report transfer refused".into(),
    }
}

#[test]
fn owned_continued_step_turn_two_terminal_report_claims_original_owner() {
    continued_reduce_chain_step_ack(18, true, false);
}

#[test]
fn owned_continued_step_turn_two_terminal_report_receipt_refusal_keeps_release_sticky() {
    continued_reduce_chain_step_ack(19, true, false);
}

#[test]
fn owned_continued_step_turn_two_terminal_report_terminal_refusal_retains_report() {
    continued_reduce_chain_step_ack(20, true, false);
}

#[test]
fn owned_continued_step_turn_two_terminal_report_projection_refusal_retains_report() {
    continued_reduce_chain_step_ack(21, true, false);
}
