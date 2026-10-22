//! Real nested HIR calls, independently constructed PLAN, and full owner Drop.
use super::super::tests::fixture;
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::cleanup_plan::replay::ordinary_plan_history::producer::view;
use crate::test_allocation_observer::measure;

fn budget(program: &ResolvedProgram, function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    budget
}
fn joint(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    // The ordinary expression walker itself calls the new whole-fork receiver;
    // no final history is synthesized from the independently produced PLAN.
    let mut expected = with_emission(|| hir_skeleton_paths(program, function, budget))?;
    budget.reserve_bytes(
        function,
        sequence::ENTRY_CALL,
        "whole HIR complete content sort",
    )?;
    sequence::sort(&mut expected, function, budget)?;
    budget.reserve_bytes(
        function,
        view::PRODUCE_CALL,
        "whole independent PLAN producer",
    )?;
    let produced = view::produce(function, budget)?;
    budget.reserve_bytes(
        function,
        view::COMPARE_CALL,
        "whole independent history comparison",
    )?;
    view::compare(produced, &expected, budget)
}

#[test]
fn complete_immediate_old_nested_receiver_and_production_gates_survive() {
    let mut source = include_str!("../../../../replay.rs").to_owned();
    // Reverse exactly the new public shared-call receiver before composing
    // the independent DAG and HIR feature inverses (full f1d66/3666 root).
    let current_call = r#####"            let observations = ordinary_plan_history::completed_fork::emission::extend_call!(
                observations, suffix, work
            )?;"#####;
    let old_call = r#####"            let observations = work.extend_owned_observations(
                observations,
                &suffix.observations,
                "call suffix clone",
            )?;"#####;
    assert_eq!(source.matches(current_call).count(), 1);
    source = source.replacen(current_call, old_call, 1);
    assert_eq!(source.len(), 326_654);
    {
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            "853eccb0d72747e0d824711d9973c2ca6404ef20f5a3e3ed370dff8239f57d6c"
        );
    }
    // Finite additive DAG stage from combined 3666e9a27 to the exact HIR-only
    // e50001241 root. The complete original HIR inverse below is unchanged.
    let current = "let share_prefix = work.prefix_tail_sharing(&suffixes)?;";
    let old = "let share_prefix = work.shares_prefix_tail(&suffixes)?;";
    assert_eq!(source.matches(current).count(), 2);
    source = source.replacen(current, old, 2);
    let current = "sequence_vector::reuse!(prefixes, suffixes, work, token share_prefix, minimum, precharged);";
    let old =
        "sequence_vector::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged);";
    assert_eq!(source.matches(current).count(), 1);
    source = source.replacen(current, old, 1);
    let current = r#####"        if !precharged {
            skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);
        }"#####;
    let old = r#####"        if !precharged && share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }"#####;
    assert_eq!(source.matches(current).count(), 1);
    source = source.replacen(current, old, 1);
    let current = r#####"        skeleton_work::dag_storage::share_prefix!(work, share_prefix, prefix.observations);

"#####;
    let old = r#####"        if share_prefix {
            work.share_prefix_tail(&mut prefix.observations)?;
        }
"#####;
    assert_eq!(source.matches(current).count(), 1);
    source = source.replacen(current, old, 1);
    assert_eq!(source.len(), 326_635);
    {
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            "a2970cce9c29cbf6845f4b7d75728f91dfbf89c68556ccfa87848de676c354fa"
        );
    }
    let start = source.find("fn sequence_skeleton_paths(").unwrap();
    let end = source[start + 1..].find("\nfn ").unwrap() + start + 1;
    let hook = concat!(
        "    let prefixes = ordinary_plan_history::completed_fork::emission::dispatch!(\n",
        "        prefixes, suffixes, work\n",
        "    );\n",
    );
    let current = &source[start..end];
    assert_eq!(current.matches(hook).count(), 1);
    assert_eq!(
        current.replacen(hook, "", 1),
        include_str!("../sequence.before.txt")
    );
    let restored = source.replacen(hook, "", 1).replacen(
        "    ordinary_plan_history::completed_fork::shipping::finish!(function, budget, expected);\n", "", 1);
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "5b40d53c43ccf6335bd2fbaf33c3c57d75b5bcfab2a7d6ebbd03851d77ee6e9d"
    );
    assert!(!selected());
    with_emission(|| {
        assert!(selected() && !before_selected());
        with_before(|| assert!(selected() && before_selected()));
        assert!(selected() && !before_selected());
    });
    assert!(!selected());
}

