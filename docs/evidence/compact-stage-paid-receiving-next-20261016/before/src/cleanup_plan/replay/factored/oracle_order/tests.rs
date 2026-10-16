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
            let x = CopySource::new(function, &mut construction, (*a).clone()).unwrap();
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
