//! Actual policy parity and full independent Tree driver accounting.
use super::*;
use crate::cleanup_plan::replay::expression_facts::Lookup;

impl Session<'_> {
    /// Execute the complete original Tree receiver on this actual Session.
    /// The compatibility receiver and its child remain source-exact; this
    /// bridge only supplies the same authenticated context and sticky entry.
    fn validate_scalar_tree_before(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        self.failed = true;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        scalar_replay_tree::validate(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum Route {
    Tree,
    LegacyArena,
    SelectedArena,
}
const ROUTES: [Route; 3] = [Route::Tree, Route::LegacyArena, Route::SelectedArena];
const SOURCE: &str = r#"module scalar.policy;
@id("scalar.leaf.λ") fn leaf(value:i64)->i64 requires value>=0 ensures result>=0 {if value==0 {1}else{value}}
@id("scalar.calls") fn calls(value:i64)->i64 requires value>=0 ensures result>=0 {leaf(value)+leaf(0)}
@id("scalar.bool") fn boolean(flag:bool)->bool {flag && (if flag {true}else{false})}
@id("scalar.size") fn count(value:usize)->usize {if value>0usize {value-1usize}else{0usize}}
@id("scalar.byte") fn byte(value:u8)->i64 {i64_from_u8(value)}
@id("scalar.text") fn text(value:borrow str)->i64 {str_len_bytes(value)}
@id("main") fn main()->i64 {leaf(3)}
"#;
fn corpus() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "scalar-policy-λ.spx").unwrap()).unwrap()
}
fn run(
    route: Route,
    selected: &mut Session<'_>,
    function: &ResolvedFunction,
) -> Result<(), expression_facts::errors::EngineError> {
    match route {
        Route::Tree => selected
            .validate_scalar_tree_before(function)
            .map_err(Into::into),
        Route::LegacyArena => selected.validate_scalar(function).map_err(Into::into),
        Route::SelectedArena => selected.validate_scalar_paid(function),
    }
}
#[derive(Clone, Copy, Debug)]
struct Usage {
    remaining: usize,
    reserved: usize,
    work: usize,
}
fn clean(metrics: crate::test_allocation_observer::Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
fn success(
    route: Route,
    context: &dyn ReplayContext,
    function: &ResolvedFunction,
    physical: usize,
    work: Option<usize>,
) -> Usage {
    let ((usage, metrics), refused, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            measure(|| {
                let mut selected = session(context, function, physical);
                if let Some(work) = work {
                    assert!(work <= selected.usage().0);
                    selected.budget.remaining = work;
                }
                let initial = selected.usage().0;
                let held = selected.budget.physical.as_deref().unwrap() as *const _;
                run(route, &mut selected, function).unwrap();
                assert!(!selected.failed);
                assert_eq!(
                    selected.budget.physical.as_deref().unwrap() as *const _,
                    held
                );
                let (remaining, reserved) = selected.usage();
                let usage = Usage {
                    remaining,
                    reserved,
                    work: initial - remaining,
                };
                drop(
                    selected
                        .finish_paid()
                        .ok()
                        .expect("full actual successful Session"),
                );
                usage
            })
        });
    assert!(!refused);
    clean(metrics);
    assert!(
        metrics.allocated <= usage.reserved,
        "{route:?}: {metrics:?}, {usage:?}"
    );
    assert!(metrics.highwater <= usage.reserved);
    usage
}
#[test]
fn complete_tree_and_both_arena_policies_keep_full_scalar_corpus_ids_and_verdicts() {
    let program = corpus();
    let context = primitive_fixture::Context::new(&program);
    let before = crate::cache_codec::encode(&program).unwrap();
    let mut different = false;
    for function in &program.functions {
        assert!(function.cleanup_plan.lexical_control.is_none());
        let (_, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut old = session(&context, function, usize::MAX);
            let tree = expression_facts::expression_facts_with_reservation(
                function,
                old.budget.physical.as_deref_mut().unwrap(),
            )
            .unwrap();
            let mut current = session(&context, function, usize::MAX);
            let arena = expression_facts::with_budget(function, &mut current.budget).unwrap();
            assert!(matches!(&arena, expression_facts::Facts::Arena(_)));
            let entries = Lookup::entries_paid(&arena, function, &mut current.budget).unwrap();
            assert_eq!(tree.len(), arena.len());
            for ((old_id, old_fact), (id, fact)) in tree.iter().zip(entries) {
                assert_eq!(old_id, id);
                assert_eq!(old_id.as_str().as_ptr(), id.as_str().as_ptr());
                match (old_fact, fact) {
                    (Some(old), Some(new)) => {
                        assert_eq!(old.callee, new.callee);
                        assert_eq!(old.instance, new.instance);
                        assert_eq!(old.arguments, new.arguments);
                        assert_eq!(old.type_arguments, new.type_arguments);
                    }
                    (None, None) => {}
                    _ => panic!("complete original operation fact differs"),
                }
            }
        });
        assert!(!refused);
        let tree = success(Route::Tree, &context, function, usize::MAX, None);
        let legacy = success(Route::LegacyArena, &context, function, usize::MAX, None);
        let paid = success(Route::SelectedArena, &context, function, usize::MAX, None);
        assert_eq!(
            (legacy.remaining, legacy.reserved),
            (paid.remaining, paid.reserved)
        );
        assert_eq!(legacy.work, paid.work);
        different |= (tree.remaining, tree.reserved) != (legacy.remaining, legacy.reserved);
    }
    assert!(
        different,
        "complete Tree is a real independent representation, not a new-core adapter"
    );
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), before);
}
#[test]
fn every_real_scalar_representation_keeps_its_own_exact_short_work_backing_and_sticky_drop() {
    let program = tiny();
    let context = primitive_fixture::Context::new(&program);
    let function = &program.functions[0];
    for route in ROUTES {
        let full = success(route, &context, function, usize::MAX, None);
        assert!(full.work > 0);
        let exact = success(route, &context, function, full.reserved, Some(full.work));
        assert_eq!(exact.remaining, 0);
        assert_eq!(exact.reserved, full.reserved);
        for (physical, work) in [
            (full.reserved - 1, None),
            (full.reserved, Some(full.work - 1)),
            (full.reserved, Some(0)),
        ] {
            let (((), metrics), _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                measure(|| {
                    let mut selected = session(&context, function, physical);
                    if let Some(work) = work {
                        selected.budget.remaining = work;
                    }
                    let ledger = selected.budget.physical.as_deref().unwrap() as *const _;
                    let error = run(route, &mut selected, function).unwrap_err();
                    assert!(selected.failed);
                    let used = selected.usage();
                    let receipt = selected
                        .budget
                        .physical
                        .as_deref_mut()
                        .unwrap()
                        .reserve(0)
                        .unwrap_err();
                    let again = run(route, &mut selected, function).unwrap_err();
                    assert_eq!(selected.usage(), used);
                    assert_eq!(
                        selected.budget.physical.as_deref().unwrap() as *const _,
                        ledger
                    );
                    assert_eq!(
                        selected
                            .budget
                            .physical
                            .as_deref_mut()
                            .unwrap()
                            .reserve(0)
                            .unwrap_err(),
                        receipt
                    );
                    drop(again);
                    drop(selected);
                    drop(error);
                })
            });
            clean(metrics);
        }
    }
}
#[test]
fn full_tree_semantic_primary_and_simultaneous_hostiles_match_policy_facades_through_drop() {
    for attack in 0..3 {
        // This internal primitive fixture grants no source authority. Keep the
        // malformed function an actual held member so the complete driver,
        // rather than its pointer-membership guard, selects the primary.
        let mut program = corpus();
        let forged = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "scalar.calls")
            .unwrap();
        match attack {
            0 => forged.return_type = ResolvedType::Bytes,
            1 => forged.cleanup_plan.schema = "semaprax.invalid-scalar-plan",
            _ => {
                forged.return_type = ResolvedType::Bytes;
                forged.cleanup_plan.schema = "semaprax.invalid-scalar-plan";
            }
        }
        let context = primitive_fixture::Context::new(&program);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "scalar.calls")
            .unwrap();
        let expected = replay_error_old(
            function,
            if attack == 1 {
                "selected scalar helper requires its exact nonlexical schema"
            } else {
                "selected helper signature is outside Copy scalars"
            },
        );
        let (((tree, legacy, paid), metrics), refused, _) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                measure(|| {
                    let mut outcomes = Vec::new();
                    for route in ROUTES {
                        let mut selected = session(&context, function, usize::MAX);
                        let ledger = selected.budget.physical.as_deref().unwrap() as *const _;
                        let error = run(route, &mut selected, function).unwrap_err();
                        let expression_facts::errors::EngineError::Owned(error) = error else {
                            panic!("pre-fact primary must retain full Diagnostic")
                        };
                        assert!(selected.failed);
                        assert_eq!(error, expected);
                        let first = error.clone();
                        let retry = run(route, &mut selected, function).unwrap_err();
                        assert_eq!(
                            selected.budget.physical.as_deref().unwrap() as *const _,
                            ledger
                        );
                        drop(retry);
                        drop(selected);
                        assert_eq!(
                            error, first,
                            "original primary survives complete Session Drop"
                        );
                        drop(error);
                        outcomes.push(first);
                    }
                    // Consume comparisons and owned errors inside the observer.
                    assert_eq!(outcomes[0], outcomes[1]);
                    assert_eq!(outcomes[1], outcomes[2]);
                    let summaries = outcomes
                        .iter()
                        .map(|e| (e.code, e.message.contains("outside Copy scalars")))
                        .collect::<Vec<_>>();
                    (summaries[0], summaries[1], summaries[2])
                })
            });
        assert!(!refused);
        clean(metrics);
        assert_eq!(tree, legacy);
        assert_eq!(legacy, paid);
        if attack != 1 {
            assert!(
                tree.1,
                "signature must remain first even with a second hostile"
            );
        }
    }
}

