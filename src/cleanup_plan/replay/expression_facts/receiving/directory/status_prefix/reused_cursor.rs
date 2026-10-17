//! Paid mutable insertion scratch; no query, cached verdict or publication authority.
use super::*;

// Whole original receiver bank, now acquired by the caller before its ABI.
pub(super) const ALLOCATE_BYTES: usize = 3 * size_of::<IndexSearch>()
    + 2 * size_of::<Box<IndexSearch>>()
    + size_of::<Result<Box<IndexSearch>, Diagnostic>>()
    + 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &str)>();
pub(super) const INSERT_BYTES: usize = 2 * size_of::<(
    &mut HeldPrefix<'_, '_>,
    &ResolvedFunction,
    &StatusSourceId,
    &mut ReplayBudget,
)>() + size_of::<&str>()
    + 3 * size_of::<usize>()
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + size_of::<Node<&StatusSourceId>>()
    + size_of::<bool>()
    + 3 * size_of::<&mut ()>();

#[cfg(test)]
pub(super) fn allocate(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &'static str,
) -> Result<Box<IndexSearch>, Diagnostic> {
    // Initialized value, move receiver, heap backing, Box and returned receipt.
    budget.reserve_bytes(
        function,
        3 * size_of::<IndexSearch>()
            + 2 * size_of::<Box<IndexSearch>>()
            + size_of::<Result<Box<IndexSearch>, Diagnostic>>()
            + 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &str)>(),
        phase,
    )?;
    budget.charge(function, 27, phase)?;
    let search = IndexSearch {
        probe: Probe {
            found: false,
            parent: NIL,
            before: NIL,
            after: NIL,
            side: Ordering::Equal,
        },
        at: NIL,
        parent: NIL,
        parent_one: false,
        new_one: false,
        before: NIL,
        after: NIL,
        byte: 0,
        mask: 0,
        symbol: 0,
        direction: false,
        common: 0,
        candidate: NIL,
        position: 0,
        step: IndexStep {
            node: NIL,
            one: false,
            before: NIL,
            after: NIL,
        },
    };
    Ok(Box::new(search))
}

pub(super) fn allocate_prepaid(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &'static str,
) -> Result<Box<IndexSearch>, Diagnostic> {
    // Initialized value, move receiver, heap backing, Box and returned receipt.
    budget.charge(function, 27, phase)?;
    let search = IndexSearch {
        probe: Probe {
            found: false,
            parent: NIL,
            before: NIL,
            after: NIL,
            side: Ordering::Equal,
        },
        at: NIL,
        parent: NIL,
        parent_one: false,
        new_one: false,
        before: NIL,
        after: NIL,
        byte: 0,
        mask: 0,
        symbol: 0,
        direction: false,
        common: 0,
        candidate: NIL,
        position: 0,
        step: IndexStep {
            node: NIL,
            one: false,
            before: NIL,
            after: NIL,
        },
    };
    Ok(Box::new(search))
}

