//! Owned String Loops v2: user functions with `string` parameters or results
//! called in `while` and `for` bodies, executed on the reference interpreter,
//! generated C11 with an allocation-counting harness, and the
//! String-settling Core Wasm profile, plus the shapes that stay refused with
//! stable diagnostics.

use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

use super::owned_string_loops_v1::support::Fixture;

const SOURCE: &str = r#"
module test.owned_string_loops_v2;

@id("calls.limit")
fn limit(value: i64) -> i64
    requires value < 7
{
    value
}

@id("calls.weight")
fn weight(line: string) -> i64
{
    string_len(line)
}

@id("calls.label")
fn label(n: i64) -> string
{
    string_concat("n=", string_from_i64(n))
}

@id("calls.wrap")
fn wrap(text: string, n: i64) -> string
{
    string_concat(string_concat("<", text), string_concat(string_from_i64(limit(n)), ">"))
}

@id("calls.square")
fn square(text: string) -> string
{
    let n = string_len(text);
    string_concat(text, string_from_i64(n))
}

@id("calls.scaled")
fn scaled(text: string, factor: i64) -> i64
{
    string_len(text) * factor
}

@id("calls.consume")
fn consume() -> i64
{
    let mut i = 0;
    let mut total = 0;
    while i < 50 {
        let piece = string_from_i64(i);
        total = total + weight(piece);
        total = total + weight("lit");
        total = total + weight(string_slice("abcdef", 1, 4));
        i = i + 1;
        0
    }
    total
}

@id("calls.produce")
fn produce() -> i64
{
    let mut out = "";
    let mut i = 0;
    while i < 30 {
        let l = label(i);
        out = string_concat(out, l);
        out = string_concat(out, label(i + 1));
        let unused = label(i * 2);
        i = i + 1;
        0
    }
    string_len(out)
}

@id("calls.nested")
fn nested() -> i64
{
    let mut out = "";
    let mut i = 0;
    while i < 6 {
        let text = "abc";
        out = string_concat(out, wrap(square(text), i));
        let hit = if i % 2 == 0 {
            weight(label(i))
        } else {
            0
        };
        i = i + 1 + hit - hit;
        0
    }
    string_len(out) * 100 + i
}

@id("calls.contract")
fn contract() -> i64
{
    let mut out = "c";
    let mut i = 0;
    while i < 10 {
        let kept = label(i);
        out = string_concat(out, wrap(kept, i));
        i = i + 1;
        0
    }
    string_len(out)
}

@id("calls.overflow")
fn overflow() -> i64
{
    let mut out = "o";
    let mut big = 1;
    let mut i = 0;
    while i < 100 {
        let note = label(i);
        out = string_concat(out, square("ab"));
        big = big * scaled(note, 1000);
        i = i + 1;
        0
    }
    string_len(out)
}

@id("calls.traverse")
fn traverse() -> i64
{
    let mut building = vec_with_capacity<i64>(3usize);
    building = vec_push<i64>(building, 7);
    building = vec_push<i64>(building, 42);
    building = vec_push<i64>(building, 1000);
    let values = building;
    let mut out = "v";
    for item in values {
        out = string_concat(out, label(item));
        out = string_concat(out, string_from_i64(weight(label(item))));
        0
    }
    string_len(out)
}

@id("calls.echo")
fn echo(text: string) -> string
{
    string_concat(text, "!")
}

@id("calls.literal")
fn literal() -> i64
{
    let mut out = "w";
    let mut i = 0;
    while i < 20 {
        out = string_concat(out, echo("ab"));
        i = i + weight(echo(""));
        0
    }
    string_len(out)
}

@id("calls.literal_contract")
fn literal_contract() -> i64
{
    let mut out = "w";
    let mut i = 0;
    while i < 20 {
        let kept = echo("q");
        out = string_concat(out, echo(kept));
        i = limit(i) + weight(echo(""));
        0
    }
    string_len(out)
}

@id("app.main")
fn main() -> i64
{
    produce()
}
"#;

