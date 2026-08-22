//! Deterministic, read-only OpenAPI Schema Generation v1 plus compatibility v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX source into one
//! canonical OpenAPI 3.1 document wrapped in a `semaprax.openapi.v1`
//! envelope. Only explicitly selected monomorphic effect-free functions with
//! direct by-value `i64`/`bool` parameters and results are admitted; every
//! other selection fails closed with a stable exclusion reason. The document
//! is serialized through `serde_json::Value` maps, whose canonical
//! sorted-key compact form makes the exact bytes replayable, and the envelope
//! carries a domain-separated SHA-256 digest over those exact payload bytes.
//!
//! [`compatibility`] reads exactly two previously generated envelopes,
//! independently re-authenticates their schemas and digests, classifies the
//! deterministic difference into ordered breaking/non-breaking/informational
//! findings, and reports a `semaprax.openapi-compat.v1` verdict.
//!
//! Migration rules (prose only): a `breaking` verdict requires a major
//! version bump before replacing any published document; a `compatible`
//! verdict permits at most a minor or patch bump; informational findings
//! require review but no version action. No external migration tooling is
//! claimed or invoked.
//!
//! Diagnostic codes allocated by this module (`SPX-OA1xx`, unused before):
//! - `SPX-OA101`: invalid generation options or selection set.
//! - `SPX-OA102`: a selection matches no function name or stable id.
//! - `SPX-OA103`: a selected function is excluded from the scalar profile.
//! - `SPX-OA104`: a compatibility input is not valid JSON or fails envelope
//!   authentication (foreign schema, malformed structure, or digest mismatch).
//! - `SPX-OA105`: the bounded output budget was exhausted; nothing is emitted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};

use crate::ast::{Function, ParamMode, Program, Type};
use crate::bounded_output::{budgeted_clone, with_limit};
use crate::diagnostic::Diagnostic;
use crate::{format, graph, parse, patch, verify};

pub const SCHEMA: &str = "semaprax.openapi.v1";
pub const COMPAT_SCHEMA: &str = "semaprax.openapi-compat.v1";

pub const MAX_FUNCTIONS: usize = 32;
const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const OPENAPI_VERSION: &str = "3.1.0";
const STATUS_COMPONENT_NAME: &str = "Semaprax.Status.v1";
const REQUEST_SUFFIX: &str = ".Request";
const RESULT_SUFFIX: &str = ".Result";

const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";

/// Stable finding codes reported by [`compatibility`]. These are report
/// payload codes, distinct from the `SPX-*` diagnostics above.
const FINDING_OPERATION_REMOVED: &str = "OAC-B001";
const FINDING_PARAMETER_REMOVED: &str = "OAC-B002";
const FINDING_PARAMETER_TYPE_CHANGED: &str = "OAC-B003";
const FINDING_REQUIRED_PARAMETER_ADDED: &str = "OAC-B004";
const FINDING_RESULT_TYPE_CHANGED: &str = "OAC-B005";
const FINDING_OPERATION_ADDED: &str = "OAC-N001";
const FINDING_OPERATION_DESCRIPTION_CHANGED: &str = "OAC-I001";
const FINDING_SOURCE_REVISION_CHANGED: &str = "OAC-I002";

const SEVERITY_BREAKING: &str = "breaking";
const SEVERITY_NON_BREAKING: &str = "non-breaking";
const SEVERITY_INFORMATIONAL: &str = "informational";

const VERDICT_BREAKING: &str = "breaking";
const VERDICT_COMPATIBLE: &str = "compatible";

const I64_DESCRIPTION: &str = "Signed 64-bit two's-complement integer; \
range [-9223372036854775808, 9223372036854775807]; little-endian byte order \
in SEMAPRAX target ABIs.";
const BOOL_DESCRIPTION: &str = "Canonical true/false boolean.";

