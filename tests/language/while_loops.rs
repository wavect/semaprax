//! Executable evidence for Bounded While-Loops v1.
//!
//! Proves that statement-level `while` loops over the admitted Copy-scalar
//! profile produce identical observable results on native C11 O0/O2 and
//! Node/Wasm, that condition-dependent checked-arithmetic failures select the
//! exact same normalized status on every backend including the reference
//! interpreter, that fuel exhaustion fails closed, and that every new
//! diagnostic (SPX-T251/T252/T253) plus the additive Graph-v15 selection are
//! stable. Programs without while syntax must stay byte-identical.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions, DEFAULT_MAX_STEPS};
use semaprax::{codegen, format, graph, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const CORPUS: &str = r#"
module test.while_loops_v1;

@id("while.count_sum")
fn count_sum(limit: i64) -> i64 {
    let mut counter = 0;
    let mut total = 0;
    while counter < limit {
        counter = counter + 1;
        total = total + counter;
        counter < limit
    }
    total
}

@id("while.digit_sum")
fn digit_sum(value: i64) -> i64 {
    let mut remaining = value;
    let mut total = 0;
    while remaining > 0 {
        total = total + remaining % 10;
        remaining = remaining / 10;
        remaining > 0
    }
    total
}

@id("while.nested")
fn nested(width: i64, height: i64) -> i64 {
    let mut row = 0;
    let mut total = 0;
    while row < height {
        let mut column = 0;
        while column < width {
            column = column + 1;
            total = total + row * width + column;
            column < width
        }
        row = row + 1;
        row < height
    }
    total
}

@id("while.in_if")
fn in_if(flag: bool, limit: i64) -> i64 {
    let mut total = 0;
    if flag {
        let mut counter = 0;
        while counter < limit {
            counter = counter + 1;
            total = total + 2;
            counter < limit
        }
        total
    } else {
        total - 7
    }
}

@id("while.zero_iterations")
fn zero_iterations(flag: bool) -> i64 {
    let mut total = 3;
    while flag {
        total = total + 100;
        flag
    }
    if total == 3 { 30 } else { total }
}

@id("while.div_fails_on_iteration")
fn div_fails(start: i64) -> i64 {
    let mut n = start;
    let mut total = 0;
    while n >= 0 {
        let quotient = 6 / n;
        total = total + quotient;
        n = n - 1;
        n >= 0
    }
    total
}

@id("main")
fn main() -> i64 { count_sum(4) + digit_sum(98765) }
"#;

const PLAIN_STABLE: &str = r#"
module test.mutation_plain;

@id("plain.stable")
fn stable() -> i64 {
    let total = 1;
    let frozen = 3;
    total + frozen
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn write_corpus(stem: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-while-{}-{stem}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join("corpus.spx");
    std::fs::write(&path, CORPUS).expect("corpus source");
    path
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn verify_diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("while-diag.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn while_programs_round_trip_canonically_and_keep_revisions_stable() {
    let program = parse(CORPUS, Path::new("roundtrip.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("while remaining > 0 {"),
        "loops render in canonical header-inline form: {canonical}"
    );
    assert!(canonical.contains("while column < width {"));
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn graph_serialization_is_deterministic_and_selects_v15_only_with_while_nodes() {
    let program = parse(CORPUS, Path::new("graph.spx")).unwrap();
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);
    assert!(first.contains("\"kind\":\"while\""), "{first}");
    let wire = serde_json::from_str::<serde_json::Value>(&first).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v15");

    // Programs without while syntax keep their exact previous lattice entry.
    let plain = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let plain_json = graph::to_json(&plain).unwrap();
    assert!(!plain_json.contains("\"kind\":\"while\""));
    let wire = serde_json::from_str::<serde_json::Value>(&plain_json).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v10");
}

#[test]
fn non_while_graph_bytes_are_pinned_to_pre_feature_output() {
    // This digest was captured from a build without Bounded While-Loops v1
    // (shared with Explicit Mutation v1's pin) and must never drift.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(!json.contains("\"kind\":\"while\""));
    use sha2::{Digest, Sha256};
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(json.as_bytes()))
    );
    assert_eq!(
        digest,
        "sha256:6fe42635e96022507876aabd25acfe06f28521aba50132a5dc16b5070c45cfa7"
    );
}

#[test]
fn admitted_loops_add_no_cleanup_structure_and_replay_exactly() {
    let program = parse(
        r#"
module test.while_cleanup;

@id("clean.loopy")
fn loopy(limit: i64) -> i64 {
    let mut counter = 0;
    let mut total = 0;
    while counter < limit {
        counter = counter + 1;
        total = total + counter * 2;
        counter < limit
    }
    total
}

@id("main")
fn main() -> i64 { 0 }
"#,
        Path::new("cleanup.spx"),
    )
    .unwrap();
    // `hir::resolve` validates ordinary HIR, rebuilds every CleanupPlan, and
    // exact-compares it against the independent replay gate; success here is
    // itself the replay evidence.
    let resolved = hir::resolve(&program).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "clean.loopy")
        .expect("resolved loop function");
    // Copy-scalar loops own nothing: no slots and no finalizers anywhere.
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());
    assert!(
        function
            .cleanup_plan
            .exits
            .iter()
            .all(|exit| exit.finalize_in_order.is_empty()),
        "scalar loops never finalize"
    );
}

#[test]
fn non_bool_while_condition_is_spx_t251() {
    let report = verify_diagnostics(
        r#"
module test.while_nonbool;
@id("app.main")
fn main() -> i64 {
    let mut count = 0;
    while count {
        count = count + 1;
        true
    }
    0
}
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-T251")
        .expect("non-bool conditions are rejected");
    assert!(diagnostic
        .message
        .contains("`while` condition must be bool"));
}

#[test]
fn record_content_inside_loops_is_spx_t252() {
    let report = verify_diagnostics(
        r#"
module test.while_record;
record Point {
    x: i64,
}
@id("app.main")
fn main() -> i64 {
    let mut count = 0;
    while count < 3 {
        let point = Point { x: count };
        count = count + point.x;
        count < 3
    }
    0
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-T252"),
        "record construction stays outside the loop slice: {report:?}"
    );
}

#[test]
fn string_conditions_and_unsafe_inside_loops_are_spx_t252() {
    // Computed String conditions are admitted; consuming an enclosing owner
    // and an unsafe boundary remain outside the loop condition profile.
    let sources = [
        (
            "string condition",
            r#"
module test.while_string;
@id("app.main")
fn main() -> i64 {
    let text = "x";
    let mut count = 0;
    while count < string_len(string_concat(text, "x")) {
        count = count + 1;
        count < 2
    }
    0
}
"#,
        ),
        (
            "unsafe boundary",
            r#"
module test.while_unsafe;
permit { unsafe }
@id("app.main")
fn main() -> i64 {
    while false {
        @audit("discarded") unsafe { 0 }
        false
    }
    0
}
"#,
        ),
    ];
    for (label, source) in sources {
        let report = verify_diagnostics(source);
        assert!(
            report.iter().any(|item| item.code == "SPX-T252"),
            "{label} inside loops is rejected: {report:?}"
        );
    }
}

#[test]
fn nonscalar_calls_inside_loops_are_spx_t252() {
    let report = verify_diagnostics(
        r#"
module test.while_call;
record Token {
    weight: i64,
}

@id("call.consume")
fn consume(token: own Token) -> i64 { token.weight }
@id("app.main")
fn main() -> i64 {
    let token = Token { weight: 1 };
    let mut count = 0;
    while count < 3 {
        count = count + consume(token);
        count < 3
    }
    0
}
"#,
    );
    assert!(
        report
            .iter()
            .any(|item| item.code == "SPX-T252" && item.message.contains("`consume`")),
        "own-parameter calls stay outside loops: {report:?}"
    );
}

#[test]
fn borrowed_copy_scalar_vec_helpers_are_admitted_inside_loops() {
    for (ty, value) in [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'x'"),
        ("f32", "7.0f32"),
        ("f64", "7.0"),
        ("bool", "true"),
    ] {
        let source = format!(
            r#"module test.while_vec_call;
@id("call.length")
fn length(values: borrow Vec<{ty}>) -> usize {{ vec_len<{ty}>(values) }}
@id("app.main")
fn main() -> i64 {{
    let values = vec_push<{ty}>(vec_with_capacity<{ty}>(1usize), {value});
    let mut iterations = 0usize;
    let mut observed = 0usize;
    while iterations < 1usize {{
        observed = length(values);
        iterations = iterations + 1usize;
        0
    }}
    i64_from_usize(observed)
}}
"#
        );
        let checked = semaprax::check(&source, "while-copy-vec-call.spx")
            .unwrap_or_else(|errors| panic!("{ty}: {errors:?}"));
        let resolved = hir::resolve(&checked).unwrap_or_else(|error| panic!("{ty}: {error:?}"));
        hir::validate(&resolved).unwrap_or_else(|error| panic!("{ty}: {error:?}"));
        if ty == "i64" {
            let mut hostile = resolved.clone();
            let main = hostile
                .functions
                .iter_mut()
                .find(|function| function.id.as_str() == "app.main")
                .unwrap();
            let hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
                panic!("Vec loop main remains a block")
            };
            let body = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    hir::ResolvedStatement::While { body, .. } => Some(body),
                    _ => None,
                })
                .expect("Vec loop remains present");
            let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
                panic!("Vec loop body remains a block")
            };
            let argument = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    hir::ResolvedStatement::Assign { value, .. } => match &mut value.kind {
                        hir::ResolvedExprKind::Call { args, .. } => args.first_mut(),
                        _ => None,
                    },
                    _ => None,
                })
                .expect("borrowed Vec helper call retains its argument");
            argument.ownership = hir::OwnershipMode::Value;
            assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
        }
    }
}

