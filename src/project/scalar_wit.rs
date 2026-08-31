//! Authority-free WIT interface projection for one retained scalar Project.
//!
//! This module describes the already admitted Project-v1 scalar surface. It
//! emits no Core Wasm or Component Model bytes and owns no filesystem,
//! publication, runtime, or host authority.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{DeclarationId, IdentityOrigin, OwnershipMode, ResolvedProgram, ResolvedType};

use super::manifest::PROJECT_SCHEMA;

pub const SCALAR_WIT_INTERFACE_SCHEMA: &str = "semaprax.project.scalar-wit-interface.v1";
pub const MAX_SCALAR_WIT_INTERFACE_BYTES: usize = 65_536;
pub const MAX_SCALAR_WIT_DESCRIPTOR_BYTES: usize = 262_144;

const MAX_EXPORTS: usize = 32;
const MAX_PARAMETERS: usize = 8;
const MAX_STABLE_ID_BYTES: usize = 128;
const DIGEST_DOMAIN: &[u8] = b"semaprax.project.scalar-wit-interface.digest.v1\0";
const WIT_DIGEST_DOMAIN: &[u8] = b"semaprax.project.scalar-wit-interface.wit-digest.v1\0";
const WIT_PACKAGE: &str = "semaprax:project-scalar@1.0.0";
const WIT_INTERFACE: &str = "exports";
const WIT_WORLD: &str = "project-scalar-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarWitTypeV1 {
    I64,
    Bool,
}

impl ScalarWitTypeV1 {
    #[must_use]
    pub const fn wit_name(self) -> &'static str {
        match self {
            Self::I64 => "s64",
            Self::Bool => "bool",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarWitExportV1 {
    stable_id: DeclarationId,
    wit_name: String,
    parameters: Vec<ScalarWitTypeV1>,
    result: ScalarWitTypeV1,
}

impl ScalarWitExportV1 {
    #[must_use]
    pub fn stable_id(&self) -> &DeclarationId {
        &self.stable_id
    }

    #[must_use]
    pub fn wit_name(&self) -> &str {
        &self.wit_name
    }

    #[must_use]
    pub fn parameters(&self) -> &[ScalarWitTypeV1] {
        &self.parameters
    }

    #[must_use]
    pub const fn result(&self) -> ScalarWitTypeV1 {
        self.result
    }
}

/// One immutable, subject-bound description of the Project-v1 public scalar
/// WIT surface. The WIT text is an interface artifact, not executable bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarWitInterfaceArtifactV1 {
    project_name: String,
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    exports: Vec<ScalarWitExportV1>,
    wit: String,
    wit_digest: String,
}

impl ScalarWitInterfaceArtifactV1 {
    #[must_use]
    pub const fn schema(&self) -> &'static str {
        SCALAR_WIT_INTERFACE_SCHEMA
    }

    #[must_use]
    pub const fn project_schema(&self) -> &'static str {
        PROJECT_SCHEMA
    }

    #[must_use]
    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    #[must_use]
    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    #[must_use]
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    #[must_use]
    pub fn project_graph_digest(&self) -> &str {
        &self.project_graph_digest
    }

    #[must_use]
    pub fn exports(&self) -> &[ScalarWitExportV1] {
        &self.exports
    }

    #[must_use]
    pub fn wit(&self) -> &str {
        &self.wit
    }

    #[must_use]
    pub fn wit_digest(&self) -> &str {
        &self.wit_digest
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        render_descriptor(self).into_bytes()
    }

    #[must_use]
    pub fn digest(&self) -> String {
        domain_digest(&self.canonical_bytes())
    }
}

#[derive(Clone, Copy)]
pub(super) struct ScalarWitSubject<'a> {
    pub project_name: &'a str,
    pub project_revision: &'a str,
    pub workspace_revision: &'a str,
    pub project_graph_digest: &'a str,
}