const ARITHMETIC_STATUS_NOTE: &str = "Checked i64 arithmetic failures select \
the compiler-owned failure domain semaprax.arithmetic.v1 codes 1 add_overflow, \
2 sub_overflow, 3 mul_overflow, 4 division_by_zero, 5 division_overflow, \
6 remainder_by_zero, 7 remainder_overflow, 8 negation_overflow.";
const CONTRACT_STATUS_NOTE: &str = "A violated requires clause selects the \
compiler-owned failure domain semaprax.contract.v1 code 1; a violated ensures \
clause selects code 2.";
const TOTAL_SIGNATURE_NOTE: &str = "The direct signature declares no contract \
clauses and contains no i64 values, so no compiler-owned failure status is \
attributed to it here.";

const MIGRATION_NOTES: [&str; 3] = [
    "A breaking verdict requires a major version bump before replacing any published document.",
    "A compatible verdict permits at most a minor or patch version bump.",
    "Informational findings require review but no version action.",
];

const NONCLAIMS_GENERATION_JSON: &str = "[\
\"no_protobuf_grpc_graphql_sql\",\
\"no_schema_import_parsing\",\
\"no_live_conformance_fixtures\",\
\"no_registry_server_or_hosting\",\
\"no_target_execution\",\
\"read_only_no_source_changes\"]";

const NONCLAIMS_COMPAT_JSON: &str = "[\
\"no_external_compatibility_tooling\",\
\"no_live_conformance_fixtures\",\
\"no_registry_server_or_hosting\",\
\"no_source_changes\"]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenApiOptions {
    pub max_bytes: usize,
}

impl OpenApiOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(options_error(format!(
                "openapi max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for OpenApiOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn options_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-OA101", message)
}

fn selection_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-OA102", message)
}

fn excluded_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-OA103", message)
}

fn authentication_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-OA104", message)
}

fn budget_error(limit: usize, required: usize) -> Diagnostic {
    Diagnostic::io(
        "SPX-OA105",
        format!(
            "bounded-output budget exceeded: the canonical report needs {required} bytes but the limit is {limit} bytes; failing closed without emitting truncated bytes"
        ),
    )
}

