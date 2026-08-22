//! Compiler-owned, target-neutral cleanup trace-path certificates.
//!
//! The semantic event dictionary is only a vocabulary.  This module compiles
//! the independently replay-validated cleanup CFG into a deterministic trie
//! DFA whose accepting states bind both the exact ordinal sequence and its
//! terminal outcome.  Native hosts authenticate the certificate separately
//! and can then validate a response without allocating or reconstructing
//! cleanup semantics.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{
    BlockId, CleanupPlace, CleanupResultSource, CleanupTerminator, CleanupTransition, EdgeId,
    ExitContinuation, StatusSourceId, StorageId,
};
use crate::conformance::TraceEventKind;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedType};
use crate::semantic_trace::SemanticEventDictionary;

pub const TRACE_PATH_CERTIFICATE_V1: &str = "semaprax.trace-path-certificate.v1";

const FINGERPRINT_DOMAIN: &[u8] = b"semaprax.trace-path-certificate-fingerprint.v1\0";
const MAX_PATHS: usize = 65_536;
const MAX_WORK_UNITS: usize = 1_000_000;

/// Outcome class authenticated by an accepting DFA state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TracePathOutcome {
    ScalarSuccess,
    OwnedSuccess,
    Failure { selected_ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DfaState {
    transitions: Vec<(u32, u32)>,
    accepts: Vec<TracePathOutcome>,
}

/// Canonical compiler certificate for all valid paths of one cleanup CFG.
///
/// Fields are private so downstream hosts can receive compiler evidence but
/// cannot manufacture a different accepted language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TracePathCertificate {
    schema: &'static str,
    function: DeclarationId,
    dictionary_fingerprint: [u8; 32],
    max_path_events: u32,
    states: Vec<DfaState>,
}

impl TracePathCertificate {
    #[must_use]
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    #[must_use]
    pub fn function(&self) -> &DeclarationId {
        &self.function
    }

    #[must_use]
    pub fn dictionary_fingerprint(&self) -> [u8; 32] {
        self.dictionary_fingerprint
    }

    #[must_use]
    pub fn max_path_events(&self) -> u32 {
        self.max_path_events
    }

