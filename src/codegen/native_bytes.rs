//! Canonical CleanupPlan bridge for compiler-owned `Bytes` in ordinary C11.
//!
//! This module never discovers ownership from lexical HIR. It indexes the
//! replay-validated plan's exact storage, transitions, flags, and finalizer
//! order. The value emitter may only perform an owned move after this bridge
//! authenticates the corresponding transition.

use std::collections::{BTreeMap, BTreeSet};

use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{CleanupTransition, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedFunction, ResolvedType};

#[derive(Clone, Debug)]
pub(super) struct NativeBytesPlan {
    slots: BTreeMap<StorageId, ByteSlot>,
    transitions: BTreeMap<ExpressionId, Vec<CleanupTransition>>,
    finalizers: Vec<ByteSlot>,
    scope_exits: Vec<(BTreeSet<StorageId>, Vec<ByteSlot>)>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ByteSlot {
    storage: StorageId,
    value: String,
    flag: String,
}

impl NativeBytesPlan {
    pub(super) fn build(function: &ResolvedFunction) -> Result<Option<Self>, Diagnostic> {
        let mut slots = BTreeMap::new();
        let mut by_flag = BTreeMap::new();
        for slot in &function.cleanup_plan.slots {
            if !matches!(slot.ty, ResolvedType::Bytes) {
                continue;
            }
            let FieldLivenessShape::Leaf { flag, lifecycle } = &slot.field_liveness_shape else {
                return Err(error("Bytes cleanup slot is not one direct leaf"));
            };
            if lifecycle.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID {
                return Err(error("Bytes cleanup slot has a noncanonical lifecycle"));
            }
            let byte_slot = ByteSlot {
                storage: slot.storage.clone(),
                value: format!("spx_bytes_slot_{}", slot.id.0),
                flag: format!("spx_bytes_live_{}", flag.0),
            };
            if slots
                .insert(slot.storage.clone(), byte_slot.clone())
                .is_some()
                || by_flag.insert(*flag, byte_slot).is_some()
            {
                return Err(error("Bytes cleanup storage or flag is duplicated"));
            }
        }
        if slots.is_empty() {
            return Ok(None);
        }

        let mut transitions = BTreeMap::<ExpressionId, Vec<CleanupTransition>>::new();
        for block in &function.cleanup_plan.blocks {
            for transition in &block.transitions {
                let at = match transition {
                    CleanupTransition::Initialize { at, .. }
                    | CleanupTransition::Transfer { at, .. } => Some(at),
                    CleanupTransition::CallCommit { call, .. } => Some(call),
                    CleanupTransition::SelectFailure { .. }
                    | CleanupTransition::StageCopyResult { .. } => None,
                };
                if let Some(at) = at {
                    transitions
                        .entry(at.clone())
                        .or_default()
                        .push(transition.clone());
                }
            }
        }

        // Construct the union of every exit's exact pairwise precedence and
        // choose one deterministic topological order. Dead guards make absent
        // actions inert, while every actual exit retains its complete
        // canonical relative order. Reject only contradictory exit orders.
        let terminal_sequences = function
            .cleanup_plan
            .exits
            .iter()
            .filter(|exit| {
                !matches!(
                    exit.continuation,
                    crate::cleanup_plan::ExitContinuation::Continue(_)
                )
            })
            .map(|exit| {
                exit.finalize_in_order
                    .iter()
                    .filter_map(|action| by_flag.get(&action.guard_flag).cloned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let finalizers = canonical_finalizer_order(&terminal_sequences)?;
        let mut scope_exits = Vec::new();
        for region in &function.cleanup_plan.regions {
            let storage = region
                .slots
                .iter()
                .filter(|storage| slots.contains_key(*storage))
                .cloned()
                .collect::<BTreeSet<_>>();
            if storage.is_empty() {
                continue;
            }
            if region.parent.is_none() {
                scope_exits.push((storage, Vec::new()));
                continue;
            }
            let exit = function
                .cleanup_plan
                .exits
                .get(region.normal_scope_end.0 as usize)
                .filter(|exit| exit.id == region.normal_scope_end)
                .ok_or_else(|| error("Bytes region has no canonical normal-scope exit"))?;
            if !matches!(
                exit.continuation,
                crate::cleanup_plan::ExitContinuation::Continue(_)
            ) || exit.leaves_regions.as_slice() != [region.id]
            {
                return Err(error("Bytes region normal-scope exit is not canonical"));
            }
            let actions = exit
                .finalize_in_order
                .iter()
                .filter_map(|action| by_flag.get(&action.guard_flag).cloned())
                .collect::<Vec<_>>();
            scope_exits.push((storage, actions));
        }
        Ok(Some(Self {
            slots,
            transitions,
            finalizers,
            scope_exits,
        }))
    }

    pub(super) fn declarations(&self, function: &ResolvedFunction) -> String {
        let live_parameters = function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .iter()
            .map(|place| &place.storage)
            .collect::<BTreeSet<_>>();
        let mut output = String::new();
        for slot in self.slots.values() {
            output.push_str(&format!("    spx_bytes_v1 {} = {{0}};\n", slot.value));
            output.push_str(&format!(
                "    bool {} = {};\n",
                slot.flag,
                live_parameters.contains(&slot.storage)
            ));
        }
        output
    }

    pub(super) fn value(&self, storage: &StorageId) -> Result<&str, Diagnostic> {
        self.slots
            .get(storage)
            .map(|slot| slot.value.as_str())
            .ok_or_else(|| error(format!("Bytes storage `{storage:?}` is not indexed")))
    }

    pub(super) fn initialize_parameter(
        &self,
        storage: &StorageId,
        parameter: &str,
    ) -> Result<String, Diagnostic> {
        let slot = self
            .slots
            .get(storage)
            .ok_or_else(|| error("owned Bytes parameter has no cleanup slot"))?;
        Ok(format!("    {} = {parameter};\n", slot.value))
    }

    pub(super) fn apply_at(&self, at: &ExpressionId) -> Result<String, Diagnostic> {
        let mut output = String::new();
        for transition in self.transitions.get(at).into_iter().flatten() {
            match transition {
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } if self.slots.contains_key(&destination.storage) => {
                    let source = self
                        .slots
                        .get(&source.storage)
                        .ok_or_else(|| error("Bytes transfer source is not indexed"))?;
                    let destination = self
                        .slots
                        .get(&destination.storage)
                        .ok_or_else(|| error("Bytes transfer destination is not indexed"))?;
                    output.push_str(&format!(
                        "if (!{} || {}) spx_runtime_invariant_failure(\"Bytes plan transfer liveness {} to {}\");\n{} = spx_bytes_move(&{});\n{} = false;\n{} = true;\n",
                        source.flag,
                        destination.flag,
                        source.value,
                        destination.value,
                        destination.value,
                        source.value,
                        source.flag,
                        destination.flag
                    ));
                }
                CleanupTransition::Initialize { destination, .. }
                    if self.slots.contains_key(&destination.storage) =>
                {
                    let destination = &self.slots[&destination.storage];
                    output.push_str(&format!(
                        "if ({}) spx_runtime_invariant_failure(\"Bytes plan initialize liveness\");\n{} = true;\n",
                        destination.flag, destination.flag
                    ));
                }
                CleanupTransition::CallCommit { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. }
                | CleanupTransition::Initialize { .. }
                | CleanupTransition::Transfer { .. } => {}
            }
        }
        Ok(output)
    }

    pub(super) fn result_at(&self, at: &ExpressionId) -> Option<&str> {
        self.transitions.get(at).and_then(|transitions| {
            transitions
                .iter()
                .rev()
                .find_map(|transition| match transition {
                    CleanupTransition::Initialize { destination, .. }
                    | CleanupTransition::Transfer { destination, .. } => self
                        .slots
                        .get(&destination.storage)
                        .map(|slot| slot.value.as_str()),
                    CleanupTransition::CallCommit { .. }
                    | CleanupTransition::SelectFailure { .. }
                    | CleanupTransition::StageCopyResult { .. } => None,
                })
        })
    }

    pub(super) fn transfer_to(&self, storage: &StorageId) -> Result<String, Diagnostic> {
        let mut matches = self
            .transitions
            .values()
            .flatten()
            .filter_map(|transition| {
                let CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } = transition
                else {
                    return None;
                };
                (destination.storage == *storage).then_some((source, destination))
            });
        let Some((source, destination)) = matches.next() else {
            return Err(error(format!(
                "Bytes destination `{storage:?}` has no canonical transfer"
            )));
        };
        if matches.next().is_some() {
            return Err(error(format!(
                "Bytes destination `{storage:?}` has ambiguous transfers"
            )));
        }
        let source = self
            .slots
            .get(&source.storage)
            .ok_or_else(|| error("Bytes transfer source is not indexed"))?;
        let destination = self
            .slots
            .get(&destination.storage)
            .ok_or_else(|| error("Bytes transfer destination is not indexed"))?;
        Ok(format!(
            "if (!{} || {}) spx_runtime_invariant_failure(\"Bytes plan transfer liveness {} to {}\");\n{} = spx_bytes_move(&{});\n{} = false;\n{} = true;\n",
            source.flag,
            destination.flag,
            source.value,
            destination.value,
            destination.value,
            source.value,
            source.flag,
            destination.flag
        ))
    }

    pub(super) fn transfer_from_to(
        &self,
        source_value: &str,
        storage: &StorageId,
    ) -> Result<String, Diagnostic> {
        let mut matches = self
            .transitions
            .values()
            .flatten()
            .filter_map(|transition| {
                let CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } = transition
                else {
                    return None;
                };
                let source_slot = self.slots.get(&source.storage)?;
                (source_slot.value == source_value && destination.storage == *storage)
                    .then_some((source, destination))
            });
        let Some((source, destination)) = matches.next() else {
            return Err(error(format!(
                "Bytes branch source `{source_value}` has no transfer to `{storage:?}`"
            )));
        };
        if matches.next().is_some() {
            return Err(error("Bytes branch transfer is ambiguous"));
        }
        let source = &self.slots[&source.storage];
        let destination = &self.slots[&destination.storage];
        Ok(format!(
            "if (!{} || {}) spx_runtime_invariant_failure(\"Bytes plan branch transfer liveness\");\n{} = spx_bytes_move(&{});\n{} = false;\n{} = true;\n",
            source.flag,
            destination.flag,
            destination.value,
            source.value,
            source.flag,
            destination.flag
        ))
    }