#[cfg(test)]
impl<'hir> HeldPrefix<'_, 'hir> {
    pub(super) fn insert_reusing_cursor_before(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        let phase = self.directory.phase;
        budget.reserve_bytes(
            function,
            2 * size_of::<(
                &mut HeldPrefix<'_, '_>,
                &ResolvedFunction,
                &StatusSourceId,
                &mut ReplayBudget,
            )>() + size_of::<&str>()
                + 3 * size_of::<usize>()
                + 2 * size_of::<Result<bool, Diagnostic>>()
                + size_of::<Node<&StatusSourceId>>()
                + size_of::<bool>()
                + 3 * size_of::<&mut ()>(),
            phase,
        )?;
        budget.charge(function, 2, phase)?;
        if self.directory.nodes.is_empty() {
            budget.charge(function, 3, phase)?;
            let prefix = key.expression.as_str().len();
            let inserted = self.directory.insert(function, key, budget)?;
            if inserted {
                self.index
                    .as_mut()
                    .unwrap()
                    .seed(0, function, budget, phase)?;
                self.prefix = prefix;
            }
            return Ok(inserted);
        }
        let prefix = self.common_prefix(function, key, budget)?;
        budget.charge(function, 3, phase)?;
        let table = self.index.as_mut().unwrap();
        let directory = &mut *self.directory;
        let search = table.cursor.as_mut().unwrap();
        // These fields live in the fully paid Box for this builder. Reset each
        // in place; no owning IndexSearch or Result<IndexSearch> crosses an ABI.
        budget.charge(function, 24, phase)?;
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
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            search.byte = table.nodes[search.at].byte;
            search.mask = table.nodes[search.at].mask;
            if search.byte == NIL {
                break;
            }
            budget.charge(function, INDEX_BIT_UNITS + 9, phase)?;
            search.step.node = search.at;
            search.step.one = index_bit!(search, key);
            search.step.before = search.before;
            search.step.after = search.after;
            budget.reserve_vec(function, &mut table.path, 1, phase)?;
            table.path.push(search.step);
            if search.step.one {
                search.before = table.nodes[search.at].zero;
                search.at = table.nodes[search.at].one;
            } else {
                search.after = table.nodes[search.at].one;
                search.at = table.nodes[search.at].zero;
            }
        }
        budget.charge(function, 3, phase)?;
        search.candidate = table.nodes[search.at].zero;
        // Inline the complete original comparison into the held scratch. The
        // actual borrowed/scalar locals remain newly paid on every invocation;
        // no Comparison, pair receipt or nested Result is constructed here.
        budget.reserve_bytes(
            function,
            size_of::<&StatusSourceId>()
                + 2 * size_of::<&[u8]>()
                + 2 * size_of::<usize>()
                + 2 * size_of::<u8>()
                + size_of::<Ordering>(),
            phase,
        )?;
        budget.charge(function, COMPARISON_UNITS, phase)?;
        let candidate = directory.nodes[search.candidate].key;
        let left = key.expression.as_str().as_bytes();
        let right = candidate.expression.as_str().as_bytes();
        let end = left.len().min(right.len());
        search.common = prefix;
        let units = end
            .checked_sub(prefix)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow"))?;
        budget.charge(function, units, phase)?;
        search.probe.side = 'comparison: {
            while search.common < end {
                let a = left[search.common];
                let b = right[search.common];
                if a != b {
                    break 'comparison a.cmp(&b);
                }
                search.common += 1;
            }
            budget.charge(function, 2, phase)?;
            let order = left.len().cmp(&right.len());
            if order != Ordering::Equal {
                break 'comparison order;
            }
            budget.charge(function, 1, phase)?;
            key.lane.cmp(&candidate.lane)
        };
        if search.probe.side == Ordering::Equal {
            budget.charge(function, 1, phase)?;
            search.probe.found = true;
            return Ok(false);
        }
        // The complete comparison supplies the exact first differing expression
        // byte, prefix boundary, or lane. The mask is its highest differing bit.
        budget.charge(function, 24, phase)?;
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
        budget.charge(function, 1, phase)?;
        search.position = table.path.len();
        // Byte coordinates increase down a path; masks decrease within a byte.
        while search.position != 0 {
            budget.charge(function, VISIT_UNITS + 9, phase)?;
            search.candidate = table.path[search.position - 1].node;
            if table.nodes[search.candidate].byte < search.byte
                || (table.nodes[search.candidate].byte == search.byte
                    && table.nodes[search.candidate].mask > search.mask)
            {
                break;
            }
            search.position -= 1;
        }
        budget.charge(function, 12, phase)?;
        if search.position < table.path.len() {
            search.at = table.path[search.position].node;
            search.before = table.path[search.position].before;
            search.after = table.path[search.position].after;
        }
        if search.position != 0 {
            search.parent = table.path[search.position - 1].node;
            search.parent_one = table.path[search.position - 1].one;
        }
        budget.charge(function, INDEX_BIT_UNITS + 4, phase)?;
        search.new_one = index_bit!(search, key);
        if search.new_one {
            search.before = search.at;
        } else {
            search.after = search.at;
        }
        // Extremal leaves supply the exact full-order AVL neighbours.
        while search.before != NIL {
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            if table.nodes[search.before].byte == NIL {
                search.probe.before = table.nodes[search.before].zero;
                break;
            }
            search.before = table.nodes[search.before].one;
        }
        while search.after != NIL {
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            if table.nodes[search.after].byte == NIL {
                search.probe.after = table.nodes[search.after].zero;
                break;
            }
            search.after = table.nodes[search.after].zero;
        }
        budget.charge(function, 6, phase)?;
        if search.probe.before != NIL && directory.nodes[search.probe.before].right == NIL {
            search.probe.parent = search.probe.before;
            search.probe.side = Ordering::Greater;
        } else {
            search.probe.parent = search.probe.after;
            search.probe.side = Ordering::Less;
        }
        directory.grow(function, budget)?;
        budget.charge(function, VISIT_UNITS, phase)?;
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
        budget.reserve_bytes(
            function,
            2 * size_of::<IndexNode>() + 2 * size_of::<usize>(),
            phase,
        )?;
        budget.reserve_vec(function, &mut table.nodes, 2, phase)?;
        budget.charge(function, 18, phase)?;
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
        directory.rebalance(parent, function, budget)?;
        budget.charge(function, 1, phase)?;
        self.prefix = prefix;
        Ok(true)
    }
}

