//! Private deterministic WIT/component-boundary evidence.
//!
//! This freezes one scalar result/status interface and its JavaScript adapter,
//! a standalone scalar Component Model v1 fixture, and checked generated-core
//! component v2 evidence. None is a public WIT import/export surface.

use sha2::{Digest, Sha256};

use crate::{ast::Program, diagnostic::Diagnostic, graph, wasm};

mod generic_record_v7;
mod nested_record_v6;
mod record_pattern_v8;
mod result_v3;
mod scalar_algebra_v5;
mod source_result_v4;

pub use generic_record_v7::{
    emit_private_generic_record_component_v7, validate_private_generic_record_component_v7,
    PrivateGenericRecordComponentArtifactV7, ValidatedPrivateGenericRecordComponentV7,
};
pub use nested_record_v6::{
    emit_private_nested_record_component_v6, validate_private_nested_record_component_v6,
    PrivateNestedRecordComponentArtifactV6, ValidatedPrivateNestedRecordComponentV6,
};
pub use record_pattern_v8::{
    emit_private_record_pattern_component_v8, validate_private_record_pattern_component_v8,
    PrivateRecordPatternComponentArtifactV8, ValidatedPrivateRecordPatternComponentV8,
};
pub use result_v3::{
    emit_private_result_component_v3, validate_private_result_component_v3,
    PrivateResultComponentArtifactV3, ValidatedPrivateResultComponentV3,
};
pub use scalar_algebra_v5::{
    emit_private_scalar_algebra_component_v5, validate_private_scalar_algebra_component_v5,
    PrivateScalarAlgebraComponentArtifactV5, ValidatedPrivateScalarAlgebraComponentV5,
};
pub use source_result_v4::{
    emit_private_source_result_component_v4, validate_private_source_result_component_v4,
    PrivateSourceResultComponentArtifactV4, ValidatedPrivateSourceResultComponentV4,
};

const MAGIC: &[u8; 8] = b"SPXWIT01";

const WIT: &str = "package semaprax:private@0.1.0;\n\ninterface evaluation {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  evaluate: func(left: s64, right: s64) -> result<s64, status>;\n}\n\nworld semaprax-private-v1 {\n  export evaluation;\n}\n";

const SCHEMA: &str = "{\"abi\":\"wasm-component-canonical-v1\",\"copy\":{\"status.domain\":\"utf8-copy\"},\"interface\":\"semaprax:private/evaluation@0.1.0\",\"mapping\":{\"status.domain\":\"semaprax.status.v1.domain_id\"},\"result\":{\"err\":\"status\",\"ok\":\"s64\"},\"version\":1}";

const JAVASCRIPT: &str = r#"function spxOwnDataSnapshot(candidate, error) {
  if (candidate === null || typeof candidate !== "object" || Array.isArray(candidate)) throw new TypeError(error);
  let descriptors;
  try { descriptors = Object.getOwnPropertyDescriptors(candidate); }
  catch (_error) { throw new TypeError(error); }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key !== "string") throw new TypeError(error);
    const descriptor = descriptors[key];
    if (!descriptor.enumerable || !Object.prototype.hasOwnProperty.call(descriptor, "value")) throw new TypeError(error);
    snapshot[key] = descriptor.value;
  }
  return snapshot;
}

export function normalizeEvaluation(result) {
  const snapshot = spxOwnDataSnapshot(result, "SPX-WIT-RESULT");
  const keys = Object.keys(snapshot);
  if (keys.length !== 1 || (keys[0] !== "ok" && keys[0] !== "err")) throw new TypeError("SPX-WIT-TAG");
  if (keys[0] === "ok") {
    if (typeof snapshot.ok !== "bigint") throw new TypeError("SPX-WIT-I64");
    return { ok: snapshot.ok };
  }
  const value = spxOwnDataSnapshot(snapshot.err, "SPX-WIT-STATUS");
  const statusKeys = Object.keys(value).sort().join(",");
  const domainBytes = typeof value.domain === "string" ? new TextEncoder().encode(value.domain) : null;
  if (statusKeys !== "class,code,domain,retryable" || domainBytes === null ||
      domainBytes.length < 1 || domainBytes.length > 255 || domainBytes.includes(0) ||
      new TextDecoder("utf-8", { fatal: true }).decode(domainBytes) !== value.domain ||
      typeof value.code !== "number" || !Number.isInteger(value.code) || value.code <= 0 || value.code > 0xFFFF_FFFF ||
      typeof value.class !== "number" || !Number.isInteger(value.class) || value.class < 1 || value.class > 5 ||
      !(value.retryable === null || typeof value.retryable === "boolean")) throw new TypeError("SPX-WIT-STATUS");
  return { err: { domain: value.domain, code: value.code, class: value.class, retryable: value.retryable } };
}
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateWitBundleV1 {
    pub wit: &'static str,
    pub schema_json: &'static str,
    pub javascript_adapter: &'static str,
    pub digest: [u8; 32],
    bytes: Vec<u8>,
}

#[must_use]
pub fn emit_private_wit_bundle_v1() -> PrivateWitBundleV1 {
    let mut bytes = Vec::with_capacity(20 + WIT.len() + SCHEMA.len() + JAVASCRIPT.len());
    bytes.extend_from_slice(MAGIC);
    for field in [WIT.as_bytes(), SCHEMA.as_bytes(), JAVASCRIPT.as_bytes()] {
        bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
        bytes.extend_from_slice(field);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    PrivateWitBundleV1 {
        wit: WIT,
        schema_json: SCHEMA,
        javascript_adapter: JAVASCRIPT,
        digest,
        bytes,
    }
}

impl PrivateWitBundleV1 {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

const COMPONENT_HEADER: &[u8; 8] = b"\0asm\x0d\0\x01\0";

/// A deterministic, standards-valid Component Model binary for the private
/// scalar-success profile.
///
/// This artifact deliberately does not implement the `SPXWIT01`
/// `result<s64, status>` interface. It proves the smaller binary mechanics:
/// one embedded import-free core module, one canonical scalar lift, and one
/// component function export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateComponentArtifactV1 {
    bytes: Vec<u8>,
    pub digest: [u8; 32],
}

impl PrivateComponentArtifactV1 {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Exact failures produced by the independent private component parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateComponentValidationError {
    Header,
    Encoding,
    Profile,
    CoreModule,
}

impl PrivateComponentValidationError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Header => "SPX-WIT101",
            Self::Encoding => "SPX-WIT102",
            Self::Profile => "SPX-WIT103",
            Self::CoreModule => "SPX-WIT104",
        }
    }
}

/// The only data admitted from a validated private component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateComponentV1<'a> {
    core_module: &'a [u8],
}

impl<'a> ValidatedPrivateComponentV1<'a> {
    #[must_use]
    pub const fn core_module(self) -> &'a [u8] {
        self.core_module
    }

    #[must_use]
    pub const fn export_name(self) -> &'static str {
        "evaluate"
    }
}

/// Emit the exact private scalar Component Model profile.
#[must_use]
pub fn emit_private_component_v1() -> PrivateComponentArtifactV1 {
    let core = emit_private_component_core_v1();
    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, &core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);

    let mut alias = vec![0x00, 0x00, 0x01, 0x00];
    push_name(&mut alias, "evaluate");
    push_counted_section(&mut bytes, 6, 1, &alias);

    let mut function_type = vec![0x40, 0x02];
    push_name(&mut function_type, "left");
    function_type.push(0x78); // component s64
    push_name(&mut function_type, "right");
    function_type.extend([0x78, 0x00, 0x78]); // s64 param, one unnamed s64 result
    push_counted_section(&mut bytes, 7, 1, &function_type);

    push_counted_section(&mut bytes, 8, 1, &[0x00, 0x00, 0x00, 0x00, 0x00]);
    let mut export = vec![0x00];
    push_name(&mut export, "evaluate");
    export.extend([0x01, 0x00, 0x00]); // component func 0, no type ascription
    push_counted_section(&mut bytes, 11, 1, &export);

    let digest = Sha256::digest(&bytes).into();
    PrivateComponentArtifactV1 { bytes, digest }
}

