use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SCALARS: &str = r#"
module test.char_scalars;

@id("c.first")
fn first() -> char { 'a' }

@id("c.classify")
fn classify(value: char) -> i64 {
    if value < 'b' {
        1
    } else {
        if value == 'z' { 2 } else { 3 }
    }
}

@id("c.pick")
fn pick(which: i64) -> char {
    if which == 0 { '\0' } else {
        if which == 1 { '\\' } else {
            if which == 2 { '\'' } else { '\u{1f600}' }
        }
    }
}

@id("app.main")
fn main() -> i64 {
    let letter = 'A';
    let newline = '\n';
    if first() == 'a' && classify(letter) == 1 && classify('z') == 2 && newline == '\n' {
        let nul = pick(0);
        if nul == '\0' && pick(6) > 'z' {
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
module test.char_records;

@id("glyph.type")
record Glyph {
    @id("glyph.symbol") symbol: char,
    @id("glyph.weight") weight: i64,
}

@id("glyph.make")
fn make(symbol: char) -> Glyph { Glyph { symbol: symbol, weight: 2 } }

@id("glyph.heavier_symbol")
fn heavier_symbol(left: Glyph, right: Glyph) -> char {
    if left.weight > right.weight { left.symbol } else { right.symbol }
}

@id("app.main")
fn main() -> i64 {
    let light = make('x');
    let heavy = light with { weight: 9 };
    if heavier_symbol(light, heavy) == 'x' && heavy.symbol == 'x' && light.symbol < 'y' {
        4
    } else {
        5
    }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("chars.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn char_programs_round_trip_canonically_and_hash_stably() {
    for source in [SCALARS, RECORDS] {
        let program = parse(source, Path::new("chars.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert!(verify::verify(&reparsed).is_empty());
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
        assert_eq!(format::canonical(&reparsed), canonical);
    }

    let program = parse(SCALARS, Path::new("chars.spx")).unwrap();
    let canonical = format::canonical(&program);
    assert!(canonical.contains("'\\n'"), "named escapes stay canonical");
    assert!(
        canonical.contains("'\\u{1f600}'"),
        "non-ASCII scalars project as lowercase unicode escapes: {canonical}"
    );
    assert!(canonical.contains("'A'"), "printable ASCII stays direct");
}

#[test]
fn every_named_escape_canonicalizes_exactly() {
    let source = r#"
module test.char_escapes;
@id("t.pick")
fn pick(which: i64) -> char {
    if which == 0 { '\0' } else {
        if which == 1 { '\t' } else {
            if which == 2 { '\r' } else {
                if which == 3 { '\\' } else { '\'' }
            }
        }
    }
}
@id("app.main")
fn main() -> i64 {
    let quote = pick(4);
    if pick(0) == '\0' && pick(1) == '\t' && pick(2) == '\r' && pick(3) == '\\' && quote == '\'' {
        1
    } else {
        0
    }
}
"#;
    let program = parse(source, Path::new("escapes.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    for escape in ["'\\0'", "'\\t'", "'\\r'", "'\\\\'", "'\\''"] {
        assert!(
            canonical.contains(escape),
            "escape {escape} must survive the canonical projection: {canonical}"
        );
    }
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
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
        | hir::ResolvedExprKind::Uint8(_)
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

#[test]
fn resolved_hir_keeps_exact_char_scalars_and_types() {
    let program = hir::resolve(&parse(SCALARS, Path::new("chars.spx")).unwrap()).unwrap();

    let first = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "c.first")
        .unwrap();
    assert_eq!(first.return_type, hir::ResolvedType::Char);
    let mut scalars = Vec::new();
    walk(&first.body, &mut |expression| {
        if let hir::ResolvedExprKind::Char(value) = &expression.kind {
            assert_eq!(expression.ty, hir::ResolvedType::Char);
            scalars.push(*value);
        }
    });
    assert_eq!(scalars, vec![0x61]);

    // Comparisons keep char operands and produce bool results.
    let classify = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "c.classify")
        .unwrap();
    assert_eq!(classify.params[0].ty, hir::ResolvedType::Char);
    let hir::ResolvedExprKind::If { condition, .. } = &body_tail(classify).kind else {
        panic!("classify body must branch");
    };
    assert_eq!(condition.ty, hir::ResolvedType::Bool);
    let hir::ResolvedExprKind::Binary { op, left, .. } = &condition.kind else {
        panic!("classify condition must compare");
    };
    assert_eq!(*op, ast::BinaryOp::Lt);
    assert_eq!(left.ty, hir::ResolvedType::Char);
}

/// Single-expression bodies are canonical blocks; this returns the tail.
fn body_tail(function: &hir::ResolvedFunction) -> &hir::ResolvedExpr {
    let hir::ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("function bodies are blocks");
    };
    tail
}

#[test]
fn graph_json_exposes_deterministic_char_nodes() {
    let program = parse(SCALARS, Path::new("chars.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"char\""));
    assert!(first.contains("\"name\":\"char\""));
    // The exact scalar payload and canonical display survive serialization.
    assert!(first.contains("\"value\":97,\"display\":\"'a'\""));
    assert!(first.contains("\"display\":\"'\\\\n'\""));
}

#[test]
fn char_type_mismatch_diagnostics_are_stable() {
    let arithmetic = diagnostics(
        r#"
module test.char_arith;
@id("app.main")
fn main() -> i64 { if 'a' + 'b' == 'c' { 7 } else { 8 } }
"#,
    );
    assert!(
        arithmetic
            .iter()
            .any(|item| item.code == "SPX-T208" && item.message.contains('+')),
        "char arithmetic must be rejected: {arithmetic:?}"
    );

    let negation = diagnostics(
        r#"
module test.char_neg;
@id("app.main")
fn main() -> i64 { let negated = -'a'; 0 }
"#,
    );
    assert!(
        negation.iter().any(|item| item.code == "SPX-T206"),
        "char negation must be rejected"
    );

    let mixed_equality = diagnostics(
        r#"
module test.char_equal;
@id("app.main")
fn main() -> i64 { if 'a' == 97 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_equality.iter().any(|item| item.code == "SPX-T207"),
        "mixed char-integer equality must be rejected"
    );

    let mixed_ordering = diagnostics(
        r#"
module test.char_order;
@id("app.main")
fn main() -> i64 { if 'a' < 98 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_ordering.iter().any(|item| item.code == "SPX-T208"),
        "mixed char-integer ordering must be rejected"
    );

    let argument = diagnostics(
        r#"
module test.char_arg;
@id("t.take")
fn take(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { take('a') }
"#,
    );
    assert!(
        argument.iter().any(|item| item.code == "SPX-T205"),
        "implicit char-to-int conversion must be rejected"
    );

    let condition = diagnostics(
        r#"
module test.char_cond;
@id("app.main")
fn main() -> i64 { if 'a' { 7 } else { 8 } }
"#,
    );
    assert!(
        condition.iter().any(|item| item.code == "SPX-T210"),
        "char conditions must be rejected"
    );
}

#[test]
fn char_literal_lexer_diagnostics_are_stable() {
    let cases: [(&str, &str); 5] = [
        ("let empty = ''; 0", "SPX-P008"),
        ("let multi = 'ab'; 0", "SPX-P008"),
        ("let bad_escape = '\\q'; 0", "SPX-P007"),
        ("let bad_unicode = '\\u{110000}'; 0", "SPX-P007"),
        ("let surrogate = '\\u{d800}'; 0", "SPX-P007"),
    ];
    for (statement, expected) in cases {
        let source = format!(
            r#"
module test.char_lex;
@id("app.main")
fn main() -> i64 {{ {statement} }}
"#
        );
        let error = parse(&source, Path::new("char-lex.spx")).unwrap_err();
        assert_eq!(error.code, expected, "{statement}: {error}");
    }

    let unterminated = parse(
        "module t;\n@id(\"m\")\nfn m() -> char { 'a",
        Path::new("eof.spx"),
    )
    .unwrap_err();
    assert_eq!(unterminated.code, "SPX-P006");

    let unclosed = parse(
        "module t;\n@id(\"m\")\nfn m() -> i64 { let c = 'x; 0 }",
        Path::new("run.spx"),
    )
    .unwrap_err();
    assert_eq!(unclosed.code, "SPX-P008");
}

#[test]
fn native_char_scalars_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SCALARS, Path::new("char-scalars-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("uint32_t spx_result"));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    uint32_t letter = UINT32_C(0);
    if ({first}(&context, &letter) != SPX_STATUS_SUCCESS || letter != UINT32_C(97)) return 11;
    int64_t class_value = INT64_C(0);
    if ({classify}(&context, UINT32_C(97), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(1)) return 12;
    if ({classify}(&context, UINT32_C(122), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(2)) return 13;
    if ({classify}(&context, UINT32_C(126), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(3)) return 14;
    uint32_t picked = UINT32_C(0);
    if ({pick}(&context, INT64_C(3), &picked) != SPX_STATUS_SUCCESS || picked != UINT32_C(0x1f600)) return 15;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(7)) return 16;
    return 0;
}}
"#,
        first = symbol("c.first"),
        classify = symbol("c.classify"),
        pick = symbol("c.pick"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "char scalars");
}

#[test]
fn native_char_records_keep_four_byte_symbols() {
    if !command_available("clang") {
        return;
    }
    let program = parse(RECORDS, Path::new("char-records-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    let glyph = format!("spx_record_{}", hex_identity("glyph.type"));
    let symbol_field = format!("spx_field_{}", hex_identity("glyph.symbol"));
    let weight_field = format!("spx_field_{}", hex_identity("glyph.weight"));

    // Native64 layout: symbol char (uint32_t) @0..4, weight @8, size 16.
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {glyph}) == UINT32_C(16)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {glyph}, {symbol_field}) == UINT32_C(0)"
    )));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {glyph} output;
    memset(&output, 0xa5, sizeof(output));
    if ({make}(&context, UINT32_C(120), &output) != SPX_STATUS_SUCCESS) return 11;
    if (output.{symbol_field} != UINT32_C(120)) return 12;
    uint32_t heavier = UINT32_C(0);
    struct {glyph} light = output;
    light.{weight_field} = INT64_C(1);
    if ({heavier}(&context, &light, &output, &heavier) != SPX_STATUS_SUCCESS || heavier != UINT32_C(120)) return 13;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(4)) return 14;
    return 0;
}}
"#,
        glyph = glyph,
        make = symbol("glyph.make"),
        symbol_field = symbol_field,
        weight_field = weight_field,
        heavier = symbol("glyph.heavier_symbol"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "char records");
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-char-native-{label}-{}-{id}", std::process::id());
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
fn wasm_char_programs_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    for (source, expected, label) in [(SCALARS, 7i64, "scalars"), (RECORDS, 4i64, "records")] {
        let program = parse(source, Path::new("chars-wasm.spx")).unwrap();
        let bytes = wasm::emit_module(&program).unwrap();
        assert_eq!(bytes, wasm::emit_module(&program).unwrap(), "{label}");
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-char-wasm-{label}-{}-{id}", std::process::id());
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
console.log("char-wasm-ok");
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
            "Node char {label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "char-wasm-ok"
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
