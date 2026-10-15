use super::*;
use crate::test_allocation_observer::measure;
fn program() -> ResolvedProgram {
    let owner = include_str!("../function_scope/tests.rs");
    let source = owner
        .split_once("const CONTRACT_SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    crate::hir::resolve(&crate::check(source, "copy-call.spx").unwrap()).unwrap()
}
#[test]
fn complete_argument_receiver_keeps_every_ownership_mode_type_span_and_parameter_identity() {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let seed = program
        .functions
        .iter()
        .flat_map(|f| &f.params)
        .next()
        .unwrap();
    for function in &program.functions {
        for ownership in [
            OwnershipMode::Value,
            OwnershipMode::Own,
            OwnershipMode::Borrow,
            OwnershipMode::Shared,
        ] {
            let mut argument = function.body.clone();
            argument.ownership = ownership;
            for mode in [
                OwnershipMode::Value,
                OwnershipMode::Own,
                OwnershipMode::Borrow,
                OwnershipMode::Shared,
            ] {
                let mut param = seed.clone();
                param.ty = function.body.ty.clone();
                param.ownership = mode;
                let old = validator.validate_argument_ownership_view_before(
                    &argument,
                    ParameterView::owned(&param),
                );
                let current = validator
                    .validate_argument_ownership_view(&argument, ParameterView::owned(&param));
                assert_eq!(current, old);
            }
        }
    }
}
#[test]
fn actual_argument_copy_receiver_removes_layout_allocation_preserves_old_and_sticky_zero() {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let mut param = program
        .functions
        .iter()
        .flat_map(|f| &f.params)
        .next()
        .unwrap()
        .clone();
    param.ty = ResolvedType::I64;
    param.ownership = OwnershipMode::Value;
    let (((), current), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            measure(|| {
                for _ in 0..17 {
                    validator
                        .validate_argument_ownership_view(&main.body, ParameterView::owned(&param))
                        .unwrap();
                }
            })
        });
    assert!(!refused && paid == 0 && first.is_none());
    assert!(!current.invalid && current.live == 0 && current.allocated == 0);
    let ((), old) = measure(|| {
        let _old = ownership_flags::original_guard();
        for _ in 0..17 {
            validator
                .validate_argument_ownership_view(&main.body, ParameterView::owned(&param))
                .unwrap();
        }
    });
    assert!(!old.invalid && old.live == 0 && old.allocated == old.freed);
    assert!(old.allocated > current.allocated);
    let (((), cost), refused, paid, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            measure(|| {
                assert!(validator
                    .validate_argument_ownership_view(&main.body, ParameterView::owned(&param))
                    .is_err())
            })
        });
    assert!(refused && paid == 0 && first.unwrap().requested == 1);
    assert!(!cost.invalid && cost.live == 0 && cost.allocated == 0);
}
#[test]
fn complete_original_argument_body_is_compiled_exactly_with_only_entry_name_changed() {
    let archived = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ownership-copy-flags/validate_argument_ownership_view.before.txt"
    ));
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(archived.as_bytes()))
        ),
        "c89a9ba94b04a69f60e49e56261c497cd1033a2f2c70051625fe6fc3679343a8"
    );
    assert!(include_str!("copy_before.rs").contains(&archived.replace(
        "fn validate_argument_ownership_view(",
        "fn validate_argument_ownership_view_before("
    )));
}

#[test]
fn full_original_byte_corpus_keeps_exact_place_and_intrinsic_borrow_diagnostics() {
    let owner = include_str!("tests.rs");
    let source = owner
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    let program =
        crate::hir::resolve(&crate::parse(source, "copy-byte-corpus.spx").unwrap()).unwrap();
    let validator = HirValidator::new(&program).unwrap();
    let mut saw_place = false;
    for function in &program.functions {
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Call {
                callee,
                args: arguments,
                ..
            } = &expression.kind
            {
                if let Some(operation) = crate::byte_ops::by_id(callee.as_str()) {
                    let view = CallParameters::Byte(operation);
                    for (index, argument) in arguments.iter().enumerate() {
                        saw_place |= matches!(argument.kind, ResolvedExprKind::Place(_));
                        let before = validator.validate_argument_ownership_view_before(
                            argument,
                            view.parameter(index),
                        );
                        let current = validator
                            .validate_argument_ownership_view(argument, view.parameter(index));
                        assert_eq!(current, before);
                        let mut hostile = argument.clone();
                        hostile.ownership = OwnershipMode::Shared;
                        assert_eq!(
                            validator
                                .validate_argument_ownership_view(&hostile, view.parameter(index)),
                            validator.validate_argument_ownership_view_before(
                                &hostile,
                                view.parameter(index)
                            ),
                        );
                    }
                }
            }
            crate::hir::push_resolved_expression_children_in_authored_order(
                expression,
                &mut pending,
            );
        }
    }
    assert!(saw_place);
}