#[test]
fn borrowed_copy_scalar_vec_helpers_keep_owned_noncopy_and_vec_result_shapes_closed() {
    for (label, declaration, setup, call) in [
        (
            "owned Vec parameter",
            "fn inspect(values: own Vec<i64>) -> usize { vec_len<i64>(values) }",
            "let values = vec_push<i64>(vec_with_capacity<i64>(1usize), 7);",
            "inspect(values)",
        ),
        (
            "non-Copy Vec parameter",
            "fn inspect(values: borrow Vec<Bytes>) -> usize { vec_len<Bytes>(values) }",
            "let values = vec_push<Bytes>(vec_with_capacity<Bytes>(1usize), bytes_zeroed(1usize));",
            "inspect(values)",
        ),
        (
            "Vec result",
            "fn inspect() -> Vec<i64> { vec_with_capacity<i64>(1usize) }",
            "",
            "inspect()",
        ),
    ] {
        let source = format!(
            r#"module test.while_vec_refusal;
@id("call.inspect") {declaration}
@id("app.main")
fn main() -> i64 {{
    {setup}
    let mut iterations = 0usize;
    while iterations < 1usize {{
        let rejected = {call};
        iterations = iterations + 1usize;
        0
    }}
    0
}}
"#
        );
        let report = verify_diagnostics(&source);
        assert!(
            report
                .iter()
                .any(|item| item.code == "SPX-T252" && item.message.contains("`inspect`")),
            "{label} stays outside loop calls: {report:?}"
        );
    }
}

