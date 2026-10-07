//! Default source execution follows the verified declaration named main.
//! Automatic entry identities are permitted here only; the explicit-ID
//! interpretation and retained-call profiles keep their existing admission.

use super::{Diagnostic, Interpretation, InterpreterOptions, SourceProfile};
use std::path::Path;

pub fn interpret(
    source_path: &Path,
    entry_id: &str,
    options: &InterpreterOptions,
) -> Result<Interpretation, Vec<Diagnostic>> {
    execute(source_path, entry_id, options, SourceProfile::Legacy)
}

pub fn interpret_internal_strings(
    source_path: &Path,
    entry_id: &str,
    options: &InterpreterOptions,
) -> Result<Interpretation, Vec<Diagnostic>> {
    execute(
        source_path,
        entry_id,
        options,
        SourceProfile::InternalStrings,
    )
}

fn execute(
    source_path: &Path,
    entry_id: &str,
    options: &InterpreterOptions,
    profile: SourceProfile,
) -> Result<Interpretation, Vec<Diagnostic>> {
    InterpreterOptions::new(options.max_bytes, options.max_steps).map_err(|error| vec![error])?;
    let path = source_path.to_path_buf();
    let entry = entry_id.to_owned();
    let options = *options;
    let thread = std::thread::Builder::new()
        .name("semaprax-source-entry".to_owned())
        .stack_size(super::EVALUATION_STACK_BYTES)
        .spawn(move || {
            super::interpret_on_current_thread(&path, &entry, &[], &options, profile, true)
        })
        .map_err(|error| {
            vec![super::guard_error(&format!(
                "source entry evaluation thread failed: {error}"
            ))]
        })?;
    thread.join().unwrap_or_else(|_| {
        Err(vec![super::guard_error(
            "source entry evaluation thread panicked",
        )])
    })
}

pub(super) fn admit_automatic(function: &crate::ast::Function, allow_automatic_main: bool) -> bool {
    allow_automatic_main && function.name == "main" && !function.explicit_id
}

pub(super) fn include_entry<'a>(
    admitted: &mut std::collections::BTreeMap<&'a str, &'a crate::hir::ResolvedFunction>,
    entry: &'a crate::hir::ResolvedFunction,
    automatic_main: bool,
) {
    if automatic_main {
        admitted.insert(entry.id.as_str(), entry);
    }
}

pub(super) fn resolved_main(
    program: &crate::hir::ResolvedProgram,
    function: &crate::hir::ResolvedFunction,
    entry_id: &str,
) -> bool {
    function.name == "main" && function.id.as_str() == entry_id && program.entrypoint == function.id
}
