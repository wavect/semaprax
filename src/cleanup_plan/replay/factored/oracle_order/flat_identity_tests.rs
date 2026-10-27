//! New cuts belong to the new operation stream; old work cuts are independent.
use super::*;
// Historical controls keep the complete immediate pre-byte lexical producer.
use super::{before_byte_items as items, BEFORE_BYTE_IDENTITY_RECEIVERS as FLAT_IDENTITY_RECEIVERS};
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
                &program,
                &a,
                &b,
                false,
                whole.charged,
                allowance,
                Err(&expected),
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

thread_local! {
    static LEXICAL_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(super) fn lexical_before_selected() -> bool {
    LEXICAL_BEFORE.with(|mode| mode.get())
}
fn lexical_before<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            LEXICAL_BEFORE.with(|mode| mode.set(self.0));
        }
    }
    let _restore = Restore(LEXICAL_BEFORE.with(|mode| mode.replace(true)));
    body()
}

#[test]
fn lexical_identity_complete_immediate_invoice_and_all_work_cuts_keep_old_receivers() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("lexical\0é.identity".to_owned()),
        value: false,
    });
    let separate = Rc::new((*x).clone());
    for right in [&x, &separate] {
        let a = [x.clone(), x.clone(), x.clone()];
        let b = [right.clone(), right.clone(), right.clone()];
        let old = lexical_before(|| {
            run(
                &program,
                &a,
                &b,
                false,
                1 << 20,
                1 << 20,
                Ok(Ordering::Equal),
            )
        });
        let current = run(
            &program,
            &a,
            &b,
            false,
            1 << 20,
            1 << 20,
            Ok(Ordering::Equal),
        );
        assert_eq!(current.work, old.work);
        assert_eq!(
            old.charged - current.charged,
            a.len() * (BEFORE_LEXICAL_IDENTITY_RECEIVERS - FLAT_IDENTITY_RECEIVERS),
            "same ABI, full current immediate old bank, no forwarding padding"
        );
        assert_eq!(
            BEFORE_LEXICAL_IDENTITY_RECEIVERS - FLAT_IDENTITY_RECEIVERS,
            2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
                + 2 * std::mem::size_of::<Result<(), physical_budget::ByteFailure>>()
        );
        for cap in 0..=old.work {
            let expected = lexical_before(|| {
                let scope = OracleScope::open_with_work(1 << 20, cap).unwrap();
                let lease = scope.lease();
                let invocation = enter(&program, function, lease).unwrap();
                let mut budget =
                    ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                let actual = items(&a, &b, function, &mut budget);
                drop(budget);
                assert_eq!(invocation.exit(), Ok(()));
                let _ = scope.finish();
                actual
            });
            let expected_ref = expected.as_ref().copied();
            let old = lexical_before(|| run(&program, &a, &b, false, 1 << 20, cap, expected_ref));
            let current = run(&program, &a, &b, false, 1 << 20, cap, expected_ref);
            assert_eq!(current.work, old.work);
            assert_eq!(current.first_work_refusal, old.first_work_refusal);
            assert_eq!(current.physical_failure, old.physical_failure);
            drop(expected);
        }
        for old in [false, true] {
            let body = || {
                let ample = run(
                    &program,
                    &a,
                    &b,
                    false,
                    1 << 20,
                    1 << 20,
                    Ok(Ordering::Equal),
                );
                let exact = run(
                    &program,
                    &a,
                    &b,
                    false,
                    ample.charged,
                    ample.work,
                    Ok(Ordering::Equal),
                );
                assert_eq!((exact.charged, exact.work), (ample.charged, ample.work));
                let refusal = replay_error(function,
                    "cleanup replay physical reservation exhausted during oracle flat identity receivers");
                let short = run(
                    &program,
                    &a,
                    &b,
                    false,
                    ample.charged - 1,
                    ample.work,
                    Err(&refusal),
                );
                assert_eq!(
                    short.first_refusal.unwrap().remaining,
                    if old {
                        BEFORE_LEXICAL_IDENTITY_RECEIVERS
                    } else {
                        FLAT_IDENTITY_RECEIVERS
                    } - 1
                );
            };
            if old {
                lexical_before(body)
            } else {
                body()
            }
        }
        eprintln!(
            "LEXICAL_IDENTITY right_shared={} current={current:?} immediate_old={old:?}",
            Rc::ptr_eq(&x, right)
        );
        drop((a, b));
    }
    assert_eq!(Rc::strong_count(&x), 1);
    assert_eq!(Rc::strong_count(&separate), 1);
}

