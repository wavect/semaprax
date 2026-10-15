//! Complete saved-owned facts and narrow payload construction/receiving/Drop scope.
use super::*;
use errors::{old, same_owned};
use std::mem::size_of;

fn scalar(id: &str) -> ResolvedExpr {
    let program = program();
    let mut expression = program.functions[0].body.clone();
    expression.id = ExpressionId::from_owned(id.into());
    expression.kind = ResolvedExprKind::Int(0);
    expression.ty = ResolvedType::I64;
    expression.ownership = OwnershipMode::Value;
    expression
}
fn function(kind: ResolvedExprKind) -> ResolvedFunction {
    let mut function = program().functions.remove(0);
    function.requires.clear();
    function.ensures.clear();
    function.body = scalar("payload.body");
    function.body.kind = kind;
    function
}

#[test]
fn all_operation_payloads_preserve_saved_owned_facts_and_exact_current_references() {
    // These are fact-collector inputs, not admission of the hostile HIR below.
    for count in [0, 1, 2, 17] {
        let args = || {
            (0..count)
                .map(|index| scalar(&format!("payload.argument.{index}")))
                .collect()
        };
        let nominal = ResolvedType::Nominal {
            declaration: DeclarationId::new("payload.nominal"),
            arguments: vec![ResolvedType::Nominal {
                declaration: DeclarationId::new("payload.inner"),
                arguments: vec![ResolvedType::U8],
            }],
        };
        let instance = FunctionInstanceId::derive(
            &DeclarationId::new("payload.generic"),
            std::slice::from_ref(&nominal),
        );
        let cases = [
            ResolvedExprKind::Call {
                callee: DeclarationId::new("payload.generic"),
                instance: Some(instance),
                args: args(),
                type_arguments: vec![nominal],
            },
            ResolvedExprKind::Invoke {
                callable: Box::new(scalar("payload.callable")),
                args: args(),
            },
            ResolvedExprKind::ByteRange {
                operation: DeclarationId::new("core.byte.range"),
                source: Box::new(scalar("payload.source")),
                start: Box::new(scalar("payload.start")),
                end: Box::new(scalar("payload.end")),
            },
            ResolvedExprKind::VecFieldRead {
                origin: crate::hir::VecFieldReadOrigin::Explicit,
                element: ResolvedType::I64,
                field: DeclarationId::new("payload.field"),
                bytes: false,
                args: args(),
            },
            ResolvedExprKind::LiteralFormat {
                template: "{} {}".into(),
                args: args(),
            },
            ResolvedExprKind::NativeRustImportCall(crate::hir::ResolvedNativeRustImportCall {
                expression: ExpressionId::from_owned("payload.body".into()),
                import: DeclarationId::new("payload.import"),
                args: args(),
                result: crate::hir::ResolvedImportResultKind::OwnedString,
            }),
            ResolvedExprKind::HostCommandCall(crate::hir::ResolvedHostCommandCall {
                expression: ExpressionId::from_owned("payload.body".into()),
                operation: crate::hir::ResolvedHostCommandOperation::ArgsLen,
                args: args(),
            }),
        ];
        for kind in cases {
            let mut function = function(kind);
            if matches!(
                function.body.kind,
                ResolvedExprKind::NativeRustImportCall(_)
            ) {
                function.body.ownership = OwnershipMode::Own;
            }
            let saved = old::expression_facts(&function).unwrap();
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                let facts = expression_facts(&function).unwrap();
                same_owned(&saved, &facts);
                let fact = facts[&function.body.id].as_ref().unwrap();
                match &function.body.kind {
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        args,
                        type_arguments,
                    } => {
                        assert!(std::ptr::eq(fact.callee.as_ref(), callee));
                        assert!(std::ptr::eq(
                            fact.instance.unwrap(),
                            instance.as_ref().unwrap()
                        ));
                        assert!(std::ptr::eq(
                            fact.type_arguments.as_slice(),
                            type_arguments.as_slice()
                        ));
                        assert_eq!(fact.arguments.len(), count);
                        for (index, arg) in args.iter().enumerate() {
                            assert!(std::ptr::eq(fact.arguments.get(index).unwrap(), &arg.id));
                        }
                        assert!(fact.arguments.get(count).is_none());
                    }
                    ResolvedExprKind::Invoke { callable, .. } => {
                        assert_eq!(fact.type_arguments.len(), 1);
                        assert!(std::ptr::eq(&fact.type_arguments[0], &callable.ty));
                        assert!(fact.instance.is_none());
                    }
                    ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } => {
                        for (index, expression) in [source, start, end].into_iter().enumerate() {
                            assert!(std::ptr::eq(
                                fact.arguments.get(index).unwrap(),
                                &expression.id
                            ));
                        }
                        assert!(fact.arguments.get(3).is_none());
                    }
                    _ => assert!(fact.instance.is_none()),
                }
                // Every variable-length kind exposes exactly its current argument IDs,
                // including an empty slice, without borrowing a substitute directory.
                let current_args = match &function.body.kind {
                    ResolvedExprKind::Call { args, .. }
                    | ResolvedExprKind::Invoke { args, .. }
                    | ResolvedExprKind::VecFieldRead { args, .. }
                    | ResolvedExprKind::LiteralFormat { args, .. } => Some(args),
                    ResolvedExprKind::NativeRustImportCall(call) => Some(&call.args),
                    ResolvedExprKind::HostCommandCall(call) => Some(&call.args),
                    ResolvedExprKind::ByteRange { .. } => None,
                    _ => panic!("complete operation matrix"),
                };
                if let Some(args) = current_args {
                    assert_eq!(fact.arguments.len(), count);
                    for (index, arg) in args.iter().enumerate() {
                        assert!(std::ptr::eq(fact.arguments.get(index).unwrap(), &arg.id));
                    }
                    assert!(fact.arguments.get(count).is_none());
                }
                drop(facts);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
            // Duplicate IDs retain the saved producer's refusal before later child work.
            let mut hostile = function.clone();
            if let Some(child) = expression_children::ReplayChildCursor::new(&function.body).next()
            {
                hostile.body.id = child.id.clone();
            } else {
                // Zero-argument operations have no child to duplicate. Retain the
                // complete body→ensures traversal and duplicate-first refusal.
                hostile.ensures.push(hostile.body.clone());
            }
            let saved = old::expression_facts(&hostile).err().unwrap();
            let current = expression_facts(&hostile).err().unwrap();
            assert_eq!((saved.code, saved.message), (current.code, current.message));
        }
    }
}

