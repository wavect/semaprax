//! Target-neutral semantic event dictionaries for backend conformance.
//!
//! A dictionary assigns one deterministic nonzero ordinal to each semantic
//! event shape that generated code may emit for one validated single-frame
//! function. Native and Wasm code execute their real control flow and emit only
//! these ordinals; hosts map them back to identities without reconstructing
//! cleanup, liveness, failure selection, or publication behavior.

use crate::cleanup_plan::{CleanupTransition, ExitContinuation, StatusProducer};
use crate::conformance::{
    ConformanceTrace, ImportSite, InvocationPath, NormalizedStatus, OperationOutcome, Retryability,
    StatusClass, TraceEvent, TraceEventKind, TraceOutcome,
};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    self, DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedResourceDropKind,
    ResolvedTypeDeclarationKind,
};
use sha2::{Digest, Sha256};

pub const SEMANTIC_EVENT_DICTIONARY_V1: &str = "semaprax.semantic-event-dictionary.v1";
pub const OWNED_RESOURCE_CORPUS_V1_SCENARIOS: [&str; 14] = [
    "discard-zero",
    "discard-max",
    "discard-two-reverse",
    "requires-false",
    "requires-true",
    "checked-success",
    "checked-add-overflow",
    "checked-precondition-false",
    "identity-zero",
    "identity-max",
    "choose-second-zero-max",
    "choose-second-zero-zero",
    "choose-second-requires-false",
    "ensures-false",
];
const DICTIONARY_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.semantic-event-dictionary-fingerprint.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticEventEntry {
    pub ordinal: u32,
    pub event: TraceEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticEventDictionary {
    schema: &'static str,
    function: DeclarationId,
    entries: Vec<SemanticEventEntry>,
}

impl SemanticEventDictionary {
    #[must_use]
    pub fn schema(&self) -> &'static str {
        self.schema
    }

    #[must_use]
    pub fn function(&self) -> &DeclarationId {
        &self.function
    }

    #[must_use]
    pub fn entries(&self) -> &[SemanticEventEntry] {
        &self.entries
    }

    #[must_use]
    pub fn ordinal_for(&self, event: &TraceEventKind) -> Option<u32> {
        self.entries
            .iter()
            .find(|entry| &entry.event == event)
            .map(|entry| entry.ordinal)
    }

    /// Canonical target-neutral dictionary bytes committed by
    /// [`Self::fingerprint`]. Hosts may cache or transport these bytes, but
    /// must still authenticate the descriptor fingerprint before using them.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        format!(
            "{{\"schema\":{},\"function\":{},\"entries\":{}}}",
            quote_json(self.schema),
            quote_json(self.function.as_str()),
            json_array(&self.entries, |entry| format!(
                "{{\"ordinal\":{},\"event\":{}}}",
                entry.ordinal,
                event_kind_json(&entry.event)
            )),
        )
    }

    /// Hash the canonical target-neutral dictionary projection. The ordinal
    /// order and every semantic identity are committed; physical pointers,
    /// handles, target layouts, and execution outcomes are absent.
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let projection = self.canonical_json();
        let mut hasher = Sha256::new();
        hasher.update(DICTIONARY_FINGERPRINT_DOMAIN);
        hasher.update((projection.len() as u64).to_le_bytes());
        hasher.update(projection.as_bytes());
        hasher.finalize().into()
    }

    /// Materialize an emitted ordinal sequence without choosing any path or
    /// inferring any event. Unknown/reserved ordinals fail closed.
    pub fn materialize_events(&self, ordinals: &[u32]) -> Result<Vec<TraceEvent>, Diagnostic> {
        ordinals
            .iter()
            .map(|ordinal| {
                let event = self
                    .entries
                    .iter()
                    .find(|entry| entry.ordinal == *ordinal)
                    .ok_or_else(|| {
                        dictionary_error(format!(
                            "runtime emitted unknown semantic event ordinal {ordinal}"
                        ))
                    })?;
                Ok(TraceEvent {
                    function: self.function.clone(),
                    invocation: InvocationPath::default(),
                    event: event.event.clone(),
                })
            })
            .collect()
    }

    pub fn materialize_trace(
        &self,
        scenario_id: impl Into<String>,
        ordinals: &[u32],
        outcome: TraceOutcome,
    ) -> Result<ConformanceTrace, Diagnostic> {
        Ok(ConformanceTrace::new(
            scenario_id,
            self.function.clone(),
            self.materialize_events(ordinals)?,
            outcome,
        ))
    }
}

