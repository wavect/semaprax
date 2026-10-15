//! Complete paid ordered key directory for receiving replay. Indices are private
//! construction coordinates; keys are compared in full and never hashed.
use super::*;
use crate::cleanup_plan::ExitTargetId;
use std::cmp::Ordering;

const NIL: usize = usize::MAX;
// Per-step charges mirror the expression-fact arena.
const VISIT_UNITS: usize = 3;
const STEP_UNITS: usize = 2;
const REBALANCE_UNITS: usize = 16;
const ROTATION_UNITS: usize = 20;

/// Work for one full-key comparison: full string `min(len) + 1` plus one unit per
/// structural numeric field. `None` on arithmetic overflow.
pub(super) trait KeyWork: Ord {
    fn comparison_units(&self, other: &Self) -> Option<usize>;
}

impl KeyWork for u32 {
    fn comparison_units(&self, _: &Self) -> Option<usize> {
        Some(1)
    }
}

impl KeyWork for ExpressionId {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        self.as_str().len().min(other.as_str().len()).checked_add(1)
    }
}

impl KeyWork for StorageId {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        let payload = match (self, other) {
            (Self::Value(left), Self::Value(right)) => left
                .as_str()
                .len()
                .min(right.as_str().len())
                .checked_add(1)?,
            (Self::Temporary(left), Self::Temporary(right)) => left.comparison_units(right)?,
            (
                Self::CallArgument {
                    call: left_call,
                    parameter_index: _,
                    value_expression: left_value,
                },
                Self::CallArgument {
                    call: right_call,
                    parameter_index: _,
                    value_expression: right_value,
                },
            ) => left_call
                .comparison_units(right_call)?
                .checked_add(1)?
                .checked_add(left_value.comparison_units(right_value)?)?,
            _ => 0,
        };
        // One variant discriminator; full Ord remains the actual comparator.
        payload.checked_add(1)
    }
}

impl KeyWork for StatusSourceId {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        // The lane is one structural numeric field.
        self.expression
            .comparison_units(&other.expression)?
            .checked_add(1)
    }
}

macro_rules! numeric_key {
    ($($ty:ty),* $(,)?) => { $(impl KeyWork for $ty {
        fn comparison_units(&self, _: &Self) -> Option<usize> { Some(1) }
    })* };
}
numeric_key!(EdgeId, ExitTargetId, LivenessFlagId, StatusCase);

impl<T: KeyWork> KeyWork for &T {
    fn comparison_units(&self, other: &Self) -> Option<usize> {
        T::comparison_units(self, other)
    }
}

struct Node<K> {
    key: K,
    left: usize,
    right: usize,
    parent: usize,
    next: usize,
    height: u8,
}

struct Probe {
    found: bool,
    parent: usize,
    before: usize,
    after: usize,
    side: Ordering,
}

pub(super) struct Directory<K> {
    nodes: Vec<Node<K>>,
    root: usize,
    first: usize,
    phase: &'static str,
}

struct Iter<'a, K> {
    nodes: &'a [Node<K>],
    next: usize,
}

impl<'a, K> Iterator for Iter<'a, K> {
    type Item = &'a K;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.nodes.get(self.next)?;
        self.next = node.next;
        Some(&node.key)
    }
}

fn overflow(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    detail: &'static str,
) -> Diagnostic {
    budget.budget_refusal(function, detail, "")
}

