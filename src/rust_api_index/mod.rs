//! Replay-only admission for compiler-resolved Rust API metadata.
//!
//! This crate deliberately does not invoke Cargo, rustc, rustdoc, or a macro
//! expander.  A separately installed, pinned extractor may emit the compact
//! envelope accepted here.  Stable consumers only replay a prepared envelope;
//! they never silently download or select a nightly compiler.

mod closed_owner;
mod demand;

pub use demand::{
    enabled_projected_records, projected_records_from_json, projected_records_to_json,
    resolve_demanded_associated_types, resolve_demanded_instantiations, AssociatedTypeRequest,
    ConcreteType, ConstArgument, DemandError, DemandedAssociatedType, DemandedInstantiation,
    InstantiationRequest, ProjectedRecord, ProjectionError, MAX_CONCRETE_TYPE_BYTES,
    MAX_DEMANDED_INSTANTIATIONS, MAX_PROJECTED_LABEL_BYTES, MAX_PROJECTED_RECORDS,
};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const RUST_API_INDEX_SCHEMA: &str = "semaprax.rust-api-index.v2";
pub const RUSTDOC_EXTRACTOR_SCHEMA: &str = "semaprax.rustdoc-extractor.v2";
pub const MAX_INDEX_BYTES: usize = 1_048_576;
pub const MAX_ITEMS: usize = 512;
pub const MAX_TYPES: usize = 512;
pub const MAX_PATH_BYTES: usize = 512;
pub const MAX_SIGNATURE_BYTES: usize = 4_096;
pub const MAX_DOC_BYTES: usize = 16_384;
pub const MAX_TOTAL_DOC_BYTES: usize = 524_288;
pub const MAX_GENERIC_PARAMS: usize = 64;
pub const MAX_GENERIC_METADATA_BYTES: usize = 16_384;
pub const MAX_TYPE_REFERENCES: usize = 256;
pub const MAX_TYPE_DEPTH: usize = 32;
pub const EXTRACTOR_SETUP_INSTRUCTION: &str = "Install the project-pinned nightly rustdoc JSON extractor, or provide an index prepared for the selected stable target and features.";

