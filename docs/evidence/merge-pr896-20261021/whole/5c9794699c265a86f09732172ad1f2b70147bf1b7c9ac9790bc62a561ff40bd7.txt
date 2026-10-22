//! The actual retained canonical frame-loop method, not a forwarding adapter.
use super::super::super::*;

// Preserve both complete immediate-predecessor macros in this independently
// compiled historical control; lexical macro scope does not cross registration.
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

impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn before_borrowed_single_id_inner_into(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) {
        macro_rules! attempt {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    Err(error) => {
                        receiving.refusal = Some(error);
                        receiving.ready = true;
                        return;
                    }
                }
            };
        }
        self.successor_unit_paid = false;
        self.successor_general_paid = false;
        receiving.overflow = false;
        attempt!(budget.reserve_vec(
            function,
            &mut self.frames,
            1,
            "factored canonical initial frame",
        ));
        // This direct root construction has no Diagnostic-bearing helper Result.
        // Both enum values, actual Rc initializer/receiving values and payment
        // Result remain real and prepaid; the general successor helper is unchanged.
        if budget.pays_allocations() {
            attempt!(budget.reserve_bytes(
                function,
                2 * std::mem::size_of::<SmallWeighted>()
                    + 2 * std::mem::size_of::<SharedWeighted>()
                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                "factored direct root enum receiving",
            ));
        }
        attempt!(budget.reserve_bytes(
            function,
            physical_clone::rc_backing_bytes::<SmallWeighted>(),
            "factored weighted shared header",
        ));
        let shared = SmallWeighted::Many(root);
        let root = Rc::new(shared);
        let root_copy = attempt!(share_weighted(function, budget, &root));
        self.frames.push(CanonicalFrame::Enter(root_copy));
        while let Some(frame) = self.frames.pop() {
            attempt!(budget.charge(function, 1, "factored canonical frame"));
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if self.open.contains(&state) {
                        receiving.refusal =
                            Some(replay_error(function, "cleanup CFG contains a cycle"));
                        receiving.ready = true;
                        return;
                    }
                    attempt!(budget.reserve_vec(
                        function,
                        &mut self.open,
                        1,
                        "factored retained open-state identity slots",
                    ));
                    self.open
                        .push(attempt!(share_weighted(function, budget, &state)));
                    let Some((accepted, successors)) = (match budget {
                        ReplayBudget {
                            payer: AllocationPayer::ActiveBuilder,
                            ..
                        } => lexical_successors!(self, function, state, budget, receiving, attempt),
                        _ => attempt!(
                            self.successors_with_receiving(function, &state, budget, receiving)
                        ),
                    }) else {
                        receiving.finish_overflow();
                        return;
                    };
                    let child_count = successors.len();
                    attempt!(budget.reserve_vec(
                        function,
                        &mut self.frames,
                        1,
                        "factored canonical exit frame",
                    ));
                    let exit_index = self.frames.len();
                    self.frames
                        .push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &self.frames[exit_index]
                        else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child =
                            attempt!(share_weighted(function, budget, &successors[index].1));
                        attempt!(budget.reserve_vec(
                            function,
                            &mut self.frames,
                            1,
                            "factored canonical child frames",
                        ));
                        self.frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    self.open.retain(|entry| entry != &state);
                    attempt!(budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    ));
                    // The old signature-work debit above remains first. Acquire
                    // the actual new branch receivers before evaluating the guard.
                    if budget.pays_allocations() {
                        attempt!(budget.reserve_bytes(
                            function,
                            empty_signature::BRANCH_CONTROLS,
                            "canonical empty signature branch receiving",
                        ));
                    }
                    if successors.is_empty() {
                        if budget.pays_allocations() {
                            attempt!(budget.reserve_bytes(
                                function,
                                empty_signature::EMPTY_CONTROLS,
                                "canonical empty signature receiving",
                            ));
                        }
                        self.child_ids.clear();
                        // Retain the original key-vector zero authentication;
                        // no child directory, borrowed query or Zip exists here.
                        attempt!(budget.reserve_array::<(Label, usize)>(
                            function,
                            0,
                            "factored canonical signature vector",
                        ));
                        let key = (accepted, Vec::new());
                        let id = match self.ids.get(&key) {
                            Some(id) => {
                                #[cfg(test)]
                                arena_tests::signature_lookup(true);
                                *id
                            }
                            None => {
                                #[cfg(test)]
                                arena_tests::signature_lookup(false);
                                intern_arena::prepare_insert!(
                                    self.ids,
                                    intern_arena::Signature,
                                    function,
                                    budget
                                );
                                let id = self.ids.len();
                                self.ids.insert_prepaid(key, id);
                                id
                            }
                        };
                        self.child_ids.clear();
                        if !self.states.contains_key(&state) {
                            intern_arena::prepare_insert!(
                                self.states,
                                SharedWeighted,
                                function,
                                budget
                            );
                        }
                        self.states.insert_prepaid(state, id);
                        continue;
                    }
                    attempt!(budget.reserve_bytes(
                        function,
                        intern_arena::SignatureQuery::LEXICAL_EXIT_CONTROLS,
                        "canonical borrowed signature receiving",
                    ));
                    // Retained scalar scratch contains only the child IDs for
                    // this frame; the epsilon cursor keeps its original shape.
                    self.child_ids.clear();
                    lexical_child_scratch::prepare!(self, successors, function, budget, receiving);
                    attempt!(budget.reserve_array::<[usize; 2]>(
                        function,
                        successors.len(),
                        "canonical child identity receiving",
                    ));
                    // Resolve every ID before signature comparison can stop at
                    // its first mismatch. This preserves the original failure order.
                    for (_, child) in &successors {
                        let Some(id) = self.states.get(child) else {
                            receiving.refusal =
                                Some(replay_error(function, "cleanup CFG contains a cycle"));
                            receiving.ready = true;
                            return;
                        };
                        self.child_ids.push(*id);
                    }
                    let query = intern_arena::SignatureQuery {
                        accepted,
                        successors: &successors,
                        children: &self.child_ids,
                    };
                    let id = match self.ids.get(&query) {
                        Some(id) => {
                            #[cfg(test)]
                            arena_tests::signature_lookup(true);
                            *id
                        }
                        None => {
                            #[cfg(test)]
                            arena_tests::signature_lookup(false);
                            attempt!(budget.reserve_array::<(Label, usize)>(
                                function,
                                successors.len(),
                                "factored canonical signature vector",
                            ));
                            // Every ID is already resolved. Move each original
                            // label with its borrowed ID, without an owned
                            // scalar cursor or repeated index/increment operands.
                            attempt!(budget.reserve_bytes(
                                function,
                                intern_arena::SignatureQuery::MISS_CONTROLS,
                                "canonical signature miss iterator receiving",
                            ));
                            attempt!(budget.reserve_array::<[&usize; 2]>(
                                function,
                                successors.len(),
                                "canonical signature miss borrowed IDs",
                            ));
                            let mut key = Vec::with_capacity(successors.len());
                            for ((label, _child), id) in successors.into_iter().zip(&self.child_ids)
                            {
                                key.push((label, *id));
                            }
                            let key = (accepted, key);
                            intern_arena::prepare_insert!(
                                self.ids,
                                intern_arena::Signature,
                                function,
                                budget
                            );
                            let id = self.ids.len();
                            self.ids.insert_prepaid(key, id);
                            id
                        }
                    };
                    self.child_ids.clear();
                    if !self.states.contains_key(&state) {
                        intern_arena::prepare_insert!(
                            self.states,
                            SharedWeighted,
                            function,
                            budget
                        );
                    }
                    self.states.insert_prepaid(state, id);
                }
            }
        }
        match self.states.get(&root) {
            Some(id) => receiving.id = *id,
            None => receiving.overflow = true,
        }
        receiving.ready = true;
    }
}

impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn before_borrowed_single_id_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        let _operation = crate::bounded_output::stage_debits::operation_guard(
            crate::bounded_output::stage_debits::Operation::ReplayFactoredCanonical,
        );
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
                        + std::mem::size_of::<Vec<CanonicalFrame>>()
                        + std::mem::size_of::<Vec<SharedWeighted>>()
                        + std::mem::size_of::<bool>(),
                    "factored retained canonical scratch controls",
                )?;
            }
            self.scratch_paid = true;
        }
        self.before_borrowed_single_id_inner_into(function, root, budget, receiving);
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

// Whole immediate pre-singleton entry; identifier-only namespace glue.
impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn matched_884_id(
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
        self.before_borrowed_single_id_with_receiving(function, root, budget, &mut receiving)
    }
}
