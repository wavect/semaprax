//! Read-only "next construct" projection for Universal Semantic Query v1.
//!
//! Given one exact checked expression inside one retained function or
//! function-template declaration, report the closed, bounded construct
//! vocabulary genuinely admissible at that position: whether a literal of the
//! expected type is admissible, which of the enclosing declaration's own
//! parameters may stand in for it, and which retained functions or templates
//! returning that exact type may be called there. This is a checked answer
//! bound to the exact retained HIR of the selected revision, not a grammar
//! suggestion: every admitted entry is derived only from facts the compiler
//! already verified (the expression's checked type and ownership mode,
//! Explicit Mutation v1's own closed Copy-scalar set, each candidate
//! parameter's declared ownership mode, and each candidate function's
//! declared effect list) rather than from new inference this module invents.
//!
//! Scope this module deliberately does not cover, so it never returns a
//! false positive:
//!
//! - Local `let` bindings, match bindings, and closure captures are not
//!   candidates in v1; only the enclosing declaration's own parameters are,
//!   because their lexical visibility is unconditional (no block-scope walk
//!   is needed to prove it).
//! - A parameter declared `own` is never admitted as a value candidate, even
//!   when its type matches, because proving it has not already been moved
//!   away earlier in the body needs flow-sensitive tracking across branches
//!   and loops that this module does not perform. It is reported excluded
//!   with reason `flow_sensitive_availability_not_computed` instead of
//!   guessed either way.
//! - A candidate identifier's own declared ownership mode must exactly match
//!   the checked ownership mode already required at the target position
//!   (`value`, `borrow`, `shared`, or `own`); a type match under a different
//!   mode is silently omitted rather than asserted, since this module does
//!   not verify implicit borrow-of-owned or borrow-of-temporary admission.
//! - A `call` candidate is offered only at a `value` or `own` position,
//!   never at a `borrow`/`shared` position, matching the documented rule
//!   that a borrowed parameter must name an existing binding rather than a
//!   call result.
//! - A `call` candidate is admitted only when the callee's declared
//!   `uses { .. }` effect set is already a subset of the target
//!   declaration's own declared effects, mirroring the exact `SPX-E102`
//!   check the verifier performs on a direct call; it is not proof that the
//!   call's arguments, contracts, or nested effects will validate.
//!
//! None of this proves a completed program will verify; full validation
//! after generation remains mandatory.

use std::collections::BTreeMap;

use serde_json::json;

use crate::hir::{is_scalar_resolved_type, OwnershipMode, ResolvedExprKind};

use super::semantic_query::{
    capacity, invalid, render, MAX_SEMANTIC_QUERY_RESULT_BYTES,
    SEMANTIC_QUERY_NEXT_CONSTRUCTS_SCHEMA,
};
use super::semantic_query_facts::{programs, walk_expression, MAX_FACT_WALK};
use super::SemanticWorkspaceSnapshot;

/// Each of the two lists (`admitted`, `excluded`) is capped independently
/// after deterministic sorting, so truncation always drops the same
/// lexicographic tail rather than an order-dependent one.
const MAX_CANDIDATES_PER_LIST: usize = 128;

type Result<T> = std::result::Result<T, Vec<crate::diagnostic::Diagnostic>>;

#[derive(Clone, Debug, PartialEq)]
struct ParamFact {
    name: String,
    ownership: OwnershipMode,
    type_identity: String,
}

#[derive(Clone, Debug, PartialEq)]
struct TargetFact {
    expected_type_identity: String,
    position_ownership_mode: OwnershipMode,
    expression_kind: &'static str,
    literal_admissible: bool,
    declared_effects: Vec<String>,
    params: Vec<ParamFact>,
}

