//! Deterministic, read-only UI Dialect Schema Projection v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.ui-dialect-schema.v1`)
//! describing its typed application schema: every admitted public non-generic
//! scalar-field record becomes one state-shape descriptor whose names,
//! types, offsets, sizes, and alignments come exclusively from the checked
//! Native64 compiler layouts, and every admitted explicit-ID monomorphic
//! by-value effect-free scalar function becomes one typed action descriptor
//! with its parameter/result types. An explicit empty-by-default UI section
//! (`controls`, `accessibility`, `navigation`) is always present as a
//! reserved nonclaim field. This is a schema PROJECTION only: no rendering,
//! no runtime, no DOM.
//!
//! Function admission mirrors Canonical ABI Report v1 exactly. Record
//! admission excludes automatic-identity, generic, resource, variant, and
//! mixed-field-type declarations with dedicated closed reasons.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope
//! shape, declared byte count, domain-separated payload digest, and equality
//! of every embedded state-shape layout digest and action signature digest
//! with its derivation from the listed values.
//!
//! Diagnostics use the previously unused `SPX-U1xx` family:
//! - `SPX-U101`: invalid options (bounds, malformed values).
//! - `SPX-U102`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-U103`: envelope consistency or replay failure.
//!
//! This tranche adds no typed update/view language constructs, no semantic
//! controls, no accessibility, navigation, localization, assets, platform
//! blocks, or custom rendering, executes nothing, and changes no source.

use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::aggregate_layout::{self, AggregateTarget};
use crate::ast::{Function, ParamMode, Type, TypeDeclaration, TypeDeclarationKind};
use crate::bounded_output::{budgeted_clone, with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{self, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind};
use crate::{graph, parse, patch, verify};

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.ui-dialect-schema.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.payload.v1\0";
const STATE_SHAPE_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.state-shape.v1\0";
const ACTION_SIGNATURE_DIGEST_DOMAIN: &[u8] = b"semaprax.ui-dialect-schema.action-signature.v1\0";

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";

const RECORD_REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const RECORD_REASON_GENERIC_TYPE: &str = "generic_type";
const RECORD_REASON_RESOURCE_TYPE: &str = "resource_type";
const RECORD_REASON_VARIANT_TYPE: &str = "variant_type";
const RECORD_REASON_MIXED_FIELD_TYPES: &str = "mixed_field_types";

const KIND_RECORD: &str = "record";
const KIND_FUNCTION: &str = "function";

const NONCLAIMS_JSON: &str = "\"schema_projection_only\",\
\"no_typed_update_or_view_language_constructs\",\
\"no_semantic_controls\",\
\"no_accessibility\",\
\"no_navigation\",\
\"no_localization\",\
\"no_assets\",\
\"no_platform_blocks\",\
\"no_custom_rendering\",\
\"no_target_execution\",\
\"read_only_no_source_changes\"";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiSchemaOptions {
    pub max_bytes: usize,
}

impl UiSchemaOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "ui-schema max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for UiSchemaOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-U101", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-U103", message)
}

struct StateShapeField {
    name: String,
    ty: &'static str,
    offset: u32,
    size_bytes: u32,
    align_bytes: u32,
}

struct StateShape {
    stable_id: String,
    name: String,
    size_bytes: u32,
    align_bytes: u32,
    fields: Vec<StateShapeField>,
}

struct ActionParameter {
    name: String,
    ty: &'static str,
}

struct Action {
    stable_id: String,
    name: String,
    parameters: Vec<ActionParameter>,
    result_ty: &'static str,
}

struct ExcludedEntry {
    kind: &'static str,
    stable_id: String,
    name: String,
    reason: &'static str,
}

/// One independently authenticated state shape returned by
/// [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStateShapeField {
    pub index: u32,
    pub name: String,
    pub ty: String,
    pub offset: u32,
    pub size_bytes: u32,
    pub align_bytes: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedStateShape {
    pub stable_id: String,
    pub name: String,
    pub size_bytes: u32,
    pub align_bytes: u32,
    pub fields: Vec<VerifiedStateShapeField>,
}

/// One independently authenticated action descriptor returned by
/// [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedAction {
    pub stable_id: String,
    pub name: String,
    pub parameters: Vec<(String, String)>,
    pub result_ty: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedUiSchema {
    pub state_shapes: Vec<VerifiedStateShape>,
    pub actions: Vec<VerifiedAction>,
}

