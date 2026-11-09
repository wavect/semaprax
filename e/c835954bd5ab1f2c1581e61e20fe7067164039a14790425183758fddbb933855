//! Full old/current receiver and literal paid-directory controls.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let owner = include_str!("../../expression_facts.rs");
    let source = owner
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    let source = format!("module replay.fact_owner;{source}");
    crate::hir::resolve(&crate::check(&source, "replay-fact-owner.spx").unwrap()).unwrap()
}

#[test]
fn indexed_directories_keep_full_opaque_keys_sorted_rotations_duplicates_hits_and_misses() {
    let program = program();
    let function = &program.functions[0];
    let keys: Vec<_> = (0..257)
        .map(|n| {
            ExpressionId::from_owned(format!("λ.識別子.{}.{n:03}\0whole", "prefix".repeat(17)))
        })
        .collect();
    for reverse in [false, true] {
        let mut budget = ReplayBudget::unobserved();
        let mut directory =
            Directory::new(function, &mut budget, "receiving full-key control").unwrap();
        let mut original = BTreeSet::new();
        for index in 0..keys.len() {
            let index = if reverse {
                keys.len() - index - 1
            } else {
                index
            };
            assert_eq!(
                original.insert(&keys[index]),
                directory
                    .insert(function, &keys[index], &mut budget)
                    .unwrap()
            );
            assert!(directory.balanced());
            assert_eq!(
                directory.keys().copied().collect::<Vec<_>>(),
                original.iter().copied().collect::<Vec<_>>()
            );
        }
        let capacity = directory.capacity();
        let duplicate = &keys[137];
        assert!(!directory.insert(function, duplicate, &mut budget).unwrap());
        assert_eq!(capacity, directory.capacity());
        let missing = ExpressionId::from_owned("λ.識別子.missing\0full".into());
        for key in [duplicate, &missing] {
            assert_eq!(
                original.contains(&key),
                directory.contains(function, &key, &mut budget).unwrap()
            );
        }
        assert!(directory.balanced());
    }
}

fn complete(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    old: bool,
) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::arena::build::<errors::Legacy>(
        function,
        capacity::Payer::Selected(
            &mut crate::cleanup_plan::replay::physical_storage::PhysicalReservation::new(
                usize::MAX,
            ),
        ),
        &mut budget,
    )?);
    let storage = validate_slots(function)?;
    let leaves = collect_leaves(function)?;
    if old {
        super::super::receiving_before::validate_status_sources(function, &facts, &mut budget)?;
        super::super::receiving_before::validate_blocks_and_edges(
            program,
            function,
            &facts,
            &storage,
            &leaves,
            None,
            &mut budget,
        )
    } else {
        validate_status_sources(function, &facts, &mut budget)?;
        validate_blocks_and_edges(
            program,
            function,
            &facts,
            &storage,
            &leaves,
            None,
            &mut budget,
        )
    }
}

#[test]
fn full_compiled_btree_receiver_keeps_every_field_and_first_diagnostic_against_current() {
    let program = program();
    let mut saw_status = false;
    let mut saw_edge = false;
    let mut saw_commit = false;
    for original in &program.functions {
        assert_eq!(
            complete(&program, original, true),
            complete(&program, original, false)
        );
        let mut attacks = Vec::new();
        if let Some(source) = original.cleanup_plan.status_sources.first() {
            saw_status = true;
            let mut duplicate = original.clone();
            duplicate.cleanup_plan.status_sources.push(source.clone());
            let mut later = source.clone();
            later.id.expression = ExpressionId::from_owned("missing.later.status\0opaque".into());
            duplicate.cleanup_plan.status_sources.push(later);
            attacks.push(duplicate);
            let mut wrong = original.clone();
            wrong.cleanup_plan.status_sources[0].id.expression =
                ExpressionId::from_owned("missing.first.status\0opaque".into());
            attacks.push(wrong);
        }
        if !original.cleanup_plan.edges.is_empty() {
            saw_edge = true;
            let mut wrong = original.clone();
            wrong.cleanup_plan.edges[0].id = EdgeId(u32::MAX);
            attacks.push(wrong);
        }
        for (block_index, block) in original.cleanup_plan.blocks.iter().enumerate() {
            if let Some(position) = block
                .transitions
                .iter()
                .position(|t| matches!(t, CleanupTransition::CallCommit { .. }))
            {
                saw_commit = true;
                let mut repeated = original.clone();
                let copy = repeated.cleanup_plan.blocks[block_index].transitions[position].clone();
                repeated.cleanup_plan.blocks[block_index]
                    .transitions
                    .push(copy);
                if original.cleanup_plan.lexical_control.is_none() {
                    attacks.push(repeated);
                } else {
                    // The receiver intentionally delegates per-domain census to
                    // the unchanged typed skeleton for lexical control plans.
                    assert_eq!(
                        complete(&program, &repeated, true),
                        complete(&program, &repeated, false)
                    );
                }
                let mut unknown = original.clone();
                let CleanupTransition::CallCommit { call, .. } =
                    &mut unknown.cleanup_plan.blocks[block_index].transitions[position]
                else {
                    unreachable!();
                };
                *call = ExpressionId::from_owned("missing.call.commit\0full".into());
                attacks.push(unknown);
            }
        }
        for hostile in attacks {
            let old = complete(&program, &hostile, true);
            let current = complete(&program, &hostile, false);
            assert!(old.is_err());
            assert_eq!(old, current);
        }
    }
    assert!(saw_status && saw_edge && saw_commit);
}

