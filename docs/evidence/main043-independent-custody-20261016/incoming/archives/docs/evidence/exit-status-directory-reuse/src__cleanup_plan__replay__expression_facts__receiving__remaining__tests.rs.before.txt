//! Complete old/current exit/finalizer outcomes; only directory funding is claimed.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
const LIMIT: usize = 16 * 1024 * 1024;
fn program() -> ResolvedProgram {
    let owner = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/expression_facts.rs"
    ));
    let source = owner
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    let source = format!("module replay.fact_owner;{source}");
    crate::hir::resolve(&crate::check(&source, "exit-receiving.spx").unwrap()).unwrap()
}
fn complete(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
) -> Result<(), Diagnostic> {
    let storage = crate::cleanup_plan::replay::validate_slots(function)?;
    let leaves = crate::cleanup_plan::replay::collect_leaves(function)?;
    if old {
        crate::cleanup_plan::replay::validate_exits(
            program,
            function,
            &storage,
            &leaves,
            &BTreeSet::new(),
        )
    } else {
        let mut budget = ReplayBudget::new();
        assert!(budget.pays_allocations());
        validate_exits(
            program,
            function,
            &storage,
            &leaves,
            &BTreeSet::new(),
            &mut budget,
        )
    }
}
fn parity(program: &ResolvedProgram, function: &ResolvedFunction) {
    let old = complete(program, function, true);
    let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            let current = complete(program, function, false);
            assert_eq!(current, old);
            drop(current);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    let ((), actual) = measure(|| {
        let saved = complete(program, function, true);
        assert_eq!(saved, old);
        drop(saved);
    });
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
}
#[test]
fn complete_exit_receivers_preserve_all_old_fields_and_first_semantic_errors() {
    let program = program();
    let mut saw_exit = false;
    let mut saw_status = false;
    for function in &program.functions {
        parity(&program, function);
        if function.cleanup_plan.exits.is_empty() {
            continue;
        }
        saw_exit = true;
        let mut attacks = Vec::new();
        let mut noncontiguous = function.clone();
        noncontiguous.cleanup_plan.exits[0].id = crate::cleanup_plan::ExitTargetId(u32::MAX);
        attacks.push(noncontiguous);
        let mut unknown_from = function.clone();
        unknown_from.cleanup_plan.exits[0].from = BlockId(u32::MAX);
        attacks.push(unknown_from);
        let mut chain = function.clone();
        chain.cleanup_plan.exits[0]
            .leaves_regions
            .push(crate::cleanup_plan::CleanupRegionId(u32::MAX));
        attacks.push(chain);
        let mut unit = function.clone();
        unit.cleanup_plan.exits[0].continuation = ExitContinuation::ReturnUnit;
        attacks.push(unit);
        if let Some(status) = function.cleanup_plan.status_sources.first() {
            saw_status = true;
            let mut duplicate = function.clone();
            duplicate.cleanup_plan.status_sources.push(status.clone());
            parity(&program, &duplicate);
            let mut missing = function.clone();
            let mut source = status.id.clone();
            source.expression = ExpressionId::from_owned("unknown.λ\0🙂status".into());
            missing.cleanup_plan.exits[0].continuation = ExitContinuation::ReturnFailure { source };
            attacks.push(missing);
        }
        for attack in attacks {
            assert!(complete(&program, &attack, true).is_err());
            parity(&program, &attack);
        }
    }
    assert!(saw_exit && saw_status);
}
#[test]
fn complete_finalizer_receivers_keep_duplicate_lifecycle_case_and_guard_priority() {
    let program = program();
    let mut saw_actions = false;
    for function in &program.functions {
        let storage = crate::cleanup_plan::replay::validate_slots(function).unwrap();
        let leaves = crate::cleanup_plan::replay::collect_leaves(function).unwrap();
        for exit in &function.cleanup_plan.exits {
            if exit.finalize_in_order.is_empty() {
                continue;
            }
            saw_actions = true;
            let first = &exit.finalize_in_order[0];
            let mut cases = vec![exit.finalize_in_order.clone()];
            let mut duplicate = exit.finalize_in_order.clone();
            duplicate.push(first.clone());
            cases.push(duplicate);
            let mut guard = exit.finalize_in_order.clone();
            guard[0].guard_flag = LivenessFlagId(u32::MAX);
            cases.push(guard);
            let mut lifecycle = exit.finalize_in_order.clone();
            lifecycle[0].lifecycle_id = DeclarationId::new("unknown.λ\0🙂lifecycle");
            cases.push(lifecycle);
            let mut path = exit.finalize_in_order.clone();
            path[0]
                .source
                .projections
                .push(DeclarationId::new("unknown.λ\0🙂projection"));
            cases.push(path);
            let mut conditional = exit.finalize_in_order.clone();
            conditional[0].active_case = Some(crate::cleanup_plan::VariantCaseGuard {
                storage: first.source.storage.clone(),
                variant: DeclarationId::new("foreign.λ\0🙂variant"),
                case: DeclarationId::new("foreign.λ\0🙂case"),
            });
            cases.push(conditional);
            for actions in cases {
                let old = crate::cleanup_plan::replay::validate_finalizer_actions(
                    function, &actions, &storage, &leaves,
                );
                let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let current = validate_finalizer_actions(
                            function,
                            &actions,
                            &storage,
                            &leaves,
                            &mut budget,
                        );
                        assert_eq!(current, old);
                        drop(current);
                        drop(budget);
                    })
                });
                assert!(!refused && first.is_none());
                assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            }
        }
    }
    assert!(saw_actions);
}
fn directories(
    function: &ResolvedFunction,
    keys: &[StatusSourceId],
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut statuses = Directory::new(function, budget, "exit status controller")?;
    let mut edges = Directory::new(function, budget, "exit continuation controller")?;
    let mut flags = Directory::new(function, budget, "finalizer guard controller")?;
    for (i, key) in keys.iter().enumerate() {
        assert!(statuses.insert(function, key, budget)?);
        assert!(edges.insert(function, EdgeId(i as u32), budget)?);
        assert!(flags.insert(function, LivenessFlagId(i as u32), budget)?);
    }
    for (i, key) in keys.iter().enumerate() {
        assert!(!statuses.insert(function, key, budget)?);
        assert!(!edges.insert(function, EdgeId(i as u32), budget)?);
        assert!(!flags.insert(function, LivenessFlagId(i as u32), budget)?);
        assert!(statuses.contains(function, &key, budget)?);
    }
    assert!(statuses.balanced() && edges.balanced() && flags.balanced());
    drop(flags);
    drop(edges);
    drop(statuses);
    Ok(())
}
#[test]
fn actual_three_directory_receivers_are_prepaid_at_exact_short_zero_sticky_through_drop() {
    let program = program();
    let function = &program.functions[0];
    let keys: Vec<_> = (0..97)
        .map(|i| StatusSourceId {
            expression: ExpressionId::from_owned(format!("exit.λ\0🙂.{i:03}")),
            lane: StatusLane::OperationFailure,
        })
        .collect();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = directories(function, &keys, &mut budget);
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = active_remaining();
                    drop(directories(function, &keys, &mut budget));
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, active_remaining());
                } else {
                    result.as_ref().unwrap();
                }
                drop(result);
                drop(budget);
            })
        })
    };
    let (((), actual), refused, paid, first) = observe(LIMIT);
    assert!(!refused && paid > 0 && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    for limit in [paid, paid - 1, 0] {
        let (((), actual), refused, used, first) = observe(limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        assert!(actual.allocated <= used && actual.highwater <= used);
        if limit == paid {
            assert!(!refused && used == paid && first.is_none());
        } else {
            assert!(refused && first.is_some());
        }
        if limit == 0 {
            assert_eq!(actual.allocated, 0);
        }
    }
}

#[test]
fn complete_exit_work_exact_short_and_sticky_use_the_same_original_budget() {
    let program = program();
    let function = program
        .functions
        .iter()
        .max_by_key(|f| f.cleanup_plan.status_sources.len())
        .unwrap();
    let storage = crate::cleanup_plan::replay::validate_slots(function).unwrap();
    let leaves = crate::cleanup_plan::replay::collect_leaves(function).unwrap();
    let (required, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        let mut budget = ReplayBudget::new();
        let initial = budget.remaining;
        validate_exits(
            &program,
            function,
            &storage,
            &leaves,
            &BTreeSet::new(),
            &mut budget,
        )
        .unwrap();
        initial - budget.remaining
    });
    assert!(!refused && required > 0 && first.is_none());
    for work in [required, required - 1, 0] {
        let (((), actual), _, _, _) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget.remaining = work;
                let result = validate_exits(
                    &program,
                    function,
                    &storage,
                    &leaves,
                    &BTreeSet::new(),
                    &mut budget,
                );
                if work == required {
                    assert!(result.is_ok());
                    assert_eq!(budget.remaining, 0);
                } else {
                    assert!(result.is_err() && budget.refusal_spent);
                    let remaining = budget.remaining;
                    let retry = validate_exits(
                        &program,
                        function,
                        &storage,
                        &leaves,
                        &BTreeSet::new(),
                        &mut budget,
                    );
                    assert!(retry.is_err());
                    assert_eq!(budget.remaining, remaining);
                    drop(retry);
                }
                drop(result);
                drop(budget);
            })
        });
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    }
}

