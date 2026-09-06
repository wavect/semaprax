//! Exact transparent generic wrappers admitted only for `std.collections`.

use std::cell::Cell;

use crate::ast::{Expr, ExprKind, Function, Program, Type};
use crate::hir::{
    OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunctionTemplate, ResolvedProgram,
    ResolvedType,
};

use super::{resolved_vec, VecOp};

pub(crate) const MODULE: &str = "std.collections";

thread_local! {
    static AUTHENTICATED_LINKED_SOURCE: Cell<bool> = const { Cell::new(false) };
}

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

pub(crate) fn source_module_is_authenticated(program: &Program) -> bool {
    program.module == MODULE || AUTHENTICATED_LINKED_SOURCE.with(Cell::get)
}

pub(crate) fn wrapper_id(op: VecOp) -> &'static str {
    match op {
        VecOp::WithCapacity => "std.collections.vec.with-capacity",
        VecOp::Push => "std.collections.vec.push",
        VecOp::Len => "std.collections.vec.len",
        VecOp::Capacity => "std.collections.vec.capacity",
        VecOp::Get => "std.collections.vec.get",
        VecOp::ReserveExact => "std.collections.vec.reserve-exact",
        VecOp::Set => "std.collections.vec.set",
        VecOp::Clear => "std.collections.vec.clear",
    }
}

pub(crate) fn wrapper_name(op: VecOp) -> &'static str {
    match op {
        VecOp::WithCapacity => "with_capacity",
        VecOp::Push => "push",
        VecOp::Len => "len",
        VecOp::Capacity => "capacity",
        VecOp::Get => "get",
        VecOp::ReserveExact => "reserve_exact",
        VecOp::Set => "set",
        VecOp::Clear => "clear",
    }
}

pub(crate) fn wrapper_by_id(id: &str) -> Option<VecOp> {
    super::ALL.into_iter().find(|op| wrapper_id(*op) == id)
}

pub(crate) fn is_source_candidate(program: &Program, function: &Function) -> bool {
    program.module == MODULE
        && (wrapper_by_id(&function.stable_id).is_some()
            || super::ALL
                .into_iter()
                .any(|op| wrapper_name(op) == function.name))
}

pub(crate) fn source_wrapper(program: &Program, function: &Function) -> Option<VecOp> {
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
    let parameter = Type::Named {
        name: "T".to_owned(),
        arguments: Vec::new(),
    };
    let expected = super::ast_params(op, &parameter);
    let expected_names: &[&str] = match op {
        VecOp::WithCapacity => &["capacity"],
        VecOp::Push => &["values", "value"],
        VecOp::Len | VecOp::Capacity => &["values"],
        VecOp::Get => &["values", "index"],
        VecOp::ReserveExact => &["values", "additional"],
        VecOp::Set => &["values", "index", "value"],
        VecOp::Clear => &["values"],
    };
    if function.params.len() != expected.len()
        || function
            .params
            .iter()
            .zip(expected.iter().zip(expected_names))
            .any(|(actual, (expected, name))| {
                actual.name != *name || actual.mode != expected.mode || actual.ty != expected.ty
            })
        || function.return_type != op.ast_return_type(&parameter)
    {
        return None;
    }
    exact_source_body(&function.body, op, &function.params).then_some(op)
}

fn exact_source_body(body: &Expr, op: VecOp, params: &[crate::ast::Param]) -> bool {
    let ExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    if !statements.is_empty() {
        return false;
    }
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &tail.kind
    else {
        return false;
    };
    name == op.name()
        && type_arguments
            == &[Type::Named {
                name: "T".to_owned(),
                arguments: Vec::new(),
            }]
        && args.len() == params.len()
        && args.iter().zip(params).all(
            |(argument, parameter)| matches!(&argument.kind, ExprKind::Var(name) if name == &parameter.name),
        )
}

pub(crate) fn source_parameter_is_admitted(
    program: &Program,
    function: &Function,
    op: VecOp,
    ty: &Type,
) -> bool {
    source_wrapper(program, function) == Some(op)
        && matches!(ty, Type::Named { name, arguments } if name == "T" && arguments.is_empty())
}

