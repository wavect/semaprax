//! Additive internal String record ownership, without a public byte ABI.
use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};
use serde_json::Value;

use super::owned_string_loops_v1::support::Fixture;

const SOURCE: &str = r#"module test.owned_string_records;

@id("task")
record Task {
    @id("task.title")
    title: string,
    @id("task.label")
    label: string,
    @id("task.points")
    points: i64,
}

@id("task.make")
fn make() -> Task {
    Task { title: "a\u{0}é", label: "x", points: 7 }
}

@id("task.measure")
fn measure(item: borrow Task) -> i64 {
    string_len(item.title) + string_len(item.label) + item.points
}

@id("task.consume")
fn consume(item: own Task) -> i64 {
    match own item { Task { title: text, label: tag, points: points } => string_len(text) + string_len(tag) + points, }
}

@id("task.forward")
fn forward(item: own Task) -> Task {
    item
}

@id("task.extract-title")
fn title(item: own Task) -> string {
    match own item { Task { title: text, label: tag, points: points } => text, }
}

@id("task.sink")
fn sink(item: own Task, tail: i64) -> i64 {
    consume(item) + tail
}

@id("task.reject")
fn reject(item: own Task) -> i64
    requires item.points < 0
{
    consume(item)
}

@id("task.bad-post")
fn bad_post() -> Task
    ensures result.points == 0
{
    make()
}

@id("case.constructor-failure")
fn constructor_failure() -> i64 {
    let item = Task { title: "prefix", label: string_concat("staged", "owner"), points: 1 / 0 };
    consume(item)
}

@id("case.argument-failure")
fn argument_failure() -> i64 {
    sink(make(), 1 / 0)
}

@id("case.precondition-failure")
fn precondition_failure() -> i64 {
    reject(make())
}

@id("case.postcondition-failure")
fn postcondition_failure() -> i64 {
    consume(bad_post())
}

@id("app.main")
fn main() -> i64 {
    let item = forward(make());
    let before = measure(item);
    let updated = item with { title: "wide" };
    let after = measure(updated);
    consume(updated) + before + after + string_len(title(make()))
}
"#;

fn program() -> semaprax::ast::Program {
    let parsed = parse(SOURCE, Path::new("owned-string-records.spx")).unwrap();
    assert!(verify::verify(&parsed).is_empty());
    parsed
}

#[test]
fn owned_string_records_round_trip_and_authenticate_leaf_projections() {
    let parsed = program();
    let canonical = format::canonical(&parsed);
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&parsed), graph::revision(&reparsed));
    let projected = graph::to_json(&parsed).unwrap();
    assert_eq!(projected, graph::to_json(&reparsed).unwrap());
    for field in ["task.title", "task.label"] {
        assert!(projected.contains(field));
    }
    assert!(projected.contains("core.string.drop"));
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let update = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(
        update.cleanup_plan.schema,
        semaprax::cleanup_plan::CLEANUP_PLAN_SCHEMA_V9
    );
}

#[test]
fn owned_string_record_refusals_and_hostile_hir_remain_compile_time_errors() {
    let parsed = program();
    let resolved = hir::resolve(&parsed).unwrap();
    let mut forged = resolved.clone();
    forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "task.measure")
        .unwrap()
        .params[0]
        .ownership = hir::OwnershipMode::Value;
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    let mut forged = resolved.clone();
    let consume = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "task.consume")
        .unwrap();
    consume
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| exit.finalize_in_order.len() >= 2)
        .expect("two String leaves settle in canonical order")
        .finalize_in_order
        .reverse();
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    for (source, diagnostic) in [
        (
            SOURCE.replace("item: borrow Task", "item: Task"),
            "SPX-O001",
        ),
        (
            SOURCE.replace(
                "consume(updated) + before + after",
                "consume(updated) + consume(updated) + before + after",
            ),
            "SPX-O101",
        ),
        (
            SOURCE.replace("let updated = item with", "let updated = make() with"),
            "SPX-O117",
        ),
    ] {
        let candidate = parse(&source, Path::new("refused.spx")).unwrap();
        let diagnostics = verify::verify(&candidate);
        assert!(
            diagnostics.iter().any(|d| d.code == diagnostic),
            "{diagnostics:?}"
        );
        assert!(diagnostics.iter().all(|d| d.code != "SPX-H006"));
    }
}

#[test]
fn owned_string_records_interpreter_settles_success_and_sticky_failures() {
    program();
    let fixture = Fixture::new(SOURCE);
    for _ in 0..4 {
        let report = interpreter::internal_strings::interpret(
            &fixture.source,
            "app.main",
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap();
        let envelope: Value = serde_json::from_str(&report.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], "40");
        interpreter::internal_strings::verify_envelope_against_source(
            &report.envelope,
            &fixture.source,
        )
        .unwrap();
    }
    for id in [
        "case.constructor-failure",
        "case.argument-failure",
        "case.precondition-failure",
        "case.postcondition-failure",
    ] {
        let report = interpreter::internal_strings::interpret(
            &fixture.source,
            id,
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!report.returned);
        interpreter::internal_strings::verify_envelope_against_source(
            &report.envelope,
            &fixture.source,
        )
        .unwrap();
    }
    fixture.cleanup();
}

