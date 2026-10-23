//! Complete independently compiled pre-release caller and immutable owner custody.
use super::*;
use canonical_shared::inline_singleton::hir_last_use;

#[rustfmt::skip]
fn compare_before_memo_release<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Option<bool>, Diagnostic> {
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase =
        budget.paid_phase(crate::workflow_profile::plan_comparison::PaidPhase::FactoredComparison);

    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    budget.reserve_bytes(
        function,
        canonical_shared::inline_singleton::Canonicalizer::CONSTRUCTOR_CONTROLS,
        "canonical ordered dictionary owner receiving",
    )?;
    let mut canonical = canonical_shared::inline_singleton::Canonicalizer::new(&automaton, sink);
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::inline_singleton::Canonicalizer::COLD_NODE_CONTROLS,
            "factored cold terminal node predicate",
        )?;
    }
    if canonical.cold_dead_node(hir_root) {
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
        budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
        let Some(hir) = canonical.id_for_comparison(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        #[cfg(test)]
        let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
        #[cfg(test)]
        let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
        budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
        let Some(plan) = canonical.id_for_comparison(function, vec![(plan_root, 1)], budget)?
        else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS
                + canonical_shared::inline_singleton::Canonicalizer::ACTIVE_DISPATCH_CONTROLS,
            "factored held closure invocation controls",
        )?;
    }
    let mut receiving =
        canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(function, budget);
    let canonical_id = canonical.canonical_dispatch(&budget.payer);
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalHIR);
    #[cfg(test)]
    let _successor_phase = canonical_shared::lexical_successors::census::phase(true);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
    let Some(hir) = canonical_id(
        &mut canonical,
        function,
        vec![(hir_root, 1)],
        &mut *budget,
        &mut receiving,
    )?
    else {
        return Ok(None);
    };
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::CanonicalPLAN);
    #[cfg(test)]
    let _successor_phase = canonical_shared::lexical_successors::census::phase(false);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
    let Some(plan) = canonical_id(
        &mut canonical,
        function,
        vec![(plan_root, 1)],
        &mut *budget,
        &mut receiving,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}

