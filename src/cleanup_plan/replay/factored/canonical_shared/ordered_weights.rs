//! Dense, directly prepaid numeric AVL arena. Keys never index an allocation.
//! Published closures are immutable; parent links give ascending iteration with
//! no recursive cursor, auxiliary allocation, or sparse-key directory gaps.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use std::mem::size_of;

type Link = u32;
const NONE: Link = Link::MAX;
// Each closure entry except one optional own/sink entry follows a successful
// original route debit. Closed entries all follow a weighted-route debit.
// Geometric spare slots are bounded independently of the numeric key range.
const MAX_KEYS: usize = super::super::super::MAX_REPLAY_WORK_UNITS + 1;
const MAX_SLOTS: usize = MAX_KEYS.next_power_of_two();
const _: () = assert!(MAX_SLOTS < Link::MAX as usize);

#[derive(Debug)]
struct Node {
    key: usize,
    weight: u64,
    left: Link,
    right: Link,
    parent: Link,
    height: u8,
}

#[derive(Debug)]
pub(super) struct OrderedWeights {
    // All nodes.len() slots have real, already-paid empty Node values.
    // Only the prefix ..used participates in the AVL tree.
    nodes: Vec<Node>,
    root: Link,
    used: usize,
    first_key: usize,
    first_weight: u64,
}

impl OrderedWeights {
    // Both real inline owner constructions include the scalar singleton slot.
    pub(super) const CONTROLS: usize = 2 * size_of::<Self>();
    // Only growth invokes this Result-returning helper: return, positive payment,
    // reserve_vec and initializer payment Results. Every ordinary spare-slot
    // insert returns unit. Bounded integer capacity arithmetic makes no Option.
    pub(super) const GROW_CONTROLS: usize = 4 * size_of::<Result<(), Diagnostic>>();
    pub(super) fn new() -> Self {
        Self {
            nodes: Vec::new(),
            root: NONE,
            used: 0,
            first_key: 0,
            first_weight: 0,
        }
    }
    pub(super) fn len(&self) -> usize {
        self.used
    }
    pub(super) fn needs_capacity(&self) -> bool {
        self.used != 0 && (self.nodes.is_empty() || self.used == self.nodes.len())
    }
    pub(super) fn grow(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                Self::GROW_CONTROLS,
                "factored ordered arena growth controls",
            )?;
        }
        assert!(
            self.needs_capacity(),
            "growth only constructs a necessary arena"
        );
        // Bounds are checked before arithmetic, allocation or AVL mutation.
        // MAX_KEYS+1 is representable and its next power of two <= MAX_SLOTS.
        // Every later used->Link conversion is also bounded by initialized len.
        if self.used >= MAX_KEYS {
            return Err(budget.budget_refusal(function, "ordered arena index bound exceeded", ""));
        }
        let required = self.used + 1;
        let target = required.next_power_of_two();
        assert!(target <= MAX_SLOTS);
        let inline = self.nodes.is_empty();
        let additional = required - self.nodes.len();
        budget.reserve_vec(
            function,
            &mut self.nodes,
            additional,
            "factored ordered weight arena",
        )?;
        // Legacy-unobserved reserve_vec intentionally allocates nothing. This
        // explicit growth uses the same deterministic target on that route.
        if !budget.pays_allocations() {
            self.nodes.reserve_exact(target - self.nodes.len());
        }
        assert_eq!(
            self.nodes.capacity(),
            target,
            "exact prepaid arena capacity"
        );
        let additional = target - self.nodes.len();
        budget.reserve_array::<Node>(
            function,
            additional,
            "factored ordered arena empty initializers",
        )?;
        self.nodes.resize_with(target, || Node {
            key: 0,
            weight: 0,
            left: NONE,
            right: NONE,
            parent: NONE,
            height: 0,
        });
        if inline {
            self.nodes[0].key = self.first_key;
            self.nodes[0].weight = self.first_weight;
            self.nodes[0].height = 1;
            self.root = 0;
        }
        Ok(())
    }
    pub(super) fn get(&self, key: &usize) -> Option<&u64> {
        if self.used == 1 && self.nodes.is_empty() {
            return if self.first_key == *key {
                Some(&self.first_weight)
            } else {
                None
            };
        }
        let mut at = self.root;
        while at != NONE {
            let node = &self.nodes[at as usize];
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
        if self.used == 1 && self.nodes.is_empty() {
            return if self.first_key == *key {
                Some(&mut self.first_weight)
            } else {
                None
            };
        }
        let mut at = self.root;
        while at != NONE {
            if *key == self.nodes[at as usize].key {
                return Some(&mut self.nodes[at as usize].weight);
            }
            at = if *key < self.nodes[at as usize].key {
                self.nodes[at as usize].left
            } else {
                self.nodes[at as usize].right
            };
        }
        None
    }
    /// The caller establishes absence and grows only if needs_capacity().
    /// This success path constructs no Result, Node, vector or heap carrier.
    /// It mutates the owner's paid inline slot or a real preinitialized node.
    pub(super) fn insert_prepaid(&mut self, key: usize, weight: u64) {
        if self.used == 0 {
            self.first_key = key;
            self.first_weight = weight;
            self.used = 1;
            return;
        }
        assert!(!self.nodes.is_empty() && self.used < self.nodes.len());
        assert!(self.nodes.len() <= MAX_SLOTS);
        let mut parent = NONE;
        let mut at = self.root;
        while at != NONE {
            parent = at;
            assert_ne!(key, self.nodes[at as usize].key, "new ordered weight key");
            at = if key < self.nodes[at as usize].key {
                self.nodes[at as usize].left
            } else {
                self.nodes[at as usize].right
            };
        }
        // used < initialized len <= MAX_SLOTS < u32::MAX, independently of
        // numeric key magnitude. The conversion has no fallible carrier.
        let index = self.used as Link;
        let slot = &mut self.nodes[self.used];
        assert_eq!(slot.height, 0, "an unused initialized slot");
        slot.key = key;
        slot.weight = weight;
        slot.parent = parent;
        slot.height = 1;
        self.used += 1;
        if parent == NONE {
            self.root = index;
        } else if key < self.nodes[parent as usize].key {
            self.nodes[parent as usize].left = index;
        } else {
            self.nodes[parent as usize].right = index;
        }
        self.rebalance(parent);
    }
    fn height(&self, at: Link) -> u8 {
        if at == NONE {
            0
        } else {
            self.nodes[at as usize].height
        }
    }
    fn refresh(&mut self, at: Link) {
        // An AVL tree with usize-addressable storage has height below 128 on
        // every admitted host, so u8 height arithmetic cannot wrap.
        self.nodes[at as usize].height = 1 + self
            .height(self.nodes[at as usize].left)
            .max(self.height(self.nodes[at as usize].right));
    }
    fn balance(&self, at: Link) -> i16 {
        i16::from(self.height(self.nodes[at as usize].left))
            - i16::from(self.height(self.nodes[at as usize].right))
    }
    fn replace_child(&mut self, parent: Link, old: Link, new: Link) {
        if parent == NONE {
            self.root = new;
        } else if self.nodes[parent as usize].left == old {
            self.nodes[parent as usize].left = new;
        } else {
            assert_eq!(self.nodes[parent as usize].right, old);
            self.nodes[parent as usize].right = new;
        }
        self.nodes[new as usize].parent = parent;
    }
    fn rotate_left(&mut self, at: Link) -> Link {
        let pivot = self.nodes[at as usize].right;
        let middle = self.nodes[pivot as usize].left;
        let parent = self.nodes[at as usize].parent;
        self.replace_child(parent, at, pivot);
        self.nodes[pivot as usize].left = at;
        self.nodes[at as usize].parent = pivot;
        self.nodes[at as usize].right = middle;
        if middle != NONE {
            self.nodes[middle as usize].parent = at;
        }
        self.refresh(at);
        self.refresh(pivot);
        pivot
    }
    fn rotate_right(&mut self, at: Link) -> Link {
        let pivot = self.nodes[at as usize].left;
        let middle = self.nodes[pivot as usize].right;
        let parent = self.nodes[at as usize].parent;
        self.replace_child(parent, at, pivot);
        self.nodes[pivot as usize].right = at;
        self.nodes[at as usize].parent = pivot;
        self.nodes[at as usize].left = middle;
        if middle != NONE {
            self.nodes[middle as usize].parent = at;
        }
        self.refresh(at);
        self.refresh(pivot);
        pivot
    }
    fn rebalance(&mut self, mut at: Link) {
        while at != NONE {
            self.refresh(at);
            if self.balance(at) > 1 {
                let left = self.nodes[at as usize].left;
                if self.balance(left) < 0 {
                    self.rotate_left(left);
                }
                at = self.rotate_right(at);
            } else if self.balance(at) < -1 {
                let right = self.nodes[at as usize].right;
                if self.balance(right) > 0 {
                    self.rotate_right(right);
                }
                at = self.rotate_left(at);
            }
            at = self.nodes[at as usize].parent;
        }
    }
    pub(super) fn iter(&self) -> Iter<'_> {
        let inline = self.used == 1 && self.nodes.is_empty();
        let mut next = if inline { 0 } else { self.root };
        if !inline && next != NONE {
            while self.nodes[next as usize].left != NONE {
                next = self.nodes[next as usize].left;
            }
        }
        Iter { owner: self, next }
    }
}