#[test]
fn record_owner_renewal_replays_every_copy_argument_at_both_trust_boundaries() {
    let rejected = r#"
module test.while_record_renewal_argument;
@id("cursor.type") record Cursor {
    @id("cursor.data") data: Bytes,
    @id("cursor.position") position: usize,
}
@id("cursor.renew")
fn renew(value: own Cursor, amount: i64) -> Cursor { value }
@id("cursor.identity")
fn identity<T>(value: T) -> T { value }
@id("app.main")
fn main() -> i64 {
    let mut cursor = Cursor { data: bytes_zeroed(0usize), position: 0usize };
    let mut count = 0;
    while count < 1 {
        cursor = renew(cursor, identity<i64>(count));
        count = count + 1;
        count < 1
    }
    0
}
"#;
    let report = verify_diagnostics(rejected);
    assert!(
        report
            .iter()
            .any(|item| item.code == "SPX-T252" && item.message.contains("generic calls")),
        "renewal must not hide a disallowed nested argument: {report:?}"
    );

    let admitted = rejected.replace("identity<i64>(count)", "count");
    let parsed = semaprax::check(&admitted, "while-record-renewal-argument.spx").unwrap();
    let mut hostile = hir::resolve(&parsed).unwrap();
    hir::validate(&hostile).unwrap();
    let main = hostile
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
        panic!("renewal witness main must remain a block")
    };
    let while_body = statements
        .iter_mut()
        .find_map(|statement| match statement {
            hir::ResolvedStatement::While { body, .. } => Some(body),
            _ => None,
        })
        .expect("renewal witness while statement");
    let hir::ResolvedExprKind::Block { statements, .. } = &mut while_body.kind else {
        panic!("renewal witness while body must remain a block")
    };
    let argument = statements
        .iter_mut()
        .find_map(|statement| match statement {
            hir::ResolvedStatement::Assign { value, .. } => match &mut value.kind {
                hir::ResolvedExprKind::Call { args, .. } => args.get_mut(1),
                _ => None,
            },
            _ => None,
        })
        .expect("renewal witness Copy argument");
    argument.kind = hir::ResolvedExprKind::ArrayU8(vec![1]);
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    assert_eq!(
        interpreter::evaluate_resolved_owned_data(&hostile, "app.main", &[], 10_000).unwrap_err()
            [0]
        .code,
        "SPX-H006"
    );
    assert_eq!(codegen::emit_hir_c(&hostile).unwrap_err().code, "SPX-H006");
    assert_eq!(
        semaprax::wasm::emit_resolved_module(&hostile)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}