fn symbol(id: &str) -> String {
    format!(
        "spx_decl_{}",
        id.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[test]
fn owned_string_records_native_release_every_leaf_after_each_call() {
    let parsed = program();
    let generated = codegen::emit_c(&parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(&parsed).unwrap());
    let mut probe = format!("{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c"));
    probe.push_str(&format!("for(unsigned i=0;i<4;++i) {{ int64_t value=INT64_MIN; REQUIRE({}(&context,&value)==0); REQUIRE(value==40); REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees); }}\n", symbol("app.main")));
    for id in [
        "case.constructor-failure",
        "case.argument-failure",
        "case.precondition-failure",
        "case.postcondition-failure",
    ] {
        probe.push_str(&format!("{{ int64_t value=INT64_MIN; spx_status_token token={}(&context,&value); REQUIRE(token!=0); REQUIRE(value==INT64_MIN); REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees); }}\n", symbol(id)));
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), "");
    }
    fixture.cleanup();
}

#[test]
fn owned_string_records_wasm_release_every_leaf_between_entry_calls() {
    let parsed = program();
    let fixture = Fixture::new(SOURCE);
    let root = fixture.root.join("web");
    wasm::build_web(&parsed, &root).unwrap();
    std::fs::write(root.join("probe.mjs"), r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:16});
for(let i=0;i<8;i++) if(instance.exports.semaprax_main()!==40n) throw Error('String record result or settlement changed');
"#).unwrap();
    let output = Command::new("node")
        .arg(root.join("probe.mjs"))
        .current_dir(&root)
        .output()
        .expect("Node is required for the owned String record Wasm gate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let names = [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ];
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), names.len());
    for name in names {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    fixture.cleanup();
}

#[test]
fn aggregate_wasm_string_compare_uses_unsigned_utf8_bytes_and_settles_clones() {
    let source = r#"module test.wasm_string_order;
@id("marker") record Marker { @id("marker.code") code: i64, }
@id("app.main") fn main() -> i64 {
    let marker = Marker { code: 0 };
    let a = "a\u{0}";
    let b = "a";
    let unicode = "é";
    let ascii = "z";
    let empty = "";
    marker.code + if string_compare(a, b) == 1 { 1 } else { 100 }
        + if string_compare(unicode, ascii) == 1 { 1 } else { 100 }
        + if string_compare("a", "b") == -1 { 1 } else { 100 }
        + if string_compare(empty, "") == 0 { 1 } else { 100 }
}
"#;
    let parsed = parse(source, Path::new("wasm-string-order.spx")).unwrap();
    assert!(verify::verify(&parsed).is_empty());
    let fixture = Fixture::new(source);
    let root = fixture.root.join("web");
    wasm::build_web(&parsed, &root).unwrap();
    std::fs::write(root.join("probe.mjs"), r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const module=await WebAssembly.compile(bytes);
const imports=WebAssembly.Module.imports(module);
if(imports.filter(entry=>entry.name==='spx_string_compare_v2').length!==1) throw Error('comparison must select one optional import');
const {instance}=await instantiateBytes(bytes,{maxOwnedByteEntries:16});
for(let i=0;i<8;i++) if(instance.exports.semaprax_main()!==4n) throw Error('bytewise String ordering or settlement changed');
"#).unwrap();
    let output = Command::new("node")
        .arg(root.join("probe.mjs"))
        .current_dir(&root)
        .output()
        .expect("Node is required for the aggregate String comparator gate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let names = [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ];
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), names.len());
    for name in names {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    fixture.cleanup();
}

#[test]
fn owned_string_record_layout_cannot_silently_drop_record_invariants() {
    let declaration = r#"module invariant.record; @id("packet") record Packet {@id("packet.label") label:string,@id("packet.quantity") quantity:i64,} requires quantity>0"#;
    let schema = format!("{declaration} @id(\"main\") fn main()->i64{{0}}");
    let ast = semaprax::parse(&schema, "invariant-schema.spx").unwrap();
    assert!(semaprax::verify::verify(&ast).is_empty());
    let document = semaprax::graph::to_json(&ast).unwrap();
    assert!(document.contains("packet#invariant"));
    let executable =
        format!("{declaration} fn pass(value:own Packet)->Packet{{value}} fn main()->i64{{0}}");
    let ast = semaprax::parse(&executable, "invariant-executable.spx").unwrap();
    assert!(semaprax::verify::verify(&ast)
        .iter()
        .any(|error| error.code == "SPX-T309"));
    assert!(semaprax::hir::resolve(&ast).is_err());
}
