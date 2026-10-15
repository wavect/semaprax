use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module prefix.lookup; @id(\"prefix.main\") fn main() -> i64 { 0 }",
            "prefix-lookup.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

fn arena<'hir>(function: &'hir ResolvedFunction, keys: &[ExpressionId]) -> Arena<'hir> {
    let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
    let mut capacity =
        capacity::FactCapacity::with_payer(function, capacity::Payer::Selected(&mut physical))
            .unwrap();
    let mut arena = Arena::new(&mut capacity).unwrap();
    let mut budget = ReplayBudget::unobserved();
    for key in keys {
        arena.prepare_growth_work(function, &mut budget).unwrap();
        arena.reserve_next(&mut capacity).unwrap();
        arena
            .prepare_insert_work(function, key, &mut budget)
            .unwrap();
        assert!(arena.insert(key.clone(), None, &mut capacity).unwrap());
    }
    assert!(arena.balanced());
    arena
}

fn hit(
    function: &ResolvedFunction,
    arena: &Arena<'_>,
    query: &ExpressionId,
    old: bool,
) -> (bool, usize) {
    let mut budget = ReplayBudget::unobserved();
    let (result, allocation) = measure(|| {
        let result = if old {
            arena.find(function, query, &mut budget)
        } else {
            find(arena, function, query, &mut budget)
        };
        result.unwrap().is_some()
    });
    assert!(!allocation.invalid && allocation.allocated == 0 && allocation.live == 0);
    (result, MAX_REPLAY_WORK_UNITS - budget.remaining)
}

#[test]
fn complete_old_lookup_matches_every_opaque_byte_endpoint_hit_miss_and_prefix() {
    let program = program();
    let function = &program.functions[0];
    let mut texts = vec![
        "".to_owned(),
        "\0".to_owned(),
        "\0\0".to_owned(),
        "a".to_owned(),
        "aa".to_owned(),
        "a\0".to_owned(),
        "a\0b".to_owned(),
        "α".to_owned(),
        "β".to_owned(),
        "λ".to_owned(),
        "λ\0".to_owned(),
        "識別子".to_owned(),
        "🙂".to_owned(),
    ];
    for index in 0..257 {
        texts.push(format!(
            "opaque.{}.{index:03}\0suffix",
            "λ\0識別子🙂".repeat(24)
        ));
    }
    texts.sort();
    texts.dedup();
    let keys: Vec<_> = texts
        .iter()
        .cloned()
        .map(ExpressionId::from_owned)
        .collect();
    let mut saw_saving = false;
    let (mut old_total, mut new_total) = (0usize, 0usize);
    for reversed in [false, true] {
        let mut order = keys.clone();
        if reversed {
            order.reverse();
        }
        let arena = arena(function, &order);
        for key in &keys {
            let old = hit(function, &arena, key, true);
            let new = hit(function, &arena, key, false);
            assert!(old.0 && new.0);
            let height = usize::from(arena.height(arena.root));
            let seed_and_bookkeeping = 4
                + 3
                + 3 * height
                + 2 * (key.as_str().len() + 1 + BOUND_UNITS)
                + BOUND_UNITS * height;
            assert!(new.1 <= old.1 + seed_and_bookkeeping);
            saw_saving |= new.1 < old.1;
            old_total += old.1;
            new_total += new.1;
        }
        for query in ["\0a", "a\0a", "ab", "λ\0b", "識別", "🙂\0", "zzzz"] {
            let query = ExpressionId::from_owned(query.into());
            assert_eq!(
                hit(function, &arena, &query, true).0,
                hit(function, &arena, &query, false).0
            );
        }
    }
    assert!(
        saw_saving,
        "actual repeated full-byte comparisons must be eliminated"
    );
    assert!(
        new_total < old_total,
        "complete long-prefix hit workload must reduce work"
    );
    // Separate nonempty endpoints permit both early misses, including strings
    // above every Unicode key. Both insertion orders exercise both branches.
    let endpoint_keys: Vec<_> = (0..33)
        .map(|n| ExpressionId::from_owned(format!("middle.{n:03}")))
        .collect();
    for reversed in [false, true] {
        let mut order = endpoint_keys.clone();
        if reversed {
            order.reverse();
        }
        let endpoint_arena = arena(function, &order);
        for text in ["", "\0", "z", "\u{10ffff}"] {
            let query = ExpressionId::from_owned(text.into());
            assert!(!hit(function, &endpoint_arena, &query, true).0);
            assert!(!hit(function, &endpoint_arena, &query, false).0);
        }
    }
}

