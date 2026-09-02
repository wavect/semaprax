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
    trace_certificate_fingerprint: [u8; 32],
    certificate: NativeSettlementCertificate,
    trace_evidence_witnesses: BTreeMap<[u8; 32], TraceEvidenceWitness>,
}

impl NativeSettlementDerivation {
    pub(super) const fn recovery_contract_fingerprint(&self) -> [u8; 32] {
        self.recovery_contract_fingerprint
    }

    pub(super) const fn trace_certificate_fingerprint(&self) -> [u8; 32] {
        self.trace_certificate_fingerprint
    }

    pub(super) fn certificate(&self) -> &NativeSettlementCertificate {
        &self.certificate
    }

    pub(super) fn trace_evidence_witness(
        &self,
        fingerprint: &[u8; 32],
    ) -> Option<&TraceEvidenceWitness> {
        self.trace_evidence_witnesses.get(fingerprint)
    }
}

/// Canonical witness whose digest is carried by one `CertifyOutcome` edge.
/// This remains compiler-private proof data and confers no runtime authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TraceEvidenceWitness {
    ordinals: Vec<u32>,
    outcome: TracePathOutcome,
}

impl TraceEvidenceWitness {
    pub(super) fn ordinals(&self) -> &[u32] {
        &self.ordinals
    }

    pub(super) const fn outcome(&self) -> TracePathOutcome {
        self.outcome
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
    trace_ordinals: Vec<u32>,
    trace_outcome: TracePathOutcome,
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
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(derivation_error(
            "native settlement does not admit generic function templates or instances",
        ));
    }
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
    let mut trace_evidence_witnesses = BTreeMap::new();
    for terminal in &terminals {
        let witness = TraceEvidenceWitness {
            ordinals: terminal.trace_ordinals.clone(),
            outcome: terminal.trace_outcome,
        };
        match trace_evidence_witnesses.insert(terminal.trace_evidence, witness.clone()) {
            Some(existing) if existing != witness => {
                return Err(derivation_error(
                    "trace-evidence digest collision has inconsistent witnesses",
                ));
            }
            Some(_) | None => {}
        }
    }
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
        trace_certificate_fingerprint: trace_certificate.fingerprint(),
        certificate,
        trace_evidence_witnesses,
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
                            trace_ordinals: state.trace_ordinals,
                            trace_outcome,
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
                            trace_ordinals: state.trace_ordinals,
                            trace_outcome,
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
        CleanupTransition::Initialize { .. }
        | CleanupTransition::InitializeVariant { .. }
        | CleanupTransition::TransferVariant { .. }
        | CleanupTransition::AuthenticateVariantCase { .. }
        | CleanupTransition::CallCommit { .. }
        | CleanupTransition::StageCopyResult { .. } => {
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
        FieldLivenessShape::Variant { cases, .. } => {
            for case in cases {
                projections.push(case.case.clone());
                for field in &case.fields {
                    projections.push(field.field.clone());
                    collect_flags(storage, projections, &field.shape, place, flags);
                    projections.pop();
                }
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
#[path = "native_settlement_derivation/tests.rs"]
mod tests;
