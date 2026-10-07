//! Owned String Loops v1: owned `string` values in `while` and `for` bodies
//! and the same-owner append `text = string_concat(text, …)`, executed on the
//! reference interpreter, generated C11 with an allocation-counting harness,
//! and the String-settling Core Wasm profile, plus the shapes that stay
//! refused with stable diagnostics.

use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

#[path = "../interpreter_internal_strings_v1/support.rs"]
pub(super) mod support;
use support::Fixture;

const SOURCE: &str = r#"
module test.owned_string_loops;

@id("loops.limit")
fn limit(value: i64) -> i64
    requires value < 7
{
    value
}

@id("loops.join")
fn join() -> i64
{
    let mut out = string_from_i64(0);
    let mut i = 1;
    while i < 1000 {
        out = string_concat(out, ",");
        out = string_concat(out, string_from_i64(i));
        i = i + 1;
        0
    }
    string_len(out)
}

@id("loops.literal")
fn literal() -> i64
{
    let mut out = "a";
    let mut log = "b";
    let mut i = 0;
    while i < 100 {
        out = string_concat(out, "xy");
        let note = "tmp";
        let hit = if i % 3 == 0 {
            log = string_concat(log, string_concat("[", "]"));
            1
        } else {
            0
        };
        let mut k = 0;
        while k < 2 {
            out = string_concat(out, "!");
            k = k + hit + 1 - hit;
            0
        }
        i = i + string_len(note) - 2;
        0
    }
    string_len(out) * 1000 + string_len(log)
}

@id("loops.overflow")
fn overflow() -> i64
{
    let mut out = "a";
    let mut other = "b";
    let mut big = 1;
    let mut i = 0;
    while i < 100 {
        out = string_concat(out, "xy");
        let note = "tmp";
        other = string_concat(other, note);
        big = big * 1000;
        i = i + 1;
        0
    }
    string_len(out) + string_len(other)
}

@id("loops.contract")
fn contract() -> i64
{
    let mut out = "c";
    let mut i = 0;
    while i < 10 {
        out = string_concat(out, "z");
        i = limit(i) + 1;
        0
    }
    string_len(out)
}

@id("loops.staged")
fn staged() -> i64
{
    let mut out = "s";
    let mut keep = "k";
    let mut i = 0;
    while i < 10 {
        out = string_concat(out, string_from_char(if limit(i) > 5 { 'b' } else { 'c' }));
        keep = string_concat(keep, "z");
        i = i + 1;
        0
    }
    string_len(out) + string_len(keep)
}

@id("loops.traverse")
fn traverse() -> i64
{
    let mut building = vec_with_capacity<i64>(3usize);
    building = vec_push<i64>(building, 7);
    building = vec_push<i64>(building, 42);
    building = vec_push<i64>(building, 1000);
    let values = building;
    let mut out = "v";
    for item in values {
        out = string_concat(out, ";");
        out = string_concat(out, string_from_i64(item));
        0
    }
    string_len(out)
}

@id("loops.condition_grow")
fn condition_grow() -> i64
{
    let mut out = "é";
    let mut i = 0;
    while string_len(out) < 6 {
        out = string_concat(out, "x");
        i = i + 1;
        0
    }
    string_len(out) * 10 + i
}

@id("loops.condition_stable")
fn condition_stable() -> i64
{
    let text = "abc";
    let mut i = 0;
    while i < 1000 && string_len(text) == 3 {
        i = i + 1;
        0
    }
    i
}

@id("loops.condition_skip")
fn condition_skip() -> i64
{
    let mut out = "a";
    while string_len(out) < 1 {
        out = string_concat(out, "x");
        0
    }
    7
}

@id("loops.condition_failure")
fn condition_failure() -> i64
{
    let text = "a";
    while string_len(text) + 9223372036854775807 > 0 {
        0
    }
    0
}

@id("loops.condition_nested")
fn condition_nested() -> i64
{
    let text = "a";
    while { while string_len(text) < 1 { 0 } false } { 0 }
    7
}

@id("app.main")
fn main() -> i64
{
    join()
}
"#;

/// Every case and its observation on every lane that admits it.
const CASES: &[(&str, &str)] = &[
    ("loops.join", "ok|3889"),
    ("loops.literal", "ok|401069"),
    ("loops.overflow", "semaprax.arithmetic.v1|3"),
    ("loops.contract", "semaprax.contract.v1|1"),
    ("loops.staged", "semaprax.contract.v1|1"),
    ("loops.traverse", "ok|11"),
    ("loops.condition_grow", "ok|64"),
    ("loops.condition_stable", "ok|1000"),
    ("loops.condition_skip", "ok|7"),
    ("loops.condition_nested", "ok|7"),
    ("loops.condition_failure", "semaprax.arithmetic.v1|1"),
];

