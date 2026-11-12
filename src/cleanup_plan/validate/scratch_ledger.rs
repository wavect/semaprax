//! Cleanup-replay scratch is metered on its own per-program ledger.
//!
//! Independent replay is verification scratch: every carrier it reserves is
//! released when the program's replay returns, and none of it is retained by
//! the resolved program, graph or artifact. Under an active builder it keeps
//! the exact ordinary `ActiveBuilder` payer and every per-allocation charge,
//! but those charges are debited from a fresh detached ledger instead of being
//! settled cumulatively into the enclosing builder ledger. The ledger is
//! independent of the enclosing ledger's remaining or nested limit, since a
//! stage budget is sized for its own retained output.
//!
//! The charges count allocation requests, not live bytes: carriers freed
//! inside one member stay debited (3,635,613,686 bytes for the unfactored
//! ShiftSim schedule). The ledger therefore refuses only a runaway counter;
//! replay itself stays bounded by `MAX_REPLAY_WORK_UNITS`. Exhaustion is the
//! existing `SPX-H006` refusal. Raw public HIR validation selects a coupled
//! child of its caller's ledger, preserving the same scratch ceiling and
//! caller floor; explicit staged workspace validation retains the detached
//! owner. Without an active ledger replay stays unobserved, and an already
//! refused enclosing ledger keeps its route.
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

/// Per-program cleanup-replay charge ceiling (4 GiB on 64-bit hosts).
pub(in crate::cleanup_plan) const MAX_REPLAY_SCRATCH_BYTES: usize = if usize::BITS >= 64 {
    4 << 30
} else {
    usize::MAX
};

/// Complete independent replay of a resolved program on the detached ledger.
pub(in crate::cleanup_plan) fn validate_program(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    detached(|| {
        #[cfg(feature = "unstable-workflow-profiling")]
        let _paid_phase = crate::workflow_profile::plan_comparison::active_paid_phase(
            crate::workflow_profile::plan_comparison::PaidPhase::CleanupValidation,
        );
        crate::cleanup_plan::replay::validate_program(program)
    })
}

/// Complete independent replay of a projected context on the detached ledger.
pub(in crate::cleanup_plan) fn validate_program_context(
    program: &dyn crate::cleanup::ReplayContext,
) -> Result<(), Diagnostic> {
    detached(|| crate::cleanup_plan::replay::validate_program_context(program))
}

fn detached(replay: impl FnOnce() -> Result<(), Diagnostic>) -> Result<(), Diagnostic> {
    if crate::bounded_output::active_limit().is_none() || crate::bounded_output::active_refused() {
        return replay();
    }
    let (result, refused, _) = if COUPLED.with(std::cell::Cell::get) {
        // A raw HIR caller owns this verification too. Keep both its trailer
        // floor and the original per-program scratch ceiling. Settle the
        // actual debit and propagate the actual required allocator refusal.
        let allowance = crate::bounded_output::active_usable_remaining().unwrap_or(0);
        let (result, refused, consumed, receipt) = crate::bounded_output::with_limit_usage_refusal(
            MAX_REPLAY_SCRATCH_BYTES.min(allowance),
            replay,
        );
        if refused {
            crate::bounded_output::refuse_active_required(receipt.map_or(0, |r| r.requested));
        }
        (result, refused, consumed)
    } else {
        crate::bounded_output::detached::with_detached_limit(MAX_REPLAY_SCRATCH_BYTES, replay)
    };
    match result {
        Err(error) => Err(error),
        Ok(()) if refused => Err(Diagnostic::io(
            "SPX-H006",
            "cleanup replay scratch exceeds its per-program physical reservation",
        )),
        Ok(()) => Ok(()),
    }
}

// Public raw-HIR validation retains its caller-owned payer contract. The
// workspace's explicit staged entry keeps its independent scratch owner.
thread_local! {
    static COUPLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(crate) fn with_coupled<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            COUPLED.with(|scope| scope.set(self.0));
        }
    }
    let _restore = Restore(COUPLED.with(|scope| scope.replace(true)));
    operation()
}

#[test]
fn coupled_raw_validation_preserves_parent_floor_and_actual_sticky_refusal() {
    let (result, refused, consumed, receipt) =
        crate::bounded_output::with_limit_usage_refusal(10, || {
            assert!(crate::bounded_output::set_active_floor(3));
            with_coupled(|| {
                detached(|| {
                    assert!(crate::bounded_output::reserve_active_required(7));
                    assert!(!crate::bounded_output::reserve_active_required(1));
                    Ok(())
                })
            })
        });
    assert_eq!(result.unwrap_err().code, "SPX-H006");
    assert!(refused);
    assert_eq!(consumed, 7);
    let receipt = receipt.unwrap();
    assert_eq!(receipt.requested, 1);
    assert_eq!(receipt.remaining, 3);
    assert_eq!(receipt.floor, 3);
}
#[test]
fn coupled_raw_validation_unwind_restores_the_staged_independent_owner() {
    let (result, refused, consumed) = crate::bounded_output::with_limit_usage(10, || {
        assert!(std::panic::catch_unwind(|| with_coupled(|| panic!("scope unwind"))).is_err());
        detached(|| {
            assert!(crate::bounded_output::reserve_active_required(20));
            Ok(())
        })
    });
    assert!(result.is_ok());
    assert!(!refused);
    assert_eq!(consumed, 0);
}
