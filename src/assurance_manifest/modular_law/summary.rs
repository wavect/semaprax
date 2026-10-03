//! Straight-line checked-summary composition draft for LAW-06.
//!
//! All proof state is derived from retained HIR in one invocation. The
//! dependency plan and every callee's postconditions are checked before any
//! summary is consumed; caller preconditions are proved in evaluation order,
//! with only earlier summaries in each query.

use std::collections::{BTreeMap, BTreeSet};

use crate::assurance_manifest::obligation::{obligation_id, ObligationKind};
use crate::assurance_manifest::smt_discharge::{
    self as smt, DischargeOutcome, Provisioning, RunLimits,
};
use crate::ast::{BinaryOp, Expr, ExprKind, Function, Param, ParamMode, Span, Type};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType, ValueId};
use crate::project::ProjectRevision;

use super::{
    collect, functions, inline::inline_subject, plan, prove::ProvedClause, CallOccurrence, Plan,
    Refusal,
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
    expression_id: String,
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
                    return Err(Refusal::LazySummary {
                        owner: owner.into(),
                    });
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
                    expression_id: expr.id.as_str().to_owned(),
                });
                return Ok(var(&output));
            }
            ResolvedExprKind::If { .. } => {
                return Err(Refusal::BranchingSummary {
                    owner: owner.into(),
                })
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
    pub caller_precondition_obligation_ids: Vec<String>,
    pub caller_precondition_scripts: Vec<String>,
    pub caller_postcondition_obligation_ids: Vec<String>,
    pub caller_postcondition_scripts: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum ModularFailure {
    Refused(Refusal),
    CalleeProof(String),
    Precondition { callee: String, reason: String },
    Postcondition { reason: String },
}

pub(super) struct Query {
    pub function: Function,
    pub index: usize,
    pub obligation_id: String,
    pub declaration_id: String,
    pub summary_digest: String,
}

pub(super) struct Prepared {
    pub plan: Plan,
    pub callees: Vec<Query>,
    pub preconditions: Vec<Query>,
    pub caller: Vec<Query>,
}

fn boolean_subject(
    template: &Function,
    params: &[Param],
    assumptions: &[Expr],
    predicate: Expr,
) -> Function {
    let mut subject = template.clone();
    subject.params = params.to_vec();
    subject.return_type = Type::Bool;
    subject.requires = assumptions.to_vec();
    subject.body = predicate;
    subject.ensures = vec![var("result")];
    subject
}

/// Admission and query preparation have no solver/process authority. Query
/// order is the proof order: dependencies, staged call checks, then caller.
/// A consumer must stop at the first non-proof and must not use later
/// summaries to justify an earlier precondition.
pub(super) fn prepare(revision: &ProjectRevision, target: &str) -> Result<Prepared, Refusal> {
    let plan = plan(revision, target)?;
    let target_fn = functions(revision)
        .get(target)
        .copied()
        .ok_or_else(|| Refusal::Missing { id: target.into() })?
        .clone();
    let mut template = None;
    for source in revision.sources() {
        if source.source_graph_schema() == "semaprax.native-law.v1" {
            continue;
        }
        let parsed =
            crate::parse(source.source(), source.path()).map_err(|_| Refusal::MissingSource {
                id: source.path().into(),
            })?;
        for function in &parsed.functions {
            if function.stable_id == target && template.replace(function.clone()).is_some() {
                return Err(Refusal::MissingSource { id: target.into() });
            }
        }
    }
    let mut template = template.ok_or_else(|| Refusal::MissingSource { id: target.into() })?;
    if template.ensures.is_empty() || template.params.len() != target_fn.params.len() {
        return Err(unsupported(target));
    }
    for contract in target_fn.requires.iter().chain(&target_fn.ensures) {
        let mut calls = Vec::<CallOccurrence>::new();
        collect(contract, target, &mut calls)?;
        if !calls.is_empty() {
            return Err(unsupported(target));
        }
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
    let body = symbolic.expression(&target_fn.body, &scope, target)?;
    let frames = symbolic.frames;
    if frames.is_empty() {
        return Err(unsupported(target));
    }

    let mut callees = Vec::new();
    for summary in plan
        .summaries
        .iter()
        .take(plan.summaries.len().saturating_sub(1))
    {
        let function = inline_subject(revision, &summary.declaration_id)?;
        if function.ensures.is_empty() {
            return Err(unsupported(&summary.declaration_id));
        }
        for index in 0..function.ensures.len() {
            callees.push(Query {
                function: function.clone(),
                index,
                obligation_id: smt::postcondition_obligation_id(&summary.declaration_id, index),
                declaration_id: summary.declaration_id.clone(),
                summary_digest: summary.digest.clone(),
            });
        }
    }
    let mut params = template.params.clone();
    for frame in &frames {
        params.push(Param {
            name: frame.output.clone(),
            mode: ParamMode::Value,
            ty: ast_type(&frame.callee.return_type)?,
            span: Span::default(),
        });
    }
    let mut assumptions = template.requires.clone();
    let mut preconditions = Vec::new();
    for frame in &frames {
        let id = frame.callee.id.as_str();
        let mut call_scope = Scope::new();
        for (arg_index, (formal, actual)) in frame.callee.params.iter().zip(&frame.args).enumerate()
        {
            call_scope.insert(formal.id.clone(), actual.clone());
            let tautology = node(ExprKind::Binary {
                op: BinaryOp::Eq,
                left: Box::new(actual.clone()),
                right: Box::new(actual.clone()),
            });
            preconditions.push(Query {
                function: boolean_subject(&template, &params, &assumptions, tautology),
                index: 0,
                obligation_id: obligation_id(
                    ObligationKind::Precondition,
                    target,
                    &format!("call:{}:arg:{arg_index}", frame.expression_id),
                ),
                declaration_id: id.into(),
                summary_digest: String::new(),
            });
        }
        for (require_index, require) in frame.callee.requires.iter().enumerate() {
            let instantiated = instantiate(require, &call_scope, id)?;
            preconditions.push(Query {
                function: boolean_subject(&template, &params, &assumptions, instantiated.clone()),
                index: 0,
                obligation_id: obligation_id(
                    ObligationKind::Precondition,
                    target,
                    &format!("call:{}:requires:{require_index}", frame.expression_id),
                ),
                declaration_id: id.into(),
                summary_digest: String::new(),
            });
            assumptions.push(instantiated);
        }
        call_scope.insert(frame.callee.result_id.clone(), var(&frame.output));
        for ensure in &frame.callee.ensures {
            assumptions.push(instantiate(ensure, &call_scope, id)?);
        }
    }
    template.params = params;
    template.requires = assumptions;
    template.body = body;
    let caller_digest = plan
        .summaries
        .last()
        .map(|row| row.digest.clone())
        .unwrap_or_default();
    let caller = (0..template.ensures.len())
        .map(|index| Query {
            function: template.clone(),
            index,
            obligation_id: smt::postcondition_obligation_id(target, index),
            declaration_id: target.into(),
            summary_digest: caller_digest.clone(),
        })
        .collect();
    Ok(Prepared {
        plan,
        callees,
        preconditions,
        caller,
    })
}

pub(super) fn prove_with<F>(
    revision: &ProjectRevision,
    target: &str,
    mut discharge: F,
) -> Result<ModularProof, ModularFailure>
where
    F: FnMut(&Function, usize) -> Result<DischargeOutcome, String>,
{
    let prepared = prepare(revision, target).map_err(ModularFailure::Refused)?;
    let mut checked_callee_clauses = Vec::new();
    for query in &prepared.callees {
        match discharge(&query.function, query.index).map_err(ModularFailure::CalleeProof)? {
            DischargeOutcome::Proved {
                script_digest,
                solver_identity,
                solver_version,
            } => {
                checked_callee_clauses.push(ProvedClause {
                    declaration_id: query.declaration_id.clone(),
                    obligation_id: query.obligation_id.clone(),
                    summary_digest: query.summary_digest.clone(),
                    ensures_index: query.index,
                    script_digest,
                    solver_identity,
                    solver_version,
                });
            }
            DischargeOutcome::Refuted { .. } => {
                return Err(ModularFailure::CalleeProof("callee refuted".into()))
            }
            DischargeOutcome::Inconclusive { reason } => {
                return Err(ModularFailure::CalleeProof(reason))
            }
        }
    }
    let mut precondition_obligation_ids = Vec::new();
    let mut precondition_scripts = Vec::new();
    for query in &prepared.preconditions {
        match discharge(&query.function, query.index).map_err(|reason| {
            ModularFailure::Precondition {
                callee: query.declaration_id.clone(),
                reason,
            }
        })? {
            DischargeOutcome::Proved { script_digest, .. } => {
                precondition_obligation_ids.push(query.obligation_id.clone());
                precondition_scripts.push(script_digest)
            }
            DischargeOutcome::Refuted { .. } => {
                return Err(ModularFailure::Precondition {
                    callee: query.declaration_id.clone(),
                    reason: "abstract counterexample; no real caller witness established".into(),
                })
            }
            DischargeOutcome::Inconclusive { reason } => {
                return Err(ModularFailure::Precondition {
                    callee: query.declaration_id.clone(),
                    reason,
                })
            }
        }
    }
    let mut caller_postcondition_obligation_ids = Vec::new();
    let mut caller_postcondition_scripts = Vec::new();
    for query in &prepared.caller {
        match discharge(&query.function, query.index)
            .map_err(|reason| ModularFailure::Postcondition { reason })?
        {
            DischargeOutcome::Proved { script_digest, .. } => {
                caller_postcondition_obligation_ids.push(query.obligation_id.clone());
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
        plan: prepared.plan,
        checked_callee_clauses,
        caller_precondition_obligation_ids: precondition_obligation_ids,
        caller_precondition_scripts: precondition_scripts,
        caller_postcondition_obligation_ids,
        caller_postcondition_scripts,
    })
}

/// Prove a straight-line caller using exact, separately checked callee
/// summaries and the explicit Z3 path. The installed Project route uses the
/// same prepared queries through its registered process provider.
pub fn prove_straight_line(
    revision: &ProjectRevision,
    target: &str,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> Result<ModularProof, ModularFailure> {
    prove_with(revision, target, |function, index| {
        Ok(smt::discharge_postcondition(
            function,
            index,
            provisioning,
            limits,
        ))
    })
}
