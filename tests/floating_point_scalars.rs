use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SCALARS: &str = r#"
module test.float_scalars;

@id("f.scale")
fn scale(value: f64) -> f64 { value * 2.5 + 0.5 }

@id("f.narrow")
fn narrow(value: f32) -> f32 { value / 4.0f32 - 0.25f32 }

@id("f.classify")
fn classify(left: f64, right: f64) -> i64 {
    let smaller = left < right;
    if smaller { 1 } else {
        if left == right { 2 } else { 3 }
    }
}

@id("f.invert")
fn invert(value: f64) -> f64 { -value }

@id("f.divide")
fn divide(left: f64, right: f64) -> f64 { left / right }

@id("app.main")
fn main() -> i64 {
    let scaled = scale(1.5);
    let half = narrow(9.0f32);
    let inverted = -scaled;
    if scaled == 4.25 && half == 2.0f32 && inverted < 0.0 {
        let quotient = 1.0 / 0.0;
        if quotient > 10000000000.0 && classify(1.0, 2.0) == 1 {
            7
        } else {
            8
        }
    } else {
        9
    }
}
"#;

const RECORDS: &str = r#"
module test.float_records;

@id("vec.type")
record Vec2 {
    @id("vec.x") x: f64,
    @id("vec.y") y: f64,
}

@id("mix.type")
record Mix {
    @id("mix.ratio") ratio: f32,
    @id("mix.point") point: Vec2,
    @id("mix.flag") flag: bool,
}

@id("mix.make")
fn make(x: f64, y: f64) -> Mix { Mix { ratio: 0.5f32, point: Vec2 { x: x, y: y }, flag: true } }

@id("mix.project_x")
fn project_x(mix: Mix) -> f64 { mix.point.x }

