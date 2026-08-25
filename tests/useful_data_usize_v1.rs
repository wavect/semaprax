use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{self, ResolvedExprKind, ResolvedType};
use semaprax::interpreter::{self, ArgumentValue, InterpreterOptions};
use semaprax::{codegen, format, graph, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

const SOURCE: &str = r#"
module test.useful_data_usize;

@id("usize.mix")
fn mix(left: usize, right: usize) -> usize {
    let mut value = left + right;
    value = value * 3usize;
    value = value / 2usize;
    value % 11usize
}

@id("usize.max")
fn max_value() -> usize { 18446744073709551615usize }

@id("usize.ordered")
fn ordered(left: usize, right: usize) -> bool { left < right }

@id("usize.add")
fn checked_add(left: usize, right: usize) -> usize { left + right }

@id("usize.sub")
fn checked_sub(left: usize, right: usize) -> usize { left - right }

@id("usize.mul")
fn checked_mul(left: usize, right: usize) -> usize { left * right }

@id("usize.div")
fn checked_div(left: usize, right: usize) -> usize { left / right }

@id("usize.rem")
fn checked_rem(left: usize, right: usize) -> usize { left % right }

@id("app.main")
fn main() -> i64 { if ordered(0usize, max_value()) { 1 } else { 0 } }
"#;

fn expressions(root: &hir::ResolvedExpr) -> Vec<&hir::ResolvedExpr> {
    let mut pending = vec![root];
    let mut out = Vec::new();
    while let Some(expression) = pending.pop() {
        out.push(expression);
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => pending.extend(args),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(&call.args),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.extend(statements.iter().map(|statement| statement.value()));
                pending.push(tail);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.extend([
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref(),
                ]);
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                pending.push(scrutinee);
                pending.extend(arms.iter().filter_map(|arm| arm.guard.as_deref()));
                pending.extend(arms.iter().map(|arm| &arm.value));
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    out
}

#[test]
fn usize_round_trips_and_resolves_as_exact_u64() {
    let program = parse(SOURCE, Path::new("usize.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("18446744073709551615usize"));
    assert_eq!(
        format::canonical(&parse(&canonical, "canonical.spx").unwrap()),
        canonical
    );

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let maximum = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "usize.max")
        .unwrap();
    assert_eq!(maximum.return_type, ResolvedType::Usize);
    assert!(expressions(&maximum.body).iter().any(|expression| {
        matches!(expression.kind, ResolvedExprKind::Usize(u64::MAX))
            && expression.ty == ResolvedType::Usize
    }));
    let facts = resolved
        .declarations
        .type_facts(&ResolvedType::Usize)
        .unwrap();
    assert!(facts.copy);
    assert!(!facts.needs_drop);
}

#[test]
fn malformed_out_of_range_and_negative_usize_literals_are_t260() {
    for source in [
        SOURCE.replace("0usize", "18446744073709551616usize"),
        SOURCE.replace("0usize", "-1usize"),
    ] {
        let error = parse(&source, "bad-usize.spx").unwrap_err();
        assert_eq!(error.code, "SPX-T260");
    }
}

#[test]
fn interpreter_uses_unsigned_u64_arithmetic_and_ordering() {
    assert_eq!(
        interpreter::parse_argument("18446744073709551615usize").unwrap(),
        ArgumentValue::Usize(u64::MAX)
    );
    assert!(interpreter::parse_argument("-1usize").is_err());

    let path = std::env::temp_dir().join(format!("semaprax-usize-{}.spx", std::process::id()));
    std::fs::write(&path, SOURCE).unwrap();
    let result = interpreter::interpret(
        &path,
        "usize.mix",
        &["7usize".to_owned(), "5usize".to_owned()],
        &InterpreterOptions::default(),
    )
    .unwrap();
    assert!(result.returned);
    assert!(result.envelope.contains("\"type\":\"usize\""));
    assert!(result.envelope.contains("\"value\":\"7usize\""));
    interpreter::verify_envelope(&result.envelope).unwrap();
    let _ = std::fs::remove_file(path);
}

#[test]
fn usize_interpreter_selects_every_admitted_failure_status() {
    let path = std::env::temp_dir().join(format!(
        "semaprax-usize-failures-{}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    let cases = [
        ("usize.add", ["18446744073709551615usize", "1usize"], 1),
        ("usize.sub", ["0usize", "1usize"], 2),
        ("usize.mul", ["18446744073709551615usize", "2usize"], 3),
        ("usize.div", ["1usize", "0usize"], 4),
        ("usize.rem", ["1usize", "0usize"], 6),
    ];
    for (function, arguments, code) in cases {
        let result = interpreter::interpret(
            &path,
            function,
            &arguments.map(str::to_owned),
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!result.returned, "{function} unexpectedly returned");
        assert!(result
            .envelope
            .contains("\"domain_id\":\"semaprax.arithmetic.v1\""));
        assert!(
            result.envelope.contains(&format!("\"code\":{code}")),
            "{function}: {}",
            result.envelope
        );
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn usize_selects_graph_v17_without_widening_legacy_schema() {
    let usize_graph = graph::to_json(&parse(SOURCE, "usize-graph.spx").unwrap()).unwrap();
    assert!(usize_graph.contains("\"schema\":\"semaprax.graph.v17\""));
    assert!(usize_graph.contains("\"kind\":\"usize\""));
    assert!(usize_graph.contains("\"name\":\"usize\""));
    assert!(usize_graph.contains("18446744073709551615"));

    let legacy = parse(
        "module legacy;\n@id(\"app.main\")\nfn main() -> i64 { 1 }\n",
        "legacy.spx",
    )
    .unwrap();
    let legacy_graph = graph::to_json(&legacy).unwrap();
    assert!(legacy_graph.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(!legacy_graph.contains("graph.v17"));
}

#[test]
fn usize_native_o0_o2_and_wasm_execute_the_same_program() {
    let program = parse(SOURCE, "usize-backends.spx").unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert!(generated.contains("uint64_t"));
    assert!(generated.contains("spx_rt_usize_add"));
    let failure_probe = format!(
        r#"
typedef spx_status_token (*usize_binary)(struct spx_context *, uint64_t, uint64_t, uint64_t *);
static int expect_failure(usize_binary operation, uint64_t left, uint64_t right, uint32_t code) {{
    struct spx_status_entry entries[UINT32_C(4)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(31), entries, UINT32_C(4), NULL, NULL, NULL)) return 10;
    uint64_t result = UINT64_C(0xfeedfacefeedface);
    spx_status_token token = operation(&context, left, right, &result);
    if (token == SPX_STATUS_SUCCESS || result != UINT64_C(0xfeedfacefeedface)) return 11;
    const struct spx_normalized_status *status = spx_status_resolve(&context, token);
    if (status == NULL || strcmp(status->domain_id, "semaprax.arithmetic.v1") != 0) return 12;
    if (status->code != code || status->status_class != SPX_STATUS_CLASS_ARITHMETIC) {{
        fprintf(stderr, "expected code %u, received %u class %u\n", code, status->code, (uint32_t)status->status_class);
        return 13;
    }}
    return 0;
}}
int main(void) {{
    int result = 0;
    if ((result = expect_failure({add}, UINT64_MAX, UINT64_C(1), UINT32_C(1))) != 0) return result;
    if ((result = expect_failure({sub}, UINT64_C(0), UINT64_C(1), UINT32_C(2))) != 0) return result;
    if ((result = expect_failure({mul}, UINT64_MAX, UINT64_C(2), UINT32_C(3))) != 0) return result;
    if ((result = expect_failure({div}, UINT64_C(1), UINT64_C(0), UINT32_C(4))) != 0) return result;
    if ((result = expect_failure({rem}, UINT64_C(1), UINT64_C(0), UINT32_C(6))) != 0) return result;
    return 0;
}}
"#,
        add = symbol("usize.add"),
        sub = symbol("usize.sub"),
        mul = symbol("usize.mul"),
        div = symbol("usize.div"),
        rem = symbol("usize.rem"),
    );
    if Command::new("clang").arg("--version").output().is_ok() {
        for optimization in ["-O0", "-O2"] {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let stem = format!("semaprax-usize-{}-{id}", std::process::id());
            let source_path = std::env::temp_dir().join(format!("{stem}.c"));
            let executable_path =
                std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
            std::fs::write(&source_path, &generated).unwrap();
            let compiled = Command::new("clang")
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                .arg(&source_path)
                .arg("-o")
                .arg(&executable_path)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let output = Command::new(&executable_path).output().unwrap();
            let _ = std::fs::remove_file(source_path);
            let _ = std::fs::remove_file(executable_path);
            assert!(output.status.success());
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");

            let failure_source = std::env::temp_dir().join(format!("{stem}-failures.c"));
            let failure_executable = std::env::temp_dir()
                .join(format!("{stem}-failures{}", std::env::consts::EXE_SUFFIX));
            std::fs::write(&failure_source, format!("{generated}\n{failure_probe}")).unwrap();
            let compiled = Command::new("clang")
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    optimization,
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&failure_source)
                .arg("-o")
                .arg(&failure_executable)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let failures = Command::new(&failure_executable).output().unwrap();
            let _ = std::fs::remove_file(failure_source);
            let _ = std::fs::remove_file(failure_executable);
            assert!(
                failures.status.success(),
                "native usize failures: {failures:?}"
            );
        }
    }

    if Command::new("node").arg("--version").output().is_ok() {
        let bytes = wasm::emit_module(&program).unwrap();
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-usize-wasm-{}-{id}", std::process::id());
        let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
        std::fs::write(&wasm_path, bytes).unwrap();
        let script = r#"
const fs = require('fs');
const bytes = fs.readFileSync(process.argv[1]);
WebAssembly.instantiate(bytes, { env: {
  spx_add(){ throw new Error('unexpected spx_add'); },
  spx_sub(){ throw new Error('unexpected spx_sub'); },
  spx_mul(){ throw new Error('unexpected spx_mul'); },
  spx_div(){ throw new Error('unexpected spx_div'); },
  spx_rem(){ throw new Error('unexpected spx_rem'); },
  spx_neg(){ throw new Error('unexpected spx_neg'); },
  spx_contract_fail(){ throw new Error('unexpected contract failure'); }
}}).then(({instance}) => {
  if (instance.exports.semaprax_main() !== 1n) process.exit(2);
}).catch(error => { console.error(error); process.exit(3); });
"#;
        let output = Command::new("node")
            .arg("-e")
            .arg(script)
            .arg(&wasm_path)
            .output()
            .unwrap();
        let _ = std::fs::remove_file(wasm_path);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
