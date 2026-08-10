use std::path::Path;

use semaprax::hir::{self, DeclarationId, OwnershipMode, ResolvedExprKind, ResolvedType};
use semaprax::{format, graph, parse, verify};

const SOURCE: &str = r#"
module test.result_try;

@id("test.fallible")
fn fallible(flag: bool, value: i64) -> Result<i64, bool> {
    if flag {
        Result<i64, bool>::Err { error: true }
    } else {
        Result<i64, bool>::Ok { value: value }
    }
}

@id("test.lift")
fn lift(flag: bool, value: i64) -> Result<bool, bool>
    ensures match result {
        Result::Ok { value: ok_value } => ok_value,
        Result::Err { error } => error,
    }
{
    let number = fallible(flag, value)?;
    Result<bool, bool>::Ok { value: number > 0 }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("result-try.spx")).unwrap()
}

fn error_codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("result-try-error.spx")).unwrap();
    verify::verify(&program)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn try_expr(program: &mut semaprax::hir::ResolvedProgram) -> &mut semaprax::hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "test.lift")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("lift body must remain a block");
    };
    let semaprax::hir::ResolvedStatement::Let { value, .. } = &mut statements[0];
    value
}

#[test]
fn postfix_result_try_has_a_canonical_round_trip_and_postfix_precedence() {
    let program = program();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("let number = fallible(flag, value)?;"));
    assert!(!canonical.contains("fallible(flag, value) ?"));
    let reparsed = parse(&canonical, Path::new("result-try-canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
}

#[test]
fn result_try_requires_an_ordinary_result_body_and_exact_error_residual() {
    let option_operand = SOURCE
        .replace(
            "let number = fallible(flag, value)?;",
            "let number = Option<i64>::Some { value: value }?;",
        )
        .replace("number > 0", "true");
    assert_eq!(error_codes(&option_operand), ["SPX-T218"]);

    let non_result_return = r#"
module test.bad_try_context;
@id("test.unwrap")
fn unwrap(value: Result<i64, bool>) -> i64 { value? }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(non_result_return), ["SPX-T218"]);

    let mismatched_error = SOURCE
        .replace(
            "fn lift(flag: bool, value: i64) -> Result<bool, bool>",
            "fn lift(flag: bool, value: i64) -> Result<bool, i64>",
        )
        .replace("Result<bool, bool>::Ok", "Result<bool, i64>::Ok")
        .replace(
            "Result::Err { error } => error,",
            "Result::Err { error } => error > 0,",
        );
    assert_eq!(error_codes(&mismatched_error), ["SPX-T219"]);
}

#[test]
fn result_try_is_rejected_in_requires_and_ensures() {
    let source = r#"
module test.try_contract;
@id("test.contract")
fn contract(guard: Result<bool, bool>) -> Result<i64, bool>
    requires guard?
    ensures guard?
{
    Result<i64, bool>::Ok { value: 1 }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(source), ["SPX-T218", "SPX-T218"]);
}

#[test]
fn resolved_try_authenticates_both_instances_and_every_prelude_member() {
    let mut resolved = hir::resolve(&program()).unwrap();
    hir::validate(&resolved).unwrap();
    let expression = try_expr(&mut resolved);
    let ResolvedExprKind::Try {
        operand,
        result,
        ok_case,
        ok_field,
        err_case,
        err_field,
        residual_type,
    } = &expression.kind
    else {
        panic!("let initializer must remain an explicit Try node");
    };
    assert_eq!(expression.ty, ResolvedType::I64);
    assert_eq!(expression.ownership, OwnershipMode::Value);
    assert_eq!(result.as_str(), "core.result");
    assert_eq!(ok_case.as_str(), "core.result.ok");
    assert_eq!(ok_field.as_str(), "core.result.ok.value");
    assert_eq!(err_case.as_str(), "core.result.err");
    assert_eq!(err_field.as_str(), "core.result.err.error");
    assert_eq!(
        operand.ty,
        ResolvedType::Nominal {
            declaration: DeclarationId::new("core.result"),
            arguments: vec![ResolvedType::I64, ResolvedType::Bool],
        }
    );
    assert_eq!(
        residual_type,
        &ResolvedType::Nominal {
            declaration: DeclarationId::new("core.result"),
            arguments: vec![ResolvedType::Bool, ResolvedType::Bool],
        }
    );
}

#[test]
fn independent_hir_validation_rejects_try_identity_type_and_ownership_confusion() {
    for identity in 0..5 {
        let mut hostile = hir::resolve(&program()).unwrap();
        let ResolvedExprKind::Try {
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            ..
        } = &mut try_expr(&mut hostile).kind
        else {
            unreachable!();
        };
        *match identity {
            0 => result,
            1 => ok_case,
            2 => ok_field,
            3 => err_case,
            4 => err_field,
            _ => unreachable!(),
        } = DeclarationId::new("hostile.result.member");
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }

    let mut hostile = hir::resolve(&program()).unwrap();
    let ResolvedExprKind::Try { operand, .. } = &mut try_expr(&mut hostile).kind else {
        unreachable!();
    };
    operand.ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("core.result"),
        arguments: vec![ResolvedType::Bool, ResolvedType::Bool],
    };
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    let mut hostile = hir::resolve(&program()).unwrap();
    let ResolvedExprKind::Try { residual_type, .. } = &mut try_expr(&mut hostile).kind else {
        unreachable!();
    };
    *residual_type = ResolvedType::Nominal {
        declaration: DeclarationId::new("core.result"),
        arguments: vec![ResolvedType::Bool, ResolvedType::I64],
    };
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    let mut hostile = hir::resolve(&program()).unwrap();
    try_expr(&mut hostile).ty = ResolvedType::Bool;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

    let mut hostile = hir::resolve(&program()).unwrap();
    try_expr(&mut hostile).ownership = OwnershipMode::Own;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}