impl<K: KeyWork> Directory<K> {
    pub(super) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        phase: &'static str,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_bytes(function, 0, phase)?;
        budget.charge(function, 0, phase)?;
        Ok(Self {
            nodes: Vec::new(),
            root: NIL,
            first: NIL,
            phase,
        })
    }

    #[cfg(test)]
    pub(super) fn nodes_pointer_for_test(&self) -> *const () {
        self.nodes.as_ptr().cast()
    }

    #[cfg(test)]
    pub(super) fn first_key_for_test(&self) -> Option<&K> {
        self.nodes.get(self.first).map(|node| &node.key)
    }

    #[cfg(test)]
    pub(super) fn node_layout_bytes() -> usize {
        std::mem::size_of::<Node<K>>()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }

    fn authenticate(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.reserve_bytes(function, 0, self.phase)?;
        budget.charge(function, 0, self.phase)
    }

    fn next_paid<'a>(
        &self,
        iter: &mut Iter<'a, K>,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a K>, Diagnostic> {
        if iter.next != NIL {
            budget.charge(function, STEP_UNITS, self.phase)?;
        }
        Ok(iter.next())
    }

    fn iter(&self) -> Iter<'_, K> {
        Iter {
            nodes: &self.nodes,
            next: self.first,
        }
    }

    fn probe(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<Probe, Diagnostic> {
        let mut probe = Probe {
            found: false,
            parent: NIL,
            before: NIL,
            after: NIL,
            side: Ordering::Equal,
        };
        let mut at = self.root;
        while at != NIL {
            let node = &self.nodes[at];
            let units = key
                .comparison_units(&node.key)
                .and_then(|units| units.checked_add(VISIT_UNITS))
                .ok_or_else(|| {
                    overflow(function, budget, "directory key comparison work overflow")
                })?;
            budget.charge(function, units, self.phase)?;
            probe.side = Ord::cmp(key, &node.key);
            if probe.side == Ordering::Equal {
                probe.found = true;
                return Ok(probe);
            }
            probe.parent = at;
            if probe.side == Ordering::Less {
                probe.after = at;
                at = node.left;
            } else {
                probe.before = at;
                at = node.right;
            }
        }
        Ok(probe)
    }

    pub(super) fn contains(
        &self,
        function: &ResolvedFunction,
        key: &K,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        Ok(self.probe(function, key, budget)?.found)
    }

    /// Returns `false` for a duplicate, like `BTreeSet::insert`.
    pub(super) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: K,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        let probe = self.probe(function, &key, budget)?;
        if probe.found {
            return Ok(false);
        }
        self.grow(function, budget)?;
        budget.charge(function, VISIT_UNITS, self.phase)?;
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            left: NIL,
            right: NIL,
            parent: probe.parent,
            next: probe.after,
            height: 1,
        });
        if probe.before == NIL {
            self.first = index;
        } else {
            self.nodes[probe.before].next = index;
        }
        if probe.parent == NIL {
            self.root = index;
        } else if probe.side == Ordering::Less {
            self.nodes[probe.parent].left = index;
        } else {
            self.nodes[probe.parent].right = index;
        }
        self.rebalance(probe.parent, function, budget)?;
        Ok(true)
    }

    // Relocation is metered before any work; `reserve_vec` pays the complete
    // replacement array while the old allocation is still live.
    fn grow(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.nodes.len() == self.nodes.capacity() {
            let units =
                self.nodes.len().checked_add(1).ok_or_else(|| {
                    overflow(function, budget, "directory relocation work overflow")
                })?;
            budget.charge(function, units, self.phase)?;
        }
        budget.reserve_vec(function, &mut self.nodes, 1, self.phase)
    }

    /// Full-key set equality by lockstep ordered walk.
    pub(super) fn same_keys(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        if self.len() != other.len() {
            return Ok(false);
        }
        let (mut left, mut right) = (self.iter(), other.iter());
        loop {
            match (
                self.next_paid(&mut left, function, budget)?,
                other.next_paid(&mut right, function, budget)?,
            ) {
                (Some(a), Some(b)) => {
                    Self::charge_pair(a, b, function, budget, self.phase)?;
                    if Ord::cmp(a, b) != Ordering::Equal {
                        return Ok(false);
                    }
                }
                (None, None) => return Ok(true),
                _ => return Ok(false),
            }
        }
    }

    /// Ordered merge walk; `true` when no full key is shared.
    pub(super) fn is_disjoint(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.authenticate(function, budget)?;
        let (mut left, mut right) = (self.iter(), other.iter());
        let (mut a, mut b) = (
            self.next_paid(&mut left, function, budget)?,
            other.next_paid(&mut right, function, budget)?,
        );
        while let (Some(x), Some(y)) = (a, b) {
            Self::charge_pair(x, y, function, budget, self.phase)?;
            match Ord::cmp(x, y) {
                Ordering::Equal => return Ok(false),
                Ordering::Less => a = self.next_paid(&mut left, function, budget)?,
                Ordering::Greater => b = other.next_paid(&mut right, function, budget)?,
            }
        }
        Ok(true)
    }

    fn charge_pair(
        a: &K,
        b: &K,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        phase: &'static str,
    ) -> Result<(), Diagnostic> {
        let units = a
            .comparison_units(b)
            .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow"))?;
        budget.charge(function, units, phase)
    }

    fn height(&self, at: usize) -> u8 {
        if at == NIL {
            0
        } else {
            self.nodes[at].height
        }
    }

    // Callers meter the enclosing rebalance/rotation step before calling.
    fn update(
        &mut self,
        at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        let height = self
            .height(self.nodes[at].left)
            .max(self.height(self.nodes[at].right))
            .checked_add(1)
            .ok_or_else(|| overflow(function, budget, "directory AVL height overflow"))?;
        self.nodes[at].height = height;
        Ok(())
    }

    fn attach(&mut self, parent: usize, old: usize, next: usize) {
        if parent == NIL {
            self.root = next;
        } else if self.nodes[parent].left == old {
            self.nodes[parent].left = next;
        } else {
            self.nodes[parent].right = next;
        }
        self.nodes[next].parent = parent;
    }

    fn rotate_left(
        &mut self,
        at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        budget.charge(function, ROTATION_UNITS, self.phase)?;
        let right = self.nodes[at].right;
        let middle = self.nodes[right].left;
        let parent = self.nodes[at].parent;
        self.attach(parent, at, right);
        self.nodes[right].left = at;
        self.nodes[at].parent = right;
        self.nodes[at].right = middle;
        if middle != NIL {
            self.nodes[middle].parent = at;
        }
        self.update(at, function, budget)?;
        self.update(right, function, budget)?;
        Ok(right)
    }

    fn rotate_right(
        &mut self,
        at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        budget.charge(function, ROTATION_UNITS, self.phase)?;
        let left = self.nodes[at].left;
        let middle = self.nodes[left].right;
        let parent = self.nodes[at].parent;
        self.attach(parent, at, left);
        self.nodes[left].right = at;
        self.nodes[at].parent = left;
        self.nodes[at].left = middle;
        if middle != NIL {
            self.nodes[middle].parent = at;
        }
        self.update(at, function, budget)?;
        self.update(left, function, budget)?;
        Ok(left)
    }

    fn rebalance(
        &mut self,
        mut at: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        while at != NIL {
            budget.charge(function, REBALANCE_UNITS, self.phase)?;
            self.update(at, function, budget)?;
            let (left, right) = (self.nodes[at].left, self.nodes[at].right);
            let top = if i16::from(self.height(left)) - i16::from(self.height(right)) > 1 {
                if self.height(self.nodes[left].left) < self.height(self.nodes[left].right) {
                    self.rotate_left(left, function, budget)?;
                }
                self.rotate_right(at, function, budget)?
            } else if i16::from(self.height(right)) - i16::from(self.height(left)) > 1 {
                if self.height(self.nodes[right].right) < self.height(self.nodes[right].left) {
                    self.rotate_right(right, function, budget)?;
                }
                self.rotate_left(at, function, budget)?
            } else {
                at
            };
            at = self.nodes[top].parent;
        }
        Ok(())
    }
}
#[cfg(test)]
impl<K: KeyWork> Directory<K> {
    pub(super) fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter()
    }
    pub(super) fn capacity(&self) -> usize {
        self.nodes.capacity()
    }
    pub(super) fn balanced(&self) -> bool {
        fn height<K>(nodes: &[Node<K>], at: usize, parent: usize) -> Option<u8> {
            if at == NIL {
                return Some(0);
            }
            let node = nodes.get(at)?;
            if node.parent != parent {
                return None;
            }
            let left = height(nodes, node.left, at)?;
            let right = height(nodes, node.right, at)?;
            if left.abs_diff(right) > 1 || node.height != left.max(right) + 1 {
                return None;
            }
            Some(node.height)
        }
        height(&self.nodes, self.root, NIL).is_some()
    }
}

mod status_prefix;
pub(super) use status_prefix::Owner as StatusInsertionOwner;