#[test]
fn lexical_identity_keeps_lent_and_receiver_work_trace_and_expired_generation() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("local.identity".to_owned()),
        value: false,
    });
    let values = [x.clone(), x.clone(), x.clone()];
    for lent in [false, true] {
        for work in 0..=7 {
            let invoke = || {
                let _ = crate::cleanup_plan::replay::take_replay_work_refusal();
                let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
                let lease = scope.lease();
                let invocation = enter(&program, function, lease).unwrap();
                let mut budget = ReplayBudget::with_skeleton_limit_oracle(work, lease);
                budget.lent_skeleton = lent;
                budget.remaining = work;
                let result = items(&values, &values, function, &mut budget);
                let state = (
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent,
                    budget.refusal_spent,
                );
                let trace = crate::cleanup_plan::replay::take_replay_work_refusal().map(|row| {
                    assert!(row.caller.line() != 0);
                    (
                        row.caller.file(),
                        row.requested,
                        row.remaining,
                        row.status_sources,
                        row.first_status_bytes,
                        row.payer,
                    )
                });
                drop(budget);
                assert_eq!(invocation.exit(), Ok(()));
                (result, state, trace, scope.finish())
            };
            let expected = lexical_before(invoke).0;
            let observe = |old| {
                let body = || {
                    measure(|| {
                        let (actual, state, trace, receipt) = invoke();
                        assert_eq!(actual, expected, "lent={lent} work={work} old={old}");
                        drop(actual);
                        (state, trace, receipt)
                    })
                };
                if old {
                    lexical_before(body)
                } else {
                    body()
                }
            };
            let old = observe(true);
            let current = observe(false);
            assert_eq!(
                old.0 .0, current.0 .0,
                "complete remaining/spent/sticky fields"
            );
            assert_eq!(
                old.0 .1, current.0 .1,
                "actual source trace owner and all debit fields"
            );
            assert_eq!(old.0 .2.work, current.0 .2.work);
            assert_eq!(old.0 .2.first_work_refusal, current.0 .2.first_work_refusal);
            for record in [&old, &current] {
                assert!(!record.1.invalid && record.1.live == 0);
                assert_eq!(record.1.allocated, record.1.freed);
                assert!(record.1.allocated <= record.0 .2.charged);
            }
            drop(expected);
        }
    }
    // The unchanged hostile owner exercises real expired/foreign generation
    // controls. Execute it at both complete immediate and lexical identities.
    lexical_before(expired_pointer_receiver_never_spends_foreign_scope_or_changes_old_diagnostic);
    expired_pointer_receiver_never_spends_foreign_scope_or_changes_old_diagnostic();
    drop(values);
    assert_eq!(Rc::strong_count(&x), 1);
}

