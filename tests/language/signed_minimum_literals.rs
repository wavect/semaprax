//! Executable evidence for Signed Minimum Literals v1.
//!
//! Proves that `-9223372036854775808` and `-2147483648i32` are admitted as
//! ordinary literals, that every other position keeps the stable `SPX-P003`
//! out-of-range rejection, that the canonical formatter and the semantic
//! graph agree with the parser, and that the reference interpreter, the
//! native C11 backend at O0/O2, and Node/Wasm agree on both the admitted
//! values and the normalized checked-arithmetic failures the minimum still
//! selects under `-MIN` and `MIN / -1`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::ast::{self, ExprKind};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, graph, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Every admitted boundary spelling for both signed widths: minimum,
/// minimum+1, maximum-1, and maximum, plus the minimum in a match pattern.
const BOUNDARIES: &str = r#"
module test.signed_minimum;

@id("i64.min")
fn i64_min() -> i64 { -9223372036854775808 }

@id("i64.min.spaced")
fn i64_min_spaced() -> i64 { - 9223372036854775808 }

@id("i64.min.successor")
fn i64_min_successor() -> i64 { -9223372036854775807 }

@id("i64.max.predecessor")
fn i64_max_predecessor() -> i64 { 9223372036854775806 }

@id("i64.max")
fn i64_max() -> i64 { 9223372036854775807 }

@id("i32.min")
fn i32_min() -> i32 { -2147483648i32 }

@id("i32.min.successor")
fn i32_min_successor() -> i32 { -2147483647i32 }

@id("i32.max.predecessor")
fn i32_max_predecessor() -> i32 { 2147483646i32 }

@id("i32.max")
fn i32_max() -> i32 { 2147483647i32 }

@id("i64.classify")
fn classify(value: i64) -> i64 {
    match value {
        -9223372036854775808 => 1,
        9223372036854775807 => 2,
        _ => 3,
    }
}

@id("app.main")
fn main() -> i64 {
    let low = i64_min();
    let low32 = i32_min();
    if low == i64_min_spaced() && low + 1 == i64_min_successor() && i64_max() - 1 == i64_max_predecessor() && low32 + 1i32 == i32_min_successor() && i32_max() - 1i32 == i32_max_predecessor() && classify(low) == 1 && classify(i64_max()) == 2 && classify(0) == 3 { 7 } else { 9 }
}
"#;

/// Checked arithmetic over the admitted minimum. Both functions take their
/// operand across a call boundary so no producer can fold the failure away.
const OVERFLOW: &str = r#"
module test.signed_minimum_overflow;

@id("m.negate")
fn negate(value: i64) -> i64 { -value }

@id("m.divide")
fn divide(value: i64, divisor: i64) -> i64 { value / divisor }

@id("m.negate.min")
fn negate_min() -> i64 { negate(-9223372036854775808) }

@id("m.divide.min")
fn divide_min() -> i64 { divide(-9223372036854775808, -1) }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn write_temp(source: &str) -> PathBuf {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-signed-minimum-{}-{ordinal}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn hex_symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::from("spx_decl_");
    for byte in id.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

/// The tail expression of one declared function body, by stable identity.
fn body_tail<'a>(program: &'a ast::Program, stable_id: &str) -> &'a ast::Expr {
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == stable_id)
        .unwrap_or_else(|| panic!("`{stable_id}` is declared"));
    match &function.body.kind {
        ExprKind::Block { tail, .. } => tail.as_ref(),
        _ => &function.body,
    }
}

/// Wraps one expression in a whole module so a rejection can be attributed to
/// the literal rather than to the surrounding shape.
fn expression_module(expression: &str) -> String {
    format!(
        "module test.signed_minimum_reject;\n@id(\"app.main\")\nfn main() -> i64 {{ {expression} }}\n"
    )
}

fn rejection(expression: &str) -> semaprax::diagnostic::Diagnostic {
    parse(&expression_module(expression), Path::new("minimum.spx"))
        .expect_err(&format!("`{expression}` must be rejected"))
}

// ---------------------------------------------------------------------------
// Admission, canonical round trip, and the semantic graph.
// ---------------------------------------------------------------------------

#[test]
fn signed_minimum_boundaries_round_trip_canonically_for_both_widths() {
    let program = parse(BOUNDARIES, Path::new("minimum.spx")).unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "{:?}",
        verify::verify(&program)
    );

    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("-9223372036854775808"),
        "the minimum keeps its ordinary spelling: {canonical}"
    );
    assert!(
        canonical.contains("-2147483648i32"),
        "the i32 minimum keeps its suffix: {canonical}"
    );
    assert!(
        !canonical.contains("- 9223372036854775808"),
        "the spaced form normalizes to the canonical one: {canonical}"
    );

    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
}

