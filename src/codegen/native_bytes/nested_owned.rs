//! Exact C member paths for authenticated nested record cleanup leaves.

use std::collections::{BTreeMap, BTreeSet};

use super::error;
use crate::cleanup_plan::{CleanupPlace, CleanupTransition, StorageId};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId};
use crate::variant_layout::{VariantCaseLayout, VariantFieldValueKind, VariantLayout};

use crate::codegen::native_emit::c_case_symbol;

pub(super) fn transition_at(transition: &CleanupTransition) -> Option<&ExpressionId> {
    match transition {
        CleanupTransition::Initialize { at, .. }
        | CleanupTransition::InitializeVariant { at, .. }
        | CleanupTransition::Transfer { at, .. }
        | CleanupTransition::Renew { at, .. }
        | CleanupTransition::ReserveRenewal { at, .. }
        | CleanupTransition::TransferVariant { at, .. }
        | CleanupTransition::AuthenticateVariantCase { at, .. } => Some(at),
        CleanupTransition::CallCommit { call, .. } => Some(call),
        CleanupTransition::SelectFailure { .. } | CleanupTransition::StageCopyResult { .. } => None,
    }
}

pub(super) fn transition_destination(transition: &CleanupTransition) -> Option<&CleanupPlace> {
    match transition {
        CleanupTransition::Initialize { destination, .. }
        | CleanupTransition::Transfer { destination, .. }
        | CleanupTransition::Renew { destination, .. } => Some(destination),
        _ => None,
    }
}

pub(super) fn c_field_path(path: &[DeclarationId]) -> Result<String, Diagnostic> {
    if path.is_empty() {
        return Err(super::error(
            "nested owned Bytes leaf has an empty field path",
        ));
    }
    Ok(path
        .iter()
        .map(crate::codegen::native_emit::c_field_symbol)
        .collect::<Vec<_>>()
        .join("."))
}

pub(super) fn materialize_variant_borrow_view(
    plan: &super::NativeBytesPlan,
    storage: &StorageId,
    carrier: &str,
    discriminant: &str,
    layout: &VariantLayout,
) -> Result<String, Diagnostic> {
    let leaves = plan
        .storage_leaves
        .get(storage)
        .ok_or_else(|| super::error("owned variant borrow-view storage has no Bytes leaves"))?;
    let mut output = format!(
        "if (({discriminant}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant borrow-view tag\");\n",
        layout.cases.len()
    );
    for case in &layout.cases {
        for place in leaves
            .iter()
            .filter(|place| place.projections.first() == Some(&case.case))
        {
            let [case_id, field_id, ..] = place.projections.as_slice() else {
                return Err(super::error(
                    "owned variant borrow-view Bytes leaf is not case-qualified",
                ));
            };
            if case_id != &case.case || case.field(field_id).is_none() {
                return Err(super::error(
                    "owned variant borrow-view leaf disagrees with layout",
                ));
            }
            let slot = &plan.slots[place];
            output.push_str(&format!(
                "if (({discriminant}).spx_tag == UINT32_C({})) {{\n    if (!{}) spx_runtime_invariant_failure(\"dead active owned variant borrow-view field\");\n    ({carrier}).spx_payload.{}.{} = {};\n}} else if ({}) spx_runtime_invariant_failure(\"inactive owned variant borrow-view field is live\");\n",
                case.tag,
                slot.flag,
                c_case_symbol(case_id),
                variant_field_path(case, &place.projections[1..])?,
                slot.value,
                slot.flag,
            ));
        }
    }
    Ok(output)
}

