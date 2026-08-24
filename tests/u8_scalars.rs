use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SCALARS: &str = r#"
module test.u8_scalars;

@id("b.max")
fn max() -> u8 { 255u8 }

@id("b.classify")
fn classify(value: u8) -> i64 {
    if value < 10u8 {
        1
    } else {
        if value == 200u8 { 2 } else { 3 }
    }
}

@id("b.pick")
fn pick(which: i64) -> u8 {
    if which == 0 { 0u8 } else {
        if which == 1 { 1u8 } else { 128u8 }
    }
}

@id("b.checked")
fn checked(left: u8, right: u8) -> u8 { left + right }

@id("app.main")
fn main() -> i64 {
    let small = 7u8;
    if max() == 255u8 && classify(small) == 1 && classify(200u8) == 2 && pick(0) == 0u8 {
        let sum = checked(200u8, 55u8);
        if sum == 255u8 && pick(6) > 8u8 && 4u8 / 2u8 == 2u8 && 9u8 - 1u8 == 8u8 {
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
module test.u8_records;

@id("tag.type")
record Sample {
    @id("tag.value") tag: u8,
    @id("tag.weight") weight: i64,
}

@id("tag.make")
fn make(tag: u8) -> Sample { Sample { tag: tag, weight: 2 } }

@id("tag.heavier_tag")
fn heavier_tag(left: Sample, right: Sample) -> u8 {
    if left.weight > right.weight { left.tag } else { right.tag }
}

@id("app.main")
fn main() -> i64 {
    let light = make(10u8);
    let heavy = light with { weight: 9 };
    if heavier_tag(light, heavy) == 10u8 && heavy.tag == 10u8 && light.tag < 11u8 {
        4
    } else {
        5
    }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("bytes.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn u8_programs_round_trip_canonically_and_hash_stably() {
    for source in [SCALARS, RECORDS] {
        let program = parse(source, Path::new("bytes.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let canonical = format::canonical(&program);
        let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert!(verify::verify(&reparsed).is_empty());
        assert_eq!(graph::revision(&program), graph::revision(&reparsed));
        assert_eq!(format::canonical(&reparsed), canonical);
    }

    let program = parse(SCALARS, Path::new("bytes.spx")).unwrap();
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("255u8"),
        "explicit suffix keeps declared width stable: {canonical}"
    );
    assert!(
        canonical.contains("0u8"),
        "zero stays suffixed: {canonical}"
    );
}

#[test]
fn every_u8_boundary_literal_round_trips_exactly() {
    let source = r#"
module test.u8_bounds;
@id("t.pick")
fn pick(which: i64) -> u8 {
    if which == 0 { 0u8 } else {
        if which == 1 { 1u8 } else {
            if which == 2 { 127u8 } else {
                if which == 3 { 128u8 } else {
                    if which == 4 { 254u8 } else { 255u8 }
                }
            }
        }
    }
}
@id("app.main")
fn main() -> i64 {
    if pick(0) == 0u8 && pick(1) == 1u8 && pick(2) == 127u8 && pick(3) == 128u8
        && pick(4) == 254u8 && pick(5) == 255u8 {
        1
    } else {
        0
    }
}
"#;
    let program = parse(source, Path::new("bounds.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    for literal in ["0u8", "1u8", "127u8", "128u8", "254u8", "255u8"] {
        assert!(
            canonical.contains(literal),
            "boundary literal {literal} must survive the canonical projection: {canonical}"
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
        | hir::ResolvedExprKind::Project { base: value, .. }
        | hir::ResolvedExprKind::Upcast { source: value } => vec![value.as_ref()],
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
fn resolved_hir_keeps_exact_u8_scalars_and_types() {
    let program = hir::resolve(&parse(SCALARS, Path::new("bytes.spx")).unwrap()).unwrap();

    let max_fn = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "b.max")
        .unwrap();
    assert_eq!(max_fn.return_type, hir::ResolvedType::U8);
    let mut scalars = Vec::new();
    walk(&max_fn.body, &mut |expression| {
        if let hir::ResolvedExprKind::Uint8(value) = &expression.kind {
            assert_eq!(expression.ty, hir::ResolvedType::U8);
            scalars.push(*value);
        }
    });
    assert_eq!(scalars, vec![255]);

    // Comparisons keep u8 operands and produce bool results; checked addition
    // keeps the u8 result type.
    let checked = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "b.checked")
        .unwrap();
    assert_eq!(checked.params[0].ty, hir::ResolvedType::U8);
    assert_eq!(checked.params[1].ty, hir::ResolvedType::U8);
    assert_eq!(checked.return_type, hir::ResolvedType::U8);
    let hir::ResolvedExprKind::Binary { op, left, .. } = &body_tail(checked).kind else {
        panic!("checked body must add");
    };
    assert_eq!(*op, ast::BinaryOp::Add);
    assert_eq!(left.ty, hir::ResolvedType::U8);
    assert_eq!(body_tail(checked).ty, hir::ResolvedType::U8);
}

#[test]
fn graph_json_exposes_deterministic_uint8_nodes() {
    let program = parse(SCALARS, Path::new("bytes.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"uint8\""));
    assert!(first.contains("\"name\":\"u8\""));
    assert!(first.contains("\"layout_key\":\"scalar:u8\""));
    // The exact scalar payload survives serialization.
    assert!(first.contains("\"kind\":\"uint8\",\"value\":255"));
    assert!(first.contains("\"kind\":\"uint8\",\"value\":200"));
}

#[test]
fn u8_type_mismatch_diagnostics_are_stable() {
    let mixed_equality = diagnostics(
        r#"
module test.u8_equal;
@id("app.main")
fn main() -> i64 { if 1 == 1u8 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_equality.iter().any(|item| item.code == "SPX-T207"),
        "mixed integer-u8 equality must be rejected: {:?}",
        mixed_equality
    );

    let boolean_operand = diagnostics(
        r#"
module test.u8_boolean;
@id("app.main")
fn main() -> i64 { if 1u8 && 2u8 { 7 } else { 8 } }
"#,
    );
    assert!(
        boolean_operand.iter().any(|item| item.code == "SPX-T208"),
        "boolean operators reject u8 operands"
    );

    let condition = diagnostics(
        r#"
module test.u8_cond;
@id("app.main")
fn main() -> i64 { if 2u8 { 7 } else { 8 } }
"#,
    );
    assert!(
        condition.iter().any(|item| item.code == "SPX-T210"),
        "u8 conditions must be rejected"
    );

    let argument = diagnostics(
        r#"
module test.u8_arg;
@id("t.take")
fn take(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { take(2u8) }
"#,
    );
    assert!(
        argument.iter().any(|item| item.code == "SPX-T205"),
        "implicit u8-to-int conversion must be rejected"
    );

    let negation = diagnostics(
        r#"
module test.u8_neg;
@id("app.main")
fn main() -> i64 { let b = 2u8; let negated = -b; 0 }
"#,
    );
    assert!(
        negation.iter().any(|item| item.code == "SPX-T206"),
        "u8 negation must be rejected"
    );

    let remainder = diagnostics(
        r#"
module test.u8_rem;
@id("app.main")
fn main() -> i64 { if 9u8 % 2u8 == 1u8 { 7 } else { 8 } }
"#,
    );
    assert!(
        remainder.iter().any(|item| item.code == "SPX-T208"),
        "remainder stays i64-only"
    );

    let mixed_ordering = diagnostics(
        r#"
module test.u8_order;
@id("app.main")
fn main() -> i64 { if 1u8 < 2 { 7 } else { 8 } }
"#,
    );
    assert!(
        mixed_ordering.iter().any(|item| item.code == "SPX-T208"),
        "mixed u8-integer ordering must be rejected"
    );

    let generic_slot = diagnostics(
        r#"
module test.u8_generic;
@id("t.pick")
fn pick<T>(value: T) -> T { value }
@id("app.main")
fn main() -> i64 { if pick<u8>(2u8) == 2u8 { 7 } else { 8 } }
"#,
    );
    assert!(
        !generic_slot.is_empty(),
        "u8 stays outside generic instantiation arguments"
    );
}

#[test]
fn u8_literal_lexer_diagnostics_are_stable() {
    let cases: [(&str, &str); 3] = [
        ("let over = 256u8; 0", "SPX-P003"),
        ("let bad_suffix = 12ux; 0", "SPX-P003"),
        ("let glued = 12u81; 0", "SPX-P003"),
    ];
    for (statement, expected) in cases {
        let source = format!(
            r#"
module test.u8_lex;
@id("app.main")
fn main() -> i64 {{ {statement} }}
"#
        );
        let error = parse(&source, Path::new("u8-lex.spx")).unwrap_err();
        assert_eq!(error.code, expected, "{statement}: {error}");
    }

    // Plain digit runs stay unsuffixed i64 literals.
    let plain = parse(
        "module t;\n@id(\"app.main\")\nfn main() -> i64 { 3000000000 }",
        Path::new("plain.spx"),
    )
    .unwrap();
    assert!(verify::verify(&plain).is_empty());
}

#[test]
fn native_u8_scalars_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SCALARS, Path::new("u8-scalars-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated.contains("uint8_t spx_result"));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    uint8_t byte_out = UINT8_C(0);
    if ({max_fn}(&context, &byte_out) != SPX_STATUS_SUCCESS || byte_out != UINT8_C(255)) return 11;
    int64_t class_value = INT64_C(0);
    if ({classify}(&context, UINT8_C(7), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(1)) return 12;
    if ({classify}(&context, UINT8_C(200), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(2)) return 13;
    if ({classify}(&context, UINT8_C(201), &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(3)) return 14;
    uint8_t picked = UINT8_C(0);
    if ({pick}(&context, INT64_C(2), &picked) != SPX_STATUS_SUCCESS || picked != UINT8_C(128)) return 15;
    uint8_t sum = UINT8_C(0);
    if ({checked}(&context, UINT8_C(200), UINT8_C(55), &sum) != SPX_STATUS_SUCCESS || sum != UINT8_C(255)) return 16;
    if ({checked}(&context, UINT8_C(250), UINT8_C(10), &sum) == SPX_STATUS_SUCCESS) return 17;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(7)) return 19;
    return 0;
}}
"#,
        max_fn = symbol("b.max"),
        classify = symbol("b.classify"),
        pick = symbol("b.pick"),
        checked = symbol("b.checked"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "u8 scalars");
}

#[test]
fn native_u8_checked_underflow_and_division_select_exact_statuses() {
    if !command_available("clang") {
        return;
    }
    let underflow_source = r#"
module test.u8_underflow_wrapper;
@id("app.main")
fn main() -> i64 { if 0u8 - 1u8 == 255u8 { 1 } else { 2 } }
"#;
    let program = parse(underflow_source, Path::new("u8-underflow.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let main_symbol = format!("spx_decl_{}", hex_identity("app.main"));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t entry = 0;
    if ({main_symbol}(&context, &entry) == SPX_STATUS_SUCCESS) return 11;
    return 0;
}}
"#,
    );
    run_native_probe(&generated, &probe, "u8 underflow");

    let division_source = r#"
module test.u8_division_wrapper;
@id("app.main")
fn main() -> i64 { if 4u8 / 0u8 == 0u8 { 1 } else { 2 } }
"#;
    let program = parse(division_source, Path::new("u8-division.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    run_native_probe(&generated, &probe, "u8 division by zero");

    // The ordinary entry wrapper publishes the checked-arithmetic failure
    // with the exact normalized exit path (71) and operation detail.
    let overflow_entry = compile_entry_wrapper(
        r#"
module test.u8_overflow_wrapper;
@id("app.main")
fn main() -> i64 { if 255u8 + 1u8 == 0u8 { 1 } else { 2 } }
"#,
        "u8 overflow",
    );
    assert_eq!(overflow_entry.status.code(), Some(71));
    let stderr = String::from_utf8_lossy(&overflow_entry.stderr).into_owned();
    assert!(stderr.contains("addition overflow"), "stderr={stderr}");
}

#[test]
fn native_u8_records_keep_one_byte_tags_before_padding() {
    if !command_available("clang") {
        return;
    }
    let program = parse(RECORDS, Path::new("u8-records-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    let sample = format!("spx_record_{}", hex_identity("tag.type"));
    let tag_field = format!("spx_field_{}", hex_identity("tag.value"));
    let weight_field = format!("spx_field_{}", hex_identity("tag.weight"));

    // Native64 layout: tag u8 @0 size 1, pad to 8, weight @8, total 16.
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {sample}) == UINT32_C(16)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {sample}, {tag_field}) == UINT32_C(0)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {sample}, {weight_field}) == UINT32_C(8)"
    )));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {sample} output;
    memset(&output, 0xa5, sizeof(output));
    if ({make}(&context, UINT8_C(120), &output) != SPX_STATUS_SUCCESS) return 11;
    if (output.{tag_field} != UINT8_C(120)) return 12;
    if (sizeof(output.{tag_field}) != UINT32_C(1)) return 13;
    uint8_t heavier = UINT8_C(0);
    struct {sample} light = output;
    light.{weight_field} = INT64_C(1);
    if ({heavier}(&context, &light, &output, &heavier) != SPX_STATUS_SUCCESS || heavier != UINT8_C(120)) return 14;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(4)) return 15;
    return 0;
}}
"#,
        sample = sample,
        make = symbol("tag.make"),
        tag_field = tag_field,
        weight_field = weight_field,
        heavier = symbol("tag.heavier_tag"),
        main_fn = symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "u8 records");
}

fn compile_entry_wrapper(source: &str, label: &str) -> std::process::Output {
    let program = parse(source, Path::new("native-entry-wrapper.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-u8-wrapper-{}-{id}", std::process::id());
    let _ = label;
    let source_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable_path =
        std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, generated).unwrap();
    let compiled = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&source_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "u8 entry wrapper did not compile: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let executed = Command::new(&executable_path).output().unwrap();
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&executable_path);
    executed
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-u8-native-{label}-{}-{id}", std::process::id());
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
fn wasm_u8_scalar_programs_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    for (source, expected, label) in [
        (SCALARS, 7i64, "scalars"),
        (RECORDS, 4i64, "records"),
        (BOUNDS_WASM_SOURCE, 1i64, "bounds"),
    ] {
        let program = parse(source, Path::new("bytes-wasm.spx")).unwrap();
        let bytes = wasm::emit_module(&program).unwrap();
        assert_eq!(bytes, wasm::emit_module(&program).unwrap(), "{label}");
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-u8-wasm-{label}-{}-{id}", std::process::id());
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
console.log("u8-wasm-ok");
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
            "Node u8 {label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "u8-wasm-ok");
    }
}

const BOUNDS_WASM_SOURCE: &str = r#"
module test.u8_wasm_bounds;
@id("w.add")
fn add(left: u8, right: u8) -> u8 { left + right }
@id("app.main")
fn main() -> i64 {
    if add(200u8, 55u8) == 255u8 && add(0u8, 0u8) == 0u8 && add(128u8, 127u8) == 255u8
        && 4u8 / 2u8 == 2u8 && 9u8 - 1u8 == 8u8 && 250u8 > 100u8 && 100u8 <= 100u8 {
        1
    } else {
        0
    }
}
"#;

#[test]
fn wasm_u8_checked_arithmetic_traps_on_out_of_range_results() {
    if !command_available("node") {
        return;
    }
    for (body, label) in [
        ("if 255u8 + 1u8 == 0u8 { 1 } else { 2 }", "add overflow"),
        ("if 0u8 - 1u8 == 255u8 { 1 } else { 2 }", "sub underflow"),
        ("if 16u8 * 16u8 == 0u8 { 1 } else { 2 }", "mul overflow"),
        ("if 4u8 / 0u8 == 0u8 { 1 } else { 2 }", "division by zero"),
    ] {
        let source = format!(
            r#"
module test.u8_wasm_trap;
@id("app.main")
fn main() -> i64 {{ {body} }}
"#
        );
        let program = parse(&source, Path::new("u8-wasm-trap.spx")).unwrap();
        let bytes = wasm::emit_module(&program).unwrap();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-u8-wasm-trap-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
        std::fs::write(&wasm_path, bytes).unwrap();
        std::fs::write(
            &script_path,
            r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const bytes = await readFile(process.argv[2]);
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
let trapped = false;
try { instance.exports.semaprax_main(); } catch { trapped = true; }
if (!trapped) throw new Error("checked u8 failure did not trap");
console.log("u8-trap-ok");
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
            "Node u8 {label} trap probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "u8-trap-ok");
    }
}

#[test]
fn property_tests_analyze_uint8_literals_after_widening() {
    let source = r#"
module test.u8_property;
@id("t.identity")
fn identity(value: i64) -> i64 { let b = 2u8; value }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = std::env::temp_dir().join(format!(
        "semaprax-u8-property-{}-{id}.spx",
        std::process::id(),
        id = NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let report = semaprax::properties::generate(&path, &Default::default()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&report).unwrap();
    let entry = value["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "identity")
        .expect("identity function is analyzed");
    // Widening admitted the full Copy-scalar surface, so a `u8` literal in an
    // otherwise scalar body is now evaluated instead of deferred.
    assert_eq!(entry["outcome"], "analyzed");
    assert!(entry["discharged_cases"].as_u64().unwrap() > 0);
    let _ = std::fs::remove_file(&path);
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