fn directory_run(
    function: &ResolvedFunction,
    keys: &[ExpressionId],
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut directory = Directory::new(function, budget, "receiving literal paid directory")?;
    for key in keys {
        directory.insert(function, key, budget)?;
    }
    assert!(directory.balanced());
    let capacity = directory.capacity();
    for key in keys {
        assert!(directory.contains(function, &key, budget)?);
        assert!(!directory.insert(function, key, budget)?);
    }
    assert_eq!(capacity, directory.capacity());
    assert!(directory.same_keys(&directory, function, budget)?);
    Ok(())
}

#[test]
fn every_directory_acquisition_exact_short_zero_and_sticky_is_paid_through_drop() {
    let program = program();
    let function = &program.functions[0];
    let keys: Vec<_> = (0..97)
        .map(|n| ExpressionId::from_owned(format!("paid.識別子.{n:03}\0full")))
        .collect();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = directory_run(function, &keys, &mut budget);
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = active_remaining();
                    drop(directory_run(function, &keys, &mut budget));
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
    let (((), wide), refused, required, _) = observe(usize::MAX);
    assert!(!refused && required > 0 && !wide.invalid && wide.live == 0);
    assert_eq!(wide.allocated, wide.freed);
    assert!(wide.allocated <= required && wide.highwater <= required);
    for limit in [required, required - 1, 0] {
        let (((), actual), refused, used, first) = observe(limit);
        assert!(!actual.invalid && actual.live == 0);
        assert_eq!(actual.allocated, actual.freed);
        assert!(actual.allocated <= used && actual.highwater <= used);
        if limit == required {
            assert!(!refused && used == required && first.is_none());
        } else {
            assert!(refused && first.is_some());
        }
        if limit == 0 {
            assert_eq!(actual.allocated, 0);
        }
    }
}

#[test]
fn full_key_search_insert_relocation_rotation_and_iteration_keep_same_work_payer() {
    let program = program();
    let function = &program.functions[0];
    let keys: Vec<_> = (0..97)
        .map(|n| ExpressionId::from_owned(format!("work.full.{}.{n:03}\0", "λ識".repeat(17))))
        .collect();
    // Measure the same paid growth route used below. The unobserved receiver
    // leaves allocation to Vec's ordinary growth, which can relocate at
    // different lengths and therefore has a different work debit.
    let (required, refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let initial = budget.remaining;
        directory_run(function, &keys, &mut budget).unwrap();
        initial - budget.remaining
    });
    assert!(!refused);
    assert!(required > keys.len());
    for available in [required, required - 1] {
        let (((), metrics), _, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget.remaining = available;
                let result = directory_run(function, &keys, &mut budget);
                if available == required {
                    result.as_ref().unwrap();
                    assert_eq!(budget.remaining, 0);
                } else {
                    assert!(result
                        .as_ref()
                        .unwrap_err()
                        .message
                        .contains("cleanup replay work budget exhausted"));
                    assert!(budget.refusal_spent);
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = budget.remaining;
                    drop(directory_run(function, &keys, &mut budget));
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, budget.remaining);
                }
                drop(result);
                drop(budget);
            })
        });
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    }
}

