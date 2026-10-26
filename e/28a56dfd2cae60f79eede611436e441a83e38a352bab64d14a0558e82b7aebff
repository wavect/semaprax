use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = include_str!("../../../expression_facts.rs")
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    let source = format!("module replay.fact_owner;{source}");
    crate::hir::resolve(&crate::check(&source, "retained-status.spx").unwrap()).unwrap()
}

// Inputs/facts are outside this phase observer. Both full receivers, their paid
// directories, result diagnostics, budget and every retained allocation drop inside.
fn run(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    retained: bool,
) -> (Result<(), Diagnostic>, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let mut reference_budget = ReplayBudget::unobserved();
    let expected = super::super::validate_status_sources(function, &facts, &mut reference_budget)
        .and_then(|()| {
            super::super::validate_blocks_and_edges(
                program,
                function,
                &facts,
                &storage,
                &leaves,
                None,
                &mut reference_budget,
            )
        });
    let ((output, allocation), refused, _, _) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = if retained {
                validate_status_sources_retained(function, &facts, &mut budget).and_then(
                    |statuses| {
                        validate_blocks_and_edges_retained(
                            program,
                            function,
                            &facts,
                            &storage,
                            &leaves,
                            None,
                            &mut budget,
                            &statuses,
                        )
                    },
                )
            } else {
                super::super::validate_status_sources(function, &facts, &mut budget).and_then(
                    |()| {
                        super::super::validate_blocks_and_edges(
                            program,
                            function,
                            &facts,
                            &storage,
                            &leaves,
                            None,
                            &mut budget,
                        )
                    },
                )
            };
            let work = MAX_REPLAY_WORK_UNITS - budget.remaining;
            assert_eq!(result, expected);
            drop(result);
            drop(budget);
            work
        })
    });
    assert!(!refused);
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    (expected, output)
}

#[test]
fn retained_statuses_preserve_complete_receivers_six_fields_first_error_and_drop() {
    let program = program();
    let mut saved = false;
    for function in &program.functions {
        let old = run(&program, function, false);
        let new = run(&program, function, true);
        assert_eq!(old.0, new.0);
        if !function.cleanup_plan.status_sources.is_empty() && old.0.is_ok() {
            assert!(new.1 < old.1);
            saved = true;
        }
        let mut attacks = Vec::new();
        if let Some(source) = function.cleanup_plan.status_sources.first() {
            let mut duplicate = function.clone();
            duplicate.cleanup_plan.status_sources.push(source.clone());
            let mut later = source.clone();
            later.id.expression = ExpressionId::from_owned("foreign.later.status".into());
            duplicate.cleanup_plan.status_sources.push(later);
            attacks.push(duplicate);
            let mut unknown = function.clone();
            unknown.cleanup_plan.status_sources[0].id.expression =
                ExpressionId::from_owned("foreign.first.status".into());
            attacks.push(unknown);
        }
        if !function.cleanup_plan.edges.is_empty() {
            let mut edge = function.clone();
            edge.cleanup_plan.edges[0].id = EdgeId(u32::MAX);
            attacks.push(edge);
        }
        for hostile in attacks {
            let old = run(&program, &hostile, false);
            let new = run(&program, &hostile, true);
            assert!(old.0.is_err());
            assert_eq!(old.0, new.0);
        }
    }
    assert!(saved);
}

#[test]
fn retained_statuses_reject_foreign_function_budget_and_sticky_work_refusal() {
    let program = program();
    let function = &program.functions[0];
    let foreign = function.clone();
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let mut budget = ReplayBudget::unobserved();
    let statuses = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
    let mut other = ReplayBudget::unobserved();
    for (subject, payer) in [(&foreign, &mut budget), (function, &mut other)] {
        let actual = validate_blocks_and_edges_retained(
            &program, subject, &facts, &storage, &leaves, None, payer, &statuses,
        );
        assert_eq!(
            actual,
            Err(replay_error(
                subject,
                "status directory belongs to another function or budget"
            ))
        );
    }
    budget.remaining = 0;
    let first = budget
        .charge(function, 1, "retained status refusal control")
        .unwrap_err();
    assert!(first.message.contains("work budget exhausted"));
    let result = validate_blocks_and_edges_retained(
        &program,
        function,
        &facts,
        &storage,
        &leaves,
        None,
        &mut budget,
        &statuses,
    );
    assert!(result.is_err());
    assert_eq!(budget.remaining, 0);
}

