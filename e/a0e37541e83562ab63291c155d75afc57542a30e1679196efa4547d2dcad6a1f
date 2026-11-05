//! Exact transparent generic wrappers admitted only for `std.mem`.
use super::BoxOp;
use crate::ast::program_source::ProgramSource;
use crate::ast::{Expr, ExprKind, Function, Type};
use crate::hir::{
    ResolvedExpr, ResolvedExprKind, ResolvedFunctionTemplate, ResolvedProgram, ResolvedType,
};
use std::cell::Cell;
pub(crate) const MODULE: &str = "std.mem";
thread_local! { static AUTHENTICATED_LINKED_SOURCE: Cell<bool> = const { Cell::new(false) }; }
pub(crate) fn with_authenticated_linked_source<T>(operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            AUTHENTICATED_LINKED_SOURCE.with(|active| active.set(self.0));
        }
    }
    let restore = Restore(AUTHENTICATED_LINKED_SOURCE.with(|active| active.replace(true)));
    let result = operation();
    drop(restore);
    result
}
pub(crate) fn source_module_is_authenticated(program: &dyn ProgramSource) -> bool {
    program.module() == MODULE || AUTHENTICATED_LINKED_SOURCE.with(Cell::get)
}
pub(crate) fn wrapper_id(op: BoxOp) -> &'static str {
    match op {
        BoxOp::New => "std.mem.box.new",
        BoxOp::Get => "std.mem.box.get",
        BoxOp::IntoInner => "std.mem.box.into-inner",
    }
}
pub(crate) fn wrapper_name(op: BoxOp) -> &'static str {
    match op {
        BoxOp::New => "new",
        BoxOp::Get => "get",
        BoxOp::IntoInner => "into_inner",
    }
}
pub(crate) fn wrapper_by_id(id: &str) -> Option<BoxOp> {
    super::ALL.into_iter().find(|op| wrapper_id(*op) == id)
}
#[cfg(test)]
pub(crate) fn is_source_candidate(program: &dyn ProgramSource, function: &Function) -> bool {
    program.module() == MODULE
        && (wrapper_by_id(&function.stable_id).is_some()
            || super::ALL
                .into_iter()
                .any(|op| wrapper_name(op) == function.name))
}
pub(crate) fn is_source_candidate_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
) -> bool {
    program.module() == MODULE
        && (wrapper_by_id(&function.payload().stable_id).is_some()
            || super::ALL
                .into_iter()
                .any(|op| wrapper_name(op) == function.name()))
}
pub(crate) fn source_wrapper(program: &dyn ProgramSource, function: &Function) -> Option<BoxOp> {
    if !source_module_is_authenticated(program)
        || !function.explicit_id
        || function.type_parameters.len() != 1
        || function.type_parameters[0].name != "T"
        || !function.effects.is_empty()
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
    {
        return None;
    }
    let op = wrapper_by_id(&function.stable_id)?;
    if function.name != wrapper_name(op) {
        return None;
    }
    if function.params.len() != 1
        || function.params[0].name != "value"
        || function.params[0].mode
            != match op.param_ownership() {
                crate::hir::OwnershipMode::Own => crate::ast::ParamMode::Own,
                crate::hir::OwnershipMode::Borrow => crate::ast::ParamMode::Borrow,
                _ => crate::ast::ParamMode::Value,
            }
        || !match op {
            BoxOp::New => generic_parameter(&function.params[0].ty),
            _ => generic_box(&function.params[0].ty),
        }
        || !match op {
            BoxOp::New => generic_box(&function.return_type),
            _ => generic_parameter(&function.return_type),
        }
    {
        return None;
    }
    exact_source_body(&function.body, op, &function.params[0]).then_some(op)
}

/// Downstream presentation classification: the current projected module and
/// original declaration name control recognition; local lookup separately uses
/// the effective binding name. The complete reserved declaration guard remains.
pub(crate) fn source_wrapper_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
) -> Option<BoxOp> {
    if !source_module_is_authenticated(program)
        || !function.payload().explicit_id
        || function.payload().type_parameters.len() != 1
        || function.payload().type_parameters[0].name != "T"
        || !function.payload().effects.is_empty()
        || !function.payload().requires.is_empty()
        || !function.payload().ensures.is_empty()
    {
        return None;
    }
    let op = wrapper_by_id(&function.payload().stable_id)?;
    if function.declaration_name() != wrapper_name(op) {
        return None;
    }
    if function.payload().params.len() != 1
        || function.payload().params[0].name != "value"
        || function.payload().params[0].mode
            != match op.param_ownership() {
                crate::hir::OwnershipMode::Own => crate::ast::ParamMode::Own,
                crate::hir::OwnershipMode::Borrow => crate::ast::ParamMode::Borrow,
                _ => crate::ast::ParamMode::Value,
            }
        || !match op {
            BoxOp::New => generic_parameter(&function.payload().params[0].ty),
            _ => generic_box(&function.payload().params[0].ty),
        }
        || !match op {
            BoxOp::New => generic_box(&function.payload().return_type),
            _ => generic_parameter(&function.payload().return_type),
        }
    {
        return None;
    }
    exact_source_body(&function.payload().body, op, &function.payload().params[0]).then_some(op)
}
fn exact_source_body(body: &Expr, op: BoxOp, param: &crate::ast::Param) -> bool {
    let ExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return false;
    };
    statements.is_empty()
        && name == op.name()
        && matches!(type_arguments.as_slice(), [argument] if generic_parameter(argument))
        && matches!(args.as_slice(),[arg] if matches!(&arg.kind,ExprKind::Var(name) if name==&param.name))
}
fn generic_parameter(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == "T" && arguments.is_empty())
}
fn generic_box(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == "Box" && matches!(arguments.as_slice(), [element] if generic_parameter(element)))
}