#[test]
fn real_tiny_branch_and_full_nested_callers_keep_old_contents_work_and_independent_plan() {
    let program = fixture();
    for function in &program.functions {
        // Both complete HIR construction paths are compiled independently: the
        // original whole sequence body and the new immutable producer.
        let mut old_budget = budget(&program, function);
        let mut old =
            with_before(|| hir_skeleton_paths(&program, function, &mut old_budget)).unwrap();
        let mut new_budget = budget(&program, function);
        let mut actual =
            with_emission(|| hir_skeleton_paths(&program, function, &mut new_budget)).unwrap();
        assert_eq!(actual.len(), old.len());
        assert_eq!(new_budget.skeleton_spent, old_budget.skeleton_spent);
        assert_eq!(new_budget.skeleton_remaining, old_budget.skeleton_remaining);
        new_budget
            .reserve_bytes(
                function,
                3 * sequence::ENTRY_CALL,
                "whole paired HIR sort and equality",
            )
            .unwrap();
        sequence::sort(&mut actual, function, &mut new_budget).unwrap();
        sequence::sort(&mut old, function, &mut new_budget).unwrap();
        assert!(sequence::equal(&actual, &old, function, &mut new_budget).unwrap());
        if function.id.as_str() == "f" {
            assert!(actual.iter().any(|path| path.observations.prefix.is_some()));
        }
        // The real public payer and funded ordinary payer both enter the same
        // independent complete PLAN comparator, with no shared identity seal.
        let mut public = budget(&program, function);
        joint(&program, function, &mut public).unwrap();
        with_limit_usage_refusal(usize::MAX, || {
            let mut funded = budget(&program, function);
            joint(&program, function, &mut funded).unwrap();
        });
    }
}

#[test]
fn whole_nested_hir_plan_exact_short_sticky_floor_and_full_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let run = |limit, floor| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                assert!(set_active_floor(floor));
                let mut budget = ReplayBudget::new();
                let result =
                    reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                        .and_then(|()| joint(&program, function, &mut budget));
                if result.is_err() {
                    assert!(
                        budget
                            .reserve_bytes(function, 0, "whole history sticky zero")
                            .is_err()
                    );
                }
                drop(budget);
                result.map_err(|error| {
                    let code = error.code;
                    drop(error);
                    code
                })
            })
        })
    };
    let ((result, heap), refused, paid, _) = run(usize::MAX, 0);
    result.unwrap();
    assert!(!refused && !heap.invalid && heap.live == 0);
    assert_eq!(heap.allocated, heap.freed);
    assert!(heap.allocated <= paid);
    for (limit, floor, success) in [
        (paid, 0, true),
        (paid - 1, 0, false),
        (0, 0, false),
        (paid, paid, false),
    ] {
        let ((result, heap), refused, _, first) = run(limit, floor);
        assert_eq!(result.is_ok(), success);
        assert!(!heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
        if !success {
            assert!(refused && first.is_some());
            assert_eq!(result.unwrap_err(), "SPX-H006");
        }
    }
}

