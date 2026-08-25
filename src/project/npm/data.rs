//! Deterministic npm facade for Project v3's `useful-data.v1` profile.
//!
//! The public boundary is deliberately narrower than the internal byte-data
//! carrier: callers provide ordinary fixed `Uint8Array` values and receive
//! scalars. Raw packed carriers, scratch memory, and owned arena tokens never
//! escape this generated module.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{OwnershipMode, ResolvedProgram, ResolvedType};

use super::{
    artifact, package_error, payload_digest_artifacts_v2, render_carrier_artifacts,
    valid_package_name, valid_package_semver, valid_sha256_fact, NpmArtifact, NpmBuildIdentity,
    ProjectNpmBuild, PROJECT_NPM_BUILD_SCHEMA_V2,
};
use crate::project::{ProjectManifest, PROJECT_PROFILE_USEFUL_DATA_V1, PROJECT_SCHEMA_V3};

pub(super) const USEFUL_DATA_PACKAGE_PATHS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.data-exports.json",
    "package.json",
];

const MAX_WASM_BYTES: usize = 16 * 1024 * 1024;
const MAX_EXPORTS: usize = 32;
const MAX_PARAMETERS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DataType {
    SliceU8,
    I64,
    Bool,
    Usize,
}

impl DataType {
    fn json(self) -> &'static str {
        match self {
            Self::SliceU8 => "slice-u8",
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
        }
    }

    fn typescript(self) -> &'static str {
        match self {
            Self::SliceU8 => "Uint8Array",
            Self::I64 | Self::Usize => "bigint",
            Self::Bool => "boolean",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DataExport {
    stable_id: String,
    wasm_export: String,
    parameters: Vec<DataType>,
    result: DataType,
}

pub(super) fn prepare(
    manifest: &ProjectManifest,
    program: &ResolvedProgram,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
    max_bytes: usize,
) -> Result<ProjectNpmBuild, Diagnostic> {
    let version = require_profile(manifest)?;
    super::validate_carrier_limit(0, max_bytes)?;
    let exports = derive_exports(program, manifest.web_exports())?;
    let wasm =
        crate::wasm::emit_resolved_module_with_byte_exports(program, manifest.web_exports())?;
    let recipe = super::render_semantic_recipe(program)?;
    let artifacts = render_package(manifest, &wasm, &exports)?;
    let artifact_bytes = artifacts.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(item.bytes.len())
            .filter(|value| *value <= max_bytes)
            .ok_or_else(|| package_error("npm build artifacts exceed the trusted limit"))
    })?;
    let identity = NpmBuildIdentity {
        project_schema: manifest.schema(),
        package: manifest.name(),
        version,
        project_revision,
        workspace_revision,
        project_graph_digest,
        semantic_recipe: &recipe,
    };
    let payload_digest = payload_digest_artifacts_v2(identity, &artifacts);
    let envelope = render_carrier_artifacts(
        PROJECT_NPM_BUILD_SCHEMA_V2,
        identity,
        &artifacts,
        artifact_bytes,
        &payload_digest,
    );
    super::validate_carrier_limit(envelope.len(), max_bytes)?;
    let build = ProjectNpmBuild {
        envelope,
        payload_digest,
        artifact_bytes,
        max_bytes,
        trusted: super::trusted_binding(identity),
    };
    build.verify()?;
    Ok(build)
}

fn require_profile(manifest: &ProjectManifest) -> Result<&str, Diagnostic> {
    if !manifest.is_v3() || manifest.profile() != Some(PROJECT_PROFILE_USEFUL_DATA_V1) {
        return Err(package_error(
            "npm data facade requires the useful-data.v1 Project v3 profile",
        ));
    }
    manifest
        .package_version()
        .ok_or_else(|| package_error("npm data facade requires a package version"))
}

