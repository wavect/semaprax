//! Whole old/current receiver, diagnostics and last Drop stay inside the observer.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};

fn program(count: usize) -> ResolvedProgram {
    let mut source = String::from(
        "module proof.status_cases;\n@id(\"cases.add\") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1}\n@id(\"cases.main\") fn main()->i64 {\n",
    );
    for n in 0..count {
        source.push_str(&format!("let v{n}=add({n})+3-2*4/2%3;\n"));
    }
    source.push_str("-1}\n");
    crate::hir::resolve(&crate::check(&source, "status-case-proof.spx").unwrap()).unwrap()
}
fn facts(function: &ResolvedFunction) -> Facts<'_> {
    let mut preparation = ReplayBudget::unobserved();
    Facts::Arena(
        crate::cleanup_plan::replay::expression_facts::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    )
}
fn receive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    budget: &mut ReplayBudget,
    current: bool,
) -> Result<(), Diagnostic> {
    if current {
        let proof = StatusCases::validate(program, function, budget)?;
        let statuses = validate_status_sources_certified(function, facts, budget, &proof)?;
        drop(statuses);
        drop(proof);
    } else {
        crate::cleanup_plan::replay::validate_required_status_sources(program, function)?;
        let statuses = validate_status_sources_retained(function, facts, budget)?;
        drop(statuses);
    }
    Ok(())
}
fn observe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: bool,
    bytes: usize,
    work: usize,
    expected: Option<&Result<(), Diagnostic>>,
) -> (bool, usize, usize) {
    let facts = facts(function);
    let (((ok, spent), metrics), refused, paid, _) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work;
            let result = receive(program, function, &facts, &mut budget, current);
            if let Some(expected) = expected {
                assert_eq!(&result, expected, "all six diagnostic fields");
            }
            let ok = result.is_ok();
            drop(result);
            let spent = work - budget.remaining;
            drop(budget);
            (ok, spent)
        })
    });
    assert!(
        !metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed,
        "whole producer/diagnostic/last Drop: {metrics:?}"
    );
    if ok {
        assert!(!refused);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    }
    (ok, spent, paid)
}

#[test]
fn complete_old_and_certified_cases_preserve_all_operations_rows_and_whole_drop() {
    for count in [1, 8, 33] {
        let program = program(count);
        use crate::cleanup_plan::CheckedOperation;
        for expected in [
            CheckedOperation::Neg,
            CheckedOperation::Add,
            CheckedOperation::Sub,
            CheckedOperation::Mul,
            CheckedOperation::Div,
            CheckedOperation::Rem,
        ] {
            assert!(program.functions.iter().flat_map(|function|
                &function.cleanup_plan.status_sources).any(|source|
                matches!(&source.producer, StatusProducer::CheckedArithmetic { operation, .. }
                    if *operation == expected)), "fixture covers every checked operation");
        }
        for function in &program.functions {
            let old = observe(
                &program,
                function,
                false,
                usize::MAX,
                MAX_REPLAY_WORK_UNITS,
                Some(&Ok(())),
            );
            let current = observe(
                &program,
                function,
                true,
                usize::MAX,
                MAX_REPLAY_WORK_UNITS,
                Some(&Ok(())),
            );
            assert!(old.0 && current.0);
            // The models charge their actual distinct operations. No universal
            // paid/work improvement follows from eliminating a case directory.
            assert!(old.1 > 0 && current.1 > 0 && old.2 > 0 && current.2 > 0);
        }
    }
}

