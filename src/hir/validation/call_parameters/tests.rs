use super::*;

const SOURCE: &str = r#"
module test.byte_signature_views;
@id("bytes.read") fn read(input: borrow Slice<u8>) -> usize { byte_len(input) }
@id("bytes.view") fn view(input: borrow Bytes) -> usize {
    let slice = bytes_as_slice(input);
    byte_len(slice)
}
@id("bytes.owned") fn owned() -> Bytes {
    bytes_set(bytes_zeroed(1usize), 0usize, 1u8)
}
@id("bytes.empty") fn empty(input: [u8; 0]) -> usize {
    let slice = array_as_slice(input);
    byte_len(slice)
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let ast = crate::parse(SOURCE, std::path::Path::new("byte-signature-views.spx")).unwrap();
    crate::hir::resolve(&ast).unwrap()
}

fn same_diagnostic(left: &Diagnostic, right: &Diagnostic) {
    assert_eq!(left.code, right.code);
    assert_eq!(left.severity, right.severity);
    assert_eq!(left.message, right.message);
    assert_eq!(left.path, right.path);
    assert_eq!(left.span, right.span);
    assert_eq!(left.help, right.help);
}

#[test]
fn byte_signature_views_match_every_owned_descriptor_without_materializing_labels() {
    for operation in ByteOp::ALL {
        let owned = crate::byte_ops::resolved_params(operation);
        let ((), overflow, used) = crate::bounded_output::with_limit_usage(0, || {
            let view = CallParameters::Byte(operation);
            assert_eq!(view.owned_capacity(), 0);
            for (index, expected) in owned.iter().enumerate() {
                let parameter = view.parameter(index);
                assert_eq!(parameter.ty, &expected.ty);
                assert_eq!(parameter.ownership, expected.ownership);
            }
        });
        assert!(!overflow);
        assert_eq!(used, 0);
        let view = CallParameters::Byte(operation);
        for (index, expected) in owned.iter().enumerate() {
            assert_eq!(
                format!("{}", view.parameter(index).identity),
                expected.id.as_str()
            );
        }
        let expected_capacity = owned.capacity() * std::mem::size_of::<ResolvedParam>()
            + owned
                .iter()
                .map(|parameter| {
                    parameter.id.as_str().len()
                        + parameter.name.capacity()
                        + resolved_type_owned_capacity(&parameter.ty)
                })
                .sum::<usize>();
        assert_eq!(
            CallParameters::Owned(owned).owned_capacity(),
            expected_capacity
        );
    }
}

fn compare_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut iterative = HirValidator::new(program).unwrap();
    let mut scope = BTreeMap::new();
    for parameter in &function.params {
        scope.insert(
            parameter.id.clone(),
            ValidationBinding {
                ty: parameter.ty.clone(),
                ownership: parameter.ownership,
                availability: Availability::Available,
                active_loans: BTreeSet::new(),
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        );
        if parameter.ty == ResolvedType::SliceU8 {
            iterative.byte_slice_aliases.insert(
                parameter.id.clone(),
                Place {
                    root: parameter.id.clone(),
                    projections: Vec::new(),
                },
            );
        }
    }
    let mut recursive = iterative.clone();
    let mut recursive_scope = scope.clone();
    let effects = function.effects.iter().cloned().collect();
    let reference = recursive.validate_expr_recursive_reference(
        &execution,
        expression,
        &mut recursive_scope,
        "body",
        true,
        Some(&effects),
    );
    let actual = iterative.validate_expr_iterative(
        &execution,
        expression,
        &mut scope,
        "body",
        true,
        Some(&effects),
    );
    HirValidator::assert_validation_oracle(
        &actual,
        &reference,
        &iterative,
        &recursive,
        &scope,
        &recursive_scope,
        "body",
    );
    actual
}

#[test]
fn byte_signature_views_keep_recursive_success_and_call_shape_refusal_exact() {
    let program = program();
    let wire = crate::cache_codec::encode(&program).unwrap();
    for identity in ["bytes.read", "bytes.view", "bytes.owned", "bytes.empty"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == identity)
            .unwrap();
        compare_expression(&program, function, &function.body).unwrap();
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "bytes.read")
        .unwrap();
    let mut hostile = function.body.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        unreachable!()
    };
    args.clear();
    let error = compare_expression(&program, function, &hostile).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "byte operation `byte_len` expects 1 arguments but received 0"
    );
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), wire);
}

