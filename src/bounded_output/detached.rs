//! A separate root ledger for a bounded sub-computation whose scratch is
//! accounted by its own owner rather than settled into the enclosing ledger.
//!
//! The enclosing budget, its remaining bytes, floor, stage and first refusal
//! are neither read for admission nor changed: the detached ledger is not a
//! child, so its consumption is never transferred to the enclosing ledger and
//! nothing is refunded to it. The previous active ledger is restored on exit,
//! including unwind. Callers must fail closed on a refused detached ledger.
use super::{Budget, ACTIVE};
use std::cell::Cell;
use std::rc::Rc;

/// Run `operation` against a fresh root ledger of exactly `limit` bytes.
/// Returns its value, whether the detached ledger refused, and its debit.
pub(crate) fn with_detached_limit<T>(
    limit: usize,
    operation: impl FnOnce() -> T,
) -> (T, bool, usize) {
    struct Restore(Option<Rc<Budget>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            ACTIVE.with(|active| {
                active.replace(previous);
            });
            #[cfg(feature = "unstable-workflow-profiling")]
            crate::workflow_profile::plan_comparison::observe_ledger_transition();
        }
    }
    let budget = Rc::new(Budget {
        initial: limit,
        remaining: Cell::new(limit),
        floor: Cell::new(0),
        overflowed: Cell::new(false),
        first_refusal: Cell::new(None),
        first_site: Cell::new(None),
        stage: Cell::new(None),
    });
    let restore = Restore(ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget)))));
    #[cfg(feature = "unstable-workflow-profiling")]
    crate::workflow_profile::plan_comparison::observe_ledger_transition();
    let value = operation();
    let overflowed = budget.overflowed.get();
    let consumed = limit.saturating_sub(budget.remaining.get());
    drop(restore);
    #[cfg(test)]
    DETACHED_DEBITS.with(|total| total.set(total.get().saturating_add(consumed)));
    (value, overflowed, consumed)
}

#[cfg(test)]
thread_local! {
    // Test-only receipt sum; it grants nothing and is never read for admission.
    static DETACHED_DEBITS: Cell<usize> = const { Cell::new(0) };
}

/// Test-only running sum of every completed detached ledger's debit on this
/// thread, so an observation can add detached scratch to its enclosing debits.
#[cfg(test)]
pub(crate) fn detached_debits() -> usize {
    DETACHED_DEBITS.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::super::{
        active_limit, active_remaining, reserve_active, reserve_active_required, with_limit_usage,
    };
    use super::with_detached_limit;

    #[test]
    fn detached_debit_never_settles_into_or_reads_the_enclosing_ledger() {
        let ((), refused, used) = with_limit_usage(100, || {
            assert!(reserve_active(10));
            let ((), detached_refused, detached_used) = with_detached_limit(1_000, || {
                assert_eq!(active_limit(), Some(1_000));
                // The detached ledger admits more than the enclosing remainder.
                assert!(reserve_active_required(500));
            });
            assert!(!detached_refused);
            assert_eq!(detached_used, 500);
            assert_eq!(active_remaining(), Some(90));
            assert!(reserve_active(5));
        });
        assert!(!refused);
        assert_eq!(used, 15);
    }

    #[test]
    fn detached_refusal_is_reported_and_leaves_the_enclosing_ledger_unrefused() {
        let ((), refused, used) = with_limit_usage(100, || {
            let ((), detached_refused, detached_used) = with_detached_limit(8, || {
                assert!(reserve_active_required(8));
                assert!(!reserve_active_required(1));
                // Sticky within the detached ledger only.
                assert!(!reserve_active_required(0));
            });
            assert!(detached_refused);
            assert_eq!(detached_used, 8);
            assert!(reserve_active_required(0));
        });
        assert!(!refused);
        assert_eq!(used, 0);
    }

    #[test]
    fn detached_ledger_restores_the_enclosing_ledger_on_unwind() {
        let ((), refused, used) = with_limit_usage(100, || {
            let unwound = std::panic::catch_unwind(|| {
                with_detached_limit(8, || panic!("detached operation panicked"))
            });
            assert!(unwound.is_err());
            assert_eq!(active_limit(), Some(100));
            assert!(reserve_active(3));
        });
        assert!(!refused);
        assert_eq!(used, 3);
    }
}
