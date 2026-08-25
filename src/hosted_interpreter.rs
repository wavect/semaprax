//! Hosted execution seam for the bounded, success-published stdout transcript.
//!
//! This is intentionally separate from `semaprax.interpret.v1`, whose closed
//! profile remains effect-free. The returned transcript is sealed only when
//! the invocation returns successfully; every failure outcome carries empty
//! bytes.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::ResolvedEvaluation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedStdoutTranscript {
    pub evaluation: ResolvedEvaluation,
    pub transcript: Vec<u8>,
}

pub fn execute_stdout_transcript(
    program: &ResolvedProgram,
    entry_id: &str,
    max_steps: usize,
) -> Result<HostedStdoutTranscript, Vec<Diagnostic>> {
    crate::host_io_ops::validate_stdout_profile_authority(program)
        .map_err(|diagnostic| vec![diagnostic])?;
    let (evaluation, transcript) =
        crate::interpreter::evaluate_resolved_stdout_transcript(program, entry_id, max_steps)?;
    Ok(HostedStdoutTranscript {
        evaluation,
        transcript,
    })
}