#[test]
fn lexical_identity_full_old_sources_general_producer_cold_bank_and_items_are_preserved() {
    use sha2::{Digest, Sha256};
    let packet = include_str!("lexical_identity_source_inverse.json");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::read(root.join(
            "src/cleanup_plan/replay/factored/oracle_order/lexical_identity_source_inverse.json"
        ))
        .unwrap(),
        packet.as_bytes()
    );
    let data: serde_json::Value = serde_json::from_str(packet).unwrap();
    assert_eq!(data["owners"].as_array().unwrap().len(), 2);
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    for (path, current, length, sha256) in [
        (
            "src/cleanup_plan/replay/physical_budget.rs",
            include_str!("../../physical_budget.rs"),
            43414,
            "dca3a126f4c0dfde7e45db233d011100c5f9411cd77766260a9b1a4669c05cbd",
        ),
        (
            "src/cleanup_plan/replay/factored/oracle_order.rs",
            include_str!("../oracle_order.rs"),
            34880,
            "bcedf71e28fe75eb13d6ebdad0b9fd05dad4d2c83262024f19cf8237786a9f4d",
        ),
    ] {
        assert_eq!(std::fs::read(root.join(path)).unwrap(), current.as_bytes());
        let rows = data["owners"].as_array().unwrap();
        let row = rows.iter().find(|row| row["path"] == path).unwrap();
        assert_eq!(rows.iter().filter(|row| row["path"] == path).count(), 1);
        assert_eq!(row["current"]["length"], current.len());
        assert_eq!(row["current"]["sha256"], digest(current.as_bytes()));
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let start = edit["start"].as_u64().unwrap() as usize;
            let end = edit["end"].as_u64().unwrap() as usize;
            assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
            restored.replace_range(start..end, edit["before"].as_str().unwrap());
        }
        assert_eq!(restored.len(), length);
        assert_eq!(digest(restored.as_bytes()), sha256);
        assert_eq!(row["before"]["length"], length);
        assert_eq!(row["before"]["sha256"], sha256);
        if path.ends_with("physical_budget.rs") {
            let start = restored
                .find("    // Complete original general work producer")
                .unwrap();
            let end = restored[start..]
                .find("    // Same complete tree-layout producer")
                .unwrap()
                + start;
            assert!(
                current.contains(&restored[start..end]),
                "whole original general work producer byte-exact"
            );
        } else {
            let start = restored.find("const FLAT_IDENTITY_ENTRY:").unwrap();
            let end = restored[start..].find("\nfn items(").unwrap() + start;
            assert!(
                current.contains(&restored[start..end]),
                "whole cold bank byte-exact"
            );
            let start = restored.find("fn items(").unwrap();
            let end = restored[start..]
                .find("\n/// Exact existing prefix pointer identity")
                .unwrap()
                + start;
            let definition = &restored[start..end];
            let body =
                &definition[definition.find('{').unwrap() + 1..definition.rfind('}').unwrap()];
            let fragment = &current[current
                .find("macro_rules! before_lexical_identity_items")
                .unwrap()..];
            let start = fragment.find("=> {{").unwrap() + 5;
            let end = fragment[start..].find("    }};").unwrap() + start;
            let mut actual = fragment[start..end].to_owned();
            for arg in ["a", "b", "function", "budget"] {
                actual = actual.replace(&format!("${arg}"), arg);
            }
            actual = actual.replace(
                "BEFORE_LEXICAL_IDENTITY_RECEIVERS",
                "FLAT_IDENTITY_RECEIVERS",
            );
            let normalize = |text: &str| {
                text.chars()
                    .filter(|ch| !ch.is_whitespace())
                    .collect::<String>()
                    .replace(
                        "\"oracleflatobservationidentity\",)",
                        "\"oracleflatobservationidentity\")",
                    )
            };
            assert_eq!(
                normalize(&actual),
                normalize(body),
                "entire same-ABI immediate items, original hot tariff"
            );
        }
        assert_eq!(std::fs::read(root.join(path)).unwrap(), current.as_bytes());
    }
    assert_eq!(
        std::fs::read(root.join(
            "src/cleanup_plan/replay/factored/oracle_order/lexical_identity_source_inverse.json"
        ))
        .unwrap(),
        packet.as_bytes()
    );
}

// OPT950 controls use the actual current items, not the historical aliases.
fn byte_run(
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
            super::before_byte_items(a, b, function, &mut budget)
        } else {
            super::items(a, b, function, &mut budget)
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
fn lexical_bytes_full_immediate_invoice_work_cuts_and_drop_are_exact() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("byte\0é.identity".to_owned()), value: false,
    });
    let separate = Rc::new((*x).clone());
    assert_eq!(BEFORE_BYTE_IDENTITY_RECEIVERS - super::FLAT_IDENTITY_RECEIVERS,
        2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>());
    for n in [0, 1, 2, 17] {
        for right in [&x, &separate] {
            let a = vec![x.clone(); n];
            let b = vec![right.clone(); n];
            let old = byte_run(&program, &a, &b, true, 1 << 24, 1 << 24, Ok(Ordering::Equal));
            let new = byte_run(&program, &a, &b, false, 1 << 24, 1 << 24, Ok(Ordering::Equal));
            assert_eq!(old.work, new.work);
            assert_eq!(old.charged - new.charged,
                n * (BEFORE_BYTE_IDENTITY_RECEIVERS - super::FLAT_IDENTITY_RECEIVERS));
            for cap in 0..=old.work {
                let scope = OracleScope::open_with_work(1 << 24, cap).unwrap();
                let lease = scope.lease();
                let invocation = enter(&program, function, lease).unwrap();
                let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                let expected = super::before_byte_items(&a, &b, function, &mut budget);
                drop(budget);
                assert_eq!(invocation.exit(), Ok(()));
                scope.finish();
                let old = byte_run(&program, &a, &b, true, 1 << 24, cap, expected.as_ref().copied());
                let new = byte_run(&program, &a, &b, false, 1 << 24, cap, expected.as_ref().copied());
                assert_eq!(old.work, new.work);
                assert_eq!(old.first_work_refusal, new.first_work_refusal);
                assert_eq!(old.physical_failure, new.physical_failure);
                drop(expected);
            }
            for old in [true, false] {
                let ample = byte_run(&program, &a, &b, old, 1 << 24, 1 << 24, Ok(Ordering::Equal));
                let exact = byte_run(&program, &a, &b, old, ample.charged, ample.work, Ok(Ordering::Equal));
                assert_eq!((ample.charged, ample.work), (exact.charged, exact.work));
                if n != 0 {
                    let expected = replay_error(function,
                        "cleanup replay physical reservation exhausted during oracle flat identity receivers");
                    let short = byte_run(&program, &a, &b, old, ample.charged - 1, ample.work, Err(&expected));
                    let bank = if old { BEFORE_BYTE_IDENTITY_RECEIVERS } else { super::FLAT_IDENTITY_RECEIVERS };
                    assert_eq!(short.first_refusal.unwrap().requested, bank);
                    assert_eq!(short.first_refusal.unwrap().remaining, bank - 1);
                }
            }
            drop((a, b));
        }
    }
    assert_eq!(Rc::strong_count(&x), 1);
    assert_eq!(Rc::strong_count(&separate), 1);
}

