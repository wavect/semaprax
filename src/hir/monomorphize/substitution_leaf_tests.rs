//! Full frozen substitution bodies, exact diagnostics and actual receiving Drop.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};

fn leaves() -> Vec<ResolvedType> {
    use ResolvedType::*;
    vec![
        Never,
        Unit,
        I64,
        I32,
        Char,
        U8,
        Usize,
        ArrayU8(0),
        ArrayU8(u32::MAX),
        F32,
        F64,
        Bool,
        String,
        OnceFunction,
        OnceFunctionI64,
        OnceFunctionI64Pair,
        MutFunctionI64,
        Bytes,
        Str,
        SliceU8,
        StringMap,
    ]
}
fn resolved_measure(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    saved: bool,
) -> Metrics {
    measure(|| {
        drop(if saved {
            substitution_saved::substitute_type(template, owner, arguments)
        } else {
            substitute_type(template, owner, arguments)
        })
    })
    .1
}
#[test]
fn resolved_leaf_substitution_keeps_full_old_values_diagnostics_payloads_and_drop() {
    let owner = DeclarationId::new("substitute.owner.完全");
    let foreign = DeclarationId::new("substitute.foreign.α");
    let mut values = leaves();
    values.push(ResolvedType::Nominal {
        declaration: owner.clone(),
        arguments: vec![],
    });
    for value in values {
        assert_eq!(
            substitute_type(&value, &owner, &[]),
            substitution_saved::substitute_type(&value, &owner, &[])
        );
        let old = resolved_measure(&value, &owner, &[], true);
        let (((), current), refused, used, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                drop(substitute_type(&value, &owner, &[]).unwrap());
            })
        });
        assert!(!refused && used == 0 && first.is_none());
        assert!(
            !current.invalid
                && current.allocated == 0
                && current.live == 0
                && current.allocations == 0
        );
        assert!(!old.invalid && old.allocated > 0 && old.live == 0);
        assert_eq!(old.allocated, old.freed);
    }
    let compound = ResolvedType::Function {
        parameters: vec![
            ResolvedType::Nominal {
                declaration: owner.clone(),
                arguments: vec![ResolvedType::Bytes; 16]
            };
            16
        ],
        result: Box::new(ResolvedType::String),
    };
    for actual in [ResolvedType::I64, compound.clone()] {
        for index in [0, 1, u32::MAX] {
            for parameter_owner in [&owner, &foreign] {
                let template = ResolvedType::TypeParameter {
                    owner: parameter_owner.clone(),
                    index,
                };
                assert_eq!(
                    substitute_type(&template, &owner, std::slice::from_ref(&actual)),
                    substitution_saved::substitute_type(
                        &template,
                        &owner,
                        std::slice::from_ref(&actual)
                    )
                );
                let old = resolved_measure(&template, &owner, std::slice::from_ref(&actual), true);
                let current =
                    resolved_measure(&template, &owner, std::slice::from_ref(&actual), false);
                assert!(!old.invalid && !current.invalid && old.live == 0 && current.live == 0);
                assert_eq!(old.allocated, old.freed);
                assert_eq!(current.allocated, current.freed);
                assert!(current.allocated < old.allocated, "temporary carriers only");
                if parameter_owner == &owner && index == 0 && actual == compound {
                    assert!(current.allocated > 0, "owned substituted payload remains");
                }
            }
        }
    }
    // Recursive roots stay on the exact old traversal, including first error
    // priority when foreign and missing parameters occur in different positions.
    for parameters in [
        vec![
            ResolvedType::TypeParameter {
                owner: foreign.clone(),
                index: 0,
            },
            ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 1,
            },
        ],
        vec![
            ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 1,
            },
            ResolvedType::TypeParameter {
                owner: foreign.clone(),
                index: 0,
            },
        ],
        vec![compound.clone()],
    ] {
        let template = ResolvedType::Function {
            parameters,
            result: Box::new(ResolvedType::I64),
        };
        assert_eq!(
            substitute_type(&template, &owner, &[]),
            substitution_saved::substitute_type(&template, &owner, &[])
        );
        assert_eq!(
            resolved_measure(&template, &owner, &[], false),
            resolved_measure(&template, &owner, &[], true)
        );
    }
}
const SOURCE: &str = r#"module substitution.leaf;
@id("substitution.identity") fn identity<T>(value:T)->T {value}
@id("substitution.main") fn main()->i64 {identity<i64>(1)}
"#;
#[test]
fn source_leaf_substitution_preserves_named_generic_priority_and_owned_payload_drop() {
    let ast = crate::parse(SOURCE, "substitution-leaf.spx").unwrap();
    let mut function = ast.functions[0].clone();
    function
        .type_parameters
        .push(function.type_parameters[0].clone());
    let mut types = vec![
        Type::Never,
        Type::I64,
        Type::I32,
        Type::Char,
        Type::U8,
        Type::Usize,
        Type::ArrayU8(u32::MAX),
        Type::F32,
        Type::F64,
        Type::Bool,
        Type::String,
        Type::OnceFunction,
        Type::OnceFunctionI64,
        Type::OnceFunctionI64Pair,
        Type::MutFunctionI64,
        Type::Bytes,
        Type::Str,
        Type::SliceU8,
        Type::StringMap,
    ];
    types.extend([
        Type::Named {
            name: "T".to_owned(),
            arguments: vec![],
        },
        Type::Named {
            name: "foreign.完全".to_owned(),
            arguments: vec![],
        },
        Type::Named {
            name: "T".to_owned(),
            arguments: vec![Type::I64],
        },
        Type::Function {
            parameters: vec![Type::Named {
                name: "T".to_owned(),
                arguments: vec![],
            }],
            result: Box::new(Type::Bool),
        },
    ]);
    let payload = Type::Function {
        parameters: vec![
            Type::Named {
                name: "Owned.完全".to_owned(),
                arguments: vec![Type::Bytes; 16]
            };
            16
        ],
        result: Box::new(Type::String),
    };
    for template in types {
        for arguments in [vec![], vec![Type::I64, Type::String], vec![payload.clone()]] {
            assert_eq!(
                substitute_source_function_type(&function, &arguments, &template),
                substitution_saved::substitute_source_function_type(
                    &function, &arguments, &template
                )
            );
            let (_, old) = measure(|| {
                drop(substitution_saved::substitute_source_function_type(
                    &function, &arguments, &template,
                ))
            });
            let (_, current) = measure(|| {
                drop(substitute_source_function_type(
                    &function, &arguments, &template,
                ))
            });
            assert!(!old.invalid && !current.invalid && old.live == 0 && current.live == 0);
            assert_eq!(old.allocated, old.freed);
            assert_eq!(current.allocated, current.freed);
            match &template {
                Type::Function { .. } => assert_eq!(old, current),
                Type::Named { arguments, .. } if !arguments.is_empty() => assert_eq!(old, current),
                _ => assert!(current.allocated < old.allocated),
            }
            if !matches!(template, Type::Named { .. } | Type::Function { .. }) {
                assert_eq!(current.allocated, 0, "literal inline leaf backing");
            }
        }
    }
    let named = Type::Named {
        name: "T".to_owned(),
        arguments: vec![],
    };
    assert_eq!(
        substitute_source_function_type(&function, &[Type::I64, Type::String], &named),
        Some(Type::I64)
    );
    assert_eq!(
        substitute_source_function_type(&function, &[], &named),
        None
    );
}
