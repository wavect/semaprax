//! Actual payload constructors, every byte boundary, sticky refusal and full Drop.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
const LIMIT: usize = 64 * 1024 * 1024;
fn rich() -> ResolvedExpr {
    let source = "module clone.closure; @id(\"main\") fn main()->i64{0}";
    let program = crate::hir::resolve(&crate::check(source, "clone.spx").unwrap()).unwrap();
    let mut value = program.functions[0].body.clone();
    let leaf = value.clone();
    let binding = ResolvedBinding {
        id: ValueId::new("copy.binding".to_owned()),
        name: "complete owned binding name".to_owned(),
        ownership: OwnershipMode::Value,
        ty: ResolvedType::Function {
            parameters: vec![ResolvedType::I64; 3],
            result: Box::new(ResolvedType::Bool),
        },
        span: leaf.span,
    };
    let mut call = leaf.clone();
    call.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new("retained.call"),
        type_arguments: vec![binding.ty.clone()],
        instance: Some(FunctionInstanceId("retained.instance".to_owned())),
        args: vec![leaf.clone(), leaf.clone()],
    };
    let mut unary = leaf.clone();
    unary.kind = ResolvedExprKind::Unary {
        op: crate::ast::UnaryOp::Neg,
        value: Box::new(call),
    };
    let mut binary = leaf.clone();
    binary.kind = ResolvedExprKind::Binary {
        op: crate::ast::BinaryOp::Add,
        left: Box::new(unary),
        right: Box::new(leaf.clone()),
    };
    let mut branch = leaf.clone();
    branch.kind = ResolvedExprKind::If {
        condition: Box::new(leaf.clone()),
        then_branch: Box::new(binary),
        else_branch: Box::new(leaf.clone()),
    };
    value.ty = binding.ty.clone();
    value.kind = ResolvedExprKind::VecConstructor {
        element: binding.ty.clone(),
        count: 7,
        statements: vec![
            ResolvedStatement::Let {
                binding: binding.clone(),
                mutable: true,
                value: leaf.clone(),
                span: leaf.span,
            },
            ResolvedStatement::Assign {
                binding,
                field: Some(DeclarationId::new("retained.field")),
                value: leaf.clone(),
                span: leaf.span,
            },
            ResolvedStatement::While {
                condition: Box::new(leaf.clone()),
                body: Box::new(leaf.clone()),
                span: leaf.span,
            },
        ],
        tail: Box::new(branch),
    };
    value
}
fn lifetime(
    input: &ResolvedExpr,
    limit: usize,
) -> (bool, crate::test_allocation_observer::Metrics, bool, usize) {
    let ((accepted, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let result = copy::expression(input);
                let accepted = result.is_ok();
                if let Ok(value) = &result {
                    assert_eq!(value, input);
                } else {
                    assert!(crate::bounded_output::active_refused());
                    let before = current_metrics();
                    let retry = copy::expression(input);
                    assert!(retry.is_err());
                    drop(retry);
                    let after = current_metrics();
                    assert_eq!(after.allocations, before.allocations);
                    assert_eq!(after.allocated, before.allocated);
                }
                let remaining = crate::bounded_output::active_remaining();
                drop(result);
                assert_eq!(remaining, crate::bounded_output::active_remaining());
                accepted
            })
        });
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= paid && observed.highwater <= paid);
    (accepted, observed, refused, paid)
}
#[test]
fn complete_literal_payload_clone_exact_short_sticky_and_every_paid_boundary() {
    let input = rich();
    let prior = input.clone();
    let (accepted, whole, refused, paid) = lifetime(&input, LIMIT);
    assert!(accepted && !refused && paid > 0);
    let (accepted, exact, refused, exact_paid) = lifetime(&input, paid);
    assert!(accepted && !refused);
    assert_eq!(whole, exact);
    assert_eq!(paid, exact_paid);
    for limit in 0..paid {
        let (accepted, _, refused, used) = lifetime(&input, limit);
        assert!(!accepted && refused && used <= limit);
        assert_eq!(input, prior);
    }
}
#[test]
fn paid_identity_and_result_keep_full_original_spelling_and_drop() {
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("closure.owner"));
    let expression = ExpressionId::new(&owner, "body");
    let expected = crate::hir::closure::closure_id(&expression);
    let expected_result = ValueId::result(&FunctionExecutionId::Monomorphic(expected.clone()));
    let (((), observed), refused, paid) = crate::bounded_output::with_limit_usage(LIMIT, || {
        measure(|| {
            let id = identity(&expression).unwrap();
            let result = result(&id).unwrap();
            assert_eq!(id, expected);
            assert_eq!(result, expected_result);
            drop(result);
            drop(id);
        })
    });
    assert!(!refused && !observed.invalid && paid > 0);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= paid && observed.highwater <= paid);
}
#[test]
fn complete_closure_source_join_preserves_original_product_and_recursive_shape_inventory() {
    let source = concat!(
        include_str!("../../closure.rs"),
        include_str!("../validation.rs"),
        include_str!("../once.rs"),
        include_str!("../mutable.rs"),
        include_str!("../borrowed.rs"),
        include_str!("../paid.rs"),
        include_str!("copy.rs"),
        include_str!("tests.rs"),
        include_str!("original_tests.rs")
    );
    for marker in [
        "pub fn closure_function(",
        "body: *body.clone()",
        "pending.push(tail)",
        "ResolvedStatement::While",
        "other => other.clone()",
        "copy::expression(body)?",
    ] {
        assert!(
            source.contains(marker),
            "complete closure source lost {marker}"
        );
    }
}

