//! Canonical, authority-free Project-v11 nested owned-record description.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{DeclarationId, ResolvedProgram};

use super::{PublicApiParameterType, PublicApiSubject};

mod codec;
mod derivation;
mod projections;

pub use derivation::derive_nested_owned_record_api_descriptor;
pub use projections::render_nested_owned_record_c_header;

pub const NESTED_OWNED_RECORD_PROJECT_SCHEMA: &str = "semaprax.project.v11";
pub const NESTED_OWNED_RECORD_API_SCHEMA: &str = "semaprax.public-nested-owned-record-api.v1";
pub const MAX_NESTED_RECORD_DEPTH: usize = 64;
pub const MAX_NESTED_RECORD_OWNED_LEAVES: usize = 256;
pub const MAX_NESTED_RECORD_VISITED_FIELDS: usize = 4_096;
pub const MAX_NESTED_RECORD_DESCRIPTOR_BYTES: usize = 1024 * 1024;
pub const MAX_NESTED_RECORD_OWNED_OUTPUT_BYTES: usize = 65_536;

const DIGEST_DOMAIN: &[u8] = b"semaprax.public-nested-owned-record-api.digest.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NestedOwnedRecordFieldType {
    I64,
    Bool,
    Usize,
    OwnedBytes,
    Record(DeclarationId),
}