@id("app.main")
fn main() -> i64 {
    let mix = make(1.5, 2.5);
    let moved = mix with { ratio: 0.25f32 };
    if project_x(moved) == 1.5 && moved.ratio == 0.25f32 && mix.point.y == 2.5 {
        4
    } else {
        5
    }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("floats.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn float_programs_round_trip_canonically_and_hash_stably() {
    for source in [SCALARS, RECORDS] {
        let program = parse(source, Path::new("floats.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        assert!(
            canonical.contains("4.0f32") || canonical.contains("0.5f32"),
            "an f32 suffix must survive the canonical projection: {canonical}"
        );
        assert!(canonical.contains("2.5"), "plain decimals stay canonical");
        assert!(!canonical.contains("1e"), "no bare exponent forms");
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert!(verify::verify(&reparsed).is_empty());
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
        assert_eq!(format::canonical(&reparsed), canonical);
    }
}

#[test]
fn integral_float_values_canonicalize_with_an_explicit_fraction() {
    let source = r#"
module test.float_whole;
@id("t.identity")
fn identity(value: f64) -> f64 { 3.0 + value * 10.0 }
@id("app.main")
fn main() -> i64 { if identity(4.0) == 43.0 { 1 } else { 0 } }
"#;
    let program = parse(source, Path::new("whole.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    // Whole-number floats must not degrade to integer literals on rewrite.
    assert!(canonical.contains("3.0"), "{canonical}");
    assert!(canonical.contains("10.0"), "{canonical}");
    assert!(canonical.contains("4.0"), "{canonical}");
    assert!(canonical.contains("43.0"), "{canonical}");
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
}

/// Collects every direct child expression of a resolved expression.
fn children(expression: &hir::ResolvedExpr) -> Vec<&hir::ResolvedExpr> {
    match &expression.kind {
        hir::ResolvedExprKind::Call { args, .. } => args.iter().collect(),
        hir::ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().collect(),
        hir::ResolvedExprKind::Unary { value, .. }
        | hir::ResolvedExprKind::Try { operand: value, .. }
        | hir::ResolvedExprKind::TryOption { operand: value, .. }
        | hir::ResolvedExprKind::Project { base: value, .. } => vec![value.as_ref()],
        hir::ResolvedExprKind::Binary { left, right, .. } => vec![left.as_ref(), right.as_ref()],
        hir::ResolvedExprKind::Block { statements, tail } => statements
            .iter()
            .map(|statement| {
                let hir::ResolvedStatement::Let { value, .. } = statement;
                value
            })
            .chain(std::iter::once(tail.as_ref()))
            .collect(),
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ],
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        hir::ResolvedExprKind::Match { scrutinee, arms } => {
            let mut collected = vec![scrutinee.as_ref()];
            collected.extend(arms.iter().map(|arm| &arm.value));
            collected
        }
        hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            let mut collected = vec![base.as_ref()];
            collected.extend(fields.iter().map(|field| &field.value));
            collected
        }
        hir::ResolvedExprKind::Int(_)
        | hir::ResolvedExprKind::Char(_)
        | hir::ResolvedExprKind::Float32(_)
        | hir::ResolvedExprKind::Float64(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::Place(_) => Vec::new(),
    }
}

fn walk<'a>(expression: &'a hir::ResolvedExpr, visit: &mut impl FnMut(&'a hir::ResolvedExpr)) {
    visit(expression);
    for child in children(expression) {
        walk(child, visit);
    }
}

/// Single-expression bodies are canonical blocks; this returns the tail.
fn body_tail(function: &hir::ResolvedFunction) -> &hir::ResolvedExpr {
    let hir::ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("function bodies are blocks");
    };
    tail
}

#[test]
fn resolved_hir_keeps_exact_float_literal_bits_and_types() {
    let program = hir::resolve(&parse(SCALARS, Path::new("floats.spx")).unwrap()).unwrap();

    let scale = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "f.scale")
        .unwrap();
    assert_eq!(scale.return_type, hir::ResolvedType::F64);
    assert_eq!(scale.params[0].ty, hir::ResolvedType::F64);
    let mut wide_literals = Vec::new();
    walk(&scale.body, &mut |expression| {
        if let hir::ResolvedExprKind::Float64(bits) = &expression.kind {
            assert_eq!(expression.ty, hir::ResolvedType::F64);
            wide_literals.push(*bits);
        }
    });
    assert!(wide_literals.contains(&2.5f64.to_bits()));
    assert!(wide_literals.contains(&0.5f64.to_bits()));

    let narrow = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "f.narrow")
        .unwrap();
    assert_eq!(narrow.return_type, hir::ResolvedType::F32);
    assert_eq!(narrow.params[0].ty, hir::ResolvedType::F32);
    let mut narrow_literals = Vec::new();
    walk(&narrow.body, &mut |expression| {
        if let hir::ResolvedExprKind::Float32(bits) = &expression.kind {
            assert_eq!(expression.ty, hir::ResolvedType::F32);
            narrow_literals.push(*bits);
        }
    });
    assert!(narrow_literals.contains(&4.0f32.to_bits()));
    assert!(narrow_literals.contains(&0.25f32.to_bits()));

    // Division keeps the operand's float type in HIR.
    let divide = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "f.divide")
        .unwrap();
    let hir::ResolvedExprKind::Binary { op, .. } = &body_tail(divide).kind else {
        panic!("divide body must be binary");
    };
    assert_eq!(*op, ast::BinaryOp::Div);
    assert_eq!(divide.body.ty, hir::ResolvedType::F64);
}

#[test]
fn resolved_hir_negation_keeps_float_type() {
    let program = hir::resolve(&parse(SCALARS, Path::new("floats.spx")).unwrap()).unwrap();
    let invert = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "f.invert")
        .unwrap();
    let hir::ResolvedExprKind::Unary { op, value } = &body_tail(invert).kind else {
        panic!("invert body must be unary");
    };
    assert_eq!(*op, ast::UnaryOp::Neg);
    assert_eq!(value.ty, hir::ResolvedType::F64);
    assert_eq!(invert.body.ty, hir::ResolvedType::F64);
    // The negated operand is the f64 parameter place.
    let hir::ResolvedExprKind::Place(place) = &value.kind else {
        panic!("negated operand must be the parameter place");
    };
    assert!(place.projections.is_empty());
    assert_eq!(value.ty, hir::ResolvedType::F64);
    assert_eq!(invert.body.ty, hir::ResolvedType::F64);
}

#[test]
fn graph_json_exposes_deterministic_float_nodes() {
    let program = parse(SCALARS, Path::new("floats.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"float64\""));
    assert!(first.contains("\"kind\":\"float32\""));
    assert!(first.contains("\"name\":\"f64\""));
    assert!(first.contains("\"name\":\"f32\""));
    // Bit-exact payload survives serialization.
    let expected_bits = format!("{:016x}", (10000000000.0f64).to_bits());
    assert!(first.contains(&expected_bits));
}

