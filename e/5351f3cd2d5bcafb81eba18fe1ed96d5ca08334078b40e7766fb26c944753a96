use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
const SOURCE: &str = "module receiving; @id(\"f\") fn f()->i64 {let x=if 1==1 {2}else{3};x} @id(\"main\") fn main()->i64 {0}";
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "continuation-receiving.spx").unwrap()).unwrap()
}

#[test]
fn complete_storage_backed_comparison_pairs_keep_the_original_funded_verdict() {
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old_budget = ReplayBudget::new();
                let mut budget = ReplayBudget::new();
                let mut old =
                    continuation_storage::Storage::for_program(&program, &mut old_budget).unwrap();
                let mut current =
                    continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
                assert!(old.has_receiving_owner() && current.has_receiving_owner());
                old.authenticate(&program, function, &mut old_budget)
                    .unwrap();
                current
                    .authenticate(&program, function, &mut budget)
                    .unwrap();
                let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
                before::cross_check(&program, function, &mut old);
                assert_eq!(SKELETON_MATERIALIZATIONS.with(Cell::get), saved);
                super::cross_check(&program, function, &mut current).unwrap();
                assert_eq!(SKELETON_MATERIALIZATIONS.with(Cell::get), saved);
                drop(old);
                drop(current);
                drop(old_budget);
                drop(budget);
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn complete_cross_check_returns_refusal_at_zero_and_one_short_without_losing_the_counter() {
    let program = program();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let function = &program.functions[0];
                let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
                let mut budget = ReplayBudget::new();
                let mut storage =
                    match continuation_storage::Storage::for_program(&program, &mut budget) {
                        Ok(storage) => storage,
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            assert!(crate::bounded_output::active_refused());
                            drop(error);
                            return false;
                        }
                    };
                assert!(storage.has_receiving_owner());
                let result = oracle_hook::cross_check_with_storage(
                    &program,
                    function,
                    &mut budget,
                    Some(&mut storage),
                );
                assert_eq!(SKELETON_MATERIALIZATIONS.with(Cell::get), saved);
                if let Err(error) = &result {
                    assert_eq!(error.code, "SPX-H006");
                    assert!(crate::bounded_output::active_refused());
                    assert_eq!(
                        (error.path.as_ref(), error.span, error.help.as_ref()),
                        (None, None, None)
                    );
                }
                let success = result.is_ok();
                drop(result);
                drop(storage);
                drop(budget);
                success
            })
        })
    };
    let ((success, allocation), refused, exact, first) = observe(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact);
    assert!(exact > 0);
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = observe(limit);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
        if limit == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(paid, exact);
        } else {
            assert!(!success && refused && first.is_some());
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn whole_original_and_current_comparison_pairs_execute_with_paid_last_drop() {
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            // Full original pair stays inside the same owning allocation window.
            for function in &program.functions {
                super::super::cross_check(&program, function);
            }
            let mut budget = ReplayBudget::new();
            let mut storage =
                continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
            let mut pointer = None;
            for function in &program.functions {
                oracle_hook::cross_check_with_storage(
                    &program,
                    function,
                    &mut budget,
                    Some(&mut storage),
                )
                .unwrap();
                let frames = storage.frames.as_ref().unwrap();
                assert!(frames.is_empty());
                if let Some(previous) = pointer {
                    assert_eq!(frames.as_ptr(), previous);
                }
                pointer = Some(frames.as_ptr());
            }
            drop(storage);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}
#[test]
fn current_factored_storage_preserves_old_automata_work_and_full_hostile_diagnostic() {
    // Preserve the malformed inherited panic separately from the stable diagnostic.
    // Both complete algorithms still run, including their unwind and last Drop.
    let mut malformed = program();
    malformed.functions[0].cleanup_plan.blocks.clear();
    // The default harness hook may initialize retained output/backtrace caches.
    // Replace only that test instrumentation outside the complete compiler
    // observation. Both actual panic payloads, unwinds and final Drop stay in it.
    struct RestoreHook<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for RestoreHook<F> {
        fn drop(&mut self) {
            self.0.take().expect("one hook restoration")();
        }
    }
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let restore_hook = RestoreHook(Some(|| std::panic::set_hook(previous_hook)));
    // Catch the entire observation too: restoring a hook during active unwind
    // would panic. Restore normally first, then resume any assertion failure.
    let measured = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let function = &malformed.functions[0];
                let mut old_budget = ReplayBudget::with_skeleton_limit(0);
                let mut budget = ReplayBudget::with_skeleton_limit(0);
                let mut storage =
                    continuation_storage::Storage::for_program(&malformed, &mut budget).unwrap();
                let old = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_comparison(&malformed, function, &mut old_budget, None, None)
                }))
                .err()
                .expect("complete old receiver retains its malformed-block panic");
                let current = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build_comparison(&malformed, function, &mut budget, None, Some(&mut storage))
                }))
                .err()
                .expect("current receiver retains the same malformed-block panic");
                fn message(payload: &(dyn std::any::Any + Send)) -> Option<&str> {
                    payload
                        .downcast_ref::<String>()
                        .map(String::as_str)
                        .or_else(|| payload.downcast_ref::<&'static str>().copied())
                }
                assert_eq!(
                    message(&*old),
                    Some("index out of bounds: the len is 0 but the index is 0")
                );
                assert_eq!(message(&*current), message(&*old));
                drop(old);
                drop(current);
                drop(storage);
                drop(old_budget);
                drop(budget);
            })
        })
    }));
    drop(restore_hook);
    let (((), allocation), refused, _, first) = match measured {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    eprintln!(
        "malformed old/current compiler panic observation: {allocation:?}; invalid_reason={:?}",
        crate::test_allocation_observer::invalid_reason()
    );
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    // Rust panic payload/hook allocations are observed too, but have no compiler
    // debit claim. The valid compiler cases below retain the full funding bound.
    for hostile in [false, true] {
        let mut program = program();
        if hostile {
            let plan = &mut program.functions[0].cleanup_plan;
            let edge = plan
                .blocks
                .iter()
                .find_map(|block| {
                    if let CleanupTerminator::Branch(edges) = &block.terminator {
                        edges.first().copied()
                    } else {
                        None
                    }
                })
                .expect("fixture has a real branch edge");
            plan.edges[edge.0 as usize].condition = EdgeCondition::Always;
        }
        let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let function = &program.functions[0];
                let mut old_budget = ReplayBudget::with_skeleton_limit(0);
                let mut budget = ReplayBudget::with_skeleton_limit(0);
                let mut storage =
                    continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
                let old = build_comparison(&program, function, &mut old_budget, None, None);
                let current =
                    build_comparison(&program, function, &mut budget, None, Some(&mut storage));
                assert_eq!(
                    (
                        old_budget.remaining,
                        old_budget.skeleton_spent,
                        old_budget.skeleton_remaining
                    ),
                    (
                        budget.remaining,
                        budget.skeleton_spent,
                        budget.skeleton_remaining
                    )
                );
                match (old, current) {
                    (Ok(old), Ok(current)) => {
                        assert!(!hostile);
                        assert_eq!(
                            (old.sink, old.hir_root, old.plan_root),
                            (current.sink, current.hir_root, current.plan_root)
                        );
                        assert_eq!(old.automaton.nodes.len(), current.automaton.nodes.len());
                        for (a, b) in old.automaton.nodes.iter().zip(&current.automaton.nodes) {
                            assert_eq!(a.moves(), b.moves());
                            assert_eq!(a.epsilon(), b.epsilon());
                        }
                    }
                    (Err(old), Err(current)) => {
                        assert!(hostile);
                        assert_eq!(old, current);
                        assert_eq!(old.code, "SPX-H006");
                        assert!(old
                            .message
                            .ends_with("branch skeleton contains an unconditional edge"));
                    }
                    _ => panic!("complete old/current verdict mismatch"),
                }
                drop(storage);
                drop(old_budget);
                drop(budget);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= paid);
    }
}
#[test]
fn paid_factored_receiving_exact_short_zero_keeps_first_failure_and_whole_drop() {
    let program = program();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::with_skeleton_limit(0);
                let mut storage =
                    match continuation_storage::Storage::for_program(&program, &mut budget) {
                        Ok(value) => value,
                        Err(error) => {
                            assert_eq!(error.code, "SPX-H006");
                            drop(error);
                            return false;
                        }
                    };
                let result =
                    build_comparison(&program, function, &mut budget, None, Some(&mut storage));
                let success = result.is_ok();
                if let Err(error) = &result {
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.path, None);
                    assert_eq!(error.span, None);
                    assert_eq!(error.help, None);
                    let at = current_metrics().allocated;
                    let retry = budget.reserve_bytes(function, 0, "retry").unwrap_err();
                    assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                    drop(retry);
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(result);
                drop(storage);
                drop(budget);
                success
            })
        })
    };
    let ((success, allocation), refused, exact, first) = observe(usize::MAX);
    assert!(success && !refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= exact);
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, paid, first) = observe(limit);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        if limit == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(paid, exact);
        } else {
            assert!(!success && refused && first.is_some());
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
#[test]
fn foreign_then_restored_invocation_cannot_acquire_or_reuse_held_frames() {
    let program = program();
    for cached in [false, true] {
        let expected = Diagnostic::io("SPX-H006", format!("cleanup plan for function `{}` failed independent replay: typed-HIR continuation storage belongs to another invocation", program.functions[0].id));
        let (((), allocation), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::new();
                let mut storage =
                    continuation_storage::Storage::for_program(&program, &mut budget).unwrap();
                if cached {
                    let (lease, frames) = storage.acquire(function, &mut budget).unwrap();
                    storage.release(lease, frames);
                }
                let pointer = storage.frames.as_ref().unwrap().as_ptr();
                let (_, nested_refused, nested_used, nested_first) =
                    with_limit_usage_refusal(usize::MAX, || {
                        let at = current_metrics().allocated;
                        let error = storage
                            .authenticate(&program, function, &mut budget)
                            .unwrap_err();
                        assert_eq!(error, expected);
                        drop(error);
                        assert_eq!(current_metrics().allocated, at);
                    });
                assert!(!nested_refused && nested_used == 0 && nested_first.is_none());
                let at = current_metrics().allocated;
                let error = storage
                    .authenticate(&program, function, &mut budget)
                    .unwrap_err();
                assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
                drop(error);
                assert_eq!(current_metrics().allocated, at);
                assert_eq!(storage.frames.as_ref().unwrap().as_ptr(), pointer);
                drop(storage);
                drop(budget);
            })
        });
        assert!(refused && first.is_some());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn complete_prior_receiver_and_funded_disagreement_assertion_remain_byte_exact() {
    use sha2::{Digest, Sha256};
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/storage-oracle-refusal-v1/continuation_receiver.before.rs.txt"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "c946aa0786e0ec977aeb233ad8b12cf186076d2e3352dcad201377bb18f7a7ca"
    );
    let before = include_str!("before.rs");
    assert_eq!(
        before,
        original.strip_suffix("#[cfg(test)]\nmod tests;\n").unwrap()
    );
    let changes = [
        ("    let _debit_route = debit_routes::route(debit_routes::Route::UnoptedCrosscheck);\n", ""),
        ("    let _debit_phase = debit_routes::phase(debit_routes::Phase::EnumeratedHIR);\n", ""),
        ("        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);\n", ""),
        ("        let _debit_phase = debit_routes::phase(debit_routes::Phase::PathComparison);\n", ""),
        ("    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);\n", ""),
        ("    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let paid_enumeration = enumerating\n        .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckEnumerated);\n", ""),
        ("    #[cfg(feature = \"unstable-workflow-profiling\")]\n    drop(paid_enumeration);\n", ""),
        ("        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_crosscheck = factored\n            .paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::CrosscheckFactored);\n", ""),
        (") -> Result<(), Diagnostic> {", ") {"),
        ("        let verdict =\n            match compare_with_index_storage(program, function, &mut factored, None, Some(storage))\n            {\n                // A refused ledger cannot finance a verdict. Preserve the exact\n                // error and let the caller report its sticky builder refusal.\n                Err(error) if crate::bounded_output::active_refused() => {\n                    SKELETON_MATERIALIZATIONS.with(|count| count.set(saved));\n                    return Err(error);\n                }\n                verdict => verdict,\n            };\n", "        let verdict =\n            compare_with_index_storage(program, function, &mut factored, None, Some(storage));\n"),
        ("    Ok(())\n", ""),
        ("#[cfg(test)]\nmod before;\n", ""),

    ];
    let mut restored = include_str!("../continuation_receiver.rs").to_owned();
    for (new, previous) in changes {
        assert_eq!(restored.matches(new).count(), 1);
        restored = restored.replacen(new, previous, 1);
    }
    assert_eq!(restored, original);
}

