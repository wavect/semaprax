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

// Historical source contracts receive their exact old image ONLY after the
// actual compiled current root/new children/archives authenticate physically.
#[cfg(test)]
pub(super) fn whole_merge_before(current: &[u8]) -> Vec<u8> {
    use std::io::Read;
    const CURRENT: &str = include_str!("../merge.rs");
    const BEFORE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/7f066dff4bbc040dd5b90645588a907173290f119f05532e73429c12209ce9b8"));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        ("src/hir/validation/scope_checkpoint/merge.rs", CURRENT),
        ("src/hir/validation/scope_checkpoint/merge/partial_retain.rs", include_str!("partial_retain.rs")),
        ("src/hir/validation/scope_checkpoint/merge/partial_retain/before.rs", include_str!("partial_retain/before.rs")),
        ("src/hir/validation/scope_checkpoint/merge/partial_retain/tests.rs", include_str!("partial_retain/tests.rs")),
        ("src/hir/validation/scope_checkpoint/merge/moved_direct/tests.rs", include_str!("moved_direct/tests.rs")),
        ("e/7f066dff4bbc040dd5b90645588a907173290f119f05532e73429c12209ce9b8", BEFORE),
    ];
    let authenticate = || {
        for (path, compiled) in sources {
            assert!(compiled.len() <= 2_097_152);
            let mut physical = Vec::new();
            std::fs::File::open(root.join(path)).unwrap()
                .take((compiled.len() + 1) as u64).read_to_end(&mut physical).unwrap();
            assert_eq!(physical, compiled.as_bytes(), "{path}");
        }
    };
    assert_eq!(current, CURRENT.as_bytes());
    authenticate();
    let edits: &[(usize, usize, &str, &str)] = &[
        (111,131,"mod partial_retain;\n",""),
        (2511,2785,"        // The SAME owned candidate builder still copies every prefix, even\n        // duplicates and rejected paths. Retain reuses that one tree; all\n        // copied path payload and other output/COW fees remain independent.\n        let slots = node.checked_mul(count)?;\n","        // The old candidate builder copies every prefix even when that prefix\n        // is a duplicate. Its filtered output reuses vectors but builds a new\n        // set. Keep both complete tree directories and every prefix payload.\n        let slots = node.checked_mul(count)?.checked_mul(2)?;\n"),
        (3146,3223,"    charge::reserve(Some(moved_direct::CONTROL + partial_retain::CONTROL))?;\n","    charge::reserve(Some(moved_direct::CONTROL))?;\n"),
        (5756,5895,"    if partial_retain::before_selected() {\n        return partial_retain::before::conditional(target, source, ids);\n    }\n    #[cfg(test)]\n",""),
        (6599,6695,"        let definitely_partial = HirValidator::join_definitely_partial_retained(before, after);\n","        let definitely_partial = HirValidator::join_definitely_partial(before, after);\n"),
        (7721,7862,"    if partial_retain::before_selected() {\n        return partial_retain::before::branches(target, left, right, ids);\n    }\n    #[cfg(test)]\n",""),
        (8664,8758,"        let definitely_partial = HirValidator::join_definitely_partial_retained(left, right);\n","        let definitely_partial = HirValidator::join_definitely_partial(left, right);\n"),
    ];
    let mut restored = current.to_vec();
    for &(start, end, present, previous) in edits.iter().rev() {
        assert_eq!(&restored[start..end], present.as_bytes());
        restored.splice(start..end, previous.bytes());
    }
    assert_eq!(restored, BEFORE.as_bytes());
    authenticate();
    restored
}