/// Independently parse and validate the private component profile.
///
/// This parser does not call the emitter and never repairs, sorts, or ignores
/// sections. Lengths and integers must use canonical unsigned LEB128.
pub fn validate_private_component_v1(
    candidate: &[u8],
) -> Result<ValidatedPrivateComponentV1<'_>, PrivateComponentValidationError> {
    let mut component = Cursor::new(candidate);
    if component.take(8)? != COMPONENT_HEADER {
        return Err(PrivateComponentValidationError::Header);
    }

    let core = component.section(1)?;
    validate_private_component_core_v1(core)?;
    validate_exact_counted_section(
        component.section(2)?,
        &[0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;

    let mut alias = Cursor::new(component.section(6)?);
    alias.expect_u32(1, PrivateComponentValidationError::Profile)?;
    alias.expect_bytes(
        &[0x00, 0x00, 0x01, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    alias.expect_name("evaluate", PrivateComponentValidationError::Profile)?;
    alias.finish(PrivateComponentValidationError::Profile)?;

    let mut ty = Cursor::new(component.section(7)?);
    ty.expect_u32(1, PrivateComponentValidationError::Profile)?;
    ty.expect_bytes(&[0x40], PrivateComponentValidationError::Profile)?;
    ty.expect_u32(2, PrivateComponentValidationError::Profile)?;
    ty.expect_name("left", PrivateComponentValidationError::Profile)?;
    ty.expect_bytes(&[0x78], PrivateComponentValidationError::Profile)?;
    ty.expect_name("right", PrivateComponentValidationError::Profile)?;
    ty.expect_bytes(
        &[0x78, 0x00, 0x78],
        PrivateComponentValidationError::Profile,
    )?;
    ty.finish(PrivateComponentValidationError::Profile)?;

    validate_exact_counted_section(
        component.section(8)?,
        &[0x00, 0x00, 0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;

    let mut export = Cursor::new(component.section(11)?);
    export.expect_u32(1, PrivateComponentValidationError::Profile)?;
    export.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    export.expect_name("evaluate", PrivateComponentValidationError::Profile)?;
    export.expect_bytes(
        &[0x01, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    export.finish(PrivateComponentValidationError::Profile)?;
    component.finish(PrivateComponentValidationError::Profile)?;

    Ok(ValidatedPrivateComponentV1 { core_module: core })
}

/// Dependency-free JavaScript runtime for the exact private scalar component
/// profile. It snapshots and authenticates the component container and exact
/// core module, then delegates execution to the host WebAssembly engine.
#[must_use]
pub const fn private_component_runtime_javascript_v1() -> &'static str {
    PRIVATE_COMPONENT_RUNTIME_JAVASCRIPT_V1
}

const PRIVATE_COMPONENT_RUNTIME_JAVASCRIPT_V1: &str = r#"function spxComponentCursor(bytes) {
  let offset = 0;
  const take = count => {
    if (!Number.isInteger(count) || count < 0 || offset + count > bytes.length) throw new TypeError("SPX-WIT102");
    const value = bytes.subarray(offset, offset + count); offset += count; return value;
  };
  const u32 = () => {
    const start = offset; let value = 0; let shift = 0;
    for (let index = 0; index < 5; index++) {
      const byte = take(1)[0];
      if (index === 4 && (byte & 0xf0) !== 0) throw new TypeError("SPX-WIT102");
      value |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) {
        const encoded = []; let canonical = value >>> 0;
        do { let part = canonical & 0x7f; canonical >>>= 7; if (canonical !== 0) part |= 0x80; encoded.push(part); } while (canonical !== 0);
        if (offset - start !== encoded.length) throw new TypeError("SPX-WIT102");
        return value >>> 0;
      }
      shift += 7;
    }
    throw new TypeError("SPX-WIT102");
  };
  return { take, u32, done: () => offset === bytes.length };
}

export async function instantiatePrivateScalarComponentV1(candidate) {
  const source = candidate instanceof Uint8Array ? candidate : new Uint8Array(candidate);
  const bytes = Uint8Array.from(source);
  const cursor = spxComponentCursor(bytes);
  const header = cursor.take(8);
  const expectedHeader = [0,97,115,109,13,0,1,0];
  if (!expectedHeader.every((byte, index) => header[index] === byte)) throw new TypeError("SPX-WIT101");
  const expectedSections = [1,2,6,7,8,11];
  const sections = new Map();
  for (const expected of expectedSections) {
    if (cursor.take(1)[0] !== expected) throw new TypeError("SPX-WIT103");
    const payload = cursor.take(cursor.u32());
    sections.set(expected, payload);
  }
  if (!cursor.done()) throw new TypeError("SPX-WIT103");
  const exact = (id, expected) => {
    const actual = sections.get(id);
    if (actual.length !== expected.length || !expected.every((byte, index) => actual[index] === byte)) throw new TypeError("SPX-WIT103");
  };
  exact(1, [0,97,115,109,1,0,0,0,1,7,1,96,2,126,126,1,126,3,2,1,0,7,12,1,8,101,118,97,108,117,97,116,101,0,0,10,9,1,7,0,32,0,32,1,124,11]);
  exact(2, [1,0,0,0]);
  exact(6, [1,0,0,1,0,8,101,118,97,108,117,97,116,101]);
  exact(7, [1,64,2,4,108,101,102,116,120,5,114,105,103,104,116,120,0,120]);
  exact(8, [1,0,0,0,0,0]);
  exact(11, [1,0,8,101,118,97,108,117,97,116,101,1,0,0]);
  const { instance } = await WebAssembly.instantiate(sections.get(1), {});
  if (typeof instance.exports.evaluate !== "function") throw new TypeError("SPX-WIT104");
  const i64Minimum = -(1n << 63n);
  const i64Maximum = (1n << 63n) - 1n;
  return Object.freeze({
    evaluate(left, right) {
      if (typeof left !== "bigint" || typeof right !== "bigint" ||
          left < i64Minimum || left > i64Maximum || right < i64Minimum || right > i64Maximum) throw new TypeError("SPX-WIT-I64");
      const result = instance.exports.evaluate(left, right);
      if (typeof result !== "bigint") throw new TypeError("SPX-WIT104");
      return result;
    },
  });
}
"#;

/// A deterministic private Component Model artifact whose application core is
/// emitted by the checked SEMAPRAX Wasm backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateCheckedComponentArtifactV2 {
    bytes: Vec<u8>,
    digest: [u8; 32],
    generated_core_digest: [u8; 32],
    runtime_core_digest: [u8; 32],
    source_revision: String,
}

impl PrivateCheckedComponentArtifactV2 {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    #[must_use]
    pub const fn generated_core_digest(&self) -> [u8; 32] {
        self.generated_core_digest
    }

    #[must_use]
    pub const fn runtime_core_digest(&self) -> [u8; 32] {
        self.runtime_core_digest
    }
}

/// Independently admitted cores from the private checked-component profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPrivateCheckedComponentV2<'a> {
    runtime_core: &'a [u8],
    generated_core: &'a [u8],
}

impl<'a> ValidatedPrivateCheckedComponentV2<'a> {
    #[must_use]
    pub const fn runtime_core(self) -> &'a [u8] {
        self.runtime_core
    }

    #[must_use]
    pub const fn generated_core(self) -> &'a [u8] {
        self.generated_core
    }

    #[must_use]
    pub const fn export_name(self) -> &'static str {
        "evaluate"
    }
}

