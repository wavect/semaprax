//! Bounded transitive proof for ordinary same-owner byte-writing helpers.
//! Declaration spelling, attached cleanup and generated names carry no authority.
use crate::ast::program_source::ProgramSource;
use crate::ast::{Expr, ExprKind, Function, ParamMode, Type};
use crate::hir::{
    OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType,
};
mod bindings;
pub(crate) use bindings::HelperBindings;
mod module_index;
mod resolved;
pub(crate) mod validation_frontier;
pub(crate) use module_index::ModuleIndex;
mod source;
mod source_index;
#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete historical receiver retained for independent test comparisons"
)]
pub(crate) mod source_index_original;
#[cfg(test)]
pub(crate) use resolved::checked_absent_with_work_reservation;
#[cfg(test)]
pub(crate) use resolved::checked_bindings_funded;
pub(crate) use resolved::checked_bindings_funded_with_work;
pub(crate) use resolved::{
    bindings, checked_bindings, checked_bindings_with_reservation_and_work,
    nonallocating as resolved_nonallocating,
    nonallocating_with_cursor as resolved_nonallocating_with_cursor, ProofCursor,
};
pub(crate) use source::nonallocating as source_nonallocating;
pub(crate) use source_index::{SourceBacking, SourceHelpers, SourceIndex, SourceProofs};
const DEPTH: usize = crate::cache_codec::MAX_DEPTH;
const WORK: usize = crate::cache_codec::MAX_NODES;

fn byte_operation(op: crate::byte_ops::ByteOp) -> bool {
    matches!(
        op,
        crate::byte_ops::ByteOp::Len
            | crate::byte_ops::ByteOp::Get
            | crate::byte_ops::ByteOp::Range
            | crate::byte_ops::ByteOp::BytesAsSlice
            | crate::byte_ops::ByteOp::ArrayAsSlice
            | crate::byte_ops::ByteOp::StrAsBytes
            | crate::byte_ops::ByteOp::StringAsStr
            | crate::byte_ops::ByteOp::Set
            | crate::byte_ops::ByteOp::Set5
            | crate::byte_ops::ByteOp::Set1Or5
            | crate::byte_ops::ByteOp::Set1Or6Or48
    )
}
fn borrowed_text_operation(op: crate::str_ops::StrOp) -> bool {
    matches!(
        op,
        crate::str_ops::StrOp::LenBytes
            | crate::str_ops::StrOp::IsEmpty
            | crate::str_ops::StrOp::StartsWith
            | crate::str_ops::StrOp::Contains
            | crate::str_ops::StrOp::ByteAt
    )
}
fn text_operation(op: crate::string_ops::StringOp) -> bool {
    use crate::string_ops::StringOp::*;
    matches!(
        op,
        Len | IsEmpty | StartsWith | Contains | LenChars | Find | ToI64 | ByteAt | Compare
    ) || (crate::hir::is_scalar_resolved_type(&op.return_type())
        && !matches!(op, MapGetOr | MapHas | MapLen | MapValueAt))
}
fn vec_operation(op: crate::vec_ops::VecOp) -> bool {
    matches!(
        op,
        crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity | crate::vec_ops::VecOp::Get
    )
}
fn ast_param(program: &dyn ProgramSource, mode: ParamMode, ty: &Type) -> bool {
    match mode {
        ParamMode::Own => *ty == Type::Bytes,
        ParamMode::Value => {
            crate::source_verify::is_scalar_source_type(ty)
                || crate::source_verify::copy_record_source_admitted(program, ty)
                || super::ast_copy_variant(program, ty)
        }
        ParamMode::Borrow => {
            super::ast_param_admitted(program, mode, ty)
                || matches!(ty, Type::Named { name, arguments } if arguments.is_empty()
                && program.types().iter().any(|d| d.name == *name && matches!(d.kind, crate::ast::TypeDeclarationKind::Record { .. })))
        }
        ParamMode::Shared => false,
    }
}
pub(crate) fn source_target(program: &dyn ProgramSource, target: &Function) -> bool {
    target.return_type == Type::Bytes
        && target
            .params
            .iter()
            .filter(|p| p.mode == ParamMode::Own && p.ty == Type::Bytes)
            .count()
            == 1
        && target
            .params
            .iter()
            .all(|p| ast_param(program, p.mode, &p.ty))
        && source_nonallocating(program, target)
}
pub(crate) fn source_assignment(program: &dyn ProgramSource, binding: &str, value: &Expr) -> bool {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = program.functions().iter().find(|f| f.name == *name) else {
        return false;
    };
    type_arguments.is_empty()
        && args.len() == target.params.len()
        && source_target(program, target)
        && target.params.iter().zip(args).all(|(p, arg)| {
            p.mode != ParamMode::Own || matches!(&arg.kind, ExprKind::Var(name) if name == binding)
        })
}
fn resolved_param(
    program: &dyn crate::cleanup::ReplayContext,
    mode: OwnershipMode,
    ty: &ResolvedType,
) -> bool {
    match mode {
        OwnershipMode::Own => *ty == ResolvedType::Bytes,
        OwnershipMode::Value => {
            crate::hir::is_scalar_resolved_type(ty)
                || crate::hir::copy_record_collection::admitted(program.declarations(), ty)
                || super::resolved_match_scrutinee_admitted(program.declarations(), ty)
        }
        OwnershipMode::Borrow => {
            super::resolved_param_admitted(program.declarations(), mode, ty)
                || matches!(ty, ResolvedType::Nominal { declaration, arguments } if arguments.is_empty() && program.declarations().record_fields(declaration).is_some())
                || crate::hir::owned_collection_record::admitted(ty, program.declarations())
        }
        OwnershipMode::Shared => false,
    }
}
pub(crate) fn resolved_target(
    program: &dyn crate::cleanup::ReplayContext,
    target: &ResolvedFunction,
) -> bool {
    target.return_type == ResolvedType::Bytes
        && target
            .params
            .iter()
            .filter(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
            .count()
            == 1
        && target
            .params
            .iter()
            .all(|p| resolved_param(program, p.ownership, &p.ty))
        && resolved_nonallocating(program, target)
}
pub(crate) fn assignment(
    program: &dyn crate::cleanup::ReplayContext,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Call {
        callee,
        instance: None,
        ..
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = program.resolve_call_target(callee, None) else {
        return false;
    };
    assignment_with_target(
        program,
        binding,
        value,
        target,
        resolved_target(program, target),
    )
}
fn assignment_with_target(
    _program: &dyn crate::cleanup::ReplayContext,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
    target: &ResolvedFunction,
    proved: bool,
) -> bool {
    let ResolvedExprKind::Call {
        type_arguments,
        args,
        instance: None,
        ..
    } = &value.kind
    else {
        return false;
    };
    binding.ty == ResolvedType::Bytes && binding.ownership == OwnershipMode::Own
        && value.ty == ResolvedType::Bytes && value.ownership == OwnershipMode::Own
        && type_arguments.is_empty() && args.len() == target.params.len()
        && proved
        && target.params.iter().zip(args).all(|(p, arg)| {
            p.ty == arg.ty && match p.ownership {
                OwnershipMode::Own => arg.ownership == OwnershipMode::Own
                    && matches!(&arg.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty()),
                OwnershipMode::Value => arg.ownership == OwnershipMode::Value,
                OwnershipMode::Borrow => matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow),
                OwnershipMode::Shared => false,
            }
        })
}

#[cfg(test)]
mod tests;
