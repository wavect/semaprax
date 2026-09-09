//! Closed operations over one explicitly supplied immutable environment snapshot.
use crate::ast::{Param, ParamMode, Span, Type};
use crate::hir::{OwnershipMode, ResolvedHostCommandOperation as Op, ResolvedType};
pub(crate) const EFFECT: &str = "process.environment.read";
pub(crate) const STATUS_DOMAIN: &str = "semaprax.environment-input.v1";
pub(crate) const INDEX_OUT_OF_BOUNDS: u32 = 1;
pub(crate) const INVALID_INPUT: u32 = 2;
pub(crate) const CAPACITY_EXCEEDED: u32 = 3;
pub(crate) const AUTHORITY_DENIED: u32 = 4;
pub(crate) const STATUS_CODES: [u32; 4] = [
    INDEX_OUT_OF_BOUNDS,
    INVALID_INPUT,
    CAPACITY_EXCEEDED,
    AUTHORITY_DENIED,
];
pub(crate) const MAX_ENTRIES: u64 = 256;
pub(crate) const MAX_INPUT_BYTES: u64 = 65_536;
pub(crate) const ARENA_ID: &str = "core.host.environment-arena";
pub(crate) const OPERATIONS: [Op; 3] = [Op::EnvLen, Op::EnvNameUtf8, Op::EnvValueUtf8];
pub(crate) const fn is_environment(op: Op) -> bool {
    matches!(op, Op::EnvLen | Op::EnvNameUtf8 | Op::EnvValueUtf8)
}
pub(crate) const fn is_lookup(op: Op) -> bool {
    matches!(op, Op::EnvNameUtf8 | Op::EnvValueUtf8)
}
pub(crate) fn by_name(value: &str) -> Option<Op> {
    OPERATIONS.into_iter().find(|op| name(*op) == value)
}
pub(crate) fn by_id(value: &str) -> Option<Op> {
    OPERATIONS.into_iter().find(|op| id(*op) == value)
}
pub(crate) const fn name(op: Op) -> &'static str {
    match op {
        Op::EnvLen => "env_len",
        Op::EnvNameUtf8 => "env_name_utf8",
        Op::EnvValueUtf8 => "env_value_utf8",
        _ => panic!("not an environment operation"),
    }
}
pub(crate) const fn id(op: Op) -> &'static str {
    match op {
        Op::EnvLen => "core.host.env-len",
        Op::EnvNameUtf8 => "core.host.env-name-utf8",
        Op::EnvValueUtf8 => "core.host.env-value-utf8",
        _ => panic!("not an environment operation"),
    }
}
pub(crate) const fn arity(op: Op) -> usize {
    if is_lookup(op) {
        1
    } else {
        0
    }
}
pub(crate) const fn ast_return_type(op: Op) -> Type {
    if is_lookup(op) {
        Type::Str
    } else {
        Type::Usize
    }
}
pub(crate) const fn return_type(op: Op) -> ResolvedType {
    if is_lookup(op) {
        ResolvedType::Str
    } else {
        ResolvedType::Usize
    }
}
pub(crate) const fn result_ownership(op: Op) -> OwnershipMode {
    if is_lookup(op) {
        OwnershipMode::Borrow
    } else {
        OwnershipMode::Value
    }
}
pub(crate) fn accepts_ast(op: Op, index: usize, ty: &Type) -> bool {
    is_lookup(op) && index == 0 && *ty == Type::Usize
}
pub(crate) fn accepts_resolved(op: Op, index: usize, ty: &ResolvedType) -> bool {
    is_lookup(op) && index == 0 && *ty == ResolvedType::Usize
}
pub(crate) fn ast_params(op: Op) -> Vec<Param> {
    if is_lookup(op) {
        vec![Param {
            name: "index".to_owned(),
            mode: ParamMode::Value,
            ty: Type::Usize,
            span: Span::default(),
        }]
    } else {
        vec![]
    }
}

/// Actual checked operations select the additive semantics, never a permit alone.
pub(crate) fn program_uses_environment(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            let mut found = false;
            crate::hir::function_value::walk(function, |expression| {
                if let crate::hir::ResolvedExprKind::HostCommandCall(call) = &expression.kind {
                    found |= is_environment(call.operation);
                }
            });
            found
        })
}