pub(super) fn authenticate_transfers_at(
    plan: &super::NativeBytesPlan,
    at: &ExpressionId,
    phase: Option<(&BTreeSet<StorageId>, bool)>,
) -> Result<String, Diagnostic> {
    // Several canonical transfers may share an expression boundary. Treat
    // them as one ordered transaction: later transfers can consume a slot
    // produced by an earlier transfer, so checking every source against the
    // pre-transaction runtime state would reject a valid chain.
    let mut initial = BTreeMap::<String, (bool, String)>::new();
    let mut simulated = BTreeMap::<String, bool>::new();
    for transition in plan
        .transitions
        .get(at)
        .into_iter()
        .flatten()
        .filter(|transition| {
            phase.is_none_or(|(bindings, entering)| {
                record_match_entry(transition, bindings) == entering
            })
        })
    {
        let CleanupTransition::Transfer {
            source,
            destination,
            ..
        } = transition
        else {
            continue;
        };
        for (source, destination) in plan.transfer_pairs(source, destination)? {
            if source.flag == destination.flag {
                return Err(super::error(
                    "record transfer transaction aliases source and destination",
                ));
            }
            let source_state = simulated.get(&source.flag).copied();
            if source_state == Some(false) {
                return Err(super::error(
                    "record transfer transaction consumes a simulated dead source",
                ));
            }
            if source_state.is_none() {
                initial
                    .entry(source.flag.clone())
                    .or_insert_with(|| (true, source.value.clone()));
            }
            let destination_state = simulated.get(&destination.flag).copied();
            if destination_state == Some(true) {
                return Err(super::error(
                    "record transfer transaction overwrites a simulated live destination",
                ));
            }
            if destination_state.is_none() {
                initial
                    .entry(destination.flag.clone())
                    .or_insert_with(|| (false, destination.value.clone()));
            }
            simulated.insert(source.flag.clone(), false);
            simulated.insert(destination.flag.clone(), true);
        }
    }
    let mut output = String::new();
    for (flag, (must_be_live, value)) in initial {
        let failed = if must_be_live {
            format!("!{flag}")
        } else {
            flag
        };
        let state = if must_be_live { "live" } else { "dead" };
        output.push_str(&format!(
            "if ({failed}) spx_runtime_invariant_failure(\"record transfer preflight requires {state} {value}\");\n"
        ));
    }
    Ok(output)
}