const CHECKED_RUNTIME_CORE_V2_SHA256: [u8; 32] = [
    107, 218, 111, 164, 153, 237, 95, 189, 213, 36, 9, 38, 76, 115, 211, 151, 91, 30, 214, 179, 58,
    220, 84, 251, 168, 250, 95, 166, 49, 17, 148, 45,
];

/// Compose one verified SEMAPRAX program with the private checked core runtime
/// and lift its generated `semaprax_main() -> i64` export as
/// `evaluate() -> s64`.
pub fn emit_private_checked_component_v2(
    program: &Program,
) -> Result<PrivateCheckedComponentArtifactV2, Diagnostic> {
    let generated_core = wasm::emit_module(program)?;
    validate_generated_scalar_core_v2(&generated_core).map_err(|_| {
        Diagnostic::io(
            "SPX-WIT106",
            "private checked component v2 admits only the scalar core import/export profile",
        )
    })?;
    let runtime_core = emit_checked_runtime_core_v2();
    let generated_core_digest = Sha256::digest(&generated_core).into();
    let runtime_core_digest = Sha256::digest(&runtime_core).into();
    let source_revision = graph::revision(program);

    let mut bytes = COMPONENT_HEADER.to_vec();
    push_section(&mut bytes, 1, &runtime_core);
    push_counted_section(&mut bytes, 2, 1, &[0x00, 0x00, 0x00]);
    push_section(&mut bytes, 1, &generated_core);

    let mut generated_instance = vec![0x00, 0x01, 0x01];
    push_name(&mut generated_instance, "env");
    generated_instance.extend([0x12, 0x00]);
    push_counted_section(&mut bytes, 2, 1, &generated_instance);

    let mut alias = vec![0x00, 0x00, 0x01, 0x01];
    push_name(&mut alias, "semaprax_main");
    push_counted_section(&mut bytes, 6, 1, &alias);
    push_counted_section(&mut bytes, 7, 1, &[0x40, 0x00, 0x00, 0x78]);
    push_counted_section(&mut bytes, 8, 1, &[0x00, 0x00, 0x00, 0x00, 0x00]);
    let mut export = vec![0x00];
    push_name(&mut export, "evaluate");
    export.extend([0x01, 0x00, 0x00]);
    push_counted_section(&mut bytes, 11, 1, &export);

    let digest = Sha256::digest(&bytes).into();
    Ok(PrivateCheckedComponentArtifactV2 {
        bytes,
        digest,
        generated_core_digest,
        runtime_core_digest,
        source_revision,
    })
}