#[test]
fn the_semantic_graph_carries_the_exact_minimum_as_one_literal() {
    let program = parse(BOUNDARIES, Path::new("minimum.spx")).unwrap();
    let document = graph::to_json(&program).unwrap();
    assert!(
        document.contains("-9223372036854775808"),
        "the graph publishes the exact minimum"
    );
    assert!(
        document.contains("-2147483648"),
        "the graph publishes the exact i32 minimum"
    );

    // The minimum is a literal, not a negation node over an unrepresentable
    // magnitude, so no producer has to reconstruct the sign.
    assert!(
        matches!(body_tail(&program, "i64.min").kind, ExprKind::Int(i64::MIN)),
        "expected a folded i64 literal, found {:?}",
        body_tail(&program, "i64.min").kind
    );
    assert!(
        matches!(
            body_tail(&program, "i32.min").kind,
            ExprKind::Int32(i32::MIN)
        ),
        "expected a folded i32 literal, found {:?}",
        body_tail(&program, "i32.min").kind
    );
}

// ---------------------------------------------------------------------------
// Stable rejection everywhere the magnitude is not directly negated.
// ---------------------------------------------------------------------------

#[test]
fn out_of_range_magnitudes_keep_their_stable_located_rejection() {
    for (expression, message) in [
        // Positive MAX + 1 is still not a value of its type.
        ("9223372036854775808", "outside the i64 range"),
        ("2147483648i32", "outside the i32 range"),
        // A parenthesis is not trivia: the magnitude is then a positive
        // literal in its own right.
        ("-(9223372036854775808)", "outside the i64 range"),
        ("-(2147483648i32)", "outside the i32 range"),
        // Subtraction never retokenizes into a negated literal.
        ("1 - 9223372036854775808", "outside the i64 range"),
        // MIN - 1 overflows the magnitude itself, at the lexer.
        ("-9223372036854775809", "outside the i64 range"),
        ("-2147483649i32", "outside the i32 range"),
        // Argument and operand positions carry no special negation context.
        ("{ let value = 9223372036854775808; value }", "i64 range"),
    ] {
        let diagnostic = rejection(expression);
        assert_eq!(diagnostic.code, "SPX-P003", "{expression}: {diagnostic}");
        assert!(
            diagnostic.message.contains(message),
            "{expression}: {diagnostic}"
        );
        let span = diagnostic.span.expect("a located rejection");
        assert!(span.line >= 1 && span.column >= 1, "{expression}");
        assert!(span.end > span.start, "{expression}");
    }
}

#[test]
fn the_signed_minimum_magnitude_is_rejected_in_pattern_position() {
    let source = r#"
module test.signed_minimum_pattern;
@id("app.main")
fn main() -> i64 {
    match 0 {
        9223372036854775808 => 1,
        _ => 0,
    }
}
"#;
    let diagnostic = parse(source, Path::new("pattern.spx")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-P003", "{diagnostic}");
    assert!(
        diagnostic.message.contains("outside the i64 range"),
        "{diagnostic}"
    );
}

#[test]
fn a_line_break_between_the_sign_and_the_magnitude_is_trivia() {
    // Whitespace and comments are trivia, so the admitted form is a grammar
    // rule rather than a lexical one.
    let source = r#"
module test.signed_minimum_trivia;
@id("app.main")
fn main() -> i64 {
    -
    // the sign and its magnitude are one literal
    9223372036854775808
}
"#;
    let program = parse(source, Path::new("trivia.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert!(format::canonical(&program).contains("-9223372036854775808"));
}

// ---------------------------------------------------------------------------
// Backend agreement on admitted values.
// ---------------------------------------------------------------------------

#[test]
fn the_interpreter_returns_the_boundary_result() {
    let path = write_temp(BOUNDARIES);
    let interpretation =
        interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
            .expect("boundary module interprets");
    assert!(interpretation.returned, "{}", interpretation.envelope);
    assert!(
        interpretation
            .envelope
            .contains("\"kind\":\"returned\",\"type\":\"i64\",\"value\":\"7\""),
        "{}",
        interpretation.envelope
    );
    cleanup(&path);
}

#[test]
fn the_interpreter_selects_checked_overflow_for_negation_and_division() {
    let path = write_temp(OVERFLOW);
    for (symbol, code) in [("m.negate.min", 8u32), ("m.divide.min", 5)] {
        let interpretation =
            interpreter::interpret(&path, symbol, &[], &InterpreterOptions::default())
                .unwrap_or_else(|errors| panic!("{symbol}: {errors:?}"));
        assert!(!interpretation.returned, "{symbol} must fail closed");
        assert!(
            interpretation.envelope.contains(&format!(
                "\"kind\":\"failed\",\"status\":{{\"schema\":\"semaprax.status.v1\",\
\"domain_id\":\"semaprax.arithmetic.v1\",\"code\":{code},"
            )),
            "{symbol}: {}",
            interpretation.envelope
        );
    }
    cleanup(&path);
}

#[test]
fn native_c11_agrees_at_o0_and_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(BOUNDARIES, Path::new("minimum-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    // C has no negative literals, so the minimum must not be spelled as a
    // negated out-of-range magnitude.
    assert!(
        !generated.contains("INT64_C(9223372036854775808)"),
        "the C minimum must not name an unrepresentable magnitude"
    );
    assert!(
        !generated.contains("INT32_C(-2147483648)"),
        "the C i32 minimum must not name a wider type than int32_t"
    );

    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t wide = INT64_C(0);
    if ({min}(&context, &wide) != SPX_STATUS_SUCCESS || wide != INT64_MIN) return 11;
    if ({spaced}(&context, &wide) != SPX_STATUS_SUCCESS || wide != INT64_MIN) return 12;
    if ({successor}(&context, &wide) != SPX_STATUS_SUCCESS || wide != INT64_MIN + INT64_C(1)) return 13;
    if ({max}(&context, &wide) != SPX_STATUS_SUCCESS || wide != INT64_MAX) return 14;
    int32_t narrow = INT32_C(0);
    if ({min32}(&context, &narrow) != SPX_STATUS_SUCCESS || narrow != INT32_MIN) return 15;
    if ({max32}(&context, &narrow) != SPX_STATUS_SUCCESS || narrow != INT32_MAX) return 16;
    int64_t entry = INT64_C(0);
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(7)) return 17;
    return 0;
}}
"#,
        min = hex_symbol("i64.min"),
        spaced = hex_symbol("i64.min.spaced"),
        successor = hex_symbol("i64.min.successor"),
        max = hex_symbol("i64.max"),
        min32 = hex_symbol("i32.min"),
        max32 = hex_symbol("i32.max"),
        main_fn = hex_symbol("app.main"),
    );
    run_native_probe(&generated, &probe, "boundaries");
}