/// Projects one verified source into the canonical OpenAPI envelope.
///
/// `selections` must contain between 1 and [`MAX_FUNCTIONS`] entries naming
/// functions by stable id or plain name; duplicates are rejected.
pub fn generate(
    source_path: &Path,
    selections: &[String],
    options: &OpenApiOptions,
) -> Result<String, Vec<Diagnostic>> {
    if selections.is_empty() || selections.len() > MAX_FUNCTIONS {
        return Err(vec![options_error(format!(
            "openapi requires between 1 and {MAX_FUNCTIONS} --function selections"
        ))]);
    }
    let mut unique = BTreeSet::new();
    for selection in selections {
        if !unique.insert(selection.as_str()) {
            return Err(vec![options_error(format!(
                "duplicate openapi function selection `{selection}`"
            ))]);
        }
    }

    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);

    let mut index_by_stable_id: BTreeMap<&str, usize> = BTreeMap::new();
    let mut index_by_name: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, function) in program.functions.iter().enumerate() {
        index_by_stable_id
            .entry(function.stable_id.as_str())
            .or_insert(index);
        index_by_name.entry(function.name.as_str()).or_insert(index);
    }
    let mut selected_indices: Vec<usize> = Vec::with_capacity(selections.len());
    let mut errors = Vec::new();
    for selection in selections {
        let index = index_by_stable_id
            .get(selection.as_str())
            .or_else(|| index_by_name.get(selection.as_str()));
        match index {
            Some(index) => selected_indices.push(*index),
            None => errors.push(selection_error(format!(
                "selection `{selection}` matches no function name or stable id in {}",
                source_path.display()
            ))),
        }
    }
    selected_indices.sort_unstable();
    selected_indices.dedup();
    let mut admitted: Vec<&Function> = Vec::with_capacity(selected_indices.len());
    for index in &selected_indices {
        let function = &program.functions[*index];
        if let Some(reason) = admission(function) {
            errors.push(excluded_error(format!(
                "function `{}` is excluded from OpenAPI Schema Generation v1; reason={reason}",
                function.stable_id
            )));
        } else {
            admitted.push(function);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let envelope = build_envelope(
        source_path,
        snapshot.source(),
        &program,
        &revision,
        &admitted,
        options,
    )?;
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Admission vocabulary mirrors the Public Scalar Export Profile: only
/// monomorphic, effect-free functions over direct by-value `i64`/`bool`
/// scalars are admitted. Bodies are not interpreted.
fn admission(function: &Function) -> Option<&'static str> {
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

fn build_envelope(
    source_path: &Path,
    source: &str,
    program: &Program,
    revision: &str,
    admitted: &[&Function],
    options: &OpenApiOptions,
) -> Result<String, Vec<Diagnostic>> {
    let document = build_document(program, revision, admitted)?;

    let mut names = BTreeSet::new();
    for function in admitted {
        if !names.insert(derived_name(&function.stable_id)) {
            return Err(vec![excluded_error(format!(
                "functions `{}` collide on the deterministic component name `{}`",
                function.stable_id,
                derived_name(&function.stable_id)
            ))]);
        }
    }

    let payload_digest = document_digest(&Value::Object(document.clone()));
    let envelope = Map::from_iter([
        ("schema".to_owned(), Value::String(SCHEMA.to_owned())),
        (
            "source".to_owned(),
            Value::Object(Map::from_iter([
                (
                    "path".to_owned(),
                    Value::String(source_path.display().to_string()),
                ),
                ("revision".to_owned(), Value::String(revision.to_owned())),
                ("sha256".to_owned(), Value::String(source_digest(source))),
            ])),
        ),
        (
            "limits".to_owned(),
            Value::Object(Map::from_iter([
                ("max_functions".to_owned(), Value::from(MAX_FUNCTIONS)),
                ("max_bytes".to_owned(), Value::from(options.max_bytes)),
            ])),
        ),
        ("operations".to_owned(), Value::from(admitted.len())),
        ("sha256".to_owned(), Value::String(payload_digest)),
        ("document".to_owned(), Value::Object(document)),
        (
            "nonclaims".to_owned(),
            serde_json::from_str(NONCLAIMS_GENERATION_JSON)
                .expect("generation nonclaims constant is valid JSON"),
        ),
    ]);

    render_bounded(&Value::Object(envelope), options.max_bytes)
}

fn render_bounded(value: &Value, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
    let (rendered, overflowed) = with_limit(max_bytes, || {
        let bytes = serde_json::to_string(value).unwrap_or_default();
        budgeted_clone(&bytes)
    });
    if overflowed || rendered.len() > max_bytes || rendered.is_empty() {
        let required = if rendered.is_empty() || overflowed {
            max_bytes.saturating_add(1)
        } else {
            rendered.len()
        };
        return Err(vec![budget_error(max_bytes, required)]);
    }
    Ok(rendered)
}

fn build_document(
    program: &Program,
    revision: &str,
    admitted: &[&Function],
) -> Result<Map<String, Value>, Vec<Diagnostic>> {
    let mut schemas = Map::new();
    let mut paths = Map::new();
    let mut status_needed = false;

    for function in admitted {
        let component = derived_name(&function.stable_id);
        let has_i64 = signature_has_i64(function);
        let has_contracts = !function.requires.is_empty() || !function.ensures.is_empty();

        let mut request_properties = Map::new();
        let mut required = Vec::with_capacity(function.params.len());
        for param in &function.params {
            request_properties.insert(param.name.clone(), scalar_schema(&param.ty));
            required.push(Value::String(param.name.clone()));
        }
        let request_name = format!("{component}{REQUEST_SUFFIX}");
        schemas.insert(
            request_name.clone(),
            Value::Object(Map::from_iter([
                ("type".to_owned(), Value::String("object".to_owned())),
                ("properties".to_owned(), Value::Object(request_properties)),
                ("required".to_owned(), Value::Array(required)),
                ("additionalProperties".to_owned(), Value::Bool(false)),
            ])),
        );
        let result_name = format!("{component}{RESULT_SUFFIX}");
        schemas.insert(result_name.clone(), scalar_schema(&function.return_type));

        let mut responses = Map::new();
        responses.insert(
            "200".to_owned(),
            Value::Object(Map::from_iter([
                (
                    "description".to_owned(),
                    Value::String("Success.".to_owned()),
                ),
                (
                    "content".to_owned(),
                    Value::Object(Map::from_iter([(
                        "application/json".to_owned(),
                        Value::Object(Map::from_iter([(
                            "schema".to_owned(),
                            Value::Object(Map::from_iter([(
                                "$ref".to_owned(),
                                Value::String(format!("#/components/schemas/{result_name}")),
                            )])),
                        )])),
                    )])),
                ),
            ])),
        );
        if has_i64 || has_contracts {
            status_needed = true;
            responses.insert(
                "default".to_owned(),
                Value::Object(Map::from_iter([
                    (
                        "description".to_owned(),
                        Value::String("Compiler-owned SEMAPRAX failure status.".to_owned()),
                    ),
                    (
                        "content".to_owned(),
                        Value::Object(Map::from_iter([(
                            "application/json".to_owned(),
                            Value::Object(Map::from_iter([(
                                "schema".to_owned(),
                                Value::Object(Map::from_iter([(
                                    "$ref".to_owned(),
                                    Value::String(format!(
                                        "#/components/schemas/{STATUS_COMPONENT_NAME}"
                                    )),
                                )])),
                            )])),
                        )])),
                    ),
                ])),
            );
        }

        let operation = Value::Object(Map::from_iter([
            (
                "operationId".to_owned(),
                Value::String(derived_name(&function.stable_id)),
            ),
            (
                "x-stable-id".to_owned(),
                Value::String(function.stable_id.clone()),
            ),
            (
                "description".to_owned(),
                Value::String(operation_description(function)),
            ),
            (
                "requestBody".to_owned(),
                Value::Object(Map::from_iter([
                    ("required".to_owned(), Value::Bool(true)),
                    (
                        "content".to_owned(),
                        Value::Object(Map::from_iter([(
                            "application/json".to_owned(),
                            Value::Object(Map::from_iter([(
                                "schema".to_owned(),
                                Value::Object(Map::from_iter([(
                                    "$ref".to_owned(),
                                    Value::String(format!("#/components/schemas/{request_name}")),
                                )])),
                            )])),
                        )])),
                    ),
                ])),
            ),
            ("responses".to_owned(), Value::Object(responses)),
        ]));
        paths.insert(
            format!("/{}", function.stable_id),
            Value::Object(Map::from_iter([("post".to_owned(), operation)])),
        );
    }

    if status_needed {
        schemas.insert(STATUS_COMPONENT_NAME.to_owned(), status_schema());
    }

    let title = format!("{} OpenAPI schema", program.module);
    let description = format!(
        "Deterministic SEMAPRAX OpenAPI Schema Generation v1 projection of the verified module {}; \
integer range and byte-order notes are static descriptions derived from declared types.",
        program.module
    );

    Ok(Map::from_iter([
        (
            "openapi".to_owned(),
            Value::String(OPENAPI_VERSION.to_owned()),
        ),
        (
            "info".to_owned(),
            Value::Object(Map::from_iter([
                ("title".to_owned(), Value::String(title)),
                ("version".to_owned(), Value::String(revision.to_owned())),
                ("description".to_owned(), Value::String(description)),
            ])),
        ),
        ("paths".to_owned(), Value::Object(paths)),
        (
            "components".to_owned(),
            Value::Object(Map::from_iter([(
                "schemas".to_owned(),
                Value::Object(schemas),
            )])),
        ),
    ]))
}

fn signature_has_i64(function: &Function) -> bool {
    function.params.iter().any(|param| param.ty == Type::I64) || function.return_type == Type::I64
}

fn scalar_schema(ty: &Type) -> Value {
    match ty {
        Type::Bool => Value::Object(Map::from_iter([
            ("type".to_owned(), Value::String("boolean".to_owned())),
            (
                "description".to_owned(),
                Value::String(BOOL_DESCRIPTION.to_owned()),
            ),
        ])),
        _ => Value::Object(Map::from_iter([
            ("type".to_owned(), Value::String("integer".to_owned())),
            ("format".to_owned(), Value::String("int64".to_owned())),
            (
                "description".to_owned(),
                Value::String(I64_DESCRIPTION.to_owned()),
            ),
        ])),
    }
}

fn status_schema() -> Value {
    Value::Object(Map::from_iter([
        ("type".to_owned(), Value::String("object".to_owned())),
        (
            "description".to_owned(),
            Value::String(
                "Normalized SEMAPRAX failure status (semaprax.status.v1). Compiler-owned \
domains: semaprax.arithmetic.v1 codes 1 add_overflow, 2 sub_overflow, 3 mul_overflow, \
4 division_by_zero, 5 division_overflow, 6 remainder_by_zero, 7 remainder_overflow, \
8 negation_overflow (class arithmetic); semaprax.contract.v1 codes 1 requires-false and \
2 ensures-false (class contract). All listed statuses are non-retryable."
                    .to_owned(),
            ),
        ),
        (
            "properties".to_owned(),
            Value::Object(Map::from_iter([
                (
                    "schema".to_owned(),
                    Value::Object(Map::from_iter([(
                        "type".to_owned(),
                        Value::String("string".to_owned()),
                    )])),
                ),
                (
                    "domain_id".to_owned(),
                    Value::Object(Map::from_iter([(
                        "type".to_owned(),
                        Value::String("string".to_owned()),
                    )])),
                ),
                (
                    "code".to_owned(),
                    Value::Object(Map::from_iter([
                        ("type".to_owned(), Value::String("integer".to_owned())),
                        ("format".to_owned(), Value::String("int32".to_owned())),
                        ("minimum".to_owned(), Value::from(1)),
                    ])),
                ),
                (
                    "class".to_owned(),
                    Value::Object(Map::from_iter([
                        ("type".to_owned(), Value::String("string".to_owned())),
                        (
                            "enum".to_owned(),
                            Value::Array(vec![
                                Value::String("arithmetic".to_owned()),
                                Value::String("contract".to_owned()),
                            ]),
                        ),
                    ])),
                ),
                (
                    "retryable".to_owned(),
                    Value::Object(Map::from_iter([(
                        "type".to_owned(),
                        Value::String("boolean".to_owned()),
                    )])),
                ),
            ])),
        ),
        (
            "required".to_owned(),
            Value::Array(vec![
                Value::String("schema".to_owned()),
                Value::String("domain_id".to_owned()),
                Value::String("code".to_owned()),
                Value::String("class".to_owned()),
                Value::String("retryable".to_owned()),
            ]),
        ),
        ("additionalProperties".to_owned(), Value::Bool(false)),
    ]))
}

fn operation_description(function: &Function) -> String {
    let mut description = format!("SEMAPRAX function {}.", function.name);
    for clause in function.requires.iter() {
        description.push_str(&format!(" requires {};", format::expr(clause, 0)));
    }
    for clause in function.ensures.iter() {
        description.push_str(&format!(" ensures {};", format::expr(clause, 0)));
    }
    description.push(' ');
    if !function.requires.is_empty() || !function.ensures.is_empty() {
        description.push_str(CONTRACT_STATUS_NOTE);
        description.push(' ');
    }
    if signature_has_i64(function) {
        description.push_str(ARITHMETIC_STATUS_NOTE);
    } else if function.requires.is_empty() && function.ensures.is_empty() {
        description.push_str(TOTAL_SIGNATURE_NOTE);
    }
    description
}

/// Deterministic component/operation name: every character outside
/// `[A-Za-z0-9_]` becomes `_`. Collisions across selections fail closed.
fn derived_name(stable_id: &str) -> String {
    stable_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn source_digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.openapi.source.v1\0");
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Domain-separated SHA-256 over the exact canonical payload bytes of one
/// generated document. Compatibility replays this digest over the parsed
/// document's canonical re-serialization, which is byte-stable because the
/// document round-trips through sorted-key compact JSON.
fn document_digest(document: &Value) -> String {
    let payload = serde_json::to_vec(document).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.openapi.document.v1\0");
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(&payload);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Reads and fully authenticates one generated envelope.
fn authenticate(raw: &str, label: &str) -> Result<Value, Vec<Diagnostic>> {
    let value: Value = serde_json::from_str(raw).map_err(|error| {
        vec![authentication_error(format!(
            "{label} is not valid JSON: {error}"
        ))]
    })?;
    let object = value.as_object().ok_or_else(|| {
        vec![authentication_error(format!(
            "{label} is not a JSON object"
        ))]
    })?;
    let schema = object
        .get("schema")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            vec![authentication_error(format!(
                "{label} carries no envelope schema string"
            ))]
        })?;
    if schema != SCHEMA {
        return Err(vec![authentication_error(format!(
            "{label} is not a {SCHEMA} envelope"
        ))]);
    }
    let document = object.get("document").cloned().ok_or_else(|| {
        vec![authentication_error(format!(
            "{label} carries no document member"
        ))]
    })?;
    if document.get("openapi").and_then(Value::as_str) != Some(OPENAPI_VERSION) {
        return Err(vec![authentication_error(format!(
            "{label} document is not an OpenAPI {OPENAPI_VERSION} projection"
        ))]);
    }
    if document
        .get("paths")
        .and_then(Value::as_object)
        .is_none_or(|paths| paths.is_empty())
    {
        return Err(vec![authentication_error(format!(
            "{label} document carries no operations"
        ))]);
    }
    let claimed = object
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            vec![authentication_error(format!(
                "{label} carries no sha256 digest"
            ))]
        })?;
    let actual = document_digest(&document);
    if claimed != actual {
        return Err(vec![authentication_error(format!(
            "{label} document digest mismatch: claimed {claimed}, computed {actual}"
        ))]);
    }
    Ok(value)
}

