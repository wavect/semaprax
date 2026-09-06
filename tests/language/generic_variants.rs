use std::path::Path;

use semaprax::hir::{
    self, IdentityOrigin, ResolvedExprKind, ResolvedMatchPattern, ResolvedType,
    ResolvedTypeDeclarationKind,
};
use semaprax::{format, graph, parse, verify};

const SOURCE: &str = r#"
module test.generic_variants;

@id("test.choice")
variant Choice<T> {
    @id("test.choice.none")
    None,
    @id("test.choice.value")
    Value {
        @id("test.choice.value.value")
        value: T,
    },
}

@id("test.choice_i64")
fn choice_i64(choice: Choice<i64>) -> i64 {
    match choice {
        Choice::Value { value } => value,
        Choice::None {} => 0,
    }
}

@id("test.choice_bool")
fn choice_bool(choice: Choice<bool>) -> i64 {
    match choice {
        Choice::Value { value } => if value { 1 } else { 0 },
        Choice::None {} => 0,
    }
}

@id("test.option_i64")
fn option_i64(option: Option<i64>) -> i64 {
    match option {
        Option::Some { value } => value,
        Option::None {} => 0,
    }
}

@id("test.result_i64_bool")
fn result_i64_bool(outcome: Result<i64, bool>) -> i64 {
    match outcome {
        Result::Ok { value } => value,
        Result::Err { error } => if error { 1 } else { 0 },
    }
}

@id("app.main")
fn main() -> i64 {
    let none = Option<i64>::None {};
    let okay = Result<i64, bool>::Ok { value: 1 };
    choice_i64(Choice<i64>::Value { value: 42 })
}
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("generic-variants.spx")).unwrap()
}

fn error_codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("generic-variant-error.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn generic_variants_and_ordinary_prelude_have_a_canonical_human_projection() {
    let program = program();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("generic-variants-canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert!(canonical.contains("variant Choice<T> {"));
    assert!(canonical.contains("choice: Choice<i64>"));
    assert!(canonical.contains("Choice<i64>::Value { value: 42 }"));
    assert!(canonical.contains("Option<i64>::None {}"));
    assert!(canonical.contains("Result<i64, bool>::Ok { value: 1 }"));
    assert!(canonical.contains("Option::Some { value } => value,"));
    assert!(!canonical.contains("variant Option"));
    assert!(!canonical.contains("variant Result"));
}

#[test]
fn generic_hir_uses_owner_index_parameters_and_concrete_substitution() {
    let resolved = hir::resolve(&program()).unwrap();
    hir::validate(&resolved).unwrap();

    let choice = resolved
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "test.choice")
        .unwrap();
    assert_eq!(choice.type_parameters[0].name, "T");
    assert_eq!(choice.type_parameters[0].index, 0);
    let ResolvedTypeDeclarationKind::Variant { cases } = &choice.kind else {
        panic!("Choice must remain a variant template");
    };
    assert_eq!(
        cases[1].fields[0].ty,
        ResolvedType::TypeParameter {
            owner: choice.id.clone(),
            index: 0,
        }
    );

    for (id, parameters) in [("core.option", 1), ("core.result", 2)] {
        let declaration = resolved
            .types
            .iter()
            .find(|declaration| declaration.id.as_str() == id)
            .unwrap();
        assert_eq!(declaration.type_parameters.len(), parameters);
        assert_eq!(
            resolved
                .declarations
                .declaration(&declaration.id)
                .unwrap()
                .identity_origin,
            IdentityOrigin::CompilerOwned
        );
    }

    let main = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &main.body.kind else {
        panic!("function body must be a block");
    };
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        panic!("main tail must call choice_i64");
    };
    let ResolvedExprKind::ConstructVariant {
        variant,
        case,
        fields,
    } = &args[0].kind
    else {
        panic!("call argument must construct Choice<i64>");
    };
    assert_eq!(variant.as_str(), "test.choice");
    assert_eq!(case.as_str(), "test.choice.value");
    assert_eq!(fields[0].value.ty, ResolvedType::I64);
    assert_eq!(
        args[0].ty,
        ResolvedType::Nominal {
            declaration: choice.id.clone(),
            arguments: vec![ResolvedType::I64],
        }
    );

    let option = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.option_i64")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &option.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        panic!("option body must resolve as match");
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[0].pattern else {
        panic!("first Option arm must be a variant pattern");
    };
    assert_eq!(fields[0].binding.ty, ResolvedType::I64);
}

