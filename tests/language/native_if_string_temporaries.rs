//! Scalar If branches settle their String operands before loop-cell reuse.
use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

use super::owned_string_loops_v1::support::Fixture;

const SOURCE: &str = r#"
module test.if_string_temporaries;

@id("if_temps.find")
fn find() -> i64 {
    let text = "a";
    let mut i = 0;
    let mut sum = 0;
    while i < 3 {
        let found = if i % 2 == 0 { string_find(text, "a", 0) } else { string_find(text, "missing", 0) };
        sum = sum + found;
        i = i + 1;
        0
    }
    sum
}

@id("if_temps.length")
fn length() -> i64 {
    let text = "ab";
    let mut i = 0;
    let mut sum = 0;
    while i < 4 {
        let size = if i % 2 == 0 { string_len(text) } else {
            let scratch = string_concat("x", "y");
            string_len(scratch)
        };
        sum = sum + size;
        i = i + 1;
        0
    }
    sum
}

@id("if_temps.nested")
fn nested() -> i64 {
    let mut i = 0;
    let mut sum = 0;
    while i < 4 {
        let size = if i % 2 == 0 {
            if i == 0 { string_len("a") } else { string_len("bb") }
        } else { string_len("ccc") };
        sum = sum + size;
        i = i + 1;
        0
    }
    sum
}

@id("if_temps.owned_result")
fn owned_result() -> i64 {
    let mut i = 0;
    let mut sum = 0;
    while i < 4 {
        let chosen = if i % 2 == 0 { "a" } else { string_concat("b", "b") };
        sum = sum + string_len(chosen);
        i = i + 1;
        0
    }
    sum
}

@id("if_temps.guard_false")
fn guard_false() -> i64 {
    let mut i = 0;
    let mut sum = 0;
    while i < 3 {
        let size = if i < 3 {
            match i { 0 if string_len("guard") < 0 => 10, _ => string_len("tail"), }
        } else { 0 };
        sum = sum + size;
        i = i + 1;
        0
    }
    sum
}

@id("if_temps.guard_failure")
fn guard_failure() -> i64 {
    let kept = "kept";
    let n = 1;
    if true {
        match n { 1 if string_len("guard") > 0 && n + 9223372036854775807 < 0 => 1, _ => string_len(kept), }
    } else { 0 }
}

@id("app.main")
fn main() -> i64 { find() }
"#;

const CASES: &[(&str, &str)] = &[
    ("if_temps.find", "ok|-1"),
    ("if_temps.length", "ok|8"),
    ("if_temps.nested", "ok|9"),
    ("if_temps.owned_result", "ok|6"),
    ("if_temps.guard_false", "ok|12"),
    ("if_temps.guard_failure", "semaprax.arithmetic.v1|1"),
];

#[test]
fn scalar_branch_temporaries_round_trip_and_match_interpreter() {
    let program = parse(SOURCE, Path::new("if-string-temporaries.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("if-string-temporaries-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    let fixture = Fixture::new(&canonical);
    for (id, expected) in CASES {
        let result =
            interpreter::interpret(&fixture.source, id, &[], &InterpreterOptions::default())
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
fn native_scalar_branch_temporaries_settle_before_reuse() {
    let program = parse(SOURCE, Path::new("if-string-temporaries-native.spx")).unwrap();
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
            r#"for (int repeat=0;repeat<3;repeat++) {{
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
        for _ in 0..3 {
            expected.push_str(&format!("{id}|{observation}\n"));
        }
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn wasm_length_branches_keep_their_existing_settlement() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, Path::new("if-string-temporaries-wasm.spx")).unwrap();
    // Find retains its existing Wasm refusal; Len is the admitted control.
    let selected = CASES
        .iter()
        .filter(|(id, _)| *id != "if_temps.find")
        .map(|(id, _)| (*id).to_owned())
        .collect::<Vec<_>>();
    let artifact = emit_module(&program, &selected, InternalStringOptions::default()).unwrap();
    let mut fixture = Fixture::new(SOURCE);
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
    let script = fixture.write("probe.mjs", format!(r#"import {{readFileSync}} from 'node:fs';
import {{instantiate}} from './program.mjs';
const api=await instantiate(Uint8Array.from(readFileSync('program.wasm')));
for(const id of {}){{
  for(let repeat=0;repeat<3;repeat++){{
    const outcome=api.call(id);
    const observed=outcome.kind==='success'?`ok|${{outcome.value}}`:`${{outcome.domain}}|${{outcome.code}}`;
    process.stdout.write(`${{id}}|${{observed}}\n`);
  }}
}}
"#, serde_json::to_string(&selected).unwrap()));
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
        .filter(|(id, _)| *id != "if_temps.find")
        .flat_map(|(id, outcome)| std::iter::repeat_n(format!("{id}|{outcome}\n"), 3))
        .collect::<String>();
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
    fixture.cleanup();
}
