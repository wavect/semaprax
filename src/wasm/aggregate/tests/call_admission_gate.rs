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
