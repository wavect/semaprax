//! Actual typed child receiver and complete independent old producer controls.
use super::{Permit, Refusal};
use crate::cleanup_plan::replay::{
    expression_facts, primitive_fixture, replay_error, selected_context, semantic_refusal,
    ReplayBudget,
};
use crate::cleanup_plan::{renewal_profile, ReplayPhysicalReservation};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExprKind, ResolvedFunction, ResolvedProgram};
use crate::test_allocation_observer::measure;
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module child.receipt; @id(\"main:é\") fn main()->i64 {let mut n=0;n=n+1;n}",
            "child.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn rendered(
    function: &ResolvedFunction,
    error: expression_facts::errors::EngineError,
) -> Diagnostic {
    match error {
        expression_facts::errors::EngineError::Structural(receipt) => receipt.legacy(function),
        expression_facts::errors::EngineError::Owned(d) => d,
        _ => panic!("unexpected child channel"),
    }
}
#[test]
#[allow(
    clippy::single_element_loop,
    reason = "The complete saved control retains its original expressions for independent parity checks."
)]
fn complete_old_child_receivers_keep_success_raw_errors_prefix_and_order() {
    let program = fixture();
    let context = primitive_fixture::Context::new(&program);
    let f = &program.functions[0];
    let old = renewal_profile::bindings_with_reservation_and_work(
        &context,
        f,
        &mut ReplayPhysicalReservation::new(usize::MAX),
        &mut |_| Ok(()),
    )
    .unwrap();
    let new = renewal_profile::bindings_with_reservation_and_work_inline(
        &context,
        f,
        &mut ReplayPhysicalReservation::new(usize::MAX),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(old.appends(), new.appends());
    assert_eq!(old.has_helpers(), new.has_helpers());
    for id in [f.body.id.clone()] {
        assert_eq!(old.get(&id).map(|x| &x.id), new.get(&id).map(|x| &x.id));
    }
    let old = renewal_profile::bindings_with_reservation_and_work(
        &context,
        f,
        &mut ReplayPhysicalReservation::new(0),
        &mut |_| Ok(()),
    )
    .err()
    .unwrap();
    let new = renewal_profile::bindings_with_reservation_and_work_inline(
        &context,
        f,
        &mut ReplayPhysicalReservation::new(0),
        &mut |_| Ok(()),
    )
    .err()
    .unwrap();
    same(&old, &new.legacy());
    let mut budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
    let permit = Permit::acquire::<renewal_profile::RenewalBindings<'_>>(f, &mut budget).unwrap();
    let old = renewal_profile::bindings_with_reservation_and_work(
        &context,
        f,
        &mut ReplayPhysicalReservation::new(usize::MAX),
        &mut |_| Err(replay_error(f, "selected renewal work exhausted")),
    )
    .err()
    .unwrap();
    let new = renewal_profile::bindings_with_reservation_and_work_inline(
        &context,
        f,
        &mut ReplayPhysicalReservation::new(usize::MAX),
        &mut |_| Err(Refusal::RenewalWork),
    )
    .err()
    .unwrap();
    same(&old, &rendered(f, permit.refuse(new)));
    let mut deep = f.clone();
    let ResolvedExprKind::Block { tail, .. } = &deep.body.kind else {
        panic!("block")
    };
    let leaf = tail.as_ref().clone();
    let mut nested = leaf.clone();
    for _ in 0..=crate::cache_codec::MAX_DEPTH + 1 {
        let mut parent = leaf.clone();
        parent.kind = ResolvedExprKind::Block {
            statements: Vec::new(),
            tail: Box::new(nested),
        };
        nested = parent;
    }
    deep.body = nested;
    let old = crate::hir::lexical_exit::requires(&deep).unwrap_err();
    let new = crate::hir::lexical_exit::requires_inline(&deep).unwrap_err();
    same(&old, &new.legacy());
}
#[test]
fn actual_paid_context_uses_child_receiving_and_keeps_complete_old_success_and_drop() {
    let program = fixture();
    let context = primitive_fixture::Context::new(&program);
    let f = &program.functions[0];
    let (_, seen) = measure(|| {
        let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let mut new = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let original =
            selected_context::Context::new_paid_before_children(&context, f, &mut old, false)
                .unwrap();
        let current = selected_context::Context::new_paid(&context, f, &mut new, false).unwrap();
        assert!(std::ptr::eq(original.function, f));
        assert!(std::ptr::eq(current.function, f));
        assert!(
            new.remaining < old.remaining,
            "actual added control work is not discounted"
        );
        assert!(
            new.physical.as_deref().unwrap().charged() > old.physical.as_deref().unwrap().charged()
        );
        drop(current);
        drop(original);
        drop(new);
        drop(old);
    });
    assert!(!seen.invalid);
    assert_eq!(seen.live, 0);
    assert_eq!(seen.allocated, seen.freed);
}

fn setup_bytes(f: &ResolvedFunction) -> usize {
    std::mem::size_of::<ReplayPhysicalReservation>()
        + 514 * std::mem::size_of::<Option<(&crate::hir::ResolvedType, usize)>>()
        + f.id.as_str().len()
        + 512
}
fn permit_bytes() -> usize {
    std::mem::size_of::<(
        Permit<'static>,
        Refusal,
        Option<Refusal>,
        semantic_refusal::Receipt,
        expression_facts::errors::EngineError,
        Option<expression_facts::errors::EngineError>,
        Result<renewal_profile::RenewalBindings<'static>, Refusal>,
        Result<renewal_profile::RenewalBindings<'static>, expression_facts::errors::EngineError>,
        Result<Permit<'static>, expression_facts::errors::EngineError>,
        Result<selected_context::Context<'static>, expression_facts::errors::EngineError>,
        Result<(), expression_facts::errors::EngineError>,
    )>()
}
#[test]
fn child_permit_exact_short_zero_bytes_and_work_preserve_first_ledger_full_drop() {
    let program = fixture();
    let f = &program.functions[0];
    let bytes = permit_bytes();
    for (extra, work, success) in [
        (0, usize::MAX, false),
        (bytes - 1, usize::MAX, false),
        (bytes, 0, false),
        (bytes, 10, false),
        (bytes, 11, true),
    ] {
        let (_, seen) = measure(|| {
            let mut budget = ReplayBudget::with_physical(f, setup_bytes(f) + extra).unwrap();
            budget.remaining = work;
            let ledger = budget.physical.as_deref().unwrap() as *const ReplayPhysicalReservation;
            let text = budget.refusal_text.as_ref().unwrap().as_ptr();
            let result = Permit::acquire::<renewal_profile::RenewalBindings<'_>>(f, &mut budget);
            assert_eq!(result.is_ok(), success);
            match result {
                Ok(permit) => {
                    assert_eq!(budget.remaining, 0);
                    assert_eq!(
                        budget.physical.as_deref().unwrap().charged(),
                        setup_bytes(f) + bytes
                    );
                    // No byte/work allowance remains to fund a later receipt.
                    let before = crate::test_allocation_observer::current_metrics();
                    let receipt = permit.refuse(Refusal::RenewalWork);
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(before.allocated, after.allocated);
                    let expression_facts::errors::EngineError::Structural(receipt) = receipt else {
                        panic!("typed receipt")
                    };
                    assert_eq!(receipt.stage(), "selected child");
                    assert_eq!(receipt.source().as_str(), f.body.id.as_str());
                    drop(receipt);
                }
                Err(error) => {
                    let expression_facts::errors::EngineError::Owned(error) = error else {
                        panic!("original prepaid budget text")
                    };
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.message.as_ptr(), text);
                    let physical = budget.physical.as_deref_mut().unwrap();
                    let first = physical.reserve(0).unwrap_err();
                    assert_eq!(physical.reserve(1).unwrap_err(), first);
                    drop(error);
                }
            }
            assert_eq!(
                budget.physical.as_deref().unwrap() as *const ReplayPhysicalReservation,
                ledger
            );
            drop(budget);
        });
        assert!(!seen.invalid);
        assert_eq!(seen.live, 0);
        assert_eq!(seen.allocated, seen.freed);
    }
}

