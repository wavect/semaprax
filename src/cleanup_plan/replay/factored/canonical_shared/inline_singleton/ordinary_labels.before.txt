//! Inline singleton Exit transport; the complete immediate owner remains in the parent.
use super::*;
// Invoked only after preflight proved an unbound, nonfailed Memo. This is the
// original cold activation body at its caller: no authenticate Result or
// forwarded method arguments are constructed. Nested reservation Results,
// first-refusal order, full Memo/Binding storage and capture remain real.
macro_rules! activate_closure {
    ($this:ident, $function:ident, $budget:ident, $error:ident, $failure:block) => {{
        if let Err($error) =
            $budget.reserve_bytes($function, 0, "factored closure memo authentication")
        {
            $this.closures.failed = true;
            $failure
        }
        if $budget.pays_allocations() {
            if let Err($error) = $budget.reserve_bytes(
                $function,
                std::mem::size_of::<empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>>()
                    + std::mem::size_of::<Option<empty_memo::Binding>>(),
                "factored closure memo controls",
            ) {
                $this.closures.failed = true;
                $failure
            }
        }
        let binding = Some(empty_memo::Binding {
            function: std::ptr::from_ref($function),
            budget: std::ptr::from_ref($budget),
            ledger: crate::bounded_output::classifier_string::ClassifierLedger::capture(),
        });
        $this.closures.binding = binding;
    }};
}

// Lexical unit publication deliberately introduces no forwarding borrowed
// receiver: preflight replaces the old RetainedMemo auth method boundary, and
// the old zero-auth call borrows the already-live invocation refusal field here.
macro_rules! authenticate_closure {
    ($this:ident, $function:ident, $budget:ident, $receiving:ident) => {{
        // The returned branch predicate replaces the former controls_paid /
        // binding branch; no second Option<Binding> borrow or named local exists.
        if $this.closures.preflight_owned($function, $budget) {
            if $this.closures.failed() {
                $receiving.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
            } else {
                $budget.authenticate_zero_into(
                    $function,
                    "factored closure memo authentication",
                    &mut $receiving.refusal,
                );
                if $receiving.refusal.is_some() {
                    $this.closures.failed = true;
                }
            }
        } else if $this.closures.failed() {
            $receiving.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
        } else {
            'activation: {
                activate_closure!($this, $function, $budget, error, {
                    $receiving.refusal = Some(error);
                    break 'activation;
                });
            }
        }
    }};
}
macro_rules! publish_closure {
    ($this:ident, $function:ident, $budget:ident, $receiving:ident, $operation:expr) => {{
        authenticate_closure!($this, $function, $budget, $receiving);
        if $receiving.refusal.is_some() {
            $receiving.ready = true;
            return;
        }
        if let Err(error) = $operation {
            $this.closures.failed = true;
            $receiving.refuse(error);
            return;
        }
    }};
}

type SharedWeighted = Rc<SmallWeighted>;

enum CanonicalFrame<'hir> {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
    ExitOne(SharedWeighted, u64, (&'hir Label, SharedWeighted)),
}
const _: () = assert!(
    std::mem::size_of::<CanonicalFrame<'static>>() == std::mem::size_of::<super::CanonicalFrame>()
);

// The caller has already paid the moved vector's complete allocated backing.
// Rc owns that vector without copying it; its counters and full enum header are
// separately acquired while all preceding owners remain fully charged.
fn own_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: Weighted,
) -> Result<SharedWeighted, Diagnostic> {
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            2 * std::mem::size_of::<SmallWeighted>()
                + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>()
                + std::mem::size_of::<Result<(), Diagnostic>>(),
            "factored moved many weighted enum receiving",
        )?;
    }
    budget.reserve_bytes(
        function,
        physical_clone::rc_backing_bytes::<SmallWeighted>(),
        "factored weighted shared header",
    )?;
    let shared = SmallWeighted::Many(state);
    Ok(Rc::new(shared))
}

// Vec's FromIterator growth policy may round 1..3 items up to four slots.
// Build this producer through its exact precharged capacity instead.
fn weighted_successor(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    targets: BTreeMap<usize, u64>,
) -> Result<SharedWeighted, Diagnostic> {
    budget.reserve_array::<(usize, u64)>(
        function,
        targets.len(),
        "factored weighted successor vector",
    )?;
    let mut state = Vec::with_capacity(targets.len());
    state.extend(targets);
    own_weighted(function, budget, state)
}

