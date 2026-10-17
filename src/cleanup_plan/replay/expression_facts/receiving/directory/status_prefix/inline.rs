//! In-place status insertion receipts. The caller owns the sole diagnostic cell.
//! This is representation storage, never a source or verification certificate.
use super::*;

const AUTH_BYTES: usize = 2 * size_of::<(
    &Owner<'_>,
    &Directory<&StatusSourceId>,
    &ResolvedFunction,
    &StatusSourceId,
    &mut ReplayBudget,
    &mut Option<Diagnostic>,
)>() + 2 * size_of::<bool>()
    + size_of::<Option<Refusal>>()
    + size_of::<&StatusSourceId>()
    + size_of::<usize>();
const ROW_BYTES: usize = 4 * size_of::<(
    &mut HeldPrefix<'_, '_>,
    &ResolvedFunction,
    &StatusSourceId,
    &mut ReplayBudget,
    &mut Option<Diagnostic>,
)>() + 16 * size_of::<bool>()
    + 2 * size_of::<usize>();

pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn fund_failure(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.reserve_bytes(
        function,
        2 * size_of::<Option<Diagnostic>>()
            + 2 * size_of::<&mut Option<Diagnostic>>()
            + 2 * size_of::<bool>()
            + 2 * size_of::<Result<(), Diagnostic>>()
            + 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget)>()
            + 2 * size_of::<(&HeldPrefix<'_, '_>, &ResolvedFunction, &mut ReplayBudget)>(),
        "held status diagnostic cell",
    )?;
    budget.charge(function, 2, "held status diagnostic cell")
}

impl<'hir> Owner<'hir> {
    fn authenticate_inline(
        &self,
        directory: &Directory<&'hir StatusSourceId>,
        function: &ResolvedFunction,
        key: &StatusSourceId,
        budget: &mut ReplayBudget,
        failure: &mut Option<Diagnostic>,
    ) -> bool {
        macro_rules! attempt {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => {
                        *failure = Some(error);
                        return false;
                    }
                }
            };
        }
        if let Some(first) = self.first.get() {
            *failure = Some(self.refuse(first, function, budget));
            return false;
        }
        if !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.directory, directory)
            || !std::ptr::eq(self.budget, budget)
        {
            *failure = Some(self.refuse(Refusal::Binding, function, budget));
            return false;
        }
        if !self.ledger.current() {
            *failure = Some(self.refuse(Refusal::Invocation, function, budget));
            return false;
        }
        attempt!(budget.reserve_bytes(function, 0, directory.phase));
        attempt!(budget.charge(function, 0, directory.phase));
        attempt!(budget.reserve_bytes(function, AUTH_BYTES, directory.phase));
        attempt!(budget.charge(function, 1, directory.phase));
        if function
            .cleanup_plan
            .status_sources
            .get(self.next.get())
            .is_none_or(|source| !std::ptr::eq(&source.id, key))
        {
            *failure = Some(self.refuse(Refusal::Binding, function, budget));
            return false;
        }
        true
    }
}

