//! Authored owning controls, not an executed application or saving claim.
use super::{current_into, try_compare, Failure, AUTH, CALL, OWNER};
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal};
use crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission;
use crate::cleanup_plan::replay::{
    hir_skeleton_paths, ordinary_plan_history, reserve_program_skeleton_work, validate_program,
    AdmissionMode, AllocationPayer, Diagnostic, ReplayBudget, SkeletonPath, SkeletonTerminal,
};
use crate::cleanup_plan::{CleanupTerminator, ExitContinuation};
use crate::hir::{ResolvedFunction, ResolvedProgram};
use crate::test_allocation_observer::measure;

fn dense(n: usize) -> ResolvedProgram {
    let calls = std::iter::repeat_n("f()", n)
        .collect::<Vec<_>>()
        .join(" + ");
    let source = format!("module ordinary; @id(\"ordinary.f\") fn f()->i64 {{1}} @id(\"ordinary.main\") fn main()->i64 {{{calls}}}");
    let checked = crate::check(&source, "ordinary.spx").unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn budget(program: &ResolvedProgram, function: &ResolvedFunction) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(program, std::iter::once(function), &mut budget).unwrap();
    budget
}
fn flat(program: &ResolvedProgram, function: &ResolvedFunction) -> Vec<SkeletonPath> {
    let mut budget = budget(program, function);
    let paths =
        emission::with_before(|| hir_skeleton_paths(program, function, &mut budget)).unwrap();
    assert!(paths.iter().all(|path| path.observations.prefix.is_none()));
    paths
}
fn call(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    expected: &mut [SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.reserve_bytes(function, CALL, "flat admission test caller")?;
    try_compare(function, budget, expected)
}
fn dispatch(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    mut expected: Vec<SkeletonPath>,
    fell_back: &mut bool,
) -> Result<(), Diagnostic> {
    crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::finish_current!(
        function, budget, expected
    );
    *fell_back = true;
    Ok(())
}
fn same_error(a: Diagnostic, b: Diagnostic) {
    assert_eq!(
        (a.code, a.severity, a.message, a.path, a.span, a.help),
        (b.code, b.severity, b.message, b.path, b.span, b.help)
    );
}

#[test]
fn dense_checked_add_branches_use_independent_plan_and_preserve_every_old_path() {
    for n in [2, 8, 64] {
        let program = dense(n);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "ordinary.main")
            .unwrap();
        // Owning source construction maps each of the n-1 checked additions
        // through split_status. Count actual resolved branches as well.
        assert!(
            function
                .cleanup_plan
                .blocks
                .iter()
                .filter(
                    |b| matches!(&b.terminator, CleanupTerminator::Branch(edges) if edges.len()>1)
                )
                .count()
                >= n - 1
        );
        let mut expected = flat(&program, function);
        let mut old_budget = budget(&program, function);
        let mut old = ordinary_plan_history::before::produce(function, &mut old_budget).unwrap();
        old.sort();
        expected.sort();
        assert_eq!(expected, old);
        let mut actual_budget = budget(&program, function);
        assert!(call(function, &mut actual_budget, &mut expected).unwrap());
        assert_eq!(actual_budget.skeleton_spent, old_budget.skeleton_spent);
        assert_eq!(expected, old);
        let tiny = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "ordinary.f")
            .unwrap();
        let mut tiny_expected = flat(&program, tiny);
        let unchanged = tiny_expected.clone();
        let mut tiny_budget = budget(&program, tiny);
        assert!(!call(tiny, &mut tiny_budget, &mut tiny_expected).unwrap());
        assert_eq!(
            tiny_expected, unchanged,
            "no branch means complete old fallback"
        );
    }
}

