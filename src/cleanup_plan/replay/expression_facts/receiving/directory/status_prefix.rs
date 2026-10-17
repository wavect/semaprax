//! Status-only insertion over the exact held immutable key set. No lookup authority.
use super::{
    AllocationPayer, Diagnostic, Directory, NIL, Node, Ordering, Probe, ReplayBudget, ResolvedFunction,
    StatusLane, StatusSourceId, VISIT_UNITS, overflow,
};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::cell::Cell;
use std::mem::size_of;

#[derive(Clone, Copy)]
enum Refusal {
    Binding,
    Invocation,
    Budget,
}

// Held only during the actual current source-order insertion loop. Raw pointers
// are compared, never dereferenced; moving either receiver fails closed.
pub(in crate::cleanup_plan::replay::expression_facts::receiving) struct Owner<'hir> {
    function: &'hir ResolvedFunction,
    directory: *const Directory<&'hir StatusSourceId>,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    next: Cell<usize>,
    first: Cell<Option<Refusal>>,
}
impl<'hir> Owner<'hir> {
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn new(
        function: &'hir ResolvedFunction,
        directory: &Directory<&'hir StatusSourceId>,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_bytes(
            function,
            2 * size_of::<Self>() + size_of::<Result<Self, Diagnostic>>(),
            directory.phase,
        )?;
        budget.charge(function, 3, directory.phase)?;
        if !directory.nodes.is_empty() || directory.root != NIL || directory.first != NIL {
            return Err(budget.budget_refusal(
                function,
                "status prefix insertion requires a fresh current directory",
                "",
            ));
        }
        // Six actual initialized binding/control fields, one unit each.
        budget.charge(function, 6, directory.phase)?;
        Ok(Self {
            function,
            directory,
            budget,
            ledger: ClassifierLedger::capture(),
            next: Cell::new(0),
            first: Cell::new(None),
        })
    }
    fn refuse(
        &self,
        reason: Refusal,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Diagnostic {
        let selected = self.first.get().unwrap_or(reason);
        self.first.set(Some(selected));
        budget.budget_refusal(
            function,
            match selected {
                Refusal::Binding => {
                    "status prefix insertion belongs to another subject or receiver"
                }
                Refusal::Invocation => "status prefix insertion belongs to another invocation",
                Refusal::Budget => "selected status prefix insertion budget already failed",
            },
            "",
        )
    }
    fn authenticate(
        &self,
        directory: &Directory<&'hir StatusSourceId>,
        function: &ResolvedFunction,
        key: &StatusSourceId,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if let Some(first) = self.first.get() {
            return Err(self.refuse(first, function, budget));
        }
        if !std::ptr::eq(self.function, function)
            || !std::ptr::eq(self.directory, directory)
            || !std::ptr::eq(self.budget, budget)
        {
            return Err(self.refuse(Refusal::Binding, function, budget));
        }
        if !self.ledger.current() {
            return Err(self.refuse(Refusal::Invocation, function, budget));
        }
        budget.reserve_bytes(function, 0, directory.phase)?;
        budget.charge(function, 0, directory.phase)?;
        // The authorization receipt and the inner/outer insertion Results are
        // new receiving carriers. Fund them through this authenticated owner
        // before producing the successful receipt or entering the probe.
        budget.reserve_bytes(
            function,
            size_of::<Result<(), Diagnostic>>() + 2 * size_of::<Result<bool, Diagnostic>>(),
            directory.phase,
        )?;
        // One exact indexed current-row identity test; no textual ID parsing or
        // extra full-function scan precedes the original duplicate frontier.
        budget.charge(function, 1, directory.phase)?;
        if function
            .cleanup_plan
            .status_sources
            .get(self.next.get())
            .is_none_or(|source| !std::ptr::eq(&source.id, key))
        {
            return Err(self.refuse(Refusal::Binding, function, budget));
        }
        Ok(())
    }
}

