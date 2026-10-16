//! Comparison-owned ordered arenas. Keys and insertion ordinals never move
//! during AVL rotations; every fallible capacity acquisition precedes insertion.
use super::{Label, SharedWeighted};
use std::cmp::Ordering;
use std::mem::size_of;

const NONE: usize = usize::MAX;
pub(super) type Signature = (u64, Vec<(Label, usize)>);

#[derive(Debug)]
pub(super) struct Node<K> {
    key: K,
    value: usize,
    left: usize,
    right: usize,
    parent: usize,
    height: usize,
}

#[derive(Debug)]
pub(super) struct Arena<K> {
    pub(super) nodes: Vec<Node<K>>,
    root: usize,
}

pub(super) trait Query<K> {
    fn compare(&self, stored: &K) -> Ordering;
}
impl<K: Ord> Query<K> for K {
    fn compare(&self, stored: &K) -> Ordering {
        self.cmp(stored)
    }
}

// The child IDs were all resolved, in original order, before this view exists.
// Its two borrowed slices are not a cached signature or source authority.
pub(super) struct SignatureQuery<'a> {
    pub(super) accepted: u64,
    pub(super) successors: &'a [(Label, SharedWeighted)],
    pub(super) children: &'a [usize],
}
impl SignatureQuery<'_> {
    pub(super) const CONTROLS: usize = 2 * size_of::<Self>()
        + 2 * size_of::<&Vec<(Label, SharedWeighted)>>()
        + 2 * size_of::<&Vec<usize>>()
        + size_of::<&[(Label, SharedWeighted)]>()
        + size_of::<&[usize]>();
    // Additional per-exit producers: this positive admission, retained scratch
    // growth, and child scalar admission. The original signature reserve_array
    // still exists on misses; no old Result is credited as a reused grant.
    pub(super) const EXIT_CONTROLS: usize = Self::CONTROLS
        + 3 * size_of::<Result<(), super::Diagnostic>>()
        + 2 * size_of::<std::slice::Iter<'static, (Label, SharedWeighted)>>()
        + 2 * size_of::<&Vec<(Label, SharedWeighted)>>()
        + size_of::<&[(Label, SharedWeighted)]>();
}
impl Query<Signature> for SignatureQuery<'_> {
    fn compare(&self, stored: &Signature) -> Ordering {
        let order = self.accepted.cmp(&stored.0);
        if order != Ordering::Equal {
            return order;
        }
        for index in 0..self.successors.len().min(stored.1.len()) {
            let order = self.successors[index].0.cmp(&stored.1[index].0);
            if order != Ordering::Equal {
                return order;
            }
            let order = self.children[index].cmp(&stored.1[index].1);
            if order != Ordering::Equal {
                return order;
            }
        }
        self.successors.len().cmp(&stored.1.len())
    }
}

