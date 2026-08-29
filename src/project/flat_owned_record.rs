//! Canonical Project-v9 flat owned-record description and host projections.
//!
//! This module is authority-free. It never exposes a native aggregate layout:
//! target adapters receive one opaque owned-byte handle plus authenticated
//! scalar values, copy the bytes, settle the handle, and only then construct a
//! JavaScript object or safe Rust struct.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{DeclarationId, ResolvedProgram};

use super::{PublicApiParameterType, PublicApiSubject};

mod derivation;
mod metadata;
mod projections;
mod settlement;

pub use derivation::derive_flat_owned_record_api_descriptor;
pub use metadata::{
    render_flat_owned_record_metadata, render_flat_owned_record_rust_sdk_manifest,
    replay_flat_owned_record_metadata, replay_flat_owned_record_rust_sdk_manifest,
};
pub use projections::{render_flat_owned_record_rust, render_flat_owned_record_typescript};
pub use settlement::FlatOwnedRecordSettlement;

pub const FLAT_OWNED_RECORD_PROJECT_SCHEMA: &str = "semaprax.project.v9";
pub const FLAT_OWNED_RECORD_API_SCHEMA: &str = "semaprax.public-flat-owned-record-api.v1";
pub const FLAT_OWNED_RECORD_METADATA_SCHEMA: &str = "semaprax.flat-owned-record-api.v1";
pub const FLAT_OWNED_RECORD_NPM_BUILD_SCHEMA: &str = "semaprax.project-npm-build.v8";
pub const FLAT_OWNED_RECORD_RUST_SDK_SCHEMA: &str = "semaprax.native-rust-flat-owned-record-sdk.v1";
pub const MAX_FLAT_RECORD_FIELDS: usize = 64;
pub const MAX_FLAT_RECORD_DESCRIPTOR_BYTES: usize = 1024 * 1024;

const DIGEST_DOMAIN: &[u8] = b"semaprax.public-flat-owned-record-api.digest.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlatOwnedRecordFieldType {
    I64,
    Bool,
    Usize,
    OwnedBytes,
}

impl FlatOwnedRecordFieldType {
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
            Self::OwnedBytes => "owned-bytes",
        }
    }

    const fn typescript(self) -> &'static str {
        match self {
            Self::I64 | Self::Usize => "bigint",
            Self::Bool => "boolean",
            Self::OwnedBytes => "Uint8Array",
        }
    }

    const fn rust(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
            Self::OwnedBytes => "Vec<u8>",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordField {
    stable_id: DeclarationId,
    source_name: String,
    host_name: String,
    ordinal: u32,
    ty: FlatOwnedRecordFieldType,
}

impl FlatOwnedRecordField {
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
    pub const fn ty(&self) -> FlatOwnedRecordFieldType {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordExport {
    stable_id: DeclarationId,
    typescript_name: String,
    rust_method_name: String,
    parameters: Vec<(String, String, PublicApiParameterType)>,
    record_id: DeclarationId,
    record_host_name: String,
    record_source_name: String,
    fields: Vec<FlatOwnedRecordField>,
}

impl FlatOwnedRecordExport {
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
    pub fn record_id(&self) -> &DeclarationId {
        &self.record_id
    }
    pub fn record_host_name(&self) -> &str {
        &self.record_host_name
    }
    pub fn record_source_name(&self) -> &str {
        &self.record_source_name
    }
    pub fn fields(&self) -> &[FlatOwnedRecordField] {
        &self.fields
    }
}

/// Private target call plan. `owned_field_ordinal` identifies the sole opaque
/// handle; the scalar ordinals are copied values, never struct offsets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordCarrierPlan {
    pub record_id: DeclarationId,
    pub owned_field_ordinal: u32,
    pub scalar_field_ordinals: Vec<u32>,
    pub copy_before_settle: bool,
    pub publish_after_settle: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordApiDescriptor {
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    exports: Vec<FlatOwnedRecordExport>,
}

impl FlatOwnedRecordApiDescriptor {
    pub fn exports(&self) -> &[FlatOwnedRecordExport] {
        &self.exports
    }
    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }
    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }
    pub fn project_graph_digest(&self) -> &str {
        &self.project_graph_digest
    }
    pub fn canonical_bytes(&self) -> Vec<u8> {
        render_descriptor(self).into_bytes()
    }
    pub fn digest(&self) -> String {
        domain_digest(DIGEST_DOMAIN, &self.canonical_bytes())
    }
    pub fn carrier_plans(&self) -> Vec<FlatOwnedRecordCarrierPlan> {
        self.exports
            .iter()
            .map(|export| FlatOwnedRecordCarrierPlan {
                record_id: export.record_id.clone(),
                owned_field_ordinal: export
                    .fields
                    .iter()
                    .find(|field| field.ty == FlatOwnedRecordFieldType::OwnedBytes)
                    .expect("descriptor admission proves one owned field")
                    .ordinal,
                scalar_field_ordinals: export
                    .fields
                    .iter()
                    .filter(|field| field.ty != FlatOwnedRecordFieldType::OwnedBytes)
                    .map(|field| field.ordinal)
                    .collect(),
                copy_before_settle: true,
                publish_after_settle: true,
            })
            .collect()
    }
}

pub fn replay_flat_owned_record_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
    submitted: &[u8],
    submitted_digest: &str,
) -> Result<FlatOwnedRecordApiDescriptor, Diagnostic> {
    if submitted.is_empty()
        || submitted.len() > MAX_FLAT_RECORD_DESCRIPTOR_BYTES
        || !submitted.ends_with(b"\n")
        || submitted.contains(&0)
        || domain_digest(DIGEST_DOMAIN, submitted) != submitted_digest
    {
        return Err(error(
            "flat owned-record descriptor framing or digest is invalid",
        ));
    }
    let value: Value = serde_json::from_slice(submitted)
        .map_err(|_| error("flat owned-record descriptor JSON is invalid"))?;
    let root = value
        .as_object()
        .filter(|root| {
            root.len() == 8
                && root.get("schema").and_then(Value::as_str) == Some(FLAT_OWNED_RECORD_API_SCHEMA)
                && root.get("project_schema").and_then(Value::as_str)
                    == Some(FLAT_OWNED_RECORD_PROJECT_SCHEMA)
                && root.get("exports").and_then(Value::as_array).is_some()
                && root.get("limits").and_then(Value::as_object).is_some()
                && root.get("settlement").and_then(Value::as_object).is_some()
        })
        .ok_or_else(|| error("flat owned-record descriptor root is not closed"))?;
    for key in root.keys() {
        if !matches!(
            key.as_str(),
            "schema"
                | "project_schema"
                | "project_revision"
                | "workspace_revision"
                | "project_graph_digest"
                | "exports"
                | "limits"
                | "settlement"
        ) {
            return Err(error(
                "flat owned-record descriptor contains an unknown field",
            ));
        }
    }
    let rebuilt = derive_flat_owned_record_api_descriptor(program, selected, subject)?;
    if submitted != rebuilt.canonical_bytes() || submitted_digest != rebuilt.digest() {
        return Err(error(
            "flat owned-record descriptor does not replay against retained HIR",
        ));
    }
    Ok(rebuilt)
}

