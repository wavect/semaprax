//! Exact canonical scope-exit selection for Copy-scalar match children.
//!
//! A guard or selected arm value can contain nested lexical expressions with
//! their own owned String slots. Those nested regions settle independently;
//! the native emitter must select the one structural child exit described by
//! the scalar-match control flow, rather than treating every descendant slot
//! as an interchangeable scope anchor.

use std::collections::{BTreeMap, BTreeSet};

use crate::cleanup_plan::{
    CleanupPlace, CleanupRegionId, CleanupTerminator, EdgeCondition, ExitContinuation, StorageId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedFunction};

use super::{error, ByteSlot, NativeBytesPlan};

#[derive(Clone, Debug)]
pub(super) struct ScopeExit {
    region: CleanupRegionId,
    parent: Option<CleanupRegionId>,
    storage: BTreeSet<StorageId>,
    actions: Vec<ByteSlot>,
    guard_branch: Option<ExpressionId>,
    parent_arm_selected_scrutinees: BTreeSet<ExpressionId>,
}

impl ScopeExit {
    pub(super) fn actions(&self) -> &[ByteSlot] {
        &self.actions
    }
}

pub(super) fn build_scope_exits(
    function: &ResolvedFunction,
    storage_leaves: &BTreeMap<StorageId, Vec<CleanupPlace>>,
    by_flag: &BTreeMap<crate::cleanup::LivenessFlagId, ByteSlot>,
) -> Result<Vec<ScopeExit>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut arm_selected_by_region = BTreeMap::<CleanupRegionId, BTreeSet<ExpressionId>>::new();
    for edge in &plan.edges {
        let Some(block) = plan
            .blocks
            .get(edge.from.0 as usize)
            .filter(|block| block.id == edge.from)
        else {
            return Err(error("Bytes cleanup edge has no canonical source block"));
        };
        if let EdgeCondition::ArmSelected { scrutinee, .. } = &edge.condition {
            arm_selected_by_region
                .entry(block.region)
                .or_default()
                .insert(scrutinee.clone());
        }
    }
    let mut exits = Vec::with_capacity(plan.regions.len());
    for region in &plan.regions {
        let storage = region
            .slots
            .iter()
            .filter(|storage| storage_leaves.contains_key(*storage))
            .cloned()
            .collect::<BTreeSet<_>>();
        if region.parent.is_none() {
            exits.push(ScopeExit {
                region: region.id,
                parent: None,
                storage,
                actions: Vec::new(),
                guard_branch: None,
                parent_arm_selected_scrutinees: BTreeSet::new(),
            });
            continue;
        }
        let exit = plan
            .exits
            .get(region.normal_scope_end.0 as usize)
            .filter(|exit| exit.id == region.normal_scope_end)
            .ok_or_else(|| error("Bytes region has no canonical normal-scope exit"))?;
        let ExitContinuation::Continue(continue_edge) = &exit.continuation else {
            return Err(error("Bytes region normal-scope exit is not canonical"));
        };
        if exit.leaves_regions.as_slice() != [region.id] {
            return Err(error("Bytes region normal-scope exit is not canonical"));
        }
        let actions = exit
            .finalize_in_order
            .iter()
            .filter_map(|action| by_flag.get(&action.guard_flag).cloned())
            .collect::<Vec<_>>();
        exits.push(ScopeExit {
            region: region.id,
            parent: region.parent,
            storage,
            actions,
            guard_branch: boolean_branch_expression(plan, *continue_edge)?,
            parent_arm_selected_scrutinees: region
                .parent
                .and_then(|parent| arm_selected_by_region.get(&parent).cloned())
                .unwrap_or_default(),
        });
    }
    Ok(exits)
}

