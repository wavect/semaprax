//! Control-dependent resumable plan (issue #296, plan identity v3).
//!
//! The sequential plan admits only direct top-level `yield` sites. This plan
//! admits the structured-control placements `parser::yields` accepts: a
//! `yield` as the direct `let`/assignment value of a block reached from the
//! body only through `if`/`else` branches, `while` bodies, or block-valued
//! slots. Every site is a static compiler-owned state; the dynamic control
//! state at a suspension is the ordered list of sites already settled, which
//! the continuation binding commits to together with their answers. Resume
//! replays the pure prefix and must reach exactly the recorded site sequence,
//! so a branch decision or loop count is never taken from the continuation.
//!
//! The profile is the same Copy-scalar, effect-free, cleanup-free one as the
//! sequential plan. Loops are bounded dynamically: at most
//! [`MAX_CONTROL_SUSPENSIONS`] suspensions per invocation. No yield-free
//! backend projection exists for this plan, so ordinary and prepared
//! native/Wasm lanes keep refusing it.

use super::{
    frame, hash_scalar, invalid, reject_reachable_resumable_callees, reject_yield_in_contracts,
    require_scalar_expression_tree, state, ResumablePlanIdentity, ResumableScalar, ResumableState,
    ResumableStateId, ResumableStateKind, ResumableSuspensionBinding, MAX_RESUMABLE_YIELDS,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ExpressionId, IdentityOrigin, OwnershipMode, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedYieldsClause,
};
use sha2::{Digest, Sha256};

const CONTROL_PLAN_IDENTITY_DOMAIN: &[u8] = b"semaprax.resumable-control-plan.v3\0";
const CONTROL_BINDING_DOMAIN: &[u8] = b"semaprax.resumable-control-binding.v3\0";

/// Dynamic bound on suspensions of one control-dependent invocation.
pub const MAX_CONTROL_SUSPENSIONS: usize = 16;

/// One static suspension site of a control-dependent plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSite {
    pub state: ResumableState,
    pub expression: ExpressionId,
    pub request_expression: ExpressionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlResumablePlan {
    pub function_id: DeclarationId,
    pub identity: ResumablePlanIdentity,
    pub entry: ResumableState,
    pub sites: Vec<ControlSite>,
    pub complete: ResumableState,
    pub request_type: ResolvedType,
    pub response_type: ResolvedType,
}

impl ControlResumablePlan {
    pub fn site_of_expression(&self, expression: &ExpressionId) -> Option<usize> {
        self.sites
            .iter()
            .position(|site| site.expression == *expression)
    }

    pub fn site_of_state(&self, state: &ResumableStateId) -> Option<usize> {
        self.sites.iter().position(|site| site.state.id == *state)
    }

    /// Bind a suspension at static `site` to the exact arguments and the
    /// ordered settled history `(site, answer)`.
    pub fn binding(
        &self,
        site: usize,
        arguments: &[ResumableScalar],
        history: &[(usize, ResumableScalar)],
    ) -> Result<ResumableSuspensionBinding, Diagnostic> {
        let current = self
            .sites
            .get(site)
            .ok_or_else(|| invalid("control suspension site is out of bounds"))?;
        if history.len() >= MAX_CONTROL_SUSPENSIONS {
            return Err(invalid("control suspension history exceeds its bound"));
        }
        let mut hasher = Sha256::new();
        hasher.update(CONTROL_BINDING_DOMAIN);
        frame(&mut hasher, self.identity.as_bytes());
        frame(&mut hasher, current.state.id.as_str().as_bytes());
        hasher.update((arguments.len() as u64).to_le_bytes());
        for argument in arguments {
            hash_scalar(&mut hasher, argument);
        }
        hasher.update((history.len() as u64).to_le_bytes());
        for (settled, answer) in history {
            let settled = self
                .sites
                .get(*settled)
                .ok_or_else(|| invalid("control history site is out of bounds"))?;
            frame(&mut hasher, settled.state.id.as_str().as_bytes());
            hash_scalar(&mut hasher, answer);
        }
        Ok(ResumableSuspensionBinding(hasher.finalize().into()))
    }
}

/// The shared signature/profile preamble of every resumable plan: canonical
/// function, persistent identity, not the entrypoint, `yields`, no effects,
/// Copy-scalar channel and parameters, and no owned cleanup state.
pub(super) fn check_resumable_profile<'a>(
    program: &ResolvedProgram,
    function: &'a ResolvedFunction,
) -> Result<&'a ResolvedYieldsClause, Diagnostic> {
    let canonical = program
        .functions
        .iter()
        .find(|candidate| candidate.id == function.id)
        .ok_or_else(|| invalid("resumable function is absent from the resolved program"))?;
    if canonical != function {
        return Err(invalid(
            "resumable function disagrees with the resolved program's canonical function",
        ));
    }
    let declaration = program
        .declarations
        .declaration(&function.id)
        .ok_or_else(|| invalid("resumable function lacks a declaration-index entry"))?;
    if declaration.identity_origin != IdentityOrigin::Explicit {
        return Err(invalid(
            "resumable function does not have an explicit persistent identity",
        ));
    }
    if program.entrypoint == function.id {
        return Err(invalid(
            "resumable projection cannot replace the program entrypoint",
        ));
    }
    let yields = function
        .yields
        .as_ref()
        .ok_or_else(|| invalid("resumable lowering requires a `yields` clause"))?;
    if !function.effects.is_empty() {
        return Err(invalid(
            "resumable lowering does not admit ordinary effects before suspension",
        ));
    }
    if !hir::is_scalar_resolved_type(&yields.request_type)
        || !hir::is_scalar_resolved_type(&yields.response_type)
        || function
            .params
            .iter()
            .any(|parameter| !hir::is_scalar_resolved_type(&parameter.ty))
    {
        return Err(invalid(
            "resumable lowering requires Copy-scalar request, response, and parameter types",
        ));
    }
    if !function.cleanup.slots.is_empty()
        || !function.cleanup.flags.is_empty()
        || !function
            .cleanup
            .entry_state
            .live_owned_parameters
            .is_empty()
        || !function
            .cleanup
            .entry_state
            .conditional_owned_parameters
            .is_empty()
    {
        return Err(invalid(
            "resumable lowering found owned cleanup state in the Copy-scalar profile",
        ));
    }
    Ok(yields)
}

