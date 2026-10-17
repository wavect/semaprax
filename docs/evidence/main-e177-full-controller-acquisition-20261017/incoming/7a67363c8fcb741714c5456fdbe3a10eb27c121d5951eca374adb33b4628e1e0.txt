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
    mode: impl Debug,
    prebound: usize,
    consumed: usize,
    overflowed: bool,
    refusal: &impl Debug,
    site: &impl Debug,
    errors: &impl Debug,
) {
    if ENABLED.with(Cell::get) {
        eprintln!("identity-core-attempt mode={mode:?} prebound={prebound} consumed={consumed} overflowed={overflowed} refusal={refusal:?} site={site:?} errors={errors:?}");
    }
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
