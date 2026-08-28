//! Compiler-owned, capability-authenticated command I/O operations.
//!
//! These operations are not authored imports and are deliberately distinct
//! from the native-Rust callback ABI. Their complete signature, authority,
//! and failure shape is derived from the closed operation enum.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Param, ParamMode, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExprKind, ResolvedHostCommandOperation,
    ResolvedParam, ResolvedProgram, ResolvedType, ValueId,
};

pub(crate) const ARGS_LEN_NAME: &str = "args_len";
pub(crate) const ARG_UTF8_NAME: &str = "arg_utf8";
pub(crate) const STDIN_READ_NAME: &str = "stdin_read";
pub(crate) const STDERR_WRITE_NAME: &str = "stderr_write";
pub(crate) const STDOUT_APPEND_NAME: &str = "stdout_append";
pub(crate) const STDERR_APPEND_NAME: &str = "stderr_append";
pub(crate) const ARGS_LEN_ID: &str = "core.host.args-len";
pub(crate) const ARG_UTF8_ID: &str = "core.host.arg-utf8";
pub(crate) const STDIN_READ_ID: &str = "core.host.stdin-read";
pub(crate) const STDERR_WRITE_ID: &str = "core.host.stderr-write";
pub(crate) const STDOUT_APPEND_ID: &str = "core.host.stdout-append";
pub(crate) const STDERR_APPEND_ID: &str = "core.host.stderr-append";
pub(crate) const ARGS_READ_EFFECT: &str = "process.args.read";
pub(crate) const STDIN_READ_EFFECT: &str = "process.stdin.read";
pub(crate) const STDERR_WRITE_EFFECT: &str = "process.stderr.write";
pub(crate) const STDOUT_WRITE_EFFECT: &str = "process.stdout.write";
pub(crate) const INPUT_STATUS_DOMAIN: &str = "semaprax.command-input.v1";
/// Compatibility alias for the original command-input-only operation set.
pub(crate) const STATUS_DOMAIN: &str = INPUT_STATUS_DOMAIN;
pub(crate) const OUTPUT_STATUS_DOMAIN: &str = "semaprax.command-output.v1";
pub(crate) const MAX_ARGUMENTS: u64 = 16;
pub(crate) const MAX_INPUT_BYTES: u64 = 65_536;
/// Exact cumulative stdout-plus-stderr transcript bound for append operations.
pub(crate) const MAX_OUTPUT_BYTES: u64 = 65_536;
pub(crate) const ARG_INDEX_OUT_OF_BOUNDS: u32 = 1;
pub(crate) const ARG_INVALID_UTF8: u32 = 2;
pub(crate) const STDIN_READ_FAILED: u32 = 3;
pub(crate) const INPUT_CAPACITY_EXCEEDED: u32 = 4;
pub(crate) const OUTPUT_CAPACITY_EXCEEDED: u32 = 1;

/// The two closed command-operation profiles. Keeping this target-neutral is
/// what prevents a backend from inferring a newer carrier from arbitrary HIR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandOperationProfile {
    LanguageV1,
    LineV1,
}

