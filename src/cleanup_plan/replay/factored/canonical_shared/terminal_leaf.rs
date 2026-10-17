//! Cold terminal singleton producer: no general frame/cursor/output owner exists.
use super::*;
use std::mem::size_of;

// These lexical authentications retain the original zero-request positions and
// error selection without constructing a general ClosureReceiving destination.
macro_rules! authenticate_terminal {
    ($this:ident, $function:ident, $budget:ident) => {{
        if $this.closures.preflight($function, $budget) {
            if $this.closures.failed() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            if let Err(error) = $budget.reserve_bytes($function, 0, "factored closure memo authentication") {
                $this.closures.failed = true;
                return Err(error);
            }
        } else if $this.closures.failed() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        } else {
            activate_closure!($this, $function, $budget, error, { return Err(error); });
        }
    }};
}

impl Canonicalizer<'_> {
    // Predicate return/receiving, node/length scalars and borrowed owner, plus
    // the reservation's real Result. No general owner is acquired by this bank.
    pub(in crate::cleanup_plan::replay::factored) const COLD_NODE_CONTROLS: usize =
        2 * size_of::<bool>() + 4 * size_of::<usize>()
        + 2 * size_of::<&Self>() + size_of::<Result<(), Diagnostic>>();
    pub(super) const COLD_ROOT_CONTROLS: usize = Self::COLD_NODE_CONTROLS
        + 2 * size_of::<&Weighted>() + 2 * size_of::<u64>();
    // Both actual Result owners and the nested Option initializer/receiving.
    pub(super) const COLD_RESULT_CONTROLS: usize =
        2 * size_of::<Result<Option<usize>, Diagnostic>>() + 2 * size_of::<Option<usize>>();
    const COLD_OWNERS: usize = size_of::<Weighted>()
        + 2 * size_of::<SmallWeighted>() + 2 * size_of::<SharedWeighted>()
        + 2 * size_of::<intern_arena::Signature>() + size_of::<Vec<(Label, usize)>>()
        + 2 * size_of::<usize>() + size_of::<Diagnostic>()
        + size_of::<Result<(), Diagnostic>>();

    // This is structural eligibility, never proof from a failed Memo lookup.
    // A cold receiver has published nothing and has no retained general scratch.
    pub(in crate::cleanup_plan::replay::factored) fn cold_dead_node(&self, node: usize) -> bool {
        !self.closures.failed() && self.closures.unpublished()
            && self.states.nodes.is_empty() && self.ids.nodes.is_empty()
            && !self.scratch_paid && self.cursor.is_empty() && self.frames.is_empty()
            && self.open.is_empty() && self.child_ids.is_empty()
            && node < self.automaton.nodes.len() && node != self.sink
            && self.automaton.nodes[node].moves().is_empty()
            && self.automaton.nodes[node].epsilon().is_empty()
    }

    // Actual comparison dispatch does not add the test adapter's initial Memo
    // authentication. A warm/foreign/poisoned owner is rejected before paying a
    // fresh destination; every nonleaf keeps the complete general producer.
    pub(in crate::cleanup_plan::replay::factored) fn id_for_comparison(
        &mut self, function: &ResolvedFunction, root: Weighted, budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        self.closures.preflight(function, budget);
        if self.closures.failed() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        // Existing received owner place: no copied tag, bool or reference.
        // scratch_paid=true makes cold_dead_node false by its own definition.
        if let Self { scratch_paid: false, .. } = self {
        if budget.pays_allocations() {
            budget.reserve_bytes(function, Self::COLD_ROOT_CONTROLS,
                "factored cold terminal root predicate")?;
            budget.reserve_bytes(function, Self::COLD_RESULT_CONTROLS,
                "factored cold terminal result receiving")?;
        }
        if root.len() == 1 && root[0].1 == 1 && self.cold_dead_node(root[0].0) {
            return self.cold_dead_id(function, root, budget);
        }
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(function, closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls")?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.id_with_receiving(function, root, budget, &mut receiving)
    }

    // Only the exact paid guard in either id dispatcher calls this producer. The root remains
    // an ordinary owned Many key; its caller already paid the complete Vec
    // backing. No key or id is invented, and both original arenas are populated.
    pub(super) fn cold_dead_id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        tests::note_actual_entry(budget);
        authenticate_terminal!(self, function, budget);
        if budget.pays_allocations() {
            budget.reserve_bytes(function, Self::COLD_OWNERS, "factored cold terminal owning receiving")?;
        }
        let node = root[0].0;
        budget.reserve_bytes(function, physical_clone::rc_backing_bytes::<SmallWeighted>(),
            "factored weighted shared header")?;
        let shared = SmallWeighted::Many(root);
        let state = Rc::new(shared);
        // Complete immediate-old work protocol, in its original order. There
        // is no frame/open Rc clone or cursor to acquire for this leaf.
        budget.charge(function, 2, "factored weighted state copy")?;
        budget.reserve_bytes(function, 0, "factored weighted state backing")?;
        budget.charge(function, 1, "factored canonical frame")?;
        budget.charge(function, 2, "factored weighted state copy")?;
        budget.reserve_bytes(function, 0, "factored weighted state backing")?;
        authenticate_terminal!(self, function, budget);
        budget.charge(function, 1, "factored closure cursor")?;
        budget.charge(function, 1, "factored closure cursor")?;
        budget.charge(function, 1, "factored skeleton closure")?;
        authenticate_terminal!(self, function, budget);
        if let Err(error) = self.closures.insert_empty_bound(function, node, budget) {
            self.closures.failed = true;
            return Err(error);
        }
        budget.reserve_array::<(Label, SharedWeighted)>(function, 0, "factored successor vector")?;
        budget.charge(function, 1, "factored canonical frame")?;
        budget.charge(function, 0, "factored canonical signature construction")?;
        budget.reserve_array::<(Label, usize)>(function, 0, "factored canonical signature vector")?;
        // Empty arenas are part of the guard. Keep original insertion ordinals
        // and full storage, but there is no borrowed lookup/query to construct.
        let key = (0, Vec::new());
        let id = self.ids.len();
        #[cfg(test)]
        arena_tests::signature_lookup(false);
        macro_rules! attempt { ($operation:expr) => { $operation? }; }
        intern_arena::prepare_insert!(self.ids, intern_arena::Signature, function, budget);
        self.ids.insert_prepaid(key, id);
        intern_arena::prepare_insert!(self.states, SharedWeighted, function, budget);
        self.states.insert_prepaid(state, id);
        Ok(Some(id))
    }
}
