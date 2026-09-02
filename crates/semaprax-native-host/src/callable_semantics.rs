//! Authenticated semantic-result gate for callable native responses.
//!
//! This layer authenticates the compiler-built semantic dictionary and
//! trace-path certificate, walks the certificate DFA before materialization,
//! and then checks publication-critical outcome relationships. The compiler's
//! independent cleanup-plan validation and replay remain responsible for
//! proving the certificate was derived from valid target-neutral semantics.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "callable semantic validation remains staged behind SPX-B104"
    )
)]

use semaprax::cleanup_plan::{CleanupResultSource, StatusSourceId, StorageId};
use std::num::NonZeroU64;
use std::sync::Arc;

use semaprax::conformance::{InvocationPath, NormalizedStatus, TraceEvent, TraceEventKind};
use semaprax::semantic_trace::{
    SemanticEventDictionary, SemanticEventEntry, SEMANTIC_EVENT_DICTIONARY_V1,
};
use semaprax::trace_path_certificate::{
    TracePathCertificate, TracePathOutcome, TRACE_PATH_CERTIFICATE_V1,
};

use crate::callable_wire::{
    decode_response_into, DecodedResponse, DecodedResponseHead, ResponseOutcome, WireError,
};
use crate::descriptor_v2::{Descriptor, Parameter, ResultShape};

