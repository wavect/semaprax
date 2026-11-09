//! Oracle backing is reached locally; old fixed construction is independent.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleReceipt, OracleRefusal, OracleScope};
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module oracle.frames; @id(\"oracle.frames.main\") fn main()->i64 {0}",
            "oracle-frames.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn run(
    program: &ResolvedProgram,
    depth: usize,
    old: bool,
    bytes: usize,
    work: usize,
    expected: Option<&Diagnostic>,
) -> OracleReceipt {
    let function = &program.functions[0];
    let (receipt, observed) = measure(|| {
        let scope = OracleScope::open_with_work(bytes, work).unwrap();
        let lease = scope.lease();
        let invocation = enter(program, function, lease).unwrap();
        let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        let result = (|| {
            let mut frames = if old {
                oracle_reached::old_fresh(function, &mut budget)
            } else {
                fresh(function, &mut budget)
            }?;
            for index in 0..depth {
                if old {
                    oracle_reached::old_reserve_push(&mut frames, function, &mut budget)
                } else {
                    reserve_push(&mut frames, function, &mut budget)
                }?;
                frames.push(if index.is_multiple_of(2) {
                    Frame::Eval(&function.body)
                } else {
                    Frame::UpcastPassthrough
                });
                for (ordinal, frame) in frames.iter().enumerate() {
                    match frame {
                        Frame::Eval(expr) if ordinal.is_multiple_of(2) => {
                            assert!(std::ptr::eq(*expr, &function.body))
                        }
                        Frame::UpcastPassthrough if !ordinal.is_multiple_of(2) => {}
                        _ => panic!("relocation changed continuation order"),
                    }
                }
            }
            assert_eq!(frames.len(), depth);
            if !old && depth <= 2 {
                assert_eq!(frames.capacity(), 2);
            }
            if old {
                assert_eq!(frames.capacity(), FRAME_CAPACITY);
            }
            drop(frames);
            Ok::<_, Diagnostic>(())
        })();
        assert_eq!(
            result.as_ref().err(),
            expected,
            "complete six-field Diagnostic"
        );
        let receipt = scope.receipt();
        let refused = receipt.first_refusal.is_some()
            || receipt.first_work_refusal.is_some()
            || receipt.physical_failure.is_some();
        drop(result);
        if refused {
            assert_eq!(lease.charge(0), Err(OracleRefusal::Sticky));
            assert_eq!(
                budget.reserve_bytes(function, 0, "continuation sticky retry"),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
        }
        drop(budget);
        assert_eq!(invocation.exit(), Ok(()));
        scope.finish()
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= receipt.charged);
    receipt
}

#[test]
fn oracle_initial_and_relocated_backing_keep_exact_short_zero_and_drop() {
    let program = fixture();
    let function = &program.functions[0];
    for depth in [0, 1, 2, 3, 17, 516] {
        for old in [true, false] {
            let ample = run(&program, depth, old, 1 << 24, 1 << 20, None);
            let exact = run(&program, depth, old, ample.charged, ample.work, None);
            assert_eq!((exact.charged, exact.work), (ample.charged, ample.work));
            let empty = Diagnostic::io("SPX-H006", String::new());
            run(&program, depth, old, 0, ample.work, Some(&empty));
        }
    }
    let shallow = run(&program, 2, false, 1 << 24, 1 << 20, None);
    let original = run(&program, 2, true, 1 << 24, 1 << 20, None);
    assert!(
        shallow.charged < original.charged,
        "actual fixed backing and its fee are gone"
    );
    let deep = run(&program, 516, false, 1 << 24, 1 << 20, None);
    assert!(
        deep.charged > original.charged,
        "deep cumulative backing can regress"
    );
    assert!(
        deep.work > original.work,
        "all relocations/release are paid"
    );
    let bytes_short = replay_error(
        function,
        "cleanup replay physical reservation exhausted during oracle continuation initial backing",
    );
    run(
        &program,
        2,
        false,
        shallow.charged - 1,
        shallow.work,
        Some(&bytes_short),
    );
    let work_zero = replay_error(function,
        "oracle account work allowance refused during oracle continuation initial allocation and release");
    run(&program, 2, false, shallow.charged, 0, Some(&work_zero));
    run(
        &program,
        2,
        false,
        shallow.charged,
        shallow.work - 1,
        Some(&work_zero),
    );
    let limit = replay_error(
        function,
        "typed-HIR skeleton traversal exceeds the admitted depth",
    );
    for old in [true, false] {
        run(
            &program,
            FRAME_CAPACITY + 1,
            old,
            1 << 24,
            1 << 20,
            Some(&limit),
        );
    }
}

#[test]
fn constructor_unwind_drops_actual_new_and_complete_old_backing_and_authority() {
    let program = fixture();
    let function = &program.functions[0];
    for old in [true, false] {
        let (panicked, observed) = measure(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let scope = OracleScope::open_with_work(1 << 24, 1 << 20).unwrap();
                let lease = scope.lease();
                let _invocation = enter(&program, function, lease).unwrap();
                let mut budget =
                    ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                let mut frames = if old {
                    oracle_reached::old_fresh(function, &mut budget)
                } else {
                    fresh(function, &mut budget)
                }
                .unwrap();
                frames.push(Frame::Eval(&function.body));
                budget
                    .reserve_bytes(
                        function,
                        std::mem::size_of::<u8>(),
                        "control unwind payload",
                    )
                    .unwrap();
                std::panic::resume_unwind(Box::new(17u8));
            }))
            .is_err()
        });
        assert!(panicked);
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(OracleScope::open_with_work(1 << 20, 1 << 20).is_ok());
    }
}

