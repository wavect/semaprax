//! Merge checked sibling histories once, then union authenticated pending holes.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{ProjectCandidateDraft, MAX_PROJECT_CANDIDATE_HOLES};
use crate::diagnostic::Diagnostic;
use crate::project::candidate::wire;
use crate::project::SemanticChange;

pub const PROJECT_CANDIDATE_DRAFT_MERGE_SCHEMA: &str = "semaprax.project-candidate-draft-merge.v1";
pub const MAX_PROJECT_CANDIDATE_DRAFT_MERGE_BYTES: usize = 1024 * 1024;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// An unresolved draft and descriptive parent mappings, never a release of its
/// private last-valid candidate or authority to publish source.
pub struct ProjectCandidateDraftMerge {
    draft: ProjectCandidateDraft,
    json: String,
}
impl ProjectCandidateDraftMerge {
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
    /// Merge histories sharing the exact original base, then independently
    /// authenticate both parents' pending regions against that one result.
    /// Exact duplicate selectors coalesce; conflicts and overlaps fail closed.
    pub fn merge(
        &self,
        expected_draft: &str,
        other: &Self,
        expected_other: &str,
    ) -> Result<ProjectCandidateDraftMerge> {
        self.require_digest(expected_draft)?;
        other.require_digest(expected_other)?;
        // The existing owner preserves the common original base, exact common
        // prefix and right-suffix/left-suffix order. Never reapply either full
        // history on top of the resulting candidate's source revision.
        let replay = self.last_valid.merge(
            self.last_valid.candidate_digest(),
            &other.last_valid,
            other.last_valid.candidate_digest(),
        )?;
        let ancestry: Value = serde_json::from_str(replay.to_json())
            .map_err(|_| invalid("checked candidate merge report is invalid"))?;
        let prefix = self
            .last_valid
            .changes
            .iter()
            .zip(&other.last_valid.changes)
            .take_while(|(left, right)| left.to_json() == right.to_json())
            .count();
        // A no-op fill is still an actual source intention. Compare opposing
        // protected writes as well as final fingerprints; infer no tombstones
        // or unrecorded hole lineage from a merely absent pending selector.
        opposing_writes(self, &other.last_valid.changes[prefix..])?;
        opposing_writes(other, &self.last_valid.changes[prefix..])?;
        let candidate = Arc::new(replay.into_candidate());
        let (left, left_rows) = self.rebind_pending(Arc::clone(&candidate))?;
        let (right, right_rows) = other.rebind_pending(Arc::clone(&candidate))?;
        let mut union = BTreeMap::new();
        append_pending(&left, "left", &mut union)?;
        append_pending(&right, "right", &mut union)?;
        let mut draft = Self::open(candidate)?;
        for (hole, item) in &union {
            let selected = &item.selector;
            draft = match selected.kind {
                Kind::Body => draft.with_body_hole(draft.draft_digest(), &selected.target, hole)?,
                Kind::Expression => draft.with_expression_hole(
                    draft.draft_digest(),
                    &selected.target,
                    selected
                        .expression
                        .as_deref()
                        .expect("typed expression selection"),
                    hole,
                )?,
                Kind::Contract => draft.with_contract_expression_hole(
                    draft.draft_digest(),
                    &selected.target,
                    selected
                        .expression
                        .as_deref()
                        .expect("typed contract selection"),
                    hole,
                )?,
            };
        }
        draft.validate_pending_contexts()?;
        let rows = union.into_iter().map(|(hole,item)| json!({
            "hole_id":hole,"kind":item.selector.kind.name(),"target":item.selector.target,
            "expression_id":item.selector.expression,"parents":item.parents,"context_refreshed":true
        })).collect::<Vec<_>>();
        let json = wire::render(json!({
            "schema":PROJECT_CANDIDATE_DRAFT_MERGE_SCHEMA,
            "left_parent_draft_digest":self.draft_digest(),
            "right_parent_draft_digest":other.draft_digest(),
            "original_base_revision":self.last_valid.base_revision().project_revision(),
            "result_base_revision":draft.last_valid.base_revision().project_revision(),
            "result_draft_digest":draft.draft_digest(),
            "last_valid_merge":ancestry,"left_holes":left_rows,"right_holes":right_rows,"holes":rows,
            "materializable":false,"source_authority":false,
            "validation":"checked_history_merge_and_pending_selector_readmission",
            "nonclaims":["no_unresolved_source_or_candidate_release","no_inferred_hole_tombstones_or_lineage","not_behavioral_equivalence","not_contract_implication","no_runtime_or_project_test_execution","no_source_commit_authority","conservative_opposing_region_writes_not_arbitrary_subtree_merge"]
        }), MAX_PROJECT_CANDIDATE_DRAFT_MERGE_BYTES)
            .map_err(|_| capacity("draft merge report exceeds its byte bound"))?;
        Ok(ProjectCandidateDraftMerge { draft, json })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind {
    Body,
    Expression,
    Contract,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Body => "function_body",
            Self::Expression => "expression",
            Self::Contract => "contract_expression",
        }
    }
}
#[derive(Eq, PartialEq)]
struct Selector {
    kind: Kind,
    target: String,
    expression: Option<String>,
}
struct UnionHole {
    selector: Selector,
    parents: Vec<&'static str>,
}

