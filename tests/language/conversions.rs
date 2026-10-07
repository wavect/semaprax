//! Conversions v1 (`docs/LANGUAGE-ERGONOMICS-V1.md`): `f64_from_i64`,
//! `i64_from_f64`, `usize_from_i64`, and `i64_from_usize`. The same corpus
//! runs on the reference interpreter and on generated C11 under an
//! allocation-counting allocator, including every checked
//! `semaprax.convert.v1` failure, and through `semaprax run` on both routes.
//! Core Wasm refuses the family with one stable diagnostic.

use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

// Loaded once by the sibling module; a second `mod` would duplicate it.
use super::owned_string_loops_v1::support;
use support::Fixture;

const SOURCE: &str = r#"module test.conversions;

@id("conv.mean")
fn mean() -> i64
{
    let total = 10;
    let count = 4;
    let average = f64_from_i64(total) / f64_from_i64(count);
    i64_from_f64(average * 10.0)
}

@id("conv.truncate")
fn truncate() -> i64
{
    i64_from_f64(2.75) * 100 + i64_from_f64(-2.75) * 10 + i64_from_f64(-0.5)
}

@id("conv.round_trip")
fn round_trip() -> i64
{
    i64_from_f64(f64_from_i64(-9223372036854775808)) + i64_from_f64(f64_from_i64(9007199254740993)) - 9007199254740992
}

@id("conv.extremes")
fn extremes() -> i64
{
    let low = i64_from_f64(-9223372036854776000.0);
    let high = i64_from_f64(9223372036854775000.0);
    if low == -9223372036854775807 - 1 && high == 9223372036854774784 { 1 } else { 0 }
}

@id("conv.sizes")
fn sizes() -> i64
{
    let n = usize_from_i64(5);
    i64_from_usize(9223372036854775807usize) - 9223372036854775807 + i64_from_usize(n + 2usize)
}

@id("conv.loop")
fn sum_indexes() -> i64
{
    let mut index = 0usize;
    let mut total = 0;
    while index < usize_from_i64(4) {
        total = total + i64_from_usize(index);
        index = index + 1usize;
        0
    }
    total
}

@id("conv.negative_size")
fn negative_size() -> i64
{
    let n = usize_from_i64(-1);
    i64_from_usize(n)
}

@id("conv.size_too_large")
fn size_too_large() -> i64
{
    i64_from_usize(9223372036854775808usize)
}

@id("conv.float_too_large")
fn float_too_large() -> i64
{
    i64_from_f64(9223372036854776000.0)
}

@id("conv.float_too_small")
fn float_too_small() -> i64
{
    i64_from_f64(-10000000000000000000.0)
}

@id("conv.not_a_number")
fn not_a_number() -> i64
{
    let zero = 0.0;
    i64_from_f64(zero / zero)
}

@id("conv.loop_failure")
fn loop_failure() -> i64
{
    let mut value = 2;
    let mut total = 0usize;
    while value > -3 {
        total = total + usize_from_i64(value);
        value = value - 1;
        0
    }
    i64_from_usize(total)
}

@id("app.main")
fn main() -> i64
{
    mean() + truncate() + round_trip() + extremes() + sizes() + sum_indexes()
}
"#;

/// Every case and its observation on both lanes.
const CASES: &[(&str, &str)] = &[
    ("conv.mean", "ok|25"),
    ("conv.truncate", "ok|180"),
    ("conv.round_trip", "ok|-9223372036854775808"),
    ("conv.extremes", "ok|1"),
    ("conv.sizes", "ok|7"),
    ("conv.loop", "ok|6"),
    ("conv.negative_size", "semaprax.convert.v1|1"),
    ("conv.size_too_large", "semaprax.convert.v1|1"),
    ("conv.float_too_large", "semaprax.convert.v1|1"),
    ("conv.float_too_small", "semaprax.convert.v1|1"),
    ("conv.not_a_number", "semaprax.convert.v1|2"),
    ("conv.loop_failure", "semaprax.convert.v1|1"),
];