#[test]
fn expired_constructor_receiver_keeps_old_diagnostic_without_foreign_debit() {
    let program = fixture();
    let foreign_program = fixture();
    let function = &program.functions[0];
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
            oracle_reached::old_fresh(function, &mut budget)
        } else {
            fresh(function, &mut budget)
        };
        assert_eq!(actual.as_ref().err(), Some(&expected));
        assert_eq!(foreign_scope.receipt(), before);
        assert_eq!(
            budget.reserve_bytes(function, 0, "expired retry"),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        drop((actual, budget));
        assert_eq!(foreign.exit(), Ok(()));
        assert_eq!(foreign_scope.finish(), before);
    }
}

#[test]
fn new_oracle_fee_frontiers_exact_short_and_sticky_preserve_positive_active_floor() {
    use crate::bounded_output::{
        active_remaining, set_active_floor, with_limit_usage_refusal_site,
    };
    let program = fixture();
    let function = &program.functions[0];
    let shallow = run(&program, 2, false, 1 << 20, 1 << 20, None);
    let grown = run(&program, 3, false, 1 << 20, 1 << 20, None);
    let fresh_fee = shallow.charged - 2 * std::mem::size_of::<Frame<'_>>();
    let growth_fee = shallow.charged + oracle_reached::GROW_CALL;
    let fresh_bank = replay_error(
        function,
        "cleanup replay physical reservation exhausted during oracle continuation fresh receivers",
    );
    let fresh_backing = replay_error(
        function,
        "cleanup replay physical reservation exhausted during oracle continuation initial backing",
    );
    let growth_bank = replay_error(
        function,
        "cleanup replay physical reservation exhausted during oracle continuation growth receivers",
    );
    let growth_backing = replay_error(function,
        "cleanup replay physical reservation exhausted during oracle continuation replacement backing");
    for (depth, bytes, work, expected, request) in [
        (2, shallow.charged, shallow.work, None, 0),
        (
            2,
            fresh_fee - 1,
            shallow.work,
            Some(&fresh_bank),
            oracle_reached::FRESH_CALL,
        ),
        (
            2,
            fresh_fee,
            shallow.work,
            Some(&fresh_backing),
            2 * std::mem::size_of::<Frame<'_>>(),
        ),
        (3, grown.charged, grown.work, None, 0),
        (
            3,
            growth_fee - 1,
            grown.work,
            Some(&growth_bank),
            oracle_reached::GROW_CALL,
        ),
        (
            3,
            growth_fee,
            grown.work,
            Some(&growth_backing),
            4 * std::mem::size_of::<Frame<'_>>(),
        ),
    ] {
        let (receipt, refused, consumed, first, site) =
            with_limit_usage_refusal_site(1 << 20, || {
                let remaining = active_remaining().unwrap();
                assert!(remaining > 0 && set_active_floor(remaining));
                let receipt = run(&program, depth, false, bytes, work, expected);
                assert_eq!(
                    active_remaining(),
                    Some(remaining),
                    "Oracle never spends ACTIVE floor"
                );
                receipt
            });
        assert!(!refused && consumed == 0 && first.is_none() && site.is_none());
        if expected.is_some() {
            assert_eq!(receipt.first_refusal.unwrap().requested, request);
        } else {
            assert!(
                receipt.first_refusal.is_none()
                    && receipt.first_work_refusal.is_none()
                    && receipt.physical_failure.is_none()
            );
        }
    }
}
