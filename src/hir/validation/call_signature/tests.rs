//! Exact current signature views and saved recursive validator/state oracle.
use super::*;
const SOURCE: &str = r#"module signature.views;
permit {host.math}
@id("host.rust") interface RustHost permits {host.math} {
 @id("host.combine") import rust fn combine(left:i64,right:i64)->i64 effects {host.math} failure infallible;
}
@id("call.identity") fn identity(input:own Vec<i64>)->Vec<i64> uses {host.math} {input}
@id("call.direct") fn direct(input:own Vec<i64>)->Vec<i64> uses {host.math} {identity(input)}
@id("call.native") fn native(value:i64)->i64 uses {host.math} {combine(value,value)}
@id("app.main") fn main()->i64 {0}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(SOURCE, std::path::Path::new("signature-views.spx")).unwrap())
        .unwrap()
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
                contract_local: false,
                mutable: false,
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
fn borrowed_signatures_keep_saved_recursive_diagnostics_argument_order_and_scope_state() {
    let program = program();
    crate::hir::validate(&program).unwrap();
    for id in ["call.direct", "call.native"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        compare_expression(&program, function, &function.body).unwrap();
        let mut undeclared = function.clone();
        undeclared.effects.clear();
        let error = compare_expression(&program, &undeclared, &function.body).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("undeclared effect"));
        let mut hostile = function.body.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
            panic!("block")
        };
        match &mut tail.kind {
            ResolvedExprKind::Call { args, .. } => args.clear(),
            ResolvedExprKind::NativeRustImportCall(call) => call.args[1].ty = ResolvedType::Bool,
            _ => panic!("call"),
        }
        assert_eq!(
            compare_expression(&program, function, &hostile)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        let mut hostile = function.body.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
            panic!("block")
        };
        tail.ty = ResolvedType::Bool;
        assert_eq!(
            compare_expression(&program, function, &hostile)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    // Actual repeated call sites reach both current authored and Rust-import
    // signatures; the recursive owner remains the independent semantic oracle.
    let mut wide = SOURCE.replace(
        "@id(\"app.main\") fn main()->i64 {0}",
        "@id(\"call.scalar\") fn scalar(value:i64)->i64 uses {host.math} {value}",
    );
    wide.push_str(" @id(\"app.main\") fn main()->i64 {0} @id(\"call.wide\") fn wide()->i64 uses {host.math} {");
    for index in 0..64 {
        wide.push_str(&format!(
            "let local{index}=scalar({index}); let native{index}=combine({index},{index});"
        ));
    }
    wide.push_str("0}");
    let wide = crate::hir::resolve(
        &crate::parse(&wide, std::path::Path::new("wide-signature-views.spx")).unwrap(),
    )
    .unwrap();
    let function = wide
        .functions
        .iter()
        .find(|function| function.id.as_str() == "call.wide")
        .unwrap();
    compare_expression(&wide, function, &function.body).unwrap();
    let target = &program.functions[0];
    let view = ResultType::Borrowed(&target.return_type);
    assert_eq!(owned_capacity(&view), 0);
    assert!(std::ptr::eq(&*view, &target.return_type));
    let derived = ResultType::Owned(target.return_type.clone());
    assert!(owned_capacity(&derived) > 0);
    assert_eq!(*derived, target.return_type);
}
#[test]
fn wide_current_signature_access_removes_literal_owned_payload_and_drops_without_allocating() {
    let program = program();
    let target = &program.functions[0];
    let import = &program.interfaces[0].imports[0];
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &target.return_type
    else {
        panic!("Vec nominal")
    };
    assert_eq!(arguments.as_slice(), &[ResolvedType::I64]);
    assert_eq!(target.effects.as_slice(), &["host.math".to_owned()]);
    assert_eq!(import.parameters.len(), 2);
    // Clone requests only: inline descriptors are stack storage; original Program
    // and validator setup, diagnostic reporting and final publication are excluded.
    assert_eq!(declaration.as_str(), "core.vec");
    // Only the singleton argument Vec is owned clone payload: declaration
    // identities are static/shared handles, not copied text.
    let result_payload = std::mem::size_of::<ResolvedType>();
    let static_id = DeclarationId::compiler(crate::hir::ids::CompilerDeclaration::Vec);
    let shared_id = DeclarationId::new("signature.shared.é");
    let (_, identity_clones) = crate::test_allocation_observer::measure(|| {
        for _ in 0..64 {
            let fixed = static_id.clone();
            let shared = shared_id.clone();
            assert_eq!(fixed, static_id);
            assert_eq!(shared, shared_id);
            assert_eq!(fixed.as_str().as_ptr(), static_id.as_str().as_ptr());
            assert_eq!(shared.as_str().as_ptr(), shared_id.as_str().as_ptr());
            drop((fixed, shared));
        }
    });
    assert_eq!(
        (
            identity_clones.allocations,
            identity_clones.reallocations,
            identity_clones.allocated,
            identity_clones.freed,
            identity_clones.live
        ),
        (0, 0, 0, 0, 0)
    );
    assert!(!identity_clones.invalid);
    let effects_payload = std::mem::size_of::<String>() + "host.math".len();
    let native_payload =
        2 * std::mem::size_of::<ResolvedImportParameter>() + "left".len() + "right".len();
    let expected = 64 * (result_payload + effects_payload + native_payload);
    let (_, old) = crate::test_allocation_observer::measure(|| {
        for _ in 0..64 {
            let result = target.return_type.clone();
            let effects = target.effects.clone();
            let params = import.parameters.clone();
            assert_eq!(result, target.return_type);
            assert_eq!(effects, target.effects);
            assert_eq!(params, import.parameters);
            drop((result, effects, params));
        }
    });
    let (_, current) = crate::test_allocation_observer::measure(|| {
        for _ in 0..64 {
            let result = ResultType::Borrowed(&target.return_type);
            let effects = std::borrow::Cow::<[String]>::Borrowed(target.effects.as_slice());
            let params = import.parameters.as_slice();
            assert!(std::ptr::eq(&*result, &target.return_type));
            assert_eq!(effects.as_ref(), target.effects.as_slice());
            for (index, param) in params.iter().enumerate().take(2) {
                assert!(std::ptr::eq(param, &import.parameters[index]));
            }
            assert_eq!(owned_capacity(&result), 0);
            drop((result, effects));
        }
    });
    assert_eq!(
        (old.allocated, old.freed, old.live),
        (expected, expected, 0)
    );
    assert_eq!(
        (
            current.allocations,
            current.reallocations,
            current.allocated,
            current.live
        ),
        (0, 0, 0, 0)
    );
    assert!(!old.invalid && !current.invalid);
}
