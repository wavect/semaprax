use semaprax::interpreter::{
    evaluate_resolved_owned_data, OwnedDataCleanupEvent, OwnedDataEvaluationOutcome,
    OwnedDataValue, DEFAULT_MAX_STEPS,
};
use semaprax::{hir, parse, verify};

const SOURCE: &str = r#"
module owned_data.interpreter;

@id("owned.direct")
fn direct(input: borrow Slice<u8>) -> Bytes {
    bytes_copy(input)
}

@id("owned.maybe")
fn maybe(input: borrow Slice<u8>) -> Option<Bytes> {
    if byte_len(input) == 0usize {
        Option<Bytes>::None {}
    } else {
        Option<Bytes>::Some { value: bytes_copy(input) }
    }
}

@id("owned.result")
fn result_value(input: borrow Slice<u8>) -> Result<Bytes, i64> {
    if byte_len(input) == 0usize {
        Result<Bytes, i64>::Err { error: -7 }
    } else {
        Result<Bytes, i64>::Ok { value: bytes_copy(input) }
    }
}

@id("owned.failure")
fn failure(input: borrow Slice<u8>) -> Bytes requires false {
    bytes_copy(input)
}

@id("owned.main")
fn main() -> i64 { 0 }
"#;

fn program() -> hir::ResolvedProgram {
    let ast = parse(SOURCE, "owned-data-interpreter-v1.spx").unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    program
}

fn evaluate(function: &str, input: &[u8]) -> semaprax::interpreter::OwnedDataEvaluation {
    evaluate_resolved_owned_data(&program(), function, input, DEFAULT_MAX_STEPS).unwrap()
}

#[test]
fn direct_option_and_result_copy_out_have_exact_normalized_settlement() {
    let direct = evaluate("owned.direct", &[0, 0xff, 7]);
    assert_eq!(direct.function_id.as_str(), "owned.direct");
    assert_eq!(
        direct.outcome,
        OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(vec![0, 0xff, 7]))
    );
    assert_eq!(
        direct.cleanup_events,
        [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
    );

    let none = evaluate("owned.maybe", &[]);
    assert_eq!(
        none.outcome,
        OwnedDataEvaluationOutcome::Returned(OwnedDataValue::OptionBytes(None))
    );
    assert!(none.cleanup_events.is_empty());

    let some = evaluate("owned.maybe", &[1]);
    assert_eq!(
        some.outcome,
        OwnedDataEvaluationOutcome::Returned(OwnedDataValue::OptionBytes(Some(vec![1])))
    );
    assert_eq!(
        some.cleanup_events,
        [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
    );

    let err = evaluate("owned.result", &[]);
    assert_eq!(
        err.outcome,
        OwnedDataEvaluationOutcome::Returned(OwnedDataValue::ResultBytesI64(Err(-7)))
    );
    assert!(err.cleanup_events.is_empty());

    let ok = evaluate("owned.result", &[2, 3]);
    assert_eq!(
        ok.outcome,
        OwnedDataEvaluationOutcome::Returned(OwnedDataValue::ResultBytesI64(Ok(vec![2, 3])))
    );
    assert_eq!(
        ok.cleanup_events,
        [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
    );
}

#[test]
fn language_failure_is_not_a_returned_variant_and_publishes_no_cleanup() {
    let failure = evaluate("owned.failure", &[9]);
    assert!(matches!(
        failure.outcome,
        OwnedDataEvaluationOutcome::LanguageFailure(_)
    ));
    assert!(failure.cleanup_events.is_empty());
}

#[test]
fn boundary_rejects_wrong_signature_and_oversize_input_before_evaluation() {
    let program = program();
    assert!(evaluate_resolved_owned_data(&program, "owned.main", &[], DEFAULT_MAX_STEPS).is_err());
    let oversized = vec![0; 65_537];
    assert!(
        evaluate_resolved_owned_data(&program, "owned.direct", &oversized, DEFAULT_MAX_STEPS,)
            .is_err()
    );
}
