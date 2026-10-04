//! Internal immutable cons-cell value for the future checked `List<i64>` lane.
//! This module grants no source, backend, ABI, or proof admission on its own.

use std::sync::Arc;

/// The physical carrier has an explicit finite bound. Lean's `List Int`
/// denotes mathematical lists of arbitrary finite length; a proof about that
/// denotation does not establish runtime success beyond this bound.
pub(crate) const MAX_LENGTH: usize = 8_192;

#[derive(Debug)]
struct Node {
    head: i64,
    tail: Option<Arc<Node>>,
    length: usize,
}

/// A persistent algebraic list. Constructors never rewrite an existing node;
/// sharing a tail cannot change either value. Source ownership will still
/// decide whether a caller may retain an alias when this carrier is admitted.
#[derive(Clone, Debug, Default)]
pub(crate) struct ImmutableList {
    root: Option<Arc<Node>>,
}

impl PartialEq for ImmutableList {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for ImmutableList {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ListError {
    LengthLimit,
}

pub(crate) enum ListStep {
    Nil,
    Cons { head: i64, tail: ImmutableList },
}

impl ImmutableList {
    pub(crate) fn nil() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.root.as_ref().map_or(0, |node| node.length)
    }

    /// Stage the length check before the ownership commit. On refusal the
    /// caller receives its unchanged tail, so no failed constructor can
    /// partially publish a new list or lose the old owner.
    pub(crate) fn cons(head: i64, mut tail: Self) -> Result<Self, (ListError, Self)> {
        let Some(length) = tail.len().checked_add(1) else {
            return Err((ListError::LengthLimit, tail));
        };
        if length > MAX_LENGTH {
            return Err((ListError::LengthLimit, tail));
        }
        Ok(Self {
            root: Some(Arc::new(Node {
                head,
                tail: tail.root.take(),
                length,
            })),
        })
    }

    /// Consuming case selection preserves the immutable tail. A shared tail
    /// remains valid even when the original root is dropped.
    pub(crate) fn uncons(mut self) -> ListStep {
        let Some(root) = self.root.take() else {
            return ListStep::Nil;
        };
        ListStep::Cons {
            head: root.head,
            tail: Self {
                root: root.tail.clone(),
            },
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = i64> + '_ {
        let mut current = self.root.as_ref().map(Arc::as_ref);
        std::iter::from_fn(move || {
            let node = current?;
            current = node.tail.as_ref().map(Arc::as_ref);
            Some(node.head)
        })
    }
}

impl Drop for ImmutableList {
    fn drop(&mut self) {
        // A long, uniquely owned spine is released iteratively. A shared tail
        // is left to its remaining owner, without recursive finalization here.
        let mut current = self.root.take();
        while let Some(root) = current {
            let Ok(mut node) = Arc::try_unwrap(root) else {
                break;
            };
            current = node.tail.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ImmutableList as List, ListError, ListStep, MAX_LENGTH};

    #[test]
    fn cons_and_uncons_preserve_shared_tail_and_refuse_before_commit() {
        let tail = List::cons(2, List::nil()).unwrap();
        let retained = tail.clone();
        let list = List::cons(1, tail).unwrap();
        assert_eq!(list.iter().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(retained.iter().collect::<Vec<_>>(), [2]);
        let ListStep::Cons { head, tail } = list.uncons() else {
            panic!("nonempty list became nil")
        };
        assert_eq!(head, 1);
        assert_eq!(tail.iter().collect::<Vec<_>>(), [2]);
        assert_eq!(retained.iter().collect::<Vec<_>>(), [2]);
        assert!(matches!(List::nil().uncons(), ListStep::Nil));

        let mut full = List::nil();
        for item in 0..MAX_LENGTH {
            full = List::cons(item as i64, full).unwrap();
        }
        let (error, unchanged) = List::cons(-1, full).unwrap_err();
        assert_eq!(error, ListError::LengthLimit);
        assert_eq!(unchanged.len(), MAX_LENGTH);
        assert_eq!(unchanged.iter().next(), Some((MAX_LENGTH - 1) as i64));
        // `unchanged` drops a long spine without recursive stack growth.
    }
}
