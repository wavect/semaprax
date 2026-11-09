//! New cuts belong to the new operation stream; old work cuts are independent.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleReceipt, OracleRefusal, OracleScope};
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module flat.identity; @id(\"flat.identity.main\") fn main()->i64 {0}",
            "flat-identity.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn run(
    program: &ResolvedProgram,
    a: &[Rc<SkeletonObservation>],
    b: &[Rc<SkeletonObservation>],
    old: bool,
    bytes: usize,
    work: usize,
    expected: Result<Ordering, &Diagnostic>,
) -> OracleReceipt {
    let function = &program.functions[0];
    let (receipt, observed) = measure(|| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let invocation = enter(program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        let actual = if old {
            before_flat_identity::items(a, b, function, &mut budget)
        } else {
            items(a, b, function, &mut budget)
        };
        assert_eq!(
            actual.as_ref().copied(),
            expected,
            "complete six-field Diagnostic"
        );
        let refused = actual.is_err();
        drop(actual);
        if refused {
            assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
            assert_eq!(
                budget.reserve_bytes(function, 0, "identity sticky retry"),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
        }
        drop(budget);
        assert_eq!(invocation.exit(), Ok(()));
        scope.finish()
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(
        observed.allocated <= receipt.charged,
        "old={old} observed={observed:?} complete_reference={receipt:?}"
    );
    receipt
}

#[test]
fn same_rc_and_equal_separate_rc_keep_order_with_new_exact_and_short_cuts() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("flat\0é.long.identity".to_owned()),
        value: false,
    });
    let y = Rc::new((*x).clone());
    let z = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("flat\0é.long.identity".to_owned()),
        value: true,
    });
    for (a, b, expected) in [
        (vec![x.clone()], vec![x.clone()], Ordering::Equal),
        (vec![x.clone()], vec![y.clone()], Ordering::Equal),
        (vec![x.clone()], vec![z.clone()], Ordering::Less),
        (vec![z.clone()], vec![x.clone()], Ordering::Greater),
        (vec![x.clone()], vec![x.clone(), y.clone()], Ordering::Less),
    ] {
        for old in [true, false] {
            let ample = run(&program, &a, &b, old, 1 << 20, 1 << 20, Ok(expected));
            let exact = run(
                &program,
                &a,
                &b,
                old,
                ample.charged,
                ample.work,
                Ok(expected),
            );
            assert_eq!((exact.charged, exact.work), (ample.charged, ample.work));
            let empty = Diagnostic::io("SPX-H006", String::new());
            run(&program, &a, &b, old, 0, ample.work, Err(&empty));
            let zero = replay_error(
                function,
                "oracle account work allowance refused during oracle flat observation member",
            );
            run(&program, &a, &b, old, ample.charged, 0, Err(&zero));
        }
    }
    // A same-Rc hit replaces the real byte walk; a miss pays one new operation
    // and still executes every old field. No cache or derived comparator does it.
    let same = run(
        &program,
        &[x.clone()],
        &[x.clone()],
        false,
        1 << 20,
        1 << 20,
        Ok(Ordering::Equal),
    );
    let separate = run(
        &program,
        &[x.clone()],
        &[y.clone()],
        false,
        1 << 20,
        1 << 20,
        Ok(Ordering::Equal),
    );
    let old = run(
        &program,
        &[x.clone()],
        &[x.clone()],
        true,
        1 << 20,
        1 << 20,
        Ok(Ordering::Equal),
    );
    assert_eq!(same.work, 3); // original member, pointer operation, length
    assert_eq!(separate.work, old.work + 1);
    assert!(same.work < old.work);
    let byte_short = replay_error(
        function,
        "cleanup replay physical reservation exhausted during oracle flat identity receivers",
    );
    run(
        &program,
        &[x.clone()],
        &[x.clone()],
        false,
        same.charged - 1,
        same.work,
        Err(&byte_short),
    );
    let work_short = replay_error(
        function,
        "oracle account work allowance refused during oracle flat observation length",
    );
    run(
        &program,
        &[x.clone()],
        &[x],
        false,
        same.charged,
        same.work - 1,
        Err(&work_short),
    );
}

#[test]
fn identity_keeps_original_zero_receiver_work_failure_before_new_bank() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("floor.identity".to_owned()),
        value: false,
    });
    let expected = replay_error(
        function,
        "cleanup replay work budget exhausted during oracle flat observation member",
    );
    for old in [true, false] {
        let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let lease = scope.lease();
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        budget.prepare_builder(function).unwrap();
        budget.remaining = 0;
        let before = scope.receipt().charged;
        let actual = if old {
            before_flat_identity::items(&[x.clone()], &[x.clone()], function, &mut budget)
        } else {
            items(&[x.clone()], &[x.clone()], function, &mut budget)
        };
        assert_eq!(actual.as_ref().err(), Some(&expected));
        assert_eq!(scope.receipt().charged, before, "new bank never reached");
        drop((actual, budget));
        assert_eq!(invocation.exit(), Ok(()));
        scope.finish();
    }
}