pub(super) fn next_constructs_payload(
    snapshot: &SemanticWorkspaceSnapshot,
    stable_id: &str,
    expression_id: &str,
) -> Result<String> {
    let revision = snapshot.generation().revision();
    let mut facts = Vec::new();
    for program in programs(revision) {
        for function in &program.functions {
            if function.id.as_str() == stable_id {
                facts.push(target_fact(
                    expression_id,
                    &function.params,
                    &function.effects,
                    function
                        .requires
                        .iter()
                        .chain(std::iter::once(&function.body))
                        .chain(&function.ensures),
                )?);
            }
        }
        for template in &program.function_templates {
            if template.id.as_str() == stable_id {
                facts.push(target_fact(
                    expression_id,
                    &template.params,
                    &template.effects,
                    template
                        .requires
                        .iter()
                        .chain(std::iter::once(&template.body))
                        .chain(&template.ensures),
                )?);
            }
        }
    }
    let Some(first) = facts.first() else {
        return Err(invalid(
            "semantic next-construct query stable function declaration is unknown",
        ));
    };
    if facts.iter().skip(1).any(|fact| fact != first) {
        return Err(invalid(
            "semantic next-construct query found conflicting retained HIR facts",
        ));
    }
    let target = first.clone();

    let mut admitted = Vec::new();
    let mut excluded = Vec::new();

    if target.literal_admissible {
        admitted.push((
            "literal",
            String::new(),
            json!({
                "construct": "literal",
                "insertion_template": "<literal>",
                "type_identity": target.expected_type_identity,
            }),
        ));
    }

    for param in &target.params {
        if param.type_identity != target.expected_type_identity
            || param.ownership != target.position_ownership_mode
        {
            continue;
        }
        if param.ownership == OwnershipMode::Own {
            excluded.push((
                "parameter_reference",
                param.name.clone(),
                json!({
                    "construct": "parameter_reference",
                    "name": param.name,
                    "reason": "flow_sensitive_availability_not_computed",
                }),
            ));
        } else {
            admitted.push((
                "parameter_reference",
                param.name.clone(),
                json!({
                    "construct": "parameter_reference",
                    "insertion_template": param.name,
                    "name": param.name,
                    "ownership_mode": ownership_name(param.ownership),
                }),
            ));
        }
    }

    if matches!(
        target.position_ownership_mode,
        OwnershipMode::Value | OwnershipMode::Own
    ) {
        let mut declarations = BTreeMap::<String, CallFact>::new();
        let mut visited = 0usize;
        for program in programs(revision) {
            for function in &program.functions {
                visited = visited.saturating_add(1);
                if visited > MAX_FACT_WALK {
                    return Err(capacity(
                        "semantic next-construct query exceeds its declaration walk bound",
                    ));
                }
                index_call_candidate(
                    &mut declarations,
                    function.id.as_str(),
                    &function.name,
                    function.return_type.identity_key(),
                    &function.effects,
                    function.params.len(),
                )?;
            }
            for template in &program.function_templates {
                visited = visited.saturating_add(1);
                if visited > MAX_FACT_WALK {
                    return Err(capacity(
                        "semantic next-construct query exceeds its declaration walk bound",
                    ));
                }
                index_call_candidate(
                    &mut declarations,
                    template.id.as_str(),
                    &template.name,
                    template.return_type.identity_key(),
                    &template.effects,
                    template.params.len(),
                )?;
            }
        }
        for (id, fact) in declarations {
            if fact.return_type_identity != target.expected_type_identity {
                continue;
            }
            let missing = fact
                .effects
                .iter()
                .filter(|effect| !target.declared_effects.iter().any(|owned| owned == *effect))
                .cloned()
                .collect::<Vec<_>>();
            if missing.is_empty() {
                admitted.push((
                    "call",
                    id.clone(),
                    json!({
                        "arity": fact.arity,
                        "construct": "call",
                        "insertion_template": format!("{}(...)", fact.name),
                        "name": fact.name,
                        "stable_id": id,
                    }),
                ));
            } else {
                excluded.push((
                    "call",
                    id.clone(),
                    json!({
                        "construct": "call",
                        "missing_effects": missing,
                        "name": fact.name,
                        "reason": "effect_not_available",
                        "stable_id": id,
                    }),
                ));
            }
        }
    }

    admitted.sort_by(|left, right| left.0.cmp(right.0).then_with(|| left.1.cmp(&right.1)));
    excluded.sort_by(|left, right| left.0.cmp(right.0).then_with(|| left.1.cmp(&right.1)));
    let admitted_total = admitted.len();
    let excluded_total = excluded.len();
    let admitted_truncated = admitted_total > MAX_CANDIDATES_PER_LIST;
    let excluded_truncated = excluded_total > MAX_CANDIDATES_PER_LIST;
    admitted.truncate(MAX_CANDIDATES_PER_LIST);
    excluded.truncate(MAX_CANDIDATES_PER_LIST);

    let generation = snapshot.generation();
    render(
        json!({
            "admitted": admitted.into_iter().map(|(_, _, value)| value).collect::<Vec<_>>(),
            "excluded": excluded.into_iter().map(|(_, _, value)| value).collect::<Vec<_>>(),
            "expected_type_identity": target.expected_type_identity,
            "expression_id": expression_id,
            "expression_kind": target.expression_kind,
            "image_digest": generation.image().image_digest(),
            "limits": {
                "max_candidates_per_list": MAX_CANDIDATES_PER_LIST,
                "max_walk": MAX_FACT_WALK,
            },
            "nonclaims": [
                "admitted_does_not_prove_a_completed_program_will_verify",
                "argument_shapes_contracts_and_nested_effects_of_a_call_are_not_checked",
                "local_let_match_and_closure_bindings_are_not_yet_covered_candidates",
                "flow_sensitive_move_state_across_branches_and_loops_is_not_computed",
                "vocabulary_is_closed_to_literal_parameter_reference_and_call_in_v1",
            ],
            "position_ownership_mode": ownership_name(target.position_ownership_mode),
            "project_revision": generation.revision().project_revision(),
            "schema": SEMANTIC_QUERY_NEXT_CONSTRUCTS_SCHEMA,
            "stable_id": stable_id,
            "totals": {
                "admitted": admitted_total,
                "admitted_truncated": admitted_truncated,
                "excluded": excluded_total,
                "excluded_truncated": excluded_truncated,
            },
            "vocabulary": ["literal", "parameter_reference", "call"],
            "workspace_revision": generation.workspace_revision(),
        }),
        MAX_SEMANTIC_QUERY_RESULT_BYTES,
        true,
    )
}

