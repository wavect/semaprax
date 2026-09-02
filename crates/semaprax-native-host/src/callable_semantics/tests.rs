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
    let parsed = semaprax::parse(source, Path::new("callable-certificate-binding.spx")).unwrap();
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
    let parsed = semaprax::parse(OWNED_SOURCE, Path::new("callable-owned-semantics.spx")).unwrap();
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
    let artifact =
        emit_native_callable_admission(&corpus.program, &DeclarationId::new("token.choose-second"))
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
        let artifact =
            emit_native_callable_admission(&corpus.program, &DeclarationId::new(case.function_id))
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
