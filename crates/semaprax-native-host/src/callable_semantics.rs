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
mod tests {
    use std::path::Path;

    use semaprax::cleanup_plan::{ContractPhase, StatusLane};
    use semaprax::codegen::emit_native_callable_admission;
    use semaprax::conformance::{TraceEventKind, TraceOutcome};
    use semaprax::hir::{self, DeclarationId};
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;
    use semaprax::semantic_trace::build_semantic_event_dictionary;
    use semaprax::trace_path_certificate::build_trace_path_certificate;

    use super::*;
    use crate::callable_wire::{encode_response, DecodedResponse};
    use crate::descriptor_v2::{Capacities, Fingerprints, ResultShape};

    const SOURCE: &str = r#"module test.callable_semantics;

@id("test.checked")
fn checked(value: i64) -> i64
requires value >= 0
ensures result >= 0
{
    value
}

@id("test.other")
fn other() -> i64 { 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    const OWNED_SOURCE: &str = r#"module test.callable_owned_semantics;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    const CHANGED_SOURCE: &str = r#"module test.callable_semantics;

@id("test.checked")
fn checked(value: i64) -> i64
requires value >= 0
requires value != 99
ensures result >= 0
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn dictionaries() -> (SemanticEventDictionary, SemanticEventDictionary) {
        let parsed = semaprax::parse(SOURCE, Path::new("callable-semantics.spx")).unwrap();
        let program = hir::resolve(&parsed).unwrap();
        (
            build_semantic_event_dictionary(&program, &DeclarationId::new("test.checked")).unwrap(),
            build_semantic_event_dictionary(&program, &DeclarationId::new("test.other")).unwrap(),
        )
    }

    fn certificate(dictionary: &SemanticEventDictionary) -> TracePathCertificate {
        let source = if dictionary.function().as_str().starts_with("token.") {
            OWNED_SOURCE
        } else {
            SOURCE
        };
        let parsed = semaprax::parse(source, Path::new("callable-certificate.spx")).unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| &function.id == dictionary.function())
            .unwrap();
        build_trace_path_certificate(&program, function, dictionary).unwrap()
    }

    fn dictionary_and_certificate(
        source: &str,
        function_id: &str,
    ) -> (SemanticEventDictionary, TracePathCertificate) {
        let parsed =
            semaprax::parse(source, Path::new("callable-certificate-binding.spx")).unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
            .unwrap();
        let dictionary = build_semantic_event_dictionary(&program, &function.id).unwrap();
        let certificate = build_trace_path_certificate(&program, function, &dictionary).unwrap();
        (dictionary, certificate)
    }

    fn descriptor(dictionary: &SemanticEventDictionary) -> Descriptor {
        let certificate = certificate(dictionary);
        Descriptor {
            target: "test-target".to_owned(),
            fingerprints: Fingerprints {
                schema: [1; 32],
                target: [2; 32],
                semantic_module: [3; 32],
                physical_module: [4; 32],
                function_template: [5; 32],
                execution_cleanup: [6; 32],
                event_dictionary: dictionary.fingerprint(),
                trace_path_certificate: certificate.fingerprint(),
                request_schema: [8; 32],
                response_schema: [9; 32],
                call_abi: [10; 32],
                call_contract: [11; 32],
            },
            module: "test.callable_semantics".to_owned(),
            function: dictionary.function().as_str().to_owned(),
            getter_symbol: "spx_getter".to_owned(),
            callable_symbol: "spx_call".to_owned(),
            call_abi_tag: 1,
            obligations: 0x0f,
            capacities: Capacities {
                max_request_bytes: 64,
                max_response_bytes: 68 + 12 + 4 * 16,
                max_event_count: 16,
                dictionary_bytes: dictionary.canonical_json().len() as u32,
                dictionary_entries: dictionary.entries().len() as u32,
            },
            parameters: Vec::new(),
            result: ResultShape::ScalarI64,
        }
    }

    fn owned_dictionary(function_id: &str) -> (SemanticEventDictionary, String) {
        let parsed =
            semaprax::parse(OWNED_SOURCE, Path::new("callable-owned-semantics.spx")).unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
            .unwrap();
        let value = function.params[0].id.as_str().to_owned();
        let dictionary = build_semantic_event_dictionary(&program, &function.id).unwrap();
        (dictionary, value)
    }