fn derive_exports(
    program: &ResolvedProgram,
    selected: &[String],
) -> Result<Vec<DataExport>, Diagnostic> {
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let exports = selected
        .iter()
        .map(|stable_id| {
            let function = functions.get(stable_id.as_str()).ok_or_else(|| {
                package_error(format!("selected npm data export `{stable_id}` is absent"))
            })?;
            let parameters = function
                .params
                .iter()
                .map(|parameter| match (&parameter.ty, parameter.ownership) {
                    (ResolvedType::SliceU8, OwnershipMode::Borrow) => Ok(DataType::SliceU8),
                    _ => Err(package_error(format!(
                        "selected npm data export `{stable_id}` has a non-slice parameter"
                    ))),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if parameters.is_empty() || parameters.len() > MAX_PARAMETERS {
                return Err(package_error(format!(
                    "selected npm data export `{stable_id}` must have 1..={MAX_PARAMETERS} borrowed byte-slice parameters"
                )));
            }
            let result = match function.return_type {
                ResolvedType::I64 => DataType::I64,
                ResolvedType::Bool => DataType::Bool,
                ResolvedType::Usize => DataType::Usize,
                _ => {
                    return Err(package_error(format!(
                        "selected npm data export `{stable_id}` has an unsupported result"
                    )))
                }
            };
            Ok(DataExport {
                stable_id: stable_id.clone(),
                wasm_export: raw_symbol(stable_id),
                parameters,
                result,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_exports(&exports)?;
    Ok(exports)
}

fn validate_exports(exports: &[DataExport]) -> Result<(), Diagnostic> {
    if !(1..=MAX_EXPORTS).contains(&exports.len()) {
        return Err(package_error(format!(
            "npm data facade requires 1..={MAX_EXPORTS} exports"
        )));
    }
    let mut previous: Option<&str> = None;
    for export in exports {
        if previous.is_some_and(|value| value.as_bytes() >= export.stable_id.as_bytes()) {
            return Err(package_error(
                "npm data facade exports must be strictly sorted and unique",
            ));
        }
        previous = Some(&export.stable_id);
        if export.wasm_export != raw_symbol(&export.stable_id)
            || export.stable_id.is_empty()
            || !export.stable_id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
            || export.parameters.is_empty()
            || export.parameters.len() > MAX_PARAMETERS
            || export.parameters.iter().any(|ty| *ty != DataType::SliceU8)
            || matches!(export.result, DataType::SliceU8)
        {
            return Err(package_error("npm data facade export ABI is invalid"));
        }
    }
    Ok(())
}

fn render_package(
    manifest: &ProjectManifest,
    wasm: &[u8],
    exports: &[DataExport],
) -> Result<[NpmArtifact; 6], Diagnostic> {
    let version = require_profile(manifest)?;
    if wasm.is_empty() || wasm.len() > MAX_WASM_BYTES {
        return Err(package_error(format!(
            "npm data facade Wasm must contain 1..={MAX_WASM_BYTES} bytes"
        )));
    }
    validate_exports(exports)?;
    let digest = hex_sha256(wasm);
    let runtime = render_runtime(&digest);
    let bindings = render_bindings(exports, &digest);
    let declarations = render_declarations(exports);
    let metadata = render_metadata(manifest.name(), version, &digest, exports);
    let package = render_package_json(manifest.name(), version);
    Ok([
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.data-exports.json", metadata.as_bytes()),
        artifact("package.json", package.as_bytes()),
    ])
}

fn render_runtime(wasm_sha256: &str) -> String {
    format!(
        r#"const EXPECTED_WASM_SHA256 = "{wasm_sha256}";
const MIN_I64 = -(1n << 63n), MAX_I64 = (1n << 63n) - 1n;
const TypedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const typedTag = Object.getOwnPropertyDescriptor(TypedArrayPrototype, Symbol.toStringTag).get;
const typedBuffer = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "buffer").get;
const typedOffset = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "byteOffset").get;
const typedLength = Object.getOwnPropertyDescriptor(TypedArrayPrototype, "byteLength").get;
const typedSet = TypedArrayPrototype.set;
const sharedBufferLength = typeof SharedArrayBuffer === "undefined" ? null : Object.getOwnPropertyDescriptor(SharedArrayBuffer.prototype, "byteLength").get;
const arrayBufferResizable = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "resizable")?.get;
const arrayBufferSlice = ArrayBuffer.prototype.slice;
const reflectApply = Reflect.apply;
const objectGetPrototypeOf = Object.getPrototypeOf;
function snapshotUint8(value, label) {{
  let buffer; try {{ buffer = reflectApply(typedBuffer, value, []); }} catch {{ throw new TypeError(`${{label}} must be an ordinary attached Uint8Array`); }}
  if (objectGetPrototypeOf(value) !== Uint8Array.prototype || reflectApply(typedTag, value, []) !== "Uint8Array") throw new TypeError(`${{label}} must be an ordinary Uint8Array`);
  if (sharedBufferLength !== null) {{ let shared = false; try {{ reflectApply(sharedBufferLength, buffer, []); shared = true; }} catch {{}} if (shared) throw new TypeError(`${{label}} must not use SharedArrayBuffer`); }}
  if (arrayBufferResizable !== undefined && reflectApply(arrayBufferResizable, buffer, [])) throw new TypeError(`${{label}} must not use a resizable ArrayBuffer`);
  reflectApply(arrayBufferSlice, buffer, [0, 0]);
  const offset = reflectApply(typedOffset, value, []), length = reflectApply(typedLength, value, []);
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(length)) throw new TypeError(`${{label}} is detached or out of bounds`);
  const copy = new Uint8Array(length); reflectApply(typedSet, copy, [value, 0]);
  if (copy.byteLength !== length) throw new TypeError(`${{label}} changed during snapshot`);
  return copy;
}}
export class SemapraxDataError extends Error {{
  constructor(code, domain = "semaprax.data-adapter.v1") {{ super(`SEMAPRAX data call failed with code ${{code}}`); this.name = "SemapraxDataError"; this.code = code; this.domain = domain; }}
}}
function semantic(code, domain) {{ throw new SemapraxDataError(code, domain); }}
function checked(value, code) {{ if (value < MIN_I64 || value > MAX_I64) semantic(code, "semaprax.arithmetic.v1"); return value; }}
function createByteArena() {{
  const entries = new Map(); let nextToken = 1, instance = null, poisoned = false;
  const decode = carrier => {{
    if (typeof carrier !== "bigint") throw new Error("SEMAPRAX byte carrier is not i64");
    const word = BigInt.asUintN(64, carrier), length = Number(word & 0xffffffffn), root = Number((word >> 32n) & 0xffffffffn);
    if (length > 65536) throw new Error("SEMAPRAX byte carrier length invariant");
    return {{ word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff }};
  }};
  const memory = () => {{
    const candidate = instance?.exports.memory;
    if (!(candidate instanceof WebAssembly.Memory) || candidate.buffer.byteLength !== 131072) throw new Error("SEMAPRAX fixed byte memory invariant");
    return new Uint8Array(candidate.buffer);
  }};
  const resolve = value => {{
    if (!value.tagged || value.token === 0) throw new Error("SEMAPRAX owned Bytes token invariant");
    const entry = entries.get(value.token);
    if (!(entry instanceof Uint8Array) || entry.byteLength !== value.length) throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
    return entry;
  }};
  const read = value => {{
    if (value.tagged) return resolve(value);
    if (value.root > 131072 - value.length) throw new Error("SEMAPRAX fixed byte range invariant");
    return memory().slice(value.root, value.root + value.length);
  }};
  const allocate = bytes => {{
    if (entries.size >= 16 || nextToken > 0x7fffffff) throw new Error("SEMAPRAX owned Bytes arena exhausted");
    const token = nextToken++, owned = new Uint8Array(bytes); entries.set(token, owned);
    return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(owned.byteLength));
  }};
  const imports = Object.freeze({{
    spx_bytes_copy: carrier => allocate(read(decode(carrier))),
    spx_bytes_get: (carrier, index) => {{ const bytes = read(decode(carrier)), offset = BigInt.asUintN(64, index); return offset >= BigInt(bytes.byteLength) ? -1 : bytes[Number(offset)]; }},
    spx_bytes_drop: carrier => {{ const value = decode(carrier); resolve(value); entries.delete(value.token); }},
    spx_bytes_as_slice: carrier => {{ const value = decode(carrier); value.tagged ? resolve(value) : read(value); return BigInt.asIntN(64, value.word); }},
  }});
  return Object.freeze({{
    imports,
    bind(value) {{ if (instance !== null) throw new Error("SEMAPRAX byte arena already bound"); instance = value; }},
    begin() {{ if (poisoned) throw new Error("SEMAPRAX data runtime is poisoned"); if (entries.size !== 0) {{ poisoned = true; throw new Error("SEMAPRAX byte arena entered unsettled"); }} }},
    settle() {{ if (entries.size !== 0) {{ poisoned = true; throw new Error("SEMAPRAX byte arena did not settle"); }} }},
    poison() {{ poisoned = true; }},
  }});
}}
export async function instantiateCore(input) {{
  const bytes = snapshotUint8(input, "SEMAPRAX module bytes");
  if (globalThis.crypto?.subtle === undefined) throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", bytes));
  const actual = Array.from(digest, value => value.toString(16).padStart(2, "0")).join("");
  if (actual !== EXPECTED_WASM_SHA256) throw new Error("SEMAPRAX WebAssembly artifact authentication failed");
  const arena = createByteArena();
  const env = Object.freeze({{
    spx_add: (a,b) => checked(a+b,1), spx_sub: (a,b) => checked(a-b,2), spx_mul: (a,b) => checked(a*b,3),
    spx_div: (a,b) => {{ if (b===0n) semantic(4,"semaprax.arithmetic.v1"); if (a===MIN_I64&&b===-1n) semantic(5,"semaprax.arithmetic.v1"); return a/b; }},
    spx_rem: (a,b) => {{ if (b===0n) semantic(6,"semaprax.arithmetic.v1"); if (a===MIN_I64&&b===-1n) semantic(7,"semaprax.arithmetic.v1"); return a%b; }},
    spx_neg: value => checked(-value,8), spx_contract_fail: code => semantic(code,"semaprax.contract.v1"), ...arena.imports,
  }});
  const result = await WebAssembly.instantiate(bytes, Object.freeze({{ env }}));
  arena.bind(result.instance);
  return Object.freeze({{ instance: result.instance, arena, snapshotUint8 }});
}}
"#
    )
}

fn render_bindings(exports: &[DataExport], wasm_sha256: &str) -> String {
    let facts = exports
        .iter()
        .map(|export| {
            format!(
                "[{},Object.freeze({{raw:{},params:Object.freeze([{}]),result:{}}})]",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|ty| quote_json(ty.json()))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(export.result.json()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"import {{ instantiateCore, SemapraxDataError }} from "./semaprax.js";
const EXPECTED_WASM_SHA256 = "{wasm_sha256}";
const ENTRIES = Object.freeze([{facts}]);
const FACTS = Object.create(null);
for (const [id, fact] of ENTRIES) Object.defineProperty(FACTS, id, {{ value: fact, enumerable: true }});
Object.freeze(FACTS);
const EXPORT_IDS = Object.freeze(ENTRIES.map(row => row[0]));
function globalNumber(value, name) {{ const raw = value instanceof WebAssembly.Global ? value.value : value; if (!Number.isSafeInteger(raw) || raw < 0 || raw > 0xffffffff) throw new Error(`invalid SEMAPRAX ${{name}} export`); return raw; }}
function scalarResult(value, type) {{
  if (type === "bool") {{ if (value !== 0 && value !== 1) throw new Error("SEMAPRAX adapter returned noncanonical bool"); return value === 1; }}
  if (typeof value !== "bigint") throw new Error("SEMAPRAX adapter returned non-i64 bits");
  return type === "usize" ? BigInt.asUintN(64, value) : value;
}}
function aggregateStatus(status) {{
  if (status >= 1 && status <= 8) return new SemapraxDataError(status, "semaprax.arithmetic.v1");
  if (status >= 9 && status <= 10) return new SemapraxDataError(status, "semaprax.contract.v1");
  throw new Error(`invalid SEMAPRAX aggregate status ${{status}}`);
}}
function facade(linked) {{
  const e = linked.instance.exports;
  if (!(e.memory instanceof WebAssembly.Memory) || e.memory.buffer.byteLength !== 131072) throw new Error("SEMAPRAX data memory is invalid");
  const base = globalNumber(e.__spx_data_scratch_base_v1, "data scratch base"), capacity = globalNumber(e.__spx_data_scratch_capacity_v1, "data scratch capacity");
  if (base !== 0 || capacity !== 65536 || base > e.memory.buffer.byteLength || capacity > e.memory.buffer.byteLength - base) throw new Error("SEMAPRAX data scratch range is invalid");
  let busy = false, poisoned = false;
  function invoke(id, values) {{
    if (poisoned) throw new Error("SEMAPRAX data runtime is poisoned");
    const fact = FACTS[id]; if (fact === undefined) throw new RangeError(`unknown SEMAPRAX data export: ${{id}}`);
    if (values.length !== fact.params.length) throw new TypeError(`SEMAPRAX data export ${{id}} expects ${{fact.params.length}} arguments`);
    if (busy) throw new SemapraxDataError(12);
    const snapshots = values.map((value, index) => linked.snapshotUint8(value, `argument ${{index}}`));
    let used = 0;
    for (const bytes of snapshots) {{ if (bytes.byteLength > capacity - used) throw new SemapraxDataError(11); used += bytes.byteLength; }}
    busy = true;
    let primaryError = null, began = false, memory = null;
    try {{
      memory = new Uint8Array(e.memory.buffer, base, capacity);
      const raw = e[fact.raw]; if (typeof raw !== "function") throw new Error(`SEMAPRAX data adapter missing: ${{fact.raw}}`);
      const rawArgs = []; let offset = 0;
      for (const bytes of snapshots) {{ memory.set(bytes, offset); rawArgs.push(base + offset, bytes.byteLength); offset += bytes.byteLength; }}
      linked.arena.begin(); began = true;
      const value = raw(...rawArgs), status = globalNumber(e.__spx_data_status_v1, "data status");
      if (status !== 0) primaryError = aggregateStatus(status);
      else return scalarResult(value, fact.result);
    }} catch (error) {{ primaryError = error; if (!(error instanceof SemapraxDataError)) {{ poisoned = true; linked.arena.poison(); }} }}
    finally {{
      let settlementError = null;
      if (began) {{ try {{ linked.arena.settle(); }} catch (error) {{ poisoned = true; settlementError = error; }} }}
      if (memory !== null && used !== 0) {{ try {{ memory.fill(0, 0, used); }} catch (error) {{ poisoned = true; settlementError ??= error; }} }}
      busy = false;
      if (settlementError !== null) throw settlementError;
    }}
    throw primaryError;
  }}
  const functions = Object.create(null);
  for (const id of EXPORT_IDS) Object.defineProperty(functions, id, {{ value: (...values) => invoke(id, values), enumerable: true }});
  return Object.freeze({{ functions: Object.freeze(functions), call: (id, ...values) => invoke(id, values), wasmSha256: EXPECTED_WASM_SHA256 }});
}}
export async function instantiate(bytes) {{ return facade(await instantiateCore(bytes)); }}
export const exportIds = EXPORT_IDS;
export {{ SemapraxDataError }};
export default instantiate;
"#
    )
}

fn render_declarations(exports: &[DataExport]) -> String {
    let functions = exports
        .iter()
        .map(|export| {
            let parameters = export
                .parameters
                .iter()
                .enumerate()
                .map(|(index, ty)| format!("arg{index}: {}", ty.typescript()))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "  readonly {}: ({parameters}) => {};",
                quote_json(&export.stable_id),
                export.result.typescript()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("export type DataFailureCode = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12;\nexport declare class SemapraxDataError extends Error {{ readonly code: DataFailureCode; readonly domain: string; }}\nexport interface UsefulDataFunctions {{\n{functions}\n}}\nexport interface UsefulDataRuntime {{ readonly functions: Readonly<UsefulDataFunctions>; call<I extends keyof UsefulDataFunctions>(id: I, ...args: Parameters<UsefulDataFunctions[I]>): ReturnType<UsefulDataFunctions[I]>; readonly wasmSha256: string; }}\nexport declare function instantiate(bytes: Uint8Array): Promise<UsefulDataRuntime>;\nexport declare const exportIds: readonly (keyof UsefulDataFunctions)[];\nexport default instantiate;\n")
}

fn render_metadata(name: &str, version: &str, wasm_sha256: &str, exports: &[DataExport]) -> String {
    let functions = exports
        .iter()
        .map(|export| {
            format!(
                "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
                quote_json(&export.stable_id),
                quote_json(&export.wasm_export),
                export
                    .parameters
                    .iter()
                    .map(|ty| quote_json(ty.json()))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(export.result.json())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"schema\":\"semaprax.data-exports.v1\",\"package\":{},\"version\":{},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":{}}},\"scratch\":{{\"memory\":\"memory\",\"status\":\"__spx_data_status_v1\",\"base\":\"__spx_data_scratch_base_v1\",\"capacity\":\"__spx_data_scratch_capacity_v1\",\"boundary_code\":11,\"busy_code\":12}},\"functions\":[{}]}}\n", quote_json(name), quote_json(version), quote_json(wasm_sha256), functions)
}

fn render_package_json(name: &str, version: &str) -> String {
    format!("{{\"name\":{},\"version\":{},\"type\":\"module\",\"sideEffects\":false,\"exports\":{{\".\":{{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"}},\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.data-exports.json\"}},\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.data-exports.json\"],\"engines\":{{\"node\":\">=22\"}}}}\n", quote_json(name), quote_json(version))
}

pub(super) fn validate_replayed(
    identity: NpmBuildIdentity<'_>,
    artifacts: &[NpmArtifact; 6],
) -> Result<(), Diagnostic> {
    if identity.project_schema != PROJECT_SCHEMA_V3
        || !valid_package_name(identity.package)
        || !valid_package_semver(identity.version)
        || !valid_sha256_fact(identity.project_revision)
        || !valid_sha256_fact(identity.workspace_revision)
        || !valid_sha256_fact(identity.project_graph_digest)
    {
        return Err(package_error(
            "npm data build identity facts are not canonical",
        ));
    }
    let wasm = artifact_bytes(artifacts, "app.wasm")?;
    wasmparser::Validator::new()
        .validate_all(wasm)
        .map_err(|_| package_error("npm data app.wasm is not structurally valid"))?;
    let metadata: serde_json::Value =
        serde_json::from_slice(artifact_bytes(artifacts, "semaprax.data-exports.json")?)
            .map_err(|_| package_error("npm data metadata is not valid JSON"))?;
    let object = metadata
        .as_object()
        .ok_or_else(|| package_error("npm data metadata must be one object"))?;
    super::require_exact_keys(
        object,
        &[
            "functions",
            "package",
            "schema",
            "scratch",
            "version",
            "wasm",
        ],
    )?;
    if super::json_string(object, "schema")? != "semaprax.data-exports.v1"
        || super::json_string(object, "package")? != identity.package
        || super::json_string(object, "version")? != identity.version
    {
        return Err(package_error("npm data metadata identity disagrees"));
    }
    let scratch = object
        .get("scratch")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| package_error("npm data scratch metadata is invalid"))?;
    super::require_exact_keys(
        scratch,
        &[
            "base",
            "boundary_code",
            "busy_code",
            "capacity",
            "memory",
            "status",
        ],
    )?;
    if super::json_string(scratch, "memory")? != "memory"
        || super::json_string(scratch, "status")? != "__spx_data_status_v1"
        || super::json_string(scratch, "base")? != "__spx_data_scratch_base_v1"
        || super::json_string(scratch, "capacity")? != "__spx_data_scratch_capacity_v1"
        || scratch
            .get("boundary_code")
            .and_then(serde_json::Value::as_u64)
            != Some(11)
        || scratch.get("busy_code").and_then(serde_json::Value::as_u64) != Some(12)
    {
        return Err(package_error("npm data scratch metadata disagrees"));
    }
    let wasm_fact = object
        .get("wasm")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| package_error("npm data Wasm metadata is invalid"))?;
    super::require_exact_keys(wasm_fact, &["path", "sha256"])?;
    let wasm_sha256 = hex_sha256(wasm);
    if super::json_string(wasm_fact, "path")? != "app.wasm"
        || super::json_string(wasm_fact, "sha256")? != wasm_sha256
    {
        return Err(package_error("npm data Wasm digest disagrees"));
    }
    let functions = object
        .get("functions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("npm data function metadata is invalid"))?;
    let exports = functions
        .iter()
        .map(parse_export)
        .collect::<Result<Vec<_>, _>>()?;
    validate_exports(&exports)?;
    validate_wasm_inventory(wasm, &exports)?;
    let ast = crate::parse(
        identity.semantic_recipe,
        Path::new("semaprax-project-npm-data-recipe.spx"),
    )
    .map_err(|_| package_error("npm data semantic recipe does not parse"))?;
    let replayed = crate::hir::resolve(&ast).map_err(|diagnostics| {
        let detail = diagnostics
            .first()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .unwrap_or_else(|| "unknown resolver failure".to_owned());
        package_error(format!(
            "npm data semantic recipe does not resolve: {detail}"
        ))
    })?;
    if super::render_semantic_recipe(&replayed)? != identity.semantic_recipe {
        return Err(package_error("npm data semantic recipe is not canonical"));
    }
    let selected = exports
        .iter()
        .map(|export| export.stable_id.clone())
        .collect::<Vec<_>>();
    if derive_exports(&replayed, &selected)? != exports {
        return Err(package_error("npm data recipe ABI disagrees"));
    }
    let expected_wasm = crate::wasm::emit_resolved_module_with_byte_exports(&replayed, &selected)?;
    if expected_wasm != wasm {
        return Err(package_error(
            "npm data app.wasm disagrees with semantic replay",
        ));
    }
    let expected =
        render_package_from_identity(identity.package, identity.version, wasm, &exports)?;
    if artifacts != &expected {
        return Err(package_error(
            "npm data generated artifacts disagree with semantic replay",
        ));
    }
    Ok(())
}

fn render_package_from_identity(
    name: &str,
    version: &str,
    wasm: &[u8],
    exports: &[DataExport],
) -> Result<[NpmArtifact; 6], Diagnostic> {
    let digest = hex_sha256(wasm);
    let runtime = render_runtime(&digest);
    let bindings = render_bindings(exports, &digest);
    let declarations = render_declarations(exports);
    let metadata = render_metadata(name, version, &digest, exports);
    let package = render_package_json(name, version);
    Ok([
        artifact("app.wasm", wasm),
        artifact("semaprax.js", runtime.as_bytes()),
        artifact("semaprax.bindings.js", bindings.as_bytes()),
        artifact("semaprax.bindings.d.ts", declarations.as_bytes()),
        artifact("semaprax.data-exports.json", metadata.as_bytes()),
        artifact("package.json", package.as_bytes()),
    ])
}

fn parse_export(value: &serde_json::Value) -> Result<DataExport, Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| package_error("npm data export row is invalid"))?;
    super::require_exact_keys(
        object,
        &["parameters", "result", "stable_id", "wasm_export"],
    )?;
    let parameters = object
        .get("parameters")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| package_error("npm data export parameters are invalid"))?
        .iter()
        .map(parse_type)
        .collect::<Result<Vec<_>, _>>()?;
    let result = parse_type(
        object
            .get("result")
            .ok_or_else(|| package_error("npm data export result is absent"))?,
    )?;
    Ok(DataExport {
        stable_id: super::json_string(object, "stable_id")?.to_owned(),
        wasm_export: super::json_string(object, "wasm_export")?.to_owned(),
        parameters,
        result,
    })
}

