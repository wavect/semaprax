//! Test-only immediate predecessor dispatch; no production state or payer.
use std::cell::Cell;
#[path = "compact_receiving/before.rs"]
pub(super) mod before;
thread_local! { static BEFORE: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn before_requested() -> bool {
    BEFORE.with(Cell::get)
}
fn with_before<T>(old: bool, operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|flag| flag.set(self.0));
        }
    }
    let restore = Restore(BEFORE.with(|flag| flag.replace(old)));
    let result = operation();
    drop(restore);
    result
}
#[path = "compact_receiving/tests.rs"]
mod tests;
