//! Root classification before outer carriers; saved paid and original callbacks
//! remain independent. Physical accounting is separate from conservative debit.
use super::super::{
    tests::{fixtures, payer},
    ReplayBudget, ResolvedProgram,
};
use super::*;
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::hir::TypeFacts;
use crate::test_allocation_observer::{current_metrics, measure};

fn observe(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    limit: usize,
    saved: bool,
) -> (
    ((), crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    let expected = crate::cleanup::type_needs_resource_cleanup(program, ty);
    let function = &program.functions[0];
    with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let run = |budget: &mut ReplayBudget| {
                let mut held = payer(function, budget);
                if saved {
                    super::saved_paid::needs_drop(program, ty, &mut held)
                } else {
                    super::needs_drop_paid(program, ty, &mut held)
                }
            };
            let result = run(&mut budget);
            if crate::bounded_output::active_refused() {
                assert!(matches!(result, Err(Failure::Physical(_))));
                let before = active_remaining();
                assert!(!reserve_active_required(0));
                let start = current_metrics();
                drop(run(&mut budget));
                let end = current_metrics();
                assert_eq!(start.allocated, end.allocated);
                assert_eq!(start.allocations, end.allocations);
                assert_eq!(before, active_remaining());
            } else {
                match (&result, &expected) {
                    (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
                    (Err(Failure::Semantic(actual)), Err(expected)) => assert_eq!(actual, expected),
                    _ => panic!("root disagrees with complete original callback"),
                }
            }
            drop(result);
            drop(budget);
        })
    })
}

#[test]
fn root_first_full_cached_hostile_leaf_parity_exact_short_zero_sticky_and_drop() {
    let program = fixtures().into_iter().next().unwrap();
    let owner = program.functions[0].id.clone();
    let roots = vec![
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::ArrayU8(37),
        ResolvedType::Bytes,
        ResolvedType::String,
        ResolvedType::StringMap,
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::MutFunctionI64,
        ResolvedType::Function {
            parameters: vec![ResolvedType::Bytes; 128],
            result: Box::new(ResolvedType::I64),
        },
        ResolvedType::TypeParameter {
            owner,
            index: u32::MAX,
        },
    ];
    for ty in roots {
        // None from an uncached generic root, natural facts, and both hostile
        // cache outcomes all keep the independent original diagnostic precedence.
        for cached in [None, Some(false), Some(true)] {
            let mut changed = program.clone();
            if let Some(needs_drop) = cached {
                changed.declarations.classifier_cache_for_test(
                    &ty,
                    TypeFacts {
                        copy: false,
                        contains_resource: true,
                        sized: false,
                        needs_drop,
                        layout_key: "root-hostile-cache-precedence".to_owned(),
                    },
                );
            }
            super::tests::parity(&changed, &ty);
            let (((), old), refused, old_used, _) = observe(&changed, &ty, usize::MAX, true);
            assert!(!refused && !old.invalid && old.live == 0);
            assert_eq!(old.allocated, old.freed);
            let (((), wide), refused, acquired, _) = observe(&changed, &ty, usize::MAX, false);
            assert!(!refused && acquired < old_used);
            assert!(!wide.invalid && wide.live == 0 && wide.allocated == wide.freed);
            assert!(wide.allocated <= acquired && wide.highwater <= acquired);
            // Leaf roots cannot allocate a traversal carrier; any remaining
            // allocation belongs to paid classifier or exact semantic text.
            assert!(wide.allocated <= old.allocated);
            let expected_zero = match &ty {
                // These exact built-in leaf roots construct no classifier or
                // outer traversal carrier. Hostile true Copy facts instead
                // require the original semantic error key/text constructors.
                ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool => cached != Some(true),
                ResolvedType::Bytes
                | ResolvedType::String
                | ResolvedType::StringMap
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::MutFunctionI64 => true,
                ResolvedType::ArrayU8(37) => cached == Some(false),
                ResolvedType::TypeParameter {
                    index: u32::MAX, ..
                } => cached.is_some(),
                ResolvedType::Function { parameters, .. } => {
                    // The literal 128-child query exceeds the borrowed cache
                    // census and must retain the complete paid constructor.
                    assert_eq!(parameters.len(), 128);
                    false
                }
                _ => panic!("new root requires an independent acquisition expectation"),
            };
            if expected_zero {
                assert_eq!(acquired, 0, "known absent constructors: {ty:?}/{cached:?}");
                // A real fast route constructs nothing: healthy zero only
                // authenticates the existing ledger, so every limit admits it.
                assert_eq!(wide.allocated, 0);
                let (((), zero), refused, used, first) = observe(&changed, &ty, 0, false);
                assert!(
                    !refused
                        && first.is_none()
                        && used == 0
                        && zero.allocated == 0
                        && zero.live == 0
                );
                continue;
            }
            assert!(
                acquired > 0,
                "original required constructors: {ty:?}/{cached:?}"
            );
            let (((), exact), refused, used, _) = observe(&changed, &ty, acquired, false);
            assert!(!refused && used == acquired && !exact.invalid && exact.live == 0);
            assert_eq!(exact.allocated, exact.freed);
            let (((), short), refused, _, first) = observe(&changed, &ty, acquired - 1, false);
            assert!(refused && first.is_some() && !short.invalid && short.live == 0);
            assert_eq!(short.allocated, short.freed);
            let (((), zero), refused, used, first) = observe(&changed, &ty, 0, false);
            assert!(
                refused && first.is_some() && used == 0 && zero.allocated == 0 && zero.live == 0
            );
        }
    }
}

