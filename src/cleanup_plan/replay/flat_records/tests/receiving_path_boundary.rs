//! Full path algorithm comparison, with independently retained old boundary calls.
use super::program;
use crate::cleanup_plan::replay::{
    self, expression_facts::errors::EngineError, lexical_cert, lexical_hir, path_join,
    selected_context, ReplayBudget,
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
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(super::SOURCE);
    fixture.with_prepared_context(|source| {
        // Replacing the invalid payer/source pairing must retain the complete
        // original lexical-function corpus, including declaration order.
        let original = program();
        assert_eq!(
            source
                .functions()
                .iter()
                .filter(|f| f.cleanup_plan.lexical_control.is_some())
                .map(|f| &f.id)
                .collect::<Vec<_>>(),
            original
                .functions
                .iter()
                .filter(|f| f.cleanup_plan.lexical_control.is_some())
                .map(|f| &f.id)
                .collect::<Vec<_>>()
        );
        for f in source
            .functions()
            .iter()
            .filter(|f| f.cleanup_plan.lexical_control.is_some())
        {
            let owner = FunctionExecutionId::Monomorphic(f.id.clone());
            crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
            let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
            let auth =
                lexical_cert::authenticate(source, f, &owner, &facts, &mut ReplayBudget::new())
                    .unwrap();
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
                let old_context =
                    selected_context::Context::new_paid(source, f, &mut old, true).unwrap();
                let expected = path_join::before_path_boundary::validate_path_states::<Diagnostic>(
                    &old_context,
                    subject,
                    &slots,
                    &leaves,
                    &mut old,
                    Some(&auth),
                    None,
                );
                let mut new = ReplayBudget::with_physical(subject, usize::MAX).unwrap();
                let new_context =
                    selected_context::Context::new_paid(source, f, &mut new, true).unwrap();
                let actual = path_join::validate_path_states::<EngineError>(
                    &new_context,
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
    });
}
#[test]
fn paid_path_boundary_actual_limits_work_sticky_and_complete_drop() {
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(super::SOURCE);
    fixture.with_prepared_context(|source| {
        // Replacing the invalid payer/source pairing must retain the complete
        // original lexical-function corpus, including declaration order.
        let original = program();
        assert_eq!(
            source
                .functions()
                .iter()
                .filter(|f| f.cleanup_plan.lexical_control.is_some())
                .map(|f| &f.id)
                .collect::<Vec<_>>(),
            original
                .functions
                .iter()
                .filter(|f| f.cleanup_plan.lexical_control.is_some())
                .map(|f| &f.id)
                .collect::<Vec<_>>()
        );
        for f in source
            .functions()
            .iter()
            .filter(|f| f.cleanup_plan.lexical_control.is_some())
        {
            let owner = FunctionExecutionId::Monomorphic(f.id.clone());
            crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
            let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
            let auth =
                lexical_cert::authenticate(source, f, &owner, &facts, &mut ReplayBudget::new())
                    .unwrap();
            let slots = replay::validate_slots_paid(f, &mut ReplayBudget::new()).unwrap();
            let leaves = replay::collect_leaves_paid(f, &mut ReplayBudget::new()).unwrap();
            // Context profile/census construction and its complete destruction
            // belong to each receiver's SAME exact byte/work owner below.
            for old in [false, true] {
                let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let setup = b.physical.as_ref().unwrap().charged();
                let initial = b.remaining;
                let context = selected_context::Context::new_paid(source, f, &mut b, true).unwrap();
                (if old {
                    path_join::before_path_boundary::validate_path_states::<Diagnostic>(
                        &context,
                        f,
                        &slots,
                        &leaves,
                        &mut b,
                        Some(&auth),
                        None,
                    )
                    .map_err(EngineError::Owned)
                } else {
                    path_join::validate_path_states::<EngineError>(
                        &context,
                        f,
                        &slots,
                        &leaves,
                        &mut b,
                        Some(&auth),
                        None,
                    )
                })
                .unwrap();
                let exact = b.physical.as_ref().unwrap().charged();
                let work = initial - b.remaining;
                drop(context);
                drop(b);
                assert!(exact > setup && work > 0);
                for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
                    let ((), seen) = measure(|| {
                        let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                        let ledger = b.physical.as_deref().unwrap() as *const _;
                        let result =
                            (|| {
                                let context =
                                    selected_context::Context::new_paid(source, f, &mut b, true)?;
                                if old {
                                    path_join::before_path_boundary::validate_path_states::<
                                        Diagnostic,
                                    >(
                                        &context, f, &slots, &leaves, &mut b, Some(&auth), None
                                    )
                                    .map_err(EngineError::Owned)
                                } else {
                                    path_join::validate_path_states::<EngineError>(
                                        &context,
                                        f,
                                        &slots,
                                        &leaves,
                                        &mut b,
                                        Some(&auth),
                                        None,
                                    )
                                }
                            })();
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
                        (|| {
                            let context =
                                selected_context::Context::new_paid(source, f, &mut b, true)?;
                            if old {
                                path_join::before_path_boundary::validate_path_states::<Diagnostic>(
                                    &context,
                                    f,
                                    &slots,
                                    &leaves,
                                    &mut b,
                                    Some(&auth),
                                    None,
                                )
                                .map_err(EngineError::Owned)
                            } else {
                                path_join::validate_path_states::<EngineError>(
                                    &context,
                                    f,
                                    &slots,
                                    &leaves,
                                    &mut b,
                                    Some(&auth),
                                    None,
                                )
                            }
                        })()
                        .is_ok(),
                        remaining == work
                    );
                }
            }
        }
    });
}

#[test]
fn raw_selected_path_without_current_census_retains_original_six_field_refusal() {
    // Preserve the original invalid setup as an explicit negative. A raw
    // ResolvedProgram still cannot lend the current selected append directory.
    let p = program();
    let f = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.identity")
        .unwrap();
    let owner = FunctionExecutionId::Monomorphic(f.id.clone());
    crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
    let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
    let auth = lexical_cert::authenticate(&p, f, &owner, &facts, &mut ReplayBudget::new()).unwrap();
    let slots = replay::validate_slots_paid(f, &mut ReplayBudget::new()).unwrap();
    let leaves = replay::collect_leaves_paid(f, &mut ReplayBudget::new()).unwrap();
    let plan = f.cleanup_plan.clone();
    let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
    let expected = path_join::before_path_boundary::validate_path_states::<Diagnostic>(
        &p,
        f,
        &slots,
        &leaves,
        &mut old,
        Some(&auth),
        None,
    )
    .unwrap_err();
    let mut new = ReplayBudget::with_physical(f, usize::MAX).unwrap();
    let actual = path_join::validate_path_states::<EngineError>(
        &p,
        f,
        &slots,
        &leaves,
        &mut new,
        Some(&auth),
        None,
    )
    .unwrap_err();
    let actual = render(actual, f);
    same(&actual, &expected);
    let missing = replay::replay_error(f, "selected append lacks current function census");
    same(&actual, &missing);
    assert_eq!(f.cleanup_plan, plan);
}