fn share_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: &SharedWeighted,
) -> Result<SharedWeighted, Diagnostic> {
    let work = state
        .len()
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "factored weighted state work overflowed"))?;
    budget.charge(function, work, "factored weighted state copy")?;
    // Authentication remains sticky even though this clone creates no backing.
    // The receiving frame/map slot has its own full carrier reservation.
    budget.reserve_bytes(function, 0, "factored weighted state backing")?;
    Ok(Rc::clone(state))
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
pub(in crate::cleanup_plan::replay::factored) struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: empty_memo::Memo<Rc<ordered_weights::OrderedWeights>>,
    states: intern_arena::Arena<SharedWeighted>,
    ids: intern_arena::Arena<intern_arena::Signature>,
    child_ids: Vec<usize>,
    cursor: Vec<(usize, bool)>,
    frames: Vec<CanonicalFrame<'a>>,
    open: Vec<SharedWeighted>,
    scratch_paid: bool,
    successor_unit_paid: bool,
    successor_general_paid: bool,
}

impl<'a> Canonicalizer<'a> {
    // Shipping caller admits the complete new owner and both arena header
    // initializer/receiving pairs before entering this infallible constructor.
    // The old test constructor boundary remains explicit and unreceived.
    pub(in crate::cleanup_plan::replay::factored) const CONSTRUCTOR_CONTROLS: usize = 2
        * std::mem::size_of::<Self>()
        + 2 * std::mem::size_of::<intern_arena::Arena<SharedWeighted>>()
        + 2 * std::mem::size_of::<intern_arena::Arena<intern_arena::Signature>>()
        + 2 * std::mem::size_of::<Vec<intern_arena::Node<SharedWeighted>>>()
        + 2 * std::mem::size_of::<Vec<intern_arena::Node<intern_arena::Signature>>>()
        + 2 * std::mem::size_of::<Vec<usize>>()
        + std::mem::size_of::<Result<(), Diagnostic>>();

