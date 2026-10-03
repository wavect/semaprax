//! Straight-line checked-summary composition draft for LAW-06.
//!
//! All proof state is derived from retained HIR in one invocation. The
//! dependency plan and every callee's postconditions are checked before any
//! summary is consumed; caller preconditions are proved in evaluation order,
//! with only earlier summaries in each query.

use std::collections::{BTreeMap, BTreeSet};

use crate::assurance_manifest::smt_discharge::{
    self as smt, DischargeOutcome, Provisioning, RunLimits,
};
use crate::ast::{BinaryOp, Expr, ExprKind, Function, Param, ParamMode, Span, Type};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType, ValueId};
use crate::project::ProjectRevision;

use super::{
    collect, functions, plan,
    prove::{prove_postconditions, ProvedClause},
    CallOccurrence, Plan, Refusal,
};

type Scope = BTreeMap<ValueId, Expr>;
fn node(kind: ExprKind) -> Expr {
    Expr {
        kind,
        span: Span::default(),
    }
}
fn var(name: &str) -> Expr {
    node(ExprKind::Var(name.to_owned()))
}
fn unsupported(id: &str) -> Refusal {
    Refusal::UnsupportedExpr { owner: id.into() }
}

fn ast_type(ty: &ResolvedType) -> Result<Type, Refusal> {
    Ok(match ty {
        ResolvedType::I64 => Type::I64,
        ResolvedType::I32 => Type::I32,
        ResolvedType::U8 => Type::U8,
        ResolvedType::Usize => Type::Usize,
        ResolvedType::Bool => Type::Bool,
        _ => {
            return Err(Refusal::NonScalar {
                id: "summary result".into(),
            })
        }
    })
}

#[derive(Clone)]
struct Frame {
    callee: ResolvedFunction,
    args: Vec<Expr>,
    output: String,
}

struct Symbolic<'a> {
    functions: BTreeMap<String, &'a ResolvedFunction>,
    reserved: BTreeSet<String>,
    frames: Vec<Frame>,
}

impl Symbolic<'_> {
    fn expression(
        &mut self,
        expr: &ResolvedExpr,
        scope: &Scope,
        owner: &str,
    ) -> Result<Expr, Refusal> {
        let kind = match &expr.kind {
            ResolvedExprKind::Int(v) => ExprKind::Int(*v),
            ResolvedExprKind::Int32(v) => ExprKind::Int32(*v),
            ResolvedExprKind::Uint8(v) => ExprKind::Uint8(*v),
            ResolvedExprKind::Usize(v) => ExprKind::Usize(*v),
            ResolvedExprKind::Bool(v) => ExprKind::Bool(*v),
            ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                return scope
                    .get(&place.root)
                    .cloned()
                    .ok_or_else(|| unsupported(owner))
            }
            ResolvedExprKind::Unary { op, value } => ExprKind::Unary {
                op: *op,
                value: Box::new(self.expression(value, scope, owner)?),
            },
            ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
                return self.expression(tail, scope, owner)
            }
            ResolvedExprKind::Binary { op, left, right } => {
                // Calls under lazy operands need a path-sensitive summary
                // predicate. Keep that extension outside this first profile.
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    return Err(unsupported(owner));
                }
                ExprKind::Binary {
                    op: *op,
                    left: Box::new(self.expression(left, scope, owner)?),
                    right: Box::new(self.expression(right, scope, owner)?),
                }
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } if type_arguments.is_empty() && instance.is_none() => {
                let id = callee.as_str();
                let callee = self
                    .functions
                    .get(id)
                    .ok_or_else(|| Refusal::Missing { id: id.into() })?;
                let callee = (*callee).clone();
                if callee.params.len() != args.len() {
                    return Err(unsupported(id));
                }
                let args = args
                    .iter()
                    .map(|arg| self.expression(arg, scope, owner))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut index = self.frames.len();
                let output = loop {
                    let candidate = format!("spx_summary_output_{index}");
                    if self.reserved.insert(candidate.clone()) {
                        break candidate;
                    }
                    index += 1;
                };
                self.frames.push(Frame {
                    callee,
                    args,
                    output: output.clone(),
                });
                return Ok(var(&output));
            }
            _ => return Err(unsupported(owner)),
        };
        Ok(node(kind))
    }
}

