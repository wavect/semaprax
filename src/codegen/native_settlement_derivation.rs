//! Compiler-derived callable-v3 settlement proof data.
//!
//! This module stops at deterministic, target-neutral derivation. It emits no
//! descriptor, provider, loader, host authority, physical finalizer, or public
//! API, and it does not alter callable-v2 bundle contents or `SPX-B104`.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "callable-v3 remains unreachable from compiler preflight"
    )
)]

use std::collections::{BTreeMap, BTreeSet, HashMap};

use sha2::{Digest, Sha256};

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{
    BlockId, CleanupPlace, CleanupResultSource, CleanupTerminator, CleanupTransition, EdgeId,
    ExitContinuation, FinalizeAction, StatusSourceId, StorageId,
};
use crate::conformance::TraceEventKind;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclarationKind,
};
use crate::native_settlement::{
    NativeSettlementCertificate, SettlementCheckpointSpec, SettlementOutcome,
    SettlementProgressAction, SettlementProgressEdge, SettlementResourceState,
};
use crate::semantic_trace::SemanticEventDictionary;
use crate::trace_path_certificate::{TracePathCertificate, TracePathOutcome};

use super::native_host_contract::{
    NativeAdapterParameterProjection, NativeAdapterResultProjection,
};
use super::{native_cleanup, native_host_contract, native_resource, native_value};

const RECOVERY_CONTRACT_DOMAIN: &[u8] = b"semaprax.native-recovery-contract.v1\0";
const TRACE_EVIDENCE_DOMAIN: &[u8] = b"semaprax.native-recovery-trace-evidence.v1\0";
const MAX_DERIVATION_PATHS: usize = 65_536;
const MAX_DERIVATION_WORK_UNITS: usize = 1_000_000;

/// Private, authority-free result of callable-v3 settlement derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeSettlementDerivation {
    recovery_contract_fingerprint: [u8; 32],
    certificate: NativeSettlementCertificate,
}

impl NativeSettlementDerivation {
    pub(super) const fn recovery_contract_fingerprint(&self) -> [u8; 32] {
        self.recovery_contract_fingerprint
    }

    pub(super) fn certificate(&self) -> &NativeSettlementCertificate {
        &self.certificate
    }
}