/// Every case and its observation on every lane that admits it.
const CASES: &[(&str, &str)] = &[
    ("calls.consume", "ok|390"),
    ("calls.produce", "ok|221"),
    ("calls.nested", "ok|4206"),
    ("calls.contract", "semaprax.contract.v1|1"),
    ("calls.overflow", "semaprax.arithmetic.v1|3"),
    ("calls.traverse", "ok|17"),
    ("calls.literal", "ok|61"),
    ("calls.literal_contract", "semaprax.contract.v1|1"),
];

/// The literal-only cases the String-settling Wasm profile admits; numeric
/// text and Vec values stay outside that closed profile.
const WASM_CASES: &[&str] = &["calls.literal", "calls.literal_contract"];

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn loop_calls_round_trip_and_resolve() {
    let program = parse(SOURCE, Path::new("owned-string-loops-v2.spx")).unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "{:?}",
        verify::verify(&program)
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("owned-string-loops-v2-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    assert!(graph.contains("\"callee\":\"calls.label\""));
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn reference_interpreter_calls_string_functions_in_loops() {
    let canonical =
        format::canonical(&parse(SOURCE, Path::new("owned-string-loops-v2.spx")).unwrap());
    let fixture = Fixture::new(&canonical);
    for (id, expected) in CASES {
        // `semaprax run` retries a program the canonical profile refuses
        // (user functions over `string`) on the internal String profile.
        let options = InterpreterOptions::default();
        let result = interpreter::interpret(&fixture.source, id, &[], &options)
            .or_else(|_| {
                interpreter::internal_strings::interpret(&fixture.source, id, &[], &options)
            })
            .unwrap();
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
fn native_loop_calls_settle_every_allocation_on_every_exit() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("owned-string-loops-v2-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    // The Vec carrier allocates with `calloc`, which the counting harness
    // does not observe; `calls.traverse` keeps its interpreter evidence.
    for (id, observation) in CASES.iter().filter(|(id, _)| *id != "calls.traverse") {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        REQUIRE(value==INT64_MIN);
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
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn string_settling_wasm_profile_runs_loop_calls() {
    if !command_available("node") || WASM_CASES.is_empty() {
        return;
    }
    let program = parse(SOURCE, Path::new("owned-string-loops-v2-wasm.spx")).unwrap();
    let selected = WASM_CASES
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let artifact = emit_module(&program, &selected, InternalStringOptions::default()).unwrap();
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
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
            serde_json::to_string(WASM_CASES).unwrap()
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

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("refused.spx")).unwrap();
    verify::verify(&program)
}

const HELPERS: &str = "module test.refused;\n\n@id(\"refused.weight\")\nfn weight(line: string) -> i64\n{\n    string_len(line)\n}\n\n@id(\"refused.pair\")\nrecord Pair {\n    @id(\"refused.pair.a\")\n    a: i64,\n}\n\n@id(\"refused.first\")\nfn first(pair: Pair) -> i64\n{\n    pair.a\n}\n\n";

#[test]
fn shapes_outside_owned_string_loops_v2_stay_refused() {
    // A `string` parameter consumes its argument: passing a string declared
    // before the loop would move it on the first iteration.
    let found = diagnostics(&format!(
        "{HELPERS}@id(\"app.main\")\nfn main() -> i64\n{{\n    let text = \"x\";\n    let mut i = 0;\n    while i < 2 {{\n        i = i + weight(text);\n        0\n    }}\n    i\n}}\n"
    ));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code, "SPX-T252");
    assert_eq!(
        found[0].message,
        "ownership of `text` changes inside a while loop, which is not yet admitted"
    );
    // Record parameters stay outside the loop-call profile.
    let found = diagnostics(&format!(
        "{HELPERS}@id(\"app.main\")\nfn main() -> i64\n{{\n    let mut i = 0;\n    while i < 2 {{\n        i = i + first(Pair {{ a: 1 }});\n        0\n    }}\n    i\n}}\n"
    ));
    assert!(
        found.iter().any(|diagnostic| diagnostic.code == "SPX-T252"
            && diagnostic.message
                == "call `first` is not admitted in while bodies; only functions over scalars, byte slices and strings qualify"),
        "{found:?}"
    );
}
