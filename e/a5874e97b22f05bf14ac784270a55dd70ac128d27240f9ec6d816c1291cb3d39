//! Complete ordinary old/current callers, ample-budget semantic priority.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            r#"module inventory.operands;
@id("main:é") fn main()->i64 {
 let mut n=0; while n<3 {n=n+1; if n==1 {return n;}; 0} return 9;
}
"#,
            "return-operands.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn full_old_and_current_ordinary_lexical_callers_keep_success_and_first_diagnostic_fields() {
    let p = program();
    for attack in 0..5 {
        let mut input = p.clone();
        let f = &mut input.functions[0];
        if attack == 1 || attack == 4 {
            f.cleanup_plan.lexical_control.as_mut().unwrap().schema = "foreign";
        }
        if attack == 2 {
            for exit in &mut f.cleanup_plan.exits {
                if let ExitContinuation::FunctionReturn {
                    source: CleanupResultSource::Scalar { expression },
                    ..
                } = &mut exit.continuation
                {
                    *expression = ExpressionId::from_owned("foreign-return:雪".into());
                    break;
                }
            }
        }
        if attack == 3 || attack == 4 {
            f.cleanup_plan.exits[0].id = crate::cleanup_plan::ExitTargetId(u32::MAX);
        }
        let f = &input.functions[0];
        let (frontiers, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
            let (frontiers, observed) = measure(|| {
                let mut old = ReplayBudget::new();
                let mut current = ReplayBudget::new();
                assert!(!old.selected() && current.pays_allocations());
                let before_old = active_remaining().unwrap();
                let a = before_operands::validate_with_index(&input, f, &mut old, None);
                let after_old = active_remaining().unwrap();
                let b = validate_with_index(&input, f, &mut current, None);
                let after_current = active_remaining().unwrap();
                let frontiers = [
                    MAX_REPLAY_WORK_UNITS - old.remaining,
                    MAX_REPLAY_WORK_UNITS - current.remaining,
                    before_old - after_old,
                    after_old - after_current,
                ];
                match (&a, &b) {
                    (Ok(()), Ok(())) => assert_eq!(attack, 0),
                    (Err(a), Err(b)) => {
                        assert_eq!(a.code, b.code);
                        assert_eq!(a.severity, b.severity);
                        assert_eq!(a.message, b.message);
                        assert_eq!(a.path, b.path);
                        assert_eq!(a.span, b.span);
                        assert_eq!(a.help, b.help);
                    }
                    _ => panic!("complete lexical verdict differs"),
                }
                // The old verification work remains. New directory/control work
                // is additional; absolute used frontiers are not claimed equal.
                assert!(current.remaining <= old.remaining);
                drop(b);
                drop(a);
                drop(current);
                drop(old);
                frontiers
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            frontiers
        });
        assert!(!refused && first.is_none());
        // Logging is outside both the allocation observer and the paid owner.
        eprintln!(
            "#832 attack={attack} old_work={} current_work={} old_declared_bytes={} current_declared_bytes={}",
            frontiers[0], frontiers[1], frontiers[2], frontiers[3]
        );
    }
}

#[test]
fn no_owner_ordinary_caller_remains_the_complete_original_route() {
    let p = program();
    let f = &p.functions[0];
    let mut a = ReplayBudget::new();
    let mut b = ReplayBudget::new();
    assert!(!a.pays_allocations());
    let old = before_operands::validate_with_index(&p, f, &mut a, None);
    let current = validate_with_index(&p, f, &mut b, None);
    assert_eq!(old, current);
    assert_eq!(a.remaining, b.remaining);
}

mod source;