fn render_descriptor(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut output = String::new();
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(FLAT_OWNED_RECORD_API_SCHEMA));
    output.push_str(",\"project_schema\":");
    output.push_str(&quote_json(FLAT_OWNED_RECORD_PROJECT_SCHEMA));
    output.push_str(",\"project_revision\":");
    output.push_str(&quote_json(&descriptor.project_revision));
    output.push_str(",\"workspace_revision\":");
    output.push_str(&quote_json(&descriptor.workspace_revision));
    output.push_str(",\"project_graph_digest\":");
    output.push_str(&quote_json(&descriptor.project_graph_digest));
    output.push_str(",\"exports\":[");
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        if export_index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        output.push_str(&quote_json(export.stable_id.as_str()));
        output.push_str(",\"typescript_name\":");
        output.push_str(&quote_json(&export.typescript_name));
        output.push_str(",\"rust_method_name\":");
        output.push_str(&quote_json(&export.rust_method_name));
        output.push_str(",\"parameters\":[");
        for (index, (id, name, ty)) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            output.push_str(&quote_json(id));
            output.push_str(",\"source_name\":");
            output.push_str(&quote_json(name));
            output.push_str(",\"ordinal\":");
            output.push_str(&index.to_string());
            output.push_str(",\"type\":");
            output.push_str(&quote_json(ty.wire_name()));
            output.push('}');
        }
        output.push_str("],\"result\":{\"type\":\"flat-owned-record\",\"record_id\":");
        output.push_str(&quote_json(export.record_id.as_str()));
        output.push_str(",\"record_source_name\":");
        output.push_str(&quote_json(&export.record_source_name));
        output.push_str(",\"record_host_name\":");
        output.push_str(&quote_json(&export.record_host_name));
        output.push_str(",\"fields\":[");
        for (index, field) in export.fields.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            output.push_str(&quote_json(field.stable_id.as_str()));
            output.push_str(",\"source_name\":");
            output.push_str(&quote_json(&field.source_name));
            output.push_str(",\"host_name\":");
            output.push_str(&quote_json(&field.host_name));
            output.push_str(",\"ordinal\":");
            output.push_str(&field.ordinal.to_string());
            output.push_str(",\"type\":");
            output.push_str(&quote_json(field.ty.wire_name()));
            output.push('}');
        }
        output.push_str("]}}");
    }
    output.push_str("],\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_record_fields\":64,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576},\"settlement\":{\"carrier\":\"opaque-handle-plus-scalars.v1\",\"copy_before_settle\":true,\"publish_after_settle\":true,\"exactly_one_owned_field\":true}}\n");
    output
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J113", message)
}
