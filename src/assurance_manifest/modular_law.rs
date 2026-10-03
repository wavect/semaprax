//! Exact, authority-free admission and identity plan for modular scalar laws.
//!
//! This plan is not a proof. A summary may be consumed only after the selected
//! installed tool has proved its own obligations and every dependency in this
//! exact topological order. The per-function digest excludes unrelated source
//! rows; the separate Project revision still binds any eventual attachment.

pub mod inline;
pub mod prove;

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest as _, Sha256};

use crate::ast::BinaryOp;
use crate::hir::{
    DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedStatement, ResolvedType,
};
use crate::project::ProjectRevision;

pub const PROFILE: &str = "semaprax.modular-scalar-law-plan.v1";
const MAX_FUNCTIONS: usize = 64;
const MAX_CALLS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    Missing { id: String },
    MissingSource { id: String },
    Capacity,
    Cycle { id: String },
    Effectful { id: String },
    NonScalar { id: String },
    Generic { id: String },
    DynamicCall { owner: String },
    ForeignCall { owner: String },
    UnsupportedExpr { owner: String },
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "missing_function",
            Self::MissingSource { .. } => "missing_source_identity",
            Self::Capacity => "capacity",
            Self::Cycle { .. } => "cyclic_summary",
            Self::Effectful { .. } => "effectful_function",
            Self::NonScalar { .. } => "non_scalar_function",
            Self::Generic { .. } => "generic_function",
            Self::DynamicCall { .. } => "dynamic_call",
            Self::ForeignCall { .. } => "foreign_call",
            Self::UnsupportedExpr { .. } => "unsupported_expression",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallOccurrence {
    pub expression_id: String,
    pub callee: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryDependency {
    pub callee: String,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Summary {
    pub declaration_id: String,
    pub local_digest: String,
    pub digest: String,
    pub calls: Vec<CallOccurrence>,
    pub dependencies: Vec<SummaryDependency>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan {
    pub profile: &'static str,
    pub project_revision: String,
    pub target: String,
    /// Callees precede their callers. Every row is derived from retained HIR.
    pub summaries: Vec<Summary>,
}

fn digest(parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.modular-scalar-law-summary.v1\0");
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn scalar(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::Bool
    )
}

fn admit_signature(function: &ResolvedFunction) -> Result<(), Refusal> {
    let id = function.id.as_str().to_owned();
    if !function.effects.is_empty() || function.yields.is_some() {
        return Err(Refusal::Effectful { id });
    }
    if !scalar(&function.return_type)
        || function
            .params
            .iter()
            .any(|param| !scalar(&param.ty) || param.ownership != OwnershipMode::Value)
    {
        return Err(Refusal::NonScalar { id });
    }
    Ok(())
}

fn collect(
    expr: &ResolvedExpr,
    owner: &str,
    calls: &mut Vec<CallOccurrence>,
) -> Result<(), Refusal> {
    if !scalar(&expr.ty) || expr.ownership != OwnershipMode::Value {
        return Err(Refusal::NonScalar {
            id: owner.to_owned(),
        });
    }
    match &expr.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Bool(_) => Ok(()),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => Ok(()),
        ResolvedExprKind::Unary { value, .. } => collect(value, owner, calls),
        ResolvedExprKind::Binary { op, left, right } => {
            if matches!(op, BinaryOp::Div | BinaryOp::Rem)
                || (*op == BinaryOp::Mul
                    && !matches!(
                        left.kind,
                        ResolvedExprKind::Int(_)
                            | ResolvedExprKind::Int32(_)
                            | ResolvedExprKind::Uint8(_)
                            | ResolvedExprKind::Usize(_)
                    )
                    && !matches!(
                        right.kind,
                        ResolvedExprKind::Int(_)
                            | ResolvedExprKind::Int32(_)
                            | ResolvedExprKind::Uint8(_)
                            | ResolvedExprKind::Usize(_)
                    ))
            {
                return Err(Refusal::UnsupportedExpr {
                    owner: owner.into(),
                });
            }
            collect(left, owner, calls)?;
            collect(right, owner, calls)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect(condition, owner, calls)?;
            collect(then_branch, owner, calls)?;
            collect(else_branch, owner, calls)
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let {
                    binding,
                    mutable: false,
                    value,
                    ..
                } = statement
                else {
                    return Err(Refusal::UnsupportedExpr {
                        owner: owner.into(),
                    });
                };
                if !scalar(&binding.ty) || binding.ownership != OwnershipMode::Value {
                    return Err(Refusal::NonScalar { id: owner.into() });
                }
                collect(value, owner, calls)?;
            }
            collect(tail, owner, calls)
        }
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            if !type_arguments.is_empty() || instance.is_some() {
                return Err(Refusal::Generic {
                    id: callee.as_str().into(),
                });
            }
            for arg in args {
                collect(arg, owner, calls)?;
            }
            if calls.len() >= MAX_CALLS {
                return Err(Refusal::Capacity);
            }
            calls.push(CallOccurrence {
                expression_id: expr.id.as_str().to_owned(),
                callee: callee.as_str().to_owned(),
            });
            Ok(())
        }
        ResolvedExprKind::Invoke { .. }
        | ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::Closure { .. } => Err(Refusal::DynamicCall {
            owner: owner.into(),
        }),
        ResolvedExprKind::NativeRustImportCall(_) | ResolvedExprKind::HostCommandCall(_) => {
            Err(Refusal::ForeignCall {
                owner: owner.into(),
            })
        }
        _ => Err(Refusal::UnsupportedExpr {
            owner: owner.into(),
        }),
    }
}

