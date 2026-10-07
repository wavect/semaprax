//! Owned `string` values selected by `if` and `match`: bindings, literals and
//! call results joined by `if`, `if` as a call argument, `match` arms over
//! payload-free variants and scalars, and `string` parameters and results of
//! user functions. The same corpus runs on the reference interpreter, on
//! generated C11 under an allocation-counting allocator that rejects
//! duplicate and foreign frees and requires zero live allocations after every
//! case, and on the String-settling Core Wasm profile. A String-bearing record outside the bounded executable profile retains
//! its stable signature refusal.

use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

// Loaded once by the sibling module; a second `mod` would duplicate it.
use super::owned_string_loops_v1::support;
use support::Fixture;

const SOURCE: &str = r#"
module test.string_branches;

@id("branch.severity")
variant Severity {
    @id("branch.severity.critical")
    Critical,
    @id("branch.severity.minor")
    Minor,
}

@id("branch.pick")
fn pick(n: i64) -> i64
{
    let a = string_concat("ab", "c");
    let b = string_concat("de", "");
    let c = if n > 0 { a } else { b };
    string_len(c)
}

@id("branch.if_bindings")
fn if_bindings() -> i64
{
    pick(1) * 10 + pick(0)
}

@id("branch.if_literals")
fn if_literals() -> i64
{
    let n = 0;
    let c = if n > 0 { "abc" } else { "de" };
    let d = if n == 0 { string_concat("a", "bc") } else { "de" };
    string_len(c) * 10 + string_len(d)
}

@id("branch.if_argument")
fn if_argument() -> i64
{
    let c = true;
    let s = string_concat(if c { "a" } else { "bb" }, "x");
    let t = string_concat("y", if c { "a" } else { "bb" });
    string_len(s) * 10 + string_len(t)
}

@id("branch.if_block_tail")
fn if_block_tail(flag: bool) -> string
{
    let scratch = string_concat("scope", "");
    if flag { "a" } else { string_concat("b", "b") }
}

@id("branch.if_block_tail_case")
fn if_block_tail_case() -> i64
{
    string_len(if_block_tail(true)) * 10 + string_len(if_block_tail(false))
}

@id("branch.action")
fn action(severity: Severity) -> string
{
    match severity { Severity::Critical {} => "page", _ => "watch", }
}

@id("branch.match_variant_call")
fn match_variant_call() -> i64
{
    string_len(action(Severity::Critical {})) * 10 + string_len(action(Severity::Minor {}))
}

@id("branch.match_variant_let")
fn match_variant_let() -> i64
{
    let severity = Severity::Minor {};
    let kept = string_concat("keep", "");
    let chosen = match severity { Severity::Critical {} => string_concat("pa", "ge"), Severity::Minor {} => kept, };
    string_len(chosen)
}

@id("branch.match_argument")
fn match_argument() -> i64
{
    let severity = Severity::Critical {};
    let n = 3;
    string_len(match severity { Severity::Critical {} => "page", _ => "watch", }) * 100 + string_len(string_concat(match n { 1 => "a", _ => "bb", }, match severity { Severity::Minor {} => "c", Severity::Critical {} => "dd", }))
}

@id("branch.match_variant_if")
fn match_variant_if(severity: Severity, flag: bool) -> string
{
    match severity { Severity::Critical {} => if flag { "a" } else { string_concat("b", "b") }, _ => "ccc", }
}

@id("branch.match_variant_if_case")
fn match_variant_if_case() -> i64
{
    string_len(match_variant_if(Severity::Critical {}, true)) * 100 + string_len(match_variant_if(Severity::Critical {}, false)) * 10 + string_len(match_variant_if(Severity::Minor {}, false))
}

@id("branch.label")
fn label(n: i64) -> string
{
    match n { 0 => "zero", 1 => "one", _ => "many", }
}

@id("branch.match_scalar_if")
fn match_scalar_if(n: i64, flag: bool) -> string
{
    match n { 0 => if flag { "a" } else { "bb" }, _ => if flag { string_concat("c", "") } else { "ddd" }, }
}

@id("branch.match_scalar_if_case")
fn match_scalar_if_case() -> i64
{
    string_len(match_scalar_if(0, true)) * 1000 + string_len(match_scalar_if(0, false)) * 100 + string_len(match_scalar_if(1, true)) * 10 + string_len(match_scalar_if(1, false))
}

