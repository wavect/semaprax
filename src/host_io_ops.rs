//! Compiler-owned, capability-authenticated host I/O operations.

use crate::ast::{Param, ParamMode, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    IdentityOrigin, OwnershipMode, ResolvedParam, ResolvedProgram, ResolvedType, ValueId,
};

pub(crate) const STDOUT_WRITE_NAME: &str = "stdout_write";
pub(crate) const STDOUT_WRITE_ID: &str = "core.host.stdout-write";
pub(crate) const STDOUT_WRITE_EFFECT: &str = "process.stdout.write";
pub(crate) const MAX_STDOUT_TRANSCRIPT_BYTES: u64 =
    crate::byte_data_capacity::MAX_STDOUT_TRANSCRIPT_BYTES;
pub(crate) const MAX_STDOUT_WRITES_PER_PATH: u64 =
    crate::byte_data_capacity::MAX_STDOUT_WRITES_PER_PATH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostIoOp {
    StdoutWrite,
}

impl HostIoOp {
    pub(crate) const fn name(self) -> &'static str {
        STDOUT_WRITE_NAME
    }

    pub(crate) const fn id(self) -> &'static str {
        STDOUT_WRITE_ID
    }

    pub(crate) const fn effect(self) -> &'static str {
        STDOUT_WRITE_EFFECT
    }

    pub(crate) const fn arity(self) -> usize {
        1
    }

    pub(crate) fn return_type(self) -> ResolvedType {
        ResolvedType::Usize
    }

    pub(crate) fn ast_return_type(self) -> Type {
        Type::Usize
    }

    pub(crate) fn accepts_resolved(self, index: usize, ty: &ResolvedType) -> bool {
        index == 0 && *ty == ResolvedType::SliceU8
    }

    pub(crate) fn accepts_ast(self, index: usize, ty: &Type) -> bool {
        index == 0 && *ty == Type::SliceU8
    }
}

pub(crate) fn by_name(name: &str) -> Option<HostIoOp> {
    (name == STDOUT_WRITE_NAME).then_some(HostIoOp::StdoutWrite)
}

pub(crate) fn by_id(id: &str) -> Option<HostIoOp> {
    (id == STDOUT_WRITE_ID).then_some(HostIoOp::StdoutWrite)
}

pub(crate) fn ast_params(_op: HostIoOp) -> Vec<Param> {
    vec![Param {
        name: "value".to_owned(),
        mode: ParamMode::Borrow,
        ty: Type::SliceU8,
        span: Span::default(),
    }]
}

pub(crate) fn resolved_params(op: HostIoOp) -> Vec<ResolvedParam> {
    vec![ResolvedParam {
        id: ValueId::intrinsic_parameter(op.id(), 0),
        name: "value".to_owned(),
        ownership: OwnershipMode::Borrow,
        ty: ResolvedType::SliceU8,
        span: Span::default(),
    }]
}

/// Authenticate the complete authority inventory of a standalone stdout
/// transcript profile before any hosted or target-specific work begins.
///
/// These emitters materialize the complete resolved function inventory, so an
/// unrelated function with wider effects is still outside the profile even if
/// the selected root cannot call it.
pub(crate) fn validate_stdout_profile_authority(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    if program.permits != [STDOUT_WRITE_EFFECT] {
        return Err(profile_authority_error(
            "module permits must be exactly `process.stdout.write`",
        ));
    }
    if !program.interfaces.is_empty() {
        return Err(profile_authority_error(
            "interfaces and imports are not admitted",
        ));
    }
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(profile_authority_error(
            "generic function templates and instances are not admitted",
        ));
    }
    if program.types.iter().any(|declaration| {
        program
            .declarations
            .declaration(&declaration.id)
            .is_none_or(|indexed| indexed.identity_origin != IdentityOrigin::CompilerOwned)
    }) {
        return Err(profile_authority_error(
            "authored aggregate and resource declarations are not admitted",
        ));
    }
    for function in &program.functions {
        if !function.effects.is_empty() && function.effects != [STDOUT_WRITE_EFFECT] {
            return Err(profile_authority_error(format!(
                "function `{}` has authority outside `process.stdout.write`",
                function.id
            )));
        }
    }
    Ok(())
}

fn profile_authority_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-T269",
        format!(
            "Bounded Stdout Transcript v1 authority mismatch: {}",
            message.into()
        ),
    )
}
