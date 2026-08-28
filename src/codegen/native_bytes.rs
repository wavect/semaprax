//! Canonical CleanupPlan bridge for compiler-owned `Bytes` in ordinary C11.
//!
//! This module never discovers ownership from lexical HIR. It indexes the
//! replay-validated plan's exact storage, transitions, flags, and finalizer
//! order. The value emitter may only perform an owned move after this bridge
//! authenticates the corresponding transition.

use std::collections::{BTreeMap, BTreeSet};

use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{CleanupPlace, CleanupTransition, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedFunction};

use super::native_emit::c_field_symbol;

#[derive(Clone, Debug)]
pub(super) struct NativeBytesPlan {
    slots: BTreeMap<CleanupPlace, ByteSlot>,
    storage_leaves: BTreeMap<StorageId, Vec<CleanupPlace>>,
    transitions: BTreeMap<ExpressionId, Vec<CleanupTransition>>,
    finalizers: Vec<ByteSlot>,
    scope_exits: Vec<(BTreeSet<StorageId>, Vec<ByteSlot>)>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ByteSlot {
    place: CleanupPlace,
    value: String,
    flag: String,
}

impl NativeBytesPlan {
    pub(super) fn build(function: &ResolvedFunction) -> Result<Option<Self>, Diagnostic> {
        let mut slots = BTreeMap::new();
        let mut storage_leaves = BTreeMap::<StorageId, Vec<CleanupPlace>>::new();
        let mut by_flag = BTreeMap::new();
        for slot in &function.cleanup_plan.slots {
            flatten_byte_leaves(
                &slot.storage,
                &slot.field_liveness_shape,
                &mut Vec::new(),
                &mut |place, flag, lifecycle| {
                    if lifecycle.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID {
                        // This bridge owns only compiler-owned Bytes leaves.
                        // Authenticated user-resource lifecycles remain under
                        // the separate native resource cleanup classifier.
                        return Ok(());
                    }
                    let value = if place.projections.is_empty() {
                        format!("spx_bytes_slot_{}", slot.id.0)
                    } else {
                        format!("spx_bytes_slot_{}_leaf_{}", slot.id.0, flag.0)
                    };
                    let byte_slot = ByteSlot {
                        place: place.clone(),
                        value,
                        flag: format!("spx_bytes_live_{}", flag.0),
                    };
                    if slots.insert(place.clone(), byte_slot.clone()).is_some()
                        || by_flag.insert(flag, byte_slot).is_some()
                    {
                        return Err(error("Bytes cleanup place or flag is duplicated"));
                    }
                    storage_leaves
                        .entry(place.storage.clone())
                        .or_default()
                        .push(place);
                    Ok(())
                },
            )?;
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
                .filter(|storage| storage_leaves.contains_key(*storage))
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
            storage_leaves,
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
            .collect::<BTreeSet<_>>();
        let mut output = String::new();
        for slot in self.slots.values() {
            output.push_str(&format!("    spx_bytes_v1 {} = {{0}};\n", slot.value));
            output.push_str(&format!(
                "    bool {} = {};\n",
                slot.flag,
                live_parameters
                    .iter()
                    .any(|place| place_contains(place, &slot.place))
            ));
        }
        output
    }

    pub(super) fn value(&self, storage: &StorageId) -> Result<&str, Diagnostic> {
        self.value_at(&CleanupPlace {
            storage: storage.clone(),
            projections: Vec::new(),
        })
    }

    pub(super) fn value_at(&self, place: &CleanupPlace) -> Result<&str, Diagnostic> {
        self.slots
            .get(place)
            .map(|slot| slot.value.as_str())
            .ok_or_else(|| error(format!("Bytes place `{place:?}` is not indexed")))
    }

    pub(super) fn initialize_parameter(
        &self,
        storage: &StorageId,
        parameter: &str,
    ) -> Result<String, Diagnostic> {
        let place = CleanupPlace {
            storage: storage.clone(),
            projections: Vec::new(),
        };
        let slot = self
            .slots
            .get(&place)
            .ok_or_else(|| error("owned Bytes parameter has no cleanup slot"))?;
        Ok(format!("    {} = {parameter};\n", slot.value))
    }

    pub(super) fn initialize_record_parameter(
        &self,
        storage: &StorageId,
        parameter: &str,
    ) -> Result<String, Diagnostic> {
        let leaves = self
            .storage_leaves
            .get(storage)
            .ok_or_else(|| error("owned record parameter has no projected Bytes leaves"))?;
        let mut output = String::new();
        for place in leaves {
            let [field] = place.projections.as_slice() else {
                return Err(error("owned record parameter Bytes leaf is not direct"));
            };
            let slot = &self.slots[place];
            output.push_str(&format!(
                "    {} = spx_bytes_move(&({parameter}->{}));\n",
                slot.value,
                c_field_symbol(field)
            ));
        }
        Ok(output)
    }

    pub(super) fn has_projected_leaves(&self, storage: &StorageId) -> bool {
        self.storage_leaves
            .get(storage)
            .is_some_and(|leaves| leaves.iter().any(|place| !place.projections.is_empty()))
    }

    pub(super) fn apply_at(&self, at: &ExpressionId) -> Result<String, Diagnostic> {
        let mut output = String::new();
        for transition in self.transitions.get(at).into_iter().flatten() {
            match transition {
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } => {
                    for (source, destination) in self.transfer_pairs(source, destination)? {
                        output.push_str(&emit_transfer(source, destination, "plan transfer"));
                    }
                }
                CleanupTransition::Initialize { destination, .. } => {
                    for place in self.leaves_under(destination)? {
                        let destination = &self.slots[place];
                        output.push_str(&format!(
                            "if ({}) spx_runtime_invariant_failure(\"Bytes plan initialize liveness\");\n{} = true;\n",
                            destination.flag, destination.flag
                        ));
                    }
                }
                CleanupTransition::CallCommit { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
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
                    | CleanupTransition::Transfer { destination, .. } => {
                        self.slots.get(destination).map(|slot| slot.value.as_str())
                    }
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
                (destination.storage == *storage && destination.projections.is_empty())
                    .then_some((source, destination))
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
            .get(source)
            .ok_or_else(|| error("Bytes transfer source is not indexed"))?;
        let destination = self
            .slots
            .get(destination)
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
                let source_slot = self.slots.get(source)?;
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
        let source = &self.slots[source];
        let destination = &self.slots[destination];
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
            .find_map(|(place, slot)| match &place.storage {
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

    pub(super) fn call_argument_storage(
        &self,
        call: &ExpressionId,
        parameter_index: u32,
    ) -> Result<StorageId, Diagnostic> {
        let mut matches = self.storage_leaves.keys().filter(|storage| {
            matches!(
                storage,
                StorageId::CallArgument {
                    call: owner,
                    parameter_index: candidate,
                    ..
                } if owner == call && *candidate == parameter_index
            )
        });
        let storage = matches
            .next()
            .cloned()
            .ok_or_else(|| error("owned record call argument has no canonical epoch"))?;
        if matches.next().is_some() {
            return Err(error("owned record call argument epoch is ambiguous"));
        }
        Ok(storage)
    }

    pub(super) fn materialize_record_carrier(
        &self,
        storage: &StorageId,
        carrier: &str,
    ) -> Result<String, Diagnostic> {
        let leaves = self
            .storage_leaves
            .get(storage)
            .ok_or_else(|| error("owned record carrier storage has no Bytes leaves"))?;
        let mut output = String::new();
        for place in leaves {
            let [field] = place.projections.as_slice() else {
                return Err(error("owned record carrier Bytes leaf is not direct"));
            };
            let slot = &self.slots[place];
            output.push_str(&format!(
                "if (!{}) spx_runtime_invariant_failure(\"dead owned record field\");\n({carrier}).{} = spx_bytes_move(&{});\n{} = false;\n",
                slot.flag,
                c_field_symbol(field),
                slot.value,
                slot.flag,
            ));
        }
        Ok(output)
    }

    pub(super) fn initialize_record_result_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
    ) -> Result<String, Diagnostic> {
        let mut destinations = self.transitions.get(at).into_iter().flatten().filter_map(
            |transition| match transition {
                CleanupTransition::Initialize { destination, .. }
                    if !destination.projections.is_empty()
                        || self.storage_leaves.get(&destination.storage).is_some_and(
                            |leaves| leaves.iter().any(|leaf| !leaf.projections.is_empty()),
                        ) =>
                {
                    Some(destination)
                }
                _ => None,
            },
        );
        let destination = destinations
            .next()
            .ok_or_else(|| error("owned record result has no canonical initialization"))?;
        if destinations.next().is_some() {
            return Err(error("owned record result initialization is ambiguous"));
        }
        let mut output = String::new();
        for place in self.leaves_under(destination)? {
            let relative = &place.projections[destination.projections.len()..];
            let [field] = relative else {
                return Err(error("owned record result Bytes leaf is not direct"));
            };
            let slot = &self.slots[place];
            output.push_str(&format!(
                "{} = spx_bytes_move(&(({carrier}).{}));\n",
                slot.value,
                c_field_symbol(field),
            ));
        }
        Ok(output)
    }

    pub(super) fn publish_record_result(&self, carrier: &str) -> Result<String, Diagnostic> {
        self.materialize_record_carrier(&StorageId::ProvisionalResult, carrier)
    }

    pub(super) fn projected_value(
        &self,
        storage: &StorageId,
        field: &crate::hir::DeclarationId,
    ) -> Result<&str, Diagnostic> {
        self.value_at(&CleanupPlace {
            storage: storage.clone(),
            projections: vec![field.clone()],
        })
    }

    pub(super) fn projected_value_if_present(
        &self,
        storage: &StorageId,
        field: &crate::hir::DeclarationId,
    ) -> Option<&str> {
        self.slots
            .get(&CleanupPlace {
                storage: storage.clone(),
                projections: vec![field.clone()],
            })
            .map(|slot| slot.value.as_str())
    }

    pub(super) fn provisional(&self) -> Result<(&str, &str), Diagnostic> {
        let slot = self
            .slots
            .get(&CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            })
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
            let guard = if terminal && slot.place.storage == StorageId::ProvisionalResult {
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

    fn leaves_under(&self, prefix: &CleanupPlace) -> Result<Vec<&CleanupPlace>, Diagnostic> {
        let leaves = self
            .storage_leaves
            .get(&prefix.storage)
            .into_iter()
            .flatten()
            .filter(|place| place.projections.starts_with(&prefix.projections))
            .collect::<Vec<_>>();
        Ok(leaves)
    }

    fn transfer_pairs(
        &self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
    ) -> Result<Vec<(&ByteSlot, &ByteSlot)>, Diagnostic> {
        let sources = self.leaves_under(source)?;
        let destinations = self.leaves_under(destination)?;
        if sources.is_empty() && destinations.is_empty() {
            return Ok(Vec::new());
        }
        if sources.len() != destinations.len() {
            return Err(error("Bytes plan transfer leaf cardinality disagrees"));
        }
        let mut destination_by_suffix = BTreeMap::new();
        for place in destinations {
            destination_by_suffix.insert(
                place.projections[destination.projections.len()..].to_vec(),
                &self.slots[place],
            );
        }
        sources
            .into_iter()
            .map(|place| {
                let suffix = place.projections[source.projections.len()..].to_vec();
                let destination = destination_by_suffix
                    .remove(&suffix)
                    .ok_or_else(|| error("Bytes plan transfer projections disagree"))?;
                Ok((&self.slots[place], destination))
            })
            .collect()
    }
}

fn emit_transfer(source: &ByteSlot, destination: &ByteSlot, context: &str) -> String {
    format!(
        "if (!{} || {}) spx_runtime_invariant_failure(\"Bytes {context} liveness {} to {}\");\n{} = spx_bytes_move(&{});\n{} = false;\n{} = true;\n",
        source.flag,
        destination.flag,
        source.value,
        destination.value,
        destination.value,
        source.value,
        source.flag,
        destination.flag
    )
}

fn place_contains(prefix: &CleanupPlace, leaf: &CleanupPlace) -> bool {
    prefix.storage == leaf.storage && leaf.projections.starts_with(&prefix.projections)
}

fn flatten_byte_leaves(
    storage: &StorageId,
    shape: &FieldLivenessShape,
    projections: &mut Vec<crate::hir::DeclarationId>,
    visit: &mut impl FnMut(
        CleanupPlace,
        crate::cleanup::LivenessFlagId,
        &crate::hir::DeclarationId,
    ) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match shape {
        FieldLivenessShape::NoDrop => Ok(()),
        FieldLivenessShape::Leaf { flag, lifecycle } => {
            if projections.len() > 1 {
                return Err(error("nested owned Bytes record leaf is outside flat v1"));
            }
            visit(
                CleanupPlace {
                    storage: storage.clone(),
                    projections: projections.clone(),
                },
                *flag,
                lifecycle,
            )
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                flatten_byte_leaves(storage, &field.shape, projections, visit)?;
                projections.pop();
            }
            Ok(())
        }
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
            place: CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: Vec::new(),
            },
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

    fn projected_plan(
        source_fields: &[&str],
        destination_fields: &[&str],
    ) -> (NativeBytesPlan, StorageId, StorageId) {
        let source = crate::parse(
            "module test.native_bytes_plan; fn probe(source: i64, destination: i64) -> i64 { source } fn main() -> i64 { 0 }",
            std::path::Path::new("native-bytes-plan.spx"),
        )
        .unwrap();
        let resolved = crate::hir::resolve(&source).unwrap();
        let probe = resolved
            .functions
            .iter()
            .find(|function| function.name == "probe")
            .unwrap();
        let source_storage = StorageId::Value(probe.params[0].id.clone());
        let destination_storage = StorageId::Value(probe.params[1].id.clone());
        let mut slots = BTreeMap::new();
        let mut storage_leaves = BTreeMap::new();
        for (storage, fields, prefix) in [
            (&source_storage, source_fields, "source"),
            (&destination_storage, destination_fields, "destination"),
        ] {
            let mut leaves = Vec::new();
            for (index, field) in fields.iter().enumerate() {
                let place = CleanupPlace {
                    storage: storage.clone(),
                    projections: vec![crate::hir::DeclarationId::new(*field)],
                };
                slots.insert(
                    place.clone(),
                    ByteSlot {
                        place: place.clone(),
                        value: format!("{prefix}_{index}"),
                        flag: format!("{prefix}_live_{index}"),
                    },
                );
                leaves.push(place);
            }
            storage_leaves.insert(storage.clone(), leaves);
        }
        (
            NativeBytesPlan {
                slots,
                storage_leaves,
                transitions: BTreeMap::new(),
                finalizers: Vec::new(),
                scope_exits: Vec::new(),
            },
            source_storage,
            destination_storage,
        )
    }

    #[test]
    fn whole_record_transfer_pairs_exact_projected_leaves_in_plan_order() {
        let (plan, source, destination) =
            projected_plan(&["field.z", "field.a"], &["field.z", "field.a"]);
        let pairs = plan
            .transfer_pairs(
                &CleanupPlace {
                    storage: source,
                    projections: Vec::new(),
                },
                &CleanupPlace {
                    storage: destination,
                    projections: Vec::new(),
                },
            )
            .unwrap();
        assert_eq!(
            pairs
                .iter()
                .map(|(source, destination)| (source.value.as_str(), destination.value.as_str()))
                .collect::<Vec<_>>(),
            [("source_0", "destination_0"), ("source_1", "destination_1")]
        );
    }

    #[test]
    fn whole_record_transfer_rejects_hostile_field_identity_substitution() {
        let (plan, source, destination) = projected_plan(
            &["field.left", "field.right"],
            &["field.left", "field.other"],
        );
        assert!(plan
            .transfer_pairs(
                &CleanupPlace {
                    storage: source,
                    projections: Vec::new(),
                },
                &CleanupPlace {
                    storage: destination,
                    projections: Vec::new(),
                },
            )
            .is_err());
    }
}
