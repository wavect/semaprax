use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup_plan::CleanupPlan;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const BOUNDED: &str = r#"
module test.unsafe_boundaries;

permit { unsafe }

@id("ub.accumulate")
fn accumulate() -> i64 {
    let mut total = 1;
    @audit("boundary review 2026-08: arithmetic only")
    unsafe {
        total = total + 41;
        0
    }
    @audit("nested boundary re-entry")
    unsafe {
        let step = 100;
        total = total + step;
        0
    }
    total
}

@id("main")
fn main() -> i64 { accumulate() }
"#;

const WITHOUT_PERMIT: &str = r#"
module test.unsafe_no_permit;

@id("app.main")
fn main() -> i64 {
    @audit("missing capability declaration")
    unsafe {
        0
    }
    0
}
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

fn diagnostics(source: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let program = parse(source, Path::new("unsafe.spx")).unwrap();
    verify::verify(&program)
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

#[test]
fn unsafe_programs_round_trip_canonically_and_verify_cleanly_with_permit() {
    let program = parse(BOUNDED, Path::new("unsafe.spx")).unwrap();
    assert!(
        verify::verify(&program).is_empty(),
        "`permit {{ unsafe }}` admits audited boundary statements"
    );
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("@audit(\"boundary review 2026-08: arithmetic only\") unsafe {"),
        "canonical form keeps the verbatim audit summary: {canonical}"
    );
    assert!(
        canonical.contains("@audit(\"nested boundary re-entry\") unsafe {"),
        "every boundary keeps its own summary: {canonical}"
    );
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn missing_capability_declaration_is_spx_n101() {
    let report = diagnostics(WITHOUT_PERMIT);
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-N101")
        .expect("unsafe blocks require the module capability declaration");
    assert!(
        diagnostic.message.contains("permit { unsafe }"),
        "the diagnostic names the exact required declaration: {diagnostic}"
    );

    // Adding exactly the mirrored module permit clears the diagnostic.
    let permitted = WITHOUT_PERMIT.replace(
        "module test.unsafe_no_permit;",
        "module test.unsafe_no_permit;\n\npermit { unsafe }",
    );
    assert!(diagnostics(&permitted).is_empty());
}

#[test]
fn missing_audit_annotation_is_spx_n102() {
    let error = parse(
        "module t;\npermit { unsafe }\n@id(\"m\")\nfn m() -> i64 { unsafe { 0 } }",
        Path::new("no-audit.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-N102");

    // An unknown attribute in boundary position stays rejected.
    let unknown = parse(
        "module t;\npermit { unsafe }\n@id(\"m\")\nfn m() -> i64 { @id(\"x\") unsafe { 0 } }",
        Path::new("bad-attr.spx"),
    )
    .unwrap_err();
    assert_eq!(unknown.code, "SPX-P102");
}

#[test]
fn empty_or_malformed_audit_summary_is_spx_n103() {
    let error = parse(
        "module t;\npermit { unsafe }\n@id(\"m\")\nfn m() -> i64 { @audit(\"\") unsafe { 0 } }",
        Path::new("empty-audit.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-N103");

    let non_string = parse(
        "module t;\npermit { unsafe }\n@id(\"m\")\nfn m() -> i64 { @audit(7) unsafe { 0 } }",
        Path::new("num-audit.spx"),
    )
    .unwrap_err();
    assert_eq!(non_string.code, "SPX-N103");
}

#[test]
fn nonscalar_body_results_are_spx_n104() {
    let report = diagnostics(
        r#"
module test.unsafe_record_body;

permit { unsafe }

record Point {
    x: i64,
    y: i64,
}

@id("app.main")
fn main() -> i64 {
    let point = Point { x: 1, y: 2 };
    @audit("record-valued boundary body")
    unsafe {
        point with { x: 3 }
    }
    point.x
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-N104"),
        "boundary bodies must produce scalar Copy values in v1: {report:?}"
    );
}

#[test]
fn unsafe_statements_in_contracts_are_spx_n105() {
    let report = diagnostics(
        r#"
module test.unsafe_contract;

permit { unsafe }

@id("c.check")
fn check(value: i64) -> i64
requires {{
    @audit("contract purity probe")
    unsafe {
        0
    }
    value > 0
}}
{ value }
@id("app.main")
fn main() -> i64 { check(1) }
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-N105"),
        "contract expressions remain pure: {report:?}"
    );
}

#[test]
fn graph_serialization_adds_explicit_unsafe_boundary_nodes() {
    let program = parse(BOUNDED, Path::new("unsafe.spx")).unwrap();

    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);

    assert!(
        first.contains("\"kind\":\"unsafe\""),
        "boundaries serialize as explicit unsafe nodes: {first}"
    );
    assert!(
        first.contains("\"audit\":\"boundary review 2026-08: arithmetic only\""),
        "the audit summary is recorded verbatim: {first}"
    );
    assert_eq!(
        first.matches("\"kind\":\"unsafe\"").count(),
        2,
        "each boundary statement becomes exactly one node"
    );

    let wire_value: serde_json::Value = serde_json::from_str(&first).unwrap();
    fn collect_audits(value: &serde_json::Value, audits: &mut Vec<String>) {
        if let Some(object) = value.as_object() {
            if object.get("kind") == Some(&serde_json::Value::from("unsafe")) {
                audits.push(object["audit"].as_str().unwrap().to_owned());
            }
            for child in object.values() {
                collect_audits(child, audits);
            }
        } else if let Some(array) = value.as_array() {
            for child in array {
                collect_audits(child, audits);
            }
        }
    }
    let mut audits = Vec::new();
    collect_audits(&wire_value, &mut audits);
    assert_eq!(audits.len(), 2);
    assert!(audits.contains(&"nested boundary re-entry".to_owned()));

    // Boundary mechanics select no new schema level.
    assert_eq!(wire_value["schema"], "semaprax.graph.v10");
}

#[test]
fn non_unsafe_graph_bytes_are_pinned_to_pre_feature_output() {
    // This digest was captured from a build before Unsafe Boundary Mechanics
    // v1 (originally pinned by Explicit Mutation v1) and must never drift:
    // programs without boundary syntax serialize byte-for-byte identically.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(!json.contains("\"kind\":\"unsafe\""));
    assert!(!json.contains("\"audit\":"));
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
fn straight_line_boundaries_add_no_cleanup_structure() {
    let source = r#"
module test.unsafe_cleanup;

permit { unsafe }

@id("clean.with_boundary")
fn with_boundary() -> i64 {
    let mut total = 1;
    @audit("cleanup shape probe")
    unsafe {
        total = total + 2;
        0
    }
    total
}

@id("clean.block_equivalent")
fn block_equivalent() -> i64 {
    let mut total = 1;
    let sink = {
        total = total + 2;
        0
    };
    total
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("unsafe-cleanup.spx")).unwrap();
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
    let bounded: CleanupPlan = function("clean.with_boundary");
    let plain: CleanupPlan = function("clean.block_equivalent");

    assert_eq!(bounded.blocks.len(), plain.blocks.len());
    assert_eq!(bounded.regions.len(), plain.regions.len());
    assert_eq!(bounded.exits.len(), plain.exits.len());
    assert_eq!(bounded.slots.len(), plain.slots.len());

    for plan in [&bounded, &plain] {
        assert!(plan.slots.is_empty());
        for exit in &plan.exits {
            assert!(
                exit.finalize_in_order.is_empty(),
                "scalar plans never finalize"
            );
        }
    }
    // The boundary lowers exactly like its ordinary block-equivalent body:
    // plans embed function-qualified expression paths, so compare modulo
    // those names.
    let normalize = |plan: &CleanupPlan| {
        format!("{plan:?}")
            .replace("clean.with_boundary", "FN")
            .replace("clean.block_equivalent", "FN")
            .replace("declaration:19:", "declaration:N:")
            .replace("declaration:22:", "declaration:N:")
            .replace("FN:expression:21:", "FN:EXPR:")
            .replace("FN:expression:22:", "FN:EXPR:")
            .replace("body.s1.body.s0.value", "PROBE")
            .replace("body.s1.value.s0.value", "PROBE")
    };
    assert_eq!(
        normalize(&bounded),
        normalize(&plain),
        "the boundary adds no cleanup structure over the block equivalent"
    );
}

#[test]
fn native_unsafe_boundaries_execute_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(BOUNDED, Path::new("unsafe-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t accumulated = UINT64_C(0);
    if ({accumulate}(&context, &accumulated) != SPX_STATUS_SUCCESS || accumulated != INT64_C(142)) return 11;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(142)) return 12;
    return 0;
}}
"#,
        accumulate = symbol("ub.accumulate"),
        main_fn = symbol("main"),
    );
    run_native_probe(&generated, &probe, "unsafe boundaries");
}

#[test]
fn native_unsafe_checked_overflow_selects_a_failure_status() {
    if !command_available("clang") {
        return;
    }
    let source = r#"
module test.unsafe_native_overflow;

permit { unsafe }

@id("o.assign_add")
fn assign_add() -> i32 {
    let mut value = 2147483647i32;
    @audit("checked overflow probe")
    unsafe {
        value = value + 1i32;
        0
    }
    value
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("unsafe-native-overflow.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();

    let assign_add = format!("spx_decl_{}", hex_identity("o.assign_add"));
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int32_t out = UINT32_C(0);
    if ({assign_add}(&context, &out) == SPX_STATUS_SUCCESS) return 11;
    return 0;
}}
"#
    );
    run_native_probe(&generated, &probe, "unsafe overflow");
}

