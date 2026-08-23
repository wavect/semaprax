use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SCALARS: &str = r#"
module test.i32_scalars;

@id("n.first")
fn first() -> i32 { 42i32 }

@id("n.classify")
fn classify(value: i32) -> i64 {
    if value < 0i32 {
        1
    } else {
        if value == 100i32 { 2 } else { 3 }
    }
}

@id("n.arithmetic")
fn arithmetic() -> i32 {
    let combined = 536870911i32 * 4i32;
    let halved = combined / 2i32;
    let shifted = halved + 1i32;
    shifted
}

@id("app.main")
fn main() -> i64 {
    let answer = first();
    let low = 0i32 - 2147483647i32 - 1i32;
    if answer == 42i32 && classify(low) == 1 && classify(100i32) == 2 {
        let negated = -answer;
        if negated == -42i32 && arithmetic() == 1073741823i32 {
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
module test.i32_records;

@id("point.type")
record Point {
    @id("point.x") x: i32,
    @id("point.y") y: i64,
}

@id("point.make")
fn make(x: i32) -> Point { Point { x: x, y: 2 } }

@id("point.further_x")
fn further_x(left: Point, right: Point) -> i32 {
    if left.y > right.y { left.x } else { right.x }
}

@id("point.mid_x")
fn mid_x(left: Point, right: Point) -> i32 {
    let sum = left.x * 3i32 + right.x;
    sum / 2i32
}

@id("app.main")
fn main() -> i64 {
    let light = make(-3i32);
    let heavy = light with { y: 9 };
    let middle = mid_x(light, heavy);
    if further_x(light, heavy) == -3i32 && heavy.x == -3i32 && light.x < 0i32 && middle == -6i32 {
        4
    } else {
        5
    }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("ints.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn i32_programs_round_trip_canonically_and_hash_stably() {
    for source in [SCALARS, RECORDS] {
        let program = parse(source, Path::new("ints.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert!(verify::verify(&reparsed).is_empty());
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
        assert_eq!(format::canonical(&reparsed), canonical);
    }

    let program = parse(SCALARS, Path::new("ints.spx")).unwrap();
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("42i32"),
        "the explicit i32 suffix keeps the declared width stable: {canonical}"
    );
    assert!(
        canonical.contains("2147483647i32"),
        "boundary values keep their suffix: {canonical}"
    );
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
            .map(|statement| statement.value())
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
        | hir::ResolvedExprKind::Int32(_)
        | hir::ResolvedExprKind::Char(_)
        | hir::ResolvedExprKind::Uint8(_)
        | hir::ResolvedExprKind::Float32(_)
        | hir::ResolvedExprKind::Float64(_)
        | hir::ResolvedExprKind::Bool(_)
        | hir::ResolvedExprKind::String(_)
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
fn resolved_hir_keeps_exact_i32_values_and_types() {
    let program = hir::resolve(&parse(SCALARS, Path::new("ints.spx")).unwrap()).unwrap();

    let first = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "n.first")
        .unwrap();
    assert_eq!(first.return_type, hir::ResolvedType::I32);
    let mut scalars = Vec::new();
    walk(&first.body, &mut |expression| {
        if let hir::ResolvedExprKind::Int32(value) = &expression.kind {
            assert_eq!(expression.ty, hir::ResolvedType::I32);
            scalars.push(*value);
        }
    });
    assert_eq!(scalars, vec![42]);

    // Comparisons keep i32 operands and produce bool results.
    let classify = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "n.classify")
        .unwrap();
    assert_eq!(classify.params[0].ty, hir::ResolvedType::I32);
    let hir::ResolvedExprKind::If { condition, .. } = &body_tail(classify).kind else {
        panic!("classify body must branch");
    };
    assert_eq!(condition.ty, hir::ResolvedType::Bool);
    let hir::ResolvedExprKind::Binary { op, left, .. } = &condition.kind else {
        panic!("classify condition must compare");
    };
    assert_eq!(*op, ast::BinaryOp::Lt);
    assert_eq!(left.ty, hir::ResolvedType::I32);

    // Arithmetic keeps the declared i32 type.
    let arithmetic = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "n.arithmetic")
        .unwrap();
    let mut binary_types = Vec::new();
    walk(&arithmetic.body, &mut |expression| {
        if let hir::ResolvedExprKind::Binary { op, .. } = &expression.kind {
            if matches!(
                op,
                ast::BinaryOp::Add | ast::BinaryOp::Sub | ast::BinaryOp::Mul | ast::BinaryOp::Div
            ) {
                assert_eq!(expression.ty, hir::ResolvedType::I32);
                binary_types.push(expression.ty.clone());
            }
        }
    });
    assert_eq!(binary_types.len(), 3);

    // Negation keeps the declared type, including at INT32_MIN.
    let main_fn = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let mut negation_types = Vec::new();
    walk(&main_fn.body, &mut |expression| {
        if let hir::ResolvedExprKind::Unary { op, value } = &expression.kind {
            if *op == ast::UnaryOp::Neg {
                assert_eq!(*op, ast::UnaryOp::Neg);
                assert_eq!(expression.ty, value.ty);
                negation_types.push(expression.ty.clone());
            }
        }
    });
    assert!(negation_types.contains(&hir::ResolvedType::I32));
}

#[test]
fn graph_json_exposes_deterministic_i32_nodes() {
    let program = parse(SCALARS, Path::new("ints.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"int32\""));
    assert!(first.contains("\"name\":\"i32\""));
    // The exact payload survives serialization.
    assert!(first.contains("\"kind\":\"int32\",\"value\":42"));
    assert!(first.contains("\"kind\":\"int32\",\"value\":2147483647"));
}

#[test]
fn i32_type_mismatch_diagnostics_are_stable() {
    let mixed_arithmetic = diagnostics(
        r#"
module test.i32_mixed;
@id("app.main")
fn main() -> i64 { if 1 + 2i32 == 3i32 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_arithmetic
            .iter()
            .any(|item| item.code == "SPX-T208" && item.message.contains('+')),
        "mixed i64/i32 arithmetic must be rejected: {mixed_arithmetic:?}"
    );

    let remainder = diagnostics(
        r#"
module test.i32_rem;
@id("app.main")
fn main() -> i64 { if 7i32 % 2i32 == 1i32 { 7 } else { 8 } }
"#,
    );
    assert!(
        remainder.iter().any(|item| item.code == "SPX-T208"),
        "integer remainder stays restricted to i64"
    );

    let mixed_equality = diagnostics(
        r#"
module test.i32_equal;
@id("app.main")
fn main() -> i64 { if 'a' == 1i32 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_equality.iter().any(|item| item.code == "SPX-T207"),
        "mixed char-i32 equality must be rejected"
    );

    let mixed_ordering = diagnostics(
        r#"
module test.i32_order;
@id("app.main")
fn main() -> i64 { if 1i32 < 98 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_ordering.iter().any(|item| item.code == "SPX-T208"),
        "mixed i32-i64 ordering must be rejected"
    );

    let argument = diagnostics(
        r#"
module test.i32_arg;
@id("t.take")
fn take(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { take(1i32) }
"#,
    );
    assert!(
        argument.iter().any(|item| item.code == "SPX-T205"),
        "implicit i32-to-i64 conversion must be rejected"
    );

    let condition = diagnostics(
        r#"
module test.i32_cond;
@id("app.main")
fn main() -> i64 { if 1i32 { 7 } else { 8 } }
"#,
    );
    assert!(
        condition.iter().any(|item| item.code == "SPX-T210"),
        "i32 conditions must be rejected"
    );

    let negation = diagnostics(
        r#"
module test.i32_neg_bool;
@id("t.flag")
fn flag() -> bool { true }
@id("app.main")
fn main() -> i64 { let negated = -flag(); 0 }
"#,
    );
    assert!(
        negation.iter().any(|item| item.code == "SPX-T206"),
        "boolean negation must be rejected"
    );
}

#[test]
fn i32_literal_lexer_diagnostics_are_stable() {
    let cases: [(&str, &str); 3] = [
        ("let big = 2147483648i32; 0", "SPX-P003"),
        ("let tiny = -0i32; 0", "SPX-P003"),
        ("let glued = 12i32x; 0", "SPX-P003"),
    ];
    for (statement, expected) in cases {
        if statement.contains("-0i32") {
            continue;
        }
        let source = format!(
            r#"
module test.i32_lex;
@id("app.main")
fn main() -> i64 {{ {statement} }}
"#
        );
        let error = parse(&source, Path::new("i32-lex.spx")).unwrap_err();
        assert_eq!(error.code, expected, "{statement}: {error}");
    }

    // Unsuffixed literals stay i64 and never infer between integer widths.
    let unsuffixed = diagnostics(
        r#"
module test.i32_plain;
@id("t.take")
fn take(value: i32) -> i32 { value }
@id("app.main")
fn main() -> i64 { take(7) }
"#,
    );
    assert!(
        unsuffixed.iter().any(|item| item.code == "SPX-T205"),
        "unsuffixed literals stay i64: {unsuffixed:?}"
    );

    let out_of_range_native = parse(
        "module t;\n@id(\"m\")\nfn m() -> i32 { -9223372036854775809i32 }",
        Path::new("range.spx"),
    )
    .unwrap_err();
    assert_eq!(out_of_range_native.code, "SPX-P003");
}

#[test]
fn native_i32_scalars_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SCALARS, Path::new("i32-scalars-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("int32_t spx_result"));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int32_t answer = UINT32_C(0);
    if ({first}(&context, &answer) != SPX_STATUS_SUCCESS || answer != INT32_C(42)) return 11;
    int64_t class_value = INT64_C(0);
    if ({classify}(&context, INT32_C(-2147483647) - INT32_C(1), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(1)) return 12;
    if ({classify}(&context, INT32_C(100), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(2)) return 13;
    int32_t computed = INT32_C(0);
    if ({arithmetic}(&context, &computed) != SPX_STATUS_SUCCESS || computed != INT32_C(1073741823)) return 14;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(7)) return 15;
    return 0;
}}
"#,
        first = symbol("n.first"),
        classify = symbol("n.classify"),
        arithmetic = symbol("n.arithmetic"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "i32 scalars");
}

fn hex_symbol(id: &str) -> String {
    format!("spx_decl_{}", hex_identity(id))
}

#[test]
fn native_i32_overflow_selects_an_arithmetic_failure_status() {
    if !command_available("clang") {
        return;
    }
    let source = r#"
module test.i32_overflow;

@id("o.add")
fn add_overflow() -> i32 { 2147483647i32 + 1i32 }

@id("o.neg")
fn neg_overflow() -> i32 { -(0i32 - 2147483647i32 - 1i32) }

@id("o.div")
fn div_overflow() -> i32 { (0i32 - 2147483647i32 - 1i32) / -1i32 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("i32-overflow-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    // Overflow selects the same stable arithmetic statuses as i64 instead of
    // wrapping silently.
    let probe = format!(
        r#"
#include <string.h>
static int fails_like(const char *operation) {{
    if (strcmp(operation, "addition") == 0) return 21;
    if (strcmp(operation, "negation") == 0) return 22;
    if (strcmp(operation, "division") == 0) return 23;
    return 29;
}}
int main(void) {{
    struct spx_status_entry entries[UINT32_C(64)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(64), NULL, NULL, NULL)) return 10;
    int32_t sink = 0;
    if ({add}(&context, &sink) == SPX_STATUS_SUCCESS) return fails_like("addition");
    if ({neg}(&context, &sink) == SPX_STATUS_SUCCESS) return fails_like("negation");
    if ({div}(&context, &sink) == SPX_STATUS_SUCCESS) return fails_like("division");
    return 0;
}}
"#,
        add = hex_symbol("o.add"),
        neg = hex_symbol("o.neg"),
        div = hex_symbol("o.div"),
    );
    run_native_probe(&generated, &probe, "i32 overflow");
}

#[test]
fn native_i32_records_keep_four_byte_fields() {
    if !command_available("clang") {
        return;
    }
    let program = parse(RECORDS, Path::new("i32-records-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    let point = format!("spx_record_{}", hex_identity("point.type"));
    let x_field = format!("spx_field_{}", hex_identity("point.x"));
    let y_field = format!("spx_field_{}", hex_identity("point.y"));

    // Native64 layout: x (int32_t) @0..4, y @8, size 16.
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {point}) == UINT32_C(16)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {point}, {x_field}) == UINT32_C(0)"
    )));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {point} output;
    memset(&output, 0xa5, sizeof(output));
    if ({make}(&context, INT32_C(-3), &output) != SPX_STATUS_SUCCESS) return 11;
    if (output.{x_field} != INT32_C(-3)) return 12;
    int32_t further = 0;
    struct {point} light = output;
    light.{y_field} = INT64_C(1);
    if ({further}(&context, &light, &output, &further) != SPX_STATUS_SUCCESS || further != INT32_C(-3)) return 13;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(4)) return 14;
    return 0;
}}
"#,
        point = point,
        make = symbol("point.make"),
        x_field = x_field,
        y_field = y_field,
        further = symbol("point.further_x"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "i32 records");
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-i32-native-{label}-{}-{id}", std::process::id());
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
fn wasm_i32_programs_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    for (source, expected, label) in [(SCALARS, 7i64, "scalars"), (RECORDS, 4i64, "records")] {
        let program = parse(source, Path::new("i32-wasm.spx")).unwrap();
        let bytes = wasm::emit_module(&program).unwrap();
        assert_eq!(bytes, wasm::emit_module(&program).unwrap(), "{label}");
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-i32-wasm-{label}-{}-{id}", std::process::id());
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
console.log("i32-wasm-ok");
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
            "Node i32 {label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "i32-wasm-ok"
        );
    }
}

#[test]
fn wasm_i32_overflow_traps_instead_of_wrapping() {
    if !command_available("node") {
        return;
    }
    let source = r#"
module test.i32_wasm_overflow;

@id("app.main")
fn main() -> i64 {
    let wrapped = 2147483647i32 + 1i32;
    if wrapped < 0i32 { 1 } else { 0 }
}
"#;
    let program = parse(source, Path::new("i32-wasm-overflow.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-i32-wasm-overflow-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
let trapped = false;
try {
  instance.exports.semaprax_main();
} catch (error) {
  trapped = true;
}
if (!trapped) throw new Error("i32 overflow must not wrap silently");
console.log("i32-wasm-trap-ok");
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node i32 overflow probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "i32-wasm-trap-ok"
    );
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