#[test]
fn independent_hir_validation_rejects_generic_instance_and_prelude_mutations() {
    let mut wrong_constructor_instance = hir::resolve(&program()).unwrap();
    let choice_id = wrong_constructor_instance
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "test.choice")
        .unwrap()
        .id
        .clone();
    let main = wrong_constructor_instance
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut main.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        unreachable!();
    };
    args[0].ty = ResolvedType::Nominal {
        declaration: choice_id,
        arguments: vec![ResolvedType::Bool],
    };
    assert_eq!(
        hir::validate(&wrong_constructor_instance).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_binding = hir::resolve(&program()).unwrap();
    let function = wrong_binding
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.choice_bool")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        unreachable!();
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[0].pattern else {
        unreachable!();
    };
    fields[0].binding.ty = ResolvedType::I64;
    assert_eq!(hir::validate(&wrong_binding).unwrap_err().code, "SPX-H006");

    let mut wrong_prelude_tags = hir::resolve(&program()).unwrap();
    let option = wrong_prelude_tags
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "core.option")
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut option.kind else {
        unreachable!();
    };
    cases.swap(0, 1);
    assert_eq!(
        hir::validate(&wrong_prelude_tags).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn generic_admission_is_explicit_arity_checked_and_copy_scalar_only() {
    let declaration = r#"
@id("test.choice")
variant Choice<T> {
    @id("test.choice.value") Value {
        @id("test.choice.value.value") value: T,
    },
}
"#;
    let missing_constructor_arguments = format!(
        "module test.missing_constructor;\n{declaration}\n@id(\"app.main\") fn main() -> i64 {{ let choice = Choice::Value {{ value: 1 }}; 0 }}"
    );
    assert!(error_codes(&missing_constructor_arguments).contains(&"SPX-T221"));

    let wrong_arity = format!(
        "module test.wrong_arity;\n{declaration}\n@id(\"test.take\") fn take(choice: Choice<i64, bool>) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert_eq!(error_codes(&wrong_arity), ["SPX-T221"]);

    let nested = format!(
        "module test.nested;\n{declaration}\n@id(\"test.take\") fn take(choice: Choice<Option<i64>>) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert_eq!(error_codes(&nested), ["SPX-T223"]);

    let unknown_parameter = format!(
        "module test.unknown_parameter;\n{}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}",
        declaration.replace("value: T", "value: U")
    );
    assert_eq!(error_codes(&unknown_parameter), ["SPX-T220"]);

    let duplicate_parameter = format!(
        "module test.duplicate_parameter;\n{}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}",
        declaration.replace("variant Choice<T>", "variant Choice<T, T>")
    );
    assert_eq!(error_codes(&duplicate_parameter), ["SPX-T220"]);

    let nested_template = format!(
        "module test.nested_template;\n{}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}",
        declaration.replace("value: T", "value: Option<T>")
    );
    assert_eq!(error_codes(&nested_template), ["SPX-T223", "SPX-T215"]);
}

#[test]
fn reserved_prelude_names_and_generic_resources_are_rejected() {
    let reserved = r#"
module test.reserved;
@id("test.option") variant Option<T> { @id("test.option.none") None, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(reserved), ["SPX-S113"]);

    let reserved_id = r#"
module test.reserved_id;
@id("core.option") variant Other { @id("test.other.none") None, }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(reserved_id), ["SPX-S102"]);

    let generic_resource = r#"
module test.generic_resource;
@id("test.resource") resource Resource<T>;
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(generic_resource).contains(&"SPX-T223"));
}

#[test]
fn malformed_generic_lists_have_the_stable_parser_diagnostic() {
    for malformed in [
        SOURCE.replace("variant Choice<T>", "variant Choice<>"),
        SOURCE.replace("Choice<i64>::Value", "Choice<i64,>::Value"),
        SOURCE.replace("Option<i64>::None", "Option<i64 bool>::None"),
    ] {
        assert_eq!(
            parse(&malformed, Path::new("malformed-generics.spx"))
                .unwrap_err()
                .code,
            "SPX-P106"
        );
    }
}

const OWNED_GENERIC_VARIANT_SOURCE: &str = r#"
module test.owned_generic_variants;

@id("test.either")
variant Either<L, R> {
    @id("test.either.left")
    Left {
        @id("test.either.left.value")
        value: L,
    },
    @id("test.either.right")
    Right {
        @id("test.either.right.value")
        value: R,
    },
}

@id("test.inspect_left")
fn inspect_left(value: own Either<Bytes, i64>) -> i64 {
    let borrowed = match borrow value {
        Either::Left { value: payload } => {
            let length = byte_len(bytes_as_slice(payload));
            if length == 0usize { 0 } else { 1 }
        },
        Either::Right { value: marker } => marker,
    };
    match own value {
        Either::Left { value: payload } => {
            let length = byte_len(bytes_as_slice(payload));
            if length == 0usize { borrowed } else { borrowed }
        },
        Either::Right { value: marker } => marker,
    }
}

@id("test.inspect_right")
fn inspect_right(value: own Either<i64, Bytes>) -> i64 {
    match own value {
        Either::Left { value: marker } => marker,
        Either::Right { value: payload } => {
            let length = byte_len(bytes_as_slice(payload));
            if length == 0usize { 0 } else { 1 }
        },
    }
}

@id("test.make_left")
fn make_left(input: borrow Slice<u8>) -> Either<Bytes, i64> {
    Either<Bytes, i64>::Left { value: bytes_copy(input) }
}

@id("test.make_right")
fn make_right(input: borrow Slice<u8>) -> Either<i64, Bytes> {
    Either<i64, Bytes>::Right { value: bytes_copy(input) }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn concrete_owned_generic_variants_resolve_exact_instances_and_match_bindings() {
    let parsed = parse(
        OWNED_GENERIC_VARIANT_SOURCE,
        Path::new("owned-generic-variants.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let canonical = format::canonical(&parsed);
    let reparsed = parse(
        &canonical,
        Path::new("owned-generic-variants-canonical.spx"),
    )
    .unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));

    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let either = resolved
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "test.either")
        .unwrap();
    let left_instance = ResolvedType::Nominal {
        declaration: either.id.clone(),
        arguments: vec![ResolvedType::Bytes, ResolvedType::I64],
    };
    let right_instance = ResolvedType::Nominal {
        declaration: either.id.clone(),
        arguments: vec![ResolvedType::I64, ResolvedType::Bytes],
    };
    let inspect_left = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.inspect_left")
        .unwrap();
    assert_eq!(inspect_left.params[0].ty, left_instance);
    let ResolvedExprKind::Block { statements, tail } = &inspect_left.body.kind else {
        panic!("inspect_left must retain its block");
    };
    let semaprax::hir::ResolvedStatement::Let {
        value: borrowed, ..
    } = &statements[0]
    else {
        panic!("first statement must retain borrowed match");
    };
    let ResolvedExprKind::Match { arms, .. } = &borrowed.kind else {
        panic!("borrowed initializer must resolve as a match");
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[0].pattern else {
        panic!("left borrowed arm must retain a variant pattern");
    };
    assert_eq!(fields[0].binding.ty, ResolvedType::Bytes);
    assert_eq!(
        fields[0].binding.ownership,
        semaprax::hir::OwnershipMode::Borrow
    );
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        panic!("tail must resolve as an owning match");
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[0].pattern else {
        panic!("left owning arm must retain a variant pattern");
    };
    assert_eq!(fields[0].binding.ty, ResolvedType::Bytes);
    assert_eq!(
        fields[0].binding.ownership,
        semaprax::hir::OwnershipMode::Own
    );

    let inspect_right = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.inspect_right")
        .unwrap();
    assert_eq!(inspect_right.params[0].ty, right_instance);
    let ResolvedExprKind::Block { tail, .. } = &inspect_right.body.kind else {
        panic!("inspect_right must retain its block");
    };
    let ResolvedExprKind::Match { arms, .. } = &tail.kind else {
        panic!("inspect_right tail must resolve as a match");
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
        panic!("right owning arm must retain a variant pattern");
    };
    assert_eq!(fields[0].binding.ty, ResolvedType::Bytes);
    assert_eq!(
        fields[0].binding.ownership,
        semaprax::hir::OwnershipMode::Own
    );

    for (function, expected) in [
        ("test.make_left", left_instance),
        ("test.make_right", right_instance),
    ] {
        let function = resolved
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == function)
            .unwrap();
        assert_eq!(function.return_type, expected);
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("constructor function must retain its block");
        };
        let ResolvedExprKind::ConstructVariant { fields, .. } = &tail.kind else {
            panic!("constructor function must retain a variant construction");
        };
        assert_eq!(fields[0].value.ty, ResolvedType::Bytes);
    }
}