fn run_native_probe(generated: &str, probe: &str, label: &str) {
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-unsafe-native-{label}-{}-{id}", std::process::id());
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
        let stderr = String::from_utf8_lossy(&executed.stderr).into_owned();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "{label} failed at {optimization}: status={status:?} stderr={stderr}"
        );
    }
}

#[test]
fn wasm_unsafe_boundaries_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(BOUNDED, Path::new("unsafe-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-unsafe-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const expected = BigInt(process.argv[3]);
const SPX_MIN = -(2n ** 63n), SPX_MAX = 2n ** 63n - 1n;
const bounded = (value, what) => {
  if (value < SPX_MIN || value > SPX_MAX) throw new RangeError(`checked ${what} failure`);
  return value;
};
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: (a, b) => bounded(a + b, "addition"),
  spx_sub: (a, b) => bounded(a - b, "subtraction"),
  spx_mul: (a, b) => bounded(a * b, "multiplication"),
  spx_div: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("division"); return a / b; },
  spx_rem: (a, b) => { if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("remainder"); return a % b; },
  spx_neg: (a) => bounded(-a, "negation"),
  spx_contract_fail: () => { throw new Error("SEMAPRAX contract failure"); },
} });
for (let index = 0; index < 4096; index += 1) {
  const observed = instance.exports.semaprax_main();
  if (observed !== expected) throw new Error(`result mismatch ${observed}`);
}
console.log("unsafe-wasm-ok");
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg("142")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node unsafe boundary run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "unsafe-wasm-ok"
    );
}

#[test]
fn wasm_unsafe_checked_overflow_traps_instead_of_wrapping() {
    if !command_available("node") {
        return;
    }
    let source = r#"
module test.unsafe_wasm_overflow;

permit { unsafe }

@id("main")
fn main() -> i64 {
    let mut narrow = 2147483647i32;
    @audit("checked overflow probe")
    unsafe {
        narrow = narrow + 1i32;
        0
    }
    if narrow < 0i32 { 1 } else { 0 }
}
"#;
    let program = parse(source, Path::new("unsafe-wasm-overflow.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-unsafe-wasm-overflow-{}-{id}", std::process::id());
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
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
let trapped = false;
try {
  instance.exports.semaprax_main();
} catch (error) {
  trapped = true;
}
if (!trapped) throw new Error("boundary overflow must not wrap silently");
console.log("unsafe-wasm-trap-ok");
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
        "Node unsafe overflow probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "unsafe-wasm-trap-ok"
    );
}
