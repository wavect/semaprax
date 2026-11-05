//! The actual retained canonical frame-loop method, not a forwarding adapter.
use super::super::*;
#[cfg(test)]
mod before_arity_dispatch;
#[cfg(test)]
mod before_borrowed_single;
mod borrowed_single;

impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn id_inner_into(
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
                    + std::mem::size_of::<Result<(), Diagnostic>>()
                    + borrowed_single::LOCAL_CONTROLS,
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
        // The original direct-root admission above prepaid both bool values.
        // This one primitive frame belongs to this nonrecursive activation.
        let mut exit_admission_paid = false;
        // No initializer value is formed. This stable receiving place retains
        // the ORIGINAL Vec::len result used by the signature-work call.
        let mut exit_arity: usize;
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
                CanonicalFrame::Exit(state, accepted, mut successors) => {
                    self.open.retain(|entry| entry != &state);
                    exit_arity = successors.len();
                    attempt!(budget.charge(
                        function,
                        exit_arity,
                        "factored canonical signature construction",
                    ));
                    // The pattern reads the retained place: no extra len(),
                    // moved scrutinee local, query or helper return exists.
                    match exit_arity {
                        1 => {
                            #[cfg(test)]
                            borrowed_single::note(budget, exit_admission_paid);
                            attempt!(budget.reserve_bytes(
                                function,
                                if exit_admission_paid {
                                    borrowed_single::WARM
                                } else {
                                    borrowed_single::COLD
                                },
                                "canonical lexical exit receiving",
                            ));
                            exit_admission_paid = true;
                            // One original iterator.next, without an Index or slice
                            // helper. Its temporary iterator ends before remove(0).
                            let Some((label, child)) = successors.iter().next() else {
                                unreachable!("one successor was matched");
                            };
                            let Some(child_id) = self.states.get(child) else {
                                receiving.refusal =
                                    Some(replay_error(function, "cleanup CFG contains a cycle"));
                                receiving.ready = true;
                                return;
                            };
                            let query = borrowed_single::OneQuery {
                                accepted,
                                label,
                                child: child_id,
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
                                        1,
                                        "factored canonical signature vector",
                                    ));
                                    let mut key = Vec::with_capacity(1);
                                    {
                                        let (label, _child) = successors.remove(0);
                                        key.push((label, *child_id));
                                    }
                                    // Same owned Vec input slots as the original
                                    // into_iter hand-off; release its buffer before
                                    // either arena can acquire backing, as before.
                                    drop(successors);
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
                            continue;
                        }
                        _ => {}
                    }
                    // Empty/general routes retain their complete original
                    // admission, predicate, query/scratch/miss and Drop order.
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
