//! Dense, directly prepaid numeric AVL arena. Keys never index an allocation.
//! Published closures are immutable; parent links give ascending iteration with
//! no recursive cursor, auxiliary allocation, or sparse-key directory gaps.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

const NONE: usize = usize::MAX;

#[derive(Debug)]
struct Node {
    key: usize,
    weight: u64,
    left: usize,
    right: usize,
    parent: usize,
    height: u8,
}

#[derive(Debug)]
pub(super) struct OrderedWeights {
    nodes: Vec<Node>,
    root: usize,
}

impl OrderedWeights {
    // The caller pays both initializer and receiving inline owner before new.
    pub(super) const CONTROLS: usize = 2 * size_of::<Self>();
    // This helper's return, its positive payment receiver and reserve_vec's
    // receiver are distinct source constructions, including no-growth inserts.
    // Node is the initializer; the Vec receiving slot/backing is paid separately.
    pub(super) const INSERT_CONTROLS: usize =
        3 * size_of::<Result<(), Diagnostic>>() + size_of::<Node>();
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: NONE,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }
    pub(super) fn get(&self, key: &usize) -> Option<&u64> {
        let mut at = self.root;
        while at != NONE {
            let node = &self.nodes[at];
            if *key == node.key {
                return Some(&node.weight);
            }
            at = if *key < node.key {
                node.left
            } else {
                node.right
            };
        }
        None
    }
    pub(super) fn get_mut(&mut self, key: &usize) -> Option<&mut u64> {
        let mut at = self.root;
        while at != NONE {
            if *key == self.nodes[at].key {
                return Some(&mut self.nodes[at].weight);
            }
            at = if *key < self.nodes[at].key {
                self.nodes[at].left
            } else {
                self.nodes[at].right
            };
        }
        None
    }
    /// The caller establishes absence before requesting the one new node.
    /// All fallible acquisition precedes mutation; failed growth keeps contents.
    pub(super) fn insert_new(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        key: usize,
        weight: u64,
    ) -> Result<(), Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                Self::INSERT_CONTROLS,
                "factored ordered weight insertion controls",
            )?;
        }
        budget.reserve_vec(
            function,
            &mut self.nodes,
            1,
            "factored ordered weight arena",
        )?;
        let mut parent = NONE;
        let mut at = self.root;
        while at != NONE {
            parent = at;
            assert_ne!(key, self.nodes[at].key, "new ordered weight key");
            at = if key < self.nodes[at].key {
                self.nodes[at].left
            } else {
                self.nodes[at].right
            };
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            weight,
            left: NONE,
            right: NONE,
            parent,
            height: 1,
        });
        if parent == NONE {
            self.root = index;
        } else if key < self.nodes[parent].key {
            self.nodes[parent].left = index;
        } else {
            self.nodes[parent].right = index;
        }
        self.rebalance(parent);
        Ok(())
    }
    fn height(&self, at: usize) -> u8 {
        if at == NONE { 0 } else { self.nodes[at].height }
    }
    fn refresh(&mut self, at: usize) {
        // An AVL tree with usize-addressable storage has height below 128 on
        // every admitted host, so u8 height arithmetic cannot wrap.
        self.nodes[at].height = 1 + self
            .height(self.nodes[at].left)
            .max(self.height(self.nodes[at].right));
    }
    fn balance(&self, at: usize) -> i16 {
        i16::from(self.height(self.nodes[at].left)) - i16::from(self.height(self.nodes[at].right))
    }
    fn replace_child(&mut self, parent: usize, old: usize, new: usize) {
        if parent == NONE {
            self.root = new;
        } else if self.nodes[parent].left == old {
            self.nodes[parent].left = new;
        } else {
            assert_eq!(self.nodes[parent].right, old);
            self.nodes[parent].right = new;
        }
        self.nodes[new].parent = parent;
    }
    fn rotate_left(&mut self, at: usize) -> usize {
        let pivot = self.nodes[at].right;
        let middle = self.nodes[pivot].left;
        let parent = self.nodes[at].parent;
        self.replace_child(parent, at, pivot);
        self.nodes[pivot].left = at;
        self.nodes[at].parent = pivot;
        self.nodes[at].right = middle;
        if middle != NONE {
            self.nodes[middle].parent = at;
        }
        self.refresh(at);
        self.refresh(pivot);
        pivot
    }
    fn rotate_right(&mut self, at: usize) -> usize {
        let pivot = self.nodes[at].left;
        let middle = self.nodes[pivot].right;
        let parent = self.nodes[at].parent;
        self.replace_child(parent, at, pivot);
        self.nodes[pivot].right = at;
        self.nodes[at].parent = pivot;
        self.nodes[at].left = middle;
        if middle != NONE {
            self.nodes[middle].parent = at;
        }
        self.refresh(at);
        self.refresh(pivot);
        pivot
    }
    fn rebalance(&mut self, mut at: usize) {
        while at != NONE {
            self.refresh(at);
            if self.balance(at) > 1 {
                let left = self.nodes[at].left;
                if self.balance(left) < 0 {
                    self.rotate_left(left);
                }
                at = self.rotate_right(at);
            } else if self.balance(at) < -1 {
                let right = self.nodes[at].right;
                if self.balance(right) > 0 {
                    self.rotate_right(right);
                }
                at = self.rotate_left(at);
            }
            at = self.nodes[at].parent;
        }
    }
    pub(super) fn iter(&self) -> Iter<'_> {
        let mut next = self.root;
        if next != NONE {
            while self.nodes[next].left != NONE {
                next = self.nodes[next].left;
            }
        }
        Iter { owner: self, next }
    }
}

