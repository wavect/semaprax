//! Declaration-shaped source cost for the expected projection: what one
//! function, parameter, field, or type contributes to the builder pre-bound,
//! split into the part every retained projection keeps and the contract an
//! imported stub discards.

use crate::ast::{Function, Type};
use crate::diagnostic::Diagnostic;

use super::ast_expr_cost;
use super::cost::StructuralCost;

pub(super) fn ast_field_cost(
    field: &crate::ast::FieldDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(field)?;
    cost.string(&field.stable_id)?;
    cost.string(&field.name)?;
    ast_type_cost(&field.ty, cost)
}

pub(super) fn ast_function_cost(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    ast_function_signature_cost(function, cost)?;
    ast_function_contract_cost_into(function, cost)
}

/// The part of a function every retained projection keeps: its identity, its
/// name, its type parameters, its parameters, its return type, and its effect
/// row. An imported function is retained as exactly this much plus a synthetic
/// default body.
pub(super) fn ast_function_signature_cost(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(function)?;
    cost.string(&function.stable_id)?;
    cost.string(&function.name)?;
    for parameter in &function.type_parameters {
        cost.value(parameter)?;
        cost.string(&parameter.name)?;
    }
    for param in &function.params {
        ast_param_cost(param, cost)?;
    }
    ast_type_cost(&function.return_type, cost)?;
    for effect in &function.effects {
        cost.string(effect)?;
    }
    Ok(())
}

/// The preconditions, body, and postconditions of a function. An imported
/// function's contract is cloned transiently and then discarded, so it is
/// charged as a peak rather than as retained structure.
pub(super) fn ast_function_contract_cost(
    function: &Function,
) -> Result<StructuralCost, Vec<Diagnostic>> {
    let mut cost = StructuralCost::new();
    ast_function_contract_cost_into(function, &mut cost)?;
    Ok(cost)
}

fn ast_function_contract_cost_into(
    function: &Function,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        ast_expr_cost(expression, cost)?;
    }
    Ok(())
}

pub(super) fn ast_param_cost(
    param: &crate::ast::Param,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.value(param)?;
    cost.string(&param.name)?;
    ast_type_cost(&param.ty, cost)
}

pub(super) fn ast_type_cost(ty: &Type, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(ty)?;
    if let Type::Named { name, arguments } = ty {
        cost.string(name)?;
        for argument in arguments {
            ast_type_cost(argument, cost)?;
        }
    }
    Ok(())
}