#[test]
fn record_owner_renewal_cannot_replace_storage_with_a_live_projected_borrow() {
    let source = r#"
module test.while_record_renewal_live_borrow;
@id("cursor.type") record Cursor {
    @id("cursor.data") data: Bytes,
    @id("cursor.position") position: usize,
}
@id("cursor.renew")
fn renew(value: own Cursor, amount: usize) -> Cursor { value }
@id("app.main")
fn main() -> i64 {
    let mut cursor = Cursor { data: bytes_zeroed(1usize), position: 0usize };
    let view = bytes_as_slice(cursor.data);
    let mut count = 0usize;
    while count < 1usize {
        cursor = renew(cursor, count);
        count = count + 1usize;
        count < 1usize
    }
    if byte_len(view) == 1usize { 0 } else { 1 }
}
"#;
    let report = verify_diagnostics(source);
    assert_eq!(
        report
            .iter()
            .map(|item| (item.code, item.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                "SPX-T265",
                "move or call transfer would invalidate a lexical byte view"
            ),
            (
                "SPX-T265",
                "assignment would replace storage held by a lexical byte view"
            ),
        ],
        "renewal must refuse both transfer and replacement while its projected borrow remains live"
    );
}

#[test]
fn while_in_contract_expression_is_spx_t253() {
    let report = verify_diagnostics(
        r#"
module test.while_contract;
@id("c.check")
fn check(value: i64) -> i64
requires {{
    let mut count = 0;
    while count < value {
        count = count + 1;
        count < value
    }
    count > 0
}}
{ value }
@id("app.main")
fn main() -> i64 { check(1) }
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-T253"),
        "contract expressions remain loop-free: {report:?}"
    );
}

#[test]
fn native_loops_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(CORPUS, Path::new("native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(96), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t out = 0;
    if ({count_sum}(&context, INT64_C(4), &out) != SPX_STATUS_SUCCESS || out != INT64_C(10)) return 11;
    if ({digit_sum}(&context, INT64_C(98765), &out) != SPX_STATUS_SUCCESS || out != INT64_C(35)) return 12;
    if ({nested}(&context, INT64_C(3), INT64_C(2), &out) != SPX_STATUS_SUCCESS || out != INT64_C(21)) return 13;
    if ({in_if}(&context, UINT8_C(1), INT64_C(5), &out) != SPX_STATUS_SUCCESS || out != INT64_C(10)) return 14;
    if ({in_if}(&context, UINT8_C(0), INT64_C(5), &out) != SPX_STATUS_SUCCESS || out != INT64_C(-7)) return 15;
    if ({zero_iterations}(&context, UINT8_C(0), &out) != SPX_STATUS_SUCCESS || out != INT64_C(30)) return 16;
    if ({main_fn}(&context, &out) != SPX_STATUS_SUCCESS || out != INT64_C(45)) return 17;
    return 0;
}}
"#,
        count_sum = symbol("while.count_sum"),
        digit_sum = symbol("while.digit_sum"),
        nested = symbol("while.nested"),
        in_if = symbol("while.in_if"),
        zero_iterations = symbol("while.zero_iterations"),
        main_fn = symbol("main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-while-native-{}-{id}", std::process::id());
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
            "native C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let stderr = String::from_utf8_lossy(&executed.stderr).into_owned();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "native failed at {optimization}: {:?} {stderr}",
            executed.status.code()
        );
    }
}

