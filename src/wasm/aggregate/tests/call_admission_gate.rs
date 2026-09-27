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
///
/// A coordinator review of the legacy scalar-core emitter's own admission
/// (issue #293 P2-2, see `call_depth_admission_refuses_the_same_ceiling_on_
/// the_legacy_scalar_core_emitter` below) asked whether this aggregate
/// family has the same property: can any exit that skips the shared
/// epilogue -- a genuine Wasm `unreachable` invariant trap -- leave the
/// live-frame counter poisoned for a later call on the same instance? It
/// cannot: every recoverable status this family reports, refused call
/// depth included, converges on one shared exit that decrements the
/// counter unconditionally before returning a status *value*, never a
/// trap (see `call_admission`'s own module documentation); the only raw
/// `unreachable` this family emits is a shadow-stack-underflow invariant
/// guard in each scalar-export wrapper, which runs before that wrapper
/// ever calls into the counted lane, so it cannot leave the counter
/// touched either. The trailing check below confirms the property holds
/// for the reachable case: a refusal must not poison a later, shallower
/// call on the same instance.
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
// A refusal must not poison this same instance for a later, shallower call
// (coordinator review of P2-2): every recoverable status here -- refused
// call depth included -- converges on one shared exit that decrements the
// live-frame counter unconditionally before returning, so it never traps
// and never leaves the counter elevated. Confirm that holds by calling
// shallow right after the two refusals above, on the identical instance.
const afterRefusal = 10n;
if (recurse(afterRefusal, output) !== 0) {{
  throw new Error("post-refusal status: refusal poisoned this instance");
}}
if (view.getBigInt64(output, true) !== afterRefusal) {{
  throw new Error("post-refusal result: refusal poisoned this instance");
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
// One instance for every call below: production host glue
// (`wasm/browser_runtime.js`'s `invoke`) reuses exactly one instance across
// many calls, catching and normalizing each checked failure rather than
// discarding the instance, so this is the realistic shape to test.
const instance = await WebAssembly.instantiate(module, {{ env }});
function recurse(depth) {{
  return instance.exports["{symbol}"](depth);
}}
// Exactly at the ceiling: {max_call_depth} admitted frames (depths
// 0..{max_call_depth_minus_one}), the deepest call still returns normally.
const atCeiling = BigInt({max_call_depth} - 1);
const atCeilingResult = recurse(atCeiling);
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
    recurse(depth);
  }} catch (error) {{
    if (!(error instanceof Refused)) throw error;
    refused = true;
  }}
  if (!refused) throw new Error(`depth ${{depth}} was admitted instead of refused`);
}}
// Coordinator review of P2-2: a refused call really does trap on this
// backend (unlike aggregate), so without `scalar_call_admission::emit_reset`
// resetting the live-frame counter at this wrapper's own entry, the
// depth-300 refusal above would leave it stuck near 300 forever after on
// this instance -- poisoning every later call, however shallow, with the
// identical refusal. Confirm a shallow call right after those two refusals,
// on the identical instance, still succeeds.
const afterRefusal = recurse(10n);
if (afterRefusal !== 10n) {{
  throw new Error(
    `post-refusal result: ${{afterRefusal}} (this instance is poisoned)`
  );
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

/// Issue #293 P2-2 (coordinator review): the live-frame counter this
/// backend's call-depth admission relies on must not stay poisoned by any
/// trapped call, not only a refused call depth -- a checked-arithmetic
/// overflow reports through the identical trapping channel (a real host
/// import throw, `unreachable` only as its fail-closed fallback). Recurse
/// to depth 100, overflow there, and confirm a later, shallower call on the
/// same instance still succeeds instead of being refused at a phantom
/// depth left over from the overflow's own never-decremented increments.
#[test]
fn call_depth_admission_reset_survives_an_unrelated_trap_on_the_legacy_scalar_core_emitter() {
    let node = Command::new("node").arg("--version").output().unwrap();
    if !node.status.success() {
        return;
    }
    let source = r#"
module test.wasm_scalar_call_depth_reset;
@id("test.wasm_scalar_call_depth_reset.overflow_at_depth")
fn overflow_at_depth(depth: i64) -> i64 {
    if depth <= 0 { 9223372036854775807 + 1 } else { overflow_at_depth(depth - 1) }
}
@id("test.wasm_scalar_call_depth_reset.shallow")
fn shallow(depth: i64) -> i64 { if depth <= 0 { 0 } else { 1 + shallow(depth - 1) } }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let overflow_id = "test.wasm_scalar_call_depth_reset.overflow_at_depth";
    let shallow_id = "test.wasm_scalar_call_depth_reset.shallow";
    let program = parse(source, Path::new("wasm-scalar-call-depth-reset.spx")).unwrap();
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
    let export_ids = [overflow_id.to_owned(), shallow_id.to_owned()];
    let bytes = crate::wasm::emit_module_with_scalar_exports(&program, &export_ids).unwrap();
    let overflow_symbol = crate::wasm::scalar_exports::raw_symbol(overflow_id);
    let shallow_symbol = crate::wasm::scalar_exports::raw_symbol(shallow_id);

    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-wasm-scalar-call-depth-reset-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, &bytes).unwrap();
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
function checked(value, operation) {{
  if (value < SPX_MIN || value > SPX_MAX) {{
    throw new Error(`SEMAPRAX checked arithmetic failure: ${{operation}}`);
  }}
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
  spx_contract_fail: code => {{ throw new Error(`unexpected contract fail code ${{code}}`); }},
}});
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env }});
// Recurse 100 frames deep (well under the 256 ceiling, so call-depth
// admission itself never refuses here), then overflow `i64::MAX + 1`: the
// real checked-arithmetic host import throws, and this backend's
// fail-closed `unreachable` is never reached.
let trapped = false;
try {{
  instance.exports["{overflow_symbol}"](100n);
}} catch (error) {{
  trapped = true;
}}
if (!trapped) throw new Error("overflow at depth 100 did not trap");
// A later, unrelated, shallow call (well short of the 256 ceiling on its
// own) on the identical instance must still succeed: the overflow trap
// above left its own ~101 live-frame counter increments uncompensated
// (this backend traps rather than converging on a shared decrementing
// exit, see `scalar_call_admission`'s module documentation), so without
// this wrapper's own entry reset (`scalar_call_admission::emit_reset`) the
// leftover ~101 plus this call's own 201 frames would together exceed the
// ceiling and this call would be refused instead of succeeding -- an
// observable way this exact test catches a missing reset, not only the
// depth-300-then-10 case above.
const shallowResult = instance.exports["{shallow_symbol}"](200n);
if (shallowResult !== 200n) {{
  throw new Error(
    `post-overflow shallow result: ${{shallowResult}} (this instance is poisoned)`
  );
}}
"#,
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