#[test]
fn complete_original_tree_receiver_child_and_existing_usage_assertions_stay_byte_exact() {
    let original = include_str!(
        "../../../../../../docs/evidence/scalar-fact-policy-tree/original-scalar-replay.txt"
    );
    let original_records = include_str!(
        "../../../../../../docs/evidence/scalar-fact-policy-tree/original-scalar-records.txt"
    );
    assert_eq!(include_str!("../../../scalar_replay_tree.rs"), original);
    assert_eq!(
        include_str!("../../../scalar_replay_tree/records.rs"),
        original_records
    );
    assert_eq!(
        include_str!("../../../scalar_replay/records.rs"),
        original_records
    );
    assert_eq!(
        include_str!("../../../selected_entry.rs"),
        include_str!(
            "../../../../../../docs/evidence/scalar-fact-policy-tree/original-selected-entry.txt"
        )
    );
    let old = r#"    validate_core(program, function, budget, |function, budget| {
        let physical = budget
            .physical
            .as_deref_mut()
            .ok_or_else(|| replay_error(function, "selected scalar helper lacks backing"))?;
        expression_facts::expression_facts_with_reservation(function, physical)
            .map(expression_facts::Facts::Tree)
    })"#;
    let new = r#"    validate_core(program, function, budget, |function, budget| {
        // This facade compares error policies on the same genuine Arena route.
        // The complete independent Tree driver remains compiled for its own
        // semantic and representation-accounting controls.
        if budget.physical.is_none() {
            return Err(replay_error(
                function,
                "selected scalar helper lacks backing",
            ));
        }
        expression_facts::with_budget(function, budget)
    })"#;
    assert_eq!(original.matches(old).count(), 1);
    assert_eq!(
        include_str!("../../../scalar_replay.rs"),
        original.replacen(old, new, 1)
    );
    let old_tests = include_str!(
        "../../../../../../docs/evidence/scalar-fact-policy-tree/original-fact-errors.txt"
    );
    let current_tests = include_str!("../fact_errors.rs");
    assert_eq!(
        current_tests,
        format!("{old_tests}\n#[path = \"fact_errors/tree_policy.rs\"]\nmod tree_policy;\n")
    );
    assert_eq!(
        old_tests
            .matches("assert_eq!(old.usage(), paid.usage());")
            .count(),
        2
    );
    assert_eq!(
        current_tests
            .matches("assert_eq!(old.usage(), paid.usage());")
            .count(),
        2
    );
    let registration = include_str!("../../../../replay.rs");
    assert!(registration.contains(
        r#"#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete independently compiled original Tree scalar receiver"
)]
mod scalar_replay_tree;"#
    ));
}

#[path = "tree_policy/nominal.rs"]
mod nominal;