fn parse_type(value: &serde_json::Value) -> Result<DataType, Diagnostic> {
    match value.as_str() {
        Some("slice-u8") => Ok(DataType::SliceU8),
        Some("i64") => Ok(DataType::I64),
        Some("bool") => Ok(DataType::Bool),
        Some("usize") => Ok(DataType::Usize),
        _ => Err(package_error("npm data ABI type is unsupported")),
    }
}

fn validate_wasm_inventory(wasm: &[u8], exports: &[DataExport]) -> Result<(), Diagnostic> {
    use wasmparser::{ExternalKind, Parser, Payload};
    let mut actual = BTreeMap::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::ExportSection(section) =
            payload.map_err(|_| package_error("npm data Wasm is not parseable"))?
        {
            for export in section {
                let export =
                    export.map_err(|_| package_error("npm data Wasm export is invalid"))?;
                if actual.insert(export.name.to_owned(), export.kind).is_some() {
                    return Err(package_error("npm data Wasm repeats an export"));
                }
            }
        }
    }
    let mut expected = BTreeMap::from([
        ("memory".to_owned(), ExternalKind::Memory),
        ("__spx_data_status_v1".to_owned(), ExternalKind::Global),
        (
            "__spx_data_scratch_base_v1".to_owned(),
            ExternalKind::Global,
        ),
        (
            "__spx_data_scratch_capacity_v1".to_owned(),
            ExternalKind::Global,
        ),
    ]);
    for export in exports {
        expected.insert(export.wasm_export.clone(), ExternalKind::Func);
    }
    if actual != expected {
        return Err(package_error("npm data Wasm export inventory disagrees"));
    }
    Ok(())
}