#[test]
fn status_lane_and_copied_numeric_keys_keep_complete_original_order_and_disjointness() {
    let program = program();
    let function = &program.functions[0];
    let expression = ExpressionId::from_owned("same.expression.λ\0opaque".into());
    let keys = [
        StatusSourceId {
            expression: expression.clone(),
            lane: StatusLane::OperationFailure,
        },
        StatusSourceId {
            expression,
            lane: StatusLane::OwnerAdmission,
        },
    ];
    let mut budget = ReplayBudget::unobserved();
    let mut status = Directory::new(function, &mut budget, "receiving lane control").unwrap();
    let mut old = BTreeSet::new();
    for key in &keys {
        assert_eq!(
            old.insert(key),
            status.insert(function, key, &mut budget).unwrap()
        );
    }
    assert_eq!(
        old.iter().copied().collect::<Vec<_>>(),
        status.keys().copied().collect::<Vec<_>>()
    );
    assert_eq!(status.len(), 2);
    let mut left = Directory::new(function, &mut budget, "receiving numeric left").unwrap();
    let mut right = Directory::new(function, &mut budget, "receiving numeric right").unwrap();
    let mut old_left = BTreeSet::new();
    let mut old_right = BTreeSet::new();
    for key in [
        LivenessFlagId(7),
        LivenessFlagId(0),
        LivenessFlagId(u32::MAX),
        LivenessFlagId(7),
    ] {
        assert_eq!(
            old_left.insert(key),
            left.insert(function, key, &mut budget).unwrap()
        );
    }
    for key in [LivenessFlagId(1), LivenessFlagId(8)] {
        assert_eq!(
            old_right.insert(key),
            right.insert(function, key, &mut budget).unwrap()
        );
    }
    assert_eq!(
        old_left.is_disjoint(&old_right),
        left.is_disjoint(&right, function, &mut budget).unwrap()
    );
    right
        .insert(function, LivenessFlagId(7), &mut budget)
        .unwrap();
    old_right.insert(LivenessFlagId(7));
    assert_eq!(
        old_left.is_disjoint(&old_right),
        left.is_disjoint(&right, function, &mut budget).unwrap()
    );
}

#[test]
fn complete_current_and_archived_receiving_boundaries_retain_all_raw_costs_through_drop() {
    let program = program();
    for function in &program.functions {
        let storage = validate_slots(function).unwrap();
        let leaves = collect_leaves(function).unwrap();
        for old in [true, false] {
            let observe = |limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let result = (|| {
                            // Shared unchanged paid producer builds all exact immutable input facts.
                            // The receiving body is the full independent old or actual current route.
                            let facts = super::super::with_budget(function, &mut budget)?;
                            if old {
                                super::super::receiving_before::validate_status_sources(
                                    function,
                                    &facts,
                                    &mut budget,
                                )?;
                                super::super::receiving_before::validate_blocks_and_edges(
                                    &program,
                                    function,
                                    &facts,
                                    &storage,
                                    &leaves,
                                    None,
                                    &mut budget,
                                )
                            } else {
                                validate_status_sources(function, &facts, &mut budget)?;
                                validate_blocks_and_edges(
                                    &program,
                                    function,
                                    &facts,
                                    &storage,
                                    &leaves,
                                    None,
                                    &mut budget,
                                )
                            }
                        })();
                        if crate::bounded_output::active_refused() {
                            assert!(result.is_err());
                        } else {
                            result.as_ref().unwrap();
                        }
                        drop(result);
                        drop(budget);
                    })
                })
            };
            let (((), wide), refused, required, _) = observe(usize::MAX);
            assert!(!refused && required > 0 && !wide.invalid && wide.live == 0);
            assert_eq!(wide.allocated, wide.freed);
            let (((), exact), refused, used, _) = observe(required);
            assert!(!refused && used == required && !exact.invalid && exact.live == 0);
            assert_eq!(exact.allocated, exact.freed);
            let (((), short), refused, _, first) = observe(required - 1);
            assert!(refused && first.is_some() && !short.invalid && short.live == 0);
            assert_eq!(short.allocated, short.freed);
            let (((), zero), refused, _, first) = observe(0);
            assert!(refused && first.is_some() && !zero.invalid && zero.live == 0);
            assert_eq!(zero.allocated, 0);
            // Neither the saved unpriced sets nor unrelated retained helper residuals
            // receive a discount or a new whole-program funding claim.
            eprintln!(
                "whole receiving {} old={old} physical={wide:?} paid={required}",
                function.id
            );
        }
    }
}