// Replaces the old BTreeMap borrowed iterator at its existing loop sites.
// Its two-word control has no backing; the numeric arena owns every node.
pub(super) struct Iter<'a> {
    owner: &'a OrderedWeights,
    next: usize,
}
impl<'a> Iterator for Iter<'a> {
    type Item = (&'a usize, &'a u64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == NONE {
            return None;
        }
        let at = self.next;
        let nodes = &self.owner.nodes;
        self.next = nodes[at].right;
        if self.next != NONE {
            while nodes[self.next].left != NONE {
                self.next = nodes[self.next].left;
            }
        } else {
            let mut child = at;
            self.next = nodes[child].parent;
            while self.next != NONE && nodes[self.next].right == child {
                child = self.next;
                self.next = nodes[child].parent;
            }
        }
        Some((&nodes[at].key, &nodes[at].weight))
    }
}
impl PartialEq for OrderedWeights {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for OrderedWeights {}

// Both variants borrow the existing immutable closure or own one paid arena.
// The singleton receiver moves its already-authenticated Rc, never clones a map.
#[derive(Debug)]
pub(super) enum Closed {
    Borrowed(std::rc::Rc<OrderedWeights>),
    Owned(OrderedWeights),
}
impl Closed {
    pub(super) const CONTROLS: usize = 2 * size_of::<Self>() + size_of::<&OrderedWeights>();
    pub(super) fn weights(&self) -> &OrderedWeights {
        match self {
            Self::Borrowed(weights) => weights,
            Self::Owned(weights) => weights,
        }
    }
}

#[cfg(test)]
mod receiver_tests;
#[cfg(test)]
mod source;
#[cfg(test)]
mod tests;

// Compare old/current representations without constructing a projected map in
// the measured region. The complete original assertions are archived/inverted.
#[cfg(test)]
pub(crate) fn assert_closure_result(
    actual: &Result<Option<std::rc::Rc<OrderedWeights>>, Diagnostic>,
    expected: &Result<Option<std::rc::Rc<std::collections::BTreeMap<usize, u64>>>, Diagnostic>,
) {
    match (actual, expected) {
        (Err(actual), Err(expected)) => assert_eq!(actual, expected),
        (Ok(None), Ok(None)) => {}
        (Ok(Some(actual)), Ok(Some(expected))) => {
            assert!(
                actual.iter().eq(expected.iter()),
                "complete numeric closure entries"
            );
        }
        _ => panic!("closure outcome mismatch: actual={actual:?}, expected={expected:?}"),
    }
}
#[cfg(test)]
pub(crate) fn assert_closure_view(
    actual: Option<&std::rc::Rc<OrderedWeights>>,
    expected: Option<&std::rc::Rc<std::collections::BTreeMap<usize, u64>>>,
) {
    match (actual, expected) {
        (None, None) => {}
        (Some(actual), Some(expected)) => assert!(actual.iter().eq(expected.iter())),
        _ => panic!("closure view mismatch: actual={actual:?}, expected={expected:?}"),
    }
}