fn artifact_bytes<'a>(artifacts: &'a [NpmArtifact; 6], path: &str) -> Result<&'a [u8], Diagnostic> {
    artifacts
        .iter()
        .find(|item| item.path == path)
        .map(NpmArtifact::bytes)
        .ok_or_else(|| package_error(format!("npm data artifact `{path}` is absent")))
}

fn raw_symbol(stable_id: &str) -> String {
    let mut symbol = String::from("spx_data_");
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        write!(symbol, "{byte:02x}").expect("String writes cannot fail");
    }
    symbol
}
fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity<'a>(recipe: &'a str, project_schema: &'a str) -> NpmBuildIdentity<'a> {
        NpmBuildIdentity {
            project_schema,
            package: "data-package",
            version: "1.0.0",
            project_revision:
                "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            workspace_revision:
                "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            project_graph_digest:
                "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            semantic_recipe: recipe,
        }
    }

    #[test]
    fn v2_replay_rejects_resigned_artifact_and_cross_label_substitution() {
        let source = crate::parse(
            "module data.app;\n@id(\"data.len\") fn len(value: borrow Slice<u8>) -> usize { byte_len(value) }\n@id(\"main\") fn main() -> i64 { 0 }\n",
            Path::new("data-app.spx"),
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let manifest = ProjectManifest::parse(
            "schema = \"semaprax.project.v3\"\nname = \"data-package\"\nversion = \"1.0.0\"\nprofile = \"useful-data.v1\"\nentry = \"data.app\"\nsources = [\"data-app.spx\", \"data-tests.spx\"]\nweb_exports = [\"data.len\"]\ntests = [\"data.tests\"]\n",
        )
        .unwrap();
        let exports = derive_exports(&program, manifest.web_exports()).unwrap();
        let wasm =
            crate::wasm::emit_resolved_module_with_byte_exports(&program, manifest.web_exports())
                .unwrap();
        let recipe = super::super::render_semantic_recipe(&program).unwrap();
        let artifacts = render_package(&manifest, &wasm, &exports).unwrap();
        let resign = |identity, artifacts: &[NpmArtifact; 6]| {
            let total = artifacts.iter().map(|artifact| artifact.bytes.len()).sum();
            let digest = payload_digest_artifacts_v2(identity, artifacts);
            render_carrier_artifacts(
                PROJECT_NPM_BUILD_SCHEMA_V2,
                identity,
                artifacts,
                total,
                &digest,
            )
        };
        let trusted = identity(&recipe, PROJECT_SCHEMA_V3);
        let canonical = resign(trusted, &artifacts);
        ProjectNpmBuild::inspect_envelope(&canonical, canonical.len()).unwrap();

        for path in [
            "app.wasm",
            "semaprax.js",
            "semaprax.data-exports.json",
            "package.json",
        ] {
            let mut replaced = artifacts.clone();
            replaced
                .iter_mut()
                .find(|item| item.path == path)
                .unwrap()
                .bytes[0] ^= 1;
            let resigned = resign(trusted, &replaced);
            assert!(
                ProjectNpmBuild::inspect_envelope(&resigned, resigned.len()).is_err(),
                "{path}"
            );
        }

        let cross_profile = resign(
            identity(&recipe, super::super::super::PROJECT_SCHEMA_V2),
            &artifacts,
        );
        assert!(ProjectNpmBuild::inspect_envelope(&cross_profile, cross_profile.len()).is_err());
        let total = artifacts.iter().map(|artifact| artifact.bytes.len()).sum();
        let digest = payload_digest_artifacts_v2(trusted, &artifacts);
        let cross_schema = render_carrier_artifacts(
            super::super::PROJECT_NPM_BUILD_SCHEMA,
            trusted,
            &artifacts,
            total,
            &digest,
        );
        assert!(ProjectNpmBuild::inspect_envelope(&cross_schema, cross_schema.len()).is_err());
    }
}
