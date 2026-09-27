use super::*;

/// Issue #293 P2-1: Core Wasm's `call_admission` module gives every emitted
/// function the same call-depth ceiling the interpreter (`MAX_CALL_DEPTH`)
/// and the native C11 backend (`SPX_MAX_CALL_DEPTH`) already enforced, so a
/// recursive metered function refuses at the identical depth instead of
/// diverging into fuel exhaustion or an uncontrolled host-engine stack trap.
/// This drives the compiled bytecode directly (bypassing the Agent Stage
/// executor's `project::public_api` packaging, which refuses any recursive
/// closure outright for an unrelated, pre-existing reason -- see the
/// `semantic_work_parity` cross-backend gate for the interpreter/native
/// legs of this same admission).
#[test]
fn call_depth_admission_refuses_the_same_ceiling_every_backend_shares() {
    let node = Command::new("node").arg("--version").output().unwrap();
    if !node.status.success() {
        return;
    }
    let source = r#"
module test.wasm_call_depth;
@id("test.wasm_call_depth.recurse")
fn recurse(depth: i64) -> i64 { if depth <= 0 { 0 } else { 1 + recurse(depth - 1) } }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let resolved = hir::resolve(&parse(source, Path::new("wasm-call-depth.spx")).unwrap()).unwrap();
    // `emit_profile` (the general compiled-Wasm target `crate::wasm::emit_module`
    // itself reaches for a plain program like this one) admits ordinary
    // recursion; the Agent Stage executor's owned-data package family
    // (`emit_owned_data_exports`) and its `project::public_api` packaging both
    // refuse a cyclic call graph outright for an unrelated, pre-existing
    // reason -- a private shadow-stack sizing pass that needs a statically
    // bounded call depth, and a closure-serialization format that needs a DAG,
    // respectively. Depth admission belongs to the shared `emit_function`
    // lowering both families use, so this still exercises the real fix.
    let bytes = emit_profile(&resolved, true, false).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-wasm-call-depth-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let recurse = format!(
        "__spx_test_{}",
        hex_identity(&DeclarationId::new("test.wasm_call_depth.recurse"))
    );
    // `CALL_DEPTH_STATUS` (18): the private status a refused call-depth
    // admission propagates, disjoint from every public arithmetic/contract
    // status (`1..=10`) and every other private status this backend
    // reserves.
    let call_depth_status = super::super::call_admission::CALL_DEPTH_STATUS;
    let max_call_depth = super::super::call_admission::MAX_CALL_DEPTH;
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const env = Object.freeze({{
  spx_add: (a, b) => a + b, spx_sub: (a, b) => a - b,
  spx_mul: () => {{ throw new Error("spx_mul"); }},
  spx_div: () => {{ throw new Error("spx_div"); }},
  spx_rem: () => {{ throw new Error("spx_rem"); }},
  spx_neg: () => {{ throw new Error("spx_neg"); }},
  spx_contract_fail: () => {{ throw new Error("spx_contract_fail"); }},
}});
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env }});
const output = 2048;
const view = new DataView(instance.exports.__spx_test_memory.buffer);
const recurse = instance.exports["{recurse}"];
// Exactly at the ceiling: {max_call_depth} admitted frames (depths
// 0..{max_call_depth_minus_one}), the deepest call still returns normally.
const atCeiling = BigInt({max_call_depth} - 1);
if (recurse(atCeiling, output) !== 0) throw new Error("at-ceiling status");
if (view.getBigInt64(output, true) !== atCeiling) throw new Error("at-ceiling result");
// One frame deeper refuses with the private call-depth status, never a
// language-visible one and never an uncontrolled trap.
const overCeiling = BigInt({max_call_depth});
if (recurse(overCeiling, output) !== {call_depth_status}) {{
  throw new Error("over-ceiling status");
}}
// Far deeper than the ceiling: still the identical refusal, not a trap, not
// fuel exhaustion (Core Wasm has no fuel outside a scoped semantic meter).
if (recurse(300n, output) !== {call_depth_status}) {{
  throw new Error("deep-recursion status");
}}
"#,
        max_call_depth_minus_one = max_call_depth - 1,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(script_path);
    let _ = std::fs::remove_file(wasm_path);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Issue #293 P2-2: the audit that found P2-1's gap. A plain scalar