#[test]
fn complete_root_receivers_and_only_named_paid_caller_projections_keep_original_bytes() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(include_bytes!("selected.rs")))
        ),
        "446459444ef20047d571618c4c85c4cb1d8151f52bb1b587221c795bff0b52b9"
    );
    let inverse: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/borrowed-selected-status-816/inverse.json"
    )))
    .unwrap();
    let mut current = include_str!("../remaining.rs").to_owned();
    for edit in inverse["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(current.matches(after).count(), 1);
        current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/borrowed-selected-status-816/remaining.before.txt"
        ))
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "78f39b761c68fc9145736540359659eb7edd7a80253fcaec1716f7ce55fcc85e"
    );
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/remaining-receiving-exits-816/exact-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/remaining-receiving-exits-816/src__cleanup_plan__replay.rs.before.txt"))),
        (include_str!("../../receiving.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/remaining-receiving-exits-816/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/remaining-receiving-exits-816/src__cleanup_plan__replay__lexical_replay.rs.before.txt"))),
    ];
    assert_eq!(records.as_array().unwrap().len(), sources.len());
    for (record, (current, original)) in records.as_array().unwrap().iter().zip(sources) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let mut current = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(current.matches(after).count(), 1);
            current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    let projections: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/remaining-receiving-exits-816/receiver-projections.json"
    )))
    .unwrap();
    let originals = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/remaining-receiving-exits-816/exit.before.txt"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/remaining-receiving-exits-816/finalizers.before.txt"
        )),
    ];
    assert_eq!(projections.as_array().unwrap().len(), originals.len());
    for (record, original) in projections.as_array().unwrap().iter().zip(originals) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let after = record["after"].as_str().unwrap();
        assert_eq!(current.matches(after).count(), 1);
        assert_eq!(record["before"].as_str().unwrap(), original);
        assert_eq!(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay.rs"
            ))
            .matches(original)
            .count(),
            1
        );
    }
}
