//! Complete immediate2d4b receiver. Only module wiring adapts the namespace.
use super::{
    app_census, empty_closure, inline_weighted, ordered_alias, ordered_weights, successor_targets,
    tests,
};
use super::{
    physical_clone, replay_error, Automaton, BTreeMap, Diagnostic, Label, Rc, ReplayBudget,
    ResolvedFunction, Weighted,
};
use inline_weighted::SmallWeighted;
#[path = "before_unit/closure_receiving.rs"]
mod closure_receiving;
#[path = "before_unit/empty_memo.rs"]
mod empty_memo;
type SharedWeighted = Rc<SmallWeighted>;

enum CanonicalFrame {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
}

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
pub(super) struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: empty_memo::RetainedMemo<Rc<ordered_weights::OrderedWeights>>,
    states: BTreeMap<SharedWeighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
    cursor: Vec<(usize, bool)>,
    frames: Vec<CanonicalFrame>,
    open: Vec<SharedWeighted>,
    scratch_paid: bool,
}

impl<'a> Canonicalizer<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: empty_memo::RetainedMemo::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
            cursor: Vec::new(),
            frames: Vec::new(),
            open: Vec::new(),
            scratch_paid: false,
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
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
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
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
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
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
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
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
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
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            receiving.refusal = Some(self.closures.failure());
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
                closure_receiving::ClosureReceiving::QUERY_CONTROLS
                    + std::mem::size_of::<Result<(), Diagnostic>>()
                    + std::mem::size_of::<usize>(),
                "factored held closure producer and take controls",
            ) {
                receiving.refusal = Some(error);
                receiving.ready = true;
                return;
            }
        }
        if let Err(error) = self.prepare_scratch(function, budget) {
            receiving.refusal = Some(error);
            receiving.ready = true;
            return;
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
                        attempt!(self.closures.insert_empty_retained(function, node, budget));
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
                attempt!(self
                    .closures
                    .insert_retained(function, node, budget, || shared));
                #[cfg(test)]
                app_census::published();
                continue;
            }
            if node != self.sink && state.moves().is_empty() && state.epsilon().is_empty() {
                attempt!(budget.charge(function, 1, "factored skeleton closure"));
                attempt!(self.closures.insert_empty_retained(function, node, budget));
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
            attempt!(self
                .closures
                .insert_retained(function, node, budget, || Rc::new(closure)));
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
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<BTreeMap<Label, successor_targets::Targets>>()
                        + std::mem::size_of::<Option<&mut successor_targets::Targets>>(),
                    "factored successor label receiving controls",
                )?;
            }
            let mut successors = BTreeMap::<Label, successor_targets::Targets>::new();
            for (node, weight) in closed.iter() {
                let moves = self.automaton.nodes[*node].moves();
                budget.charge(
                    function,
                    moves.len().saturating_add(1),
                    "factored skeleton subset construction",
                )?;
                for (label, child) in moves {
                    if !successors.contains_key(label) {
                        budget.reserve_btree_node::<Label, successor_targets::Targets>(
                            function,
                            "factored successor label node",
                        )?;
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                2 * std::mem::size_of::<successor_targets::Targets>()
                                    + std::mem::size_of::<Option<u64>>()
                                    + std::mem::size_of::<Option<successor_targets::Targets>>(),
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
                            successor_targets::Targets::One {
                                child: *child,
                                weight,
                            },
                        );
                    } else {
                        let targets = successors.get_mut(label).expect("existing label");
                        match targets {
                            successor_targets::Targets::One {
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
                            successor_targets::Targets::One {
                                child: first,
                                weight: total,
                            } => {
                                assert_ne!(*first, *child);
                                if budget.pays_allocations() {
                                    budget.reserve_bytes(
                                        function,
                                        2 * std::mem::size_of::<BTreeMap<usize, u64>>()
                                            + std::mem::size_of::<successor_targets::Targets>()
                                            + 2 * std::mem::size_of::<Option<u64>>()
                                            + 2 * std::mem::size_of::<(usize, u64)>(),
                                        "factored inline successor promotion carriers",
                                    )?;
                                }
                                budget.reserve_bytes(
                                    function,
                                    0,
                                    "factored successor target promotion authentication",
                                )?;
                                budget.reserve_btree_node::<usize, u64>(
                                    function,
                                    "factored successor target node",
                                )?;
                                budget.reserve_btree_node::<usize, u64>(
                                    function,
                                    "factored successor target node",
                                )?;
                                let mut promoted = BTreeMap::new();
                                promoted.insert(*first, *total);
                                let sum = 0_u64
                                    .checked_add(*weight)
                                    .expect("zero plus u64 cannot overflow");
                                promoted.insert(*child, sum);
                                *targets = successor_targets::Targets::Many(promoted);
                            }
                            successor_targets::Targets::Many(targets) => {
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
                            std::collections::btree_map::IntoIter<
                                Label,
                                successor_targets::Targets,
                            >,
                        >() + std::mem::size_of::<Option<(Label, successor_targets::Targets)>>()
                            + std::mem::size_of::<(Label, successor_targets::Targets)>(),
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
                        successor_targets::Targets::One { child, weight } => {
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
                        successor_targets::Targets::Many(targets) => {
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
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<BTreeMap<Label, successor_targets::Targets>>()
                    + std::mem::size_of::<Option<&mut successor_targets::Targets>>(),
                "factored successor label receiving controls",
            )?;
        }
        let mut successors = BTreeMap::<Label, successor_targets::Targets>::new();
        for (node, weight) in closed.iter() {
            let moves = self.automaton.nodes[*node].moves();
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, successor_targets::Targets>(
                        function,
                        "factored successor label node",
                    )?;
                    if budget.pays_allocations() {
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<successor_targets::Targets>()
                                + std::mem::size_of::<Option<u64>>()
                                + std::mem::size_of::<Option<successor_targets::Targets>>(),
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
                        successor_targets::Targets::One {
                            child: *child,
                            weight,
                        },
                    );
                } else {
                    let targets = successors.get_mut(label).expect("existing label");
                    match targets {
                        successor_targets::Targets::One {
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
                        successor_targets::Targets::One {
                            child: first,
                            weight: total,
                        } => {
                            assert_ne!(*first, *child);
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    2 * std::mem::size_of::<BTreeMap<usize, u64>>()
                                        + std::mem::size_of::<successor_targets::Targets>()
                                        + 2 * std::mem::size_of::<Option<u64>>()
                                        + 2 * std::mem::size_of::<(usize, u64)>(),
                                    "factored inline successor promotion carriers",
                                )?;
                            }
                            budget.reserve_bytes(
                                function,
                                0,
                                "factored successor target promotion authentication",
                            )?;
                            budget.reserve_btree_node::<usize, u64>(
                                function,
                                "factored successor target node",
                            )?;
                            budget.reserve_btree_node::<usize, u64>(
                                function,
                                "factored successor target node",
                            )?;
                            let mut promoted = BTreeMap::new();
                            promoted.insert(*first, *total);
                            let sum = 0_u64
                                .checked_add(*weight)
                                .expect("zero plus u64 cannot overflow");
                            promoted.insert(*child, sum);
                            *targets = successor_targets::Targets::Many(promoted);
                        }
                        successor_targets::Targets::Many(targets) => {
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
                        std::collections::btree_map::IntoIter<Label, successor_targets::Targets>,
                    >() + std::mem::size_of::<Option<(Label, successor_targets::Targets)>>()
                        + std::mem::size_of::<(Label, successor_targets::Targets)>(),
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
                    successor_targets::Targets::One { child, weight } => {
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
                    successor_targets::Targets::Many(targets) => {
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

    pub(super) fn id_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        tests::note_actual_entry(budget);
        // Authenticate even when states already contains the requested root.
        // One equivalent() call holds this function and budget borrowed across
        // both roots; no closure/cache view leaves that invocation.
        // Validate the held destination before any ordinary payer query.
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Result<Option<usize>, Diagnostic>>()
                    + std::mem::size_of::<Weighted>()
                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                "factored canonical scratch receiving and moved root",
            )?;
        }
        self.prepare_scratch(function, budget)?;
        let result = self.id_inner(function, root, budget, receiving);
        // Retain capacity, never owned references, across either root and all
        // success, arithmetic, work, physical and semantic failure exits.
        self.cursor.clear();
        self.frames.clear();
        self.open.clear();
        result
    }

    fn prepare_scratch(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if !self.scratch_paid {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Vec<(usize, bool)>>()
                        + std::mem::size_of::<Vec<CanonicalFrame>>()
                        + std::mem::size_of::<Vec<SharedWeighted>>()
                        + std::mem::size_of::<bool>(),
                    "factored retained canonical scratch controls",
                )?;
            }
            self.scratch_paid = true;
        }
        Ok(())
    }

    fn id_inner(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_vec(
            function,
            &mut self.frames,
            1,
            "factored canonical initial frame",
        )?;
        // This direct root construction has no Diagnostic-bearing helper Result.
        // Both enum values, actual Rc initializer/receiving values and payment
        // Result remain real and prepaid; the general successor helper is unchanged.
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                2 * std::mem::size_of::<SmallWeighted>()
                    + 2 * std::mem::size_of::<SharedWeighted>()
                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                "factored direct root enum receiving",
            )?;
        }
        budget.reserve_bytes(
            function,
            physical_clone::rc_backing_bytes::<SmallWeighted>(),
            "factored weighted shared header",
        )?;
        let shared = SmallWeighted::Many(root);
        let root = Rc::new(shared);
        let root_copy = share_weighted(function, budget, &root)?;
        self.frames.push(CanonicalFrame::Enter(root_copy));
        while let Some(frame) = self.frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if self.open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_vec(
                        function,
                        &mut self.open,
                        1,
                        "factored retained open-state identity slots",
                    )?;
                    self.open.push(share_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) =
                        self.successors_with_receiving(function, &state, budget, receiving)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut self.frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = self.frames.len();
                    self.frames
                        .push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &self.frames[exit_index]
                        else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child = share_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut self.frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        self.frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    self.open.retain(|entry| entry != &state);
                    budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    )?;
                    budget.reserve_array::<(Label, usize)>(
                        function,
                        successors.len(),
                        "factored canonical signature vector",
                    )?;
                    let mut key = Vec::with_capacity(successors.len());
                    for (label, child) in successors {
                        let Some(id) = self.states.get(&child) else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let key = (accepted, key);
                    let id = match self.ids.get(&key) {
                        Some(id) => *id,
                        None => {
                            budget.reserve_btree_node::<(u64, Vec<(Label, usize)>), usize>(
                                function,
                                "factored canonical signature node",
                            )?;
                            let id = self.ids.len();
                            self.ids.insert(key, id);
                            id
                        }
                    };
                    if !self.states.contains_key(&state) {
                        budget.reserve_btree_node::<SharedWeighted, usize>(
                            function,
                            "factored canonical state memo node",
                        )?;
                    }
                    self.states.insert(state, id);
                }
            }
        }
        Ok(self.states.get(&root).copied())
    }
}

impl Canonicalizer<'_> {
    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> {
        &self.ids
    }
    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> {
        &self.states
    }
}