#[test]
fn full_old_comparator_rejects_the_same_late_content_terminal_and_multiplicity() {
    let program = dense(4);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let expected = flat(&program, function);
    assert!(expected.len() > 1);
    for change in 0..3 {
        let mut changed = expected.clone();
        match change {
            0 => {
                changed.pop();
            }
            1 => changed.last_mut().unwrap().terminal = SkeletonTerminal::LoopLatch,
            _ => {
                let path = changed
                    .iter_mut()
                    .rev()
                    .find(|path| !path.observations.items.is_empty())
                    .unwrap();
                assert!(path.observations.items.pop().is_some());
            }
        }
        changed.sort();
        // This frozen history comparator predates public unobserved backing:
        // its reserve_vec receiver requires the original ActiveBuilder owner.
        // Keep its complete bytes and every six-field diagnostic comparison.
        let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            let mut old_budget = budget(&program, function);
            assert_eq!(old_budget.payer, AllocationPayer::ActiveBuilder);
            let produced = ordinary_plan_history::produce(function, &mut old_budget).unwrap();
            let old = ordinary_plan_history::before_comparison::compare(
                function,
                &mut old_budget,
                produced,
                &changed,
            )
            .unwrap_err();
            let mut actual_budget = budget(&program, function);
            assert_eq!(actual_budget.payer, AllocationPayer::ActiveBuilder);
            let mut actual_expected = changed.clone();
            let actual = call(function, &mut actual_budget, &mut actual_expected).unwrap_err();
            same_error(actual, old);
        });
        assert!(!refused && first.is_none());

        // The complete immediate-old flat admission caller already admits
        // LegacyUnobserved and supplies real public cursor backing. Compare
        // the actual public route for ALL of the same hostile mutations too.
        let mut old_budget = budget(&program, function);
        assert_eq!(old_budget.payer, AllocationPayer::LegacyUnobserved);
        old_budget
            .reserve_bytes(
                function,
                super::immediate_before::CALL,
                "flat admission test caller",
            )
            .unwrap();
        let mut old_expected = changed.clone();
        let old =
            super::immediate_before::try_compare(function, &mut old_budget, &mut old_expected)
                .unwrap_err();
        let mut actual_budget = budget(&program, function);
        assert_eq!(actual_budget.payer, AllocationPayer::LegacyUnobserved);
        let actual = call(function, &mut actual_budget, &mut changed).unwrap_err();
        same_error(actual, old);
    }
}

#[test]
fn every_original_plan_skeleton_event_cut_keeps_all_six_refusal_fields() {
    let program = dense(2);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let expected = flat(&program, function);
    with_limit_usage_refusal(usize::MAX, || {
        let mut baseline = budget(&program, function);
        ordinary_plan_history::before::produce(function, &mut baseline).unwrap();
        let count = baseline.skeleton_spent;
        assert!(count > 0);
        for cut in 0..=count {
            let mut old_budget = budget(&program, function);
            old_budget.skeleton_remaining = cut;
            let old = ordinary_plan_history::before::produce(function, &mut old_budget).map(drop);
            let mut actual_budget = budget(&program, function);
            actual_budget.skeleton_remaining = cut;
            let mut changed = expected.clone();
            let actual =
                call(function, &mut actual_budget, &mut changed).map(|selected| assert!(selected));
            assert_eq!(old_budget.skeleton_spent, actual_budget.skeleton_spent);
            assert_eq!(old.is_ok(), cut == count);
            assert_eq!(actual.is_ok(), cut == count);
            if let (Err(a), Err(b)) = (old, actual) {
                same_error(a, b);
            }
        }
    });
}