/// The failure probe resolves the selected normalized status directly through
/// the compiler-owned runtime helpers and prints its exact arithmetic code.
const NATIVE_FAILURE_PROBE: &str = r#"
#include <stdio.h>
#include <string.h>

static spx_status_token PROBE_SYMBOL(
    struct spx_context *spx_ctx,
    int64_t spx_start,
    int64_t *spx_result_out
);

__attribute__((unused)) static int probe_main(void) {
    static struct spx_status_entry entries[UINT32_C(32)];
    static struct spx_context context;
    memset(&context, 0, sizeof(context));
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 60;
    int64_t out = 0;
    uint32_t spx_status = PROBE_SYMBOL(&context, INT64_C(3), &out);
    if (spx_status == SPX_STATUS_SUCCESS) return 61;
    const struct spx_normalized_status *status = spx_status_resolve(&context, spx_status);
    if (status == NULL) return 62;
    if (status->status_class != SPX_STATUS_CLASS_ARITHMETIC) return 63;
    if (status->code != UINT32_C(4)) return 64;
    if (strcmp(status->domain_id, "semaprax.arithmetic.v1") != 0) return 65;
    printf("%s/%u\n", status->domain_id, status->code);
    return 66;
}

int main(void) { return probe_main(); }
"#;

#[test]
fn native_condition_dependent_division_by_zero_selects_exact_status_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    // Point `main` at the failing loop in this temporary copy of the corpus
    // so the loop function is part of the entry closure and gets defined.
    let failing = CORPUS.replace(
        "@id(\"main\")\nfn main() -> i64 { count_sum(4) + digit_sum(98765) }",
        "@id(\"main\")\nfn main() -> i64 { div_fails(3) }",
    );
    let source_path = Path::new("native-fail.spx");
    let program = parse(&failing, source_path).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    // start=3 performs three good iterations then divides by zero on the next
    // pass; the normalized arithmetic failure must surface identically at
    // both optimization levels.
    let probe = NATIVE_FAILURE_PROBE.replace(
        "PROBE_SYMBOL",
        &format!("spx_decl_{}", hex_identity("while.div_fails_on_iteration")),
    );
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-while-native-fail-{}-{id}", std::process::id());
        let source_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source_path, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "failure-probe C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let stdout = String::from_utf8_lossy(&executed.stdout).into_owned();
        let _ = std::fs::remove_file(&source_path);
        let _ = std::fs::remove_file(&executable);
        assert_eq!(
            executed.status.code(),
            Some(66),
            "exact normalized status probe at {optimization}: {}",
            String::from_utf8_lossy(&executed.stderr)
        );
        assert!(
            stdout.contains("semaprax.arithmetic.v1/4"),
            "division-by-zero selects code 4 in semaprax.arithmetic.v1 at {optimization}: {stdout}"
        );
    }
}