    fn owned_descriptor(dictionary: &SemanticEventDictionary, value: String) -> Descriptor {
        let mut descriptor = descriptor(dictionary);
        descriptor.parameters = vec![Parameter::Owned {
            index: 0,
            value,
            owner_ordinal: 0,
            resource: "token.type".to_owned(),
            lifecycle: "token.drop".to_owned(),
            payload_wire_kind: 1,
        }];
        descriptor.result = ResultShape::OwnedInput {
            parameter_index: 0,
            owner_ordinal: 0,
        };
        descriptor
    }

    fn ordinal(
        dictionary: &SemanticEventDictionary,
        predicate: impl Fn(&TraceEventKind) -> bool,
    ) -> u32 {
        dictionary
            .entries()
            .iter()
            .find(|entry| predicate(&entry.event))
            .unwrap()
            .ordinal
    }

    fn result_ordinal(dictionary: &SemanticEventDictionary) -> u32 {
        ordinal(dictionary, |event| {
            matches!(event, TraceEventKind::ResultCommit { .. })
        })
    }

    fn failure_ordinals(dictionary: &SemanticEventDictionary) -> Vec<u32> {
        dictionary
            .entries()
            .iter()
            .filter_map(|entry| {
                matches!(entry.event, TraceEventKind::SelectFailure { .. }).then_some(entry.ordinal)
            })
            .collect()
    }

    fn response(outcome: ResponseOutcome, ordinals: Vec<u32>) -> DecodedResponse {
        DecodedResponse {
            outcome,
            declared_len: 0,
            semantic_ordinals: ordinals,
        }
    }

    fn authenticate(
        descriptor: &Descriptor,
        dictionary: SemanticEventDictionary,
    ) -> Result<AuthenticatedSemanticDictionary, SemanticValidationError> {
        let certificate = certificate(&dictionary);
        authenticate_dictionary(descriptor, dictionary, certificate)
    }

    fn invocation() -> NonZeroU64 {
        NonZeroU64::new(0x0102_0304_0506_0708).unwrap()
    }

    #[test]
    fn trace_certificate_function_dictionary_fingerprint_and_capacity_are_independent() {
        let (dictionary, other_dictionary) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let other_function = certificate(&other_dictionary);
        assert_eq!(
            authenticate_dictionary(&descriptor, dictionary.clone(), other_function).err(),
            Some(SemanticValidationError::WrongTracePathCertificateFunction)
        );

        let (changed_dictionary, changed_certificate) =
            dictionary_and_certificate(CHANGED_SOURCE, "test.checked");
        assert_ne!(changed_dictionary.fingerprint(), dictionary.fingerprint());
        assert_eq!(
            authenticate_dictionary(&descriptor, dictionary.clone(), changed_certificate).err(),
            Some(SemanticValidationError::WrongTracePathCertificateDictionary)
        );

        let certificate = certificate(&dictionary);
        let mut wrong_fingerprint = descriptor.clone();
        wrong_fingerprint.fingerprints.trace_path_certificate[0] ^= 1;
        assert_eq!(
            authenticate_dictionary(&wrong_fingerprint, dictionary.clone(), certificate).err(),
            Some(SemanticValidationError::WrongTracePathCertificateFingerprint)
        );

        let corpus = build_owned_resource_corpus_v1().unwrap();
        let artifact = emit_native_callable_admission(
            &corpus.program,
            &DeclarationId::new("token.choose-second"),
        )
        .unwrap();
        let mut insufficient = Descriptor::parse(artifact.descriptor()).unwrap();
        let certificate = artifact.trace_path_certificate().clone();
        assert!(certificate.max_path_events() > 3);
        insufficient.capacities.max_event_count = certificate.max_path_events() - 1;
        assert_eq!(
            authenticate_dictionary(
                &insufficient,
                artifact.semantic_event_dictionary().clone(),
                certificate,
            )
            .err(),
            Some(SemanticValidationError::TracePathCertificateExceedsEventCapacity)
        );
    }