#[derive(Clone)]
struct PathState {
    block: BlockId,
    owners_by_flag: BTreeMap<LivenessFlagId, u32>,
    progress: Vec<PhysicalProgress>,
    selected: Option<StatusSourceId>,
    trace_ordinals: Vec<u32>,
    visited: BTreeSet<BlockId>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TerminalPath {
    progress: Vec<PhysicalProgress>,
    outcome: SettlementOutcome,
    trace_evidence: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PhysicalProgress {
    Finalize(u32),
    StageOwnedResult(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DerivedCheckpointRow {
    states: Vec<SettlementResourceState>,
    outcome: Option<SettlementOutcome>,
    abort: Vec<u32>,
}

/// Derive one certificate from the exact validated program member.
pub(super) fn derive_native_settlement(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeSettlementDerivation, Diagnostic> {
    crate::hir::validate(program)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| &candidate.id == function_id)
        .ok_or_else(|| derivation_error("settlement function is not in the validated program"))?;

    // Reuse the current private callable-v2 classifier only as admission proof;
    // no callable-v2 fingerprint or artifact becomes part of this contract.
    let resource_abi = native_resource::build_resource_abi(program)?;
    let cleanup = native_cleanup::classify(program, function)?;
    let values = native_value::plan(program, function, &cleanup, &resource_abi, &HashMap::new())?;
    let template = native_host_contract::derive_from_admitted(
        program,
        &function.id,
        &resource_abi,
        &cleanup,
        &values,
    )?;
    let projection = native_host_contract::project_for_callable_abi(&template);

    let owners = owner_meanings(program, function, &projection.parameters)?;
    if owners.is_empty() {
        return Err(derivation_error(
            "settlement derivation requires at least one direct owned resource",
        ));
    }
    let result_owner = match &projection.result {
        NativeAdapterResultProjection::ScalarI64 => None,
        NativeAdapterResultProjection::OwnedInput { owner_ordinal, .. } => Some(
            u32::try_from(*owner_ordinal)
                .map_err(|_| derivation_error("owned result ordinal exceeds u32"))?,
        ),
    };
    if result_owner.is_some_and(|ordinal| ordinal as usize >= owners.len()) {
        return Err(derivation_error(
            "owned result ordinal is outside the owner table",
        ));
    }

    let recovery_contract_fingerprint = recovery_contract_fingerprint(
        &program.module,
        function,
        &owners,
        result_owner,
        &crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan),
    );
    let dictionary = crate::semantic_trace::build_semantic_event_dictionary(program, &function.id)?;
    let trace_certificate = crate::trace_path_certificate::build_trace_path_certificate(
        program,
        function,
        &dictionary,
    )?;
    let terminals = collect_terminal_paths(
        function,
        &owners,
        result_owner,
        &dictionary,
        &trace_certificate,
    )?;
    let (checkpoints, progress_edges) = derive_checkpoints(&terminals, owners.len())?;
    let certificate = NativeSettlementCertificate::try_new_with_progress(
        function.id.clone(),
        recovery_contract_fingerprint,
        owners.len(),
        checkpoints,
        vec![1],
        progress_edges,
    )
    .map_err(|error| {
        derivation_error(format!("invalid derived settlement certificate: {error}"))
    })?;

    Ok(NativeSettlementDerivation {
        recovery_contract_fingerprint,
        certificate,
    })
}

#[derive(Clone)]
struct OwnerMeaning {
    parameter_index: usize,
    value_id: String,
    resource: String,
    lifecycle: String,
}

fn owner_meanings(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    parameters: &[NativeAdapterParameterProjection],
) -> Result<Vec<OwnerMeaning>, Diagnostic> {
    let mut owners = Vec::new();
    for parameter in parameters {
        let NativeAdapterParameterProjection::OwnedResource {
            parameter_index,
            value_id,
            owner_ordinal,
            ..
        } = parameter
        else {
            continue;
        };
        if *owner_ordinal != owners.len() {
            return Err(derivation_error(
                "owned parameter ordinals are not dense signature order",
            ));
        }
        let resolved = function
            .params
            .iter()
            .find(|candidate| candidate.id == *value_id)
            .ok_or_else(|| derivation_error("owned parameter is absent from validated HIR"))?;
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &resolved.ty
        else {
            return Err(derivation_error("owned parameter is not a direct resource"));
        };
        if !arguments.is_empty() {
            return Err(derivation_error(
                "generic owned resources are outside callable v3",
            ));
        }
        let ty = program
            .types
            .iter()
            .find(|candidate| candidate.id == *declaration)
            .ok_or_else(|| derivation_error("owned resource declaration is absent"))?;
        let ResolvedTypeDeclarationKind::Resource { drop } = &ty.kind else {
            return Err(derivation_error(
                "owned parameter is not a resource declaration",
            ));
        };
        if drop.kind != ResolvedResourceDropKind::Trivial {
            return Err(derivation_error(
                "only trivial resource lifecycles are admitted for callable v3 derivation",
            ));
        }
        owners.push(OwnerMeaning {
            parameter_index: *parameter_index,
            value_id: value_id.as_str().to_owned(),
            resource: declaration.as_str().to_owned(),
            lifecycle: drop.id.as_str().to_owned(),
        });
    }
    Ok(owners)
}

fn recovery_contract_fingerprint(
    module: &str,
    function: &ResolvedFunction,
    owners: &[OwnerMeaning],
    result_owner: Option<u32>,
    cleanup_plan: &str,
) -> [u8; 32] {
    let owner_json = owners
        .iter()
        .enumerate()
        .map(|(ordinal, owner)| {
            format!(
                "{{\"owner_ordinal\":{ordinal},\"parameter_index\":{},\"value\":{},\"resource\":{},\"lifecycle\":{}}}",
                owner.parameter_index,
                quote_json(&owner.value_id),
                quote_json(&owner.resource),
                quote_json(&owner.lifecycle),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let result = result_owner.map_or_else(
        || "{\"kind\":\"scalar_i64\"}".to_owned(),
        |ordinal| format!("{{\"kind\":\"owned_input\",\"owner_ordinal\":{ordinal}}}"),
    );
    let projection = format!(
        "{{\"module\":{},\"function\":{},\"owners\":[{owner_json}],\"result\":{result},\"cleanup_plan\":{cleanup_plan}}}",
        quote_json(module),
        quote_json(function.id.as_str()),
    );
    let mut hasher = Sha256::new();
    hasher.update(RECOVERY_CONTRACT_DOMAIN);
    hasher.update((projection.len() as u64).to_le_bytes());
    hasher.update(projection.as_bytes());
    hasher.finalize().into()
}

fn collect_terminal_paths(
    function: &ResolvedFunction,
    owners: &[OwnerMeaning],
    result_owner: Option<u32>,
    dictionary: &SemanticEventDictionary,
    trace_certificate: &TracePathCertificate,
) -> Result<Vec<TerminalPath>, Diagnostic> {
    let mut owners_by_flag = BTreeMap::new();
    for (ordinal, owner) in owners.iter().enumerate() {
        let parameter = function
            .params
            .get(owner.parameter_index)
            .ok_or_else(|| derivation_error("owner parameter index is absent"))?;
        if parameter.id.as_str() != owner.value_id {
            return Err(derivation_error(
                "owner parameter binding disagrees with HIR",
            ));
        }
        let place = CleanupPlace {
            storage: StorageId::Value(parameter.id.clone()),
            projections: Vec::new(),
        };
        let flags = flags_under(function, &place)?;
        if flags.len() != 1 {
            return Err(derivation_error(
                "callable-v3 direct owner must have exactly one cleanup leaf",
            ));
        }
        let ordinal =
            u32::try_from(ordinal).map_err(|_| derivation_error("owner ordinal exceeds u32"))?;
        if owners_by_flag.insert(flags[0], ordinal).is_some() {
            return Err(derivation_error("cleanup owner flag is duplicated"));
        }
    }

    let mut pending = vec![PathState {
        block: function.cleanup_plan.entry,
        owners_by_flag,
        progress: Vec::new(),
        selected: None,
        trace_ordinals: Vec::new(),
        visited: BTreeSet::new(),
    }];
    let mut terminals = Vec::new();
    let mut seen_terminals = BTreeSet::new();
    let mut work = 0_usize;
    while let Some(mut state) = pending.pop() {
        work = work
            .checked_add(1)
            .ok_or_else(|| derivation_error("settlement derivation work counter overflow"))?;
        if work > MAX_DERIVATION_WORK_UNITS {
            return Err(derivation_error(
                "settlement derivation work budget exhausted",
            ));
        }
        if !state.visited.insert(state.block) {
            return Err(derivation_error("settlement cleanup path revisits a block"));
        }
        let block = function
            .cleanup_plan
            .blocks
            .iter()
            .find(|block| block.id == state.block)
            .ok_or_else(|| derivation_error("settlement path references a missing block"))?;
        for transition in &block.transitions {
            apply_transition(function, dictionary, &mut state, transition)?;
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                state.block = edge_target(function, state.block, *edge)?;
                pending.push(state);
            }
            CleanupTerminator::Branch(edges) => {
                for edge in edges.iter().rev() {
                    let mut branch = state.clone();
                    branch.block = edge_target(function, state.block, *edge)?;
                    pending.push(branch);
                }
            }
            CleanupTerminator::Exit(exit_id) => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter()
                    .find(|exit| exit.id == *exit_id && exit.from == state.block)
                    .ok_or_else(|| derivation_error("settlement path references a missing exit"))?;
                for action in &exit.finalize_in_order {
                    finalize_owner(function, dictionary, &mut state, action)?;
                }
                match &exit.continuation {
                    ExitContinuation::Continue(edge) => {
                        state.block = edge_target(function, state.block, *edge)?;
                        pending.push(state);
                    }
                    ExitContinuation::CommitResult { source } => {
                        let outcome = match source {
                            CleanupResultSource::Scalar { .. } => {
                                if !state.owners_by_flag.is_empty() {
                                    return Err(derivation_error(
                                        "scalar settlement leaves a live owner",
                                    ));
                                }
                                SettlementOutcome::ScalarSuccess
                            }
                            CleanupResultSource::Owned { storage } => {
                                let selected = owner_for_place(function, &state, storage)?;
                                if Some(selected) != result_owner
                                    || state.owners_by_flag.values().copied().collect::<Vec<_>>()
                                        != [selected]
                                {
                                    return Err(derivation_error(
                                        "owned settlement result disagrees with compiler proof",
                                    ));
                                }
                                SettlementOutcome::OwnedSuccess {
                                    owner_ordinal: selected,
                                }
                            }
                        };
                        state.trace_ordinals.push(event_ordinal(
                            dictionary,
                            &TraceEventKind::ResultCommit {
                                source: source.clone(),
                            },
                        )?);
                        let trace_outcome = match outcome {
                            SettlementOutcome::ScalarSuccess => TracePathOutcome::ScalarSuccess,
                            SettlementOutcome::OwnedSuccess { .. } => {
                                TracePathOutcome::OwnedSuccess
                            }
                            SettlementOutcome::SemanticFailure => unreachable!(),
                        };
                        if !trace_certificate.accepts(&state.trace_ordinals, trace_outcome) {
                            return Err(derivation_error(
                                "settlement success path is absent from the trace certificate",
                            ));
                        }
                        let terminal = TerminalPath {
                            progress: state.progress,
                            outcome,
                            trace_evidence: trace_evidence(
                                trace_certificate,
                                &state.trace_ordinals,
                                trace_outcome,
                            ),
                        };
                        if seen_terminals.insert(terminal.clone()) {
                            terminals.push(terminal);
                        }
                    }
                    ExitContinuation::ReturnFailure { source } => {
                        if !state.owners_by_flag.is_empty() {
                            return Err(derivation_error(
                                "semantic failure settlement leaves a live owner",
                            ));
                        }
                        if state.selected.as_ref() != Some(source) {
                            return Err(derivation_error(
                                "semantic failure does not match selected status",
                            ));
                        }
                        let selected_ordinal = select_failure_ordinal(dictionary, source)?;
                        let trace_outcome = TracePathOutcome::Failure { selected_ordinal };
                        if !trace_certificate.accepts(&state.trace_ordinals, trace_outcome) {
                            return Err(derivation_error(
                                "settlement failure path is absent from the trace certificate",
                            ));
                        }
                        let terminal = TerminalPath {
                            progress: state.progress,
                            outcome: SettlementOutcome::SemanticFailure,
                            trace_evidence: trace_evidence(
                                trace_certificate,
                                &state.trace_ordinals,
                                trace_outcome,
                            ),
                        };
                        if seen_terminals.insert(terminal.clone()) {
                            terminals.push(terminal);
                        }
                    }
                    ExitContinuation::ReturnUnit => {
                        return Err(derivation_error("unit return is outside callable v3"));
                    }
                }
            }
        }
        if pending.len().saturating_add(terminals.len()) > MAX_DERIVATION_PATHS {
            return Err(derivation_error(
                "settlement path count exceeds the audited limit",
            ));
        }
    }
    if terminals.is_empty() {
        return Err(derivation_error("settlement CFG has no terminal path"));
    }
    Ok(terminals)
}

fn apply_transition(
    function: &ResolvedFunction,
    dictionary: &SemanticEventDictionary,
    state: &mut PathState,
    transition: &CleanupTransition,
) -> Result<(), Diagnostic> {
    match transition {
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            let source_flags = flags_under(function, source)?;
            let destination_flags = flags_under(function, destination)?;
            if source_flags.len() != 1 || destination_flags.len() != 1 {
                return Err(derivation_error(
                    "callable-v3 transfer must preserve one direct owner leaf",
                ));
            }
            let owner = state
                .owners_by_flag
                .remove(&source_flags[0])
                .ok_or_else(|| derivation_error("settlement transfer source is not live"))?;
            if state
                .owners_by_flag
                .insert(destination_flags[0], owner)
                .is_some()
            {
                return Err(derivation_error("settlement transfer destination is live"));
            }
            state.trace_ordinals.push(event_ordinal(
                dictionary,
                &TraceEventKind::Transfer {
                    at: at.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                },
            )?);
            if destination.storage == StorageId::ProvisionalResult {
                state
                    .progress
                    .push(PhysicalProgress::StageOwnedResult(owner));
            }
        }
        CleanupTransition::SelectFailure { source } => {
            if state.selected.replace(source.clone()).is_some() {
                return Err(derivation_error(
                    "settlement failure selection is not write-once",
                ));
            }
            state
                .trace_ordinals
                .push(select_failure_ordinal(dictionary, source)?);
        }
        CleanupTransition::Initialize { .. } | CleanupTransition::CallCommit { .. } => {
            return Err(derivation_error(
                "transition is outside the callable-v3 direct-owner slice",
            ));
        }
    }
    Ok(())
}

fn finalize_owner(
    function: &ResolvedFunction,
    dictionary: &SemanticEventDictionary,
    state: &mut PathState,
    action: &FinalizeAction,
) -> Result<(), Diagnostic> {
    let flags = flags_under(function, &action.source)?;
    if flags != [action.guard_flag] {
        return Err(derivation_error(
            "settlement finalizer does not name one exact owner leaf",
        ));
    }
    let owner = state
        .owners_by_flag
        .remove(&action.guard_flag)
        .ok_or_else(|| derivation_error("settlement finalizer owner is not live"))?;
    state
        .trace_ordinals
        .push(finalizer_ordinal(dictionary, action, true)?);
    state
        .trace_ordinals
        .push(finalizer_ordinal(dictionary, action, false)?);
    state.progress.push(PhysicalProgress::Finalize(owner));
    Ok(())
}

fn owner_for_place(
    function: &ResolvedFunction,
    state: &PathState,
    place: &CleanupPlace,
) -> Result<u32, Diagnostic> {
    let flags = flags_under(function, place)?;
    if flags.len() != 1 {
        return Err(derivation_error(
            "settlement result has multiple owner leaves",
        ));
    }
    state
        .owners_by_flag
        .get(&flags[0])
        .copied()
        .ok_or_else(|| derivation_error("settlement result owner is not live"))
}

fn flags_under(
    function: &ResolvedFunction,
    place: &CleanupPlace,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    let mut flags = Vec::new();
    for slot in &function.cleanup_plan.slots {
        if slot.storage != place.storage {
            continue;
        }
        collect_flags(
            &slot.storage,
            &mut Vec::new(),
            &slot.field_liveness_shape,
            place,
            &mut flags,
        );
    }
    if flags.is_empty() {
        return Err(derivation_error(
            "settlement cleanup place has no liveness flag",
        ));
    }
    Ok(flags)
}

fn collect_flags(
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    place: &CleanupPlace,
    flags: &mut Vec<LivenessFlagId>,
) {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { flag, .. } => {
            if storage == &place.storage && projections.starts_with(&place.projections) {
                flags.push(*flag);
            }
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                collect_flags(storage, projections, &field.shape, place, flags);
                projections.pop();
            }
        }
    }
}

fn edge_target(
    function: &ResolvedFunction,
    from: BlockId,
    edge_id: EdgeId,
) -> Result<BlockId, Diagnostic> {
    function
        .cleanup_plan
        .edges
        .iter()
        .find(|edge| edge.id == edge_id && edge.from == from)
        .map(|edge| edge.to)
        .ok_or_else(|| derivation_error("settlement path references a missing edge"))
}

fn event_ordinal(
    dictionary: &SemanticEventDictionary,
    event: &TraceEventKind,
) -> Result<u32, Diagnostic> {
    dictionary
        .ordinal_for(event)
        .ok_or_else(|| derivation_error("settlement event is absent from its trace dictionary"))
}

fn select_failure_ordinal(
    dictionary: &SemanticEventDictionary,
    source: &StatusSourceId,
) -> Result<u32, Diagnostic> {
    let mut matches = dictionary.entries().iter().filter_map(|entry| {
        matches!(
            &entry.event,
            TraceEventKind::SelectFailure { source: candidate, .. } if candidate == source
        )
        .then_some(entry.ordinal)
    });
    let ordinal = matches
        .next()
        .ok_or_else(|| derivation_error("failure selection is absent from trace dictionary"))?;
    if matches.next().is_some() {
        return Err(derivation_error(
            "failure selection is ambiguous in trace dictionary",
        ));
    }
    Ok(ordinal)
}

fn finalizer_ordinal(
    dictionary: &SemanticEventDictionary,
    action: &FinalizeAction,
    begin: bool,
) -> Result<u32, Diagnostic> {
    let mut matches = dictionary.entries().iter().filter_map(|entry| {
        let matched = match &entry.event {
            TraceEventKind::FinalizeBegin {
                source,
                lifecycle_id,
                guard_flag,
                ..
            } if begin => {
                source == &action.source
                    && lifecycle_id == &action.lifecycle_id
                    && guard_flag == &action.guard_flag
            }
            TraceEventKind::FinalizeEnd {
                source,
                lifecycle_id,
                guard_flag,
                ..
            } if !begin => {
                source == &action.source
                    && lifecycle_id == &action.lifecycle_id
                    && guard_flag == &action.guard_flag
            }
            _ => false,
        };
        matched.then_some(entry.ordinal)
    });
    let ordinal = matches
        .next()
        .ok_or_else(|| derivation_error("finalizer is absent from trace dictionary"))?;
    if matches.next().is_some() {
        return Err(derivation_error(
            "finalizer is ambiguous in trace dictionary",
        ));
    }
    Ok(ordinal)
}

fn trace_evidence(
    certificate: &TracePathCertificate,
    ordinals: &[u32],
    outcome: TracePathOutcome,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TRACE_EVIDENCE_DOMAIN);
    hasher.update(certificate.fingerprint());
    hasher.update((ordinals.len() as u64).to_le_bytes());
    for ordinal in ordinals {
        hasher.update(ordinal.to_le_bytes());
    }
    match outcome {
        TracePathOutcome::ScalarSuccess => hasher.update([1]),
        TracePathOutcome::OwnedSuccess => hasher.update([2]),
        TracePathOutcome::Failure { selected_ordinal } => {
            hasher.update([3]);
            hasher.update(selected_ordinal.to_le_bytes());
        }
    }
    hasher.finalize().into()
}

