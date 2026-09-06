//! Closed authentication for the only Project v8 package without Web exports.

use std::path::Path;

use crate::diagnostic::Diagnostic;

use crate::semantic_workspace::SemanticWorkspaceSource;

use super::manifest::ProjectManifest;

const WRAPPER_IDS: [&str; 5] = [
    "std.collections.vec.with-capacity",
    "std.collections.vec.push",
    "std.collections.vec.len",
    "std.collections.vec.capacity",
    "std.collections.vec.get",
];

const AUTHENTICATED_SOURCES: [(&str, &str); 3] = [
    (
        "src/collections.spx",
        include_str!("../../std/collections/src/collections.spx"),
    ),
    (
        "src/examples.spx",
        include_str!("../../std/collections/src/examples.spx"),
    ),
    (
        "src/tests.spx",
        include_str!("../../std/collections/src/tests.spx"),
    ),
];

pub(super) fn authenticate_no_export_package(
    manifest: &ProjectManifest,
    sources: &[SemanticWorkspaceSource],
) -> Result<(), Vec<Diagnostic>> {
    if !manifest.is_no_export_std_collections() {
        return Ok(());
    }
    if AUTHENTICATED_SOURCES.iter().any(|(path, expected)| {
        !sources
            .iter()
            .any(|source| source.path == *path && source.source == *expected)
    }) {
        return Err(rejected());
    }
    let source = sources
        .iter()
        .find(|source| source.path == "src/collections.spx")
        .ok_or_else(rejected)?;
    let program =
        crate::parse(&source.source, Path::new(&source.path)).map_err(|error| vec![error])?;
    let ids = program
        .functions
        .iter()
        .map(|function| function.stable_id.as_str())
        .collect::<Vec<_>>();
    if program.module != crate::vec_ops::MODULE
        || !program.module_uses.is_empty()
        || !program.permits.is_empty()
        || !program.types.is_empty()
        || !program.interfaces.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.agents.is_empty()
        || ids != WRAPPER_IDS
        || program
            .functions
            .iter()
            .any(|function| crate::vec_ops::source_wrapper(&program, function).is_none())
    {
        return Err(rejected());
    }
    Ok(())
}

fn rejected() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-J100",
        "the no-export std.collections package must contain exactly its authenticated wrapper, example, and conformance sources",
    )]
}