/// Validate exactly the operations reachable from the selected command.
/// Disconnected declarations remain irrelevant, while every backend and the
/// workspace linker share the same version boundary.
pub(crate) fn validate_operation_profile(
    program: &ResolvedProgram,
    command: &DeclarationId,
    profile: CommandOperationProfile,
) -> Result<(), Diagnostic> {
    let available = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let available_instances = program
        .function_instances
        .iter()
        .map(|instance| (instance.id.clone(), &instance.function))
        .collect::<BTreeMap<_, _>>();
    let command_function = available
        .get(command)
        .copied()
        .ok_or_else(|| profile_error("selected command is absent"))?;
    let mut pending_functions = vec![(format!("m:{command}"), command_function)];
    let mut visited = BTreeSet::new();
    let mut saw_range = false;
    let mut saw_append = false;
    let mut saw_legacy_write = false;

    while let Some((execution_id, function)) = pending_functions.pop() {
        if !visited.insert(execution_id) {
            continue;
        }
        let mut expressions =
            Vec::with_capacity(function.requires.len() + function.ensures.len() + 1);
        expressions.extend(function.ensures.iter().rev());
        expressions.push(&function.body);
        expressions.extend(function.requires.iter().rev());
        while let Some(expression) = expressions.pop() {
            match &expression.kind {
                ResolvedExprKind::ByteRange { .. } => saw_range = true,
                ResolvedExprKind::HostCommandCall(call) => match call.operation {
                    ResolvedHostCommandOperation::StdoutAppend
                    | ResolvedHostCommandOperation::StderrAppend => saw_append = true,
                    ResolvedHostCommandOperation::StderrWrite => saw_legacy_write = true,
                    ResolvedHostCommandOperation::ArgsLen
                    | ResolvedHostCommandOperation::ArgUtf8
                    | ResolvedHostCommandOperation::StdinRead => {}
                },
                ResolvedExprKind::Call {
                    callee, instance, ..
                } => {
                    if callee.as_str() == crate::host_io_ops::STDOUT_WRITE_ID {
                        saw_legacy_write = true;
                    } else if let Some(instance) = instance {
                        let target =
                            available_instances.get(instance).copied().ok_or_else(|| {
                                profile_error("selected command closure names an absent instance")
                            })?;
                        pending_functions.push((format!("g:{instance}"), target));
                    } else if let Some(target) = available.get(callee).copied() {
                        pending_functions.push((format!("m:{callee}"), target));
                    }
                }
                _ => {}
            }
            hir::push_resolved_expression_children_in_authored_order(expression, &mut expressions);
        }
    }

    match profile {
        CommandOperationProfile::LanguageV1 if saw_range || saw_append => Err(profile_error(
            "Language Command I/O v1 cannot reach byte_range, stdout_append, or stderr_append",
        )),
        CommandOperationProfile::LineV1 if saw_legacy_write => Err(profile_error(
            "Line Command I/O v1 cannot mix legacy transcript writes with append operations",
        )),
        CommandOperationProfile::LineV1 if !saw_range || !saw_append => Err(profile_error(
            "Line Command I/O v1 must reach byte_range and stdout_append or stderr_append",
        )),
        CommandOperationProfile::LanguageV1 | CommandOperationProfile::LineV1 => Ok(()),
    }
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-W114", message)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandIoFailure {
    Infallible,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandIoStatusMetadata {
    pub domain: &'static str,
    pub codes: &'static [u32],
}

pub(crate) fn by_name(name: &str) -> Option<ResolvedHostCommandOperation> {
    match name {
        ARGS_LEN_NAME => Some(ResolvedHostCommandOperation::ArgsLen),
        ARG_UTF8_NAME => Some(ResolvedHostCommandOperation::ArgUtf8),
        STDIN_READ_NAME => Some(ResolvedHostCommandOperation::StdinRead),
        STDERR_WRITE_NAME => Some(ResolvedHostCommandOperation::StderrWrite),
        STDOUT_APPEND_NAME => Some(ResolvedHostCommandOperation::StdoutAppend),
        STDERR_APPEND_NAME => Some(ResolvedHostCommandOperation::StderrAppend),
        _ => None,
    }
}

pub(crate) fn by_id(id: &str) -> Option<ResolvedHostCommandOperation> {
    match id {
        ARGS_LEN_ID => Some(ResolvedHostCommandOperation::ArgsLen),
        ARG_UTF8_ID => Some(ResolvedHostCommandOperation::ArgUtf8),
        STDIN_READ_ID => Some(ResolvedHostCommandOperation::StdinRead),
        STDERR_WRITE_ID => Some(ResolvedHostCommandOperation::StderrWrite),
        STDOUT_APPEND_ID => Some(ResolvedHostCommandOperation::StdoutAppend),
        STDERR_APPEND_ID => Some(ResolvedHostCommandOperation::StderrAppend),
        _ => None,
    }
}

pub(crate) const fn name(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::ArgsLen => ARGS_LEN_NAME,
        ResolvedHostCommandOperation::ArgUtf8 => ARG_UTF8_NAME,
        ResolvedHostCommandOperation::StdinRead => STDIN_READ_NAME,
        ResolvedHostCommandOperation::StderrWrite => STDERR_WRITE_NAME,
        ResolvedHostCommandOperation::StdoutAppend => STDOUT_APPEND_NAME,
        ResolvedHostCommandOperation::StderrAppend => STDERR_APPEND_NAME,
    }
}

pub(crate) const fn id(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::ArgsLen => ARGS_LEN_ID,
        ResolvedHostCommandOperation::ArgUtf8 => ARG_UTF8_ID,
        ResolvedHostCommandOperation::StdinRead => STDIN_READ_ID,
        ResolvedHostCommandOperation::StderrWrite => STDERR_WRITE_ID,
        ResolvedHostCommandOperation::StdoutAppend => STDOUT_APPEND_ID,
        ResolvedHostCommandOperation::StderrAppend => STDERR_APPEND_ID,
    }
}