#[test]
fn native_c11_selects_checked_overflow_for_negation_and_division() {
    if !command_available("clang") {
        return;
    }
    let program = parse(OVERFLOW, Path::new("minimum-overflow-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(64)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(64), NULL, NULL, NULL)) return 10;
    int64_t sink = INT64_C(0);
    if ({negate}(&context, &sink) == SPX_STATUS_SUCCESS) return 21;
    if ({divide}(&context, &sink) == SPX_STATUS_SUCCESS) return 22;
    return 0;
}}
"#,
        negate = hex_symbol("m.negate.min"),
        divide = hex_symbol("m.divide.min"),
    );
    run_native_probe(&generated, &probe, "overflow");
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-signed-minimum-{label}-{}-{id}",
            std::process::id()
        );
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
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "{label} failed at {optimization}: status={status:?}"
        );
    }
}

#[test]
fn wasm_agrees_with_the_other_backends_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(BOUNDARIES, Path::new("minimum-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-signed-minimum-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
// The checked i64 host shim, mirroring the compiler's own reference imports:
// every boundary operand must survive without wrapping.
const MIN = -(1n << 63n);
const MAX = (1n << 63n) - 1n;
const checked = (value) => {
  if (value < MIN || value > MAX) throw new Error(`arithmetic overflow ${value}`);
  return value;
};
const bytes = await readFile(process.argv[2]);
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => checked(a + b),
  spx_sub: (a, b) => checked(a - b),
  spx_mul: (a, b) => checked(a * b),
  spx_div: (a, b) => { if (b === 0n) throw new Error("division by zero"); return checked(a / b); },
  spx_rem: (a, b) => { if (b === 0n) throw new Error("remainder by zero"); return a % b; },
  spx_neg: (value) => checked(-value),
  spx_contract_fail: (code) => { throw new Error(`contract ${code}`); },
} });
const observed = instance.exports.semaprax_main();
if (observed !== 7n) throw new Error(`result mismatch ${observed}`);
console.log("signed-minimum-wasm-ok");
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
        "Node signed-minimum run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "signed-minimum-wasm-ok"
    );
}

#[test]
fn wasm_negation_of_the_minimum_fails_closed_instead_of_wrapping() {
    if !command_available("node") {
        return;
    }
    let source = r#"
module test.signed_minimum_wasm_overflow;

@id("m.negate")
fn negate(value: i64) -> i64 { -value }

@id("app.main")
fn main() -> i64 { negate(-9223372036854775808) }
"#;
    let program = parse(source, Path::new("minimum-wasm-overflow.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-signed-minimum-wasm-overflow-{}-{id}",
        std::process::id()
    );
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
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"),
  spx_neg: (value) => { const folded = -value; if (folded > 0x7fffffffffffffffn) { throw new Error("negation overflow"); } return folded; },
  spx_contract_fail: fail("spx_contract_fail"),
} });
let failed = false;
try {
  instance.exports.semaprax_main();
} catch (error) {
  failed = true;
}
if (!failed) throw new Error("negating the minimum must not wrap silently");
console.log("signed-minimum-wasm-overflow-ok");
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
        "Node signed-minimum overflow run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "signed-minimum-wasm-overflow-ok"
    );
}