/// Compares two previously generated envelopes and emits the canonical
/// compatibility report.
///
/// Both inputs are authenticated before any classification runs; the report
/// binds inputs by their digests and revisions, never by filesystem paths, so
/// the exact report bytes are reproducible across machines.
pub fn compatibility(
    base_path: &Path,
    candidate_path: &Path,
    options: &OpenApiOptions,
) -> Result<String, Vec<Diagnostic>> {
    let base_raw = std::fs::read_to_string(base_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", base_path.display()),
        )]
    })?;
    let candidate_raw = std::fs::read_to_string(candidate_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I001",
            format!("cannot read {}: {error}", candidate_path.display()),
        )]
    })?;
    let base = authenticate(&base_raw, "base document")?;
    let candidate = authenticate(&candidate_raw, "candidate document")?;
    let findings = classify(&base, &candidate);

    let breaking = findings
        .iter()
        .filter(|finding| finding.severity == SEVERITY_BREAKING)
        .count();
    let non_breaking = findings
        .iter()
        .filter(|finding| finding.severity == SEVERITY_NON_BREAKING)
        .count();
    let informational = findings
        .iter()
        .filter(|finding| finding.severity == SEVERITY_INFORMATIONAL)
        .count();
    let verdict = if breaking > 0 {
        VERDICT_BREAKING
    } else {
        VERDICT_COMPATIBLE
    };

    let findings_value: Vec<Value> = findings.iter().map(|finding| finding.to_value()).collect();
    let input_binding = compatibility_input_digest(
        document_digest(&base["document"]),
        document_digest(&candidate["document"]),
    );

    let report = Value::Object(Map::from_iter([
        ("schema".to_owned(), Value::String(COMPAT_SCHEMA.to_owned())),
        ("base".to_owned(), envelope_reference(&base, "base")),
        (
            "candidate".to_owned(),
            envelope_reference(&candidate, "candidate"),
        ),
        ("verdict".to_owned(), Value::String(verdict.to_owned())),
        (
            "summary".to_owned(),
            Value::Object(Map::from_iter([
                ("breaking".to_owned(), Value::from(breaking)),
                ("non_breaking".to_owned(), Value::from(non_breaking)),
                ("informational".to_owned(), Value::from(informational)),
            ])),
        ),
        ("findings".to_owned(), Value::Array(findings_value)),
        ("input_sha256".to_owned(), Value::String(input_binding)),
        (
            "limits".to_owned(),
            Value::Object(Map::from_iter([(
                "max_bytes".to_owned(),
                Value::from(options.max_bytes),
            )])),
        ),
        (
            "migration".to_owned(),
            Value::Object(Map::from_iter([
                (
                    "major_version_bump_required".to_owned(),
                    Value::Bool(verdict == VERDICT_BREAKING),
                ),
                (
                    "notes".to_owned(),
                    Value::Array(
                        MIGRATION_NOTES
                            .iter()
                            .map(|note| Value::String((*note).to_owned()))
                            .collect(),
                    ),
                ),
            ])),
        ),
        (
            "nonclaims".to_owned(),
            serde_json::from_str::<Value>(NONCLAIMS_COMPAT_JSON)
                .expect("compatibility nonclaims constant is valid JSON"),
        ),
    ]));

    render_bounded(&report, options.max_bytes)
}