    #[test]
    fn trace_dfa_rejects_cleanup_omission_duplication_reordering_and_interleaving() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let authenticate_case = |scenario: &str| {
            let case = corpus
                .cases
                .iter()
                .find(|case| case.scenario_id == scenario)
                .unwrap();
            let artifact = emit_native_callable_admission(
                &corpus.program,
                &DeclarationId::new(case.function_id),
            )
            .unwrap();
            let descriptor = Descriptor::parse(artifact.descriptor()).unwrap();
            let dictionary = artifact.semantic_event_dictionary().clone();
            let ordinals = case
                .reference
                .events
                .iter()
                .map(|event| dictionary.ordinal_for(&event.event).unwrap())
                .collect::<Vec<_>>();
            let outcome = match &case.reference.outcome {
                TraceOutcome::Success { .. } => match descriptor.result {
                    ResultShape::ScalarI64 => ResponseOutcome::ScalarSuccess(0),
                    ResultShape::OwnedInput { owner_ordinal, .. } => {
                        ResponseOutcome::OwnedSuccess { owner_ordinal }
                    }
                },
                TraceOutcome::Failure {
                    selected_source, ..
                } => ResponseOutcome::Failure {
                    selected_ordinal: dictionary
                        .entries()
                        .iter()
                        .find_map(|entry| {
                            matches!(
                                &entry.event,
                                TraceEventKind::SelectFailure { source, .. }
                                    if source == selected_source
                            )
                            .then_some(entry.ordinal)
                        })
                        .unwrap(),
                },
            };
            let authenticated = authenticate_dictionary(
                &descriptor,
                dictionary.clone(),
                artifact.trace_path_certificate().clone(),
            )
            .unwrap();
            (authenticated, dictionary, ordinals, outcome)
        };