const INDEX_DIGEST_DOMAIN: &[u8] = b"semaprax.rust-api-index.digest.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustApiIndex {
    canonical: String,
    digest: String,
    package: PackageIdentity,
    target: String,
    feature_digest: String,
    extractor: ExtractorIdentity,
    items: Vec<ApiItem>,
    types: Vec<TypeRecord>,
    stable_rustc_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageIdentity {
    pub name: String,
    pub version: String,
    pub source_sha256: String,
    pub renamed_from: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractorIdentity {
    pub mode: ExtractorMode,
    pub executable_sha256: String,
    pub rustc_version: String,
    pub rustdoc_format: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Private,
    Crate,
    Restricted,
    External,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenericParameterKind {
    Lifetime,
    Type,
    Const,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericParameter {
    pub name: String,
    pub kind: GenericParameterKind,
    /// Canonical rustdoc JSON fragments retain trait and lifetime bounds.
    pub bounds: Vec<String>,
    pub default: Option<String>,
    pub const_type: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericMetadata {
    pub parameters: Vec<GenericParameter>,
    /// Canonical rustdoc JSON fragments for `where` predicates.
    pub where_predicates: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeMetadata {
    pub bounds: Vec<String>,
    pub default: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractorMode {
    Prepared,
    NightlyRustdocJson,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiItem {
    pub path: String,
    pub kind: ItemKind,
    pub receiver: Receiver,
    pub signature: String,
    pub type_depth: usize,
    pub visibility: Visibility,
    pub docs: Option<String>,
    pub span: Option<SourceSpan>,
    pub generics: GenericMetadata,
    pub associated_type: Option<AssociatedTypeMetadata>,
    pub type_roots: Vec<String>,
    pub reachable_types: Vec<String>,
    pub type_closure_depth: usize,
    pub closure_complete: bool,
    pub support: Support,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeRecordKind {
    External,
    Struct,
    Enum,
    Union,
    TypeAlias,
    Trait,
    AssociatedType,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRecord {
    pub path: String,
    pub kind: TypeRecordKind,
    pub visibility: Visibility,
    pub docs: Option<String>,
    pub span: Option<SourceSpan>,
    pub generics: GenericMetadata,
    pub references: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Function,
    InherentMethod,
    TraitMethod,
    AssociatedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Receiver {
    None,
    Shared,
    Mutable,
    Owned,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Support {
    Supported,
    Rejected { reason: RejectionReason },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    Private,
    SealedTrait,
    OpaqueReturn,
    UnsupportedGeneric,
    UnsupportedSignature,
    IncompleteTypeClosure,
    ExpansionLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IndexError {
    Malformed,
    SetupRequired,
    IdentityMismatch,
    ItemUnavailable,
}

impl RustApiIndex {
    /// Replays an exact prepared index.  It accepts no extractor output and
    /// therefore remains usable on a stable-only installation.
    pub fn replay(bytes: &[u8]) -> Result<Self, IndexError> {
        validate_input(bytes)?;
        let value: Value = serde_json::from_slice(bytes).map_err(|_| IndexError::Malformed)?;
        let index = Self::parse_value(&value)?;
        if index.canonical.as_bytes() != bytes {
            return Err(IndexError::Malformed);
        }
        let digest = index_digest(bytes);
        Ok(Self { digest, ..index })
    }

    fn parse_value(value: &Value) -> Result<Self, IndexError> {
        let root = exact_object(value, 9)?;
        if string(root, "schema")? != RUST_API_INDEX_SCHEMA {
            return Err(IndexError::Malformed);
        }
        let package = parse_package(required(root, "package")?)?;
        let target = bounded(string(root, "target")?, MAX_PATH_BYTES)?;
        let feature_digest = digest(string(root, "feature_digest")?)?.to_owned();
        let stable_rustc_version = parse_stable_rustc(string(root, "stable_rustc_version")?)?;
        let extractor = parse_extractor(required(root, "extractor")?)?;
        let limits = exact_object(required(root, "limits")?, 7)?;
        if number(limits, "max_items")? != MAX_ITEMS
            || number(limits, "max_types")? != MAX_TYPES
            || number(limits, "max_type_depth")? != MAX_TYPE_DEPTH
            || number(limits, "max_type_references")? != MAX_TYPE_REFERENCES
            || number(limits, "max_index_bytes")? != MAX_INDEX_BYTES
            || number(limits, "max_doc_bytes")? != MAX_DOC_BYTES
            || number(limits, "max_total_doc_bytes")? != MAX_TOTAL_DOC_BYTES
        {
            return Err(IndexError::Malformed);
        }
        let rows = required(root, "items")?
            .as_array()
            .filter(|rows| rows.len() <= MAX_ITEMS)
            .ok_or(IndexError::Malformed)?;
        let mut paths = BTreeSet::new();
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let item = parse_item(row)?;
            if !paths.insert(item.path.clone()) {
                return Err(IndexError::Malformed);
            }
            items.push(item);
        }
        if !items
            .windows(2)
            .all(|pair| pair[0].path.as_bytes() < pair[1].path.as_bytes())
        {
            return Err(IndexError::Malformed);
        }
        let type_rows = required(root, "types")?
            .as_array()
            .filter(|rows| rows.len() <= MAX_TYPES)
            .ok_or(IndexError::Malformed)?;
        let mut type_paths = BTreeSet::new();
        let mut types = Vec::with_capacity(type_rows.len());
        for row in type_rows {
            let record = parse_type_record(row)?;
            if !type_paths.insert(record.path.clone()) {
                return Err(IndexError::Malformed);
            }
            types.push(record);
        }
        if !types
            .windows(2)
            .all(|pair| pair[0].path.as_bytes() < pair[1].path.as_bytes())
        {
            return Err(IndexError::Malformed);
        }
        let type_map = types
            .iter()
            .map(|record| (record.path.as_str(), record))
            .collect::<std::collections::BTreeMap<_, _>>();
        if types
            .iter()
            .flat_map(|record| &record.references)
            .any(|path| !type_map.contains_key(path.as_str()))
        {
            return Err(IndexError::Malformed);
        }
        let mut total_doc_bytes = 0usize;
        for item in &items {
            total_doc_bytes = total_doc_bytes
                .checked_add(item.docs.as_ref().map_or(0, String::len))
                .ok_or(IndexError::Malformed)?;
            let (complete, depth) = validate_type_closure(item, &type_map)?;
            if item.closure_complete != complete || item.type_closure_depth != depth {
                return Err(IndexError::Malformed);
            }
        }
        for record in &types {
            total_doc_bytes = total_doc_bytes
                .checked_add(record.docs.as_ref().map_or(0, String::len))
                .ok_or(IndexError::Malformed)?;
        }
        if total_doc_bytes > MAX_TOTAL_DOC_BYTES {
            return Err(IndexError::Malformed);
        }
        let index = Self {
            canonical: render(
                &package,
                &target,
                &feature_digest,
                &stable_rustc_version,
                &extractor,
                &items,
                &types,
            ),
            digest: String::new(),
            package,
            target,
            feature_digest,
            extractor,
            items,
            types,
            stable_rustc_version,
        };
        Ok(index)
    }

    /// Admits output from the explicit extractor protocol.  This does not run
    /// the extractor.  A missing extractor is a setup error, never a fallback.
    pub fn admit_extractor_output(bytes: &[u8]) -> Result<Self, IndexError> {
        validate_input(bytes)?;
        let value: Value = serde_json::from_slice(bytes).map_err(|_| IndexError::Malformed)?;
        let root = exact_object(&value, 2)?;
        if string(root, "schema")? != RUSTDOC_EXTRACTOR_SCHEMA {
            return Err(IndexError::SetupRequired);
        }
        let admitted = Self::parse_value(required(root, "index")?)?;
        if admitted.extractor.mode != ExtractorMode::NightlyRustdocJson {
            return Err(IndexError::Malformed);
        }
        Ok(admitted)
    }

    pub fn canonical_json(&self) -> &str {
        &self.canonical
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn package(&self) -> &PackageIdentity {
        &self.package
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn feature_digest(&self) -> &str {
        &self.feature_digest
    }
    pub fn extractor(&self) -> &ExtractorIdentity {
        &self.extractor
    }
    pub fn stable_rustc_version(&self) -> &str {
        &self.stable_rustc_version
    }
    pub fn items(&self) -> &[ApiItem] {
        &self.items
    }
    pub fn types(&self) -> &[TypeRecord] {
        &self.types
    }
}

fn parse_package(value: &Value) -> Result<PackageIdentity, IndexError> {
    let object = exact_object(value, 4)?;
    let renamed = match required(object, "renamed_from")? {
        Value::Null => None,
        Value::String(value) => Some(bounded(value, MAX_PATH_BYTES)?),
        _ => return Err(IndexError::Malformed),
    };
    Ok(PackageIdentity {
        name: bounded(string(object, "name")?, MAX_PATH_BYTES)?,
        version: bounded(string(object, "version")?, 128)?,
        source_sha256: digest(string(object, "source_sha256")?)?.to_owned(),
        renamed_from: renamed,
    })
}

fn parse_extractor(value: &Value) -> Result<ExtractorIdentity, IndexError> {
    let object = exact_object(value, 4)?;
    let mode = match string(object, "mode")? {
        "prepared" => ExtractorMode::Prepared,
        "nightly-rustdoc-json" => ExtractorMode::NightlyRustdocJson,
        _ => return Err(IndexError::Malformed),
    };
    if mode == ExtractorMode::Prepared
        && !string(object, "rustdoc_format")?.starts_with("prepared:")
    {
        return Err(IndexError::Malformed);
    }
    Ok(ExtractorIdentity {
        mode,
        executable_sha256: digest(string(object, "executable_sha256")?)?.to_owned(),
        rustc_version: bounded(string(object, "rustc_version")?, 256)?,
        rustdoc_format: bounded(string(object, "rustdoc_format")?, 256)?,
    })
}

fn parse_item(value: &Value) -> Result<ApiItem, IndexError> {
    let object = exact_object(value, 16)?;
    let kind = match string(object, "kind")? {
        "function" => ItemKind::Function,
        "inherent_method" => ItemKind::InherentMethod,
        "trait_method" => ItemKind::TraitMethod,
        "associated_type" => ItemKind::AssociatedType,
        _ => return Err(IndexError::Malformed),
    };
    let receiver = match string(object, "receiver")? {
        "none" => Receiver::None,
        "shared" => Receiver::Shared,
        "mutable" => Receiver::Mutable,
        "owned" => Receiver::Owned,
        _ => return Err(IndexError::Malformed),
    };
    let type_depth = number(object, "type_depth")?;
    if type_depth > MAX_TYPE_DEPTH {
        return Err(IndexError::Malformed);
    }
    let visibility = parse_visibility(string(object, "visibility")?)?;
    let docs = parse_docs(required(object, "docs")?)?;
    let span = parse_span(required(object, "span")?)?;
    let generics = parse_generics(required(object, "generics")?)?;
    let associated_type = parse_associated_type(required(object, "associated_type")?)?;
    let type_roots = parse_paths(required(object, "type_roots")?, MAX_TYPE_REFERENCES)?;
    let reachable_types = parse_paths(required(object, "reachable_types")?, MAX_TYPE_REFERENCES)?;
    let type_closure_depth = number(object, "type_closure_depth")?;
    if type_closure_depth > MAX_TYPE_DEPTH {
        return Err(IndexError::Malformed);
    }
    let closure_complete = required(object, "closure_complete")?
        .as_bool()
        .ok_or(IndexError::Malformed)?;
    let support = match string(object, "support")? {
        "supported" if required(object, "reason")? == &Value::Null => Support::Supported,
        "rejected" => Support::Rejected {
            reason: parse_reason(string(object, "reason")?)?,
        },
        _ => return Err(IndexError::Malformed),
    };
    let item_path = bounded(string(object, "path")?, MAX_PATH_BYTES)?;
    if visibility != Visibility::Public
        && support
            != (Support::Rejected {
                reason: RejectionReason::Private,
            })
    {
        return Err(IndexError::Malformed);
    }
    if !closure_complete && support == Support::Supported {
        return Err(IndexError::Malformed);
    }
    if (kind == ItemKind::AssociatedType) != associated_type.is_some() {
        return Err(IndexError::Malformed);
    }
    Ok(ApiItem {
        path: item_path,
        kind,
        receiver,
        signature: bounded(string(object, "signature")?, MAX_SIGNATURE_BYTES)?,
        type_depth,
        visibility,
        docs,
        span,
        generics,
        associated_type,
        type_roots,
        reachable_types,
        type_closure_depth,
        closure_complete,
        support,
    })
}

fn parse_type_record(value: &Value) -> Result<TypeRecord, IndexError> {
    let object = exact_object(value, 7)?;
    let kind = match string(object, "kind")? {
        "external" => TypeRecordKind::External,
        "struct" => TypeRecordKind::Struct,
        "enum" => TypeRecordKind::Enum,
        "union" => TypeRecordKind::Union,
        "type_alias" => TypeRecordKind::TypeAlias,
        "trait" => TypeRecordKind::Trait,
        "associated_type" => TypeRecordKind::AssociatedType,
        "other" => TypeRecordKind::Other,
        _ => return Err(IndexError::Malformed),
    };
    let references = parse_paths(required(object, "references")?, MAX_TYPE_REFERENCES)?;
    Ok(TypeRecord {
        path: bounded(string(object, "path")?, MAX_PATH_BYTES)?,
        kind,
        visibility: parse_visibility(string(object, "visibility")?)?,
        docs: parse_docs(required(object, "docs")?)?,
        span: parse_span(required(object, "span")?)?,
        generics: parse_generics(required(object, "generics")?)?,
        references,
    })
}

fn parse_visibility(value: &str) -> Result<Visibility, IndexError> {
    match value {
        "public" => Ok(Visibility::Public),
        "private" => Ok(Visibility::Private),
        "crate" => Ok(Visibility::Crate),
        "restricted" => Ok(Visibility::Restricted),
        "external" => Ok(Visibility::External),
        _ => Err(IndexError::Malformed),
    }
}

fn parse_docs(value: &Value) -> Result<Option<String>, IndexError> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => {
            if text.len() > MAX_DOC_BYTES || text.contains('\0') {
                return Err(IndexError::Malformed);
            }
            Ok(Some(text.clone()))
        }
        _ => Err(IndexError::Malformed),
    }
}

fn parse_span(value: &Value) -> Result<Option<SourceSpan>, IndexError> {
    if value.is_null() {
        return Ok(None);
    }
    let object = exact_object(value, 5)?;
    let file = bounded(string(object, "file")?, MAX_PATH_BYTES)?;
    if file.starts_with('/') || file.split('/').any(|part| part == ".." || part == ".") {
        return Err(IndexError::Malformed);
    }
    let span = SourceSpan {
        file,
        start_line: number(object, "start_line")?,
        start_column: number(object, "start_column")?,
        end_line: number(object, "end_line")?,
        end_column: number(object, "end_column")?,
    };
    if span.start_line == 0
        || span.start_column == 0
        || span.end_line < span.start_line
        || (span.end_line == span.start_line && span.end_column < span.start_column)
    {
        return Err(IndexError::Malformed);
    }
    Ok(Some(span))
}

fn parse_generics(value: &Value) -> Result<GenericMetadata, IndexError> {
    let object = exact_object(value, 2)?;
    let parameters = required(object, "parameters")?
        .as_array()
        .filter(|parameters| parameters.len() <= MAX_GENERIC_PARAMS)
        .ok_or(IndexError::Malformed)?;
    let mut parsed_parameters = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let parameter = exact_object(parameter, 5)?;
        let kind = match string(parameter, "kind")? {
            "lifetime" => GenericParameterKind::Lifetime,
            "type" => GenericParameterKind::Type,
            "const" => GenericParameterKind::Const,
            _ => return Err(IndexError::Malformed),
        };
        let bounds = parse_json_fragments(required(parameter, "bounds")?)?;
        let default =
            parse_optional_text(required(parameter, "default")?, MAX_GENERIC_METADATA_BYTES)?;
        let const_type = parse_optional_text(
            required(parameter, "const_type")?,
            MAX_GENERIC_METADATA_BYTES,
        )?;
        if (kind == GenericParameterKind::Const) != const_type.is_some() {
            return Err(IndexError::Malformed);
        }
        parsed_parameters.push(GenericParameter {
            name: bounded(string(parameter, "name")?, MAX_PATH_BYTES)?,
            kind,
            bounds,
            default,
            const_type,
        });
    }
    let where_predicates = parse_json_fragments(required(object, "where_predicates")?)?;
    Ok(GenericMetadata {
        parameters: parsed_parameters,
        where_predicates,
    })
}

fn parse_associated_type(value: &Value) -> Result<Option<AssociatedTypeMetadata>, IndexError> {
    if value.is_null() {
        return Ok(None);
    }
    let object = exact_object(value, 2)?;
    Ok(Some(AssociatedTypeMetadata {
        bounds: parse_json_fragments(required(object, "bounds")?)?,
        default: parse_optional_text(required(object, "default")?, MAX_GENERIC_METADATA_BYTES)?,
    }))
}

fn parse_json_fragments(value: &Value) -> Result<Vec<String>, IndexError> {
    let fragments = value
        .as_array()
        .filter(|fragments| fragments.len() <= MAX_GENERIC_PARAMS)
        .ok_or(IndexError::Malformed)?;
    fragments
        .iter()
        .map(|fragment| {
            let text = string_value(fragment)?;
            if text.len() > MAX_GENERIC_METADATA_BYTES {
                return Err(IndexError::Malformed);
            }
            let parsed: Value = serde_json::from_str(text).map_err(|_| IndexError::Malformed)?;
            if serde_json::to_string(&parsed).map_err(|_| IndexError::Malformed)? != text {
                return Err(IndexError::Malformed);
            }
            Ok(text.to_owned())
        })
        .collect()
}

fn parse_paths(value: &Value, maximum: usize) -> Result<Vec<String>, IndexError> {
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= maximum)
        .ok_or(IndexError::Malformed)?;
    let paths = rows
        .iter()
        .map(|value| bounded(string_value(value)?, MAX_PATH_BYTES))
        .collect::<Result<Vec<_>, _>>()?;
    if !paths
        .windows(2)
        .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        return Err(IndexError::Malformed);
    }
    Ok(paths)
}

fn validate_type_closure(
    item: &ApiItem,
    types: &std::collections::BTreeMap<&str, &TypeRecord>,
) -> Result<(bool, usize), IndexError> {
    let mut pending = item
        .type_roots
        .iter()
        .map(|path| (path.as_str(), 1usize))
        .collect::<std::collections::VecDeque<_>>();
    let mut reached = std::collections::BTreeMap::<&str, usize>::new();
    let mut complete = true;
    let mut maximum_depth = 0;
    while let Some((path, depth)) = pending.pop_front() {
        if reached.contains_key(path) {
            continue;
        }
        if depth > MAX_TYPE_DEPTH {
            return Err(IndexError::Malformed);
        }
        let record = types.get(path).ok_or(IndexError::Malformed)?;
        reached.insert(path, depth);
        maximum_depth = maximum_depth.max(depth);
        complete &= record.visibility == Visibility::Public
            && !matches!(
                record.kind,
                TypeRecordKind::External | TypeRecordKind::Other
            );
        for reference in &record.references {
            pending.push_back((reference.as_str(), depth + 1));
        }
        if reached.len() > MAX_TYPE_REFERENCES {
            return Err(IndexError::Malformed);
        }
    }
    let reached_paths = reached.keys().copied().collect::<Vec<_>>();
    if reached_paths
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>()
        != item.reachable_types
    {
        return Err(IndexError::Malformed);
    }
    Ok((complete, maximum_depth))
}

fn parse_reason(value: &str) -> Result<RejectionReason, IndexError> {
    match value {
        "private" => Ok(RejectionReason::Private),
        "sealed_trait" => Ok(RejectionReason::SealedTrait),
        "opaque_return" => Ok(RejectionReason::OpaqueReturn),
        "unsupported_generic" => Ok(RejectionReason::UnsupportedGeneric),
        "unsupported_signature" => Ok(RejectionReason::UnsupportedSignature),
        "incomplete_type_closure" => Ok(RejectionReason::IncompleteTypeClosure),
        "expansion_limit" => Ok(RejectionReason::ExpansionLimit),
        _ => Err(IndexError::Malformed),
    }
}

fn validate_input(bytes: &[u8]) -> Result<(), IndexError> {
    if bytes.is_empty()
        || bytes.len() > MAX_INDEX_BYTES
        || !bytes.ends_with(b"\n")
        || bytes.contains(&0)
    {
        return Err(IndexError::Malformed);
    }
    Ok(())
}

fn exact_object(value: &Value, fields: usize) -> Result<&Map<String, Value>, IndexError> {
    value
        .as_object()
        .filter(|object| object.len() == fields)
        .ok_or(IndexError::Malformed)
}
fn required<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a Value, IndexError> {
    object.get(name).ok_or(IndexError::Malformed)
}
fn string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, IndexError> {
    required(object, name)?
        .as_str()
        .ok_or(IndexError::Malformed)
}
fn number(object: &Map<String, Value>, name: &str) -> Result<usize, IndexError> {
    required(object, name)?
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(IndexError::Malformed)
}
fn bounded(value: &str, maximum: usize) -> Result<String, IndexError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        Err(IndexError::Malformed)
    } else {
        Ok(value.to_owned())
    }
}
fn string_value(value: &Value) -> Result<&str, IndexError> {
    value.as_str().ok_or(IndexError::Malformed)
}
fn parse_optional_text(value: &Value, maximum: usize) -> Result<Option<String>, IndexError> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(bounded(text, maximum)?)),
        _ => Err(IndexError::Malformed),
    }
}
fn parse_stable_rustc(value: &str) -> Result<String, IndexError> {
    let value = bounded(value, 256)?;
    let release = value
        .split_whitespace()
        .nth(1)
        .ok_or(IndexError::Malformed)?;
    if !value.starts_with("rustc ") || release.contains('-') {
        return Err(IndexError::Malformed);
    }
    Ok(value)
}
fn digest(value: &str) -> Result<&str, IndexError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value)
    } else {
        Err(IndexError::Malformed)
    }
}

