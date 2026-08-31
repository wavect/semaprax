//! Private identity-marker syntax for conflict fingerprints only. The input
//! occurrence proof is the ordinary Operations AST/HIR join, never spelling.
use super::{capacity, conflict, wire, ProjectRevision, MAX_FINGERPRINT_BYTES};
use crate::ast::{Program, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::{self, WorkspaceOperationOccurrence, WorkspaceSource};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MARKER_PREFIX: &str = "spx_rebase_ref_";
const MAX_SIDECAR_BYTES: usize = 64 * 1024 * 1024;
const MAX_OCCURRENCES: usize = 1_048_576;

pub(super) fn programs(
    revision: &ProjectRevision,
    programs: Vec<Program>,
) -> Result<Vec<Program>, Vec<Diagnostic>> {
    if !programs.iter().any(|program| {
        program.types.iter().any(|ty| {
            ty.explicit_id
                && matches!(
                    ty.kind,
                    TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Variant { .. }
                )
        })
    }) {
        return Ok(programs);
    }
    let sources = revision
        .sources()
        .iter()
        .map(|source| WorkspaceSource {
            path: source.path().to_owned(),
            source: source.source().to_owned(),
        })
        .collect::<Vec<_>>();
    // Reserve a private vocabulary rather than allowing authored identifiers to
    // impersonate a marker. This rejects conservatively, without a fallback.
    for source in &sources {
        let token_bytes = source
            .source
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_mul(std::mem::size_of::<crate::lexer::Token>()))
            .ok_or_else(|| capacity("candidate rebase marker scan token bound overflow"))?;
        if token_bytes > MAX_SIDECAR_BYTES {
            return Err(capacity(
                "candidate rebase marker scan exceeds its token allocation bound",
            ));
        }
        for token in crate::lexer::lex(&source.source, &source.path).map_err(|error| vec![error])? {
            if matches!(token.kind, crate::lexer::TokenKind::Ident(ref name) if name.starts_with(MARKER_PREFIX))
            {
                return Err(conflict("candidate rebase source identifier collides with its private identity marker namespace"));
            }
        }
    }
    let (sidecar, overflow) = crate::bounded_output::with_limit(MAX_SIDECAR_BYTES, || {
        workspace_graph::project_operation_sidecar(
            &programs,
            &sources,
            revision.semantic.image_modules(),
        )
    });
    if overflow {
        return Err(capacity(
            "candidate rebase occurrence proof exceeds its retained builder bound",
        ));
    }
    let sidecar = sidecar?;
    let declarations = sidecar
        .declarations
        .iter()
        .map(|declaration| (declaration.id.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    let mut markers = BTreeMap::new();
    let mut shapes = BTreeMap::new();
    let mut shape_bytes = 0usize;
    for declaration in &sidecar.declarations {
        if declaration.explicit
            && matches!(
                declaration.kind,
                "record" | "variant" | "record_field" | "variant_case" | "variant_field"
            )
        {
            let ancestor = declaration
                .namespace_owner
                .as_deref()
                .and_then(|owner| declarations.get(owner))
                .and_then(|owner| owner.namespace_owner.as_deref());
            let nominal_owner = ancestor
                .or(declaration.namespace_owner.as_deref())
                .unwrap_or(declaration.id.as_str());
            if !shapes.contains_key(nominal_owner) {
                let mut shape =
                    super::intent::nominal_type_dependency_fingerprint(revision, nominal_owner)?
                        .ok_or_else(|| {
                            conflict(
                        "candidate rebase reference lacks its checked nominal owner descriptor",
                    )
                        })?;
                descriptor(&mut shape);
                let bytes = serde_json::to_vec(&shape).map_err(|_| {
                    conflict("candidate rebase owner descriptor serialization failed")
                })?;
                shape_bytes = shape_bytes.saturating_add(bytes.len());
                if shape_bytes > MAX_SIDECAR_BYTES {
                    return Err(capacity(
                        "candidate rebase nominal owner descriptor work exceeds its byte bound",
                    ));
                }
                shapes.insert(
                    nominal_owner,
                    wire::digest(b"semaprax.rebase.reference-owner.v1\0", &bytes),
                );
            }
            let identity = serde_json::to_vec(&json!({"kind":declaration.kind,"id":declaration.id,"owner":declaration.namespace_owner,"ancestor":ancestor,"shape":shapes[nominal_owner]}))
                .map_err(|_| conflict("candidate rebase identity marker serialization failed"))?;
            let marker = format!(
                "{MARKER_PREFIX}{}",
                &wire::digest(b"semaprax.rebase.source-reference.v1\0", &identity)[7..]
            );
            if markers.insert(declaration.id.as_str(), marker).is_some() {
                return Err(conflict(
                    "candidate rebase occurrence identities are duplicated",
                ));
            }
        }
    }
    let mut edits: BTreeMap<&str, Vec<(&WorkspaceOperationOccurrence, &str, &str)>> =
        BTreeMap::new();
    let mut count = 0usize;
    for declaration in &sidecar.declarations {
        if let Some(marker) = markers.get(declaration.id.as_str()) {
            for occurrence in &declaration.occurrences {
                add(
                    &mut edits,
                    occurrence,
                    &declaration.name,
                    marker,
                    &mut count,
                )?;
            }
        }
    }
    for import in &sidecar.imports {
        if import.kind == "type" {
            if let Some(marker) = markers.get(import.target_id.as_str()) {
                for occurrence in &import.occurrences {
                    add(&mut edits, occurrence, &import.alias, marker, &mut count)?;
                }
            }
        }
    }
    let mut output = Vec::new();
    let mut total = 0usize;
    for source in &sources {
        let mut selected = edits.remove(source.path.as_str()).unwrap_or_default();
        selected.sort_by_key(|(occurrence, _, _)| (occurrence.span.start, occurrence.span.end));
        let mut cursor = 0usize;
        let mut normalized = String::new();
        for (occurrence, spelling, marker) in selected {
            let span = occurrence.span;
            if span.start < cursor || source.source.get(span.start..span.end) != Some(spelling) {
                return Err(conflict(
                    "candidate rebase occurrence spans disagree with exact retained source",
                ));
            }
            append(
                &mut normalized,
                &source.source[cursor..span.start],
                &mut total,
            )?;
            append(&mut normalized, marker, &mut total)?;
            if let Some(binding) = &occurrence.shorthand_binding {
                append(&mut normalized, ": ", &mut total)?;
                append(&mut normalized, binding, &mut total)?;
            }
            cursor = span.end;
        }
        append(&mut normalized, &source.source[cursor..], &mut total)?;
        // These parse-only copies cannot escape to candidate materialization.
        // Formatting them preserves AST order while excluding source spans.
        output.push(crate::parse(&normalized, &source.path).map_err(|error| vec![error])?);
    }
    if !edits.is_empty() {
        return Err(conflict(
            "candidate rebase occurrence proof contains an unknown source path",
        ));
    }
    Ok(output)
}

fn add<'a>(
    edits: &mut BTreeMap<&'a str, Vec<(&'a WorkspaceOperationOccurrence, &'a str, &'a str)>>,
    occurrence: &'a WorkspaceOperationOccurrence,
    spelling: &'a str,
    marker: &'a str,
    count: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    *count = count.saturating_add(1);
    if *count > MAX_OCCURRENCES {
        return Err(capacity(
            "candidate rebase occurrence count exceeds its bound",
        ));
    }
    edits
        .entry(&occurrence.path)
        .or_default()
        .push((occurrence, spelling, marker));
    Ok(())
}

