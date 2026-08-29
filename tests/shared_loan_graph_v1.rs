use std::path::Path;

use semaprax::hir;
use semaprax::loan_plan::{LoanCause, LOAN_PLAN_SCHEMA_V1};
use semaprax::{graph, parse, verify};

const LOAN_SOURCE: &str = r#"
module test.shared_loan_graph_v1;

@id("loan.consume-bytes")
fn consume_bytes(value: own Bytes) -> i64 { 7 }

@id("loan.projected")
fn projected() -> i64 {
    let source = [7u8, 8u8, 9u8];
    let owned = bytes_copy(array_as_slice(source));
    let parent = bytes_as_slice(owned);
    let child = byte_range(parent, 1usize, byte_len(parent));
    let sibling = bytes_as_slice(owned);
    let byte_observed = if byte_len(child) + byte_len(sibling) > 0usize { 1 } else { 0 };
    consume_bytes(owned) + byte_observed
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

const LEGACY_V22_SOURCE: &str = r#"
module test.shared_loan_legacy_v22;

@id("legacy.choice")
variant Choice {
    @id("legacy.choice.none") None,
    @id("legacy.choice.data") Data {
        @id("legacy.choice.data.payload") payload: Bytes,
    },
}

@id("legacy.identity")
fn identity(value: own Choice) -> Choice { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn parsed(source: &str, name: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new(name)).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "unexpected diagnostics: {diagnostics:?}"
    );
    program
}

#[test]
fn graph_v23_is_selected_deterministically_and_serializes_exact_typed_loan_provenance() {
    let parsed = parsed(LOAN_SOURCE, "shared-loan-graph-v1.spx");
    let first = graph::to_json(&parsed).unwrap();
    assert_eq!(first, graph::to_json(&parsed).unwrap());
    assert!(first.starts_with("{\"schema\":\"semaprax.graph.v23\","));
    assert!(first.contains("\"kind\":\"loan_plan\",\"schema\":\"semaprax.loan-plan.v1\""));
    assert!(first.contains("\"kind\":\"place\""));
    assert!(first.contains("\"projections\":[]"));
    assert!(first.contains("\"kind\":\"slice_view\""));
    assert!(first.contains("\"kind\":\"loan_endpoint\""));
    assert!(first.contains("\"kind\":\"loan_edge\""));

    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    let projected = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "loan.projected")
        .unwrap();
    assert_eq!(projected.loan_plan.schema, LOAN_PLAN_SCHEMA_V1);
    assert!(projected.loan_plan.loans.len() >= 4);
    let slice_loans = projected
        .loan_plan
        .loans
        .iter()
        .filter(|loan| matches!(loan.cause, LoanCause::SliceView))
        .collect::<Vec<_>>();
    assert!(slice_loans.len() >= 3);
    assert!(slice_loans
        .iter()
        .all(|loan| loan.origin.projections.is_empty()));
    assert!(slice_loans.iter().any(|candidate| {
        slice_loans
            .iter()
            .filter(|loan| loan.origin.root == candidate.origin.root)
            .count()
            >= 3
    }));
    assert!(projected
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.parent.is_some()));

    let diagnostic = graph::reject_evidence_schema("semaprax.graph.v23").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-G410");
}

#[test]
fn a_no_loan_owned_variant_preserves_graph_v22_and_contains_no_v23_carrier() {
    let parsed = parsed(LEGACY_V22_SOURCE, "shared-loan-legacy-v22.spx");
    let first = graph::to_json(&parsed).unwrap();
    assert_eq!(first, graph::to_json(&parsed).unwrap());
    assert!(first.starts_with("{\"schema\":\"semaprax.graph.v22\","));
    assert!(!first.contains("semaprax.loan-plan.v1"));
    assert!(!first.contains("\"loans\":"));
}
