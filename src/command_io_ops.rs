//! Compiler-owned, capability-authenticated command I/O operations.
//!
//! These operations are not authored imports and are deliberately distinct
//! from the native-Rust callback ABI. Their complete signature, authority,
//! and failure shape is derived from the closed operation enum.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{
    OwnershipMode, ResolvedHostCommandOperation, ResolvedParam, ResolvedType, ValueId,
};

pub(crate) const ARGS_LEN_NAME: &str = "args_len";
pub(crate) const ARG_UTF8_NAME: &str = "arg_utf8";
pub(crate) const STDIN_READ_NAME: &str = "stdin_read";
pub(crate) const STDERR_WRITE_NAME: &str = "stderr_write";
pub(crate) const ARGS_LEN_ID: &str = "core.host.args-len";
pub(crate) const ARG_UTF8_ID: &str = "core.host.arg-utf8";
pub(crate) const STDIN_READ_ID: &str = "core.host.stdin-read";
pub(crate) const STDERR_WRITE_ID: &str = "core.host.stderr-write";
pub(crate) const ARGS_READ_EFFECT: &str = "process.args.read";
pub(crate) const STDIN_READ_EFFECT: &str = "process.stdin.read";
pub(crate) const STDERR_WRITE_EFFECT: &str = "process.stderr.write";
pub(crate) const STATUS_DOMAIN: &str = "semaprax.command-input.v1";
pub(crate) const MAX_ARGUMENTS: u64 = 16;
pub(crate) const MAX_INPUT_BYTES: u64 = 65_536;
pub(crate) const ARG_INDEX_OUT_OF_BOUNDS: u32 = 1;
pub(crate) const ARG_INVALID_UTF8: u32 = 2;
pub(crate) const STDIN_READ_FAILED: u32 = 3;
pub(crate) const INPUT_CAPACITY_EXCEEDED: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandIoFailure {
    Infallible,
    Status,
}

pub(crate) fn by_name(name: &str) -> Option<ResolvedHostCommandOperation> {
    match name {
        ARGS_LEN_NAME => Some(ResolvedHostCommandOperation::ArgsLen),
        ARG_UTF8_NAME => Some(ResolvedHostCommandOperation::ArgUtf8),
        STDIN_READ_NAME => Some(ResolvedHostCommandOperation::StdinRead),
        STDERR_WRITE_NAME => Some(ResolvedHostCommandOperation::StderrWrite),
        _ => None,
    }
}

pub(crate) fn by_id(id: &str) -> Option<ResolvedHostCommandOperation> {
    match id {
        ARGS_LEN_ID => Some(ResolvedHostCommandOperation::ArgsLen),
        ARG_UTF8_ID => Some(ResolvedHostCommandOperation::ArgUtf8),
        STDIN_READ_ID => Some(ResolvedHostCommandOperation::StdinRead),
        STDERR_WRITE_ID => Some(ResolvedHostCommandOperation::StderrWrite),
        _ => None,
    }
}

pub(crate) const fn name(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::ArgsLen => ARGS_LEN_NAME,
        ResolvedHostCommandOperation::ArgUtf8 => ARG_UTF8_NAME,
        ResolvedHostCommandOperation::StdinRead => STDIN_READ_NAME,
        ResolvedHostCommandOperation::StderrWrite => STDERR_WRITE_NAME,
    }
}

pub(crate) const fn id(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::ArgsLen => ARGS_LEN_ID,
        ResolvedHostCommandOperation::ArgUtf8 => ARG_UTF8_ID,
        ResolvedHostCommandOperation::StdinRead => STDIN_READ_ID,
        ResolvedHostCommandOperation::StderrWrite => STDERR_WRITE_ID,
    }
}

pub(crate) const fn effect(op: ResolvedHostCommandOperation) -> &'static str {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::ArgUtf8 => {
            ARGS_READ_EFFECT
        }
        ResolvedHostCommandOperation::StdinRead => STDIN_READ_EFFECT,
        ResolvedHostCommandOperation::StderrWrite => STDERR_WRITE_EFFECT,
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
    }
}

pub(crate) const fn arity(op: ResolvedHostCommandOperation) -> usize {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StdinRead => 0,
        ResolvedHostCommandOperation::ArgUtf8 | ResolvedHostCommandOperation::StderrWrite => 1,
    }
}

pub(crate) const fn ast_return_type(op: ResolvedHostCommandOperation) -> Type {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => {
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
        ResolvedHostCommandOperation::ArgUtf8 => ResolvedType::Str,
        ResolvedHostCommandOperation::StdinRead => ResolvedType::Bytes,
    }
}

pub(crate) const fn result_ownership(op: ResolvedHostCommandOperation) -> OwnershipMode {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StderrWrite => {
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
        )
}

pub(crate) fn ast_params(op: ResolvedHostCommandOperation) -> Vec<Param> {
    match op {
        ResolvedHostCommandOperation::ArgsLen | ResolvedHostCommandOperation::StdinRead => {
            Vec::new()
        }
        ResolvedHostCommandOperation::ArgUtf8 | ResolvedHostCommandOperation::StderrWrite => {
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
