//! Rebase valid history and readmit pending selectors without materialization.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};

use super::ProjectCandidateDraft;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, ResolvedFunction, ResolvedType};
use crate::project::candidate::{expression, intent, parse_revision, rebase, wire};
use crate::project::ProjectRevision;

pub const PROJECT_CANDIDATE_DRAFT_REBASE_SCHEMA: &str =
    "semaprax.project-candidate-draft-rebase.v1";
pub const MAX_PROJECT_CANDIDATE_DRAFT_REBASE_BYTES: usize = 1024 * 1024;
const MAX_VISITS: usize = 1_048_576;
const MAX_DEPTH: usize = 256;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// A still-opaque draft and descriptive ancestry. Only the ordinary completion
/// API may release its candidate after all pending holes have been filled.
pub struct ProjectCandidateDraftRebase {
    draft: ProjectCandidateDraft,
    json: String,
}
impl ProjectCandidateDraftRebase {
    pub fn draft(&self) -> &ProjectCandidateDraft {
        &self.draft
    }
    pub fn into_draft(self) -> ProjectCandidateDraft {
        self.draft
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

impl ProjectCandidateDraft {
    /// Replay all previously checked intentions on an admitted new base, reject
    /// conflicts in pending regions, then rejoin and readmit every pending hole.
    /// This does not authenticate current filesystem paths or publish source.
    pub fn rebase(
        &self,
        expected_draft: &str,
        new_base: Arc<ProjectRevision>,
        expected_new_base: &str,
    ) -> Result<ProjectCandidateDraftRebase> {
        self.require_digest(expected_draft)?;
        let replay = self.last_valid.rebase(
            self.last_valid.candidate_digest(),
            new_base,
            expected_new_base,
        )?;
        let ancestry: Value = serde_json::from_str(replay.to_json())
            .map_err(|_| invalid("checked candidate rebase report is invalid"))?;
        let mut draft = Self::open(Arc::new(replay.into_candidate()))?;
        let before = self.last_valid.revision();
        let after = draft.last_valid.revision();
        let body_targets = self
            .holes
            .values()
            .map(String::as_str)
            .chain(
                self.expression_holes
                    .values()
                    .map(|(target, _)| target.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let contract_targets = self
            .contract_expression_holes
            .values()
            .map(|(target, _)| target.as_str())
            .collect::<BTreeSet<_>>();
        let mut callees = BTreeSet::new();
        for (target, id) in self.contract_expression_holes.values() {
            callees.extend(expression::contract_call_targets(before, target, id)?);
        }
        let changes = if body_targets.is_empty() && contract_targets.is_empty() {
            BTreeMap::new()
        } else {
            rebase::pending_draft_conflicts(
                before,
                after,
                &body_targets,
                &contract_targets,
                &callees,
            )?
        };
        // Bind actual nominal owners throughout each protected region, including
        // inferred intermediates with scalar results. Names alone are not joins.
        let mut visits = 0;
        for target in body_targets.union(&contract_targets) {
            let (_, old) = self.resolved_function(target)?;
            let (_, new) = draft.resolved_function(target)?;
            let body = body_targets.contains(target);
            let contracts = contract_targets.contains(target);
            let old_types = nominal_owners(old, body, contracts, &mut visits)?;
            let new_types = nominal_owners(new, body, contracts, &mut visits)?;
            if old_types != new_types {
                return Err(conflict(
                    "pending draft nominal dependencies changed identity",
                ));
            }
            for id in old_types {
                let left = intent::nominal_type_dependency_fingerprint(before, &id)?;
                let right = intent::nominal_type_dependency_fingerprint(after, &id)?;
                if left.is_none() || left != right {
                    return Err(conflict(
                        "pending draft nominal owner shape changed or is unavailable",
                    ));
                }
            }
        }
        let old_programs =
            if self.expression_holes.is_empty() && self.contract_expression_holes.is_empty() {
                Vec::new()
            } else {
                parse_revision(before)?
            };
        let new_programs = if old_programs.is_empty() {
            Vec::new()
        } else {
            parse_revision(after)?
        };
        // Compute mappings before rebuilding the immutable draft. No old ID is
        // assumed to survive, and lexical scope facts are independently joined.
        let mut mapped = BTreeMap::new();
        for (contract, holes) in [
            (false, &self.expression_holes),
            (true, &self.contract_expression_holes),
        ] {
            for (hole, (target, id)) in holes {
                let next = if contract {
                    expression::remap_contract_selection(before, after, target, id)?
                } else {
                    expression::remap_selection(before, after, target, id)?
                };
                let left = if contract {
                    expression::authored_contract_selection(before, &old_programs, target, id)?
                } else {
                    expression::authored_selection(before, &old_programs, target, id)?
                };
                let right = if contract {
                    expression::authored_contract_selection(after, &new_programs, target, &next)?
                } else {
                    expression::authored_selection(after, &new_programs, target, &next)?
                };
                if left.scope.len() != right.scope.len()
                    || left.scope.iter().zip(&right.scope).any(|(a, b)| {
                        a.name != b.name
                            || a.ty != b.ty
                            || a.ownership != b.ownership
                            || a.mutable != b.mutable
                    })
                {
                    return Err(conflict(
                        "pending expression lexical scope type or ownership changed",
                    ));
                }
                mapped.insert(hole.clone(), next);
            }
        }
        let mut rows = BTreeMap::new();
        for (hole, target) in &self.holes {
            draft = draft.with_body_hole(draft.draft_digest(), target, hole)?;
            rows.insert(
                hole.clone(),
                row(hole, "function_body", target, None, None, &changes),
            );
        }
        for (contract, holes) in [
            (false, &self.expression_holes),
            (true, &self.contract_expression_holes),
        ] {
            for (hole, (target, old)) in holes {
                let new = &mapped[hole];
                draft = if contract {
                    draft.with_contract_expression_hole(draft.draft_digest(), target, new, hole)?
                } else {
                    draft.with_expression_hole(draft.draft_digest(), target, new, hole)?
                };
                rows.insert(
                    hole.clone(),
                    row(
                        hole,
                        if contract {
                            "contract_expression"
                        } else {
                            "expression"
                        },
                        target,
                        Some(old),
                        Some(new),
                        &changes,
                    ),
                );
            }
        }
        // Reconstruct every full context from the final retained checked state,
        // including calls, contracts and prior cleanup/loan plans. Each query
        // keeps its existing bounds; discard bytes rather than retain a cache.
        for hole in rows.keys() {
            draft.hole_context(draft.draft_digest(), hole)?;
        }
        let json = wire::render(json!({
            "schema":PROJECT_CANDIDATE_DRAFT_REBASE_SCHEMA,
            "parent_draft_digest":self.draft_digest(),
            "original_base_revision":self.last_valid.base_revision().project_revision(),
            "onto_revision":expected_new_base,
            "result_base_revision":draft.last_valid.base_revision().project_revision(),
            "result_draft_digest":draft.draft_digest(),
            "last_valid_rebase":ancestry,"holes":rows.into_values().collect::<Vec<_>>(),
            "materializable":false,"source_authority":false,
            "validation":"checked_history_replay_and_pending_selector_readmission",
            "nonclaims":["no_unresolved_source_or_candidate_release","not_behavioral_equivalence","not_contract_implication","no_runtime_or_project_test_execution","no_source_commit_authority","conservative_region_conflicts_not_arbitrary_subtree_merge"]
        }), MAX_PROJECT_CANDIDATE_DRAFT_REBASE_BYTES)
            .map_err(|_| capacity("draft rebase report exceeds its byte bound"))?;
        Ok(ProjectCandidateDraftRebase { draft, json })
    }
}

fn row(
    hole: &str,
    kind: &str,
    target: &str,
    old: Option<&String>,
    new: Option<&String>,
    changes: &BTreeMap<String, (bool, bool)>,
) -> Value {
    let (body, contracts) = changes[target];
    json!({"hole_id":hole,"kind":kind,"target":target,"old_expression_id":old,"new_expression_id":new,
        "concurrent_body_change":body,"concurrent_contract_change":contracts,"context_refreshed":true})
}

fn nominal_owners(
    function: &ResolvedFunction,
    body: bool,
    contracts: bool,
    visits: &mut usize,
) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for ty in function
        .params
        .iter()
        .map(|p| &p.ty)
        .chain(std::iter::once(&function.return_type))
    {
        collect_type(ty, &mut ids, visits)?;
    }
    let mut stack = Vec::new();
    if body {
        stack.push((&function.body, 0usize));
    }
    if contracts {
        stack.extend(
            function
                .requires
                .iter()
                .chain(&function.ensures)
                .map(|e| (e, 0)),
        );
    }
    while let Some((expression, depth)) = stack.pop() {
        charge(visits, 1)?;
        if depth > MAX_DEPTH {
            return Err(capacity(
                "draft rebase checked expression depth exceeds its bound",
            ));
        }
        collect_type(&expression.ty, &mut ids, visits)?;
        let mut children = Vec::new();
        hir::push_resolved_expression_children_in_authored_order(expression, &mut children);
        if children.len().saturating_add(stack.len()) > MAX_VISITS.saturating_sub(*visits) {
            return Err(capacity(
                "draft rebase checked expression inventory exceeds its bound",
            ));
        }
        stack.extend(children.into_iter().map(|e| (e, depth + 1)));
    }
    Ok(ids)
}
fn collect_type(ty: &ResolvedType, ids: &mut BTreeSet<String>, visits: &mut usize) -> Result<()> {
    let mut stack = vec![(ty, 0usize)];
    while let Some((ty, depth)) = stack.pop() {
        charge(visits, 1)?;
        if depth > MAX_DEPTH {
            return Err(capacity(
                "draft rebase checked type depth exceeds its bound",
            ));
        }
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        {
            ids.insert(declaration.as_str().to_owned());
            if arguments.len().saturating_add(stack.len()) > MAX_VISITS.saturating_sub(*visits) {
                return Err(capacity(
                    "draft rebase checked type inventory exceeds its bound",
                ));
            }
            stack.extend(arguments.iter().map(|t| (t, depth + 1)));
        }
    }
    Ok(())
}
fn charge(visits: &mut usize, count: usize) -> Result<()> {
    *visits = visits
        .checked_add(count)
        .ok_or_else(|| capacity("draft rebase traversal accounting overflow"))?;
    if *visits > MAX_VISITS {
        return Err(capacity("draft rebase traversal exceeds its bound"));
    }
    Ok(())
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G343", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G344", message)]
}
fn conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G345", message)]
}