const NODE_LOOP_RUNNER: &str = r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const expected = BigInt(process.argv[3]);
const SPX_MIN = -(2n ** 63n), SPX_MAX = 2n ** 63n - 1n;
const bounded = (value, what) => {
  if (value < SPX_MIN || value > SPX_MAX) throw new RangeError(`checked ${what} failure`);
  return value;
};
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => bounded(a + b, "addition"),
  spx_sub: (a, b) => bounded(a - b, "subtraction"),
  spx_mul: (a, b) => bounded(a * b, "multiplication"),
  spx_div: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("division"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("remainder"); return a % b; },
  spx_neg: (a) => bounded(-a, "negation"),
  spx_contract_fail: fail("spx_contract_fail"),
}});
for (let index = 0; index < 1024; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("while-wasm-ok");
"#;

#[test]
fn wasm_loops_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let corpus_path = write_corpus("wasm");
    let corpus_source = std::fs::read_to_string(&corpus_path).unwrap();
    let program = parse(&corpus_source, &corpus_path).expect("parse corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, semaprax::wasm::emit_module(&program).unwrap());

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-while-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(&script_path, NODE_LOOP_RUNNER).unwrap();
    // main() = count_sum(4) + digit_sum(98765) = 10 + 35 = 45, matching the
    // native probe exactly.
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg("45")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node while leg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "while-wasm-ok"
    );
}

#[test]
fn wasm_condition_dependent_division_failure_surfaces_in_node() {
    if !command_available("node") {
        return;
    }
    let corpus_path = write_corpus("wasm-fail");
    let corpus_source = std::fs::read_to_string(&corpus_path).unwrap();
    let program = parse(&corpus_source, &corpus_path).expect("parse corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-while-wasm-div-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const SPX_MIN = -(2n ** 63n), SPX_MAX = 2n ** 63n - 1n;
const bounded = (value, what) => {
  if (value < SPX_MIN || value > SPX_MAX) throw new RangeError(`checked ${what} failure`);
  return value;
};
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => bounded(a + b, "addition"),
  spx_sub: (a, b) => bounded(a - b, "subtraction"),
  spx_mul: (a, b) => bounded(a * b, "multiplication"),
  spx_div: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("division by zero"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("remainder"); return a % b; },
  spx_neg: (a) => bounded(-a, "negation"),
  spx_contract_fail: () => { throw new Error("SEMAPRAX contract failure"); },
}});
// The loop runs three good iterations and then divides by zero inside the
// body; the host import throws and no wrapped result may exist.
let threw = false;
try {
  instance.exports.semaprax_main();
} catch (error) {
  threw = true;
  if (!String(error).includes("division")) throw error;
}
if (!threw) throw new Error("loop-carried division by zero must not wrap");
console.log("while-wasm-div-ok");
"#,
    )
    .unwrap();
    // Point `main` at the failing loop by rewriting only this temporary copy
    // of the corpus so the exported entry runs div_fails(3).
    let failing = CORPUS.replace(
        "@id(\"main\")\nfn main() -> i64 { count_sum(4) + digit_sum(98765) }",
        "@id(\"main\")\nfn main() -> i64 { div_fails(3) }",
    );
    std::fs::write(&corpus_path, &failing).unwrap();
    let program = parse(&failing, &corpus_path).expect("parse failing corpus");
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    std::fs::write(&wasm_path, bytes).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node division leg failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "while-wasm-div-ok"
    );
}

fn envelope_for(path: &Path, token: &str, arguments: &[&str], max_steps: usize) -> String {
    let owned: Vec<String> = arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect();
    let options = InterpreterOptions::new(65536, max_steps).unwrap();
    interpreter::interpret(path, token, &owned, &options)
        .expect("interpretation")
        .envelope
}

#[test]
fn interpreter_agrees_with_backends_on_loop_results() {
    let path = write_corpus("interpret");

    for (token, arguments, expected) in [
        ("while.count_sum", vec!["4"], "10"),
        ("while.digit_sum", vec!["98765"], "35"),
        ("while.nested", vec!["3", "2"], "21"),
        ("while.zero_iterations", vec!["false"], "30"),
    ] {
        let owned: Vec<String> = arguments
            .iter()
            .map(|argument| argument.to_string())
            .collect();
        let options = InterpreterOptions::new(65536, DEFAULT_MAX_STEPS).unwrap();
        let envelope = interpreter::interpret(&path, token, &owned, &options)
            .expect("interpretation")
            .envelope;
        let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
        let outcome = &parsed["payload"]["outcome"];
        assert_eq!(outcome["kind"], "returned", "{token}: {envelope}");
        assert_eq!(
            outcome["value"], expected,
            "{token} agrees with both backends: {envelope}"
        );
    }

    // Condition-dependent failure selects the exact compiler-owned
    // arithmetic division-by-zero status on the fourth iteration.
    let failed = envelope_for(
        &path,
        "while.div_fails_on_iteration",
        &["3"],
        DEFAULT_MAX_STEPS,
    );
    let parsed: serde_json::Value = serde_json::from_str(&failed).unwrap();
    let outcome = &parsed["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed");
    assert_eq!(outcome["status"]["schema"], "semaprax.status.v1");
    assert_eq!(outcome["status"]["domain_id"], "semaprax.arithmetic.v1");
    assert_eq!(outcome["status"]["code"], 4);
    assert_eq!(outcome["status"]["class"], "arithmetic");
}