#[test]
fn float_type_mismatch_diagnostics_are_stable() {
    let remainder = diagnostics(
        r#"
module test.float_rem;
@id("t.main")
fn main() -> i64 { if 1.5 % 0.5 == 0.5 { 7 } else { 8 } }
"#,
    );
    assert!(
        remainder
            .iter()
            .any(|item| item.code == "SPX-T208" && item.message.contains('%')),
        "float remainder must be rejected: {remainder:?}"
    );

    let mixed_arithmetic = diagnostics(
        r#"
module test.float_mixed;
@id("app.main")
fn main() -> i64 { if 1.5 + 1.0f32 == 2.5 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_arithmetic.iter().any(|item| item.code == "SPX-T208"),
        "mixed-width arithmetic must be rejected"
    );

    let mixed_equality = diagnostics(
        r#"
module test.float_equal;
@id("app.main")
fn main() -> i64 { if 1.5 == 1.5f32 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_equality.iter().any(|item| item.code == "SPX-T207"),
        "mixed-width equality must be rejected"
    );

    let argument = diagnostics(
        r#"
module test.float_arg;
@id("t.take")
fn take(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { take(1.5) }
"#,
    );
    assert!(
        argument
            .iter()
            .any(|item| item.code == "SPX-T205" && item.message.contains("i64")),
        "implicit float-to-int conversion must be rejected"
    );

    let condition = diagnostics(
        r#"
module test.float_cond;
@id("app.main")
fn main() -> i64 { if 1.5 { 7 } else { 8 } }
"#,
    );
    assert!(
        condition.iter().any(|item| item.code == "SPX-T210"),
        "float conditions must be rejected"
    );

    let bool_operand = diagnostics(
        r#"
module test.float_bool;
@id("app.main")
fn main() -> i64 { if true && 1.5 { 7 } else { 8 } }
"#,
    );
    assert!(
        bool_operand.iter().any(|item| item.code == "SPX-T208"),
        "float lazy-boolean operands must be rejected"
    );

    let generic_argument = diagnostics(
        r#"
module test.float_generic;
@id("t.box")
record Box<T> { @id("t.box.value") value: T, }
@id("t.make")
fn make(value: i64) -> Box<i64> { Box<i64> { value: value } }
@id("app.main")
fn main() -> i64 { let boxed = make(1); boxed.value }
"#,
    );
    assert!(generic_argument.is_empty());
}

#[test]
fn malformed_float_literals_fail_with_stable_lexer_diagnostics() {
    let cases = [
        ("1.5x", "suffix"),
        ("1.5e", "exponent"),
        ("1.0e9999", "range"),
        ("1.5e+", "exponent"),
    ];
    for (literal, expected_message_part) in cases {
        let source = format!(
            r#"
module test.float_lex;
@id("app.main")
fn main() -> i64 {{ if {literal} > 0.0 {{ 7 }} else {{ 8 }} }}
"#
        );
        let error =
            parse(&source, Path::new("lex.spx")).expect_err("malformed literal must be rejected");
        assert_eq!(error.code, "SPX-P003", "{literal}: {}", error.message);
        assert!(
            error.message.contains(expected_message_part),
            "{}: {}",
            literal,
            error.message
        );
    }
}

#[test]
fn hostile_non_finite_float_literals_reject_before_backends() {
    let program = hir::resolve(&parse(SCALARS, Path::new("floats.spx")).unwrap()).unwrap();

    let mut nan_program = program.clone();
    replace_first_f64_bits(&mut nan_program, f64::NAN.to_bits());
    let error = hir::validate(&nan_program).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error.message.contains("finite"));
    assert!(codegen::emit_hir_c(&nan_program).is_err());
    assert!(wasm::emit_resolved_module(&nan_program).is_err());

    let mut infinite_program = program.clone();
    replace_first_f64_bits(&mut infinite_program, f64::INFINITY.to_bits());
    assert!(hir::validate(&infinite_program).is_err());

    let mut hostile_f32 = program;
    replace_first_f32_bits(&mut hostile_f32, f32::NAN.to_bits());
    assert!(hir::validate(&hostile_f32).is_err());
}

fn replace_first_f64_bits(program: &mut hir::ResolvedProgram, bits: u64) -> bool {
    for function in &mut program.functions {
        if replace_f64_in_expression(&mut function.body, bits) {
            return true;
        }
    }
    false
}

fn replace_f64_in_expression(expression: &mut hir::ResolvedExpr, bits: u64) -> bool {
    if matches!(expression.kind, hir::ResolvedExprKind::Float64(_)) {
        expression.kind = hir::ResolvedExprKind::Float64(bits);
        return true;
    }
    match &mut expression.kind {
        hir::ResolvedExprKind::Unary { value, .. }
        | hir::ResolvedExprKind::Try { operand: value, .. }
        | hir::ResolvedExprKind::TryOption { operand: value, .. }
        | hir::ResolvedExprKind::Project { base: value, .. } => {
            replace_f64_in_expression(value, bits)
        }
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            replace_f64_in_expression(left, bits) || replace_f64_in_expression(right, bits)
        }
        hir::ResolvedExprKind::Block { statements, tail } => {
            statements.iter_mut().any(|statement| {
                let hir::ResolvedStatement::Let { value, .. } = statement;
                replace_f64_in_expression(value, bits)
            }) || replace_f64_in_expression(tail, bits)
        }
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            replace_f64_in_expression(condition, bits)
                || replace_f64_in_expression(then_branch, bits)
                || replace_f64_in_expression(else_branch, bits)
        }
        hir::ResolvedExprKind::Call { args, .. } => args
            .iter_mut()
            .any(|argument| replace_f64_in_expression(argument, bits)),
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. }
        | hir::ResolvedExprKind::UpdateRecord { fields, .. } => fields
            .iter_mut()
            .any(|field| replace_f64_in_expression(&mut field.value, bits)),
        hir::ResolvedExprKind::Match { scrutinee, arms } => {
            replace_f64_in_expression(scrutinee, bits)
                || arms
                    .iter_mut()
                    .any(|arm| replace_f64_in_expression(&mut arm.value, bits))
        }
        _ => false,
    }
}