#[test]
fn whole_nested_work_failures_preserve_all_six_diagnostic_fields() {
    // Original-event cuts come from the independently compiled old caller's
    // actual skeleton debit total, not a guessed work horizon. Reducing the
    // already reserved loan donates/refunds nothing; added metadata still pays
    // from the untouched ordinary free-work account.
    for program in [fixture(), nested_call_program()] {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "f" || function.id.as_str() == "nested")
            .unwrap();
        with_limit_usage_refusal(usize::MAX, || {
            let mut baseline = budget(&program, function);
            with_before(|| hir_skeleton_paths(&program, function, &mut baseline)).unwrap();
            let events = baseline.skeleton_spent;
            assert!(events > 0);
            for cut in 0..=events {
                let attempt = |prior| {
                    let mut budget = budget(&program, function);
                    budget.skeleton_remaining = cut;
                    let result = if prior {
                        with_before(|| hir_skeleton_paths(&program, function, &mut budget))
                    } else {
                        hir_skeleton_paths(&program, function, &mut budget)
                    }
                    .map(drop);
                    (result, budget.skeleton_spent)
                };
                let (before, old_spent) = attempt(true);
                let (current, new_spent) = attempt(false);
                assert_eq!(new_spent, old_spent);
                assert_eq!(before.is_ok(), cut == events);
                assert_eq!(current.is_ok(), cut == events);
                match (before, current) {
                    (Err(a), Err(b)) => assert_eq!(
                        (a.code, a.severity, a.message, a.path, a.span, a.help),
                        (b.code, b.severity, b.message, b.path, b.span, b.help)
                    ),
                    (Ok(()), Ok(())) => {}
                    _ => panic!("original event-aligned refusal changed"),
                }
            }
        });
    }
    let program = fixture();
    let function = &program.functions[0];
    let attempt = |cut, prior| {
        with_limit_usage_refusal(usize::MAX, || {
            let mut budget = budget(&program, function);
            budget.remaining = cut;
            if prior {
                with_before(|| hir_skeleton_paths(&program, function, &mut budget)).map(drop)
            } else {
                with_emission(|| hir_skeleton_paths(&program, function, &mut budget)).map(drop)
            }
        })
        .0
    };
    // Added traversals have explicit free-work costs, so compare each complete
    // producer with itself at identical cuts; a shared byte/work total grants
    // no permission to substitute a different refusal or waive its fields.
    for prior in [false, true] {
        for cut in 0..128 {
            match (attempt(cut, prior), attempt(cut, prior)) {
                (Ok(()), Ok(())) => {}
                (Err(a), Err(b)) => assert_eq!(
                    (a.code, a.severity, a.message, a.path, a.span, a.help),
                    (b.code, b.severity, b.message, b.path, b.span, b.help)
                ),
                _ => panic!("same whole caller and work cut changed result"),
            }
        }
    }
}