fn event_kind_json(event: &TraceEventKind) -> String {
    match event {
        TraceEventKind::Initialize { at, destination } => format!(
            "{{\"kind\":\"initialize\",\"at\":{},\"destination\":{}}}",
            quote_json(at.as_str()),
            place_json(destination),
        ),
        TraceEventKind::Transfer {
            at,
            source,
            destination,
        } => format!(
            "{{\"kind\":\"transfer\",\"at\":{},\"source\":{},\"destination\":{}}}",
            quote_json(at.as_str()),
            place_json(source),
            place_json(destination),
        ),
        TraceEventKind::CallCommit {
            call,
            callee,
            arguments,
        } => format!(
            "{{\"kind\":\"call_commit\",\"call\":{},\"callee\":{},\"arguments\":{}}}",
            quote_json(call.as_str()),
            quote_json(callee.as_str()),
            json_array(arguments, |argument| format!(
                "{{\"parameter_index\":{},\"source\":{}}}",
                argument.parameter_index,
                place_json(&argument.source),
            )),
        ),
        TraceEventKind::ImportBegin { site, import_id } => format!(
            "{{\"kind\":\"import_begin\",\"site\":{},\"import_id\":{}}}",
            import_site_json(site),
            quote_json(import_id.as_str()),
        ),
        TraceEventKind::CallImportEnd {
            expression,
            import_id,
            outcome,
        } => format!(
            "{{\"kind\":\"call_import_end\",\"expression\":{},\"import_id\":{},\"outcome\":{}}}",
            quote_json(expression.as_str()),
            quote_json(import_id.as_str()),
            operation_outcome_json(outcome),
        ),
        TraceEventKind::FinalizerImportEnd {
            source,
            lifecycle_id,
            import_id,
        } => format!(
            "{{\"kind\":\"finalizer_import_end\",\"source\":{},\"lifecycle_id\":{},\"import_id\":{}}}",
            place_json(source),
            quote_json(lifecycle_id.as_str()),
            quote_json(import_id.as_str()),
        ),
        TraceEventKind::SelectFailure { source, status } => format!(
            "{{\"kind\":\"select_failure\",\"source\":{},\"status\":{}}}",
            status_source_json(source),
            status_json(status),
        ),
        TraceEventKind::FinalizeBegin {
            source,
            lifecycle_id,
            guard_flag,
            binding_import,
        } => finalize_json(
            "finalize_begin",
            source,
            lifecycle_id.as_str(),
            guard_flag.0,
            binding_import.as_ref().map(|value| value.as_str()),
        ),
        TraceEventKind::FinalizeEnd {
            source,
            lifecycle_id,
            guard_flag,
            binding_import,
        } => finalize_json(
            "finalize_end",
            source,
            lifecycle_id.as_str(),
            guard_flag.0,
            binding_import.as_ref().map(|value| value.as_str()),
        ),
        TraceEventKind::ResultCommit { source } => format!(
            "{{\"kind\":\"result_commit\",\"source\":{}}}",
            result_source_json(source),
        ),
    }
}

fn finalize_json(
    kind: &str,
    source: &crate::cleanup_plan::CleanupPlace,
    lifecycle_id: &str,
    guard_flag: u32,
    binding_import: Option<&str>,
) -> String {
    let binding_import = binding_import.map_or_else(|| "null".to_owned(), quote_json);
    format!(
        "{{\"kind\":{},\"source\":{},\"lifecycle_id\":{},\"guard_flag\":{},\"binding_import\":{}}}",
        quote_json(kind),
        place_json(source),
        quote_json(lifecycle_id),
        guard_flag,
        binding_import,
    )
}

fn import_site_json(site: &ImportSite) -> String {
    match site {
        ImportSite::Call { expression } => format!(
            "{{\"kind\":\"call\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        ImportSite::Finalizer {
            source,
            lifecycle_id,
        } => format!(
            "{{\"kind\":\"finalizer\",\"source\":{},\"lifecycle_id\":{}}}",
            place_json(source),
            quote_json(lifecycle_id.as_str()),
        ),
    }
}

fn operation_outcome_json(outcome: &OperationOutcome) -> String {
    match outcome {
        OperationOutcome::Success => "{\"kind\":\"success\"}".to_owned(),
        OperationOutcome::Failure(status) => format!(
            "{{\"kind\":\"failure\",\"status\":{}}}",
            status_json(status)
        ),
    }
}