pub(super) fn derive_scalar_wit_interface_v1(
    program: &ResolvedProgram,
    selected: &[String],
    subject: ScalarWitSubject<'_>,
) -> Result<ScalarWitInterfaceArtifactV1, Diagnostic> {
    validate_subject(subject)?;
    crate::hir::validate(program)?;
    if !(1..=MAX_EXPORTS).contains(&selected.len()) {
        return Err(capacity(format!(
            "scalar WIT interface requires 1..={MAX_EXPORTS} selected exports"
        )));
    }

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut previous: Option<&str> = None;
    let mut exports = Vec::with_capacity(selected.len());
    for stable_id in selected {
        if !seen.insert(stable_id.as_str()) {
            return Err(interface_error(format!(
                "scalar WIT export `{stable_id}` is selected more than once"
            )));
        }
        if previous.is_some_and(|prior| prior.as_bytes() >= stable_id.as_bytes()) {
            return Err(interface_error(
                "scalar WIT export identities are not in canonical manifest order",
            ));
        }
        previous = Some(stable_id);
        validate_stable_id(stable_id)?;
        let function = functions.get(stable_id.as_str()).copied().ok_or_else(|| {
            interface_error(format!(
                "scalar WIT export `{stable_id}` does not name a monomorphic function"
            ))
        })?;
        let declaration = program
            .declarations
            .declaration(&function.id)
            .ok_or_else(|| {
                interface_error(format!(
                    "scalar WIT export `{stable_id}` is absent from the declaration index"
                ))
            })?;
        if declaration.identity_origin != IdentityOrigin::Explicit {
            return Err(interface_error(format!(
                "scalar WIT export `{stable_id}` must have an explicit identity"
            )));
        }
        if !function.effects.is_empty() {
            return Err(interface_error(format!(
                "scalar WIT export `{stable_id}` is not effect-free"
            )));
        }
        if function.params.len() > MAX_PARAMETERS {
            return Err(capacity(format!(
                "scalar WIT export `{stable_id}` exceeds the {MAX_PARAMETERS}-parameter limit"
            )));
        }
        let parameters = function
            .params
            .iter()
            .map(|parameter| {
                if parameter.ownership != OwnershipMode::Value {
                    return Err(interface_error(format!(
                        "scalar WIT export `{stable_id}` has a non-value parameter"
                    )));
                }
                wit_type(&parameter.ty).ok_or_else(|| {
                    interface_error(format!(
                        "scalar WIT export `{stable_id}` has a non-scalar parameter"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = wit_type(&function.return_type).ok_or_else(|| {
            interface_error(format!(
                "scalar WIT export `{stable_id}` has a non-scalar result"
            ))
        })?;
        exports.push(ScalarWitExportV1 {
            stable_id: function.id.clone(),
            wit_name: wit_function_name(stable_id),
            parameters,
            result,
        });
    }
    let wit = render_wit(&exports);
    if wit.len() > MAX_SCALAR_WIT_INTERFACE_BYTES {
        return Err(capacity(
            "scalar WIT interface exceeds its exact byte limit",
        ));
    }
    let artifact = ScalarWitInterfaceArtifactV1 {
        project_name: subject.project_name.to_owned(),
        project_revision: subject.project_revision.to_owned(),
        workspace_revision: subject.workspace_revision.to_owned(),
        project_graph_digest: subject.project_graph_digest.to_owned(),
        exports,
        wit_digest: wit_digest(wit.as_bytes()),
        wit,
    };
    if artifact.canonical_bytes().len() > MAX_SCALAR_WIT_DESCRIPTOR_BYTES {
        return Err(capacity(
            "scalar WIT interface descriptor exceeds its exact byte limit",
        ));
    }
    Ok(artifact)
}

pub(super) fn replay_scalar_wit_interface_v1(
    program: &ResolvedProgram,
    selected: &[String],
    subject: ScalarWitSubject<'_>,
    submitted: &[u8],
    submitted_digest: &str,
) -> Result<ScalarWitInterfaceArtifactV1, Diagnostic> {
    if submitted.len() > MAX_SCALAR_WIT_DESCRIPTOR_BYTES {
        return Err(capacity(
            "scalar WIT interface descriptor exceeds its exact byte limit",
        ));
    }
    let value: Value = serde_json::from_slice(submitted)
        .map_err(|_| interface_error("scalar WIT interface descriptor JSON is invalid"))?;
    let root = value
        .as_object()
        .filter(|root| {
            root.len() == 12
                && root.get("schema").and_then(Value::as_str) == Some(SCALAR_WIT_INTERFACE_SCHEMA)
                && root.get("project_schema").and_then(Value::as_str) == Some(PROJECT_SCHEMA)
                && root.get("exports").and_then(Value::as_array).is_some()
                && root.get("mapping").and_then(Value::as_object).is_some()
                && root.get("limits").and_then(Value::as_object).is_some()
                && root.get("wit").and_then(Value::as_str).is_some()
                && root.get("wit_digest").and_then(Value::as_str).is_some()
        })
        .ok_or_else(|| interface_error("scalar WIT interface descriptor root is not closed"))?;
    if root.keys().any(|key| {
        !matches!(
            key.as_str(),
            "schema"
                | "project_schema"
                | "project_name"
                | "project_revision"
                | "workspace_revision"
                | "project_graph_digest"
                | "exports"
                | "mapping"
                | "wit"
                | "wit_digest"
                | "limits"
                | "nonclaims"
        )
    }) {
        return Err(interface_error(
            "scalar WIT interface descriptor contains an unknown field",
        ));
    }
    if domain_digest(submitted) != submitted_digest {
        return Err(interface_error(
            "scalar WIT interface descriptor digest does not match",
        ));
    }
    let rebuilt = derive_scalar_wit_interface_v1(program, selected, subject)?;
    if submitted != rebuilt.canonical_bytes().as_slice() || submitted_digest != rebuilt.digest() {
        return Err(interface_error(
            "scalar WIT interface descriptor does not replay against retained HIR",
        ));
    }
    Ok(rebuilt)
}

fn validate_subject(subject: ScalarWitSubject<'_>) -> Result<(), Diagnostic> {
    if subject.project_name.is_empty()
        || subject.project_name.len() > 64
        || !subject.project_name.as_bytes()[0].is_ascii_lowercase()
        || !subject
            .project_name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(interface_error(
            "scalar WIT interface project name is not canonical",
        ));
    }
    for (name, value) in [
        ("project revision", subject.project_revision),
        ("workspace revision", subject.workspace_revision),
        ("project graph digest", subject.project_graph_digest),
    ] {
        if !is_sha256_fact(value) {
            return Err(interface_error(format!(
                "scalar WIT interface {name} is not a canonical SHA-256 fact"
            )));
        }
    }
    Ok(())
}

fn validate_stable_id(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.len() > MAX_STABLE_ID_BYTES {
        return Err(capacity(format!(
            "scalar WIT stable identities require 1..={MAX_STABLE_ID_BYTES} bytes"
        )));
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    }) {
        return Err(interface_error(format!(
            "scalar WIT stable identity `{value}` is not canonical"
        )));
    }
    Ok(())
}

fn wit_type(ty: &ResolvedType) -> Option<ScalarWitTypeV1> {
    match ty {
        ResolvedType::I64 => Some(ScalarWitTypeV1::I64),
        ResolvedType::Bool => Some(ScalarWitTypeV1::Bool),
        _ => None,
    }
}

fn wit_function_name(stable_id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut name = String::with_capacity(4 + stable_id.len() * 2);
    name.push_str("spx-");
    for byte in stable_id.bytes() {
        name.push(HEX[(byte >> 4) as usize] as char);
        name.push(HEX[(byte & 0x0f) as usize] as char);
    }
    name
}

fn render_wit(exports: &[ScalarWitExportV1]) -> String {
    let mut output = format!(
        "package {WIT_PACKAGE};\n\ninterface {WIT_INTERFACE} {{\n  record status {{ domain: string, code: u32, class: u8, retryable: option<bool> }}\n"
    );
    for export in exports {
        output.push_str("  ");
        output.push_str(&export.wit_name);
        output.push_str(": func(");
        for (index, parameter) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str("arg-");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter.wit_name());
        }
        output.push_str(") -> result<");
        output.push_str(export.result.wit_name());
        output.push_str(", status>;\n");
    }
    output.push_str("}\n\nworld ");
    output.push_str(WIT_WORLD);
    output.push_str(" {\n  export ");
    output.push_str(WIT_INTERFACE);
    output.push_str(";\n}\n");
    output
}

fn render_descriptor(artifact: &ScalarWitInterfaceArtifactV1) -> String {
    let mut output = String::new();
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(SCALAR_WIT_INTERFACE_SCHEMA));
    output.push_str(",\"project_schema\":");
    output.push_str(&quote_json(PROJECT_SCHEMA));
    output.push_str(",\"project_name\":");
    output.push_str(&quote_json(&artifact.project_name));
    output.push_str(",\"project_revision\":");
    output.push_str(&quote_json(&artifact.project_revision));
    output.push_str(",\"workspace_revision\":");
    output.push_str(&quote_json(&artifact.workspace_revision));
    output.push_str(",\"project_graph_digest\":");
    output.push_str(&quote_json(&artifact.project_graph_digest));
    output.push_str(",\"exports\":[");
    for (export_index, export) in artifact.exports.iter().enumerate() {
        if export_index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        output.push_str(&quote_json(export.stable_id.as_str()));
        output.push_str(",\"wit_name\":");
        output.push_str(&quote_json(&export.wit_name));
        output.push_str(",\"parameters\":[");
        for (parameter_index, parameter) in export.parameters.iter().enumerate() {
            if parameter_index != 0 {
                output.push(',');
            }
            output.push_str(&quote_json(parameter.wit_name()));
        }
        output.push_str("],\"result\":");
        output.push_str(&quote_json(export.result.wit_name()));
        output.push('}');
    }
    output.push_str("],\"wit\":");
    output.push_str(&quote_json(&artifact.wit));
    output.push_str(",\"wit_digest\":");
    output.push_str(&quote_json(&artifact.wit_digest));
    output.push_str(",\"mapping\":{\"i64\":\"s64\",\"bool\":\"bool\",\"function_result\":\"result<T,status>\",\"status\":{\"schema\":\"semaprax.status.v1\",\"domain\":{\"wit\":\"string\",\"semantic\":\"domain_id\",\"min_utf8_bytes\":1,\"max_utf8_bytes\":255,\"forbid_nul\":true},\"code\":{\"wit\":\"u32\",\"semantic\":\"nonzero\"},\"class\":{\"wit\":\"u8\",\"ordinals\":{\"contract\":1,\"arithmetic\":2,\"import\":3,\"explicit_close\":4,\"adapter\":5}},\"retryable\":{\"wit\":\"option<bool>\",\"false\":false,\"true\":true,\"unknown\":null}}}");
    output.push_str(",\"limits\":{\"exports\":32,\"parameters\":8,\"interface_bytes\":65536,\"descriptor_bytes\":262144}");
    output.push_str(",\"nonclaims\":[\"no_component_binary_or_runtime\",\"no_imports_resources_capabilities_or_wasi\",\"no_filesystem_publication_or_execution_authority\"]}");
    output
}

fn is_sha256_fact(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn domain_digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(DIGEST_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn wit_digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(WIT_DIGEST_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn interface_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT111", message)
}

fn capacity(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-WIT112", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identity_names_are_injective_without_lossy_normalization() {
        let names = ["a.b", "a-b", "a_b"].map(wit_function_name);
        assert_eq!(names[0], "spx-612e62");
        assert_eq!(names[1], "spx-612d62");
        assert_eq!(names[2], "spx-615f62");
        assert_eq!(names.into_iter().collect::<BTreeSet<_>>().len(), 3);
    }

    #[test]
    fn wit_uses_ordinal_parameters_and_typed_status_results() {
        let wit = render_wit(&[ScalarWitExportV1 {
            stable_id: DeclarationId::new("example.call"),
            wit_name: wit_function_name("example.call"),
            parameters: vec![ScalarWitTypeV1::I64, ScalarWitTypeV1::Bool],
            result: ScalarWitTypeV1::Bool,
        }]);
        assert!(wit.starts_with("package semaprax:project-scalar@1.0.0;\n"));
        assert!(wit.contains(
            "spx-6578616d706c652e63616c6c: func(arg-0: s64, arg-1: bool) -> result<bool, status>;"
        ));
        assert!(wit.ends_with("world project-scalar-v1 {\n  export exports;\n}\n"));
    }
}