#[test]
fn exact_short_zero_floor_and_complete_drop_charge_every_new_owner() {
    let program = dense(8);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let expected = flat(&program, function);
    let run = |limit, floor| {
        let mut changed = expected.clone(); // Fixture construction is outside observation.
        with_limit_usage_refusal(limit, || {
            set_active_floor(floor);
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result =
                    reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                        .and_then(|_reserved| call(function, &mut budget, &mut changed));
                if result.is_err() {
                    assert!(budget
                        .reserve_bytes(function, 0, "flat admission sticky zero")
                        .is_err());
                }
                let success = matches!(result, Ok(true));
                drop(result); // Include the complete refusal-text owner in Drop observation.
                success
            })
        })
    };
    let ((result, allocation), refused, paid, first) = run(usize::MAX, 0);
    assert!(result);
    assert!(!refused && first.is_none() && !allocation.invalid);
    assert!(paid >= allocation.allocated);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    let ((result, allocation), refused, exact, first) = run(paid, 0);
    assert!(result && !refused && first.is_none());
    assert_eq!(exact, paid);
    assert!(!allocation.invalid && allocation.live == 0);
    for (limit, floor) in [(0, 0), (paid - 1, 0), (paid, 1)] {
        let ((result, allocation), refused, _, first) = run(limit, floor);
        assert!(!result && refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn unsupported_payers_and_lexical_exits_keep_hir_unchanged_and_foreign_auth_is_sticky() {
    let program = dense(2);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let expected = flat(&program, function);
    with_limit_usage_refusal(usize::MAX, || {
        for excluded in 0..6 {
            let mut budget = budget(&program, function);
            let mut changed = expected.clone();
            let mut altered = function.clone();
            match excluded {
                0 => budget.admission = AdmissionMode::SelectedRetained,
                1 => budget.lent_skeleton = true,
                2 => budget.merge_paths = true,
                3 => {
                    altered.cleanup_plan.exits[0].continuation = ExitContinuation::LoopLatch {
                        target: crate::hir::ControlTarget::Function(
                            crate::hir::FunctionExecutionId::Monomorphic(function.id.clone()),
                        ),
                    }
                }
                4 => altered.cleanup_plan.blocks[0].transitions.push(
                    crate::cleanup_plan::CleanupTransition::ReserveRenewal {
                        at: altered.body.id.clone(),
                        binding: crate::cleanup_plan::CleanupPlace {
                            storage: crate::cleanup_plan::StorageId::ProvisionalResult,
                            projections: Vec::new(),
                        },
                    },
                ),
                _ => altered.cleanup_plan.blocks.clear(),
            }
            if excluded < 3 {
                let mut fell_back = false;
                dispatch(&altered, &mut budget, changed, &mut fell_back).unwrap();
                assert!(
                    fell_back,
                    "unsupported payer must not enter the new selector"
                );
            } else {
                assert!(!call(&altered, &mut budget, &mut changed).unwrap());
                assert_eq!(changed, expected);
            }
        }
    });
    let ((inner, restored), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = budget(&program, function);
        budget
            .reserve_bytes(
                function,
                AUTH + OWNER
                    + ClassifierLedger::capture_into_controls()
                    + ClassifierLedger::current_receiving_controls(),
                "held flat auth test",
            )
            .unwrap();
        let mut ledger = ClassifierLedger::empty_receiving();
        ledger.capture_current_into();
        let mut failure = Failure {
            error: None,
            function,
            budget: &budget,
        };
        let mut authenticated = false;
        let inner = with_limit_usage_refusal(usize::MAX, || {
            current_into(&ledger, &mut authenticated, &mut failure);
            if authenticated {
                Ok(())
            } else {
                Err(failure.error.take().unwrap())
            }
        });
        assert!(inner.0.is_err() && !inner.1 && inner.2 == 0 && inner.3.is_none());
        let restored = budget.reserve_bytes(function, 0, "flat auth restored sticky owner");
        (inner, restored)
    });
    assert!(inner.0.is_err() && restored.is_err() && refused && first.is_some());
}

#[test]
fn complete_old_root_bypass_and_source_inverse_survive_actual_whole_callers() {
    use sha2::{Digest, Sha256};
    let before = include_str!("before_shipping.rs.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        "e9d21c81c7596c9c5b81f6ec66b70e9b803bcc9d5ab48ca6d27e3ee21e7ca000"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(include_bytes!("before_emission.rs.txt")))
        ),
        "93d8038785b042f61b5d6bd100b24930d800d0ccc1ae6dfa7b9c6884bb814cc7"
    );
    let mut current = include_str!("../../shipping.rs").to_owned();
    let registration="\n#[path = \"shipping/flat_admission.rs\"]\npub(in crate::cleanup_plan::replay) mod flat_admission;\n";
    assert_eq!(current.matches(registration).count(), 1);
    current = current.replacen(registration, "", 1);
    let hook=concat!(
        "                $budget.reserve_bytes($function,\n",
        "                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::flat_admission::CALL,\n",
        "                    \"flat HIR independent PLAN admission caller\")?;\n",
        "                if $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::shipping::flat_admission::try_compare(\n",
        "                    $function, $budget, &mut $expected)? { return Ok(()); }\n");
    assert_eq!(current.matches(hook).count(), 1);
    assert_eq!(current.replacen(hook, "", 1), before);
    let retained_epoch = crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::retained::before_emission(include_str!("../../emission.rs"));
    let current = retained_epoch.as_str();
    let visible = "pub(in crate::cleanup_plan::replay) fn with_before<T>";
    assert_eq!(current.matches(visible).count(), 1);
    let memo = current.replacen(visible, "fn with_before<T>", 1);
    // Preserve the entire independently captured memo producer, then compose
    // its three finite original stages to the same complete a966 predecessor.
    assert_eq!(memo, include_str!("memo_emission.rs.txt"));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(memo.as_bytes()))
        ),
        "32620bdd298e9c6a5df9246ec302f75ae65df486076bbc72341884015d9144b8"
    );
    let before_emission = include_str!("before_emission.rs.txt");
    let old_loop = &before_emission[before_emission
        .find("    for path in prefixes.iter().chain(suffixes.iter()) {")
        .unwrap()
        ..before_emission
            .find("    let mut result = Vec::new();")
            .unwrap()];
    let new_loop = &memo[memo
        .find("    work.budget.reserve_bytes(\n        work.function,\n        validation::CALL,")
        .unwrap()..memo.find("    let mut result = Vec::new();").unwrap()];
    let bank_start = before_emission
        .find("    + 2 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>()")
        .unwrap();
    let bank_end = bank_start
        + before_emission[bank_start..]
            .find("    + 2 * size_of::<ClassifierLedger>()")
            .unwrap();
    assert_eq!(before_emission.matches(old_loop).count(), 1);
    let expected_memo = before_emission.replacen(old_loop, new_loop, 1).replacen(
        &before_emission[bank_start..bank_end],
        "",
        1,
    ) + "\nmod validation;\n";
    assert_eq!(memo, expected_memo);
    let program = dense(8);
    emission::with_before(|| validate_program(&program)).unwrap();
    validate_program(&program).unwrap();
    with_limit_usage_refusal(usize::MAX, || {
        emission::with_before(|| validate_program(&program)).unwrap();
        validate_program(&program).unwrap();
    });
}