/// recursive function -- no authored record/class/variant, no concrete
/// generic variant, no byte-array data, no Vec, no Box -- is emitted by the
/// separate legacy scalar-core emitter (`emit_resolved_module_internal`'s
/// final branch, in `src/wasm.rs` itself, not `aggregate`), which never
/// called `call_admission`: `wasm::emit_module` admitted unbounded
/// recursion on Core Wasm while the interpreter and native C11 backend both
/// refused at the identical ceiling. This drives that exact scenario
/// end to end: `crate::wasm::emit_module_with_scalar_exports` (the Public
/// Scalar Export Profile v1 -- see `scalar_exports.rs`) reaches the same
/// legacy branch `wasm::emit_module` does for this program (confirmed below
/// by the ordinary seven-import, no-memory, single-function-export shape),
/// and exports `recurse` directly by its stable ID, so no wrapping `main`
/// call contributes an extra frame to the depth count.
#[test]
fn call_depth_admission_refuses_the_same_ceiling_on_the_legacy_scalar_core_emitter() {
    let node = Command::new("node").arg("--version").output().unwrap();
    if !node.status.success() {
        return;
    }
    let source = r#"
module test.wasm_scalar_call_depth;
@id("test.wasm_scalar_call_depth.recurse")
fn recurse(depth: i64) -> i64 { if depth <= 0 { 0 } else { 1 + recurse(depth - 1) } }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let stable_id = "test.wasm_scalar_call_depth.recurse";
    let program = parse(source, Path::new("wasm-scalar-call-depth.spx")).unwrap();
    let diagnostics = crate::verify::verify(&program);
    assert!(
        diagnostics.is_empty(),
        "{}",
        diagnostics
            .iter()
            .map(|item| format!("{}: {}", item.code, item.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
    let export_ids = [stable_id.to_owned()];
    let bytes = crate::wasm::emit_module_with_scalar_exports(&program, &export_ids).unwrap();
    // Confirm this really is the legacy scalar-core branch, not `aggregate`:
    // the ordinary seven scalar runtime imports, no memory, and exactly one
    // function export (the selected scalar adapter, never `semaprax_main`
    // -- the Public Scalar Export Profile deliberately omits it).
    let mut imports = Vec::new();
    let mut function_exports = Vec::new();
    let mut has_memory = false;
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            wasmparser::Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    imports.push(import.unwrap().name.to_owned());
                }
            }
            wasmparser::Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    if export.kind == wasmparser::ExternalKind::Func {
                        function_exports.push(export.name.to_owned());
                    }
                }
            }
            wasmparser::Payload::MemorySection(_) => has_memory = true,
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [
            "spx_add",
            "spx_sub",
            "spx_mul",
            "spx_div",
            "spx_rem",
            "spx_neg",
            "spx_contract_fail",
        ]
    );
    assert!(!has_memory, "a pure scalar export profile has no memory");
    let symbol = crate::wasm::scalar_exports::raw_symbol(stable_id);
    assert_eq!(function_exports, [symbol.clone()]);

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-wasm-scalar-call-depth-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, &bytes).unwrap();
    // The private status legacy's `spx_contract_fail` call passes for a
    // refused call-depth admission is the identical wire value
    // `aggregate::call_admission::CALL_DEPTH_STATUS` (18); the host glue
    // this test provides mirrors the production mapping in
    // `wasm/browser_runtime.js`.
    let call_depth_status = super::super::call_admission::CALL_DEPTH_STATUS;
    let max_call_depth = super::super::call_admission::MAX_CALL_DEPTH;
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
class Refused extends Error {{}}
const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
function checked(value, operation) {{
  if (value < SPX_MIN || value > SPX_MAX) throw new Error(`unexpected ${{operation}}`);
  return value;
}}
const unexpected = name => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const env = Object.freeze({{
  spx_add: (a, b) => checked(a + b, "addition overflow"),
  spx_sub: (a, b) => checked(a - b, "subtraction overflow"),
  spx_mul: unexpected("spx_mul"),
  spx_div: unexpected("spx_div"),
  spx_rem: unexpected("spx_rem"),
  spx_neg: unexpected("spx_neg"),
  spx_contract_fail: code => {{
    if (code !== {call_depth_status}) throw new Error(`unexpected contract fail code ${{code}}`);
    throw new Refused("call-depth admission refused");
  }},
}});
const module = await WebAssembly.compile(bytes);
async function recurse(depth) {{
  const instance = await WebAssembly.instantiate(module, {{ env }});
  return instance.exports["{symbol}"](depth);
}}
// Exactly at the ceiling: {max_call_depth} admitted frames (depths
// 0..{max_call_depth_minus_one}), the deepest call still returns normally.
const atCeiling = BigInt({max_call_depth} - 1);
const atCeilingResult = await recurse(atCeiling);
if (atCeilingResult !== atCeiling) {{
  throw new Error(`at-ceiling result: ${{atCeilingResult}}`);
}}
// One frame deeper refuses through the same channel every other checked
// runtime failure already reports through on this backend: a call to
// `spx_contract_fail`, never an uncontrolled trap or fuel exhaustion (Core
// Wasm has no fuel outside a scoped semantic meter).
for (const depth of [BigInt({max_call_depth}), 300n]) {{
  let refused = false;
  try {{
    await recurse(depth);
  }} catch (error) {{
    if (!(error instanceof Refused)) throw error;
    refused = true;
  }}
  if (!refused) throw new Error(`depth ${{depth}} was admitted instead of refused`);
}}
"#,
        max_call_depth_minus_one = max_call_depth - 1,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(script_path);
    let _ = std::fs::remove_file(wasm_path);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