#[test]
fn concrete_owned_generic_variants_admit_every_copy_scalar_in_both_positions() {
    for (scalar_name, scalar_type) in [
        ("i64", ResolvedType::I64),
        ("i32", ResolvedType::I32),
        ("u8", ResolvedType::U8),
        ("usize", ResolvedType::Usize),
        ("char", ResolvedType::Char),
        ("f32", ResolvedType::F32),
        ("f64", ResolvedType::F64),
        ("bool", ResolvedType::Bool),
    ] {
        let source = format!(
            r#"module test.owned_generic_variant_{scalar_name};
@id("test.scalar.either")
variant Either<L, R> {{
  @id("test.scalar.either.left") Left {{
    @id("test.scalar.either.left.value") value: L,
  }},
  @id("test.scalar.either.right") Right {{
    @id("test.scalar.either.right.value") value: R,
  }},
}}
@id("test.scalar.consume-left")
fn consume_left(value: own Either<Bytes, {scalar_name}>) -> i64 {{
  match own value {{
    Either::Left {{ value: payload }} => 0,
    Either::Right {{ value: scalar }} => 0,
  }}
}}
@id("test.scalar.consume-right")
fn consume_right(value: own Either<{scalar_name}, Bytes>) -> i64 {{
  match own value {{
    Either::Left {{ value: scalar }} => 0,
    Either::Right {{ value: payload }} => 0,
  }}
}}
@id("app.main") fn main() -> i64 {{ 0 }}
"#
        );
        let parsed = parse(&source, Path::new("owned-generic-variant-copy-scalars.spx")).unwrap();
        let diagnostics = verify::verify(&parsed);
        assert!(diagnostics.is_empty(), "{scalar_name}: {diagnostics:#?}");
        let resolved = hir::resolve(&parsed).unwrap();
        hir::validate(&resolved).unwrap();
        let either = resolved
            .types
            .iter()
            .find(|declaration| declaration.id.as_str() == "test.scalar.either")
            .unwrap();

        for (function_id, arguments, owned_arm, scalar_arm) in [
            (
                "test.scalar.consume-left",
                vec![ResolvedType::Bytes, scalar_type.clone()],
                0usize,
                1usize,
            ),
            (
                "test.scalar.consume-right",
                vec![scalar_type.clone(), ResolvedType::Bytes],
                1usize,
                0usize,
            ),
        ] {
            let function = resolved
                .functions
                .iter()
                .find(|function| function.id.as_str() == function_id)
                .unwrap();
            assert_eq!(
                function.params[0].ty,
                ResolvedType::Nominal {
                    declaration: either.id.clone(),
                    arguments,
                },
                "{scalar_name}: {function_id} parameter"
            );
            let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
                panic!("{scalar_name}: {function_id} must retain its block");
            };
            let ResolvedExprKind::Match { mode, arms, .. } = &tail.kind else {
                panic!("{scalar_name}: {function_id} must retain its match");
            };
            assert_eq!(*mode, semaprax::hir::ResolvedMatchMode::Own);

            for (arm_index, expected_case, expected_field, expected_ty, expected_ownership) in [
                (
                    owned_arm,
                    if owned_arm == 0 {
                        "test.scalar.either.left"
                    } else {
                        "test.scalar.either.right"
                    },
                    if owned_arm == 0 {
                        "test.scalar.either.left.value"
                    } else {
                        "test.scalar.either.right.value"
                    },
                    ResolvedType::Bytes,
                    semaprax::hir::OwnershipMode::Own,
                ),
                (
                    scalar_arm,
                    if scalar_arm == 0 {
                        "test.scalar.either.left"
                    } else {
                        "test.scalar.either.right"
                    },
                    if scalar_arm == 0 {
                        "test.scalar.either.left.value"
                    } else {
                        "test.scalar.either.right.value"
                    },
                    scalar_type.clone(),
                    semaprax::hir::OwnershipMode::Value,
                ),
            ] {
                let ResolvedMatchPattern::Variant { case, fields, .. } = &arms[arm_index].pattern
                else {
                    panic!("{scalar_name}: {function_id} arm must retain a variant pattern");
                };
                assert_eq!(case.as_str(), expected_case);
                assert_eq!(fields[0].field.as_str(), expected_field);
                assert_eq!(fields[0].binding.ty, expected_ty);
                assert_eq!(fields[0].binding.ownership, expected_ownership);
            }
        }
    }
}

