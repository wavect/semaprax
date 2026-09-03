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
use crate::hir::{DeclarationId, ExpressionId, ResolvedFunction};
use crate::variant_layout::VariantLayout;

use super::native_emit::{c_case_symbol, c_field_symbol};

mod nested_owned;

#[derive(Clone, Debug)]
pub(super) struct NativeBytesPlan {
    slots: BTreeMap<CleanupPlace, ByteSlot>,
    storage_leaves: BTreeMap<StorageId, Vec<CleanupPlace>>,
    transitions: BTreeMap<ExpressionId, Vec<CleanupTransition>>,
    finalizers: Vec<ByteSlot>,
    scope_exits: Vec<(BTreeSet<StorageId>, Vec<ByteSlot>)>,
    referenced_places: BTreeSet<CleanupPlace>,
    inactive_places: BTreeSet<CleanupPlace>,
    variant_storage: BTreeSet<StorageId>,
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
        let mut variant_domains = BTreeMap::<StorageId, BTreeSet<DeclarationId>>::new();
        let mut by_flag = BTreeMap::new();
        for slot in &function.cleanup_plan.slots {
            if let FieldLivenessShape::Variant { cases, .. } = &slot.field_liveness_shape {
                variant_domains.insert(
                    slot.storage.clone(),
                    cases.iter().map(|case| case.case.clone()).collect(),
                );
            }
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
                    | CleanupTransition::InitializeVariant { at, .. }
                    | CleanupTransition::Transfer { at, .. }
                    | CleanupTransition::TransferVariant { at, .. }
                    | CleanupTransition::AuthenticateVariantCase { at, .. } => Some(at),
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
        let reachable_cases = reachable_variant_cases(function, &variant_domains, &transitions);
        let variant_storage = variant_domains.keys().cloned().collect();
        let inactive_places = storage_leaves
            .iter()
            .flat_map(|(storage, leaves)| {
                let reachable = reachable_cases.get(storage);
                leaves.iter().filter_map(move |place| {
                    let [case, _field] = place.projections.as_slice() else {
                        return None;
                    };
                    reachable
                        .is_some_and(|cases| !cases.contains(case))
                        .then_some(place.clone())
                })
            })
            .collect::<BTreeSet<_>>();

        let mut referenced_places = BTreeSet::new();
        for place in &function.cleanup_plan.entry_state.live_owned_parameters {
            mark_referenced_under(&mut referenced_places, &storage_leaves, place);
        }
        for entry in &function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
        {
            mark_referenced_under(
                &mut referenced_places,
                &storage_leaves,
                &CleanupPlace {
                    storage: entry.storage.clone(),
                    projections: Vec::new(),
                },
            );
        }
        for transition in transitions.values().flatten() {
            match transition {
                CleanupTransition::Initialize { destination, .. }
                | CleanupTransition::InitializeVariant { destination, .. } => {
                    mark_referenced_under(&mut referenced_places, &storage_leaves, destination);
                }
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } => {
                    mark_referenced_under(&mut referenced_places, &storage_leaves, source);
                    mark_referenced_under(&mut referenced_places, &storage_leaves, destination);
                }
                CleanupTransition::TransferVariant {
                    source,
                    destination,
                    ..
                } => {
                    let reachable = reachable_cases.get(&source.storage);
                    for place in leaves_under_map(&storage_leaves, source)
                        .into_iter()
                        .chain(leaves_under_map(&storage_leaves, destination))
                    {
                        if place
                            .projections
                            .first()
                            .is_none_or(|case| reachable.is_none_or(|cases| cases.contains(case)))
                        {
                            referenced_places.insert(place);
                        }
                    }
                }
                CleanupTransition::CallCommit { arguments, .. } => {
                    for argument in arguments {
                        // Aggregate owned arguments are physically
                        // materialized as a closed carrier, whose inactive
                        // flags are checked too.
                        mark_referenced_under(
                            &mut referenced_places,
                            &storage_leaves,
                            &argument.source,
                        );
                    }
                }
                CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        referenced_places.extend(finalizers.iter().map(|slot| slot.place.clone()));
        referenced_places.extend(
            scope_exits
                .iter()
                .flat_map(|(_, actions)| actions)
                .map(|slot| slot.place.clone()),
        );
        // Result publication materializes the complete variant carrier and
        // checks every inactive flag before crossing the ABI boundary.
        if variant_domains.contains_key(&StorageId::ProvisionalResult) {
            mark_referenced_under(
                &mut referenced_places,
                &storage_leaves,
                &CleanupPlace {
                    storage: StorageId::ProvisionalResult,
                    projections: Vec::new(),
                },
            );
        }
        Ok(Some(Self {
            slots,
            storage_leaves,
            transitions,
            finalizers,
            scope_exits,
            referenced_places,
            inactive_places,
            variant_storage,
        }))
    }

    pub(super) fn declarations(
        &self,
        function: &ResolvedFunction,
        provider_retains_all_slots: bool,
    ) -> String {
        let live_parameters = function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .iter()
            .collect::<BTreeSet<_>>();
        let mut output = String::new();
        for slot in self.slots.values() {
            // A case-qualified leaf remains part of the exact static
            // inventory even when a proven payload-free case makes that leaf
            // unreachable in this function. Keep it visible to the cleanup
            // bridge without letting strict Clang warning gates reject the
            // intentionally inert declaration.
            let maybe_unused = if provider_retains_all_slots
                || (self.inactive_places.contains(&slot.place)
                    && !self.referenced_places.contains(&slot.place))
            {
                " __attribute__((unused))"
            } else {
                ""
            };
            output.push_str(&format!(
                "    spx_bytes_v1 {}{} = {{0}};\n",
                slot.value, maybe_unused
            ));
            output.push_str(&format!(
                "    bool {}{} = {};\n",
                slot.flag,
                maybe_unused,
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
            let slot = &self.slots[place];
            let path = nested_owned::c_field_path(&place.projections)?;
            output.push_str(&format!(
                "    {} = spx_bytes_move(&({parameter}->{path}));\n",
                slot.value,
            ));
        }
        Ok(output)
    }

    pub(super) fn initialize_variant_parameter(
        &self,
        storage: &StorageId,
        parameter: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let leaves = self
            .storage_leaves
            .get(storage)
            .ok_or_else(|| error("owned variant parameter has no projected Bytes leaves"))?;
        let mut output = format!(
            "    if ({parameter}->spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant parameter tag\");\n",
            layout.cases.len()
        );
        for case in &layout.cases {
            let case_leaves = leaves
                .iter()
                .filter(|place| place.projections.first() == Some(&case.case))
                .collect::<Vec<_>>();
            if case_leaves.is_empty() {
                continue;
            }
            output.push_str(&format!(
                "    if ({parameter}->spx_tag == UINT32_C({})) {{\n",
                case.tag
            ));
            for place in case_leaves {
                let [case_id, field_id] = place.projections.as_slice() else {
                    return Err(error(
                        "owned variant parameter Bytes leaf is not case-qualified",
                    ));
                };
                if case_id != &case.case || case.field(field_id).is_none() {
                    return Err(error("owned variant parameter leaf disagrees with layout"));
                }
                let slot = &self.slots[place];
                output.push_str(&format!(
                    "        if ({}) spx_runtime_invariant_failure(\"owned variant parameter leaf already live\");\n        {} = spx_bytes_move(&({parameter}->spx_payload.{}.{}));\n        {} = true;\n",
                    slot.flag,
                    slot.value,
                    c_case_symbol(case_id),
                    c_field_symbol(field_id),
                    slot.flag,
                ));
            }
            output.push_str("    }\n");
        }
        Ok(output)
    }

    pub(super) fn has_projected_leaves(&self, storage: &StorageId) -> bool {
        self.storage_leaves
            .get(storage)
            .is_some_and(|leaves| leaves.iter().any(|place| !place.projections.is_empty()))
    }

    pub(super) fn has_variant_leaves(&self, storage: &StorageId) -> bool {
        self.variant_storage.contains(storage)
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
                    if destination.projections.is_empty()
                        && self.has_variant_leaves(&destination.storage)
                    {
                        // A variant root is conditionally initialized from its
                        // authenticated runtime tag by the carrier bridge.
                        continue;
                    }
                    for place in self.leaves_under(destination)? {
                        let destination = &self.slots[place];
                        output.push_str(&format!(
                            "if ({}) spx_runtime_invariant_failure(\"Bytes plan initialize liveness\");\n{} = true;\n",
                            destination.flag, destination.flag
                        ));
                    }
                }
                CleanupTransition::CallCommit { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        Ok(output)
    }

    pub(super) fn apply_variant_case_at(
        &self,
        at: &ExpressionId,
        case: &DeclarationId,
    ) -> Result<String, Diagnostic> {
        let mut output = String::new();
        for transition in self.transitions.get(at).into_iter().flatten() {
            match transition {
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                } if source.projections.first() == Some(case)
                    || destination.projections.first() == Some(case) =>
                {
                    for (source, destination) in self.transfer_pairs(source, destination)? {
                        output.push_str(&emit_transfer(
                            source,
                            destination,
                            "selected variant transfer",
                        ));
                    }
                }
                CleanupTransition::TransferVariant {
                    source,
                    destination,
                    ..
                } => {
                    for (source, destination) in
                        self.transfer_case_pairs(source, destination, case)?
                    {
                        output.push_str(&emit_transfer(
                            source,
                            destination,
                            "known variant-case transfer",
                        ));
                    }
                }
                CleanupTransition::Initialize { destination, .. }
                    if destination.projections.first() == Some(case) =>
                {
                    for place in self.leaves_under(destination)? {
                        let destination = &self.slots[place];
                        output.push_str(&format!(
                            "if ({}) spx_runtime_invariant_failure(\"selected variant initialize liveness\");\n{} = true;\n",
                            destination.flag, destination.flag
                        ));
                    }
                }
                CleanupTransition::Initialize { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::Transfer { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::CallCommit { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        Ok(output)
    }

    pub(super) fn apply_variant_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut output = self.apply_at(at)?;
        let variant_transitions = self
            .transitions
            .get(at)
            .into_iter()
            .flatten()
            .filter_map(|transition| match transition {
                CleanupTransition::TransferVariant {
                    source,
                    destination,
                    variant,
                    ..
                } => Some((source, destination, variant)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let alternative_destinations = variant_transitions.iter().fold(
            BTreeMap::<&CleanupPlace, usize>::new(),
            |mut counts, row| {
                *counts.entry(row.1).or_default() += 1;
                counts
            },
        );
        for (source, destination, variant) in variant_transitions {
            // Variant-producing `if` branches are transferred while each
            // branch is still selected. Reapplying their joined alternatives
            // here would demand that both mutually exclusive sources be live.
            if alternative_destinations[&destination] > 1 {
                continue;
            }
            if variant != &layout.variant {
                return Err(error("variant transfer identity disagrees with layout"));
            }
            output.push_str(&self.emit_variant_transfer(source, destination, carrier, layout)?);
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
                    | CleanupTransition::InitializeVariant { .. }
                    | CleanupTransition::TransferVariant { .. }
                    | CleanupTransition::AuthenticateVariantCase { .. }
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

    pub(super) fn transfer_field_at(
        &self,
        at: &ExpressionId,
        source_value: &str,
        destination: &CleanupPlace,
    ) -> Result<String, Diagnostic> {
        let mut matches =
            self.transitions.get(at).into_iter().flatten().filter_map(
                |transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination: target,
                        ..
                    } if target == destination => Some(source),
                    _ => None,
                },
            );
        let source = matches
            .next()
            .ok_or_else(|| error("owned Bytes field has no canonical initializer transfer"))?;
        if matches.next().is_some() {
            return Err(error("owned Bytes field initializer transfer is ambiguous"));
        }
        let source = self
            .slots
            .get(source)
            .ok_or_else(|| error("owned Bytes field source is not indexed"))?;
        let destination = self
            .slots
            .get(destination)
            .ok_or_else(|| error("owned Bytes field destination is not indexed"))?;
        // Calls and blocks may already have applied this exact transition.
        // Places and branch values leave it to their consuming constructor.
        if source_value == destination.value {
            return Ok(String::new());
        }
        if source_value != source.value {
            return Err(error(
                "owned Bytes field value disagrees with its plan source",
            ));
        }
        Ok(emit_transfer(
            source,
            destination,
            "field initializer transfer",
        ))
    }

    pub(super) fn transfer_branch_at(
        &self,
        at: &ExpressionId,
        source_value: &str,
        destination: &CleanupPlace,
    ) -> Result<String, Diagnostic> {
        // Branch joins are anchored at the parent expression. Select only
        // the reached branch's exact source/destination relation; never replay
        // the other branch transitions sharing that parent anchor.
        let mut matches =
            self.transitions.get(at).into_iter().flatten().filter_map(
                |transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination: target,
                        ..
                    } if target == destination => {
                        let slot = self.slots.get(source)?;
                        (slot.value == source_value).then_some(source)
                    }
                    _ => None,
                },
            );
        let source = matches
            .next()
            .ok_or_else(|| error("Bytes branch source has no canonical result transfer"))?;
        if matches.next().is_some() {
            return Err(error("Bytes branch transfer is ambiguous"));
        }
        let source = self
            .slots
            .get(source)
            .ok_or_else(|| error("Bytes branch source is not indexed"))?;
        let destination = self
            .slots
            .get(destination)
            .ok_or_else(|| error("Bytes branch destination is not indexed"))?;
        Ok(emit_transfer(source, destination, "branch transfer"))
    }

    pub(super) fn transfer_variant_branch_to(
        &self,
        source_at: &ExpressionId,
        destination_storage: &StorageId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let source = self
            .transitions
            .get(source_at)
            .into_iter()
            .flatten()
            .rev()
            .find_map(|transition| match transition {
                CleanupTransition::TransferVariant {
                    destination,
                    variant,
                    ..
                } if variant == &layout.variant => Some(destination.clone()),
                CleanupTransition::InitializeVariant {
                    destination,
                    variant,
                    ..
                } if variant == &layout.variant => Some(destination.clone()),
                _ => None,
            })
            .ok_or_else(|| error("owned variant branch has no authenticated source"))?;
        let destination = CleanupPlace {
            storage: destination_storage.clone(),
            projections: Vec::new(),
        };
        self.emit_variant_transfer(&source, &destination, carrier, layout)
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
            let slot = &self.slots[place];
            let path = nested_owned::c_field_path(&place.projections)?;
            output.push_str(&format!(
                "if (!{}) spx_runtime_invariant_failure(\"dead owned record field\");\n({carrier}).{path} = spx_bytes_move(&{});\n{} = false;\n",
                slot.flag,
                slot.value,
                slot.flag,
            ));
        }
        Ok(output)
    }

    pub(super) fn materialize_variant_carrier(
        &self,
        storage: &StorageId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let leaves = self
            .storage_leaves
            .get(storage)
            .ok_or_else(|| error("owned variant carrier storage has no Bytes leaves"))?;
        let mut output = format!(
            "if (({carrier}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant carrier tag\");\n",
            layout.cases.len()
        );
        for case in &layout.cases {
            for place in leaves
                .iter()
                .filter(|place| place.projections.first() == Some(&case.case))
            {
                let [case_id, field_id] = place.projections.as_slice() else {
                    return Err(error(
                        "owned variant carrier Bytes leaf is not case-qualified",
                    ));
                };
                if case_id != &case.case || case.field(field_id).is_none() {
                    return Err(error("owned variant carrier leaf disagrees with layout"));
                }
                let slot = &self.slots[place];
                output.push_str(&format!(
                    "if (({carrier}).spx_tag == UINT32_C({})) {{\n    if (!{}) spx_runtime_invariant_failure(\"dead active owned variant field\");\n    ({carrier}).spx_payload.{}.{} = spx_bytes_move(&{});\n    {} = false;\n}} else if ({}) spx_runtime_invariant_failure(\"inactive owned variant field is live\");\n",
                    case.tag,
                    slot.flag,
                    c_case_symbol(case_id),
                    c_field_symbol(field_id),
                    slot.value,
                    slot.flag,
                    slot.flag,
                ));
            }
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
            let path = nested_owned::c_field_path(relative)?;
            let slot = &self.slots[place];
            output.push_str(&format!(
                "{} = spx_bytes_move(&(({carrier}).{path}));\n",
                slot.value,
            ));
        }
        Ok(output)
    }

    pub(super) fn initialize_variant_result_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut destinations = self.transitions.get(at).into_iter().flatten().filter_map(
            |transition| match transition {
                CleanupTransition::InitializeVariant {
                    destination,
                    variant,
                    ..
                } if destination.projections.is_empty()
                    && self.has_variant_leaves(&destination.storage)
                    && variant == &layout.variant =>
                {
                    Some(destination)
                }
                _ => None,
            },
        );
        let destination = destinations
            .next()
            .ok_or_else(|| error("owned variant result has no canonical initialization"))?;
        if destinations.next().is_some() {
            return Err(error("owned variant result initialization is ambiguous"));
        }
        let mut output = format!(
            "if (({carrier}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant result tag\");\n",
            layout.cases.len()
        );
        for place in self.leaves_under(destination)? {
            let [case_id, field_id] = place.projections.as_slice() else {
                return Err(error(
                    "owned variant result Bytes leaf is not case-qualified",
                ));
            };
            let case = layout
                .case(case_id)
                .ok_or_else(|| error("owned variant result case disagrees with layout"))?;
            if case.field(field_id).is_none() {
                return Err(error("owned variant result field disagrees with layout"));
            }
            let slot = &self.slots[place];
            output.push_str(&format!(
                "if (({carrier}).spx_tag == UINT32_C({})) {{\n    if ({}) spx_runtime_invariant_failure(\"owned variant result leaf already live\");\n    {} = spx_bytes_move(&(({carrier}).spx_payload.{}.{}));\n    {} = true;\n}} else if ({}) spx_runtime_invariant_failure(\"inactive owned variant result leaf is live\");\n",
                case.tag,
                slot.flag,
                slot.value,
                c_case_symbol(case_id),
                c_field_symbol(field_id),
                slot.flag,
                slot.flag,
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
        path: &[crate::hir::DeclarationId],
    ) -> Result<&str, Diagnostic> {
        self.value_at(&CleanupPlace {
            storage: storage.clone(),
            projections: path.to_vec(),
        })
    }

    pub(super) fn projected_value_if_present(
        &self,
        storage: &StorageId,
        path: &[crate::hir::DeclarationId],
    ) -> Option<&str> {
        self.slots
            .get(&CleanupPlace {
                storage: storage.clone(),
                projections: path.to_vec(),
            })
            .map(|slot| slot.value.as_str())
    }

    pub(super) fn variant_value_if_present(
        &self,
        storage: &StorageId,
        case: &DeclarationId,
        field: &DeclarationId,
    ) -> Option<&str> {
        self.slots
            .get(&CleanupPlace {
                storage: storage.clone(),
                projections: vec![case.clone(), field.clone()],
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

    fn transfer_case_pairs(
        &self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
        case: &DeclarationId,
    ) -> Result<Vec<(&ByteSlot, &ByteSlot)>, Diagnostic> {
        Ok(self
            .transfer_pairs(source, destination)?
            .into_iter()
            .filter(|(source, destination)| {
                source
                    .place
                    .projections
                    .get(source.place.projections.len().saturating_sub(2))
                    == Some(case)
                    && destination
                        .place
                        .projections
                        .get(destination.place.projections.len().saturating_sub(2))
                        == Some(case)
            })
            .collect())
    }

    fn emit_variant_transfer(
        &self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut output = format!(
            "if (({carrier}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid dynamic variant transfer tag\");\n",
            layout.cases.len()
        );
        for case in &layout.cases {
            let selected = self.transfer_case_pairs(source, destination, &case.case)?;
            for (source, destination) in selected {
                output.push_str(&format!(
                    "if (({carrier}).spx_tag == UINT32_C({})) {{\n{}\n}} else if ({} || {}) spx_runtime_invariant_failure(\"inactive dynamic variant leaf is live\");\n",
                    case.tag,
                    emit_transfer(source, destination, "dynamic variant transfer").trim_end(),
                    source.flag,
                    destination.flag,
                ));
            }
        }
        Ok(output)
    }
}

fn leaves_under_map(
    storage_leaves: &BTreeMap<StorageId, Vec<CleanupPlace>>,
    place: &CleanupPlace,
) -> Vec<CleanupPlace> {
    storage_leaves
        .get(&place.storage)
        .into_iter()
        .flatten()
        .filter(|leaf| leaf.projections.starts_with(&place.projections))
        .cloned()
        .collect()
}

fn mark_referenced_under(
    referenced_places: &mut BTreeSet<CleanupPlace>,
    storage_leaves: &BTreeMap<StorageId, Vec<CleanupPlace>>,
    place: &CleanupPlace,
) {
    referenced_places.extend(leaves_under_map(storage_leaves, place));
}

fn reachable_variant_cases(
    function: &ResolvedFunction,
    domains: &BTreeMap<StorageId, BTreeSet<DeclarationId>>,
    transitions: &BTreeMap<ExpressionId, Vec<CleanupTransition>>,
) -> BTreeMap<StorageId, BTreeSet<DeclarationId>> {
    let mut reachable = BTreeMap::<StorageId, BTreeSet<DeclarationId>>::new();
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        if let crate::hir::ResolvedExprKind::ConstructVariant { case, .. } = &expression.kind {
            reachable
                .entry(StorageId::Temporary(expression.id.clone()))
                .or_default()
                .insert(case.clone());
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    for entry in &function
        .cleanup_plan
        .entry_state
        .conditional_owned_parameters
    {
        reachable
            .entry(entry.storage.clone())
            .or_default()
            .extend(entry.cases.iter().map(|case| case.case.clone()));
    }

    let mut transfer_edges = Vec::new();
    let mut transfer_destinations = BTreeSet::new();
    for transition in transitions.values().flatten() {
        match transition {
            CleanupTransition::InitializeVariant { destination, .. } => {
                if let Some(domain) = domains.get(&destination.storage) {
                    reachable
                        .entry(destination.storage.clone())
                        .or_default()
                        .extend(domain.iter().cloned());
                }
            }
            CleanupTransition::TransferVariant {
                source,
                destination,
                ..
            } => {
                transfer_destinations.insert(destination.storage.clone());
                transfer_edges.push((source.storage.clone(), destination.storage.clone()));
            }
            CleanupTransition::Initialize { .. }
            | CleanupTransition::Transfer { .. }
            | CleanupTransition::AuthenticateVariantCase { .. }
            | CleanupTransition::CallCommit { .. }
            | CleanupTransition::SelectFailure { .. }
            | CleanupTransition::StageCopyResult { .. } => {}
        }
    }
    for (storage, domain) in domains {
        if !reachable.contains_key(storage) && !transfer_destinations.contains(storage) {
            reachable.insert(storage.clone(), domain.clone());
        }
    }
    propagate_variant_cases(&transfer_edges, &mut reachable);
    for (storage, domain) in domains {
        reachable
            .entry(storage.clone())
            .or_insert_with(|| domain.clone());
    }
    propagate_variant_cases(&transfer_edges, &mut reachable);
    reachable
}

fn propagate_variant_cases(
    edges: &[(StorageId, StorageId)],
    reachable: &mut BTreeMap<StorageId, BTreeSet<DeclarationId>>,
) {
    loop {
        let mut changed = false;
        for (source, destination) in edges {
            let source_cases = reachable.get(source).cloned().unwrap_or_default();
            let destination_cases = reachable.entry(destination.clone()).or_default();
            let before = destination_cases.len();
            destination_cases.extend(source_cases);
            changed |= destination_cases.len() != before;
        }
        if !changed {
            break;
        }
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
        FieldLivenessShape::Leaf { flag, lifecycle } => visit(
            CleanupPlace {
                storage: storage.clone(),
                projections: projections.clone(),
            },
            *flag,
            lifecycle,
        ),
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                flatten_byte_leaves(storage, &field.shape, projections, visit)?;
                projections.pop();
            }
            Ok(())
        }
        FieldLivenessShape::Variant { cases, .. } => {
            for case in cases {
                projections.push(case.case.clone());
                for field in &case.fields {
                    projections.push(field.field.clone());
                    flatten_byte_leaves(storage, &field.shape, projections, visit)?;
                    projections.pop();
                }
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
                referenced_places: BTreeSet::new(),
                inactive_places: BTreeSet::new(),
                variant_storage: BTreeSet::new(),
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