fn status_source_json(source: &crate::cleanup_plan::StatusSourceId) -> String {
    let lane = match source.lane {
        crate::cleanup_plan::StatusLane::OperationFailure => "operation_failure",
        crate::cleanup_plan::StatusLane::ContractFalse => "contract_false",
    };
    format!(
        "{{\"expression\":{},\"lane\":{}}}",
        quote_json(source.expression.as_str()),
        quote_json(lane),
    )
}

fn status_json(status: &NormalizedStatus) -> String {
    let class = match status.class() {
        StatusClass::Contract => "contract",
        StatusClass::Arithmetic => "arithmetic",
        StatusClass::Import => "import",
        StatusClass::ExplicitClose => "explicit_close",
        StatusClass::Adapter => "adapter",
    };
    let retryable = match status.retryability() {
        Retryability::Known(value) => value.to_string(),
        Retryability::Unknown => quote_json("unknown"),
    };
    format!(
        "{{\"schema\":{},\"domain_id\":{},\"code\":{},\"class\":{},\"retryable\":{}}}",
        quote_json(status.schema()),
        quote_json(status.domain_id()),
        status.code(),
        quote_json(class),
        retryable,
    )
}

fn result_source_json(source: &crate::cleanup_plan::CleanupResultSource) -> String {
    match source {
        crate::cleanup_plan::CleanupResultSource::Scalar { expression } => format!(
            "{{\"kind\":\"scalar\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        crate::cleanup_plan::CleanupResultSource::Owned { storage } => {
            format!("{{\"kind\":\"owned\",\"storage\":{}}}", place_json(storage))
        }
    }
}

fn place_json(place: &crate::cleanup_plan::CleanupPlace) -> String {
    format!(
        "{{\"storage\":{},\"projections\":{}}}",
        storage_json(&place.storage),
        json_array(&place.projections, |projection| quote_json(
            projection.as_str()
        )),
    )
}

fn storage_json(storage: &crate::cleanup_plan::StorageId) -> String {
    match storage {
        crate::cleanup_plan::StorageId::Value(value) => format!(
            "{{\"kind\":\"value\",\"value\":{}}}",
            quote_json(value.as_str())
        ),
        crate::cleanup_plan::StorageId::Temporary(expression) => format!(
            "{{\"kind\":\"temporary\",\"expression\":{}}}",
            quote_json(expression.as_str())
        ),
        crate::cleanup_plan::StorageId::CallArgument {
            call,
            parameter_index,
            value_expression,
        } => format!(
            "{{\"kind\":\"call_argument\",\"call\":{},\"parameter_index\":{},\"value_expression\":{}}}",
            quote_json(call.as_str()),
            parameter_index,
            quote_json(value_expression.as_str()),
        ),
        crate::cleanup_plan::StorageId::ProvisionalResult => {
            "{\"kind\":\"provisional_result\"}".to_owned()
        }
    }
}

