use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup_plan::CleanupPlan;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const CORPUS: &str = r#"
module test.field_mutation;

@id("fm.point")
record Point {
    @id("fm.point.x")
    x: i64,
    @id("fm.point.y")
    y: i64,
    @id("fm.point.on")
    on: bool,
}

@id("fm.cell")
record Cell {
    @id("fm.cell.small")
    small: i32,
}

@id("fm.counter")
class Counter {
    @id("fm.counter.value")
    value: i64,

    @id("fm.counter.get")
    fn get(self: Counter) -> i64
{
        self.value
    }

    @id("fm.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter
{
        Counter { value: self.value + amount }
    }
}

@id("fm.mutate_records")
fn mutate_records(flag: bool) -> i64 {
    let mut point = Point { x: 1, y: 2, on: false };
    point.x = point.x + 41;
    point.y = point.y + point.x;
    let mut branch = Point { x: 0, y: 0, on: false };
    let delta = if flag {
        branch.x = 30;
        branch.x
    } else {
        branch.y = 7;
        0 - branch.y
    };
    if point.on { delta } else { point.x + point.y + delta }
}

@id("fm.mutate_class")
fn mutate_class() -> i64 {
    let mut counter = Counter { value: 3 };
    counter.value = counter.value * counter.value;
    let doubled = counter.bumped(counter.get());
    doubled.get()
}

@id("app.main")
fn main() -> i64 { mutate_records(true) + mutate_class() }
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

const OVERFLOW_MAIN: &str = r#"
module test.field_overflow;

@id("fm.cell")
record Cell {
    @id("fm.cell.small")
    small: i32,
}

@id("app.main")
fn main() -> i64 {
    let mut cell = Cell { small: 2147483647i32 };
    cell.small = cell.small + 1i32;
    if cell.small < 0i32 { 1 } else { 0 }
}
"#;

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("field.spx")).unwrap();
    verify::verify(&program)
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn field_programs_round_trip_canonically_and_hash_stably() {
    let program = parse(CORPUS, Path::new("field.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("let mut point = Point { x: 1, y: 2, on: false };"),
        "field targets keep their `let mut` bindings in canonical form: {canonical}"
    );
    assert!(
        canonical.contains("point.x = point.x + 41;"),
        "direct field assignments render in place: {canonical}"
    );
    assert!(
        canonical.contains("counter.value = counter.value * counter.value;"),
        "class field assignments render in place: {canonical}"
    );
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn field_assignment_is_statement_only_and_cannot_chain_or_nest() {
    // Assignment inside an expression position is still a parse error.
    let error = parse(
        "module t;\n@id(\"m\")\nfn m() -> i64 { let mut x = 1; (x = 2) }",
        Path::new("expr.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-P106", "no assignment expression exists");

    // Field reads stay expressions: only `=` turns them into statements.
    let program = parse(
        r#"
module t.chain;

@id("t.inner")
record Inner {
    @id("t.inner.v")
    v: i64,
}

@id("t.outer")
record Outer {
    @id("t.outer.in")
    inn: Inner,
}

@id("m")
fn m() -> i64 {
    let o = Outer { inn: Inner { v: 7 } };
    o.inn.v + 1
}

@id("app.main")
fn main() -> i64 { m() }
"#,
        Path::new("read.spx"),
    )
    .unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "projections read cleanly"
    );
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("o.inn.v + 1"),
        "projection chains stay pure expressions: {canonical}"
    );
}

#[test]
fn assigning_through_an_immutable_binding_is_spx_u107() {
    // A plain `let` record local rejects field stores.
    let report = diagnostics(
        r#"
module test.field_immutable_let;

@id("fm.p")
record P {
    @id("fm.p.x")
    x: i64,
}

@id("app.main")
fn main() -> i64 {
    let frozen = P { x: 1 };
    frozen.x = 2;
    frozen.x
}
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-U107")
        .expect("immutable field target must be rejected");
    assert!(
        diagnostic.message.contains("`frozen`"),
        "the diagnostic names the immutable binding: {diagnostic}"
    );

    // Parameters are immutable, including class receivers.
    let parameter_report = diagnostics(
        r#"
module test.field_immutable_param;

@id("fm.c")
class C {
    @id("fm.c.v")
    v: i64,
}

@id("m")
fn m(c: C) -> i64 {
    c.v = 3;
    c.v
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    assert!(
        parameter_report.iter().any(|item| item.code == "SPX-U107"),
        "parameters reject field stores: {parameter_report:?}"
    );

    // Method receivers stay closed: mutating through `self` is rejected by
    // the resolver (source-level verification does not descend into method
    // bodies in this slice, but resolution fails closed before any backend).
    let self_report = hir::resolve(
        &parse(
            r#"
module test.field_self;

@id("fm.self_counter")
class SelfCounter {
    @id("fm.self_counter.ticks")
    ticks: i64,

    @id("fm.self_counter.tick")
    fn tick(self: SelfCounter) -> i64
{
        self.ticks = self.ticks + 1;
        self.ticks
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
            Path::new("self.spx"),
        )
        .unwrap(),
    )
    .expect_err("self field mutation must be rejected");
    assert!(
        self_report.iter().any(|item| item.code == "SPX-U107"),
        "`self` mutation stays closed: {self_report:?}"
    );
}

#[test]
fn unknown_fields_are_spx_u108() {
    let report = diagnostics(
        r#"
module test.field_unknown;

@id("fm.q")
record Q {
    @id("fm.q.a")
    a: i64,
}

@id("m")
fn m() -> i64 {
    let mut q = Q { a: 1 };
    q.missing = 3;
    q.a
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-U108")
        .expect("unknown fields must be rejected");
    assert!(
        diagnostic.message.contains("missing"),
        "the diagnostic names the unknown field: {diagnostic}"
    );
}

#[test]
fn non_scalar_fields_are_spx_u109() {
    // Whole-field replacement of an aggregate-typed field stays closed.
    let report = diagnostics(
        r#"
module test.field_aggregate;

@id("fm.inner")
record Inner {
    @id("fm.inner.v")
    v: i64,
}

@id("fm.outer")
record Outer {
    @id("fm.outer.in")
    inn: Inner,
}

@id("m")
fn m() -> i64 {
    let mut o = Outer { inn: Inner { v: 1 } };
    o.inn = Inner { v: 2 };
    o.inn.v
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-U109"),
        "only scalar Copy fields are admitted: {report:?}"
    );
}

#[test]
fn field_value_type_mismatch_is_spx_u110() {
    let report = diagnostics(
        r#"
module test.field_type_mismatch;

@id("fm.s")
record S {
    @id("fm.s.a")
    a: i64,
    @id("fm.s.b")
    b: i32,
}

@id("m")
fn m() -> i64 {
    let mut s = S { a: 1, b: 2i32 };
    s.b = 7;
    s.a
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-U110")
        .expect("exact field-type match is required");
    assert!(
        diagnostic
            .message
            .contains("does not exactly match field type"),
        "the mismatch diagnostic speaks about the field type: {diagnostic}"
    );
}

#[test]
fn nested_place_chains_are_rejected_with_spx_u111() {
    let error = parse(
        r#"
module test.field_nested;

@id("fm.nested_inner")
record NestedInner {
    @id("fm.nested_inner.v")
    v: i64,
}

@id("fm.nested_outer")
record NestedOuter {
    @id("fm.nested_outer.in")
    inn: NestedInner,
}

@id("m")
fn m() -> i64 {
    let mut o = NestedOuter { inn: NestedInner { v: 1 } };
    o.inn.v = 5;
    o.inn.v
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
        Path::new("nested.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-U111");
}

#[test]
fn field_mutation_of_non_records_is_spx_u112() {
    let report = diagnostics(
        r#"
module test.field_scalar_base;

@id("m")
fn m() -> i64 {
    let mut n = 1;
    n.field = 2;
    n
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-U112")
        .expect("scalar bases reject field stores");
    assert!(
        diagnostic.message.contains("non-record"),
        "the diagnostic explains the base must be a record: {diagnostic}"
    );
}

#[test]
fn graph_serialization_names_fields_additively_and_stays_deterministic() {
    let program = parse(CORPUS, Path::new("field-graph.spx")).unwrap();

    // Deterministic serialization across repeated renders.
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);

    // Assign nodes keep their additive shape: every store in this corpus is
    // a direct field store carrying exactly one `field` attribute naming the
    // targeted field's stable id.
    let wire_value: serde_json::Value = serde_json::from_str(&first).unwrap();
    fn collect_assignments(value: &serde_json::Value, out: &mut Vec<(String, String)>) {
        if let Some(object) = value.as_object() {
            if object.get("kind") == Some(&serde_json::Value::from("assign")) {
                out.push((
                    object["target"]["id"].as_str().unwrap().to_owned(),
                    object
                        .get("field")
                        .and_then(|field| field.as_str())
                        .expect("corpus stores are all field stores")
                        .to_owned(),
                ));
            }
            for child in object.values() {
                collect_assignments(child, out);
            }
        } else if let Some(array) = value.as_array() {
            for child in array {
                collect_assignments(child, out);
            }
        }
    }
    let mut assignments = Vec::new();
    collect_assignments(&wire_value, &mut assignments);
    assert_eq!(assignments.len(), 5);
    let mut field_counts = std::collections::BTreeMap::new();
    for (_, field) in &assignments {
        *field_counts.entry(field.as_str()).or_insert(0usize) += 1;
    }
    assert_eq!(field_counts.get("fm.point.x"), Some(&2));
    assert_eq!(field_counts.get("fm.point.y"), Some(&2));
    assert_eq!(field_counts.get("fm.counter.value"), Some(&1));

    // Schema selection ignores field-mutation-only programs.
    let wire = serde_json::from_str::<serde_json::Value>(&first).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v10");
}

#[test]
fn non_field_graph_bytes_are_pinned_to_pre_feature_output() {
    // This digest was captured from a build before Explicit Mutation v1 and
    // still holds: plain programs serialize byte-for-byte identically, which
    // proves the field attribute never leaks into old graphs.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(!json.contains("\"kind\":\"assign\""));
    use sha2::{Digest, Sha256};
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(json.as_bytes()))
    );
    assert_eq!(
        digest,
        "sha256:6fe42635e96022507876aabd25acfe06f28521aba50132a5dc16b5070c45cfa7"
    );

    // An aggregate program without field stores also carries no assign nodes.
    let aggregate_program = parse(
        r#"
module test.field_plain_aggregate;

@id("fm.plain_point")
record PlainPoint {
    @id("fm.plain_point.px")
    px: i64,
}

@id("m")
fn m() -> i64 {
    let p = PlainPoint { px: 3 };
    p.px
}

@id("app.main")
fn main() -> i64 { m() }
"#,
        Path::new("plain-aggregate.spx"),
    )
    .unwrap();
    let aggregate_json = graph::to_json(&aggregate_program).unwrap();
    assert!(!aggregate_json.contains("\"kind\":\"assign\""));
    assert!(!aggregate_json.contains("\"mutable\":true"));
}

#[test]
fn straight_line_field_mutation_adds_no_cleanup_structure() {
    let source = r#"
module test.field_cleanup;

@id("fm.cleanup_point")
record CleanupPoint {
    @id("fm.cleanup_point.cx")
    cx: i64,
    @id("fm.cleanup_point.cy")
    cy: i64,
}

@id("clean.with_fields")
fn with_fields() -> i64 {
    let mut p = CleanupPoint { cx: 1, cy: 2 };
    p.cx = p.cx + 2;
    p.cy = p.cy * 3;
    p.cx + p.cy
}

@id("clean.initializers_only")
fn initializers_only() -> i64 {
    let p0 = CleanupPoint { cx: 1, cy: 2 };
    let x1 = p0.cx + 2;
    let y1 = p0.cy * 3;
    x1 + y1
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("field-cleanup.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let function = |id: &str| {
        resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing `{id}`"))
            .cleanup_plan
            .clone()
    };
    let mutated: CleanupPlan = function("clean.with_fields");
    let plain: CleanupPlan = function("clean.initializers_only");

    // Identical shape: field stores lower their RHS exactly like the
    // initializer-only equivalent and add no regions, exits, blocks, or
    // slots.
    assert_eq!(mutated.blocks.len(), plain.blocks.len());
    assert_eq!(mutated.regions.len(), plain.regions.len());
    assert_eq!(mutated.exits.len(), plain.exits.len());
    assert_eq!(mutated.slots.len(), plain.slots.len());
    for plan in [&mutated, &plain] {
        assert!(plan.slots.is_empty(), "Copy records own nothing");
        for exit in &plan.exits {
            assert!(
                exit.finalize_in_order.is_empty(),
                "Copy-record plans never finalize"
            );
        }
    }
    // Plans embed function-qualified ids, so compare modulo those names.
    let normalize = |plan: &CleanupPlan| {
        let rendered = format!("{plan:?}")
            .replace("clean.with_fields", "FN")
            .replace("clean.initializers_only", "FN");
        let mut normalized = String::with_capacity(rendered.len());
        let mut rest = rendered.as_str();
        while let Some(position) = rest.find("declaration:") {
            let split = position + "declaration:".len();
            normalized.push_str(&rest[..split]);
            rest = &rest[split..];
            let digits = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
            rest = &rest[digits..];
        }
        normalized.push_str(rest);
        normalized
    };
    assert_eq!(
        normalize(&mutated),
        normalize(&plain),
        "straight-line field mutation keeps CleanupPlan output unchanged"
    );
}

#[test]
fn native_field_mutation_executes_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(CORPUS, Path::new("field-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t records_true = UINT64_C(0);
    if ({records}(&context, UINT8_C(1), &records_true) != SPX_STATUS_SUCCESS || records_true != INT64_C(116)) return 11;
    int64_t records_false = UINT64_C(0);
    if ({records}(&context, UINT8_C(0), &records_false) != SPX_STATUS_SUCCESS || records_false != INT64_C(79)) return 12;
    int64_t class_value = UINT64_C(0);
    if ({class_fn}(&context, &class_value) != SPX_STATUS_SUCCESS || class_value != INT64_C(18)) return 13;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(134)) return 14;
    return 0;
}}
"#,
        records = symbol("fm.mutate_records"),
        class_fn = symbol("fm.mutate_class"),
        main_fn = symbol("app.main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-field-native-{}-{id}", std::process::id());
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
        let status = executed.status.code();
        let stderr = String::from_utf8_lossy(&executed.stderr).into_owned();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "native failed at {optimization}: status={status:?} stderr={stderr}"
        );
    }
}

#[test]
fn wasm_field_mutation_matches_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(CORPUS, Path::new("field-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root =
        std::env::temp_dir().join(format!("semaprax-field-wasm-{}-{id}", std::process::id()));
    let native = std::env::temp_dir().join(format!(
        "semaprax-field-wasm-{}-{id}.native{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    codegen::build(&program, &native).unwrap();
    let native_output = Command::new(&native).output().unwrap();
    let _ = std::fs::remove_file(&native);
    assert!(native_output.status.success(), "native corpus run failed");
    assert_eq!(String::from_utf8_lossy(&native_output.stdout).trim(), "134");

    wasm::build_web(&program, &root).unwrap();
    let node = Command::new("node")
        .arg("scripts/verify-web.mjs")
        .arg(&root)
        .arg("134")
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        node.status.success(),
        "Node field mutation failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&node.stdout).trim(), "134");
}

#[test]
fn wasm_checked_field_overflow_traps_instead_of_wrapping() {
    if !command_available("node") {
        return;
    }
    let program = parse(OVERFLOW_MAIN, Path::new("field-wasm-overflow.spx")).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-field-wasm-overflow-{}-{id}",
        std::process::id()
    ));
    wasm::build_web(&program, &root).unwrap();
    let node = Command::new("node")
        .arg("scripts/verify-web.mjs")
        .arg(&root)
        .arg("error:SEMAPRAX checked arithmetic failure")
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        node.status.success(),
        "Node field overflow probe failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&node.stdout).trim(),
        "error:SEMAPRAX checked arithmetic failure"
    );
}

#[test]
fn native_checked_field_overflow_selects_a_failure_status() {
    if !command_available("clang") {
        return;
    }
    let program = parse(OVERFLOW_MAIN, Path::new("field-native-overflow.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    let main_fn = format!("spx_decl_{}", hex_identity("app.main"));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t out = 0;
    if ({main_fn}(&context, &out) == SPX_STATUS_SUCCESS) return 11;
    return 0;
}}
"#
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-field-native-overflow-{}-{id}", std::process::id());
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
            "overflow C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).status().unwrap();
        let _ = std::fs::remove_file(&source_path);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.success(),
            "field-assigned overflow must surface a failure status at {optimization}"
        );
    }
}

fn hex_identity(value: &str) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}
