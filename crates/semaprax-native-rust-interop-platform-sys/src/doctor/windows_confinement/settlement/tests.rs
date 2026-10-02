//! Host-independent: pure state-machine logic, no Win32 call anywhere here.
use super::*;

#[test]
fn sticky_settlement_preserves_first_terminal_status_over_later_cleanup_attempts() {
    let mut state = StickySettlement::default();
    state.select(Settlement::Failed(FailureReason::ExitCode(7)));
    state.select(Settlement::Completed);
    state.select(Settlement::Cancelled);
    state.select(Settlement::Uncertain(
        UncertainReason::ActiveProcessesNonZero,
    ));
    assert_eq!(
        state.resolve(),
        Some(Settlement::Failed(FailureReason::ExitCode(7)))
    );
}

#[test]
fn sticky_settlement_lets_a_pending_completed_be_overridden_by_a_later_terminal_status() {
    let mut state = StickySettlement::default();
    state.select(Settlement::Completed);
    state.select(Settlement::Uncertain(
        UncertainReason::ActiveProcessesNonZero,
    ));
    assert_eq!(
        state.resolve(),
        Some(Settlement::Uncertain(
            UncertainReason::ActiveProcessesNonZero
        ))
    );
}

#[test]
fn sticky_settlement_never_lets_a_later_completed_override_an_earlier_terminal_status() {
    for first in [
        Settlement::Failed(FailureReason::OutputLimit),
        Settlement::Failed(FailureReason::CpuTimeLimit),
        Settlement::Failed(FailureReason::JobLimitViolation),
        Settlement::Cancelled,
        Settlement::Uncertain(UncertainReason::WaitFailed),
        Settlement::Uncertain(UncertainReason::QueryFailed),
        Settlement::Uncertain(UncertainReason::KillAmbiguous),
        Settlement::Uncertain(UncertainReason::KillWaitTimedOut),
        Settlement::Uncertain(UncertainReason::OutputReadFailed),
    ] {
        let mut state = StickySettlement::default();
        state.select(first);
        state.select(Settlement::Completed);
        assert_eq!(state.resolve(), Some(first), "{first:?}");
    }
}

#[test]
fn sticky_settlement_with_no_selection_resolves_to_none() {
    assert_eq!(StickySettlement::default().resolve(), None);
}

#[test]
fn two_terminal_selections_keep_the_first_regardless_of_which_kind() {
    let mut state = StickySettlement::default();
    state.select(Settlement::Cancelled);
    state.select(Settlement::Failed(FailureReason::ExitCode(1)));
    assert_eq!(state.resolve(), Some(Settlement::Cancelled));
}