fn lexical_byte_probe(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
    reserve_flat_identity!(budget, function);
    Ok(())
}
#[test]
fn lexical_bytes_observer_completes_before_first_short_floor_and_sticky_return() {
    use crate::bounded_output::{active_remaining, set_active_floor, with_limit_usage_refusal_site};
    let program = fixture();
    let function = &program.functions[0];
    for short in [false, true] {
        let ((report, failed), refused, _, first, site) = with_limit_usage_refusal_site(1 << 20, || {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            budget.prepare_builder(function).unwrap();
            let remaining = active_remaining().unwrap();
            let bank = super::FLAT_IDENTITY_RECEIVERS;
            assert!(set_active_floor(remaining - bank + usize::from(short)));
            let scope = crate::cleanup_plan::replay::merge_census::Scope::enter(function, &budget);
            let result = lexical_byte_probe(function, &mut budget);
            if short {
                assert_eq!(result.as_ref().err(), Some(&replay_error(function,
                    "cleanup replay physical reservation exhausted during oracle flat identity receivers")));
            } else { assert_eq!(result, Ok(())); }
            let report = scope.report();
            assert!(report.complete && report.foreign == 0);
            assert_eq!(report.cells[12].attempts, 1);
            assert_eq!(report.cells[12].requested, bank);
            assert_eq!(report.cells[12].acquired, if short { 0 } else { bank });
            assert_eq!(report.cells[12].refused, usize::from(short));
            assert_eq!(report.last[12].unwrap().phase, "oracle flat identity receivers");
            drop(result);
            if short {
                assert_eq!(lexical_byte_probe(function, &mut budget),
                    Err(Diagnostic::io("SPX-H006", String::new())));
                let sticky = scope.report();
                assert!(sticky.complete);
                assert_eq!((sticky.cells[12].attempts, sticky.cells[12].refused), (2, 2));
                assert_eq!(sticky.cells[12].acquired, 0);
            }
            drop(scope);
            drop(budget);
            (report, short)
        });
        assert_eq!(refused, failed);
        if short {
            assert_eq!(first.unwrap().requested, super::FLAT_IDENTITY_RECEIVERS);
            assert!(site.unwrap().line() > 0);
        } else { assert!(first.is_none() && site.is_none()); }
        assert_eq!(report.cells[12].refused, usize::from(short));
    }
}