    #[must_use]
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.states
            .iter()
            .map(|state| state.transitions.len())
            .sum()
    }

    /// Validate an already bounded ordinal slice by walking the admitted DFA.
    /// This operation performs no allocation.
    #[must_use]
    pub fn accepts(&self, ordinals: &[u32], outcome: TracePathOutcome) -> bool {
        if ordinals.len() > self.max_path_events as usize {
            return false;
        }
        let mut state = 0_usize;
        for ordinal in ordinals {
            let Some(current) = self.states.get(state) else {
                return false;
            };
            let Ok(position) = current
                .transitions
                .binary_search_by_key(ordinal, |(candidate, _)| *candidate)
            else {
                return false;
            };
            let Ok(next) = usize::try_from(current.transitions[position].1) else {
                return false;
            };
            state = next;
        }
        self.states
            .get(state)
            .is_some_and(|state| state.accepts.binary_search(&outcome).is_ok())
    }

    /// Canonical bytes committed by [`Self::fingerprint`].
    #[must_use]
    pub fn canonical_json(&self) -> String {
        let states = self
            .states
            .iter()
            .map(|state| {
                let transitions = state
                    .transitions
                    .iter()
                    .map(|(ordinal, next)| format!("[{},{}]", ordinal, next))
                    .collect::<Vec<_>>()
                    .join(",");
                let accepts = state
                    .accepts
                    .iter()
                    .map(outcome_json)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{\"transitions\":[{transitions}],\"accepts\":[{accepts}]}}")
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"function\":{},\"dictionary_fingerprint\":\"{}\",\"start_state\":0,\"max_path_events\":{},\"states\":[{}]}}",
            quote_json(self.schema),
            quote_json(self.function.as_str()),
            hex(&self.dictionary_fingerprint),
            self.max_path_events,
            states,
        )
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let canonical = self.canonical_json();
        let mut hasher = Sha256::new();
        hasher.update(FINGERPRINT_DOMAIN);
        hasher.update((canonical.len() as u64).to_le_bytes());
        hasher.update(canonical.as_bytes());
        hasher.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AcceptedPath {
    ordinals: Vec<u32>,
    outcome: TracePathOutcome,
}

#[derive(Clone)]
struct Leaf {
    place: CleanupPlace,
}

#[derive(Clone)]
struct PathState {
    block: BlockId,
    live: BTreeSet<LivenessFlagId>,
    selected: Option<StatusSourceId>,
    ordinals: Vec<u32>,
    visited: BTreeSet<BlockId>,
}

/// Compile the exact validated cleanup CFG into a canonical deterministic DFA.
#[doc(hidden)]
pub fn build_trace_path_certificate(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    dictionary: &SemanticEventDictionary,
) -> Result<TracePathCertificate, Diagnostic> {
    crate::hir::validate(program)?;
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(certificate_error(
            "trace-path certificates do not admit generic function templates or instances",
        ));
    }
    if !program
        .functions
        .iter()
        .any(|candidate| candidate == function)
    {
        return Err(certificate_error(
            "cleanup function is not the exact validated program member",
        ));
    }
    if dictionary.function() != &function.id {
        return Err(certificate_error(
            "cleanup plan and semantic dictionary belong to different functions",
        ));
    }
    let leaves = collect_plan_leaves(function)?;
    let mut live = BTreeSet::new();
    for place in &function.cleanup_plan.entry_state.live_owned_parameters {
        live.extend(flags_under(&leaves, place)?);
    }
    let mut pending = vec![PathState {
        block: function.cleanup_plan.entry,
        live,
        selected: None,
        ordinals: Vec::new(),
        visited: BTreeSet::new(),
    }];
    let mut accepted = BTreeSet::new();
    let mut work = 0_usize;

    while let Some(mut state) = pending.pop() {
        work = work
            .checked_add(1)
            .ok_or_else(|| certificate_error("trace-path work counter overflow"))?;
        if work > MAX_WORK_UNITS {
            return Err(certificate_error("trace-path work budget exhausted"));
        }
        if !state.visited.insert(state.block) {
            return Err(certificate_error("cleanup path revisits a block"));
        }
        let block = function
            .cleanup_plan
            .blocks
            .iter()
            .find(|block| block.id == state.block)
            .ok_or_else(|| certificate_error("cleanup path references a missing block"))?;
        for transition in &block.transitions {
            apply_transition(&leaves, dictionary, &mut state, transition)?;
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
                    .ok_or_else(|| certificate_error("cleanup path references a missing exit"))?;
                for action in &exit.finalize_in_order {
                    if state.live.remove(&action.guard_flag) {
                        state
                            .ordinals
                            .push(finalizer_ordinal(dictionary, action, true)?);
                        state
                            .ordinals
                            .push(finalizer_ordinal(dictionary, action, false)?);
                    }
                }
                match &exit.continuation {
                    ExitContinuation::Continue(edge) => {
                        state.block = edge_target(function, state.block, *edge)?;
                        pending.push(state);
                    }
                    ExitContinuation::CommitResult { source } => {
                        if state.selected.is_some() {
                            return Err(certificate_error(
                                "cleanup path publishes a result after failure selection",
                            ));
                        }
                        validate_result_liveness(&leaves, &mut state.live, source)?;
                        state.ordinals.push(event_ordinal(
                            dictionary,
                            &TraceEventKind::ResultCommit {
                                source: source.clone(),
                            },
                        )?);
                        let outcome = match function.return_type {
                            ResolvedType::Unit => TracePathOutcome::ScalarSuccess,
                            ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::Char
                            | ResolvedType::F32
                            | ResolvedType::F64
                            | ResolvedType::Bool => TracePathOutcome::ScalarSuccess,
                            ResolvedType::Nominal { .. } => TracePathOutcome::OwnedSuccess,
                            ResolvedType::TypeParameter { .. } => {
                                return Err(certificate_error(
                                    "type-parameter result is outside callable v2",
                                ))
                            }
                        };
                        insert_path(&mut accepted, state.ordinals, outcome)?;
                    }
                    ExitContinuation::ReturnFailure { source } => {
                        if state.selected.as_ref() != Some(source) || !state.live.is_empty() {
                            return Err(certificate_error(
                                "failure terminal disagrees with selected status or liveness",
                            ));
                        }
                        let selected_ordinal = select_failure_ordinal(dictionary, source)?;
                        insert_path(
                            &mut accepted,
                            state.ordinals,
                            TracePathOutcome::Failure { selected_ordinal },
                        )?;
                    }
                    ExitContinuation::ReturnUnit => {
                        return Err(certificate_error(
                            "unit return is outside callable descriptor v2",
                        ))
                    }
                }
            }
        }
        if pending.len().saturating_add(accepted.len()) > MAX_PATHS {
            return Err(certificate_error(
                "trace-path count exceeds the audited limit",
            ));
        }
    }
    if accepted.is_empty() {
        return Err(certificate_error("cleanup CFG has no accepting trace path"));
    }
    build_dfa(function.id.clone(), dictionary.fingerprint(), accepted)
}