/// Independently parse the checked-component profile and bind its application
/// core to the compiler-provided expected digest.
pub fn validate_private_checked_component_v2(
    candidate: &[u8],
    expected_generated_core_digest: [u8; 32],
) -> Result<ValidatedPrivateCheckedComponentV2<'_>, PrivateComponentValidationError> {
    let mut component = Cursor::new(candidate);
    if component.take(8)? != COMPONENT_HEADER {
        return Err(PrivateComponentValidationError::Header);
    }
    let runtime_core = component.section(1)?;
    if <[u8; 32]>::from(Sha256::digest(runtime_core)) != CHECKED_RUNTIME_CORE_V2_SHA256 {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    validate_exact_counted_section(
        component.section(2)?,
        &[0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    let generated_core = component.section(1)?;
    if <[u8; 32]>::from(Sha256::digest(generated_core)) != expected_generated_core_digest {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    validate_generated_scalar_core_v2(generated_core)?;
    let mut instance = Cursor::new(component.section(2)?);
    instance.expect_u32(1, PrivateComponentValidationError::Profile)?;
    instance.expect_bytes(
        &[0x00, 0x01, 0x01],
        PrivateComponentValidationError::Profile,
    )?;
    instance.expect_name("env", PrivateComponentValidationError::Profile)?;
    instance.expect_bytes(&[0x12, 0x00], PrivateComponentValidationError::Profile)?;
    instance.finish(PrivateComponentValidationError::Profile)?;

    let mut alias = Cursor::new(component.section(6)?);
    alias.expect_u32(1, PrivateComponentValidationError::Profile)?;
    alias.expect_bytes(
        &[0x00, 0x00, 0x01, 0x01],
        PrivateComponentValidationError::Profile,
    )?;
    alias.expect_name("semaprax_main", PrivateComponentValidationError::Profile)?;
    alias.finish(PrivateComponentValidationError::Profile)?;
    validate_exact_counted_section(
        component.section(7)?,
        &[0x40, 0x00, 0x00, 0x78],
        PrivateComponentValidationError::Profile,
    )?;
    validate_exact_counted_section(
        component.section(8)?,
        &[0x00, 0x00, 0x00, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    let mut export = Cursor::new(component.section(11)?);
    export.expect_u32(1, PrivateComponentValidationError::Profile)?;
    export.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    export.expect_name("evaluate", PrivateComponentValidationError::Profile)?;
    export.expect_bytes(
        &[0x01, 0x00, 0x00],
        PrivateComponentValidationError::Profile,
    )?;
    export.finish(PrivateComponentValidationError::Profile)?;
    component.finish(PrivateComponentValidationError::Profile)?;
    Ok(ValidatedPrivateCheckedComponentV2 {
        runtime_core,
        generated_core,
    })
}

/// Emit an artifact-bound, dependency-free runtime for the private checked
/// component profile. The runtime authenticates and parses the whole component
/// before executing its two core instances through the host WebAssembly engine.
#[must_use]
pub fn private_checked_component_runtime_javascript_v2(
    artifact: &PrivateCheckedComponentArtifactV2,
) -> String {
    format!(
        "const SPX_CHECKED_COMPONENT_V2_SHA256 = \"{}\";\n{}",
        hex_digest(&Sha256::digest(artifact.bytes()).into()),
        PRIVATE_CHECKED_COMPONENT_RUNTIME_JAVASCRIPT_V2
    )
}

const PRIVATE_CHECKED_COMPONENT_RUNTIME_JAVASCRIPT_V2: &str = r#"function spxCheckedComponentCursor(bytes) {
  let offset = 0;
  const take = count => {
    if (!Number.isInteger(count) || count < 0 || offset + count > bytes.length) throw new TypeError("SPX-WIT102");
    const value = bytes.subarray(offset, offset + count); offset += count; return value;
  };
  const u32 = () => {
    const start = offset; let value = 0; let shift = 0;
    for (let index = 0; index < 5; index++) {
      const byte = take(1)[0];
      if (index === 4 && (byte & 0xf0) !== 0) throw new TypeError("SPX-WIT102");
      value |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) {
        const encoded = []; let canonical = value >>> 0;
        do { let part = canonical & 0x7f; canonical >>>= 7; if (canonical !== 0) part |= 0x80; encoded.push(part); } while (canonical !== 0);
        if (offset - start !== encoded.length) throw new TypeError("SPX-WIT102");
        return value >>> 0;
      }
      shift += 7;
    }
    throw new TypeError("SPX-WIT102");
  };
  return { take, u32, done: () => offset === bytes.length };
}

async function spxCheckedComponentDigest(bytes) {
  if (globalThis.crypto === undefined || globalThis.crypto.subtle === undefined) throw new TypeError("SPX-WIT105");
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("");
}

export async function instantiatePrivateCheckedComponentV2(candidate) {
  let source;
  if (candidate instanceof ArrayBuffer) source = new Uint8Array(candidate);
  else if (ArrayBuffer.isView(candidate)) source = new Uint8Array(candidate.buffer, candidate.byteOffset, candidate.byteLength);
  else throw new TypeError("SPX-WIT102");
  const bytes = Uint8Array.from(source);
  if (await spxCheckedComponentDigest(bytes) !== SPX_CHECKED_COMPONENT_V2_SHA256) throw new TypeError("SPX-WIT105");

  const cursor = spxCheckedComponentCursor(bytes);
  const expectedHeader = [0,97,115,109,13,0,1,0];
  const header = cursor.take(8);
  if (!expectedHeader.every((byte, index) => header[index] === byte)) throw new TypeError("SPX-WIT101");
  const section = id => {
    if (cursor.take(1)[0] !== id) throw new TypeError("SPX-WIT103");
    return cursor.take(cursor.u32());
  };
  const exact = (actual, expected) => {
    if (actual.length !== expected.length || !expected.every((byte, index) => actual[index] === byte)) throw new TypeError("SPX-WIT103");
  };
  const runtimeCore = section(1);
  exact(section(2), [1,0,0,0]);
  const generatedCore = section(1);
  exact(section(2), [1,0,1,1,3,101,110,118,18,0]);
  exact(section(6), [1,0,0,1,1,13,115,101,109,97,112,114,97,120,95,109,97,105,110]);
  exact(section(7), [1,64,0,0,120]);
  exact(section(8), [1,0,0,0,0,0]);
  exact(section(11), [1,0,8,101,118,97,108,117,97,116,101,1,0,0]);
  if (!cursor.done()) throw new TypeError("SPX-WIT103");

  const runtime = (await WebAssembly.instantiate(runtimeCore, {})).instance;
  const generated = (await WebAssembly.instantiate(generatedCore, { env: runtime.exports })).instance;
  if (typeof generated.exports.semaprax_main !== "function") throw new TypeError("SPX-WIT104");
  return Object.freeze({
    evaluate() {
      if (arguments.length !== 0) throw new TypeError("SPX-WIT-I64");
      const result = generated.exports.semaprax_main();
      if (typeof result !== "bigint") throw new TypeError("SPX-WIT104");
      return result;
    },
  });
}
"#;

fn hex_digest(digest: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn validate_generated_scalar_core_v2(
    candidate: &[u8],
) -> Result<(), PrivateComponentValidationError> {
    let mut module = Cursor::new(candidate);
    if module.take(8)? != b"\0asm\x01\0\0\0" {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    let types = module.section(1)?;
    if types.is_empty() {
        return Err(PrivateComponentValidationError::Profile);
    }
    let mut imports = Cursor::new(module.section(2)?);
    imports.expect_u32(7, PrivateComponentValidationError::Profile)?;
    for (name, type_index) in [
        ("spx_add", 0),
        ("spx_sub", 0),
        ("spx_mul", 0),
        ("spx_div", 0),
        ("spx_rem", 0),
        ("spx_neg", 1),
        ("spx_contract_fail", 2),
    ] {
        imports.expect_name("env", PrivateComponentValidationError::Profile)?;
        imports.expect_name(name, PrivateComponentValidationError::Profile)?;
        imports.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
        imports.expect_u32(type_index, PrivateComponentValidationError::Profile)?;
    }
    imports.finish(PrivateComponentValidationError::Profile)?;

    let functions = module.section(3)?;
    if functions.is_empty() {
        return Err(PrivateComponentValidationError::Profile);
    }
    let mut exports = Cursor::new(module.section(7)?);
    exports.expect_u32(1, PrivateComponentValidationError::Profile)?;
    exports.expect_name("semaprax_main", PrivateComponentValidationError::Profile)?;
    exports.expect_bytes(&[0x00], PrivateComponentValidationError::Profile)?;
    if exports.u32()? < 7 {
        return Err(PrivateComponentValidationError::Profile);
    }
    exports.finish(PrivateComponentValidationError::Profile)?;
    if module.section(10)?.is_empty() {
        return Err(PrivateComponentValidationError::Profile);
    }
    module.finish(PrivateComponentValidationError::Profile)
}

fn emit_checked_runtime_core_v2() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    push_section(
        &mut module,
        1,
        &[
            0x03, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e, 0x60, 0x01, 0x7e, 0x01, 0x7e, 0x60, 0x00,
            0x00,
        ],
    );
    push_section(
        &mut module,
        3,
        &[0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02],
    );
    let mut exports = Vec::new();
    push_u32(&mut exports, 7);
    for (index, name) in [
        "spx_add",
        "spx_sub",
        "spx_mul",
        "spx_div",
        "spx_rem",
        "spx_neg",
        "spx_contract_fail",
    ]
    .into_iter()
    .enumerate()
    {
        push_name(&mut exports, name);
        exports.push(0x00);
        push_u32(&mut exports, index as u32);
    }
    push_section(&mut module, 7, &exports);

    let mut code = Vec::new();
    push_u32(&mut code, 7);
    push_core_body(&mut code, &[0x01, 0x01, 0x7e], &checked_add_body());
    push_core_body(&mut code, &[0x01, 0x01, 0x7e], &checked_sub_body());
    push_core_body(&mut code, &[0x01, 0x01, 0x7e], &checked_mul_body());
    push_core_body(&mut code, &[0x00], &[0x20, 0x00, 0x20, 0x01, 0x7f]);
    push_core_body(&mut code, &[0x00], &checked_rem_body());
    push_core_body(&mut code, &[0x00], &checked_neg_body());
    push_core_body(&mut code, &[0x00], &[0x00]);
    push_section(&mut module, 10, &code);
    module
}

fn checked_add_body() -> Vec<u8> {
    vec![
        0x20, 0x00, 0x20, 0x01, 0x7c, 0x21, 0x02, 0x20, 0x00, 0x20, 0x02, 0x85, 0x20, 0x01, 0x20,
        0x02, 0x85, 0x83, 0x42, 0x00, 0x53, 0x04, 0x40, 0x00, 0x0b, 0x20, 0x02,
    ]
}

fn checked_sub_body() -> Vec<u8> {
    vec![
        0x20, 0x00, 0x20, 0x01, 0x7d, 0x21, 0x02, 0x20, 0x00, 0x20, 0x01, 0x85, 0x20, 0x00, 0x20,
        0x02, 0x85, 0x83, 0x42, 0x00, 0x53, 0x04, 0x40, 0x00, 0x0b, 0x20, 0x02,
    ]
}

fn checked_mul_body() -> Vec<u8> {
    vec![
        0x20, 0x00, 0x20, 0x01, 0x7e, 0x21, 0x02, 0x20, 0x01, 0x50, 0x04, 0x7e, 0x20, 0x02, 0x05,
        0x20, 0x02, 0x20, 0x01, 0x7f, 0x20, 0x00, 0x52, 0x04, 0x40, 0x00, 0x0b, 0x20, 0x02, 0x0b,
    ]
}

fn checked_rem_body() -> Vec<u8> {
    let mut body = vec![0x20, 0x01, 0x50, 0x04, 0x40, 0x00, 0x0b, 0x20, 0x00, 0x42];
    push_i64(&mut body, i64::MIN);
    body.extend([0x51, 0x20, 0x01, 0x42]);
    push_i64(&mut body, -1);
    body.extend([
        0x51, 0x71, 0x04, 0x40, 0x00, 0x0b, 0x20, 0x00, 0x20, 0x01, 0x81,
    ]);
    body
}

fn checked_neg_body() -> Vec<u8> {
    let mut body = vec![0x20, 0x00, 0x42];
    push_i64(&mut body, i64::MIN);
    body.extend([0x51, 0x04, 0x40, 0x00, 0x0b, 0x42, 0x00, 0x20, 0x00, 0x7d]);
    body
}

fn push_core_body(code: &mut Vec<u8>, locals: &[u8], instructions: &[u8]) {
    let mut body = locals.to_vec();
    body.extend_from_slice(instructions);
    body.push(0x0b);
    push_u32(code, body.len() as u32);
    code.extend_from_slice(&body);
}

fn push_i64(output: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        output.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

fn emit_private_component_core_v1() -> Vec<u8> {
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    push_section(&mut module, 1, &[0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e]);
    push_section(&mut module, 3, &[0x01, 0x00]);
    let mut export = vec![0x01];
    push_name(&mut export, "evaluate");
    export.extend([0x00, 0x00]);
    push_section(&mut module, 7, &export);
    push_section(
        &mut module,
        10,
        &[0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x7c, 0x0b],
    );
    module
}

fn validate_private_component_core_v1(
    candidate: &[u8],
) -> Result<(), PrivateComponentValidationError> {
    let mut module = Cursor::new(candidate);
    if module.take(8)? != b"\0asm\x01\0\0\0" {
        return Err(PrivateComponentValidationError::CoreModule);
    }
    validate_exact_payload(
        module.section(1)?,
        &[0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e],
        PrivateComponentValidationError::CoreModule,
    )?;
    validate_exact_payload(
        module.section(3)?,
        &[0x01, 0x00],
        PrivateComponentValidationError::CoreModule,
    )?;
    let mut export = Cursor::new(module.section(7)?);
    export.expect_u32(1, PrivateComponentValidationError::CoreModule)?;
    export.expect_name("evaluate", PrivateComponentValidationError::CoreModule)?;
    export.expect_bytes(&[0x00, 0x00], PrivateComponentValidationError::CoreModule)?;
    export.finish(PrivateComponentValidationError::CoreModule)?;
    validate_exact_payload(
        module.section(10)?,
        &[0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x7c, 0x0b],
        PrivateComponentValidationError::CoreModule,
    )?;
    module.finish(PrivateComponentValidationError::CoreModule)
}

fn validate_exact_counted_section(
    candidate: &[u8],
    entries: &[u8],
    error: PrivateComponentValidationError,
) -> Result<(), PrivateComponentValidationError> {
    let mut cursor = Cursor::new(candidate);
    cursor.expect_u32(1, error)?;
    cursor.expect_bytes(entries, error)?;
    cursor.finish(error)
}

fn validate_exact_payload(
    actual: &[u8],
    expected: &[u8],
    error: PrivateComponentValidationError,
) -> Result<(), PrivateComponentValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error)
    }
}

fn push_counted_section(output: &mut Vec<u8>, id: u8, count: u32, entries: &[u8]) {
    let mut payload = Vec::new();
    push_u32(&mut payload, count);
    payload.extend_from_slice(entries);
    push_section(output, id, &payload);
}

fn push_section(output: &mut Vec<u8>, id: u8, payload: &[u8]) {
    output.push(id);
    push_u32(
        output,
        u32::try_from(payload.len()).expect("private component payload fits u32"),
    );
    output.extend_from_slice(payload);
}

fn push_name(output: &mut Vec<u8>, name: &str) {
    push_u32(
        output,
        u32::try_from(name.len()).expect("private component name fits u32"),
    );
    output.extend_from_slice(name.as_bytes());
}

fn push_u32(output: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

#[derive(Clone, Copy)]
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PrivateComponentValidationError> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(PrivateComponentValidationError::Encoding)?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn u32(&mut self) -> Result<u32, PrivateComponentValidationError> {
        let start = self.offset;
        let mut value = 0_u32;
        for index in 0..5 {
            let byte = self.take(1)?[0];
            if index == 4 && byte & 0xf0 != 0 {
                return Err(PrivateComponentValidationError::Encoding);
            }
            value |= u32::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                let mut canonical = Vec::new();
                push_u32(&mut canonical, value);
                if canonical.len() != self.offset - start {
                    return Err(PrivateComponentValidationError::Encoding);
                }
                return Ok(value);
            }
        }
        Err(PrivateComponentValidationError::Encoding)
    }

    fn section(&mut self, expected_id: u8) -> Result<&'a [u8], PrivateComponentValidationError> {
        if self.take(1)?[0] != expected_id {
            return Err(PrivateComponentValidationError::Profile);
        }
        let length =
            usize::try_from(self.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
        self.take(length)
    }

    fn expect_u32(
        &mut self,
        expected: u32,
        error: PrivateComponentValidationError,
    ) -> Result<(), PrivateComponentValidationError> {
        if self.u32()? == expected {
            Ok(())
        } else {
            Err(error)
        }
    }

    fn expect_bytes(
        &mut self,
        expected: &[u8],
        error: PrivateComponentValidationError,
    ) -> Result<(), PrivateComponentValidationError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(error)
        }
    }

    fn expect_name(
        &mut self,
        expected: &str,
        error: PrivateComponentValidationError,
    ) -> Result<(), PrivateComponentValidationError> {
        let length =
            usize::try_from(self.u32()?).map_err(|_| PrivateComponentValidationError::Encoding)?;
        if self.take(length)? == expected.as_bytes() {
            Ok(())
        } else {
            Err(error)
        }
    }

    fn finish(
        self,
        error: PrivateComponentValidationError,
    ) -> Result<(), PrivateComponentValidationError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(error)
        }
    }
}