#[test]
fn complete_source_walk_rejects_hostile_cases_ids_callee_and_lane_before_certificate() {
    let program = program(2);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cases.main")
        .unwrap();
    let arithmetic = function
        .cleanup_plan
        .status_sources
        .iter()
        .position(|s| matches!(s.producer, StatusProducer::CheckedArithmetic { .. }))
        .unwrap();
    let division = function
        .cleanup_plan
        .status_sources
        .iter()
        .position(|s| {
            matches!(&s.producer, StatusProducer::CheckedArithmetic { normalized_cases, .. }
            if normalized_cases.len() == 2)
        })
        .unwrap();
    let mut attacks = Vec::new();
    for kind in 0..4 {
        let mut changed = function.clone();
        let StatusProducer::CheckedArithmetic {
            normalized_cases,
            operation,
        } = &mut changed.cleanup_plan.status_sources[if kind == 2 { division } else { arithmetic }]
            .producer
        else {
            unreachable!()
        };
        match kind {
            0 => normalized_cases.clear(),
            1 => normalized_cases.push(normalized_cases[0]),
            2 => normalized_cases.reverse(),
            _ => *operation = crate::cleanup_plan::CheckedOperation::Rem,
        }
        attacks.push(changed);
    }
    let mut changed = function.clone();
    changed
        .cleanup_plan
        .status_sources
        .push(changed.cleanup_plan.status_sources[0].clone());
    attacks.push(changed);
    let mut changed = function.clone();
    changed.cleanup_plan.status_sources[0].id.expression =
        ExpressionId::from_owned("missing.row".into());
    attacks.push(changed);
    let mut changed = function.clone();
    changed.cleanup_plan.status_sources[0].id.lane = StatusLane::ContractFalse;
    attacks.push(changed);
    let mut changed = function.clone();
    let StatusProducer::PropagatedCall { callee } =
        &mut changed.cleanup_plan.status_sources[0].producer
    else {
        panic!("first source is add call")
    };
    *callee = DeclarationId::new("wrong.callee");
    attacks.push(changed);
    for changed in attacks {
        let facts = facts(&changed);
        let mut budget = ReplayBudget::unobserved();
        let expected = receive(&program, &changed, &facts, &mut budget, false);
        assert!(expected.is_err());
        observe(
            &program,
            &changed,
            false,
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            Some(&expected),
        );
        observe(
            &program,
            &changed,
            true,
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            Some(&expected),
        );
    }
}

#[test]
fn each_receiver_has_derived_exact_short_and_zero_bytes_and_work() {
    let program = program(3);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cases.main")
        .unwrap();
    for current in [false, true] {
        let ample = observe(
            &program,
            function,
            current,
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            Some(&Ok(())),
        );
        assert!(ample.0);
        assert!(observe(&program, function, current, ample.2, ample.1, Some(&Ok(()))).0);
        assert!(
            !observe(
                &program,
                function,
                current,
                ample.2 - 1,
                MAX_REPLAY_WORK_UNITS,
                None
            )
            .0
        );
        assert!(!observe(&program, function, current, 0, MAX_REPLAY_WORK_UNITS, None).0);
        assert!(!observe(&program, function, current, usize::MAX, ample.1 - 1, None).0);
        assert!(!observe(&program, function, current, usize::MAX, 0, None).0);
    }
}

#[test]
fn proof_binds_exact_rows_function_budget_and_foreign_restored_invocation() {
    let program = program(2);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cases.main")
        .unwrap();
    let foreign_function = function.clone();
    let forged = function.cleanup_plan.status_sources[0].clone();
    let empty = Diagnostic::io("SPX-H006", String::new());
    for kind in 0..5 {
        let (((), metrics), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let proof = StatusCases::validate(&program, function, &mut budget).unwrap();
                let attempt = proof.begin(function, &mut budget).unwrap();
                let at = current_metrics().allocated;
                let error = match kind {
                    0 => proof.member(
                        &foreign_function,
                        &foreign_function.cleanup_plan.status_sources[0],
                        0,
                        &mut budget,
                    ),
                    1 => {
                        let mut foreign = ReplayBudget::new();
                        proof.member(
                            function,
                            &function.cleanup_plan.status_sources[0],
                            0,
                            &mut foreign,
                        )
                    }
                    2 => proof.member(function, &forged, 0, &mut budget),
                    3 => proof.member(
                        function,
                        &function.cleanup_plan.status_sources[0],
                        usize::MAX,
                        &mut budget,
                    ),
                    _ => {
                        with_limit_usage_refusal(usize::MAX, || {
                            let inside = current_metrics().allocated;
                            let result = proof.member(
                                function,
                                &function.cleanup_plan.status_sources[0],
                                0,
                                &mut budget,
                            );
                            assert_eq!(current_metrics().allocated, inside);
                            result
                        })
                        .0
                    }
                };
                assert_eq!(error, Err(empty.clone()));
                let restored = current_metrics().allocated;
                assert_eq!(
                    proof.member(
                        function,
                        &function.cleanup_plan.status_sources[0],
                        0,
                        &mut budget
                    ),
                    Err(empty.clone())
                );
                assert_eq!(current_metrics().allocated, restored);
                if kind != 4 {
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(attempt);
                drop(proof);
                drop(budget);
            })
        });
        // A foreign current() deliberately poisons its original invocation.
        assert_eq!(refused, kind == 4);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
}