/// Generate the canonical `semaprax.ui-dialect-schema.v1` envelope JSON for
/// one verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(source_path: &Path, options: &UiSchemaOptions) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let mut excluded: Vec<ExcludedEntry> = Vec::new();
    let mut admitted_records = Vec::new();
    for declaration in &program.types {
        if let Some(reason) = record_admission(declaration) {
            excluded.push(ExcludedEntry {
                kind: KIND_RECORD,
                stable_id: declaration.stable_id.clone(),
                name: declaration.name.clone(),
                reason,
            });
        } else {
            admitted_records.push(declaration.stable_id.clone());
        }
    }
    let mut actions = Vec::new();
    for function in &program.functions {
        match function_admission(function) {
            Some(reason) => excluded.push(ExcludedEntry {
                kind: KIND_FUNCTION,
                stable_id: function.stable_id.clone(),
                name: function.name.clone(),
                reason,
            }),
            None => actions.push(Action {
                stable_id: function.stable_id.clone(),
                name: function.name.clone(),
                parameters: function
                    .params
                    .iter()
                    .map(|param| ActionParameter {
                        name: param.name.clone(),
                        ty: ast_scalar_type_name(&param.ty).expect("admitted scalar parameter"),
                    })
                    .collect(),
                result_ty: ast_scalar_type_name(&function.return_type)
                    .expect("admitted scalar result"),
            }),
        }
    }

    // State-shape offsets, sizes, and alignments come exclusively from the
    // checked compiler layouts, so projection resolves the real HIR.
    let mut state_shapes = Vec::with_capacity(admitted_records.len());
    if !admitted_records.is_empty() {
        let resolved = hir::resolve(&program)?;
        for stable_id in &admitted_records {
            state_shapes.push(project_state_shape(&resolved, stable_id)?);
        }
    }
    state_shapes.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    actions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    excluded.sort_by(|left, right| {
        left.kind
            .as_bytes()
            .cmp(right.kind.as_bytes())
            .then(left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()))
    });

    let input = SchemaInput {
        module_name: program.module.clone(),
        records_total: program.types.len(),
        functions_total: program.functions.len(),
        state_shapes,
        actions,
        excluded,
    };
    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();

    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(&path_text, &revision, &digest, &input, options.max_bytes)
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-U102",
            "ui-schema output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

struct SchemaInput {
    module_name: String,
    records_total: usize,
    functions_total: usize,
    state_shapes: Vec<StateShape>,
    actions: Vec<Action>,
    excluded: Vec<ExcludedEntry>,
}

/// Closed AST-level admission gate for records: only public non-generic
/// records whose fields are all direct `i64`/`bool` scalars are admitted;
/// every other declaration gets exactly one closed exclusion reason.
fn record_admission(declaration: &TypeDeclaration) -> Option<&'static str> {
    if !declaration.explicit_id {
        return Some(RECORD_REASON_AUTOMATIC_IDENTITY);
    }
    if !declaration.type_parameters.is_empty() {
        return Some(RECORD_REASON_GENERIC_TYPE);
    }
    match &declaration.kind {
        TypeDeclarationKind::Resource { .. } => Some(RECORD_REASON_RESOURCE_TYPE),
        TypeDeclarationKind::Variant { .. } => Some(RECORD_REASON_VARIANT_TYPE),
        TypeDeclarationKind::Record { fields } => {
            if fields
                .iter()
                .any(|field| !matches!(field.ty, Type::I64 | Type::Bool))
            {
                Some(RECORD_REASON_MIXED_FIELD_TYPES)
            } else {
                None
            }
        }
    }
}

fn ast_scalar_type_name(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::I64 => Some("i64"),
        Type::Bool => Some("bool"),
        _ => None,
    }
}

