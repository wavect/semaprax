//! Actual affine report/Preparation/Session lifetime for the shared fact core.
//! Source, active enclosing Budget and observer metadata are outside the scope.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
use std::mem::size_of;
const FORM: ReportForm = ReportForm::new("SPX-T781", "typed expression fact refusal");

fn tiny() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::check(
            "module facts.paid; @id(\"main\") fn main()->i64 {1}",
            "facts.spx",
        )
        .unwrap(),
    )
    .unwrap()
}
fn session<'a>(
    program: &'a dyn ReplayContext,
    function: &'a ResolvedFunction,
    limit: usize,
) -> Session<'a> {
    let acquired = match Acquired::try_acquire(PhysicalReservation::new(limit), FORM) {
        Ok(acquired) => acquired,
        Err(_) => panic!("paid carrier"),
    };
    let preparation = match acquired.prepare(function) {
        Ok(preparation) => preparation,
        Err(_) => panic!("preparation"),
    };
    match preparation.into_paid_session(program, function) {
        Ok(session) => session,
        Err(_) => panic!("session"),
    }
}

#[test]
fn paid_fact_cursor_and_growth_exact_short_keep_first_receipt_and_drop_one_owner() {
    let program = tiny();
    // Standalone resolution retains unused generic prelude declarations. The
    // selected primitive receiver lends the actual authored module projection,
    // exactly as the existing Session owning controls, rather than admitting
    // the unrelated prelude inventory.
    let context = primitive_fixture::Context::new(&program);
    let function = &program.functions[0];
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("block")
    };
    assert!(statements.is_empty());
    assert!(matches!(tail.kind, ResolvedExprKind::Int(1)));
    // Constructor baseline is measured separately, not called an independent
    // formula. Added fact requests are the independent two-node literal census.
    let (_, refused, base) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let session = session(&context, &program.functions[0], usize::MAX);
        let base = session.usage().1;
        drop(session);
        base
    });
    assert!(!refused);
    // Layout twin of FactCapacity's four held fields, not its implementation.
    struct LeaseLayout {
        _epoch: u64,
        _thread: std::marker::PhantomData<*const ()>,
    }
    #[allow(dead_code, reason = "independent physical layout twin")]
    enum PayerLayout<'a> {
        Selected(&'a mut PhysicalReservation),
        ActiveBuilder,
        // The test build retains an epoch and a zero-sized thread-affinity
        // marker for the independently funded oracle payer. Its discriminant
        // also belongs to the actual selected Tree carrier's inline backing.
        Oracle(LeaseLayout),
    }
    assert_eq!(
        size_of::<LeaseLayout>(),
        size_of::<super::super::super::allocation::OracleLease>()
    );
    struct CapacityLayout<'a> {
        _function: &'a ResolvedFunction,
        _physical: PayerLayout<'a>,
        _types: [Option<(&'a ResolvedType, usize)>; 514],
        _work: usize,
    }
    type Frame<'a> = Option<(expression_children::ReplayChildCursor<'a>, bool)>;
    let cursor = size_of::<CapacityLayout<'_>>() + 514 * size_of::<Frame<'_>>();
    let node = physical_storage::btree_node_upper::<ExpressionId, Option<CallFact>>().unwrap();
    let full = base + cursor + 2 * node;
    for (limit, success) in [
        (base, false),
        (base + cursor - 1, false),
        (full - 1, false),
        (full, true),
    ] {
        let (_, overflowed, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let (_, observed) = measure(|| {
                let mut selected = session(&context, &program.functions[0], limit);
                let backing = selected.report.as_ref().unwrap().backing();
                let text = selected.budget.refusal_text.as_ref().unwrap().as_ptr();
                let ledger = selected.budget.physical.as_deref().unwrap() as *const _;
                match expression_facts::paid(function, &mut selected.budget) {
                    Ok(facts) => {
                        assert!(success);
                        assert_eq!(facts.len(), 2);
                        assert_eq!(selected.usage().1, full);
                        drop(facts);
                        drop(
                            selected
                                .finish_paid()
                                .unwrap_or_else(|_| panic!("successful paid finish")),
                        );
                    }
                    Err(error) => {
                        assert!(!success);
                        let error_pointer = match &error {
                            expression_facts::errors::EngineError::Owned(error) => {
                                assert_eq!(error.code, "SPX-H006");
                                assert!(error.message.contains("physical expression-fact"));
                                error.message.as_ptr()
                            }
                            _ => panic!("physical refusal moves paid diagnostic"),
                        };
                        assert_eq!(error_pointer, text);
                        let before_retry = current_metrics();
                        selected.failed = true;
                        assert!(matches!(
                            selected.validate_scalar_paid(function),
                            Err(expression_facts::errors::EngineError::Failed)
                        ));
                        assert_eq!(current_metrics().allocations, before_retry.allocations);
                        let mut refusal = selected.into_failed_preparation().refuse_engine(error);
                        assert_eq!(refusal.report().unwrap().as_ptr(), backing.0);
                        assert_eq!(refusal.report().unwrap()[0].message.as_ptr(), backing.1);
                        assert_eq!(refusal.receipt().unwrap().message.as_ptr(), text);
                        assert_eq!(refusal.ledger_mut().unwrap() as *const _, ledger);
                        assert!(refusal.ledger_mut().unwrap().reserve(0).is_err());
                        let before_report = current_metrics();
                        drop(
                            refusal
                                .into_report(FORM)
                                .unwrap_or_else(|_| panic!("same outward form")),
                        );
                        assert_eq!(current_metrics().allocations, before_report.allocations);
                    }
                }
            });
            assert!(!observed.invalid);
            assert_eq!(observed.allocated, observed.freed);
            assert_eq!(observed.live, 0);
        });
        assert!(!overflowed);
    }
}

