//! Private search scratch borrowed from one exact immutable function/arena.
use super::*;
use std::mem::size_of;

struct Bytes<'a> {
    left: &'a [u8],
    right: &'a [u8],
    end: usize,
    common: usize,
}
struct Compared<'a> {
    bytes: Bytes<'a>,
    order: Ordering,
    units: usize,
    a: u8,
    b: u8,
}
struct Control {
    lower: usize,
    upper: usize,
    at: usize,
    last: usize,
}
/// The mutable budget loan prevents foreign/replaced budgets and nested use.
/// Arena/function loans prevent replacement and outliving either source.
pub(in crate::cleanup_plan::replay::expression_facts) struct Session<'arena, 'hir, 'budget> {
    arena: &'arena Arena<'hir>,
    function: &'arena ResolvedFunction,
    pub(in crate::cleanup_plan::replay::expression_facts) budget: &'budget mut ReplayBudget,
    control: Control,
    compared: Compared<'arena>,
}
/// The paid prefix and its actual mutable payer loan have one owner.
pub(in crate::cleanup_plan::replay::expression_facts) struct PrefixSession<'arena, 'hir, 'budget> {
    arena: &'arena Arena<'hir>,
    function: &'arena ResolvedFunction,
    pub(in crate::cleanup_plan::replay::expression_facts) budget: &'budget mut ReplayBudget,
    control: Control,
    compared: Compared<'arena>,
    prefix: usize,
}
/// An immutable comparison input; the session owns and funds its own prefix.
pub(in crate::cleanup_plan::replay::expression_facts) struct PrefixView<'arena, 'hir> {
    arena: &'arena Arena<'hir>,
    function: &'arena ResolvedFunction,
    bytes: usize,
}
// This is the unchanged compound lookup ABI, including BOTH old Result terms.
const RESULTS: usize =
    2 * size_of::<Result<Option<&'static Option<CallFact<'static>>>, Diagnostic>>();
