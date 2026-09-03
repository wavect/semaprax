//! Exact source-bound fingerprints for operation declarations.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::ast::Span;
use crate::diagnostic::Diagnostic;

use super::super::{
    active_builder_limit, limit_error, reserve_builder_structure, WorkspaceOperationDeclaration,
    WorkspaceOperationImport,
};

pub(super) fn declaration(
    declaration: &WorkspaceOperationDeclaration,
    declarations: &[WorkspaceOperationDeclaration],
    imports: &[WorkspaceOperationImport],
    sources: &BTreeMap<&str, &str>,
) -> Result<String, Vec<Diagnostic>> {
    let source = sources
        .get(declaration.path.as_str())
        .ok_or_else(super::operation_sidecar_disagreement)?;
    source
        .get(declaration.span.start..declaration.span.end)
        .ok_or_else(super::operation_sidecar_disagreement)?;
    let occurrence_count = declarations
        .iter()
        .map(|target| target.occurrences.len())
        .chain(imports.iter().map(|target| target.occurrences.len()))
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(super::operation_sidecar_disagreement)?;
    reserve_builder_structure(
        occurrence_count
            .checked_mul(std::mem::size_of::<(Span, &str, Option<&str>)>())
            .ok_or_else(|| vec![limit_error("change_builder_bytes", active_builder_limit())])?,
    )?;
    let mut substitutions = declarations
        .iter()
        .flat_map(|target| {
            target.occurrences.iter().filter_map(|occurrence| {
                (occurrence.path == declaration.path
                    && occurrence.span.start >= declaration.span.start
                    && occurrence.span.end <= declaration.span.end)
                    .then_some((
                        occurrence.span,
                        target.id.as_str(),
                        occurrence.shorthand_binding.as_deref(),
                    ))
            })
        })
        .chain(imports.iter().flat_map(|target| {
            target.occurrences.iter().filter_map(|occurrence| {
                (occurrence.path == declaration.path
                    && occurrence.span.start >= declaration.span.start
                    && occurrence.span.end <= declaration.span.end)
                    .then_some((
                        occurrence.span,
                        target.target_id.as_str(),
                        occurrence.shorthand_binding.as_deref(),
                    ))
            })
        }))
        .collect::<Vec<_>>();
    substitutions.sort_by_key(|(span, _, _)| (span.start, span.end));
    if substitutions
        .windows(2)
        .any(|pair| pair[0].0.end > pair[1].0.start)
    {
        return Err(super::operation_sidecar_disagreement());
    }
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-workspace-operations.normalized-declaration.v1\0");
    let mut cursor = declaration.span.start;
    for (span, identity, shorthand_binding) in substitutions {
        hasher.update(&source.as_bytes()[cursor..span.start]);
        hasher.update((identity.len() as u64).to_le_bytes());
        hasher.update(identity.as_bytes());
        if let Some(binding) = shorthand_binding {
            hasher.update(b": ");
            hasher.update(binding.as_bytes());
        }
        cursor = span.end;
    }
    hasher.update(&source.as_bytes()[cursor..declaration.span.end]);
    reserve_builder_structure(71)?;
    Ok(crate::bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )))
}
