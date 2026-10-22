//! One invocation's current outer byte-loop entry; never serialized or reused.
use super::{Binding, Scope};
use crate::diagnostic::Diagnostic;
use std::{collections::BTreeMap, mem::size_of};

pub(super) struct Ancestry<'a> {
    outer: Option<(Scope<'a>, usize)>,
}
fn reserve(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(Diagnostic::io_static(
            "SPX-H006",
            "nested byte lowering ancestry storage refused",
        ))
    }
}
/// The new transient Rc::clone result/EnterByteLoop initializer has one Scope
/// receiving carrier. It is distinct from the retained whole Frame backing and
/// invocation Option carrier. No new heap/control-block allocation is claimed;
/// reservations are cumulative and are never released or credited.
/// Frame COUNT and maximum layout remain the existing actual resolver census;
/// EnterByteLoop replaces EnterLoop, rather than pushing extra continuations.
pub(in crate::hir) fn reserve_scope() -> Result<(), Diagnostic> {
    reserve(size_of::<Scope<'_>>())
}
impl<'a> Ancestry<'a> {
    pub(super) fn acquire() -> Result<Self, Diagnostic> {
        reserve(size_of::<Self>() + size_of::<Result<Self, Diagnostic>>())?;
        Ok(Self { outer: None })
    }
    pub(super) fn enter(&mut self, scope: Scope<'a>, depth: usize) {
        // lexical_exit::enter_loop has already checked/incremented its existing
        // bounded depth before this publication. No new counter/quota exists.
        // The matching pending BlockForBody already retains this exact scope
        // until AFTER LeaveLoop clears us, so its existing deduplicated scope
        // census covers all shared map backing throughout this extra lease.
        if self.outer.is_none() {
            self.outer = Some((scope, depth));
        }
    }
    pub(super) fn leave(&mut self, depth: usize) {
        if self.outer.as_ref().is_some_and(|(_, outer)| depth < *outer) {
            self.outer = None;
        }
    }
    pub(super) fn entry(&self) -> Option<&BTreeMap<String, Binding>> {
        self.outer.as_ref().map(|(scope, _)| &***scope)
    }
}

#[cfg(test)]
mod tests {
    use super::Ancestry;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::hir::{resolver_scope, Binding};
    use crate::test_allocation_observer::measure;
    use std::{collections::BTreeMap, mem::size_of};
    #[test]
    fn nested_byte_ancestry_prepays_controls_and_unwinds_outermost_scope_without_allocating() {
        let map = BTreeMap::<String, Binding>::new();
        let entry = resolver_scope::borrowed(&map);
        let state_bytes = size_of::<Ancestry<'_>>()
            + size_of::<Result<Ancestry<'_>, crate::diagnostic::Diagnostic>>();
        let handle_bytes = size_of::<crate::hir::resolver_scope::Scope<'_>>();
        let expected = state_bytes + 2 * handle_bytes;
        let (((), setup_refused, setup_used, setup_first), setup) =
            measure(|| with_limit_usage_refusal(0, || ()));
        assert!(!setup_refused && setup_used == 0 && setup_first.is_none());
        assert_eq!(setup.allocations, 1);
        assert_eq!(setup.live, 0);
        assert_eq!(setup.allocated, setup.freed);
        assert!(!setup.invalid);
        eprintln!("ancestry external ledger setup/full Drop: {setup:?}; receiver observations follow separately");
        for limit in [
            0,
            state_bytes - 1,
            state_bytes + handle_bytes - 1,
            expected - 1,
            expected,
        ] {
            let operation = || {
                let mut state = Ancestry::acquire()?;
                assert!(state.entry().is_none());
                super::reserve_scope()?;
                state.enter(entry.clone(), 1);
                super::reserve_scope()?;
                state.enter(entry.clone(), 3);
                state.leave(2);
                assert!(std::ptr::eq(state.entry().unwrap(), &map));
                state.leave(1);
                assert!(state.entry().is_some());
                state.leave(0);
                assert!(state.entry().is_none());
                Ok::<_, crate::diagnostic::Diagnostic>(())
            };
            // Retain the complete external setup + compiler + final Drop
            // observation, independently of the already-created-ledger scope.
            let ((whole_result, whole_refused, whole_used, whole_first), whole) =
                measure(|| with_limit_usage_refusal(limit, operation));
            assert_eq!(
                (whole.allocations, whole.allocated, whole.live),
                (setup.allocations, setup.allocated, 0)
            );
            assert_eq!(whole.allocated, whole.freed);
            assert!(!whole.invalid);
            // The zero-heap claim is only the complete compiler receiver inside
            // the real caller-owned invocation ledger, never ledger creation.
            let ((result, metrics), refused, used, first) =
                with_limit_usage_refusal(limit, || measure(operation));
            assert_eq!(
                (&result, refused, used, first),
                (&whole_result, whole_refused, whole_used, whole_first)
            );
            assert_eq!(result.is_ok(), limit == expected);
            assert_eq!(refused, limit < expected);
            assert_eq!(first.is_some(), limit < expected);
            let acquired = if limit < state_bytes {
                0
            } else if limit < state_bytes + handle_bytes {
                state_bytes
            } else if limit < expected {
                state_bytes + handle_bytes
            } else {
                expected
            };
            assert_eq!(used, acquired);
            assert_eq!(
                (metrics.allocations, metrics.allocated, metrics.live),
                (0, 0, 0)
            );
        }
        let ((_, metrics), refused, _, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(Ancestry::acquire().is_err());
                assert!(super::reserve_scope().is_err());
                assert!(Ancestry::acquire().is_err());
            })
        });
        assert!(refused && first.is_some());
        assert_eq!(
            (metrics.allocations, metrics.allocated, metrics.live),
            (0, 0, 0)
        );
        // Panic machinery is not a producer allocation: unwind is checked
        // separately from the complete acquisition/Drop observations above.
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut state = Ancestry::acquire().unwrap();
            state.enter(entry.clone(), 1);
            panic!("unwind byte ancestry");
        }));
        assert!(unwind.is_err());
        assert_eq!(std::rc::Rc::strong_count(&entry), 1);
    }
}