#[test]
fn complete_pre_retention_sources_and_all_inverse_hooks_remain_exact() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/cleanup-status-directory-retention/exact-source-inverses.json"
    )))
    .unwrap();
    let complete_sources = [
        ("src/cleanup_plan/replay/expression_facts/receiving.rs", "src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt"))),
        ("src/cleanup_plan/replay.rs", "src__cleanup_plan__replay.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__cleanup_plan__replay.rs.before.txt"))),
        ("src/graph.rs", "src__graph.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/graph.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__graph.rs.before.txt"))),
        ("src/workspace_graph/source_schema/static_text/tests/retained_lexical_composition.rs", "src__workspace_graph__source_schema__static_text__tests__retained_lexical_composition.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/source_schema/static_text/tests/retained_lexical_composition.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__workspace_graph__source_schema__static_text__tests__retained_lexical_composition.rs.before.txt"))),
        ("src/cleanup_plan/replay/schema/paid_summary/tests.rs", "src__cleanup_plan__replay__schema__paid_summary__tests.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/schema/paid_summary/tests.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__cleanup_plan__replay__schema__paid_summary__tests.rs.before.txt"))),
        ("src/cleanup_plan/replay/allocation/tests/accounting_scope.rs", "src__cleanup_plan__replay__allocation__tests__accounting_scope.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/allocation/tests/accounting_scope.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__cleanup_plan__replay__allocation__tests__accounting_scope.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/a.rs", "src__cleanup_plan__replay__live_oracle__source_custody__a.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/a.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/src__cleanup_plan__replay__live_oracle__source_custody__a.rs.before.txt"))),
    ];
    assert_eq!(records.as_array().unwrap().len(), complete_sources.len());
    for record in records.as_array().unwrap() {
        let path = record["source"].as_str().unwrap();
        let (_, archive_name, current, archive) = complete_sources
            .iter()
            .find(|(source, _, _, _)| *source == path)
            .unwrap();
        assert_eq!(record["archive"].as_str().unwrap(), *archive_name);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        assert_eq!(before_status_retention(path, current), *archive);
    }
}

#[test]
fn retained_status_control_funding_exact_short_zero_and_foreign_active_are_sticky() {
    let program = program();
    let function = &program.functions[0];
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let probe = |limit, retain| {
        let ((success, allocation), refused, debit, first) =
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let success = if retain {
                        let result = super::status_before::validate_status_sources_retained(
                            function,
                            &facts,
                            &mut budget,
                        );
                        let success = result.is_ok();
                        drop(result);
                        success
                    } else {
                        let result =
                            super::super::validate_status_sources(function, &facts, &mut budget);
                        let success = result.is_ok();
                        drop(result);
                        success
                    };
                    drop(budget);
                    success
                })
            });
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        (success, refused, debit, first)
    };
    let old = probe(1 << 26, false);
    let exact = old.2.checked_add(CONTROL_BYTES).unwrap();
    let full = probe(exact, true);
    assert!(old.0 && !old.1 && old.3.is_none());
    assert!(full.0 && !full.1 && full.3.is_none());
    assert_eq!(full.2, exact);
    for limit in [0, exact - 1] {
        let refused = probe(limit, true);
        assert!(!refused.0 && refused.1 && refused.3.is_some());
    }
    let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let statuses = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
            let (_, foreign_refused, foreign_debit, foreign_first) =
                with_limit_usage_refusal(1 << 26, || {
                    let result = validate_blocks_and_edges_retained(
                        &program,
                        function,
                        &facts,
                        &storage,
                        &leaves,
                        None,
                        &mut budget,
                        &statuses,
                    );
                    assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
                    assert!(result
                        .as_ref()
                        .unwrap_err()
                        .message
                        .contains("invocation owner changed"));
                    drop(result);
                });
            assert!(!foreign_refused && foreign_first.is_none());
            assert_eq!(foreign_debit, 0);
            let restored = validate_blocks_and_edges_retained(
                &program,
                function,
                &facts,
                &storage,
                &leaves,
                None,
                &mut budget,
                &statuses,
            );
            assert!(restored.is_err());
            assert_eq!(budget.remaining, 0);
            drop(restored);
            drop(statuses);
            drop(budget);
        })
    });
    assert!(refused && first.is_some());
    assert_eq!(first.unwrap().requested, 0);
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}


