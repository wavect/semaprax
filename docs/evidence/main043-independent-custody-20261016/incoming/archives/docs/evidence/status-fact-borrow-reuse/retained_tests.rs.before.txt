use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = include_str!("../../../expression_facts.rs")
        .split_once("r#\"module replay.fact_owner;").unwrap().1
        .split_once("\"#,").unwrap().0;
    let source = format!("module replay.fact_owner;{source}");
    crate::hir::resolve(&crate::check(&source, "retained-status.spx").unwrap()).unwrap()
}

// Inputs/facts are outside this phase observer. Both full receivers, their paid
// directories, result diagnostics, budget and every retained allocation drop inside.
fn run(program: &ResolvedProgram, function: &ResolvedFunction, retained: bool)
    -> (Result<(), Diagnostic>, usize)
{
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(
        function, capacity::Payer::Selected(
            &mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut preparation,
    ).unwrap());
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let mut reference_budget = ReplayBudget::unobserved();
    let expected = super::super::validate_status_sources(function, &facts, &mut reference_budget)
        .and_then(|()| super::super::validate_blocks_and_edges(program, function, &facts,
            &storage, &leaves, None, &mut reference_budget));
    let ((output, allocation), refused, _, _) = with_limit_usage_refusal(1 << 26, || measure(|| {
        let mut budget = ReplayBudget::new();
        let result = if retained {
            validate_status_sources_retained(function, &facts, &mut budget).and_then(|statuses| {
                validate_blocks_and_edges_retained(program, function, &facts, &storage,
                    &leaves, None, &mut budget, &statuses)
            })
        } else {
            super::super::validate_status_sources(function, &facts, &mut budget).and_then(|()| {
                super::super::validate_blocks_and_edges(program, function, &facts, &storage,
                    &leaves, None, &mut budget)
            })
        };
        let work = MAX_REPLAY_WORK_UNITS - budget.remaining;
        assert_eq!(result, expected);
        drop(result);
        drop(budget);
        work
    }));
    assert!(!refused);
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed);
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
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(
        function, capacity::Payer::Selected(
            &mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut preparation,
    ).unwrap());
    let mut budget = ReplayBudget::unobserved();
    let statuses = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
    let mut other = ReplayBudget::unobserved();
    for (subject, payer) in [(&foreign, &mut budget), (function, &mut other)] {
        let actual = validate_blocks_and_edges_retained(&program, subject, &facts,
            &storage, &leaves, None, payer, &statuses);
        assert_eq!(actual, Err(replay_error(subject,
            "status directory belongs to another function or budget")));
    }
    budget.remaining = 0;
    let first = budget.charge(function, 1, "retained status refusal control").unwrap_err();
    assert!(first.message.contains("work budget exhausted"));
    let result = validate_blocks_and_edges_retained(&program, function, &facts,
        &storage, &leaves, None, &mut budget, &statuses);
    assert!(result.is_err());
    assert_eq!(budget.remaining, 0);
}

#[test]
fn complete_pre_retention_sources_and_all_inverse_hooks_remain_exact() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/docs/evidence/cleanup-status-directory-retention/exact-source-inverses.json"
    ))).unwrap();
    for record in records.as_array().unwrap() {
        let path = record["source"].as_str().unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let archive = std::fs::read_to_string(root.join("docs/evidence/cleanup-status-directory-retention")
            .join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))),
            record["sha256"].as_str().unwrap());
        assert_eq!(before_status_retention(path, &current), archive);
    }
}


#[test]
fn retained_status_control_funding_exact_short_zero_and_foreign_active_are_sticky() {
    let program = program();
    let function = &program.functions[0];
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(
        function, capacity::Payer::Selected(
            &mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut preparation,
    ).unwrap());
    let probe = |limit, retain| {
        let ((success, allocation), refused, debit, first) = with_limit_usage_refusal(limit, || measure(|| {
            let mut budget = ReplayBudget::new();
            let success = if retain {
                let result = validate_status_sources_retained(function, &facts, &mut budget);
                let success = result.is_ok();
                drop(result);
                success
            } else {
                let result = super::super::validate_status_sources(function, &facts, &mut budget);
                let success = result.is_ok();
                drop(result);
                success
            };
            drop(budget);
            success
        }));
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed);
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
    let (((), allocation), refused, _, first) = with_limit_usage_refusal(1 << 26, || measure(|| {
        let mut budget = ReplayBudget::new();
        let statuses = validate_status_sources_retained(function, &facts, &mut budget).unwrap();
        let (_, foreign_refused, foreign_debit, foreign_first) = with_limit_usage_refusal(1 << 26, || {
            let result = validate_blocks_and_edges_retained(&program, function, &facts,
                &storage, &leaves, None, &mut budget, &statuses);
            assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
            assert!(result.as_ref().unwrap_err().message.contains("invocation owner changed"));
            drop(result);
        });
        assert!(!foreign_refused && foreign_first.is_none());
        assert_eq!(foreign_debit, 0);
        let restored = validate_blocks_and_edges_retained(&program, function, &facts,
            &storage, &leaves, None, &mut budget, &statuses);
        assert!(restored.is_err());
        assert_eq!(budget.remaining, 0);
        drop(restored);
        drop(statuses);
        drop(budget);
    }));
    assert!(refused && first.is_some());
    assert_eq!(first.unwrap().requested, 0);
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed);
}
