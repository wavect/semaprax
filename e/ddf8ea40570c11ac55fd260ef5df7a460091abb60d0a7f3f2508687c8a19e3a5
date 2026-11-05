#[test]
fn selected_generation_retains_complete_immediate_methods_and_original_arena_gate() {
    let original=include_str!("../../../../../docs/evidence/selected-successor-generation-20261017/before/src/cleanup_plan/replay/factored/canonical_shared.rs");
    let current = include_str!("../canonical_shared.rs");
    let acquired = crate::source_reflows_merge::main67107_inputs::canonical_import_input(
        "src/cleanup_plan/replay/factored/canonical_shared.rs",
        current,
    );
    let current = acquired.as_str();
    let restored = current
        .replace(
            "                AllocationPayer::SelectedReservation => {}\n",
            "",
        )
        .replace(
            "            AllocationPayer::SelectedReservation => {}\n",
            "",
        );
    assert_eq!(restored, original);
    let tests = include_str!("arena_tests.rs");
    let acquired_tests = crate::source_reflows_merge::main67107_inputs::semantic_batch_previous(
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs",
        tests,
        "base851",
    );
    let tests = acquired_tests.as_str();
    let oldtests=include_str!("../../../../../docs/evidence/selected-successor-generation-20261017/before/src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs");
    assert!(tests.starts_with(oldtests));
    for (start, end) in [
        ("    fn successors(\n", "    #[cfg(test)]\n    fn closure("),
        (
            "    fn successors_with_receiving(\n",
            "    pub(super) fn id_with_receiving(",
        ),
    ] {
        let offset = original.find(start).unwrap();
        let body = &original[offset..offset + original[offset..].find(end).unwrap()];
        let renamed = body
            .replace("fn successors(", "fn previous_selected_successors(")
            .replace(
                "fn successors_with_receiving(",
                "fn previous_selected_successors_with_receiving(",
            )
            .replace(
                "self.successors_with_receiving(",
                "self.previous_selected_successors_with_receiving(",
            );
        assert_eq!(tests.matches(&renamed).count(), 1);
    }
}

// Whole pre-preflight adapters retained independently; negative control below.
impl Canonicalizer<'_> {
    #[cfg(test)]
    fn current_successors_before_owner_preflight(
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
    fn previous_successors_before_owner_preflight(
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
        self.previous_selected_successors_with_receiving(function, state, budget, &mut receiving)
    }
}

#[test]
fn complete_before_successor_adapters_record_payment_before_foreign_scope_as_negative() {
    use crate::bounded_output::with_limit_usage_refusal;
    let function = fixture();
    let (graph, sink, roots) = super::tests::automaton(&function);
    let state = selected_state(&[(roots[0], 1)]);
    for old in [true, false] {
        with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_physical(&function, 1 << 26).unwrap();
            budget
                .reserve_bytes(
                    &function,
                    Canonicalizer::CONSTRUCTOR_CONTROLS,
                    "before adapter negative owner",
                )
                .unwrap();
            let mut owner = Canonicalizer::new(&graph, sink);
            let initial = if old {
                owner.previous_successors_before_owner_preflight(&function, &state, &mut budget)
            } else {
                owner.current_successors_before_owner_preflight(&function, &state, &mut budget)
            };
            assert!(initial.unwrap().is_some());
            // The complete previous adapter pays before it reaches the bound
            // Memo's scope check. A zero child proves that original bad order;
            // this source remains a negative, not a new production acceptance.
            let (result, refused, _, _) = with_limit_usage_refusal(0, || {
                if old {
                    owner.previous_successors_before_owner_preflight(&function, &state, &mut budget)
                } else {
                    owner.current_successors_before_owner_preflight(&function, &state, &mut budget)
                }
            });
            let error = result.unwrap_err();
            assert!(refused);
            assert_eq!(error, super::super::replay_error(&function,
                "cleanup replay physical reservation exhausted during factored held closure invocation controls"));
        });
    }
}

