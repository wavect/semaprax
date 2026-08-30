//! Source-only nominal display renames through the shared authenticated
//! Operations occurrence planner. No second index or publication route.
use crate::ast::{Program, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use crate::semantic_workspace::SemanticWorkspaceSource;
use serde_json::Value;

use super::{intent, parse_revision};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Invocation-local output of exact Operations replay; never caller supplied.
pub(super) struct NominalRename {
    sources: Vec<SemanticWorkspaceSource>,
}

/// Eligibility is source ownership/kind discovery, not a guarantee that an
/// arbitrary name or every reference form passes the shared replay engine.
pub(super) fn eligible(revision: &ProjectRevision, target: &str) -> Result<bool> {
    let programs = parse_revision(revision)?;
    let mut found = false;
    for program in &programs {
        for declaration in &program.types {
            if declaration.stable_id == target
                && declaration.explicit_id
                && matches!(
                    declaration.kind,
                    TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Variant { .. }
                )
            {
                if found {
                    return Err(invalid("candidate nominal rename identity is ambiguous"));
                }
                found = true;
            }
        }
    }
    Ok(found)
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(intent::IntentSummary, NominalRename)> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("candidate nominal rename must be an object"))?;
    if object.len() != 3
        || ["kind", "target", "name"]
            .iter()
            .any(|key| !object.contains_key(*key))
        || request["kind"] != "rename_declaration"
    {
        return Err(invalid(
            "candidate nominal rename has missing or unknown fields",
        ));
    }
    let target = request["target"]
        .as_str()
        .ok_or_else(|| invalid("candidate nominal rename requires a stable identity"))?;
    if target.is_empty() || target.len() > intent::MAX_ID_BYTES || target.contains('\0') {
        return Err(invalid(
            "candidate nominal rename identity exceeds its grammar",
        ));
    }
    let name = intent::identifier(
        request["name"]
            .as_str()
            .ok_or_else(|| invalid("candidate nominal rename requires a display name"))?,
    )?;
    let old = programs
        .iter()
        .flat_map(|program| &program.types)
        .find(|declaration| declaration.stable_id == target)
        .ok_or_else(|| invalid("candidate nominal rename source declaration is absent"))?;
    if old.name == name {
        return Err(invalid(
            "candidate declaration rename must change its display name",
        ));
    }
    if !old.explicit_id
        || !matches!(
            old.kind,
            TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Variant { .. }
        )
    {
        return Err(invalid(
            "candidate nominal rename requires an explicit record or variant",
        ));
    }
    let sources = crate::semantic_workspace_operations::derive_nominal_rename(
        revision
            .sources()
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source.path().to_owned(),
                source: source.source().to_owned(),
            })
            .collect(),
        revision.manifest().entry(),
        target,
        name,
    )?;
    if sources.len() != programs.len()
        || sources
            .iter()
            .zip(programs.iter())
            .any(|(source, program)| source.path != program.path)
    {
        return Err(invalid(
            "candidate nominal rename changed the canonical source inventory",
        ));
    }
    // Preserve canonical source as the sole handoff. The shared planner already
    // replayed identities, occurrences, names, normalized meaning and edges;
    // Candidate apply still independently rebuilds the complete Project twice.
    for (program, source) in programs.iter_mut().zip(&sources) {
        *program = crate::parse(&source.source, &source.path).map_err(|error| vec![error])?;
    }
    Ok((
        intent::IntentSummary {
            target_id: target.to_owned(),
            kind: "rename_declaration".to_owned(),
            migrated_calls: 0,
        },
        NominalRename { sources },
    ))
}

pub(super) fn validate(after: &ProjectRevision, rename: &NominalRename) -> Result<()> {
    if rename.sources.len() != after.sources().len()
        || rename
            .sources
            .iter()
            .zip(after.sources())
            .any(|(expected, actual)| {
                expected.path != actual.path() || expected.source != actual.source()
            })
    {
        return Err(invalid(
            "candidate nominal rename disagrees with its replayed source plan",
        ));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
