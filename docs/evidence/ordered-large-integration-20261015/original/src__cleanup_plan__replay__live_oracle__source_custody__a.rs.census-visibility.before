use super::undo;
#[test]
fn complete_original_sources_and_compiled_owning_receivers_remain_exact() {
    {
        let mut current =
            include_str!("../../../../bounded_output/classifier_string.rs").to_owned();
        undo(
            &mut current,
            r#"mod explicit;

mod inventory_work;

#[cfg(test)]
mod oracle_work;
"#,
            r#"mod explicit;

mod inventory_work;
"#,
        );
        undo(
            &mut current,
            r#"/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);
impl ClassifierLedger {
    /// A test-account caller authenticates its own invocation separately. This
    /// detached marker contains no ACTIVE handle and grants no ordinary payer.
    #[cfg(test)]
    pub(crate) fn oracle_marker() -> Self {
        Self(None)
    }

    pub(crate) fn capture() -> Self {
        Self(active())
    }
"#,
            r#"/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);
impl ClassifierLedger {
    pub(crate) fn capture() -> Self {
        Self(active())
    }
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__bounded_output__classifier_string.rs.before"
            ),
            "src/bounded_output/classifier_string.rs"
        );
    }
    {
        let mut current = include_str!("../../../../cleanup_plan.rs").to_owned(); undo(&mut current, "\n#[cfg(test)]\npub(crate) use replay::EpsilonAppCensus;\n", "");
        undo(
            &mut current,
            r#"
// Shared pinned compiler container layout, not cleanup authority.
pub(crate) use replay::replay_btree_node_upper as compiler_btree_node_upper;

#[cfg(test)]
pub(crate) use replay::with_financed_test_oracles;
"#,
            r#"
// Shared pinned compiler container layout, not cleanup authority.
pub(crate) use replay::replay_btree_node_upper as compiler_btree_node_upper;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan.rs.before"
            ),
            "src/cleanup_plan.rs"
        );
    }
    {
        let mut current = include_str!("../../../replay.rs").to_owned(); undo(&mut current, "    let status_cases = expression_facts::receiving::StatusCases::validate(program, function, budget)?;", "    validate_required_status_sources(program, function)?;"); undo(&mut current, "    let statuses = expression_facts::receiving::validate_status_sources_certified(\n        function, &expression_facts, budget, &status_cases,", "    let statuses = expression_facts::receiving::validate_status_sources_retained(\n        function, &expression_facts, budget,"); undo(&mut current, "\n#[cfg(test)]\npub(crate) use factored::EpsilonAppCensus;\n", ""); undo(&mut current, "    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;\n", ""); undo(&mut current, "    let mut budget = ReplayBudget::new();\n    let helpers = ordinary_renewals::index(full, &mut budget)?;", "    let mut budget = ReplayBudget::new();\n    let mut continuations = continuation_storage::Storage::new(full);\n    let helpers = ordinary_renewals::index(full, &mut budget)?;");
        undo(&mut current, "    expression_facts::receiving::validate_exits_retained(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n        &statuses,\n    )?;\n", "    expression_facts::receiving::validate_exits(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n    )?;\n");
        undo(&mut current,
            r#"    let statuses = expression_facts::receiving::validate_status_sources_retained(
        function, &expression_facts, budget,
    )?;"#,
            r#"    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;"#);
        undo(&mut current, "    expression_facts::receiving::validate_blocks_and_edges_retained(\n",
            "    expression_facts::receiving::validate_blocks_and_edges(\n");
        undo(&mut current, "        budget,\n        &statuses,\n    )?;",
            "        budget,\n    )?;");
        undo(&mut current, "pub(crate) use live_oracle::with_financed_test_oracles;", "pub(super) use live_oracle::with_financed_test_oracles;");
        undo(
            &mut current,
            r#"
#[cfg(test)]
pub(super) mod phase_measurement;

#[cfg(test)]
mod live_oracle;
#[cfg(test)]
pub(super) use live_oracle::with_financed_test_oracles;
"#,
            r#"
#[cfg(test)]
pub(super) mod phase_measurement;
"#,
        );
        undo(
            &mut current,
            r####"    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;
    expression_facts::receiving::validate_regions(function, &storage, budget)?;
    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;
    expression_facts::receiving::validate_blocks_and_edges(
"####,
            r####"    expression_facts::receiving::validate_status_sources(function, &expression_facts, budget)?;
    validate_regions(function, &storage)?;
    validate_entry(function, &storage, &leaves)?;
    expression_facts::receiving::validate_blocks_and_edges(
"####,
        );
        undo(
            &mut current,
            r####"    )?;
    expression_facts::receiving::validate_exits(
        program,
        function,
        &storage,
        &leaves,
        &BTreeSet::new(),
        budget,
    )?;
    structural_maps::validate_reference_coverage(function, budget)?;
"####,
            r####"    )?;
    validate_exits(program, function, &storage, &leaves, &BTreeSet::new())?;
    structural_maps::validate_reference_coverage(function, budget)?;
"####,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay.rs.before"
            ),
            "src/cleanup_plan/replay.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation.rs").to_owned();
        undo(
            &mut current,
            r#"mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope, ReplayFailure,
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, raw_error_metered,
    replay_error_funded,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
"#,
            r#"mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, replay_error_funded,
    InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope, ReplayFailure,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation.rs.before"
            ),
            "src/cleanup_plan/replay/allocation.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation/oracle.rs").to_owned();
        undo(
            &mut current,
            r#"mod tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    ReplayFailure, raw_error_funded, raw_error_metered, replay_error_funded,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
"#,
            r#"mod tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    raw_error_funded, replay_error_funded, ReplayFailure,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation__oracle.rs.before"
            ),
            "src/cleanup_plan/replay/allocation/oracle.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation/oracle/diagnostic.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    Ok(Diagnostic::io("SPX-H006", text))
}

mod metered;
pub(in crate::cleanup_plan::replay) use metered::raw as raw_error_metered;
"#,
            r#"    }
    Ok(Diagnostic::io("SPX-H006", text))
}
"#,
        );
        undo(
            &mut current,
            r#"    lease: OracleLease,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    if !matches!(super::super::opted(), Ok(None)) {
        return Err(physical(
            lease,
            "opted oracle raw diagnostic requires its metered receiver",
        ));
    }
    let mut count = Count(0);
    if write!(count, "{message}").is_err() {
        return Err(physical(lease, "raw diagnostic length overflowed"));
"#,
            r#"    lease: OracleLease,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(count, "{message}").is_err() {
        return Err(physical(lease, "raw diagnostic length overflowed"));
"#,
        );
        undo(
            &mut current,
            r#"    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    match super::super::opted() {
        Ok(None) => {}
        Ok(Some(current)) if current == lease => return metered::replay(lease, function, message),
        Ok(Some(_)) | Err(_) => {
            return Err(physical(
                lease,
                "opted oracle diagnostic lease is unavailable or differs",
            ));
        }
    }
    let mut count = Count(0);
    if write!(
        count,
"#,
            r#"    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(
        count,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation__oracle__diagnostic.rs.before"
            ),
            "src/cleanup_plan/replay/allocation/oracle/diagnostic.rs"
        );
    }
    {
        let mut current = include_str!("../../factored.rs").to_owned(); undo(&mut current, "\n#[cfg(test)]\npub(super) use canonical_shared::app_census::Scope as EpsilonAppCensus;\n", ""); undo(&mut current, "        oracle_hook::cross_check_with_storage(\n            program,\n            function,\n            budget,\n            continuations,\n        )?;", "        oracle_hook::cross_check(program, function);"); undo(&mut current, "\n#[cfg(test)]\nmod continuation_receiver;\n", "");
        undo(&mut current, "#[cfg(test)]\nenum CanonicalFrame {\n    Enter(Weighted),\n    Exit(Weighted, u64, Vec<(Label, Weighted)>),\n}\n\n", "");
        // Join the complete relocated old receiver before the existing inverse.
        let before = include_str!("../../factored/canonical_before.rs");
        let original = before
            .strip_prefix(
                r###"//! Complete pre-sharing receiver; its interior is retained verbatim.
use super::*;

"###,
            )
            .unwrap()
            .strip_suffix(
                r###"// The wrapper exposes the unchanged receiver to sibling controls.
pub(super) struct Receiver<'a>(Canonicalizer<'a>);
impl<'a> Receiver<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self(Canonicalizer::new(automaton, sink))
    }
    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        self.0.id(function, root, budget)
    }
    pub(super) fn states(&self) -> &BTreeMap<Weighted, usize> {
        &self.0.states
    }
    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> {
        &self.0.ids
    }
}
"###,
            )
            .unwrap();
        undo(&mut current, "#[cfg(test)]\nmod canonical_before;\nmod canonical_shared;\n", "");
        undo(&mut current, "#[cfg(test)]\nuse canonical_before::Receiver as Canonicalizer;\n", "");
        undo(
            &mut current,
            "    let mut canonical = canonical_shared::Canonicalizer::new(&automaton, sink);",
            "    let mut canonical = Canonicalizer::new(&automaton, sink);",
        );
        let marker = "/// Test builds compare the factored verdict";
        assert_eq!(current.matches(marker).count(), 1);
        assert_eq!(original.matches("#[rustfmt::skip]\n").count(), 1);
        let original = original.replacen("#[rustfmt::skip]\n", "", 1);
        // The complete preexisting inverse below already restores this same
        // old receiver. Compose that restoration once, then authenticate its
        // entire body against the independent compiled receiver below.
        undo(
            &mut current,
            r#"        assert_eq!(short.allocated, short.freed);
    }
}

