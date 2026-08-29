//! Canonical Project-v9 flat owned-record description and host projections.
//!
//! This module is authority-free. It never exposes a native aggregate layout:
//! target adapters receive one opaque owned-byte handle plus authenticated
//! scalar values, copy the bytes, settle the handle, and only then construct a
//! JavaScript object or safe Rust struct.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::call_index::PersistentCallIndex;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    DeclarationId, IdentityOrigin, ResolvedFunction, ResolvedProgram, ResolvedType,
    ResolvedTypeDeclarationKind,
};

use super::public_api::{
    parameter_type, rust_method_name, selected_closure, valid_sha256_fact,
    validate_closure_function, validate_selected,
};
use super::{PublicApiParameterType, PublicApiSubject};

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
            Self::Usize => "u64",
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

/// Target-neutral publication sequencer. It carries no provider handle and
/// performs no copy/drop itself; adapters advance it only after the named
/// physical step has succeeded. A failure is sticky and publication is
/// impossible until authentication, copy, and settlement all completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordSettlement {
    state: SettlementState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettlementState {
    Received,
    Authenticated,
    Copied,
    Settled,
    Published,
    Failed,
}

impl FlatOwnedRecordSettlement {
    pub const fn received() -> Self {
        Self {
            state: SettlementState::Received,
        }
    }

    pub fn authenticated(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Received, SettlementState::Authenticated)
    }

    pub fn copy_completed(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Authenticated, SettlementState::Copied)
    }

    pub fn settlement_completed(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Copied, SettlementState::Settled)
    }

    pub fn publish(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Settled, SettlementState::Published)
    }

    pub fn fail(&mut self) -> Result<(), Diagnostic> {
        if self.state == SettlementState::Published {
            return Err(error(
                "flat owned-record failure cannot replace a published result",
            ));
        }
        self.state = SettlementState::Failed;
        Ok(())
    }

    pub const fn is_published(self) -> bool {
        matches!(self.state, SettlementState::Published)
    }

    fn advance(
        &mut self,
        expected: SettlementState,
        next: SettlementState,
    ) -> Result<(), Diagnostic> {
        if self.state != expected {
            self.state = SettlementState::Failed;
            return Err(error(
                "flat owned-record publication transition is out of order",
            ));
        }
        self.state = next;
        Ok(())
    }
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