#[test]
fn borrowed_payloads_remove_literal_clone_requests_through_receiving_and_drop() {
    let callee = "payload.callee";
    let nominal = "payload.nominal";
    let function = function(ResolvedExprKind::Call {
        callee: DeclarationId::new(callee),
        instance: None,
        args: vec![scalar("payload.a"), scalar("payload.b")],
        type_arguments: vec![ResolvedType::Nominal {
            declaration: DeclarationId::new(nominal),
            arguments: vec![ResolvedType::U8],
        }],
    });
    // Complete independent maps are built outside this payload-only observer.
    // Their node layouts need not match on this or any other target.
    let saved = old::expression_facts(&function).unwrap();
    let current = expression_facts(&function).unwrap();
    same_owned(&saved, &current);
    // Identity carriers have an independent policy (#785 may share their
    // text). Observe that old clone boundary separately; fixed array requests
    // below remain an independently authored literal census.
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        ..
    } = &function.body.kind
    else {
        panic!("call")
    };
    let ResolvedType::Nominal { declaration, .. } = &type_arguments[0] else {
        panic!("nominal")
    };
    let ((), identity_clones) = crate::test_allocation_observer::measure(|| {
        drop(callee.clone());
        drop(declaration.clone());
    });
    let removed =
        2 * size_of::<ExpressionId>() + 2 * size_of::<ResolvedType>() + identity_clones.allocated;
    let (_, old_usage) = crate::test_allocation_observer::measure(|| {
        let fact = saved[&function.body.id].as_ref().unwrap().clone();
        assert_eq!(fact.arguments.len(), 2);
        drop(fact);
    });
    let absent = ExpressionId::from_owned("payload.absent".into());
    let (_, current_usage) = crate::test_allocation_observer::measure(|| {
        let mut budget = ReplayBudget::new();
        let fact = current
            .fact(&function, &function.body.id, &mut budget)
            .unwrap()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone();
        let ResolvedExprKind::Call {
            args,
            type_arguments,
            ..
        } = &function.body.kind
        else {
            panic!("call")
        };
        assert!(fact.arguments.iter().eq(args.iter().map(|arg| &arg.id)));
        assert_eq!(fact.type_arguments.as_slice(), type_arguments.as_slice());
        assert!(fact.instance.is_none());
        assert!(current
            .fact(&function, &absent, &mut budget)
            .unwrap()
            .is_none());
        drop(fact);
        drop(budget);
    });
    assert_eq!(old_usage.allocated - current_usage.allocated, removed);
    assert_eq!(
        old_usage.allocations - current_usage.allocations,
        3 + identity_clones.allocations
    );
    // Conservation: exact current reservations still include the old cloned
    // payload envelope, and one-short refuses before publication as before.
    let mut probe = PhysicalReservation::new(usize::MAX);
    drop(expression_facts_with_reservation(&function, &mut probe).unwrap());
    let required = probe.charged();
    for allowance in [required, required - 1] {
        let mut physical = PhysicalReservation::new(allowance);
        let result = expression_facts_with_reservation(&function, &mut physical);
        assert_eq!(result.is_ok(), allowance == required);
        drop(result);
        if allowance < required {
            assert!(physical.reserve(0).is_err());
            assert!(physical.reserve(1).is_err());
        }
    }
}

#[test]
fn current_fact_payloads_survive_complete_replay_and_drop_before_hir_owner() {
    let program = program();
    let original = program.clone();
    for function in &program.functions {
        let saved = old::expression_facts(function).unwrap();
        let ((), _scope) = crate::test_allocation_observer::measure(|| {
            let facts = expression_facts(function).unwrap();
            same_owned(&saved, &facts);
            // Full ordinary receiver rederives its own map from the same held
            // function. No test map is installed as replay authority.
            validate_structure(&program, function).unwrap();
            drop(facts);
        });
    }
    assert_eq!(program, original);
}

mod arena;
