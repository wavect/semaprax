//! Later Completed joins the actual authorization/effect/reducer owner chain.
mod terminal;
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveLaterModelCompletedV8;

pub(super) fn run<'j>(
    journal: &'j SourceOwnedWaitJournalV8,
    mut completed: LiveLaterModelCompletedV8<'j>,
    weak: &[std::sync::Weak<[u8]>],
    fault: u8,
) {
    let sequence = journal.begin_session().unwrap().sequence();
    let before = journal.lease.try_borrow_mut().unwrap().read().unwrap();
    if fault == 16 {
        completed.test_corrupt_join_wait();
        let (owner, error) = completed
            .into_continued_model()
            .err()
            .expect("foreign wait refuses the consuming join");
        assert_eq!(
            error,
            crate::live_invocation::source_journal::SourceJournalError::Binding
        );
        assert!(weak.iter().any(|root| root.strong_count() == 1));
        assert!(journal.begin_session().is_err());
        assert_eq!(
            journal
                .lease
                .try_borrow()
                .unwrap()
                .test_persisted_snapshot()
                .unwrap(),
            before
        );
        drop(owner);
        return;
    }
    let joined = completed
        .into_continued_model()
        .unwrap_or_else(|(_, e)| panic!("physical completion join: {e:?}"));
    assert_eq!(journal.begin_session().unwrap().sequence(), sequence);
    assert_eq!(
        journal.lease.try_borrow_mut().unwrap().read().unwrap(),
        before
    );
    assert!(weak.iter().any(|root| root.strong_count() == 1));
    if fault == 17 {
        journal
            .lease
            .try_borrow_mut()
            .unwrap()
            .test_fail_before_write(sequence + 1);
        let failed = advance_live_owned_continued_authorize_v8(journal, joined)
            .err()
            .expect("later ProposalAdmitted physical ACK refusal");
        assert!(matches!(
            failed,
            LiveContinuedAuthorizeDriverFailureV8::Append(_)
        ));
        assert!(weak.iter().any(|root| root.strong_count() == 1));
        assert!(journal.begin_session().is_err());
        assert_eq!(
            journal
                .lease
                .try_borrow()
                .unwrap()
                .test_persisted_snapshot()
                .unwrap(),
            before
        );
        drop(failed);
        return;
    }
    let authorization = advance_live_owned_continued_authorize_v8(journal, joined)
        .unwrap_or_else(|_| panic!("later actual authorization ACKs"));
    assert_eq!(journal.begin_session().unwrap().sequence(), sequence + 5);
    let effect = advance_live_owned_continued_effect_v8(journal, authorization)
        .unwrap_or_else(|_| panic!("later Ready and Consumed ACKs"));
    let intent = advance_live_owned_continued_intent_v8(journal, effect)
        .unwrap_or_else(|_| panic!("later actual effect Intent ACK"));
    let mut host = ActualEffectProbe { calls: 0 };
    let recorded = advance_live_owned_continued_effect_dispatch_v8(journal, intent, &mut host)
        .unwrap_or_else(|_| panic!("later actual effect and settlement"));
    assert_eq!(host.calls, 1);
    let settlement = journal.begin_session().unwrap();
    let (_, _, turn, row) = settlement.continued_settlement_facts().unwrap();
    assert_eq!(turn, 2);
    assert!(matches!(row, EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedEffectSettlementRecorded { turn: 2, attempt: 0, .. })));
    let releases = Rc::new(Cell::new(0));
    let observed = Rc::clone(&releases);
    let cleanup = advance_live_owned_continued_cleanup_v8(journal, recorded, move |_| {
        observed.set(observed.get() + 1)
    })
    .unwrap_or_else(|_| panic!("later Decision cleanup receipt"));
    assert_eq!(releases.get(), 1);
    let reserved = advance_live_owned_continued_reduce_v8(journal, cleanup)
        .unwrap_or_else(|_| panic!("later original Reduce reservation"));
    let accounting = *reserved.accounting();
    let evaluated = reserved
        .evaluate()
        .unwrap_or_else(|_| panic!("later physical Reduce entry"));
    let facts = evaluated
        .stage_facts()
        .unwrap_or_else(|_| panic!("later full Step facts"));
    assert!(facts.step().is_some());
    assert!(facts.consumed() <= facts.allowance());
    assert_eq!(*evaluated.accounting(), accounting);
    let step = evaluated
        .prepare_step()
        .unwrap_or_else(|_| panic!("later owned Step candidate"));
    assert!(matches!(step.selected_row(), EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedReduceStaged { turn: 2, attempt: 0, .. })));
    let acknowledged = journal
        .begin_session()
        .unwrap()
        .append_owned_step(step)
        .unwrap_or_else(|_| panic!("later Step ACK"));
    let crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::step::LiveStepAcknowledgedV8::Continued(staged) = acknowledged.advance_step().unwrap_or_else(|_| panic!("later retained Step")) else { panic!("continued Step") };
    staged.validate_live().unwrap();
    assert_eq!(host.calls, 1);
    if matches!(fault, 18..=21) {
        terminal::run(journal, staged, weak, fault);
    } else {
        drop(staged);
    }
}
#[test]
fn owned_continued_step_turn_two_completed_joins_actual_effect_reduce_step() {
    continued_reduce_chain_step_ack(0, true, false);
}
#[test]
fn owned_continued_step_turn_two_completed_join_rejects_foreign_wait() {
    continued_reduce_chain_step_ack(16, true, false);
}
#[test]
fn owned_continued_step_turn_two_join_authorization_ack_failure_retains_state() {
    continued_reduce_chain_step_ack(17, true, false);
}
