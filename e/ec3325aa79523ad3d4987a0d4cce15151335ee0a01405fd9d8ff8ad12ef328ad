//! Guarded last use of HIR-private closure payloads between exact root proofs.
use super::super::super::{Node, SkeletonTerminal};
use super::*;
use crate::bounded_output::classifier_string::{ClassifierLedger, StorageParent};
use std::mem::size_of;

// Fresh between-root authentication cannot spend an old canonical-call bank.
// current_receiving_controls binds current()->ACTIVE's cloned Rc/TLS/Option
// roles. StorageParent::TERMINAL_CONTROLS is exactly one record_refusal's
// receipt/site/stage/floor/remaining/overflow bank (classifier_string.rs).
// Its borrowed Budget receiver excludes current()'s held Option/Rc bindings
// and two Rc::deref actual/formal/output triples. All eight are thin loans to
// Sized fields; &() prices their target-layout reference width without forming
// a proxy runtime owner or depending on the private Budget type's layout.
// preflight_owned's successful paid arm ALSO calls active_refused()->active().
// The following source-bound inventory pays that second clone/TLS transport,
// moved Option::is_some_and receiver/callback and sticky Cell::get separately.
// Budget is Sized and private: Rc<()> and loans to () are layout proxies for
// its thin pointer carriers only; no proxy runtime object is constructed.
const STICKY_ACTIVE_CONTROLS: usize = {
    use std::cell::{Cell, Ref, RefCell};
    use std::thread::LocalKey;
    type Active = RefCell<Option<Rc<()>>>;
    type Loan = Ref<'static, Option<Rc<()>>>;
    2 * size_of::<&ReplayBudget>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&LocalKey<Active>>()
        + 4 * size_of::<&Active>()
        + size_of::<Loan>()
        + 2 * size_of::<&Loan>()
        + 3 * size_of::<&Option<Rc<()>>>()
        + 6 * size_of::<Option<Rc<()>>>()
        + 5 * size_of::<Rc<()>>()
        + 6 * size_of::<&Rc<()>>()
        + size_of::<&()>()
        + 2 * size_of::<&Cell<bool>>()
        + 4 * size_of::<bool>()
};
const AUTHENTICATION: usize = STICKY_ACTIVE_CONTROLS
    + ClassifierLedger::current_receiving_controls()
    + StorageParent::TERMINAL_CONTROLS
    + 8 * size_of::<&()>()
    + 2 * size_of::<(&mut empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>, &ResolvedFunction, &ReplayBudget)>()
    + 6 * size_of::<&empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>>()
    + 2 * size_of::<&mut empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>>()
    + 8 * size_of::<bool>()
    // One immediate terminal creates empty String, actual/formal io arguments,
    // Diagnostic result and its caller receiver; the Result bank is below.
    + 3 * size_of::<String>()
    + 2 * size_of::<(&'static str, String)>()
    + 2 * size_of::<Diagnostic>();

// Caller pays before constructing the new invocation's actual/formal carriers.
pub(in crate::cleanup_plan::replay::factored) const CALL: usize = AUTHENTICATION
    + 4 * size_of::<(
        &mut Canonicalizer<'static>,
        &ResolvedFunction,
        &mut ReplayBudget,
        usize,
        usize,
    )>()
    + 4 * size_of::<Result<bool, Diagnostic>>()
    + 2 * size_of::<&Result<bool, Diagnostic>>()
    + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>()
    + empty_memo::Memo::<Rc<ordered_weights::OrderedWeights>>::RELEASE_CALL
    + 4 * size_of::<bool>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<[SkeletonTerminal; 3]>()
    + 2 * size_of::<std::iter::Enumerate<std::array::IntoIter<SkeletonTerminal, 3>>>()
    + 2 * size_of::<&mut std::iter::Enumerate<std::array::IntoIter<SkeletonTerminal, 3>>>()
    + 2 * size_of::<Option<(usize, SkeletonTerminal)>>()
    + 2 * size_of::<(usize, SkeletonTerminal)>();
// Each step constructs reserve_bytes AND charge actual/formal arguments.
const STEP: usize = 4 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 4 * size_of::<Result<(), Diagnostic>>()
    // Terminal entry pays first next; each terminal step renews the next probe,
    // including the final None, without reusing its previous return receiver.
    + 2 * size_of::<Option<(usize, SkeletonTerminal)>>()
    + 2 * size_of::<(usize, SkeletonTerminal)>()
    + 2 * size_of::<&mut std::iter::Enumerate<std::array::IntoIter<SkeletonTerminal, 3>>>()
    + 2 * size_of::<&mut std::slice::Iter<'static, Node>>()
    + 2 * size_of::<&mut std::slice::Iter<'static, (Label, usize)>>()
    + 2 * size_of::<&mut std::slice::Iter<'static, usize>>()
    // First/next iterator probes are prepaid by entry/the preceding step.
    + 4 * size_of::<&Node>()
    + 2 * size_of::<&[(Label, usize)]>()
    + 2 * size_of::<&[usize]>()
    + 2 * size_of::<Option<&Node>>()
    + 2 * size_of::<Option<&(Label, usize)>>()
    + 2 * size_of::<Option<&usize>>()
    + 2 * size_of::<(&Label, &usize)>()
    + 2 * size_of::<std::slice::Iter<'static, Node>>()
    + 2 * size_of::<std::slice::Iter<'static, (Label, usize)>>()
    + 2 * size_of::<std::slice::Iter<'static, usize>>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<bool>();

// Preserve the original complete release method below as an independent
// entry. Shipping additionally retires only proven-dead exact-key payloads and
// empty scratch; the Signature arena and its stable values are never modified.
pub(in crate::cleanup_plan::replay::factored) const STORAGE_CALL: usize =
    CALL + 4 * size_of::<(
        &mut Canonicalizer<'static>,
        &ResolvedFunction,
        &mut ReplayBudget,
        usize,
        usize,
    )>() + 4 * size_of::<Result<bool, Diagnostic>>()
        + intern_arena::Arena::<SharedWeighted>::RELEASE_CONTROLS
        + 3 * size_of::<Vec<usize>>()
        + 3 * size_of::<Vec<(usize, bool)>>()
        + 3 * size_of::<Vec<CanonicalFrame<'static>>>()
        + 3 * size_of::<Vec<SharedWeighted>>()
        + 4 * size_of::<&mut Canonicalizer<'static>>()
        + 4 * size_of::<bool>();

impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored) fn release_hir_storage(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        hir_root: usize,
        plan_root: usize,
    ) -> Result<bool, Diagnostic> {
        if !self.release_hir_private(function, budget, hir_root, plan_root)? {
            return Ok(false);
        }
        if let ReplayBudget {
            payer: AllocationPayer::LegacyUnobserved,
            ..
        } = budget
        {
        } else {
            return Ok(true);
        }
        // The old proof established empty scratch and forbids every PLAN edge
        // into [hir_root,plan_root), including epsilon and unreachable nodes.
        // Exact weighted keys containing one forbidden node cannot be PLAN hits.
        if let Err(error) = self
            .states
            .release_private_keys(function, budget, hir_root, plan_root)
        {
            self.closures.reject_after_error();
            return Err(error);
        }
        self.child_ids = Vec::new();
        self.cursor = Vec::new();
        self.frames = Vec::new();
        self.open = Vec::new();
        Ok(true)
    }
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
    pub(in crate::cleanup_plan::replay::factored) fn retirement_matches_old(
        &self,
        old: &Self,
        first: usize,
        end: usize,
    ) -> bool {
        self.ids.iter().eq(old.ids.iter())
            && self.states.iter().eq(old
                .states
                .iter()
                .filter(|(key, _)| !key.iter().any(|(node, _)| *node >= first && *node < end)))
    }
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
