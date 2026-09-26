use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::public_generic_consumer::rust_calling::{OwnedByteField, RecordShape};
use semaprax::public_generic_consumer::typescript_calling::generate_typescript_calling_consumer;

static NEXT: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module provider.artifact;

@id("provider.leaf-pair")
record LeafPair {
    @id("provider.leaf-pair.left")
    left: Bytes,
    @id("provider.leaf-pair.right")
    right: Bytes,
}

@id("provider.envelope")
record Envelope<T> {
    @id("provider.envelope.payload")
    payload: T,
}

@id("provider.transform")
fn transform(value: own Envelope<LeafPair>) -> Envelope<LeafPair> {
    Envelope<LeafPair> { payload: LeafPair { left: value.payload.right, right: value.payload.left } }
}

@id("provider.main")
fn main() -> i64 { 0 }
"#;

const FAILING_SOURCE: &str = r#"module provider.artifact;

@id("provider.leaf-pair")
record LeafPair {
    @id("provider.leaf-pair.left")
    left: Bytes,
    @id("provider.leaf-pair.right")
    right: Bytes,
}

@id("provider.envelope")
record Envelope<T> {
    @id("provider.envelope.payload")
    payload: T,
}

@id("provider.transform")
fn transform(value: own Envelope<LeafPair>) -> Envelope<LeafPair>
    requires false
{
    value
}

@id("provider.main")
fn main() -> i64 { 0 }
"#;

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn exact_tsc_version(stdout: &[u8]) -> bool {
    std::str::from_utf8(stdout).is_ok_and(|text| text.trim() == "Version 5.8.3")
}

fn checked_tsc(candidate: &Path) -> Result<PathBuf, String> {
    // Resolve before the generated package changes the compiler's cwd.
    let resolved = candidate
        .canonicalize()
        .map_err(|error| format!("{}: {error}", candidate.display()))?;
    let version = Command::new(&resolved)
        .arg("--version")
        .output()
        .map_err(|error| format!("{}: {error}", resolved.display()))?;
    if !version.status.success() || !exact_tsc_version(&version.stdout) {
        return Err(format!(
            "{} requires exactly TypeScript 5.8.3, got {:?}",
            resolved.display(),
            String::from_utf8_lossy(&version.stdout)
        ));
    }
    Ok(resolved)
}

fn required_tsc() -> PathBuf {
    assert!(checked_tsc(Path::new("missing-pinned-tsc")).is_err());
    assert!(!exact_tsc_version(b"Version 5.8.30\n"));
    assert!(!exact_tsc_version(b"Version 5.9.0\n"));
    if let Some(explicit) = env::var_os("SPX_PG_TSC").or_else(|| env::var_os("TSC")) {
        return checked_tsc(Path::new(&explicit))
            .expect("explicit TypeScript compiler must be the pinned 5.8.3 image");
    }
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            if cfg!(windows) {
                candidates.push(directory.join("tsc.cmd"));
                candidates.push(directory.join("tsc.exe"));
            }
            candidates.push(directory.join("tsc"));
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join("Library/pnpm/tsc"));
        candidates.push(home.join(".local/share/pnpm/tsc"));
    }
    candidates
        .iter()
        .find_map(|candidate| checked_tsc(candidate).ok())
        .expect("generated compiled-provider consumer requires pinned TypeScript 5.8.3")
}

fn generated_field_name(identity: &str) -> String {
    let suffix = identity
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("field_{suffix}")
}

fn resolved_program(source: &str) -> semaprax::hir::ResolvedProgram {
    let program = semaprax::check(source, Path::new("compiler-provider-artifact.spx")).unwrap();
    semaprax::hir::resolve(&program).unwrap()
}