#[test]
fn concrete_owned_generic_variant_profile_rejects_two_owned_and_closed_shapes() {
    let declaration = r#"
@id("test.either")
variant Either<L, R> {
    @id("test.either.left") Left { @id("test.either.left.value") value: L, },
    @id("test.either.right") Right { @id("test.either.right.value") value: R, },
}
"#;
    for (module, ty, expected) in [
        ("two_owned", "Either<Bytes, Bytes>", "SPX-T268"),
        ("string_leaf", "Either<Bytes, String>", "SPX-T268"),
        ("nested_variant", "Either<Bytes, Option<i64>>", "SPX-T268"),
    ] {
        let source = format!(
            "module test.{module};\n{declaration}\n@id(\"test.bad\") fn bad(value: own {ty}) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
        );
        assert!(error_codes(&source).contains(&expected), "{module}");
    }

    let nonconcrete = format!(
        "module test.nonconcrete;\n{declaration}\n@id(\"test.bad\") fn bad<T>(value: own Either<Bytes, T>) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert!(error_codes(&nonconcrete).contains(&"SPX-T268"));

    let nested = format!(
        "module test.nested_owned_variant;\n{declaration}\n@id(\"test.outer\") variant Outer<T> {{ @id(\"test.outer.value\") Value {{ @id(\"test.outer.value.value\") value: T, }}, }}\n@id(\"test.bad\") fn bad(value: own Outer<Either<Bytes, i64>>) -> i64 {{ 0 }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}"
    );
    assert!(error_codes(&nested).contains(&"SPX-T223"));

    let class = r#"
module test.generic_class_owned_variant;
@id("test.box") class Box<T> { @id("test.box.value") value: T, }
@id("test.bad") fn bad(value: own Box<Bytes>) -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(class).contains(&"SPX-T268"));
}

