//! Capture-free, bounded proof-only lowering of linked scalar calls.
use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{BinaryOp, Expr, ExprKind, Function, Span, Statement, Type};
use crate::hir::{
    ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ResolvedType, ValueId,
};
use crate::project::ProjectRevision;

use super::{functions, plan, Refusal};

const MAX_NODES: usize = 4096;
type Scope = BTreeMap<ValueId, String>;

fn node(kind: ExprKind) -> Expr {
    Expr {
        kind,
        span: Span::default(),
    }
}
fn unsupported(owner: &str) -> Refusal {
    Refusal::UnsupportedExpr {
        owner: owner.into(),
    }
}

struct Lowerer<'a> {
    functions: BTreeMap<String, &'a ResolvedFunction>,
    reserved: BTreeSet<String>,
    fresh: usize,
    nodes: usize,
}

impl Lowerer<'_> {
    fn tick(&mut self) -> Result<(), Refusal> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            Err(Refusal::Capacity)
        } else {
            Ok(())
        }
    }

    fn fresh(&mut self, hint: &str) -> Result<String, Refusal> {
        self.tick()?;
        loop {
            self.fresh += 1;
            let name = format!("spx_modular_{hint}_{}", self.fresh);
            if self.reserved.insert(name.clone()) {
                return Ok(name);
            }
        }
    }

    fn expr(&mut self, expr: &ResolvedExpr, scope: &Scope, owner: &str) -> Result<Expr, Refusal> {
        self.tick()?;
        let kind = match &expr.kind {
            ResolvedExprKind::Int(v) => ExprKind::Int(*v),
            ResolvedExprKind::Int32(v) => ExprKind::Int32(*v),
            ResolvedExprKind::Uint8(v) => ExprKind::Uint8(*v),
            ResolvedExprKind::Usize(v) => ExprKind::Usize(*v),
            ResolvedExprKind::Bool(v) => ExprKind::Bool(*v),
            ResolvedExprKind::Place(place) if place.projections.is_empty() => ExprKind::Var(
                scope
                    .get(&place.root)
                    .ok_or_else(|| unsupported(owner))?
                    .clone(),
            ),
            ResolvedExprKind::Unary { op, value } => ExprKind::Unary {
                op: *op,
                value: Box::new(self.expr(value, scope, owner)?),
            },
            ResolvedExprKind::Binary { op, left, right } => ExprKind::Binary {
                op: *op,
                left: Box::new(self.expr(left, scope, owner)?),
                right: Box::new(self.expr(right, scope, owner)?),
            },
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => ExprKind::If {
                condition: Box::new(self.expr(condition, scope, owner)?),
                then_branch: Box::new(self.expr(then_branch, scope, owner)?),
                else_branch: Box::new(self.expr(else_branch, scope, owner)?),
            },
            ResolvedExprKind::Block { statements, tail } => {
                let mut local = scope.clone();
                let mut lowered = Vec::new();
                for statement in statements {
                    let ResolvedStatement::Let {
                        binding,
                        mutable: false,
                        value,
                        ..
                    } = statement
                    else {
                        return Err(unsupported(owner));
                    };
                    let value = self.expr(value, &local, owner)?;
                    let name = self.fresh("local")?;
                    local.insert(binding.id.clone(), name.clone());
                    lowered.push(Statement::Let {
                        syntax: crate::ast::LetSyntax::Authored,
                        name,
                        name_span: Span::default(),
                        mutable: false,
                        declared: None,
                        value,
                        span: Span::default(),
                    });
                }
                ExprKind::Block {
                    statements: lowered,
                    tail: Box::new(self.expr(tail, &local, owner)?),
                }
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } if type_arguments.is_empty() && instance.is_none() => {
                return self.call(callee.as_str(), args, scope)
            }
            _ => return Err(unsupported(owner)),
        };
        Ok(node(kind))
    }

    fn call(&mut self, id: &str, args: &[ResolvedExpr], scope: &Scope) -> Result<Expr, Refusal> {
        let callee = self
            .functions
            .get(id)
            .ok_or_else(|| Refusal::Missing { id: id.into() })?;
        if callee.params.len() != args.len() {
            return Err(unsupported(id));
        }
        let callee = (*callee).clone();
        let mut actuals = Vec::new();
        let mut callee_scope = Scope::new();
        for (arg, param) in args.iter().zip(&callee.params) {
            let value = self.expr(arg, scope, id)?;
            let name = self.fresh("actual")?;
            callee_scope.insert(param.id.clone(), name.clone());
            actuals.push(Statement::Let {
                syntax: crate::ast::LetSyntax::Authored,
                name,
                name_span: Span::default(),
                mutable: false,
                declared: None,
                value,
                span: Span::default(),
            });
        }
        let mut tail = self.expr(&callee.body, &callee_scope, id)?;
        // Reverse construction yields source-order guards. The rejected path
        // is a checked overflow, which the existing VC must prove unreachable.
        for require in callee.requires.iter().rev() {
            let condition = self.expr(require, &callee_scope, id)?;
            tail = node(ExprKind::If {
                condition: Box::new(condition),
                then_branch: Box::new(tail),
                else_branch: Box::new(trap(&callee.return_type, self.fresh("trap")?)?),
            });
        }
        Ok(node(ExprKind::Block {
            statements: actuals,
            tail: Box::new(tail),
        }))
    }
}