#[test]
fn complete_old_helper_proof_and_collection_probe_keep_all_fields_and_drop() {
    let parsed = crate::check(
        include_str!("../../../../../tests/owned_data/byte_helper_renewal.spx"),
        "helper-child.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let (_, seen) = measure(|| {
        for f in &program.functions {
            let mut a = ReplayPhysicalReservation::new(usize::MAX);
            let mut b = ReplayPhysicalReservation::new(usize::MAX);
            let old = crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work(
                &program,
                f,
                &mut a,
                &mut |_| Ok(()),
            );
            let before_current = crate::test_allocation_observer::current_metrics();
            let current =
                crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(
                    &program,
                    f,
                    &mut b,
                    &mut |_| Ok(()),
                );
            match (old, current) {
                (Ok(old), Ok(current)) => {
                    assert_eq!(old.len(), current.len());
                    let after_current = crate::test_allocation_observer::current_metrics();
                    assert!(
                        b.charged() >= after_current.allocated - before_current.allocated,
                        "current paid {}, requested {}, whole-window peak {}",
                        b.charged(),
                        after_current.allocated - before_current.allocated,
                        after_current.highwater
                    );
                    // Exact current ceiling is the sum of the actual reserved
                    // layouts, independently constrained by whole allocator Drop.
                    let required = b.charged();
                    for limit in [0, required - 1, required] {
                        let mut physical = ReplayPhysicalReservation::new(limit);
                        let result=crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(&program,f,&mut physical,&mut |_|Ok(()));
                        assert_eq!(result.is_ok(), limit == required);
                        if result.is_err() {
                            let first = physical.reserve(0).unwrap_err();
                            assert_eq!(physical.reserve(1).unwrap_err(), first);
                        }
                        drop(result);
                        drop(physical);
                    }
                    let mut required_work = 0usize;
                    let result=crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(&program,f,&mut ReplayPhysicalReservation::new(usize::MAX),&mut |units|{required_work=required_work.checked_add(units).unwrap();Ok(())}).unwrap();
                    drop(result);
                    for allowance in [0, required_work - 1, required_work] {
                        let mut remaining = allowance;
                        let result=crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(&program,f,&mut ReplayPhysicalReservation::new(usize::MAX),&mut |units|{if let Some(next)=remaining.checked_sub(units){remaining=next;Ok(())}else{remaining=0;Err(Refusal::RenewalWork)}});
                        assert_eq!(result.is_ok(), allowance == required_work);
                        assert_eq!(remaining, 0);
                        drop(result);
                    }

                    // Exact held statement identities, not only counts.
                    for (id, binding) in old.borrowed_entries() {
                        assert!(current
                            .get_exact(id)
                            .is_some_and(|current| std::ptr::eq(current, binding)));
                    }
                    drop(current);
                    drop(old);
                }
                (Err(old), Err(current)) => same(&old, &current.legacy()),
                _ => panic!("complete helper verdict differs"),
            }
            let old = crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work(
                &program,
                f,
                &mut ReplayPhysicalReservation::new(0),
                &mut |_| Ok(()),
            );
            let current =
                crate::loop_calls::byte_writer::checked_bindings_with_reservation_and_work_inline(
                    &program,
                    f,
                    &mut ReplayPhysicalReservation::new(0),
                    &mut |_| Ok(()),
                );
            // New current carrier payment can refuse before the old empty fast path.
            // Both actual outcomes remain visible rather than asserting a free path.
            match (old, current) {
                (Err(old), Err(current)) => same(&old, &current.legacy()),
                (Ok(old), Err(Refusal::HelperCensus)) => {
                    assert_eq!(old.len(), 0);
                    drop(old)
                }
                _ => panic!("zero child controls must refuse"),
            }
            drop(a);
            drop(b);
        }
    });
    assert!(!seen.invalid);
    assert_eq!(seen.live, 0);
    assert_eq!(seen.allocated, seen.freed);
    let old =
        crate::cleanup::prepay_selected_collection_probes(&program, usize::MAX, &mut |_| Ok(()))
            .unwrap_err();
    same(&old, &Refusal::CollectionProbe.legacy());
}

mod source;