/// The literal-only cases the String-settling Wasm profile admits; numeric
/// text and Vec values stay outside that closed profile.
const WASM_CASES: &[&str] = &[
    "loops.literal",
    "loops.overflow",
    "loops.contract",
    "loops.staged",
    "loops.condition_grow",
    "loops.condition_stable",
    "loops.condition_skip",
    "loops.condition_nested",
    "loops.condition_failure",
];

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn owned_string_loops_round_trip_and_move_the_appended_owner() {
    let program = parse(SOURCE, Path::new("owned-string-loops.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("owned-string-loops-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    // `out = string_concat(out, ",")` in `loops.join` moves the binding into
    // the call's first argument: the canonical plan transfers the `value`
    // storage of `out` at the operand and never allocates a clone there.
    let operand = "declaration:10:loops.join:expression:27:body.s2.body.s0.value.arg.0";
    let transfer = format!(
        "\"kind\":\"transfer\",\"at\":\"{operand}\",\"source\":{{\"kind\":\"cleanup_place\",\"storage\":{{\"kind\":\"value\""
    );
    assert!(graph.contains(&transfer), "{graph}");
    assert!(!graph.contains(&format!("\"kind\":\"initialize\",\"at\":\"{operand}\"")));
    assert!(graph.contains("\"callee\":\"core.string.concat\""));
    let mut resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let read = condition_operand(&mut resolved).id.clone();
    assert!(!graph.contains(&format!("\"kind\":\"initialize\",\"at\":\"{read}\"")));
    let inactive = semaprax::cleanup_plan::StorageId::Temporary(read.clone());
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loops.condition_skip")
        .unwrap();
    assert_eq!(
        function
            .cleanup_plan
            .regions
            .iter()
            .filter(|region| region.slots.contains(&inactive))
            .count(),
        1,
        "the inactive inventory temporary must retain one authenticated region"
    );
    assert!(graph.contains("\"callee\":\"core.string.len\""));
}

fn condition_operand(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "loops.condition_skip")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!()
    };
    let hir::ResolvedStatement::While { condition, .. } = &mut statements[1] else {
        panic!()
    };
    let hir::ResolvedExprKind::Binary { left, .. } = &mut condition.kind else {
        panic!()
    };
    let hir::ResolvedExprKind::Call { args, .. } = &mut left.kind else {
        panic!()
    };
    &mut args[0]
}

#[test]
fn string_length_conditions_reject_forged_operands_and_clone_plans() {
    let program = parse(SOURCE, Path::new("string-length-condition-hostile.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    for hostile in 0..3 {
        let mut candidate = resolved.clone();
        let operand = condition_operand(&mut candidate);
        match hostile {
            0 => operand.ownership = hir::OwnershipMode::Borrow,
            1 => operand.kind = hir::ResolvedExprKind::String("allocated".to_owned()),
            _ => {
                let hir::ResolvedExprKind::Place(place) = &mut operand.kind else {
                    panic!()
                };
                place
                    .projections
                    .push(hir::PlaceProjection::Field(hir::DeclarationId::new(
                        "foreign.field",
                    )));
            }
        }
        assert_eq!(hir::validate(&candidate).unwrap_err().code, "SPX-H006");
        assert_eq!(
            semaprax::codegen::emit_hir_c(&candidate).unwrap_err().code,
            "SPX-H006"
        );
    }
    let mut candidate = resolved;
    let read = condition_operand(&mut candidate).id.clone();
    let function = candidate
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "loops.condition_skip")
        .unwrap();
    let initialization = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(
                transition,
                semaprax::cleanup_plan::CleanupTransition::Initialize { .. }
            )
        })
        .unwrap();
    let semaprax::cleanup_plan::CleanupTransition::Initialize { at, .. } = initialization else {
        unreachable!()
    };
    *at = read;
    assert_eq!(hir::validate(&candidate).unwrap_err().code, "SPX-H006");
}

