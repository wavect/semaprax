//! Online paid full-expression facts. Indices are private construction coordinates.
use super::*;
use std::cmp::Ordering;
mod prefix;
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
    root: usize,
    first: usize,
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
    pub(super) fn prepare_insert_work(
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
    pub(super) fn insert(
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
            prefix::find_receiving(self, function, key, budget)
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
