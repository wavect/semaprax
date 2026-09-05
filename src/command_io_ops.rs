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
    /// Bounded Language Network I/O v1: Language Command I/O plus the closed
    /// network operation family, appends, and `byte_range`; legacy
    /// single-write transcripts are excluded.
    NetworkV1,
    /// Network Service I/O v1 adds authenticated TLS clients and explicit
    /// listen/accept lifecycle operations.
    ServiceV1,
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
    let mut saw_network = false;
    let mut saw_service = false;

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
                    network if crate::network_io_ops::is_network(network) => {
                        saw_network = true;
                        saw_service |= crate::network_io_ops::is_service(network);
                    }
                    _ => unreachable!("closed host-command operation inventory"),
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
        CommandOperationProfile::LanguageV1 | CommandOperationProfile::LineV1 if saw_network => {
            Err(profile_error(
                "network operations require the Language Network I/O v1 profile",
            ))
        }
        CommandOperationProfile::NetworkV1 if !saw_network => Err(profile_error(
            "Language Network I/O v1 must reach at least one network operation",
        )),
        CommandOperationProfile::NetworkV1 if saw_legacy_write => Err(profile_error(
            "Language Network I/O v1 cannot mix legacy transcript writes with append operations",
        )),
        CommandOperationProfile::NetworkV1 if saw_service => Err(profile_error(
            "TLS and listen operations require the Network Service I/O v1 profile",
        )),
        CommandOperationProfile::ServiceV1 if !saw_service => Err(profile_error(
            "Network Service I/O v1 must reach TLS or listen operations",
        )),
        CommandOperationProfile::ServiceV1 if saw_legacy_write => Err(profile_error(
            "Network Service I/O v1 cannot mix legacy transcript writes with append operations",
        )),
        CommandOperationProfile::LanguageV1 if saw_range || saw_append => Err(profile_error(
            "Language Command I/O v1 cannot reach byte_range, stdout_append, or stderr_append",
        )),
        CommandOperationProfile::LineV1 if saw_legacy_write => Err(profile_error(
            "Line Command I/O v1 cannot mix legacy transcript writes with append operations",
        )),
        CommandOperationProfile::LineV1 if !saw_range || !saw_append => Err(profile_error(
            "Line Command I/O v1 must reach byte_range and stdout_append or stderr_append",
        )),
        CommandOperationProfile::LanguageV1
        | CommandOperationProfile::LineV1
        | CommandOperationProfile::NetworkV1
        | CommandOperationProfile::ServiceV1 => Ok(()),
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
        _ => crate::network_io_ops::by_name(name),
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
        _ => crate::network_io_ops::by_id(id),
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
        network => crate::network_io_ops::name(network),
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
        network => crate::network_io_ops::id(network),
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
        network => crate::network_io_ops::effect(network),
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
        network => {
            debug_assert!(crate::network_io_ops::is_fallible(network));
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
        service if crate::network_io_ops::is_service(service) => Some(CommandIoStatusMetadata {
            domain: crate::network_io_ops::SERVICE_STATUS_DOMAIN,
            codes: &crate::network_io_ops::SERVICE_STATUS_CODES,
        }),
        _ => Some(CommandIoStatusMetadata {
            domain: crate::network_io_ops::STATUS_DOMAIN,
            codes: &crate::network_io_ops::STATUS_CODES,
        }),
    }
}

pub(crate) const fn arity(op: ResolvedHostCommandOperation) -> usize {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StdinRead => 0,
        ResolvedHostCommandOperation::ArgUtf8 | ResolvedHostCommandOperation::StderrWrite => 1,
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            1
        }
        network => crate::network_io_ops::arity(network),
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
        network => crate::network_io_ops::ast_return_type(network),
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
        network => crate::network_io_ops::return_type(network),
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
        network => crate::network_io_ops::result_ownership(network),
    }
}

/// Every effect a call site must declare: the primary effect plus, for
/// `net_stream_stdout`, the `process.stdout.write` transcript effect.
pub(crate) fn required_effects(
    op: ResolvedHostCommandOperation,
) -> impl Iterator<Item = &'static str> {
    let secondary = if crate::network_io_ops::is_network(op) {
        crate::network_io_ops::secondary_effect(op)
    } else {
        None
    };
    std::iter::once(effect(op)).chain(secondary)
}