#[cfg(test)]
mod oracle_order;
"#,
            r#"        assert_eq!(short.allocated, short.freed);
    }
}
"#,
        );
        undo(
            &mut current,
            r#"        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
"#,
            r#"        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
"#,
        );
        undo(
            &mut current,
            r#"        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        oracle_hook::cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
"#,
            r#"        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
"#,
        );
        undo(
            &mut current,
            r####"
/// Test builds compare the factored verdict with the enumerating one on every
"####,
            r####"
enum CanonicalFrame {
    Enter(Weighted),
    Exit(Weighted, u64, Vec<(Label, Weighted)>),
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: BTreeMap<usize, Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<Weighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> Canonicalizer<'a> {
    fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: BTreeMap::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
        }
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
        budget.reserve_array::<(usize, bool)>(function, 1, "factored closure initial cursor")?;
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                stack.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            let mut closure = BTreeMap::new();
            if !state.moves.is_empty() || node == self.sink {
                budget.reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                closure.insert(node, 1_u64);
            }
            for target in &state.epsilon {
                let Some(inner) = self.closures.get(target) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
                    budget.charge(function, 1, "factored closure route addition")?;
                    if !closure.contains_key(inner_node) {
                        budget
                            .reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                    }
                    let slot = closure.entry(*inner_node).or_insert(0_u64);
                    let Some(sum) = slot.checked_add(*routes) else {
                        return Ok(None);
                    };
                    *slot = sum;
                }
            }
            budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            )?;
            budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<BTreeMap<usize, u64>>(),
                "factored closure shared header",
            )?;
            budget.reserve_btree_node::<usize, Rc<BTreeMap<usize, u64>>>(
                function,
                "factored closure memo node",
            )?;
            self.closures.insert(node, Rc::new(closure));
        }
        Ok(self.closures.get(&root).cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, Weighted)>)>, Diagnostic> {
        let mut closed = BTreeMap::<usize, u64>::new();
        for (node, weight) in state {
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    closed.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if !closed.contains_key(inner) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored weighted closure node",
                    )?;
                }
                closed.insert(*inner, sum);
            }
        }
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        let mut successors = BTreeMap::<Label, BTreeMap<usize, u64>>::new();
        for (node, weight) in &closed {
            let moves = &self.automaton.nodes[*node].moves;
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, BTreeMap<usize, u64>>(
                        function,
                        "factored successor label node",
                    )?;
                }
                let targets = successors.entry(label.clone()).or_default();
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
        budget.reserve_array::<(Label, Weighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            budget.reserve_array::<(usize, u64)>(
                function,
                targets.len(),
                "factored weighted successor vector",
            )?;
            result.push((label, targets.into_iter().collect()));
        }
        Ok(Some((accepted, result)))
    }

    fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root_copy = storage::clone_weighted(function, budget, &root)?;
        let mut frames = vec![CanonicalFrame::Enter(root_copy)];
        let mut open = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_btree_node::<Weighted, ()>(
                        function,
                        "factored open-state identity node",
                    )?;
                    open.insert(storage::clone_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = frames.len();
                    frames.push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &frames[exit_index] else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child =
                            storage::clone_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(&state);
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
                        budget.reserve_btree_node::<Weighted, usize>(
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

/// Test builds compare the factored verdict with the enumerating one on every
"####,
        );
        let joined = format!("{original}{marker}");
        assert_eq!(current.matches(&joined).count(), 1,
            "complete compiled old canonical receiver remains byte-exact");
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored.rs.before"
            ),
            "src/cleanup_plan/replay/factored.rs"
        );
    }
    {
        let mut current =
            include_str!("../../factored/borrowed_canonical/dictionary/keys.rs").to_owned();
        undo(
            &mut current,
            r#"        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        #[cfg(test)]
        if budget.oracle_lease().is_some() {
            return crate::cleanup_plan::replay::factored::oracle_order::label(
                self, other, function, budget,
            );
        }
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
"#,
            r#"        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before"
            ),
            "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"
        );
    }
    {
        let mut current = include_str!("../../factored/oracle_hook.rs").to_owned(); undo(&mut current, "/// Ordinary storage sharing changes backing only; explicitly opted oracle\n/// routes keep their complete independent account and never borrow this lease.\npub(super) fn cross_check_with_storage<'hir>(\n    program: &'hir dyn crate::cleanup::ReplayContext,\n    function: &'hir ResolvedFunction,\n    budget: &mut ReplayBudget,\n    storage: Option<&mut continuation_storage::Storage<'hir>>,\n) -> Result<(), Diagnostic> {\n    match super::super::allocation::opted() {\n        Ok(None) => match storage {\n            Some(storage) if storage.has_receiving_owner() => {\n                storage.authenticate(program, function, budget)?;\n                super::continuation_receiver::cross_check(program, function, storage);\n            }\n            _ => super::cross_check(program, function),\n        },\n        Ok(Some(lease)) => cross_check_oracle(program, function, lease),\n        Err(refusal) => panic!(\"opted oracle lease is not live: {refusal:?}\"),\n    }\n    Ok(())\n}\n\n", "");
        undo(&mut current, r###"
// Classification of an undecided result carries no comparison authority.
fn require_decided(
    function: &ResolvedFunction,
    factored: &Result<Option<bool>, Diagnostic>,
) {
    assert!(
        !matches!(factored, Ok(None)),
        "funded factored skeleton comparison is undecided for `{}`",
        function.id,
    );
}

#[cfg(test)]
mod closure_controls;
"###, "");
        undo(&mut current, "        require_decided(function, &factored);\n", "");
        undo(
            &mut current,
            r#"            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            same_paths(function, &mut enumerating, &hir, &plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
"#,
            r#"            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            Ok(hir == plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
"#,
        );
        undo(
            &mut current,
            r#"/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    _lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    super::oracle_order::sort(paths, function, budget)
}
fn same_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    a: &[SkeletonPath],
    b: &[SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "oracle path vector length")?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (x, y) in a.iter().zip(b) {
        budget.charge(function, 1, "oracle path vector member")?;
        if super::oracle_order::path(x, y, function, budget)? != std::cmp::Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn cross_check_oracle(
"#,
            r#"/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    let bytes = paths.len().checked_mul(std::mem::size_of::<SkeletonPath>());
    if bytes.is_none() {
        lease.fail_physical("skeleton sort scratch size overflowed");
    }
    match bytes.map(|bytes| lease.charge(bytes)) {
        Some(Ok(())) => {
            paths.sort();
            Ok(())
        }
        _ => {
            Err(budget.budget_refusal(function, "oracle account refused skeleton sort scratch", ""))
        }
    }
}

pub(super) fn cross_check_oracle(
"#,
        );
        undo(
            &mut current,
            r#"//! Explicitly financed test comparison hook. Ordinary unopted callers retain
//! the complete original enumerating/factored receiver. Owning tests opt into
//! one independent finite account without consuming their live builder ledger.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
pub(super) fn cross_check(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
"#,
            r#"//! Pending test-only funded comparison receiver. Live `factored::cross_check`
//! retains its complete original body: neither actual owner control opts in.
//! Explicit private tests exercise this receiver while the documented transitive
//! acquisition audit remains open. It is not a fully funded live-hook claim.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
fn cross_check(program: &dyn crate::cleanup::ReplayContext, function: &ResolvedFunction) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored__oracle_hook.rs.before"
            ),
            "src/cleanup_plan/replay/factored/oracle_hook.rs"
        );
    }
    {
        let mut current = include_str!("../../nested_shape/paid.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    Ok(shape)
}

#[cfg(test)]
mod oracle;
"#,
            r#"    }
    Ok(shape)
}
"#,
        );
        undo(
            &mut current,
            r#"    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    #[cfg(test)]
    if physical.oracle_lease().is_some() {
        return oracle::derive_reusing(
            program, function, root, next_flag, bounds, physical, scratch,
        );
    }
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
"#,
            r#"    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__nested_shape__paid.rs.before"
            ),
            "src/cleanup_plan/replay/nested_shape/paid.rs"
        );
    }
    {
        let mut current = include_str!("../../ordinary_inventory/substitute.rs").to_owned();
        undo(
            &mut current,
            r#"                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(
                            function,
                            budget,
                            format_args!(
                                "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                            ),
                        ));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
"#,
            r#"                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(function, budget, format_args!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
"#,
        );
        undo(
            &mut current,
            r#"                "oracle invocation authority refused a raw diagnostic",
            );
        }
        let result = if matches!(allocation::opted(), Ok(Some(current)) if current == lease) {
            allocation::raw_error_metered(lease, function, message)
        } else {
            allocation::raw_error_funded(lease, message)
        };
        return match result {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
"#,
            r#"                "oracle invocation authority refused a raw diagnostic",
            );
        }
        return match allocation::raw_error_funded(lease, message) {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__ordinary_inventory__substitute.rs.before"
            ),
            "src/cleanup_plan/replay/ordinary_inventory/substitute.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts.rs").to_owned();
        undo(
            &mut current,
            r#"mod inventory_lifecycle;

pub(super) mod inventory_summary;

#[cfg(test)]
pub(super) mod oracle_inventory;
"#,
            r#"mod inventory_lifecycle;

pub(super) mod inventory_summary;
"#,
        );
        undo(
            &mut current,
            r#"        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::needs_drop(program, function, ty, budget);
    }
    let mut payer = Payer {
        function,
"#,
            r#"        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    let mut payer = Payer {
        function,
"#,
        );
        undo(
            &mut current,
            r#"        }
        return Ok(index.type_flags(ty));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::flags(index, function, ty, budget);
    }
    index.type_flags_paid(
        ty,
        Payer {
"#,
            r#"        }
        return Ok(index.type_flags(ty));
    }
    index.type_flags_paid(
        ty,
        Payer {
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(&mut current, r###"use crate::hir::{DeclarationIndex, TypeFactsPayer};

pub(super) struct Payer<'a> {
    function: &'a ResolvedFunction,
    budget: &'a mut ReplayBudget,
"###, r###"use crate::hir::{DeclarationIndex, TypeFactsPayer};

struct Payer<'a> {
    function: &'a ResolvedFunction,
    budget: &'a mut ReplayBudget,
"###);
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts/inventory_encoded.rs").to_owned();
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |_, payer: &mut Payer<'_>| {
                inventory_text::current(payer)?;
                Ok(true)
            },
            &mut work,
            false,
        );
    }
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

"#,
        );
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            true,
        );
    }
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
"#,
        );
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            false,
        );
    }
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
"#,
        );
        undo(
            &mut current,
            r#"pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return ClassifierString::acquire_oracle_work(payer, &mut pay, &mut work);
    }
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
"#,
            r#"pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_encoded.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_encoded.rs"
        );
    }
    {
        let mut current =
            include_str!("../../paid_type_facts/inventory_encoded/tests.rs").to_owned();
        undo(
            &mut current,
            r###"}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    let current = include_str!("../../../../bounded_output/classifier_string.rs")
        .strip_suffix("\n#[cfg(test)]\nmod oracle_work;\n")
        .unwrap();
    let marker = "    /// A test-account caller authenticates its own invocation separately. This\n    /// detached marker contains no ACTIVE handle and grants no ordinary payer.\n    #[cfg(test)]\n    pub(crate) fn oracle_marker() -> Self {\n        Self(None)\n    }\n\n";
    assert_eq!(current.matches(marker).count(), 1);
    let current = current.replace(marker, "");
    assert_eq!(
        current.strip_suffix("\nmod inventory_work;\n").unwrap(),
        include_str!("../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"###,
            r#"}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(&mut current, r###"fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"###, r###"fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"###);
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_encoded__tests.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_encoded/tests.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts/inventory_tests.rs").to_owned();
        // Undo only the unique visibility hook before the complete old receiver.
        undo(&mut current, r###"    assert_eq!(current.matches("pub(super) struct Payer<'a> {").count(), 1);
    let current = current.replacen(
        "pub(super) struct Payer<'a> {",
        "struct Payer<'a> {",
        1,
    );
"###, "");
        undo(
            &mut current,
            r###"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../paid_type_facts.rs");
    let current = current
        .strip_suffix("\n#[cfg(test)]\npub(super) mod oracle_inventory;\n")
        .unwrap();
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::flags(index, function, ty, budget);\n    }\n",
        "",
    );
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::needs_drop(program, function, ty, budget);\n    }\n",
        "",
    );
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
"###,
            r#"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
"#,
        );
        // Exact receiving edits; the complete original source remains the final oracle.
        undo(&mut current, r###"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
"###, r###"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
"###);
        // Exact composed receiving hooks; reconstruct every original byte below.
        undo(&mut current, r###"    for program in tests::fixtures() {
        for function in &program.functions {
            let (results, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget.prepare_builder(function).unwrap();
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
"###, r###"    for program in tests::fixtures() {
        for function in &program.functions {
            let mut budget = ReplayBudget::new();
            let (results, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
"###);
        undo(&mut current, r###"    assert!(adapter.contains("text::raw(payer, message)"));
}

// The complete prior setup/body stays compiled against its original pure bool
// receiver. It grants no backing authority and makes no funded-owner claim.
mod original_raw_owner {
    use super::*;
    fn inventory_needs_drop(
        program: &dyn crate::cleanup::ReplayContext,
        ty: &ResolvedType,
        _payer: &mut Payer<'_>,
    ) -> Result<bool, Diagnostic> {
        crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| Diagnostic::io("SPX-H006", message))
    }
    #[test]
    fn raw_inventory_classifier_keeps_complete_original_diagnostic_fields() {
        for program in tests::fixtures() {
            for function in &program.functions {
                let mut budget = ReplayBudget::new();
                let (results, refused, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        reserve_program_skeleton_work(
                            &program,
                            std::iter::once(function),
                            &mut budget,
                        )
                        .unwrap();
                        let mut results = Vec::new();
                        for ty in function
                            .params
                            .iter()
                            .map(|p| &p.ty)
                            .chain(std::iter::once(&function.return_type))
                        {
                            results.push((
                                crate::cleanup::type_needs_resource_cleanup(&program, ty)
                                    .map_err(|m| Diagnostic::io("SPX-H006", m)),
                                inventory_needs_drop(
                                    &program,
                                    ty,
                                    &mut tests::payer(function, &mut budget),
                                ),
                            ));
                        }
                        results
                    });
                assert!(!refused);
                for (old, current) in results {
                    assert_eq!(current, old);
                }
            }
        }
    }
}

#[test]
fn complete_prior_raw_and_zero_work_receivers_and_identity_formatter_remain_source_bound() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/compiled-original-projection.json"
    ))
    .unwrap();
    let body = proof["compiled_raw_body"].as_str().unwrap();
    assert!(include_str!("inventory_tests.rs").contains(body));
    // Invert only the enclosing module's four-space indentation, then the
    // one complete callback reflow (including rustfmt's trailing call comma).
    let mut projected = body
        .lines()
        .map(|line| line.strip_prefix("    ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    projected.push('\n');
    let reflow = proof["reflow_before"].as_str().unwrap();
    assert_eq!(projected.matches(reflow).count(), 1);
    projected = projected.replacen(reflow, proof["reflow_after"].as_str().unwrap(), 1);
    projected.push('\n');
    assert_eq!(
        projected,
        include_str!(
            "../../../../docs/evidence/paid-classifier-receiving-f8/raw-inventory-owner.before.txt"
        )
    );
    let current = include_str!("../allocation/oracle/tests.rs");
    let old = include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/oracle-fast-owner.before.txt"
    );
    // The complete old zero-backing owner uses the declared prior receiver;
    // the independent funded current owner remains on its real production path.
    let dispatch = "                let received = nested_shape::type_needs_drop(&program, function, ty).unwrap();";
    let original_dispatch = "                let received = crate::cleanup_plan::replay::nested_shape::type_needs_drop(\n                    &program, function, ty,\n                )\n                .unwrap();";
    assert_eq!(current.matches(dispatch).count(), 1);
    let current = current.replacen(dispatch, original_dispatch, 1);
    let alias = "    use original_fast_type_probe as nested_shape;\n";
    assert_eq!(current.matches(alias).count(), 1);
    assert!(current.replacen(alias, "", 1).contains(old));
    let current =
        include_str!("../../../hir/declaration_index/flags/paid/inventory_constructor.rs");
    let hook = "    // A logical sink can select the existing sticky refusal without returning\n    // a physical error. Authenticate before publishing only a successful value;\n    // Storage::work leaves an already selected first error untouched.\n    let _ = storage.work(0, \"inventory identity receiving authority\");\n";
    assert_eq!(current.matches(hook).count(), 1);
    assert_eq!(
        current.replacen(hook, "", 1),
        include_str!(
        "../../../../docs/evidence/paid-classifier-receiving-f8/inventory-constructor.before.txt"
    )
    );
}
"###, r###"    assert!(adapter.contains("text::raw(payer, message)"));
}
"###);
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_tests.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_tests.rs"
        );
    }
    {
        let mut current = include_str!("../../skeleton_work/oracle_entry.rs").to_owned();
        undo(
            &mut current,
            r#"                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves =
            match crate::literal_format::checked_moving_string_operands_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(moves) => moves,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded_with_work(
            function, &mut pay, &mut units,
        ) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded_with_work(
                program, function, &mut pay, &mut units,
            ) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
"#,
            r#"                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves = match crate::literal_format::checked_moving_string_operands_funded(
            function, &mut pay,
        ) {
            Ok(moves) => moves,
            Err(error) => return Err(refused(budget, function, error)),
        };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded(function, &mut pay) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded(function, &mut pay) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded(program, function, &mut pay) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__skeleton_work__oracle_entry.rs.before"
            ),
            "src/cleanup_plan/replay/skeleton_work/oracle_entry.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/function_value.rs").to_owned();
        undo(
            &mut current,
            r#"    };
    Ok((&INVOKE_ID, args, invocation_params(expression)?))
}

#[cfg(test)]
pub(crate) use funded::work::{walk_funded_paying_with_work, walk_funded_with_work};
"#,
            r#"    };
    Ok((&INVOKE_ID, args, invocation_params(expression)?))
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__function_value.rs.before"
            ),
            "src/hir/function_value.rs"
        );
    }
}
