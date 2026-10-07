//! Single-file command-line programs (`docs/TEXT-TOOLKIT-V1.md`).
//!
//! A single `.spx` file whose module permits are a nonempty subset of
//! [`EFFECTS`] other than exactly `process.stdout.write` is a command-line
//! program. `semaprax run` and `semaprax build --target native` give it the
//! arguments after `--` (or the built program's argv), stage stdout and
//! stderr until `main` settles, grant `file_read_text` read-only access below
//! the invocation directory when it permits `fs.read`, and use `main`'s `i64`
//! result as the process exit status. No other authority is reachable.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedHostCommandOperation, ResolvedProgram};

/// The sorted authority a command-line program may permit.
pub(crate) const EFFECTS: [&str; 4] = [
    crate::string_ops::FILE_READ_TEXT_EFFECT,
    crate::command_io_ops::ARGS_READ_EFFECT,
    crate::command_io_ops::STDERR_WRITE_EFFECT,
    crate::host_io_ops::STDOUT_WRITE_EFFECT,
];

/// The exit status range `main` may return.
pub const EXIT_STATUS_RANGE: std::ops::RangeInclusive<i64> = 0..=255;

/// Whether module permits select the command-line profile.
pub fn selects(permits: &[String]) -> bool {
    !permits.is_empty()
        && permits != [crate::host_io_ops::STDOUT_WRITE_EFFECT]
        && permits
            .iter()
            .all(|effect| EFFECTS.contains(&effect.as_str()))
}

/// Authenticate the complete authority of a command-line program before any
/// hosted or native work: permits, per-function effects, and every reachable
/// host operation.
pub(crate) fn validate_authority(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if !selects(&program.permits) {
        return Err(authority_error(
            "module permits must be a subset of `fs.read`, `process.args.read`, `process.stderr.write`, and `process.stdout.write`",
        ));
    }
    if !program.interfaces.is_empty() {
        return Err(authority_error("interfaces and imports are not admitted"));
    }
    let functions = program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    );
    for function in functions {
        if let Some(effect) = function
            .effects
            .iter()
            .find(|effect| !program.permits.contains(effect))
        {
            return Err(authority_error(format!(
                "function `{}` uses `{effect}` outside the module permits",
                function.id
            )));
        }
        let mut pending: Vec<&ResolvedExpr> = vec![&function.body];
        pending.extend(function.requires.iter().chain(&function.ensures));
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::HostCommandCall(call) = &expression.kind {
                if !matches!(
                    call.operation,
                    ResolvedHostCommandOperation::ArgsLen
                        | ResolvedHostCommandOperation::ArgUtf8
                        | ResolvedHostCommandOperation::StderrWrite
                ) {
                    return Err(authority_error(format!(
                        "`{}` is outside the command-line profile; read files with `file_read_text`",
                        crate::command_io_ops::name(call.operation)
                    )));
                }
            }
            crate::hir::push_resolved_expression_children_in_authored_order(
                expression,
                &mut pending,
            );
        }
    }
    Ok(())
}

fn authority_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-T273",
        format!(
            "single-file command-line program authority mismatch: {}",
            message.into()
        ),
    )
}