#[test]
fn byte_signature_views_preserve_owned_bytes_identity_and_borrow_authority_failures() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "bytes.owned")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        unreachable!()
    };
    let mut argument = args[0].clone();
    argument.ownership = OwnershipMode::Borrow;
    let validator = HirValidator::new(&program).unwrap();
    let owned = crate::byte_ops::resolved_params(ByteOp::Set);
    let view = CallParameters::Byte(ByteOp::Set);
    let expected = validator
        .validate_argument_ownership(&argument, &owned[0])
        .unwrap_err();
    let actual = validator
        .validate_argument_ownership_view(&argument, view.parameter(0))
        .unwrap_err();
    same_diagnostic(&actual, &expected);
    assert_eq!(
        actual.message,
        "argument ownership is incompatible with parameter `core.bytes.set.param.0`"
    );
    assert_eq!(actual.span, Some(argument.span));

    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "bytes.view")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        unreachable!()
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        unreachable!()
    };
    let ResolvedExprKind::BorrowPlace { place, .. } = &value.kind else {
        unreachable!()
    };
    // Source views lower to BorrowPlace. A forged decoded Call must still
    // traverse the exact borrowed-Bytes authority refusal through either API.
    let mut argument = value.clone();
    argument.kind = ResolvedExprKind::Place(place.clone());
    argument.ty = ResolvedType::Bytes;
    let mut call = value.clone();
    call.kind = ResolvedExprKind::Call {
        callee: DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        type_arguments: Vec::new(),
        instance: None,
        args: vec![argument.clone()],
    };
    let owned = crate::byte_ops::resolved_params(ByteOp::BytesAsSlice);
    let view = CallParameters::Byte(ByteOp::BytesAsSlice);
    let parameter = view.parameter(0);
    let scope = BTreeMap::new();
    let expected = validator
        .validate_borrowed_bytes_call_argument(&call, &argument, &owned[0], 0, &scope)
        .unwrap_err();
    let actual = validator
        .validate_borrowed_bytes_call_argument_fields(
            &call,
            &argument,
            (parameter.ty, parameter.ownership),
            0,
            &scope,
        )
        .unwrap_err();
    same_diagnostic(&actual, &expected);
    assert_eq!(actual.message, "borrowed Bytes call root is out of scope");
    assert_eq!(actual.span, Some(argument.span));
}

#[test]
fn ownership_view_borrows_nominal_facts_and_preserves_byte_and_missing_fact_diagnostics() {
    let source = r#"
module test.nominal_signature_view;
@id("payload.type") record Payload { @id("payload.bytes") bytes: Bytes, }
@id("payload.identity") fn identity(input: own Payload) -> Payload { input }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let ast = crate::parse(source, std::path::Path::new("nominal-signature-view.spx")).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let wire = crate::cache_codec::encode(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "payload.identity")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        unreachable!()
    };
    let parameter = &function.params[0];
    assert_eq!(parameter.ownership, OwnershipMode::Own);
    assert_eq!(tail.ownership, OwnershipMode::Own);
    let validator = HirValidator::new(&program).unwrap();
    let facts = validator
        .borrowed_type_facts(&parameter.ty)
        .unwrap()
        .unwrap();
    assert!(matches!(facts, std::borrow::Cow::Borrowed(_)));
    let expected_facts = program.declarations.type_facts(&parameter.ty).unwrap();
    assert!(!expected_facts.copy);
    let ((), overflow, used) = crate::bounded_output::with_limit_usage(0, || {
        for _ in 0..64 {
            validator
                .validate_argument_ownership_view(tail, ParameterView::owned(parameter))
                .unwrap();
        }
    });
    assert!(!overflow, "ownership view rebuilt a charged nominal key");
    assert_eq!(used, 0);

    let mut argument = tail.as_ref().clone();
    argument.ty = ResolvedType::Bytes;
    let bytes = CallParameters::Byte(ByteOp::Set);
    validator
        .validate_argument_ownership_view(&argument, bytes.parameter(0))
        .unwrap();
    argument.ownership = OwnershipMode::Borrow;
    let error = validator
        .validate_argument_ownership_view(&argument, bytes.parameter(0))
        .unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "argument ownership is incompatible with parameter `core.bytes.set.param.0`"
    );
    assert_eq!(error.span, Some(argument.span));
    assert!(error.path.is_none());
    assert!(error.help.is_none());

    let mut missing = parameter.clone();
    missing.ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("payload.missing"),
        arguments: Vec::new(),
    };
    assert!(program.declarations.type_facts(&missing.ty).is_none());
    let error = validator
        .validate_argument_ownership_view(tail, ParameterView::owned(&missing))
        .unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "type `nominal:15:payload.missing:0:` has no semantic facts"
    );
    assert_eq!(error.span, Some(tail.span));
    assert!(error.path.is_none());
    assert!(error.help.is_none());
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), wire);
}

