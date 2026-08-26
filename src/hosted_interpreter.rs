//! Hosted execution seam for the bounded, success-published stdout transcript.
//!
//! This is intentionally separate from `semaprax.interpret.v1`, whose closed
//! profile remains effect-free. The returned transcript is sealed only when
//! the invocation returns successfully; every failure outcome carries empty
//! bytes.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::{CommandEvaluation, ResolvedEvaluation};

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

/// Immutable, invocation-owned host input for Language Command I/O v1.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostedCommandInput {
    /// Exact arguments excluding `argv[0]`.
    pub arguments: Vec<String>,
    /// Exact arbitrary stdin bytes.
    pub stdin: Vec<u8>,
}

/// Settled hosted command result. Both transcripts are empty unless the
/// language entry returned a bool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostedCommandResult {
    pub evaluation: CommandEvaluation,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Execute the exact selected zero-argument bool command against injected
/// input. This seam has no ambient argv, stdin, stdout, stderr, filesystem, or
/// process authority.
pub fn execute_language_command(
    program: &ResolvedProgram,
    entry_id: &str,
    input: &HostedCommandInput,
    max_steps: usize,
) -> Result<HostedCommandResult, Vec<Diagnostic>> {
    let (evaluation, stdout, stderr) = crate::interpreter::evaluate_resolved_language_command(
        program,
        entry_id,
        &input.arguments,
        &input.stdin,
        max_steps,
    )?;
    Ok(HostedCommandResult {
        evaluation,
        stdout,
        stderr,
    })
}