struct CallFact {
    name: String,
    return_type_identity: String,
    effects: Vec<String>,
    arity: usize,
}

fn index_call_candidate(
    declarations: &mut BTreeMap<String, CallFact>,
    id: &str,
    name: &str,
    return_type_identity: String,
    effects: &[String],
    arity: usize,
) -> Result<()> {
    let mut sorted_effects = effects.to_vec();
    sorted_effects.sort();
    let fact = CallFact {
        name: name.to_owned(),
        return_type_identity,
        effects: sorted_effects,
        arity,
    };
    if let Some(existing) = declarations.get(id) {
        if existing.name != fact.name
            || existing.return_type_identity != fact.return_type_identity
            || existing.effects != fact.effects
            || existing.arity != fact.arity
        {
            return Err(invalid(
                "semantic next-construct query found conflicting retained function facts",
            ));
        }
        return Ok(());
    }
    declarations.insert(id.to_owned(), fact);
    Ok(())
}

fn target_fact<'a>(
    expression_id: &str,
    params: &[crate::hir::ResolvedParam],
    effects: &[String],
    roots: impl Iterator<Item = &'a crate::hir::ResolvedExpr>,
) -> Result<TargetFact> {
    let mut matches = Vec::new();
    let mut visited = 0usize;
    for root in roots {
        walk_expression(root, &mut visited, &mut |expression| {
            if expression.id.as_str() == expression_id {
                matches.push(expression);
            }
        })?;
    }
    let [expression] = matches.as_slice() else {
        return Err(invalid(
            "semantic next-construct query expression is not uniquely owned by the selected declaration",
        ));
    };
    let mut declared_effects = effects.to_vec();
    declared_effects.sort();
    Ok(TargetFact {
        expected_type_identity: expression.ty.identity_key(),
        position_ownership_mode: expression.ownership,
        expression_kind: expression_kind(&expression.kind),
        literal_admissible: is_scalar_resolved_type(&expression.ty),
        declared_effects,
        params: params
            .iter()
            .map(|param| ParamFact {
                name: param.name.clone(),
                ownership: param.ownership,
                type_identity: param.ty.identity_key(),
            })
            .collect(),
    })
}

fn ownership_name(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

fn expression_kind(kind: &ResolvedExprKind) -> &'static str {
    // Deliberately coarse: only used to label the payload, never to gate
    // admission. The closed per-kind name list is owned by
    // `semantic_query_facts::expression_kind`; this module does not depend on
    // its exact arms and falls back to a stable generic label instead of
    // duplicating that full match.
    match kind {
        ResolvedExprKind::Place(_) | ResolvedExprKind::BorrowPlace { .. } => "place",
        ResolvedExprKind::Invoke { .. } | ResolvedExprKind::Call { .. } => "call",
        _ => "expression",
    }
}
