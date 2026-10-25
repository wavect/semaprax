//! Borrow existing ordered keys; construct only the retained result map.
use super::*;
type Cursor<'a> = std::collections::btree_map::Iter<'a, Vec<PlaceProjection>, Availability>;
type Head<'a> = Option<(&'a Vec<PlaceProjection>, &'a Availability)>;
// Includes the new gateway checked count/product/Option path before arithmetic,
// full helper call/result, both iterators/terminal next/heads and result owner.
pub(super) const CONTROL: usize = 4 * std::mem::size_of::<Cursor<'_>>()
    + 8 * std::mem::size_of::<Head<'_>>()
    + 2 * std::mem::size_of::<(&ValidationBinding, &ValidationBinding)>()
    + 4 * std::mem::size_of::<BTreeMap<Vec<PlaceProjection>, Availability>>()
    + std::mem::size_of::<[usize; 8]>()
    + 4 * std::mem::size_of::<Option<usize>>()
    + 4 * std::mem::size_of::<Result<(), Diagnostic>>();
// Per original input key: every new next receiver/result, path/state loan,
// cmp and branch, output clone/insert actual+formal transport. Final output
// backing and path payload are separately retained in the old scratch map.
pub(super) const ROW: usize = 2 * std::mem::size_of::<Head<'_>>()
    + 2 * std::mem::size_of::<(&Head<'_>, &Head<'_>)>()
    + 2 * std::mem::size_of::<&mut Cursor<'_>>()
    + 3 * std::mem::size_of::<(&Vec<PlaceProjection>, Availability)>()
    + 2 * std::mem::size_of::<(&&Vec<PlaceProjection>, &&Vec<PlaceProjection>)>()
    + 2 * std::mem::size_of::<(&Vec<PlaceProjection>, Vec<PlaceProjection>)>()
    + 4 * std::mem::size_of::<Availability>()
    + 2 * std::mem::size_of::<std::cmp::Ordering>()
    + 8 * std::mem::size_of::<bool>()
    + 2 * std::mem::size_of::<(
        &mut BTreeMap<Vec<PlaceProjection>, Availability>,
        Vec<PlaceProjection>,
        Availability,
    )>()
    + 2 * std::mem::size_of::<Option<Availability>>();
// The sole owning callers prepay CONTROL+ROW*(left.len+right.len) and ALL
// original final-map/path-prefix/loan/COW backing before reaching this body.
pub(super) fn join(
    left: &ValidationBinding,
    right: &ValidationBinding,
) -> BTreeMap<Vec<PlaceProjection>, Availability> {
    let mut left = left.moved_places.iter();
    let mut right = right.moved_places.iter();
    let mut a = left.next();
    let mut b = right.next();
    let mut result = BTreeMap::new();
    loop {
        let (path, state) = match (&a, &b) {
            (Some((ka, va)), Some((kb, vb))) => match ka.cmp(kb) {
                std::cmp::Ordering::Less => {
                    let output = (*ka, (**va).join(Availability::Available));
                    a = left.next();
                    output
                }
                std::cmp::Ordering::Greater => {
                    let output = (*kb, Availability::Available.join(**vb));
                    b = right.next();
                    output
                }
                std::cmp::Ordering::Equal => {
                    let output = (*ka, (**va).join(**vb));
                    a = left.next();
                    b = right.next();
                    output
                }
            },
            (Some((ka, va)), None) => {
                let output = (*ka, (**va).join(Availability::Available));
                a = left.next();
                output
            }
            (None, Some((kb, vb))) => {
                let output = (*kb, Availability::Available.join(**vb));
                b = right.next();
                output
            }
            (None, None) => break,
        };
        if state != Availability::Available {
            result.insert(path.clone(), state);
        }
    }
    result
}
#[cfg(test)]
pub(super) mod before;
#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn before_selected() -> bool {
    BEFORE.with(std::cell::Cell::get)
}
#[cfg(test)]
fn with_before<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|v| v.set(self.0));
        }
    }
    let restore = Restore(BEFORE.with(|v| v.replace(true)));
    let result = body();
    drop(restore);
    result
}
#[cfg(test)]
mod tests;