    pub(super) fn call_argument(
        &self,
        call: &ExpressionId,
        parameter_index: u32,
    ) -> Result<(&str, &str), Diagnostic> {
        self.slots
            .iter()
            .find_map(|(storage, slot)| match storage {
                StorageId::CallArgument {
                    call: owner,
                    parameter_index: candidate,
                    ..
                } if owner == call && *candidate == parameter_index => {
                    Some((slot.value.as_str(), slot.flag.as_str()))
                }
                _ => None,
            })
            .ok_or_else(|| error("owned Bytes call argument has no canonical epoch"))
    }

    pub(super) fn provisional(&self) -> Result<(&str, &str), Diagnostic> {
        let slot = self
            .slots
            .get(&StorageId::ProvisionalResult)
            .ok_or_else(|| error("owned Bytes result has no provisional slot"))?;
        Ok((&slot.value, &slot.flag))
    }

    pub(super) fn epilogue(&self) -> String {
        self.emit_finalizers(&self.finalizers, true)
    }

    pub(super) fn scope_exit(&self, anchors: &BTreeSet<StorageId>) -> Result<String, Diagnostic> {
        let mut matches = self
            .scope_exits
            .iter()
            .filter(|(storage, _)| storage.iter().any(|slot| anchors.contains(slot)));
        let Some((_, actions)) = matches.next() else {
            return if anchors.is_empty() {
                Ok(String::new())
            } else {
                Err(error("Bytes block has no authenticated CleanupPlan region"))
            };
        };
        if matches.next().is_some() {
            return Err(error("Bytes block maps to multiple CleanupPlan regions"));
        }
        Ok(self.emit_finalizers(actions, false))
    }

