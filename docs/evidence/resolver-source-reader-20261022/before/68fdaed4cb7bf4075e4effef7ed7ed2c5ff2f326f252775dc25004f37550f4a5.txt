//! Byte-receiver construction against the actual allocator and the live
//! builder ledger: exact retained backing, complete Drop, an exact limit, a
//! one-short sticky refusal, and an up-front refusal that constructs nothing.
use super::*;
use crate::bounded_output::{active_refused, with_limit_usage};
use crate::hir::{DeclarationId, FunctionInstanceId};
use crate::test_allocation_observer::measure;

const STATEMENT: &str = "body.s1";
const ITEM_VALUE: &str = "body.s1.value.s2.body.s0.value";

fn owners() -> [FunctionExecutionId; 2] {
    [
        FunctionExecutionId::Monomorphic(DeclarationId::new("app.bytes.sum")),
        FunctionExecutionId::Generic(FunctionInstanceId("λ-bytes-instance".to_owned())),
    ]
}

fn bindings(function: &FunctionExecutionId) -> (ResolvedBinding, ResolvedBinding, ResolvedExpr) {
    let span = Span::default();
    let view = ResolvedBinding {
        id: ValueId::parameter(function, 0),
        name: "view".to_owned(),
        ownership: OwnershipMode::Borrow,
        ty: ResolvedType::SliceU8,
        span,
    };
    let counter = ResolvedBinding {
        id: ValueId::local(function, "body.s1.value.s1"),
        name: "#for-index".to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Usize,
        span,
    };
    let body = ResolvedExpr {
        id: ExpressionId::new(function, "body.s1.value.s2.body.s1.value"),
        ty: ResolvedType::Usize,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Usize(0),
        span,
    };
    (view, counter, body)
}

/// The five declaration identities one receiver constructs, in order.
fn declarations(receiver: &ResolvedExpr) -> [&DeclarationId; 5] {
    let ResolvedExprKind::Match {
        scrutinee, arms, ..
    } = &receiver.kind
    else {
        panic!("receiver match")
    };
    let ResolvedType::Nominal { declaration, .. } = &scrutinee.ty else {
        panic!("Option<u8> result type")
    };
    let ResolvedExprKind::Call { callee, .. } = &scrutinee.kind else {
        panic!("byte_get call")
    };
    let [ResolvedMatchArm {
        pattern:
            ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            },
        ..
    }] = arms.as_slice()
    else {
        panic!("one Some arm")
    };
    [callee, declaration, variant, case, &fields[0].field]
}

#[test]
fn receiver_declarations_are_compiler_identities_equal_to_owned_text() {
    let function = &owners()[0];
    let (view, counter, _) = bindings(function);
    let span = Span::default();
    // Independent owned-text oracle, in receiver order.
    let oracle = [
        crate::byte_ops::GET_ID,
        crate::prelude::OPTION_ID,
        crate::prelude::OPTION_ID,
        crate::prelude::OPTION_SOME_ID,
        crate::prelude::OPTION_SOME_VALUE_ID,
    ]
    .map(|text| DeclarationId::new(text.to_owned()));
    let receiver = super::receiver(function, ITEM_VALUE, &view, &counter, span, span);
    for (declaration, oracle) in declarations(&receiver).into_iter().zip(&oracle) {
        assert!(declaration.is_compiler_static());
        assert_eq!(declaration.owned_allocation_bytes(), 0);
        assert_eq!(declaration, oracle);
        assert_eq!(declaration.as_str(), oracle.as_str());
    }
}

#[test]
fn receiver_retains_exactly_its_debited_backing_and_drops_all_of_it() {
    for function in owners() {
        let (view, counter, _) = bindings(&function);
        let span = Span::default();
        let backing = receiver_backing(&function, ITEM_VALUE.len()).unwrap();
        let ((), complete) = measure(|| {
            let receiver = super::receiver(&function, ITEM_VALUE, &view, &counter, span, span);
            // Five shared identity carriers plus the independently computed
            // retained receiver backing. Inspect while the owner is still live.
            assert_eq!(
                receiver_backing(&function, ITEM_VALUE.len()).unwrap(),
                backing
            );
            assert!(crate::hir::is_partial_byte_receiver(&receiver));
            let retained = crate::test_allocation_observer::current_metrics();
            assert!(!retained.invalid);
            assert_eq!(
                retained.live,
                backing + 5 * ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES
            );
            let allocations = retained.allocations;
            drop(receiver);
            let settled = crate::test_allocation_observer::current_metrics();
            assert_eq!(settled.allocations, allocations, "Drop adds no allocation");
            assert_eq!(settled.live, 0);
        });
        assert!(!complete.invalid);
        assert_eq!(complete.live, 0);
        assert_eq!(complete.freed, complete.allocated);

        let ((), transient) = measure(|| {
            drop(super::receiver(
                &function, ITEM_VALUE, &view, &counter, span, span,
            ))
        });
        assert!(!transient.invalid);
        assert_eq!(transient.live, 0);
        assert_eq!(transient.freed, transient.allocated);
    }
}