fn instantiate(expr: &ResolvedExpr, scope: &Scope, owner: &str) -> Result<Expr, Refusal> {
    let kind = match &expr.kind {
        ResolvedExprKind::Int(v) => ExprKind::Int(*v),
        ResolvedExprKind::Int32(v) => ExprKind::Int32(*v),
        ResolvedExprKind::Uint8(v) => ExprKind::Uint8(*v),
        ResolvedExprKind::Usize(v) => ExprKind::Usize(*v),
        ResolvedExprKind::Bool(v) => ExprKind::Bool(*v),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => {
            return scope
                .get(&place.root)
                .cloned()
                .ok_or_else(|| unsupported(owner))
        }
        ResolvedExprKind::Unary { op, value } => ExprKind::Unary {
            op: *op,
            value: Box::new(instantiate(value, scope, owner)?),
        },
        ResolvedExprKind::Binary { op, left, right } => ExprKind::Binary {
            op: *op,
            left: Box::new(instantiate(left, scope, owner)?),
            right: Box::new(instantiate(right, scope, owner)?),
        },
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => ExprKind::If {
            condition: Box::new(instantiate(condition, scope, owner)?),
            then_branch: Box::new(instantiate(then_branch, scope, owner)?),
            else_branch: Box::new(instantiate(else_branch, scope, owner)?),
        },
        _ => return Err(unsupported(owner)),
    };
    Ok(node(kind))
}

