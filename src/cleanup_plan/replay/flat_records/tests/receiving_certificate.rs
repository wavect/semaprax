//! Certificate phase uses complete old/current algorithms, no semantic repair.
use super::program;
use crate::cleanup_plan::replay::{
    expression_facts::errors::EngineError, lexical_cert, lexical_hir, ReplayBudget,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{FunctionExecutionId, ResolvedFunction};
use crate::test_allocation_observer::measure;
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn render(error: EngineError, f: &ResolvedFunction) -> Diagnostic {
    match error {
        EngineError::Owned(error) => error,
        EngineError::Structural(receipt) => receipt.legacy(f),
        other => panic!("unexpected certificate error {other:?}"),
    }
}
#[test]
fn complete_certificate_receivers_keep_all_fields_order_and_hostile_failures() {
    let p = program();
    for f in p
        .functions
        .iter()
        .filter(|f| f.cleanup_plan.lexical_control.is_some())
    {
        let owner = FunctionExecutionId::Monomorphic(f.id.clone());
        crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
        for mask in 0..8 {
            let mut changed = f.clone();
            let control = changed.cleanup_plan.lexical_control.as_deref_mut().unwrap();
            match mask {
                0 => {}
                1 => control.schema = "unknown.lexical.certificate",
                2 => {
                    control.loops.pop();
                }
                3 => {
                    control.exits.pop();
                }
                4 => {
                    if let Some(resume) = control.resumes.first_mut() {
                        resume.entry = crate::cleanup_plan::BlockId(u32::MAX);
                    }
                }
                5 => control
                    .abrupt_only_regions
                    .push(crate::cleanup_plan::CleanupRegionId(u32::MAX)),
                6 => {
                    control.function_success_exits.pop();
                }
                _ => {
                    if let Some(edge) = changed
                        .cleanup_plan
                        .edges
                        .iter()
                        .find(|e| {
                            matches!(
                                &e.condition,
                                crate::cleanup_plan::EdgeCondition::BooleanResult(..)
                            )
                        })
                        .cloned()
                    {
                        changed.cleanup_plan.edges.push(edge);
                    }
                }
            }
            let facts = lexical_hir::collect(&changed, &mut ReplayBudget::new()).unwrap();
            let old = lexical_cert::authenticate(
                &p,
                &changed,
                &owner,
                &facts,
                &mut ReplayBudget::with_physical(&changed, usize::MAX).unwrap(),
            );
            let mut b = ReplayBudget::with_physical(&changed, usize::MAX).unwrap();
            let new = lexical_cert::paid::authenticate(&p, &changed, &owner, &facts, &mut b);
            match (old, new) {
                (Ok(a), Ok(b)) => assert!(
                    a == b,
                    "all five Auth maps, including private boundary fields, must match"
                ),
                (Err(a), Err(b)) => same(&a, &render(b, &changed)),
                _ => panic!("complete certificate verdict/order differs"),
            }
        }
    }
}
#[test]
fn certificate_actual_receiving_exact_short_work_and_whole_drop() {
    let p = program();
    for f in p
        .functions
        .iter()
        .filter(|f| f.cleanup_plan.lexical_control.is_some())
    {
        let owner = FunctionExecutionId::Monomorphic(f.id.clone());
        crate::hir::lexical_exit::validate_owner(f, &owner).unwrap();
        // The caller owns independently collected immutable HIR input. Both
        // constructor phase observers borrow the same input; no cache warming.
        let facts = lexical_hir::collect(f, &mut ReplayBudget::new()).unwrap();
        let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let before = old.remaining;
        drop(lexical_cert::authenticate(&p, f, &owner, &facts, &mut old).unwrap());
        let old_paid = old.physical.as_deref().unwrap().charged();
        let old_work = before - old.remaining;
        drop(old);
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let setup = b.physical.as_deref().unwrap().charged();
        let before = b.remaining;
        drop(lexical_cert::paid::authenticate(&p, f, &owner, &facts, &mut b).unwrap());
        let exact = b.physical.as_deref().unwrap().charged();
        let work = before - b.remaining;
        drop(b);
        eprintln!("certificate-receiving {} old_paid={old_paid} new_paid={exact} old_work={old_work} new_work={work}",f.id);
        for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
            let ((), seen) = measure(|| {
                let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                let ledger = b.physical.as_deref().unwrap() as *const _;
                let text = b.refusal_text.as_ref().unwrap().as_ptr();
                let result = lexical_cert::paid::authenticate(&p, f, &owner, &facts, &mut b);
                assert_eq!(result.is_ok(), accepted);
                match &result {
                    Ok(_) => assert!(accepted),
                    Err(EngineError::Owned(error)) => {
                        assert_eq!(error.code, "SPX-H006");
                        assert_eq!(error.message.as_ptr(), text);
                        let physical = b.physical.as_deref_mut().unwrap();
                        let first = physical.reserve(0).unwrap_err();
                        assert_eq!(physical.reserve(1).unwrap_err(), first);
                    }
                    _ => panic!("physical refusal must preserve prepaid message"),
                }
                assert_eq!(b.physical.as_deref().unwrap() as *const _, ledger);
                if accepted {
                    assert_eq!(b.physical.as_deref().unwrap().charged(), exact);
                }
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
                lexical_cert::paid::authenticate(&p, f, &owner, &facts, &mut b).is_ok(),
                remaining == work
            );
        }
    }
}

#[test]
fn certificate_precision_complete_paid_drivers_keep_source_and_first_refusal() {
    use crate::cleanup_plan::replay::{lexical_replay, scalar_replay, selected_context};
    let scalar = "module family.scalar; @id(\"family.scalar.main\") fn main()->i64 {0}";
    for (source, id, lexical) in [
        (super::SOURCE, "flat.make", true),
        (scalar, "family.scalar.main", false),
    ] {
        let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(source);
        // Keep the original subject and hostile mutations, but authenticate the
        // complete current directory before either independent driver runs.
        fixture.with_prepared_context(|source| {
            let f = source
                .functions()
                .iter()
                .find(|f| f.id.as_str() == id)
                .unwrap();
            for hostile in [false, true] {
                let mut changed = f.clone();
                if hostile {
                    changed.cleanup_plan.regions.clear();
                }
                // The immutable lease authenticates the original owning subject.
                // Hostile attached metadata is checked against that same source.
                // Full driver admission may refuse the cloned subject earlier;
                // both independent routes must select the same complete error.
                let subject = if hostile { &changed } else { f };
                let mut old_budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let old_context =
                    selected_context::Context::new_paid(source, f, &mut old_budget, lexical)
                        .unwrap();
                let old = if lexical {
                    lexical_replay::before_certificate::validate_paid(
                        &old_context,
                        subject,
                        &mut old_budget,
                    )
                } else {
                    scalar_replay::before_certificate::validate_paid(
                        &old_context,
                        subject,
                        &mut old_budget,
                    )
                };
                let mut new_budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let new_context =
                    selected_context::Context::new_paid(source, f, &mut new_budget, lexical)
                        .unwrap();
                let new = if lexical {
                    lexical_replay::validate_paid(&new_context, subject, &mut new_budget)
                } else {
                    scalar_replay::validate_paid(&new_context, subject, &mut new_budget)
                };
                let render = |error| match error {
                    EngineError::Owned(error) => error,
                    EngineError::Structural(receipt) => receipt.legacy(subject),
                    other => panic!("unexpected error {other:?}"),
                };
                match (old, new) {
                    (Ok(()), Ok(())) => assert!(!hostile),
                    (Err(old), Err(new)) => {
                        assert!(hostile);
                        same(&render(old), &render(new));
                    }
                    _ => panic!("complete old/current paid driver differs"),
                }
            }
        });
    }
}

#[test]
fn readonly_scalar_receiving_removes_exactly_its_dead_deferred_slots() {
    use crate::cleanup_plan::replay::{scalar_replay, selected_context::child::Deferred};
    use std::mem::size_of;
    let source = "module pure.receiving; @id(\"pure.main\") fn main()->i64 {0}";
    let p = crate::hir::resolve(&crate::check(source, "pure-receiving.spx").unwrap()).unwrap();
    let f = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "pure.main")
        .unwrap();
    for signature in [false, true] {
        let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let old_setup = old.physical.as_deref().unwrap().charged();
        let old_before = old.remaining;
        let mut new = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let new_setup = new.physical.as_deref().unwrap().charged();
        let new_before = new.remaining;
        let expected = if signature {
            assert_eq!(
                scalar_replay::before_certificate::records::paid::signature(&p, f, &mut old)
                    .unwrap(),
                scalar_replay::records::paid::signature(&p, f, &mut new).unwrap()
            );
            size_of::<(
                Deferred,
                Option<Deferred>,
                Result<bool, Deferred>,
                Result<bool, Deferred>,
            )>()
        } else {
            assert_eq!(
                scalar_replay::before_certificate::records::paid::expression(
                    &p, f, &f.body, &mut old
                )
                .unwrap(),
                scalar_replay::records::paid::expression(&p, f, &f.body, &mut new).unwrap()
            );
            size_of::<(
                Deferred,
                Option<Deferred>,
                Result<Option<bool>, Deferred>,
                Result<Option<bool>, Deferred>,
            )>()
        };
        assert_eq!(
            (old_before - old.remaining) - (new_before - new.remaining),
            4
        );
        assert_eq!(
            (old.physical.as_deref().unwrap().charged() - old_setup)
                - (new.physical.as_deref().unwrap().charged() - new_setup),
            expected
        );
    }
}