fn apply_transition(
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
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
            let source_flags = flags_under(leaves, source)?;
            let destination_flags = flags_under(leaves, destination)?;
            if source_flags.len() != destination_flags.len()
                || source_flags.iter().any(|flag| !state.live.contains(flag))
                || destination_flags
                    .iter()
                    .any(|flag| state.live.contains(flag))
            {
                return Err(certificate_error("transfer has invalid path liveness"));
            }
            state.live.retain(|flag| !source_flags.contains(flag));
            state.live.extend(destination_flags);
            state.ordinals.push(event_ordinal(
                dictionary,
                &TraceEventKind::Transfer {
                    at: at.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                },
            )?);
        }
        CleanupTransition::SelectFailure { source } => {
            if state.selected.replace(source.clone()).is_some() {
                return Err(certificate_error("failure selection is not write-once"));
            }
            state
                .ordinals
                .push(select_failure_ordinal(dictionary, source)?);
        }
        CleanupTransition::Initialize { .. }
        | CleanupTransition::CallCommit { .. }
        | CleanupTransition::StageCopyResult { .. } => {
            return Err(certificate_error(
                "transition is outside the callable-v2 single-frame slice",
            ))
        }
    }
    Ok(())
}

fn validate_result_liveness(
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
    live: &mut BTreeSet<LivenessFlagId>,
    source: &CleanupResultSource,
) -> Result<(), Diagnostic> {
    match source {
        CleanupResultSource::Scalar { .. } => {
            if !live.is_empty() {
                return Err(certificate_error(
                    "scalar result precedes non-result cleanup",
                ));
            }
        }
        CleanupResultSource::Owned { storage } => {
            let result = flags_under(leaves, storage)?;
            if live != &result {
                return Err(certificate_error(
                    "owned result precedes non-result cleanup",
                ));
            }
            live.clear();
        }
    }
    Ok(())
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
        .ok_or_else(|| certificate_error("cleanup path references a missing edge"))
}

fn event_ordinal(
    dictionary: &SemanticEventDictionary,
    event: &TraceEventKind,
) -> Result<u32, Diagnostic> {
    dictionary
        .ordinal_for(event)
        .ok_or_else(|| certificate_error("cleanup event is absent from its semantic dictionary"))
}

fn select_failure_ordinal(
    dictionary: &SemanticEventDictionary,
    source: &StatusSourceId,
) -> Result<u32, Diagnostic> {
    let mut matches = dictionary.entries().iter().filter_map(|entry| {
        matches!(
            &entry.event,
            TraceEventKind::SelectFailure {
                source: candidate,
                ..
            } if candidate == source
        )
        .then_some(entry.ordinal)
    });
    let ordinal = matches
        .next()
        .ok_or_else(|| certificate_error("failure selection is absent from its dictionary"))?;
    if matches.next().is_some() {
        return Err(certificate_error(
            "failure selection is ambiguous in its dictionary",
        ));
    }
    Ok(ordinal)
}

fn finalizer_ordinal(
    dictionary: &SemanticEventDictionary,
    action: &crate::cleanup_plan::FinalizeAction,
    begin: bool,
) -> Result<u32, Diagnostic> {
    let mut matches = dictionary.entries().iter().filter_map(|entry| {
        let matches = match &entry.event {
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
        matches.then_some(entry.ordinal)
    });
    let ordinal = matches
        .next()
        .ok_or_else(|| certificate_error("finalizer event is absent from its dictionary"))?;
    if matches.next().is_some() {
        return Err(certificate_error(
            "finalizer event is ambiguous in its dictionary",
        ));
    }
    Ok(ordinal)
}

fn collect_plan_leaves(
    function: &ResolvedFunction,
) -> Result<BTreeMap<LivenessFlagId, Leaf>, Diagnostic> {
    let mut leaves = BTreeMap::new();
    for slot in &function.cleanup_plan.slots {
        collect_leaves(
            &slot.storage,
            &mut Vec::new(),
            &slot.field_liveness_shape,
            &mut leaves,
        )?;
    }
    Ok(leaves)
}

fn collect_leaves(
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { flag, .. } => {
            if leaves
                .insert(
                    *flag,
                    Leaf {
                        place: CleanupPlace {
                            storage: storage.clone(),
                            projections: projections.clone(),
                        },
                    },
                )
                .is_some()
            {
                return Err(certificate_error("cleanup flag is declared more than once"));
            }
        }
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                collect_leaves(storage, projections, &field.shape, leaves)?;
                projections.pop();
            }
        }
    }
    Ok(())
}

