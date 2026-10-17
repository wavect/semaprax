use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn function() -> ResolvedFunction {
    let mut source = String::from("module inline.status; @id(\"inline.f\") fn f(x:i64)->i64{");
    for i in 0..256 {
        if i == 0 {
            source.push_str("let v0=x+1;");
        } else {
            source.push_str(&format!("let v{i}=v{}+1;", i - 1));
        }
    }
    source.push_str("v255} @id(\"inline.main\") fn main()->i64{0}");
    let mut program =
        crate::hir::resolve(&crate::check(&source, "inline-status.spx").unwrap()).unwrap();
    let at = program
        .functions
        .iter()
        .position(|f| f.id.as_str() == "inline.f")
        .unwrap();
    let function = program.functions.swap_remove(at);
    assert_eq!(function.cleanup_plan.status_sources.len(), 256);
    function
}

// Complete old and new validation loops execute against the same independently
// built facts. All current status directories, errors and budgets drop inside
// the allocation observer; input preparation supplies no allocation credit.
fn compare(function: &ResolvedFunction) -> (bool, usize, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        crate::cleanup_plan::replay::expression_facts::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let mut reference = ReplayBudget::unobserved();
    let expected =
        super::super::validate_status_sources_held_prefix(function, &facts, &mut reference)
            .map(|statuses| statuses.ids.as_ref().unwrap().len());
    let mut paid = [0; 2];
    for inline in [false, true] {
        let (((), metrics), refused, spent, first) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let actual = if inline {
                    validate(function, &facts, &mut budget)
                } else {
                    super::super::validate_status_sources_held_prefix(function, &facts, &mut budget)
                };
                assert_eq!(
                    actual.as_ref().map(|s| s.ids.as_ref().unwrap().len()),
                    expected.as_ref().copied()
                );
                if let Ok(statuses) = &actual {
                    let ids = statuses.ids.as_ref().unwrap();
                    assert!(ids.balanced());
                    assert_eq!(ids.len(), function.cleanup_plan.status_sources.len());
                    for source in &function.cleanup_plan.status_sources {
                        assert!(ids.keys().any(|key| std::ptr::eq(*key, &source.id)));
                    }
                }
                drop(actual);
                drop(budget);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        assert!(metrics.allocated <= spent && metrics.highwater <= spent);
        paid[usize::from(inline)] = spent;
    }
    (expected.is_ok(), paid[0], paid[1])
}

#[test]
fn complete_inline_status_loop_keeps_all_current_rows_and_pays_every_live_control() {
    let function = function();
    let (success, old, new) = compare(&function);
    assert!(success);
    assert!(new < old);
}

#[test]
fn complete_inline_status_loop_keeps_duplicate_unknown_case_callee_and_lane_refusals() {
    let function = function();
    for attack in 0..6 {
        let mut hostile = function.clone();
        match attack {
            0 => {
                hostile
                    .cleanup_plan
                    .status_sources
                    .push(function.cleanup_plan.status_sources[0].clone());
                let mut later = function.cleanup_plan.status_sources[0].clone();
                later.id.expression = ExpressionId::from_owned("foreign.later".into());
                hostile.cleanup_plan.status_sources.push(later);
            }
            1 => {
                hostile.cleanup_plan.status_sources[0].id.expression =
                    ExpressionId::from_owned("foreign.first".into())
            }
            2 => hostile.cleanup_plan.status_sources[0].id.lane = StatusLane::ContractFalse,
            3 | 4 => {
                let StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } = &mut hostile.cleanup_plan.status_sources[0].producer
                else {
                    panic!("checked arithmetic fixture");
                };
                if attack == 3 {
                    normalized_cases.clear();
                } else {
                    normalized_cases.push(normalized_cases[0]);
                }
            }
            _ => {
                hostile.cleanup_plan.status_sources[0].producer = StatusProducer::PropagatedCall {
                    callee: function.id.clone(),
                }
            }
        }
        assert!(!compare(&hostile).0);
    }
}
