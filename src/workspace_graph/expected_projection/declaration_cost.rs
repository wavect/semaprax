//! Declaration-shaped source cost for the expected projection: what one
//! function, parameter, field, or type contributes to the builder pre-bound,
//! split into the part every retained projection keeps and the contract an
//! imported stub discards.

use crate::ast::{Expr, ExprKind, Function, Program, Type};
use crate::diagnostic::Diagnostic;

use super::cost::StructuralCost;
use super::{ast_pattern_cost, ast_type_declaration_cost};

pub(super) fn ast_field_cost(
    field: &crate::ast::FieldDeclaration,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    cost.inline_type_parent(field)?;
    cost.embedded_string(&field.stable_id)?;
    cost.embedded_string(&field.name)?;
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
    cost.function_signature(function)?;
    cost.embedded_string(&function.stable_id)?;
    cost.embedded_string(&function.name)?;
    for parameter in &function.type_parameters {
        cost.value(parameter)?;
        cost.embedded_string(&parameter.name)?;
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
    cost.inline_type_parent(param)?;
    cost.embedded_string(&param.name)?;
    ast_type_cost(&param.ty, cost)
}

pub(super) fn ast_type_cost(ty: &Type, cost: &mut StructuralCost) -> Result<(), Vec<Diagnostic>> {
    cost.value(ty)?;
    if let Type::Named { name, arguments } = ty {
        cost.embedded_string(name)?;
        for argument in arguments {
            ast_type_cost(argument, cost)?;
        }
    }
    if cost.is_raw_fallback() {
        if let Type::Function { parameters, result } = ty {
            for parameter in parameters {
                ast_type_cost(parameter, cost)?;
            }
            ast_type_cost(result, cost)?;
        }
    }
    Ok(())
}

/// Where the parent places a child's resolved root. `CarrierFree` is used only
/// for a `Box` slot of a unary, binary, `if` or block-tail node, or an element
/// of a free-function call's argument list (every intrinsic route only moves
/// or boxes its arguments): no binding, statement, field initializer, arm or
/// pattern field is constructed for it there. Only literal/unary/binary and
/// `if` roots in such a slot are censused; variable roots are also censused
/// outside every authored closure subtree, retaining all their storage.
/// Block roots in those same positions are also carrier-free: their statements
/// remain independently charged and their tail is boxed by finish_block. Every
/// other child, and every root reached through `ast_expr_cost`, is `Delegated` and keeps its complete
/// bundle.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RootPosition {
    CarrierFree,
    Delegated,
}

pub(super) fn ast_expr_cost(
    expression: &Expr,
    cost: &mut StructuralCost,
) -> Result<(), Vec<Diagnostic>> {
    ast_expr_cost_at(expression, cost, RootPosition::Delegated, true)
}

