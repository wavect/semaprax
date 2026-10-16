use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleRefusal, OracleScope};
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module oracle.order; @id(\"oracle.order.main\") fn main()->i64 {0}",
            "oracle-order.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn compact_staged_order_preserves_canonical_fields_work_and_refusals() {
    use super::super::super::compact_stage::CopySource;
    let checked = crate::check(
        r#"module oracle.staged;
@id("result_path") fn result_path(value: Result<i64, i64>) -> Result<i64, i64> {
    let payload = value?;
    Result<i64, i64>::Ok { value: payload }
}
@id("option_path") fn option_path(value: Option<i64>) -> Option<i64> {
    let payload = value?;
    Option<i64>::Some { value: payload }
}
@id("main") fn main() -> i64 { 0 }
"#,
        "oracle-staged.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = &program.functions[0];
    let sources: Vec<_> = program
        .functions
        .iter()
        .flat_map(|function| &function.cleanup_plan.blocks)
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::StageCopyResult { source } => Some(source),
            _ => None,
        })
        .collect();
    for ordinal in 0..3 {
        assert!(sources
            .iter()
            .any(|source| staged_ordinal(source) == ordinal));
    }
    let mut construction = ReplayBudget::new();
    for a in &sources {
        for b in &sources {
            super::super::super::compact_stage::prepare!(function, &mut construction, *a, unwrap);
            let x = CopySource::new(function, &mut construction, (*a).clone()).unwrap();
            super::super::super::compact_stage::prepare!(function, &mut construction, *b, unwrap);
            let y = CopySource::new(function, &mut construction, (*b).clone()).unwrap();
            for work in [0, 1, 2, MAX_REPLAY_WORK_UNITS] {
                let mut original = ReplayBudget::new();
                let mut compact = ReplayBudget::new();
                original.remaining = work;
                compact.remaining = work;
                let expected = staged(a, b, function, &mut original);
                let actual = compact_staged(&x, &y, function, &mut compact);
                assert_eq!(actual, expected);
                assert_eq!(compact.remaining, original.remaining);
                assert_eq!(compact.skeleton_remaining, original.skeleton_remaining);
                if let Ok(order) = actual {
                    assert_eq!(order, a.cmp(b));
                }
            }
        }
    }
}

fn types() -> Vec<ResolvedType> {
    use ResolvedType::*;
    let mut values = vec![
        Never,
        OnceFunction,
        OnceFunctionI64,
        OnceFunctionI64Pair,
        MutFunctionI64,
        Function {
            parameters: Vec::new(),
            result: Box::new(Unit),
        },
        Unit,
        I64,
        I32,
        Char,
        U8,
        Usize,
        ArrayU8(0),
        ArrayU8(17),
        F32,
        F64,
        Bool,
        String,
        Bytes,
        Str,
        SliceU8,
        StringMap,
        TypeParameter {
            owner: DeclarationId::new("owner\0é"),
            index: 0,
        },
        TypeParameter {
            owner: DeclarationId::new("owner\0é"),
            index: 17,
        },
        Nominal {
            declaration: DeclarationId::new("nominal\0é"),
            arguments: Vec::new(),
        },
    ];
    for width in [0, 1, 17] {
        values.push(Function {
            parameters: vec![I64; width],
            result: Box::new(Bytes),
        });
        values.push(Nominal {
            declaration: DeclarationId::new("nominal\0é"),
            arguments: vec![
                Function {
                    parameters: vec![I64, Bytes],
                    result: Box::new(Unit)
                };
                width
            ],
        });
    }
    let mut deep = Bytes;
    for _ in 0..32 {
        deep = Nominal {
            declaration: DeclarationId::new("deep"),
            arguments: vec![deep],
        };
    }
    values.push(deep);
    values
}
#[test]
fn paid_type_order_matches_full_derived_order_all_variants_arguments_and_utf8() {
    let program = fixture();
    let function = &program.functions[0];
    let values = types();
    let ((matched, receipt), observation) = measure(|| {
        let scope = OracleScope::open_with_work(1 << 20, 1 << 25).unwrap();
        let lease = scope.lease();
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        let mut matched = true;
        for a in &values {
            for b in &values {
                matched &= resolved_type(a, b, function, &mut budget).unwrap() == a.cmp(b);
            }
        }
        drop(budget);
        assert_eq!(invocation.exit(), Ok(()));
        (matched, scope.finish())
    });
    assert!(matched && !observation.invalid);
    assert_eq!(observation.live, 0);
    assert_eq!(observation.allocated, observation.freed);
    assert!(observation.allocated <= receipt.charged);
    assert!(
        receipt.first_refusal.is_none()
            && receipt.first_work_refusal.is_none()
            && receipt.physical_failure.is_none()
    );
}

