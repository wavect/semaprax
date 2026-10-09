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
    assert_eq!(actual.span, argument.span);

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
    assert_eq!(actual.span, argument.span);
}