#[test]
fn interpreter_fuel_exhaustion_fails_closed_on_nonterminating_loops() {
    let path = write_corpus("fuel");
    let options = InterpreterOptions::new(65536, 16).unwrap();
    let interpretation = interpreter::interpret(
        &path,
        "while.count_sum",
        &["1000000000000".to_owned()],
        &options,
    )
    .expect("interpretation");
    let parsed: serde_json::Value = serde_json::from_str(&interpretation.envelope).unwrap();
    assert_eq!(parsed["payload"]["outcome"]["kind"], "fuel_exhausted");
    assert_eq!(parsed["payload"]["fuel"]["exhausted"], true);
    assert!(!interpretation.returned, "exhausted loops return nothing");
}

const RECORD_BORROW_RENEWAL: &str = include_str!("while_loops/record_borrow_renewal.spx");

#[test]
fn record_owner_one_pass_pipeline_uses_a_fresh_binding_outside_while() {
    let source = r#"module test.record_owner_pipeline;
@id("pipeline.matcher")
record Matcher {
    @id("pipeline.matcher.storage") storage: Bytes,
    @id("pipeline.matcher.position") position: usize,
}
@id("pipeline.advance")
fn advance(state: own Matcher, input: borrow Slice<u8>) -> Matcher {
    match own state { Matcher { storage, position } => Matcher { storage: storage, position: position + byte_len(input) }, }
}
@id("pipeline.observe")
fn observe(state: borrow Matcher) -> usize { state.position }
@id("pipeline.valid")
fn valid() -> usize {
    let input = [1u8];
    let view = array_as_slice(input);
    let current = Matcher { storage: bytes_zeroed(1usize), position: 0usize };
    let next = advance(current, view);
    observe(next)
}
@id("main")
fn main() -> i64 { if valid() == 1usize { 1 } else { 0 } }
@id("pipeline.invalid")
fn invalid() -> usize {
    let input = [1u8];
    let view = array_as_slice(input);
    let mut current = Matcher { storage: bytes_zeroed(1usize), position: 0usize };
    current = advance(current, view);
    observe(current)
}
"#;
    let valid = source.split(r#"@id("pipeline.invalid")"#).next().unwrap();
    semaprax::check(valid, "record-owner-pipeline-valid.spx").unwrap();
    let diagnostics = semaprax::check(source, "record-owner-pipeline.spx").unwrap_err();
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        vec!["SPX-U105", "SPX-O101"]
    );
}

#[test]
fn record_owner_renewal_named_views_preserve_canonical_projection_and_cleanup() {
    let program = semaprax::check(RECORD_BORROW_RENEWAL, "record-borrow-renewal.spx").unwrap();
    let canonical = format::canonical(&program);
    assert_eq!(
        format::canonical(&parse(&canonical, "roundtrip.spx").unwrap()),
        canonical
    );
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let run = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "matcher.run")
        .unwrap();
    assert_eq!(run.cleanup_plan.schema, "semaprax.cleanup-plan.v12");
    for reserve in [true, false] {
        assert_eq!(
            run.cleanup_plan
                .blocks
                .iter()
                .flat_map(|b| &b.transitions)
                .filter(|t| {
                    if reserve {
                        matches!(
                            t,
                            semaprax::cleanup_plan::CleanupTransition::ReserveRenewal { .. }
                        )
                    } else {
                        matches!(t, semaprax::cleanup_plan::CleanupTransition::Renew { .. })
                    }
                })
                .count(),
            1
        );
    }
    let json = graph::to_json(&program).unwrap();
    assert_eq!(json, graph::to_json(&program).unwrap());
    graph::verify_json(&program, &json).unwrap();
    assert!(!run.loan_plan.loans.is_empty());
    assert!(run.loan_plan.loans.iter().any(|loan| matches!(
        loan.cause,
        semaprax::loan_plan::LoanCause::BorrowedCall { .. }
    )));
    assert_eq!(
        codegen::emit_c(&program).unwrap(),
        codegen::emit_c(&program).unwrap()
    );
    assert_eq!(
        semaprax::wasm::emit_module(&program).unwrap(),
        semaprax::wasm::emit_module(&program).unwrap()
    );
}