// The facts, source, slots and leaves are prepared independently of either
// receiver's observed interval. Both complete status/blocks receivers and all
// retained directories/results drop before the allocation observer closes.
fn expected_directory_probe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: bool,
    limit: usize,
) -> (bool, usize, usize, usize, bool) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        ).unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let mut reference = ReplayBudget::unobserved();
    let expected = validate_status_sources_retained(function, &facts, &mut reference)
        .and_then(|statuses| before_expected_directory::validate_blocks_and_edges_retained(
            program, function, &facts, &storage, &leaves, None, &mut reference, &statuses,
        ));
    let ((result, allocation), refused, debit, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = validate_status_sources_retained(function, &facts, &mut budget)
                .and_then(|statuses| {
                    if current {
                        validate_blocks_and_edges_retained(
                            program, function, &facts, &storage, &leaves, None, &mut budget, &statuses,
                        )
                    } else {
                        before_expected_directory::validate_blocks_and_edges_retained(
                            program, function, &facts, &storage, &leaves, None, &mut budget, &statuses,
                        )
                    }
                });
            // Compare every Diagnostic field at the same admitted budget. A
            // deliberate tight-budget refusal is separately asserted below.
            if limit == 1 << 26 { assert_eq!(result, expected); }
            let success = result.is_ok();
            let work = MAX_REPLAY_WORK_UNITS - budget.remaining;
            drop(result);
            drop(budget);
            (success, work)
        })
    });
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert_eq!(refused, first.is_some());
    (result.0, result.1, allocation.allocated, debit, refused)
}

fn lexical_expected_directory_program(calls: usize) -> ResolvedProgram {
    let body = "leaf(value);".repeat(calls);
    let source = format!(r#"module replay.expected;
@id("expected.leaf") fn leaf(value:i64)->i64 {{ value }}
@id("expected.walk") fn walk()->i64 {{
 let mut value=0;
 while value<3 {{ value=value+1; {body} 0 }}
 value
}}
@id("app.main") fn main()->i64 {{walk()}}
"#);
    crate::hir::resolve(&crate::check(&source, "expected-directory.spx").unwrap()).unwrap()
}

#[test]
fn unused_lexical_expected_directory_keeps_complete_receivers_and_drop() {
    let mut saved = false;
    for calls in [0, 1, 8, 64] {
        let program = lexical_expected_directory_program(calls);
        for function in &program.functions {
            let old = expected_directory_probe(&program, function, false, 1 << 26);
            let new = expected_directory_probe(&program, function, true, 1 << 26);
            assert!(old.0 && new.0 && !old.4 && !new.4);
            if function.cleanup_plan.lexical_control.is_none() {
                assert_eq!(old, new);
            } else {
                assert!(new.1 <= old.1 && new.2 <= old.2 && new.3 <= old.3);
                if calls > 0 {
                    assert!(new.1 < old.1 && new.2 < old.2 && new.3 < old.3);
                    saved = true;
                }
            }
            // A late foreign call retains the complete owning receiver's error
            // before the removed terminal directory could have been built.
            let mut hostile = function.clone();
            let mut changed = false;
            for block in hostile.cleanup_plan.blocks.iter_mut().rev() {
                for action in block.transitions.iter_mut().rev() {
                    if let CleanupTransition::CallCommit { call, .. } = action {
                        *call = ExpressionId::from_owned("foreign.expected.call".into());
                        changed = true;
                        break;
                    }
                }
                if changed { break; }
            }
            if changed {
                let old = expected_directory_probe(&program, &hostile, false, 1 << 26);
                let new = expected_directory_probe(&program, &hostile, true, 1 << 26);
                assert!(!old.0 && !new.0);
            }
        }
    }
    assert!(saved);
}

#[test]
fn unused_lexical_expected_directory_exact_short_zero_and_invoice_coverage() {
    let program = lexical_expected_directory_program(8);
    let function = program.functions.iter()
        .find(|f| f.cleanup_plan.lexical_control.is_some()).unwrap();
    let full = expected_directory_probe(&program, function, true, 1 << 26);
    assert!(full.0 && !full.4 && full.3 > 0);
    let exact = expected_directory_probe(&program, function, true, full.3);
    assert!(exact.0 && !exact.4);
    assert_eq!(exact.3, full.3);
    for limit in [0, full.3 - 1] {
        let short = expected_directory_probe(&program, function, true, limit);
        assert!(!short.0 && short.4);
        assert!(short.3 <= limit);
    }
    // Cumulative charged bytes must still cover actual observed allocation.
    // This is not a physical-floor refusal control.
    assert!(full.3 >= full.2);
}


fn expected_directory_full_replay(
    program: &ResolvedProgram, function: &ResolvedFunction, before: bool,
) -> Result<(), Diagnostic> {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) { EXPECTED_DIRECTORY_BEFORE.set(self.0); }
    }
    let _restore = Restore(EXPECTED_DIRECTORY_BEFORE.replace(before));
    crate::cleanup_plan::replay::validate_structure(program, function)
}