#[test]
fn full_original_receiving_and_helpers_are_byte_exact_and_all_current_hooks_bound() {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-receiving-directories/source-provenance.json"
    )))
    .unwrap();
    let sources = [
        (
            "src/cleanup_plan/replay/expression_facts/receiving.rs",
            ci_str!("../receiving.rs"),
            ci_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paid-receiving-directories/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt"
            )),
            "44b46c6954724d9fb9bc90dfbcb0a77e86bbf55998fdbe1ed3973393bc6744b7",
        ),
        (
            "src/cleanup_plan/replay/expression_facts.rs",
            ci_str!("../../expression_facts.rs"),
            ci_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paid-receiving-directories/src__cleanup_plan__replay__expression_facts.rs.before.txt"
            )),
            "bd26157302dd48bc3df7b8635ac9d277a64978a23e2c438a0865dfdd88ccd2ea",
        ),
    ];
    for (path, current, archived, frozen_sha) in sources {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archived.as_bytes()))
            ),
            frozen_sha
        );
        assert_eq!(
            proof["original_sources"][path]["sha256"].as_str().unwrap(),
            frozen_sha
        );
        let mut projected = current.to_owned();
        for hook in proof["full_current_projection"][path].as_array().unwrap() {
            let from = hook["from"].as_str().unwrap();
            let to = hook["to"].as_str().unwrap();
            assert_eq!(projected.matches(from).count(), 1, "{}", hook["name"]);
            projected = projected.replace(from, to);
        }
        if path == "src/cleanup_plan/replay/expression_facts.rs" {
            projected = before_expression_fact_observer(&projected);
        }
        assert_eq!(projected, archived);
    }
    assert_eq!(
        ci_str!("../receiving_before.rs"),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paid-receiving-directories/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt"
        ))
    );
    fn item<'a>(source: &'a str, name: &str) -> &'a str {
        let start = source.find(name).unwrap();
        let tail = &source[start..];
        &tail[..tail.find("\n}\n").unwrap() + 3]
    }
    let current = ci_str!("../receiving.rs");
    let original_replay = ci_str!("../../../replay.rs");
    let edge = item(current, "fn validate_owned_edge_paid(")
        .replace("fn validate_owned_edge_paid(", "fn validate_owned_edge(")
        .replace(
            "referenced: &mut Directory<EdgeId>,\n    budget: &mut ReplayBudget,",
            "referenced: &mut BTreeSet<EdgeId>,",
        )
        .replace(
            "!referenced.insert(function, edge, budget)?",
            "!referenced.insert(edge)",
        );
    assert_eq!(edge, item(original_replay, "fn validate_owned_edge("));
    let status = item(current, "fn require_status_paid(")
        .replace("fn require_status_paid(", "fn require_status(")
        .replace(
            "statuses: &Directory<&StatusSourceId>,",
            "statuses: &BTreeSet<StatusSourceId>,",
        )
        .replace(
            "    source: &StatusSourceId,\n    budget: &mut ReplayBudget,",
            "    source: &StatusSourceId,",
        )
        .replace(
            "statuses.contains(function, &source, budget)?",
            "statuses.contains(source)",
        );
    assert_eq!(status, item(original_replay, "fn require_status("));
}

#[test]
fn post_link_work_refusal_drops_partial_avl_and_cannot_reuse_its_paid_owner() {
    let program = program();
    let function = &program.functions[0];
    let mut saw_post_link_refusal = false;
    for available in 0..128 {
        let (((), metrics), _, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut directory =
                    Directory::new(function, &mut budget, "receiving post-link control").unwrap();
                budget.remaining = available;
                for key in 0..7u32 {
                    let before_len = directory.len();
                    let result = directory.insert(function, key, &mut budget);
                    if result.is_err() {
                        saw_post_link_refusal |= directory.len() > before_len;
                        assert!(budget.refusal_spent);
                        let before = crate::test_allocation_observer::current_metrics();
                        let remaining = budget.remaining;
                        drop(directory.insert(function, key, &mut budget));
                        drop(directory.contains(function, &key, &mut budget));
                        let after = crate::test_allocation_observer::current_metrics();
                        assert_eq!(
                            (before.allocations, before.allocated),
                            (after.allocations, after.allocated)
                        );
                        assert_eq!(remaining, budget.remaining);
                        drop(result);
                        break;
                    }
                }
                drop(directory);
                drop(budget);
            })
        });
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    }
    assert!(saw_post_link_refusal);
}

