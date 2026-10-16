//! Full path algorithm comparison, with independently retained old boundary calls.
use super::program;
use crate::cleanup_plan::replay::{
    self, expression_facts::errors::EngineError, lexical_cert, lexical_hir, path_join, ReplayBudget,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{FunctionExecutionId, ResolvedFunction};
use crate::test_allocation_observer::measure;
fn render(e: EngineError, f: &ResolvedFunction) -> Diagnostic {
    match e {
        EngineError::Owned(e) => e,
        EngineError::Structural(e) => e.legacy(f),
        other => panic!("unexpected path failure {other:?}"),
    }
}
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
#[test]
fn complete_old_and_paid_path_boundaries_keep_all_fields_order_and_hostile_state() {
    let p = program();
    for f in p
        .functions
        .iter()
        .filter(|f| f.cleanup_plan.lexical_control.is_some())
    {
        let owner = FunctionExecutionId::Monomorphic(f.id.clone());
        crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
        let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
        let auth =
            lexical_cert::authenticate(&p, f, &owner, &facts, &mut ReplayBudget::new()).unwrap();
        let slots = replay::validate_slots_paid(f, &mut ReplayBudget::new()).unwrap();
        let leaves = replay::collect_leaves_paid(f, &mut ReplayBudget::new()).unwrap();
        for hostile in [false, true] {
            let mut changed = f.clone();
            if hostile {
                if let Some(block) = changed
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .find(|b| !b.transitions.is_empty())
                {
                    block.transitions.insert(0, block.transitions[0].clone());
                }
            }
            let subject = if hostile { &changed } else { f };
            let plan = subject.cleanup_plan.clone();
            let mut old = ReplayBudget::with_physical(subject, usize::MAX).unwrap();
            let expected = path_join::before_path_boundary::validate_path_states::<Diagnostic>(
                &p,
                subject,
                &slots,
                &leaves,
                &mut old,
                Some(&auth),
                None,
            );
            let mut new = ReplayBudget::with_physical(subject, usize::MAX).unwrap();
            let actual = path_join::validate_path_states::<EngineError>(
                &p,
                subject,
                &slots,
                &leaves,
                &mut new,
                Some(&auth),
                None,
            );
            match (expected, actual) {
                (Ok(()), Ok(())) => {}
                (Err(a), Err(b)) => same(&a, &render(b, subject)),
                _ => panic!("whole path verdict/order differs"),
            }
            assert_eq!(subject.cleanup_plan, plan);
        }
    }
}
#[test]
fn paid_path_boundary_actual_limits_work_sticky_and_complete_drop() {
    let p = program();
    for f in p
        .functions
        .iter()
        .filter(|f| f.cleanup_plan.lexical_control.is_some())
    {
        let owner = FunctionExecutionId::Monomorphic(f.id.clone());
        crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
        let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
        let auth =
            lexical_cert::authenticate(&p, f, &owner, &facts, &mut ReplayBudget::new()).unwrap();
        let slots = replay::validate_slots_paid(f, &mut ReplayBudget::new()).unwrap();
        let leaves = replay::collect_leaves_paid(f, &mut ReplayBudget::new()).unwrap();
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let setup = b.physical.as_ref().unwrap().charged();
        let initial = b.remaining;
        path_join::validate_path_states::<EngineError>(
            &p,
            f,
            &slots,
            &leaves,
            &mut b,
            Some(&auth),
            None,
        )
        .unwrap();
        let exact = b.physical.as_ref().unwrap().charged();
        let work = initial - b.remaining;
        drop(b);
        assert!(exact > setup && work > 0);
        for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
            let ((), seen) = measure(|| {
                let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                let ledger = b.physical.as_deref().unwrap() as *const _;
                let result = path_join::validate_path_states::<EngineError>(
                    &p,
                    f,
                    &slots,
                    &leaves,
                    &mut b,
                    Some(&auth),
                    None,
                );
                assert_eq!(result.is_ok(), accepted);
                if !accepted {
                    let physical = b.physical.as_deref_mut().unwrap();
                    let first = physical.reserve(0).unwrap_err();
                    assert_eq!(first, physical.reserve(1).unwrap_err());
                }
                assert_eq!(b.physical.as_deref().unwrap() as *const _, ledger);
                drop(result);
                drop(b);
            });
            assert!(!seen.invalid);
            assert_eq!(seen.live, 0);
            assert_eq!(seen.allocated, seen.freed);
        }
        for remaining in [0, work - 1, work] {
            let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
            b.remaining = remaining;
            assert_eq!(
                path_join::validate_path_states::<EngineError>(
                    &p,
                    f,
                    &slots,
                    &leaves,
                    &mut b,
                    Some(&auth),
                    None
                )
                .is_ok(),
                remaining == work
            );
        }
    }
}
