//! Real finite account acquisitions, independent old key bytes and output Drop.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleRefusal, OracleScope};

fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module logical.oracle; @id("main") fn main()->i64 {7}"#,
            "logical-oracle.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn forwarded_account_keys_preserve_full_old_bytes_and_charge_through_drop() {
    let program = program();
    let function = &program.functions[0];
    // Fixture and complete original receiver are independent of the account.
    let ty = ResolvedType::Function {
        parameters: vec![
            ResolvedType::ArrayU8(16),
            ResolvedType::Nominal {
                declaration: function.id.clone(),
                arguments: vec![ResolvedType::ArrayU8(24)],
            },
        ],
        result: Box::new(ResolvedType::ArrayU8(32)),
    };
    let original = ty.identity_key();
    let run = |allowance, work| {
        let scope = OracleScope::open_with_work(allowance, work).unwrap();
        let lease = scope.lease();
        let ((same, retry, metrics), refused, used, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                let ((same, retry), metrics) = crate::test_allocation_observer::measure(|| {
                    let invocation = enter(&program, function, lease).unwrap();
                    let mut budget =
                        ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                    let same = {
                        let mut payer = Payer {
                            function,
                            budget: &mut budget,
                            ledger: ClassifierLedger::capture(),
                        };
                        let mut forwarded = &mut payer;
                        assert!(TypeFactsPayer::classifier_sink_is_explicit(&forwarded));
                        let result = program
                            .declarations()
                            .classifier_key_for_test(&ty, &mut forwarded);
                        let same = matches!(&result, Ok(Some(value)) if value == &original);
                        drop(result);
                        let cleanup = droppability::oracle_key_for_test(&ty, &mut forwarded);
                        let same = same && matches!(&cleanup, Ok(value) if value == &original);
                        drop(cleanup);
                        same
                    };
                    drop(budget);
                    let retry = lease.charge(0);
                    assert_eq!(invocation.exit(), Ok(()));
                    (same, retry)
                });
                (same, retry, metrics)
            });
        assert!(!refused);
        assert_eq!(used, 0);
        assert_eq!(first, None);
        let receipt = scope.finish();
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert!(
            metrics.allocated <= receipt.charged,
            "{metrics:?} {receipt:?}"
        );
        (same, retry, receipt, metrics.allocated)
    };
    let (same, retry, baseline, allocated) = run(1 << 20, 1 << 25);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert!(allocated > 0);
    assert!(baseline.first_refusal.is_none());
    let (same, retry, exact, _) = run(baseline.charged, baseline.work);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert_eq!(exact.charged, baseline.charged);
    assert!(baseline.work > 0);
    assert_eq!(exact.work, baseline.work);
    for work in [baseline.work - 1, 0] {
        let (same, retry, receipt, _) = run(baseline.charged, work);
        assert!(
            !same,
            "work refusal cannot publish complete semantic key equality"
        );
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.first_work_refusal.is_some());
        assert_eq!(receipt.work_allowance, work);
        assert!(receipt.work <= work);
    }
    for allowance in [baseline.charged - 1, 0] {
        let (same, retry, receipt, _) = run(allowance, baseline.work);
        assert!(
            !same,
            "physical refusal cannot publish a partial semantic key"
        );
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.first_refusal.is_some());
    }
}

#[test]
fn logical_sink_refuses_stale_subject_nested_entry_and_expired_lease() {
    let program = program();
    let other = self::program();
    let function = &program.functions[0];
    let scope = OracleScope::open(1 << 20).unwrap();
    let lease = scope.lease();
    let invocation = enter(&program, function, lease).unwrap();
    assert!(enter(&program, function, lease).is_err());
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
    let mut payer = Payer {
        function: &other.functions[0],
        budget: &mut budget,
        ledger: ClassifierLedger::capture(),
    };
    let ((was_refused, metrics), refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            crate::test_allocation_observer::measure(|| {
                let result = payer.classifier_reserve_sink(1);
                let was_refused = result.is_err();
                drop(result);
                was_refused
            })
        });
    assert!(was_refused);
    assert!(!metrics.invalid);
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert!(!refused);
    assert_eq!((used, first), (0, None));
    assert_eq!(scope.receipt().charged, 0);
    let original_failure = scope.receipt().physical_failure;
    assert!(original_failure.is_some());
    assert!(payer.classifier_reserve_sink(0).is_err());
    assert_eq!(scope.receipt().physical_failure, original_failure);
    assert_eq!(invocation.exit(), Ok(()));
    drop(payer);
    drop(budget);
    drop(scope);
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
    let mut payer = Payer {
        function,
        budget: &mut budget,
        ledger: ClassifierLedger::capture(),
    };
    let ((was_refused, metrics), refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            crate::test_allocation_observer::measure(|| {
                let result = payer.classifier_reserve_sink(0);
                let was_refused = result.is_err();
                drop(result);
                was_refused
            })
        });
    assert!(was_refused);
    assert!(!metrics.invalid);
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert!(!refused);
    assert_eq!((used, first), (0, None));
}