fn index_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(INDEX_DIGEST_DOMAIN);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    let hex = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn render(
    package: &PackageIdentity,
    target: &str,
    feature_digest: &str,
    stable_rustc_version: &str,
    extractor: &ExtractorIdentity,
    items: &[ApiItem],
    types: &[TypeRecord],
) -> String {
    let document = serde_json::json!({
        "schema": RUST_API_INDEX_SCHEMA,
        "package": {
            "name": package.name,
            "version": package.version,
            "source_sha256": package.source_sha256,
            "renamed_from": package.renamed_from,
        },
        "target": target,
        "feature_digest": feature_digest,
        "stable_rustc_version": stable_rustc_version,
        "extractor": {
            "mode": mode_name(extractor.mode),
            "executable_sha256": extractor.executable_sha256,
            "rustc_version": extractor.rustc_version,
            "rustdoc_format": extractor.rustdoc_format,
        },
        "items": items.iter().map(item_value).collect::<Vec<_>>(),
        "types": types.iter().map(type_record_value).collect::<Vec<_>>(),
        "limits": {
            "max_items": MAX_ITEMS,
            "max_types": MAX_TYPES,
            "max_type_depth": MAX_TYPE_DEPTH,
            "max_type_references": MAX_TYPE_REFERENCES,
            "max_index_bytes": MAX_INDEX_BYTES,
            "max_doc_bytes": MAX_DOC_BYTES,
            "max_total_doc_bytes": MAX_TOTAL_DOC_BYTES,
        }
    });
    format!(
        "{}\n",
        serde_json::to_string(&document).expect("serializable index")
    )
}