fn append_pending(
    draft: &ProjectCandidateDraft,
    parent: &'static str,
    union: &mut BTreeMap<String, UnionHole>,
) -> Result<()> {
    for (hole, target) in &draft.holes {
        append(union, hole, Kind::Body, target, None, parent)?;
    }
    for (kind, holes) in [
        (Kind::Expression, &draft.expression_holes),
        (Kind::Contract, &draft.contract_expression_holes),
    ] {
        for (hole, (target, expression)) in holes {
            append(union, hole, kind, target, Some(expression), parent)?;
        }
    }
    Ok(())
}
fn append(
    union: &mut BTreeMap<String, UnionHole>,
    hole: &str,
    kind: Kind,
    target: &str,
    expression: Option<&String>,
    parent: &'static str,
) -> Result<()> {
    let selector = Selector {
        kind,
        target: target.to_owned(),
        expression: expression.cloned(),
    };
    if let Some(existing) = union.get_mut(hole) {
        if existing.selector != selector {
            return Err(conflict(
                "draft merge hole ID selects different kinds, owners or expressions",
            ));
        }
        existing.parents.push(parent);
    } else {
        if union.len() >= MAX_PROJECT_CANDIDATE_HOLES {
            return Err(capacity(
                "draft merge pending union exceeds the shared hole bound",
            ));
        }
        union.insert(
            hole.to_owned(),
            UnionHole {
                selector,
                parents: vec![parent],
            },
        );
    }
    Ok(())
}

fn opposing_writes(draft: &ProjectCandidateDraft, changes: &[SemanticChange]) -> Result<()> {
    let bodies = draft
        .holes
        .values()
        .map(String::as_str)
        .chain(
            draft
                .expression_holes
                .values()
                .map(|(target, _)| target.as_str()),
        )
        .collect::<BTreeSet<_>>();
    let contracts = draft
        .contract_expression_holes
        .values()
        .map(|(target, _)| target.as_str())
        .collect::<BTreeSet<_>>();
    for change in changes {
        let target = change.intent["target"]
            .as_str()
            .ok_or_else(|| invalid("checked opposing intention lacks its target"))?;
        let kind = change.intent["kind"]
            .as_str()
            .ok_or_else(|| invalid("checked opposing intention lacks its kind"))?;
        let writes_body = matches!(
            kind,
            "replace_function_body"
                | "replace_expression"
                | "extract_function"
                | "change_function_signature"
        );
        let writes_contract = matches!(
            kind,
            "replace_contract_expression" | "add_contract" | "change_function_signature"
        );
        if (writes_body && bodies.contains(target))
            || (writes_contract && contracts.contains(target))
        {
            return Err(conflict("opposing checked history writes a pending protected region, including no-op or net-zero writes"));
        }
    }
    Ok(())
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G346", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G347", message)]
}
fn conflict(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G348", message)]
}
