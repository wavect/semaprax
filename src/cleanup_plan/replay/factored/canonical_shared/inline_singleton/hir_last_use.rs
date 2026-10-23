//! Guarded last use of HIR-private closure payloads between exact root proofs.
use super::super::super::{Node, SkeletonTerminal};
use super::*;
use std::mem::size_of;

// Caller pays before constructing the new invocation's actual/formal carriers.
pub(in crate::cleanup_plan::replay::factored) const CALL: usize =
    4 * size_of::<(
        &mut Canonicalizer<'static>,
        &ResolvedFunction,
        &mut ReplayBudget,
        usize,
        usize,
    )>() + 4 * size_of::<Result<bool, Diagnostic>>()
        + 2 * size_of::<&Result<bool, Diagnostic>>()
        + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + 2 * size_of::<Result<(), Diagnostic>>()
        + empty_memo::Memo::<Rc<ordered_weights::OrderedWeights>>::RELEASE_CALL
        + 4 * size_of::<bool>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<[SkeletonTerminal; 3]>()
        + 2 * size_of::<std::iter::Enumerate<std::array::IntoIter<SkeletonTerminal, 3>>>()
        + 2 * size_of::<Option<(usize, SkeletonTerminal)>>()
        + 2 * size_of::<(usize, SkeletonTerminal)>();
const STEP: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 4 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<&Node>()
    + 2 * size_of::<std::slice::Iter<'static, Node>>()
    + 2 * size_of::<std::slice::Iter<'static, (Label, usize)>>()
    + 2 * size_of::<std::slice::Iter<'static, usize>>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>();

impl Canonicalizer<'_> {
    /// The actual builder first allocates sink0/End1..3/HIR-root4, completes
    /// every HIR chain and DAG join, then allocates every PLAN entry followed
    /// by its transitions. PLAN targets are entries, fresh nodes or End1..3.
    /// Entry0 therefore names the first PLAN allocation. The runtime check also
    /// rejects shared/malformed/future builder layouts before mutating the Memo.
    pub(in crate::cleanup_plan::replay::factored) fn release_hir_private(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        hir_root: usize,
        plan_root: usize,
    ) -> Result<bool, Diagnostic> {
        // Authenticate the captured owner before reading any foreign account.
        if !self.closures.preflight_owned(function, budget) {
            return if self.closures.failed() {
                Err(Diagnostic::io("SPX-H006", String::new()))
            } else {
                Ok(false)
            };
        }
        if self.closures.failed() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        let result = self.release_hir_private_checked(function, budget, hir_root, plan_root);
        if result.is_err() {
            self.closures.reject_after_error();
        }
        result
    }
    fn release_hir_private_checked(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        hir_root: usize,
        plan_root: usize,
    ) -> Result<bool, Diagnostic> {
        budget.reserve_bytes(
            function,
            STEP,
            "factored HIR-private partition guard receiving",
        )?;
        if function.cleanup_plan.entry.0 != 0
            || self.sink != 0
            || hir_root != 4
            || plan_root <= hir_root
            || plan_root >= self.automaton.nodes.len()
            || !self.cursor.is_empty()
            || !self.child_ids.is_empty()
            || !self.frames.is_empty()
            || !self.open.is_empty()
        {
            return Ok(false);
        }
        // Common nodes are retained even when HIR and PLAN memo values alias
        // the same immutable Rc. Prove that their edges cannot reenter HIR.
        if !matches!(self.automaton.nodes[0], Node::Empty) {
            return Ok(false);
        }
        for (index, terminal) in [
            SkeletonTerminal::Success,
            SkeletonTerminal::Failure,
            SkeletonTerminal::LoopLatch,
        ]
        .into_iter()
        .enumerate()
        {
            budget.reserve_bytes(
                function,
                STEP,
                "factored common terminal partition receiving",
            )?;
            budget.charge(function, 1, "factored common terminal partition proof")?;
            if !matches!(&self.automaton.nodes[index + 1], Node::Movement((Label::End(actual), 0)) if *actual == terminal)
            {
                return Ok(false);
            }
        }
        // Check every allocated PLAN node, including unreachable ones. The
        // result is stronger than reachability from plan_root and needs no new
        // visited set, queue or independently estimated overlap bound.
        for node in &self.automaton.nodes[plan_root..] {
            budget.reserve_bytes(function, STEP, "factored PLAN partition node receiving")?;
            budget.charge(function, 1, "factored PLAN partition node proof")?;
            for (_, target) in node.moves() {
                budget.reserve_bytes(
                    function,
                    STEP,
                    "factored PLAN movement partition receiving",
                )?;
                budget.charge(function, 1, "factored PLAN movement partition proof")?;
                if (*target >= hir_root && *target < plan_root)
                    || *target >= self.automaton.nodes.len()
                {
                    return Ok(false);
                }
            }
            for target in node.epsilon() {
                budget.reserve_bytes(
                    function,
                    STEP,
                    "factored PLAN epsilon partition receiving",
                )?;
                budget.charge(function, 1, "factored PLAN epsilon partition proof")?;
                if (*target >= hir_root && *target < plan_root)
                    || *target >= self.automaton.nodes.len()
                {
                    return Ok(false);
                }
            }
        }
        self.closures
            .release_private_payloads(function, budget, hir_root, plan_root)?;
        Ok(true)
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::factored) fn private_release_receipt(&self) -> [usize; 12] {
        let (length, capacity, payloads, values) = self.closures.private_release_receipt();
        [
            length,
            capacity,
            payloads,
            values,
            self.states.nodes.len(),
            self.states.nodes.capacity(),
            self.ids.nodes.len(),
            self.ids.nodes.capacity(),
            self.child_ids.capacity(),
            self.cursor.capacity(),
            self.frames.capacity(),
            self.open.capacity(),
        ]
    }
}

#[cfg(test)]
impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored) fn seed_private_release_test(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        key: usize,
    ) {
        self.closures.authenticate(function, budget).unwrap();
        self.closures
            .insert_bound(function, key, budget, || {
                Rc::new(ordered_weights::OrderedWeights::new())
            })
            .unwrap();
    }
    pub(in crate::cleanup_plan::replay::factored) fn private_release_state(
        &self,
        key: usize,
    ) -> (u8, Option<usize>) {
        (
            self.closures.state(&key),
            self.closures
                .get(&key)
                .map(|value| Rc::as_ptr(value) as usize),
        )
    }
}
