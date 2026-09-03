use std::path::Path;

use crate::cleanup_plan::{CleanupEdge, CleanupTerminator, EdgeCondition, EdgeId, ExitTarget};
use crate::hir::{self, ResolvedProgram};
use crate::parse;

use super::*;

const SOURCE: &str = r#"module test.native_trace_capacity;
permit { io.release }

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("file.type")
resource File {
    @id("file.drop")
    drop import "file.finalize";
}

@id("file.host")
interface FileHost permits { io.release } {
    @id("file.finalize")
    import fn finalize(file: own File) -> unit
        effects { io.release }
        failure infallible
        consumes file always;
}

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("file.discard")
fn discard_file(value: own File) -> i64 uses { io.release } { 0 }

@id("token.choose")
fn choose(condition: bool, left: own Token, right: own Token) -> Token {
    if condition { left } else { right }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("native-trace-capacity.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function_mut<'a>(program: &'a mut ResolvedProgram, id: &str) -> &'a mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

#[test]
fn reverse_trivial_finalizers_and_imported_finalizers_have_exact_weights() {
    let program = program();
    let discard_two = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let discard_file = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "file.discard")
        .unwrap();

    // Two reverse-order trivial finalizers (2 + 2), then result commit (1).
    assert_eq!(required_event_capacity(&program, discard_two).unwrap(), 5);
    // Imported finalization (4), then result commit (1).
    assert_eq!(required_event_capacity(&program, discard_file).unwrap(), 5);
}

#[test]
fn branch_capacity_uses_the_longest_path_instead_of_summing_paths() {
    let program = program();
    let choose = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.choose")
        .unwrap();
    let branch = choose
        .cleanup_plan
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            CleanupTerminator::Branch(edges) => Some(edges.clone()),
            CleanupTerminator::Goto(_) | CleanupTerminator::Exit(_) => None,
        })
        .unwrap();
    assert_eq!(branch.len(), 2);
    // The attached plan's longest arm, including every guarded finalizer
    // that might emit on that arm, requires nine events. Summing the two
    // mutually exclusive arms would exceed this conservative bound.
    assert_eq!(required_event_capacity(&program, choose).unwrap(), 9);
}

#[test]
fn deep_linear_plan_uses_an_explicit_traversal_stack() {
    const DEPTH: u32 = 20_000;

    let mut program = program();
    let function = function_mut(&mut program, "app.main");
    let region = function.cleanup_plan.regions[0].id;
    let exit_id = function.cleanup_plan.regions[0].normal_scope_end;
    let continuation = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| exit.id == exit_id)
        .unwrap()
        .continuation
        .clone();
    function.cleanup_plan.blocks = (0..=DEPTH)
        .map(|index| CleanupBlock {
            id: BlockId(index),
            region,
            transitions: Vec::new(),
            terminator: if index == DEPTH {
                CleanupTerminator::Exit(exit_id)
            } else {
                CleanupTerminator::Goto(EdgeId(index))
            },
        })
        .collect();
    function.cleanup_plan.edges = (0..DEPTH)
        .map(|index| CleanupEdge {
            id: EdgeId(index),
            from: BlockId(index),
            to: BlockId(index + 1),
            condition: EdgeCondition::Always,
        })
        .collect();
    function.cleanup_plan.exits = vec![ExitTarget {
        id: exit_id,
        from: BlockId(DEPTH),
        leaves_regions: vec![region],
        finalize_in_order: Vec::new(),
        continuation,
    }];
    function.cleanup_plan.entry = BlockId(0);

    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(required_event_capacity(&program, function).unwrap(), 1);
}

#[test]
fn hostile_missing_edge_and_reachable_cycle_are_rejected() {
    let mut missing = program();
    let function = function_mut(&mut missing, "token.discard-two");
    function.cleanup_plan.blocks[0].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX));
    let function = missing
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let error = required_event_capacity(&missing, function).unwrap_err();
    assert_eq!(error.code, "SPX-B103");
    assert!(error.message.contains("missing edge"));

    let mut cyclic = program();
    let function = function_mut(&mut cyclic, "token.choose");
    let entry = function.cleanup_plan.entry;
    let edge_id = match &function
        .cleanup_plan
        .blocks
        .iter()
        .find(|block| block.id == entry)
        .unwrap()
        .terminator
    {
        CleanupTerminator::Goto(edge) => *edge,
        CleanupTerminator::Branch(edges) => edges[0],
        CleanupTerminator::Exit(_) => panic!("fixture entry must continue"),
    };
    function
        .cleanup_plan
        .edges
        .iter_mut()
        .find(|edge| edge.id == edge_id)
        .unwrap()
        .to = entry;
    let function = cyclic
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.choose")
        .unwrap();
    let error = required_event_capacity(&cyclic, function).unwrap_err();
    assert!(error.message.contains("reachable cleanup cycle"));
}

#[test]
fn hostile_lifecycle_reference_and_capacity_overflow_are_rejected() {
    let mut program = program();
    let function = function_mut(&mut program, "token.discard-two");
    function.cleanup_plan.exits[0].finalize_in_order[0].lifecycle_id =
        DeclarationId::new("missing.lifecycle");
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let error = required_event_capacity(&program, function).unwrap_err();
    assert!(error.message.contains("unknown lifecycle"));

    let error = checked_add(u32::MAX, 1, "hostile capacity").unwrap_err();
    assert_eq!(error.message, "hostile capacity exceeds u32");
}