#[test]
fn paid_fact_semantic_receipt_keeps_exact_shared_ids_and_driver_verdicts() {
    let program = tiny();
    // Standalone resolution retains unused generic prelude declarations. The
    // selected primitive receiver lends the actual authored module projection,
    // exactly as the existing Session owning controls, rather than admitting
    // the unrelated prelude inventory.
    let context = primitive_fixture::Context::new(&program);
    let mut duplicate = program.functions[0].clone();
    let root = duplicate.body.id.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut duplicate.body.kind else {
        panic!("block")
    };
    tail.id = root.clone();
    let (_, overflowed, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let (_, observed) = measure(|| {
            let mut selected = session(&context, &program.functions[0], usize::MAX);
            let original_ledger = selected.budget.physical.as_deref().unwrap() as *const _;
            let original_report = selected.report.as_ref().unwrap().backing();
            // This producer census is not HIR admission. It is reached by both
            // paid drivers; forged HIR still fails their prior validators.
            let error = expression_facts::paid(&duplicate, &mut selected.budget)
                .err()
                .unwrap();
            let before = current_metrics();
            let mut refusal = selected.into_failed_preparation().refuse_engine(error);
            assert!(
                refusal.receipt().is_none(),
                "semantic data is not fake paid text"
            );
            let receipt = refusal.fact_receipt().unwrap();
            assert_eq!(receipt.source, root);
            assert_eq!(receipt.source.as_str().as_ptr(), root.as_str().as_ptr());
            assert_eq!(
                receipt.expression.as_ref().unwrap().as_str().as_ptr(),
                root.as_str().as_ptr()
            );
            assert_eq!(receipt.expression.as_ref(), Some(&root));
            assert_eq!(refusal.report().unwrap().as_ptr(), original_report.0);
            assert_eq!(refusal.ledger_mut().unwrap() as *const _, original_ledger);
            drop(refusal.into_report(FORM).ok().unwrap());
            assert_eq!(current_metrics().allocations, before.allocations);
        });
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
        // Actual complete scalar and lexical driver success/work/backing parity.
        let mut old = session(&context, &program.functions[0], usize::MAX);
        let mut paid = session(&context, &program.functions[0], usize::MAX);
        old.validate_scalar(&program.functions[0]).unwrap();
        paid.validate_scalar_paid(&program.functions[0]).unwrap();
        assert_eq!(old.usage(), paid.usage());
        let lexical = super::program();
        let lexical_context = primitive_fixture::Context::new(&lexical);
        let function = super::members(&lexical)[0];
        let acquire = || {
            let acquired = Acquired::try_acquire(PhysicalReservation::new(usize::MAX), FORM)
                .ok()
                .unwrap();
            acquired
                .prepare(function)
                .ok()
                .unwrap()
                .into_paid_session(&lexical_context, function)
                .ok()
                .unwrap()
        };
        let mut old = acquire();
        let mut paid = acquire();
        old.validate(function).unwrap();
        paid.validate_paid(function).unwrap();
        assert_eq!(old.usage(), paid.usage());
    });
    assert!(!overflowed);
}

#[path = "fact_errors/arena.rs"]
mod arena;

#[path = "fact_errors/tree_policy.rs"]
mod tree_policy;