fn replace_first_f32_bits(program: &mut hir::ResolvedProgram, bits: u32) {
    for function in &mut program.functions {
        let expressions = [&mut function.body];
        for expression in expressions {
            if replace_f32_in_expression(expression, bits) {
                return;
            }
        }
    }
}

fn replace_f32_in_expression(expression: &mut hir::ResolvedExpr, bits: u32) -> bool {
    if matches!(expression.kind, hir::ResolvedExprKind::Float32(_)) {
        expression.kind = hir::ResolvedExprKind::Float32(bits);
        return true;
    }
    match &mut expression.kind {
        hir::ResolvedExprKind::Unary { value, .. }
        | hir::ResolvedExprKind::Try { operand: value, .. }
        | hir::ResolvedExprKind::TryOption { operand: value, .. }
        | hir::ResolvedExprKind::Project { base: value, .. } => {
            replace_f32_in_expression(value, bits)
        }
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            replace_f32_in_expression(left, bits) || replace_f32_in_expression(right, bits)
        }
        hir::ResolvedExprKind::Block { statements, tail } => {
            statements.iter_mut().any(|statement| {
                let hir::ResolvedStatement::Let { value, .. } = statement;
                replace_f32_in_expression(value, bits)
            }) || replace_f32_in_expression(tail, bits)
        }
        hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            replace_f32_in_expression(condition, bits)
                || replace_f32_in_expression(then_branch, bits)
                || replace_f32_in_expression(else_branch, bits)
        }
        hir::ResolvedExprKind::Call { args, .. } => args
            .iter_mut()
            .any(|argument| replace_f32_in_expression(argument, bits)),
        hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | hir::ResolvedExprKind::ConstructVariant { fields, .. }
        | hir::ResolvedExprKind::UpdateRecord { fields, .. } => fields
            .iter_mut()
            .any(|field| replace_f32_in_expression(&mut field.value, bits)),
        hir::ResolvedExprKind::Match { scrutinee, arms } => {
            replace_f32_in_expression(scrutinee, bits)
                || arms
                    .iter_mut()
                    .any(|arm| replace_f32_in_expression(&mut arm.value, bits))
        }
        _ => false,
    }
}

#[test]
fn scalar_export_profile_still_rejects_float_signatures() {
    let program = parse(SCALARS, Path::new("floats.spx")).unwrap();
    let error = wasm::emit_module_with_scalar_exports(&program, &["f.scale".to_owned()])
        .expect_err("float signatures are outside Public Scalar Export Profile v1");
    assert_eq!(error.code, "SPX-W115");
}