use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
fn fixture(width: usize) -> ResolvedProgram {
    let calls = std::iter::repeat_n("f()", width.max(1))
        .collect::<Vec<_>>()
        .join(" + ");
    let source = format!("module ordinary; @id(\"ordinary.f\") fn f()->i64 {{ 1 }} @id(\"ordinary.main\") fn main()->i64 {{ {calls} }}");
    crate::hir::resolve(&crate::check(&source, "memo-release.spx").unwrap()).unwrap()
}
fn canonical<'a>(
    built: &'a BuiltComparison,
) -> canonical_shared::inline_singleton::Canonicalizer<'a> {
    canonical_shared::inline_singleton::Canonicalizer::new(&built.automaton, built.sink)
}
fn receiving(
    function: &ResolvedFunction,
    budget: &ReplayBudget,
) -> canonical_shared::closure_receiving::ClosureReceiving {
    canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(function, budget)
}
#[test]
fn actual_builder_partition_releases_only_private_payloads_and_retains_all_capacity_and_ids() {
    let program = fixture(8);
    for function in &program.functions {
        let mut budget = ReplayBudget::with_skeleton_limit(0);
        let built = build_comparison(&program, function, &mut budget, None, None).unwrap();
        assert_eq!((built.sink, built.hir_root), (0, 4));
        assert_eq!(function.cleanup_plan.entry.0, 0);
        let mut canonical = canonical(&built);
        let mut receiving = receiving(function, &budget);
        let hir = canonical
            .id_with_compact_receiving(
                function,
                vec![(built.hir_root, 1)],
                &mut budget,
                &mut receiving,
            )
            .unwrap();
        assert!(hir.is_some());
        let common = (0..4)
            .map(|key| canonical.private_release_state(key))
            .collect::<Vec<_>>();
        let before = canonical.private_release_receipt();
        budget
            .reserve_bytes(
                function,
                hir_last_use::CALL,
                "private release test invocation",
            )
            .unwrap();
        assert!(canonical
            .release_hir_private(function, &mut budget, built.hir_root, built.plan_root)
            .unwrap());
        let after = canonical.private_release_receipt();
        assert_eq!(&before[..3], &after[..3]);
        assert_eq!(&before[4..], &after[4..]);
        assert!(after[3] <= before[3]);
        for key in 0..4 {
            assert_eq!(canonical.private_release_state(key), common[key]);
        }
        for key in built.hir_root..built.plan_root {
            assert_ne!(canonical.private_release_state(key).0, 2);
        }
        let plan = canonical
            .id_with_compact_receiving(
                function,
                vec![(built.plan_root, 1)],
                &mut budget,
                &mut receiving,
            )
            .unwrap();
        assert_eq!(hir, plan);
    }
}
#[test]
fn shared_invalid_or_different_entry_partitions_keep_the_complete_old_memo() {
    let program = fixture(8);
    let original = program.functions.last().unwrap();
    for case in 0..6 {
        let mut function = original.clone();
        let mut budget = ReplayBudget::with_skeleton_limit(0);
        let mut built = build_comparison(&program, &function, &mut budget, None, None).unwrap();
        let mut hir_root = built.hir_root;
        match case {
            0 => function.cleanup_plan.entry = crate::cleanup_plan::BlockId(1),
            1 => built.sink = 1,
            2 => hir_root = 3,
            3 => built.automaton.nodes[built.plan_root] = Node::Epsilon(built.hir_root),
            4 => built.automaton.nodes[1] = Node::Empty,
            _ => built.automaton.nodes[built.plan_root] = Node::Epsilon(usize::MAX),
        }
        let mut canonical = canonical(&built);
        canonical.seed_private_release_test(&function, &mut budget, built.hir_root);
        let before = canonical.private_release_receipt();
        assert!(!canonical
            .release_hir_private(&function, &mut budget, hir_root, built.plan_root)
            .unwrap());
        assert_eq!(canonical.private_release_receipt(), before);
        assert_eq!(canonical.private_release_state(built.hir_root).0, 2);
    }
}
#[test]
fn original_plan_canonical_work_cuts_keep_full_diagnostics_after_separately_paid_release() {
    let program = fixture(8);
    let function = program.functions.last().unwrap();
    for cut in (0..128).chain([256, 1024, MAX_REPLAY_WORK_UNITS]) {
        let mut build_budget = ReplayBudget::with_skeleton_limit(0);
        let built = build_comparison(&program, function, &mut build_budget, None, None).unwrap();
        let mut old = canonical(&built);
        let mut new = canonical(&built);
        let mut old_budget = ReplayBudget::unobserved();
        let mut new_budget = ReplayBudget::unobserved();
        let mut old_receiving = receiving(function, &old_budget);
        let mut new_receiving = receiving(function, &new_budget);
        let old_hir = old
            .id_with_compact_receiving(
                function,
                vec![(built.hir_root, 1)],
                &mut old_budget,
                &mut old_receiving,
            )
            .unwrap();
        let new_hir = new
            .id_with_compact_receiving(
                function,
                vec![(built.hir_root, 1)],
                &mut new_budget,
                &mut new_receiving,
            )
            .unwrap();
        assert_eq!(new_hir, old_hir);
        assert_eq!(new_budget.remaining, old_budget.remaining);
        let before = new_budget.remaining;
        assert!(new
            .release_hir_private(function, &mut new_budget, built.hir_root, built.plan_root)
            .unwrap());
        assert!(new_budget.remaining < before);
        // This is a PLAN-only work window, after the independently observed
        // release fee. No complete caller quota or allocation receipt is adjusted.
        old_budget.remaining = cut;
        new_budget.remaining = cut;
        let old_result = old.id_with_compact_receiving(
            function,
            vec![(built.plan_root, 1)],
            &mut old_budget,
            &mut old_receiving,
        );
        let new_result = new.id_with_compact_receiving(
            function,
            vec![(built.plan_root, 1)],
            &mut new_budget,
            &mut new_receiving,
        );
        match (old_result, new_result) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert_eq!(a.json(), b.json()),
            pair => panic!("PLAN work frontier differs: {pair:?}"),
        }
        assert_eq!(
            (
                new_budget.remaining,
                new_budget.skeleton_remaining,
                new_budget.skeleton_spent
            ),
            (
                old_budget.remaining,
                old_budget.skeleton_remaining,
                old_budget.skeleton_spent
            )
        );
    }
}
#[test]
fn complete_before_caller_and_current_cold_comparison_keep_verdicts_and_final_drop() {
    let program = fixture(64);
    let function = program.functions.last().unwrap();
    let run = |old| {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(0);
            let result = if old {
                compare_before_memo_release(&program, function, &mut budget, None, None)
            } else {
                compare_with_index_storage(&program, function, &mut budget, None, None)
            };
            let result = result.unwrap();
            let spent = MAX_REPLAY_WORK_UNITS - budget.remaining;
            drop(budget);
            (result, spent)
        })
    };
    let (old_result, old) = run(true);
    let (new_result, new) = run(false);
    assert_eq!(new_result.0, old_result.0);
    assert!(new_result.1 >= old_result.1);
    assert!(!old.invalid && !new.invalid && old.live == 0 && new.live == 0);
    assert_eq!(old.allocated, old.freed);
    assert_eq!(new.allocated, new.freed);
    assert_eq!(
        (new.allocations, new.reallocations, new.allocated),
        (old.allocations, old.reallocations, old.allocated)
    );
    assert!(
        new.highwater < old.highwater,
        "actual complete before={old:?} current={new:?}"
    );
}
#[test]
fn release_foreign_and_new_work_refusals_keep_one_sticky_owner_and_full_diagnostic() {
    let program = fixture(8);
    let function = program.functions.last().unwrap();
    for foreign in [false, true] {
        let other = function.clone();
        let mut budget = ReplayBudget::with_skeleton_limit(0);
        let built = build_comparison(&program, function, &mut budget, None, None).unwrap();
        let mut canonical = canonical(&built);
        canonical.seed_private_release_test(function, &mut budget, built.hir_root);
        if foreign {
            let remaining = budget.remaining;
            assert_eq!(
                canonical.release_hir_private(&other, &mut budget, built.hir_root, built.plan_root),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
            assert_eq!(budget.remaining, remaining);
        } else {
            budget.remaining = 0;
            let actual = canonical
                .release_hir_private(function, &mut budget, built.hir_root, built.plan_root)
                .unwrap_err();
            let mut expected = ReplayBudget::unobserved();
            expected.remaining = 0;
            assert_eq!(
                actual.json(),
                expected
                    .charge(function, 1, "factored common terminal partition proof")
                    .unwrap_err()
                    .json()
            );
        }
        assert_eq!(canonical.private_release_state(built.hir_root).0, 0);
        assert_eq!(
            canonical.release_hir_private(function, &mut budget, built.hir_root, built.plan_root),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
    }
}

#[test]
fn complete_before_caller_and_all_old_canonical_memo_bytes_remain_exact() {
    let digest = |b: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(b)));
    let saved = include_str!("hir_memo_release/compare.before.txt");
    let current = include_str!("hir_memo_release_tests.rs");
    let marker = "fn compare_before_memo_release<'hir>(";
    let first = current.find(marker).unwrap();
    let last = current[first..]
        .find("\nuse crate::test_allocation_observer::measure;")
        .unwrap()
        + first;
    let body = current[first..last].trim_end_matches('\n').to_owned() + "\n";
    assert_eq!(
        body.replacen(marker, "fn compare_with_index_storage<'hir>(", 1),
        saved
    );
    assert!(!saved.contains("release_hir_private"));
    assert!(!saved.contains("private_release_receipt"));
    let encoded = include_str!("hir_memo_release/source_inverse.json");
    assert_eq!(
        digest(encoded.as_bytes()),
        "83e344607710eef8375d8c69bdd13ba6bbdcca8fd4771f9f3a6cd7af3780e986"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::read(
            root.join("src/cleanup_plan/replay/factored/hir_memo_release/source_inverse.json")
        )
        .unwrap(),
        encoded.as_bytes()
    );
    let rows: serde_json::Value = serde_json::from_str(encoded).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let compiled = match path {
            "src/cleanup_plan/replay/factored.rs" => include_bytes!("../factored.rs").as_slice(),
            "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton.rs" => {
                include_bytes!("canonical_shared/inline_singleton.rs").as_slice()
            }
            "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs" => {
                include_bytes!("canonical_shared/empty_memo.rs").as_slice()
            }
            _ => panic!("unbound canonical source"),
        };
        let mut actual = std::fs::read(root.join(path)).unwrap();
        assert_eq!(actual, compiled);
        assert_eq!(actual.len() as u64, row["current_length"].as_u64().unwrap());
        assert_eq!(digest(&actual), row["current_sha256"].as_str().unwrap());
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let at = edit[0].as_u64().unwrap() as usize;
            let added = edit[1].as_str().unwrap().as_bytes();
            // The complete existing fields, methods and typed tariffs are all
            // byte-identical. Only new methods/registration/caller sites exist.
            assert_eq!(edit[2].as_str().unwrap(), "");
            assert_eq!(&actual[at..at + added.len()], added);
            actual.drain(at..at + added.len());
        }
        let before = match row["before_file"].as_str().unwrap() {
            "factored.before.txt" => {
                include_bytes!("hir_memo_release/factored.before.txt").as_slice()
            }
            "canonical.before.txt" => {
                include_bytes!("hir_memo_release/canonical.before.txt").as_slice()
            }
            "memo.before.txt" => include_bytes!("hir_memo_release/memo.before.txt").as_slice(),
            _ => panic!("unbound immediate predecessor"),
        };
        assert_eq!(actual, before);
        assert_eq!(actual.len() as u64, row["before_length"].as_u64().unwrap());
        assert_eq!(digest(&actual), row["before_sha256"].as_str().unwrap());
    }
}

