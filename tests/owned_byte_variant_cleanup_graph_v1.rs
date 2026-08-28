use std::path::Path;

use semaprax::cleanup::{FieldLivenessShape, CLEANUP_INVENTORY_SCHEMA_V2};
use semaprax::cleanup_plan::{CleanupTransition, CLEANUP_PLAN_SCHEMA_V6};
use semaprax::hir::{self, ResolvedProgram};
use semaprax::{graph, parse, verify};
use sha2::{Digest, Sha256};

const SOURCE: &str = r#"
module test.owned_variant_cleanup_graph;

@id("sum.choice")
variant Choice {
    @id("sum.choice.none") None,
    @id("sum.choice.data") Data {
        @id("sum.choice.data.payload") payload: Bytes,
        @id("sum.choice.data.marker") marker: i64,
    },
    @id("sum.choice.error") Error {
        @id("sum.choice.error.code") code: i64,
    },
}

@id("sum.make")
fn make(data: borrow Slice<u8>) -> Choice {
    Choice::Data { payload: bytes_copy(data), marker: 20 }
}

@id("sum.identity")
fn identity(value: own Choice) -> Choice { value }

@id("sum.inspect")
fn inspect(value: borrow Choice) -> i64 {
    match borrow value {
        Choice::None {} => 0,
        Choice::Data { payload, marker } => marker,
        Choice::Error { code } => code,
    }
}

@id("sum.consume")
fn consume(value: own Choice) -> i64 {
    match own value {
        Choice::None {} => 0,
        Choice::Data { payload, marker } => marker,
        Choice::Error { code } => code,
    }
}

@id("sum.guarded")
fn guarded(value: own Choice) -> i64
requires false
{
    consume(value)
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn parsed() -> semaprax::ast::Program {
    parse(SOURCE, Path::new("owned-byte-variant-cleanup-graph-v1.spx")).unwrap()
}

fn resolved() -> ResolvedProgram {
    let parsed = parsed();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "unexpected diagnostics: {diagnostics:?}"
    );
    let program = hir::resolve(&parsed).unwrap();
    hir::validate(&program).unwrap();
    program
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function {id}"))
}

fn digest(text: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

#[test]
fn inventory_v2_authenticates_case_qualified_liveness_and_conditional_entry() {
    let program = resolved();
    let consume = function(&program, "sum.consume");
    assert_eq!(consume.cleanup.schema, CLEANUP_INVENTORY_SCHEMA_V2);
    let entry = &consume.cleanup.entry_state.conditional_owned_parameters;
    assert_eq!(entry.len(), 1);
    assert!(consume.cleanup.entry_state.live_owned_parameters.is_empty());
    assert_eq!(entry[0].variant.as_str(), "sum.choice");
    assert_eq!(
        entry[0]
            .cases
            .iter()
            .map(|case| (case.case.as_str(), case.live_flags.len()))
            .collect::<Vec<_>>(),
        [
            ("sum.choice.none", 0),
            ("sum.choice.data", 1),
            ("sum.choice.error", 0),
        ]
    );
    let FieldLivenessShape::Variant { declaration, cases } = &consume.cleanup.slots[0].shape else {
        panic!("owned Choice parameter must retain a conditional variant shape")
    };
    assert_eq!(declaration.as_str(), "sum.choice");
    assert_eq!(cases.len(), 3);
    assert_eq!(cases[1].case.as_str(), "sum.choice.data");
    assert_eq!(cases[1].fields[0].field.as_str(), "sum.choice.data.payload");
    let live = entry[0].cases[1].live_flags[0];
    assert_eq!(
        consume.cleanup.flags[live.0 as usize]
            .place
            .projections
            .iter()
            .map(|projection| projection.as_str())
            .collect::<Vec<_>>(),
        ["sum.choice.data", "sum.choice.data.payload"]
    );
}

#[test]
fn plan_v6_uses_explicit_case_authentication_and_conditional_whole_transfer() {
    let program = resolved();
    let consume = function(&program, "sum.consume");
    assert_eq!(consume.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V6);
    let transitions = consume
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .collect::<Vec<_>>();
    assert_eq!(
        transitions
            .iter()
            .filter(|transition| matches!(
                transition,
                CleanupTransition::AuthenticateVariantCase { .. }
            ))
            .count(),
        3
    );

    let identity = function(&program, "sum.identity");
    assert_eq!(identity.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V6);
    assert!(identity
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| matches!(transition, CleanupTransition::TransferVariant { variant, .. } if variant.as_str() == "sum.choice")));

    let inspect = function(&program, "sum.inspect");
    assert!(inspect
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .all(|transition| !matches!(transition, CleanupTransition::TransferVariant { .. })));
}

#[test]
fn failure_cleanup_is_guarded_by_the_exact_payload_case() {
    let program = resolved();
    let guarded = function(&program, "sum.guarded");
    let guarded_finalizers = guarded
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .filter_map(|action| action.active_case.as_ref())
        .collect::<Vec<_>>();
    assert!(!guarded_finalizers.is_empty());
    assert!(guarded_finalizers.iter().all(|guard| {
        guard.variant.as_str() == "sum.choice" && guard.case.as_str() == "sum.choice.data"
    }));
}

#[test]
fn replay_rejects_conditional_entry_and_finalizer_case_forgery() {
    let mut program = resolved();
    let consume = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "sum.consume")
        .unwrap();
    consume
        .cleanup_plan
        .entry_state
        .conditional_owned_parameters[0]
        .cases
        .swap(0, 1);
    assert!(hir::validate(&program).is_err());

    let mut program = resolved();
    let guarded = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "sum.guarded")
        .unwrap();
    let action = guarded
        .cleanup_plan
        .exits
        .iter_mut()
        .flat_map(|exit| &mut exit.finalize_in_order)
        .find(|action| action.active_case.is_some())
        .expect("guarded fixture must contain a conditional finalizer");
    action.active_case.as_mut().unwrap().case = hir::DeclarationId::new("sum.choice.error");
    assert!(hir::validate(&program).is_err());
}

#[test]
fn graph_v22_is_pinned_and_legacy_evidence_consumers_fail_closed() {
    let json = graph::to_json(&parsed()).unwrap();
    assert!(json.starts_with("{\"schema\":\"semaprax.graph.v22\","));
    assert!(json.contains("\"schema\":\"semaprax.cleanup-plan.v6\""));
    assert!(json.contains("\"kind\":\"variant\",\"declaration\":\"sum.choice\""));
    assert!(json.contains("\"ownership_mode\":\"own\""));
    assert!(json.contains("\"ownership_mode\":\"borrow\""));
    assert_eq!(
        digest(&json),
        "5616b356183c3d8cd6788144c977ac68147d292b5831ffa132d080dbaa0c028b"
    );

    let diagnostic = graph::reject_evidence_schema("semaprax.graph.v22").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-G410");
    assert!(diagnostic.message.contains("owned variant"));
}