impl<K: Ord> Arena<K> {
    // Every new Node is initialized once and received by Vec::push once.
    // Capacity is separately paid in full by reserve_vec, including overlap.
    pub(super) const INSERT_CONTROLS: usize = 2 * size_of::<Node<K>>()
        + size_of::<Result<(), super::Diagnostic>>()
        + size_of::<&mut super::ReplayBudget>()
        + size_of::<&super::ResolvedFunction>()
        + size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<&mut Vec<Node<K>>>()
        + 2 * size_of::<bool>();
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: NONE,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    #[cfg(test)]
    pub(super) fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }
    pub(super) fn get<Q: Query<K>>(&self, query: &Q) -> Option<&usize> {
        let mut at = self.root;
        while at != NONE {
            match query.compare(&self.nodes[at].key) {
                Ordering::Equal => return Some(&self.nodes[at].value),
                Ordering::Less => at = self.nodes[at].left,
                Ordering::Greater => at = self.nodes[at].right,
            }
        }
        None
    }
    pub(super) fn contains_key(&self, key: &K) -> bool {
        let mut at = self.root;
        while at != NONE {
            match key.cmp(&self.nodes[at].key) {
                Ordering::Equal => return true,
                Ordering::Less => at = self.nodes[at].left,
                Ordering::Greater => at = self.nodes[at].right,
            }
        }
        false
    }
    // The caller has already prepaid the actual Node, receiving controls and
    // spare backing before moving the key. No allocation/failure is possible.
    pub(super) fn insert_prepaid(&mut self, key: K, value: usize) {
        let mut at = self.root;
        let mut parent = NONE;
        let mut left = false;
        while at != NONE {
            parent = at;
            match key.cmp(&self.nodes[at].key) {
                Ordering::Equal => {
                    self.nodes[at].value = value;
                    return;
                }
                Ordering::Less => {
                    left = true;
                    at = self.nodes[at].left;
                }
                Ordering::Greater => {
                    left = false;
                    at = self.nodes[at].right;
                }
            }
        }
        assert!(
            self.nodes.len() < self.nodes.capacity(),
            "prepaid canonical arena slot"
        );
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            value,
            left: NONE,
            right: NONE,
            parent,
            height: 1,
        });
        if parent == NONE {
            self.root = index;
        } else if left {
            self.nodes[parent].left = index;
        } else {
            self.nodes[parent].right = index;
        }
        self.rebalance(parent);
    }
    fn height(&self, at: usize) -> usize {
        if at == NONE {
            0
        } else {
            self.nodes[at].height
        }
    }
    fn refresh(&mut self, at: usize) {
        self.nodes[at].height = 1 + self
            .height(self.nodes[at].left)
            .max(self.height(self.nodes[at].right));
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
            let left = self.nodes[at].left;
            let right = self.nodes[at].right;
            if self.height(left) > self.height(right) + 1 {
                if self.height(self.nodes[left].right) > self.height(self.nodes[left].left) {
                    self.rotate_left(left);
                }
                at = self.rotate_right(at);
            } else if self.height(right) > self.height(left) + 1 {
                if self.height(self.nodes[right].left) > self.height(self.nodes[right].right) {
                    self.rotate_right(right);
                }
                at = self.rotate_left(at);
            }
            at = self.nodes[at].parent;
        }
    }
    #[cfg(test)]
    pub(super) fn iter(&self) -> Iter<'_, K> {
        let mut next = self.root;
        if next != NONE {
            while self.nodes[next].left != NONE {
                next = self.nodes[next].left;
            }
        }
        Iter { owner: self, next }
    }
}

#[cfg(test)]
pub(super) struct Iter<'a, K> {
    owner: &'a Arena<K>,
    next: usize,
}
#[cfg(test)]
impl<'a, K> Iterator for Iter<'a, K> {
    type Item = (&'a K, &'a usize);
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
        Some((&nodes[at].key, &nodes[at].value))
    }
}
#[cfg(test)]
impl<K: Ord> PartialEq<std::collections::BTreeMap<K, usize>> for Arena<K> {
    fn eq(&self, other: &std::collections::BTreeMap<K, usize>) -> bool {
        self.iter().eq(other.iter())
    }
}
#[cfg(test)]
impl PartialEq<Arena<Signature>> for std::collections::BTreeMap<Signature, usize> {
    fn eq(&self, other: &Arena<Signature>) -> bool {
        self.iter().eq(other.iter())
    }
}

// The caller's existing unit destination receives every nested Result. The
// former BTree node is actually absent; no fallible rotation follows this bank.
macro_rules! prepare_insert {
    ($arena:expr, $key:ty, $function:expr, $budget:expr) => {{
        attempt!($budget.reserve_bytes($function, crate::cleanup_plan::replay::factored::canonical_shared::intern_arena::Arena::<$key>::INSERT_CONTROLS, "canonical arena node receiving"));
        attempt!($budget.reserve_vec($function, &mut $arena.nodes, 1, "canonical ordered arena backing"));
        if !$budget.pays_allocations() { $arena.nodes.reserve(1); }
    }};
}
pub(super) use prepare_insert;