fn nested_call_program() -> ResolvedProgram {
    let source = r#"module history.call;
@id("identity") fn identity(value:i64)->i64 {value}
@id("nested") fn nested(a:bool,b:bool,c:bool)->i64 {
 identity({let n=if a {1}else{2};let m=if b {n+1}else{n+2};if c {m+1}else{m+2}})
}
@id("main") fn main()->i64 {nested(true,false,true)}
"#;
    let checked = crate::check(source, "history-call.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "history-call.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}

#[test]
fn shared_nested_call_argument_keeps_public_loan_events_and_late_content_refusals() {
    let program = nested_call_program();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "nested")
        .unwrap();
    let mut old_budget = budget(&program, function);
    let mut old = with_before(|| hir_skeleton_paths(&program, function, &mut old_budget)).unwrap();
    let mut current_budget = budget(&program, function);
    let mut current = hir_skeleton_paths(&program, function, &mut current_budget).unwrap();
    assert_eq!(current_budget.skeleton_spent, old_budget.skeleton_spent);
    assert_eq!(
        current_budget.skeleton_remaining,
        old_budget.skeleton_remaining
    );
    assert!(current
        .iter()
        .any(|path| path.observations.prefix.is_some()));
    current_budget
        .reserve_bytes(
            function,
            4 * sequence::ENTRY_CALL,
            "shared call old/current content receivers",
        )
        .unwrap();
    sequence::sort(&mut old, function, &mut current_budget).unwrap();
    sequence::sort(&mut current, function, &mut current_budget).unwrap();
    assert!(sequence::equal(&old, &current, function, &mut current_budget).unwrap());
    // A late omission inside the call's actual shared history must still be
    // refused by independent PLAN content comparison, not accepted by identity.
    assert!(!current.last().unwrap().observations.items.is_empty());
    current.last_mut().unwrap().observations.items.clear();
    sequence::sort(&mut current, function, &mut current_budget).unwrap();
    current_budget
        .reserve_bytes(
            function,
            view::PRODUCE_CALL + view::COMPARE_CALL,
            "shared call malformed content comparison receivers",
        )
        .unwrap();
    let produced = view::produce(function, &mut current_budget).unwrap();
    assert_eq!(
        view::compare(produced, &current, &mut current_budget)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    with_before(|| validate_program(&program)).unwrap();
    validate_program(&program).unwrap();
    with_limit_usage_refusal(usize::MAX, || {
        with_before(|| validate_program(&program)).unwrap();
        validate_program(&program).unwrap();
    });
}

#[test]
fn actual_late_terminal_and_multiplicity_refusals_match_the_whole_old_comparator() {
    let program = fixture();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        for mutation in 0..3 {
            let mut old_budget = budget(&program, function);
            let mut old =
                with_before(|| hir_skeleton_paths(&program, function, &mut old_budget)).unwrap();
            let mut new_budget = budget(&program, function);
            let mut current =
                with_emission(|| hir_skeleton_paths(&program, function, &mut new_budget)).unwrap();
            for (paths, budget) in [(&mut old, &mut old_budget), (&mut current, &mut new_budget)] {
                budget
                    .reserve_bytes(
                        function,
                        2 * sequence::ENTRY_CALL,
                        "whole malformed HIR content sorts",
                    )
                    .unwrap();
                sequence::sort(paths, function, budget).unwrap();
                match mutation {
                    0 => {
                        paths.pop();
                    }
                    1 => {
                        paths.last_mut().unwrap().terminal = SkeletonTerminal::Failure;
                    }
                    _ => {
                        let mut work = SkeletonWork::new(&program, function, budget);
                        let mut tail = skeleton_work::empty_expr_path();
                        tail.observations =
                            std::mem::take(&mut paths.last_mut().unwrap().observations);
                        work.push_observation(
                            &mut tail,
                            SkeletonObservation::Boolean {
                                expression: function.body.id.clone(),
                                value: false,
                            },
                            "whole malformed HIR late observation",
                        )
                        .unwrap();
                        paths.last_mut().unwrap().observations = tail.observations;
                    }
                }
                sequence::sort(paths, function, budget).unwrap();
            }
            old_budget
                .reserve_bytes(
                    function,
                    PRODUCE_CALL_CONTROLS + COMPARE_CALL_CONTROLS,
                    "whole old PLAN and comparison receivers",
                )
                .unwrap();
            let old_plan = super::super::super::produce(function, &mut old_budget).unwrap();
            let before = super::super::super::before_comparison::compare(
                function,
                &mut old_budget,
                old_plan,
                &old,
            )
            .unwrap_err();
            new_budget
                .reserve_bytes(
                    function,
                    view::PRODUCE_CALL + view::COMPARE_CALL,
                    "whole current PLAN and comparison receivers",
                )
                .unwrap();
            let actual = view::produce(function, &mut new_budget).unwrap();
            let current = view::compare(actual, &current, &mut new_budget).unwrap_err();
            assert_eq!(
                (
                    before.code,
                    before.severity,
                    before.message,
                    before.path,
                    before.span,
                    before.help
                ),
                (
                    current.code,
                    current.severity,
                    current.message,
                    current.path,
                    current.span,
                    current.help
                )
            );
        }
    });
}

#[test]
fn actual_production_public_and_funded_whole_program_keep_independent_old_route() {
    let program = fixture();
    // No candidate flag selects the new production path. The explicit old
    // control selects only the separately compiled whole predecessor sequence.
    assert!(!selected());
    with_before(|| validate_program(&program)).unwrap();
    validate_program(&program).unwrap();
    with_limit_usage_refusal(usize::MAX, || {
        with_before(|| validate_program(&program)).unwrap();
        validate_program(&program).unwrap();
    });
    assert!(!selected());
}
