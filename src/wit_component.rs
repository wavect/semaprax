//! Private deterministic WIT/component-boundary evidence.
//!
//! This freezes one scalar result/status interface and its JavaScript adapter,
//! a standalone scalar Component Model v1 fixture, and checked generated-core
//! component v2 evidence. None is a public WIT import/export surface.

use sha2::{Digest, Sha256};

use crate::{ast::Program, diagnostic::Diagnostic, graph, wasm};

mod generic_function_v9;
mod generic_record_v7;
mod nested_record_v6;
mod option_propagation_v10;
mod record_pattern_v8;
mod result_v3;
mod scalar_algebra_v5;
mod source_result_v4;

pub use generic_function_v9::{
    emit_private_generic_function_component_v9, validate_private_generic_function_component_v9,
    PrivateGenericFunctionComponentArtifactV9, ValidatedPrivateGenericFunctionComponentV9,
};
pub use generic_record_v7::{
    emit_private_generic_record_component_v7, validate_private_generic_record_component_v7,
    PrivateGenericRecordComponentArtifactV7, ValidatedPrivateGenericRecordComponentV7,
};
pub use nested_record_v6::{
    emit_private_nested_record_component_v6, validate_private_nested_record_component_v6,
    PrivateNestedRecordComponentArtifactV6, ValidatedPrivateNestedRecordComponentV6,
};
pub use option_propagation_v10::{
    emit_private_option_propagation_component_v10,
    validate_private_option_propagation_component_v10,
    PrivateOptionPropagationComponentArtifactV10, ValidatedPrivateOptionPropagationComponentV10,
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

/// Deterministic WIT for trivial-drop owned resources.
///
/// This projects each `resource Token { drop trivial; }` as a WIT
/// `resource token {}` and every `own Token` / `borrow Token` parameter as
/// `own<token>` / `borrow<token>`. Functions returning `Token` by value are
/// projected as `own<token>`. Scalars map as `i64 -> s64`, `bool -> bool`,
/// etc. Non-trivial resources or unsupported types fail with `SPX-WIT110`.
#[must_use]
pub fn is_trivial_drop_resource(program: &Program, name: &str) -> bool {
    program.types.iter().any(|declaration| {
        declaration.name == name
            && matches!(
                &declaration.kind,
                crate::ast::TypeDeclarationKind::Resource { lifecycles }
                if lifecycles.len() == 1
                    && matches!(
                        lifecycles[0].kind,
                        crate::ast::ResourceLifecycleKind::Trivial
                    )
            )
    })
}

fn wit_scalar(ty: &crate::ast::Type) -> Option<&'static str> {
    match ty {
        crate::ast::Type::I64 => Some("s64"),
        crate::ast::Type::I32 => Some("s32"),
        crate::ast::Type::Bool => Some("bool"),
        crate::ast::Type::U8 => Some("u8"),
        crate::ast::Type::F32 => Some("f32"),
        crate::ast::Type::F64 => Some("f64"),
        crate::ast::Type::Char => Some("char"),
        crate::ast::Type::String => Some("string"),
        _ => None,
    }
}

fn wit_resource_ident(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    for character in name.chars() {
        if character == '_' {
            output.push('-');
        } else {
            for lower in character.to_lowercase() {
                output.push(lower);
            }
        }
    }
    output
}

fn wit_func_ident(name: &str) -> String {
    wit_resource_ident(name)
}

/// Emit deterministic WIT exposing trivial-drop `Token` as a WIT resource.
///
/// This is the generator referenced by the owned-resource corpus test. It
/// succeeds for programs whose only non-scalar types are trivial-drop
/// resources (currently `Token`) used via `own`/`borrow` handles or owned
/// returns. Any other nominal type, generic argument, or lifecycle is
/// documented as unsupported via `SPX-WIT110`.
pub fn emit_owned_resource_wit(program: &Program) -> Result<String, Diagnostic> {
    emit_wit(program)
}