pub(crate) fn source_arguments_are_admitted(
    program: &Program,
    function: &Function,
    arguments: &[Type],
) -> bool {
    source_wrapper(program, function).is_some()
        && matches!(arguments, [argument] if super::ast_element_is_admitted(argument))
}

pub(crate) fn hir_wrapper(template: &ResolvedFunctionTemplate) -> Option<VecOp> {
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
    let expected_names: &[&str] = match op {
        VecOp::WithCapacity => &["capacity"],
        VecOp::Push => &["values", "value"],
        VecOp::Len | VecOp::Capacity => &["values"],
        VecOp::Get => &["values", "index"],
        VecOp::ReserveExact => &["values", "additional"],
        VecOp::Set => &["values", "index", "value"],
        VecOp::Clear => &["values"],
    };
    if template.params.len() != expected.len()
        || template
            .params
            .iter()
            .zip(expected.iter().zip(expected_names))
            .any(|(actual, (expected, name))| {
                actual.name != *name
                    || actual.ownership != expected.ownership
                    || actual.ty != expected.ty
            })
        || template.return_type != op.resolved_return_type(&parameter)
    {
        return None;
    }
    exact_hir_body(&template.body, op, &template.params, &parameter).then_some(op)
}

pub(crate) fn hir_wrapper_in_program(
    _program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Option<VecOp> {
    // Imported templates are cloned into the consumer program, whose module
    // is no longer `std.collections`. Exact stable identity, signature, body,
    // parameter roots, and intrinsic call identity are the HIR authority.
    hir_wrapper(template)
}

pub(crate) fn hir_arguments_are_admitted(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    arguments: &[ResolvedType],
) -> bool {
    hir_wrapper_in_program(program, template).is_some()
        && matches!(arguments, [argument] if super::resolved_element_is_admitted(argument))
}

fn exact_hir_body(
    body: &ResolvedExpr,
    op: VecOp,
    params: &[crate::hir::ResolvedParam],
    parameter: &ResolvedType,
) -> bool {
    let ResolvedExprKind::Block { statements, tail } = &body.kind else {
        return false;
    };
    if !statements.is_empty() {
        return false;
    }
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &tail.kind
    else {
        return false;
    };
    callee.as_str() == op.id()
        && type_arguments == std::slice::from_ref(parameter)
        && instance.is_none()
        && args.len() == params.len()
        && args.iter().zip(params).all(|(argument, parameter)| {
            matches!(&argument.kind, ResolvedExprKind::Place(place)
                if place.root == parameter.id && place.projections.is_empty())
        })
}

pub(crate) fn resolved_parameter_is_admitted(
    function: &crate::hir::FunctionExecutionId,
    op: VecOp,
    ty: &ResolvedType,
) -> bool {
    let crate::hir::FunctionExecutionId::Monomorphic(owner) = function else {
        return false;
    };
    wrapper_by_id(owner.as_str()) == Some(op)
        && matches!(ty, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 } if parameter_owner == owner)
}

pub(crate) fn template_type_is_admitted(
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> bool {
    let Some(op) = hir_wrapper(template) else {
        return false;
    };
    let parameter = ResolvedType::TypeParameter {
        owner: template.id.clone(),
        index: 0,
    };
    op.resolved_return_type(&parameter) == *ty
        || super::resolved_params(op, &parameter)
            .iter()
            .any(|candidate| candidate.ty == *ty)
}

pub(crate) fn template_ownership(
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> Option<OwnershipMode> {
    let op = hir_wrapper(template)?;
    (ty == &resolved_vec(ResolvedType::TypeParameter {
        owner: template.id.clone(),
        index: 0,
    }))
        .then_some(match op {
            VecOp::WithCapacity | VecOp::Push | VecOp::ReserveExact | VecOp::Set | VecOp::Clear => {
                OwnershipMode::Own
            }
            VecOp::Len | VecOp::Capacity | VecOp::Get => OwnershipMode::Borrow,
        })
}
