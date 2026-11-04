//! Online paid full-expression facts. Indices are private construction coordinates.
use super::*;
use std::cmp::Ordering;
mod prefix;
#[cfg(test)]
pub(in crate::cleanup_plan::replay::expression_facts) use prefix::compact::NARROWED_SAVING as NARROWED_LOOKUP_SAVING;
pub(super) mod retained;
const NIL: usize = usize::MAX;
struct Node<'hir> {
    key: ExpressionId,
    fact: Option<CallFact<'hir>>,
    left: usize,
    right: usize,
    parent: usize,
    next: usize,
    height: u8,
}
pub(in crate::cleanup_plan::replay) struct Arena<'hir> {
    nodes: Vec<Node<'hir>>,
    shared_prefix: (usize, usize),
    pending: std::cell::RefCell<Option<PreparedInsertion>>,
    insertion_failed: std::cell::Cell<bool>,
    root: usize,
    first: usize,
}
// One immediately consumed construction proof. The complete immutable key
// stays owned by its original Arc; no digest, numeric ID or result cache binds it.
struct PreparedInsertion {
    key: ExpressionId,
    len: usize,
    parent: usize,
    before: usize,
    after: usize,
    side: Ordering,
    found: bool,
    prefix: usize,
    work: usize,
}
fn failure(detail: &'static str, physical: bool) -> capacity::Failure {
    capacity::Failure { detail, physical }
}
fn comparison_units(a: &ExpressionId, b: &ExpressionId) -> Result<usize, capacity::Failure> {
    a.as_str()
        .len()
        .min(b.as_str().len())
        .checked_add(1)
        .ok_or_else(|| failure("expression-fact key comparison work overflow", false))
}
impl<'hir> Arena<'hir> {
    pub(super) fn new(
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<Self, capacity::Failure> {
        capacity.arena_reserve_array::<Self>(1)?;
        Ok(Self {
            nodes: Vec::new(),
            shared_prefix: (0, 0),
            pending: std::cell::RefCell::new(None),
            insertion_failed: std::cell::Cell::new(false),
            root: NIL,
            first: NIL,
        })
    }
    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }
    // Reserve the complete replacement while the old allocation is still live.
    // This occurs before dispatch, at the old node-reservation boundary.
    pub(super) fn reserve_next(
        &mut self,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<(), capacity::Failure> {
        capacity.arena_reserve_array::<Node<'hir>>(0)?;
        let required = self
            .nodes
            .len()
            .checked_add(1)
            .ok_or_else(|| failure("expression-fact arena capacity overflow", true))?;
        if required <= self.nodes.capacity() {
            return Ok(());
        }
        let target =
            super::super::physical_storage::replay_growth_capacity(self.nodes.capacity(), required)
                .ok_or_else(|| failure("expression-fact arena capacity overflow", true))?;
        capacity.arena_reserve_array::<Node<'hir>>(target)?;
        capacity.arena_work(self.nodes.len())?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(target)
            .map_err(|_| failure("physical expression-fact arena allocation refused", true))?;
        if replacement.capacity() != target {
            return Err(failure(
                "physical expression-fact arena capacity differs",
                true,
            ));
        }
        replacement.append(&mut self.nodes);
        self.nodes = replacement;
        Ok(())
    }
    #[cfg(test)]
    fn prepare_insert_work_before_rotation_bound(
        &self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        // Each search compares at most key.len()+1 bytes and visits at most
        // three indexed fields. Ancestor update/balance/parent reads cost at
        // most 16 indexed visits, plus two rotations of at most 20 visits.
        // Appending and predecessor/parent links cost at most three visits.
        let levels = usize::from(self.height(self.root)) + 1;
        let units = key
            .as_str()
            .len()
            .checked_add(60)
            .and_then(|per_level| per_level.checked_mul(levels))
            .and_then(|bound| bound.checked_add(3))
            .ok_or_else(|| {
                budget.budget_refusal(function, "expression-fact insertion work overflow", "")
            })?;
        budget.charge(function, units, "expression-fact online AVL insertion")
    }
    pub(super) fn prepare_growth_work(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.nodes.len() == self.nodes.capacity() {
            let units = self.nodes.len().checked_add(1).ok_or_else(|| {
                budget.budget_refusal(function, "expression-fact relocation work overflow", "")
            })?;
            budget.charge(
                function,
                units,
                "expression-fact arena allocation and relocation",
            )?;
        }
        Ok(())
    }
    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn insert_before_prefix(
        &mut self,
        key: ExpressionId,
        fact: Option<CallFact<'hir>>,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<bool, capacity::Failure> {
        capacity.arena_reserve_array::<Node<'hir>>(0)?;
        let (mut at, mut parent, mut before, mut after) = (self.root, NIL, NIL, NIL);
        let mut side = Ordering::Equal;
        while at != NIL {
            capacity.arena_work(3)?;
            capacity.arena_work(comparison_units(&key, &self.nodes[at].key)?)?;
            side = key.cmp(&self.nodes[at].key);
            if side == Ordering::Equal {
                return Ok(false);
            }
            parent = at;
            if side == Ordering::Less {
                after = at;
                at = self.nodes[at].left;
            } else {
                before = at;
                at = self.nodes[at].right;
            }
        }
        capacity.arena_work(3)?;
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            fact,
            left: NIL,
            right: NIL,
            parent,
            next: after,
            height: 1,
        });
        if before == NIL {
            self.first = index;
        } else {
            self.nodes[before].next = index;
        }
        if parent == NIL {
            self.root = index;
        } else if side == Ordering::Less {
            self.nodes[parent].left = index;
        } else {
            self.nodes[parent].right = index;
        }
        self.rebalance(parent, capacity)?;
        Ok(true)
    }
    fn height(&self, at: usize) -> u8 {
        if at == NIL {
            0
        } else {
            self.nodes[at].height
        }
    }
    fn update(&mut self, at: usize) -> Result<(), capacity::Failure> {
        self.nodes[at].height = self
            .height(self.nodes[at].left)
            .max(self.height(self.nodes[at].right))
            .checked_add(1)
            .ok_or_else(|| failure("expression-fact AVL height overflow", false))?;
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
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<usize, capacity::Failure> {
        capacity.arena_work(20)?;
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
        self.update(at)?;
        self.update(right)?;
        Ok(right)
    }
    fn rotate_right(
        &mut self,
        at: usize,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<usize, capacity::Failure> {
        capacity.arena_work(20)?;
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
        self.update(at)?;
        self.update(left)?;
        Ok(left)
    }
    #[cfg(test)]
    fn rebalance(
        &mut self,
        mut at: usize,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<(), capacity::Failure> {
        while at != NIL {
            capacity.arena_work(16)?;
            self.update(at)?;
            let (left, right) = (self.nodes[at].left, self.nodes[at].right);
            let top = if i16::from(self.height(left)) - i16::from(self.height(right)) > 1 {
                if self.height(self.nodes[left].left) < self.height(self.nodes[left].right) {
                    self.rotate_left(left, capacity)?;
                }
                self.rotate_right(at, capacity)?
            } else if i16::from(self.height(right)) - i16::from(self.height(left)) > 1 {
                if self.height(self.nodes[right].right) < self.height(self.nodes[right].left) {
                    self.rotate_right(right, capacity)?;
                }
                self.rotate_left(at, capacity)?
            } else {
                at
            };
            at = self.nodes[top].parent;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn prepare_insert_work(
        &self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        // Any new attempt destroys the previous one-use receipt, including a
        // refused attempt. No failed preparation can leave insertion authority.
        if self.insertion_failed.get() {
            return Err(budget.budget_refusal(
                function,
                "expression-fact insertion already failed",
                "",
            ));
        }
        self.insertion_failed.set(true);
        self.pending.borrow_mut().take();
        budget.reserve_bytes(
            function,
            4 * std::mem::size_of::<PreparedInsertion>()
                + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
                + 2 * std::mem::size_of::<(&Arena<'_>, &ExpressionId)>()
                + 4 * std::mem::size_of::<&[u8]>()
                + 12 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<Ordering>(),
            "expression-fact prepared insertion frames",
        )?;
        budget.charge(function, 8, "expression-fact insertion authority")?;
        let prefix_work = if self.nodes.is_empty() {
            7
        } else {
            self.shared_prefix
                .0
                .min(key.as_str().len())
                .saturating_add(7)
        };
        budget.charge(
            function,
            prefix_work,
            "expression-fact insertion prefix certificate",
        )?;
        let prefix = self.insertion_prefix(key);
        let (mut at, mut parent, mut before, mut after) = (self.root, NIL, NIL, NIL);
        let mut side = Ordering::Equal;
        let mut search_work = prefix_work;
        while at != NIL {
            budget.charge(function, 3, "expression-fact prepared search")?;
            search_work += 3;
            let left = &key.as_str().as_bytes()[prefix..];
            let right = &self.nodes[at].key.as_str().as_bytes()[prefix..];
            let mut offset = 0;
            side = loop {
                budget.charge(function, 1, "expression-fact prepared comparison")?;
                search_work += 1;
                if offset == left.len().min(right.len()) {
                    break left.len().cmp(&right.len());
                }
                let order = left[offset].cmp(&right[offset]);
                if order != Ordering::Equal {
                    break order;
                }
                offset += 1;
            };
            if side == Ordering::Equal {
                break;
            }
            parent = at;
            if side == Ordering::Less {
                after = at;
                at = self.nodes[at].left;
            } else {
                before = at;
                at = self.nodes[at].right;
            }
        }
        let found = at != NIL;
        // Predict only how many existing heights can change. A repair or an
        // unchanged subtree height ends propagation. All forty possible
        // rotation visits remain paid before any tree link changes.
        let mut ancestor = parent;
        let mut direction = side;
        let mut ancestors = 0usize;
        if !found {
            while ancestor != NIL {
                budget.charge(function, 8, "expression-fact insertion height prediction")?;
                search_work += 8;
                let node = &self.nodes[ancestor];
                let mut left = u16::from(self.height(node.left));
                let mut right = u16::from(self.height(node.right));
                if direction == Ordering::Less {
                    left += 1;
                } else {
                    right += 1;
                }
                ancestors += 1;
                if left.abs_diff(right) > 1 || left.max(right) + 1 == u16::from(node.height) {
                    break;
                }
                let child = ancestor;
                ancestor = node.parent;
                if ancestor != NIL {
                    direction = if self.nodes[ancestor].left == child {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
            }
        }
        let work = ancestors
            .checked_mul(19)
            .and_then(|n| n.checked_add(53))
            .ok_or_else(|| {
                budget.budget_refusal(function, "expression-fact insertion work overflow", "")
            })?;
        budget.charge(function, work, "expression-fact prepared append and repair")?;
        self.pending.replace(Some(PreparedInsertion {
            key: key.clone(),
            len: self.nodes.len(),
            parent,
            before,
            after,
            side,
            found,
            prefix,
            work: search_work,
        }));
        self.insertion_failed.set(false);
        Ok(())
    }
    pub(super) fn prepare_insert_work_with_capacity<P: errors::Policy>(
        &self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<(), P::Error> {
        // Any new attempt destroys the previous one-use receipt, including a
        // refused attempt. No failed preparation can leave insertion authority.
        if self.insertion_failed.get() {
            return Err(budget
                .budget_refusal(function, "expression-fact insertion already failed", "")
                .into());
        }
        self.insertion_failed.set(true);
        self.pending.borrow_mut().take();
        // FactCapacity holds the original selected reservation during this
        // collection. Charge that exact live payer rather than re-entering the
        // ReplayBudget whose physical loan is temporarily held by capacity.
        // An explicitly unobserved ReplayBudget has never paid these local
        // receiving carriers; its separate capacity loan still pays all heap.
        if budget.pays_allocations() {
            capacity
                .arena_reserve_array::<u8>(
                    4 * std::mem::size_of::<PreparedInsertion>()
                        + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
                        + 2 * std::mem::size_of::<(&Arena<'_>, &ExpressionId)>()
                        + 4 * std::mem::size_of::<&[u8]>()
                        + 12 * std::mem::size_of::<usize>()
                        + 2 * std::mem::size_of::<Ordering>()
                        + 2 * std::mem::size_of::<&mut capacity::FactCapacity<'hir, '_>>()
                        + 2 * std::mem::size_of::<Result<(), P::Error>>(),
                )
                .map_err(|failure| P::capacity(function, failure))?;
        }
        budget.charge(function, 8, "expression-fact insertion authority")?;
        let prefix_work = if self.nodes.is_empty() {
            7
        } else {
            self.shared_prefix
                .0
                .min(key.as_str().len())
                .saturating_add(7)
        };
        budget.charge(
            function,
            prefix_work,
            "expression-fact insertion prefix certificate",
        )?;
        let prefix = self.insertion_prefix(key);
        let (mut at, mut parent, mut before, mut after) = (self.root, NIL, NIL, NIL);
        let mut side = Ordering::Equal;
        let mut search_work = prefix_work;
        while at != NIL {
            budget.charge(function, 3, "expression-fact prepared search")?;
            search_work += 3;
            let left = &key.as_str().as_bytes()[prefix..];
            let right = &self.nodes[at].key.as_str().as_bytes()[prefix..];
            let mut offset = 0;
            side = loop {
                budget.charge(function, 1, "expression-fact prepared comparison")?;
                search_work += 1;
                if offset == left.len().min(right.len()) {
                    break left.len().cmp(&right.len());
                }
                let order = left[offset].cmp(&right[offset]);
                if order != Ordering::Equal {
                    break order;
                }
                offset += 1;
            };
            if side == Ordering::Equal {
                break;
            }
            parent = at;
            if side == Ordering::Less {
                after = at;
                at = self.nodes[at].left;
            } else {
                before = at;
                at = self.nodes[at].right;
            }
        }
        let found = at != NIL;
        // Predict only how many existing heights can change. A repair or an
        // unchanged subtree height ends propagation. All forty possible
        // rotation visits remain paid before any tree link changes.
        let mut ancestor = parent;
        let mut direction = side;
        let mut ancestors = 0usize;
        if !found {
            while ancestor != NIL {
                budget.charge(function, 8, "expression-fact insertion height prediction")?;
                search_work += 8;
                let node = &self.nodes[ancestor];
                let mut left = u16::from(self.height(node.left));
                let mut right = u16::from(self.height(node.right));
                if direction == Ordering::Less {
                    left += 1;
                } else {
                    right += 1;
                }
                ancestors += 1;
                if left.abs_diff(right) > 1 || left.max(right) + 1 == u16::from(node.height) {
                    break;
                }
                let child = ancestor;
                ancestor = node.parent;
                if ancestor != NIL {
                    direction = if self.nodes[ancestor].left == child {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    };
                }
            }
        }
        let work = ancestors
            .checked_mul(19)
            .and_then(|n| n.checked_add(53))
            .ok_or_else(|| {
                budget.budget_refusal(function, "expression-fact insertion work overflow", "")
            })?;
        budget.charge(function, work, "expression-fact prepared append and repair")?;
        self.pending.replace(Some(PreparedInsertion {
            key: key.clone(),
            len: self.nodes.len(),
            parent,
            before,
            after,
            side,
            found,
            prefix,
            work: search_work,
        }));
        self.insertion_failed.set(false);
        Ok(())
    }
    fn insertion_prefix(&self, key: &ExpressionId) -> usize {
        if self.nodes.is_empty() {
            return key.as_str().len();
        }
        if self.shared_prefix.1 != self.nodes.len() {
            return 0;
        }
        let first = self.nodes[self.first].key.as_str().as_bytes();
        let query = key.as_str().as_bytes();
        let end = self.shared_prefix.0.min(first.len()).min(query.len());
        let mut common = 0;
        while common < end && first[common] == query[common] {
            common += 1;
        }
        common
    }
    pub(super) fn insert(
        &mut self,
        key: ExpressionId,
        fact: Option<CallFact<'hir>>,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<bool, capacity::Failure> {
        if self.insertion_failed.get() {
            return Err(failure("expression-fact insertion already failed", false));
        }
        let Some(prepared) = self.pending.get_mut().take() else {
            capacity.arena_work(4)?;
            capacity.arena_reserve_array::<Result<bool, capacity::Failure>>(2)?;
            let result = self.insert_before_prepared(key, fact, capacity);
            self.insertion_failed.set(result.is_err());
            return result;
        };
        self.insertion_failed.set(true);
        capacity.arena_reserve_array::<Node<'hir>>(0)?;
        capacity.arena_work(18)?;
        // The live complete Arc prevents allocation-address reuse. Production
        // passes the original key's Arc clone directly from the HIR cursor.
        if prepared.len != self.nodes.len()
            || prepared.key.as_str().len() != key.as_str().len()
            || prepared.key.as_str().as_ptr() != key.as_str().as_ptr()
        {
            return Err(failure(
                "expression-fact prepared insertion binding differs",
                false,
            ));
        }
        capacity.arena_work(prepared.work)?;
        if prepared.found {
            self.insertion_failed.set(false);
            return Ok(false);
        }
        let (parent, before, after, side) = (
            prepared.parent,
            prepared.before,
            prepared.after,
            prepared.side,
        );
        capacity.arena_work(3)?;
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            fact,
            left: NIL,
            right: NIL,
            parent,
            next: after,
            height: 1,
        });
        if before == NIL {
            self.first = index;
        } else {
            self.nodes[before].next = index;
        }
        if parent == NIL {
            self.root = index;
        } else if side == Ordering::Less {
            self.nodes[parent].left = index;
        } else {
            self.nodes[parent].right = index;
        }
        self.shared_prefix = (prepared.prefix, self.nodes.len());
        self.rebalance_height_stable(parent, capacity)?;
        self.insertion_failed.set(false);
        Ok(true)
    }
    pub(super) fn insert_before_prepared(
        &mut self,
        key: ExpressionId,
        fact: Option<CallFact<'hir>>,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<bool, capacity::Failure> {
        capacity.arena_reserve_array::<Node<'hir>>(0)?;
        capacity.arena_reserve_array::<u8>(
            2 * std::mem::size_of::<(&Arena<'_>, &ExpressionId)>()
                + 2 * std::mem::size_of::<&[u8]>()
                + 6 * std::mem::size_of::<usize>()
                + 2 * std::mem::size_of::<Result<(), capacity::Failure>>(),
        )?;
        capacity.arena_work(
            self.shared_prefix
                .0
                .min(key.as_str().len())
                .saturating_add(7),
        )?;
        let prefix = self.insertion_prefix(&key);
        let (mut at, mut parent, mut before, mut after) = (self.root, NIL, NIL, NIL);
        let mut side = Ordering::Equal;
        while at != NIL {
            capacity.arena_work(3)?;
            let candidate = &self.nodes[at].key;
            let units = key
                .as_str()
                .len()
                .min(candidate.as_str().len())
                .checked_sub(prefix)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| failure("expression-fact insertion suffix work overflow", false))?;
            capacity.arena_work(units)?;
            side = key.as_str().as_bytes()[prefix..].cmp(&candidate.as_str().as_bytes()[prefix..]);
            if side == Ordering::Equal {
                return Ok(false);
            }
            parent = at;
            if side == Ordering::Less {
                after = at;
                at = self.nodes[at].left;
            } else {
                before = at;
                at = self.nodes[at].right;
            }
        }
        capacity.arena_work(3)?;
        let index = self.nodes.len();
        self.nodes.push(Node {
            key,
            fact,
            left: NIL,
            right: NIL,
            parent,
            next: after,
            height: 1,
        });
        if before == NIL {
            self.first = index;
        } else {
            self.nodes[before].next = index;
        }
        if parent == NIL {
            self.root = index;
        } else if side == Ordering::Less {
            self.nodes[parent].left = index;
        } else {
            self.nodes[parent].right = index;
        }
        self.shared_prefix = (prefix, self.nodes.len());
        self.rebalance_height_stable(parent, capacity)?;
        Ok(true)
    }
    fn rebalance_height_stable(
        &mut self,
        mut at: usize,
        capacity: &mut capacity::FactCapacity<'hir, '_>,
    ) -> Result<(), capacity::Failure> {
        // The new retained height and predicate are full primitive values;
        // their payment call's actual/formal and Result receivers are fresh.
        capacity.arena_reserve_array::<u8>(
            2 * std::mem::size_of::<(u8, bool)>()
                + 2 * std::mem::size_of::<(&mut capacity::FactCapacity<'_, '_>, usize)>()
                + 2 * std::mem::size_of::<Result<(), capacity::Failure>>(),
        )?;
        while at != NIL {
            capacity.arena_work(19)?;
            let previous_height = self.nodes[at].height;
            self.update(at)?;
            let (left, right) = (self.nodes[at].left, self.nodes[at].right);
            let top = if i16::from(self.height(left)) - i16::from(self.height(right)) > 1 {
                if self.height(self.nodes[left].left) < self.height(self.nodes[left].right) {
                    self.rotate_left(left, capacity)?;
                }
                self.rotate_right(at, capacity)?
            } else if i16::from(self.height(right)) - i16::from(self.height(left)) > 1 {
                if self.height(self.nodes[right].right) < self.height(self.nodes[right].left) {
                    self.rotate_right(right, capacity)?;
                }
                self.rotate_left(at, capacity)?
            } else {
                at
            };
            // A repaired insertion preserves this subtree's old height.
            // All higher ancestors already have correct heights and balance.
            if self.nodes[top].height == previous_height {
                break;
            }
            at = self.nodes[top].parent;
        }
        Ok(())
    }
    pub(super) fn entries(&self) -> Iter<'_, 'hir> {
        Iter {
            arena: self,
            next: self.first,
        }
    }
    #[cfg(test)]
    pub(super) fn balanced(&self) -> bool {
        fn height(arena: &Arena<'_>, at: usize, parent: usize) -> Option<u8> {
            if at == NIL {
                return Some(0);
            }
            let node = arena.nodes.get(at)?;
            if node.parent != parent {
                return None;
            }
            let left = height(arena, node.left, at)?;
            let right = height(arena, node.right, at)?;
            if left.abs_diff(right) > 1 || node.height != left.max(right) + 1 {
                return None;
            }
            Some(node.height)
        }
        height(self, self.root, NIL).is_some()
    }
    #[cfg(test)]
    fn find(
        &self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&Option<CallFact<'hir>>>, Diagnostic> {
        budget.reserve_bytes(function, 0, "expression-fact arena lookup authentication")?;
        budget.charge(function, 0, "expression-fact arena lookup authentication")?;
        let mut at = self.root;
        while at != NIL {
            let units = comparison_units(key, &self.nodes[at].key)
                .map_err(|f| budget.budget_refusal(function, f.detail, ""))?;
            let units = units.checked_add(3).ok_or_else(|| {
                budget.budget_refusal(function, "expression-fact lookup work overflow", "")
            })?;
            budget.charge(function, units, "expression-fact arena lookup")?;
            match key.cmp(&self.nodes[at].key) {
                Ordering::Equal => return Ok(Some(&self.nodes[at].fact)),
                Ordering::Less => at = self.nodes[at].left,
                Ordering::Greater => at = self.nodes[at].right,
            }
        }
        Ok(None)
    }
}
pub(in crate::cleanup_plan::replay) struct Iter<'a, 'hir> {
    arena: &'a Arena<'hir>,
    next: usize,
}
impl<'a, 'hir> Iterator for Iter<'a, 'hir> {
    type Item = (&'a ExpressionId, &'a Option<CallFact<'hir>>);
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == NIL {
            return None;
        }
        let node = &self.arena.nodes[self.next];
        self.next = node.next;
        Some((&node.key, &node.fact))
    }
}
impl<'hir> Lookup for Arena<'hir> {
    fn entries(&self) -> Entries<'_> {
        Entries::Arena(self.entries())
    }
    fn entries_paid(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Entries<'_>, Diagnostic> {
        budget.reserve_bytes(function, 0, "expression-fact arena entries authentication")?;
        let units = self.nodes.len().checked_add(1).ok_or_else(|| {
            budget.budget_refusal(function, "expression-fact entry work overflow", "")
        })?;
        budget.charge(function, units, "expression-fact arena entries")?;
        Ok(Entries::Arena(self.entries()))
    }
    fn fact<'a>(
        &'a self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a Option<CallFact<'a>>>, Diagnostic> {
        if budget.payer == AllocationPayer::ActiveBuilder && !self.nodes.is_empty() {
            Ok(physical_budget::compact_try!(
                prefix::compact::find_receiving(self, function, key, budget)
            ))
        } else {
            prefix::find(self, function, key, budget)
        }
    }
}

mod producer;
pub(super) fn build<'hir, P: errors::Policy>(
    function: &'hir ResolvedFunction,
    payer: capacity::Payer<'_>,
    budget: &mut ReplayBudget,
) -> Result<Arena<'hir>, P::Error> {
    let mut capacity = capacity::ArenaCapacity::new(function, payer)
        .map_err(|failure| P::capacity(function, failure))?;
    budget.charge(function, 1, "expression-fact arena owner construction")?;
    let mut facts = Arena::new(&mut capacity).map_err(|failure| P::capacity(function, failure))?;
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        producer::collect_arena_facts::<P>(
            function,
            expression,
            &mut facts,
            Some(&mut capacity),
            budget,
        )?;
    }
    Ok(facts)
}

#[cfg(test)]
#[path = "arena/cursor_tests.rs"]
mod cursor_tests;
#[cfg(test)]
#[path = "arena/producer_before_cursor.rs"]
mod producer_before_cursor;

#[cfg(test)]
#[path = "arena/before_cursor.rs"]
mod before_cursor;