#[test]
fn immediate_flat_driver_and_every_helper_binding_are_exact_source_controls() {
    let captured = include_str!("immediate_before.rs.txt");
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(captured.as_bytes()))
        ),
        "4392abffc59dee7596e750e1523f0e2b0f4c23a5f35c67caa475de5785c4f568"
    );
    let executable = include_str!("immediate_before.rs");
    let tail = "\n#[cfg(test)]\n#[path = \"flat_admission/tests.rs\"]\nmod tests;\n";
    assert!(captured.ends_with(tail));
    let rebound = captured
        .strip_suffix(tail)
        .unwrap()
        .replacen("use super::{", "use super::super::{", 1)
        .replace("super::view::", "super::super::view::")
        .replace(
            "super::enumerated_prefix::",
            "super::super::enumerated_prefix::",
        );
    assert_eq!(executable, rebound);
    // The old authentication producer and each old returning payment remain
    // independently compiled; no successor macro or output owner is shared.
    assert!(executable.contains("fn current(ledger: &ClassifierLedger) -> Result<(), Diagnostic>"));
    assert_eq!(executable.matches("current(&ledger)?;").count(), 4);
    assert_eq!(executable.matches("budget.charge(function, 1,").count(), 2);
    assert!(!executable.contains("reserve_into!"));
    assert!(!executable.contains("charge_into!"));
    // Match the successor producer's complete identifier. The immutable old
    // source already calls capture_current_into(), a distinct ledger API.
    assert_eq!(
        executable.matches("ledger.capture_current_into();").count(),
        1
    );
    assert!(!executable
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .any(|identifier| identifier == "current_into"));
}

