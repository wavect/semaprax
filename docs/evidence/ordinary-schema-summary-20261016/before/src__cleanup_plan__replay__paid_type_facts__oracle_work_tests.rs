//! Actual callback forwarding, complete old substitution/clone and receiving Drop.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleRefusal, OracleScope};

fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module work.oracle; @id(\"main\") fn main()->i64 {7}",
            "work-oracle.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn forwarding_zero_work_authenticates_without_backing_and_stale_subject_refuses() {
    let program = program();
    let other = self::program();
    let function = &program.functions[0];
    let scope = OracleScope::open(0).unwrap();
    let lease = scope.lease();
    let ((), metrics) = crate::test_allocation_observer::measure(|| {
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        let mut payer = Payer {
            function,
            budget: &mut budget,
            ledger: ClassifierLedger::capture(),
        };
        let mut forwarded = &mut payer;
        assert_eq!(
            TypeFactsPayer::oracle_work(&mut forwarded, 0, "zero forwarded work"),
            Ok(())
        );
        assert_eq!(invocation.exit(), Ok(()));
    });
    let receipt = scope.finish();
    assert_eq!((receipt.charged, receipt.work), (0, 0));
    assert_eq!(receipt.work_calls, 1);
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert!(!metrics.invalid);
    let scope = OracleScope::open(0).unwrap();
    let lease = scope.lease();
    let invocation = enter(&program, function, lease).unwrap();
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
    let mut payer = Payer {
        function: &other.functions[0],
        budget: &mut budget,
        ledger: ClassifierLedger::capture(),
    };
    let ((refused, metrics), active_refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            crate::test_allocation_observer::measure(|| {
                let error = payer.oracle_work(0, "stale work subject");
                let refused = error.is_err();
                drop(error);
                refused
            })
        });
    assert!(refused && !active_refused);
    assert_eq!((used, first), (0, None));
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert!(!metrics.invalid);
    assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
    assert_eq!(invocation.exit(), Ok(()));
    drop(payer);
    drop(budget);
    let receipt = scope.finish();
    assert_eq!((receipt.charged, receipt.work), (0, 0));
    assert!(receipt.physical_failure.is_some());
    // Reuse the expired lease itself: no replacement account or ordinary payer.
    let ((refused, metrics), active_refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            crate::test_allocation_observer::measure(|| {
                let mut budget =
                    ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                let mut payer = Payer {
                    function,
                    budget: &mut budget,
                    ledger: ClassifierLedger::capture(),
                };
                let error = payer.oracle_work(0, "expired zero work");
                let refused = error.is_err();
                drop(error);
                drop(payer);
                drop(budget);
                refused
            })
        });
    assert!(refused && !active_refused);
    assert_eq!((used, first), (0, None));
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert!(!metrics.invalid);
    // A finished scope leaves no account; the existing lease contract reports
    // Missing here. Replacing it with a new epoch exercises Expired separately.
    assert_eq!(lease.charge_work(0), Err(OracleRefusal::Missing));
    let replacement = OracleScope::open_with_work(0, 0).unwrap();
    assert_eq!(lease.charge_work(0), Err(OracleRefusal::Expired));
    let ((refused, metrics), active_refused, used, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            crate::test_allocation_observer::measure(|| {
                let mut budget =
                    ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                let mut payer = Payer {
                    function,
                    budget: &mut budget,
                    ledger: ClassifierLedger::capture(),
                };
                let error = payer.oracle_work(0, "replaced zero work");
                let refused = error.is_err();
                drop(error);
                drop(payer);
                drop(budget);
                refused
            })
        });
    assert!(refused && !active_refused);
    assert_eq!((used, first), (0, None));
    assert_eq!((metrics.allocated, metrics.live), (0, 0));
    assert!(!metrics.invalid);
    let receipt = replacement.finish();
    assert_eq!(
        (receipt.charged, receipt.work, receipt.work_calls),
        (0, 0, 0)
    );
}

#[test]
fn full_substitution_and_clone_preflight_charge_one_cumulative_account_through_drop() {
    let program = program();
    let function = &program.functions[0];
    let owner = function.id.clone();
    let template = ResolvedType::Function {
        parameters: vec![ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 0,
        }],
        result: Box::new(ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 0,
        }),
    };
    let arguments = [ResolvedType::Function {
        parameters: vec![
            ResolvedType::ArrayU8(16),
            ResolvedType::Nominal {
                declaration: owner.clone(),
                arguments: vec![ResolvedType::Bytes],
            },
        ],
        result: Box::new(ResolvedType::ArrayU8(24)),
    }];
    let original = crate::hir::substitute_type(&template, &owner, &arguments).unwrap();
    let original_payload = original.clone_payload_bytes().unwrap();
    let mut census_work = 0usize;
    let current_payload = original
        .clone_payload_bytes_with_work(&mut |units| {
            census_work = census_work.checked_add(units).unwrap();
            true
        })
        .unwrap();
    assert_eq!(current_payload, original_payload);
    assert!(census_work > 0);
    let run = |bytes, work| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let ((same, metrics), refused, used, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                let (same, metrics) = crate::test_allocation_observer::measure(|| {
                    let invocation = enter(&program, function, lease).unwrap();
                    let mut budget =
                        ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                    let result = (|| {
                        budget.prepare_builder(function)?;
                        budget.prepare_clone_cursor(function)?;
                        let current =
                            super::super::ordinary_inventory::substitute::substitute_type(
                                &template,
                                &owner,
                                &arguments,
                                function,
                                &mut budget,
                            )?;
                        let same = current == original;
                        budget.reserve_clone(function, &current, "complete cloned receiver")?;
                        let cloned = current.clone();
                        let same = same && cloned == original;
                        drop(cloned);
                        drop(current);
                        Ok::<bool, Diagnostic>(same)
                    })();
                    let same = matches!(&result, Ok(true));
                    drop(result);
                    drop(budget);
                    assert_eq!(invocation.exit(), Ok(()));
                    same
                });
                (same, metrics)
            });
        assert!(!refused);
        assert_eq!((used, first), (0, None));
        let retry = lease.charge(0);
        let receipt = scope.finish();
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert!(metrics.allocated <= receipt.charged);
        (same, retry, receipt, metrics.allocated)
    };
    let (same, retry, baseline, allocated) = run(1 << 20, 1 << 25);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert!(allocated > 0 && baseline.work > census_work);
    let (same, retry, exact, _) = run(baseline.charged, baseline.work);
    assert!(same);
    assert_eq!(retry, Ok(()));
    assert_eq!(
        (exact.charged, exact.work),
        (baseline.charged, baseline.work)
    );
    for (bytes, work) in [
        (baseline.charged - 1, baseline.work),
        (baseline.charged, baseline.work - 1),
        (0, baseline.work),
        (baseline.charged, 0),
    ] {
        let (same, retry, receipt, _) = run(bytes, work);
        assert!(
            !same,
            "work/byte refusal is not a substituted or cloned verdict"
        );
        assert_eq!(retry, Err(OracleRefusal::Sticky));
        assert!(receipt.first_refusal.is_some() || receipt.first_work_refusal.is_some());
        assert!(receipt.work <= work && receipt.charged <= bytes);
        assert!(receipt.physical_failure.is_none());
    }
}