fn source_bodies(revision: &ProjectRevision) -> Result<BTreeMap<String, String>, Refusal> {
    let mut bodies = BTreeMap::new();
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), source.path()).map_err(|_| Refusal::MissingSource {
                id: source.path().into(),
            })?;
        for function in &program.functions {
            let span = function.span;
            let text = source.source().get(span.start..span.end).ok_or_else(|| {
                Refusal::MissingSource {
                    id: function.stable_id.clone(),
                }
            })?;
            if text.is_empty()
                || bodies
                    .insert(function.stable_id.clone(), text.into())
                    .is_some()
            {
                return Err(Refusal::MissingSource {
                    id: function.stable_id.clone(),
                });
            }
        }
    }
    Ok(bodies)
}

fn functions<'a>(revision: &'a ProjectRevision) -> BTreeMap<String, &'a ResolvedFunction> {
    let mut functions = BTreeMap::new();
    for program in [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ] {
        for function in &program.functions {
            functions
                .entry(function.id.as_str().to_owned())
                .or_insert(function);
        }
    }
    functions
}

struct Planner<'a> {
    functions: BTreeMap<String, &'a ResolvedFunction>,
    bodies: BTreeMap<String, String>,
    active: BTreeSet<String>,
    completed: BTreeMap<String, String>,
    summaries: Vec<Summary>,
}

impl Planner<'_> {
    fn visit(&mut self, id: &str) -> Result<String, Refusal> {
        if let Some(digest) = self.completed.get(id) {
            return Ok(digest.clone());
        }
        if !self.active.insert(id.to_owned()) {
            return Err(Refusal::Cycle { id: id.into() });
        }
        if self.active.len() > MAX_FUNCTIONS || self.summaries.len() >= MAX_FUNCTIONS {
            return Err(Refusal::Capacity);
        }
        let function = self
            .functions
            .get(id)
            .ok_or_else(|| Refusal::Missing { id: id.into() })?;
        admit_signature(function)?;
        let body = self
            .bodies
            .get(id)
            .ok_or_else(|| Refusal::MissingSource { id: id.into() })?;
        let local_digest = digest(&[PROFILE, env!("CARGO_PKG_VERSION"), id, body]);
        let mut calls = Vec::new();
        for expr in &function.requires {
            collect(expr, id, &mut calls)?;
        }
        collect(&function.body, id, &mut calls)?;
        for expr in &function.ensures {
            collect(expr, id, &mut calls)?;
        }
        // Drop the shared borrow before visiting transitive dependencies.
        let mut dependencies = Vec::new();
        for call in &calls {
            let child_digest = self.visit(&call.callee)?;
            dependencies.push(SummaryDependency {
                callee: call.callee.clone(),
                digest: child_digest,
            });
        }
        let mut parts = vec![local_digest.as_str()];
        for dependency in &dependencies {
            parts.push(dependency.callee.as_str());
            parts.push(dependency.digest.as_str());
        }
        let semantic_digest = digest(&parts);
        self.active.remove(id);
        self.completed.insert(id.into(), semantic_digest.clone());
        self.summaries.push(Summary {
            declaration_id: id.into(),
            local_digest,
            digest: semantic_digest.clone(),
            calls,
            dependencies,
        });
        Ok(semantic_digest)
    }
}

/// Build an exact dependency order from already linked Project HIR and
/// authenticated source bytes. This does not consult a solver or promote a law.
pub fn plan(revision: &ProjectRevision, target: &str) -> Result<Plan, Refusal> {
    let mut planner = Planner {
        functions: functions(revision),
        bodies: source_bodies(revision)?,
        active: BTreeSet::new(),
        completed: BTreeMap::new(),
        summaries: Vec::new(),
    };
    planner.visit(target)?;
    Ok(Plan {
        profile: PROFILE,
        project_revision: revision.project_revision().into(),
        target: target.into(),
        summaries: planner.summaries,
    })
}