/// True when some `yield` is not a direct statement value or tail of the
/// function's own block, so only the control plan can lower it.
pub fn is_control_dependent(function: &ResolvedFunction) -> bool {
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        return false;
    };
    let direct = statements
        .iter()
        .filter(|statement| match statement {
            ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => {
                matches!(value.kind, ResolvedExprKind::Yield { .. })
            }
            _ => false,
        })
        .count()
        + usize::from(matches!(tail.kind, ResolvedExprKind::Yield { .. }));
    count_yields(&function.body) != direct
}

fn count_yields(root: &ResolvedExpr) -> usize {
    let mut count = 0;
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Yield { .. }) {
            count += 1;
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    count
}

/// Collect sites in authored order through exactly the placements the
/// parser admits. Any other `yield` makes the counts disagree.
fn collect_sites<'a>(
    expression: &'a ResolvedExpr,
    admits_yield: bool,
    top: bool,
    sites: &mut Vec<&'a ResolvedExpr>,
) {
    match &expression.kind {
        ResolvedExprKind::Yield { .. } if admits_yield => sites.push(expression),
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            collect_sites(then_branch, false, false, sites);
            collect_sites(else_branch, false, false, sites);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. } => {
                        collect_sites(value, true, false, sites)
                    }
                    ResolvedStatement::While { body, .. } => {
                        collect_sites(body, false, false, sites)
                    }
                    ResolvedStatement::Unsafe { .. } => {}
                }
            }
            collect_sites(tail, top, false, sites);
        }
        _ => {}
    }
}

/// Lower a control-dependent function. A function whose yields are all
/// direct top-level sites is refused here: it keeps its sequential plan and
/// v2 envelope identity.
pub fn lower_control(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<ControlResumablePlan, Diagnostic> {
    let yields = check_resumable_profile(program, function)?;
    reject_yield_in_contracts(function)?;
    if !is_control_dependent(function) {
        return Err(invalid(
            "resumable function has only direct top-level yields; use the sequential plan",
        ));
    }
    let mut found = Vec::new();
    collect_sites(&function.body, false, true, &mut found);
    if found.len() != count_yields(&function.body) {
        return Err(invalid(
            "resumable yield is outside the admitted structured-control placements",
        ));
    }
    if !(1..=MAX_RESUMABLE_YIELDS).contains(&found.len()) {
        return Err(invalid(format!(
            "control resumable lowering requires between 1 and {MAX_RESUMABLE_YIELDS} static yield sites; found {}",
            found.len()
        )));
    }
    let mut sites = Vec::with_capacity(found.len());
    for expression in &found {
        let ResolvedExprKind::Yield { request } = &expression.kind else {
            unreachable!("collected sites are yields")
        };
        if request.ty != yields.request_type
            || expression.ty != yields.response_type
            || request.ownership != OwnershipMode::Value
            || expression.ownership != OwnershipMode::Value
        {
            return Err(invalid(
                "resumable yield request/response types or ownership disagree with its declaration",
            ));
        }
        sites.push(ControlSite {
            state: state(
                &function.id,
                ResumableStateKind::Suspended,
                Some(&expression.id),
            ),
            expression: expression.id.clone(),
            request_expression: request.id.clone(),
        });
    }
    require_scalar_expression_tree(&function.body)?;
    reject_reachable_resumable_callees(program, function)?;
    let encoded = crate::cache_codec::encode(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .next()
            .unwrap_or_else(|| invalid("checked-program encoding failed without a diagnostic"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(CONTROL_PLAN_IDENTITY_DOMAIN);
    frame(&mut hasher, &encoded);
    frame(&mut hasher, function.id.as_str().as_bytes());
    hasher.update((sites.len() as u64).to_le_bytes());
    for site in &sites {
        frame(&mut hasher, site.expression.as_str().as_bytes());
    }
    Ok(ControlResumablePlan {
        function_id: function.id.clone(),
        identity: ResumablePlanIdentity(hasher.finalize().into()),
        entry: state(&function.id, ResumableStateKind::Entry, None),
        sites,
        complete: state(&function.id, ResumableStateKind::Complete, None),
        request_type: yields.request_type.clone(),
        response_type: yields.response_type.clone(),
    })
}