// Complete immediate d3 entry bodies; only private method names/visibility differ.
mod warm_before {
    use super::super::*;
    impl Canonicalizer<'_> {
        pub(super) fn previous_warm_id(
            &mut self,
            function: &ResolvedFunction,
            root: Weighted,
            budget: &mut ReplayBudget,
        ) -> Result<Option<usize>, Diagnostic> {
            // Preserve the old entry's owner rejection before a fresh test-only slot
            // can query or charge a foreign payer. A valid call still pays acquisition.
            if self.closures.preflight(function, budget) {
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
        pub(super) fn previous_warm_id_for_comparison(
            &mut self,
            function: &ResolvedFunction,
            root: Weighted,
            budget: &mut ReplayBudget,
        ) -> Result<Option<usize>, Diagnostic> {
            self.closures.preflight(function, budget);
            if self.closures.failed() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    Self::COLD_ROOT_CONTROLS,
                    "factored cold terminal root predicate",
                )?;
                budget.reserve_bytes(
                    function,
                    Self::COLD_RESULT_CONTROLS,
                    "factored cold terminal result receiving",
                )?;
            }
            if root.len() == 1 && root[0].1 == 1 && self.cold_dead_node(root[0].0) {
                return self.cold_dead_id(function, root, budget);
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
    }
}

#[test]
fn warmed_canonical_entries_skip_only_impossible_cold_receivers_and_keep_old_authentication() {
    let function = fixture();
    let run = |previous, production, limit| {
        let (graph, sink, roots) = corpus(&function, 0);
        measure(|| {
            let result = (|| {
                let mut budget = ReplayBudget::with_physical(&function, limit)?;
                budget.reserve_bytes(
                    &function,
                    Canonicalizer::CONSTRUCTOR_CONTROLS,
                    "canonical ordered dictionary owner receiving",
                )?;
                let mut owner = Canonicalizer::new(&graph, sink);
                let mut checksum = 0_u64;
                for root in roots {
                    budget.reserve_array::<(usize, u64)>(&function, 1, "arena corpus root")?;
                    let root = vec![(root, 1)];
                    let id = match (previous, production) {
                        (true, true) => {
                            owner.previous_warm_id_for_comparison(&function, root, &mut budget)
                        }
                        (true, false) => owner.previous_warm_id(&function, root, &mut budget),
                        (false, true) => owner.id_for_comparison(&function, root, &mut budget),
                        (false, false) => owner.id(&function, root, &mut budget),
                    }?
                    .unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
                    assert!(owner.scratch_paid);
                }
                let result = (
                    budget.physical.as_ref().unwrap().charged(),
                    budget.remaining,
                    checksum,
                );
                drop((owner, budget));
                Ok::<_, Diagnostic>(result)
            })();
            result.map_err(|error| {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
            })
        })
    };
    for production in [false, true] {
        let (old, old_heap) = run(true, production, 1 << 26);
        let (current, heap) = run(false, production, 1 << 26);
        let old = old.unwrap();
        let current = current.unwrap();
        assert_eq!(
            (old.1, old.2),
            (current.1, current.2),
            "all original work and IDs"
        );
        let skipped = Canonicalizer::COLD_ROOT_CONTROLS
            + if production {
                Canonicalizer::COLD_RESULT_CONTROLS
            } else {
                0
            };
        assert_eq!(
            old.0 - current.0,
            31 * skipped,
            "only the 31 impossible warm predicates"
        );
        for observed in [old_heap, heap] {
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
        }
        for limit in [0, current.0 - 1, current.0] {
            let (result, observed) = run(false, production, limit);
            assert_eq!(result.is_ok(), limit == current.0);
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
        }
        let mut errors = Vec::new();
        for previous in [true, false] {
            let (graph, sink, roots) = corpus(&function, 0);
            let mut budget = ReplayBudget::with_physical(&function, 1 << 26).unwrap();
            budget
                .reserve_bytes(
                    &function,
                    Canonicalizer::CONSTRUCTOR_CONTROLS,
                    "canonical ordered dictionary owner receiving",
                )
                .unwrap();
            let mut owner = Canonicalizer::new(&graph, sink);
            let root = vec![(roots[0], 1)];
            match (previous, production) {
                (true, true) => owner.previous_warm_id_for_comparison(&function, root, &mut budget),
                (true, false) => owner.previous_warm_id(&function, root, &mut budget),
                (false, true) => owner.id_for_comparison(&function, root, &mut budget),
                (false, false) => owner.id(&function, root, &mut budget),
            }
            .unwrap();
            let foreign = function.clone();
            let charged = budget.physical.as_ref().unwrap().charged();
            let error = match (previous, production) {
                (true, true) => {
                    owner.previous_warm_id_for_comparison(&foreign, Vec::new(), &mut budget)
                }
                (true, false) => owner.previous_warm_id(&foreign, Vec::new(), &mut budget),
                (false, true) => owner.id_for_comparison(&foreign, Vec::new(), &mut budget),
                (false, false) => owner.id(&foreign, Vec::new(), &mut budget),
            }
            .unwrap_err();
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
            let sticky = owner
                .id_for_comparison(&function, Vec::new(), &mut budget)
                .unwrap_err();
            assert_eq!(error, sticky, "all six sticky diagnostic fields");
            errors.push(error);
        }
        assert_eq!(errors[0], errors[1], "complete original foreign diagnostic");
    }
}
