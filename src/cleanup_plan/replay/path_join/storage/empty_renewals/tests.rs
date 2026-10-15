use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

pub(in crate::cleanup_plan::replay::path_join::storage) fn function() -> ResolvedFunction {
    let parsed = crate::check(
        "module clone; @id(\"main\") fn main()->i64 {0}",
        "clone.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed).unwrap().functions.remove(0)
}

pub(in crate::cleanup_plan::replay::path_join::storage) fn state(
    previously_populated: bool,
) -> PathState {
    let owner = crate::hir::FunctionExecutionId::Monomorphic(DeclarationId::new("clone.owner"));
    let expression = ExpressionId::new(&owner, "body.clone");
    let shared = DeclarationId::new("clone.case:é\0");
    let place = CleanupPlace {
        storage: StorageId::ProvisionalResult,
        projections: vec![shared.clone()],
    };
    let mut renewals = BTreeMap::new();
    if previously_populated {
        renewals.insert(
            expression.clone(),
            crate::cleanup_plan::renewal_history::RenewalHistory::new(
                vec![LivenessFlagId(1)],
                None,
            ),
        );
        drop(renewals.pop_first().unwrap());
    }
    PathState {
        renewals,
        live_order: vec![LivenessFlagId(1), LivenessFlagId(2)],
        conditional_variants: vec![ReplayConditionalVariant {
            root: place.clone(),
            variant: shared.clone(),
            cases: vec![(shared.clone(), vec![LivenessFlagId(3), LivenessFlagId(4)])],
        }],
        pending_failure: Some(StatusSourceId {
            expression: expression.clone(),
            lane: StatusLane::ContractFalse,
        }),
        selected_failure: Some(StatusSourceId {
            expression: ExpressionId::new(&owner, "body.selected"),
            lane: StatusLane::ContractFalse,
        }),
        staged_copy_result: Some(StagedCopyResultSource::Body {
            expression,
            instance: ResolvedType::Function {
                parameters: vec![ResolvedType::Nominal {
                    declaration: shared,
                    arguments: vec![ResolvedType::I64],
                }],
                result: Box::new(ResolvedType::I64),
            },
        }),
        published: true,
        returned: Some(ReturnedSource::Owned(place)),
    }
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
    let (((ok, work), metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = (|| {
                // This is the unchanged real caller's request, before either
                // physical clone receiver is selected.
                budget.charge(
                    function,
                    crate::cleanup_plan::replay::renewal::retained_units(state),
                    "renewal branch history clone",
                )?;
                let copied = if current {
                    assert!(selected(state, &budget));
                    let payload = reserve(function, state, &mut budget, "ownership branch clone")?;
                    copy(payload)
                } else {
                    super::super::reserve_state_clone(
                        function,
                        state,
                        &mut budget,
                        "ownership branch clone",
                    )?;
                    state.clone()
                };
                assert_eq!(&copied, state);
                drop(copied);
                Ok::<(), Diagnostic>(())
            })();
            let ok = result.is_ok();
            if let Err(error) = result {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let before = current_metrics().allocated;
                let Err(later) = reserve(function, state, &mut budget, "ownership branch clone")
                else {
                    panic!("a failed builder cannot publish another payload");
                };
                assert_eq!(later.code, "SPX-H006");
                drop(later);
                assert_eq!(current_metrics().allocated, before);
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

#[test]
fn empty_renewal_branch_clone_preserves_every_field_work_and_complete_drop() {
    let function = function();
    for previously_populated in [false, true] {
        let state = state(previously_populated);
        // Both orders retain the same immutable input and observe each complete
        // producer through its result and budget's final Drop.
        for current_first in [false, true] {
            let first = observe(&function, &state, current_first, usize::MAX);
            let second = observe(&function, &state, !current_first, usize::MAX);
            let (current, old) = if current_first {
                (first, second)
            } else {
                (second, first)
            };
            for receipt in [&current, &old] {
                assert!(receipt.ok && !receipt.refused && receipt.first.is_none());
                assert!(!receipt.metrics.invalid);
                assert_eq!(receipt.metrics.live, 0);
                assert_eq!(receipt.metrics.allocated, receipt.metrics.freed);
                assert!(receipt.metrics.allocated <= receipt.used);
            }
            assert_eq!(current.work, old.work);
            let root = super::super::super::super::physical_storage::btree_node_upper::<
                ExpressionId,
                crate::cleanup_plan::renewal_history::RenewalHistory,
            >()
            .unwrap();
            assert_eq!(current.used + root, old.used + controls_bytes());
            assert!(current.used < old.used);
            // Independent requested-layout oracle: one refusal String, five
            // ownership/variant Vecs, three type buffers and returned projections.
            let field_bytes = 4 * size_of::<LivenessFlagId>()
                + size_of::<ReplayConditionalVariant>()
                + 2 * size_of::<DeclarationId>()
                + size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
                + 3 * size_of::<ResolvedType>();
            let expected = field_bytes + function.id.as_str().len() + 512;
            assert_eq!(current.metrics.allocations, 10);
            assert_eq!(current.metrics.allocated, expected);
            assert!(current.metrics.allocations <= old.metrics.allocations);
            assert!(current.metrics.allocated <= old.metrics.allocated);
            assert_eq!(current.metrics.reallocations, old.metrics.reallocations);
        }
    }
}

#[test]
fn empty_renewal_clone_exact_short_zero_and_each_request_frontier_remain_sticky() {
    let function = function();
    let state = state(true);
    let complete = observe(&function, &state, true, usize::MAX);
    assert!(complete.ok && !complete.refused && complete.used > 0);
    let exact = observe(&function, &state, true, complete.used);
    assert!(exact.ok && !exact.refused);
    assert_eq!(exact.used, complete.used);
    assert_eq!(exact.metrics, complete.metrics);
    let short = observe(&function, &state, true, complete.used - 1);
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
            assert_eq!(receipt.used, complete.used);
            return;
        }
        assert!(receipt.refused);
        let first = receipt.first.unwrap();
        let next = receipt.used + first.requested + first.floor;
        assert!(next > limit);
        limit = next;
    }
    panic!("every reservation frontier must lead to the complete unchanged clone");
}

#[test]
fn atomic_type_clone_omits_only_the_absent_census_cursor_and_preserves_all_tags() {
    let function = function();
    let mut state = state(true);
    for ty in [
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
    ] {
        assert!(clone_leaf(&ty));
        state.staged_copy_result = Some(StagedCopyResultSource::Body {
            expression: ExpressionId::from_owned("atomic.clone".to_owned()),
            instance: ty,
        });
        let old = observe(&function, &state, false, usize::MAX);
        let current = observe(&function, &state, true, usize::MAX);
        assert!(old.ok && current.ok && !old.refused && !current.refused);
        assert_eq!(current.work, old.work);
        let scratch =
            (crate::cache_codec::MAX_DEPTH + 1) * size_of::<Option<(&ResolvedType, usize)>>();
        let root = super::super::super::super::physical_storage::btree_node_upper::<
            ExpressionId,
            crate::cleanup_plan::renewal_history::RenewalHistory,
        >()
        .unwrap();
        assert_eq!(current.used + root + scratch, old.used + controls_bytes());
        assert_eq!(current.metrics.allocations, 7);
        assert!(!current.metrics.invalid);
        assert_eq!(current.metrics.live, 0);
        assert_eq!(current.metrics.allocated, current.metrics.freed);
        let exact = observe(&function, &state, true, current.used);
        assert!(exact.ok && !exact.refused);
        assert_eq!(exact.metrics, current.metrics);
        let short = observe(&function, &state, true, current.used - 1);
        assert!(!short.ok && short.refused && short.first.is_some());
    }
}

#[test]
fn nonempty_and_selected_or_unobserved_renewal_routes_keep_the_original_receiver() {
    let function = function();
    let mut state = state(false);
    let mut unobserved = ReplayBudget::unobserved();
    let mut paid = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    for budget in [&mut unobserved, &mut paid] {
        assert!(!selected(&state, budget));
        let Err(error) = reserve(&function, &state, budget, "closed route") else {
            panic!("a different replay route cannot publish this payload");
        };
        assert_eq!(
            error,
            Diagnostic::io_static(
                "SPX-H006",
                "empty renewal clone requires an ordinary builder and an empty map"
            )
        );
        super::super::reserve_state_clone(&function, &state, budget, "original clone").unwrap();
        assert_eq!(state.clone(), state);
    }
    state.renewals.insert(
        ExpressionId::from_owned("nonempty".to_owned()),
        crate::cleanup_plan::renewal_history::RenewalHistory::new(Vec::new(), None),
    );
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        assert!(!selected(&state, &budget));
        assert!(payload_bytes(&state).is_none());
        super::super::reserve_state_clone(
            &function,
            &state,
            &mut budget,
            "original nonempty clone",
        )
        .unwrap();
        assert_eq!(state.clone(), state);
    });
    assert!(!refused);
}
