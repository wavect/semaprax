//! Opt-in receipts for the unchanged real retry loop; no trace consumes a ledger.
use std::cell::Cell;
use std::fmt::Debug;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
}

pub(in crate::workspace_graph) fn observe<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.with(|enabled| enabled.set(self.0));
        }
    }
    let _restore = Restore(ENABLED.with(|enabled| enabled.replace(true)));
    run()
}

pub(in crate::workspace_graph) fn record(
    mode: u8,
    prebound: usize,
    consumed: usize,
    overflowed: bool,
    refusal: &impl Debug,
    site: &impl Debug,
    errors: &impl Debug,
) {
    TYPED.with(|slot| {
        if let Some(mut trace) = slot.get() {
            if trace.len < trace.rows.len() {
                trace.rows[trace.len] = (mode, prebound, overflowed);
                trace.len += 1;
            } else {
                trace.lost = true;
            }
            slot.set(Some(trace));
        }
    });
    if ENABLED.with(Cell::get) {
        eprintln!("identity-core-attempt mode={mode:?} prebound={prebound} consumed={consumed} overflowed={overflowed} refusal={refusal:?} site={site:?} errors={errors:?}");
    }
}

#[derive(Clone, Copy)]
pub(in crate::workspace_graph) struct Attempts {
    pub rows: [(u8, usize, bool); 8],
    pub len: usize,
    pub lost: bool,
}
thread_local! {
    static TYPED: Cell<Option<Attempts>> = const { Cell::new(None) };
}
pub(in crate::workspace_graph) fn observe_typed<T>(run: impl FnOnce() -> T) -> (T, Attempts) {
    struct Restore(Option<Attempts>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TYPED.with(|trace| trace.set(self.0));
        }
    }
    let mut previous = TYPED.with(Cell::get);
    let nested = previous.is_some();
    if let Some(ref mut outer) = previous {
        outer.lost = true;
    }
    let _restore = Restore(previous);
    TYPED.with(|trace| {
        trace.set(Some(Attempts {
            rows: [(0, 0, false); 8],
            len: 0,
            lost: nested,
        }))
    });
    let result = observe(run);
    let trace = TYPED.with(Cell::get).unwrap();
    (result, trace)
}

#[test]
fn opt_in_preserves_borrowed_result_and_nested_scope_without_default_formatting() {
    assert!(!ENABLED.with(Cell::get));
    let result = vec![3u8, 7, 11];
    let pointer = result.as_ptr();
    let returned = observe(|| {
        assert!(ENABLED.with(Cell::get));
        let returned = observe(|| &result);
        assert!(ENABLED.with(Cell::get));
        returned
    });
    assert_eq!(returned.as_ptr(), pointer);
    assert_eq!(returned, &result);
    assert!(!ENABLED.with(Cell::get));
}

#[test]
fn disabled_receipts_do_not_format_and_enabled_receipts_borrow_all_original_inputs() {
    struct Seen<'a>(&'a Cell<usize>);
    impl Debug for Seen<'_> {
        fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.set(self.0.get() + 1);
            output.write_str("borrowed-original")
        }
    }
    let calls = Cell::new(0);
    let value = Seen(&calls);
    record(2, 100, 150, true, &value, &value, &value);
    assert_eq!(calls.get(), 0);
    observe(|| record(2, 100, 150, true, &value, &value, &value));
    assert_eq!(calls.get(), 3);
    record(5, 80, 120, false, &value, &value, &value);
    assert_eq!(calls.get(), 3);
    assert!(!ENABLED.with(Cell::get));
}

#[test]
fn unwind_restores_each_original_observer_scope() {
    assert!(!ENABLED.with(Cell::get));
    observe(|| {
        let failure = std::panic::catch_unwind(|| observe(|| panic!("inner receipt unwind")));
        assert!(failure.is_err());
        assert!(ENABLED.with(Cell::get));
    });
    assert!(!ENABLED.with(Cell::get));
    let failure = std::panic::catch_unwind(|| observe(|| panic!("outer receipt unwind")));
    assert!(failure.is_err());
    assert!(!ENABLED.with(Cell::get));
}

#[test]
fn typed_attempts_refuse_loss_reentry_and_restore_after_unwind() {
    assert!(TYPED.with(Cell::get).is_none());
    let (_, outer) = observe_typed(|| {
        for _ in 0..9 {
            record(1, 3, 4, true, &(), &(), &());
        }
    });
    assert_eq!(outer.len, 8);
    assert!(outer.lost);
    let (_, outer) = observe_typed(|| {
        let (_, inner) = observe_typed(|| record(2, 5, 6, false, &(), &(), &()));
        assert!(inner.lost);
    });
    assert!(outer.lost);
    let result = std::panic::catch_unwind(|| observe_typed(|| panic!("typed unwind")));
    assert!(result.is_err());
    assert!(TYPED.with(Cell::get).is_none());
    assert!(!ENABLED.with(Cell::get));
}