#[test]
fn selected_member_refusal_latches_without_post_refusal_acquisition_or_drop_allocation() {
    let program = program(1);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cases.main")
        .unwrap();
    let (((), metrics), _, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let proof = StatusCases::validate(&program, function, &mut budget).unwrap();
            let attempt = proof.begin(function, &mut budget).unwrap();
            budget.remaining = 0;
            let first = proof
                .member(
                    function,
                    &function.cleanup_plan.status_sources[0],
                    0,
                    &mut budget,
                )
                .unwrap_err();
            assert_eq!(first.code, "SPX-H006");
            assert!(first.message.contains("complete status case proof"));
            drop(first);
            let at = current_metrics().allocated;
            let later = proof.member(
                function,
                &function.cleanup_plan.status_sources[0],
                0,
                &mut budget,
            );
            assert_eq!(later, Err(Diagnostic::io("SPX-H006", String::new())));
            drop(later);
            drop(attempt);
            drop(proof);
            drop(budget);
            assert_eq!(current_metrics().allocated, at);
        })
    });
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
}

#[test]
fn complete_direct_receiver_and_independent_source_discovery_remain_byte_exact() {
    let old = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/status-case-proof-816/src__cleanup_plan__replay__expression_facts__receiving__retained.rs.before.txt"
    ));
    let current = include_str!("../../retained.rs");
    let start = "pub(in crate::cleanup_plan::replay) fn validate_status_sources_retained";
    let end = "// The retained key proof";
    assert_eq!(
        current
            .split_once(start)
            .unwrap()
            .1
            .split_once(end)
            .unwrap()
            .0,
        old.split_once(start).unwrap().1.split_once(end).unwrap().0
    );
    assert_eq!(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_status.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/status-case-proof-816/src__cleanup_plan__replay__ordinary_status.rs.before.txt"
        ))
    );
}

#[test]
fn attempts_cannot_complete_after_poison_foreign_payer_or_successful_consumption() {
    let program = program(1);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "cases.main")
        .unwrap();
    for kind in 0..4 {
        let (((), metrics), _, _, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let proof = StatusCases::validate(&program, function, &mut budget).unwrap();
                let mut attempt = proof.begin(function, &mut budget).unwrap();
                let at = current_metrics().allocated;
                if kind == 0 {
                    assert!(proof.begin(function, &mut budget).is_err());
                    assert!(attempt.complete(function, &mut budget).is_err());
                } else if kind == 1 {
                    let mut foreign = ReplayBudget::new();
                    assert!(attempt.complete(function, &mut foreign).is_err());
                    assert!(attempt.complete(function, &mut budget).is_err());
                } else if kind == 2 {
                    proof.reject();
                    assert!(attempt.complete(function, &mut budget).is_err());
                } else {
                    attempt.complete(function, &mut budget).unwrap();
                    assert!(proof.begin(function, &mut budget).is_err());
                    assert!(attempt.complete(function, &mut budget).is_err());
                }
                assert_eq!(current_metrics().allocated, at);
                drop(attempt);
                drop(proof);
                drop(budget);
            })
        });
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    }
}