pub fn derive_flat_owned_record_api_descriptor(
    program: &ResolvedProgram,
    selected: &[String],
    subject: PublicApiSubject<'_>,
) -> Result<FlatOwnedRecordApiDescriptor, Diagnostic> {
    if subject.project_schema != FLAT_OWNED_RECORD_PROJECT_SCHEMA
        || !valid_sha256_fact(subject.project_revision)
        || !valid_sha256_fact(subject.workspace_revision)
        || !valid_sha256_fact(subject.project_graph_digest)
    {
        return Err(error("flat owned-record descriptor subject is invalid"));
    }
    validate_selected(selected)?;
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let index = PersistentCallIndex::build(program)?;
    let closure = selected_closure(program, selected, &functions, &index)?;
    for id in closure {
        validate_closure_function(
            functions
                .get(&id)
                .ok_or_else(|| error("flat owned-record closure is incomplete"))?,
        )?;
    }

    let mut exports = Vec::with_capacity(selected.len());
    let mut rust_methods = BTreeSet::new();
    for stable_id in selected {
        let function = functions
            .get(&DeclarationId::new(stable_id.clone()))
            .ok_or_else(|| error("flat owned-record export is absent"))?;
        exports.push(derive_export(
            program,
            function,
            stable_id,
            &mut rust_methods,
        )?);
    }
    let mut record_names = BTreeMap::<String, DeclarationId>::new();
    for export in &exports {
        if let Some(previous) =
            record_names.insert(export.record_host_name.clone(), export.record_id.clone())
        {
            if previous != export.record_id {
                return Err(error("flat owned-record host type identities collide"));
            }
        }
        if export
            .fields
            .iter()
            .map(|field| field.host_name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != export.fields.len()
        {
            return Err(error("flat owned-record host field identities collide"));
        }
    }
    let descriptor = FlatOwnedRecordApiDescriptor {
        project_revision: subject.project_revision.to_owned(),
        workspace_revision: subject.workspace_revision.to_owned(),
        project_graph_digest: subject.project_graph_digest.to_owned(),
        exports,
    };
    if descriptor.canonical_bytes().len() > MAX_FLAT_RECORD_DESCRIPTOR_BYTES {
        return Err(error("flat owned-record descriptor exceeds its byte limit"));
    }
    Ok(descriptor)
}

fn derive_export(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    stable_id: &str,
    rust_methods: &mut BTreeSet<String>,
) -> Result<FlatOwnedRecordExport, Diagnostic> {
    let function_fact = program
        .declarations
        .declaration(&function.id)
        .ok_or_else(|| error("flat owned-record export lacks declaration metadata"))?;
    if function_fact.identity_origin != IdentityOrigin::Explicit
        || function.id == program.entrypoint
    {
        return Err(error(
            "flat owned-record export must have an explicit non-entry stable identity",
        ));
    }
    let parameters = function
        .params
        .iter()
        .map(|parameter| {
            let ty = parameter_type(&parameter.ty, parameter.ownership)
                .ok_or_else(|| error("flat owned-record export parameter is unsupported"))?;
            Ok((parameter.id.as_str().to_owned(), parameter.name.clone(), ty))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if parameters.len() > super::MAX_PUBLIC_API_PARAMETERS {
        return Err(error("flat owned-record export has too many parameters"));
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &function.return_type
    else {
        return Err(error("flat owned-record export result is not a record"));
    };
    if !arguments.is_empty() {
        return Err(error("flat owned-record result must be monomorphic"));
    }
    let record = program
        .types
        .iter()
        .find(|candidate| &candidate.id == declaration)
        .ok_or_else(|| error("flat owned-record result declaration is absent"))?;
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(error("flat owned-record result must be an authored record"));
    };
    if !record.type_parameters.is_empty()
        || fields.is_empty()
        || fields.len() > MAX_FLAT_RECORD_FIELDS
    {
        return Err(error("flat owned-record result field inventory is invalid"));
    }
    let record_fact = program
        .declarations
        .declaration(&record.id)
        .ok_or_else(|| error("flat owned-record result lacks declaration metadata"))?;
    if record_fact.identity_origin != IdentityOrigin::Explicit {
        return Err(error("flat owned-record result requires an explicit @id"));
    }
    let mut owned = 0_usize;
    let fields = fields
        .iter()
        .enumerate()
        .map(|(ordinal, field)| {
            if field.index as usize != ordinal {
                return Err(error("flat owned-record field ordinals are not canonical"));
            }
            let fact = program
                .declarations
                .declaration(&field.id)
                .ok_or_else(|| error("flat owned-record field lacks declaration metadata"))?;
            if fact.identity_origin != IdentityOrigin::Explicit {
                return Err(error(
                    "flat owned-record fields require explicit @id values",
                ));
            }
            let ty = match field.ty {
                ResolvedType::I64 => FlatOwnedRecordFieldType::I64,
                ResolvedType::Bool => FlatOwnedRecordFieldType::Bool,
                ResolvedType::Usize => FlatOwnedRecordFieldType::Usize,
                ResolvedType::Bytes => {
                    owned += 1;
                    FlatOwnedRecordFieldType::OwnedBytes
                }
                _ => return Err(error("flat owned-record field type is unsupported")),
            };
            Ok(FlatOwnedRecordField {
                stable_id: field.id.clone(),
                source_name: field.name.clone(),
                host_name: host_field_name(&field.name, field.id.as_str()),
                ordinal: field.index,
                ty,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if owned != 1 {
        return Err(error(
            "flat owned-record result requires exactly one direct Bytes field",
        ));
    }
    let rust_method_name = rust_method_name(stable_id)?;
    if !rust_methods.insert(rust_method_name.clone()) {
        return Err(error("flat owned-record Rust method identities collide"));
    }
    Ok(FlatOwnedRecordExport {
        stable_id: function.id.clone(),
        typescript_name: stable_id.to_owned(),
        rust_method_name,
        parameters,
        record_id: record.id.clone(),
        record_host_name: host_record_name(&record.name, record.id.as_str()),
        record_source_name: record.name.clone(),
        fields,
    })
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

pub fn render_flat_owned_record_typescript(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::new();
    for (name, export) in records {
        output.push_str("export interface ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("  readonly ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.typescript());
            output.push_str(";\n");
        }
        output.push_str("}\n");
    }
    output.push_str("export interface SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("  readonly ");
        output.push_str(&quote_json(export.typescript_name()));
        output.push_str(": (");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            output.push_str("arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_typescript(*ty));
        }
        output.push_str(") => ");
        output.push_str(&export.record_host_name);
        output.push_str(";\n");
    }
    output.push_str("}\n");
    output
}

pub fn render_flat_owned_record_rust(descriptor: &FlatOwnedRecordApiDescriptor) -> String {
    let mut records = BTreeMap::<&str, &FlatOwnedRecordExport>::new();
    for export in &descriptor.exports {
        records.entry(&export.record_host_name).or_insert(export);
    }
    let mut output = String::from(
        "#![forbid(unsafe_code)]\n#[derive(Clone, Debug, Eq, PartialEq)]\npub struct CallError { message: &'static str }\nimpl CallError { pub fn message(&self) -> &str { self.message } }\n",
    );
    for (name, export) in records {
        output.push_str("#[derive(Clone, Debug, Eq, PartialEq)]\npub struct ");
        output.push_str(name);
        output.push_str(" {\n");
        for field in &export.fields {
            output.push_str("    pub ");
            output.push_str(&field.host_name);
            output.push_str(": ");
            output.push_str(field.ty.rust());
            output.push_str(",\n");
        }
        output.push_str("}\n");
    }
    output.push_str("pub trait SemapraxApi {\n");
    for export in &descriptor.exports {
        output.push_str("    fn ");
        output.push_str(export.rust_method_name());
        output.push_str("(&self");
        for (index, (_, _, ty)) in export.parameters.iter().enumerate() {
            output.push_str(", arg");
            output.push_str(&index.to_string());
            output.push_str(": ");
            output.push_str(parameter_rust(*ty));
        }
        output.push_str(") -> Result<");
        output.push_str(&export.record_host_name);
        output.push_str(", CallError>;\n");
    }
    output.push_str("}\n");
    output
}

/// Render the v9 npm semantic metadata. Publication code must bind these bytes
/// into the additive v8 npm carrier; this function performs no I/O.
pub fn render_flat_owned_record_metadata(
    descriptor: &FlatOwnedRecordApiDescriptor,
    wasm_sha256: &str,
) -> Result<Vec<u8>, Diagnostic> {
    if !valid_sha256_fact(wasm_sha256) {
        return Err(error("flat owned-record Wasm digest is invalid"));
    }
    let mut output = String::new();
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(FLAT_OWNED_RECORD_METADATA_SCHEMA));
    output.push_str(",\"descriptor\":");
    output.push_str(&quote_json(
        &String::from_utf8(descriptor.canonical_bytes()).expect("canonical descriptor is UTF-8"),
    ));
    output.push_str(",\"descriptor_digest\":");
    output.push_str(&quote_json(&descriptor.digest()));
    output.push_str(",\"wasm_sha256\":");
    output.push_str(&quote_json(wasm_sha256));
    output.push_str(",\"result_carrier\":\"opaque-handle-plus-scalars.v1\",\"settlement\":{\"copy_before_settle\":true,\"publish_after_settle\":true,\"failure_slot_unchanged\":true},\"artifacts\":[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\",\"package.json\"]}\n");
    Ok(output.into_bytes())
}

pub fn replay_flat_owned_record_metadata(
    descriptor: &FlatOwnedRecordApiDescriptor,
    wasm_sha256: &str,
    submitted: &[u8],
) -> Result<(), Diagnostic> {
    let value: Value = serde_json::from_slice(submitted)
        .map_err(|_| error("flat owned-record npm metadata is invalid"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 7)
        .ok_or_else(|| error("flat owned-record npm metadata root is not closed"))?;
    for key in root.keys() {
        if !matches!(
            key.as_str(),
            "schema"
                | "descriptor"
                | "descriptor_digest"
                | "wasm_sha256"
                | "result_carrier"
                | "settlement"
                | "artifacts"
        ) {
            return Err(error("flat owned-record npm metadata has an unknown field"));
        }
    }
    if submitted != render_flat_owned_record_metadata(descriptor, wasm_sha256)? {
        return Err(error(
            "flat owned-record npm metadata does not replay exactly",
        ));
    }
    Ok(())
}

/// Render the target-neutral safe-Rust package manifest inputs. The private
/// FFI/provider inventory is digest-bound but never projected into safe code.
pub fn render_flat_owned_record_rust_sdk_manifest(
    descriptor: &FlatOwnedRecordApiDescriptor,
    provider_inventory_digest: &str,
) -> Result<Vec<u8>, Diagnostic> {
    if !valid_sha256_fact(provider_inventory_digest) {
        return Err(error(
            "flat owned-record provider inventory digest is invalid",
        ));
    }
    let mut output = String::new();
    output.push_str("{\"schema\":");
    output.push_str(&quote_json(FLAT_OWNED_RECORD_RUST_SDK_SCHEMA));
    output.push_str(",\"descriptor\":");
    output.push_str(&quote_json(
        &String::from_utf8(descriptor.canonical_bytes()).expect("canonical descriptor is UTF-8"),
    ));
    output.push_str(",\"descriptor_digest\":");
    output.push_str(&quote_json(&descriptor.digest()));
    output.push_str(",\"provider_inventory_digest\":");
    output.push_str(&quote_json(provider_inventory_digest));
    output.push_str(",\"safe_api\":\"forbid-unsafe.v1\",\"result_carrier\":\"private-opaque-handle-plus-scalars.v1\",\"settlement\":{\"copy_before_settle\":true,\"publish_after_settle\":true,\"panic_crosses_ffi\":false}}\n");
    Ok(output.into_bytes())
}

pub fn replay_flat_owned_record_rust_sdk_manifest(
    descriptor: &FlatOwnedRecordApiDescriptor,
    provider_inventory_digest: &str,
    submitted: &[u8],
) -> Result<(), Diagnostic> {
    let value: Value = serde_json::from_slice(submitted)
        .map_err(|_| error("flat owned-record Rust SDK manifest is invalid"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 7)
        .ok_or_else(|| error("flat owned-record Rust SDK manifest root is not closed"))?;
    for key in root.keys() {
        if !matches!(
            key.as_str(),
            "schema"
                | "descriptor"
                | "descriptor_digest"
                | "provider_inventory_digest"
                | "safe_api"
                | "result_carrier"
                | "settlement"
        ) {
            return Err(error(
                "flat owned-record Rust SDK manifest has an unknown field",
            ));
        }
    }
    if submitted
        != render_flat_owned_record_rust_sdk_manifest(descriptor, provider_inventory_digest)?
    {
        return Err(error(
            "flat owned-record Rust SDK manifest does not replay exactly",
        ));
    }
    Ok(())
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

fn stable_host_name(prefix: &str, stable_id: &str) -> String {
    let digest = Sha256::digest(stable_id.as_bytes());
    let hex = format!("{:x}", crate::digest_hex::LowerHex(digest));
    match prefix {
        "record" => format!("SpxRecordH{hex}"),
        "field" => format!("spx_field_h{hex}"),
        _ => unreachable!("closed host-name family"),
    }
}

fn host_record_name(source_name: &str, stable_id: &str) -> String {
    if source_name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase())
        && source_name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        source_name.to_owned()
    } else {
        stable_host_name("record", stable_id)
    }
}

fn host_field_name(source_name: &str, stable_id: &str) -> String {
    const RUST_KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ];
    if source_name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && source_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !RUST_KEYWORDS.contains(&source_name)
    {
        source_name.to_owned()
    } else {
        stable_host_name("field", stable_id)
    }
}

fn parameter_typescript(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "bigint",
        PublicApiParameterType::Bool => "boolean",
        PublicApiParameterType::BorrowStr => "string",
        PublicApiParameterType::BorrowSliceU8 => "Uint8Array",
    }
}

fn parameter_rust(ty: PublicApiParameterType) -> &'static str {
    match ty {
        PublicApiParameterType::I64 => "i64",
        PublicApiParameterType::Bool => "bool",
        PublicApiParameterType::BorrowStr => "&str",
        PublicApiParameterType::BorrowSliceU8 => "&[u8]",
    }
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