/// The single while-body admission rule shared by the source verifier, the
/// admission oracle, and HIR validation, so the three cannot disagree: the
/// runtime-bounded appends plus every Copy-scalar network operation. Owned
/// results (`stdin_read`, `net_recv`) and the legacy single writes stay out.
pub(crate) const fn admitted_in_while(op: ResolvedHostCommandOperation) -> bool {
    match op {
        ResolvedHostCommandOperation::StdoutAppend | ResolvedHostCommandOperation::StderrAppend => {
            true
        }
        ResolvedHostCommandOperation::ArgsLen
        | ResolvedHostCommandOperation::ArgUtf8
        | ResolvedHostCommandOperation::StdinRead
        | ResolvedHostCommandOperation::StderrWrite => false,
        network => crate::network_io_ops::admitted_in_while(network),
    }
}

pub(crate) fn accepts_ast(op: ResolvedHostCommandOperation, index: usize, ty: &Type) -> bool {
    if crate::network_io_ops::is_network(op) {
        return crate::network_io_ops::accepts_ast(op, index, ty);
    }
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
    if crate::network_io_ops::is_network(op) {
        return crate::network_io_ops::accepts_resolved(op, index, ty);
    }
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
    if crate::network_io_ops::is_network(op) {
        return crate::network_io_ops::ast_params(op);
    }
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
        _ => unreachable!("network operations return above"),
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

    const NETWORK_MODULE: &str = r#"
module test.network_profile;
permit { network.connect, network.read, network.write, process.stdout.write }
@id("net.run")
fn run() -> bool uses { network.connect, network.read, network.write, process.stdout.write } {
    let host = [49u8];
    let handle = net_connect(array_as_slice(host), 80usize);
    let sent = net_send(handle, array_as_slice(host));
    let streamed = net_stream_stdout(handle, 16usize);
    net_close(handle) == 0usize && sent + streamed > 0usize
}
@id("plain.run")
fn plain() -> bool uses { process.stdout.write } {
    let host = [49u8];
    stdout_append(array_as_slice(host)) == 1usize
}
@id("legacy.run")
fn legacy() -> bool uses { network.connect, process.stdout.write } {
    let host = [49u8];
    let handle = net_connect(array_as_slice(host), 80usize);
    stdout_write(array_as_slice(host)) == 1usize && net_close(handle) == 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;

    #[test]
    fn network_operations_belong_only_to_the_network_profile() {
        let program = resolved(NETWORK_MODULE);
        let run = DeclarationId::new("net.run");
        for profile in [
            CommandOperationProfile::LanguageV1,
            CommandOperationProfile::LineV1,
        ] {
            let error = validate_operation_profile(&program, &run, profile).unwrap_err();
            assert_eq!(error.code, "SPX-W114", "{profile:?}");
            assert!(
                error
                    .message
                    .contains("network operations require the Language Network I/O v1 profile"),
                "{profile:?}: {}",
                error.message
            );
        }
        validate_operation_profile(&program, &run, CommandOperationProfile::NetworkV1).unwrap();

        let error = validate_operation_profile(
            &program,
            &DeclarationId::new("plain.run"),
            CommandOperationProfile::NetworkV1,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W114");
        assert!(error
            .message
            .contains("must reach at least one network operation"));

        let error = validate_operation_profile(
            &program,
            &DeclarationId::new("legacy.run"),
            CommandOperationProfile::NetworkV1,
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-W114");
        assert!(error
            .message
            .contains("cannot mix legacy transcript writes"));
    }

    #[test]
    fn while_admission_and_required_effects_follow_the_operation_tables() {
        for op in crate::network_io_ops::OPERATIONS {
            assert_eq!(
                admitted_in_while(op),
                op != ResolvedHostCommandOperation::NetRecv,
                "{op:?}"
            );
            let required = required_effects(op).collect::<Vec<_>>();
            assert_eq!(required[0], effect(op));
            assert_eq!(
                required.len(),
                if op == ResolvedHostCommandOperation::NetStreamStdout {
                    2
                } else {
                    1
                },
                "{op:?}"
            );
        }
        assert_eq!(
            required_effects(ResolvedHostCommandOperation::NetStreamStdout).collect::<Vec<_>>(),
            [
                crate::network_io_ops::NETWORK_READ_EFFECT,
                STDOUT_WRITE_EFFECT
            ]
        );
        assert!(admitted_in_while(
            ResolvedHostCommandOperation::StdoutAppend
        ));
        assert!(admitted_in_while(
            ResolvedHostCommandOperation::StderrAppend
        ));
        for op in [
            ResolvedHostCommandOperation::ArgsLen,
            ResolvedHostCommandOperation::ArgUtf8,
            ResolvedHostCommandOperation::StdinRead,
            ResolvedHostCommandOperation::StderrWrite,
        ] {
            assert!(!admitted_in_while(op), "{op:?}");
            assert_eq!(required_effects(op).count(), 1, "{op:?}");
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
