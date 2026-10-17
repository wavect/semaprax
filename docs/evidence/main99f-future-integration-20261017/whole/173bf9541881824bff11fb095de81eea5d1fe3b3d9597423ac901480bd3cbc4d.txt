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
    fn status_key(&self) -> Option<&StatusSourceId> {
        None
    }
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
    fn status_key(&self) -> Option<&StatusSourceId> {
        Some(self)
    }
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
    fn status_key(&self) -> Option<&StatusSourceId> {
        T::status_key(self)
    }
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
    lookup: Option<Box<status_prefix::FrozenStatusIndex>>,
}

struct LookupCursor {
    at: usize,
    byte: usize,
    mask: u16,
    symbol: u16,
    direction: bool,
    units: usize,
}
// Complete initialized cursor plus receiving state, status-key Option and
// borrowed receivers, final candidate/ordering and comparison temporaries.
const LOOKUP_BYTES: usize = 2 * std::mem::size_of::<LookupCursor>()
    + 2 * std::mem::size_of::<Option<&StatusSourceId>>()
    + 4 * std::mem::size_of::<&StatusSourceId>()
    + std::mem::size_of::<&status_prefix::FrozenStatusIndex>()
    + 2 * std::mem::size_of::<Option<usize>>()
    + std::mem::size_of::<usize>()
    + std::mem::size_of::<Ordering>();

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
        // The new optional field has two initialized receiving slots. Existing
        // directory fields and reservation requests receive no credit.
        budget.reserve_bytes(
            function,
            2 * std::mem::size_of::<Option<Box<status_prefix::FrozenStatusIndex>>>(),
            phase,
        )?;
        budget.charge(function, 1, phase)?;
        Ok(Self {
            nodes: Vec::new(),
            root: NIL,
            first: NIL,
            phase,
            lookup: None,
        })
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
        if let Some(index) = &self.lookup {
            // Binding checks read no foreign payer or cached key. A rejected
            // receiver remains rejected even after its original owner returns.
            if index.failed.get() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            if !std::ptr::eq(index.function, function)
                || !std::ptr::eq(index.budget, budget)
                || !index.ledger.current()
                || index.keys != self.nodes.as_ptr() as usize
                || index.len != self.nodes.len()
            {
                index.failed.set(true);
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            self.authenticate(function, budget)?;
            // All traversal state is one fixed receiving frame. Loop steps
            // update its paid fields rather than constructing a new path.
            budget.reserve_bytes(function, LOOKUP_BYTES, self.phase)?;
            budget.charge(function, 24, self.phase)?;
            let Some(query) = key.status_key() else {
                index.failed.set(true);
                return Err(Diagnostic::io("SPX-H006", String::new()));
            };
            let mut cursor = LookupCursor {
                at: index.root,
                byte: 0,
                mask: 0,
                symbol: 0,
                direction: false,
                units: 0,
            };
            cursor.byte = index.nodes[cursor.at].byte;
            while cursor.byte != NIL {
                budget.charge(function, VISIT_UNITS + 24, self.phase)?;
                cursor.mask = index.nodes[cursor.at].mask;
                cursor.symbol = if cursor.byte < query.expression.as_str().len() {
                    256 | query.expression.as_str().as_bytes()[cursor.byte] as u16
                } else if cursor.byte == query.expression.as_str().len() {
                    match query.lane {
                        StatusLane::OwnerAdmission => 0,
                        StatusLane::OperationFailure => 1,
                        StatusLane::AllocationFailure => 2,
                        StatusLane::ContractFalse => 3,
                    }
                } else {
                    0
                };
                cursor.direction = cursor.symbol & cursor.mask != 0;
                cursor.at = if cursor.direction {
                    index.nodes[cursor.at].one
                } else {
                    index.nodes[cursor.at].zero
                };
                cursor.byte = index.nodes[cursor.at].byte;
            }
            budget.charge(function, VISIT_UNITS + 2, self.phase)?;
            cursor.at = index.nodes[cursor.at].zero;
            let candidate = &self.nodes[cursor.at];
            // The index selects only a candidate. Preserve the complete old
            // key-comparison request and actual Ord equality, without a prefix
            // discount, identifier parsing, hashes or numeric-ID authority.
            cursor.units = match key
                .comparison_units(&candidate.key)
                .and_then(|units| units.checked_add(VISIT_UNITS))
            {
                Some(units) => units,
                None => {
                    return Err(overflow(
                        function,
                        budget,
                        "directory key comparison work overflow",
                    ))
                }
            };
            budget.charge(function, cursor.units, self.phase)?;
            return Ok(Ord::cmp(key, &candidate.key) == Ordering::Equal);
        }
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
        if let Some(index) = &self.lookup {
            // Publication seals this privately owned directory. No mutation or
            // fallback can revive a rejected read-only index.
            index.failed.set(true);
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
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
