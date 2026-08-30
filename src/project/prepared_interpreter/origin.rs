use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::interpreter::{self, PreparedResolvedI64};

use super::super::ProjectRevision;
use super::model::{preparation_diagnostics, prepare_error, MIN_PROJECT_SOURCE_TRACE_BYTES};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionOrigin {
    pub(super) path: String,
    pub(super) source_revision: String,
    pub(super) source_digest: String,
    pub(super) source_bytes: usize,
}

pub(super) struct PreparedClosures {
    pub(super) entry: PreparedResolvedI64,
    pub(super) test: PreparedResolvedI64,
    pub(super) origins: BTreeMap<String, FunctionOrigin>,
}

pub(super) fn prepare_closures(
    revision: &ProjectRevision,
) -> Result<PreparedClosures, Vec<Diagnostic>> {
    let entry = interpreter::prepare_resolved_zero_arg_i64(
        revision.entry_program(),
        revision.entry_program().entrypoint.as_str(),
    )
    .map_err(preparation_diagnostics)?;
    let test = interpreter::prepare_resolved_zero_arg_i64(
        revision.test_program(),
        revision.test_program().entrypoint.as_str(),
    )
    .map_err(preparation_diagnostics)?;
    let nodes = entry
        .origin_nodes()
        .checked_add(test.origin_nodes())
        .ok_or_else(|| vec![prepare_error("prepared node accounting overflowed")])?;
    let mut bytes = entry
        .index_bytes()
        .checked_add(test.index_bytes())
        .ok_or_else(|| vec![prepare_error("prepared index accounting overflowed")])?;
    if nodes > interpreter::MAX_PREPARED_ORIGIN_NODES {
        return Err(vec![prepare_error(
            "combined entry/test origin-node bound exceeded",
        )]);
    }
    let mut origins = BTreeMap::new();
    for id in entry.function_ids().chain(test.function_ids()) {
        let semantic = revision.semantic.rename_function(id).ok_or_else(|| {
            vec![prepare_error(
                "prepared function has no Phase-A source identity",
            )]
        })?;
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == semantic.path)
            .ok_or_else(|| vec![prepare_error("prepared source path is absent")])?;
        let origin = FunctionOrigin {
            path: source.path().to_owned(),
            source_revision: source.source_revision().to_owned(),
            source_digest: source.source_digest().to_owned(),
            source_bytes: source.source().len(),
        };
        insert_origin(&mut origins, id, origin, &mut bytes)?;
    }
    if bytes > interpreter::MAX_PREPARED_INDEX_BYTES {
        return Err(vec![prepare_error(
            "combined prepared index byte bound exceeded",
        )]);
    }
    validate_origin_spans(revision.entry_program(), &entry, &origins)?;
    validate_origin_spans(revision.test_program(), &test, &origins)?;
    Ok(PreparedClosures {
        entry,
        test,
        origins,
    })
}

pub(super) fn insert_origin(
    origins: &mut BTreeMap<String, FunctionOrigin>,
    id: &str,
    origin: FunctionOrigin,
    bytes: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(previous) = origins.get(id) {
        if previous != &origin {
            return Err(vec![prepare_error(
                "duplicate prepared function source-origin facts disagree",
            )]);
        }
        return Ok(());
    }
    *bytes = bytes
        .checked_add(id.len())
        .and_then(|value| value.checked_add(origin.path.len()))
        .and_then(|value| value.checked_add(origin.source_revision.len()))
        .and_then(|value| value.checked_add(origin.source_digest.len()))
        .ok_or_else(|| vec![prepare_error("prepared source index accounting overflowed")])?;
    origins.insert(id.to_owned(), origin);
    Ok(())
}

fn validate_origin_spans(
    program: &crate::hir::ResolvedProgram,
    prepared: &PreparedResolvedI64,
    origins: &BTreeMap<String, FunctionOrigin>,
) -> Result<(), Vec<Diagnostic>> {
    for id in prepared.function_ids() {
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .ok_or_else(|| vec![prepare_error("prepared function index drifted")])?;
        let origin = origins
            .get(id)
            .ok_or_else(|| vec![prepare_error("prepared source origin is absent")])?;
        let mut expressions = function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
            .collect::<Vec<_>>();
        while let Some(expression) = expressions.pop() {
            if expression.span.start > expression.span.end
                || expression.span.end > origin.source_bytes
            {
                return Err(vec![prepare_error(
                    "prepared expression span is outside its authenticated source",
                )]);
            }
            let fact_bytes = id
                .len()
                .checked_add(expression.id.as_str().len())
                .and_then(|value| value.checked_add(origin.path.len()))
                .and_then(|value| value.checked_add(origin.source_revision.len()))
                .and_then(|value| value.checked_add(origin.source_digest.len()))
                .ok_or_else(|| vec![prepare_error("prepared origin fact overflowed")])?;
            if fact_bytes > MIN_PROJECT_SOURCE_TRACE_BYTES / 2 {
                return Err(vec![prepare_error(
                    "one prepared source-origin fact cannot fit the minimum trace envelope",
                )]);
            }
            expressions.extend(interpreter::trace_child_expressions(expression));
        }
    }
    Ok(())
}