fn boolean_branch_expression(
    plan: &crate::cleanup_plan::CleanupPlan,
    continue_edge: crate::cleanup_plan::EdgeId,
) -> Result<Option<ExpressionId>, Diagnostic> {
    let edge = plan
        .edges
        .get(continue_edge.0 as usize)
        .filter(|edge| edge.id == continue_edge)
        .ok_or_else(|| error("Bytes normal-scope continuation has no canonical edge"))?;
    let block = plan
        .blocks
        .get(edge.to.0 as usize)
        .filter(|block| block.id == edge.to)
        .ok_or_else(|| error("Bytes normal-scope continuation has no canonical block"))?;
    let CleanupTerminator::Branch(branches) = &block.terminator else {
        return Ok(None);
    };
    let mut guard = None;
    let mut outcomes = BTreeSet::new();
    for branch in branches {
        let edge = plan
            .edges
            .get(branch.0 as usize)
            .filter(|edge| edge.id == *branch)
            .ok_or_else(|| error("Bytes Boolean branch has no canonical edge"))?;
        let EdgeCondition::BooleanResult(expression, outcome) = &edge.condition else {
            return Ok(None);
        };
        if guard.as_ref().is_some_and(|current| current != expression) {
            return Ok(None);
        }
        guard = Some(expression.clone());
        outcomes.insert(*outcome);
    }
    Ok((outcomes == BTreeSet::from([false, true]))
        .then_some(guard)
        .flatten())
}

impl NativeBytesPlan {
    /// Whether `storage` is a finalizable slot of some lexical region.
    pub(in crate::codegen) fn is_region_slot(&self, storage: &StorageId) -> bool {
        self.scope_exits
            .iter()
            .any(|scope| scope.storage.contains(storage))
    }

    pub(in crate::codegen) fn scope_exit(
        &self,
        anchors: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        let mut matches = self
            .scope_exits
            .iter()
            .filter(|scope| scope.storage.iter().any(|slot| anchors.contains(slot)));
        let Some(scope) = matches.next() else {
            return if anchors.is_empty() {
                Ok(String::new())
            } else {
                Err(error("Bytes block has no authenticated CleanupPlan region"))
            };
        };
        if matches.next().is_some() {
            return Err(error("Bytes block maps to multiple CleanupPlan regions"));
        }
        Ok(self.emit_finalizers(&scope.actions, false))
    }

    pub(in crate::codegen) fn scalar_match_guard_scope_exit(
        &self,
        guard: &ExpressionId,
        anchors: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        self.scalar_match_scope_exit(anchors, |scope| scope.guard_branch.as_ref() == Some(guard))
    }

    pub(in crate::codegen) fn scalar_match_value_scope_exit(
        &self,
        scrutinee: &ExpressionId,
        anchors: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        self.scalar_match_scope_exit(anchors, |scope| {
            scope.parent_arm_selected_scrutinees.contains(scrutinee)
        })
    }

    fn scalar_match_scope_exit(
        &self,
        anchors: &BTreeSet<StorageId>,
        matches: impl Fn(&ScopeExit) -> bool,
    ) -> Result<String, Diagnostic> {
        if anchors.is_empty() {
            return Ok(String::new());
        }
        let by_region = self
            .scope_exits
            .iter()
            .map(|scope| (scope.region, scope))
            .collect::<BTreeMap<_, _>>();
        let mut selected = None;
        for anchor in anchors {
            let Some(start) = self
                .scope_exits
                .iter()
                .find(|scope| scope.storage.contains(anchor))
            else {
                return Err(error("String scalar-match anchor has no cleanup region"));
            };
            let mut region = Some(start.region);
            let candidate = loop {
                let current = region
                    .and_then(|region| by_region.get(&region).copied())
                    .ok_or_else(|| error("String scalar-match region parent is not canonical"))?;
                if matches(current) {
                    break current;
                }
                region = current.parent;
            };
            if selected.is_some_and(|prior: &ScopeExit| prior.region != candidate.region) {
                return Err(error(
                    "String scalar-match anchors select different canonical regions",
                ));
            }
            selected = Some(candidate);
        }
        let selected = selected.ok_or_else(|| error("String scalar-match exit has no anchors"))?;
        Ok(self.emit_finalizers(&selected.actions, false))
    }
}