impl NestedOwnedRecordFieldType {
    fn render(&self, output: &mut String) {
        match self {
            Self::I64 => output.push_str("\"i64\""),
            Self::Bool => output.push_str("\"bool\""),
            Self::Usize => output.push_str("\"usize\""),
            Self::OwnedBytes => output.push_str("\"owned-bytes\""),
            Self::Record(id) => {
                output.push_str("\"record\",\"record_id\":");
                output.push_str(&quote_json(id.as_str()));
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedOwnedRecordField {
    stable_id: DeclarationId,
    source_name: String,
    host_name: String,
    ordinal: u32,
    ty: NestedOwnedRecordFieldType,
}

impl NestedOwnedRecordField {
    pub fn stable_id(&self) -> &DeclarationId {
        &self.stable_id
    }
    pub fn source_name(&self) -> &str {
        &self.source_name
    }
    pub fn host_name(&self) -> &str {
        &self.host_name
    }
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub fn ty(&self) -> &NestedOwnedRecordFieldType {
        &self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedOwnedRecordType {
    stable_id: DeclarationId,
    source_name: String,
    host_name: String,
    fields: Vec<NestedOwnedRecordField>,
}

impl NestedOwnedRecordType {
    pub fn stable_id(&self) -> &DeclarationId {
        &self.stable_id
    }
    pub fn source_name(&self) -> &str {
        &self.source_name
    }
    pub fn host_name(&self) -> &str {
        &self.host_name
    }
    pub fn fields(&self) -> &[NestedOwnedRecordField] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedOwnedRecordLeaf {
    field_path: Vec<DeclarationId>,
    ordinal: u32,
    ty: NestedOwnedRecordLeafType,
}

impl NestedOwnedRecordLeaf {
    pub fn field_path(&self) -> &[DeclarationId] {
        &self.field_path
    }
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub const fn ty(&self) -> NestedOwnedRecordLeafType {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NestedOwnedRecordLeafType {
    I64,
    Bool,
    Usize,
    OwnedBytes,
}

impl NestedOwnedRecordLeafType {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
            Self::OwnedBytes => "owned-bytes",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedOwnedRecordExport {
    stable_id: DeclarationId,
    typescript_name: String,
    rust_method_name: String,
    parameters: Vec<(String, String, PublicApiParameterType)>,
    result_record_id: DeclarationId,
    leaves: Vec<NestedOwnedRecordLeaf>,
}

impl NestedOwnedRecordExport {
    pub fn stable_id(&self) -> &DeclarationId {
        &self.stable_id
    }
    pub fn typescript_name(&self) -> &str {
        &self.typescript_name
    }
    pub fn rust_method_name(&self) -> &str {
        &self.rust_method_name
    }
    pub fn parameters(&self) -> &[(String, String, PublicApiParameterType)] {
        &self.parameters
    }
    pub fn result_record_id(&self) -> &DeclarationId {
        &self.result_record_id
    }
    pub fn leaves(&self) -> &[NestedOwnedRecordLeaf] {
        &self.leaves
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NestedOwnedRecordApiDescriptor {
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    exports: Vec<NestedOwnedRecordExport>,
    records: Vec<NestedOwnedRecordType>,
}

impl NestedOwnedRecordApiDescriptor {
    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
    pub fn project_graph_digest(&self) -> &str {
        &self.project_graph_digest
    }
    pub fn exports(&self) -> &[NestedOwnedRecordExport] {
        &self.exports
    }
    pub fn records(&self) -> &[NestedOwnedRecordType] {
        &self.records
    }
    pub fn canonical_bytes(&self) -> Vec<u8> {
        render_descriptor(self).into_bytes()
    }
    pub fn digest(&self) -> String {
        domain_digest(&self.canonical_bytes())
    }
}

pub fn replay_nested_owned_record_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    submitted: &[u8],
    submitted_digest: &str,
) -> Result<NestedOwnedRecordApiDescriptor, Diagnostic> {
    if submitted.is_empty()
        || submitted.len() > MAX_NESTED_RECORD_DESCRIPTOR_BYTES
        || !submitted.ends_with(b"\n")
        || submitted.contains(&0)
        || domain_digest(submitted) != submitted_digest
    {
        return Err(error(
            "nested owned-record descriptor framing or digest is invalid",
        ));
    }
    let value: Value = serde_json::from_slice(submitted)
        .map_err(|_| error("nested owned-record descriptor JSON is invalid"))?;
    let root = value
        .as_object()
        .filter(|root| {
            root.len() == 9
                && root.get("schema").and_then(Value::as_str)
                    == Some(NESTED_OWNED_RECORD_API_SCHEMA)
                && root.get("project_schema").and_then(Value::as_str)
                    == Some(NESTED_OWNED_RECORD_PROJECT_SCHEMA)
                && root.get("exports").and_then(Value::as_array).is_some()
                && root.get("records").and_then(Value::as_array).is_some()
                && root.get("limits").and_then(Value::as_object).is_some()
                && root.get("settlement").and_then(Value::as_object).is_some()
        })
        .ok_or_else(|| error("nested owned-record descriptor root is not closed"))?;
    for key in root.keys() {
        if !matches!(
            key.as_str(),
            "schema"
                | "project_schema"
                | "project_revision"
                | "workspace_revision"
                | "project_graph_digest"
                | "exports"
                | "records"
                | "limits"
                | "settlement"
        ) {
            return Err(error(
                "nested owned-record descriptor contains an unknown field",
            ));
        }
    }
    codec::validate_closed_descriptor(root)?;
    let rebuilt = derive_nested_owned_record_api_descriptor(program, selected, subject)?;
    if submitted != rebuilt.canonical_bytes() || submitted_digest != rebuilt.digest() {
        return Err(error(
            "nested owned-record descriptor does not replay against retained HIR",
        ));
    }
    Ok(rebuilt)
}

fn render_descriptor(descriptor: &NestedOwnedRecordApiDescriptor) -> String {
    let mut out = String::from("{\"schema\":");
    out.push_str(&quote_json(NESTED_OWNED_RECORD_API_SCHEMA));
    out.push_str(",\"project_schema\":");
    out.push_str(&quote_json(NESTED_OWNED_RECORD_PROJECT_SCHEMA));
    out.push_str(",\"project_revision\":");
    out.push_str(&quote_json(&descriptor.project_revision));
    out.push_str(",\"workspace_revision\":");
    out.push_str(&quote_json(&descriptor.workspace_revision));
    out.push_str(",\"project_graph_digest\":");
    out.push_str(&quote_json(&descriptor.project_graph_digest));
    out.push_str(",\"exports\":[");
    for (i, export) in descriptor.exports.iter().enumerate() {
        if i != 0 {
            out.push(',');
        }
        out.push_str("{\"stable_id\":");
        out.push_str(&quote_json(export.stable_id.as_str()));
        out.push_str(",\"typescript_name\":");
        out.push_str(&quote_json(&export.typescript_name));
        out.push_str(",\"rust_method_name\":");
        out.push_str(&quote_json(&export.rust_method_name));
        out.push_str(",\"parameters\":[");
        for (ordinal, (id, name, ty)) in export.parameters.iter().enumerate() {
            if ordinal != 0 {
                out.push(',');
            }
            out.push_str("{\"stable_id\":");
            out.push_str(&quote_json(id));
            out.push_str(",\"source_name\":");
            out.push_str(&quote_json(name));
            out.push_str(",\"ordinal\":");
            out.push_str(&ordinal.to_string());
            out.push_str(",\"type\":");
            out.push_str(&quote_json(ty.wire_name()));
            out.push('}');
        }
        out.push_str("],\"result_record_id\":");
        out.push_str(&quote_json(export.result_record_id.as_str()));
        out.push_str(",\"leaves\":[");
        for (leaf_index, leaf) in export.leaves.iter().enumerate() {
            if leaf_index != 0 {
                out.push(',');
            }
            out.push_str("{\"path\":[");
            for (part_index, part) in leaf.field_path.iter().enumerate() {
                if part_index != 0 {
                    out.push(',');
                }
                out.push_str(&quote_json(part.as_str()));
            }
            out.push_str("],\"ordinal\":");
            out.push_str(&leaf.ordinal.to_string());
            out.push_str(",\"type\":");
            out.push_str(&quote_json(leaf.ty.wire_name()));
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("],\"records\":[");
    for (i, record) in descriptor.records.iter().enumerate() {
        if i != 0 {
            out.push(',');
        }
        out.push_str("{\"stable_id\":");
        out.push_str(&quote_json(record.stable_id.as_str()));
        out.push_str(",\"source_name\":");
        out.push_str(&quote_json(&record.source_name));
        out.push_str(",\"host_name\":");
        out.push_str(&quote_json(&record.host_name));
        out.push_str(",\"fields\":[");
        for (j, field) in record.fields.iter().enumerate() {
            if j != 0 {
                out.push(',');
            }
            out.push_str("{\"stable_id\":");
            out.push_str(&quote_json(field.stable_id.as_str()));
            out.push_str(",\"source_name\":");
            out.push_str(&quote_json(&field.source_name));
            out.push_str(",\"host_name\":");
            out.push_str(&quote_json(&field.host_name));
            out.push_str(",\"ordinal\":");
            out.push_str(&field.ordinal.to_string());
            out.push_str(",\"type\":");
            field.ty.render(&mut out);
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("],\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_record_depth\":64,\"max_owned_leaves\":256,\"max_examined_fields\":4096,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576},\"settlement\":{\"carrier\":\"opaque-multi-handle-plus-scalars.v1\",\"preflight_all_handles\":true,\"batch_attach\":true,\"copy_all_before_settle\":true,\"publish_after_settle\":true}}\n");
    out
}

fn stable_host_name(prefix: &str, stable_id: &str) -> String {
    let mut output = prefix.to_owned();
    for byte in stable_id.bytes() {
        use std::fmt::Write as _;
        write!(output, "{byte:02x}").expect("String writes cannot fail");
    }
    output
}

fn domain_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J118", message)
}