fn append(output: &mut String, value: &str, total: &mut usize) -> Result<(), Vec<Diagnostic>> {
    *total = total.saturating_add(value.len());
    if *total > MAX_FINGERPRINT_BYTES {
        return Err(capacity(
            "candidate rebase normalized source exceeds its byte bound",
        ));
    }
    output.push_str(value);
    Ok(())
}

/// Strip only display names from the compiler-owned aggregate descriptor
/// grammar. IDs, owner namespaces, indices/order, types, origin and parameter
/// names remain exact; arbitrary recursive JSON keys are never discarded.
pub(super) fn descriptor(value: &mut Value) {
    if let Some(record) = value.get_mut("record") {
        descriptor(record);
        return;
    }
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if !matches!(
        object.get("kind").and_then(Value::as_str),
        Some("record" | "variant" | "match")
    ) {
        return;
    }
    object.remove("name");
    if let Some(fields) = object.get_mut("fields").and_then(Value::as_array_mut) {
        for field in fields {
            if let Some(field) = field.as_object_mut() {
                field.remove("name");
            }
        }
    }
    if let Some(cases) = object.get_mut("cases").and_then(Value::as_array_mut) {
        for case in cases {
            if let Some(case) = case.as_object_mut() {
                case.remove("name");
                if let Some(fields) = case.get_mut("fields").and_then(Value::as_array_mut) {
                    for field in fields {
                        if let Some(field) = field.as_object_mut() {
                            field.remove("name");
                        }
                    }
                }
            }
        }
    }
}
