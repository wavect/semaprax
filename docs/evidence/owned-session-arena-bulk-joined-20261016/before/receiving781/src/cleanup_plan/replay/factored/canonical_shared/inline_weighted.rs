//! Persistent weighted state content is independent of its One/Many storage.
use super::Weighted;
use std::cmp::Ordering;
use std::ops::Deref;

#[derive(Debug)]
pub(super) enum SmallWeighted {
    One([(usize, u64); 1]),
    Many(Weighted),
}
impl SmallWeighted {
    pub(super) fn as_slice(&self) -> &[(usize, u64)] {
        match self { Self::One(item) => item, Self::Many(items) => items.as_slice() }
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        match self { Self::One(_) => 1, Self::Many(items) => items.capacity() }
    }
}
impl Deref for SmallWeighted {
    type Target = [(usize, u64)];
    fn deref(&self) -> &Self::Target { self.as_slice() }
}
impl PartialEq for SmallWeighted {
    fn eq(&self, other: &Self) -> bool { self.as_slice() == other.as_slice() }
}
impl Eq for SmallWeighted {}
impl PartialOrd for SmallWeighted {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Ord for SmallWeighted {
    fn cmp(&self, other: &Self) -> Ordering { self.as_slice().cmp(other.as_slice()) }
}

#[cfg(test)]
pub(super) fn assert_states(
    current: &super::BTreeMap<super::SharedWeighted, usize>,
    old: &super::BTreeMap<super::Rc<Weighted>, usize>,
) {
    assert_eq!(current.len(), old.len());
    for ((actual, id), (expected, old_id)) in current.iter().zip(old) {
        assert_eq!(actual.as_slice(), expected.as_slice());
        assert_eq!(id, old_id);
    }
}
#[cfg(test)]
#[allow(clippy::type_complexity)]
pub(super) fn assert_successors(
    current: &Result<Option<(u64, Vec<(super::Label, super::SharedWeighted)>)>, super::Diagnostic>,
    old: &Result<Option<(u64, Vec<(super::Label, super::Rc<Weighted>)>)>, super::Diagnostic>,
) {
    match (current, old) {
        (Ok(Some((accepted, targets))), Ok(Some((old_accepted, old_targets)))) => {
            assert_eq!(accepted, old_accepted);
            assert_eq!(targets.len(), old_targets.len());
            for ((label, state), (old_label, old_state)) in targets.iter().zip(old_targets) {
                assert_eq!(label, old_label);
                assert_eq!(state.as_slice(), old_state.as_slice());
            }
        }
        (Ok(None), Ok(None)) => {}
        (Err(actual), Err(expected)) => assert_eq!(actual, expected),
        _ => panic!("complete successor verdict differs"),
    }
}
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod source;