#[test]
fn lexical_bytes_expired_oracle_keeps_foreign_receipt_and_complete_diagnostic() {
    let program = fixture();
    let function = &program.functions[0];
    let x = Rc::new(SkeletonObservation::Boolean {
        expression: ExpressionId::from_owned("byte.expired".to_owned()), value: false,
    });
    for old in [true, false] {
        let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let lease = scope.lease();
        let invocation = enter(&program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        budget.prepare_builder(function).unwrap();
        assert_eq!(invocation.exit(), Ok(()));
        scope.finish();
        let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let invocation = enter(&program, function, scope.lease()).unwrap();
        let before = scope.receipt();
        let result = if old {
            match budget.reserve_compact_bytes(function, BEFORE_BYTE_IDENTITY_RECEIVERS,
                "oracle flat identity receivers") {
                Ok(()) => Ok(()), Err(error) => Err(error.into_diagnostic()),
            }
        } else { lexical_byte_probe(function, &mut budget) };
        assert_eq!(result, Err(replay_error(function, "ordinary builder already failed")));
        assert_eq!(scope.receipt(), before);
        drop((result, budget));
        assert_eq!(invocation.exit(), Ok(()));
        assert_eq!(scope.finish(), before);
    }
    assert_eq!(Rc::strong_count(&x), 1);
}

#[test]
fn lexical_bytes_entire_immediate_sources_and_original_positive_producer_are_preserved() {
    use sha2::{Digest, Sha256};
    let packet = include_str!("lexical_identity_source_inverse.json");
    let data: serde_json::Value = serde_json::from_str(packet).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let metadata = root.join("src/cleanup_plan/replay/factored/oracle_order/lexical_identity_source_inverse.json");
    assert_eq!(std::fs::read(&metadata).unwrap(), packet.as_bytes());
    let scope = &data["opt950_byte_lexicalization"];
    for (path, current) in [
        ("src/cleanup_plan/replay/factored/oracle_order.rs", include_str!("../oracle_order.rs")),
        ("src/cleanup_plan/replay/factored/oracle_order/flat_identity_tests.rs", include_str!("flat_identity_tests.rs")),
    ] {
        let rows = scope["owners"].as_array().unwrap();
        assert_eq!(rows.iter().filter(|row| row["path"] == path).count(), 1);
        let row = rows.iter().find(|row| row["path"] == path).unwrap();
        assert_eq!(std::fs::read(root.join(path)).unwrap(), current.as_bytes());
        assert_eq!(row["current"]["length"], current.len());
        assert_eq!(row["current"]["sha256"], digest(current.as_bytes()));
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let start = edit["start"].as_u64().unwrap() as usize;
            let end = edit["end"].as_u64().unwrap() as usize;
            assert_eq!(&restored[start..end], edit["current"].as_str().unwrap());
            restored.replace_range(start..end, edit["before"].as_str().unwrap());
        }
        let sha = row["before"]["sha256"].as_str().unwrap();
        assert_eq!(row["before"]["length"], restored.len());
        assert_eq!(digest(restored.as_bytes()), sha);
        assert_eq!(std::fs::read(root.join("e").join(sha)).unwrap(), restored.as_bytes());
        if path.ends_with("oracle_order.rs") {
            let start = restored.find("fn items(\n").unwrap();
            let end = restored[start..].find("\n/// Exact existing prefix pointer identity").unwrap() + start;
            let held = restored[start..end].replace("fn items(", "fn before_byte_items(")
                .replace("FLAT_IDENTITY_RECEIVERS", "BEFORE_BYTE_IDENTITY_RECEIVERS");
            assert!(current.contains(&held), "whole immediate same-signature items and tariff");
        }
        assert_eq!(std::fs::read(root.join(path)).unwrap(), current.as_bytes());
        assert_eq!(std::fs::read(root.join("e").join(sha)).unwrap(), restored.as_bytes());
    }
    let prior = &scope["whole_immediate_metadata"];
    let archive = root.join("e").join(prior["sha256"].as_str().unwrap());
    let before = std::fs::read(&archive).unwrap();
    assert_eq!(prior["length"], before.len());
    assert_eq!(prior["sha256"], digest(&before));
    let previous: serde_json::Value = serde_json::from_slice(&before).unwrap();
    let physical = include_str!("../../physical_budget.rs");
    let oldrow = previous["owners"].as_array().unwrap().iter().find(|row| row["path"] ==
        "src/cleanup_plan/replay/physical_budget.rs").unwrap();
    assert_eq!(oldrow["current"]["length"], physical.len());
    assert_eq!(oldrow["current"]["sha256"], digest(physical.as_bytes()));
    assert_eq!(data["owners"][0], previous["owners"][0], "unchanged full general producer custody row");
    assert_eq!(std::fs::read(root.join("src/cleanup_plan/replay/physical_budget.rs")).unwrap(), physical.as_bytes());
    assert_eq!(std::fs::read(archive).unwrap(), before);
    assert_eq!(std::fs::read(metadata).unwrap(), packet.as_bytes());
}