#[test]
fn record_owner_renewal_named_views_execute_and_settle_on_three_backends() {
    use super::owned_string_loops_v1::support::Fixture;
    let program = semaprax::check(RECORD_BORROW_RENEWAL, "record-borrow-renewal.spx").unwrap();
    let mut fixture = Fixture::new(RECORD_BORROW_RENEWAL);
    let result = interpreter::internal_strings::interpret(
        &fixture.source,
        "app.main",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap();
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(document["payload"]["outcome"]["value"], "15");
    let generated = codegen::emit_c(&program).unwrap();
    let probe = format!("{}\n#define FIXTURE_TRACK_CALLOC\n{}\n{generated}\n#undef malloc\n#undef calloc\n#undef free\n#undef FIXTURE_TRACK_CALLOC\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\nfor(unsigned i=0;i<8;++i) {{ int64_t value=INT64_MIN; REQUIRE(spx_decl_{}(&context,&value)==0); REQUIRE(value==15); REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees); }}\nreturn 0; }}\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c"), hex_identity("app.main"));
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), "");
    }
    let root = fixture.root.join("web");
    semaprax::wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("probe.mjs"), r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
// Matcher.storage, independent source Bytes, and the owned label String share this arena.
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:3});
for(let i=0;i<8;i++) { const value=instance.exports.semaprax_main(); if(value!==15n) throw Error(`renewal:${value}`); }
"#).unwrap();
    let output = Command::new("node")
        .arg(root.join("probe.mjs"))
        .current_dir(&root)
        .output()
        .expect("Node is required for record renewal parity");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    for name in [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ] {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    fixture.cleanup();
}

#[test]
fn record_owner_renewal_named_views_refuse_aliases_and_temporary_operands() {
    for setup in [
        "let input = bytes_as_slice(matcher.storage); let input_alias = input;",
        "let input = bytes_as_slice(matcher.storage); let intermediate = input; let input_alias = intermediate;",
    ] {
        let source = RECORD_BORROW_RENEWAL.replace("let input = bytes_as_slice(source);\n    let input_alias = input;", setup);
        let report = verify_diagnostics(&source);
        assert_eq!(report.iter().map(|d| (d.code, d.message.as_str())).collect::<Vec<_>>(), vec![
            ("SPX-T265", "move or call transfer would invalidate a lexical byte view"),
            ("SPX-T265", "assignment would replace storage held by a lexical byte view"),
            ("SPX-T265", "move or transfer would invalidate an active shared loan"),
        ], "{report:?}");
    }
    for argument in ["bytes_as_slice(source)", "{ input_alias }"] {
        let source = RECORD_BORROW_RENEWAL.replace(
            "feed(matcher, input_alias, text, 1usize)",
            &format!("feed(matcher, {argument}, text, 1usize)"),
        );
        assert!(verify_diagnostics(&source)
            .iter()
            .any(|d| d.code == "SPX-T252"));
    }
    let allocating = RECORD_BORROW_RENEWAL.replace(
        "storage: storage, position: position +",
        "storage: bytes_copy(input), position: position +",
    );
    assert!(verify_diagnostics(&allocating)
        .iter()
        .any(|d| d.code == "SPX-T267"));
}

#[path = "while_loops/ascii_pattern_source.rs"]
mod ascii_pattern_source;