#[test]
fn root_first_never_zero_touch_preserves_sticky_refusal_without_carrier() {
    let program = fixtures().into_iter().next().unwrap();
    super::tests::parity(&program, &ResolvedType::Never);
    let (((), metrics), refused, used, _) = observe(&program, &ResolvedType::Never, 0, false);
    assert!(!refused && used == 0 && metrics.allocated == 0 && metrics.live == 0);
    let function = &program.functions[0];
    let (((), metrics), refused, used, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            assert!(!reserve_active_required(1));
            let before = active_remaining();
            let mut budget = ReplayBudget::new();
            let result = super::needs_drop_paid(
                &program,
                &ResolvedType::Never,
                &mut payer(function, &mut budget),
            );
            assert!(matches!(result, Err(Failure::Physical(_))));
            assert_eq!(before, active_remaining());
            drop(result);
            drop(budget);
        })
    });
    assert!(refused && first.is_some() && used == 0 && metrics.allocated == 0 && metrics.live == 0);
}

#[test]
fn root_first_nominal_true_retains_complete_paid_traversal_without_double_charge() {
    for mut program in fixtures() {
        let roots: Vec<_> = program
            .types
            .iter()
            .map(|item| ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            })
            .collect();
        for ty in roots {
            program.declarations.classifier_cache_for_test(
                &ty,
                TypeFacts {
                    copy: false,
                    contains_resource: true,
                    sized: false,
                    needs_drop: true,
                    layout_key: "nominal-retains-original-walk".to_owned(),
                },
            );
            super::tests::parity(&program, &ty);
            let (((), old), refused, acquired, _) = observe(&program, &ty, usize::MAX, true);
            assert!(!refused && acquired > 0 && !old.invalid && old.live == 0);
            let (((), current), refused, used, _) = observe(&program, &ty, usize::MAX, false);
            assert!(!refused && used == acquired);
            assert_eq!(old, current, "nominal storage and Drop remain complete");
            let (((), exact), refused, used, _) = observe(&program, &ty, acquired, false);
            assert!(!refused && used == acquired && !exact.invalid && exact.live == 0);
            assert_eq!(exact.allocated, exact.freed);
            let (((), short), refused, _, first) = observe(&program, &ty, acquired - 1, false);
            assert!(refused && first.is_some() && !short.invalid && short.live == 0);
            assert_eq!(short.allocated, short.freed);
            let (((), zero), refused, used, first) = observe(&program, &ty, 0, false);
            assert!(refused && first.is_some() && used == 0 && zero.allocated == 0);
        }
    }
}