#[test]
fn every_immediate_flat_work_cut_and_malformed_fallback_keep_exact_outputs() {
    let program = dense(4);
    let original = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let expected = flat(&program, original);
    for malformed in 0..5 {
        let mut function = original.clone();
        match malformed {
            0 => {}
            1 => function.cleanup_plan.blocks.clear(),
            2 => function.cleanup_plan.exits.clear(),
            3 => function.cleanup_plan.blocks[0].transitions.push(
                crate::cleanup_plan::CleanupTransition::ReserveRenewal {
                    at: function.body.id.clone(),
                    binding: crate::cleanup_plan::CleanupPlace {
                        storage: crate::cleanup_plan::StorageId::ProvisionalResult,
                        projections: Vec::new(),
                    },
                },
            ),
            _ => {
                function.cleanup_plan.exits[0].continuation = ExitContinuation::LoopLatch {
                    target: crate::hir::ControlTarget::Function(
                        crate::hir::FunctionExecutionId::Monomorphic(function.id.clone()),
                    ),
                }
            }
        }
        let mut unlimited = budget(&program, &function);
        let initial_remaining = unlimited.remaining;
        let mut old_expected = expected.clone();
        super::immediate_before::try_compare(&function, &mut unlimited, &mut old_expected).unwrap();
        let normal_work = initial_remaining - unlimited.remaining;
        // Retain every original held-skeleton cut. Independently cut ordinary
        // scan work as well: it uses remaining rather than skeleton_remaining.
        let cuts = (0..=unlimited.skeleton_spent)
            .map(|cut| (cut, None))
            .chain((0..=normal_work).map(|cut| (unlimited.skeleton_spent, Some(cut))));
        for (skeleton_cut, normal_cut) in cuts {
            let mut old_budget = budget(&program, &function);
            let mut new_budget = budget(&program, &function);
            old_budget.skeleton_remaining = skeleton_cut;
            new_budget.skeleton_remaining = skeleton_cut;
            if let Some(cut) = normal_cut {
                old_budget.remaining = cut;
                new_budget.remaining = cut;
            }
            let mut old_expected = expected.clone();
            let mut new_expected = expected.clone();
            let old =
                super::immediate_before::try_compare(&function, &mut old_budget, &mut old_expected);
            let current = call(&function, &mut new_budget, &mut new_expected);
            assert_eq!(old_budget.remaining, new_budget.remaining);
            assert_eq!(old_budget.skeleton_spent, new_budget.skeleton_spent);
            assert_eq!(old_budget.skeleton_remaining, new_budget.skeleton_remaining);
            assert_eq!(old_expected, new_expected);
            match (old, current) {
                (Ok(a), Ok(b)) => assert_eq!(a, b),
                (Err(a), Err(b)) => same_error(a, b),
                _ => panic!("immediate flat driver changed work or fallback result"),
            }
        }
    }
}

#[test]
fn authored_paid_flat_receiver_reduction_includes_complete_cold_drop() {
    let program = dense(64);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "ordinary.main")
        .unwrap();
    let expected = flat(&program, function);
    let run = |old: bool| {
        let mut paths = expected.clone();
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = budget(&program, function);
                let result = if old {
                    budget
                        .reserve_bytes(
                            function,
                            super::immediate_before::CALL,
                            "immediate flat test caller",
                        )
                        .unwrap();
                    super::immediate_before::try_compare(function, &mut budget, &mut paths)
                } else {
                    call(function, &mut budget, &mut paths)
                };
                assert!(matches!(result, Ok(true)));
                drop(result);
                // ReplayBudget, its complete refusal backing and all driver locals
                // drop within the allocation observer on both independently run arms.
            })
        })
    };
    let (((), old_alloc), old_refused, old_paid, old_first) = run(true);
    let (((), new_alloc), new_refused, new_paid, new_first) = run(false);
    assert!(!old_refused && !new_refused && old_first.is_none() && new_first.is_none());
    assert!(
        new_paid < old_paid,
        "new fixed owner must earn its complete renewed receiver invoice"
    );
    for allocation in [old_alloc, new_alloc] {
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}