#[test]
fn deep_paid_type_order_exact_short_zero_retains_first_refusal_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let values = types();
    let ty = values.last().unwrap();
    let run = |bytes, work| {
        let ((result, retry, receipt), observed) = measure(|| {
            let scope = OracleScope::open_with_work(bytes, work).unwrap();
            let lease = scope.lease();
            let invocation = enter(&program, function, lease).unwrap();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
            let result = resolved_type(ty, ty, function, &mut budget);
            let value = result.as_ref().ok().copied();
            drop(result);
            drop(budget);
            let retry = lease.charge(0);
            assert_eq!(invocation.exit(), Ok(()));
            (value, retry, scope.finish())
        });
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(observed.allocated <= receipt.charged);
        (result, retry, receipt)
    };
    let (value, retry, ample) = run(1 << 20, 1 << 25);
    assert_eq!(value, Some(Ordering::Equal));
    assert_eq!(retry, Ok(()));
    assert!(ample.charged > 0 && ample.work > 0);
    let (value, retry, exact) = run(ample.charged, ample.work);
    assert_eq!(value, Some(Ordering::Equal));
    assert_eq!(retry, Ok(()));
    assert_eq!((exact.charged, exact.work), (ample.charged, ample.work));
    for (bytes, work) in [
        (0, ample.work),
        (ample.charged - 1, ample.work),
        (ample.charged, 0),
        (ample.charged, ample.work - 1),
    ] {
        let (value, retry, receipt) = run(bytes, work);
        assert_eq!(value, None);
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(
            receipt.first_refusal.is_some()
                || receipt.first_work_refusal.is_some()
                || receipt.physical_failure.is_some()
        );
    }
}

