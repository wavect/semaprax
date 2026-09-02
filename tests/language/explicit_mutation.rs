use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup_plan::CleanupPlan;
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const MUTATION: &str = r#"
module test.explicit_mutation;

@id("mut.accumulate")
fn accumulate() -> i64 {
    let mut total = 0;
    total = total + 5;
    total = total * 2;
    let base = 3;
    let mut other = base;
    other = other + base;
    let mut mixed = total;
    mixed = mixed - 1;
    if other > 4 { total + other } else { mixed }
}

@id("mut.checked")
fn checked(flag: bool) -> i64 {
    let mut narrow = 100i32;
    narrow = narrow * 2000000i32;
    let guard = if flag { 0i32 - 2147483647i32 - 1i32 } else { 0i32 };
    let mut wide = 7;
    wide = wide + 1;
    if guard == 0i32 { wide + 1 } else { wide }
}

@id("main")
fn main() -> i64 { accumulate() + checked(false) }
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
    let program = parse(source, Path::new("mutation.spx")).unwrap();
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
fn mutation_programs_round_trip_canonically_and_hash_stably() {
    let program = parse(MUTATION, Path::new("mutation.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains("let mut total = 0;"),
        "mutable bindings keep the `let mut` prefix in canonical form: {canonical}"
    );
    assert!(
        canonical.contains("total = total + 5;"),
        "assignment statements render without a keyword: {canonical}"
    );
    let reparsed = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert!(verify::verify(&reparsed).is_empty());
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn immutable_bindings_and_plain_programs_render_exactly_as_before_mutation() {
    // The canonical form of a program with no mutation syntax must be
    // unchanged: no `mut`, no assignment rendering.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let canonical = format::canonical(&program);
    assert!(!canonical.contains("mut "));
    assert!(!canonical.contains("kind:\"assign\""));
    assert!(canonical.contains("let total = 1;"));
    assert!(canonical.contains("let frozen = 3;"));
}

#[test]
fn assigning_to_an_immutable_binding_is_spx_u101() {
    let report = diagnostics(
        r#"
module test.mut_immutable;
@id("app.main")
fn main() -> i64 {
    let frozen = 1;
    frozen = 2;
    frozen
}
"#,
    );
    let diagnostic = report
        .iter()
        .find(|item| item.code == "SPX-U101")
        .expect("immutable assignment must be rejected");
    assert!(diagnostic.message.contains("`frozen`"));
}

#[test]
fn mut_on_a_parameter_is_spx_u103() {
    let error = parse(
        "module t;\n@id(\"m\")\nfn m(mut value: i64) -> i64 { value }",
        Path::new("param.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-U103");

    let interface_error = parse(
        r#"
module t;
permit { host.echo }
interface Host permits { host.echo } {
    @id("h.echo")
    import rust fn host_echo(mut value: i64) -> i64
        effects { host.echo }
        failure status "host.echo.v1";
}
@id("m")
fn m(value: i64) -> i64 { value }
@id("main")
fn main() -> i64 { 0 }
"#,
        Path::new("iface.spx"),
    )
    .unwrap_err();
    assert_eq!(interface_error.code, "SPX-U103");
}

#[test]
fn duplicate_mut_modifier_is_spx_u104() {
    let error = parse(
        "module t;\n@id(\"m\")\nfn m() -> i64 { let mut mut x = 1; x }",
        Path::new("dup.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-U104");
}

#[test]
fn assigned_value_type_mismatch_is_spx_u102() {
    let report = diagnostics(
        r#"
module test.mut_type_mismatch;
@id("app.main")
fn main() -> i64 {
    let mut narrow = 1i32;
    narrow = 2;
    narrow
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-U102"),
        "exact type match is required on assignment: {report:?}"
    );

    let bool_report = diagnostics(
        r#"
module test.mut_bool_mismatch;
@id("app.main")
fn main() -> i64 {
    let mut flag = true;
    flag = false == false;
    if flag { 1 } else { 0 }
}
"#,
    );
    // bool = bool is exact and must stay clean.
    assert!(
        !bool_report
            .iter()
            .any(|item| item.code.starts_with("SPX-U10")),
        "matching types assign cleanly: {bool_report:?}"
    );
}

#[test]
fn non_scalar_or_owned_targets_are_spx_u105() {
    // Records are not part of the Explicit Mutation v1 scalar slice.
    let report = diagnostics(
        r#"
module test.mut_record;
record Point {
    x: i64,
    y: i64,
}
@id("app.main")
fn main() -> i64 {
    let mut point = Point { x: 1, y: 2 };
    point = point with { x: 3 };
    point.x
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-U105"),
        "record targets stay outside the v1 slice: {report:?}"
    );
}

#[test]
fn assignments_inside_contract_expressions_are_spx_u106() {
    let report = diagnostics(
        r#"
module test.mut_contract;
@id("c.check")
fn check(value: i64) -> i64
requires {{
    let mut seen = value;
    seen = seen + 1;
    seen > 0
}}
{ value }
@id("app.main")
fn main() -> i64 { check(1) }
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-U106"),
        "contract expressions remain pure: {report:?}"
    );
}

#[test]
fn assignment_is_statement_only_and_cannot_chain() {
    // Assignment inside an expression position is a parse error.
    let error = parse(
        "module t;\n@id(\"m\")\nfn m() -> i64 { let mut x = 1; (x = 2) }",
        Path::new("expr.spx"),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-P106", "no assignment expression exists");

    // Chained assignment has no right-associative reading either.
    let chained = parse(
        "module t;\n@id(\"m\")\nfn m() -> i64 { let mut x = 1; let mut y = 2; x = y = 3; x }",
        Path::new("chain.spx"),
    )
    .unwrap_err();
    assert_eq!(chained.code, "SPX-P106");

    // Unknown assignment targets reuse the established unknown-value code.
    let report = diagnostics(
        r#"
module test.mut_unknown;
@id("app.main")
fn main() -> i64 {
    missing = 1;
    0
}
"#,
    );
    assert!(
        report.iter().any(|item| item.code == "SPX-T202"),
        "unknown targets are unresolved values: {report:?}"
    );
}

#[test]
fn graph_serialization_includes_assignments_and_mut_flags_additively() {
    let program = parse(MUTATION, Path::new("mutation.spx")).unwrap();

    // Deterministic serialization across repeated renders.
    let first = graph::to_json(&program).unwrap();
    let second = graph::to_json(&program).unwrap();
    assert_eq!(first, second);

    // The assignment node names its target and reuses the original binding
    // identity; the mutable let carries the additive flag exactly once.
    assert!(
        first.contains("\"kind\":\"assign\""),
        "assignments serialize as assign nodes: {first}"
    );
    assert!(first.contains("\"mutable\":true"));
    assert_eq!(first.matches("\"mutable\":true").count(), 5);
    assert!(
        !first.contains("\"mutable\":false"),
        "the flag stays absent on plain lets so pre-mutation bytes hold"
    );

    let wire_value: serde_json::Value = serde_json::from_str(&first).unwrap();
    fn collect_assign_targets(
        value: &serde_json::Value,
        targets: &mut Vec<String>,
        mutable_lets: &mut Vec<String>,
    ) {
        if let Some(object) = value.as_object() {
            if object.get("kind") == Some(&serde_json::Value::from("assign")) {
                targets.push(object["target"]["id"].as_str().unwrap().to_owned());
            }
            if object.get("kind") == Some(&serde_json::Value::from("let"))
                && object.get("mutable") == Some(&serde_json::Value::from(true))
            {
                mutable_lets.push(object["binding"]["id"].as_str().unwrap().to_owned());
            }
            for child in object.values() {
                collect_assign_targets(child, targets, mutable_lets);
            }
        } else if let Some(array) = value.as_array() {
            for child in array {
                collect_assign_targets(child, targets, mutable_lets);
            }
        }
    }
    let mut targets = Vec::new();
    let mut mutable_lets = Vec::new();
    collect_assign_targets(&wire_value, &mut targets, &mut mutable_lets);
    assert_eq!(targets.len(), 6);
    assert_eq!(mutable_lets.len(), 5);
    // Every assignment target names an existing mutable let identity.
    for target in &targets {
        assert!(
            mutable_lets.contains(target),
            "target `{target}` must reuse its mutable let id"
        );
    }

    // Schema selection is untouched by mutation-only programs.
    let wire = serde_json::from_str::<serde_json::Value>(&first).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v10");
}

#[test]
fn non_mutation_graph_bytes_are_pinned_to_pre_feature_output() {
    // This digest was captured from a build without Explicit Mutation v1 and
    // must never drift: plain programs serialize byte-for-byte identically.
    let program = parse(PLAIN_STABLE, Path::new("plain.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(!json.contains("\"kind\":\"assign\""));
    assert!(!json.contains("\"mutable\":"));
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
fn straight_line_mutation_adds_no_cleanup_structure() {
    let source = r#"
module test.mut_cleanup;

@id("clean.with_mutation")
fn with_mutation() -> i64 {
    let mut total = 1;
    total = total + 2;
    total
}

@id("clean.initializers_only")
fn initializers_only() -> i64 {
    let total = 1;
    let again = total + 2;
    again
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("mut-cleanup.spx")).unwrap();
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
    let mutated: CleanupPlan = function("clean.with_mutation");
    let plain: CleanupPlan = function("clean.initializers_only");

    // Same statement count means the lowering sees identical shape: every
    // region/exit/block count must match exactly.
    assert_eq!(mutated.blocks.len(), plain.blocks.len());
    assert_eq!(mutated.regions.len(), plain.regions.len());
    assert_eq!(mutated.exits.len(), plain.exits.len());
    assert_eq!(mutated.slots.len(), plain.slots.len());

    // Scalar values own nothing: neither plan may finalize anything or keep
    // owned storage slots, proving assignments add no cleanup structure.
    for plan in [&mutated, &plain] {
        assert!(plan.slots.is_empty());
        for exit in &plan.exits {
            assert!(
                exit.finalize_in_order.is_empty(),
                "scalar plans never finalize"
            );
        }
    }
    // Identical checked-arithmetic producers and identical overall shape:
    // the assignment lowers its RHS exactly like an initializer would.
    // Plans embed function-qualified ids, so compare modulo those names.
    let normalize = |plan: &CleanupPlan| {
        format!("{plan:?}")
            .replace("clean.with_mutation", "FN")
            .replace("clean.initializers_only", "FN")
            .replace("declaration:19:", "declaration:N:")
            .replace("declaration:23:", "declaration:N:")
    };
    assert_eq!(
        normalize(&mutated),
        normalize(&plain),
        "assignment adds no cleanup structure over the initializer-only form"
    );
}

#[test]
fn native_mutation_executes_identically_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(MUTATION, Path::new("mutation-native.spx")).unwrap();
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
    if ({accumulate}(&context, &accumulated) != SPX_STATUS_SUCCESS || accumulated != INT64_C(16)) return 11;
    int64_t checked = INT64_C(0);
    if ({checked}(&context, UINT8_C(0), &checked) != SPX_STATUS_SUCCESS || checked != INT64_C(9)) return 12;
    int64_t entry = 0;
    if ({main_fn}(&context, &entry) != SPX_STATUS_SUCCESS || entry != INT64_C(25)) return 13;
    return 0;
}}
"#,
        accumulate = symbol("mut.accumulate"),
        checked = symbol("mut.checked"),
        main_fn = symbol("main"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-mutation-native-{}-{id}", std::process::id());
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
fn wasm_mutation_matches_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse(MUTATION, Path::new("mutation-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert_eq!(bytes, wasm::emit_module(&program).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-mutation-wasm-{}-{id}", std::process::id());
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
console.log("mutation-wasm-ok");
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .arg("25")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node mutation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "mutation-wasm-ok"
    );
}

#[test]
fn wasm_checked_assignment_overflow_traps_instead_of_wrapping() {
    if !command_available("node") {
        return;
    }
    let source = r#"
module test.mutation_wasm_overflow;

@id("main")
fn main() -> i64 {
    let mut narrow = 2147483647i32;
    narrow = narrow + 1i32;
    if narrow < 0i32 { 1 } else { 0 }
}
"#;
    let program = parse(source, Path::new("mutation-wasm-overflow.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-mutation-wasm-overflow-{}-{id}",
        std::process::id()
    );
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
if (!trapped) throw new Error("assigned overflow must not wrap silently");
console.log("mutation-wasm-trap-ok");
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
        "Node overflow probe failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "mutation-wasm-trap-ok"
    );
}

#[test]
fn native_checked_assignment_overflow_selects_a_failure_status() {
    if !command_available("clang") {
        return;
    }
    let source = r#"
module test.mutation_native_overflow;

@id("o.assign_add")
fn assign_add() -> i32 {
    let mut value = 2147483647i32;
    value = value + 1i32;
    value
}

@id("main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("mutation-native-overflow.spx")).unwrap();
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

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-mutation-native-overflow-{}-{id}",
            std::process::id()
        );
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
            "assigned overflow must surface a failure status at {optimization}"
        );
    }
}
