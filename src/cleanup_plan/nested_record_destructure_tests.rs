use std::path::Path;

use super::validate_structure;
use crate::cleanup_plan::{
    CleanupTransition, CLEANUP_PLAN_SCHEMA_V5, CLEANUP_PLAN_SCHEMA_V7, CLEANUP_PLAN_SCHEMA_V8,
};
use crate::{hir, parse};

const SOURCE: &str = r#"
module test.cleanup_nested_destructure;
@id("leaf.type") record Leaf {
  @id("leaf.payload") payload: Bytes,
  @id("leaf.marker") marker: i64,
}
@id("branch.type") record Branch {
  @id("branch.leaf") leaf: Leaf,
  @id("branch.enabled") enabled: bool,
}
@id("envelope.type") record Envelope {
  @id("envelope.left") left: Branch,
  @id("envelope.right") right: Branch,
  @id("envelope.sequence") sequence: i64,
}
@id("envelope.consume") fn consume(packet: own Envelope) -> i64 {
  match own packet {
    Envelope {
      left: Branch { leaf: Leaf { payload: left, marker: left_marker }, enabled: _ },
      right: Branch { leaf: Leaf { payload: right, marker: right_marker }, enabled: _ },
      sequence,
    } => left_marker + right_marker + sequence,
  }
}
@id("envelope.inspect") fn inspect(packet: borrow Envelope) -> i64 {
  match borrow packet {
    Envelope {
      left: Branch { leaf: Leaf { payload: left, marker: _ }, enabled: _ },
      right: Branch { leaf: Leaf { payload: right, marker: _ }, enabled: _ },
      sequence,
    } => sequence,
  }
}
@id("envelope.whole") fn whole(packet: own Envelope) -> Envelope { packet }
@id("app.main") fn main() -> i64 { 0 }
"#;

fn program() -> hir::ResolvedProgram {
    hir::resolve(
        &parse(SOURCE, Path::new("cleanup-nested-destructure.spx"))
            .expect("nested destructure source parses"),
    )
    .expect("nested destructure source resolves")
}

fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .expect("fixture function exists")
}

#[test]
fn v8_exact_destructure_transfers_recursive_leaves_atomically_in_declaration_order() {
    let program = program();
    let own = function(&program, "envelope.consume");
    let borrow = function(&program, "envelope.inspect");
    assert_eq!(own.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V8);
    assert_eq!(borrow.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V8);
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v28"
    );
    validate_structure(&program, own).expect("canonical owned v8 independently replays");
    validate_structure(&program, borrow).expect("canonical borrowed v8 independently replays");

    let transfer_blocks = own
        .cleanup_plan
        .blocks
        .iter()
        .filter_map(|block| {
            let transfers = block
                .transitions
                .iter()
                .filter_map(|transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination,
                        ..
                    } if source.projections.len() == 3 => {
                        Some((source.projections.clone(), destination.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            (!transfers.is_empty()).then_some(transfers)
        })
        .collect::<Vec<_>>();
    assert_eq!(transfer_blocks.len(), 1, "destructure is one atomic block");
    assert_eq!(
        transfer_blocks[0]
            .iter()
            .map(|(path, _)| path.iter().map(|id| id.as_str()).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![
            vec!["envelope.left", "branch.leaf", "leaf.payload"],
            vec!["envelope.right", "branch.leaf", "leaf.payload"],
        ]
    );
    assert!(borrow.cleanup_plan.blocks.iter().all(|block| block
        .transitions
        .iter()
        .all(|transition| !matches!(transition, CleanupTransition::Transfer { .. }))));

    let mut reordered = own.clone();
    let block = reordered
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .transitions
                .iter()
                .filter(|transition| matches!(transition, CleanupTransition::Transfer { source, .. } if source.projections.len() == 3))
                .count()
                == 2
        })
        .unwrap();
    let positions = block
        .transitions
        .iter()
        .enumerate()
        .filter_map(|(index, transition)| matches!(transition, CleanupTransition::Transfer { source, .. } if source.projections.len() == 3).then_some(index))
        .collect::<Vec<_>>();
    block.transitions.swap(positions[0], positions[1]);
    assert!(validate_structure(&program, &reordered).is_err());

    let mut downgraded = own.clone();
    downgraded.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V7;
    assert!(validate_structure(&program, &downgraded).is_err());
}

#[test]
fn whole_nested_move_and_flat_record_match_preserve_legacy_schemas() {
    let program = program();
    assert_eq!(
        function(&program, "envelope.whole").cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V7
    );

    let flat = r#"
module test.cleanup_flat_destructure;
@id("packet.type") record Packet { @id("packet.payload") payload: Bytes, }
@id("packet.take") fn take(packet: own Packet) -> i64 {
  match own packet { Packet { payload } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let flat = hir::resolve(
        &parse(flat, Path::new("cleanup-flat-destructure.spx")).expect("flat source parses"),
    )
    .expect("flat source resolves");
    assert_eq!(
        function(&flat, "packet.take").cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V5
    );
}
