//! Canonical CleanupPlan bridge for compiler-owned `Bytes` in ordinary C11.
//!
//! This module never discovers ownership from lexical HIR. It indexes the
//! replay-validated plan's exact storage, transitions, flags, and finalizer
//! order. The value emitter may only perform an owned move after this bridge
//! authenticates the corresponding transition.
use std::collections::{BTreeMap, BTreeSet};

use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{CleanupPlace, CleanupTransition, ExitContinuation, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId, ResolvedFunction};
use crate::variant_layout::VariantLayout;

use super::native_emit::c_case_symbol;

mod nested_owned;
mod origin;
mod producer;
mod view;
pub(super) use producer::NativeBytesProducer;
pub(super) use view::NativeBytesView;
mod owned_leaf;
#[cfg(test)]
mod projected_vec_tests;
mod replacement;
mod scalar_match_scope;
mod scoped_vec_field;
mod semantic_work;
mod string_slots;
use owned_leaf::{emit_transfer, ByteSlot, OwnedLeafKind};
use scalar_match_scope::ScopeExit;
mod lexical_exit;
mod terminal_exits;
pub(super) use terminal_exits::{nonlexical_failure_exit, nonlexical_success_exit};

#[derive(Clone, Debug)]
pub(super) struct NativeBytesPlan {
    slots: BTreeMap<CleanupPlace, ByteSlot>,
    storage_leaves: BTreeMap<StorageId, Vec<CleanupPlace>>,
    transitions: origin::Index<ExpressionId>,
    variant_case_transitions: origin::Index<(ExpressionId, DeclarationId)>,
    finalizers: Vec<ByteSlot>,
    /// Every terminal exit runs its exact vector at its source site.
    exit_selected_terminal: bool,
    /// Nonlexical exact routing: the terminal slots its common epilogue guards.
    nonlexical_terminal: Option<Vec<ByteSlot>>,
    scope_exits: Vec<ScopeExit>,
    referenced_places: BTreeSet<CleanupPlace>,
    inactive_places: BTreeSet<CleanupPlace>,
    variant_storage: BTreeSet<StorageId>,
    semantic_function: Option<u32>,
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
                    let kind = match lifecycle.as_str() {
                        crate::hir::closure::once::DROP_ID => OwnedLeafKind::Once,
                        crate::hir::closure::once::MIXED_DROP_ID => OwnedLeafKind::OnceI64,
                        crate::hir::closure::once::PAIR_DROP_ID => OwnedLeafKind::OnceI64Pair,
                        crate::cleanup::BYTES_DROP_LIFECYCLE_ID => OwnedLeafKind::Bytes,
                        crate::cleanup::STRING_DROP_LIFECYCLE_ID => OwnedLeafKind::String,
                        crate::string_ops::MAP_DROP_LIFECYCLE_ID => OwnedLeafKind::Map,
                        crate::map_ops::DROP_ID => OwnedLeafKind::Collection,
                        crate::cleanup::ITER_DROP_LIFECYCLE_ID => OwnedLeafKind::Iter,
                        crate::stdin_stream_ops::DROP_ID if place.projections.is_empty() => {
                            OwnedLeafKind::StdinReader
                        }
                        crate::cleanup::VEC_DROP_LIFECYCLE_ID => OwnedLeafKind::Vec,
                        crate::cleanup::BOX_DROP_LIFECYCLE_ID if place.projections.is_empty() => {
                            OwnedLeafKind::Box
                        }
                        _ => {
                            // User-resource lifecycles remain under native resource cleanup.
                            return Ok(());
                        }
                    };
                    let value = if place.projections.is_empty() {
                        format!("spx_bytes_slot_{}", slot.id.0)
                    } else {
                        format!("spx_bytes_slot_{}_leaf_{}", slot.id.0, flag.0)
                    };
                    let byte_slot = ByteSlot {
                        place: place.clone(),
                        value,
                        flag: format!("spx_bytes_live_{}", flag.0),
                        kind,
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
        let mut transitions = origin::Index::<ExpressionId>::default();
        let mut variant_case_transitions = origin::Index::default();
        for block in &function.cleanup_plan.blocks {
            let ordinary = function
                .cleanup_plan
                .lexical_block_in_context(None, block.id);
            for (at, case) in block
                .transitions
                .iter()
                .filter_map(|transition| match transition {
                    CleanupTransition::AuthenticateVariantCase { at, case, .. } => Some((at, case)),
                    _ => None,
                })
            {
                let selected = block
                    .transitions
                    .iter()
                    .filter(|transition| nested_owned::transition_at(transition) == Some(at))
                    .map(|transition| origin::Tagged {
                        block: block.id,
                        ordinary,
                        transition: transition.clone(),
                    })
                    .collect();
                if variant_case_transitions.insert_context(
                    (at.clone(), case.clone()),
                    selected,
                    &function.cleanup_plan,
                )? {
                    return Err(error("variant case transition block is duplicated"));
                }
            }
            for transition in &block.transitions {
                let at = match transition {
                    CleanupTransition::Initialize { at, .. }
                    | CleanupTransition::InitializeVariant { at, .. }
                    | CleanupTransition::Transfer { at, .. }
                    | CleanupTransition::Renew { at, .. }
                    | CleanupTransition::ReserveRenewal { at, .. }
                    | CleanupTransition::TransferVariant { at, .. }
                    | CleanupTransition::AuthenticateVariantCase { at, .. } => Some(at),
                    CleanupTransition::CallCommit { call, .. } => Some(call),
                    CleanupTransition::SelectFailure { .. }
                    | CleanupTransition::StageCopyResult { .. } => None,
                };
                if let Some(at) = at {
                    transitions.push(
                        at.clone(),
                        origin::Tagged {
                            block: block.id,
                            ordinary,
                            transition: transition.clone(),
                        },
                    );
                }
            }
        }
        // Construct the union of every exit's exact pairwise precedence and
        // choose one deterministic topological order. Dead guards make absent
        // actions inert, while every actual exit retains its complete
        // canonical relative order. Contradictory exit orders are accepted
        // only when every exit is selected at its source site, lexically or
        // by authenticated nonlexical plan links.
        let terminal = terminal_exits::function_terminal_order(function, &by_flag)?;
        let finalizers = terminal.shared;
        let scope_exits =
            scalar_match_scope::build_scope_exits(function, &storage_leaves, &by_flag)?;
        let reachable_cases = reachable_variant_cases(function, &variant_domains, &transitions);
        let variant_storage = variant_domains.keys().cloned().collect();
        let inactive_places = storage_leaves
            .iter()
            .flat_map(|(storage, leaves)| {
                let reachable = reachable_cases.get(storage);
                leaves.iter().filter_map(move |place| {
                    let [case, _field, ..] = place.projections.as_slice() else {
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
        for transition in transitions.all() {
            match transition {
                CleanupTransition::Initialize { destination, .. }
                | CleanupTransition::InitializeVariant { destination, .. } => {
                    mark_referenced_under(&mut referenced_places, &storage_leaves, destination);
                }
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                }
                | CleanupTransition::Renew {
                    source,
                    destination,
                    ..
                } => {
                    mark_referenced_under(&mut referenced_places, &storage_leaves, source);
                    mark_referenced_under(&mut referenced_places, &storage_leaves, destination);
                }
                CleanupTransition::ReserveRenewal { binding, .. } => {
                    mark_referenced_under(&mut referenced_places, &storage_leaves, binding);
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
        for exit in &function.cleanup_plan.exits {
            if matches!(
                exit.continuation,
                ExitContinuation::LoopLatch { .. }
                    | ExitContinuation::LoopBreak { .. }
                    | ExitContinuation::FunctionReturn { .. }
            ) {
                referenced_places.extend(
                    exit.finalize_in_order
                        .iter()
                        .map(|action| action.source.clone()),
                );
            }
        }
        if let Some(control) = function.cleanup_plan.lexical_control.as_deref() {
            for boundary in &control.execution_boundaries {
                for action in &boundary.finalize_in_order {
                    mark_referenced_under(&mut referenced_places, &storage_leaves, &action.source);
                }
            }
        }
        referenced_places.extend(terminal.slots.iter().map(|slot| slot.place.clone()));
        referenced_places.extend(
            scope_exits
                .iter()
                .flat_map(ScopeExit::actions)
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
            variant_case_transitions,
            finalizers,
            exit_selected_terminal: terminal.exit_selected,
            nonlexical_terminal: (terminal.mode == terminal_exits::TerminalMode::Nonlexical)
                .then(|| terminal.slots.iter().cloned().collect()),
            scope_exits,
            referenced_places,
            inactive_places,
            variant_storage,
            semantic_function: None,
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
            let maybe_unused = " __attribute__((unused))";
            output.push_str(&format!(
                "    {} {}{} = {{0}};\n",
                slot.kind.c_type(),
                slot.value,
                maybe_unused
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
                "    {} = {};\n",
                slot.value,
                slot.kind.move_call(&format!("({parameter}->{path})")),
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
                let [case_id, field_id, ..] = place.projections.as_slice() else {
                    return Err(error(
                        "owned variant parameter Bytes leaf is not case-qualified",
                    ));
                };
                if case_id != &case.case || case.field(field_id).is_none() {
                    return Err(error("owned variant parameter leaf disagrees with layout"));
                }
                let slot = &self.slots[place];
                output.push_str(&format!(
                    "        if ({}) spx_runtime_invariant_failure(\"owned variant parameter leaf already live\");\n        {} = {};\n        {} = true;\n",
                    slot.flag,
                    slot.value,
                    slot.kind.move_call(&format!("({parameter}->spx_payload.{}.{})",c_case_symbol(case_id),nested_owned::variant_field_path(case, &place.projections[1..])?)),
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

    pub(super) fn call_argument(
        &self,
        call: &ExpressionId,
        parameter_index: u32,
    ) -> Result<(&str, &str, bool), Diagnostic> {
        self.slots
            .iter()
            .find_map(|(place, slot)| match &place.storage {
                StorageId::CallArgument {
                    call: owner,
                    parameter_index: candidate,
                    ..
                } if owner == call && *candidate == parameter_index => Some((
                    slot.value.as_str(),
                    slot.flag.as_str(),
                    slot.kind == OwnedLeafKind::Vec,
                )),
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
                "if (!{}) spx_runtime_invariant_failure(\"dead owned record field\");\n({carrier}).{path} = {};\n{} = false;\n",
                slot.flag,
                slot.kind.move_call(&slot.value),
                slot.flag,
            ));
        }
        Ok(output)
    }
    pub(super) fn materialize_variant_carrier(
        &self,
        storage: &StorageId,
        carrier: &str,
        discriminant: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let leaves = self
            .storage_leaves
            .get(storage)
            .ok_or_else(|| error("owned variant carrier storage has no Bytes leaves"))?;
        let mut output = format!(
            "if (({discriminant}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant carrier tag\");\n",
            layout.cases.len()
        );
        for case in &layout.cases {
            for place in leaves
                .iter()
                .filter(|place| place.projections.first() == Some(&case.case))
            {
                let [case_id, field_id, ..] = place.projections.as_slice() else {
                    return Err(error(
                        "owned variant carrier Bytes leaf is not case-qualified",
                    ));
                };
                if case_id != &case.case || case.field(field_id).is_none() {
                    return Err(error("owned variant carrier leaf disagrees with layout"));
                }
                let slot = &self.slots[place];
                output.push_str(&format!(
                    "if (({discriminant}).spx_tag == UINT32_C({})) {{\n    if (!{}) spx_runtime_invariant_failure(\"dead active owned variant field\");\n    ({carrier}).spx_payload.{}.{} = {};\n    {} = false;\n}} else if ({}) spx_runtime_invariant_failure(\"inactive owned variant field is live\");\n",
                    case.tag,
                    slot.flag,
                    c_case_symbol(case_id),
                    nested_owned::variant_field_path(case, &place.projections[1..])?,
                    slot.kind.move_call(&slot.value),
                    slot.flag,
                    slot.flag,
                ));
            }
        }
        Ok(output)
    }

    pub(super) fn materialize_variant_borrow_view(
        &self,
        storage: &StorageId,
        carrier: &str,
        discriminant: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        nested_owned::materialize_variant_borrow_view(self, storage, carrier, discriminant, layout)
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
        if self.exit_selected_terminal {
            // Reached only under `!spx_canonical_cleanup_done`: every exact
            // terminal exit already ran, so no contradictory order is guessed.
            return self.selected_epilogue();
        }
        self.emit_finalizers(&self.finalizers, true)
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
                "    if ({guard}) {{ {} = false; {}{}; }}\n",
                slot.flag,
                self.semantic_event(slot),
                slot.kind.drop_call(&slot.value),
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
        let source_case_index = source.projections.len();
        let destination_case_index = destination.projections.len();
        Ok(self
            .transfer_pairs(source, destination)?
            .into_iter()
            .filter(|(source, destination)| {
                source.place.projections.get(source_case_index) == Some(case)
                    && destination.place.projections.get(destination_case_index) == Some(case)
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
    transitions: &origin::Index<ExpressionId>,
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
    for transition in transitions.all() {
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
            | CleanupTransition::Renew { .. }
            | CleanupTransition::ReserveRenewal { .. }
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

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B104", message)
}

impl NativeBytesProducer for NativeBytesPlan {
    fn base(&self) -> &NativeBytesPlan {
        self
    }
    fn transitions(&self) -> origin::Selection<'_, ExpressionId> {
        self.transitions.select(None)
    }
    fn variant_cases(&self) -> origin::Selection<'_, (ExpressionId, DeclarationId)> {
        self.variant_case_transitions.select(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_selected_terminal_epilogue_fails_closed_without_guessing_order() {
        let (mut plan, _, _) = projected_plan(&["field.a"], &["field.a"]);
        plan.finalizers = plan.slots.values().cloned().collect();
        let shared = plan.epilogue();
        assert!(shared.contains("spx_bytes_drop(&source_0)"));
        plan.finalizers.clear();
        plan.exit_selected_terminal = true;
        assert_eq!(
            plan.epilogue(),
            "    spx_runtime_invariant_failure(\"native terminal cleanup lacks its exact selected exit\");\n"
        );
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
                        kind: OwnedLeafKind::Bytes,
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
                transitions: origin::Index::default(),
                variant_case_transitions: origin::Index::default(),
                finalizers: Vec::new(),
                exit_selected_terminal: false,
                nonlexical_terminal: None,
                scope_exits: Vec::new(),
                referenced_places: BTreeSet::new(),
                inactive_places: BTreeSet::new(),
                variant_storage: BTreeSet::new(),
                semantic_function: None,
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
