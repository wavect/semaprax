use std::path::Path;

use semaprax::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExprKind, ResolvedMatchMode, ResolvedMatchPattern,
    ResolvedType,
};
use semaprax::{format, graph, parse, verify};

const SOURCE: &str = r#"
module test.owned_result_bytes_bytes;

@id("test.twin")
variant Twin<L, R> {
    @id("test.twin.left") Left { @id("test.twin.left.value") value: L, },
    @id("test.twin.right") Right { @id("test.twin.right.value") value: R, },
}

@id("test.result.consume")
fn consume(value: own Result<Bytes, Bytes>) -> i64 {
    let borrowed = match borrow value {
        Result::Ok { value: payload } => if byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 1 },
        Result::Err { error: payload } => if byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 2 },
    };
    match own value {
        Result::Ok { value: payload } => borrowed,
        Result::Err { error: payload } => borrowed,
    }
}

@id("test.result.make-ok")
fn make_ok(input: borrow Slice<u8>) -> Result<Bytes, Bytes> {
    Result<Bytes, Bytes>::Ok { value: bytes_copy(input) }
}

@id("test.result.make-err")
fn make_err(input: borrow Slice<u8>) -> Result<Bytes, Bytes> {
    Result<Bytes, Bytes>::Err { error: bytes_copy(input) }
}

@id("app.main") fn main() -> i64 { 0 }
"#;

fn parsed() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("owned-result-bytes-bytes.spx")).unwrap()
}

fn error_codes(source: &str) -> Vec<&'static str> {
    let parsed = parse(source, Path::new("owned-result-bytes-bytes-error.spx")).unwrap();
    verify::verify(&parsed)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn consuming_match(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedExpr {
    let consume = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.result.consume")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut consume.body.kind else {
        unreachable!();
    };
    tail
}

#[test]
fn exact_compiler_owned_result_bytes_bytes_round_trips_and_resolves() {
    let parsed = parsed();
    let diagnostics = verify::verify(&parsed);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let canonical = format::canonical(&parsed);
    let reparsed = parse(
        &canonical,
        Path::new("owned-result-bytes-bytes-canonical.spx"),
    )
    .unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(graph::revision(&parsed), graph::revision(&reparsed));

    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let result = ResolvedType::Nominal {
        declaration: DeclarationId::new("core.result"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bytes],
    };
    let consume = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "test.result.consume")
        .unwrap();
    assert_eq!(consume.params[0].ty, result);
    let ResolvedExprKind::Block { statements, tail } = &consume.body.kind else {
        unreachable!();
    };
    let hir::ResolvedStatement::Let {
        value: borrowed, ..
    } = &statements[0]
    else {
        unreachable!();
    };
    for (expression, mode, ownership) in [
        (borrowed, ResolvedMatchMode::Borrow, OwnershipMode::Borrow),
        (tail.as_ref(), ResolvedMatchMode::Own, OwnershipMode::Own),
    ] {
        let ResolvedExprKind::Match {
            mode: actual, arms, ..
        } = &expression.kind
        else {
            unreachable!();
        };
        assert_eq!(*actual, mode);
        for (arm, case, field) in [
            (&arms[0], "core.result.ok", "core.result.ok.value"),
            (&arms[1], "core.result.err", "core.result.err.error"),
        ] {
            let ResolvedMatchPattern::Variant {
                variant,
                case: actual_case,
                fields,
                ..
            } = &arm.pattern
            else {
                unreachable!();
            };
            assert_eq!(variant.as_str(), "core.result");
            assert_eq!(actual_case.as_str(), case);
            assert_eq!(fields[0].field.as_str(), field);
            assert_eq!(fields[0].binding.ty, ResolvedType::Bytes);
            assert_eq!(fields[0].binding.ownership, ownership);
        }
    }

    for (function_id, case, field) in [
        (
            "test.result.make-ok",
            "core.result.ok",
            "core.result.ok.value",
        ),
        (
            "test.result.make-err",
            "core.result.err",
            "core.result.err.error",
        ),
    ] {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
            .unwrap();
        assert_eq!(function.return_type, result);
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            unreachable!();
        };
        let ResolvedExprKind::ConstructVariant {
            variant,
            case: actual_case,
            fields,
        } = &tail.kind
        else {
            unreachable!();
        };
        assert_eq!(tail.ty, result);
        assert_eq!(variant.as_str(), "core.result");
        assert_eq!(actual_case.as_str(), case);
        assert_eq!(fields[0].field.as_str(), field);
        assert_eq!(fields[0].value.ty, ResolvedType::Bytes);
    }
}

#[test]
fn independent_hir_rejects_result_identity_argument_and_member_tampering() {
    let mut wrong_argument = hir::resolve(&parsed()).unwrap();
    let consume = wrong_argument
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.result.consume")
        .unwrap();
    let ResolvedType::Nominal { arguments, .. } = &mut consume.params[0].ty else {
        unreachable!();
    };
    arguments[1] = ResolvedType::I64;
    assert_eq!(hir::validate(&wrong_argument).unwrap_err().code, "SPX-H006");

    let mut non_result = hir::resolve(&parsed()).unwrap();
    let consume = non_result
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.result.consume")
        .unwrap();
    let ResolvedType::Nominal { declaration, .. } = &mut consume.params[0].ty else {
        unreachable!();
    };
    *declaration = DeclarationId::new("core.option");
    assert_eq!(hir::validate(&non_result).unwrap_err().code, "SPX-H006");

    for member in 0..2 {
        let mut hostile = hir::resolve(&parsed()).unwrap();
        let ResolvedExprKind::Match { arms, .. } = &mut consuming_match(&mut hostile).kind else {
            unreachable!();
        };
        let ResolvedMatchPattern::Variant { case, fields, .. } = &mut arms[1].pattern else {
            unreachable!();
        };
        if member == 0 {
            *case = DeclarationId::new("core.result.ok");
        } else {
            fields[0].field = DeclarationId::new("core.result.ok.value");
        }
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
}

#[test]
fn owned_result_postfix_try_stays_closed_pending_owned_residual_cleanup() {
    let source = r#"
module test.owned_result_try_closed;
@id("test.owned-result.try")
fn propagate(value: own Result<Bytes, Bytes>) -> Result<Bytes, Bytes> {
    let payload = value?;
    Result<Bytes, Bytes>::Ok { value: payload }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(source), ["SPX-T218", "SPX-T218"]);
}

#[test]
fn independent_hir_keeps_owned_result_try_outside_the_copy_cleanup_schema() {
    let mut hostile = hir::resolve(
        &parse(
            r#"
module test.owned_result_try_hir_closed;
@id("test.scalar-result.try")
fn propagate(value: Result<i64, bool>) -> Result<i64, bool> {
    let payload = value?;
    Result<i64, bool>::Ok { value: payload }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
            Path::new("owned-result-try-hir-closed.spx"),
        )
        .unwrap(),
    )
    .unwrap();
    let propagate = hostile
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.scalar-result.try")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut propagate.body.kind else {
        unreachable!();
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        unreachable!();
    };
    let ResolvedExprKind::Try { operand, .. } = &mut value.kind else {
        unreachable!();
    };
    operand.ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("core.result"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bytes],
    };
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}