fn item_value(item: &ApiItem) -> Value {
    let (support, reason) = match item.support {
        Support::Supported => ("supported", Value::Null),
        Support::Rejected { reason } => ("rejected", Value::String(reason_name(reason).to_owned())),
    };
    serde_json::json!({
        "path": item.path,
        "kind": kind_name(item.kind),
        "receiver": receiver_name(item.receiver),
        "signature": item.signature,
        "type_depth": item.type_depth,
        "visibility": visibility_name(item.visibility),
        "docs": item.docs,
        "span": item.span.as_ref().map(span_value),
        "generics": generics_value(&item.generics),
        "associated_type": item.associated_type.as_ref().map(associated_type_value),
        "type_roots": item.type_roots,
        "reachable_types": item.reachable_types,
        "type_closure_depth": item.type_closure_depth,
        "closure_complete": item.closure_complete,
        "support": support,
        "reason": reason,
    })
}

fn type_record_value(record: &TypeRecord) -> Value {
    serde_json::json!({
        "path": record.path,
        "kind": type_kind_name(record.kind),
        "visibility": visibility_name(record.visibility),
        "docs": record.docs,
        "span": record.span.as_ref().map(span_value),
        "generics": generics_value(&record.generics),
        "references": record.references,
    })
}

fn generics_value(generics: &GenericMetadata) -> Value {
    let parameters = generics
        .parameters
        .iter()
        .map(|parameter| {
            serde_json::json!({
                "name": parameter.name,
                "kind": generic_kind_name(parameter.kind),
                "bounds": parse_fragments_value(&parameter.bounds),
                "default": parameter.default,
                "const_type": parameter.const_type,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "parameters": parameters,
        "where_predicates": parse_fragments_value(&generics.where_predicates),
    })
}

fn associated_type_value(metadata: &AssociatedTypeMetadata) -> Value {
    serde_json::json!({
        "bounds": parse_fragments_value(&metadata.bounds),
        "default": metadata.default,
    })
}

fn parse_fragments_value(fragments: &[String]) -> Vec<String> {
    fragments.to_vec()
}

fn span_value(span: &SourceSpan) -> Value {
    serde_json::json!({
        "file": span.file,
        "start_line": span.start_line,
        "start_column": span.start_column,
        "end_line": span.end_line,
        "end_column": span.end_column,
    })
}

fn mode_name(value: ExtractorMode) -> &'static str {
    match value {
        ExtractorMode::Prepared => "prepared",
        ExtractorMode::NightlyRustdocJson => "nightly-rustdoc-json",
    }
}
fn kind_name(value: ItemKind) -> &'static str {
    match value {
        ItemKind::Function => "function",
        ItemKind::InherentMethod => "inherent_method",
        ItemKind::TraitMethod => "trait_method",
        ItemKind::AssociatedType => "associated_type",
    }
}
fn receiver_name(value: Receiver) -> &'static str {
    match value {
        Receiver::None => "none",
        Receiver::Shared => "shared",
        Receiver::Mutable => "mutable",
        Receiver::Owned => "owned",
    }
}
fn reason_name(value: RejectionReason) -> &'static str {
    match value {
        RejectionReason::Private => "private",
        RejectionReason::SealedTrait => "sealed_trait",
        RejectionReason::OpaqueReturn => "opaque_return",
        RejectionReason::UnsupportedGeneric => "unsupported_generic",
        RejectionReason::UnsupportedSignature => "unsupported_signature",
        RejectionReason::IncompleteTypeClosure => "incomplete_type_closure",
        RejectionReason::ExpansionLimit => "expansion_limit",
    }
}

