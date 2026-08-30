//! Opt-in internal String calls with the unchanged scalar/borrowed source boundary.
//!
//! This profile is separate from Interpreter v1 and every Project evaluator.
//! Fuel and output limits do not bound String heap allocation. Replay checks
//! canonical facts and source binding, not execution or provenance.

use super::{Diagnostic, Interpretation, InterpreterOptions, SourceProfile};
use crate::hir::{self, ResolvedFunction, ResolvedType};
use std::path::Path;

#[cfg(test)]
mod tests;
mod wire;

pub const SCHEMA: &str = "semaprax.interpret.internal-strings.v1";
pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ENVELOPE_BYTES: usize = 16 * 1024 * 1024;
pub(super) const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.interpret.internal-strings.payload.v1\0";

/// Evaluate a scalar/borrowed boundary with additional internal String calls.
pub fn interpret(
    source_path: &Path,
    function_token: &str,
    arguments: &[String],
    options: &InterpreterOptions,
) -> Result<Interpretation, Vec<Diagnostic>> {
    super::interpret_with_profile(
        source_path,
        function_token,
        arguments,
        options,
        SourceProfile::InternalStrings,
    )
}

/// Replay only this profile's bounded, canonically encoded envelope.
pub fn verify_envelope(envelope: &str) -> Result<(), Diagnostic> {
    if envelope.len() > MAX_ENVELOPE_BYTES {
        return Err(super::consistency_error(
            "internal String envelope exceeds 16 MiB".to_owned(),
        ));
    }
    super::verify_envelope_with_profile(envelope, SourceProfile::InternalStrings)
}

/// Replay the envelope and compare its digest with a bounded regular-file snapshot.
pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<(), Diagnostic> {
    verify_envelope(envelope)?;
    let canonical = crate::patch::canonical_source_path(source_path).map_err(|_| {
        super::consistency_error("cannot resolve internal String source".to_owned())
    })?;
    let snapshot =
        crate::patch::read_source_snapshot_bounded(&canonical, MAX_SOURCE_BYTES, "SPX-F106")
            .map_err(|_| {
                super::consistency_error(
                    "cannot read bounded internal String source snapshot".to_owned(),
                )
            })?;
    if super::bound_source_digest(envelope)? != super::source_digest(snapshot.source()) {
        return Err(super::consistency_error(
            "internal String source digest does not match current source bytes".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn signature_is_admitted(
    function: &ResolvedFunction,
    declarations: &hir::DeclarationIndex,
) -> bool {
    function.params.iter().all(|parameter| {
        // The resolver classifies an ordinary by-value String parameter as
        // Own. Admission consumes validated HIR, not its source spelling.
        (parameter.ty == ResolvedType::String && parameter.ownership == hir::OwnershipMode::Own)
            || super::resolved_data_parameter_is_admitted(
                &parameter.ty,
                parameter.ownership,
                declarations,
            )
    }) && (function.return_type == ResolvedType::String
        || super::resolved_data_result_is_admitted(&function.return_type, declarations))
}

pub(super) use wire::verify_canonical;