    pub(in crate::cleanup_plan::replay::factored) fn new(
        automaton: &'a Automaton,
        sink: usize,
    ) -> Self {
        Self {
            automaton,
            sink,
            closures: empty_memo::Memo::new(),
            states: intern_arena::Arena::new(),
            ids: intern_arena::Arena::new(),
            child_ids: Vec::new(),
            cursor: Vec::new(),
            frames: Vec::new(),
            open: Vec::new(),
            scratch_paid: false,
            successor_unit_paid: false,
            successor_general_paid: false,
        }
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    // Single-request test entries pay their real owner here; equivalent()
    // instead lends one invocation owner across both roots. No wrapper gain claim.
    #[cfg(test)]
    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        // Preserve the old entry's owner rejection before a fresh test-only slot
        // can query or charge a foreign payer. A valid call still pays acquisition.
        if self.closures.preflight_owned(function, budget) {
            if self.closures.failed() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
        } else {
            if self.closures.failed() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            // Keep the first-cold failure before invocation-slot acquisition.
            // The outer id Result is unchanged; the inner auth Result is absent.
            activate_closure!(self, function, budget, error, {
                return Err(error);
            });
        }
        // Existing received owner place: no copied tag, bool or reference.
        // scratch_paid=true makes cold_dead_node false by its own definition.
        if let Self {
            scratch_paid: false,
            ..
        } = self
        {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    Self::COLD_ROOT_CONTROLS,
                    "factored cold terminal root predicate",
                )?;
            }
            if root.len() == 1 && root[0].1 == 1 && self.cold_dead_node(root[0].0) {
                if budget.pays_allocations() {
                    budget.reserve_bytes(
                        function,
                        Self::COLD_RESULT_CONTROLS,
                        "factored cold terminal result receiving",
                    )?;
                }
                return self.cold_dead_id(function, root, budget);
            }
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.id_with_receiving(function, root, budget, &mut receiving)
    }
    #[cfg(test)]
    #[allow(
        clippy::type_complexity,
        reason = "The independently compiled control retains its exact tuple layout and receiving types."
    )]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        // Identity/sticky preflight precedes every invocation payment. Cold
        // activation and all original payment order remain in the old body.
        self.closures.preflight_owned(function, budget);
        if self.closures.failed() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.successors_with_receiving(function, state, budget, &mut receiving)
    }
    #[cfg(test)]
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<empty_closure::Closure>, Diagnostic> {
        // Reject foreign/sticky owners before any adapter payment.
        if self.closures.preflight_owned(function, budget) {
            if self.closures.failed() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
        } else {
            if self.closures.failed() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            self.closures.authenticate(function, budget)?;
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                empty_closure::Closure::ADAPTER_CONTROLS,
                "test closure compatibility receiving",
            )?;
        }
        if self.closures.contains_key(&root) {
            budget.charge(function, 1, "factored closure cursor")?;
            return Ok(Some(match self.closures.get(&root) {
                Some(value) => empty_closure::Closure::Shared(Rc::clone(value)),
                None => empty_closure::Closure::Empty,
            }));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.closure_query(function, root, budget, &mut receiving);
        receiving.ready = false;
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if receiving.overflow {
            return Ok(None);
        }
        Ok(Some(match self.closures.get(&root) {
            Some(value) => empty_closure::Closure::Shared(Rc::clone(value)),
            None => empty_closure::Closure::Empty,
        }))
    }

    #[cfg(test)]
    fn closure_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<empty_closure::Closure>, Diagnostic> {
        // Compatibility adapter is priced separately from actual unit production.
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        authenticate_closure!(self, function, budget, receiving);
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                empty_closure::Closure::ADAPTER_CONTROLS,
                "test closure compatibility receiving",
            )?;
        }
        self.closure_query(function, root, budget, receiving);
        receiving.ready = false;
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if receiving.overflow {
            return Ok(None);
        }
        Ok(Some(match self.closures.get(&root) {
            Some(value) => empty_closure::Closure::Shared(Rc::clone(value)),
            None => empty_closure::Closure::Empty,
        }))
    }

    // Actual production gateway returns unit into the already-paid status/refusal
    // owner. No Rc clone, Option<closure>, Result or new owned view is returned.
    fn closure_query(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) {
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            receiving.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
            receiving.ready = true;
            return;
        }
        authenticate_closure!(self, function, budget, receiving);
        if receiving.refusal.is_some() {
            receiving.ready = true;
            return;
        }
        match self.closures.state(&root) {
            1 => {
                receiving.empty = true;
                receiving.overflow = false;
                match budget.charge(function, 1, "factored closure cursor") {
                    Ok(()) => receiving.ready = true,
                    Err(error) => {
                        receiving.refusal = Some(error);
                        receiving.ready = true;
                    }
                }
                return;
            }
            2 => {
                receiving.empty = false;
                receiving.overflow = false;
                match budget.charge(function, 1, "factored closure cursor") {
                    Ok(()) => receiving.ready = true,
                    Err(error) => {
                        receiving.refusal = Some(error);
                        receiving.ready = true;
                    }
                }
                return;
            }
            _ => {}
        }
        if budget.pays_allocations() {
            if let Err(error) = budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::QUERY_CONTROLS + std::mem::size_of::<usize>(),
                "factored held closure producer and take controls",
            ) {
                receiving.refusal = Some(error);
                receiving.ready = true;
                return;
            }
        }
        // This lexical first-use block has no prepare_scratch Result or
        // forwarded self/function/budget arguments. The reservation's own
        // Result and all actual retained fields remain independently owned.
        if !self.scratch_paid {
            if budget.pays_allocations() {
                if let Err(error) = budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Vec<(usize, bool)>>()
                        + std::mem::size_of::<Vec<CanonicalFrame<'_>>>()
                        + std::mem::size_of::<Vec<SharedWeighted>>()
                        + std::mem::size_of::<bool>(),
                    "factored retained canonical scratch controls",
                ) {
                    receiving.refusal = Some(error);
                    receiving.ready = true;
                    return;
                }
            }
            self.scratch_paid = true;
        }
        receiving.begin();
        #[cfg(test)]
        closure_receiving::note_producer();
        self.closure_inner_into(function, root, budget, receiving);
        self.cursor.clear();
        assert!(receiving.ready, "unit producer settled exactly once");
    }

    fn closure_inner_into(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) {
        macro_rules! attempt {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    Err(error) => {
                        receiving.refuse(error);
                        return;
                    }
                }
            };
        }
        attempt!(budget.reserve_vec(
            function,
            &mut self.cursor,
            1,
            "factored closure initial cursor",
        ));
        self.cursor.push((root, false));
        while let Some((node, expanded)) = self.cursor.pop() {
            attempt!(budget.charge(function, 1, "factored closure cursor"));
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                attempt!(budget.reserve_vec(
                    function,
                    &mut self.cursor,
                    1,
                    "factored closure cursors"
                ));
                self.cursor.push((node, true));
                for target in state.epsilon() {
                    if !self.closures.contains_key(target) {
                        attempt!(budget.reserve_vec(
                            function,
                            &mut self.cursor,
                            1,
                            "factored closure cursors",
                        ));
                        self.cursor.push((*target, false));
                    }
                }
                continue;
            }
            // Exactly one immutable epsilon child contributes the complete map.
            // Sink/own-move/duplicate-edge and all other shapes keep the full
            // general producer below, including checked route accumulation.
            if node != self.sink && state.moves().is_empty() && state.epsilon().len() == 1 {
                #[cfg(test)]
                app_census::eligible();
                let Some(inner) = self.closures.get(&state.epsilon()[0]) else {
                    if self.closures.contains_key(&state.epsilon()[0]) {
                        attempt!(budget.charge(function, 1, "factored skeleton closure"));
                        publish_closure!(
                            self,
                            function,
                            budget,
                            receiving,
                            self.closures.insert_empty_bound(function, node, budget)
                        );
                        #[cfg(test)]
                        app_census::published();
                        if node == root {
                            receiving.empty = true;
                        }
                        continue;
                    }
                    receiving.refuse(replay_error(function, "cleanup CFG contains a cycle"));
                    return;
                };
                #[cfg(test)]
                app_census::child_ready(inner.len());
                let shared = match ordered_alias::share(function, inner, budget) {
                    Ok(shared) => shared,
                    Err(error) => {
                        self.closures.reject_after_error();
                        receiving.refuse(error);
                        return;
                    }
                };
                #[cfg(test)]
                app_census::acquired();
                publish_closure!(
                    self,
                    function,
                    budget,
                    receiving,
                    self.closures
                        .insert_bound(function, node, budget, || shared)
                );
                #[cfg(test)]
                app_census::published();
                continue;
            }
            if node != self.sink && state.moves().is_empty() && state.epsilon().is_empty() {
                attempt!(budget.charge(function, 1, "factored skeleton closure"));
                publish_closure!(
                    self,
                    function,
                    budget,
                    receiving,
                    self.closures.insert_empty_bound(function, node, budget)
                );
                if node == root {
                    receiving.empty = true;
                }
                continue;
            }
            if budget.pays_allocations() {
                attempt!(budget.reserve_bytes(
                    function,
                    ordered_weights::OrderedWeights::CONTROLS,
                    "factored ordered closure controls",
                ));
            }
            let mut closure = ordered_weights::OrderedWeights::new();
            if !state.moves().is_empty() || node == self.sink {
                if closure.needs_capacity() {
                    attempt!(closure.grow(function, budget));
                }
                closure.insert_prepaid(node, 1_u64);
            }
            for target in state.epsilon() {
                let Some(inner) = self.closures.get(target) else {
                    if self.closures.contains_key(target) {
                        continue;
                    }
                    receiving.refuse(replay_error(function, "cleanup CFG contains a cycle"));
                    return;
                };
                for (inner_node, routes) in inner.iter() {
                    attempt!(budget.charge(function, 1, "factored closure route addition"));
                    if closure.get(inner_node).is_none() {
                        if closure.needs_capacity() {
                            attempt!(closure.grow(function, budget));
                        }
                        closure.insert_prepaid(*inner_node, 0_u64);
                    }
                    let slot = closure
                        .get_mut(inner_node)
                        .expect("inserted ordered weight");
                    let Some(sum) = slot.checked_add(*routes) else {
                        receiving.finish_overflow();
                        return;
                    };
                    *slot = sum;
                }
            }
            attempt!(budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            ));
            attempt!(budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<ordered_weights::OrderedWeights>(),
                "factored closure shared header",
            ));
            publish_closure!(
                self,
                function,
                budget,
                receiving,
                self.closures
                    .insert_bound(function, node, budget, || Rc::new(closure))
            );
        }
        receiving.ready = true;
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        if let [(node, 1)] = state.as_slice() {
            self.closure_query(function, *node, budget, receiving);
            receiving.ready = false;
            if receiving.refusal.is_some() {
                return Err(receiving.refusal.take().expect("published refusal"));
            }
            if receiving.overflow {
                return Ok(None);
            }
            if receiving.empty {
                // No closed map, Rc, label-group map, or iterator is constructed.
                // The original surviving empty vector and outer result remain.
                budget.reserve_array::<(Label, SharedWeighted)>(
                    function,
                    0,
                    "factored successor vector",
                )?;
                let accepted = 0;
                let result = Vec::with_capacity(0);
                return Ok(Some((accepted, result)));
            }
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    2 * std::mem::size_of::<Rc<ordered_weights::OrderedWeights>>()
                        + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                    "factored borrowed closed receiving controls",
                )?;
            }
            let closure = Rc::clone(self.closures.get(node).expect("computed shared closure"));
            for (_, routes) in closure.iter() {
                let Some(_sum) = routes
                    .checked_mul(1)
                    .and_then(|product| 0_u64.checked_add(product))
                else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
            }
            let borrowed = closure;
            let closed = &*borrowed;
            let accepted = closed.get(&self.sink).copied().unwrap_or(0);
            match &budget.payer {
                AllocationPayer::ActiveBuilder => {
                    budget.reserve_bytes(
                        function,
                        borrowed_labels::one_pass_receiving(),
                        "factored one-pass label call receivers",
                    )?;
                    return borrowed_labels::one_pass_successors(
                        function,
                        budget,
                        self.automaton,
                        closed,
                        accepted,
                    );
                }
                AllocationPayer::SelectedReservation => {}
                AllocationPayer::LegacyUnobserved => {}
                _ => {
                    budget.reserve_bytes(
                        function,
                        borrowed_labels::receiving::<ordered_weights::OrderedWeights>(),
                        "factored borrowed label call receivers",
                    )?;
                    return borrowed_labels::successors(
                        function,
                        budget,
                        self.automaton,
                        closed,
                        accepted,
                    );
                }
            }
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<BTreeMap<Label, paid_many::Targets>>()
                        + std::mem::size_of::<Option<&mut paid_many::Targets>>(),
                    "factored successor label receiving controls",
                )?;
            }
            let mut successors = BTreeMap::<Label, paid_many::Targets>::new();
            for (node, weight) in closed.iter() {
                let moves = self.automaton.nodes[*node].moves();
                budget.charge(
                    function,
                    moves.len().saturating_add(1),
                    "factored skeleton subset construction",
                )?;
                for (label, child) in moves {
                    if !successors.contains_key(label) {
                        budget.reserve_btree_node::<Label, paid_many::Targets>(
                            function,
                            "factored successor label node",
                        )?;
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                2 * std::mem::size_of::<paid_many::Targets>()
                                    + std::mem::size_of::<Option<u64>>()
                                    + std::mem::size_of::<Option<paid_many::Targets>>(),
                                "factored inline singleton target initializer and receiving",
                            )?;
                        }
                        budget.reserve_bytes(
                            function,
                            0,
                            "factored singleton target authentication",
                        )?;
                        let weight = 0_u64
                            .checked_add(*weight)
                            .expect("zero plus u64 cannot overflow");
                        successors.insert(
                            label.clone(),
                            paid_many::Targets::One {
                                child: *child,
                                weight,
                            },
                        );
                    } else {
                        let targets = successors.get_mut(label).expect("existing label");
                        match targets {
                            paid_many::Targets::One {
                                child: first,
                                weight: total,
                            } if *first == *child => {
                                // Same repeated-child checked_add before any later
                                // target acquisition or error, as in the old map.
                                let Some(sum) = total.checked_add(*weight) else {
                                    return Ok(None);
                                };
                                *total = sum;
                            }
                            paid_many::Targets::One {
                                child: first,
                                weight: total,
                            } => {
                                if budget.pays_allocations() {
                                    budget.reserve_bytes(
                                        function,
                                        paid_many::PROMOTE_RECEIVING,
                                        "factored composed promotion receivers",
                                    )?;
                                }
                                #[cfg(test)]
                                if budget.pays_allocations() {
                                    paid_many::note_branch(true);
                                }
                                *targets = paid_many::promote(
                                    function, budget, *first, *total, *child, *weight,
                                )?;
                            }
                            paid_many::Targets::PaidMany(_) => {
                                budget.reserve_bytes(
                                    function,
                                    paid_many::ADD_RECEIVING,
                                    "factored composed flat add receivers",
                                )?;
                                let paid_many::Targets::PaidMany(flat) = targets else {
                                    unreachable!("paid flat arm")
                                };
                                if !paid_many::add(function, budget, flat, *child, *weight)? {
                                    return Ok(None);
                                }
                            }
                            paid_many::Targets::Many(targets) => {
                                if !targets.contains_key(child) {
                                    budget.reserve_btree_node::<usize, u64>(
                                        function,
                                        "factored successor target node",
                                    )?;
                                }
                                let slot = targets.entry(*child).or_insert(0);
                                let Some(sum) = slot.checked_add(*weight) else {
                                    return Ok(None);
                                };
                                *slot = sum;
                            }
                        }
                    }
                }
            }
            budget.reserve_array::<(Label, SharedWeighted)>(
                function,
                successors.len(),
                "factored successor vector",
            )?;
            let mut result = Vec::with_capacity(successors.len());
            // Do not initialize an owning map iterator or its receiving tuple on
            // the empty route. Zero-byte vector authentication above still runs.
            if !successors.is_empty() {
                if budget.pays_allocations() {
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<
                            std::collections::btree_map::IntoIter<Label, paid_many::Targets>,
                        >() + std::mem::size_of::<Option<(Label, paid_many::Targets)>>()
                            + std::mem::size_of::<(Label, paid_many::Targets)>(),
                        "factored successor target iterator receiving",
                    )?;
                }
                for (label, targets) in successors {
                    if budget.pays_allocations() {
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<SharedWeighted>()
                                + std::mem::size_of::<(usize, u64)>(),
                            "factored inline successor finish receiving",
                        )?;
                    }
                    budget.reserve_bytes(
                        function,
                        0,
                        "factored successor target finish authentication",
                    )?;
                    let state = match targets {
                        paid_many::Targets::One { child, weight } => {
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    2 * std::mem::size_of::<SmallWeighted>()
                                        + std::mem::size_of::<[(usize, u64); 1]>()
                                        + std::mem::size_of::<Result<(), Diagnostic>>(),
                                    "factored inline weighted enum and array receiving",
                                )?;
                            }
                            budget.reserve_bytes(
                                function,
                                physical_clone::rc_backing_bytes::<SmallWeighted>(),
                                "factored weighted shared header",
                            )?;
                            let singleton = SmallWeighted::One([(child, weight)]);
                            Rc::new(singleton)
                        }
                        paid_many::Targets::PaidMany(_) => match budget {
                            ReplayBudget {
                                payer: AllocationPayer::LegacyUnobserved,
                                ..
                            } => {
                                budget.reserve_bytes(
                                    function,
                                    paid_many::FINISH_RECEIVING,
                                    "factored composed flat finish receivers",
                                )?;
                                let paid_many::Targets::PaidMany(flat) = targets else {
                                    unreachable!("paid flat finish")
                                };
                                own_weighted(function, budget, flat)?
                            }
                            _ => paid_many::finish!(function, budget, targets),
                        },
                        paid_many::Targets::Many(targets) => {
                            // The remaining fallible weighted producer still has
                            // its real map argument and Result receiving carrier.
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    std::mem::size_of::<BTreeMap<usize, u64>>()
                                        + std::mem::size_of::<Vec<(usize, u64)>>()
                                        + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>(),
                                    "factored many weighted producer receiving",
                                )?;
                            }
                            weighted_successor(function, budget, targets)?
                        }
                    };
                    result.push((label, state));
                }
            }
            return Ok(Some((accepted, result)));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                ordered_weights::OrderedWeights::CONTROLS
                    + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                "factored ordered accumulator controls",
            )?;
        }
        let mut owned = ordered_weights::OrderedWeights::new();
        for (node, weight) in state.iter() {
            self.closure_query(function, *node, budget, receiving);
            receiving.ready = false;
            if receiving.refusal.is_some() {
                return Err(receiving.refusal.take().expect("published refusal"));
            }
            if receiving.overflow {
                return Ok(None);
            }
            let Some(closure) = self.closures.get(node) else {
                continue;
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    owned.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if owned.get(inner).is_none() {
                    if owned.needs_capacity() {
                        owned.grow(function, budget)?;
                    }
                    owned.insert_prepaid(*inner, 0_u64);
                }
                *owned.get_mut(inner).expect("inserted ordered weight") = sum;
            }
        }
        let closed = &owned;
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        match &budget.payer {
            AllocationPayer::ActiveBuilder => {
                budget.reserve_bytes(
                    function,
                    borrowed_labels::one_pass_receiving(),
                    "factored one-pass label call receivers",
                )?;
                return borrowed_labels::one_pass_successors(
                    function,
                    budget,
                    self.automaton,
                    closed,
                    accepted,
                );
            }
            AllocationPayer::SelectedReservation => {}
            AllocationPayer::LegacyUnobserved => {}
            _ => {
                budget.reserve_bytes(
                    function,
                    borrowed_labels::receiving::<ordered_weights::OrderedWeights>(),
                    "factored borrowed label call receivers",
                )?;
                return borrowed_labels::successors(
                    function,
                    budget,
                    self.automaton,
                    closed,
                    accepted,
                );
            }
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<BTreeMap<Label, paid_many::Targets>>()
                    + std::mem::size_of::<Option<&mut paid_many::Targets>>(),
                "factored successor label receiving controls",
            )?;
        }
        let mut successors = BTreeMap::<Label, paid_many::Targets>::new();
        for (node, weight) in closed.iter() {
            let moves = self.automaton.nodes[*node].moves();
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, paid_many::Targets>(
                        function,
                        "factored successor label node",
                    )?;
                    if budget.pays_allocations() {
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<paid_many::Targets>()
                                + std::mem::size_of::<Option<u64>>()
                                + std::mem::size_of::<Option<paid_many::Targets>>(),
                            "factored inline singleton target initializer and receiving",
                        )?;
                    }
                    budget.reserve_bytes(
                        function,
                        0,
                        "factored singleton target authentication",
                    )?;
                    let weight = 0_u64
                        .checked_add(*weight)
                        .expect("zero plus u64 cannot overflow");
                    successors.insert(
                        label.clone(),
                        paid_many::Targets::One {
                            child: *child,
                            weight,
                        },
                    );
                } else {
                    let targets = successors.get_mut(label).expect("existing label");
                    match targets {
                        paid_many::Targets::One {
                            child: first,
                            weight: total,
                        } if *first == *child => {
                            // Same repeated-child checked_add before any later
                            // target acquisition or error, as in the old map.
                            let Some(sum) = total.checked_add(*weight) else {
                                return Ok(None);
                            };
                            *total = sum;
                        }
                        paid_many::Targets::One {
                            child: first,
                            weight: total,
                        } => {
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    paid_many::PROMOTE_RECEIVING,
                                    "factored composed promotion receivers",
                                )?;
                            }
                            #[cfg(test)]
                            if budget.pays_allocations() {
                                paid_many::note_branch(false);
                            }
                            *targets = paid_many::promote(
                                function, budget, *first, *total, *child, *weight,
                            )?;
                        }
                        paid_many::Targets::PaidMany(_) => {
                            budget.reserve_bytes(
                                function,
                                paid_many::ADD_RECEIVING,
                                "factored composed flat add receivers",
                            )?;
                            let paid_many::Targets::PaidMany(flat) = targets else {
                                unreachable!("paid flat arm")
                            };
                            if !paid_many::add(function, budget, flat, *child, *weight)? {
                                return Ok(None);
                            }
                        }
                        paid_many::Targets::Many(targets) => {
                            if !targets.contains_key(child) {
                                budget.reserve_btree_node::<usize, u64>(
                                    function,
                                    "factored successor target node",
                                )?;
                            }
                            let slot = targets.entry(*child).or_insert(0);
                            let Some(sum) = slot.checked_add(*weight) else {
                                return Ok(None);
                            };
                            *slot = sum;
                        }
                    }
                }
            }
        }
        budget.reserve_array::<(Label, SharedWeighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        // Do not initialize an owning map iterator or its receiving tuple on
        // the empty route. Zero-byte vector authentication above still runs.
        if !successors.is_empty() {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<
                        std::collections::btree_map::IntoIter<Label, paid_many::Targets>,
                    >() + std::mem::size_of::<Option<(Label, paid_many::Targets)>>()
                        + std::mem::size_of::<(Label, paid_many::Targets)>(),
                    "factored successor target iterator receiving",
                )?;
            }
            for (label, targets) in successors {
                if budget.pays_allocations() {
                    budget.reserve_bytes(
                        function,
                        2 * std::mem::size_of::<SharedWeighted>()
                            + std::mem::size_of::<(usize, u64)>(),
                        "factored inline successor finish receiving",
                    )?;
                }
                budget.reserve_bytes(
                    function,
                    0,
                    "factored successor target finish authentication",
                )?;
                let state = match targets {
                    paid_many::Targets::One { child, weight } => {
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                2 * std::mem::size_of::<SmallWeighted>()
                                    + std::mem::size_of::<[(usize, u64); 1]>()
                                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                                "factored inline weighted enum and array receiving",
                            )?;
                        }
                        budget.reserve_bytes(
                            function,
                            physical_clone::rc_backing_bytes::<SmallWeighted>(),
                            "factored weighted shared header",
                        )?;
                        let singleton = SmallWeighted::One([(child, weight)]);
                        Rc::new(singleton)
                    }
                    paid_many::Targets::PaidMany(_) => match budget {
                        ReplayBudget {
                            payer: AllocationPayer::LegacyUnobserved,
                            ..
                        } => {
                            budget.reserve_bytes(
                                function,
                                paid_many::FINISH_RECEIVING,
                                "factored composed flat finish receivers",
                            )?;
                            let paid_many::Targets::PaidMany(flat) = targets else {
                                unreachable!("paid flat finish")
                            };
                            own_weighted(function, budget, flat)?
                        }
                        _ => paid_many::finish!(function, budget, targets),
                    },
                    paid_many::Targets::Many(targets) => {
                        // The remaining fallible weighted producer still has
                        // its real map argument and Result receiving carrier.
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                std::mem::size_of::<BTreeMap<usize, u64>>()
                                    + std::mem::size_of::<Vec<(usize, u64)>>()
                                    + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>(),
                                "factored many weighted producer receiving",
                            )?;
                        }
                        weighted_successor(function, budget, targets)?
                    }
                };
                result.push((label, state));
            }
        }
        Ok(Some((accepted, result)))
    }

    pub(in crate::cleanup_plan::replay::factored) fn id_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        let _successor_invocation = lexical_successors::census::invocation(false, function, budget);
        #[cfg(test)]
        let _operation = crate::bounded_output::stage_debits::operation_guard(
            crate::bounded_output::stage_debits::Operation::ReplayFactoredCanonical,
        );
        #[cfg(feature = "unstable-workflow-profiling")]
        let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
            crate::bounded_output::exclusive_debits::Operation::ReplayFactoredCanonical,
        );
        #[cfg(test)]
        super::tests::note_actual_entry(budget);
        // Authenticate even when states already contains the requested root.
        // One equivalent() call holds this function and budget borrowed across
        // both roots; no closure/cache view leaves that invocation.
        // Validate the held destination before any ordinary payer query.
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        authenticate_closure!(self, function, budget, receiving);
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Weighted>(),
                "factored canonical moved root",
            )?;
        }
        // Keep first-use acquisition at its original lexical position, but
        // construct no forwarding prepare_scratch Result or argument receivers.
        if !self.scratch_paid {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Vec<(usize, bool)>>()
                        + std::mem::size_of::<Vec<CanonicalFrame<'_>>>()
                        + std::mem::size_of::<Vec<SharedWeighted>>()
                        + std::mem::size_of::<bool>(),
                    "factored retained canonical scratch controls",
                )?;
            }
            self.scratch_paid = true;
        }
        self.id_inner_into(function, root, budget, receiving);
        // Retain capacity, never owned references, across either root and all
        // success, arithmetic, work, physical and semantic failure exits.
        self.cursor.clear();
        self.child_ids.clear();
        self.frames.clear();
        self.open.clear();
        receiving.ready = false;
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if receiving.overflow {
            return Ok(None);
        }
        Ok(Some(receiving.id))
    }
}

#[macro_use]
mod lexical_successors;
mod inline_exit;
mod terminal_leaf;
#[cfg(test)]
mod tests;

const _: () = assert!(
    std::mem::size_of::<Canonicalizer<'static>>()
        == std::mem::size_of::<super::Canonicalizer<'static>>()
);
const _: () =
    assert!(Canonicalizer::CONSTRUCTOR_CONTROLS == super::Canonicalizer::CONSTRUCTOR_CONTROLS);