@id("branch.match_scalar")
fn match_scalar() -> i64
{
    string_len(label(0)) * 100 + string_len(label(1)) * 10 + string_len(label(9))
}

@id("branch.measure")
fn measure(text: string) -> i64
{
    string_len(text)
}

@id("branch.if_user_argument")
fn if_user_argument() -> i64
{
    measure(if true { "a" } else { "bb" }) * 10 + measure(if false { "c" } else { "ddd" })
}

@id("branch.string_parameter")
fn string_parameter() -> i64
{
    let text = "hello";
    measure(text) + measure(string_concat("hello", "!"))
}

@id("branch.main")
fn main() -> i64
{
    if_bindings() + if_literals() + if_argument() + if_block_tail_case() + match_variant_call() + match_variant_let() + match_argument() + match_variant_if_case() + match_scalar() + match_scalar_if_case() + string_parameter()
}
"#;

/// Every case and its observation on every lane.
const CASES: &[(&str, &str)] = &[
    ("branch.if_bindings", "ok|32"),
    ("branch.if_literals", "ok|23"),
    ("branch.if_argument", "ok|22"),
    ("branch.if_block_tail_case", "ok|12"),
    ("branch.match_variant_call", "ok|45"),
    ("branch.match_variant_let", "ok|4"),
    ("branch.match_argument", "ok|404"),
    ("branch.match_variant_if_case", "ok|123"),
    ("branch.match_scalar", "ok|434"),
    ("branch.match_scalar_if_case", "ok|1213"),
    ("branch.if_user_argument", "ok|13"),
    ("branch.string_parameter", "ok|11"),
    ("branch.main", "ok|2323"),
];