pub(super) fn apply_variant_case_at(
    plan: &super::NativeBytesPlan,
    at: &ExpressionId,
    case: &DeclarationId,
    include_variant_transfer: bool,
) -> Result<String, Diagnostic> {
    let mut output = String::new();
    for transition in plan.transitions.get(at).into_iter().flatten() {
        match transition {
            CleanupTransition::Transfer {
                source,
                destination,
                ..
            }
            | CleanupTransition::Renew {
                source,
                destination,
                ..
            } if source.projections.first() == Some(case)
                || destination.projections.first() == Some(case) =>
            {
                for (source, destination) in plan.transfer_pairs(source, destination)? {
                    output.push_str(&super::emit_transfer(
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
            } if include_variant_transfer => {
                for (source, destination) in plan.transfer_case_pairs(source, destination, case)? {
                    output.push_str(&super::emit_transfer(
                        source,
                        destination,
                        "known variant-case transfer",
                    ));
                }
            }
            CleanupTransition::Initialize { destination, .. }
                if destination.projections.first() == Some(case) =>
            {
                for place in plan.leaves_under(destination)? {
                    let destination = &plan.slots[place];
                    output.push_str(&format!(
                        "if ({}) spx_runtime_invariant_failure(\"selected variant initialize liveness\");\n{} = true;\n",
                        destination.flag, destination.flag
                    ));
                }
            }
            CleanupTransition::ReserveRenewal { .. }
            | CleanupTransition::Initialize { .. }
            | CleanupTransition::InitializeVariant { .. }
            | CleanupTransition::Transfer { .. }
            | CleanupTransition::Renew { .. }
            | CleanupTransition::TransferVariant { .. }
            | CleanupTransition::AuthenticateVariantCase { .. }
            | CleanupTransition::CallCommit { .. }
            | CleanupTransition::SelectFailure { .. }
            | CleanupTransition::StageCopyResult { .. } => {}
        }
    }
    Ok(output)
}

pub(super) fn authenticate_variant_case_at(
    plan: &super::NativeBytesPlan,
    at: &ExpressionId,
    selected: &DeclarationId,
) -> Result<String, Diagnostic> {
    let mut sources = plan
        .transitions
        .get(at)
        .into_iter()
        .flatten()
        .filter_map(|transition| match transition {
            CleanupTransition::AuthenticateVariantCase { source, case, .. } if case == selected => {
                Some(source)
            }
            CleanupTransition::TransferVariant { source, .. } => Some(source),
            _ => None,
        })
        .collect::<Vec<_>>();
    sources.dedup();
    let source = sources
        .first()
        .ok_or_else(|| super::error("selected variant case has no authenticated source"))?;
    if sources.len() != 1 {
        return Err(super::error(
            "selected variant case authentication is ambiguous",
        ));
    }
    let leaves = plan.leaves_under(source)?;
    if leaves.is_empty() {
        return Err(super::error("selected variant case has no owned leaves"));
    }
    let mut output = String::new();
    for place in leaves {
        let slot = &plan.slots[place];
        let active = place.projections.get(source.projections.len()) == Some(selected);
        output.push_str(&format!(
            "if ({}{}) spx_runtime_invariant_failure(\"owned Try case liveness disagreement\");\n",
            if active { "!" } else { "" },
            slot.flag,
        ));
    }
    Ok(output)
}

// Destructuring targets the independently checked arm bindings. Enclosing
// result transfers may share this expression ID, but execute only after the
// arm and its scope cleanup. Filtering retains each phase's canonical order.
fn record_match_entry(transition: &CleanupTransition, bindings: &BTreeSet<StorageId>) -> bool {
    matches!(transition, CleanupTransition::Transfer { destination, .. }
        if bindings.contains(&destination.storage))
}

impl super::NativeBytesPlan {
    pub(in crate::codegen) fn authenticate_transfers_at(
        &self,
        at: &ExpressionId,
    ) -> Result<String, Diagnostic> {
        authenticate_transfers_at(self, at, None)
    }
    pub(in crate::codegen) fn record_match_phase(
        &self,
        at: &ExpressionId,
        bindings: &BTreeSet<StorageId>,
        entering: bool,
        authenticate_only: bool,
    ) -> Result<String, Diagnostic> {
        if authenticate_only {
            return authenticate_transfers_at(self, at, Some((bindings, entering)));
        }
        self.apply_transitions(
            self.transitions
                .get(at)
                .into_iter()
                .flatten()
                .filter(|transition| record_match_entry(transition, bindings) == entering),
        )
    }
}

impl super::NativeBytesPlan {
    pub(in crate::codegen) fn initialize_variant_result_at(
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
            let [case_id, field_id, ..] = place.projections.as_slice() else {
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
                "if (({carrier}).spx_tag == UINT32_C({})) {{\n    if ({}) spx_runtime_invariant_failure(\"owned variant result leaf already live\");\n    {} = {};\n    {} = true;\n}} else if ({}) spx_runtime_invariant_failure(\"inactive owned variant result leaf is live\");\n",
                case.tag,
                slot.flag,
                slot.value,
                slot.kind.move_call(&format!("(({carrier}).spx_payload.{}.{})",c_case_symbol(case_id),variant_field_path(case, &place.projections[1..])?)),
                slot.flag,
                slot.flag,
            ));
        }
        Ok(output)
    }
}

/// Project only replay-authenticated record leaves inside a layout-owned case.
pub(super) fn variant_field_path(
    case: &VariantCaseLayout,
    fields: &[DeclarationId],
) -> Result<String, Diagnostic> {
    let field = fields
        .first()
        .and_then(|field| case.field(field))
        .ok_or_else(|| super::error("owned variant leaf disagrees with layout"))?;
    if fields.len() > 1 && field.value_kind != VariantFieldValueKind::OwnedRecord {
        return Err(super::error(
            "owned variant leaf nests below a non-record field",
        ));
    }
    c_field_path(fields)
}