#[test]
fn unpaid_empty_backing_rejects_foreign_nonempty_and_live_payers_without_allocating() {
    fn signature(storage: &continuation_storage::Storage<'_>) -> Option<(usize, usize, usize)> {
        storage
            .frames
            .as_ref()
            .map(|v| (v.as_ptr() as usize, v.len(), v.capacity()))
    }
    fn reject<'a>(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        storage: &mut continuation_storage::Storage<'a>,
    ) {
        let before = signature(storage);
        let (error, allocation) =
            measure(|| oracle_hook::cross_check_unpaid(program, function, storage).unwrap_err());
        assert_eq!(error, Diagnostic::io_static("SPX-H006", "unpaid continuation sharing requires idle empty same-program storage without a live payer"));
        assert_eq!(signature(storage), before);
        assert_eq!(allocation.allocations, 0);
        assert_eq!(allocation.reallocations, 0);
        assert_eq!(allocation.allocated, 0);
        assert_eq!(allocation.freed, 0);
        assert_eq!(allocation.live, 0);
        assert!(!allocation.invalid);
    }
    let program = program();
    let function = &program.functions[0];
    let mut storage = continuation_storage::Storage::new(&program);
    let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
    super::super::cross_check(&program, function);
    oracle_hook::cross_check_unpaid(&program, function, &mut storage).unwrap();
    assert_eq!(SKELETON_MATERIALIZATIONS.with(Cell::get), saved);
    assert!(storage.frames.as_ref().unwrap().is_empty());
    assert!(storage.frames.as_ref().unwrap().capacity() <= continuation_storage::FRAME_CAPACITY);
    let idle = signature(&storage);
    oracle_hook::cross_check_unpaid(&program, function, &mut storage).unwrap();
    assert_eq!(signature(&storage), idle);
    assert_eq!(SKELETON_MATERIALIZATIONS.with(Cell::get), saved);
    let foreign =
        crate::hir::resolve(&crate::check(SOURCE, "foreign-continuation.spx").unwrap()).unwrap();
    let mut foreign_storage = continuation_storage::Storage::new(&foreign);
    reject(&program, function, &mut foreign_storage);
    let mut nonempty = continuation_storage::Storage::new(&program);
    nonempty
        .frames
        .as_mut()
        .unwrap()
        .push(continuation_storage::Frame::Eval(&function.body));
    reject(&program, function, &mut nonempty);
    for limit in [0, usize::MAX] {
        let ((), refused, paid, first) =
            with_limit_usage_refusal(limit, || reject(&program, function, &mut storage));
        assert!(!refused && first.is_none());
        assert_eq!(paid, 0);
    }
    let ((), refused, paid, first) = with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert!(crate::bounded_output::active_refused());
        reject(&program, function, &mut storage);
        assert!(crate::bounded_output::active_refused());
    });
    assert!(refused && first.is_some());
    assert_eq!(paid, 0);
}