fn json_array<T>(values: &[T], mut render: impl FnMut(&T) -> String) -> String {
    let values = values.iter().map(&mut render).collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

/// Build the deterministic event dictionary for the current direct-trivial
/// resource execution slice. Unsupported shapes fail closed before any target
/// artifact can consume the dictionary.
pub fn build_semantic_event_dictionary(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<SemanticEventDictionary, Diagnostic> {
    hir::validate(program)?;
    if !program.function_templates.is_empty() || !program.function_instances.is_empty() {
        return Err(dictionary_error(
            "semantic trace dictionaries do not admit generic function templates or instances",
        ));
    }
    let function = program
        .functions
        .iter()
        .find(|function| &function.id == function_id)
        .ok_or_else(|| {
            dictionary_error(format!("function `{function_id}` is not in the program"))
        })?;
    let mut entries = Vec::new();

    for block in &function.cleanup_plan.blocks {
        for transition in &block.transitions {
            let event = match transition {
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => TraceEventKind::Transfer {
                    at: at.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                },
                CleanupTransition::SelectFailure { source } => {
                    let status_source = function
                        .cleanup_plan
                        .status_sources
                        .iter()
                        .find(|candidate| candidate.id == *source)
                        .ok_or_else(|| {
                            dictionary_error(format!(
                                "failure source `{}` is not declared",
                                source.expression
                            ))
                        })?;
                    let status = fixed_status(&status_source.producer, function)?;
                    TraceEventKind::SelectFailure {
                        source: source.clone(),
                        status,
                    }
                }
                CleanupTransition::Initialize { at, .. } => {
                    return Err(dictionary_error(format!(
                        "initialize transition `{at}` is outside the direct-resource slice"
                    )))
                }
                CleanupTransition::CallCommit { call, .. } => {
                    return Err(dictionary_error(format!(
                        "call commit `{call}` is outside the single-frame slice"
                    )))
                }
                CleanupTransition::StageCopyResult { .. } => {
                    return Err(dictionary_error(
                        "copy-result staging is outside the direct-resource slice",
                    ))
                }
            };
            push_unique(&mut entries, event)?;
        }
    }

    for exit in &function.cleanup_plan.exits {
        for action in &exit.finalize_in_order {
            let binding_import = lifecycle_binding(program, &action.lifecycle_id)?;
            push_unique(
                &mut entries,
                TraceEventKind::FinalizeBegin {
                    source: action.source.clone(),
                    lifecycle_id: action.lifecycle_id.clone(),
                    guard_flag: action.guard_flag,
                    binding_import: binding_import.clone(),
                },
            )?;
            push_unique(
                &mut entries,
                TraceEventKind::FinalizeEnd {
                    source: action.source.clone(),
                    lifecycle_id: action.lifecycle_id.clone(),
                    guard_flag: action.guard_flag,
                    binding_import,
                },
            )?;
        }
        if let ExitContinuation::CommitResult { source } = &exit.continuation {
            push_unique(
                &mut entries,
                TraceEventKind::ResultCommit {
                    source: source.clone(),
                },
            )?;
        }
    }

    if entries.is_empty() {
        return Err(dictionary_error(format!(
            "function `{}` has no semantic events in the admitted slice",
            function.id
        )));
    }
    Ok(SemanticEventDictionary {
        schema: SEMANTIC_EVENT_DICTIONARY_V1,
        function: function.id.clone(),
        entries,
    })
}

fn fixed_status(
    producer: &StatusProducer,
    function: &ResolvedFunction,
) -> Result<NormalizedStatus, Diagnostic> {
    match producer {
        StatusProducer::ContractFalse { phase, .. } => Ok(NormalizedStatus::contract(*phase)),
        StatusProducer::CheckedArithmetic {
            normalized_cases, ..
        } => match normalized_cases.as_slice() {
            [case] => Ok(NormalizedStatus::arithmetic(*case)),
            _ => Err(dictionary_error(format!(
                "function `{}` has a checked operation with non-singleton status mapping",
                function.id
            ))),
        },
        StatusProducer::PropagatedCall { callee } => Err(dictionary_error(format!(
            "propagated call `{callee}` is outside the single-frame slice"
        ))),
    }
}

fn lifecycle_binding(
    program: &ResolvedProgram,
    lifecycle: &DeclarationId,
) -> Result<Option<DeclarationId>, Diagnostic> {
    let drop = program.types.iter().find_map(|declaration| {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            return None;
        };
        (&drop.id == lifecycle).then_some(drop)
    });
    let Some(drop) = drop else {
        return Err(dictionary_error(format!(
            "lifecycle `{lifecycle}` is not declared by a resource"
        )));
    };
    match &drop.kind {
        ResolvedResourceDropKind::Trivial => Ok(None),
        ResolvedResourceDropKind::Imported { import, .. } => Err(dictionary_error(format!(
            "imported finalizer `{import}` is outside the direct-trivial slice"
        ))),
    }
}

fn push_unique(
    entries: &mut Vec<SemanticEventEntry>,
    event: TraceEventKind,
) -> Result<(), Diagnostic> {
    if entries.iter().any(|entry| entry.event == event) {
        return Ok(());
    }
    let ordinal = u32::try_from(entries.len())
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| dictionary_error("semantic event ordinal space is exhausted"))?;
    entries.push(SemanticEventEntry { ordinal, event });
    Ok(())
}