impl<'arena, 'hir, 'budget> Session<'arena, 'hir, 'budget> {
    pub(in crate::cleanup_plan::replay::expression_facts) const OLD_SCRATCH: usize =
        prefix::RETAINABLE_BYTES;
    // New selector's existing Arena::len call: outer Arena receiver pair and
    // scalar return/receive, plus its inner Vec<Node>::len receiver pair and
    // scalar return/receive. Node remains private to its actual owning module.
    pub(in crate::cleanup_plan::replay::expression_facts) const fn arena_len_bytes() -> usize {
        2 * size_of::<&Arena<'_>>() + 2 * size_of::<&Vec<Node<'_>>>() + 4 * size_of::<usize>()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn acquisition_bytes() -> usize {
        // Full initialized/receiving owner, nested field initializers,
        // constructor args, acquisition payment/work actual/formal frames and
        // Results, and two scalar acquisition receivers. Drop has only
        // trivial references/scalars.
        // The ordinary caller separately pays its continuation and selection.
        2 * size_of::<Self>()
            + 2 * size_of::<Control>()
            + 2 * size_of::<Compared<'_>>()
            + 2 * size_of::<Bytes<'_>>()
            + 2 * size_of::<(&Arena<'_>, &ResolvedFunction, &mut ReplayBudget)>()
            + 4 * size_of::<Result<(), Diagnostic>>()
            + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 2 * size_of::<usize>()
    }
    /// Called only after the caller's old zero-byte/work authentication and
    /// complete acquisition reservation, at the first actual lookup frontier.
    pub(in crate::cleanup_plan::replay::expression_facts) fn new_prepaid(
        arena: &'arena Arena<'hir>,
        function: &'arena ResolvedFunction,
        budget: &'budget mut ReplayBudget,
    ) -> Self {
        Self {
            arena,
            function,
            budget,
            control: Control {
                lower: 0,
                upper: 0,
                at: NIL,
                last: NIL,
            },
            compared: Compared {
                bytes: Bytes {
                    left: &[],
                    right: &[],
                    end: 0,
                    common: 0,
                },
                order: Ordering::Equal,
                units: 0,
                a: 0,
                b: 0,
            },
        }
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn query_bytes() -> usize {
        2 * size_of::<&mut Self>()
            + size_of::<(
                &Arena<'_>,
                &ResolvedFunction,
                &mut ReplayBudget,
                &mut Control,
                &mut Compared<'_>,
            )>()
            + 2 * size_of::<&mut ReplayBudget>()
            + size_of::<&crate::cleanup_plan::StatusSource>()
            + 2 * size_of::<bool>()
    }
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "complete immediate-old returning lookup ABI retained for legacy/control composition"
        )
    )]
    pub(in crate::cleanup_plan::replay::expression_facts) fn fact(
        &mut self,
        query: &'arena ExpressionId,
    ) -> Result<Option<&'arena Option<CallFact<'hir>>>, Diagnostic> {
        // The sole production caller preserves successful HeldPrefix::insert
        // immediately before this query. That existing guard checks the exact
        // function/budget/current ledger on EVERY row; the closed continuation
        // cannot switch ACTIVE or invoke a callback between guard and query.
        self.budget.reserve_bytes(
            self.function,
            0,
            "expression-fact arena lookup authentication",
        )?;
        self.budget.charge(
            self.function,
            0,
            "expression-fact arena lookup authentication",
        )?;
        // Both old compound Result terms and every additional query borrower
        // remain paid on each ACTUAL query. No future iteration is precharged.
        self.budget.reserve_bytes(
            self.function,
            RESULTS + Self::query_bytes(),
            "retained expression-fact query result controls",
        )?;
        // Original ordered work and complete query reset. No verdict is reused. Slice loans remain bound to the held immutable
        // function/arena; every field is reset before the next query.
        self.budget.charge(
            self.function,
            4,
            "expression-fact lookup control initialization",
        )?;
        self.control.lower = 0;
        self.control.upper = 0;
        self.control.at = self.arena.root;
        self.control.last = self.arena.root;
        self.budget.charge(
            self.function,
            8,
            "expression-fact receiving state initialization",
        )?;
        self.compared.bytes.left = query.as_str().as_bytes();
        self.compared.bytes.right = &[];
        self.compared.bytes.end = 0;
        self.compared.bytes.common = 0;
        self.compared.order = Ordering::Equal;
        self.compared.units = 0;
        self.compared.a = 0;
        self.compared.b = 0;
        let arena = self.arena;
        let function = self.function;
        let budget = &mut *self.budget;
        let control = &mut self.control;
        let compared = &mut self.compared;
        macro_rules! compare_into {
            ($candidate:expr, $prefix:expr) => {{
                budget.charge(function, 10, "expression-fact prefix control bookkeeping")?;
                compared.bytes.right = ($candidate).as_str().as_bytes();
                compared.bytes.end = compared.bytes.left.len().min(compared.bytes.right.len());
                compared.bytes.common = $prefix;
                compared.units = compared
                    .bytes
                    .end
                    .checked_sub(compared.bytes.common)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| {
                        budget.budget_refusal(
                            function,
                            "expression-fact prefix comparison work overflow",
                            "",
                        )
                    })?;
                budget.charge(
                    function,
                    compared.units,
                    "expression-fact certified suffix lookup",
                )?;
                loop {
                    if compared.bytes.common == compared.bytes.end {
                        compared.order = compared.bytes.left.len().cmp(&compared.bytes.right.len());
                        break;
                    }
                    compared.a = compared.bytes.left[compared.bytes.common];
                    compared.b = compared.bytes.right[compared.bytes.common];
                    if compared.a != compared.b {
                        compared.order = compared.a.cmp(&compared.b);
                        break;
                    }
                    compared.bytes.common += 1;
                }
            }};
        }
        if arena.nodes.len() >= 16 {
            budget.charge(function, 3, "expression-fact minimum key access")?;
            let first = &arena.nodes[arena.first];
            compare_into!(&first.key, 0);
            match compared.order {
                Ordering::Less => return Ok(None),
                Ordering::Equal => return Ok(Some(&first.fact)),
                Ordering::Greater => control.lower = compared.bytes.common,
            }
            loop {
                budget.charge(function, 3, "expression-fact maximum key access")?;
                let node = &arena.nodes[control.last];
                if node.right == NIL {
                    compare_into!(&node.key, 0);
                    match compared.order {
                        Ordering::Greater => return Ok(None),
                        Ordering::Equal => return Ok(Some(&node.fact)),
                        Ordering::Less => control.upper = compared.bytes.common,
                    }
                    break;
                }
                control.last = node.right;
            }
        }
        while control.at != NIL {
            let prefix = control.lower.min(control.upper);
            budget.charge(function, 3, "expression-fact certified node access")?;
            let node = &arena.nodes[control.at];
            compare_into!(&node.key, prefix);
            match compared.order {
                Ordering::Equal => return Ok(Some(&node.fact)),
                Ordering::Less => {
                    control.upper = compared.bytes.common;
                    control.at = node.left;
                }
                Ordering::Greater => {
                    control.lower = compared.bytes.common;
                    control.at = node.right;
                }
            }
        }
        Ok(None)
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const EDGE_QUERY_BYTES: usize = 2
        * size_of::<(&mut Self, &ExpressionId)>()
        + size_of::<(
            &Arena<'_>,
            &ResolvedFunction,
            &mut ReplayBudget,
            &mut Control,
            &mut Compared<'_>,
        )>()
        + 2 * size_of::<&mut ReplayBudget>();
    pub(in crate::cleanup_plan::replay::expression_facts) const EDGE_RECEIVING_BYTES: usize =
        RESULTS + Self::EDGE_QUERY_BYTES;
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "complete immediate-old returning lookup ABI retained for legacy/control composition"
        )
    )]
    pub(in crate::cleanup_plan::replay::expression_facts) fn fact_edge_prepaid(
        &mut self,
        query: &'arena ExpressionId,
    ) -> Result<Option<&'arena Option<CallFact<'hir>>>, Diagnostic> {
        // Only the same-ABI closed initial-edge continuation calls this method.
        // Original frozen-directory guards execute first; immutable function and
        // arena loans plus exclusive budget custody span every query. No callback
        // or ACTIVE switch occurs before this owner is dropped.
        // Original ordered work and complete query reset. No verdict is reused. Slice loans remain bound to the held immutable
        // function/arena; every field is reset before the next query.
        self.budget.charge(
            self.function,
            4,
            "expression-fact lookup control initialization",
        )?;
        self.control.lower = 0;
        self.control.upper = 0;
        self.control.at = self.arena.root;
        self.control.last = self.arena.root;
        self.budget.charge(
            self.function,
            8,
            "expression-fact receiving state initialization",
        )?;
        self.compared.bytes.left = query.as_str().as_bytes();
        self.compared.bytes.right = &[];
        self.compared.bytes.end = 0;
        self.compared.bytes.common = 0;
        self.compared.order = Ordering::Equal;
        self.compared.units = 0;
        self.compared.a = 0;
        self.compared.b = 0;
        let arena = self.arena;
        let function = self.function;
        let budget = &mut *self.budget;
        let control = &mut self.control;
        let compared = &mut self.compared;
        macro_rules! compare_into {
            ($candidate:expr, $prefix:expr) => {{
                budget.charge(function, 10, "expression-fact prefix control bookkeeping")?;
                compared.bytes.right = ($candidate).as_str().as_bytes();
                compared.bytes.end = compared.bytes.left.len().min(compared.bytes.right.len());
                compared.bytes.common = $prefix;
                compared.units = compared
                    .bytes
                    .end
                    .checked_sub(compared.bytes.common)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| {
                        budget.budget_refusal(
                            function,
                            "expression-fact prefix comparison work overflow",
                            "",
                        )
                    })?;
                budget.charge(
                    function,
                    compared.units,
                    "expression-fact certified suffix lookup",
                )?;
                loop {
                    if compared.bytes.common == compared.bytes.end {
                        compared.order = compared.bytes.left.len().cmp(&compared.bytes.right.len());
                        break;
                    }
                    compared.a = compared.bytes.left[compared.bytes.common];
                    compared.b = compared.bytes.right[compared.bytes.common];
                    if compared.a != compared.b {
                        compared.order = compared.a.cmp(&compared.b);
                        break;
                    }
                    compared.bytes.common += 1;
                }
            }};
        }
        if arena.nodes.len() >= 16 {
            budget.charge(function, 3, "expression-fact minimum key access")?;
            let first = &arena.nodes[arena.first];
            compare_into!(&first.key, 0);
            match compared.order {
                Ordering::Less => return Ok(None),
                Ordering::Equal => return Ok(Some(&first.fact)),
                Ordering::Greater => control.lower = compared.bytes.common,
            }
            loop {
                budget.charge(function, 3, "expression-fact maximum key access")?;
                let node = &arena.nodes[control.last];
                if node.right == NIL {
                    compare_into!(&node.key, 0);
                    match compared.order {
                        Ordering::Greater => return Ok(None),
                        Ordering::Equal => return Ok(Some(&node.fact)),
                        Ordering::Less => control.upper = compared.bytes.common,
                    }
                    break;
                }
                control.last = node.right;
            }
        }
        while control.at != NIL {
            let prefix = control.lower.min(control.upper);
            budget.charge(function, 3, "expression-fact certified node access")?;
            let node = &arena.nodes[control.at];
            compare_into!(&node.key, prefix);
            match compared.order {
                Ordering::Equal => return Ok(Some(&node.fact)),
                Ordering::Less => {
                    control.upper = compared.bytes.common;
                    control.at = node.left;
                }
                Ordering::Greater => {
                    control.lower = compared.bytes.common;
                    control.at = node.right;
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay::expression_facts) mod tests;

#[path = "retained_into.rs"]
mod into;

#[cfg(test)]
mod into_tests;

/// A paid common-prefix certificate for one complete immutable ordered arena.
/// Full keys and payloads remain in that arena; no query verdict is retained.
pub(in crate::cleanup_plan::replay::expression_facts) struct PrefixWitness<'arena, 'hir, 'budget> {
    arena: &'arena Arena<'hir>,
    function: &'arena ResolvedFunction,
    budget: &'budget mut ReplayBudget,
    bytes: usize,
}
impl<'arena, 'hir, 'budget> PrefixWitness<'arena, 'hir, 'budget> {
    pub(in crate::cleanup_plan::replay::expression_facts) fn acquire(
        arena: &'arena Arena<'hir>,
        function: &'arena ResolvedFunction,
        budget: &'budget mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let frames = 2 * size_of::<Self>()
            + 2 * size_of::<(&Arena<'_>, &ResolvedFunction, &mut ReplayBudget)>()
            + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 4 * size_of::<Result<(), Diagnostic>>()
            + 2 * size_of::<Result<Self, Diagnostic>>()
            + 2 * size_of::<(usize, &Node<'_>, &[u8], &[u8], usize, usize, usize)>()
            + 4 * size_of::<(&ExpressionId, &str, &[u8])>()
            + 4 * size_of::<bool>();
        budget.reserve_bytes(
            function,
            frames,
            "expression-fact prefix certificate acquisition",
        )?;
        budget.charge(
            function,
            20,
            "expression-fact prefix certificate initialization",
        )?;
        if arena.nodes.len() < 16 {
            return Ok(Self {
                arena,
                function,
                budget,
                bytes: 0,
            });
        }
        let mut last = arena.root;
        loop {
            budget.charge(function, 3, "expression-fact prefix maximum acquisition")?;
            let node = &arena.nodes[last];
            if node.right == NIL {
                break;
            }
            last = node.right;
        }
        budget.charge(function, 12, "expression-fact prefix endpoint acquisition")?;
        let left = arena.nodes[arena.first].key.as_str().as_bytes();
        let right = arena.nodes[last].key.as_str().as_bytes();
        let end = left.len().min(right.len());
        let units = end.checked_add(1).ok_or_else(|| {
            budget.budget_refusal(
                function,
                "expression-fact prefix certificate work overflow",
                "",
            )
        })?;
        budget.charge(
            function,
            units,
            "expression-fact prefix certificate byte comparison",
        )?;
        let mut common = 0;
        while common < end {
            if left[common] != right[common] {
                break;
            }
            common += 1;
        }
        Ok(Self {
            arena,
            function,
            budget,
            bytes: common,
        })
    }
    /// Move the exact certificate payer into the exact querying owner.
    pub(in crate::cleanup_plan::replay::expression_facts) fn into_session(
        self,
    ) -> Result<
        (
            PrefixSession<'arena, 'hir, 'budget>,
            PrefixView<'arena, 'hir>,
        ),
        Diagnostic,
    > {
        let frames = 2 * size_of::<PrefixView<'_, '_>>()
            + 2 * size_of::<Result<(PrefixSession<'_, '_, '_>, PrefixView<'_, '_>), Diagnostic>>()
            + 2 * size_of::<(PrefixSession<'_, '_, '_>, PrefixView<'_, '_>)>()
            + 4 * size_of::<Result<(), Diagnostic>>()
            + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 2 * size_of::<usize>();
        self.budget.reserve_bytes(
            self.function,
            frames,
            "expression-fact prefix session transfer",
        )?;
        self.budget
            .charge(self.function, 4, "expression-fact prefix session fields")?;
        let view = PrefixView {
            arena: self.arena,
            function: self.function,
            bytes: self.bytes,
        };
        let session = PrefixSession {
            arena: self.arena,
            function: self.function,
            budget: self.budget,
            control: Control {
                lower: 0,
                upper: 0,
                at: NIL,
                last: self.arena.root,
            },
            compared: Compared {
                bytes: Bytes {
                    left: &[],
                    right: &[],
                    end: 0,
                    common: 0,
                },
                order: Ordering::Equal,
                units: 0,
                a: 0,
                b: 0,
            },
            prefix: self.bytes,
        };
        Ok((session, view))
    }
}
impl<'arena, 'hir, 'budget> PrefixSession<'arena, 'hir, 'budget> {
    pub(in crate::cleanup_plan::replay::expression_facts) const OLD_SCRATCH: usize =
        Session::OLD_SCRATCH;
    pub(in crate::cleanup_plan::replay::expression_facts) const fn acquisition_bytes() -> usize {
        // Full initialized/receiving owner, nested field initializers,
        // constructor args, acquisition payment/work actual/formal frames and
        // Results, and two scalar acquisition receivers. Drop has only
        // trivial references/scalars.
        // The ordinary caller separately pays its continuation and selection.
        2 * size_of::<Self>()
            + 2 * size_of::<Control>()
            + 2 * size_of::<Compared<'_>>()
            + 2 * size_of::<Bytes<'_>>()
            + 2 * size_of::<PrefixWitness<'_, '_, '_>>()
            + 4 * size_of::<Result<(), Diagnostic>>()
            + 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 2 * size_of::<usize>()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn into_local_bytes() -> usize {
        Session::into_local_bytes()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn into_edge_local_bytes() -> usize
    {
        Session::into_edge_local_bytes()
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::expression_facts) fn fact(
        &mut self,
        query: &'arena ExpressionId,
    ) -> Result<Option<&'arena Option<CallFact<'hir>>>, Diagnostic> {
        self.budget.reserve_bytes(
            self.function,
            Session::acquisition_bytes()
                + 2 * size_of::<(&mut Self, &ExpressionId)>()
                + 2 * size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>(),
            "retained independent legacy interleaving frame",
        )?;
        self.budget.charge(
            self.function,
            12,
            "retained legacy interleaving initialization",
        )?;
        let mut legacy = Session::new_prepaid(self.arena, self.function, &mut *self.budget);
        let result = legacy.fact(query);
        self.control = legacy.control;
        self.compared = legacy.compared;
        result
    }
}
impl PrefixSession<'_, '_, '_> {
    pub(in crate::cleanup_plan::replay::expression_facts) const fn prefixed_query_bytes() -> usize {
        2 * size_of::<(
            &mut Self,
            &ExpressionId,
            &PrefixView<'_, '_>,
            &mut Option<&Option<CallFact<'_>>>,
            &mut crate::cleanup_plan::replay::ordinary_receiving::Failure<'_>,
            bool,
        )>()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn prefixed_edge_query_bytes(
    ) -> usize {
        Self::prefixed_query_bytes()
    }
    const fn prefix_query_bytes() -> usize {
        2 * size_of::<(*const Arena<'_>, *const Arena<'_>)>()
            + 2 * size_of::<(*const ResolvedFunction, *const ResolvedFunction)>()
            + 2 * size_of::<(usize, usize)>()
            + 8 * size_of::<bool>()
            + 2 * size_of::<(usize, usize)>()
            + 2 * size_of::<usize>()
    }
}

impl PrefixSession<'_, '_, '_> {
    pub(in crate::cleanup_plan::replay::expression_facts) const fn prefixed_acquisition_bytes(
    ) -> usize {
        Self::OLD_SCRATCH
            + Self::acquisition_bytes()
            + crate::cleanup_plan::replay::ordinary_receiving::Failure::acquisition_bytes()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn prefixed_edge_bytes() -> usize {
        Self::into_edge_local_bytes() + Self::prefixed_edge_query_bytes()
    }
    pub(in crate::cleanup_plan::replay::expression_facts) const fn prefixed_transition_bytes(
    ) -> usize {
        Self::prefixed_edge_bytes() + size_of::<Option<&Option<CallFact<'_>>>>()
    }
}