#[test]
fn unused_expected_directory_keeps_full_typed_skeleton_commit_coverage() {
    let mut lexical_attacks = [0usize; 3];
    let program = lexical_expected_directory_program(8);
    for function in &program.functions {
        let entries_before = EXPECTED_DIRECTORY_BEFORE_ENTRIES.get();
        let old = expected_directory_full_replay(&program, function, true);
        if function.cleanup_plan.lexical_control.is_some() {
            assert!(EXPECTED_DIRECTORY_BEFORE_ENTRIES.get() > entries_before);
        }
        let current = expected_directory_full_replay(&program, function, false);
        assert!(old.is_ok());
        assert_eq!(old, current);
        for attack in 0..3 {
            let mut hostile = function.clone();
            let mut changed = false;
            for block in &mut hostile.cleanup_plan.blocks {
                let Some(index) = block.transitions.iter().position(|action|
                    matches!(action, CleanupTransition::CallCommit { .. })) else { continue; };
                match attack {
                    0 => { block.transitions.remove(index); }
                    1 => {
                        let duplicate = block.transitions[index].clone();
                        block.transitions.insert(index, duplicate);
                    }
                    _ => {
                        let mut extra = block.transitions[index].clone();
                        if let CleanupTransition::CallCommit { call, .. } = &mut extra {
                            *call = ExpressionId::from_owned("foreign.extra.commit".into());
                        }
                        block.transitions.push(extra);
                    }
                }
                changed = true;
                break;
            }
            if changed {
                if function.cleanup_plan.lexical_control.is_some() { lexical_attacks[attack] += 1; }
                let old = expected_directory_full_replay(&program, &hostile, true);
                let current = expected_directory_full_replay(&program, &hostile, false);
                assert!(old.is_err(), "attack {attack}");
                assert_eq!(old, current, "attack {attack}");
            }
        }
    }
    assert!(lexical_attacks.into_iter().all(|count| count > 0));
    assert!(!EXPECTED_DIRECTORY_BEFORE.get());
}


#[test]
fn unused_expected_directory_whole_predecessor_inverse_remains_exact() {
    use sha2::{Digest, Sha256};
    let current = include_str!("../retained.rs");
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), "f7526f0bd15e02dfd2944a5a79f9eb2d519f26e3dee518594db36fbfacbfc403");
    let marker = "\n// Whole immediate predecessor: independent executable comparison receiver.\n#[cfg(test)]\nmod before_expected_directory {\n    use super::*;\n";
    assert_eq!(current.matches(marker).count(), 1);
    let (prefix, suffix) = current.split_once(marker).unwrap();
    let (saved, tail) = suffix.split_once("\n}\n\n// Test-only dispatch").unwrap();
    assert!(tail.starts_with(" changes only this receiver,"));
    assert_eq!(saved.len(), 23241);
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(saved.as_bytes()))), "9edd3f960d5003444ba7cc84bc77a05c16bb1e8453902a2d6ac45c9a9f7b75a6");
    let mut restored = prefix.to_owned();
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&restored.as_bytes()[6140..30084]))), "f739e5477b251e8fcbbe249996f4d7abf04c767d2c24c55aca98cb25ed656491");
    restored.replace_range(6140..30084, saved);
    assert_eq!(restored.len(), 38695);
    assert_eq!(restored.lines().count(), 895);
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), "d35fedf3a3d6ad0d4e3878eac55eccdca5cfc4653705ec7bb726f69ef93cfc00");
    let tests = include_str!("tests.rs");
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&tests.as_bytes()[..14636]))), "2d11e36011a6ff396fb6d1d2b4c7c31bf2298d696979bb2fa5a13ee2043818a5");
}