struct Search {
    probe: Probe,
    at: usize,
    maximum: usize,
    lower: usize,
    upper: usize,
    prefix: usize,
}
struct Comparison<'a> {
    left: &'a [u8],
    right: &'a [u8],
    end: usize,
    common: usize,
}
// Existing expression-fact prefix comparator's source-derived ten bookkeeping
// operations and unchanged byte-pair/end rate. No prior local/header credit.
const COMPARISON_UNITS: usize = 10;
const COMPARISON_BYTES: usize = size_of::<Comparison<'static>>()
    + size_of::<Result<(Ordering, usize), Diagnostic>>()
    + size_of::<(Ordering, usize)>()
    + size_of::<Ordering>(); // Simultaneous complete-expression ordering local.
const SEARCH_BYTES: usize = 2 * size_of::<Search>()
    + size_of::<Result<Probe, Diagnostic>>()
    + size_of::<Probe>()
    + size_of::<&StatusSourceId>(); // Simultaneously received minimum key.

fn compare(
    function: &ResolvedFunction,
    query: &StatusSourceId,
    candidate: &StatusSourceId,
    prefix: usize,
    budget: &mut ReplayBudget,
    phase: &'static str,
) -> Result<(Ordering, usize), Diagnostic> {
    budget.reserve_bytes(function, COMPARISON_BYTES, phase)?;
    budget.charge(function, COMPARISON_UNITS, phase)?;
    let mut comparison = Comparison {
        left: query.expression.as_str().as_bytes(),
        right: candidate.expression.as_str().as_bytes(),
        end: query
            .expression
            .as_str()
            .len()
            .min(candidate.expression.as_str().len()),
        common: prefix,
    };
    let units = comparison
        .end
        .checked_sub(prefix)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| overflow(function, budget, "directory key comparison work overflow"))?;
    budget.charge(function, units, phase)?;
    while comparison.common < comparison.end {
        let a = comparison.left[comparison.common];
        let b = comparison.right[comparison.common];
        if a != b {
            return Ok((a.cmp(&b), comparison.common));
        }
        comparison.common += 1;
    }
    // New scalar ordering receipt and equal-expression branch, before either.
    budget.charge(function, 2, phase)?;
    let order = comparison.left.len().cmp(&comparison.right.len());
    if order != Ordering::Equal {
        return Ok((order, comparison.common));
    }
    budget.charge(function, 1, phase)?;
    Ok((query.lane.cmp(&candidate.lane), comparison.common))
}

