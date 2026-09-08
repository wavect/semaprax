//! Bounded argument-directed inference. This pass observes type facts only;
//! ordinary argument checking remains the sole source ownership authority.
use super::binding::Binding;
use super::declared_type::{
    generic_function_arguments_are_admitted, generic_function_arguments_are_forwarded,
    substitute_function_type,
};
use super::type_table::TypeTable;
use crate::ast::{BinaryOp, Expr, ExprKind, Function, Program, Type, UnaryOp};
use std::collections::HashMap;

const MAX_EVIDENCE_DEPTH: usize = 128;
const MAX_EVIDENCE_NODES: usize = 4096;

#[derive(Default)]
struct EvidenceBudget {
    nodes: usize,
}

impl EvidenceBudget {
    fn visit(&mut self) -> Option<()> {
        self.nodes = self.nodes.checked_add(1)?;
        (self.nodes <= MAX_EVIDENCE_NODES).then_some(())
    }
}

pub(super) fn arguments(
    program: &Program,
    current: &Function,
    target: &Function,
    args: &[Expr],
    bindings: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
) -> Option<Vec<Type>> {
    let mut budget = EvidenceBudget::default();
    infer_arguments(
        program,
        current,
        target,
        args,
        bindings,
        functions,
        types,
        &mut budget,
        0,
    )
}

/// One bounded static type fact for the template-forwarding precheck. This
/// observes the same expression forms as call inference and never checks or
/// evaluates the expression.
pub(super) fn expression_type(
    program: &Program,
    current: &Function,
    expression: &Expr,
    bindings: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
) -> Option<Type> {
    evidence(
        program,
        current,
        expression,
        bindings,
        functions,
        types,
        &mut EvidenceBudget::default(),
        0,
    )
}

/// Keeps one call's immutable declaration context and mutable evidence budget
/// together so recursive omitted calls cannot reset either bound.
#[allow(clippy::too_many_arguments)]
fn infer_arguments(
    program: &Program,
    current: &Function,
    target: &Function,
    args: &[Expr],
    bindings: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    budget: &mut EvidenceBudget,
    depth: usize,
) -> Option<Vec<Type>> {
    if target.type_parameters.is_empty() || args.len() != target.params.len() {
        return None;
    }
    let parameters = target
        .type_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut inferred = vec![None; target.type_parameters.len()];
    for (formal, expression) in target.params.iter().zip(args) {
        let actual = evidence(
            program, current, expression, bindings, functions, types, budget, depth,
        )?;
        let mut pending = vec![(&formal.ty, &actual)];
        while let Some((formal, actual)) = pending.pop() {
            if let Type::Named { name, arguments } = formal {
                if arguments.is_empty() {
                    if let Some(index) = parameters.get(name.as_str()) {
                        if inferred[*index].as_ref().is_some_and(|old| old != actual) {
                            return None;
                        }
                        inferred[*index] = Some(actual.clone());
                        continue;
                    }
                }
            }
            if let (
                Type::Named {
                    name: a,
                    arguments: aa,
                },
                Type::Named {
                    name: b,
                    arguments: ba,
                },
            ) = (formal, actual)
            {
                if a != b || aa.len() != ba.len() {
                    return None;
                }
                pending.extend(aa.iter().zip(ba));
            } else if formal != actual {
                return None;
            }
        }
    }
    inferred.into_iter().collect()
}

fn numeric(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::U8 | Type::Usize | Type::F32 | Type::F64
    )
}

fn ordered(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::Char | Type::U8 | Type::Usize | Type::F32 | Type::F64
    )
}

/// Pure structural evidence shares the enclosing call's budget and does not
/// enter the ordinary expression verifier.
#[allow(clippy::too_many_arguments)]
fn evidence(
    program: &Program,
    current: &Function,
    expression: &Expr,
    bindings: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
    types: &TypeTable<'_>,
    budget: &mut EvidenceBudget,
    depth: usize,
) -> Option<Type> {
    if depth >= MAX_EVIDENCE_DEPTH || budget.visit().is_none() {
        return None;
    }
    let next = depth.checked_add(1)?;
    Some(match &expression.kind {
        ExprKind::Int(_) => Type::I64,
        ExprKind::Int32(_) => Type::I32,
        ExprKind::Uint8(_) => Type::U8,
        ExprKind::Usize(_) => Type::Usize,
        ExprKind::Char(_) => Type::Char,
        ExprKind::Float32(_) => Type::F32,
        ExprKind::Float64(_) => Type::F64,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::Var(name) => bindings.get(name)?.ty.clone(),
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            ..
        } => Type::Named {
            name: type_name.clone(),
            arguments: type_arguments.clone(),
        },
        ExprKind::Unary { op, value } => {
            let value = evidence(
                program, current, value, bindings, functions, types, budget, next,
            )?;
            match op {
                UnaryOp::Neg if matches!(value, Type::I64 | Type::I32 | Type::F32 | Type::F64) => {
                    value
                }
                UnaryOp::Not if value == Type::Bool => Type::Bool,
                UnaryOp::Neg | UnaryOp::Not => return None,
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = evidence(
                program, current, left, bindings, functions, types, budget, next,
            )?;
            let right = evidence(
                program, current, right, bindings, functions, types, budget, next,
            )?;
            if left != right {
                return None;
            }
            match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div if numeric(&left) => {
                    left
                }
                BinaryOp::Rem if left == Type::I64 => Type::I64,
                BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge if ordered(&left) => {
                    Type::Bool
                }
                BinaryOp::Eq | BinaryOp::Ne => Type::Bool,
                BinaryOp::And | BinaryOp::Or if left == Type::Bool => Type::Bool,
                _ => return None,
            }
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if evidence(
                program, current, condition, bindings, functions, types, budget, next,
            )? != Type::Bool
            {
                return None;
            }
            let then_branch = evidence(
                program,
                current,
                then_branch,
                bindings,
                functions,
                types,
                budget,
                next,
            )?;
            let else_branch = evidence(
                program,
                current,
                else_branch,
                bindings,
                functions,
                types,
                budget,
                next,
            )?;
            if then_branch != else_branch {
                return None;
            }
            then_branch
        }
        ExprKind::Block { statements, tail } if statements.is_empty() => evidence(
            program, current, tail, bindings, functions, types, budget, next,
        )?,
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            let target = functions.get(name.as_str())?;
            if args.len() != target.params.len() {
                return None;
            }
            let arguments = if target.type_parameters.is_empty() {
                if !type_arguments.is_empty() {
                    return None;
                }
                return Some(target.return_type.clone());
            } else if type_arguments.is_empty() {
                infer_arguments(
                    program, current, target, args, bindings, functions, types, budget, next,
                )?
            } else if type_arguments.len() != target.type_parameters.len() {
                return None;
            } else {
                type_arguments.clone()
            };
            if !generic_arguments_are_admitted(program, current, target, &arguments, types) {
                return None;
            }
            substitute_function_type(target, &arguments, &target.return_type)?
        }
        _ => return None,
    })
}

fn generic_arguments_are_admitted(
    program: &Program,
    current: &Function,
    target: &Function,
    arguments: &[Type],
    types: &TypeTable<'_>,
) -> bool {
    generic_function_arguments_are_admitted(target, arguments, types)
        || generic_function_arguments_are_forwarded(current, target, arguments)
        || crate::vec_ops::source_arguments_are_admitted(program, target, arguments)
        || crate::box_ops::source_arguments_are_admitted(program, target, arguments)
}