pub(crate) fn source_parameter_is_admitted(
    program: &dyn ProgramSource,
    function: &Function,
    op: BoxOp,
    ty: &Type,
) -> bool {
    source_wrapper(program, function) == Some(op)
        && matches!(ty,Type::Named{name,arguments} if name=="T"&&arguments.is_empty())
}
#[cfg(test)]
pub(crate) fn source_arguments_are_admitted(
    program: &dyn ProgramSource,
    function: &Function,
    args: &[Type],
) -> bool {
    source_wrapper(program, function).is_some()
        && matches!(args,[arg] if super::ast_element_is_admitted(arg))
}
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "staged alias-aware verifier map receiver")
)]
pub(crate) fn source_parameter_is_admitted_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
    op: BoxOp,
    ty: &Type,
) -> bool {
    source_wrapper_ref(program, function) == Some(op)
        && matches!(ty,Type::Named{name,arguments} if name=="T"&&arguments.is_empty())
}
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "staged alias-aware verifier map receiver")
)]
pub(crate) fn source_arguments_are_admitted_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
    args: &[Type],
) -> bool {
    source_wrapper_ref(program, function).is_some()
        && matches!(args,[arg] if super::ast_element_is_admitted(arg))
}
pub(crate) fn hir_wrapper(template: &ResolvedFunctionTemplate) -> Option<BoxOp> {
    if template.type_parameters.len() != 1
        || template.type_parameters[0].name != "T"
        || template.type_parameters[0].index != 0
        || !template.effects.is_empty()
        || !template.requires.is_empty()
        || !template.ensures.is_empty()
    {
        return None;
    }
    let op = wrapper_by_id(template.id.as_str())?;
    if template.name != wrapper_name(op) {
        return None;
    }
    let parameter = ResolvedType::TypeParameter {
        owner: template.id.clone(),
        index: 0,
    };
    let expected = super::resolved_params(op, &parameter);
    if template.params.len() != 1
        || template.params[0].name != "value"
        || template.params[0].ownership != expected[0].ownership
        || template.params[0].ty != expected[0].ty
        || template.return_type != op.resolved_return_type(&parameter)
    {
        return None;
    }
    exact_hir_body(&template.body, op, &template.params[0], &parameter).then_some(op)
}
pub(crate) fn hir_wrapper_in_program(
    _: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Option<BoxOp> {
    hir_wrapper(template)
}
pub(crate) fn hir_arguments_are_admitted(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    args: &[ResolvedType],
) -> bool {
    hir_wrapper_in_program(program, template).is_some()
        && matches!(args,[arg] if super::resolved_element_is_admitted(arg))
}
fn exact_hir_body(
    body: &ResolvedExpr,
    op: BoxOp,
    param: &crate::hir::ResolvedParam,
    parameter: &ResolvedType,
) -> bool {
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &tail.kind
    else {
        return false;
    };
    statements.is_empty()
        && callee.as_str() == op.id()
        && type_arguments == std::slice::from_ref(parameter)
        && instance.is_none()
        && matches!(args.as_slice(),[arg] if matches!(&arg.kind,ResolvedExprKind::Place(place) if place.root==param.id&&place.projections.is_empty()))
}
pub(crate) fn resolved_parameter_is_admitted(
    function: &crate::hir::FunctionExecutionId,
    op: BoxOp,
    ty: &ResolvedType,
) -> bool {
    let crate::hir::FunctionExecutionId::Monomorphic(owner) = function else {
        return false;
    };
    wrapper_by_id(owner.as_str()) == Some(op)
        && matches!(ty,ResolvedType::TypeParameter{owner:parameter_owner,index:0} if parameter_owner==owner)
}
pub(crate) fn template_type_is_admitted(
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> bool {
    hir_wrapper(template).is_some()
        && (matches!(ty,ResolvedType::TypeParameter{owner,index:0} if owner==&template.id)
            || matches!(ty,ResolvedType::Nominal{declaration,arguments} if declaration.as_str()==crate::prelude::BOX_ID&&matches!(arguments.as_slice(),[ResolvedType::TypeParameter{owner,index:0}] if owner==&template.id)))
}
pub(crate) fn template_ownership(
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> Option<crate::hir::OwnershipMode> {
    let op = hir_wrapper(template)?;
    let parameter = ResolvedType::TypeParameter {
        owner: template.id.clone(),
        index: 0,
    };
    (ty == &super::resolved_box(parameter)).then_some(match op {
        BoxOp::Get => crate::hir::OwnershipMode::Borrow,
        BoxOp::New | BoxOp::IntoInner => crate::hir::OwnershipMode::Own,
    })
}

#[cfg(test)]
mod original;
#[cfg(test)]
mod source_view_tests;
#[cfg(test)]
pub(crate) fn source_wrapper_original(
    program: &dyn ProgramSource,
    function: &Function,
) -> Option<BoxOp> {
    original::source_wrapper(program, function)
}