/// The String-settling Core Wasm profile carries only `i64`, `bool`, `char`,
/// and `string` values, so the variant cases and `main` stay off this lane.
const WASM_CASES: &[&str] = &[
    "branch.if_bindings",
    "branch.if_literals",
    "branch.if_argument",
    "branch.match_scalar",
    "branch.string_parameter",
];

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_branch_results_round_trip_and_project_their_joins() {
    let program = parse(SOURCE, Path::new("string-branches.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert_eq!(canonical, SOURCE.trim_start(), "corpus is canonical");
    let reparsed = parse(&canonical, Path::new("string-branches-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    let document: Value = serde_json::from_str(&graph).unwrap();
    let mut pending = vec![&document];
    let (mut string_ifs, mut string_matches) = (0, 0);
    while let Some(node) = pending.pop() {
        match node {
            Value::Object(map) => {
                let string_typed = map.get("type_id").and_then(Value::as_str) == Some("string")
                    && map.get("ownership_mode").and_then(Value::as_str) == Some("own");
                match map.get("kind").and_then(Value::as_str) {
                    Some("if") if string_typed => string_ifs += 1,
                    Some("match") if string_typed => string_matches += 1,
                    _ => {}
                }
                pending.extend(map.values());
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    assert!(string_ifs >= 5, "string-typed if joins: {string_ifs}");
    assert!(
        string_matches >= 3,
        "string-typed match joins: {string_matches}"
    );
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn reference_interpreter_runs_string_branch_results() {
    let fixture = Fixture::new(SOURCE);
    for (id, expected) in CASES {
        // `semaprax run` tries the canonical profile, then the internal
        // String profile that admits `string` parameters and results.
        let options = InterpreterOptions::default();
        let result = interpreter::interpret(&fixture.source, id, &[], &options)
            .or_else(|_| {
                interpreter::internal_strings::interpret(&fixture.source, id, &[], &options)
            })
            .unwrap_or_else(|errors| panic!("{id}: {errors:?}"));
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        assert_eq!(outcome["kind"], "returned", "{}", result.envelope);
        let observed = format!("ok|{}", outcome["value"].as_str().unwrap());
        assert_eq!(&observed, expected, "{id}");
    }
    fixture.cleanup();
}

#[test]
fn run_executes_string_branch_results_on_both_routes() {
    let fixture = Fixture::new(SOURCE);
    for native in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
        command.arg("run").arg(&fixture.source);
        if native {
            if !command_available("clang") {
                continue;
            }
            command.arg("--native");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "native={native}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "2323\n");
    }
    fixture.cleanup();
}

#[test]
fn native_string_branch_results_settle_every_allocation() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("string-branches-native.spx")).unwrap();
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
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    REQUIRE(token==0);
    (void)printf("{id}|ok|%lld\n",(long long)value);
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn string_settling_wasm_profile_releases_every_branch_owner() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("string-branches-wasm.spx")).unwrap();
    let selected = WASM_CASES
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let artifact = emit_module(&program, &selected, InternalStringOptions::default()).unwrap();
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
    // The trusted runtime settles its arena after every call: a leaked or
    // twice-released owner poisons the instance and the call throws.
    let script = fixture.write(
        "probe.mjs",
        format!(
            r#"import {{readFileSync}} from 'node:fs';
import {{instantiate}} from './program.mjs';
const api=await instantiate(Uint8Array.from(readFileSync('program.wasm')));
for(const id of {}){{
  let first;
  for(let repeat=0;repeat<3;repeat++){{
    const outcome=api.call(id);
    const observed=outcome.kind==='success'?`ok|${{outcome.value}}`:`${{outcome.domain}}|${{outcome.code}}`;
    if(first===undefined)first=observed;else if(first!==observed)throw new Error(id);
  }}
  process.stdout.write(`${{id}}|${{first}}\n`);
}}
"#,
            serde_json::to_string(&selected).unwrap()
        ),
    );
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let expected = CASES
        .iter()
        .filter(|(id, _)| WASM_CASES.contains(id))
        .map(|(id, observation)| format!("{id}|{observation}\n"))
        .collect::<String>();
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    fixture.cleanup();
}

#[test]
fn string_settling_wasm_profile_refuses_variant_matches_with_one_stable_diagnostic() {
    let program = parse(SOURCE, Path::new("string-branches-wasm-refused.spx")).unwrap();
    for id in [
        "branch.match_variant_call",
        "branch.match_variant_let",
        "branch.match_argument",
    ] {
        let error = emit_module(&program, &[id.to_owned()], InternalStringOptions::default())
            .expect_err("variants are outside the String-settling Wasm profile");
        assert_eq!(error.code, "SPX-W111", "{id}: {}", error.message);
    }
    let source = "module test.wasm_variant_match;\n\n@id(\"level\")\nvariant Level {\n    @id(\"level.low\")\n    Low {},\n    @id(\"level.high\")\n    High {},\n}\n\n@id(\"name\")\nfn name(level: Level) -> string\n{\n    match level { Level::Low {} => \"low\", _ => \"high\", }\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    string_len(name(Level::Low {}))\n}\n";
    let program = parse(source, Path::new("wasm-variant-match.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let error = emit_module(
        &program,
        &["app.main".to_owned()],
        InternalStringOptions::default(),
    )
    .expect_err("a variant signature is outside the String-settling Wasm profile");
    assert_eq!(error.code, "SPX-W111", "{}", error.message);
}

const STRING_RECORD: &str = r#"module test.string_record;

@id("task")
record Task {
    @id("task.title")
    title: string,
    @id("task.points")
    points: [u8; 2],
}
"#;

fn record_diagnostics(rest: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let source = format!("{STRING_RECORD}\n{rest}");
    let program = parse(&source, Path::new("string-record.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn string_record_shapes_outside_owned_text_profile_keep_the_stable_diagnostic() {
    // A declaration alone stays admitted: projects use it as a schema.
    let declared = record_diagnostics("@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n");
    assert!(declared.is_empty(), "{declared:?}");
    for (rest, role) in [
        (
            "@id(\"app.f\")\nfn f(t: Task) -> i64\n{\n    0\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
            "passed",
        ),
        (
            "@id(\"app.make\")\nfn make() -> Task\n{\n    Task { title: \"write\", points: [3u8, 4u8] }\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
            "returned",
        ),
    ] {
        let diagnostics = record_diagnostics(rest);
        let refused = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "SPX-T309")
            .collect::<Vec<_>>();
        assert!(!refused.is_empty(), "{role}: {diagnostics:?}");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "SPX-H006"),
            "{role}: {diagnostics:?}"
        );
        assert_eq!(
            refused[0].message,
            format!(
                "record `Task` carries `string` field `title`; a string-bearing record cannot be {role} in executable code"
            )
        );
        assert!(refused[0]
            .help
            .as_deref()
            .is_some_and(|help| help.contains("`title: string`")));
    }
}