pub(crate) const fn effect(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::ArgUtf8 => {
            ARGS_READ_EFFECT
        }
        ResolvedHostCommandOperation::StdinRead => STDIN_READ_EFFECT,
        ResolvedHostCommandOperation::StderrWrite => STDERR_WRITE_EFFECT,
        ResolvedHostCommandOperation::StdoutAppend => STDOUT_WRITE_EFFECT,
        ResolvedHostCommandOperation::StderrAppend => STDERR_WRITE_EFFECT,
    }
}

pub(crate) const fn failure(op: ResolvedHostCommandOperation) -> CommandIoFailure {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => {
            CommandIoFailure::Infallible
        }
        ResolvedHostCommandOperation::ArgUtf8 | ResolvedHostCommandOperation::StdinRead => {
            CommandIoFailure::Status
        }
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            CommandIoFailure::Status
        }
    }
}

/// The complete normalized status space for a fallible command operation.
/// Keeping this separate from `failure` preserves the existing coarse control
/// classification while making domain/code validation exact.
pub(crate) const fn status_metadata(
    op: ResolvedHostCommandOperation,
) -> Option<CommandIoStatusMetadata> {
    match op {
        ResolvedHostCommandOperation::ArgUtf8 => Some(CommandIoStatusMetadata {
            domain: INPUT_STATUS_DOMAIN,
            codes: &[ARG_INDEX_OUT_OF_BOUNDS, ARG_INVALID_UTF8],
        }),
        ResolvedHostCommandOperation::StdinRead => Some(CommandIoStatusMetadata {
            domain: INPUT_STATUS_DOMAIN,
            codes: &[STDIN_READ_FAILED, INPUT_CAPACITY_EXCEEDED],
        }),
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            Some(CommandIoStatusMetadata {
                domain: OUTPUT_STATUS_DOMAIN,
                codes: &[OUTPUT_CAPACITY_EXCEEDED],
            })
        }
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => None,
    }
}

pub(crate) const fn arity(op: ResolvedHostCommandOperation) -> usize {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StdinRead => 0,
        ResolvedHostCommandOperation::ArgUtf8 | ResolvedHostCommandOperation::StderrWrite => 1,
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            1
        }
    }
}

pub(crate) const fn ast_return_type(op: ResolvedHostCommandOperation) -> Type {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => {
            Type::Usize
        }
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            Type::Usize
        }
        ResolvedHostCommandOperation::ArgUtf8 => Type::Str,
        ResolvedHostCommandOperation::StdinRead => Type::Bytes,
    }
}

pub(crate) const fn return_type(op: ResolvedHostCommandOperation) -> ResolvedType {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => {
            ResolvedType::Usize
        }
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            ResolvedType::Usize
        }
        ResolvedHostCommandOperation::ArgUtf8 => ResolvedType::Str,
        ResolvedHostCommandOperation::StdinRead => ResolvedType::Bytes,
    }
}

pub(crate) const fn result_ownership(op: ResolvedHostCommandOperation) -> OwnershipMode {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => {
            OwnershipMode::Value
        }
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            OwnershipMode::Value
        }
        ResolvedHostCommandOperation::ArgUtf8 => OwnershipMode::Borrow,
        ResolvedHostCommandOperation::StdinRead => OwnershipMode::Own,
    }
}

pub(crate) fn accepts_ast(op: ResolvedHostCommandOperation, index: usize, ty: &Type) -> bool {
    index == 0
        && matches!(
            (op, ty),
            (ResolvedHostCommandOperation::ArgUtf8, Type::Usize)
                | (ResolvedHostCommandOperation::StderrWrite, Type::SliceU8)
                | (ResolvedHostCommandOperation::StdoutAppend, Type::SliceU8)
                | (ResolvedHostCommandOperation::StderrAppend, Type::SliceU8)
        )
}

pub(crate) fn accepts_resolved(
    op: ResolvedHostCommandOperation,
    index: usize,
    ty: &ResolvedType,
) -> bool {
    index == 0
        && matches!(
            (op, ty),
            (ResolvedHostCommandOperation::ArgUtf8, ResolvedType::Usize)
                | (
                    ResolvedHostCommandOperation::StderrWrite,
                    ResolvedType::SliceU8
                )
                | (
                    ResolvedHostCommandOperation::StdoutAppend,
                    ResolvedType::SliceU8
                )
                | (
                    ResolvedHostCommandOperation::StderrAppend,
                    ResolvedType::SliceU8
                )
        )
}

