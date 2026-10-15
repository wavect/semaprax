use super::super::empty_renewals::tests::{function, state};
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

// The complete previous tuple constructor remains independently compiled.
fn original_copy(state: &PathState) -> ControlKey {
    (
        state.pending_failure.clone(),
        state.selected_failure.clone(),
        state.staged_copy_result.clone(),
        state.published,
        state.returned.clone(),
    )
}

fn atomic_types() -> [ResolvedType; 20] {
    [
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::ArrayU8(7),
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("clone.type.parameter"),
            index: 0,
        },
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
    ]
}

struct Receipt {
    ok: bool,
    work: usize,
    metrics: Metrics,
    refused: bool,
    used: usize,
    first: Option<ReservationRefusal>,
}

fn observe(function: &ResolvedFunction, state: &PathState, current: bool, limit: usize) -> Receipt {
    let expected = original_copy(state);
    let (((ok, work), metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = (|| {
                // Exact caller work for one incoming state: the fold begins at
                // one, then charges its full live order and renewal history.
                let units = 1usize
                    .saturating_add(state.live_order.len())
                    .saturating_add(crate::cleanup_plan::replay::renewal::retained_units(state));
                budget.charge(function, units, "all-path ownership replay")?;
                let key = if current {
                    let payload = reserve(function, state, &mut budget)?;
                    copy(payload)
                } else {
                    super::super::reserve_control_clone(function, state, &mut budget)?;
                    original_copy(state)
                };
                assert_eq!(key, expected);
                drop(key);
                Ok::<(), Diagnostic>(())
            })();
            let ok = result.is_ok();
            if let Err(error) = result {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let allocated = current_metrics().allocated;
                let Err(later) = reserve(function, state, &mut budget) else {
                    panic!("a failed builder cannot publish a control payload");
                };
                assert_eq!(later.code, "SPX-H006");
                drop(later);
                assert_eq!(current_metrics().allocated, allocated);
            }
            let work = budget.remaining;
            drop(budget);
            (ok, work)
        })
    });
    Receipt {
        ok,
        work,
        metrics,
        refused,
        used,
        first,
    }
}

fn atomic_state(ty: ResolvedType, history: bool) -> PathState {
    let mut state = state(true);
    if history {
        state.renewals.insert(
            ExpressionId::from_owned("control.history".to_owned()),
            crate::cleanup_plan::renewal_history::RenewalHistory::new(
                vec![LivenessFlagId(1)],
                None,
            ),
        );
    }
    state.staged_copy_result = Some(StagedCopyResultSource::Body {
        expression: ExpressionId::from_owned("control.body".to_owned()),
        instance: ty,
    });
    state
}

#[test]
fn atomic_body_control_clone_preserves_complete_key_and_work_in_both_measurement_orders() {
    let function = function();
    for ty in atomic_types() {
        for history in [false, true] {
            let state = atomic_state(ty.clone(), history);
            for current_first in [false, true] {
                let first = observe(&function, &state, current_first, usize::MAX);
                let second = observe(&function, &state, !current_first, usize::MAX);
                let (current, old) = if current_first {
                    (first, second)
                } else {
                    (second, first)
                };
                assert!(current.ok && old.ok && !current.refused && !old.refused);
                assert!(current.first.is_none() && old.first.is_none());
                assert_eq!(current.work, old.work);
                let scratch = (crate::cache_codec::MAX_DEPTH + 1)
                    * size_of::<Option<(&ResolvedType, usize)>>();
                assert_eq!(current.used + scratch, old.used + controls_bytes());
                assert!(current.used < old.used);
                let expected_paid = function.id.as_str().len()
                    + 512
                    + 2 * size_of::<Payload<'_>>()
                    + size_of::<Result<Payload<'_>, Diagnostic>>()
                    + size_of::<ControlKey>()
                    + size_of::<DeclarationId>();
                assert_eq!(current.used, expected_paid);
                for receipt in [&current, &old] {
                    assert!(!receipt.metrics.invalid);
                    assert_eq!(receipt.metrics.live, 0);
                    assert_eq!(receipt.metrics.allocated, receipt.metrics.freed);
                    assert!(receipt.metrics.allocated <= receipt.used);
                    // Refusal String plus exactly one returned projection Vec.
                    assert_eq!(receipt.metrics.allocations, 2);
                    assert_eq!(
                        receipt.metrics.allocated,
                        function.id.as_str().len() + 512 + size_of::<DeclarationId>()
                    );
                }
                assert_eq!(current.metrics, old.metrics);
            }
        }
    }
}

#[test]
fn atomic_control_exact_short_zero_and_every_request_frontier_remain_sticky() {
    let function = function();
    let state = atomic_state(ResolvedType::I64, true);
    let full = observe(&function, &state, true, usize::MAX);
    let exact = observe(&function, &state, true, full.used);
    assert!(exact.ok && !exact.refused);
    assert_eq!(exact.metrics, full.metrics);
    let short = observe(&function, &state, true, full.used - 1);
    assert!(!short.ok && short.refused && short.first.is_some());
    let mut limit = 0;
    for _ in 0..16 {
        let receipt = observe(&function, &state, true, limit);
        assert!(!receipt.metrics.invalid);
        assert_eq!(
            (receipt.metrics.live, receipt.metrics.allocated),
            (0, receipt.metrics.freed)
        );
        if receipt.ok {
            assert!(!receipt.refused);
            assert_eq!(receipt.used, full.used);
            return;
        }
        assert!(receipt.refused);
        let first = receipt.first.unwrap();
        let next = receipt.used + first.requested + first.floor;
        assert!(next > limit);
        limit = next;
    }
    panic!("every reservation frontier must reach the complete unchanged key");
}

