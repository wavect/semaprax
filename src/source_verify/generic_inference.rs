//! Bounded argument-directed inference. This pass observes type facts only;
//! ordinary argument checking remains the sole source ownership authority.
use super::binding::Binding;
use super::declared_type::substitute_function_type;
use crate::ast::{BinaryOp, Expr, ExprKind, Function, Type, UnaryOp};
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
    current: &Function,
    target: &Function,
    args: &[Expr],
    bindings: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
) -> Option<Vec<Type>> {
    if !current.type_parameters.is_empty() || args.len() != target.params.len() {
        return None;
    }
    let parameters = target
        .type_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut inferred = vec![None; target.type_parameters.len()];
    let mut budget = EvidenceBudget::default();
    for (formal, expression) in target.params.iter().zip(args) {
        let actual = evidence(expression, bindings, functions, &mut budget, 0)?;
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

fn evidence(
    expression: &Expr,
    bindings: &HashMap<String, Binding>,
    functions: &HashMap<&str, &Function>,
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
            let value = evidence(value, bindings, functions, budget, next)?;
            match op {
                UnaryOp::Neg if matches!(value, Type::I64 | Type::I32 | Type::F32 | Type::F64) => {
                    value
                }
                UnaryOp::Not if value == Type::Bool => Type::Bool,
                UnaryOp::Neg | UnaryOp::Not => return None,
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = evidence(left, bindings, functions, budget, next)?;
            let right = evidence(right, bindings, functions, budget, next)?;
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
            if evidence(condition, bindings, functions, budget, next)? != Type::Bool {
                return None;
            }
            let then_branch = evidence(then_branch, bindings, functions, budget, next)?;
            let else_branch = evidence(else_branch, bindings, functions, budget, next)?;
            if then_branch != else_branch {
                return None;
            }
            then_branch
        }
        ExprKind::Block { statements, tail } if statements.is_empty() => {
            evidence(tail, bindings, functions, budget, next)?
        }
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } => {
            let target = functions.get(name.as_str())?;
            if args.len() != target.params.len()
                || (!target.type_parameters.is_empty()
                    && type_arguments.len() != target.type_parameters.len())
            {
                return None;
            }
            if target.type_parameters.is_empty() {
                if !type_arguments.is_empty() {
                    return None;
                }
                target.return_type.clone()
            } else if type_arguments.is_empty() {
                return None;
            } else {
                substitute_function_type(target, type_arguments, &target.return_type)?
            }
        }
        _ => return None,
    })
}