#[test]
fn reference_interpreter_builds_strings_in_loops() {
    let canonical = format::canonical(&parse(SOURCE, Path::new("owned-string-loops.spx")).unwrap());
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
fn native_string_loops_settle_every_allocation_on_every_exit() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("owned-string-loops-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    // The Vec carrier allocates with `calloc`, which the counting harness
    // does not observe; `loops.traverse` keeps its interpreter evidence.
    for (id, observation) in CASES.iter().filter(|(id, _)| *id != "loops.traverse") {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    size_t before=fixture_allocations;
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
    if(strcmp("{id}","loops.condition_stable")==0) REQUIRE(fixture_allocations-before==1);
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
fn string_settling_wasm_profile_releases_every_loop_owner() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("owned-string-loops-wasm.spx")).unwrap();
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

fn diagnostics(body: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let source =
        format!("module test.refused;\n\n@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n");
    let program = parse(&source, Path::new("refused.spx")).unwrap();
    verify::verify(&program)
}

#[test]
fn shapes_outside_owned_string_loops_v1_stay_refused() {
    // Whole replacement and an owner that is not the first operand need a
    // release at the assignment, which v1 does not lower.
    for body in [
        "    let mut text = \"x\";\n    text = \"y\";\n    string_len(text)",
        "    let mut text = \"x\";\n    text = string_concat(\"p\", text);\n    0",
    ] {
        let found = diagnostics(body);
        assert!(
            found.iter().any(|diagnostic| diagnostic.code == "SPX-U105"
                && diagnostic.message == "explicit mutation v1 supports only scalar Copy values"),
            "{found:?}"
        );
    }
    // A condition re-evaluates outside the per-iteration body region, so it
    // may create no String; named length inspection is allocation-free.
    for condition in [
        "string_len(string_concat(\"a\", \"b\")) < 9",
        "i < string_len(\"abc\")",
    ] {
        let found = diagnostics(&format!(
            "    let text = \"x\";\n    let mut i = 0;\n    while {condition} {{\n        i = i + 1;\n        0\n    }}\n    i"
        ));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "SPX-T252");
        assert_eq!(
            found[0].message,
            "string values are not admitted in while conditions; compute a scalar such as `string_len(text)` in the loop body and test that"
        );
    }
    // Consuming an outer String inside the body changes loop-carried
    // ownership. The source verifier checks every while body as an ordinary
    // block and reports the drift at the loop, before HIR, any cleanup plan,
    // or any backend exists.
    let found = diagnostics(
        "    let text = \"x\";\n    let mut i = 0;\n    while i < 2 {\n        let moved = string_concat(text, \"y\");\n        i = i + string_len(moved);\n        0\n    }\n    i",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].code, "SPX-T252");
    assert_eq!(
        found[0].message,
        "ownership of `text` changes inside a while loop, which is not yet admitted"
    );
}

#[test]
fn string_length_conditions_keep_allocating_and_consuming_shapes_refused() {
    for condition in [
        "string_len(\"a\") < 2",
        "string_len(string_concat(text, \"b\")) < 2",
        "string_len({ text }) < 2",
        "string_is_empty(text)",
        "match 0 { n if string_len(\"guard\") > n => true, _ => false, }",
        "{ while false { string_len(text) } false }",
        "{ while false { while string_len(text) < 1 { 0 } 0 } false }",
    ] {
        let found = diagnostics(&format!(
            "    let text = \"a\";\n    while {condition} {{ 0 }}\n    0"
        ));
        assert!(found.iter().any(|diagnostic| diagnostic.code == "SPX-T252"
            && diagnostic.message == "string values are not admitted in while conditions; compute a scalar such as `string_len(text)` in the loop body and test that"), "{condition}: {found:?}");
    }
}

#[test]
fn outer_string_loop_ownership_mutation_reports_exact_source_location() {
    let source = "module test.outer_string_loop_mutation;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    let text = \"x\";\n    let mut i = 0;\n    while i < 2 {\n        let moved = string_concat(text, \"y\");\n        i = i + string_len(moved);\n        0\n    }\n    i\n}\n";
    let path = Path::new("outer-string-loop-mutation.spx");
    let program = parse(source, path).unwrap();
    let found = verify::verify(&program);
    assert_eq!(found.len(), 1, "{found:?}");
    let diagnostic = &found[0];
    assert_eq!(diagnostic.code, "SPX-T252");
    assert_eq!(
        diagnostic.message,
        "ownership of `text` changes inside a while loop, which is not yet admitted"
    );
    assert_eq!(diagnostic.path.as_deref(), path.to_str());
    // The existing while expression span starts at its condition and includes
    // its body, locating the ownership drift rather than the outer declaration.
    assert_eq!(
        diagnostic.span,
        Some(semaprax::ast::Span {
            start: source.find("i < 2").unwrap(),
            end: source.find("\n    i\n}").unwrap(),
            line: 8,
            column: 11,
        })
    );
}