#[test]
fn byte_lowering_is_exact_one_short_sticky_and_refuses_before_construction() {
    for function in owners() {
        let (view, counter, body) = bindings(&function);
        let span = Span::default();
        let lower = || {
            super::lower(
                &function,
                STATEMENT,
                "byte",
                span,
                &view,
                body.clone(),
                span,
            )
        };
        let backing = receiver_backing(&function, ITEM_VALUE.len()).unwrap();

        let (admitted, overflowed, used) = with_limit_usage(1 << 20, lower);
        assert!(admitted.is_some());
        assert!(!overflowed);
        // Observe the complete NEW receiver constructor through Drop. The
        // shared sixteen-node For profile has its separate AST prebound; this
        // observation does not mislabel that existing payer as receiver debit.
        assert!(used > backing);
        let (((), allocations), overflowed, measured_used) = with_limit_usage(1 << 20, || {
            assert!(crate::bounded_output::reserve_active_required(backing));
            measure(|| {
                drop(super::receiver(
                    &function, ITEM_VALUE, &view, &counter, span, span,
                ))
            })
        });
        let mut expected = backing + 5 * ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES;
        for suffix in ["", SCRUTINEE, VIEW_ARGUMENT, INDEX_ARGUMENT, ARM_VALUE] {
            expected += crate::hir::ids::streamed::scoped_len(
                &function,
                "expression",
                ITEM_VALUE.len() + suffix.len(),
            )
            .unwrap();
        }
        expected += crate::hir::ids::streamed::scoped_len(
            &function,
            "value:local",
            ITEM_VALUE.len() + PAYLOAD_BINDING.len(),
        )
        .unwrap();
        assert!(!overflowed);
        assert_eq!(measured_used, expected);
        assert!(!allocations.invalid);
        assert!(
            allocations.allocated <= expected,
            "actual cumulative receiver requests {} exceed debit {}",
            allocations.allocated,
            expected
        );
        assert_eq!(allocations.live, 0);
        assert_eq!(allocations.allocated, allocations.freed);

        let (exact, overflowed, exact_used) = with_limit_usage(used, lower);
        assert!(exact.is_some());
        assert!(!overflowed);
        assert_eq!(exact_used, used);
        assert_eq!(exact, admitted);

        // One byte short: some identity carrier is refused, and the refusal
        // stays selected for every later reservation in this budget.
        let (sticky, overflowed, short_used) = with_limit_usage(used - 1, || {
            let lowered = lower();
            (
                lowered,
                active_refused(),
                crate::bounded_output::reserve_active_required(0),
            )
        });
        assert!(overflowed);
        assert!(sticky.1);
        assert!(!sticky.2);
        assert!(short_used < used);

        // Short of the receiver backing itself: refused before any traversal
        // carrier, identity or path text is allocated.
        let ((refused, metrics), overflowed, refused_used) =
            with_limit_usage(backing - 1, || measure(lower));
        assert!(refused.is_none());
        assert!(overflowed);
        assert_eq!(refused_used, 0);
        assert!(!metrics.invalid);
        assert_eq!(metrics.allocations, 0);
    }
}

#[test]
fn byte_lowering_constructs_its_own_snapshot_and_receiver_in_place() {
    let function = &owners()[0];
    let (view, _, body) = bindings(function);
    let span = Span::default();
    let ResolvedStatement::Let { binding, value, .. } =
        super::lower(function, STATEMENT, "byte", span, &view, body, span).unwrap()
    else {
        panic!("wrapper let")
    };
    assert_eq!(binding.name, WRAPPER);
    let ResolvedExprKind::Block { statements, .. } = &value.kind else {
        panic!("wrapper block")
    };
    let ResolvedStatement::Let {
        binding: length,
        value: count,
        ..
    } = &statements[0]
    else {
        panic!("snapshot")
    };
    assert_eq!(length.name, LENGTH);
    assert!(
        matches!(&count.kind, ResolvedExprKind::Call { callee, type_arguments, .. }
        if callee.as_str() == crate::byte_ops::LEN_ID && type_arguments.is_empty())
    );
    let ResolvedStatement::While { body, .. } = &statements[2] else {
        panic!("exit-free loop")
    };
    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        panic!("loop body")
    };
    let ResolvedStatement::Let {
        binding: item,
        value: read,
        ..
    } = &statements[0]
    else {
        panic!("item")
    };
    assert_eq!(item.ty, ResolvedType::U8);
    assert_eq!(
        read.id.as_str(),
        ExpressionId::new(function, ITEM_VALUE).as_str()
    );
    assert!(crate::hir::is_partial_byte_receiver(read));
    assert!(crate::hir::byte_traversal_region(&value).unwrap().is_some());
}