fn visibility_name(value: Visibility) -> &'static str {
    match value {
        Visibility::Public => "public",
        Visibility::Private => "private",
        Visibility::Crate => "crate",
        Visibility::Restricted => "restricted",
        Visibility::External => "external",
    }
}

fn generic_kind_name(value: GenericParameterKind) -> &'static str {
    match value {
        GenericParameterKind::Lifetime => "lifetime",
        GenericParameterKind::Type => "type",
        GenericParameterKind::Const => "const",
    }
}

fn type_kind_name(value: TypeRecordKind) -> &'static str {
    match value {
        TypeRecordKind::External => "external",
        TypeRecordKind::Struct => "struct",
        TypeRecordKind::Enum => "enum",
        TypeRecordKind::Union => "union",
        TypeRecordKind::TypeAlias => "type_alias",
        TypeRecordKind::Trait => "trait",
        TypeRecordKind::AssociatedType => "associated_type",
        TypeRecordKind::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &[u8] = include_bytes!(
        "../../crates/semaprax-rust-api-index/fixtures/protocol-envelope-example.json"
    );
    const LOCAL_EXTRACTOR: &[u8] = include_bytes!(
        "../../crates/semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
    );

    #[test]
    fn prepared_protocol_example_replays_deterministically_and_binds_identity() {
        let first = RustApiIndex::replay(EXAMPLE).unwrap();
        let second = RustApiIndex::replay(EXAMPLE).unwrap();
        assert_eq!(first.canonical_json().as_bytes(), EXAMPLE);
        assert_eq!(first.digest(), second.digest());
        assert!(first.items().iter().any(|item| item.path
            == "local_api_fixture::ReExported::contains"
            && item.support == Support::Supported));
        assert!(first.items().iter().any(|item| matches!(
            item.support,
            Support::Rejected {
                reason: RejectionReason::UnsupportedGeneric
            }
        )));
        first
            .require_identity(
                &first.package().source_sha256,
                first.target(),
                first.feature_digest(),
            )
            .unwrap();
        first
            .require_package_identity(
                "local_api_fixture",
                "0.0.0",
                "sha256:dbc31a9272b4e500ca6363d633d8c7b5dac727ce7279cc65391cff5f377010dd",
                "aarch64-apple-darwin",
                first.feature_digest(),
            )
            .unwrap();
        first
            .require_cargo_alias_identity("local_api_fixture")
            .unwrap();
        first
            .require_stable_compiler_identity(&first.stable_rustc_version())
            .unwrap();
        assert_eq!(
            first.require_stable_compiler_identity("rustc 1.97.1"),
            Err(IndexError::IdentityMismatch)
        );
        assert_eq!(
            first
                .select_supported(&["local_api_fixture::ReExported::contains"])
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            first.select_supported(&["local_api_fixture::generic_output"]),
            Err(IndexError::ItemUnavailable)
        );
        assert_eq!(
            first.select_supported(&[
                "local_api_fixture::ReExported::contains",
                "local_api_fixture::opaque_output"
            ]),
            Err(IndexError::ItemUnavailable)
        );
    }

    #[test]
    fn nightly_local_fixture_is_bounded_and_rejects_unsupported_closures() {
        let index = RustApiIndex::admit_extractor_output(LOCAL_EXTRACTOR).unwrap();
        assert_eq!(index.items().len(), 8);
        assert_eq!(index.types().len(), 9);
        assert!(index.items().iter().any(|item| {
            item.path == "local_api_fixture::MacroGenerated::answer"
                && item.support == Support::Supported
        }));
        assert!(index.items().iter().any(|item| {
            item.path == "local_api_fixture::ReExported::contains"
                && item.span.as_ref().is_some_and(|span| span.start_line == 7)
        }));
        assert!(index.items().iter().any(|item| {
            item.path == "local_api_fixture::cfg_selected" && item.support == Support::Supported
        }));
        assert!(index.items().iter().any(|item| {
            item.path == "local_api_fixture::SealedApi::HiddenOutput"
                && item.support
                    == Support::Rejected {
                        reason: RejectionReason::SealedTrait,
                    }
                && !item.closure_complete
        }));
        for path in [
            "local_api_fixture::generic_output",
            "local_api_fixture::opaque_output",
            "local_api_fixture::Measures::Output",
        ] {
            assert_eq!(
                index.select_supported(&[path]),
                Err(IndexError::ItemUnavailable)
            );
        }
        index
            .require_stable_compiler_identity(&index.stable_rustc_version())
            .unwrap();
        assert_eq!(
            index.require_stable_compiler_identity("rustc 1.97.1"),
            Err(IndexError::IdentityMismatch)
        );
    }

    #[test]
    fn malformed_truncated_and_drifted_indexes_refuse_before_use() {
        assert_eq!(
            RustApiIndex::replay(&EXAMPLE[..EXAMPLE.len() - 1]),
            Err(IndexError::Malformed)
        );
        let index = RustApiIndex::replay(EXAMPLE).unwrap();
        assert_eq!(
            index.require_identity(
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                index.target(),
                index.feature_digest()
            ),
            Err(IndexError::IdentityMismatch)
        );
        assert_eq!(
            index.require_identity(
                &index.package().source_sha256,
                "x86_64-unknown-linux-gnu",
                index.feature_digest()
            ),
            Err(IndexError::IdentityMismatch),
            "a stale target must be rejected"
        );
        assert_eq!(
            index.require_identity(
                &index.package().source_sha256,
                index.target(),
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            ),
            Err(IndexError::IdentityMismatch),
            "stale features must be rejected"
        );
        assert_eq!(
            RustApiIndex::admit_extractor_output(b"{\"schema\":\"missing\",\"index\":{}}\n"),
            Err(IndexError::SetupRequired)
        );
        assert!(!EXTRACTOR_SETUP_INSTRUCTION.is_empty());
    }

    #[test]
    fn extractor_output_requires_explicit_nightly_mode() {
        let index = std::str::from_utf8(EXAMPLE).unwrap().trim_end();
        let wrapped = format!("{{\"schema\":\"{RUSTDOC_EXTRACTOR_SCHEMA}\",\"index\":{index}}}\n");
        assert_eq!(
            RustApiIndex::admit_extractor_output(wrapped.as_bytes()),
            Err(IndexError::Malformed)
        );
    }

    #[test]
    fn item_and_depth_limits_fail_closed() {
        let mut bytes = EXAMPLE.to_vec();
        let needle = b"\"type_depth\":1";
        let offset = bytes
            .windows(needle.len())
            .position(|window| window == needle)
            .unwrap();
        bytes.splice(
            offset..offset + needle.len(),
            b"\"type_depth\":33".iter().copied(),
        );
        assert_eq!(RustApiIndex::replay(&bytes), Err(IndexError::Malformed));
        let uppercase_digest = std::str::from_utf8(EXAMPLE)
            .unwrap()
            .replace("dbc31a", "Dbc31a");
        assert_eq!(
            RustApiIndex::replay(uppercase_digest.as_bytes()),
            Err(IndexError::Malformed)
        );
    }

    #[test]
    fn item_count_and_document_byte_limits_fail_closed() {
        let mut value: Value = serde_json::from_slice(EXAMPLE).unwrap();
        let items = value.get_mut("items").unwrap().as_array_mut().unwrap();
        let template = items[0].clone();
        items.resize(MAX_ITEMS + 1, template);
        let mut too_many = serde_json::to_vec(&value).unwrap();
        too_many.push(b'\n');
        assert_eq!(RustApiIndex::replay(&too_many), Err(IndexError::Malformed));

        let mut value: Value = serde_json::from_slice(EXAMPLE).unwrap();
        let types = value.get_mut("types").unwrap().as_array_mut().unwrap();
        let template = types[0].clone();
        types.resize(MAX_TYPES + 1, template);
        let mut too_many_types = serde_json::to_vec(&value).unwrap();
        too_many_types.push(b'\n');
        assert_eq!(
            RustApiIndex::replay(&too_many_types),
            Err(IndexError::Malformed)
        );

        let mut value: Value = serde_json::from_slice(EXAMPLE).unwrap();
        value["items"][0]["docs"] = Value::String("x".repeat(MAX_DOC_BYTES + 1));
        let mut oversized_docs = serde_json::to_vec(&value).unwrap();
        oversized_docs.push(b'\n');
        assert_eq!(
            RustApiIndex::replay(&oversized_docs),
            Err(IndexError::Malformed)
        );

        let oversized = vec![b' '; MAX_INDEX_BYTES + 1];
        assert_eq!(RustApiIndex::replay(&oversized), Err(IndexError::Malformed));
    }

    #[test]
    fn reachable_type_cycles_are_visited_once_and_remain_canonical() {
        let mut value: Value = serde_json::from_slice(EXAMPLE).unwrap();
        let path = "local_api_fixture::MacroGenerated";
        let record = value["types"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|record| record["path"] == path)
            .unwrap();
        record["references"] = serde_json::json!([path]);
        let bytes = canonical_bytes(&value);
        let index = RustApiIndex::replay(&bytes).unwrap();
        let item = index
            .select_supported(&["local_api_fixture::MacroGenerated::answer"])
            .unwrap()[0];
        assert_eq!(item.reachable_types, [path]);
        assert_eq!(item.type_closure_depth, 1);
        assert!(item.closure_complete);
    }

    #[test]
    fn reachable_type_depth_and_demand_expansion_limits_fail_closed() {
        let mut deep: Value = serde_json::from_slice(EXAMPLE).unwrap();
        let paths = (0..=MAX_TYPE_DEPTH)
            .map(|depth| format!("zz_fixture::Deep{depth:02}"))
            .collect::<Vec<_>>();
        let mut deep_records = paths
            .iter()
            .enumerate()
            .map(|(position, path)| {
                let references = paths
                    .get(position + 1)
                    .cloned()
                    .into_iter()
                    .collect::<Vec<_>>();
                type_record(path.clone(), references)
            })
            .collect::<Vec<_>>();
        append_type_records(&mut deep, &mut deep_records);
        point_item_at_closure(
            &mut deep,
            "local_api_fixture::ReExported::contains",
            &paths[0],
            &paths,
            MAX_TYPE_DEPTH,
        );
        assert_eq!(
            RustApiIndex::replay(&canonical_bytes(&deep)),
            Err(IndexError::Malformed),
            "a path whose reachable chain exceeds MAX_TYPE_DEPTH must fail"
        );

        let expansion_case = |leaf_count: usize| {
            let mut value: Value = serde_json::from_slice(EXAMPLE).unwrap();
            let root = "zz_fixture::ExpansionRoot".to_owned();
            let leaves = (0..leaf_count)
                .map(|index| format!("zz_fixture::Expansion{index:03}"))
                .collect::<Vec<_>>();
            let mut paths = vec![root.clone()];
            paths.extend(leaves.iter().cloned());
            paths.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            let mut records = vec![type_record(root.clone(), leaves)];
            records.extend(
                paths
                    .iter()
                    .filter(|path| **path != root)
                    .map(|path| type_record(path.clone(), Vec::new())),
            );
            append_type_records(&mut value, &mut records);
            point_item_at_closure(
                &mut value,
                "local_api_fixture::ReExported::contains",
                &root,
                &paths,
                2,
            );
            value
        };
        let at_limit = expansion_case(MAX_TYPE_REFERENCES - 1);
        assert_eq!(
            RustApiIndex::replay(&canonical_bytes(&at_limit))
                .unwrap()
                .select_supported(&["local_api_fixture::ReExported::contains"])
                .unwrap()[0]
                .reachable_types
                .len(),
            MAX_TYPE_REFERENCES
        );
        let over_limit = expansion_case(MAX_TYPE_REFERENCES);
        assert_eq!(
            RustApiIndex::replay(&canonical_bytes(&over_limit)),
            Err(IndexError::Malformed),
            "demand expansion beyond MAX_TYPE_REFERENCES must fail"
        );
    }

    fn type_record(path: String, references: Vec<String>) -> Value {
        serde_json::json!({
            "path": path,
            "kind": "struct",
            "visibility": "public",
            "docs": null,
            "span": null,
            "generics": { "parameters": [], "where_predicates": [] },
            "references": references,
        })
    }

    fn append_type_records(value: &mut Value, records: &mut Vec<Value>) {
        let types = value["types"].as_array_mut().unwrap();
        types.append(records);
        types.sort_by(|left, right| {
            left["path"]
                .as_str()
                .unwrap()
                .as_bytes()
                .cmp(right["path"].as_str().unwrap().as_bytes())
        });
    }

    fn point_item_at_closure(
        value: &mut Value,
        item_path: &str,
        root: &str,
        reachable: &[String],
        depth: usize,
    ) {
        let item = value["items"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|item| item["path"] == item_path)
            .unwrap();
        item["type_roots"] = serde_json::json!([root]);
        item["reachable_types"] = serde_json::json!(reachable);
        item["type_closure_depth"] = serde_json::json!(depth);
        item["closure_complete"] = serde_json::json!(true);
    }

    fn canonical_bytes(value: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn selected_fixture_signatures_are_checked_by_rustc_and_mismatch_is_rejected() {
        use std::process::Command;

        let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            if env!("CARGO_PKG_NAME") == "semaprax" {
                "crates/semaprax-rust-api-index/fixtures"
            } else {
                "fixtures"
            },
        );
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compiler_version = Command::new(&rustc)
            .arg("--version")
            .output()
            .expect("read selected stable Rust compiler version");
        let version_text = String::from_utf8_lossy(&compiler_version.stdout);
        let release = version_text.split_whitespace().nth(1).unwrap_or_default();
        assert!(
            compiler_version.status.success() && !release.contains('-'),
            "fixture signature check requires a stable rustc, got {version_text}"
        );
        RustApiIndex::replay(EXAMPLE)
            .unwrap()
            .require_stable_compiler_identity(version_text.trim())
            .expect("prepared signatures must match the selected stable compiler exactly");
        for (source, should_succeed) in [
            ("stable_signature_check.rs", true),
            ("stable_signature_mismatch.rs", false),
        ] {
            let output_path = std::env::temp_dir().join(format!(
                "semaprax-ri03-{}-{}.rmeta",
                std::process::id(),
                source.replace('.', "_")
            ));
            let output = Command::new(&rustc)
                .arg("--edition=2021")
                .arg("--crate-type=lib")
                .arg("--crate-name=ri03_stable_fixture")
                .arg(fixture_dir.join(source))
                .arg("--cfg")
                .arg("feature=\"fixture-selected\"")
                .arg("--emit=metadata")
                .arg("-o")
                .arg(&output_path)
                .output()
                .expect("launch selected stable Rust compiler");
            let _ = std::fs::remove_file(&output_path);
            assert_eq!(
                output.status.success(),
                should_succeed,
                "{source}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn pinned_regex_rustdoc_envelope_replays_and_selected_signatures_compile() {
        const SOURCE: &str =
            "sha256:f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d";
        const FEATURES: &str =
            "sha256:dcacb5b38acb8b53818ae1c0cb2020947aefbea8ac5a9380e49aa0e0ec4db1aa";
        let bytes = include_bytes!(
            "../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
        );
        let index = RustApiIndex::admit_extractor_output(bytes).unwrap();
        assert_eq!(index.package().name, "regex");
        assert_eq!(index.package().version, "1.13.1");
        assert_eq!(index.package().source_sha256, SOURCE);
        assert_eq!(index.target(), "aarch64-apple-darwin");
        assert_eq!(index.feature_digest(), FEATURES);
        assert_eq!(index.extractor().rustdoc_format, "rustdoc-json:61");
        index
            .require_package_identity("regex", "1.13.1", SOURCE, "aarch64-apple-darwin", FEATURES)
            .unwrap();
        index.require_cargo_alias_identity("regex_alias").unwrap();
        assert_eq!(
            index.require_cargo_alias_identity("regex"),
            Err(IndexError::IdentityMismatch)
        );
        index
            .require_stable_compiler_identity(&index.stable_rustc_version())
            .unwrap();
        assert_eq!(
            index.require_stable_compiler_identity("rustc 1.97.1"),
            Err(IndexError::IdentityMismatch)
        );
        let selected = index
            .select_supported(&["regex::Regex::is_match", "regex::Regex::is_match_at"])
            .unwrap();
        assert_eq!(selected.len(), 2);
        assert!(selected
            .iter()
            .all(|item| item.kind == ItemKind::InherentMethod));
        assert!(selected
            .iter()
            .all(|item| item.support == Support::Supported));

        for (path, reason) in [
            (
                "regex::Regex::captures",
                RejectionReason::IncompleteTypeClosure,
            ),
            ("regex::Regex::find", RejectionReason::IncompleteTypeClosure),
            ("regex::Regex::new", RejectionReason::IncompleteTypeClosure),
        ] {
            let item = index.items().iter().find(|item| item.path == path).unwrap();
            assert_eq!(item.support, Support::Rejected { reason });
            assert!(!item.closure_complete);
            assert_eq!(
                index.select_supported(&[path]),
                Err(IndexError::ItemUnavailable)
            );
        }

        let constructor = index
            .select_closed_owner_result("regex::Regex::new", "regex::Regex", "regex::Error")
            .unwrap();
        assert_eq!(
            constructor.signature,
            "fn new(re: &str) -> core::result::Result<regex::Regex, regex::Error>"
        );
        for (path, owner, error) in [
            ("regex::Regex::new", "regex::Regex", "regex::MissingError"),
            ("regex::Regex::is_match", "regex::Regex", "regex::Error"),
            ("regex::Regex::new", "regex::Missing", "regex::Error"),
        ] {
            assert_eq!(
                index.select_closed_owner_result(path, owner, error),
                Err(IndexError::ItemUnavailable)
            );
        }

        let _: fn(&regex_alias::Regex, &str) -> bool = regex_alias::Regex::is_match;
        let _: fn(&regex_alias::Regex, &str, usize) -> bool = regex_alias::Regex::is_match_at;
        let _: for<'h> fn(&regex_alias::Regex, &'h str) -> Option<regex_alias::Captures<'h>> =
            regex_alias::Regex::captures;
        let _: for<'h> fn(&regex_alias::Regex, &'h str) -> Option<regex_alias::Match<'h>> =
            regex_alias::Regex::find;
        let _: fn(&str) -> Result<regex_alias::Regex, regex_alias::Error> = regex_alias::Regex::new;
    }
}