#[test]
fn foreign_budget_and_nested_active_scope_cannot_pay_or_revive_release_authority() {
    let program = fixture(2);
    let function = program.functions.last().unwrap();
    for nested in [false, true] {
        crate::bounded_output::with_limit_usage_refusal(1 << 28, || {
            let mut build_budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let built =
                build_comparison(&program, function, &mut build_budget, None, None).unwrap();
            let mut owner = canonical(&built);
            let mut budget = ReplayBudget::new();
            owner.seed_private_release_test(function, &mut budget, built.hir_root);
            budget
                .reserve_bytes(
                    function,
                    hir_last_use::CALL,
                    "release authority test receiving",
                )
                .unwrap();
            if nested {
                let (result, refused, paid, first) =
                    crate::bounded_output::with_limit_usage_refusal(1 << 20, || {
                        owner.release_hir_private(
                            function,
                            &mut budget,
                            built.hir_root,
                            built.plan_root,
                        )
                    });
                assert_eq!(result, Err(Diagnostic::io("SPX-H006", String::new())));
                assert!(!refused && paid == 0 && first.is_none());
            } else {
                let mut other = ReplayBudget::new();
                let remaining = crate::bounded_output::active_remaining();
                assert_eq!(
                    owner.release_hir_private(
                        function,
                        &mut other,
                        built.hir_root,
                        built.plan_root
                    ),
                    Err(Diagnostic::io("SPX-H006", String::new()))
                );
                assert_eq!(crate::bounded_output::active_remaining(), remaining);
            }
            assert_eq!(owner.private_release_state(built.hir_root).0, 0);
            assert_eq!(
                owner.release_hir_private(function, &mut budget, built.hir_root, built.plan_root),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
        });
    }
}