#[test]
fn expired_pointer_receiver_never_spends_foreign_scope_or_changes_old_diagnostic() {
    let program = fixture();
    let foreign_program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("expired.identity".to_owned()),
        value: false,
    });
    let expected = replay_error(function, "ordinary builder already failed");
    for old in [true, false] {
        let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let lease = scope.lease();
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        budget.prepare_builder(function).unwrap();
        assert_eq!(invocation.exit(), Ok(()));
        scope.finish();
        let foreign_scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let foreign = enter(
            &foreign_program,
            &foreign_program.functions[0],
            foreign_scope.lease(),
        )
        .unwrap();
        let before = foreign_scope.receipt();
        let actual = if old {
            before_flat_identity::items(&[x.clone()], &[x.clone()], function, &mut budget)
        } else {
            items(&[x.clone()], &[x.clone()], function, &mut budget)
        };
        assert_eq!(actual.as_ref().err(), Some(&expected));
        assert_eq!(
            foreign_scope.receipt(),
            before,
            "no foreign account attempts"
        );
        assert_eq!(
            budget.reserve_bytes(function, 0, "expired retry"),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        drop((actual, budget));
        assert_eq!(foreign.exit(), Ok(()));
        assert_eq!(foreign_scope.finish(), before);
    }
}

#[path = "../../oracle_closure_before/custody.rs"]
mod oracle_closure_custody;
#[test]
fn complete_old_order_is_exact_and_new_submodules_remain_source_bound() {
    let before = oracle_closure_custody::restore(
        "src/cleanup_plan/replay/factored/oracle_order.rs",
        include_str!("../oracle_order.rs"),
    );
    let executable = include_str!("before_flat_identity.rs").replace(
        "pub(super) fn items<I: Borrow<SkeletonObservation>>(",
        "fn items<I: Borrow<SkeletonObservation>>(",
    );
    assert_eq!(executable + "\n#[cfg(test)]\nmod tests;\n", before);
}

#[test]
fn pointer_fee_exact_short_and_sticky_preserve_positive_active_floor() {
    use crate::bounded_output::{
        active_remaining, set_active_floor, with_limit_usage_refusal_site,
    };
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("floor.pointer.identity".to_owned()),
        value: false,
    });
    let ample = run(
        &program,
        &[x.clone()],
        &[x.clone()],
        false,
        1 << 20,
        1 << 20,
        Ok(Ordering::Equal),
    );
    let short = replay_error(
        function,
        "cleanup replay physical reservation exhausted during oracle flat identity receivers",
    );
    for (bytes, expected) in [
        (ample.charged, Ok(Ordering::Equal)),
        (ample.charged - 1, Err(&short)),
    ] {
        let (receipt, refused, consumed, first, site) =
            with_limit_usage_refusal_site(1 << 20, || {
                let remaining = active_remaining().unwrap();
                assert!(remaining > 0 && set_active_floor(remaining));
                let receipt = run(
                    &program,
                    &[x.clone()],
                    &[x.clone()],
                    false,
                    bytes,
                    ample.work,
                    expected,
                );
                assert_eq!(
                    active_remaining(),
                    Some(remaining),
                    "Oracle never spends ACTIVE floor"
                );
                receipt
            });
        assert!(!refused && consumed == 0 && first.is_none() && site.is_none());
        if expected.is_err() {
            assert_eq!(
                receipt.first_refusal.unwrap().requested,
                FLAT_IDENTITY_RECEIVERS
            );
        } else {
            assert_eq!((receipt.charged, receipt.work), (ample.charged, ample.work));
        }
    }
}

#[test]
fn comparison_unwind_releases_owned_rc_inputs_and_oracle_authority() {
    let program = fixture();
    let function = &program.functions[0];
    // Independently observe the complete test-only unwind transport and its
    // payload before using the same operation around real compiler owners.
    // Both controls execute the identical resume_unwind(Box::new(17u8))
    // operation used below. Prepay its complete observed payload and runtime
    // transport before that operation, independently of compiler-owner work.
    // No control bytes are subtracted or credited to either measured arm.
    let control_run = || {
        measure(|| std::panic::catch_unwind(|| std::panic::resume_unwind(Box::new(17u8))).is_err())
    };
    let (control_panicked, control) = control_run();
    assert!(control_panicked);
    assert!(!control.invalid && control.live == 0);
    assert_eq!(control.allocated, control.freed);
    assert!(control.allocated >= std::mem::size_of::<u8>());
    assert!(control.allocated <= 1 << 20);
    let (repeat_panicked, repeat_control) = control_run();
    assert!(repeat_panicked);
    assert_eq!(
        control, repeat_control,
        "unwind transport control must be stable before interpreting its receipt"
    );
    eprintln!("FLAT_IDENTITY_UNWIND_CONTROL complete_runtime_and_payload={control:?} repeated={repeat_control:?}");
    for old in [true, false] {
        let charged = std::cell::Cell::new(0);
        let (panicked, observed) = measure(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
                let lease = scope.lease();
                let _invocation = enter(&program, function, lease).unwrap();
                let mut budget =
                    ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                budget
                    .reserve_bytes(
                        function,
                        2 * std::mem::size_of::<usize>()
                            + std::mem::size_of::<SkeletonObservation>()
                            + 2 * std::mem::size_of::<Rc<SkeletonObservation>>(),
                        "control Rc input",
                    )
                    .unwrap();
                let x = Rc::new(SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: false,
                });
                let result = if old {
                    before_flat_identity::items(&[x.clone()], &[x.clone()], function, &mut budget)
                } else {
                    items(&[x.clone()], &[x.clone()], function, &mut budget)
                };
                assert_eq!(result, Ok(Ordering::Equal));
                budget
                    .reserve_bytes(
                        function,
                        control.allocated,
                        "complete control unwind payload and runtime transport",
                    )
                    .unwrap();
                charged.set(scope.receipt().charged);
                std::panic::resume_unwind(Box::new(17u8));
            }))
            .is_err()
        });
        assert!(panicked);
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(
            observed.allocated <= charged.get(),
            "old={old} observed={observed:?} charged={} independent_unwind_control={control:?}",
            charged.get()
        );
        assert!(OracleScope::open_with_work(1 << 20, 1 << 20).is_ok());
    }
}

