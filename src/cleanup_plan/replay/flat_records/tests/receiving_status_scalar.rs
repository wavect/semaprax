//! Direct original/current status and scalar-family gates; no gain claim.
use crate::cleanup_plan::replay::{
    expression_facts::errors::EngineError, scalar_replay, status_construction, ReplayBudget,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedFunction, ResolvedProgram};
use crate::test_allocation_observer::measure;
const STATUS: &str = r#"module replay.paid_status;
@id("status.add") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1}
@id("status.main") fn main()->i64 {add(41)}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(STATUS, "paid-status.spx").unwrap()).unwrap()
}
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
        other => panic!("unexpected error {other:?}"),
    }
}
#[test]
fn complete_status_receiver_keeps_contract_arithmetic_call_order_and_all_fields() {
    let p = program();
    for f in &p.functions {
        for mask in 0..4 {
            let mut changed = f.clone();
            if mask & 1 != 0 {
                changed.cleanup_plan.status_sources.clear();
            }
            if mask & 2 != 0 {
                changed.cleanup_plan.status_sources.reverse();
            }
            let old = status_construction::validate_required_status_sources(
                &p,
                &changed,
                &mut ReplayBudget::new(),
            );
            let mut b = ReplayBudget::with_physical(&changed, usize::MAX).unwrap();
            let new =
                status_construction::paid::validate_required_status_sources(&p, &changed, &mut b);
            match (old, new) {
                (Ok(()), Ok(())) => {}
                (Err(old), Err(new)) => same(&old, &render(new, &changed)),
                _ => panic!("complete status verdict/order differs"),
            }
        }
    }
    // The dynamic identity remains an immutable shared operand. Rendering is
    // the explicit old oracle boundary, outside any physical observer.
    let f = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "status.main")
        .unwrap();
    let mut call: ResolvedExpr = nodes(&f.body)
        .into_iter()
        .find(|e| matches!(&e.kind, crate::hir::ResolvedExprKind::Call { .. }))
        .unwrap()
        .clone();
    let crate::hir::ResolvedExprKind::Call { callee, .. } = &mut call.kind else {
        unreachable!()
    };
    *callee = crate::hir::DeclarationId::new("unknown:é.status.callee");
    let old = status_construction::collect_for_test(
        &p,
        f,
        &call,
        &mut Vec::new(),
        &mut ReplayBudget::new(),
    )
    .unwrap_err();
    let mut budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
    let new = status_construction::paid::collect_expression_statuses(
        &p,
        f,
        &call,
        &mut Vec::new(),
        &mut budget,
    )
    .unwrap_err();
    same(&old, &render(new, f));
    assert!(old.message.contains("unknown:é.status.callee"));
}
#[test]
fn status_actual_receiving_exact_short_work_and_whole_drop() {
    let p = program();
    for f in &p.functions {
        let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let before = old.remaining;
        status_construction::validate_required_status_sources(&p, f, &mut old).unwrap();
        let old_paid = old.physical.as_deref().unwrap().charged();
        let old_work = before - old.remaining;
        drop(old);
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let setup = b.physical.as_deref().unwrap().charged();
        let before = b.remaining;
        status_construction::paid::validate_required_status_sources(&p, f, &mut b).unwrap();
        let exact = b.physical.as_deref().unwrap().charged();
        let work = before - b.remaining;
        drop(b);
        eprintln!("status-receiving {} old_paid={old_paid} new_paid={exact} old_work={old_work} new_work={work}",f.id);
        for (limit, ok) in [(setup, false), (exact - 1, false), (exact, true)] {
            let ((), seen) = measure(|| {
                let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                let ledger = b.physical.as_deref().unwrap() as *const _;
                let text = b.refusal_text.as_ref().unwrap().as_ptr();
                let result =
                    status_construction::paid::validate_required_status_sources(&p, f, &mut b);
                assert_eq!(result.is_ok(), ok);
                if let Err(EngineError::Owned(error)) = &result {
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.message.as_ptr(), text);
                    let physical = b.physical.as_deref_mut().unwrap();
                    let first = physical.reserve(0).unwrap_err();
                    assert_eq!(physical.reserve(1).unwrap_err(), first);
                } else {
                    assert!(ok);
                }
                assert_eq!(b.physical.as_deref().unwrap() as *const _, ledger);
                if ok {
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
                status_construction::paid::validate_required_status_sources(&p, f, &mut b).is_ok(),
                remaining == work
            );
        }
    }
}
fn nodes(e: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    let mut out = Vec::new();
    let mut stack = vec![crate::hir::lexical_exit::ChildCursor::new(e)];
    while let Some(mut cursor) = stack.pop() {
        out.push(cursor.expression());
        while let Some(child) = cursor.next() {
            stack.push(crate::hir::lexical_exit::ChildCursor::new(child));
        }
    }
    out
}
#[test]
fn scalar_record_receivers_keep_complete_shape_answers_and_real_limits() {
    let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(super::SOURCE);
    fixture.with_prepared_context(|context| {
        for f in context.functions() {
            let old =
                scalar_replay::records::signature_for_test(context, f, &mut ReplayBudget::new());
            let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
            let new = scalar_replay::records::paid::signature(context, f, &mut b);
            match (old, new) {
                (Ok(a), Ok(b)) => assert_eq!(a, b),
                (Err(a), Err(b)) => same(&a, &render(b, f)),
                _ => panic!("record signature differs"),
            }
            for e in nodes(&f.body) {
                let old = scalar_replay::records::expression_for_test(
                    context,
                    f,
                    e,
                    &mut ReplayBudget::new(),
                );
                let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let setup = b.physical.as_deref().unwrap().charged();
                let before = b.remaining;
                let new = scalar_replay::records::paid::expression(context, f, e, &mut b);
                let exact = b.physical.as_deref().unwrap().charged();
                let work = before - b.remaining;
                let expected = match (old, new) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a, b);
                        a
                    }
                    (Err(a), Err(b)) => {
                        same(&a, &render(b, f));
                        continue;
                    }
                    _ => panic!("record expression differs"),
                };
                drop(b);
                for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
                    let ((), seen) = measure(|| {
                        let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                        let text = b.refusal_text.as_ref().unwrap().as_ptr();
                        let result =
                            scalar_replay::records::paid::expression(context, f, e, &mut b);
                        assert_eq!(result.is_ok(), accepted);
                        match &result {
                            Ok(answer) => assert_eq!(*answer, expected),
                            Err(EngineError::Owned(error)) => {
                                assert_eq!(error.message.as_ptr(), text)
                            }
                            _ => panic!("budget boundary must retain preowned diagnostic"),
                        };
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
                        scalar_replay::records::paid::expression(context, f, e, &mut b).is_ok(),
                        remaining == work
                    );
                }
            }
        }
    });
}

#[test]
fn status_scalar_complete_paid_drivers_keep_source_and_first_refusal() {
    use crate::cleanup_plan::replay::{lexical_replay, scalar_replay, selected_context};
    let scalar = "module family.scalar; @id(\"family.scalar.main\") fn main()->i64 {0}";
    for (source, id, lexical) in [
        (super::SOURCE, "flat.make", true),
        (scalar, "family.scalar.main", false),
    ] {
        let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(source);
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
                    lexical_replay::before_status_scalar::validate_paid(
                        &old_context,
                        subject,
                        &mut old_budget,
                    )
                } else {
                    scalar_replay::before_status_scalar::validate_paid(
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