#[test]
fn unpaid_receiver_full_source_is_exact_old_comparison_plus_closed_empty_backing_route() {
    let actual = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/factored/unpaid_continuation_receiver.rs"
    ));
    // Reverse only the complete new storage-only operations, then retain
    // the entire original independent comparison/source equality below.
    let added = r########"    .and_then(|mut hir| {
        // Release unused capacity before the independently rebuilt PLAN is
        // live. This moves no semantic field and retains every HIR path.
        if hir.capacity() > hir.len() {
            enumerating.charge_skeleton(function, hir.len(), "unpaid HIR vector compaction")?;
            hir.shrink_to_fit();
        }
        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);"########;
    let prior = r########"    .and_then(|mut hir| {
        let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);"########;
    assert_eq!(actual.matches(added).count(), 1);
    let actual = actual.replacen(added, prior, 1);
    let sorted = r########"        // Equal full paths are interchangeable in this private multiset
        // comparison; unstable sorting preserves every duplicate and field.
        hir.sort_unstable();
        plan.sort_unstable();"########;
    let prior_sort = r########"        hir.sort();
        plan.sort();"########;
    assert_eq!(actual.matches(sorted).count(), 1);
    let actual = actual.replacen(sorted, prior_sort, 1);
    let actual = actual.as_str();
    let paid = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/factored/continuation_receiver.rs"
    ));
    let header = "//! Full original comparison pair using only idle empty unpaid backing.\nuse super::super::*;\n";
    let guard = "    // Empty backing is not proof, an owning IR/result bank, or a quota.\n    // This route cannot consume or inherit any live allocation account.\n    if crate::bounded_output::active_remaining().is_some()\n        || !matches!(crate::cleanup_plan::replay::allocation::opted(), Ok(None))\n        || storage.has_receiving_owner()\n        || !storage.matches(program)\n        || !storage.frames.as_ref().is_some_and(Vec::is_empty)\n    {\n        return Err(Diagnostic::io_static(\"SPX-H006\", \"unpaid continuation sharing requires idle empty same-program storage without a live payer\"));\n    }\n";
    let changed_call = "    let enumerated = hir_skeleton_paths_with_index_storage(\n        program,\n        function,\n        &mut enumerating,\n        None,\n        Some(&mut *storage),\n    )\n    .and_then(|mut hir| {";
    let ordinary_call = "    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {";
    assert!(actual.starts_with(header));
    assert_eq!(actual.matches(guard).count(), 1);
    assert_eq!(actual.matches(changed_call).count(), 1);
    let restored = actual
        .strip_prefix(header)
        .unwrap()
        .replacen(guard, "", 1)
        .replacen(changed_call, ordinary_call, 1);
    let expected = &paid[paid.find("pub(super) fn cross_check").unwrap()
        ..paid.find("\n#[cfg(test)]\nmod before;").unwrap()];
    assert_eq!(
        restored.trim_end_matches('\n'),
        expected.trim_end_matches('\n')
    );
}

#[test]
fn unpaid_compact_comparison_preserves_full_multisets_and_all_duplicate_values() {
    let program = program();
    for function in &program.functions {
        let mut hir_budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut plan_budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let languages = [
            crate::cleanup_plan::replay::hir_skeleton_paths(&program, function, &mut hir_budget)
                .unwrap(),
            crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut plan_budget).unwrap(),
        ];
        for language in languages {
            for repeats in [1, 2, 7] {
                let mut stable = std::iter::repeat_n(&language, repeats)
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>();
                let mut compact = stable.clone();
                let original_count = stable.len();
                stable.sort();
                compact.shrink_to_fit();
                compact.sort_unstable();
                assert_eq!(
                    compact, stable,
                    "complete observation values, terminal fields and multiset multiplicities"
                );
                assert_eq!(compact.len(), original_count);
            }
        }
    }
}