fn derive_checkpoints(
    terminals: &[TerminalPath],
    resource_count: usize,
) -> Result<(Vec<SettlementCheckpointSpec>, Vec<SettlementProgressEdge>), Diagnostic> {
    let initial_states = vec![SettlementResourceState::Live; resource_count];
    let first_abort = terminals
        .first()
        .map(|terminal| abort_order(&initial_states, &terminal.progress, 0))
        .ok_or_else(|| derivation_error("settlement derivation has no terminal path"))?;
    for terminal in &terminals[1..] {
        if abort_order(&initial_states, &terminal.progress, 0) != first_abort {
            return Err(derivation_error(
                "post-commit abort cleanup order is path-ambiguous",
            ));
        }
    }
    let mut rows = vec![DerivedCheckpointRow {
        states: initial_states,
        outcome: None,
        abort: first_abort,
    }];
    let mut edges = Vec::new();
    let mut edge_targets = BTreeMap::new();
    for terminal in terminals {
        let mut states = vec![SettlementResourceState::Live; resource_count];
        let mut from = 1_u32;
        for (progress_index, progress) in terminal.progress.iter().enumerate() {
            let action = match *progress {
                PhysicalProgress::Finalize(owner) => {
                    let state = states.get_mut(owner as usize).ok_or_else(|| {
                        derivation_error("settlement finalizer ordinal is outside state")
                    })?;
                    if !matches!(
                        state,
                        SettlementResourceState::Live | SettlementResourceState::ProvisionalResult
                    ) {
                        return Err(derivation_error(
                            "settlement owner is finalized more than once",
                        ));
                    }
                    *state = SettlementResourceState::Dead;
                    SettlementProgressAction::Finalize {
                        owner_ordinal: owner,
                    }
                }
                PhysicalProgress::StageOwnedResult(owner) => {
                    let state = states.get_mut(owner as usize).ok_or_else(|| {
                        derivation_error("settlement result ordinal is outside state")
                    })?;
                    if *state != SettlementResourceState::Live {
                        return Err(derivation_error("settlement result owner is not live"));
                    }
                    *state = SettlementResourceState::ProvisionalResult;
                    SettlementProgressAction::StageOwnedResult {
                        owner_ordinal: owner,
                    }
                }
            };
            let abort = abort_order(&states, &terminal.progress, progress_index + 1);
            from = insert_progress_row(
                &mut rows,
                &mut edges,
                &mut edge_targets,
                from,
                action,
                DerivedCheckpointRow {
                    states: states.clone(),
                    outcome: None,
                    abort,
                },
            )?;
        }
        let action = SettlementProgressAction::CertifyOutcome {
            trace_evidence: terminal.trace_evidence,
        };
        let abort = abort_order(&states, &terminal.progress, terminal.progress.len());
        let _ = insert_progress_row(
            &mut rows,
            &mut edges,
            &mut edge_targets,
            from,
            action,
            DerivedCheckpointRow {
                states,
                outcome: Some(terminal.outcome),
                abort,
            },
        )?;
    }

    let checkpoints = rows
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let checkpoint = u32::try_from(index + 1)
                .map_err(|_| derivation_error("settlement checkpoint count exceeds u32"))?;
            let accept = row
                .outcome
                .map(|_| {
                    row.abort
                        .iter()
                        .copied()
                        .filter(|ordinal| {
                            row.states[*ordinal as usize] == SettlementResourceState::Live
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(SettlementCheckpointSpec::new(
                checkpoint,
                row.states,
                row.outcome,
                row.abort,
                accept,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((checkpoints, edges))
}

fn next_checkpoint(rows: &[DerivedCheckpointRow]) -> Result<u32, Diagnostic> {
    u32::try_from(rows.len() + 1)
        .map_err(|_| derivation_error("settlement checkpoint count exceeds u32"))
}

fn insert_progress_row(
    rows: &mut Vec<DerivedCheckpointRow>,
    edges: &mut Vec<SettlementProgressEdge>,
    targets: &mut BTreeMap<(u32, SettlementProgressAction), u32>,
    from: u32,
    action: SettlementProgressAction,
    row: DerivedCheckpointRow,
) -> Result<u32, Diagnostic> {
    if let Some(to) = targets.get(&(from, action)).copied() {
        if rows[(to - 1) as usize] != row {
            return Err(derivation_error(
                "settlement progress prefix has inconsistent compiler meaning",
            ));
        }
        return Ok(to);
    }
    let to = next_checkpoint(rows)?;
    rows.push(row);
    edges.push(SettlementProgressEdge::new(from, to, action));
    targets.insert((from, action), to);
    Ok(to)
}

fn abort_order(
    states: &[SettlementResourceState],
    progress: &[PhysicalProgress],
    next_progress: usize,
) -> Vec<u32> {
    let mut order = Vec::new();
    for (ordinal, state) in states.iter().enumerate() {
        if *state == SettlementResourceState::ProvisionalResult {
            order.push(u32::try_from(ordinal).expect("resource bound fits u32"));
        }
    }
    for action in &progress[next_progress..] {
        let owner = match action {
            PhysicalProgress::Finalize(owner) | PhysicalProgress::StageOwnedResult(owner) => *owner,
        };
        if states[owner as usize] != SettlementResourceState::Dead && !order.contains(&owner) {
            order.push(owner);
        }
    }
    order
}

fn derivation_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-I104",
        format!("native settlement derivation: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;
    use std::num::NonZeroU64;
    use std::path::Path;

    use crate::conformance::{TraceOutcome, TraceResult};
    use crate::native_settlement::{
        AdapterAbortReason, SettlementDecision, SettlementError, SettlementOutcome,
    };
    use crate::owned_resource_corpus::build_owned_resource_corpus_v1;
    use crate::owned_resource_corpus::OWNED_RESOURCE_CORPUS_SOURCE_V1;

    use super::*;

    #[test]
    fn authoritative_corpus_derives_deterministically_and_settles_every_checkpoint() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        assert_eq!(corpus.cases.len(), 14);
        let mut functions = BTreeSet::new();
        let aborts = [
            AdapterAbortReason::PhysicalResult(7),
            AdapterAbortReason::MalformedResponse,
            AdapterAbortReason::TraceRejected,
            AdapterAbortReason::HostUnwind,
        ];
        let mut invocation = 1_u64;

        for case in &corpus.cases {
            let function = corpus
                .program
                .functions
                .iter()
                .find(|function| function.id.as_str() == case.function_id)
                .unwrap();
            let first = derive_native_settlement(&corpus.program, &function.id).unwrap();
            let second = derive_native_settlement(&corpus.program, &function.id).unwrap();
            assert_eq!(first, second, "{}", case.scenario_id);
            assert_eq!(
                first.certificate().recovery_contract(),
                first.recovery_contract_fingerprint()
            );
            assert!(first
                .recovery_contract_fingerprint()
                .iter()
                .any(|byte| *byte != 0));

            let expected = match &case.reference.outcome {
                TraceOutcome::Failure { .. } => SettlementOutcome::SemanticFailure,
                TraceOutcome::Success {
                    result: TraceResult::Owned { .. },
                } => SettlementOutcome::OwnedSuccess {
                    owner_ordinal: u32::try_from(case.expected_owned_result_ordinal.unwrap())
                        .unwrap(),
                },
                TraceOutcome::Success { .. } => SettlementOutcome::ScalarSuccess,
            };
            assert!(
                first
                    .certificate()
                    .checkpoints()
                    .iter()
                    .any(|checkpoint| checkpoint.normal_outcome() == Some(expected)),
                "missing accepted outcome for {}",
                case.scenario_id
            );

            if functions.insert(case.function_id) {
                for checkpoint in first.certificate().checkpoints() {
                    for reason in aborts {
                        let nonce = NonZeroU64::new(invocation).unwrap();
                        invocation += 1;
                        let mut frame = first
                            .certificate()
                            .prepare_frame(nonce, checkpoint.checkpoint())
                            .unwrap();
                        let decision = SettlementDecision::Abort(reason);
                        let application = first.certificate().settle(&mut frame, decision).unwrap();
                        first
                            .certificate()
                            .validate_receipt(nonce, application.receipt())
                            .unwrap();
                        let receipt = application.receipt().clone();
                        let replay = first.certificate().settle(&mut frame, decision).unwrap();
                        assert_eq!(replay.receipt(), &receipt);
                        assert!(replay.performed_actions().is_empty());
                        assert_eq!(
                            first.certificate().settle(
                                &mut frame,
                                SettlementDecision::Abort(AdapterAbortReason::MalformedResponse)
                            ),
                            if reason == AdapterAbortReason::MalformedResponse {
                                Ok(replay)
                            } else {
                                Err(SettlementError::ConflictingTerminalDecision)
                            }
                        );
                    }
                    if let Some(outcome) = checkpoint.normal_outcome() {
                        let nonce = NonZeroU64::new(invocation).unwrap();
                        invocation += 1;
                        let mut frame = first
                            .certificate()
                            .prepare_frame(nonce, checkpoint.checkpoint())
                            .unwrap();
                        let accepted = first
                            .certificate()
                            .settle(&mut frame, SettlementDecision::Accept(outcome))
                            .unwrap();
                        first
                            .certificate()
                            .validate_receipt(nonce, accepted.receipt())
                            .unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn corpus_progress_paths_pin_stage_finalize_and_terminal_order() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let derive = |id: &str| {
            derive_native_settlement(&corpus.program, &DeclarationId::new(id))
                .unwrap_or_else(|error| panic!("{id}: {error:?}"))
        };
        assert_eq!(
            outcome_path(
                &derive("token.discard-two"),
                SettlementOutcome::ScalarSuccess
            ),
            "LL-F1->LD-F0->DD-Cscalar->DD"
        );
        assert_eq!(
            outcome_path(
                &derive("token.identity"),
                SettlementOutcome::OwnedSuccess { owner_ordinal: 0 }
            ),
            "L-S0->P-Cowned0->P"
        );
        assert_eq!(
            outcome_path(
                &derive("token.choose-second"),
                SettlementOutcome::OwnedSuccess { owner_ordinal: 1 }
            ),
            "LL-S1->LP-F0->DP-Cowned1->DP"
        );
        assert_eq!(
            outcome_path(
                &derive("token.ensures-false"),
                SettlementOutcome::SemanticFailure
            ),
            "L-S0->P-F0->D-Cfailure->D"
        );
    }

    #[test]
    fn every_corpus_function_pins_its_exact_progress_graph_fingerprint() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let actual = [
            "token.discard",
            "token.discard-two",
            "token.requires",
            "token.checked",
            "token.identity",
            "token.choose-second",
            "token.ensures-false",
        ]
        .map(|function| {
            let derivation =
                derive_native_settlement(&corpus.program, &DeclarationId::new(function)).unwrap();
            (function, hex(&derivation.certificate().fingerprint()))
        });
        let expected = [
            (
                "token.discard",
                "a13af979d596bb324238d8154771c6a21f1dee40c224b6378e4f2bdfc28d5c05",
            ),
            (
                "token.discard-two",
                "8233e92bfc13ec64025617d34a4b591d9fa9b195cf862ceca545366493adc9cf",
            ),
            (
                "token.requires",
                "8b291e6eb8f3f1e0267e0ea922748b5ba65006a2e76d898817877d760f989920",
            ),
            (
                "token.checked",
                "f6a35494ea8404bf9899aa70ef861048724e187d5a096b041fbcaafa40f6c181",
            ),
            (
                "token.identity",
                "bea63e0b491482c21bfee51e13dbe7d223865930b6be198ee3d9f2afcfdbb764",
            ),
            (
                "token.choose-second",
                "71c3b9a20299bb7783e2fc225818f428fb8854d080131ce38ee3b16e7c5bc50b",
            ),
            (
                "token.ensures-false",
                "3ec777bf926dcfaa9348382119c331fda997bd42480823f1ceb585d0499da759",
            ),
        ];
        assert_eq!(
            actual.map(|(_, fingerprint)| fingerprint),
            expected.map(|(_, fingerprint)| fingerprint.to_owned())
        );
    }

    #[test]
    fn recovery_contract_binds_canonical_semantic_module_identity() {
        let first = build_owned_resource_corpus_v1().unwrap().program;
        let renamed_source = OWNED_RESOURCE_CORPUS_SOURCE_V1.replacen(
            "module test.owned_resource_corpus;",
            "module test.owned_resource_corpus_renamed;",
            1,
        );
        let parsed = crate::parse(
            &renamed_source,
            Path::new("owned-resource-corpus-module-rename.spx"),
        )
        .unwrap();
        let renamed = crate::hir::resolve(&parsed).unwrap();
        crate::hir::validate(&first).unwrap();
        crate::hir::validate(&renamed).unwrap();

        let function = DeclarationId::new("token.discard-two");
        let first = derive_native_settlement(&first, &function).unwrap();
        let renamed = derive_native_settlement(&renamed, &function).unwrap();
        assert_ne!(
            first.recovery_contract_fingerprint(),
            renamed.recovery_contract_fingerprint()
        );
        assert_ne!(
            first.certificate().fingerprint(),
            renamed.certificate().fingerprint()
        );
    }

    #[test]
    fn start_only_progress_walk_rejects_skip_duplicate_and_wrong_action_without_mutation() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let derivation =
            derive_native_settlement(&corpus.program, &DeclarationId::new("token.discard-two"))
                .unwrap();
        let certificate = derivation.certificate();
        assert_eq!(certificate.start_checkpoints(), [1]);
        let mut frame = certificate
            .prepare_start_frame(NonZeroU64::new(91).unwrap())
            .unwrap();
        let before = (frame.checkpoint(), frame.resources().to_vec());
        assert_eq!(
            certificate.advance_frame(
                &mut frame,
                SettlementProgressAction::Finalize { owner_ordinal: 0 }
            ),
            Err(SettlementError::ProgressActionNotAdmitted)
        );
        assert_eq!((frame.checkpoint(), frame.resources().to_vec()), before);

        certificate
            .advance_frame(
                &mut frame,
                SettlementProgressAction::Finalize { owner_ordinal: 1 },
            )
            .unwrap();
        let after_first = (frame.checkpoint(), frame.resources().to_vec());
        assert_eq!(
            certificate.advance_frame(
                &mut frame,
                SettlementProgressAction::Finalize { owner_ordinal: 1 }
            ),
            Err(SettlementError::ProgressActionNotAdmitted)
        );
        assert_eq!(
            (frame.checkpoint(), frame.resources().to_vec()),
            after_first
        );
        certificate
            .advance_frame(
                &mut frame,
                SettlementProgressAction::Finalize { owner_ordinal: 0 },
            )
            .unwrap();
        let certify = certificate
            .progress_edges()
            .iter()
            .find(|edge| edge.from() == frame.checkpoint())
            .unwrap()
            .action();
        certificate.advance_frame(&mut frame, certify).unwrap();
        assert_eq!(
            certificate
                .checkpoints()
                .iter()
                .find(|checkpoint| checkpoint.checkpoint() == frame.checkpoint())
                .unwrap()
                .normal_outcome(),
            Some(SettlementOutcome::ScalarSuccess)
        );
    }

    #[test]
    fn recovery_contract_and_certificate_have_exact_known_answers() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let function = corpus
            .program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "token.discard-two")
            .unwrap();
        let derivation = derive_native_settlement(&corpus.program, &function.id).unwrap();
        assert_eq!(
            hex(&derivation.recovery_contract_fingerprint()),
            "0637eea2c0daecf7c1fcf84ac5c9e80c04a7dfd98b50d20a4a53d6909195da80"
        );
        assert_eq!(derivation.certificate().canonical_json(), "{\"schema\":\"semaprax.native-settlement-certificate.v2\",\"function\":\"token.discard-two\",\"recovery_contract\":\"0637eea2c0daecf7c1fcf84ac5c9e80c04a7dfd98b50d20a4a53d6909195da80\",\"resource_count\":2,\"checkpoints\":[{\"checkpoint\":1,\"resources\":[\"live\",\"live\"],\"normal_outcome\":null,\"abort_cleanup_order\":[1,0],\"accept_cleanup_order\":[]},{\"checkpoint\":2,\"resources\":[\"live\",\"dead\"],\"normal_outcome\":null,\"abort_cleanup_order\":[0],\"accept_cleanup_order\":[]},{\"checkpoint\":3,\"resources\":[\"dead\",\"dead\"],\"normal_outcome\":null,\"abort_cleanup_order\":[],\"accept_cleanup_order\":[]},{\"checkpoint\":4,\"resources\":[\"dead\",\"dead\"],\"normal_outcome\":{\"kind\":\"scalar_success\"},\"abort_cleanup_order\":[],\"accept_cleanup_order\":[]}],\"start_checkpoints\":[1],\"progress_edges\":[{\"from\":1,\"to\":2,\"action\":{\"kind\":\"finalize\",\"owner_ordinal\":1}},{\"from\":2,\"to\":3,\"action\":{\"kind\":\"finalize\",\"owner_ordinal\":0}},{\"from\":3,\"to\":4,\"action\":{\"kind\":\"certify_outcome\",\"trace_evidence\":\"cc43560bb15664722fb9432ef6a1fa9fe1d67d4774bc3d514624f8021f25e26e\"}}]}");
        assert_eq!(
            hex(&derivation.certificate().fingerprint()),
            "8233e92bfc13ec64025617d34a4b591d9fa9b195cf862ceca545366493adc9cf"
        );

        let terminal = derivation
            .certificate()
            .checkpoints()
            .iter()
            .find(|checkpoint| {
                checkpoint.normal_outcome() == Some(SettlementOutcome::ScalarSuccess)
            })
            .unwrap();
        let invocation = NonZeroU64::new(19).unwrap();
        let mut frame = derivation
            .certificate()
            .prepare_frame(invocation, terminal.checkpoint())
            .unwrap();
        let receipt = derivation
            .certificate()
            .settle(
                &mut frame,
                SettlementDecision::Accept(SettlementOutcome::ScalarSuccess),
            )
            .unwrap();
        assert_eq!(receipt.receipt().canonical_json(), "{\"schema\":\"semaprax.native-settlement-receipt.v2\",\"function\":\"token.discard-two\",\"recovery_contract\":\"0637eea2c0daecf7c1fcf84ac5c9e80c04a7dfd98b50d20a4a53d6909195da80\",\"certificate_fingerprint\":\"8233e92bfc13ec64025617d34a4b591d9fa9b195cf862ceca545366493adc9cf\",\"invocation\":19,\"checkpoint\":4,\"decision\":{\"kind\":\"accept\",\"outcome\":{\"kind\":\"scalar_success\"}},\"actions\":[],\"dispositions\":[\"dead\",\"dead\"],\"active_finalizers\":0}");
        assert_eq!(
            hex(&receipt.receipt().fingerprint()),
            "dbee8bddf9ef76bc8cfcd0f06f98f7964bf8db6888c83c4c0ac1fd50752c9238"
        );
    }

    #[test]
    fn exact_member_and_hostile_cleanup_plans_fail_before_derivation() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let error =
            derive_native_settlement(&corpus.program, &DeclarationId::new("token.not-in-program"))
                .unwrap_err();
        assert_eq!(error.code, "SPX-I104");

        let mut reordered = corpus.program.clone();
        let function = reordered
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "token.discard-two")
            .unwrap();
        let exit = function
            .cleanup_plan
            .exits
            .iter_mut()
            .find(|exit| exit.finalize_in_order.len() == 2)
            .unwrap();
        exit.finalize_in_order.swap(0, 1);
        let function = reordered
            .functions
            .iter()
            .find(|function| function.id.as_str() == "token.discard-two")
            .unwrap();
        assert_eq!(
            derive_native_settlement(&reordered, &function.id)
                .unwrap_err()
                .code,
            "SPX-H006"
        );

        let mut duplicate = corpus.program.clone();
        let function = duplicate
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "token.discard-two")
            .unwrap();
        let exit = function
            .cleanup_plan
            .exits
            .iter_mut()
            .find(|exit| exit.finalize_in_order.len() == 2)
            .unwrap();
        exit.finalize_in_order[1] = exit.finalize_in_order[0].clone();
        let function = duplicate
            .functions
            .iter()
            .find(|function| function.id.as_str() == "token.discard-two")
            .unwrap();
        assert_eq!(
            derive_native_settlement(&duplicate, &function.id)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        })
    }

    fn outcome_path(derivation: &NativeSettlementDerivation, outcome: SettlementOutcome) -> String {
        let certificate = derivation.certificate();
        let terminal = certificate
            .checkpoints()
            .iter()
            .find(|checkpoint| checkpoint.normal_outcome() == Some(outcome))
            .unwrap()
            .checkpoint();
        let mut reverse = Vec::new();
        let mut current = terminal;
        while current != 1 {
            let edge = certificate
                .progress_edges()
                .iter()
                .find(|edge| edge.to() == current)
                .unwrap();
            reverse.push(*edge);
            current = edge.from();
        }
        reverse.reverse();
        let mut projection = state_letters(certificate.checkpoints()[0].resources());
        for edge in reverse {
            let action = match edge.action() {
                SettlementProgressAction::Finalize { owner_ordinal } => {
                    format!("F{owner_ordinal}")
                }
                SettlementProgressAction::StageOwnedResult { owner_ordinal } => {
                    format!("S{owner_ordinal}")
                }
                SettlementProgressAction::CertifyOutcome { .. } => match outcome {
                    SettlementOutcome::ScalarSuccess => "Cscalar".to_owned(),
                    SettlementOutcome::SemanticFailure => "Cfailure".to_owned(),
                    SettlementOutcome::OwnedSuccess { owner_ordinal } => {
                        format!("Cowned{owner_ordinal}")
                    }
                },
            };
            let states = certificate.checkpoints()[(edge.to() - 1) as usize].resources();
            projection.push_str(&format!("-{action}->{}", state_letters(states)));
        }
        projection
    }

    fn state_letters(states: &[SettlementResourceState]) -> String {
        states
            .iter()
            .map(|state| match state {
                SettlementResourceState::Live => 'L',
                SettlementResourceState::ProvisionalResult => 'P',
                SettlementResourceState::Dead => 'D',
                SettlementResourceState::Finalizing => 'F',
                SettlementResourceState::Published => 'U',
            })
            .collect()
    }
}