        let (discard, _, discard_path, discard_outcome) = authenticate_case("discard-zero");
        let mut omitted_finalizer = discard_path.clone();
        omitted_finalizer.remove(1);
        let mut duplicate_pair = discard_path.clone();
        duplicate_pair.splice(2..2, discard_path[..2].iter().copied());
        assert_eq!(
            discard
                .validate_response(response(discard_outcome, omitted_finalizer))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
        assert_eq!(
            discard
                .validate_response(response(discard_outcome, duplicate_pair))
                .unwrap_err(),
            SemanticValidationError::MaterializationFailed,
            "the descriptor bound rejects the duplicate before the DFA walk"
        );

        let (requires, _, mut failure_path, failure_outcome) = authenticate_case("requires-false");
        let selection = failure_path.remove(0);
        failure_path.push(selection);
        assert_eq!(
            requires
                .validate_response(response(failure_outcome, failure_path))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );

        let (choose, dictionary, choose_path, choose_outcome) =
            authenticate_case("choose-second-zero-max");
        let finalizers = choose_path
            .iter()
            .enumerate()
            .filter(|(_, ordinal)| {
                matches!(
                    dictionary.entries()[(**ordinal - 1) as usize].event,
                    TraceEventKind::FinalizeBegin { .. } | TraceEventKind::FinalizeEnd { .. }
                )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let transfers = choose_path
            .iter()
            .enumerate()
            .filter(|(_, ordinal)| {
                matches!(
                    dictionary.entries()[(**ordinal - 1) as usize].event,
                    TraceEventKind::Transfer { .. }
                )
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(finalizers.len(), 2);
        assert_eq!(transfers.len(), 2);

        let mut missing_non_result_cleanup = choose_path.clone();
        for index in finalizers.iter().rev() {
            missing_non_result_cleanup.remove(*index);
        }
        assert_eq!(
            choose
                .validate_response(response(choose_outcome, missing_non_result_cleanup))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );

        let mut interleaved_transfers = choose_path;
        let end = interleaved_transfers.remove(finalizers[1]);
        let begin = interleaved_transfers.remove(finalizers[0]);
        interleaved_transfers.splice(transfers[1]..transfers[1], [begin, end]);
        assert_eq!(
            choose
                .validate_response(response(choose_outcome, interleaved_transfers))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
    }

    #[test]
    fn authentic_dictionary_materializes_valid_success_and_failure() {
        let (dictionary, _) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let authenticated = authenticate(&descriptor, dictionary.clone()).unwrap();
        let result = result_ordinal(&dictionary);
        let success = authenticated
            .validate_response(response(ResponseOutcome::ScalarSuccess(7), vec![result]))
            .unwrap();
        assert!(matches!(
            success.outcome,
            ValidatedOutcome::ScalarSuccess(7)
        ));
        assert_eq!(success.events.len(), 1);

        let selected = failure_ordinals(&dictionary)[0];
        let failure = authenticated
            .validate_response(response(
                ResponseOutcome::Failure {
                    selected_ordinal: selected,
                },
                vec![selected],
            ))
            .unwrap();
        assert!(matches!(
            failure.outcome,
            ValidatedOutcome::Failure {
                selected_ordinal,
                ..
            } if selected_ordinal == selected
        ));
    }

    #[test]
    fn dictionary_function_fingerprint_length_and_count_are_authenticated() {
        let (dictionary, other) = dictionaries();
        let canonical = descriptor(&dictionary);

        assert_eq!(
            authenticate(&canonical, other).err(),
            Some(SemanticValidationError::WrongFunction)
        );
        let mut wrong = canonical.clone();
        wrong.fingerprints.event_dictionary[0] ^= 1;
        assert_eq!(
            authenticate(&wrong, dictionary.clone()).err(),
            Some(SemanticValidationError::WrongDictionaryFingerprint)
        );
        let mut wrong = canonical.clone();
        wrong.capacities.dictionary_bytes += 1;
        assert_eq!(
            authenticate(&wrong, dictionary.clone()).err(),
            Some(SemanticValidationError::WrongDictionaryByteLength)
        );
        let mut wrong = canonical;
        wrong.capacities.dictionary_entries += 1;
        assert_eq!(
            authenticate(&wrong, dictionary).err(),
            Some(SemanticValidationError::WrongDictionaryEntryCount)
        );

        let (dictionary, _) = dictionaries();
        let mut wrong = descriptor(&dictionary);
        wrong.capacities.dictionary_entries += 1;
        wrong.capacities.dictionary_bytes += 1;
        assert_eq!(
            authenticate(&wrong, dictionary).err(),
            Some(SemanticValidationError::WrongDictionaryEntryCount),
            "entry bounds must reject before canonical byte serialization"
        );

        let (dictionary, _) = dictionaries();
        let certificate = certificate(&dictionary);
        let mut wrong = descriptor(&dictionary);
        wrong.fingerprints.trace_path_certificate[0] ^= 1;
        assert_eq!(
            authenticate_dictionary(&wrong, dictionary, certificate).err(),
            Some(SemanticValidationError::WrongTracePathCertificateFingerprint)
        );
    }

    #[test]
    fn success_rejects_missing_or_duplicate_commit_and_any_failure_selection() {
        let (dictionary, _) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let authenticated = authenticate(&descriptor, dictionary.clone()).unwrap();
        let result = result_ordinal(&dictionary);
        let selected = failure_ordinals(&dictionary)[0];

        assert_eq!(
            authenticated
                .validate_response(response(ResponseOutcome::ScalarSuccess(1), vec![]))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::ScalarSuccess(1),
                    vec![result, result],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::ScalarSuccess(1),
                    vec![selected, result],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::ScalarSuccess(1),
                    vec![result, selected],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
    }

    #[test]
    fn failure_selected_ordinal_must_be_present_once_and_be_select_failure() {
        let (dictionary, _) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let authenticated = authenticate(&descriptor, dictionary.clone()).unwrap();
        let failures = failure_ordinals(&dictionary);
        let selected = failures[0];
        let result = result_ordinal(&dictionary);

        for ordinals in [vec![], vec![selected, selected]] {
            assert_eq!(
                authenticated
                    .validate_response(response(
                        ResponseOutcome::Failure {
                            selected_ordinal: selected,
                        },
                        ordinals,
                    ))
                    .unwrap_err(),
                SemanticValidationError::TracePathRejected
            );
        }
        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::Failure {
                        selected_ordinal: result,
                    },
                    vec![result],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
    }

    #[test]
    fn failure_rejects_other_selection_and_result_commit_cross_contamination() {
        let (dictionary, _) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let authenticated = authenticate(&descriptor, dictionary.clone()).unwrap();
        let failures = failure_ordinals(&dictionary);
        assert!(failures.len() >= 2);
        let selected = failures[0];
        let other = failures[1];
        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::Failure {
                        selected_ordinal: selected,
                    },
                    vec![selected, other],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );

        let result = result_ordinal(&dictionary);
        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::Failure {
                        selected_ordinal: selected,
                    },
                    vec![selected, result],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );
    }

    #[test]
    fn failure_source_or_status_mismatch_is_rejected_by_relation_gate() {
        let (dictionary, _) = dictionaries();
        let failures: Vec<_> = dictionary
            .entries()
            .iter()
            .filter(|entry| matches!(entry.event, TraceEventKind::SelectFailure { .. }))
            .collect();
        assert!(failures.len() >= 2);
        let selected = failures[0];
        let other = failures[1];
        let mismatched = TraceEvent {
            function: dictionary.function().clone(),
            invocation: Default::default(),
            event: other.event.clone(),
        };
        assert_eq!(
            validate_failure_shape(
                selected.ordinal,
                selected,
                &[selected.ordinal],
                &[Arc::new(mismatched)]
            ),
            Err(SemanticValidationError::FailureSelectionMismatch)
        );

        let (source, status) = match &selected.event {
            TraceEventKind::SelectFailure { source, status } => (source, status),
            _ => unreachable!(),
        };
        let (other_source, other_status) = match &other.event {
            TraceEventKind::SelectFailure { source, status } => (source, status),
            _ => unreachable!(),
        };
        assert!(
            source != other_source || status != other_status,
            "fixture failures must differ in source or normalized status"
        );
        assert!(matches!(source.lane, StatusLane::ContractFalse));
        assert!(matches!(
            status,
            value if value == &NormalizedStatus::contract(ContractPhase::Requires)
                || value == &NormalizedStatus::contract(ContractPhase::Ensures)
        ));
    }

    #[test]
    fn postcommit_decode_and_validation_reuse_preallocated_buffers_without_growth() {
        let (dictionary, _) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let result = result_ordinal(&dictionary);
        let authenticated = authenticate(&descriptor, dictionary).unwrap();
        let storage = encode_response(
            &descriptor,
            invocation(),
            ResponseOutcome::ScalarSuccess(7),
            &[result],
        )
        .unwrap();
        let mut buffers = ResponseDecodeBuffers::try_new(&descriptor).unwrap();
        let ordinal_pointer = buffers.semantic_ordinals.as_ptr();
        let ordinal_capacity = buffers.semantic_ordinals.capacity();
        let event_pointer = buffers.events.as_ptr();
        let event_capacity = buffers.events.capacity();

        let head = buffers
            .decode_response_into(&descriptor, invocation(), &storage)
            .unwrap();
        assert_eq!(buffers.semantic_ordinals(), [result]);
        assert_eq!(buffers.semantic_ordinals.as_ptr(), ordinal_pointer);
        assert_eq!(buffers.semantic_ordinals.capacity(), ordinal_capacity);

        let outcome = authenticated
            .validate_response_into(head, &mut buffers)
            .unwrap();
        assert_eq!(outcome, ValidatedOutcome::ScalarSuccess(7));
        assert_eq!(buffers.events().len(), 1);
        assert_eq!(buffers.events.as_ptr(), event_pointer);
        assert_eq!(buffers.events.capacity(), event_capacity);
    }

    #[test]
    fn postcommit_validation_requires_event_capacity_and_clears_partial_output() {
        let (dictionary, _) = dictionaries();
        let descriptor = descriptor(&dictionary);
        let selected = failure_ordinals(&dictionary)[0];
        let result = result_ordinal(&dictionary);
        let authenticated = authenticate(&descriptor, dictionary).unwrap();
        let head = DecodedResponseHead {
            outcome: ResponseOutcome::Failure {
                selected_ordinal: selected,
            },
            declared_len: 0,
        };

        let mut undersized = ResponseDecodeBuffers {
            semantic_ordinals: vec![selected],
            events: Vec::new(),
            max_event_count: descriptor.capacities.max_event_count as usize,
        };
        assert_eq!(
            authenticated.validate_response_into(head, &mut undersized),
            Err(SemanticValidationError::InsufficientEventCapacity)
        );
        assert!(undersized.events.is_empty());

        let mut buffers = ResponseDecodeBuffers::try_new(&descriptor).unwrap();
        buffers.semantic_ordinals.extend([selected, result]);
        let event_pointer = buffers.events.as_ptr();
        let event_capacity = buffers.events.capacity();
        assert_eq!(
            authenticated.validate_response_into(head, &mut buffers),
            Err(SemanticValidationError::TracePathRejected)
        );
        assert!(buffers.events.is_empty());
        assert_eq!(buffers.events.as_ptr(), event_pointer);
        assert_eq!(buffers.events.capacity(), event_capacity);
    }

    #[test]
    fn result_projection_is_authenticated_against_exact_descriptor_shape() {
        let (dictionary, value) = owned_dictionary("token.identity");
        let descriptor = owned_descriptor(&dictionary, value);
        let authenticated = authenticate(&descriptor, dictionary.clone()).unwrap();
        let transfers = authenticated.required_owned_transfer_ordinals.unwrap();
        let commit = authenticated.allowed_result_commit_ordinals[0];
        let mut success_ordinals = transfers.to_vec();
        success_ordinals.push(commit);
        let execution = authenticated
            .validate_response(response(
                ResponseOutcome::OwnedSuccess { owner_ordinal: 0 },
                success_ordinals,
            ))
            .unwrap();
        assert!(matches!(
            execution.outcome,
            ValidatedOutcome::OwnedSuccess { owner_ordinal: 0 }
        ));

        assert_eq!(
            authenticated
                .validate_response(response(
                    ResponseOutcome::OwnedSuccess { owner_ordinal: 0 },
                    vec![commit],
                ))
                .unwrap_err(),
            SemanticValidationError::TracePathRejected
        );

        let mut insufficient = descriptor.clone();
        insufficient.capacities.max_event_count = 2;
        assert_eq!(
            authenticate(&insufficient, dictionary.clone()).err(),
            Some(SemanticValidationError::InsufficientSuccessEventCapacity)
        );

        let mut wrong_value = descriptor.clone();
        let Parameter::Owned { value, .. } = &mut wrong_value.parameters[0] else {
            unreachable!()
        };
        *value = "forged.value".to_owned();
        assert_eq!(
            authenticate(&wrong_value, dictionary.clone()).err(),
            Some(SemanticValidationError::OwnedResultTransferDictionaryMismatch)
        );

        let mut wrong_owner = descriptor.clone();
        wrong_owner.result = ResultShape::OwnedInput {
            parameter_index: 0,
            owner_ordinal: 1,
        };
        assert_eq!(
            authenticate(&wrong_owner, dictionary.clone()).err(),
            Some(SemanticValidationError::OwnedResultTransferDictionaryMismatch)
        );

        let mut wrong_kind = descriptor;
        wrong_kind.result = ResultShape::ScalarI64;
        assert_eq!(
            authenticate(&wrong_kind, dictionary).err(),
            Some(SemanticValidationError::ResultCommitDictionaryMismatch)
        );
    }

    #[test]
    fn direct_trivial_finalizers_must_be_adjacent_exact_pairs() {
        let (dictionary, _) = owned_dictionary("token.discard");
        let begin = dictionary
            .entries()
            .iter()
            .find(|entry| matches!(entry.event, TraceEventKind::FinalizeBegin { .. }))
            .unwrap();
        let end = dictionary
            .entries()
            .iter()
            .find(|entry| matches!(entry.event, TraceEventKind::FinalizeEnd { .. }))
            .unwrap();
        let event = |entry: &SemanticEventEntry| {
            Arc::new(TraceEvent {
                function: dictionary.function().clone(),
                invocation: InvocationPath::default(),
                event: entry.event.clone(),
            })
        };
        let begin = event(begin);
        let end = event(end);

        assert_eq!(
            validate_direct_trivial_structure(&[Arc::clone(&begin), Arc::clone(&end)]),
            Ok(())
        );
        assert_eq!(
            validate_direct_trivial_structure(&[Arc::clone(&begin)]),
            Err(SemanticValidationError::FinalizePairMismatch)
        );
        assert_eq!(
            validate_direct_trivial_structure(&[Arc::clone(&end)]),
            Err(SemanticValidationError::OrphanFinalizeEnd)
        );

        let mut mismatched_end = (*end).clone();
        let TraceEventKind::FinalizeEnd { guard_flag, .. } = &mut mismatched_end.event else {
            unreachable!()
        };
        guard_flag.0 = guard_flag.0.checked_add(1).unwrap();
        assert_eq!(
            validate_direct_trivial_structure(&[begin, Arc::new(mismatched_end)]),
            Err(SemanticValidationError::FinalizePairMismatch)
        );
    }
}