#[test]
fn payload_copy_unwind_drops_real_owned_product_and_message_inside_observation() {
    let input = rich();
    let prior = input.clone();
    let ((caught, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(LIMIT, || {
            measure(|| {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _owned = copy::expression(&input).unwrap();
                    std::panic::resume_unwind(Box::new(String::from("closure payload unwind")));
                }));
                let error = caught.is_err();
                assert_eq!(
                    caught
                        .as_ref()
                        .unwrap_err()
                        .downcast_ref::<String>()
                        .unwrap(),
                    "closure payload unwind"
                );
                drop(caught);
                error
            })
        });
    assert!(caught && !refused && !observed.invalid && paid > 0);
    assert_eq!(input, prior);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    // The injected test panic payload has its own allocation, outside the
    // production copier's funded domain. No aggregate funding claim here.
}

#[test]
fn complete_future_profile_fallback_retains_original_payload_without_new_refusal() {
    let mut input = rich();
    input.kind = ResolvedExprKind::String("complete legacy future payload".to_owned());
    let original = input.clone();
    let current = copy::expression(&input).unwrap();
    assert_eq!(current, original);
    // Unpaid legacy fallback is explicitly outside the funded body inventory.
}

#[test]
fn census_residual_preserves_full_legacy_type_and_sticky_denial_prevents_clone() {
    let mut input = ResolvedType::I64;
    for _ in 0..600 {
        input = ResolvedType::Function {
            parameters: Vec::new(),
            result: Box::new(input),
        };
    }
    assert_eq!(input.clone_payload_bytes(), None);
    assert_eq!(ty(&input).unwrap(), input.clone());
    let (((), observed), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        measure(|| {
            let result = ty(&input);
            assert!(result.is_err());
            drop(result);
        })
    });
    assert!(refused);
    assert_eq!(paid, 0);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
    // The successful legacy clone above is deliberately outside a funding
    // observation: reserve(0) never asserts its backing was acquired.
}
#[test]
fn canonical_identity_result_every_paid_boundary_retains_inputs_and_full_drop() {
    let expression = ExpressionId::new(
        &FunctionExecutionId::Monomorphic(DeclarationId::new("closure.owner")),
        "body",
    );
    let prior = expression.clone();
    let receive = |limit| {
        let ((ok, metrics), refused, paid) = crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let result = identity(&expression).and_then(|id| result(&id));
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    let before = current_metrics();
                    let retry = identity(&expression);
                    assert!(retry.is_err());
                    drop(retry);
                    let after = current_metrics();
                    assert_eq!(before.allocations, after.allocations);
                    assert_eq!(before.allocated, after.allocated);
                }
                ok
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
        (ok, metrics, refused, paid)
    };
    let (ok, whole, refused, paid) = receive(LIMIT);
    assert!(ok && !refused && paid > 0);
    let (ok, exact, refused, exact_paid) = receive(paid);
    assert!(ok && !refused);
    assert_eq!(whole, exact);
    assert_eq!(paid, exact_paid);
    for limit in 0..paid {
        let (ok, _, refused, used) = receive(limit);
        assert!(!ok && refused && used <= limit);
        assert_eq!(expression, prior);
    }
}