fn dictionary_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native/Wasm semantic event dictionary: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::cleanup_plan::{
        execute_for_conformance, CleanupScenario, ContractPhase, StatusProducer,
    };
    use crate::conformance::{TraceOutcome, TraceResult};
    use crate::hir;

    use super::*;

    const SOURCE: &str = r#"module test.semantic_dictionary;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{
    0
}

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64 { number + 1 }

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token { second }

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token
    ensures false
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        let parsed = crate::parse(SOURCE, Path::new("semantic-dictionary.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap()
    }

    #[test]
    fn reference_trace_round_trips_only_through_emitted_ordinals() {
        let program = program();
        let id = DeclarationId::new("token.discard");
        let dictionary = build_semantic_event_dictionary(&program, &id).unwrap();
        assert_eq!(dictionary.schema(), SEMANTIC_EVENT_DICTIONARY_V1);
        assert!(dictionary.entries().iter().all(|entry| entry.ordinal != 0));

        let scenario = CleanupScenario::new("discard", Some(TraceResult::I64(0)));
        let reference = execute_for_conformance(&program, &id, scenario).unwrap();
        let ordinals = reference
            .events
            .iter()
            .map(|event| dictionary.ordinal_for(&event.event).unwrap())
            .collect::<Vec<_>>();
        let materialized = dictionary
            .materialize_trace("discard", &ordinals, reference.outcome.clone())
            .unwrap();
        assert_eq!(materialized, reference);
        assert!(dictionary.materialize_events(&[0]).is_err());
        assert!(dictionary.materialize_events(&[u32::MAX]).is_err());

        let rebuilt = build_semantic_event_dictionary(&program, &id).unwrap();
        assert_eq!(dictionary.fingerprint(), rebuilt.fingerprint());
        assert_eq!(
            dictionary.fingerprint(),
            [
                0xd8, 0x8d, 0x63, 0xd2, 0xb1, 0xc4, 0x4b, 0x6f, 0x72, 0xf0, 0xf1, 0x2d, 0x84, 0xcd,
                0x79, 0x8f, 0x2d, 0x4f, 0x79, 0xc9, 0xac, 0x4f, 0x6c, 0x8f, 0x6d, 0xcc, 0xd3, 0x9b,
                0x6b, 0x96, 0x67, 0x0e,
            ]
        );
        let canonical = dictionary.canonical_json();
        assert!(canonical.starts_with(
            "{\"schema\":\"semaprax.semantic-event-dictionary.v1\",\"function\":\"token.discard\",\"entries\":[{\"ordinal\":1,\"event\":{\"kind\":\"finalize_begin\""
        ));
        assert!(canonical.contains("{\"ordinal\":2,\"event\":{\"kind\":\"finalize_end\""));
        assert!(canonical.contains("{\"ordinal\":3,\"event\":{\"kind\":\"result_commit\""));
        for forbidden in [
            "scenario_id",
            "root_function",
            "invocation",
            "outcome",
            "semaprax.conformance-trace.v1",
        ] {
            assert!(
                !canonical.contains(forbidden),
                "dictionary leaked `{forbidden}`"
            );
        }
        assert_eq!(
            dictionary
                .entries()
                .iter()
                .map(|entry| entry.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(matches!(
            dictionary.entries()[0].event,
            TraceEventKind::FinalizeBegin { .. }
        ));
        assert!(matches!(
            dictionary.entries()[1].event,
            TraceEventKind::FinalizeEnd { .. }
        ));
        assert!(matches!(
            dictionary.entries()[2].event,
            TraceEventKind::ResultCommit { .. }
        ));
        assert!(dictionary.fingerprint().iter().any(|byte| *byte != 0));
        let identity =
            build_semantic_event_dictionary(&program, &DeclarationId::new("token.identity"))
                .unwrap();
        assert_ne!(dictionary.fingerprint(), identity.fingerprint());

        let unrelated_change = SOURCE.replace(
            "@id(\"app.main\")\nfn main() -> i64 { 0 }",
            "\n@id(\"app.other\")\nfn   main ( ) -> i64 {\n  99\n}",
        );
        let parsed = crate::parse(
            &unrelated_change,
            Path::new("semantic-dictionary-unrelated.spx"),
        )
        .unwrap();
        let changed = hir::resolve(&parsed).unwrap();
        assert_eq!(
            dictionary,
            build_semantic_event_dictionary(&changed, &id).unwrap()
        );
    }

    #[test]
    fn failure_and_checked_statuses_are_dictionary_bound() {
        let program = program();
        let requires = function(&program, "token.requires");
        let dictionary = build_semantic_event_dictionary(&program, &requires.id).unwrap();
        let source = requires
            .cleanup_plan
            .status_sources
            .iter()
            .find(|source| {
                matches!(
                    source.producer,
                    StatusProducer::ContractFalse {
                        phase: ContractPhase::Requires,
                        ..
                    }
                )
            })
            .unwrap();
        let mut scenario = CleanupScenario::new("requires-false", None);
        scenario.booleans = BTreeMap::from([(source.id.expression.clone(), false)]);
        let reference = execute_for_conformance(&program, &requires.id, scenario).unwrap();
        assert!(matches!(reference.outcome, TraceOutcome::Failure { .. }));
        assert!(reference
            .events
            .iter()
            .all(|event| dictionary.ordinal_for(&event.event).is_some()));

        let checked = function(&program, "token.checked");
        let checked_dictionary = build_semantic_event_dictionary(&program, &checked.id).unwrap();
        assert!(checked_dictionary.entries().iter().any(|entry| {
            matches!(
                entry.event,
                TraceEventKind::SelectFailure {
                    ref status,
                    ..
                } if status == &NormalizedStatus::arithmetic(crate::cleanup_plan::StatusCase::AddOverflow)
            )
        }));
    }

    #[test]
    fn owned_transfer_selection_and_failed_postcondition_round_trip_exactly() {
        let program = program();
        let owned_type = DeclarationId::new("token.type");
        for function_id in ["token.identity", "token.choose-second"] {
            let function = function(&program, function_id);
            let dictionary = build_semantic_event_dictionary(&program, &function.id).unwrap();
            let reference = execute_for_conformance(
                &program,
                &function.id,
                CleanupScenario::new(
                    function_id,
                    Some(TraceResult::Owned {
                        type_id: owned_type.clone(),
                    }),
                ),
            )
            .unwrap();
            let ordinals = reference
                .events
                .iter()
                .map(|event| dictionary.ordinal_for(&event.event).unwrap())
                .collect::<Vec<_>>();
            let materialized = dictionary
                .materialize_trace(function_id, &ordinals, reference.outcome.clone())
                .unwrap();
            assert_eq!(materialized, reference);
            assert!(reference
                .events
                .iter()
                .any(|event| matches!(event.event, TraceEventKind::Transfer { .. })));
            assert!(reference
                .events
                .iter()
                .any(|event| matches!(event.event, TraceEventKind::ResultCommit { .. })));
        }

        let ensures = function(&program, "token.ensures-false");
        let source = ensures
            .cleanup_plan
            .status_sources
            .iter()
            .find(|source| {
                matches!(
                    source.producer,
                    StatusProducer::ContractFalse {
                        phase: ContractPhase::Ensures,
                        ..
                    }
                )
            })
            .unwrap();
        let mut scenario = CleanupScenario::new("ensures-false", None);
        scenario.booleans = BTreeMap::from([(source.id.expression.clone(), false)]);
        let reference = execute_for_conformance(&program, &ensures.id, scenario).unwrap();
        let dictionary = build_semantic_event_dictionary(&program, &ensures.id).unwrap();
        let ordinals = reference
            .events
            .iter()
            .map(|event| dictionary.ordinal_for(&event.event).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            dictionary
                .materialize_trace("ensures-false", &ordinals, reference.outcome.clone())
                .unwrap(),
            reference
        );
    }

    #[test]
    fn unknown_function_and_imported_lifecycle_fail_closed() {
        let program = program();
        assert!(
            build_semantic_event_dictionary(&program, &DeclarationId::new("missing.function"))
                .is_err()
        );

        let imported = r#"module test.imported_dictionary;
permit { file.release }
@id("file.type")
resource File { @id("file.drop") drop import "file.finalize"; }
@id("file.host")
interface FileHost permits { file.release } {
    @id("file.finalize")
    import fn finalize(file: own File) -> unit
        effects { file.release }
        failure infallible
        consumes file always;
}
@id("file.discard")
fn discard(value: own File) -> i64 uses { file.release } { 0 }
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let parsed = crate::parse(imported, Path::new("semantic-dictionary-imported.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        assert!(
            build_semantic_event_dictionary(&resolved, &DeclarationId::new("file.discard"))
                .is_err()
        );
    }

    #[test]
    fn typed_result_staging_is_rejected_before_callable_trace_admission() {
        let source = r#"module test.trace_try_closed;
@id("test.forward")
fn forward(value: Result<i64, bool>) -> Result<bool, bool> {
    let number = value?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let parsed = crate::parse(source, Path::new("semantic-dictionary-try.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let diagnostic =
            build_semantic_event_dictionary(&resolved, &DeclarationId::new("test.forward"))
                .unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert!(diagnostic
            .message
            .contains("copy-result staging is outside the direct-resource slice"));
    }
}