pub fn verify_private_wit_bundle_v1(candidate: &[u8]) -> Result<(), &'static str> {
    if candidate.len() < 20 || &candidate[..8] != MAGIC {
        return Err("SPX-WIT001");
    }
    let expected = emit_private_wit_bundle_v1();
    if candidate != expected.bytes() {
        return Err("SPX-WIT002");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_CONSUMER: AtomicU64 = AtomicU64::new(0);

    const CHECKED_COMPONENT_SOURCE: &str = r#"
module test.checked_component;

@id("app.main")
fn main() -> i64 { 19 + 23 }
"#;

    fn checked_component_program() -> crate::ast::Program {
        crate::parse(CHECKED_COMPONENT_SOURCE, Path::new("checked-component.spx")).unwrap()
    }

    fn replace_unique_byte(bytes: &mut [u8], needle: &[u8], relative_index: usize, value: u8) {
        let matches = bytes
            .windows(needle.len())
            .enumerate()
            .filter_map(|(index, candidate)| (candidate == needle).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "hostile mutation anchor is not unique");
        bytes[matches[0] + relative_index] = value;
    }

    fn rehashed_artifact(
        original: &PrivateCheckedComponentArtifactV2,
        bytes: Vec<u8>,
    ) -> PrivateCheckedComponentArtifactV2 {
        let mut hostile = original.clone();
        hostile.bytes = bytes;
        hostile.digest = Sha256::digest(hostile.bytes()).into();
        hostile
    }

    struct ConsumerDirectory(PathBuf);

    impl ConsumerDirectory {
        fn create() -> Self {
            let ordinal = NEXT_CONSUMER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "semaprax-wit-default-surface-{}-{ordinal}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for ConsumerDirectory {
        fn drop(&mut self) {
            let Ok(metadata) = std::fs::symlink_metadata(&self.0) else {
                return;
            };
            if metadata.file_type().is_symlink() || metadata.is_file() {
                let _ = std::fs::remove_file(&self.0);
            } else if metadata.is_dir() {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn bundle_is_deterministic_canonical_and_mutation_closed() {
        let first = emit_private_wit_bundle_v1();
        let second = emit_private_wit_bundle_v1();
        assert_eq!(first, second);
        assert_eq!(
            first.digest,
            [
                76, 15, 16, 105, 217, 44, 141, 85, 231, 174, 10, 165, 27, 215, 130, 15, 255, 119,
                167, 24, 57, 189, 52, 39, 33, 22, 186, 145, 27, 43, 153, 52,
            ]
        );
        assert_eq!(verify_private_wit_bundle_v1(first.bytes()), Ok(()));
        assert!(first.wit.contains("result<s64, status>"));
        assert!(!first.wit.contains("resource"));
        for index in 0..first.bytes().len() {
            let mut hostile = first.bytes().to_vec();
            hostile[index] ^= 1;
            assert_eq!(
                verify_private_wit_bundle_v1(&hostile),
                Err(if index < 8 {
                    "SPX-WIT001"
                } else {
                    "SPX-WIT002"
                })
            );
        }
        for end in 0..first.bytes().len() {
            assert!(verify_private_wit_bundle_v1(&first.bytes()[..end]).is_err());
        }
        let mut trailing = first.bytes().to_vec();
        trailing.push(0);
        assert_eq!(verify_private_wit_bundle_v1(&trailing), Err("SPX-WIT002"));
    }

    #[test]
    fn node_executes_exact_javascript_result_adapter() {
        let script = format!(
            "{JAVASCRIPT}\n{}",
            r#"const reject = candidate => {
  let rejected = false;
  try { normalizeEvaluation(candidate); } catch (_error) { rejected = true; }
  if (!rejected) process.exit(92);
};
const status = (domain, code = 7) => ({ err: { domain, code, class: 3, retryable: null } });
const ok = normalizeEvaluation({ ok: 7n });
const err = normalizeEvaluation({ err: { domain: "fixture.v1", code: 7, class: 3, retryable: false } });
if (ok.ok !== 7n || err.err.code !== 7) process.exit(91);
for (const hostile of [
  { ok: 7 }, {}, status("x", 0), status("x", 0x1_0000_0000), status("", 7),
  status("a".repeat(256), 7), status("a\0b", 7), status("€".repeat(86), 7),
  status("\uD800", 7), status("\uDC00", 7)
]) reject(hostile);

const asciiMax = normalizeEvaluation(status("a".repeat(255), 0xFFFF_FFFF));
const utf8Max = normalizeEvaluation(status("€".repeat(85), 7));
const paired = normalizeEvaluation(status("😀".repeat(63) + "abc", 7));
if (asciiMax.err.code !== 0xFFFF_FFFF || utf8Max.err.domain.length !== 85 || paired.err.domain.length !== 129) process.exit(93);

let getterReads = 0;
const changingGetter = {};
Object.defineProperty(changingGetter, "ok", { enumerable: true, get() { getterReads++; return getterReads === 1 ? 7n : 7; } });
reject(changingGetter);
reject({ ok: 7n, [Symbol("hostile")]: 1 });
const statusGetter = { code: 7, class: 3, retryable: null };
Object.defineProperty(statusGetter, "domain", { enumerable: true, get() { getterReads++; return "fixture.v1"; } });
reject({ err: statusGetter });
if (getterReads !== 0) process.exit(94);

let descriptorReads = 0;
let valueReads = 0;
const changingProxy = new Proxy({ ok: 7n }, {
  ownKeys() { return ["ok"]; },
  getOwnPropertyDescriptor() {
    descriptorReads++;
    return { configurable: true, enumerable: true, value: descriptorReads === 1 ? 7n : 7 };
  },
  get() { valueReads++; return 7; }
});
if (normalizeEvaluation(changingProxy).ok !== 7n || descriptorReads !== 1 || valueReads !== 0) process.exit(95);
reject(new Proxy({}, { ownKeys() { throw new Error("hostile"); } }));
"#
        );
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        assert!(
            output.status.success(),
            "Node adapter failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn component_binary_is_deterministic_independently_parsed_and_mutation_closed() {
        let first = emit_private_component_v1();
        let second = emit_private_component_v1();
        assert_eq!(first, second);
        assert_eq!(&first.bytes()[..8], COMPONENT_HEADER);
        assert_eq!(
            first.digest,
            [
                0x3e, 0xd6, 0xbe, 0xd8, 0x47, 0x2e, 0xea, 0xe0, 0xef, 0x17, 0xf9, 0x64, 0x58, 0x62,
                0x2c, 0x9a, 0xe0, 0x32, 0xdd, 0x7a, 0x13, 0xb1, 0x15, 0xd2, 0xd7, 0xfe, 0xa7, 0xfc,
                0xfe, 0xcd, 0xe6, 0x43,
            ]
        );
        let validated = validate_private_component_v1(first.bytes()).unwrap();
        assert_eq!(validated.export_name(), "evaluate");
        assert_eq!(&validated.core_module()[..8], b"\0asm\x01\0\0\0");

        for index in 0..first.bytes().len() {
            let mut hostile = first.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(
                validate_private_component_v1(&hostile).is_err(),
                "byte {index} was not authenticated by the profile parser"
            );
        }
        for end in 0..first.bytes().len() {
            assert!(validate_private_component_v1(&first.bytes()[..end]).is_err());
        }
        let mut trailing = first.bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            validate_private_component_v1(&trailing),
            Err(PrivateComponentValidationError::Profile)
        );

        let mut noncanonical_length = first.bytes().to_vec();
        noncanonical_length.splice(9..10, [0xae, 0x00]);
        assert_eq!(
            validate_private_component_v1(&noncanonical_length),
            Err(PrivateComponentValidationError::Encoding)
        );
    }

    #[test]
    fn checked_component_v2_is_generated_bound_and_independently_parsed() {
        let program = checked_component_program();
        let first = emit_private_checked_component_v2(&program).unwrap();
        let second = emit_private_checked_component_v2(&program).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.source_revision(), graph::revision(&program));
        assert_eq!(first.runtime_core_digest(), CHECKED_RUNTIME_CORE_V2_SHA256);
        assert_eq!(
            first.digest(),
            [
                192, 191, 163, 225, 184, 136, 50, 55, 236, 153, 52, 82, 12, 249, 177, 205, 178, 73,
                40, 157, 49, 141, 108, 208, 65, 62, 99, 183, 22, 112, 59, 192,
            ]
        );

        let validated =
            validate_private_checked_component_v2(first.bytes(), first.generated_core_digest())
                .unwrap();
        assert_eq!(validated.export_name(), "evaluate");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(validated.generated_core())),
            first.generated_core_digest()
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(validated.runtime_core())),
            first.runtime_core_digest()
        );
        for index in 0..first.bytes().len() {
            let mut hostile = first.bytes().to_vec();
            hostile[index] ^= 1;
            assert!(
                validate_private_checked_component_v2(&hostile, first.generated_core_digest())
                    .is_err(),
                "component v2 byte {index} was not authenticated"
            );
        }
        for end in 0..first.bytes().len() {
            assert!(validate_private_checked_component_v2(
                &first.bytes()[..end],
                first.generated_core_digest()
            )
            .is_err());
        }
        let mut trailing = first.bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            validate_private_checked_component_v2(&trailing, first.generated_core_digest()),
            Err(PrivateComponentValidationError::Profile)
        );
    }

    #[test]
    fn checked_component_v2_digest_is_read_only_and_javascript_uses_exact_bytes() {
        let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
        let expected = private_checked_component_runtime_javascript_v2(&artifact);
        let mut forged_metadata = artifact.clone();
        forged_metadata.digest = [0xa5; 32];
        assert_ne!(forged_metadata.digest(), artifact.digest());
        assert_eq!(
            private_checked_component_runtime_javascript_v2(&forged_metadata),
            expected,
            "JavaScript authorization trusted forgeable digest metadata"
        );
    }

    #[test]
    fn upstream_validator_rejects_rehashed_component_cross_type_hostiles() {
        let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
            .validate_all(artifact.bytes())
            .expect("pinned upstream validator rejected checked component v2");

        let validated = validate_private_checked_component_v2(
            artifact.bytes(),
            artifact.generated_core_digest(),
        )
        .unwrap();
        let generated_offset = artifact
            .bytes()
            .windows(validated.generated_core().len())
            .position(|candidate| candidate == validated.generated_core())
            .unwrap();
        let runtime_offset = artifact
            .bytes()
            .windows(validated.runtime_core().len())
            .position(|candidate| candidate == validated.runtime_core())
            .unwrap();

        let mut invalid_signature = artifact.bytes().to_vec();
        replace_unique_byte(
            &mut invalid_signature[runtime_offset..runtime_offset + validated.runtime_core().len()],
            &[7, b's', b'p', b'x', b'_', b'a', b'd', b'd', 0, 0],
            9,
            5,
        );
        let mut invalid_body = artifact.bytes().to_vec();
        let generated_end = generated_offset + validated.generated_core().len();
        invalid_body[generated_end - 1] = 0;
        let mut invalid_cardinality = artifact.bytes().to_vec();
        replace_unique_byte(&mut invalid_cardinality, &[6, 19, 1, 0, 0, 1, 1, 13], 2, 2);
        let mut invalid_canonical_lift = artifact.bytes().to_vec();
        replace_unique_byte(
            &mut invalid_canonical_lift,
            &[7, 5, 1, 64, 0, 0, 120],
            6,
            127,
        );

        for (name, bytes) in [
            ("signature", invalid_signature),
            ("body", invalid_body),
            ("cardinality", invalid_cardinality),
            ("canonical-lift", invalid_canonical_lift),
        ] {
            let hostile = rehashed_artifact(&artifact, bytes);
            assert_eq!(
                hostile.digest(),
                <[u8; 32]>::from(Sha256::digest(hostile.bytes()))
            );
            assert!(
                wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
                    .validate_all(hostile.bytes())
                    .is_err(),
                "pinned upstream validator admitted rehashed hostile {name}"
            );
        }
    }

    #[test]
    fn checked_component_v2_rejects_owned_core_profiles() {
        let program = crate::parse(
            r#"module test.checked_component_owned;
@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}
@id("token.identity")
fn identity(value: own Token) -> Token { value }
@id("app.main")
fn main() -> i64 { 42 }
"#,
            Path::new("checked-component-owned.spx"),
        )
        .unwrap();
        let error = emit_private_checked_component_v2(&program).unwrap_err();
        assert_eq!(error.code, "SPX-WIT106");
    }

    #[test]
    fn checked_component_v2_ignores_only_implicit_prelude_templates() {
        for source in [
            r#"module authored_record;
@id("record.type") record Payload { @id("record.value") value: i64, }
@id("app.main") fn main() -> i64 { 42 }
"#,
            r#"module authored_variant;
@id("variant.type") variant Choice { @id("variant.none") None, }
@id("app.main") fn main() -> i64 { 42 }
"#,
        ] {
            let program = crate::parse(source, Path::new("authored-aggregate-v2.spx")).unwrap();
            let error = emit_private_checked_component_v2(&program).unwrap_err();
            assert_eq!(error.code, "SPX-WIT106");
        }

        emit_private_checked_component_v2(&checked_component_program())
            .expect("implicit Option/Result templates must preserve the scalar profile");
    }

    #[test]
    fn node_executes_generated_core_with_the_embedded_checked_runtime() {
        let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
        let validated = validate_private_checked_component_v2(
            artifact.bytes(),
            artifact.generated_core_digest(),
        )
        .unwrap();
        let runtime = validated
            .runtime_core()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let generated = validated
            .generated_core()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            r#"const runtimeBytes = new Uint8Array([{runtime}]);
const generatedBytes = new Uint8Array([{generated}]);
if (!WebAssembly.validate(runtimeBytes)) process.exit(76);
if (!WebAssembly.validate(generatedBytes)) process.exit(75);
const runtime = (await WebAssembly.instantiate(runtimeBytes, {{}})).instance;
const generated = (await WebAssembly.instantiate(generatedBytes, {{ env: runtime.exports }})).instance;
if (generated.exports.semaprax_main() !== 42n) process.exit(72);
const min = -(1n << 63n), max = (1n << 63n) - 1n;
for (const invoke of [
  () => runtime.exports.spx_add(max, 1n),
  () => runtime.exports.spx_sub(min, 1n),
  () => runtime.exports.spx_mul(max, 2n),
  () => runtime.exports.spx_div(min, -1n),
  () => runtime.exports.spx_rem(min, -1n),
  () => runtime.exports.spx_neg(min),
  () => runtime.exports.spx_contract_fail()
]) {{
  let trapped = false;
  try {{ invoke(); }} catch (error) {{ trapped = error instanceof WebAssembly.RuntimeError; }}
  if (!trapped) process.exit(73);
}}
if (runtime.exports.spx_add(19n, 23n) !== 42n ||
    runtime.exports.spx_sub(23n, 19n) !== 4n ||
    runtime.exports.spx_mul(-6n, 7n) !== -42n ||
    runtime.exports.spx_div(-42n, 7n) !== -6n ||
    runtime.exports.spx_rem(43n, 7n) !== 1n ||
    runtime.exports.spx_neg(42n) !== -42n) process.exit(74);
"#
        );
        let output = Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        assert!(
            output.status.success(),
            "Node checked component core execution failed with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn node_executes_the_authenticated_checked_component_v2_runtime() {
        let artifact = emit_private_checked_component_v2(&checked_component_program()).unwrap();
        let bytes = artifact
            .bytes()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            r#"{}
const original = new Uint8Array([{}]);
const component = await instantiatePrivateCheckedComponentV2(original);
if (component.evaluate() !== 42n || !Object.isFrozen(component)) process.exit(77);
let rejectedArgument = false;
try {{ component.evaluate(1n); }} catch (error) {{ rejectedArgument = error instanceof TypeError && error.message === "SPX-WIT-I64"; }}
if (!rejectedArgument) process.exit(78);

const copiedBeforeAwait = new Uint8Array(original);
const pending = instantiatePrivateCheckedComponentV2(copiedBeforeAwait);
copiedBeforeAwait.fill(0);
if ((await pending).evaluate() !== 42n) process.exit(79);

for (const hostile of [original.subarray(0, original.length - 1), Uint8Array.from([...original, 0])]) {{
  let rejected = false;
  try {{ await instantiatePrivateCheckedComponentV2(hostile); }}
  catch (error) {{ rejected = error instanceof TypeError; }}
  if (!rejected) process.exit(80);
}}
const changed = Uint8Array.from(original);
changed[Math.floor(changed.length / 2)] ^= 1;
let authenticated = false;
try {{ await instantiatePrivateCheckedComponentV2(changed); }}
catch (error) {{ authenticated = error instanceof TypeError && error.message === "SPX-WIT105"; }}
if (!authenticated) process.exit(81);
"#,
            private_checked_component_runtime_javascript_v2(&artifact),
            bytes
        );
        let output = Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        assert!(
            output.status.success(),
            "Node checked component v2 runtime failed with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn node_checked_component_v2_evaluate_traps_generated_overflow_and_contract_failure() {
        for (name, source) in [
            (
                "overflow",
                r#"module test.checked_component_overflow;
@id("app.main")
fn main() -> i64 { 9223372036854775807 + 1 }
"#,
            ),
            (
                "contract",
                r#"module test.checked_component_contract;
@id("app.main")
fn main() -> i64 requires false { 42 }
"#,
            ),
        ] {
            let program = crate::parse(source, Path::new("checked-component-trap.spx")).unwrap();
            let artifact = emit_private_checked_component_v2(&program).unwrap();
            let bytes = artifact
                .bytes()
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let script = format!(
                r#"{}
const component = await instantiatePrivateCheckedComponentV2(new Uint8Array([{}]));
let trapped = false;
try {{ component.evaluate(); }}
catch (error) {{ trapped = error instanceof WebAssembly.RuntimeError; }}
if (!trapped) process.exit(82);
"#,
                private_checked_component_runtime_javascript_v2(&artifact),
                bytes
            );
            let output = Command::new("node")
                .args(["--input-type=module", "--eval", &script])
                .output()
                .expect("Node is required by the existing Wasm quality gate");
            assert!(
                output.status.success(),
                "Node checked component v2 {name} did not trap through evaluate with {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn node_private_component_runtime_executes_the_embedded_core_export() {
        let artifact = emit_private_component_v1();
        let bytes = artifact
            .bytes()
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut script = format!(
            "{}\nconst bytes=new Uint8Array([{}]);\n",
            private_component_runtime_javascript_v1(),
            bytes
        );
        script.push_str(
            r#"const component = await instantiatePrivateScalarComponentV1(bytes);
if (component.evaluate(19n, 23n) !== 42n) process.exit(81);
const i64Minimum = -(1n << 63n);
const i64Maximum = (1n << 63n) - 1n;
if (component.evaluate(i64Minimum, 0n) !== i64Minimum ||
    component.evaluate(i64Maximum, 0n) !== i64Maximum) process.exit(82);
const rejectI64 = args => {
  let rejected = false;
  try { component.evaluate(...args); }
  catch (error) { rejected = error instanceof TypeError && error.message === "SPX-WIT-I64"; }
  if (!rejected) process.exit(83);
};
for (const args of [
  [19, 23], [19n, 23], [i64Minimum - 1n, 0n], [i64Maximum + 1n, 0n],
  [0n, i64Minimum - 1n], [0n, i64Maximum + 1n]
]) rejectI64(args);

const changedCore = bytes.slice();
const coreOpcode = changedCore.indexOf(0x7c);
if (coreOpcode < 10) process.exit(84);
changedCore[coreOpcode] = 0x7d;
const hostileInputs = [
  bytes.slice(0, -1),
  Uint8Array.from([...bytes, 0]),
  Uint8Array.from(bytes, (byte, index) => index === 8 ? 2 : byte),
  changedCore
];
for (const hostile of hostileInputs) {
  let rejected = false;
  try { await instantiatePrivateScalarComponentV1(hostile); }
  catch (error) { rejected = error instanceof TypeError && error.message.startsWith("SPX-WIT"); }
  if (!rejected) process.exit(85);
}

const mutable = bytes.slice();
const pending = instantiatePrivateScalarComponentV1(mutable);
mutable[coreOpcode] = 0x7d;
const snapshotted = await pending;
if (snapshotted.evaluate(19n, 23n) !== 42n) process.exit(86);
"#,
        );
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        assert!(
            output.status.success(),
            "Node private component runtime failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn default_feature_external_consumer_cannot_import_component_harness() {
        let directory = ConsumerDirectory::create();
        let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
        std::fs::write(
            directory.path().join("Cargo.toml"),
            format!(
                r#"[package]
name = "semaprax-wit-default-surface-check"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
semaprax = {{ path = "{manifest_root}", default-features = false }}
"#
            ),
        )
        .unwrap();
        std::fs::create_dir(directory.path().join("src")).unwrap();
        std::fs::write(
            directory.path().join("src/main.rs"),
            r#"use semaprax::wit_component::{
    emit_private_checked_component_v2,
    emit_private_component_v1,
    emit_private_result_component_v3,
    emit_private_scalar_algebra_component_v5,
    emit_private_source_result_component_v4,
    private_checked_component_runtime_javascript_v2,
    validate_private_checked_component_v2,
    validate_private_component_v1,
    validate_private_result_component_v3,
    validate_private_scalar_algebra_component_v5,
    validate_private_source_result_component_v4,
};

fn main() {
    let artifact = emit_private_component_v1();
    let _ = validate_private_component_v1(artifact.bytes());
    let _ = emit_private_checked_component_v2;
    let _ = private_checked_component_runtime_javascript_v2;
    let _ = validate_private_checked_component_v2;
    let _ = emit_private_result_component_v3;
    let _ = validate_private_result_component_v3;
    let _ = emit_private_scalar_algebra_component_v5;
    let _ = validate_private_scalar_algebra_component_v5;
    let _ = emit_private_source_result_component_v4;
    let _ = validate_private_source_result_component_v4;
}
"#,
        )
        .unwrap();
        let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["check", "--offline", "--manifest-path"])
            .arg(directory.path().join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", directory.path().join("target"))
            .output()
            .unwrap();
        assert!(
            !checked.status.success(),
            "default surface exposed the private component harness"
        );
        let stderr = String::from_utf8_lossy(&checked.stderr);
        assert!(
            stderr.contains("wit_component")
                && (stderr.contains("could not find") || stderr.contains("unresolved import")),
            "unexpected default-surface compiler diagnostic:\n{stderr}"
        );
    }

    #[test]
    fn feature_consumer_can_only_read_checked_component_digests_through_accessors() {
        let directory = ConsumerDirectory::create();
        let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
        std::fs::write(
            directory.path().join("Cargo.toml"),
            format!(
                r#"[package]
name = "semaprax-wit-read-only-digest-check"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
semaprax = {{ path = "{manifest_root}", default-features = false, features = ["unstable-wit-component-harness"] }}
"#
            ),
        )
        .unwrap();
        std::fs::create_dir(directory.path().join("src")).unwrap();
        std::fs::write(
            directory.path().join("src/main.rs"),
            r#"use semaprax::wit_component::{
    PrivateCheckedComponentArtifactV2,
    PrivateResultComponentArtifactV3,
    PrivateSourceResultComponentArtifactV4,
};

fn hostile(artifact: &mut PrivateCheckedComponentArtifactV2) {
    let _ = artifact.digest();
    let _ = artifact.generated_core_digest();
    let _ = artifact.runtime_core_digest();
    artifact.digest = [0; 32];
    artifact.generated_core_digest = [0; 32];
    artifact.runtime_core_digest = [0; 32];
}

fn hostile_v3(artifact: &mut PrivateResultComponentArtifactV3) {
    let _ = artifact.digest();
    let _ = artifact.generated_core_digest();
    let _ = artifact.profile_digest();
    artifact.digest = [0; 32];
    artifact.generated_core_digest = [0; 32];
    artifact.profile_digest = [0; 32];
}

fn hostile_v4(artifact: &mut PrivateSourceResultComponentArtifactV4) {
    let _ = artifact.digest();
    let _ = artifact.generated_core_digest();
    let _ = artifact.profile_digest();
    let _ = artifact.prelude_digest();
    let _ = artifact.result_i64_bool_layout_digest();
    let _ = artifact.result_bool_bool_layout_digest();
    artifact.digest = [0; 32];
    artifact.generated_core_digest = [0; 32];
    artifact.profile_digest = [0; 32];
    artifact.prelude_digest = [0; 32];
    artifact.result_i64_bool_layout_digest = [0; 32];
    artifact.result_bool_bool_layout_digest = [0; 32];
}

fn main() {}
"#,
        )
        .unwrap();
        let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["check", "--offline", "--manifest-path"])
            .arg(directory.path().join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", directory.path().join("target"))
            .output()
            .unwrap();
        assert!(!checked.status.success(), "digest fields remained writable");
        let stderr = String::from_utf8_lossy(&checked.stderr);
        for field in [
            "digest",
            "generated_core_digest",
            "runtime_core_digest",
            "profile_digest",
            "prelude_digest",
            "result_i64_bool_layout_digest",
            "result_bool_bool_layout_digest",
        ] {
            assert!(
                stderr.contains("private field") && stderr.contains(field),
                "missing private-field diagnostic for {field}:\n{stderr}"
            );
        }
    }
}
