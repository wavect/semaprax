//! Finish a source acquisition on its original canonical payer after restoring
//! the caller's ambient context. Replay keeps its original payer and priority;
//! only the explicitly supplied reservation callback uses the source payer.
use super::*;
pub(crate) fn source_owner_scope<T, U>(
    limit: usize,
    format: impl FnOnce() -> T,
    receive: impl FnOnce(T, bool, &mut dyn FnMut(usize) -> bool) -> U,
) -> (U, bool, usize) {
    struct Restore(Option<Rc<Budget>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|active| *active.borrow_mut() = self.0.take());
        }
    }
    let budget = Rc::new(Budget {
        initial: limit,
        remaining: Cell::new(limit),
        floor: Cell::new(0),
        overflowed: Cell::new(false),
        first_refusal: Cell::new(None),
        stage: Cell::new(None),
    });
    let previous = ACTIVE.with(|active| active.borrow_mut().replace(Rc::clone(&budget)));
    let restore = Restore(previous);
    let value = format();
    let formatting_refused = budget.overflowed.get();
    drop(restore);
    let mut reserve_source = |bytes| reserve(Some(&budget), bytes);
    let value = receive(value, formatting_refused, &mut reserve_source);
    (
        value,
        budget.overflowed.get(),
        limit.saturating_sub(budget.remaining.get()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_owner_callback_restores_ambient_and_reports_complete_real_debits() {
        let ((result, refused, used), outer_refused, outer_used) = with_limit_usage(0, || {
            source_owner_scope(
                16,
                || reserve_active_required(4),
                |formatted, formatting_refused, payer| {
                    assert!(formatted && !formatting_refused);
                    assert_eq!(active_remaining(), Some(0));
                    assert!(reserve_active_required(0));
                    assert!(payer(12));
                    assert!(payer(0));
                    true
                },
            )
        });
        assert!(result && !refused && !outer_refused);
        assert_eq!((used, outer_used), (16, 0));
    }
    #[test]
    fn source_owner_refusal_is_first_sticky_and_has_no_payer_substitution() {
        let (_, refused, used) = source_owner_scope(
            7,
            || reserve_active_required(2),
            |formatted, old_refusal, payer| {
                assert!(formatted && !old_refusal);
                assert!(!payer(6));
                assert!(!payer(0));
            },
        );
        assert!(refused);
        assert_eq!(used, 2);
    }
}