impl<'hir> HeldPrefix<'_, 'hir> {
    pub(super) fn insert_reusing_cursor(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        let phase = self.directory.phase;
        budget.charge(function, 2, phase)?;
        if self.directory.nodes.is_empty() {
            budget.charge(function, 3, phase)?;
            let prefix = key.expression.as_str().len();
            let inserted = self.directory.insert(function, key, budget)?;
            if inserted {
                self.index
                    .as_mut()
                    .unwrap()
                    .seed(0, function, budget, phase)?;
                self.prefix = prefix;
            }
            return Ok(inserted);
        }
        let prefix = self.common_prefix(function, key, budget)?;
        budget.charge(function, 3, phase)?;
        let table = self.index.as_mut().unwrap();
        let directory = &mut *self.directory;
        let search = table.cursor.as_mut().unwrap();
        // These fields live in the fully paid Box for this builder. Reset each
        // in place; no owning IndexSearch or Result<IndexSearch> crosses an ABI.
        budget.charge(function, 24, phase)?;
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
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            search.byte = table.nodes[search.at].byte;
            search.mask = table.nodes[search.at].mask;
            if search.byte == NIL {
                break;
            }
            budget.charge(function, INDEX_BIT_UNITS + 9, phase)?;
            search.step.node = search.at;
            search.step.one = index_bit!(search, key);
            search.step.before = search.before;
            search.step.after = search.after;
            budget.reserve_vec(function, &mut table.path, 1, phase)?;
            table.path.push(search.step);
            if search.step.one {
                search.before = table.nodes[search.at].zero;
                search.at = table.nodes[search.at].one;
            } else {
                search.after = table.nodes[search.at].one;
                search.at = table.nodes[search.at].zero;
            }
        }
        budget.charge(function, 3, phase)?;
        search.candidate = table.nodes[search.at].zero;
        // Inline the complete original comparison into the held scratch. The
        // actual borrowed/scalar locals remain newly paid on every invocation;
        // no Comparison, pair receipt or nested Result is constructed here.
        budget.reserve_bytes(
            function,
            size_of::<&StatusSourceId>()
                + 2 * size_of::<&[u8]>()
                + 2 * size_of::<usize>()
                + 2 * size_of::<u8>()
                + size_of::<Ordering>(),
            phase,
        )?;
        budget.charge(function, COMPARISON_UNITS, phase)?;
        let candidate = directory.nodes[search.candidate].key;
        let left = key.expression.as_str().as_bytes();
        let right = candidate.expression.as_str().as_bytes();
        let end = left.len().min(right.len());
        search.common = prefix;
        let units = end
            .checked_sub(prefix)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow"))?;
        budget.charge(function, units, phase)?;
        search.probe.side = 'comparison: {
            while search.common < end {
                let a = left[search.common];
                let b = right[search.common];
                if a != b {
                    break 'comparison a.cmp(&b);
                }
                search.common += 1;
            }
            budget.charge(function, 2, phase)?;
            let order = left.len().cmp(&right.len());
            if order != Ordering::Equal {
                break 'comparison order;
            }
            budget.charge(function, 1, phase)?;
            key.lane.cmp(&candidate.lane)
        };
        if search.probe.side == Ordering::Equal {
            budget.charge(function, 1, phase)?;
            search.probe.found = true;
            return Ok(false);
        }
        // The complete comparison supplies the exact first differing expression
        // byte, prefix boundary, or lane. The mask is its highest differing bit.
        budget.charge(function, 24, phase)?;
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
        budget.charge(function, 1, phase)?;
        search.position = table.path.len();
        // Byte coordinates increase down a path; masks decrease within a byte.
        while search.position != 0 {
            budget.charge(function, VISIT_UNITS + 9, phase)?;
            search.candidate = table.path[search.position - 1].node;
            if table.nodes[search.candidate].byte < search.byte
                || (table.nodes[search.candidate].byte == search.byte
                    && table.nodes[search.candidate].mask > search.mask)
            {
                break;
            }
            search.position -= 1;
        }
        budget.charge(function, 12, phase)?;
        if search.position < table.path.len() {
            search.at = table.path[search.position].node;
            search.before = table.path[search.position].before;
            search.after = table.path[search.position].after;
        }
        if search.position != 0 {
            search.parent = table.path[search.position - 1].node;
            search.parent_one = table.path[search.position - 1].one;
        }
        budget.charge(function, INDEX_BIT_UNITS + 4, phase)?;
        search.new_one = index_bit!(search, key);
        if search.new_one {
            search.before = search.at;
        } else {
            search.after = search.at;
        }
        // Extremal leaves supply the exact full-order AVL neighbours.
        while search.before != NIL {
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            if table.nodes[search.before].byte == NIL {
                search.probe.before = table.nodes[search.before].zero;
                break;
            }
            search.before = table.nodes[search.before].one;
        }
        while search.after != NIL {
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            if table.nodes[search.after].byte == NIL {
                search.probe.after = table.nodes[search.after].zero;
                break;
            }
            search.after = table.nodes[search.after].zero;
        }
        budget.charge(function, 6, phase)?;
        if search.probe.before != NIL && directory.nodes[search.probe.before].right == NIL {
            search.probe.parent = search.probe.before;
            search.probe.side = Ordering::Greater;
        } else {
            search.probe.parent = search.probe.after;
            search.probe.side = Ordering::Less;
        }
        directory.grow(function, budget)?;
        budget.charge(function, VISIT_UNITS, phase)?;
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
        budget.reserve_bytes(
            function,
            2 * size_of::<IndexNode>() + 2 * size_of::<usize>(),
            phase,
        )?;
        budget.reserve_vec(function, &mut table.nodes, 2, phase)?;
        budget.charge(function, 18, phase)?;
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
        directory.rebalance(parent, function, budget)?;
        budget.charge(function, 1, phase)?;
        self.prefix = prefix;
        Ok(true)
    }
}

#[cfg(test)]
mod tests;