impl<'hir> Directory<&'hir StatusSourceId> {
    fn certified_probe(
        &self,
        function: &ResolvedFunction,
        key: &StatusSourceId,
        budget: &mut ReplayBudget,
    ) -> Result<Probe, Diagnostic> {
        budget.reserve_bytes(function, SEARCH_BYTES, self.phase)?;
        // Six Search fields plus five nested Probe fields, one unit each.
        budget.charge(function, 11, self.phase)?;
        let mut search = Search {
            probe: Probe {
                found: false,
                parent: NIL,
                before: NIL,
                after: NIL,
                side: Ordering::Equal,
            },
            at: self.root,
            maximum: self.root,
            lower: 0,
            upper: 0,
            prefix: 0,
        };
        budget.charge(function, VISIT_UNITS, self.phase)?;
        let minimum = self.nodes[self.first].key;
        budget.charge(function, 1, self.phase)?;
        search.lower = compare(function, key, minimum, 0, budget, self.phase)?.1;
        loop {
            budget.charge(function, VISIT_UNITS, self.phase)?;
            let node = &self.nodes[search.maximum];
            if node.right == NIL {
                budget.charge(function, 1, self.phase)?;
                search.upper = compare(function, key, node.key, 0, budget, self.phase)?.1;
                break;
            }
            budget.charge(function, 1, self.phase)?;
            search.maximum = node.right;
        }
        budget.charge(function, 1, self.phase)?;
        search.prefix = search.lower.min(search.upper);
        // All stored expression keys lie between these real min/max keys.
        // Byte strings sharing a prefix form a contiguous ordered interval.
        // Lane is secondary, so every held key shares these certified bytes.
        while search.at != NIL {
            budget.charge(function, VISIT_UNITS, self.phase)?;
            let node = &self.nodes[search.at];
            budget.charge(function, 1, self.phase)?;
            search.probe.side =
                compare(function, key, node.key, search.prefix, budget, self.phase)?.0;
            if search.probe.side == Ordering::Equal {
                budget.charge(function, 1, self.phase)?;
                search.probe.found = true;
                return Ok(search.probe);
            }
            budget.charge(function, 3, self.phase)?;
            search.probe.parent = search.at;
            if search.probe.side == Ordering::Less {
                search.probe.after = search.at;
                search.at = node.left;
            } else {
                search.probe.before = search.at;
                search.at = node.right;
            }
        }
        Ok(search.probe)
    }
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn insert_certified(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
        owner: &Owner<'hir>,
    ) -> Result<bool, Diagnostic> {
        let result = (|| {
            owner.authenticate(self, function, key, budget)?;
            // No repeated-prefix opportunity exists with fewer than two keys.
            budget.charge(function, 1, self.phase)?;
            if self.nodes.len() < 2 {
                return self.insert(function, key, budget);
            }
            self.authenticate(function, budget)?;
            let probe = self.certified_probe(function, key, budget)?;
            if probe.found {
                return Ok(false);
            }
            // Exact unchanged append/grow/link/rebalance sequence from insert.
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
        })();
        match result {
            Ok(inserted) => {
                budget.charge(function, 1, self.phase).inspect_err(|_| {
                    owner.first.set(Some(Refusal::Budget));
                })?;
                owner.next.set(owner.next.get() + 1);
                Ok(inserted)
            }
            Err(error) => {
                if owner.first.get().is_none() {
                    owner.first.set(Some(Refusal::Budget));
                }
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests;

// This builder holds the only mutable directory borrow for its insertion loop.
// Its prefix is derived from exactly the keys that loop has inserted; it never
// authenticates a lookup or survives publication of the immutable directory.
pub(in crate::cleanup_plan::replay::expression_facts::receiving) struct HeldPrefix<'directory, 'hir>
{
    directory: &'directory mut Directory<&'hir StatusSourceId>,
    owner: Owner<'hir>,
    prefix: usize,
    index: Option<StatusIndex>,
}
struct HeldSearch {
    probe: Probe,
    at: usize,
}
const HELD_SEARCH_BYTES: usize = 2 * size_of::<HeldSearch>()
    + size_of::<Result<Probe, Diagnostic>>()
    + size_of::<Probe>()
    + size_of::<usize>();
const HELD_COMPARISON_BYTES: usize = size_of::<Comparison<'static>>()
    + size_of::<Result<usize, Diagnostic>>()
    + 2 * size_of::<usize>()
    + size_of::<bool>()
    + size_of::<&StatusSourceId>();

impl<'hir> Directory<&'hir StatusSourceId> {
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn held_prefix<'directory>(
        &'directory mut self,
        function: &'hir ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<HeldPrefix<'directory, 'hir>, Diagnostic> {
        budget.reserve_bytes(
            function,
            2 * size_of::<HeldPrefix<'_, '_>>()
                + size_of::<Result<HeldPrefix<'_, '_>, Diagnostic>>(),
            self.phase,
        )?;
        // Existing bindings plus the storage-index selector and empty header.
        budget.charge(function, 16, self.phase)?;
        let owner = Owner::new(function, self, budget)?;
        Ok(HeldPrefix {
            directory: self,
            owner,
            prefix: 0,
            index: (!budget.selected()
                && matches!(
                    budget.payer,
                    AllocationPayer::ActiveBuilder | AllocationPayer::LegacyUnobserved
                )
                && function.cleanup_plan.status_sources.len() >= 256)
                .then(StatusIndex::default),
        })
    }

    fn held_probe(
        &self,
        function: &ResolvedFunction,
        key: &StatusSourceId,
        prefix: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Probe, Diagnostic> {
        budget.reserve_bytes(function, HELD_SEARCH_BYTES, self.phase)?;
        // Two search fields, five nested probe fields, one received prefix.
        budget.charge(function, 8, self.phase)?;
        let mut search = HeldSearch {
            probe: Probe {
                found: false,
                parent: NIL,
                before: NIL,
                after: NIL,
                side: Ordering::Equal,
            },
            at: self.root,
        };
        while search.at != NIL {
            budget.charge(function, VISIT_UNITS, self.phase)?;
            let node = &self.nodes[search.at];
            budget.charge(function, 1, self.phase)?;
            search.probe.side = compare(function, key, node.key, prefix, budget, self.phase)?.0;
            if search.probe.side == Ordering::Equal {
                budget.charge(function, 1, self.phase)?;
                search.probe.found = true;
                return Ok(search.probe);
            }
            budget.charge(function, 3, self.phase)?;
            search.probe.parent = search.at;
            if search.probe.side == Ordering::Less {
                search.probe.after = search.at;
                search.at = node.left;
            } else {
                search.probe.before = search.at;
                search.at = node.right;
            }
        }
        Ok(search.probe)
    }
}

impl<'hir> HeldPrefix<'_, 'hir> {
    fn common_prefix(
        &self,
        function: &ResolvedFunction,
        key: &StatusSourceId,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let phase = self.directory.phase;
        budget.reserve_bytes(function, HELD_COMPARISON_BYTES, phase)?;
        budget.charge(function, VISIT_UNITS, phase)?;
        let minimum = self.directory.nodes[self.directory.first].key;
        // The existing Comparison setup plus one clamp to the held bound.
        budget.charge(function, COMPARISON_UNITS + 1, phase)?;
        let mut comparison = Comparison {
            left: key.expression.as_str().as_bytes(),
            right: minimum.expression.as_str().as_bytes(),
            end: key
                .expression
                .as_str()
                .len()
                .min(minimum.expression.as_str().len())
                .min(self.prefix),
            common: 0,
        };
        let units = comparison.end.checked_add(1).ok_or_else(|| {
            overflow(
                function,
                budget,
                "held status prefix comparison work overflow",
            )
        })?;
        budget.charge(function, units, phase)?;
        while comparison.common < comparison.end {
            if comparison.left[comparison.common] != comparison.right[comparison.common] {
                return Ok(comparison.common);
            }
            comparison.common += 1;
        }
        Ok(comparison.common)
    }

    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: &'hir StatusSourceId,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        let phase = self.directory.phase;
        let result = (|| {
            self.owner
                .authenticate(self.directory, function, key, budget)?;
            self.directory.authenticate(function, budget)?;
            budget.charge(function, 1, phase)?;
            budget.charge(function, 1, phase)?;
            if self.index.is_some() {
                return self.insert_indexed(function, key, budget);
            }
            if self.directory.nodes.is_empty() {
                budget.reserve_bytes(function, size_of::<usize>(), phase)?;
                budget.charge(function, 2, phase)?;
                let prefix = key.expression.as_str().len();
                let inserted = self.directory.insert(function, key, budget)?;
                budget.charge(function, 1, phase)?;
                if inserted {
                    self.prefix = prefix;
                }
                return Ok(inserted);
            }
            let prefix = self.common_prefix(function, key, budget)?;
            let probe = self.directory.held_probe(function, key, prefix, budget)?;
            if probe.found {
                return Ok(false);
            }
            self.directory.grow(function, budget)?;
            budget.charge(function, VISIT_UNITS, self.directory.phase)?;
            let index = self.directory.nodes.len();
            self.directory.nodes.push(Node {
                key,
                left: NIL,
                right: NIL,
                parent: probe.parent,
                next: probe.after,
                height: 1,
            });
            if probe.before == NIL {
                self.directory.first = index;
            } else {
                self.directory.nodes[probe.before].next = index;
            }
            if probe.parent == NIL {
                self.directory.root = index;
            } else if probe.side == Ordering::Less {
                self.directory.nodes[probe.parent].left = index;
            } else {
                self.directory.nodes[probe.parent].right = index;
            }
            self.directory.rebalance(probe.parent, function, budget)?;
            budget.charge(function, 1, phase)?;
            self.prefix = prefix;
            Ok(true)
        })();
        match result {
            Ok(inserted) => {
                budget.charge(function, 1, phase).inspect_err(|_| {
                    self.owner.first.set(Some(Refusal::Budget));
                })?;
                self.owner.next.set(self.owner.next.get() + 1);
                Ok(inserted)
            }
            Err(error) => {
                if self.owner.first.get().is_none() {
                    self.owner.first.set(Some(Refusal::Budget));
                }
                Err(error)
            }
        }
    }
}

// Private construction coordinates; the held AVL retains every complete key.
pub(super) struct IndexNode {
    pub(super) byte: usize,
    pub(super) mask: u16,
    pub(super) zero: usize,
    pub(super) one: usize,
}
#[derive(Clone, Copy)]
struct IndexStep {
    node: usize,
    one: bool,
    before: usize,
    after: usize,
}
#[derive(Default)]
struct StatusIndex {
    nodes: Vec<IndexNode>,
    path: Vec<IndexStep>,
    root: Option<usize>,
}
struct IndexSearch {
    probe: Probe,
    at: usize,
    parent: usize,
    parent_one: bool,
    new_one: bool,
    before: usize,
    after: usize,
    byte: usize,
    mask: u16,
    symbol: u16,
    direction: bool,
    common: usize,
    candidate: usize,
    position: usize,
    step: IndexStep,
}
// Complete caller/callee headers, receipt, returned Result and simultaneous
// scalar/borrowed comparison receivers. All bit-reader state lives in Search.
const INDEX_SEARCH_BYTES: usize = 2 * size_of::<IndexSearch>()
    + size_of::<Result<IndexSearch, Diagnostic>>()
    + 2 * size_of::<(
        &mut StatusIndex,
        &Directory<&StatusSourceId>,
        &ResolvedFunction,
        &StatusSourceId,
        usize,
        &mut ReplayBudget,
    )>()
    + size_of::<&str>()
    + 3 * size_of::<usize>()
    + 3 * size_of::<u16>()
    + 3 * size_of::<&StatusSourceId>();
const INDEX_BIT_UNITS: usize = 12;
// Real bytes have a leading marker bit (256); the end symbol is the complete
// lane discriminator. Thus prefixes, embedded NUL and UTF-8 retain full Ord.
// Each node stores the byte coordinate and mask directly, without decoding a
// packed bit ordinal or parsing any identifier syntax.
// An exhaustive mapping keeps the indexed alphabet closed when lanes evolve.
macro_rules! index_lane {
    ($lane:expr) => {
        match $lane {
            StatusLane::OwnerAdmission => 0u8,
            StatusLane::OperationFailure => 1u8,
            StatusLane::AllocationFailure => 2u8,
            StatusLane::ContractFalse => 3u8,
        }
    };
}
macro_rules! index_bit {
    ($search:ident, $key:ident) => {{
        $search.symbol = if $search.byte < $key.expression.as_str().len() {
            256 | $key.expression.as_str().as_bytes()[$search.byte] as u16
        } else if $search.byte == $key.expression.as_str().len() {
            index_lane!($key.lane) as u16
        } else {
            0
        };
        $search.direction = $search.symbol & $search.mask != 0;
        $search.direction
    }};
}
impl StatusIndex {
    fn seed(
        &mut self,
        index: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        phase: &'static str,
    ) -> Result<(), Diagnostic> {
        budget.reserve_bytes(
            function,
            2 * size_of::<(
                &mut StatusIndex,
                usize,
                &ResolvedFunction,
                &mut ReplayBudget,
                &str,
            )>() + size_of::<IndexNode>()
                + size_of::<Result<(), Diagnostic>>(),
            phase,
        )?;
        budget.reserve_vec(function, &mut self.nodes, 1, phase)?;
        budget.charge(function, 7, phase)?;
        self.nodes.push(IndexNode {
            byte: NIL,
            mask: 0,
            zero: index,
            one: NIL,
        });
        self.root = Some(0);
        Ok(())
    }
    fn probe(
        &mut self,
        directory: &Directory<&StatusSourceId>,
        function: &ResolvedFunction,
        key: &StatusSourceId,
        prefix: usize,
        budget: &mut ReplayBudget,
    ) -> Result<IndexSearch, Diagnostic> {
        let phase = directory.phase;
        budget.reserve_bytes(function, INDEX_SEARCH_BYTES, phase)?;
        budget.charge(function, 24, phase)?;
        let mut search = IndexSearch {
            probe: Probe {
                found: false,
                parent: NIL,
                before: NIL,
                after: NIL,
                side: Ordering::Equal,
            },
            at: self
                .root
                .expect("held status index seeded with the first AVL key"),
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
        self.path.clear();
        // Preserve the visited construction path, reusing its fully paid backing.
        // It supplies the insertion parent and ordered bounds after one full-key
        // comparison, so no second pass reads query bytes.
        loop {
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            search.byte = self.nodes[search.at].byte;
            search.mask = self.nodes[search.at].mask;
            if search.byte == NIL {
                break;
            }
            budget.charge(function, INDEX_BIT_UNITS + 9, phase)?;
            search.step.node = search.at;
            search.step.one = index_bit!(search, key);
            search.step.before = search.before;
            search.step.after = search.after;
            budget.reserve_vec(function, &mut self.path, 1, phase)?;
            self.path.push(search.step);
            if search.step.one {
                search.before = self.nodes[search.at].zero;
                search.at = self.nodes[search.at].one;
            } else {
                search.after = self.nodes[search.at].one;
                search.at = self.nodes[search.at].zero;
            }
        }
        budget.charge(function, 3, phase)?;
        search.candidate = self.nodes[search.at].zero;
        (search.probe.side, search.common) = compare(
            function,
            key,
            directory.nodes[search.candidate].key,
            prefix,
            budget,
            phase,
        )?;
        if search.probe.side == Ordering::Equal {
            budget.charge(function, 1, phase)?;
            search.probe.found = true;
            return Ok(search);
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
        search.position = self.path.len();
        // Byte coordinates increase down a path; masks decrease within a byte.
        while search.position != 0 {
            budget.charge(function, VISIT_UNITS + 9, phase)?;
            search.candidate = self.path[search.position - 1].node;
            if self.nodes[search.candidate].byte < search.byte
                || (self.nodes[search.candidate].byte == search.byte
                    && self.nodes[search.candidate].mask > search.mask)
            {
                break;
            }
            search.position -= 1;
        }
        budget.charge(function, 12, phase)?;
        if search.position < self.path.len() {
            search.at = self.path[search.position].node;
            search.before = self.path[search.position].before;
            search.after = self.path[search.position].after;
        }
        if search.position != 0 {
            search.parent = self.path[search.position - 1].node;
            search.parent_one = self.path[search.position - 1].one;
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
            if self.nodes[search.before].byte == NIL {
                search.probe.before = self.nodes[search.before].zero;
                break;
            }
            search.before = self.nodes[search.before].one;
        }
        while search.after != NIL {
            budget.charge(function, VISIT_UNITS + 2, phase)?;
            if self.nodes[search.after].byte == NIL {
                search.probe.after = self.nodes[search.after].zero;
                break;
            }
            search.after = self.nodes[search.after].zero;
        }
        budget.charge(function, 6, phase)?;
        if search.probe.before != NIL && directory.nodes[search.probe.before].right == NIL {
            search.probe.parent = search.probe.before;
            search.probe.side = Ordering::Greater;
        } else {
            search.probe.parent = search.probe.after;
            search.probe.side = Ordering::Less;
        }
        Ok(search)
    }
    fn commit(
        &mut self,
        search: &IndexSearch,
        index: usize,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        phase: &'static str,
    ) -> Result<(), Diagnostic> {
        budget.reserve_bytes(
            function,
            2 * size_of::<(
                &mut StatusIndex,
                &IndexSearch,
                usize,
                &ResolvedFunction,
                &mut ReplayBudget,
                &str,
            )>() + 2 * size_of::<IndexNode>()
                + size_of::<Result<(), Diagnostic>>()
                + 2 * size_of::<usize>(),
            phase,
        )?;
        budget.reserve_vec(function, &mut self.nodes, 2, phase)?;
        budget.charge(function, 18, phase)?;
        let leaf = self.nodes.len();
        let branch = leaf + 1;
        self.nodes.push(IndexNode {
            byte: NIL,
            mask: 0,
            zero: index,
            one: NIL,
        });
        self.nodes.push(IndexNode {
            byte: search.byte,
            mask: search.mask,
            zero: if search.new_one { search.at } else { leaf },
            one: if search.new_one { leaf } else { search.at },
        });
        if search.parent == NIL {
            self.root = Some(branch);
        } else if search.parent_one {
            self.nodes[search.parent].one = branch;
        } else {
            self.nodes[search.parent].zero = branch;
        }
        Ok(())
    }
}

impl<'hir> HeldPrefix<'_, 'hir> {
    fn insert_indexed(
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
                + size_of::<bool>(),
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
        let search =
            self.index
                .as_mut()
                .unwrap()
                .probe(self.directory, function, key, prefix, budget)?;
        if search.probe.found {
            return Ok(false);
        }
        self.directory.grow(function, budget)?;
        budget.charge(function, VISIT_UNITS, phase)?;
        let index = self.directory.nodes.len();
        self.directory.nodes.push(Node {
            key,
            left: NIL,
            right: NIL,
            parent: search.probe.parent,
            next: search.probe.after,
            height: 1,
        });
        if search.probe.before == NIL {
            self.directory.first = index;
        } else {
            self.directory.nodes[search.probe.before].next = index;
        }
        if search.probe.parent == NIL {
            self.directory.root = index;
        } else if search.probe.side == Ordering::Less {
            self.directory.nodes[search.probe.parent].left = index;
        } else {
            self.directory.nodes[search.probe.parent].right = index;
        }
        let parent = search.probe.parent;
        self.index
            .as_mut()
            .unwrap()
            .commit(&search, index, function, budget, phase)?;
        self.directory.rebalance(parent, function, budget)?;
        budget.charge(function, 1, phase)?;
        self.prefix = prefix;
        Ok(true)
    }
}

// Published only after every source row and producer has passed the original
// receiving checks. Keys remain immutable in the owning AVL directory.
pub(super) struct FrozenStatusIndex {
    pub(super) nodes: Vec<IndexNode>,
    pub(super) root: usize,
    pub(super) function: *const ResolvedFunction,
    pub(super) budget: *const ReplayBudget,
    pub(super) ledger: ClassifierLedger,
    pub(super) keys: usize,
    pub(super) len: usize,
    pub(super) failed: Cell<bool>,
}
impl HeldPrefix<'_, '_> {
    pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn freeze(
        self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.owner.first.get().is_some()
            || !std::ptr::eq(self.owner.function, function)
            || !std::ptr::eq(self.owner.budget, budget)
            || !std::ptr::eq(self.owner.directory, self.directory)
            || !self.owner.ledger.current()
        {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        self.directory.authenticate(function, budget)?;
        // Complete consumed/receiving builder, fixed publication controls,
        // initialized snapshot and separately allocated Box backing and pointer.
        budget.reserve_bytes(
            function,
            2 * size_of::<Self>()
                + 2 * size_of::<FrozenStatusIndex>()
                + size_of::<Result<(), Diagnostic>>()
                + 2 * size_of::<Option<Box<FrozenStatusIndex>>>()
                + size_of::<(&ResolvedFunction, &mut ReplayBudget)>(),
            self.directory.phase,
        )?;
        budget.charge(function, 24, self.directory.phase)?;
        if self.owner.next.get() != function.cleanup_plan.status_sources.len()
            || self.directory.nodes.len() != function.cleanup_plan.status_sources.len()
        {
            return Err(replay_error(
                function,
                "status index publication requires every current row",
            ));
        }
        // Retention requires an actual ACTIVE invocation. The unobserved,
        // leased oracle and selected physical routes retain their old lookup.
        if !matches!(budget.payer, AllocationPayer::ActiveBuilder) {
            return Ok(());
        }
        let Some(index) = self.index else {
            return Ok(());
        };
        let frozen = FrozenStatusIndex {
            nodes: index.nodes,
            root: index.root.expect("complete nonempty status index"),
            function,
            budget,
            ledger: ClassifierLedger::capture(),
            keys: self.directory.nodes.as_ptr() as usize,
            len: self.directory.nodes.len(),
            failed: Cell::new(false),
        };
        self.directory.lookup = Some(Box::new(frozen));
        Ok(())
    }
}
