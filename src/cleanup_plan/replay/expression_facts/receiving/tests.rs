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

// Live lexical receiver controls: every full-caller comparison enters the
// ordinary driver and retains its later certificate/CFG/path/factored checks.
fn live_lexical_program(calls: usize) -> ResolvedProgram {
    let calls = "leaf(n);".repeat(calls);
    let source = format!(r#"module replay.live_directories;
@id("live.leaf") fn leaf(n:i64)->i64 {{n}}
@id("live.walk") fn walk()->i64 {{
 let mut n=0;
 while n<4 {{ n=n+1; {calls} if n==1 {{continue;}}; if n==3 {{break;}}; 0 }}
 if n==2 {{return n;}}; return 9;
}}
@id("app.main") fn main()->i64 {{walk()}}
"#);
    crate::hir::resolve(&crate::check(&source,"live-directory.spx").unwrap()).unwrap()
}
struct LiveBefore(bool);
impl Drop for LiveBefore {fn drop(&mut self){LEXICAL_INSERT_BEFORE.set(self.0);}}
fn live_before(before: bool) -> LiveBefore {LiveBefore(LEXICAL_INSERT_BEFORE.replace(before))}
fn live_full_result(program:&ResolvedProgram,function:&ResolvedFunction,before:bool)->Result<(),Diagnostic>{
    let _before=live_before(before);
    crate::cleanup_plan::replay::validate_structure(program,function)
}
fn live_full_probe(program:&ResolvedProgram,function:&ResolvedFunction,before:bool,limit:usize)->(bool,usize,usize,usize,bool){
    let expected=live_full_result(program,function,true);
    let ((out,allocation),refused,debit,first)=with_limit_usage_refusal(limit,||measure(||{
        let _before=live_before(before);let mut budget=ReplayBudget::new();
        assert!(!budget.selected()&&budget.pays_allocations());
        let result=crate::cleanup_plan::replay::validate_structure_with_budget(program,function,&mut budget,None);
        if limit==usize::MAX {assert_eq!(result,expected,"complete ordinary lexical verdict/Diagnostic");}
        let out=(result.is_ok(),MAX_REPLAY_WORK_UNITS-budget.remaining);
        drop(result);drop(budget);out
    }));
    assert!(!allocation.invalid&&allocation.live==0);assert_eq!(allocation.allocated,allocation.freed);
    assert_eq!(refused,first.is_some());if limit==usize::MAX&&out.0{assert!(allocation.allocated<=debit,"whole successful caller actual allocations must be funded");}(out.0,out.1,allocation.allocated,debit,refused)
}
#[test]
fn live_lexical_directories_full_old_current_callers_keep_late_checks_and_real_net(){
    let mut saved=false;
    for calls in [0,1,8,64]{
        let p=live_lexical_program(calls);let f=p.functions.iter().find(|f|f.cleanup_plan.lexical_control.is_some()).unwrap();
        assert!(crate::cleanup_plan::replay::lexical_cert::requires_lexical(f));
        let old=live_full_probe(&p,f,true,usize::MAX);let new=live_full_probe(&p,f,false,usize::MAX);
        assert!(old.0&&new.0&&!old.4&&!new.4);
        if calls>=8 {assert!(new.1<old.1&&new.2<old.2&&new.3<old.3);saved=true;}
        eprintln!("LIVE944945 calls={calls} old={old:?} current={new:?}");
    }
    assert!(saved);assert!(!LEXICAL_INSERT_BEFORE.get());
}
#[test]
fn live_lexical_directories_actual_late_hostiles_keep_all_diagnostic_fields(){
    let p=live_lexical_program(8);let i=p.functions.iter().position(|f|f.cleanup_plan.lexical_control.is_some()).unwrap();
    for attack in 0..7 {
        let mut hostile=p.clone();let f=&mut hostile.functions[i];let mut changed=false;
        match attack {
            0=>{f.cleanup_plan.lexical_control.as_mut().unwrap().schema="foreign";changed=true;}
            1=>{f.cleanup_plan.exits[0].id=crate::cleanup_plan::ExitTargetId(u32::MAX);changed=true;}
            2=>{for exit in &mut f.cleanup_plan.exits {if let ExitContinuation::FunctionReturn {source:CleanupResultSource::Scalar {expression},..}=&mut exit.continuation {*expression=ExpressionId::from_owned("foreign.return".into());changed=true;break;}}}
            3|4|5=>{for block in &mut f.cleanup_plan.blocks {if let Some(at)=block.transitions.iter().position(|x|matches!(x,CleanupTransition::CallCommit{..})){match attack{3=>{block.transitions.remove(at);},4=>{let duplicate=block.transitions[at].clone();block.transitions.insert(at,duplicate);},_=>{if let CleanupTransition::CallCommit{call,..}=&mut block.transitions[at]{*call=ExpressionId::from_owned("foreign.call".into());}}}changed=true;break;}}}
            _=>{f.cleanup_plan.blocks[0].id=BlockId(u32::MAX);changed=true;}
        }
        assert!(changed,"authored fixture must reach attack {attack}");let f=&hostile.functions[i];
        let old=live_full_result(&hostile,f,true);let new=live_full_result(&hostile,f,false);
        assert!(old.is_err(),"attack {attack}");assert_eq!(new,old,"all six Diagnostic fields attack {attack}");
        let a=live_full_probe(&hostile,f,true,usize::MAX);let b=live_full_probe(&hostile,f,false,usize::MAX);assert!(!a.0&&!b.0&&!a.4&&!b.4);
    }
}
#[test]
fn live_lexical_directories_full_caller_exact_short_zero_keep_complete_drop(){
    let p=live_lexical_program(8);let f=p.functions.iter().find(|f|f.cleanup_plan.lexical_control.is_some()).unwrap();
    for before in [true,false]{
        let full=live_full_probe(&p,f,before,usize::MAX);assert!(full.0&&full.3>0);
        let exact=live_full_probe(&p,f,before,full.3);assert!(exact.0&&!exact.4);assert_eq!(exact.3,full.3);
        for limit in [0,full.3-1]{let short=live_full_probe(&p,f,before,limit);assert!(!short.0&&short.4);assert!(short.3<=limit);}
    }
}
#[test]
fn live_lexical_selector_none_constructor_tax_and_first_gate_remain_exact(){
    let p=program();let f=&p.functions[0];assert!(f.cleanup_plan.lexical_control.is_none());
    let run=|current:bool,limit|with_limit_usage_refusal(limit,||measure(||{
        let mut budget=ReplayBudget::new();let result=if current{Directory::<u32>::new_with_lexical_insert_selector(f,&mut budget,"live selector")}else{Directory::<u32>::new(f,&mut budget,"live selector")};
        let success=result.is_ok();let work=MAX_REPLAY_WORK_UNITS-budget.remaining;drop(result);drop(budget);(success,work)
    }));
    let old=run(false,usize::MAX);let new=run(true,usize::MAX);
    assert!(old.0.0.0&&new.0.0.0);assert!(!old.1&&!new.1);assert_eq!(old.0.1.allocated,new.0.1.allocated);
    assert_eq!(new.2-old.2,super::directory::LEXICAL_INSERT_SELECTOR_BYTES);assert_eq!(new.0.0.1-old.0.0.1,1);
    assert_eq!(super::directory::LEXICAL_INSERT_SELECTOR_BYTES,2*std::mem::size_of::<&Option<Box<crate::cleanup_plan::LexicalControlPlan>>>()+3*std::mem::size_of::<bool>());
    if std::mem::size_of::<usize>()==8{assert_eq!(super::directory::LEXICAL_INSERT_SELECTOR_BYTES,19);}
    for current in [false,true]{let full=run(current,usize::MAX);let exact=run(current,full.2);assert!(exact.0.0.0&&!exact.1);let zero=run(current,0);assert!(!zero.0.0.0&&zero.1);let bad=run(current,full.2-1);assert!(!bad.0.0.0&&bad.1);let positive=super::directory::LEXICAL_INSERT_CONSTRUCTOR_BASE_BYTES+if current{super::directory::LEXICAL_INSERT_SELECTOR_BYTES}else{0};assert_eq!(bad.3.unwrap().requested,positive);assert_eq!(bad.2,full.2-positive);}
    // Work is an explicit finite input, never granted or reset after a call.
    let work_probe=|current,work|{
        let mut budget=ReplayBudget {remaining:work,..ReplayBudget::unobserved()};
        let _=crate::cleanup_plan::replay::take_replay_work_refusal();
        let result=if current{Directory::<u32>::new_with_lexical_insert_selector(f,&mut budget,"live selector")}else{Directory::<u32>::new(f,&mut budget,"live selector")};
        let success=result.is_ok();let first=result.err();let remaining=budget.remaining;
        let refusal=crate::cleanup_plan::replay::take_replay_work_refusal().map(|r|(r.requested,r.remaining));
        let retry=if success{None}else if current{Directory::<u32>::new_with_lexical_insert_selector(f,&mut budget,"live selector").err()}else{Directory::<u32>::new(f,&mut budget,"live selector").err()};
        (success,first,remaining,retry,refusal)
    };
    assert_eq!(work_probe(false,1),(true,None,0,None,None));
    assert_eq!(work_probe(true,2),(true,None,0,None,None));
    let old_short=work_probe(false,0);assert!(!old_short.0&&old_short.2==0);assert_eq!(old_short.4,Some((1,0)));
    for work in [0,1]{let new_short=work_probe(true,work);assert!(!new_short.0&&new_short.2==0);assert_eq!(new_short.4,Some((2,work)));assert_eq!(new_short.1,old_short.1,"all original first-work Diagnostic fields");assert_eq!(new_short.3,old_short.3,"all original sticky-work Diagnostic fields");}
}
#[test]
fn live_lexical_selector_floor_foreign_sticky_and_unwind_use_real_gateway(){
    use crate::bounded_output::{active_remaining,clear_active_floor,set_active_floor};
    let p=program();let f=&p.functions[0];
    let (((),allocation),_,_,_)=with_limit_usage_refusal(1<<24,||measure(||{
        let mut budget=ReplayBudget::new();budget.prepare_builder(f).unwrap();
        let remaining=active_remaining().unwrap();assert!(remaining>0&&set_active_floor(remaining));
        let first=Directory::<u32>::new_with_lexical_insert_selector(f,&mut budget,"live selector");clear_active_floor();
        assert!(first.is_err());let retry=Directory::<u32>::new_with_lexical_insert_selector(f,&mut budget,"live selector");assert!(retry.is_err());assert_eq!(budget.remaining,0);drop(retry);drop(first);drop(budget);
    }));assert!(!allocation.invalid&&allocation.live==0);assert_eq!(allocation.allocated,allocation.freed);
    // An expired foreign Oracle lease must not fall back to production ACTIVE.
    let stale=crate::cleanup_plan::replay::allocation::OracleScope::open(1<<20).unwrap().lease();
    let (raw,refused,debit,first)=with_limit_usage_refusal(1<<24,||{
        let mut old=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,stale);let mut new=ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS,stale);
        let a=Directory::<u32>::new(f,&mut old,"live selector").err().unwrap();let b=Directory::<u32>::new_with_lexical_insert_selector(f,&mut new,"live selector").err().unwrap();assert_eq!(a,b);(a,b)
    });assert!(!refused&&debit==0&&first.is_none());drop(raw);
    let ((panicked,allocation),refused,_,_)=with_limit_usage_refusal(1<<24,||measure(||std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{
        let mut budget=ReplayBudget::new();let mut directory=Directory::<u32>::new_with_lexical_insert_selector(f,&mut budget,"live unwind").unwrap();for k in 0..16u32{directory.insert(f,k,&mut budget).unwrap();}panic!("intentional complete live directory unwind");
    })).is_err()));assert!(panicked&&!refused&&!allocation.invalid&&allocation.live==0);assert_eq!(allocation.allocated,allocation.freed);
}
#[test]
fn live_lexical_directories_whole_old_sources_and_dead_retained_withdrawal_are_exact(){
    use sha2::{Digest,Sha256};let hash=|s:&str|format!("{:x}",crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes())));
    let source=include_str!("../receiving.rs");let marker="\n// Whole immediate receiver before the live lexical insertion fork.\n#[cfg(test)]\nmod before_lexical_inserts {\n    use super::*;\n";
    let (prefix,tail)=source.split_once(marker).unwrap();let (old,rest)=tail.split_once("}\n#[cfg(test)]\nthread_local! {").unwrap();assert!(rest.contains("LEXICAL_INSERT_BEFORE"));
    let start=prefix.find("pub(in crate::cleanup_plan::replay) fn validate_blocks_and_edges(").unwrap();let end=prefix[start..].find("\nfn validate_edge_condition(").unwrap()+start;
    let restored=format!("{}{}{}",&prefix[..start],old,&prefix[end..]);
    assert_eq!(hash(&restored),"745d75656478f76d31b615ffd2c515c8e36c8f8da522764194c04c98fff92c4e");assert_eq!(hash(old),"fc743c6a947e1fd4085064c072b36269f99eec6621594f65e654d612c470554f");
    let driver=include_str!("../../lexical_replay/ordinary.rs");assert_eq!(hash(driver),"d947a67e20af65598c445eecf30c19a797793e2ea45d9ca1d096976fa1111927");assert_eq!(driver,include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/e/d947a67e20af65598c445eecf30c19a797793e2ea45d9ca1d096976fa1111927")));
    assert_eq!(hash(include_str!("retained.rs")),"d35fedf3a3d6ad0d4e3878eac55eccdca5cfc4653705ec7bb726f69ef93cfc00");
    assert_eq!(hash(include_str!("retained/tests.rs")),"2d11e36011a6ff396fb6d1d2b4c7c31bf2298d696979bb2fa5a13ee2043818a5");
    let tests=include_str!("tests.rs");assert_eq!(hash(&tests[..28833]),"c835954bd5ab1f2c1581e61e20fe7067164039a14790425183758fddbb933855");
}
