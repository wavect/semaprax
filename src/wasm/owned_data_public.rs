//! Target-neutral public owned-data descriptor replay before private Wasm lowering.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

pub fn emit_resolved_module_with_owned_data_exports(
    program: &ResolvedProgram,
    descriptor: &crate::project::PublicApiDescriptor,
) -> Result<Vec<u8>, Diagnostic> {
    let selected = descriptor
        .exports()
        .iter()
        .map(|export| export.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = crate::project::PublicApiSubject {
        project_schema: descriptor.project_schema(),
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    };
    let replayed = crate::project::replay_public_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed != descriptor {
        return Err(error(
            "owned-data target descriptor does not match held HIR",
        ));
    }
    let plans = super::owned_data_exports::prepare(program, descriptor)?;
    super::aggregate::emit_owned_data_exports(program, &plans)
}

pub fn emit_resolved_module_with_flat_owned_record_exports(
    program: &ResolvedProgram,
    descriptor: &crate::project::FlatOwnedRecordApiDescriptor,
) -> Result<Vec<u8>, Diagnostic> {
    let selected = descriptor
        .exports()
        .iter()
        .map(|export| export.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = crate::project::PublicApiSubject {
        project_schema: crate::project::FLAT_OWNED_RECORD_PROJECT_SCHEMA,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    };
    let replayed = crate::project::replay_flat_owned_record_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed != descriptor {
        return Err(error(
            "flat owned-record target descriptor does not match held HIR",
        ));
    }
    let plans = super::owned_data_exports::prepare_flat_records(program, descriptor)?;
    super::aggregate::emit_owned_data_exports(program, &plans)
}

pub fn emit_resolved_module_with_nested_owned_record_exports(
    program: &ResolvedProgram,
    descriptor: &crate::project::NestedOwnedRecordApiDescriptor,
) -> Result<Vec<u8>, Diagnostic> {
    let selected = descriptor
        .exports()
        .iter()
        .map(|export| export.stable_id().as_str().to_owned())
        .collect::<Vec<_>>();
    let subject = crate::project::PublicApiSubject {
        project_schema: crate::project::NESTED_OWNED_RECORD_PROJECT_SCHEMA,
        project_revision: descriptor.project_revision(),
        workspace_revision: descriptor.workspace_revision(),
        project_graph_digest: descriptor.project_graph_digest(),
    };
    let replayed = crate::project::replay_nested_owned_record_api_descriptor(
        program,
        &selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )?;
    if &replayed != descriptor {
        return Err(error(
            "nested owned-record target descriptor does not match held HIR",
        ));
    }
    let plans = super::owned_data_exports::prepare_nested_records(program, descriptor)?;
    super::aggregate::emit_owned_data_exports(program, &plans)
}

fn error(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-W124", message)
}
