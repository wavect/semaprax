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
        build_semantic_event_dictionary(&program, &DeclarationId::new("token.identity")).unwrap();
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
        build_semantic_event_dictionary(&program, &DeclarationId::new("missing.function")).is_err()
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
        build_semantic_event_dictionary(&resolved, &DeclarationId::new("file.discard")).is_err()
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
