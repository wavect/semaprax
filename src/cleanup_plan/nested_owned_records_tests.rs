use std::path::Path;

use super::validate_structure;
use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::{CleanupTerminator, CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V7};
use crate::{hir, parse};

const NESTED_SOURCE: &str = r#"
module test.cleanup_nested_owned;
@id("inner.type")
record Inner {
  @id("inner.left") left: Bytes,
  @id("inner.marker") marker: i64,
  @id("inner.right") right: Bytes,
}
@id("outer.type")
record Outer {
  @id("outer.inner") inner: Inner,
  @id("outer.trailing") trailing: Bytes,
}
@id("outer.consume") fn consume(value: own Outer) -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#;

fn nested_program() -> hir::ResolvedProgram {
    hir::resolve(
        &parse(NESTED_SOURCE, Path::new("cleanup-nested-owned-records.spx"))
            .expect("nested source parses"),
    )
    .expect("nested source resolves")
}

#[test]
fn v7_replays_full_paths_and_finalizes_parameter_leaves_in_reverse_order() {
    let program = nested_program();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "outer.consume")
        .unwrap();
    assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V7);
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v26"
    );
    validate_structure(&program, function).expect("canonical v7 plan independently replays");
    let cleanup_json = crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan);
    assert!(cleanup_json.contains("\"schema\":\"semaprax.cleanup-plan.v7\""));
    assert!(cleanup_json.contains("\"projections\":[\"outer.inner\",\"inner.left\"]"));

    let inventory_paths = function
        .cleanup
        .flags
        .iter()
        .map(|flag| {
            flag.place
                .projections
                .iter()
                .map(|projection| projection.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        inventory_paths,
        vec![
            vec!["outer.inner", "inner.left"],
            vec!["outer.inner", "inner.right"],
            vec!["outer.trailing"],
        ]
    );

    let terminal = function
        .cleanup_plan
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            CleanupTerminator::Exit(exit) => {
                function.cleanup_plan.exits.iter().find(|candidate| {
                    candidate.id == *exit && candidate.finalize_in_order.len() == 3
                })
            }
            _ => None,
        })
        .expect("parameter cleanup exit exists");
    assert_eq!(
        terminal
            .finalize_in_order
            .iter()
            .map(|action| action.guard_flag.0)
            .collect::<Vec<_>>(),
        vec![2, 1, 0]
    );
}

#[test]
fn replay_rejects_schema_downgrade_and_coordinated_shape_reordering() {
    let program = nested_program();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "outer.consume")
        .unwrap();

    let mut downgraded = function.clone();
    downgraded.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6;
    assert!(validate_structure(&program, &downgraded).is_err());

    let mut reordered = function.clone();
    for shape in [
        &mut reordered.cleanup.slots[0].shape,
        &mut reordered.cleanup_plan.slots[0].field_liveness_shape,
    ] {
        let FieldLivenessShape::Record { fields, .. } = shape else {
            panic!("outer shape remains a record")
        };
        let FieldLivenessShape::Record { fields, .. } = &mut fields[0].shape else {
            panic!("inner shape remains a record")
        };
        fields.swap(0, 2);
    }
    assert!(validate_structure(&program, &reordered).is_err());
}

#[test]
fn flat_owned_record_keeps_legacy_cleanup_and_graph_selection() {
    let source = r#"
module test.cleanup_flat_owned;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.consume") fn consume(value: own Packet) -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(
        &parse(source, Path::new("cleanup-flat-owned-record.spx")).expect("flat source parses"),
    )
    .expect("flat source resolves");
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "packet.consume")
        .unwrap();
    assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V2);
    // `Bytes` carries portable usize and byte-view facts, so the legacy
    // selection for a flat owned record is v17, not the v10 floor. The point
    // is that it stays below the nested owned-record schema.
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v17"
    );
}

#[test]
fn v27_requires_an_authenticated_nested_projected_loan() {
    let source = NESTED_SOURCE.replace(
        "@id(\"app.main\")",
        r#"@id("outer.inspect")
fn inspect(value: own Outer) -> usize {
  let view = bytes_as_slice(value.inner.left);
  byte_len(view)
}
@id("app.main")"#,
    );
    let mut program = hir::resolve(
        &parse(&source, Path::new("cleanup-nested-owned-loan.spx"))
            .expect("nested loan source parses"),
    )
    .expect("nested loan source resolves");
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v27"
    );

    for function in &mut program.functions {
        for loan in &mut function.loan_plan.loans {
            loan.origin.projections.clear();
        }
    }
    let error = crate::graph::graph_schema(&program).unwrap_err();
    assert_eq!(error.code, "SPX-G410");
}

#[test]
fn native_v25_cannot_mask_nested_v26_at_the_direct_classifier() {
    let mut nested = nested_program();
    let native_source = r#"
module test.cleanup_nested_native;
@id("native.host") interface Host permits {} {
  @id("native.ping") import rust fn ping(value: i64) -> i64
    effects {} failure infallible;
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let native = hir::resolve(
        &parse(native_source, Path::new("cleanup-nested-native.spx"))
            .expect("native source parses"),
    )
    .expect("native source resolves");
    nested.interfaces = native.interfaces;
    let error = crate::graph::graph_schema(&nested).unwrap_err();
    assert_eq!(error.code, "SPX-G410");
}