#[test]
fn compact_staged_receiving_keeps_floor_and_foreign_owner_first_refusal() {
    use crate::bounded_output::{
        active_remaining, set_active_floor, with_limit_usage_refusal_site,
    };
    use crate::cleanup_plan::replay::compact_stage::{self, CopySource};
    // Real non-body data: both the constructor's Box backing and the residual
    // comparator are reached, rather than an empty/ordinal-only alternative.
    let program = crate::hir::resolve(
        &crate::check(
            r#"module compact.hostile;
@id("compact.try") fn route(value:Result<i64,i64>)->Result<i64,i64> {
    let payload=value?;
    Result<i64,i64>::Ok {value:payload}
}
@id("compact.main") fn main()->i64 {0}
"#,
            "compact-hostile.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "compact.try")
        .unwrap();
    let source = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::StageCopyResult { source }
                if matches!(source, StagedCopyResultSource::TryResidual { .. }) =>
            {
                Some(source)
            }
            _ => None,
        })
        .expect("real Result residual source");
    let foreign_program = program.clone();
    let foreign_function = foreign_program
        .functions
        .iter()
        .find(|f| f.id == function.id)
        .unwrap();
    let phases = [
        "compact staged-source caller query controls",
        "compact staged-source residual backing",
        "compact staged comparison query controls",
    ];
    for (attack, phase) in phases.into_iter().enumerate() {
        let expected = replay_error(
            function,
            format_args!("cleanup replay physical reservation exhausted during {phase}"),
        );
        let ((remaining, allocation), refused, _, first, site) =
            with_limit_usage_refusal_site(1 << 20, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    budget.prepare_builder(function).unwrap();
                    let owned =
                        skeleton_clone(&mut budget, function, source, "floor source").unwrap();
                    if attack != 0 {
                        compact_stage::prepare!(function, &mut budget, &owned, unwrap);
                    }
                    let (compact, second) = if attack == 2 {
                        let compact = CopySource::new(function, &mut budget, owned).unwrap();
                        let owned =
                            skeleton_clone(&mut budget, function, source, "floor second source")
                                .unwrap();
                        compact_stage::prepare!(function, &mut budget, &owned, unwrap);
                        let second = CopySource::new(function, &mut budget, owned).unwrap();
                        (compact, second)
                    } else {
                        // Keep the owned parameter alive until the actual selected
                        // constructor branch below, with its complete clone paid.
                        let remaining = active_remaining().unwrap();
                        assert!(remaining > 0 && set_active_floor(remaining));
                        let error = if attack == 0 {
                            (|| {
                                compact_stage::prepare!(function, &mut budget, &owned);
                                CopySource::new(function, &mut budget, owned)
                            })()
                            .err()
                            .expect("query refuses before owned forwarding")
                        } else {
                            CopySource::new(function, &mut budget, owned)
                                .err()
                                .expect("real residual backing refuses")
                        };
                        assert_eq!(error, expected, "all six Diagnostic fields");
                        assert_eq!(budget.remaining, 0);
                        assert_eq!(
                            budget.reserve_bytes(function, 0, "floor retry"),
                            Err(Diagnostic::io("SPX-H006", String::new()))
                        );
                        drop(error);
                        drop(budget);
                        return remaining;
                    };
                    let remaining = active_remaining().unwrap();
                    assert!(remaining > 0 && set_active_floor(remaining));
                    let work = budget.remaining;
                    let error =
                        compact_staged(&compact, &second, function, &mut budget).unwrap_err();
                    assert_eq!(error, expected, "all six Diagnostic fields");
                    assert!(work > 0);
                    assert_eq!(budget.remaining, 0);
                    assert_eq!(
                        budget.reserve_bytes(function, 0, "floor retry"),
                        Err(Diagnostic::io("SPX-H006", String::new()))
                    );
                    drop((error, compact, second, budget));
                    remaining
                })
            });
        assert!(refused);
        let first = first.expect("actual positive required reservation");
        assert_eq!((first.remaining, first.floor), (remaining, remaining));
        assert!(
            first.requested > 0 && first.requested <= remaining,
            "floor crossing, not an absolute byte exhaustion"
        );
        let site = site.expect("real first refusal site survives zero retry");
        eprintln!("compact floor attack={attack} first={first:?} site={site}");
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }

    // A prior work refusal stays before the new comparison bank even when all
    // currently remaining bytes are reserved. Compare the complete old method,
    // not a synthetic expected text or a call back into compact_staged.
    let (((), allocation), refused, _, first, site) =
        with_limit_usage_refusal_site(1 << 20, || {
            measure(|| {
                let mut old = ReplayBudget::new();
                let mut current = ReplayBudget::new();
                old.prepare_builder(function).unwrap();
                current.prepare_builder(function).unwrap();
                let owned = skeleton_clone(&mut current, function, source, "work source").unwrap();
                compact_stage::prepare!(function, &mut current, &owned, unwrap);
                let compact = CopySource::new(function, &mut current, owned).unwrap();
                old.remaining = 0;
                current.remaining = 0;
                assert!(set_active_floor(active_remaining().unwrap()));
                let expected = staged(source, source, function, &mut old);
                let actual = compact_staged(&compact, &compact, function, &mut current);
                assert!(expected.is_err());
                assert_eq!(actual, expected, "complete original work-first Diagnostic");
                assert_eq!(current.remaining, old.remaining);
                drop((actual, expected, compact, old, current));
            })
        });
    assert!(
        !refused && first.is_none() && site.is_none(),
        "work failure is not fabricated as a physical floor receipt"
    );
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);

    let foreign_expected = replay_error(function, format_args!("ordinary builder already failed"));
    for old in [true, false] {
        let ((charged, allocation), refused, paid, first, site) =
            with_limit_usage_refusal_site(1 << 20, || {
                measure(|| {
                    let original_scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
                    let lease = original_scope.lease();
                    let invocation = enter(&program, function, lease).unwrap();
                    let mut budget =
                        ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                    budget.prepare_builder(function).unwrap();
                    let owned =
                        skeleton_clone(&mut budget, function, source, "foreign source").unwrap();
                    compact_stage::prepare!(function, &mut budget, &owned, unwrap);
                    let compact = CopySource::new(function, &mut budget, owned).unwrap();
                    assert_eq!(invocation.exit(), Ok(()));
                    let original = original_scope.finish();
                    assert!(
                        original.first_refusal.is_none() && original.physical_failure.is_none()
                    );
                    let foreign_scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
                    let foreign =
                        enter(&foreign_program, foreign_function, foreign_scope.lease()).unwrap();
                    let before = foreign_scope.receipt();
                    let error = if old {
                        staged(source, source, function, &mut budget)
                    } else {
                        compact_staged(&compact, &compact, function, &mut budget)
                    }
                    .unwrap_err();
                    assert_eq!(
                        error, foreign_expected,
                        "expired payer selects the complete original Diagnostic"
                    );
                    assert_eq!(budget.remaining, 0);
                    assert_eq!(
                        foreign_scope.receipt(),
                        before,
                        "no foreign byte/work/attempt debit"
                    );
                    // Constructor admission after that failure also refuses; no
                    // new source parameter/Box may be acquired with the stale lease.
                    let retry = (|| {
                        compact_stage::prepare!(function, &mut budget, source);
                        Ok::<_, Diagnostic>(())
                    })();
                    assert_eq!(retry, Err(Diagnostic::io("SPX-H006", String::new())));
                    assert_eq!(foreign_scope.receipt(), before);
                    assert_eq!(foreign.exit(), Ok(()));
                    assert_eq!(foreign_scope.finish(), before);
                    // Restoring the enclosing production scope cannot revive the
                    // expired oracle authority or its already-consumed refusal.
                    assert_eq!(
                        budget.reserve_bytes(function, 0, "restored scope retry"),
                        Err(Diagnostic::io("SPX-H006", String::new()))
                    );
                    drop((error, retry, compact, budget));
                    original.charged
                })
            });
        assert!(
            !refused && paid == 0 && first.is_none() && site.is_none(),
            "oracle failure must not fabricate a production receipt/site"
        );
        assert!(allocation.allocated <= charged);
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}