fn flags_under(
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
    place: &CleanupPlace,
) -> Result<BTreeSet<LivenessFlagId>, Diagnostic> {
    let flags = leaves
        .iter()
        .filter_map(|(flag, leaf)| {
            (leaf.place.storage == place.storage
                && leaf.place.projections.starts_with(&place.projections))
            .then_some(*flag)
        })
        .collect::<BTreeSet<_>>();
    if flags.is_empty() {
        return Err(certificate_error("cleanup place has no liveness flags"));
    }
    Ok(flags)
}

fn insert_path(
    accepted: &mut BTreeSet<AcceptedPath>,
    ordinals: Vec<u32>,
    outcome: TracePathOutcome,
) -> Result<(), Diagnostic> {
    if ordinals.is_empty() {
        return Err(certificate_error("accepted trace path is empty"));
    }
    accepted.insert(AcceptedPath { ordinals, outcome });
    Ok(())
}

fn build_dfa(
    function: DeclarationId,
    dictionary_fingerprint: [u8; 32],
    accepted: BTreeSet<AcceptedPath>,
) -> Result<TracePathCertificate, Diagnostic> {
    let max_path_events = accepted
        .iter()
        .map(|path| path.ordinals.len())
        .max()
        .ok_or_else(|| certificate_error("trace-path set is empty"))?;
    let max_path_events = u32::try_from(max_path_events)
        .map_err(|_| certificate_error("trace path exceeds u32 event capacity"))?;
    let mut transitions = vec![BTreeMap::<u32, u32>::new()];
    let mut accepts = vec![BTreeSet::<TracePathOutcome>::new()];
    for path in accepted {
        let mut state = 0_usize;
        for ordinal in path.ordinals {
            let next = if let Some(next) = transitions[state].get(&ordinal) {
                *next
            } else {
                let next = u32::try_from(transitions.len())
                    .map_err(|_| certificate_error("trace DFA state space exceeds u32"))?;
                transitions[state].insert(ordinal, next);
                transitions.push(BTreeMap::new());
                accepts.push(BTreeSet::new());
                next
            };
            state = usize::try_from(next)
                .map_err(|_| certificate_error("trace DFA state index does not fit usize"))?;
        }
        accepts[state].insert(path.outcome);
    }
    let states = transitions
        .into_iter()
        .zip(accepts)
        .map(|(transitions, accepts)| DfaState {
            transitions: transitions.into_iter().collect(),
            accepts: accepts.into_iter().collect(),
        })
        .collect();
    Ok(TracePathCertificate {
        schema: TRACE_PATH_CERTIFICATE_V1,
        function,
        dictionary_fingerprint,
        max_path_events,
        states,
    })
}

fn outcome_json(outcome: &TracePathOutcome) -> String {
    match outcome {
        TracePathOutcome::ScalarSuccess => "{\"kind\":\"scalar_success\"}".to_owned(),
        TracePathOutcome::OwnedSuccess => "{\"kind\":\"owned_success\"}".to_owned(),
        TracePathOutcome::Failure { selected_ordinal } => {
            format!("{{\"kind\":\"failure\",\"selected_ordinal\":{selected_ordinal}}}")
        }
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a string cannot fail");
    }
    output
}