pub(crate) struct AuthenticatedSemanticDictionary {
    dictionary: SemanticEventDictionary,
    trace_path_certificate: TracePathCertificate,
    event_templates: Vec<Arc<TraceEvent>>,
    failure_templates: Vec<Option<(Arc<StatusSourceId>, Arc<NormalizedStatus>)>>,
    allowed_result_commit_ordinals: Vec<u32>,
    required_owned_transfer_ordinals: Option<[u32; 2]>,
    max_event_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ValidatedOutcome {
    ScalarSuccess(i64),
    OwnedSuccess {
        owner_ordinal: usize,
    },
    Failure {
        selected_ordinal: u32,
        source: Arc<StatusSourceId>,
        status: Arc<NormalizedStatus>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedExecution {
    pub(crate) outcome: ValidatedOutcome,
    pub(crate) events: Vec<Arc<TraceEvent>>,
}

pub(crate) struct ResponseDecodeBuffers {
    semantic_ordinals: Vec<u32>,
    events: Vec<Arc<TraceEvent>>,
    max_event_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticValidationError {
    WrongDictionarySchema,
    WrongFunction,
    WrongDictionaryByteLength,
    WrongDictionaryEntryCount,
    WrongDictionaryFingerprint,
    WrongTracePathCertificateSchema,
    WrongTracePathCertificateFunction,
    WrongTracePathCertificateDictionary,
    WrongTracePathCertificateFingerprint,
    TracePathCertificateExceedsEventCapacity,
    TracePathRejected,
    NonCanonicalDictionary,
    ResultCommitDictionaryMismatch,
    OwnedResultTransferDictionaryMismatch,
    InsufficientSuccessEventCapacity,
    MaterializationFailed,
    FinalizePairMismatch,
    OrphanFinalizeEnd,
    SuccessContainsFailureSelection,
    SuccessResultCommitCount,
    SuccessResultCommitMismatch,
    SuccessResultCommitNotFinal,
    SuccessOwnedTransferMismatch,
    FailureSelectedOrdinalOccurrence,
    FailureSelectedOrdinalNotSelection,
    FailureSelectionCount,
    FailureSelectionMismatch,
    FailureContainsResultCommit,
    InsufficientEventCapacity,
    AllocationFailed,
}

pub(crate) fn authenticate_dictionary(
    descriptor: &Descriptor,
    dictionary: SemanticEventDictionary,
    trace_path_certificate: TracePathCertificate,
) -> Result<AuthenticatedSemanticDictionary, SemanticValidationError> {
    if dictionary.schema() != SEMANTIC_EVENT_DICTIONARY_V1 {
        return Err(SemanticValidationError::WrongDictionarySchema);
    }
    if dictionary.function().as_str() != descriptor.function {
        return Err(SemanticValidationError::WrongFunction);
    }
    if dictionary.entries().len() != descriptor.capacities.dictionary_entries as usize {
        return Err(SemanticValidationError::WrongDictionaryEntryCount);
    }
    let canonical = dictionary.canonical_json();
    if canonical.len() != descriptor.capacities.dictionary_bytes as usize {
        return Err(SemanticValidationError::WrongDictionaryByteLength);
    }
    if dictionary.fingerprint() != descriptor.fingerprints.event_dictionary {
        return Err(SemanticValidationError::WrongDictionaryFingerprint);
    }
    for (index, entry) in dictionary.entries().iter().enumerate() {
        let expected = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(SemanticValidationError::NonCanonicalDictionary)?;
        if entry.ordinal != expected {
            return Err(SemanticValidationError::NonCanonicalDictionary);
        }
    }
    if trace_path_certificate.schema() != TRACE_PATH_CERTIFICATE_V1 {
        return Err(SemanticValidationError::WrongTracePathCertificateSchema);
    }
    if trace_path_certificate.function().as_str() != descriptor.function {
        return Err(SemanticValidationError::WrongTracePathCertificateFunction);
    }
    if trace_path_certificate.dictionary_fingerprint() != dictionary.fingerprint() {
        return Err(SemanticValidationError::WrongTracePathCertificateDictionary);
    }
    if trace_path_certificate.fingerprint() != descriptor.fingerprints.trace_path_certificate {
        return Err(SemanticValidationError::WrongTracePathCertificateFingerprint);
    }
    let mandatory_success_events = match descriptor.result {
        ResultShape::ScalarI64 => 1,
        ResultShape::OwnedInput { .. } => 3,
    };
    if (descriptor.capacities.max_event_count as usize) < mandatory_success_events {
        return Err(SemanticValidationError::InsufficientSuccessEventCapacity);
    }
    if trace_path_certificate.max_path_events() == 0
        || trace_path_certificate.max_path_events() > descriptor.capacities.max_event_count
        || trace_path_certificate.state_count() == 0
    {
        return Err(SemanticValidationError::TracePathCertificateExceedsEventCapacity);
    }
    let (allowed_result_commit_ordinals, required_owned_transfer_ordinals) =
        authenticate_result_projection(descriptor, &dictionary)?;
    let mut event_templates = Vec::new();
    event_templates
        .try_reserve_exact(dictionary.entries().len())
        .map_err(|_| SemanticValidationError::AllocationFailed)?;
    let mut failure_templates = Vec::new();
    failure_templates
        .try_reserve_exact(dictionary.entries().len())
        .map_err(|_| SemanticValidationError::AllocationFailed)?;
    for entry in dictionary.entries() {
        event_templates.push(Arc::new(TraceEvent {
            function: dictionary.function().clone(),
            invocation: InvocationPath::default(),
            event: entry.event.clone(),
        }));
        failure_templates.push(match &entry.event {
            TraceEventKind::SelectFailure { source, status } => {
                Some((Arc::new(source.clone()), Arc::new(status.clone())))
            }
            _ => None,
        });
    }
    Ok(AuthenticatedSemanticDictionary {
        dictionary,
        trace_path_certificate,
        event_templates,
        failure_templates,
        allowed_result_commit_ordinals,
        required_owned_transfer_ordinals,
        max_event_count: descriptor.capacities.max_event_count as usize,
    })
}

fn authenticate_result_projection(
    descriptor: &Descriptor,
    dictionary: &SemanticEventDictionary,
) -> Result<(Vec<u32>, Option<[u32; 2]>), SemanticValidationError> {
    let mut commits = Vec::new();
    commits
        .try_reserve_exact(dictionary.entries().len())
        .map_err(|_| SemanticValidationError::AllocationFailed)?;
    let required_transfer = match descriptor.result {
        ResultShape::ScalarI64 => {
            for entry in dictionary.entries() {
                if matches!(
                    entry.event,
                    TraceEventKind::ResultCommit {
                        source: CleanupResultSource::Scalar { .. }
                    }
                ) {
                    commits.push(entry.ordinal);
                }
            }
            None
        }
        ResultShape::OwnedInput {
            parameter_index,
            owner_ordinal,
        } => {
            let Some(Parameter::Owned {
                index,
                value,
                owner_ordinal: parameter_owner,
                ..
            }) = descriptor.parameters.get(parameter_index)
            else {
                return Err(SemanticValidationError::OwnedResultTransferDictionaryMismatch);
            };
            if *index != parameter_index || *parameter_owner != owner_ordinal {
                return Err(SemanticValidationError::OwnedResultTransferDictionaryMismatch);
            }
            for entry in dictionary.entries() {
                if matches!(
                    &entry.event,
                    TraceEventKind::ResultCommit {
                        source: CleanupResultSource::Owned { storage }
                    } if storage.storage == StorageId::ProvisionalResult
                        && storage.projections.is_empty()
                ) {
                    commits.push(entry.ordinal);
                }
            }
            let mut chain = None;
            for stage in dictionary.entries() {
                let TraceEventKind::Transfer {
                    source,
                    destination,
                    ..
                } = &stage.event
                else {
                    continue;
                };
                let StorageId::Temporary(temporary) = &destination.storage else {
                    continue;
                };
                if !matches!(
                    &source.storage,
                    StorageId::Value(id) if id.as_str() == value
                ) || !source.projections.is_empty()
                    || !destination.projections.is_empty()
                {
                    continue;
                }
                let mut publications = dictionary.entries().iter().filter(|entry| {
                    matches!(
                        &entry.event,
                        TraceEventKind::Transfer {
                            source,
                            destination,
                            ..
                        } if source.storage == StorageId::Temporary(temporary.clone())
                            && source.projections.is_empty()
                            && destination.storage == StorageId::ProvisionalResult
                            && destination.projections.is_empty()
                    )
                });
                let Some(publication) = publications.next() else {
                    continue;
                };
                if publications.next().is_some() || chain.is_some() {
                    return Err(SemanticValidationError::OwnedResultTransferDictionaryMismatch);
                }
                chain = Some([stage.ordinal, publication.ordinal]);
            }
            let Some(chain) = chain else {
                return Err(SemanticValidationError::OwnedResultTransferDictionaryMismatch);
            };
            Some(chain)
        }
    };
    if commits.is_empty() {
        return Err(SemanticValidationError::ResultCommitDictionaryMismatch);
    }
    Ok((commits, required_transfer))
}

impl AuthenticatedSemanticDictionary {
    pub(crate) fn try_failure_status_templates(
        &self,
    ) -> Result<Vec<Option<NormalizedStatus>>, SemanticValidationError> {
        let mut statuses = Vec::new();
        statuses
            .try_reserve_exact(self.failure_templates.len())
            .map_err(|_| SemanticValidationError::AllocationFailed)?;
        statuses.extend(
            self.failure_templates
                .iter()
                .map(|template| template.as_ref().map(|(_, status)| status.as_ref().clone())),
        );
        Ok(statuses)
    }

    pub(crate) fn validate_response(
        &self,
        response: DecodedResponse,
    ) -> Result<ValidatedExecution, SemanticValidationError> {
        let mut buffers = ResponseDecodeBuffers::try_new_for_max(self.max_event_count)
            .map_err(|_| SemanticValidationError::AllocationFailed)?;
        if response.semantic_ordinals.len() > self.max_event_count {
            return Err(SemanticValidationError::MaterializationFailed);
        }
        buffers.semantic_ordinals.extend(response.semantic_ordinals);
        let outcome = self.validate_response_into(
            DecodedResponseHead {
                outcome: response.outcome,
                declared_len: response.declared_len,
            },
            &mut buffers,
        )?;
        Ok(buffers.into_execution(outcome))
    }

    pub(crate) fn validate_response_into(
        &self,
        response: DecodedResponseHead,
        buffers: &mut ResponseDecodeBuffers,
    ) -> Result<ValidatedOutcome, SemanticValidationError> {
        buffers.events.clear();
        if buffers.max_event_count != self.max_event_count
            || buffers.events.capacity() < self.max_event_count
            || buffers.semantic_ordinals.len() > self.max_event_count
        {
            return Err(SemanticValidationError::InsufficientEventCapacity);
        }
        let result = (|| {
            let path_outcome = match response.outcome {
                ResponseOutcome::ScalarSuccess(_) => TracePathOutcome::ScalarSuccess,
                ResponseOutcome::OwnedSuccess { .. } => TracePathOutcome::OwnedSuccess,
                ResponseOutcome::Failure { selected_ordinal } => {
                    TracePathOutcome::Failure { selected_ordinal }
                }
            };
            if !self
                .trace_path_certificate
                .accepts(&buffers.semantic_ordinals, path_outcome)
            {
                return Err(SemanticValidationError::TracePathRejected);
            }
            for ordinal in &buffers.semantic_ordinals {
                let index = usize::try_from(*ordinal)
                    .ok()
                    .and_then(|value| value.checked_sub(1))
                    .ok_or(SemanticValidationError::MaterializationFailed)?;
                let template = self
                    .event_templates
                    .get(index)
                    .ok_or(SemanticValidationError::MaterializationFailed)?;
                buffers.events.push(Arc::clone(template));
            }
            validate_direct_trivial_structure(&buffers.events)?;
            let outcome = match response.outcome {
                ResponseOutcome::ScalarSuccess(value) => {
                    validate_success_shape(
                        &buffers.semantic_ordinals,
                        &buffers.events,
                        &self.allowed_result_commit_ordinals,
                        self.required_owned_transfer_ordinals,
                    )?;
                    ValidatedOutcome::ScalarSuccess(value)
                }
                ResponseOutcome::OwnedSuccess { owner_ordinal } => {
                    validate_success_shape(
                        &buffers.semantic_ordinals,
                        &buffers.events,
                        &self.allowed_result_commit_ordinals,
                        self.required_owned_transfer_ordinals,
                    )?;
                    ValidatedOutcome::OwnedSuccess { owner_ordinal }
                }
                ResponseOutcome::Failure { selected_ordinal } => {
                    let index = usize::try_from(selected_ordinal)
                        .ok()
                        .and_then(|value| value.checked_sub(1))
                        .ok_or(SemanticValidationError::MaterializationFailed)?;
                    let selected = self
                        .dictionary
                        .entries()
                        .get(index)
                        .ok_or(SemanticValidationError::MaterializationFailed)?;
                    validate_failure_shape(
                        selected_ordinal,
                        selected,
                        &buffers.semantic_ordinals,
                        &buffers.events,
                    )?;
                    let (source, status) = self
                        .failure_templates
                        .get(index)
                        .and_then(Option::as_ref)
                        .ok_or(SemanticValidationError::FailureSelectedOrdinalNotSelection)?;
                    ValidatedOutcome::Failure {
                        selected_ordinal,
                        source: Arc::clone(source),
                        status: Arc::clone(status),
                    }
                }
            };
            Ok(outcome)
        })();
        if result.is_err() {
            buffers.events.clear();
        }
        result
    }
}

impl ResponseDecodeBuffers {
    pub(crate) fn try_new(descriptor: &Descriptor) -> Result<Self, WireError> {
        Self::try_new_for_max(descriptor.capacities.max_event_count as usize)
    }

    fn try_new_for_max(max_event_count: usize) -> Result<Self, WireError> {
        let mut semantic_ordinals = Vec::new();
        semantic_ordinals
            .try_reserve_exact(max_event_count)
            .map_err(|_| WireError::AllocationFailed)?;
        let mut events = Vec::new();
        events
            .try_reserve_exact(max_event_count)
            .map_err(|_| WireError::AllocationFailed)?;
        Ok(Self {
            semantic_ordinals,
            events,
            max_event_count,
        })
    }

    pub(crate) fn decode_response_into(
        &mut self,
        descriptor: &Descriptor,
        expected_invocation: NonZeroU64,
        storage: &[u8],
    ) -> Result<DecodedResponseHead, WireError> {
        if self.max_event_count != descriptor.capacities.max_event_count as usize {
            return Err(WireError::InsufficientOutputCapacity);
        }
        decode_response_into(
            descriptor,
            expected_invocation,
            storage,
            &mut self.semantic_ordinals,
        )
    }

    pub(crate) fn semantic_ordinals(&self) -> &[u32] {
        &self.semantic_ordinals
    }

    pub(crate) fn events(&self) -> &[Arc<TraceEvent>] {
        &self.events
    }

    pub(crate) fn into_execution(self, outcome: ValidatedOutcome) -> ValidatedExecution {
        ValidatedExecution {
            outcome,
            events: self.events,
        }
    }
}

fn validate_direct_trivial_structure(
    events: &[Arc<TraceEvent>],
) -> Result<(), SemanticValidationError> {
    let mut index = 0;
    while index < events.len() {
        match &events[index].event {
            TraceEventKind::FinalizeBegin {
                source,
                lifecycle_id,
                guard_flag,
                binding_import,
            } => {
                let Some(next) = events.get(index + 1) else {
                    return Err(SemanticValidationError::FinalizePairMismatch);
                };
                if !matches!(
                    &next.event,
                    TraceEventKind::FinalizeEnd {
                        source: end_source,
                        lifecycle_id: end_lifecycle,
                        guard_flag: end_guard,
                        binding_import: end_binding,
                    } if end_source == source
                        && end_lifecycle == lifecycle_id
                        && end_guard == guard_flag
                        && end_binding == binding_import
                ) {
                    return Err(SemanticValidationError::FinalizePairMismatch);
                }
                index += 2;
            }
            TraceEventKind::FinalizeEnd { .. } => {
                return Err(SemanticValidationError::OrphanFinalizeEnd);
            }
            _ => index += 1,
        }
    }
    Ok(())
}

fn validate_success_shape(
    ordinals: &[u32],
    events: &[Arc<TraceEvent>],
    allowed_result_commit_ordinals: &[u32],
    required_owned_transfer_ordinals: Option<[u32; 2]>,
) -> Result<(), SemanticValidationError> {
    let result_commit_count = events
        .iter()
        .filter(|event| matches!(event.event, TraceEventKind::ResultCommit { .. }))
        .count();
    if result_commit_count != 1 {
        return Err(SemanticValidationError::SuccessResultCommitCount);
    }
    if !matches!(
        events.last().map(|event| &event.event),
        Some(TraceEventKind::ResultCommit { .. })
    ) {
        return Err(SemanticValidationError::SuccessResultCommitNotFinal);
    }
    let Some(result_ordinal) = ordinals.last() else {
        return Err(SemanticValidationError::SuccessResultCommitCount);
    };
    if !allowed_result_commit_ordinals.contains(result_ordinal) {
        return Err(SemanticValidationError::SuccessResultCommitMismatch);
    }
    if let Some([stage_ordinal, publication_ordinal]) = required_owned_transfer_ordinals {
        let mut stage_positions = ordinals
            .iter()
            .enumerate()
            .filter(|(_, ordinal)| **ordinal == stage_ordinal)
            .map(|(index, _)| index);
        let mut publication_positions = ordinals
            .iter()
            .enumerate()
            .filter(|(_, ordinal)| **ordinal == publication_ordinal)
            .map(|(index, _)| index);
        let Some(stage_position) = stage_positions.next() else {
            return Err(SemanticValidationError::SuccessOwnedTransferMismatch);
        };
        let Some(publication_position) = publication_positions.next() else {
            return Err(SemanticValidationError::SuccessOwnedTransferMismatch);
        };
        if stage_positions.next().is_some()
            || publication_positions.next().is_some()
            || stage_position >= publication_position
        {
            return Err(SemanticValidationError::SuccessOwnedTransferMismatch);
        }
    }
    if events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::SelectFailure { .. }))
    {
        return Err(SemanticValidationError::SuccessContainsFailureSelection);
    }
    Ok(())
}

fn validate_failure_shape(
    selected_ordinal: u32,
    selected_entry: &SemanticEventEntry,
    ordinals: &[u32],
    events: &[Arc<TraceEvent>],
) -> Result<(), SemanticValidationError> {
    if ordinals
        .iter()
        .filter(|ordinal| **ordinal == selected_ordinal)
        .count()
        != 1
    {
        return Err(SemanticValidationError::FailureSelectedOrdinalOccurrence);
    }
    let TraceEventKind::SelectFailure {
        source: selected_source,
        status: selected_status,
    } = &selected_entry.event
    else {
        return Err(SemanticValidationError::FailureSelectedOrdinalNotSelection);
    };
    let mut selections = events.iter().filter_map(|event| match &event.event {
        TraceEventKind::SelectFailure { source, status } => Some((source, status)),
        _ => None,
    });
    let Some((materialized_source, materialized_status)) = selections.next() else {
        return Err(SemanticValidationError::FailureSelectionCount);
    };
    if selections.next().is_some() {
        return Err(SemanticValidationError::FailureSelectionCount);
    }
    if materialized_source != selected_source || materialized_status != selected_status {
        return Err(SemanticValidationError::FailureSelectionMismatch);
    }
    if events
        .iter()
        .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. }))
    {
        return Err(SemanticValidationError::FailureContainsResultCommit);
    }
    Ok(())
}

#[cfg(test)]
#[path = "callable_semantics/tests.rs"]
mod tests;