    fn emit_finalizers(&self, finalizers: &[ByteSlot], terminal: bool) -> String {
        let mut output = String::new();
        for slot in finalizers {
            let guard = if terminal && slot.storage == StorageId::ProvisionalResult {
                format!("{} && spx_status != SPX_STATUS_SUCCESS", slot.flag)
            } else {
                slot.flag.clone()
            };
            output.push_str(&format!(
                "    if ({guard}) {{ {} = false; spx_bytes_drop(&{}); }}\n",
                slot.flag, slot.value
            ));
        }
        output
    }
}

fn canonical_finalizer_order(sequences: &[Vec<ByteSlot>]) -> Result<Vec<ByteSlot>, Diagnostic> {
    let mut nodes = BTreeSet::<ByteSlot>::new();
    let mut successors = BTreeMap::<ByteSlot, BTreeSet<ByteSlot>>::new();
    let mut indegree = BTreeMap::<ByteSlot, usize>::new();
    for sequence in sequences {
        for slot in sequence {
            nodes.insert(slot.clone());
            indegree.entry(slot.clone()).or_insert(0);
        }
        for pair in sequence.windows(2) {
            if successors
                .entry(pair[0].clone())
                .or_default()
                .insert(pair[1].clone())
            {
                *indegree.entry(pair[1].clone()).or_insert(0) += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(slot, degree)| (*degree == 0).then_some(slot.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(slot) = ready.pop_first() {
        order.push(slot.clone());
        for successor in successors.get(&slot).into_iter().flatten() {
            let degree = indegree
                .get_mut(successor)
                .ok_or_else(|| error("Bytes finalizer precedence node is missing"))?;
            *degree = degree
                .checked_sub(1)
                .ok_or_else(|| error("Bytes finalizer precedence underflow"))?;
            if *degree == 0 {
                ready.insert(successor.clone());
            }
        }
    }
    if order.len() != nodes.len() {
        return Err(error(
            "Bytes cleanup exits contain contradictory finalizer precedence",
        ));
    }
    Ok(order)
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B104", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(name: &str) -> ByteSlot {
        ByteSlot {
            storage: StorageId::ProvisionalResult,
            value: format!("value_{name}"),
            flag: format!("flag_{name}"),
        }
    }

    #[test]
    fn finalizer_order_accepts_reordered_subset_before_superset() {
        let a = slot("a");
        let b = slot("b");
        assert_eq!(
            canonical_finalizer_order(&[vec![a.clone()], vec![b.clone(), a.clone()]]).unwrap(),
            vec![b, a]
        );
    }

    #[test]
    fn finalizer_order_rejects_contradictory_exit_precedence() {
        let a = slot("a");
        let b = slot("b");
        assert!(canonical_finalizer_order(&[vec![a.clone(), b.clone()], vec![b, a],]).is_err());
    }
}