fn envelope_reference(envelope: &Value, label: &str) -> Value {
    Value::Object(Map::from_iter([
        ("role".to_owned(), Value::String(label.to_owned())),
        (
            "sha256".to_owned(),
            Value::String(
                envelope
                    .get("sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
        (
            "source_revision".to_owned(),
            Value::String(
                envelope["source"]
                    .get("revision")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
        (
            "document_sha256".to_owned(),
            Value::String(document_digest(&envelope["document"])),
        ),
    ]))
}

fn compatibility_input_digest(base: String, candidate: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.openapi-compat.inputs.v1\0");
    for digest in [base, candidate] {
        hasher.update((digest.len() as u64).to_le_bytes());
        hasher.update(digest.as_bytes());
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

struct Finding {
    code: &'static str,
    severity: &'static str,
    location: String,
    detail: String,
}

impl Finding {
    fn to_value(&self) -> Value {
        Value::Object(Map::from_iter([
            ("code".to_owned(), Value::String(self.code.to_owned())),
            (
                "severity".to_owned(),
                Value::String(self.severity.to_owned()),
            ),
            ("location".to_owned(), Value::String(self.location.clone())),
            ("detail".to_owned(), Value::String(self.detail.clone())),
        ]))
    }
}

fn paths_of(envelope: &Value) -> BTreeMap<String, Value> {
    envelope["document"]["paths"]
        .as_object()
        .map(|paths| {
            paths
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn post_of(operation_path: &Value) -> Option<&Value> {
    operation_path.get("post")
}

/// Resolves the request schema object of one operation through its `$ref`.
fn request_schema<'a>(operation: &'a Value, schemas: &'a Map<String, Value>) -> Option<&'a Value> {
    let reference =
        operation["requestBody"]["content"]["application/json"]["schema"]["$ref"].as_str()?;
    resolve_ref(reference, schemas)
}

/// Resolves the success result schema object of one operation through its `$ref`.
fn result_schema<'a>(operation: &'a Value, schemas: &'a Map<String, Value>) -> Option<&'a Value> {
    let reference =
        operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"].as_str()?;
    resolve_ref(reference, schemas)
}

fn resolve_ref<'a>(reference: &str, schemas: &'a Map<String, Value>) -> Option<&'a Value> {
    let name = reference.strip_prefix("#/components/schemas/")?;
    schemas.get(name)
}

/// Ordered `(name, type, format)` request properties of one request schema.
fn request_parameters(schema: &Value) -> Vec<(String, String, Option<String>)> {
    let mut parameters = Vec::new();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return parameters;
    };
    for (name, property) in properties {
        parameters.push((
            name.clone(),
            property
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            property
                .get("format")
                .and_then(Value::as_str)
                .map(str::to_owned),
        ));
    }
    parameters.sort_by(|left, right| left.0.cmp(&right.0));
    parameters
}

fn result_shape(schema: &Value) -> (String, Option<String>) {
    (
        schema
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        schema
            .get("format")
            .and_then(Value::as_str)
            .map(str::to_owned),
    )
}

fn classify(base: &Value, candidate: &Value) -> Vec<Finding> {
    let mut findings = Vec::new();
    let base_paths = paths_of(base);
    let candidate_paths = paths_of(candidate);
    let base_schemas = base["document"]["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let candidate_schemas = candidate["document"]["components"]["schemas"]
        .as_object()
        .cloned()
        .unwrap_or_default();

    for (path, base_operation_path) in &base_paths {
        let Some(candidate_operation_path) = candidate_paths.get(path) else {
            findings.push(Finding {
                code: FINDING_OPERATION_REMOVED,
                severity: SEVERITY_BREAKING,
                location: path.clone(),
                detail: "operation removed".to_owned(),
            });
            continue;
        };
        let (Some(base_operation), Some(candidate_operation)) = (
            post_of(base_operation_path),
            post_of(candidate_operation_path),
        ) else {
            continue;
        };
        let (Some(base_request), Some(candidate_request)) = (
            request_schema(base_operation, &base_schemas),
            request_schema(candidate_operation, &candidate_schemas),
        ) else {
            continue;
        };
        let base_parameters = request_parameters(base_request);
        let candidate_parameters = request_parameters(candidate_request);

        for (name, base_type, base_format) in &base_parameters {
            let matched = candidate_parameters
                .iter()
                .find(|(candidate_name, _, _)| candidate_name == name);
            match matched {
                None => findings.push(Finding {
                    code: FINDING_PARAMETER_REMOVED,
                    severity: SEVERITY_BREAKING,
                    location: format!("{path}:{name}"),
                    detail: format!("request parameter `{name}` removed"),
                }),
                Some((_, candidate_type, candidate_format)) => {
                    if base_type != candidate_type || base_format != candidate_format {
                        findings.push(Finding {
                            code: FINDING_PARAMETER_TYPE_CHANGED,
                            severity: SEVERITY_BREAKING,
                            location: format!("{path}:{name}"),
                            detail: format!(
                                "request parameter `{name}` changed type from {}:{} to {}:{}",
                                base_type,
                                base_format.as_deref().unwrap_or("-"),
                                candidate_type,
                                candidate_format.as_deref().unwrap_or("-"),
                            ),
                        });
                    }
                }
            }
        }
        for (name, candidate_type, _) in &candidate_parameters {
            if base_parameters
                .iter()
                .any(|(base_name, _, _)| base_name == name)
            {
                continue;
            }
            // Every admitted scalar parameter is required, so any unknown
            // candidate parameter is a breaking addition.
            findings.push(Finding {
                code: FINDING_REQUIRED_PARAMETER_ADDED,
                severity: SEVERITY_BREAKING,
                location: format!("{path}:{name}"),
                detail: format!("required request parameter `{name}` added ({candidate_type})"),
            });
        }
        if let (Some(base_result), Some(candidate_result)) = (
            result_schema(base_operation, &base_schemas),
            result_schema(candidate_operation, &candidate_schemas),
        ) {
            let base_shape = result_shape(base_result);
            let candidate_shape = result_shape(candidate_result);
            if base_shape != candidate_shape {
                findings.push(Finding {
                    code: FINDING_RESULT_TYPE_CHANGED,
                    severity: SEVERITY_BREAKING,
                    location: path.clone(),
                    detail: format!(
                        "result changed type from {}:{} to {}:{}",
                        base_shape.0,
                        base_shape.1.as_deref().unwrap_or("-"),
                        candidate_shape.0,
                        candidate_shape.1.as_deref().unwrap_or("-"),
                    ),
                });
            }
        }
        let base_description = base_operation
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let candidate_description = candidate_operation
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if base_description != candidate_description {
            findings.push(Finding {
                code: FINDING_OPERATION_DESCRIPTION_CHANGED,
                severity: SEVERITY_INFORMATIONAL,
                location: path.clone(),
                detail: "operation description changed".to_owned(),
            });
        }
    }
    for path in candidate_paths.keys() {
        if !base_paths.contains_key(path) {
            findings.push(Finding {
                code: FINDING_OPERATION_ADDED,
                severity: SEVERITY_NON_BREAKING,
                location: path.clone(),
                detail: "operation added".to_owned(),
            });
        }
    }

    let base_revision = base["source"]["revision"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let candidate_revision = candidate["source"]["revision"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if base_revision != candidate_revision {
        findings.push(Finding {
            code: FINDING_SOURCE_REVISION_CHANGED,
            severity: SEVERITY_INFORMATIONAL,
            location: "source.revision".to_owned(),
            detail: format!("source revision changed from {base_revision} to {candidate_revision}"),
        });
    }

    findings
}
