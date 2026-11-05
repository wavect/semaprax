//! Complete immediate886 frame traversal and exact original role banks.
use super::super::super::*;

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

#[allow(
    dead_code,
    reason = "complete independently compiled immediate886 query and diagnostic fields"
)]
mod borrowed_single {
    //! One lexical Exit admission; original shared query/API/tariffs stay intact.
    use super::*;
    use std::cmp::Ordering;
    use std::mem::size_of;

    pub(super) struct OneQuery<'a> {
        pub(super) accepted: u64,
        pub(super) label: &'a Label,
        pub(super) child: &'a usize,
    }
    const _: () = assert!(
        size_of::<OneQuery<'static>>() == size_of::<intern_arena::SignatureQuery<'static>>()
    );
    const _: () = assert!(
        size_of::<&OneQuery<'static>>() == size_of::<&intern_arena::SignatureQuery<'static>>()
    );
    // Same Query::compare ABI as the complete old receiver; no constructor helper,
    // forwarding Self/Diagnostic return, payload copy or additional lookup exists.
    impl intern_arena::Query<intern_arena::Signature> for OneQuery<'_> {
        fn compare(&self, stored: &intern_arena::Signature) -> Ordering {
            let order = self.accepted.cmp(&stored.0);
            if order != Ordering::Equal {
                return order;
            }
            if stored.1.is_empty() {
                return Ordering::Greater;
            }
            let order = self.label.cmp(&stored.1[0].0);
            if order != Ordering::Equal {
                return order;
            }
            let order = self.child.cmp(&stored.1[0].1);
            if order != Ordering::Equal {
                return order;
            }
            1_usize.cmp(&stored.1.len())
        }
    }
    pub(super) const LOCAL_CONTROLS: usize = 2 * size_of::<bool>();
    // Exactly the full conservative primitive ABI: actual/formal arguments and
    // returned/received unit-success Result slots. Only this nonrecursive lexical
    // primitive place is retained; no compound query/Option/key/iterator is reused.
    pub(super) const PRIMITIVE: usize =
        2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 2 * size_of::<Result<(), Diagnostic>>();
    const QUERY: usize =
        2 * size_of::<OneQuery<'static>>() + 2 * size_of::<&Label>() + 2 * size_of::<&usize>();
    // Complete old successor iterator/borrow bank remains fresh. Its first next
    // call replaces the original loop's first next with identical Option/item ABI.
    const ITERATOR: usize = 2 * size_of::<std::slice::Iter<'static, (Label, SharedWeighted)>>()
        + 2 * size_of::<&Vec<(Label, SharedWeighted)>>()
        + size_of::<&[(Label, SharedWeighted)]>();
    // Vec::remove has both mutable receiver/index arguments, returned/received
    // owned row, bounds length/predicate and the copied scalar child key. Its
    // inner row is dropped before signature insertion, as in the original loop.
    const MOVE: usize = 2 * size_of::<(Label, SharedWeighted)>()
        + 2 * size_of::<&mut Vec<(Label, SharedWeighted)>>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<usize>();
    const GUARD: usize = 2 * size_of::<&Vec<(Label, SharedWeighted)>>()
        + 2 * size_of::<usize>()
        + 4 * size_of::<bool>();
    const SELECT: usize = 4 * size_of::<bool>() + 2 * size_of::<usize>();
    // This conservative whole-Exit bound is acquired on every Exit, including
    // empty/general branches. The complete general path pays all its additional
    // original requests; there is no subtraction or old-baseline tariff change.
    pub(super) const FRESH: usize = QUERY + ITERATOR + MOVE + GUARD;
    pub(super) const WARM: usize = FRESH + SELECT;
    pub(super) const COLD: usize = FRESH + SELECT + PRIMITIVE;

    #[cfg(test)]
    #[derive(Clone, Copy, Debug)]
    pub(super) struct Event {
        pub(super) cold: bool,
        pub(super) requested: usize,
        pub(super) paid: usize,
        pub(super) remaining: usize,
        pub(super) skeleton_remaining: usize,
        pub(super) skeleton_spent: usize,
    }
    #[cfg(test)]
    thread_local! {
        static EVENTS: std::cell::Cell<(bool, [Option<Event>; 16], usize)> = const {
            std::cell::Cell::new((false, [None; 16], 0))
        };
    }
    #[cfg(test)]
    pub(super) fn note(budget: &ReplayBudget, warm: bool) {
        EVENTS.with(|slot| {
            let (active, mut events, count) = slot.get();
            if !active {
                return;
            }
            if count < events.len() {
                events[count] = Some(Event {
                    cold: !warm,
                    requested: if warm { WARM } else { COLD },
                    paid: budget.physical.as_ref().unwrap().charged(),
                    remaining: budget.remaining,
                    skeleton_remaining: budget.skeleton_remaining,
                    skeleton_spent: budget.skeleton_spent,
                });
            }
            slot.set((active, events, count.saturating_add(1)));
        });
    }

    // Test-only exact role equations; no runtime fee helper or external authority.
    #[cfg(test)]
    impl Canonicalizer<'_> {
        pub(in crate::cleanup_plan::replay::factored::canonical_shared) const BEFORE_ARITY_BORROWED_EXIT_PRIMITIVE: usize = PRIMITIVE;
        pub(in crate::cleanup_plan::replay::factored::canonical_shared) const BEFORE_ARITY_BORROWED_EXIT_WARM: usize = WARM;
        pub(in crate::cleanup_plan::replay::factored::canonical_shared) const BEFORE_ARITY_BORROWED_EXIT_LOCAL: usize = LOCAL_CONTROLS;
    }
}

impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn before_arity_dispatch_id_inner_into(
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
                    attempt!(budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    ));
                    // One real primitive admission replaces the old separate
                    // branch/query/miss positive calls on the singleton route.
                    // Every compound query/iterator/move role remains fresh.
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
                    match successors.len() {
                        0 => {
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
                        1 => {
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
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn before_arity_dispatch_id_with_receiving(
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
        self.before_arity_dispatch_id_inner_into(function, root, budget, receiving);
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

#[test]
fn complete_immediate_886_method_roles_and_wrapper_keep_whole_source_after_only_namespace_glue() {
    const BASE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/arity-dispatch-duplicate-arena-20261017/whole-before/"
    );
    let frame = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/arity-dispatch-duplicate-arena-20261017/whole-before/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop.rs"));
    let roles = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/arity-dispatch-duplicate-arena-20261017/whole-before/src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/borrowed_single.rs"));
    let root = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/arity-dispatch-duplicate-arena-20261017/whole-before/src/cleanup_plan/replay/factored/canonical_shared.rs"));
    let macros = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/borrowed-single-macro-scope-fix-20261017/original-authentication-macros.rs.txt"));
    assert!(!BASE.is_empty());
    let method = frame[frame.find("impl Canonicalizer").unwrap()..].replacen(
        "fn id_inner_into(",
        "fn before_arity_dispatch_id_inner_into(",
        1,
    );
    let mut roles = roles.replacen("#[cfg(test)]\nmod tests;\n", "", 1);
    for name in ["PRIMITIVE", "WARM", "LOCAL"] {
        roles = roles.replace(
            &format!("const BORROWED_EXIT_{name}"),
            &format!("const BEFORE_ARITY_BORROWED_EXIT_{name}"),
        );
    }
    let start = root.find("    pub(super) fn id_with_receiving(").unwrap();
    let end = root[start..]
        .find("\n}\n\n#[cfg(test)]\nmod tests;")
        .unwrap()
        + start;
    let wrapper = root[start..end]
        .replacen("pub(super) fn id_with_receiving(", "pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn before_arity_dispatch_id_with_receiving(", 1)
        .replacen("self.id_inner_into(", "self.before_arity_dispatch_id_inner_into(", 1);
    let mut expected = String::from("//! Complete immediate886 frame traversal and exact original role banks.\nuse super::super::super::*;\n\n");
    expected.push_str(macros);
    expected.push_str("\n#[allow(dead_code, reason = \"complete independently compiled immediate886 query and diagnostic fields\")]\nmod borrowed_single {\n");
    expected.push_str(&roles);
    expected.push_str("\n}\n\n");
    expected.push_str(&method);
    expected.push_str("\nimpl Canonicalizer<'_> {\n");
    expected.push_str(&wrapper);
    expected.push_str("\n}\n");
    let current = include_str!("before_arity_dispatch.rs");
    let prefix = current.split("\n#[test]\nfn complete_immediate_886_method_roles_and_wrapper_keep_whole_source_after_only_namespace_glue").next().unwrap();
    assert_eq!(
        prefix.trim_end(),
        expected.trim_end(),
        "entire immediate886 method, exact roles, auth macros and wrapper"
    );
}