#[test]
fn independent_hir_validation_rejects_owned_generic_variant_substitution_drift() {
    let parsed = parse(
        OWNED_GENERIC_VARIANT_SOURCE,
        Path::new("owned-generic-variant-hir-hostile.spx"),
    )
    .unwrap();

    let mut wrong_parameter_index = hir::resolve(&parsed).unwrap();
    let either = wrong_parameter_index
        .types
        .iter_mut()
        .find(|declaration| declaration.id.as_str() == "test.either")
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut either.kind else {
        unreachable!();
    };
    let ResolvedType::TypeParameter { index, .. } = &mut cases[0].fields[0].ty else {
        unreachable!();
    };
    *index = 1;
    assert_eq!(
        hir::validate(&wrong_parameter_index).unwrap_err().code,
        "SPX-H006"
    );

    let mut two_owned_arguments = hir::resolve(&parsed).unwrap();
    let function = two_owned_arguments
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.inspect_left")
        .unwrap();
    let ResolvedType::Nominal { arguments, .. } = &mut function.params[0].ty else {
        unreachable!();
    };
    arguments[1] = ResolvedType::Bytes;
    assert_eq!(
        hir::validate(&two_owned_arguments).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_owned_binding = hir::resolve(&parsed).unwrap();
    let function = wrong_owned_binding
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.inspect_right")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        unreachable!();
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        unreachable!();
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[1].pattern else {
        unreachable!();
    };
    fields[0].binding.ty = ResolvedType::I64;
    assert_eq!(
        hir::validate(&wrong_owned_binding).unwrap_err().code,
        "SPX-H006"
    );
}