const USER_CALL_SOURCE: &str = r#"
module test.borrowed_user_signatures;
@id("payload.type") record Payload { @id("payload.bytes") bytes: Bytes, }
@id("scalar.identity") fn scalar(value: i64) -> i64 { value }
@id("payload.identity") fn identity(input: own Payload) -> Payload { input }
@id("payload.relay") fn relay(input: own Payload) -> Payload { identity(input) }
@id("scalar.combine") fn combine(value: i64) -> i64 { scalar(value) + scalar(1) }
@id("bytes.borrowed") fn borrowed(input: borrow Slice<u8>) -> usize { byte_len(input) }
@id("bytes.relay") fn byte_relay(input: borrow Slice<u8>) -> usize { borrowed(input) }
@id("app.main") fn main() -> i64 { combine(1) }
"#;

fn user_call_program() -> ResolvedProgram {
    let source = crate::parse(
        USER_CALL_SOURCE,
        std::path::Path::new("borrowed-user-signatures.spx"),
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}

#[test]
fn borrowed_user_signatures_keep_exact_descriptors_and_original_owned_census() {
    let program = user_call_program();
    let wire = crate::cache_codec::encode(&program).unwrap();
    for function in &program.functions {
        let owned_parameters = function.params.clone();
        let expected_capacity = owned_parameters.capacity() * std::mem::size_of::<ResolvedParam>()
            + owned_parameters
                .iter()
                .map(|parameter| {
                    parameter.id.as_str().len()
                        + parameter.name.capacity()
                        + resolved_type_owned_capacity(&parameter.ty)
                })
                .sum::<usize>();
        let owned = CallParameters::Owned(owned_parameters);
        assert_eq!(owned.owned_capacity(), expected_capacity);
        let ((), overflow, used) = crate::bounded_output::with_limit_usage(0, || {
            let borrowed = CallParameters::Borrowed(&function.params);
            assert_eq!(borrowed.owned_capacity(), 0);
            for (index, expected) in function.params.iter().enumerate() {
                let actual = borrowed.parameter(index);
                assert!(std::ptr::eq(actual.ty, &expected.ty));
                assert_eq!(actual.ownership, expected.ownership);
                let ParameterIdentity::Owned(identity) = actual.identity else {
                    unreachable!()
                };
                assert!(std::ptr::eq(identity, &expected.id));
            }
        });
        assert!(!overflow);
        assert_eq!(used, 0);
    }
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), wire);
}

#[test]
fn borrowed_user_signatures_keep_recursive_calls_and_hostile_argument_refusals_exact() {
    let program = user_call_program();
    for identity in ["payload.relay", "scalar.combine", "bytes.relay", "app.main"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == identity)
            .unwrap();
        compare_expression(&program, function, &function.body).unwrap();
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "payload.relay")
        .unwrap();
    let mut hostile = function.body.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        unreachable!()
    };
    args[0].ownership = OwnershipMode::Borrow;
    let error = compare_expression(&program, function, &hostile).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    let target = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "payload.identity")
        .unwrap();
    assert_eq!(
        error.message,
        format!(
            "argument ownership is incompatible with parameter `{}`",
            target.params[0].id
        )
    );
    let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        unreachable!()
    };
    args.clear();
    let error = compare_expression(&program, function, &hostile).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "call to `payload.identity` has 0 arguments but expects 1"
    );
}