// Replaces the old BTreeMap borrowed iterator at its existing loop sites.
// Its two-word control has no backing; the numeric arena owns every node.
pub(super) struct Iter<'a> {
    owner: &'a OrderedWeights,
    next: Link,
}
impl<'a> Iterator for Iter<'a> {
    type Item = (&'a usize, &'a u64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == NONE {
            return None;
        }
        if self.owner.used == 1 && self.owner.nodes.is_empty() {
            self.next = NONE;
            return Some((&self.owner.first_key, &self.owner.first_weight));
        }
        let at = self.next;
        let nodes = &self.owner.nodes;
        self.next = nodes[at as usize].right;
        if self.next != NONE {
            while nodes[self.next as usize].left != NONE {
                self.next = nodes[self.next as usize].left;
            }
        } else {
            let mut child = at;
            self.next = nodes[child as usize].parent;
            while self.next != NONE && nodes[self.next as usize].right == child {
                child = self.next;
                self.next = nodes[child as usize].parent;
            }
        }
        Some((&nodes[at as usize].key, &nodes[at as usize].weight))
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
mod amortized_source;
#[cfg(test)]
mod receiver_tests;
#[cfg(test)]
mod source;
#[cfg(test)]
mod tests;

// Compare old/current representations without constructing a projected map in
// the measured region. The complete original assertions are archived/inverted.
#[cfg(test)]
pub(super) fn assert_closure_result(
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
pub(super) fn assert_closure_view(
    actual: Option<&std::rc::Rc<OrderedWeights>>,
    expected: Option<&std::rc::Rc<std::collections::BTreeMap<usize, u64>>>,
) {
    match (actual, expected) {
        (None, None) => {}
        (Some(actual), Some(expected)) => assert!(actual.iter().eq(expected.iter())),
        _ => panic!("closure view mismatch: actual={actual:?}, expected={expected:?}"),
    }
}
