//! Exact transparent generic wrappers admitted only for `std.collections`.

use crate::ast::program_source::ProgramSource;
use std::cell::Cell;

use crate::ast::{Expr, ExprKind, Function, Type};
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

pub(crate) fn source_module_is_authenticated(program: &dyn ProgramSource) -> bool {
    program.module() == MODULE || AUTHENTICATED_LINKED_SOURCE.with(Cell::get)
}

pub(crate) fn wrapper_id(op: VecOp) -> Option<&'static str> {
    Some(match op {
        VecOp::WithCapacity => "std.collections.vec.with-capacity",
        VecOp::Push => "std.collections.vec.push",
        VecOp::Len => "std.collections.vec.len",
        VecOp::Capacity => "std.collections.vec.capacity",
        VecOp::Get => "std.collections.vec.get",
        VecOp::ReserveExact => "std.collections.vec.reserve-exact",
        VecOp::Set => "std.collections.vec.set",
        VecOp::Clear => "std.collections.vec.clear",
        VecOp::Sort => "std.collections.vec.sort",
        VecOp::CloneAt
        | VecOp::Replace
        | VecOp::ReserveOwned
        | VecOp::SortOwned
        | VecOp::SortBy
        | VecOp::StrAt
        | VecOp::UniqueBy
        | VecOp::UpdateField => return None,
    })
}

pub(crate) fn wrapper_name(op: VecOp) -> Option<&'static str> {
    Some(match op {
        VecOp::WithCapacity => "with_capacity",
        VecOp::Push => "push",
        VecOp::Len => "len",
        VecOp::Capacity => "capacity",
        VecOp::Get => "get",
        VecOp::ReserveExact => "reserve_exact",
        VecOp::Set => "set",
        VecOp::Clear => "clear",
        VecOp::Sort => "sort",
        VecOp::CloneAt
        | VecOp::Replace
        | VecOp::ReserveOwned
        | VecOp::SortOwned
        | VecOp::SortBy
        | VecOp::StrAt
        | VecOp::UniqueBy
        | VecOp::UpdateField => return None,
    })
}

pub(crate) fn wrapper_by_id(id: &str) -> Option<VecOp> {
    super::ALL
        .into_iter()
        .find(|op| wrapper_id(*op) == Some(id))
}

#[cfg(test)]
pub(crate) fn is_source_candidate(program: &dyn ProgramSource, function: &Function) -> bool {
    program.module() == MODULE
        && (wrapper_by_id(&function.stable_id).is_some()
            || super::ALL
                .into_iter()
                .any(|op| wrapper_name(op) == Some(function.name.as_str())))
}

pub(crate) fn is_source_candidate_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
) -> bool {
    program.module() == MODULE
        && (wrapper_by_id(&function.payload().stable_id).is_some()
            || super::ALL
                .into_iter()
                .any(|op| wrapper_name(op) == Some(function.name())))
}

pub(crate) fn source_wrapper(program: &dyn ProgramSource, function: &Function) -> Option<VecOp> {
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
    if function.name != wrapper_name(op)? {
        return None;
    }
    let expected_names: &[&str] = match op {
        VecOp::WithCapacity => &["capacity"],
        VecOp::Push => &["values", "value"],
        VecOp::Len | VecOp::Capacity => &["values"],
        VecOp::Get => &["values", "index"],
        VecOp::ReserveExact => &["values", "additional"],
        VecOp::Set => &["values", "index", "value"],
        VecOp::Clear | VecOp::Sort => &["values"],
        VecOp::CloneAt
        | VecOp::Replace
        | VecOp::ReserveOwned
        | VecOp::SortOwned
        | VecOp::SortBy
        | VecOp::StrAt
        | VecOp::UniqueBy
        | VecOp::UpdateField => return None,
    };
    if function.params.len() != expected_names.len()
        || function
            .params
            .iter()
            .zip(expected_names)
            .enumerate()
            .any(|(index, (actual, name))| {
                let mode = match op.param_ownership(index) {
                    OwnershipMode::Own => crate::ast::ParamMode::Own,
                    OwnershipMode::Borrow => crate::ast::ParamMode::Borrow,
                    _ => crate::ast::ParamMode::Value,
                };
                actual.name != *name
                    || actual.mode != mode
                    || !parameter_type(op, index, &actual.ty)
            })
        || !return_type(op, &function.return_type)
    {
        return None;
    }
    exact_source_body(&function.body, op, &function.params).then_some(op)
}

