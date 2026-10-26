//! Keep the original owned candidate builder; retain its accepted paths in
//! place instead of collecting a second tree. Authored, not runtime-qualified.
use super::*;
use std::mem::size_of;
type Set = BTreeSet<Vec<PlaceProjection>>;
// Fixed source-level std ABI at the existing positive join CONTROL frontier:
// retain receiver actual/formal, actual/formal two-loan closure environment,
// complete returned/receiving owner, plus the unit return/receiving pair.
// Predicate argument, returning bool and short-circuit are EXACTLY the old
// filter callback, not a new per-prefix operation or a credited old carrier.
pub(super) const CONTROL: usize = 2 * size_of::<&mut Set>()
    + 2 * size_of::<(&ValidationBinding, &ValidationBinding)>()
    + 2 * size_of::<Set>() + 2 * size_of::<()>();
impl HirValidator<'_> {
    pub(super) fn join_definitely_partial_retained(
        left: &ValidationBinding,
        right: &ValidationBinding,
    ) -> BTreeSet<Vec<PlaceProjection>> {
        let mut candidates = BTreeSet::new();
        for path in left
            .moved_places
            .keys()
            .chain(right.moved_places.keys())
            .chain(left.definitely_partial.iter())
            .chain(right.definitely_partial.iter())
        {
            for length in 0..=path.len() {
                candidates.insert(path[..length].to_vec());
            }
        }
        candidates.retain(move |path| {
            Self::place_availability(left, path) == Availability::Moved
                && Self::place_availability(right, path) == Availability::Moved
        });
        candidates
    }
}
#[cfg(test)]
pub(super) mod before;
#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn before_selected() -> bool { BEFORE.with(std::cell::Cell::get) }
#[cfg(test)]
pub(super) fn with_before<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) { BEFORE.with(|v| v.set(self.0)); }
    }
    let restore = Restore(BEFORE.with(|v| v.replace(true)));
    let result = body();
    drop(restore);
    result
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) use tests::whole_merge_before;