/// `main` = 25 + 180 + i64::MIN + 1 + 7 + 6, which stays in range.
const MAIN: &str = "-9223372036854775589";

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn conversions_round_trip_and_project_compiler_identities() {
    let program = parse(SOURCE, Path::new("conversions.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), SOURCE, "corpus is canonical");
    let reparsed = parse(&format::canonical(&program), Path::new("canonical.spx")).unwrap();
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    for callee in [
        "core.num.f64_from_i64",
        "core.num.i64_from_f64",
        "core.num.usize_from_i64",
        "core.num.i64_from_usize",
    ] {
        assert!(
            graph.contains(&format!("\"callee\":\"{callee}\"")),
            "{callee}"
        );
    }
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn reference_interpreter_runs_conversions() {
    let fixture = Fixture::new(SOURCE);
    for (id, expected) in CASES {
        let result =
            interpreter::interpret(&fixture.source, id, &[], &InterpreterOptions::default())
                .unwrap_or_else(|errors| panic!("{id}: {errors:?}"));
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let observed = if outcome["kind"] == "returned" {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            assert_eq!(outcome["kind"], "failed", "{}", result.envelope);
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, expected, "{id}");
    }
    fixture.cleanup();
}

#[test]
fn native_conversions_select_the_same_checked_statuses() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("conversions-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    for (id, observation) in CASES {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    int64_t value=INT64_MAX;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        REQUIRE(value==INT64_MAX);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL);
        (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    // Numeric conversions must allocate nothing. Keep the allocator hooks
    // referenced even though this corpus has no allocating operation.
    probe.push_str(
        "(void)fixture_malloc; (void)fixture_free; REQUIRE(fixture_allocations==0); return 0; }\n",
    );
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn run_executes_conversions_and_reports_a_failure_on_both_routes() {
    let fixture = Fixture::new(SOURCE);
    let failing = Fixture::new(&SOURCE.replace(
        "mean() + truncate()",
        "negative_size() + mean() + truncate()",
    ));
    for native in [false, true] {
        if native && !command_available("clang") {
            continue;
        }
        for (source, success) in [(&fixture.source, true), (&failing.source, false)] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
            command.arg("run").arg(source);
            if native {
                command.arg("--native");
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert_eq!(
                output.status.success(),
                success,
                "native={native}: {stderr}"
            );
            if success {
                assert_eq!(stdout.trim(), MAIN, "native={native}");
            } else {
                assert!(
                    stderr.contains("semaprax.convert.v1")
                        && stderr.contains("conversion out of range"),
                    "native={native}: {stderr}"
                );
            }
        }
    }
    fixture.cleanup();
    failing.cleanup();
}

#[test]
fn core_wasm_keeps_float_conversion_refusal_with_one_stable_diagnostic() {
    let program = parse(SOURCE, Path::new("conversions-wasm.spx")).unwrap();
    let error = semaprax::wasm::emit_module(&program).expect_err("scalar Core Wasm lane");
    assert_eq!(error.code, "SPX-W116", "{}", error.message);
    assert_eq!(
        error.message,
        "Conversions v1 operation `f64_from_i64` is not lowered to Core Wasm; run it on the reference interpreter or native C11"
    );
    let error = emit_module(
        &program,
        &["conv.sizes".to_owned()],
        InternalStringOptions::default(),
    )
    .expect_err("String-settling Core Wasm lane");
    assert_eq!(error.code, "SPX-W116", "{}", error.message);
}

fn diagnostics(body: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let source =
        format!("module test.refused;\n\n@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n");
    let program = parse(&source, Path::new("refused.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn conversion_misuse_has_stable_diagnostics() {
    for (body, code) in [
        ("    i64_from_f64(3)", "SPX-T205"),
        ("    i64_from_usize(3)", "SPX-T205"),
        ("    let x = f64_from_i64(1, 2);\n    0", "SPX-T204"),
        ("    let x = usize_from_i64(1.5);\n    0", "SPX-T205"),
    ] {
        let found = diagnostics(body);
        assert!(
            found.iter().any(|diagnostic| diagnostic.code == code),
            "{body}: {found:?}"
        );
    }
    let reserved = "module test.reserved;\n\n@id(\"app.convert\")\nfn f64_from_i64(value: i64) -> i64\n{\n    value\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n";
    let program = parse(reserved, Path::new("reserved.spx")).unwrap();
    assert!(verify::verify(&program)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-S113"));
}

#[test]
fn a_cast_names_the_conversion_functions() {
    let error = parse(
        "module test.cast;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    let x = 2.5;\n    x as i64\n}\n",
        Path::new("cast.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-P106");
    let help = error.help.unwrap_or_default();
    assert!(
        help.contains("i64_from_f64")
            && help.contains("f64_from_i64")
            && help.contains("usize_from_i64"),
        "{help}"
    );
}

/// `string_from_str` copies borrowed views into owned strings, which then
/// compare with `==` and feed every `string_*` operation.
const TEXT: &str = r#"module test.from_str;

@id("text.copy")
fn copy() -> i64
{
    let text = "hello";
    let view = string_as_str(text);
    let owned = string_from_str(view);
    let joined = string_concat(owned, "!");
    if joined == "hello!" { string_len(joined) } else { 0 }
}

@id("text.measure")
fn measure(s: borrow str) -> i64
{
    let owned = string_from_str(s);
    if owned == "abc" { 100 + string_len(owned) } else { string_len(owned) }
}

@id("text.parameter")
fn parameter() -> i64
{
    let text = "abc";
    let view = string_as_str(text);
    measure(view)
}

@id("text.loop")
fn copies_in_a_loop() -> i64
{
    let text = "ab";
    let view = string_as_str(text);
    let mut i = 0;
    let mut total = 0;
    while i < 3 {
        let copy = string_from_str(view);
        total = total + string_len(copy);
        i = i + 1;
        0
    }
    total
}

@id("text.failure")
fn failure_after_copy() -> i64
{
    let text = "xyz";
    let view = string_as_str(text);
    let owned = string_from_str(view);
    let size = usize_from_i64(0 - string_len(owned));
    i64_from_usize(size)
}

@id("app.main")
fn main() -> i64
{
    copy() + parameter() + copies_in_a_loop()
}
"#;

const TEXT_CASES: &[(&str, &str)] = &[
    ("text.copy", "ok|6"),
    ("text.parameter", "ok|103"),
    ("text.loop", "ok|6"),
    ("text.failure", "semaprax.convert.v1|1"),
    ("app.main", "ok|115"),
];

#[test]
fn string_from_str_copies_a_view_on_the_interpreter() {
    let program = parse(TEXT, Path::new("from-str.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), TEXT, "corpus is canonical");
    assert!(graph::to_json(&program)
        .unwrap()
        .contains("\"callee\":\"core.string.from_str\""));
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    let fixture = Fixture::new(TEXT);
    for (id, expected) in TEXT_CASES {
        let options = InterpreterOptions::default();
        let result = interpreter::interpret(&fixture.source, id, &[], &options)
            .or_else(|_| {
                interpreter::internal_strings::interpret(&fixture.source, id, &[], &options)
            })
            .unwrap_or_else(|errors| panic!("{id}: {errors:?}"));
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let observed = if outcome["kind"] == "returned" {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, expected, "{id}");
    }
    fixture.cleanup();
}

#[test]
fn native_string_from_str_settles_every_allocation() {
    if !command_available("clang") {
        return;
    }
    let program = parse(TEXT, Path::new("from-str-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    for (id, observation) in TEXT_CASES {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    int64_t value=INT64_MAX;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL);
        (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(TEXT);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

const FLAGS: &str = r#"module app.flags;

permit { process.args.read }

@id("app.main")
fn main() -> i64
    uses { process.args.read }
{
    let raw = arg_utf8(0usize);
    let flag = string_from_str(raw);
    if flag == "--top" { 7 } else { 3 }
}
"#;

#[test]
fn a_command_line_argument_compares_as_a_string_on_both_routes() {
    let program = parse(FLAGS, Path::new("flags.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), FLAGS);
    let fixture = Fixture::new(FLAGS);
    for native in [false, true] {
        if native && !command_available("clang") {
            continue;
        }
        for (argument, status) in [("--top", 7), ("--all", 3)] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
            command.arg("run").arg(&fixture.source);
            if native {
                command.arg("--native");
            }
            let output = command.args(["--", argument]).output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(status),
                "native={native} {argument}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    fixture.cleanup();
}

#[test]
fn core_wasm_refuses_string_from_str() {
    let program = parse(TEXT, Path::new("from-str-wasm.spx")).unwrap();
    let error = emit_module(
        &program,
        &["text.copy".to_owned()],
        InternalStringOptions::default(),
    )
    .expect_err("Conversions v1 is not lowered to Core Wasm");
    assert_eq!(error.code, "SPX-W116");
    assert_eq!(
        error.message,
        "Conversions v1 operation `string_from_str` is not lowered to Core Wasm; run it on the reference interpreter or native C11"
    );
}