/// Downstream presentation classification: the current projected module and
/// effective name control recognition; provider authentication uses the
/// unchanged raw classifier above. No payload-name substitution is allowed.
pub(crate) fn source_wrapper_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
) -> Option<VecOp> {
    if let Some((provider, declaration)) = function.provider_declaration() {
        if !source_module_is_authenticated(program) {
            return None;
        }
        // A borrowed presentation must be an actual member of this source.
        // The unchanged full guard checks the real provider's reserved name,
        // identity, signature, effects and complete forwarding body.
        return program.function_refs().contains_presentation(function)
            .then(|| source_wrapper(provider, declaration)).flatten();
    }
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
    if function.name() != wrapper_name(op)? {
        return None;
    }
    let expected_names: &[&str] = match op {
        VecOp::WithCapacity => &["capacity"],
        VecOp::Push => &["values", "value"],
        VecOp::Len | VecOp::Capacity => &["values"],
        VecOp::Get => &["values", "index"],
        VecOp::ReserveExact => &["values", "additional"],
        VecOp::Set => &["values", "index", "value"],
        VecOp::Clear | VecOp::Sort => &["values"],
        VecOp::CloneAt
        | VecOp::Replace
        | VecOp::ReserveOwned
        | VecOp::SortOwned
        | VecOp::SortBy
        | VecOp::StrAt
        | VecOp::UniqueBy
        | VecOp::UpdateField => return None,
    };
    if function.payload().params.len() != expected_names.len()
        || function
            .payload()
            .params
            .iter()
            .zip(expected_names)
            .enumerate()
            .any(|(index, (actual, name))| {
                let mode = match op.param_ownership(index) {
                    OwnershipMode::Own => crate::ast::ParamMode::Own,
                    OwnershipMode::Borrow => crate::ast::ParamMode::Borrow,
                    _ => crate::ast::ParamMode::Value,
                };
                actual.name != *name
                    || actual.mode != mode
                    || !parameter_type(op, index, &actual.ty)
            })
        || !return_type(op, &function.payload().return_type)
    {
        return None;
    }
    exact_source_body(&function.payload().body, op, &function.payload().params).then_some(op)
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
        && matches!(type_arguments.as_slice(), [argument] if generic_parameter(argument))
        && args.len() == params.len()
        && args.iter().zip(params).all(
            |(argument, parameter)| matches!(&argument.kind, ExprKind::Var(name) if name == &parameter.name),
        )
}

fn generic_parameter(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == "T" && arguments.is_empty())
}
fn generic_vec(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if name == "Vec" && matches!(arguments.as_slice(), [element] if generic_parameter(element)))
}
fn parameter_type(op: VecOp, index: usize, ty: &Type) -> bool {
    match (op, index) {
        (VecOp::WithCapacity, 0) | (VecOp::Get | VecOp::ReserveExact | VecOp::Set, 1) => {
            *ty == Type::Usize
        }
        (VecOp::Push, 1) | (VecOp::Set, 2) => generic_parameter(ty),
        (_, 0) => generic_vec(ty),
        _ => false,
    }
}
fn return_type(op: VecOp, ty: &Type) -> bool {
    if op.returns_owner() {
        generic_vec(ty)
    } else if matches!(op, VecOp::Len | VecOp::Capacity) {
        *ty == Type::Usize
    } else {
        generic_parameter(ty)
    }
}

#[cfg(test)]
pub(crate) fn source_parameter_is_admitted(
    program: &dyn ProgramSource,
    function: &Function,
    op: VecOp,
    ty: &Type,
) -> bool {
    source_wrapper(program, function) == Some(op)
        && matches!(ty, Type::Named { name, arguments } if name == "T" && arguments.is_empty())
}

#[cfg(test)]
pub(crate) fn source_arguments_are_admitted(
    program: &dyn ProgramSource,
    function: &Function,
    arguments: &[Type],
) -> bool {
    source_wrapper(program, function).is_some()
        && matches!(arguments, [argument] if super::ast_element_is_admitted(argument))
}

pub(crate) fn source_parameter_is_admitted_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
    op: VecOp,
    ty: &Type,
) -> bool {
    source_wrapper_ref(program, function) == Some(op)
        && matches!(ty, Type::Named { name, arguments } if name == "T" && arguments.is_empty())
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "staged alias-aware verifier map receiver")
)]
pub(crate) fn source_arguments_are_admitted_ref(
    program: &dyn ProgramSource,
    function: &crate::ast::program_source::FunctionRef<'_>,
    arguments: &[Type],
) -> bool {
    source_wrapper_ref(program, function).is_some()
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
    if template.name != wrapper_name(op)? {
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
        VecOp::Clear | VecOp::Sort => &["values"],
        VecOp::CloneAt
        | VecOp::Replace
        | VecOp::ReserveOwned
        | VecOp::SortOwned
        | VecOp::SortBy
        | VecOp::StrAt
        | VecOp::UniqueBy
        | VecOp::UpdateField => return None,
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
            VecOp::WithCapacity
            | VecOp::Push
            | VecOp::ReserveExact
            | VecOp::Set
            | VecOp::Clear
            | VecOp::Sort => OwnershipMode::Own,
            VecOp::Len | VecOp::Capacity | VecOp::Get => OwnershipMode::Borrow,
            // These intrinsic operations have no transparent std.collections wrapper.
            VecOp::CloneAt
            | VecOp::Replace
            | VecOp::ReserveOwned
            | VecOp::SortOwned
            | VecOp::SortBy
            | VecOp::StrAt
            | VecOp::UniqueBy
            | VecOp::UpdateField => {
                return None;
            }
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
) -> Option<VecOp> {
    original::source_wrapper(program, function)
}