fn certificate_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native callable trace-path certificate: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::conformance::{TraceEventKind, TraceOutcome};
    use crate::owned_resource_corpus::build_owned_resource_corpus_v1;
    use crate::semantic_trace::{
        build_semantic_event_dictionary, OWNED_RESOURCE_CORPUS_V1_SCENARIOS,
    };

    use super::*;

    #[test]
    fn all_fourteen_authoritative_paths_are_accepted_and_deterministic() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        assert_eq!(
            corpus
                .cases
                .iter()
                .map(|case| case.scenario_id)
                .collect::<Vec<_>>(),
            OWNED_RESOURCE_CORPUS_V1_SCENARIOS
        );
        let mut evidence = BTreeMap::new();
        for case in &corpus.cases {
            let function = corpus
                .program
                .functions
                .iter()
                .find(|function| function.id.as_str() == case.function_id)
                .unwrap();
            let dictionary =
                build_semantic_event_dictionary(&corpus.program, &function.id).unwrap();
            let first =
                build_trace_path_certificate(&corpus.program, function, &dictionary).unwrap();
            let second =
                build_trace_path_certificate(&corpus.program, function, &dictionary).unwrap();
            assert_eq!(first, second);
            assert_eq!(first.fingerprint(), second.fingerprint());
            assert_eq!(first.dictionary_fingerprint(), dictionary.fingerprint());
            let ordinals = case
                .reference
                .events
                .iter()
                .map(|event| dictionary.ordinal_for(&event.event).unwrap())
                .collect::<Vec<_>>();
            let outcome = trace_outcome(&case.reference.outcome, &dictionary);
            assert!(
                first.accepts(&ordinals, outcome),
                "certificate rejected {}",
                case.scenario_id
            );
            evidence.insert(case.scenario_id, (first, dictionary, ordinals, outcome));
        }

        let (discard, dictionary, ordinals, outcome) = &evidence["discard-zero"];
        assert!(matches!(
            dictionary.entries()[0].event,
            TraceEventKind::FinalizeBegin { .. }
        ));
        let mut omitted_finalizer_end = ordinals.clone();
        omitted_finalizer_end.remove(1);
        assert!(!discard.accepts(&omitted_finalizer_end, *outcome));
        let mut duplicate_pair = ordinals.clone();
        duplicate_pair.splice(2..2, ordinals[..2].iter().copied());
        assert!(!discard.accepts(&duplicate_pair, *outcome));

        let (requires, _, failure, failure_outcome) = &evidence["requires-false"];
        let mut selection_after_cleanup = failure.clone();
        let selection = selection_after_cleanup.remove(0);
        selection_after_cleanup.push(selection);
        assert!(!requires.accepts(&selection_after_cleanup, *failure_outcome));

        let (choose, choose_dictionary, choose_ordinals, choose_outcome) =
            &evidence["choose-second-zero-max"];
        let transfer_positions = choose_ordinals
            .iter()
            .enumerate()
            .filter(|(_, ordinal)| {
                matches!(
                    choose_dictionary.entries()[(**ordinal - 1) as usize].event,
                    TraceEventKind::Transfer { .. }
                )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(transfer_positions.len(), 2);
        let finalizer_positions = choose_ordinals
            .iter()
            .enumerate()
            .filter(|(_, ordinal)| {
                matches!(
                    choose_dictionary.entries()[(**ordinal - 1) as usize].event,
                    TraceEventKind::FinalizeBegin { .. } | TraceEventKind::FinalizeEnd { .. }
                )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(finalizer_positions.len(), 2);

        let mut missing_non_result_cleanup = choose_ordinals.clone();
        for index in finalizer_positions.iter().rev() {
            missing_non_result_cleanup.remove(*index);
        }
        assert!(!choose.accepts(&missing_non_result_cleanup, *choose_outcome));

        let mut interleaved_transfers = choose_ordinals.clone();
        let pair = [
            interleaved_transfers.remove(finalizer_positions[1]),
            interleaved_transfers.remove(finalizer_positions[0]),
        ];
        interleaved_transfers.splice(
            transfer_positions[1]..transfer_positions[1],
            pair.into_iter().rev(),
        );
        assert!(!choose.accepts(&interleaved_transfers, *choose_outcome));
    }

    fn trace_outcome(
        outcome: &TraceOutcome,
        dictionary: &SemanticEventDictionary,
    ) -> TracePathOutcome {
        match outcome {
            TraceOutcome::Success { result } => match result {
                crate::conformance::TraceResult::I64(_)
                | crate::conformance::TraceResult::Int32(_)
                | crate::conformance::TraceResult::Char(_)
                | crate::conformance::TraceResult::F32(_)
                | crate::conformance::TraceResult::F64(_)
                | crate::conformance::TraceResult::Bool(_) => TracePathOutcome::ScalarSuccess,
                crate::conformance::TraceResult::Owned { .. } => TracePathOutcome::OwnedSuccess,
                crate::conformance::TraceResult::Unit => panic!("unit is outside callable v2"),
            },
            TraceOutcome::Failure {
                selected_source, ..
            } => TracePathOutcome::Failure {
                selected_ordinal: select_failure_ordinal(dictionary, selected_source).unwrap(),
            },
        }
    }
}