pub(crate) fn ast_params(op: ResolvedHostCommandOperation) -> Vec<Param> {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StdinRead => {
            Vec::new()
        }
        ResolvedHostCommandOperation::ArgUtf8
        | ResolvedHostCommandOperation::StderrWrite
        | ResolvedHostCommandOperation::StdoutAppend
        | ResolvedHostCommandOperation::StderrAppend => {
            vec![Param {
                name: if matches!(op, ResolvedHostCommandOperation::ArgUtf8) {
                    "index"
                } else {
                    "value"
                }
                .to_owned(),
                mode: if matches!(op, ResolvedHostCommandOperation::ArgUtf8) {
                    ParamMode::Value
                } else {
                    ParamMode::Borrow
                },
                ty: if matches!(op, ResolvedHostCommandOperation::ArgUtf8) {
                    Type::Usize
                } else {
                    Type::SliceU8
                },
                span: Span::default(),
            }]
        }
    }
}

pub(crate) fn resolved_params(op: ResolvedHostCommandOperation) -> Vec<ResolvedParam> {
    ast_params(op)
        .into_iter()
        .enumerate()
        .map(|(index, param)| ResolvedParam {
            id: ValueId::intrinsic_parameter(id(op), index),
            name: param.name,
            ownership: param.mode.into(),
            ty: match param.ty {
                Type::Usize => ResolvedType::Usize,
                Type::SliceU8 => ResolvedType::SliceU8,
                _ => unreachable!("closed command I/O parameter table"),
            },
            span: param.span,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn resolved(source: &str) -> ResolvedProgram {
        let parsed = crate::parse(source, Path::new("command-operation-profile.spx")).unwrap();
        crate::hir::resolve(&parsed).unwrap()
    }

    #[test]
    fn append_operations_have_exact_closed_signatures_and_status_space() {
        for (op, expected_name, expected_id, expected_effect) in [
            (
                ResolvedHostCommandOperation::StdoutAppend,
                STDOUT_APPEND_NAME,
                STDOUT_APPEND_ID,
                STDOUT_WRITE_EFFECT,
            ),
            (
                ResolvedHostCommandOperation::StderrAppend,
                STDERR_APPEND_NAME,
                STDERR_APPEND_ID,
                STDERR_WRITE_EFFECT,
            ),
        ] {
            assert_eq!(by_name(expected_name), Some(op));
            assert_eq!(by_id(expected_id), Some(op));
            assert_eq!(name(op), expected_name);
            assert_eq!(id(op), expected_id);
            assert_eq!(effect(op), expected_effect);
            assert_eq!(failure(op), CommandIoFailure::Status);
            assert_eq!(arity(op), 1);
            assert_eq!(ast_return_type(op), Type::Usize);
            assert_eq!(return_type(op), ResolvedType::Usize);
            assert_eq!(result_ownership(op), OwnershipMode::Value);
            assert_eq!(MAX_OUTPUT_BYTES, 65_536);
            assert_eq!(
                status_metadata(op),
                Some(CommandIoStatusMetadata {
                    domain: OUTPUT_STATUS_DOMAIN,
                    codes: &[OUTPUT_CAPACITY_EXCEEDED],
                })
            );
            let params = resolved_params(op);
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].ownership, OwnershipMode::Borrow);
            assert_eq!(params[0].ty, ResolvedType::SliceU8);
        }
    }

    #[test]
    fn language_v1_rejects_byte_range_reached_directly_from_a_contract() {
        let program = resolved(
            r#"
module test.command_contract;
@id("command.run")
fn run(input: borrow Slice<u8>) -> bool
    requires byte_len(byte_range(input, 0usize, byte_len(input))) == byte_len(input)
{ true }
@id("main") fn main() -> i64 { 0 }
"#,
        );
        let error = validate_operation_profile(
            &program,
            &DeclarationId::new("command.run"),
            CommandOperationProfile::LanguageV1,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W114");
        assert!(error.message.contains("cannot reach byte_range"));
    }

    #[test]
    fn language_v1_follows_a_helper_reached_only_from_a_contract() {
        let program = resolved(
            r#"
module test.command_contract_helper;
@id("command.contract-helper")
fn contract_helper(input: borrow Slice<u8>) -> bool {
    byte_len(byte_range(input, 0usize, byte_len(input))) == byte_len(input)
}
@id("command.run")
fn run(input: borrow Slice<u8>) -> bool
    requires contract_helper(input)
{ true }
@id("main") fn main() -> i64 { 0 }
"#,
        );
        let error = validate_operation_profile(
            &program,
            &DeclarationId::new("command.run"),
            CommandOperationProfile::LanguageV1,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W114");
        assert!(error.message.contains("cannot reach byte_range"));
    }
}