fn endpoint_for(
    source: &str,
) -> semaprax::public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1 {
    let parsed = semaprax::parse(source, Path::new("compiler-provider-artifact.spx")).unwrap();
    let source_revision = semaprax::format::canonical(&parsed);
    let program = resolved_program(source);
    semaprax::public_generic_abi::compiler_endpoint::derive_admitted_public_generic_endpoint_v1(
        &program,
        &source_revision,
        "provider.transform",
    )
    .unwrap()
}

fn endpoint() -> semaprax::public_generic_abi::compiler_endpoint::AdmittedPublicGenericEndpointV1 {
    endpoint_for(SOURCE)
}

pub(super) fn artifact() -> semaprax::wasm::PublicGenericWasmProviderArtifactV1 {
    let program = resolved_program(SOURCE);
    semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint()).unwrap()
}

fn failing_artifact() -> semaprax::wasm::PublicGenericWasmProviderArtifactV1 {
    let program = resolved_program(FAILING_SOURCE);
    semaprax::wasm::emit_public_generic_wasm_provider_v1(&program, &endpoint_for(FAILING_SOURCE))
        .unwrap()
}

fn frame(
    direction: semaprax::public_generic_abi::carrier::trace::Direction,
    payloads: &[&[u8]],
) -> Vec<u8> {
    use semaprax::public_generic_abi::carrier::frame::{
        CarrierFrameBinding, CarrierLeaf, LeafKind,
    };

    let parsed = semaprax::parse(SOURCE, Path::new("compiler-provider-artifact.spx")).unwrap();
    let source_revision = semaprax::format::canonical(&parsed);
    let program = resolved_program(SOURCE);
    let endpoint = semaprax::public_generic_abi::compiler_endpoint::derive_admitted_public_generic_endpoint_v1(
        &program,
        &source_revision,
        "provider.transform",
    )
    .unwrap();
    let binding = CarrierFrameBinding::from_verified_descriptor(endpoint.descriptor(), direction);
    assert_eq!(binding.leaf_paths().len(), payloads.len());
    binding
        .frame_with_leaves(
            binding
                .leaf_paths()
                .iter()
                .zip(payloads)
                .map(|(path, payload)| {
                    CarrierLeaf::new(path.clone(), LeafKind::Bytes, payload.to_vec())
                })
                .collect(),
        )
        .encode()
}

#[test]
fn compiler_provider_artifact_is_deterministic_closed_and_binding_normalized() {
    let first = artifact();
    let second = artifact();
    assert_eq!(first.wasm(), second.wasm());
    assert_eq!(first.binding_bytes(), second.binding_bytes());
    assert_eq!(
        first.artifact_digest(),
        first.binding().provider_artifact_digest()
    );
    assert_eq!(
        first.binding().exported_endpoint_export_name(),
        "spx_pg_v1_call"
    );
    first.verify().unwrap();
    wasmparser::Validator::new()
        .validate_all(first.wasm())
        .unwrap();

    let imports = wasmparser::Parser::new(0)
        .parse_all(first.wasm())
        .filter_map(Result::ok)
        .filter(|payload| matches!(payload, wasmparser::Payload::ImportSection(_)))
        .count();
    assert_eq!(imports, 0, "provider gains no ambient Wasm imports");
}