fn trap(ty: &ResolvedType, name: String) -> Result<Expr, Refusal> {
    let fallback = match ty {
        ResolvedType::I64 => ExprKind::Int(0),
        ResolvedType::I32 => ExprKind::Int32(0),
        ResolvedType::U8 => ExprKind::Uint8(0),
        ResolvedType::Usize => ExprKind::Usize(0),
        ResolvedType::Bool => ExprKind::Bool(false),
        _ => {
            return Err(Refusal::NonScalar {
                id: "call return".into(),
            })
        }
    };
    let overflow = node(ExprKind::Binary {
        op: BinaryOp::Add,
        left: Box::new(node(ExprKind::Int(i64::MAX))),
        right: Box::new(node(ExprKind::Int(1))),
    });
    Ok(node(ExprKind::Block {
        statements: vec![Statement::Let {
            syntax: crate::ast::LetSyntax::Authored,
            name,
            name_span: Span::default(),
            mutable: false,
            declared: Some(Type::I64),
            value: overflow,
            span: Span::default(),
        }],
        tail: Box::new(node(fallback)),
    }))
}

/// A proof-only, call-free AST; original checked source and runtime guards stay intact.
pub fn inline_subject(revision: &ProjectRevision, target: &str) -> Result<Function, Refusal> {
    plan(revision, target)?;
    let mut source_function = None;
    for source in revision.sources() {
        if source.source_graph_schema() == "semaprax.native-law.v1" {
            continue;
        }
        let program =
            crate::parse(source.source(), source.path()).map_err(|_| Refusal::MissingSource {
                id: source.path().into(),
            })?;
        for function in &program.functions {
            if function.stable_id == target && source_function.replace(function.clone()).is_some() {
                return Err(Refusal::MissingSource { id: target.into() });
            }
        }
    }
    let mut source_function =
        source_function.ok_or_else(|| Refusal::MissingSource { id: target.into() })?;
    let functions = functions(revision);
    let target_function = (**functions
        .get(target)
        .ok_or_else(|| Refusal::Missing { id: target.into() })?)
    .clone();
    if source_function.params.len() != target_function.params.len() {
        return Err(unsupported(target));
    }
    let mut scope = Scope::new();
    let mut reserved = BTreeSet::new();
    reserved.insert("result".into());
    for (source_param, resolved_param) in source_function.params.iter().zip(&target_function.params)
    {
        reserved.insert(source_param.name.clone());
        scope.insert(resolved_param.id.clone(), source_param.name.clone());
    }
    let mut lowerer = Lowerer {
        functions,
        reserved,
        fresh: 0,
        nodes: 0,
    };
    source_function.requires = target_function
        .requires
        .iter()
        .map(|expr| lowerer.expr(expr, &scope, target))
        .collect::<Result<_, _>>()?;
    source_function.body = lowerer.expr(&target_function.body, &scope, target)?;
    scope.insert(target_function.result_id.clone(), "result".into());
    source_function.ensures = target_function
        .ensures
        .iter()
        .map(|expr| lowerer.expr(expr, &scope, target))
        .collect::<Result<_, _>>()?;
    Ok(source_function)
}