/// Closed AST-level admission gate mirroring the Canonical ABI Report v1
/// profile exactly: explicit identity, monomorphic, effect-free, by-value
/// direct `i64`/`bool` parameters and result.
fn function_admission(function: &Function) -> Option<&'static str> {
    if !function.explicit_id {
        return Some(REASON_AUTOMATIC_IDENTITY);
    }
    if !function.type_parameters.is_empty() {
        return Some(REASON_GENERIC_FUNCTION);
    }
    if !function.effects.is_empty() {
        return Some(REASON_DECLARED_EFFECTS);
    }
    for param in &function.params {
        if param.mode != ParamMode::Value {
            return Some(REASON_UNSUPPORTED_PARAMETER_MODE);
        }
        if !matches!(param.ty, Type::I64 | Type::Bool) {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if !matches!(function.return_type, Type::I64 | Type::Bool) {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
}

fn project_state_shape(
    resolved: &ResolvedProgram,
    stable_id: &str,
) -> Result<StateShape, Vec<Diagnostic>> {
    let declaration = resolved
        .types
        .iter()
        .find(|candidate| candidate.id.as_str() == stable_id)
        .ok_or_else(|| {
            vec![consistency_error(format!(
                "admitted record `{stable_id}` is absent from resolved HIR"
            ))]
        })?;
    let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind else {
        return Err(vec![consistency_error(format!(
            "admitted record `{stable_id}` is not a record in resolved HIR"
        ))]);
    };
    let layout = aggregate_layout::AggregateLayout::for_record(
        resolved,
        AggregateTarget::Native64,
        &declaration.id,
    )
    .map_err(|error| vec![error])?;
    let mut projected = Vec::with_capacity(fields.len());
    for field in fields {
        let facts = layout.field(&field.id).ok_or_else(|| {
            vec![consistency_error(format!(
                "checked layout has no entry for field `{}` of record `{stable_id}`",
                field.name
            ))]
        })?;
        projected.push(StateShapeField {
            name: field.name.clone(),
            ty: resolved_scalar_type_name(&field.ty)?,
            offset: facts.offset,
            size_bytes: facts.size,
            align_bytes: facts.align,
        });
    }
    Ok(StateShape {
        stable_id: stable_id.to_owned(),
        name: declaration.name.clone(),
        size_bytes: layout.size,
        align_bytes: layout.align,
        fields: projected,
    })
}

fn resolved_scalar_type_name(ty: &ResolvedType) -> Result<&'static str, Vec<Diagnostic>> {
    match ty {
        ResolvedType::I64 => Ok("i64"),
        ResolvedType::Bool => Ok("bool"),
        other => Err(vec![consistency_error(format!(
            "type `{}` is outside the admitted scalar profile",
            other.identity_key()
        ))]),
    }
}

/// The exact canonical layout-object bytes whose domain-separated digest is
/// embedded beside each state shape; the verifier rebuilds these bytes from
/// the parsed payload values before recomputing the digest.
fn state_shape_layout_text(
    fields: &[(&str, &str, u32, u32, u32)],
    size_bytes: u32,
    align_bytes: u32,
) -> String {
    let entries = fields
        .iter()
        .enumerate()
        .map(|(index, (name, ty, offset, field_size, field_align))| {
            bformat!(
                "{{\"index\":{},\"name\":{},\"type\":{},\"offset\":{},\
\"size_bytes\":{},\"align_bytes\":{}}}",
                index,
                quote_json(name),
                quote_json(ty),
                offset,
                field_size,
                field_align,
            )
        })
        .collect::<Vec<_>>();
    bformat!(
        "{{\"fields\":[{}],\"size_bytes\":{},\"align_bytes\":{}}}",
        entries.budgeted_join(","),
        size_bytes,
        align_bytes,
    )
}

/// The exact canonical signature-object bytes whose domain-separated digest
/// is embedded beside each action descriptor; the verifier rebuilds these
/// bytes from the parsed payload values before recomputing the digest.
fn action_signature_text(parameters: &[(&str, &str)], result_ty: &str) -> String {
    let entries = parameters
        .iter()
        .map(|(name, ty)| {
            bformat!(
                "{{\"name\":{},\"type\":{}}}",
                quote_json(name),
                quote_json(ty)
            )
        })
        .collect::<Vec<_>>();
    bformat!(
        "{{\"parameters\":[{}],\"result\":{{\"type\":{}}}}}",
        entries.budgeted_join(","),
        quote_json(result_ty),
    )
}

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
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

fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    input: &SchemaInput,
    max_bytes: usize,
) -> String {
    let state_shape_entries = input
        .state_shapes
        .iter()
        .map(|shape| {
            let field_refs = shape
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.as_str(),
                        field.ty,
                        field.offset,
                        field.size_bytes,
                        field.align_bytes,
                    )
                })
                .collect::<Vec<_>>();
            let layout = state_shape_layout_text(&field_refs, shape.size_bytes, shape.align_bytes);
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"kind\":\"{}\",\"layout\":{},\
\"layout_sha256\":{}}}",
                quote_json(&shape.stable_id),
                quote_json(&shape.name),
                KIND_RECORD,
                budgeted_clone(&layout),
                quote_json(&domain_digest(STATE_SHAPE_DIGEST_DOMAIN, layout.as_bytes(),)),
            )
        })
        .collect::<Vec<_>>();
    let action_entries = input
        .actions
        .iter()
        .map(|action| {
            let parameter_refs = action
                .parameters
                .iter()
                .map(|parameter| (parameter.name.as_str(), parameter.ty))
                .collect::<Vec<_>>();
            let signature = action_signature_text(&parameter_refs, action.result_ty);
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"kind\":\"{}\",\"role\":\"action\",\
\"signature\":{},\"signature_sha256\":{}}}",
                quote_json(&action.stable_id),
                quote_json(&action.name),
                KIND_FUNCTION,
                budgeted_clone(&signature),
                quote_json(&domain_digest(
                    ACTION_SIGNATURE_DIGEST_DOMAIN,
                    signature.as_bytes(),
                )),
            )
        })
        .collect::<Vec<_>>();
    let exclusion_entries = input
        .excluded
        .iter()
        .map(|entry| {
            bformat!(
                "{{\"kind\":\"{}\",\"stable_id\":{},\"name\":{},\"reason\":\"{}\"}}",
                entry.kind,
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                entry.reason,
            )
        })
        .collect::<Vec<_>>();

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"module\":{{\"name\":{},\"records_total\":{},\"functions_total\":{}}},\
\"inventory\":{{\"state_shapes_admitted\":{},\"actions_admitted\":{},\"excluded\":{}}},\
\"state_shapes\":[{}],\"actions\":[{}],\"exclusions\":[{}],\
\"controls\":[],\"accessibility\":[],\"navigation\":[],\
\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(&input.module_name),
        input.records_total,
        input.functions_total,
        input.state_shapes.len(),
        input.actions.len(),
        input.excluded.len(),
        state_shape_entries.budgeted_join(","),
        action_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        budgeted_clone(&payload),
    )
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, requires the reserved UI
/// sections to be present and empty, and re-authenticates every embedded
/// state-shape layout digest and action signature digest by rebuilding the
/// canonical bytes from the parsed values before returning the descriptors.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedUiSchema, Diagnostic> {
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(format!("envelope is not valid JSON: {error}")))?;
    let Some(object) = value.as_object() else {
        return Err(consistency_error(
            "envelope must be a JSON object".to_owned(),
        ));
    };
    let keys: Vec<&str> = object.keys().map(String::as_str).collect();
    if keys != ["bytes", "digest", "payload", "schema"] {
        return Err(consistency_error(format!(
            "envelope keys must be exactly [bytes, digest, payload, schema], found {keys:?}"
        )));
    }
    if object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "envelope schema must be {SCHEMA}"
        )));
    }
    let Some(envelope_digest) = object["digest"].as_str() else {
        return Err(consistency_error(
            "envelope digest must be a string".to_owned(),
        ));
    };
    let Some(declared_bytes) = object["bytes"].as_u64() else {
        return Err(consistency_error(
            "envelope bytes must be an unsigned integer".to_owned(),
        ));
    };
    const PAYLOAD_KEY: &str = "\"payload\":";
    let Some(offset) = envelope.find(PAYLOAD_KEY) else {
        return Err(consistency_error(
            "envelope is missing its payload member".to_owned(),
        ));
    };
    if !envelope.ends_with('}') {
        return Err(consistency_error("envelope must end with `}`".to_owned()));
    }
    let payload = &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "envelope payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "envelope declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    let recomputed = domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes());
    if envelope_digest != recomputed {
        return Err(consistency_error(
            "envelope digest does not match the exact payload bytes".to_owned(),
        ));
    }
    let payload_value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    if payload_value["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {SCHEMA}"
        )));
    }
    for section in ["controls", "accessibility", "navigation"] {
        let empty: Vec<serde_json::Value> = Vec::new();
        if payload_value[section].as_array() != Some(&empty) {
            return Err(consistency_error(format!(
                "payload {section} section must be an empty array in this schema version"
            )));
        }
    }

    let Some(shapes) = payload_value["state_shapes"].as_array() else {
        return Err(consistency_error(
            "payload state_shapes must be an array".to_owned(),
        ));
    };
    let mut verified_shapes = Vec::with_capacity(shapes.len());
    for shape in shapes {
        let Some(stable_id) = shape["stable_id"].as_str() else {
            return Err(consistency_error(
                "state-shape stable_id must be a string".to_owned(),
            ));
        };
        let Some(name) = shape["name"].as_str() else {
            return Err(consistency_error(
                "state-shape name must be a string".to_owned(),
            ));
        };
        let Some(size_bytes) = shape["layout"]["size_bytes"].as_u64() else {
            return Err(consistency_error(
                "state-shape layout size_bytes must be an unsigned integer".to_owned(),
            ));
        };
        let Some(align_bytes) = shape["layout"]["align_bytes"].as_u64() else {
            return Err(consistency_error(
                "state-shape layout align_bytes must be an unsigned integer".to_owned(),
            ));
        };
        let Some(fields) = shape["layout"]["fields"].as_array() else {
            return Err(consistency_error(
                "state-shape layout fields must be an array".to_owned(),
            ));
        };
        let mut field_refs = Vec::with_capacity(fields.len());
        let mut verified_fields = Vec::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            let Some(field_name) = field["name"].as_str() else {
                return Err(consistency_error(
                    "state-shape field name must be a string".to_owned(),
                ));
            };
            let Some(field_ty) = field["type"].as_str() else {
                return Err(consistency_error(
                    "state-shape field type must be a string".to_owned(),
                ));
            };
            let Some(field_offset) = field["offset"].as_u64() else {
                return Err(consistency_error(
                    "state-shape field offset must be an unsigned integer".to_owned(),
                ));
            };
            let Some(field_size) = field["size_bytes"].as_u64() else {
                return Err(consistency_error(
                    "state-shape field size_bytes must be an unsigned integer".to_owned(),
                ));
            };
            let Some(field_align) = field["align_bytes"].as_u64() else {
                return Err(consistency_error(
                    "state-shape field align_bytes must be an unsigned integer".to_owned(),
                ));
            };
            field_refs.push((
                field_name,
                field_ty,
                field_offset as u32,
                field_size as u32,
                field_align as u32,
            ));
            verified_fields.push(VerifiedStateShapeField {
                index: index as u32,
                name: field_name.to_owned(),
                ty: field_ty.to_owned(),
                offset: field_offset as u32,
                size_bytes: field_size as u32,
                align_bytes: field_align as u32,
            });
        }
        let rebuilt = state_shape_layout_text(&field_refs, size_bytes as u32, align_bytes as u32);
        let declared_digest = shape["layout_sha256"].as_str().ok_or_else(|| {
            consistency_error("state-shape layout_sha256 must be a string".to_owned())
        })?;
        if declared_digest != domain_digest(STATE_SHAPE_DIGEST_DOMAIN, rebuilt.as_bytes()) {
            return Err(consistency_error(
                "embedded state-shape layout digest does not match the listed layout values"
                    .to_owned(),
            ));
        }
        verified_shapes.push(VerifiedStateShape {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            size_bytes: size_bytes as u32,
            align_bytes: align_bytes as u32,
            fields: verified_fields,
        });
    }

    let Some(actions) = payload_value["actions"].as_array() else {
        return Err(consistency_error(
            "payload actions must be an array".to_owned(),
        ));
    };
    let mut verified_actions = Vec::with_capacity(actions.len());
    for action in actions {
        let Some(stable_id) = action["stable_id"].as_str() else {
            return Err(consistency_error(
                "action stable_id must be a string".to_owned(),
            ));
        };
        let Some(name) = action["name"].as_str() else {
            return Err(consistency_error("action name must be a string".to_owned()));
        };
        let Some(parameters) = action["signature"]["parameters"].as_array() else {
            return Err(consistency_error(
                "action signature parameters must be an array".to_owned(),
            ));
        };
        let Some(result_ty) = action["signature"]["result"]["type"].as_str() else {
            return Err(consistency_error(
                "action signature result type must be a string".to_owned(),
            ));
        };
        let mut parameter_refs = Vec::with_capacity(parameters.len());
        let mut verified_parameters = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let Some(parameter_name) = parameter["name"].as_str() else {
                return Err(consistency_error(
                    "action parameter name must be a string".to_owned(),
                ));
            };
            let Some(parameter_ty) = parameter["type"].as_str() else {
                return Err(consistency_error(
                    "action parameter type must be a string".to_owned(),
                ));
            };
            parameter_refs.push((parameter_name, parameter_ty));
            verified_parameters.push((parameter_name.to_owned(), parameter_ty.to_owned()));
        }
        let rebuilt = action_signature_text(&parameter_refs, result_ty);
        let declared_digest = action["signature_sha256"].as_str().ok_or_else(|| {
            consistency_error("action signature_sha256 must be a string".to_owned())
        })?;
        if declared_digest != domain_digest(ACTION_SIGNATURE_DIGEST_DOMAIN, rebuilt.as_bytes()) {
            return Err(consistency_error(
                "embedded action signature digest does not match the listed signature values"
                    .to_owned(),
            ));
        }
        verified_actions.push(VerifiedAction {
            stable_id: stable_id.to_owned(),
            name: name.to_owned(),
            parameters: verified_parameters,
            result_ty: result_ty.to_owned(),
        });
    }

    Ok(VerifiedUiSchema {
        state_shapes: verified_shapes,
        actions: verified_actions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_temp(source: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "semaprax-ui-schema-{}-{}.spx",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, source).unwrap();
        path
    }

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(UiSchemaOptions::new(512).is_err());
        assert!(UiSchemaOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
        assert!(UiSchemaOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
        assert_eq!(UiSchemaOptions::default().max_bytes, DEFAULT_MAX_BYTES);
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
        let second = domain_digest(PAYLOAD_DIGEST_DOMAIN, b"abc");
        assert_ne!(first, second);
        assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
    }

    #[test]
    fn canonical_texts_are_stable_and_verifier_friendly() {
        let fields = [("x", "i64", 0u32, 8u32, 8u32), ("flag", "bool", 8, 1, 1)];
        assert_eq!(
            state_shape_layout_text(&fields, 16, 8),
            "{\"fields\":[{\"index\":0,\"name\":\"x\",\"type\":\"i64\",\"offset\":0,\
\"size_bytes\":8,\"align_bytes\":8},{\"index\":1,\"name\":\"flag\",\"type\":\"bool\",\
\"offset\":8,\"size_bytes\":1,\"align_bytes\":1}],\"size_bytes\":16,\"align_bytes\":8}"
        );
        assert_eq!(
            action_signature_text(&[("left", "i64"), ("right", "i64")], "i64"),
            "{\"parameters\":[{\"name\":\"left\",\"type\":\"i64\"},\
{\"name\":\"right\",\"type\":\"i64\"}],\"result\":{\"type\":\"i64\"}}"
        );
    }

    /// State-shape facts must equal the checked Native64 compiler layouts,
    /// including the trailing-padding size of a mixed i64/bool record.
    #[test]
    fn state_shape_facts_come_from_the_checked_layouts() {
        let source = r#"
module test.shapes;

@id("shapes.point")
record Point {
    @id("shapes.point.x")
    x: i64,
    @id("shapes.point.y")
    y: i64,
    @id("shapes.point.flag")
    flag: bool,
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
        let path = write_temp(source);
        let text = std::fs::read_to_string(&path).unwrap();
        let program = parse(&text, &path).expect("parses");
        let resolved = hir::resolve(&program).expect("resolves");
        let shape = project_state_shape(&resolved, "shapes.point").expect("shape");

        let declaration = resolved
            .types
            .iter()
            .find(|candidate| candidate.id.as_str() == "shapes.point")
            .expect("resolved record");
        let checked = aggregate_layout::AggregateLayout::for_record(
            &resolved,
            AggregateTarget::Native64,
            &declaration.id,
        )
        .expect("checked layout");
        assert_eq!(
            (shape.size_bytes, shape.align_bytes),
            (checked.size, checked.align)
        );
        assert_eq!(shape.fields.len(), checked.fields.len());
        for (field, fact) in shape.fields.iter().zip(checked.fields.iter()) {
            assert_eq!(
                (field.offset, field.size_bytes, field.align_bytes),
                (fact.offset, fact.size, fact.align)
            );
        }
        assert_eq!(shape.name, "Point");
        assert_eq!(
            shape
                .fields
                .iter()
                .map(|field| field.ty)
                .collect::<Vec<_>>(),
            vec!["i64", "i64", "bool"]
        );
        // Mixed scalar padding: two i64 plus one bool pad to the record align.
        assert_eq!(shape.size_bytes, 24);
        assert_eq!(shape.align_bytes, 8);
        cleanup(&path);
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_file(path);
    }
}