/// Alias for `emit_owned_resource_wit` to keep the generic generator name
/// stable for the deterministic test.
pub fn emit_wit(program: &Program) -> Result<String, Diagnostic> {
    use std::collections::{BTreeMap, BTreeSet};

    let mut resources: BTreeMap<String, String> = BTreeMap::new();
    let mut resource_wit_names: BTreeSet<String> = BTreeSet::new();
    for declaration in &program.types {
        if let crate::ast::TypeDeclarationKind::Resource { lifecycles } = &declaration.kind {
            if lifecycles.len() != 1
                || !matches!(
                    lifecycles[0].kind,
                    crate::ast::ResourceLifecycleKind::Trivial
                )
            {
                return Err(Diagnostic::io(
                    "SPX-WIT110",
                    format!(
                        "WIT resource '{}' requires exactly `drop trivial`",
                        declaration.name
                    ),
                ));
            }
            if !declaration.type_parameters.is_empty() {
                return Err(Diagnostic::io(
                    "SPX-WIT110",
                    format!(
                        "WIT resource '{}' does not support type parameters",
                        declaration.name
                    ),
                ));
            }
            let wit_name = wit_resource_ident(&declaration.name);
            if !resource_wit_names.insert(wit_name.clone()) {
                return Err(Diagnostic::io(
                    "SPX-WIT110",
                    format!("duplicate WIT resource name '{wit_name}'"),
                ));
            }
            resources.insert(declaration.name.clone(), wit_name);
        } else if matches!(
            &declaration.kind,
            crate::ast::TypeDeclarationKind::Record { .. }
                | crate::ast::TypeDeclarationKind::Variant { .. }
                | crate::ast::TypeDeclarationKind::Class { .. }
        ) {
            return Err(Diagnostic::io(
                "SPX-WIT110",
                format!(
                    "WIT generation does not support nominal type '{}'",
                    declaration.name
                ),
            ));
        }
    }

    let map_param =
        |mode: crate::ast::ParamMode, ty: &crate::ast::Type| -> Result<String, Diagnostic> {
            if let Some(scalar) = wit_scalar(ty) {
                if mode != crate::ast::ParamMode::Value {
                    return Err(Diagnostic::io(
                        "SPX-WIT110",
                        format!("scalar param mode must be value, found '{}'", mode.text()),
                    ));
                }
                return Ok(scalar.to_string());
            }
            if let crate::ast::Type::Named { name, arguments } = ty {
                if !arguments.is_empty() {
                    return Err(Diagnostic::io(
                        "SPX-WIT110",
                        format!("WIT generation does not support generic type '{name}'"),
                    ));
                }
                if let Some(wit_name) = resources.get(name) {
                    return Ok(match mode {
                        crate::ast::ParamMode::Own => format!("own<{wit_name}>"),
                        crate::ast::ParamMode::Borrow => format!("borrow<{wit_name}>"),
                        crate::ast::ParamMode::Value => {
                            return Err(Diagnostic::io(
                                "SPX-WIT110",
                                format!("resource '{name}' param requires own or borrow"),
                            ))
                        }
                        crate::ast::ParamMode::Shared => {
                            return Err(Diagnostic::io(
                                "SPX-WIT110",
                                format!("resource '{name}' does not support shared handle"),
                            ))
                        }
                    });
                }
                return Err(Diagnostic::io(
                    "SPX-WIT110",
                    format!("unsupported WIT param type '{name}'"),
                ));
            }
            Err(Diagnostic::io(
                "SPX-WIT110",
                format!("unsupported WIT param type '{ty}'"),
            ))
        };

    let map_return = |ty: &crate::ast::Type| -> Result<String, Diagnostic> {
        if let Some(scalar) = wit_scalar(ty) {
            return Ok(scalar.to_string());
        }
        if let crate::ast::Type::Named { name, arguments } = ty {
            if !arguments.is_empty() {
                return Err(Diagnostic::io(
                    "SPX-WIT110",
                    format!("WIT generation does not support generic return type '{name}'"),
                ));
            }
            if let Some(wit_name) = resources.get(name) {
                return Ok(format!("own<{wit_name}>"));
            }
            return Err(Diagnostic::io(
                "SPX-WIT110",
                format!("unsupported WIT return type '{name}'"),
            ));
        }
        Err(Diagnostic::io(
            "SPX-WIT110",
            format!("unsupported WIT return type '{ty}'"),
        ))
    };

    let module_local = program
        .module
        .rsplit('.')
        .next()
        .unwrap_or("owned")
        .replace('_', "-")
        .to_ascii_lowercase();
    let package = format!("semaprax:{module_local}@0.1.0");
    let interface = if resources.is_empty() {
        "owned-resource".to_string()
    } else {
        "token-ops".to_string()
    };
    let world = format!("{module_local}-world");

    let mut wit = String::new();
    wit.push_str(&format!(
        "package {package};

"
    ));
    wit.push_str(&format!(
        "interface {interface} {{
"
    ));

    let mut sorted_resources: Vec<(String, String)> = resources
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    sorted_resources.sort_by(|a, b| a.1.cmp(&b.1));
    for (_, wit_name) in &sorted_resources {
        wit.push_str(&format!(
            "  resource {wit_name} {{}}
"
        ));
    }

    let mut funcs = program.functions.clone();
    funcs.sort_by(|a, b| a.name.cmp(&b.name));
    for function in &funcs {
        let is_main = function.name == "main";
        let mut params_wit = Vec::new();
        let mut skip_due_to_unsupported = false;
        for param in &function.params {
            match map_param(param.mode, &param.ty) {
                Ok(mapped) => {
                    params_wit.push(format!("{}: {}", wit_func_ident(&param.name), mapped))
                }
                Err(error) => {
                    if is_main && wit_scalar(&param.ty).is_some() {
                        skip_due_to_unsupported = true;
                        break;
                    }
                    return Err(error);
                }
            }
        }
        if skip_due_to_unsupported {
            continue;
        }
        let ret_wit = match map_return(&function.return_type) {
            Ok(value) => value,
            Err(error) => {
                if is_main && wit_scalar(&function.return_type).is_some() {
                    continue;
                }
                return Err(error);
            }
        };
        let ret_suffix = format!(" -> {ret_wit}");
        let wit_name = wit_func_ident(&function.name);
        if wit_name == "main" && funcs.len() > 1 {
            continue;
        }
        if params_wit.is_empty() {
            wit.push_str(&format!(
                "  {wit_name}: func(){ret_suffix};
"
            ));
        } else {
            wit.push_str(&format!(
                "  {wit_name}: func({}){ret_suffix};
",
                params_wit.join(", ")
            ));
        }
    }

    wit.push_str(
        "}

",
    );
    wit.push_str(&format!(
        "world {world} {{
  export {interface};
}}
"
    ));
    Ok(wit)
}

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
    49, 42, 6, 148, 128, 25, 224, 97, 188, 114, 181, 7, 75, 208, 67, 250, 140, 179, 237, 126, 145,
    140, 175, 71, 194, 87, 23, 233, 130, 230, 239, 153,
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
            0x03, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e, 0x60, 0x01, 0x7e, 0x01, 0x7e, 0x60, 0x01,
            0x7f, 0x00,
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
#[path = "wit_component/tests.rs"]
mod tests;