fn complete_original_fixture(owner: &str, file: &str) -> ResolvedProgram {
    let source = owner
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    crate::hir::resolve(&crate::parse(source, file).unwrap()).unwrap()
}

#[test]
fn full_original_corpora_keep_arithmetic_cases_and_transfer_failure_priority() {
    let programs = [
        complete_original_fixture(
            include_str!("../../../replay_tests.rs"),
            "replay-original-receiving.spx",
        ),
        complete_original_fixture(
            include_str!("../../../nested_record_destructure_tests.rs"),
            "nested-original-receiving.spx",
        ),
    ];
    let mut saw_cases = false;
    let mut saw_transfer = false;
    for program in &programs {
        for original in &program.functions {
            assert_eq!(
                complete(program, original, true),
                complete(program, original, false)
            );
            for (index, source) in original.cleanup_plan.status_sources.iter().enumerate() {
                if let StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } = &source.producer
                {
                    assert!(!normalized_cases.is_empty());
                    saw_cases = true;
                    for empty in [true, false] {
                        let mut hostile = original.clone();
                        let StatusProducer::CheckedArithmetic {
                            normalized_cases, ..
                        } = &mut hostile.cleanup_plan.status_sources[index].producer
                        else {
                            unreachable!()
                        };
                        if empty {
                            normalized_cases.clear();
                        } else {
                            normalized_cases.push(normalized_cases[0]);
                        }
                        let old = complete(program, &hostile, true);
                        assert!(old.is_err());
                        assert_eq!(old, complete(program, &hostile, false));
                    }
                }
            }
            for (block, item) in original.cleanup_plan.blocks.iter().enumerate() {
                for (index, transition) in item.transitions.iter().enumerate() {
                    if matches!(transition, CleanupTransition::Transfer { .. }) {
                        saw_transfer = true;
                        let mut hostile = original.clone();
                        let CleanupTransition::Transfer {
                            source,
                            destination,
                            ..
                        } = &mut hostile.cleanup_plan.blocks[block].transitions[index]
                        else {
                            unreachable!()
                        };
                        *destination = source.clone();
                        let old = complete(program, &hostile, true);
                        assert!(old.is_err());
                        assert_eq!(old, complete(program, &hostile, false));
                    }
                }
            }
        }
    }
    assert!(saw_cases && saw_transfer);
}

// Profiling adds observation scopes around the original receiver. Authenticate
// this complete projected generation before inverting only those exact scopes.
fn before_expression_fact_observer(current: &str) -> String {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "d5a01217bb7550905972a7ab06980bfd4580e3e361c8c50e33cf3ff39eed98f6"
    );
    let observer = "    #[cfg(test)]\n    let _operation = crate::bounded_output::stage_debits::operation_guard(\n        crate::bounded_output::stage_debits::Operation::ReplayExpressionFacts,\n    );\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n        crate::bounded_output::exclusive_debits::Operation::ReplayExpressionFacts,\n    );\n";
    assert_eq!(current.matches(observer).count(), 1);
    let restored = current.replacen(observer, "", 1);
    assert_eq!(restored, include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-receiving-directories/src__cleanup_plan__replay__expression_facts.rs.before.txt")));
    restored
}

#[test]
fn expression_fact_observer_inverse_refuses_whole_source_drift_and_old_inputs() {
    let current = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/expression-fact-observer-source-v1/projected-current.txt"
    ));
    let original = before_expression_fact_observer(current);
    for foreign in [
        String::new(),
        current[..current.len() - 1].to_owned(),
        format!("{current}\n"),
        current.replacen("ReplayExpressionFacts", "ReplayPathStates", 1),
        original,
    ] {
        assert!(std::panic::catch_unwind(|| before_expression_fact_observer(&foreign)).is_err());
    }
}