#[test]
fn actual_suffix_byte_rates_and_work_exact_short_zero_and_sticky_refusals_are_preserved() {
    let program = program();
    let function = &program.functions[0];
    let key = ExpressionId::from_owned("λ\0🙂end".into());
    let other = ExpressionId::from_owned("λ\0🙂else".into());
    let mut budget = ReplayBudget::unobserved();
    let prefix = "λ\0🙂e".len();
    let before = budget.remaining;
    let (order, common) = compare(function, &key, &other, prefix, &mut budget).unwrap();
    assert_eq!(order, key.cmp(&other));
    assert_eq!(common, prefix);
    assert_eq!(
        before - budget.remaining,
        key.as_str().len().min(other.as_str().len()) - prefix + 1 + BOUND_UNITS
    );
    let keys: Vec<_> = (0..129)
        .map(|n| ExpressionId::from_owned(format!("shared.{}.{n:03}", "opaque\0λ".repeat(32))))
        .collect();
    let arena = arena(function, &keys);
    let query = &keys[63];
    let required = hit(function, &arena, query, false).1;
    for units in [0, required - 1, required] {
        let mut budget = ReplayBudget::unobserved();
        budget.remaining = units;
        let result = find(&arena, function, query, &mut budget);
        if units == required {
            assert!(result.unwrap().is_some());
            assert_eq!(budget.remaining, 0);
        } else {
            assert_eq!(result.err().unwrap().code, "SPX-H006");
            // Same insufficient allowance cannot become a semantic miss.
            assert!(find(&arena, function, query, &mut budget).is_err());
        }
    }
}

#[test]
fn complete_current_fact_payloads_match_original_lookup_and_active_refusal_stays_sticky() {
    let owner = include_str!("../../../expression_facts.rs");
    let source = owner
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    let source = format!("module replay.fact_owner;{source}");
    let program = crate::hir::resolve(&crate::check(&source, "prefix-facts.spx").unwrap()).unwrap();
    let mut saw_call = false;
    for function in &program.functions {
        let mut physical = physical_storage::PhysicalReservation::new(usize::MAX);
        let mut preparation = ReplayBudget::unobserved();
        let arena = super::super::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical),
            &mut preparation,
        )
        .unwrap();
        assert!(
            arena.len() > 0,
            "a complete function always contributes its body expression"
        );
        for (key, fact) in arena.entries() {
            saw_call |= fact.is_some();
            let mut old = ReplayBudget::unobserved();
            let mut new = ReplayBudget::unobserved();
            let prior = arena.find(function, key, &mut old).unwrap().unwrap();
            let current = find(&arena, function, key, &mut new).unwrap().unwrap();
            // The exact same full fact object includes callee, instance,
            // argument payloads and type arguments, with no clone or projection.
            assert!(std::ptr::eq(prior, current));
            assert!(std::ptr::eq(current, fact));
        }
        let query = ExpressionId::from_owned("missing.opaque\0λ".into());
        let (((), allocation), _, _, _) =
            crate::bounded_output::with_limit_usage_refusal(1 << 26, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    budget.remaining = 0;
                    let first = find(&arena, function, &query, &mut budget);
                    assert!(first.is_err());
                    assert!(budget.refusal_spent);
                    let repeated = find(&arena, function, &query, &mut budget);
                    assert!(repeated.is_err());
                    assert_eq!(budget.remaining, 0);
                    drop(first);
                    drop(repeated);
                    drop(budget);
                })
            });
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
    }
    assert!(saw_call);
}

#[test]
fn full_initialized_lookup_and_comparison_carriers_are_paid_exact_short_zero_and_drop() {
    let program = program();
    let function = &program.functions[0];
    let key = ExpressionId::from_owned("full.control\0λ".into());
    let arena = arena(function, std::slice::from_ref(&key));
    let (_, refused, baseline, _) =
        crate::bounded_output::with_limit_usage_refusal(1 << 26, || {
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "unchanged lookup bootstrap")
                .unwrap();
            drop(budget);
        });
    assert!(!refused);
    let exact = baseline + LOOKUP_BYTES + COMPARISON_BYTES;
    for limit in [0, exact - 1, exact] {
        let ((success, allocation), refused, debit, first) =
            crate::bounded_output::with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = find(&arena, function, &key, &mut budget);
                    let success = matches!(result, Ok(Some(_)));
                    drop(result);
                    if !success {
                        assert!(find(&arena, function, &key, &mut budget).is_err());
                    }
                    drop(budget);
                    success
                })
            });
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        if limit == exact {
            assert!(success && !refused && first.is_none());
            assert_eq!(debit, exact);
        } else {
            assert!(!success && refused && first.is_some());
        }
    }
}

#[test]
fn complete_original_arena_and_every_added_source_join_have_exact_full_inverses() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/prefix-certified-expression-lookup/exact-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        (ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/arena.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prefix-certified-expression-lookup/arena.rs.before.txt")), "94e056acbccb322afa35d42ba81af4672725b036bb4cf730c9495a8bbbb44d17"),
        (ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prefix-certified-expression-lookup/hir_module_boundaries.rs.before.txt")), "57db1e409226dc2ddf5c9519071ad69822e646d6f71d11cfd16e88d51ad8c4fc"),
        (ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prefix-certified-expression-lookup/source_custody_b.rs.before.txt")), "ead6bb549f1bd85d904eecbf182f14103639489320ec8ad09f0d29232f2fe90b"),
    ];
    assert_eq!(sources.len(), records.as_array().unwrap().len());
    for ((current, original, fixed), record) in sources.into_iter().zip(records.as_array().unwrap())
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            fixed
        );
        assert_eq!(record["original_sha256"].as_str().unwrap(), fixed);
        let mut restored = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
}
