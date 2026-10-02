//! Narrow removal of an unexported, unreferenced top-level function.

use serde_json::Value;

use crate::ast::Program;
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;

use super::intent::IntentSummary;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) struct DeclarationDeletion {
    pub(super) id: String,
    pub(super) path: String,
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(IntentSummary, DeclarationDeletion)> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("deletion intent must be an object"))?;
    if object.len() != 2 || !object.contains_key("kind") || !object.contains_key("target") {
        return Err(invalid(
            "deletion intent contains missing or unknown fields",
        ));
    }
    let id = request["target"]
        .as_str()
        .ok_or_else(|| invalid("deletion target must be a stable ID"))?;
    let mut selected = None;
    for (owner, program) in programs.iter().enumerate() {
        for (index, function) in program.functions.iter().enumerate() {
            if function.stable_id == id {
                if selected.replace((owner, index)).is_some() {
                    return Err(invalid("deletion target is ambiguous"));
                }
            }
        }
    }
    let (owner, index) =
        selected.ok_or_else(|| invalid("deletion target is not a top-level function"))?;
    let function = &programs[owner].functions[index];
    if !function.explicit_id || function.name == "main" {
        return Err(invalid("deletion requires an explicit non-main function"));
    }
    if revision
        .manifest()
        .web_exports()
        .iter()
        .any(|export| export == id)
    {
        return Err(invalid("deletion target is a manifest export"));
    }
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| invalid("retained declaration graph is invalid"))?;
    let edges = graph["edges"]
        .as_array()
        .ok_or_else(|| invalid("retained declaration graph lacks edges"))?;
    if edges.iter().any(|edge| edge["target"].as_str() == Some(id)) {
        return Err(invalid("deletion target has structural consumers"));
    }
    let path = programs[owner].path.clone();
    programs[owner].functions.remove(index);
    Ok((
        IntentSummary {
            target_id: id.to_owned(),
            kind: "delete_declaration".to_owned(),
            migrated_calls: 0,
        },
        DeclarationDeletion {
            id: id.to_owned(),
            path,
        },
    ))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