impl<'hir> HeldPrefix<'_, 'hir> {
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn fund_failure_cell(
        &self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if let Some(first) = self.owner.first.get() {
            return Err(self.owner.refuse(first, function, budget));
        }
        if !std::ptr::eq(self.owner.function, function)
            || !std::ptr::eq(self.owner.directory, self.directory)
            || !std::ptr::eq(self.owner.budget, budget)
        {
            return Err(self.owner.refuse(Refusal::Binding, function, budget));
        }
        if !self.owner.ledger.current() {
            return Err(self.owner.refuse(Refusal::Invocation, function, budget));
        }
        fund_failure(function, budget)?;
        budget.charge(function, 4, "held status diagnostic cell bindings")
    }
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn insert_inline(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
        failure: &mut Option<Diagnostic>,
    ) -> bool {
        // A selected error remains owned by the caller until it is consumed.
        if failure.is_some() {
            return false;
        }
        if !self
            .index
            .as_ref()
            .is_some_and(|index| index.cursor.is_some())
        {
            // This internal protocol is entered only with an allocated active
            // cursor. Its presence supplies storage, never row authority.
            *failure = Some(self.owner.refuse(Refusal::Binding, function, budget));
            return false;
        }
        let inserted = self.insert_inline_current(function, key, budget, failure);
        if failure.is_some() && self.owner.first.get().is_none() {
            self.owner.first.set(Some(Refusal::Budget));
        }
        inserted
    }
    fn insert_inline_current(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
        failure: &mut Option<Diagnostic>,
    ) -> bool {
        macro_rules! attempt {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => {
                        *failure = Some(error);
                        return false;
                    }
                }
            };
        }
        if !self
            .owner
            .authenticate_inline(self.directory, function, key, budget, failure)
        {
            return false;
        }
        let phase = self.directory.phase;
        attempt!(budget.reserve_bytes(function, ROW_BYTES, phase));
        attempt!(self.directory.authenticate(function, budget));
        // Preserve the original two outer branches, index selector and cursor
        // branch, plus the new three in-place error/control observations.
        attempt!(budget.charge(function, 1, phase));
        attempt!(budget.charge(function, 1, phase));
        attempt!(budget.reserve_bytes(function, size_of::<&StatusIndex>(), phase));
        attempt!(budget.charge(function, 2, phase));
        attempt!(budget.charge(function, 3, phase));
        let inserted = self.insert_reusing_cursor_inline(function, key, budget, failure);
        if failure.is_some() {
            return false;
        }
        attempt!(budget.charge(function, 1, phase));
        self.owner.next.set(self.owner.next.get() + 1);
        inserted
    }
    pub(super) fn insert_reusing_cursor_inline(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
        failure: &mut Option<Diagnostic>,
    ) -> bool {
        macro_rules! attempt {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => {
                        *failure = Some(error);
                        return false;
                    }
                }
            };
        }
        let phase = self.directory.phase;
        attempt!(budget.reserve_bytes(
            function,
            2 * size_of::<(
                &mut HeldPrefix<'_, '_>,
                &ResolvedFunction,
                &StatusSourceId,
                &mut ReplayBudget,
                &mut Option<Diagnostic>,
            )>() + size_of::<&str>()
                + 3 * size_of::<usize>()
                + 2 * size_of::<bool>()
                + size_of::<Node<&StatusSourceId>>()
                + size_of::<bool>()
                + 3 * size_of::<&mut ()>(),
            phase,
        ));
        attempt!(budget.charge(function, 2, phase));
        if self.directory.nodes.is_empty() {
            attempt!(budget.charge(function, 3, phase));
            let prefix = key.expression.as_str().len();
            let inserted = attempt!(self.directory.insert(function, key, budget));
            if inserted {
                attempt!(self
                    .index
                    .as_mut()
                    .unwrap()
                    .seed(0, function, budget, phase));
                self.prefix = prefix;
            }
            return inserted;
        }
        let prefix = attempt!(self.common_prefix(function, key, budget));
        attempt!(budget.charge(function, 3, phase));
        let table = self.index.as_mut().unwrap();
        let directory = &mut *self.directory;
        let search = table.cursor.as_mut().unwrap();
        // These fields live in the fully paid Box for this builder. Reset each
        // in place; no owning IndexSearch or Result<IndexSearch> crosses an ABI.
        attempt!(budget.charge(function, 24, phase));
        search.probe.found = false;
        search.probe.parent = NIL;
        search.probe.before = NIL;
        search.probe.after = NIL;
        search.probe.side = Ordering::Equal;
        search.at = table
            .root
            .expect("held status index seeded with the first AVL key");
        search.parent = NIL;
        search.parent_one = false;
        search.new_one = false;
        search.before = NIL;
        search.after = NIL;
        search.byte = 0;
        search.mask = 0;
        search.symbol = 0;
        search.direction = false;
        search.common = 0;
        search.candidate = NIL;
        search.position = 0;
        search.step.node = NIL;
        search.step.one = false;
        search.step.before = NIL;
        search.step.after = NIL;
        table.path.clear();
        // Preserve the visited construction path, reusing its fully paid backing.
        // It supplies the insertion parent and ordered bounds after one full-key
        // comparison, so no second pass reads query bytes.
        loop {
            attempt!(budget.charge(function, VISIT_UNITS + 2, phase));
            search.byte = table.nodes[search.at].byte;
            search.mask = table.nodes[search.at].mask;
            if search.byte == NIL {
                break;
            }
            attempt!(budget.charge(function, INDEX_BIT_UNITS + 9, phase));
            search.step.node = search.at;
            search.step.one = index_bit!(search, key);
            search.step.before = search.before;
            search.step.after = search.after;
            attempt!(budget.reserve_vec(function, &mut table.path, 1, phase));
            table.path.push(search.step);
            if search.step.one {
                search.before = table.nodes[search.at].zero;
                search.at = table.nodes[search.at].one;
            } else {
                search.after = table.nodes[search.at].one;
                search.at = table.nodes[search.at].zero;
            }
        }
        attempt!(budget.charge(function, 3, phase));
        search.candidate = table.nodes[search.at].zero;
        // Inline the complete original comparison into the held scratch. The
        // actual borrowed/scalar locals remain newly paid on every invocation;
        // no Comparison, pair receipt or nested Result is constructed here.
        attempt!(budget.reserve_bytes(
            function,
            size_of::<&StatusSourceId>()
                + 2 * size_of::<&[u8]>()
                + 2 * size_of::<usize>()
                + 2 * size_of::<u8>()
                + size_of::<Ordering>(),
            phase,
        ));
        attempt!(budget.charge(function, COMPARISON_UNITS, phase));
        let candidate = directory.nodes[search.candidate].key;
        let left = key.expression.as_str().as_bytes();
        let right = candidate.expression.as_str().as_bytes();
        let end = left.len().min(right.len());
        search.common = prefix;
        let units = attempt!(end
            .checked_sub(prefix)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow")));
        attempt!(budget.charge(function, units, phase));
        search.probe.side = 'comparison: {
            while search.common < end {
                let a = left[search.common];
                let b = right[search.common];
                if a != b {
                    break 'comparison a.cmp(&b);
                }
                search.common += 1;
            }
            attempt!(budget.charge(function, 2, phase));
            let order = left.len().cmp(&right.len());
            if order != Ordering::Equal {
                break 'comparison order;
            }
            attempt!(budget.charge(function, 1, phase));
            key.lane.cmp(&candidate.lane)
        };
        if search.probe.side == Ordering::Equal {
            attempt!(budget.charge(function, 1, phase));
            search.probe.found = true;
            return false;
        }
        // The complete comparison supplies the exact first differing expression
        // byte, prefix boundary, or lane. The mask is its highest differing bit.
        attempt!(budget.charge(function, 24, phase));
        search.byte = search.common;
        search.mask = if search.common
            < key.expression.as_str().len().min(
                directory.nodes[search.candidate]
                    .key
                    .expression
                    .as_str()
                    .len(),
            ) {
            128u16
                >> (key.expression.as_str().as_bytes()[search.common]
                    ^ directory.nodes[search.candidate]
                        .key
                        .expression
                        .as_str()
                        .as_bytes()[search.common])
                    .leading_zeros()
        } else if key.expression.as_str().len()
            != directory.nodes[search.candidate]
                .key
                .expression
                .as_str()
                .len()
        {
            256
        } else {
            128u16
                >> (index_lane!(key.lane) ^ index_lane!(directory.nodes[search.candidate].key.lane))
                    .leading_zeros()
        };
        attempt!(budget.charge(function, 1, phase));
        search.position = table.path.len();
        // Byte coordinates increase down a path; masks decrease within a byte.
        while search.position != 0 {
            attempt!(budget.charge(function, VISIT_UNITS + 9, phase));
            search.candidate = table.path[search.position - 1].node;
            if table.nodes[search.candidate].byte < search.byte
                || (table.nodes[search.candidate].byte == search.byte
                    && table.nodes[search.candidate].mask > search.mask)
            {
                break;
            }
            search.position -= 1;
        }
        attempt!(budget.charge(function, 12, phase));
        if search.position < table.path.len() {
            search.at = table.path[search.position].node;
            search.before = table.path[search.position].before;
            search.after = table.path[search.position].after;
        }
        if search.position != 0 {
            search.parent = table.path[search.position - 1].node;
            search.parent_one = table.path[search.position - 1].one;
        }
        attempt!(budget.charge(function, INDEX_BIT_UNITS + 4, phase));
        search.new_one = index_bit!(search, key);
        if search.new_one {
            search.before = search.at;
        } else {
            search.after = search.at;
        }
        // Extremal leaves supply the exact full-order AVL neighbours.
        while search.before != NIL {
            attempt!(budget.charge(function, VISIT_UNITS + 2, phase));
            if table.nodes[search.before].byte == NIL {
                search.probe.before = table.nodes[search.before].zero;
                break;
            }
            search.before = table.nodes[search.before].one;
        }
        while search.after != NIL {
            attempt!(budget.charge(function, VISIT_UNITS + 2, phase));
            if table.nodes[search.after].byte == NIL {
                search.probe.after = table.nodes[search.after].zero;
                break;
            }
            search.after = table.nodes[search.after].zero;
        }
        attempt!(budget.charge(function, 6, phase));
        if search.probe.before != NIL && directory.nodes[search.probe.before].right == NIL {
            search.probe.parent = search.probe.before;
            search.probe.side = Ordering::Greater;
        } else {
            search.probe.parent = search.probe.after;
            search.probe.side = Ordering::Less;
        }
        attempt!(directory.grow(function, budget));
        attempt!(budget.charge(function, VISIT_UNITS, phase));
        let index = directory.nodes.len();
        directory.nodes.push(Node {
            key,
            left: NIL,
            right: NIL,
            parent: search.probe.parent,
            next: search.probe.after,
            height: 1,
        });
        if search.probe.before == NIL {
            directory.first = index;
        } else {
            directory.nodes[search.probe.before].next = index;
        }
        if search.probe.parent == NIL {
            directory.root = index;
        } else if search.probe.side == Ordering::Less {
            directory.nodes[search.probe.parent].left = index;
        } else {
            directory.nodes[search.probe.parent].right = index;
        }
        let parent = search.probe.parent;
        attempt!(budget.reserve_bytes(
            function,
            2 * size_of::<IndexNode>() + 2 * size_of::<usize>(),
            phase,
        ));
        attempt!(budget.reserve_vec(function, &mut table.nodes, 2, phase));
        attempt!(budget.charge(function, 18, phase));
        let leaf = table.nodes.len();
        let branch = leaf + 1;
        table.nodes.push(IndexNode {
            byte: NIL,
            mask: 0,
            zero: index,
            one: NIL,
        });
        table.nodes.push(IndexNode {
            byte: search.byte,
            mask: search.mask,
            zero: if search.new_one { search.at } else { leaf },
            one: if search.new_one { leaf } else { search.at },
        });
        if search.parent == NIL {
            table.root = Some(branch);
        } else if search.parent_one {
            table.nodes[search.parent].one = branch;
        } else {
            table.nodes[search.parent].zero = branch;
        }
        attempt!(directory.rebalance(parent, function, budget));
        attempt!(budget.charge(function, 1, phase));
        self.prefix = prefix;
        true
    }
}

#[cfg(test)]
mod tests;