#[test]
fn node_loads_exact_closed_provider_inventory_without_fixture_or_host_imports() {
    if !node_available() {
        return;
    }
    let artifact = artifact();
    let root = std::env::temp_dir().join(format!(
        "semaprax-pg-compiler-provider-artifact-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("provider.wasm"), artifact.wasm()).unwrap();
    fs::write(root.join("descriptor.bin"), artifact.descriptor_bytes()).unwrap();
    fs::write(root.join("binding.bin"), artifact.binding_bytes()).unwrap();
    fs::write(
        root.join("input.bin"),
        frame(
            semaprax::public_generic_abi::carrier::trace::Direction::Input,
            &[b"left", b"right"],
        ),
    )
    .unwrap();
    fs::write(
        root.join("expected.bin"),
        frame(
            semaprax::public_generic_abi::carrier::trace::Direction::Result,
            &[b"right", b"left"],
        ),
    )
    .unwrap();
    fs::write(
        root.join("probe.mjs"),
        r#"import { readFileSync } from 'node:fs';
const module = await WebAssembly.compile(readFileSync('provider.wasm'));
if (WebAssembly.Module.imports(module).length !== 0) throw new Error('ambient import');
const expected = ['memory','spx_pg_v1_scratch_ptr','spx_pg_v1_scratch_reserve','spx_pg_v1_scratch_capacity','spx_pg_v1_open','spx_pg_v1_input_prepare','spx_pg_v1_call','spx_pg_v1_result_export','spx_pg_v1_value_release','spx_pg_v1_result_release','spx_pg_v1_provider_close'].sort();
const actual = WebAssembly.Module.exports(module).map(item => item.name).sort();
if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error('inventory');
const instance = await WebAssembly.instantiate(module, {});
const lane = raw => ({ status: Number(raw & 0xffffffffn), value: Number((raw >> 32n) & 0xffffffffn) });
const descriptor = readFileSync('descriptor.bin');
const binding = readFileSync('binding.bin');
const input = readFileSync('input.bin');
const expectedBytes = readFileSync('expected.bin');
const reserved = lane(instance.exports.spx_pg_v1_scratch_reserve(16 * 1024 * 1024 + 2056));
if (reserved.status !== 0 || reserved.value !== 393216) throw new Error('reserve');
let memory = new Uint8Array(instance.exports.memory.buffer);
memory.set(descriptor, 393216);
memory.set(binding, 393216 + descriptor.length);
const opened = lane(instance.exports.spx_pg_v1_open(393216, descriptor.length, 393216 + descriptor.length, binding.length));
if (opened.status !== 0 || !opened.value) throw new Error('open');
memory.set(input, 393216);
const prepared = lane(instance.exports.spx_pg_v1_input_prepare(opened.value, 393216, input.length));
if (prepared.status !== 0 || !prepared.value) throw new Error('prepare:' + JSON.stringify(prepared));
const called = lane(instance.exports.spx_pg_v1_call(opened.value, prepared.value));
if (called.status !== 0 || !called.value) throw new Error('call:' + JSON.stringify(called));
const probe = lane(instance.exports.spx_pg_v1_result_export(called.value, 393216, 0));
if (probe.status !== 12 || probe.value !== expectedBytes.length) throw new Error('export probe:' + JSON.stringify(probe) + ':expected=' + expectedBytes.length);
const copied = lane(instance.exports.spx_pg_v1_result_export(called.value, 393216, probe.value));
memory = new Uint8Array(instance.exports.memory.buffer);
if (copied.status !== 0 || copied.value !== expectedBytes.length || !memory.slice(393216, 393216 + copied.value).every((byte, index) => byte === expectedBytes[index])) throw new Error('nonidentity result carrier');
if (instance.exports.spx_pg_v1_result_release(called.value) !== 0) throw new Error('result release');
if (instance.exports.spx_pg_v1_provider_close(opened.value) !== 0) throw new Error('provider close');
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_typescript_package_projects_frozen_hostility_through_the_compiled_provider() {
    assert!(
        node_available(),
        "generated compiled-provider consumer requires Node"
    );
    let tsc = required_tsc();
    let artifact = artifact();
    let endpoint = endpoint();
    let input = RecordShape::new(
        endpoint
            .descriptor()
            .input_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let output = RecordShape::new(
        endpoint
            .descriptor()
            .result_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let consumer = generate_typescript_calling_consumer(
        artifact.descriptor_bytes(),
        artifact.binding(),
        &input,
        &output,
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-pg-generated-compiled-provider-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    let package = root.join("package");
    for (name, contents) in consumer.files() {
        let path = package.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("provider.wasm"), artifact.wasm()).unwrap();
    let names = input
        .fields
        .iter()
        .map(|field| generated_field_name(&field.identity))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        2,
        "the Phase-B compiler provider has two owned leaves"
    );
    // Keep this a projection, not a shadow corpus: the generated package has
    // a descriptor-specific canonical frame, so reuse the frozen hostile
    // recipe's one-byte trailing edit and its closed malformed outcome rather
    // than feeding the fixture's unrelated descriptor-bound bytes to it.
    let trailing = semaprax::public_generic_abi::carrier::hostile_corpus::cases()
        .into_iter()
        .find(|case| case.id == "trailing_byte_after_self_digest")
        .expect("the frozen carrier corpus retains its trailing-byte recipe");
    assert_eq!(
        trailing.expected_code(),
        Some(
            semaprax::public_generic_abi::carrier::reference_decoder::CarrierRefusal::Malformed
                .code()
        )
    );
    fs::write(
        package.join("test/compiled-provider.mjs"),
        format!(
            r#"import assert from "node:assert/strict";
import {{ readFileSync }} from "node:fs";
import {{ Provider }} from "../dist/index.js";
import {{ TRUSTED_DESCRIPTOR_BYTES, TRUSTED_BINDING_BYTES }} from "../dist/descriptor.js";
import {{ SemapraxPublicGenericException }} from "../dist/errors.js";
const wasm = readFileSync(process.argv[2]);
const left = "{left}", right = "{right}";
const provider = await Provider.open(wasm);
try {{
  const output = provider.transform({{ [left]: Uint8Array.from([1, 2]), [right]: Uint8Array.from([7, 8, 9]) }});
  assert.deepEqual([...output[left]], [7, 8, 9], "compiled checked endpoint swaps the first owned leaf");
  assert.deepEqual([...output[right]], [1, 2], "compiled checked endpoint swaps the second owned leaf");
}} finally {{ provider.close(); }}
const stale = TRUSTED_DESCRIPTOR_BYTES.slice(); stale[0] ^= 1;
await assert.rejects(() => Provider.open(wasm, {{ descriptorBytes: stale }}), error => error instanceof SemapraxPublicGenericException && error.detail.kind === "descriptor-rejected");
const binding = TRUSTED_BINDING_BYTES.slice(); binding[binding.length - 1] ^= 1;
await assert.rejects(() => Provider.open(wasm, {{ bindingBytes: binding }}), error => error instanceof SemapraxPublicGenericException && error.detail.kind === "provider-mismatch");
const originalInstantiate = WebAssembly.instantiate;
let resultReleases = 0;
WebAssembly.instantiate = async (...args) => {{
  const instance = await Reflect.apply(originalInstantiate, WebAssembly, args);
  return {{ exports: new Proxy({{}}, {{
    get(_target, property) {{
      const value = Reflect.get(instance.exports, property);
      if (property !== "spx_pg_v1_result_release") return value;
      return handle => {{
        resultReleases += 1;
        assert.equal(Reflect.apply(value, instance.exports, [handle]), 0, "the real compiled release settles first");
        return 37;
      }};
    }},
  }}) }};
}};
try {{
  const provider = await Provider.open(wasm);
  assert.throws(
    () => provider.transform({{ [left]: Uint8Array.from([1, 2]), [right]: Uint8Array.from([7, 8, 9]) }}),
    error => error instanceof SemapraxPublicGenericException && error.detail.kind === "release-failed" && error.detail.status === 37,
    "the first physical release status stays primary",
  );
  assert.equal(resultReleases, 1, "a failing result release must not be retried by catch cleanup");
  provider.close();
}} finally {{
  WebAssembly.instantiate = originalInstantiate;
}}
let malformedReleases = 0;
WebAssembly.instantiate = async (...args) => {{
  const instance = await Reflect.apply(originalInstantiate, WebAssembly, args);
  return {{ exports: new Proxy({{}}, {{
    get(_target, property) {{
      const value = Reflect.get(instance.exports, property);
      if (property === "spx_pg_v1_result_release") return handle => {{
        malformedReleases += 1;
        return Reflect.apply(value, instance.exports, [handle]);
      }};
      if (property !== "spx_pg_v1_result_export") return value;
      return (handle, pointer, capacity) => {{
        const lane = Reflect.apply(value, instance.exports, [handle, pointer, capacity]);
        if (capacity === 0) return lane + (1n << 32n);
        const bytes = new Uint8Array(instance.exports.memory.buffer);
        const written = Number((lane >> 32n) & 0xffffffffn);
        assert.equal(Number(lane & 0xffffffffn), 0, "compiled export must copy before the hostile edit");
        bytes[pointer + written] = 0xff;
        return lane + (1n << 32n);
      }};
    }},
  }}) }};
}};
try {{
  const provider = await Provider.open(wasm);
  assert.throws(
    () => provider.transform({{ [left]: Uint8Array.from([1, 2]), [right]: Uint8Array.from([7, 8, 9]) }}),
    error => error instanceof SemapraxPublicGenericException && error.detail.kind === "result-rejected" && error.detail.reason === "carrier-framing",
    "the frozen trailing-byte recipe must reject at generated canonical-result decode",
  );
  assert.equal(malformedReleases, 1, "the malformed compiled result releases exactly once before close");
  provider.close();
}} finally {{
  WebAssembly.instantiate = originalInstantiate;
}}
console.log("GENERATED COMPILED CANONICAL CARRIER PASS");
"#,
            left = names[0],
            right = names[1],
        ),
    )
    .unwrap();
    let build = Command::new(&tsc)
        .current_dir(&package)
        .args(["-p", "tsconfig.json"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "tsc stderr={}",
        String::from_utf8_lossy(&build.stderr)
    );
    let execution = Command::new("node")
        .current_dir(&package)
        .args(["test/compiled-provider.mjs", "../provider.wasm"])
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        execution.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
}

#[test]
fn generated_typescript_failure_releases_the_preserved_input_before_close() {
    assert!(
        node_available(),
        "generated compiled-provider consumer requires Node"
    );
    let tsc = required_tsc();
    let artifact = failing_artifact();
    let endpoint = endpoint_for(FAILING_SOURCE);
    let input = RecordShape::new(
        endpoint
            .descriptor()
            .input_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let output = RecordShape::new(
        endpoint
            .descriptor()
            .result_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let consumer = generate_typescript_calling_consumer(
        artifact.descriptor_bytes(),
        artifact.binding(),
        &input,
        &output,
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-pg-generated-failing-provider-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    let package = root.join("package");
    for (name, contents) in consumer.files() {
        let path = package.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("provider.wasm"), artifact.wasm()).unwrap();
    let names = input
        .fields
        .iter()
        .map(|field| generated_field_name(&field.identity))
        .collect::<Vec<_>>();
    fs::write(
        package.join("test/failing-provider.mjs"),
        format!(
            r#"import assert from "node:assert/strict";
import {{ readFileSync }} from "node:fs";
import {{ Provider }} from "../dist/index.js";
import {{ SemapraxPublicGenericException }} from "../dist/errors.js";
const provider = await Provider.open(readFileSync(process.argv[2]));
try {{
  for (let attempt = 0; attempt < 2; attempt += 1) {{
    assert.throws(
      () => provider.transform({{ ["{left}"]: Uint8Array.from([1]), ["{right}"]: Uint8Array.from([2]) }}),
      error => error instanceof SemapraxPublicGenericException && error.detail.kind === "execution-failed" && error.detail.status === 11,
      "the selected checked endpoint must fail without retaining the input",
    );
  }}
  provider.close();
  provider.close();
}} finally {{
  try {{ provider.close(); }} catch {{ /* assertion above owns the failure */ }}
}}
console.log("GENERATED COMPILED FAILURE INPUT RELEASE PASS");
"#,
            left = names[0],
            right = names[1],
        ),
    )
    .unwrap();
    let build = Command::new(&tsc)
        .current_dir(&package)
        .args(["-p", "tsconfig.json"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "tsc stdout={} stderr={}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let execution = Command::new("node")
        .current_dir(&package)
        .args(["test/failing-provider.mjs", "../provider.wasm"])
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        execution.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
}

/// #287: the reference lifecycle test above proves the generated WRAPPER's
/// OWN busy/close bookkeeping. This test instead proves the compiled
/// provider's OWN closed Wasm ABI refuses the same hostility directly, with
/// no change to the shipped generator template: it temporarily wraps
/// `WebAssembly.instantiate` (the same pattern already used above, around
/// line 400) to capture the real `WebAssembly.Instance` -- and, via its own
/// exports proxy, the real provider id `spx_pg_v1_open` returns and a count
/// of every real `spx_pg_v1_call` dispatch -- while still calling the
/// generated `Provider.open(wasm)` so digest verification and
/// descriptor/binding replay are exercised exactly as generated. It then
/// drives the captured `instance.exports.spx_pg_v1_*` and writes
/// `instance.exports.memory` directly, bypassing only the safe wrapper's own
/// bookkeeping, never the module's authentication. A mutated frame shape,
/// over-capacity/out-of-bounds declared lengths (including 32-bit-wrap
/// arguments), and lifecycle misuse (export before call, release of a
/// foreign/stale handle, release of an input already consumed by `call`,
/// double release, and close) each refuse at the module's own exact status,
/// proven never to reach a second real `spx_pg_v1_call` dispatch by the
/// captured counter, and the session remains healthy (a genuine `transform`
/// still succeeds) after every non-terminal refusal. The final case also
/// keeps a SEPARATE, explicitly labeled wrapper-level close check: the
/// generated wrapper's own `#closed` guard must refuse locally too, without
/// re-touching the already-closed module.
#[test]
fn generated_typescript_diagnostics_prove_the_compiled_providers_own_abi_hostility() {
    assert!(
        node_available(),
        "generated compiled-provider consumer requires Node"
    );
    let tsc = required_tsc();
    let artifact = artifact();
    let endpoint = endpoint();
    let input = RecordShape::new(
        endpoint
            .descriptor()
            .input_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let output = RecordShape::new(
        endpoint
            .descriptor()
            .result_facts()
            .owned_leaves
            .iter()
            .cloned()
            .map(OwnedByteField::new)
            .collect(),
    );
    let consumer = generate_typescript_calling_consumer(
        artifact.descriptor_bytes(),
        artifact.binding(),
        &input,
        &output,
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-pg-generated-compiled-provider-abi-hostility-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    let package = root.join("package");
    for (name, contents) in consumer.files() {
        let path = package.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("provider.wasm"), artifact.wasm()).unwrap();
    let names = input
        .fields
        .iter()
        .map(|field| generated_field_name(&field.identity))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        2,
        "the Phase-B compiler provider has two owned leaves"
    );
    fs::write(
        package.join("test/compiled-provider-abi-hostility.mjs"),
        format!(
            r#"import assert from "node:assert/strict";
import {{ readFileSync }} from "node:fs";
import {{ Provider }} from "../dist/index.js";
import {{ encodeCanonicalInput }} from "../dist/carrier.js";
import {{ SemapraxPublicGenericException }} from "../dist/errors.js";
const wasm = readFileSync(process.argv[2]);
const left = "{left}", right = "{right}";
function fresh() {{ return {{ [left]: Uint8Array.from([1, 2]), [right]: Uint8Array.from([7, 8, 9]) }}; }}
function assertSwap(output) {{
  assert.deepEqual([...output[left]], [7, 8, 9]);
  assert.deepEqual([...output[right]], [1, 2]);
}}
function lane(raw) {{ return {{ status: Number(raw & 0xffffffffn), value: Number((raw >> 32n) & 0xffffffffn) }}; }}

// Capture the REAL instantiated module (and, via its own exports proxy, the
// real provider id `spx_pg_v1_open` returns and a count of every real
// `spx_pg_v1_call` dispatch) while still letting the generated `Provider.open`
// perform its own digest verification and descriptor/binding replay. Restored
// in `finally`; production bytes are never edited.
const originalInstantiate = WebAssembly.instantiate;
let instance = null;
let providerId = 0;
let callCount = 0;
WebAssembly.instantiate = async (...args) => {{
  instance = await Reflect.apply(originalInstantiate, WebAssembly, args);
  return {{ exports: new Proxy({{}}, {{
    get(_target, property) {{
      const value = Reflect.get(instance.exports, property);
      if (property === "spx_pg_v1_open" && typeof value === "function") {{
        return (...openArgs) => {{
          const raw = Reflect.apply(value, instance.exports, openArgs);
          providerId = lane(raw).value;
          return raw;
        }};
      }}
      if (property === "spx_pg_v1_call" && typeof value === "function") {{
        return (...callArgs) => {{ callCount += 1; return Reflect.apply(value, instance.exports, callArgs); }};
      }}
      return value;
    }},
  }}) }};
}};

try {{
  const provider = await Provider.open(wasm);
  assert.ok(instance, "the real module must have been instantiated");
  assert.notEqual(providerId, 0, "the real open() call must have been observed and returned a live provider id");

  // Baseline: a genuine call through the safe wrapper works, and is the only
  // real endpoint dispatch this whole hostile block records so far. This
  // block's own later raw `spx_pg_v1_call` invocation goes through
  // `realCall` below so it is counted on the SAME `callCount`, not just the
  // wrapper's proxied path -- one total, whichever caller dispatched it.
  function realCall(input) {{ callCount += 1; return lane(instance.exports.spx_pg_v1_call(providerId, input)); }}
  assertSwap(provider.transform(fresh()));
  assert.equal(callCount, 1, "the baseline call is the only real endpoint dispatch so far");

  const scratch = instance.exports.spx_pg_v1_scratch_ptr();
  const capacity = instance.exports.spx_pg_v1_scratch_capacity();

  // bounds/over-capacity: the compiled ABI's OWN scratch-bound check, on the
  // real module, refuses a declared length or pointer outside the fixed
  // scratch range -- including 32-bit-wraparound arguments -- before any
  // input is considered live, so none of these reach the busy check either.
  assert.equal(lane(instance.exports.spx_pg_v1_input_prepare(providerId, scratch, capacity + 1)).status, 6,
    "an over-capacity declared length must refuse as bounded");
  assert.equal(lane(instance.exports.spx_pg_v1_input_prepare(providerId, scratch - 1, 8)).status, 6,
    "a pointer before the fixed scratch range must refuse as bounded");
  assert.equal(lane(instance.exports.spx_pg_v1_input_prepare(providerId, 0xffffffff, 2 ** 31)).status, 6,
    "32-bit-wraparound pointer/length arguments must still refuse as bounded, never trap or wrap into range");

  // mutated frame shape: a single byte flipped inside the trailing self-digest
  // field's hex content (the last-appended `framed(digest(FRAME_DOMAIN, body))`
  // field, which the compiled codec recomputes from the preceding bytes and
  // compares exactly) is refused as malformed by the compiled provider's OWN
  // carrier codec, never accepted.
  const bytes = encodeCanonicalInput(fresh());
  const corrupted = bytes.slice();
  corrupted[corrupted.length - 30] ^= 0xff;
  new Uint8Array(instance.exports.memory.buffer).set(corrupted, scratch);
  assert.equal(lane(instance.exports.spx_pg_v1_input_prepare(providerId, scratch, corrupted.length)).status, 5,
    "a structurally mutated frame must refuse as malformed, not accepted");

  // None of the refusals above left a live input, and none dispatched the
  // endpoint: the session is still healthy for a genuine call.
  assertSwap(provider.transform(fresh()));
  assert.equal(callCount, 2, "the bounds/shape refusals above must never have dispatched the endpoint");

  // Lifecycle misuse against the real compiled ABI: export before call,
  // release of a foreign/stale handle, release of an input already consumed
  // by `call`, and double release must each refuse without leaking state or
  // ever reaching a second real dispatch.
  new Uint8Array(instance.exports.memory.buffer).set(bytes, scratch);
  const prepared = lane(instance.exports.spx_pg_v1_input_prepare(providerId, scratch, bytes.length));
  assert.equal(prepared.status, 0);

  assert.equal(lane(instance.exports.spx_pg_v1_result_export(prepared.value, scratch, 0)).status, 8,
    "exporting an unfilled input handle as a result must refuse");
  assert.equal(instance.exports.spx_pg_v1_result_release(999999), 8, "releasing a foreign result handle must refuse");
  assert.equal(instance.exports.spx_pg_v1_value_release(999999), 8, "releasing a foreign input handle must refuse");
  assert.equal(callCount, 2, "export-before-call and foreign-handle releases must never dispatch the endpoint");

  const called = realCall(prepared.value);
  assert.equal(called.status, 0);
  assert.equal(callCount, 3, "exactly one real endpoint dispatch for this prepared input");
  const result = called.value;

  assert.equal(instance.exports.spx_pg_v1_value_release(prepared.value), 8,
    "releasing an input handle already consumed by call must refuse, not double-dispatch");
  assert.equal(instance.exports.spx_pg_v1_result_release(result), 0, "the real first release must succeed");
  assert.equal(instance.exports.spx_pg_v1_result_release(result), 8,
    "a second release of the same handle must refuse, not double-dispatch");
  assert.equal(callCount, 3, "release/double-release misuse must never dispatch the endpoint");

  assertSwap(provider.transform(fresh()));
  assert.equal(callCount, 4);

  // Call after close, at the real module level: close the real provider (via
  // the safe wrapper, which invokes the one real `spx_pg_v1_provider_close`),
  // then prove the module's OWN ABI refuses further input preparation.
  provider.close();
  assert.equal(lane(instance.exports.spx_pg_v1_input_prepare(providerId, scratch, bytes.length)).status, 8,
    "input preparation after the real module close must refuse");
  assert.equal(callCount, 4, "a refused prepare after a real close must never reach the endpoint");

  // Call after close, at the WRAPPER level (a separate, explicitly labeled
  // check): the generated wrapper's own `#closed` guard must refuse locally
  // too, without ever re-touching the already-closed module.
  assert.throws(
    () => provider.transform(fresh()),
    error => error instanceof SemapraxPublicGenericException
      && error.detail.kind === "carrier-rejected" && error.detail.reason === "provider-closed",
    "transform after close must refuse at the wrapper level too, without touching the closed module again",
  );
  assert.equal(callCount, 4, "the wrapper-level closed refusal must never reach the endpoint");

  console.log("GENERATED COMPILED PROVIDER ABI HOSTILITY PASS");
}} finally {{
  WebAssembly.instantiate = originalInstantiate;
}}
"#,
            left = names[0],
            right = names[1],
        ),
    )
    .unwrap();
    let build = Command::new(&tsc)
        .current_dir(&package)
        .args(["-p", "tsconfig.json"])
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "tsc stderr={}",
        String::from_utf8_lossy(&build.stderr)
    );
    let execution = Command::new("node")
        .current_dir(&package)
        .args([
            "test/compiled-provider-abi-hostility.mjs",
            "../provider.wasm",
        ])
        .output()
        .unwrap();
    let _ = fs::remove_dir_all(&root);
    assert!(
        execution.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
    assert!(
        String::from_utf8_lossy(&execution.stdout)
            .contains("GENERATED COMPILED PROVIDER ABI HOSTILITY PASS"),
        "hostility runner did not emit its exact outcome marker"
    );
}