#[derive(Clone, Debug)]
pub struct ModularProof {
    pub plan: Plan,
    pub checked_callee_clauses: Vec<ProvedClause>,
    pub caller_precondition_scripts: Vec<String>,
    pub caller_postcondition_scripts: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum ModularFailure {
    Refused(Refusal),
    CalleeProof(String),
    Precondition { callee: String, reason: String },
    Postcondition { reason: String },
}

fn prove_boolean(
    template: &Function,
    params: &[Param],
    assumptions: &[Expr],
    predicate: Expr,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> Result<String, String> {
    let mut subject = template.clone();
    subject.params = params.to_vec();
    subject.return_type = Type::Bool;
    subject.requires = assumptions.to_vec();
    subject.body = predicate;
    subject.ensures = vec![var("result")];
    match smt::discharge_postcondition(&subject, 0, provisioning, limits) {
        DischargeOutcome::Proved { script_digest, .. } => Ok(script_digest),
        DischargeOutcome::Refuted { .. } => {
            Err("abstract counterexample; no real caller witness established".into())
        }
        DischargeOutcome::Inconclusive { reason } => Err(reason),
    }
}

/// Prove a straight-line caller using exact, separately checked callee
/// summaries. This is additive to bounded inlining. Refuted abstract models
/// are never reported as concrete caller failures without source replay.
pub fn prove_straight_line(
    revision: &ProjectRevision,
    target: &str,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> Result<ModularProof, ModularFailure> {
    let plan = plan(revision, target).map_err(ModularFailure::Refused)?;
    let mut checked_callee_clauses = Vec::new();
    for summary in plan
        .summaries
        .iter()
        .take(plan.summaries.len().saturating_sub(1))
    {
        let proof = prove_postconditions(revision, &summary.declaration_id, provisioning, limits)
            .map_err(|failure| ModularFailure::CalleeProof(format!("{failure:?}")))?;
        if proof.plan.project_revision != plan.project_revision
            || proof.plan.summaries.last().map(|row| &row.digest) != Some(&summary.digest)
        {
            return Err(ModularFailure::CalleeProof("dependency drift".into()));
        }
        checked_callee_clauses.extend(
            proof
                .clauses
                .into_iter()
                .filter(|clause| clause.declaration_id == summary.declaration_id),
        );
    }
    let target_fn = functions(revision)
        .get(target)
        .copied()
        .ok_or_else(|| ModularFailure::Refused(Refusal::Missing { id: target.into() }))?
        .clone();
    let mut template = None;
    for source in revision.sources() {
        let parsed = crate::parse(source.source(), source.path()).map_err(|_| {
            ModularFailure::Refused(Refusal::MissingSource {
                id: source.path().into(),
            })
        })?;
        for function in &parsed.functions {
            if function.stable_id == target {
                if template.replace(function.clone()).is_some() {
                    return Err(ModularFailure::Refused(Refusal::MissingSource {
                        id: target.into(),
                    }));
                }
            }
        }
    }
    let mut template = template
        .ok_or_else(|| ModularFailure::Refused(Refusal::MissingSource { id: target.into() }))?;
    if template.ensures.is_empty() {
        return Err(ModularFailure::Refused(unsupported(target)));
    }
    for contract in target_fn.requires.iter().chain(&target_fn.ensures) {
        let mut calls = Vec::<CallOccurrence>::new();
        collect(contract, target, &mut calls).map_err(ModularFailure::Refused)?;
        if !calls.is_empty() {
            return Err(ModularFailure::Refused(unsupported(target)));
        }
    }
    if template.params.len() != target_fn.params.len() {
        return Err(ModularFailure::Refused(unsupported(target)));
    }
    let mut scope = Scope::new();
    let mut reserved = BTreeSet::new();
    reserved.insert("result".into());
    for (source_param, resolved_param) in template.params.iter().zip(&target_fn.params) {
        reserved.insert(source_param.name.clone());
        scope.insert(resolved_param.id.clone(), var(&source_param.name));
    }
    let mut symbolic = Symbolic {
        functions: functions(revision),
        reserved,
        frames: Vec::new(),
    };
    let body = symbolic
        .expression(&target_fn.body, &scope, target)
        .map_err(ModularFailure::Refused)?;
    let frames = symbolic.frames;
    if frames.is_empty() {
        return Err(ModularFailure::Refused(unsupported(target)));
    }
    let mut params = template.params.clone();
    for frame in &frames {
        params.push(Param {
            name: frame.output.clone(),
            mode: ParamMode::Value,
            ty: ast_type(&frame.callee.return_type).map_err(ModularFailure::Refused)?,
            span: Span::default(),
        });
    }
    let mut assumptions = template.requires.clone();
    let mut precondition_scripts = Vec::new();
    for frame in &frames {
        let id = frame.callee.id.as_str();
        let mut call_scope = Scope::new();
        for (formal, actual) in frame.callee.params.iter().zip(&frame.args) {
            call_scope.insert(formal.id.clone(), actual.clone());
            let tautology = node(ExprKind::Binary {
                op: BinaryOp::Eq,
                left: Box::new(actual.clone()),
                right: Box::new(actual.clone()),
            });
            precondition_scripts.push(
                prove_boolean(
                    &template,
                    &params,
                    &assumptions,
                    tautology,
                    provisioning,
                    limits,
                )
                .map_err(|reason| ModularFailure::Precondition {
                    callee: id.into(),
                    reason,
                })?,
            );
        }
        for require in &frame.callee.requires {
            let instantiated =
                instantiate(require, &call_scope, id).map_err(ModularFailure::Refused)?;
            precondition_scripts.push(
                prove_boolean(
                    &template,
                    &params,
                    &assumptions,
                    instantiated.clone(),
                    provisioning,
                    limits,
                )
                .map_err(|reason| ModularFailure::Precondition {
                    callee: id.into(),
                    reason,
                })?,
            );
            assumptions.push(instantiated);
        }
        call_scope.insert(frame.callee.result_id.clone(), var(&frame.output));
        for ensure in &frame.callee.ensures {
            assumptions
                .push(instantiate(ensure, &call_scope, id).map_err(ModularFailure::Refused)?);
        }
    }
    template.params = params;
    template.requires = assumptions;
    template.body = body;
    let mut caller_postcondition_scripts = Vec::new();
    for index in 0..template.ensures.len() {
        match smt::discharge_postcondition(&template, index, provisioning, limits) {
            DischargeOutcome::Proved { script_digest, .. } => {
                caller_postcondition_scripts.push(script_digest)
            }
            DischargeOutcome::Refuted { .. } => {
                return Err(ModularFailure::Postcondition {
                    reason: "abstract counterexample; summary may be too weak".into(),
                })
            }
            DischargeOutcome::Inconclusive { reason } => {
                return Err(ModularFailure::Postcondition { reason })
            }
        }
    }
    Ok(ModularProof {
        plan,
        checked_callee_clauses,
        caller_precondition_scripts: precondition_scripts,
        caller_postcondition_scripts,
    })
}