fn ast_expr_cost_at(
    expression: &Expr,
    cost: &mut StructuralCost,
    position: RootPosition,
    outside_closure: bool,
) -> Result<(), Vec<Diagnostic>> {
    use RootPosition::CarrierFree;
    cost.value(expression)?;
    // `super` allocates the initial receiver before recreating its source,
    // upcast wrapper and call root; the discarded receiver still costs bytes.
    cost.add_identity_carriers(if matches!(expression.kind, ExprKind::SuperMethod { .. }) {
        4
    } else {
        3
    })?;
    // These resolver branches construct one root ExpressionId. Child trees,
    // imported defaults, generic expansions, and desugared statements keep
    // their independent bounds; other shapes keep the three-carrier ceiling.
    if matches!(
        expression.kind,
        ExprKind::Int(_)
            | ExprKind::String(_)
            | ExprKind::BorrowedUtf8Literal(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::Var(_)
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::If { .. }
            | ExprKind::Block { .. }
    ) {
        cost.note_single_identity_carrier()?;
    }
    cost.note_exact_expr_fixed_bundle()?;
    cost.account_scalar_identity(&expression.kind)?;
    cost.account_exact_absent_storage(&expression.kind)?;
    cost.account_carrier_free_root(&expression.kind, position == CarrierFree, outside_closure)?;
    match &expression.kind {
        ExprKind::Closure {
            params,
            return_type,
            body,
            ..
        } => {
            for parameter in params {
                cost.inline_type_parent(parameter)?;
                cost.embedded_string(&parameter.name)?;
                ast_type_cost(&parameter.ty, cost)?;
            }
            ast_type_cost(return_type, cost)?;
            ast_expr_cost_at(body, cost, RootPosition::Delegated, false)?;
        }
        ExprKind::Var(name) => cost.embedded_string(name)?,
        ExprKind::ArrayU8(values) => cost.add(values.len())?,
        ExprKind::RepeatArrayU8 { .. } => {}
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            cost.embedded_string(name)?;
            // Complete synthetic key/layout identities are additional HIR text.
            // Bound every transient replay buffer before entering the builder;
            // descriptor construction itself charges its actual allocation.
            if crate::vec_ops::field_unique::literal_call(name)
                || name == crate::vec_ops::field_update::NAME
            {
                cost.add_split(
                    8 * crate::vec_ops::sort_keys::MAX_DESCRIPTOR_BYTES,
                    8 * crate::vec_ops::sort_keys::MAX_DESCRIPTOR_BYTES,
                )?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost_at(argument, cost, CarrierFree, outside_closure)?;
            }
        }
        ExprKind::LexicalExit { value: None, .. } => {}
        ExprKind::LexicalExit {
            value: Some(value), ..
        } => ast_expr_cost_at(value, cost, RootPosition::Delegated, outside_closure)?,
        ExprKind::Unary { value, .. } => {
            ast_expr_cost_at(value, cost, CarrierFree, outside_closure)?
        }
        ExprKind::VecConstructor { element, value, .. } => {
            ast_type_cost(element, cost)?;
            ast_expr_cost_at(value, cost, RootPosition::Delegated, outside_closure)?;
        }
        ExprKind::Binary { left, right, .. } => {
            ast_expr_cost_at(left, cost, CarrierFree, outside_closure)?;
            ast_expr_cost_at(right, cost, CarrierFree, outside_closure)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                if matches!(
                    statement,
                    crate::ast::Statement::Let { .. } | crate::ast::Statement::Assign { .. }
                ) {
                    cost.inline_expr_parent(statement)?;
                } else {
                    cost.value(statement)?;
                }
                if matches!(
                    statement,
                    crate::ast::Statement::For { .. }
                        | crate::ast::Statement::ForBorrow { .. }
                        | crate::ast::Statement::ForOwn { .. }
                ) {
                    cost.add_identity_carriers(16)?;
                }
                // Charge the string every statement actually carries. Unsafe
                // boundaries carry their verbatim audit summary, `while`
                // carries no binding at all, bounded `for` traversal carries
                // its item binding, and `let`/assignment carry their name.
                // Matching the statement exhaustively keeps a later variant a
                // compile error here rather than a panic at check time:
                // `Statement::name` panics for every statement that is not a
                // `let` or an assignment.
                match statement {
                    crate::ast::Statement::Unsafe { audit, .. } => cost.embedded_string(audit)?,
                    crate::ast::Statement::While { .. } => cost.string("")?,
                    crate::ast::Statement::For { item, .. }
                    | crate::ast::Statement::ForOwn { item, .. } => cost.embedded_string(item)?,
                    crate::ast::Statement::ForBorrow { item, index, .. } => {
                        cost.embedded_string(item)?;
                        if let Some(index) = index {
                            cost.embedded_string(&index.name)?;
                        }
                    }
                    crate::ast::Statement::Let { name, .. }
                    | crate::ast::Statement::Assign { name, .. } => cost.embedded_string(name)?,
                }
                if cost.is_raw_fallback() {
                    match statement {
                        crate::ast::Statement::Assign {
                            field: Some(field), ..
                        } => cost.embedded_string(&field.name)?,
                        crate::ast::Statement::Let {
                            declared: Some(ty), ..
                        } => ast_type_cost(ty, cost)?,
                        _ => {}
                    }
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        ast_expr_cost_at(child, cost, RootPosition::Delegated, outside_closure)?;
                    }
                }
            }
            ast_expr_cost_at(tail, cost, CarrierFree, outside_closure)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            ast_expr_cost_at(condition, cost, CarrierFree, outside_closure)?;
            ast_expr_cost_at(then_branch, cost, CarrierFree, outside_closure)?;
            ast_expr_cost_at(else_branch, cost, CarrierFree, outside_closure)?;
        }
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            fields,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            fields,
            ..
        } => {
            cost.embedded_string(type_name)?;
            if let ExprKind::ConstructVariant { case_name, .. } = &expression.kind {
                cost.embedded_string(case_name)?;
            }
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for field in fields {
                cost.inline_expr_parent(field)?;
                cost.embedded_string(&field.name)?;
                ast_expr_cost_at(&field.value, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            ast_expr_cost_at(scrutinee, cost, RootPosition::Delegated, outside_closure)?;
            for arm in arms {
                cost.match_arm(arm)?;
                ast_pattern_cost(&arm.pattern, cost)?;
                // The boxed guard is a complete resolved expression tree too;
                // `match_arm` charges only its pointer.
                if let Some(guard) = &arm.guard {
                    ast_expr_cost_at(guard, cost, RootPosition::Delegated, outside_closure)?;
                }
                ast_expr_cost_at(&arm.value, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Try { operand } | ExprKind::Yield { request: operand } => {
            ast_expr_cost_at(operand, cost, RootPosition::Delegated, outside_closure)?
        }
        ExprKind::UpdateRecord { base, fields } => {
            ast_expr_cost_at(base, cost, RootPosition::Delegated, outside_closure)?;
            for field in fields {
                cost.inline_expr_parent(field)?;
                cost.embedded_string(&field.name)?;
                ast_expr_cost_at(&field.value, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Project { base, field, .. } => {
            ast_expr_cost_at(base, cost, RootPosition::Delegated, outside_closure)?;
            cost.embedded_string(field)?;
        }
        ExprKind::MethodCall {
            receiver,
            method,
            type_arguments,
            args,
            ..
        } => {
            ast_expr_cost_at(receiver, cost, RootPosition::Delegated, outside_closure)?;
            cost.embedded_string(method)?;
            for ty in type_arguments {
                ast_type_cost(ty, cost)?;
            }
            for argument in args {
                ast_expr_cost_at(argument, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::SuperMethod { method, args, .. } => {
            cost.embedded_string(method)?;
            for argument in args {
                ast_expr_cost_at(argument, cost, RootPosition::Delegated, outside_closure)?;
            }
        }
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_) => {}
        ExprKind::String(value) | ExprKind::BorrowedUtf8Literal(value) => {
            if cost.is_raw_fallback() {
                cost.embedded_string(value)?;
            }
        }
    }
    Ok(())
}

pub(super) fn ast_program_cost(
    program: &Program,
    cost: &mut StructuralCost,
    include_imports: bool,
) -> Result<(), Vec<Diagnostic>> {
    cost.program(program)?;
    cost.embedded_string(&program.path)?;
    cost.embedded_string(&program.module)?;
    for module_use in program.module_uses.iter().filter(|_| include_imports) {
        cost.value(module_use)?;
        cost.embedded_string(&module_use.persistent_id)?;
        cost.embedded_string(&module_use.target_module)?;
        cost.embedded_string(&module_use.alias)?;
    }
    for permit in &program.permits {
        cost.string(permit)?;
    }
    for declaration in &program.types {
        ast_type_declaration_cost(declaration, cost)?;
    }
    for interface in &program.interfaces {
        cost.value(interface)?;
        cost.embedded_string(&interface.stable_id)?;
        cost.embedded_string(&interface.name)?;
        for permit in &interface.permits {
            cost.string(permit)?;
        }
        for import in &interface.imports {
            cost.value(import)?;
            cost.embedded_string(&import.stable_id)?;
            cost.embedded_string(&import.name)?;
            for param in &import.params {
                ast_param_cost(param, cost)?;
            }
            for effect in &import.effects {
                cost.string(effect)?;
            }
            if let crate::ast::ImportFailure::Status { domain_id } = &import.failure {
                cost.embedded_string(domain_id)?;
            }
            cost.embedded_string(&import.consumes)?;
        }
    }
    for function in &program.functions {
        ast_function_cost(function, cost)?;
    }
    Ok(())
}