#[test]
fn absent_compound_and_residual_results_and_other_payers_keep_the_original_route() {
    let function = function();
    let mut state = atomic_state(ResolvedType::I64, false);
    let mut unobserved = ReplayBudget::unobserved();
    let mut paid = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    let scope = crate::cleanup_plan::replay::allocation::OracleScope::open(1 << 20).unwrap();
    let mut oracle = ReplayBudget::with_skeleton_limit_oracle(0, scope.lease());
    for budget in [&mut unobserved, &mut paid, &mut oracle] {
        assert!(!selected(&state, budget));
        let Err(error) = reserve(&function, &state, budget) else {
            panic!("another payer cannot publish an atomic control payload");
        };
        assert_eq!(
            error,
            Diagnostic::io_static(
                "SPX-H006",
                "atomic control clone requires an ordinary builder and an atomic body result"
            )
        );
        super::super::reserve_control_clone(&function, &state, budget).unwrap();
        drop(original_copy(&state));
    }
    drop(oracle);
    let receipt = scope.finish();
    assert!(receipt.charged > 0 && receipt.first_refusal.is_none());
    assert!(receipt.first_work_refusal.is_none() && receipt.physical_failure.is_none());
    let id = DeclarationId::new("control.residual");
    let expression = ExpressionId::from_owned("control.residual.body".to_owned());
    for staged in [
        None,
        super::super::empty_renewals::tests::state(true).staged_copy_result,
        Some(StagedCopyResultSource::TryResidual {
            expression: expression.clone(),
            operand: expression.clone(),
            source_instance: ResolvedType::I64,
            target_instance: ResolvedType::I64,
            result: id.clone(),
            ok_case: id.clone(),
            ok_field: id.clone(),
            err_case: id.clone(),
            err_field: id.clone(),
        }),
        Some(StagedCopyResultSource::TryOptionNone {
            expression: expression.clone(),
            operand: expression.clone(),
            source_instance: ResolvedType::I64,
            target_instance: ResolvedType::I64,
            option: id.clone(),
            some_case: id.clone(),
            some_field: id.clone(),
            none_case: id.clone(),
        }),
    ] {
        state.staged_copy_result = staged;
        let expected = original_copy(&state);
        let (((), metrics), refused, _) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    assert!(!selected(&state, &budget));
                    super::super::reserve_control_clone(&function, &state, &mut budget).unwrap();
                    let copied = original_copy(&state);
                    assert_eq!(copied, expected);
                    drop(copied);
                    drop(budget);
                })
            });
        assert!(!refused && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}

#[test]
fn absent_statuses_and_scalar_or_absent_return_keep_exact_false_publication() {
    let function = function();
    for returned in [
        None,
        Some(ReturnedSource::Scalar(ExpressionId::from_owned(
            "control.scalar".to_owned(),
        ))),
    ] {
        let mut state = atomic_state(ResolvedType::I64, false);
        state.pending_failure = None;
        state.selected_failure = None;
        state.published = false;
        state.returned = returned;
        let old = observe(&function, &state, false, usize::MAX);
        let current = observe(&function, &state, true, usize::MAX);
        assert!(old.ok && current.ok && !old.refused && !current.refused);
        assert_eq!(current.work, old.work);
        assert_eq!(current.metrics, old.metrics);
        assert!(!current.metrics.invalid);
        assert_eq!(current.metrics.live, 0);
        assert_eq!(current.metrics.allocated, current.metrics.freed);
        assert_eq!(current.metrics.allocations, 1);
        assert_eq!(current.metrics.allocated, function.id.as_str().len() + 512);
        assert_eq!(
            current.used,
            function.id.as_str().len()
                + 512
                + 2 * size_of::<Payload<'_>>()
                + size_of::<Result<Payload<'_>, Diagnostic>>()
                + size_of::<ControlKey>()
        );
    }
}

#[test]
fn selected_admission_with_an_active_builder_still_cannot_select_the_ordinary_constructor() {
    let function = function();
    let state = atomic_state(ResolvedType::I64, false);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        budget.admission = AdmissionMode::SelectedRetained;
        assert!(!selected(&state, &budget));
        let Err(error) = reserve(&function, &state, &mut budget) else {
            panic!("selected admission cannot publish the ordinary control payload");
        };
        assert_eq!(
            error,
            Diagnostic::io_static(
                "SPX-H006",
                "atomic control clone requires an ordinary builder and an atomic body result"
            )
        );
        super::super::reserve_control_clone(&function, &state, &mut budget).unwrap();
        drop(original_copy(&state));
    });
    assert!(!refused);
}