#[test]
fn first_and_tail_compact_cuts_keep_work_order_sticky_failure_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("flat\0é.tail.identity".to_owned()),
        value: false,
    });
    let separate = Rc::new((*x).clone());
    for tail in [&x, &separate] {
        let a = [x.clone(), x.clone()];
        let b = [x.clone(), tail.clone()];
        let held = (Rc::strong_count(&x), Rc::strong_count(&separate));
        let first = run(
            &program,
            &a[..1],
            &b[..1],
            false,
            1 << 20,
            1 << 20,
            Ok(Ordering::Equal),
        );
        let whole = run(
            &program,
            &a,
            &b,
            false,
            1 << 20,
            1 << 20,
            Ok(Ordering::Equal),
        );
        assert_eq!(whole.charged - first.charged, FLAT_IDENTITY_RECEIVERS);
        for (left, right, ample, before_work) in
            [(&a[..1], &b[..1], first, 1), (&a[..], &b[..], whole, 3)]
        {
            let exact = run(
                &program,
                left,
                right,
                false,
                ample.charged,
                ample.work,
                Ok(Ordering::Equal),
            );
            assert_eq!((exact.charged, exact.work), (ample.charged, ample.work));
            let expected = replay_error(function,
                "cleanup replay physical reservation exhausted during oracle flat identity receivers");
            let short = run(
                &program,
                left,
                right,
                false,
                ample.charged - 1,
                ample.work,
                Err(&expected),
            );
            let refusal = short
                .first_refusal
                .expect("the reached member's compact payment refuses");
            assert_eq!(refusal.requested, FLAT_IDENTITY_RECEIVERS);
            assert_eq!(refusal.remaining, FLAT_IDENTITY_RECEIVERS - 1);
            assert_eq!(short.work, before_work);
            assert!(short.first_work_refusal.is_none() && short.physical_failure.is_none());
            // run() also retries zero on the same refused lease/budget, checks
            // sticky failure, and observes complete budget/account Drop live0.
        }
        let expected = replay_error(
            function,
            "oracle account work allowance refused during oracle flat observation member",
        );
        let tail_work = run(&program, &a, &b, false, whole.charged, 2, Err(&expected));
        assert_eq!(tail_work.work, 2);
        assert_eq!(
            tail_work.charged, first.charged,
            "tail work fails before its compact byte bank"
        );
        assert!(tail_work.first_refusal.is_none());
        assert_eq!(tail_work.first_work_refusal.unwrap().requested, 1);
        // The identity work uses the compact producer on both reached
        // members. Its complete failure must keep the semantic Diagnostic,
        // paid bank, sticky account and final Drop at both boundaries.
        let expected = replay_error(
            function,
            "oracle account work allowance refused during oracle flat observation identity",
        );
        for (allowance, paid) in [(1, first.charged), (3, whole.charged)] {
            let identity_work = run(
                &program, &a, &b, false, whole.charged, allowance, Err(&expected),
            );
            assert_eq!(identity_work.work, allowance);
            assert_eq!(identity_work.charged, paid);
            assert!(identity_work.first_refusal.is_none());
            let refusal = identity_work.first_work_refusal.unwrap();
            assert_eq!(refusal.requested, 1);
            assert_eq!(refusal.remaining, 0);
            assert!(identity_work.physical_failure.is_none());
        }
        assert_eq!(
            held,
            (Rc::strong_count(&x), Rc::strong_count(&separate)),
            "no comparison/result/refusal retains a borrowed input owner"
        );
        drop((a, b));
    }
    assert_eq!(Rc::strong_count(&x), 1);
    assert_eq!(Rc::strong_count(&separate), 1);
    drop((x, separate));
    assert!(OracleScope::open_with_work(1 << 20, 1 << 20).is_ok());
}