#[test]
fn unused_expected_directory_lexical_floor_foreign_and_sticky_keep_all_fields() {
    use crate::bounded_output::{active_remaining, clear_active_floor, set_active_floor};
    let program = lexical_expected_directory_program(8);
    let function = program.functions.iter().find(|f| f.cleanup_plan.lexical_control.is_some()).unwrap();
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(
        function,
        capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut preparation,
    ).unwrap());
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    for foreign in [false, true] {
        let (((), allocation), _, _, _) = with_limit_usage_refusal(1 << 28, || measure(|| {
            let observe = |current| with_limit_usage_refusal(1 << 26, || {
                let mut budget = ReplayBudget::new();
                let statuses = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
                let receive = |budget: &mut ReplayBudget| {
                    if current {
                        validate_blocks_and_edges_retained(&program, function, &facts, &storage, &leaves, None, budget, &statuses)
                    } else {
                        before_expected_directory::validate_blocks_and_edges_retained(&program, function, &facts, &storage, &leaves, None, budget, &statuses)
                    }
                };
                let first = if foreign {
                    let (result, refused, debit, first) = with_limit_usage_refusal(1 << 26, || receive(&mut budget));
                    assert!(!refused && first.is_none());
                    assert_eq!(debit, 0);
                    result
                } else {
                    let remaining = active_remaining().unwrap();
                    assert!(remaining > 0 && set_active_floor(remaining));
                    let result = receive(&mut budget);
                    clear_active_floor();
                    result
                };
                assert!(first.is_err());
                let retry = receive(&mut budget);
                assert!(retry.is_err());
                assert_eq!(budget.remaining, 0);
                drop(statuses);
                drop(budget);
                (first, retry)
            });
            let old = observe(false);
            let current = observe(true);
            assert_eq!(old, current, "complete Diagnostic fields, debit, refusal and first receipt; foreign={foreign}");
            assert!(old.1);
            let receipt = old.3.as_ref().unwrap();
            if foreign {
                assert_eq!(receipt.requested, 0);
            } else {
                assert!(receipt.requested > 0 && receipt.floor > 0);
                assert_eq!(receipt.remaining, receipt.floor);
            }
            drop(old);
            drop(current);
        }));
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn unused_expected_directory_mixed_control_and_call_targets_keep_full_replay() {
    let source = r#"module replay.expected_mixed;
@id("mixed.left") fn left(value:i64)->i64 { value+1 }
@id("mixed.right") fn right(value:i64)->i64 { value+2 }
@id("mixed.straight") fn straight()->i64 { left(0)+right(1) }
@id("mixed.walk") fn walk()->i64 {
 let mut value=0;
 while value<3 {
  value=value+1;
  if value<2 { left(value) } else { right(value) };
  0
 }
 straight()+value
}
@id("app.main") fn main()->i64 { walk() }
"#;
    let program = crate::hir::resolve(&crate::check(source, "expected-mixed.spx").unwrap()).unwrap();
    let mut lexical = 0;
    let mut ordinary = 0;
    for function in &program.functions {
        if function.cleanup_plan.lexical_control.is_some() { lexical += 1; } else { ordinary += 1; }
        let old = expected_directory_full_replay(&program, function, true);
        let current = expected_directory_full_replay(&program, function, false);
        assert!(old.is_ok());
        assert_eq!(old, current);
        expected_directory_probe(&program, function, false, 1 << 26);
        expected_directory_probe(&program, function, true, 1 << 26);
    }
    assert!(lexical > 0 && ordinary > 0);
    assert!(!EXPECTED_DIRECTORY_BEFORE.get());
}