#[test]
fn native_float_records_have_frozen_layout_and_execute_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(RECORDS, Path::new("float-records-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let mix = format!("spx_record_{}", hex_identity("mix.type"));
    let ratio = format!("spx_field_{}", hex_identity("mix.ratio"));
    let point = format!("spx_field_{}", hex_identity("mix.point"));
    let flag = format!("spx_field_{}", hex_identity("mix.flag"));
    let x_field = format!("spx_field_{}", hex_identity("vec.x"));

    // Native64 layout: ratio f32 @0, point (align 8) @8..24, flag @24, total
    // rounds to alignment 8 => 32 bytes.
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {mix}) == UINT32_C(32)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {mix}, {ratio}) == UINT32_C(0)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {mix}, {point}) == UINT32_C(8)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {mix}, {flag}) == UINT32_C(24)"
    )));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {mix} output;
    memset(&output, 0xa5, sizeof(output));
    if ({make}(&context, 1.5, 2.5, &output) != SPX_STATUS_SUCCESS) return 11;
    if (!(output.{point}.{x_field} == 1.5)) return 12;
    double projected = 0.0;
    if ({project}(&context, &output, &projected) != SPX_STATUS_SUCCESS) return 13;
    if (!(projected == 1.5)) return 14;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(4)) return 15;
    return 0;
}}
"#,
        mix = mix,
        make = symbol("mix.make"),
        point = point,
        x_field = x_field,
        project = symbol("mix.project_x"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "float records");

    // Silence unused-symbol warnings in readers of this test.
    let _ = (ratio, flag);
}

const EXTENDED_SCALARS_PROBE: &str = r#"
#include <math.h>
int main(void) {
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    double wide = 0.0;
    if (spx_scale_call(&context, 1.5, &wide) != SPX_STATUS_SUCCESS) return 11;
    if (!(wide == 4.25)) return 12;
    float narrow_result = 0.0f;
    if (spx_narrow_call(&context, 9.0f, &narrow_result) != SPX_STATUS_SUCCESS) return 13;
    if (!(narrow_result == 2.0f)) return 14;
    int64_t class_value = INT64_C(0);
    if (spx_classify_call(&context, 1.0, 2.0, &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(1)) return 15;
    if (spx_classify_call(&context, 2.0, 2.0, &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(2)) return 16;
    if (spx_classify_call(&context, 3.0, 2.0, &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(3)) return 17;
    double inverted = 0.0;
    if (spx_invert_call(&context, 4.25, &inverted) != SPX_STATUS_SUCCESS) return 18;
    if (!(inverted == -4.25)) return 19;
    double quotient = 0.0;
    if (spx_divide_call(&context, 1.0, 0.0, &quotient) != SPX_STATUS_SUCCESS) return 20;
    if (!isinf(quotient) || !(quotient > 0.0)) return 21;
    if (spx_divide_call(&context, -1.0, 0.0, &quotient) != SPX_STATUS_SUCCESS) return 22;
    if (!isinf(quotient) || !(quotient < 0.0)) return 23;
    int64_t entry = 0;
    if (spx_main_call(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(7)) return 24;
    return 0;
}
"#;

#[test]
fn native_float_scalars_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SCALARS, Path::new("float-scalars-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("double spx_result"));
    assert!(generated.contains("float spx_result"));

    let probe = EXTENDED_SCALARS_PROBE
        .replace("spx_scale_call", &symbol_name("f.scale"))
        .replace("spx_narrow_call", &symbol_name("f.narrow"))
        .replace("spx_classify_call", &symbol_name("f.classify"))
        .replace("spx_invert_call", &symbol_name("f.invert"))
        .replace("spx_divide_call", &symbol_name("f.divide"))
        .replace("spx_main_call", &symbol_name("app.main"));
    run_native_probe(&generated, &probe, "float scalars");
}

fn symbol_name(id: &str) -> String {
    format!("spx_decl_{}", hex_identity(id))
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-float-native-{label}-{}-{id}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{label} C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let status = executed.status.code();
        let stderr = String::from_utf8_lossy(&executed.stderr).into_owned();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "{label} failed at {optimization}: status={status:?} stderr={stderr}"
        );
    }
}

#[test]
fn wasm_float_programs_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    for (source, expected, label) in [(SCALARS, 7i64, "scalars"), (RECORDS, 4i64, "records")] {
        let program = parse(source, Path::new("floats-wasm.spx")).unwrap();
        let bytes = wasm::emit_module(&program).unwrap();
        assert_eq!(bytes, wasm::emit_module(&program).unwrap(), "{label}");
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-float-wasm-{label}-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        std::fs::write(
            &script_path,
            r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const bytes = await readFile(process.argv[2]);
const expected = BigInt(process.argv[3]);
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
for (let index = 0; index < 4096; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("float-wasm-ok");
"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg(&script_path)
            .arg(&wasm_path)
            .arg(expected.to_string())
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&script_path);
        let _ = std::fs::remove_file(&wasm_path);
        assert!(
            output.status.success(),
            "Node float {label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "float-wasm-ok"
        );
    }
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
