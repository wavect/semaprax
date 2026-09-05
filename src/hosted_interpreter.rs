//! Hosted execution seam for the bounded, success-published stdout transcript.
//!
//! This is intentionally separate from `semaprax.interpret.v1`, whose closed
//! profile remains effect-free. The returned transcript is sealed only when
//! the invocation returns successfully; every failure outcome carries empty
//! bytes.

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::interpreter::{CommandEvaluation, ResolvedEvaluation};
use crate::network_provider::NetworkProvider;

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

/// Execute the exact selected zero-argument bool command with network
/// authority supplied by `provider`.
///
/// The module's permits must stay within the seven Language Network I/O v1
/// tokens (`network.connect`, `network.read`, `network.write`,
/// `process.args.read`, `process.stdin.read`, `process.stdout.write`,
/// `process.stderr.write`) and include at least one `network.*` token; the
/// reachable operations must satisfy the `NetworkV1` profile. Nothing here
/// grants ambient authority: the only transport is the provider the caller
/// passes, and it is settled (every connection released) before the result is
/// published, on every outcome. Both transcripts are sealed only when the
/// entry returned a bool.
pub fn execute_network_command(
    program: &ResolvedProgram,
    entry_id: &str,
    input: &HostedCommandInput,
    provider: &mut dyn NetworkProvider,
    max_steps: usize,
) -> Result<HostedCommandResult, Diagnostic> {
    let (evaluation, stdout, stderr) =
        crate::interpreter::network::command::evaluate_resolved_network_command(
            program,
            entry_id,
            &input.arguments,
            &input.stdin,
            provider,
            max_steps,
        )?;
    Ok(HostedCommandResult {
        evaluation,
        stdout,
        stderr,
    })
}
