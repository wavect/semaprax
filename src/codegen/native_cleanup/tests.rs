use std::path::Path;

use crate::cleanup_plan::ExitContinuation;
use crate::hir::{self, DeclarationId, OwnershipMode, ResolvedType};
use crate::parse;

use super::*;

const SUPPORTED: &str = r#"module test.native_cleanup_index;

@id("token.type")
resource Token {
@id("token.drop")
drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.contract-failure")
fn contract_failure(value: own Token) -> i64 requires false { 0 }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64 requires number >= 0 { number + 1 }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolve(source: &str) -> ResolvedProgram {
    let parsed = parse(source, Path::new("native-cleanup-index.spx")).unwrap();
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
fn projected_borrow_shape_gate_rejects_wrong_operation_and_depth() {
    let field = crate::hir::PlaceProjection::Field(DeclarationId::new("packet.payload"));
    let exact = crate::hir::Place {
        root: crate::hir::ValueId::intrinsic_parameter("packet", 0),
        projections: vec![field.clone()],
    };
    assert!(borrow_place_shape_is_admitted(
        &DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        &exact,
    ));
    assert!(!borrow_place_shape_is_admitted(
        &DeclarationId::new(crate::byte_ops::ARRAY_AS_SLICE_ID),
        &exact,
    ));
    let deeper = crate::hir::Place {
        root: exact.root,
        projections: vec![field.clone(), field],
    };
    assert!(!borrow_place_shape_is_admitted(
        &DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        &deeper,
    ));
}

#[test]
fn supported_direct_resource_indexes_preserve_exact_order() {
    let first = resolve(SUPPORTED);
    let second = resolve(SUPPORTED);

    for id in [
        "token.discard",
        "token.discard-two",
        "token.contract-failure",
        "token.checked",
    ] {
        let first_index = classify(&first, function(&first, id)).unwrap();
        let second_index = classify(&second, function(&second, id)).unwrap();
        assert_eq!(first_index.function_id.as_str(), id);
        assert_eq!(second_index.function_id.as_str(), id);
        assert_eq!(first_index.entry, second_index.entry);
        assert_eq!(first_index.slots, second_index.slots);
        assert_eq!(first_index.leaves, second_index.leaves);
        assert_eq!(first_index.blocks, second_index.blocks);
        assert_eq!(first_index.edges, second_index.edges);
        assert_eq!(first_index.exits, second_index.exits);
    }

    let discard = classify(&first, function(&first, "token.discard")).unwrap();
    assert_eq!(discard.slots.len(), 1);
    assert_eq!(discard.live_owned_parameters.len(), 1);
    assert_eq!(discard.leaves[0].flag, LivenessFlagId(0));
    assert_eq!(discard.leaves[0].lifecycle_id.as_str(), "token.drop");
    assert_eq!(
        discard.slot(&discard.leaves[0].place.storage),
        Some(&discard.slots[0])
    );
    assert_eq!(discard.leaf(LivenessFlagId(0)), Some(&discard.leaves[0]));
    assert!(discard.block(discard.entry).is_some());
    assert!(discard
        .edges
        .iter()
        .all(|edge| discard.edge(edge.id).is_some()));
    assert!(discard
        .exits
        .iter()
        .all(|exit| discard.exit(exit.exit.id).is_some()));

    let two = classify(&first, function(&first, "token.discard-two")).unwrap();
    let success = two
        .exits
        .iter()
        .find(|exit| {
            matches!(
                exit.exit.continuation,
                ExitContinuation::CommitResult { .. }
            )
        })
        .unwrap();
    assert_eq!(
        success
            .finalizers
            .iter()
            .map(|action| action.guard_flag)
            .collect::<Vec<_>>(),
        [LivenessFlagId(1), LivenessFlagId(0)]
    );

    let failure = classify(&first, function(&first, "token.contract-failure")).unwrap();
    assert!(failure.status_sources.iter().any(|source| {
        matches!(
            source.producer,
            crate::cleanup_plan::StatusProducer::ContractFalse { .. }
        )
    }));
    assert!(failure
        .exits
        .iter()
        .filter(|exit| {
            matches!(
                exit.exit.continuation,
                ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnFailure { .. }
            )
        })
        .all(|exit| {
            exit.finalizers
                .iter()
                .map(|action| action.guard_flag)
                .eq([LivenessFlagId(0)])
        }));

    let checked = classify(&first, function(&first, "token.checked")).unwrap();
    assert!(checked
        .blocks
        .iter()
        .any(|block| matches!(block.block.terminator, CleanupTerminator::Branch(_))));
    assert!(checked.status_sources.iter().any(|source| matches!(
        source.producer,
        crate::cleanup_plan::StatusProducer::CheckedArithmetic { .. }
    )));
    assert!(checked.status_sources.iter().any(|source| matches!(
        source.producer,
        crate::cleanup_plan::StatusProducer::ContractFalse { .. }
    )));
}

#[test]
fn conditional_and_lazy_control_flow_are_rejected_without_reconstruction() {
    let conditional = resolve(
        r#"module test.native_cleanup_if;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("token.choose") fn choose(value: own Token, condition: bool) -> i64 {
if condition { 1 } else { 0 }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = classify(&conditional, function(&conditional, "token.choose")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("conditional expression"));

    let lazy = resolve(
        r#"module test.native_cleanup_lazy;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("token.lazy") fn lazy(value: own Token, condition: bool) -> bool {
condition && true
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = classify(&lazy, function(&lazy, "token.lazy")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("lazy boolean expression"));
}

#[test]
fn resource_valued_binary_operands_are_rejected_even_in_hostile_hir() {
    let program = resolve(SUPPORTED);
    let mut hostile = function(&program, "token.discard").clone();
    let parameter = &hostile.params[0];
    let operand = ResolvedExpr {
        id: hostile.body.id.clone(),
        ty: parameter.ty.clone(),
        ownership: OwnershipMode::Borrow,
        kind: ResolvedExprKind::Place(crate::hir::Place {
            root: parameter.id.clone(),
            projections: Vec::new(),
        }),
        span: hostile.body.span,
    };
    hostile.body.ty = ResolvedType::Bool;
    hostile.body.ownership = OwnershipMode::Value;
    hostile.body.kind = ResolvedExprKind::Binary {
        op: BinaryOp::Eq,
        left: Box::new(operand.clone()),
        right: Box::new(operand),
    };

    let diagnostic = classify(&program, &hostile).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic
        .message
        .contains("resource-valued binary operands"));
}

#[test]
fn initialize_and_cleanup_bearing_continue_are_rejected_by_the_classifier() {
    let program = resolve(SUPPORTED);
    let mut initialize = function(&program, "token.discard").clone();
    initialize.cleanup_plan.blocks[0]
        .transitions
        .push(CleanupTransition::Initialize {
            at: initialize.body.id.clone(),
            destination: initialize.cleanup_plan.entry_state.live_owned_parameters[0].clone(),
        });
    let diagnostic = classify(&program, &initialize).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("initialize transition"));
    assert!(diagnostic.message.contains("physical payload source"));

    let mut continuation = function(&program, "token.contract-failure").clone();
    let continue_position = continuation
        .cleanup_plan
        .exits
        .iter()
        .position(|exit| matches!(exit.continuation, ExitContinuation::Continue(_)))
        .expect("compiler contract continuation");
    let finalizer = continuation
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .next()
        .expect("terminal cleanup")
        .clone();
    continuation.cleanup_plan.exits[continue_position]
        .finalize_in_order
        .push(finalizer);
    let diagnostic = classify(&program, &continuation).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("performs finalization"));
    assert!(diagnostic.message.contains("canonical empty-region"));

    let mut conditional = function(&program, "token.contract-failure").clone();
    let continuation_edge = conditional
        .cleanup_plan
        .exits
        .iter()
        .find_map(|exit| match exit.continuation {
            ExitContinuation::Continue(edge) => Some(edge),
            _ => None,
        })
        .expect("compiler contract continuation");
    let hostile_condition = conditional
        .cleanup_plan
        .edges
        .iter()
        .find(|edge| !matches!(edge.condition, EdgeCondition::Always))
        .expect("contract branch")
        .condition
        .clone();
    conditional
        .cleanup_plan
        .edges
        .iter_mut()
        .find(|edge| edge.id == continuation_edge)
        .expect("continuation edge")
        .condition = hostile_condition;
    let diagnostic = classify(&program, &conditional).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic
        .message
        .contains("does not own one unconditional edge"));
}

#[test]
fn records_are_rejected_precisely() {
    let program = resolve(
        r#"module test.native_cleanup_record;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("box.type") record Box { @id("box.value") value: Token, }
@id("box.discard") fn discard(value: own Box) -> i64 { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = classify(&program, function(&program, "box.discard")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert_eq!(
        diagnostic.message,
        "native cleanup first slice for function `box.discard` does not support record declaration `box.type`"
    );
}

#[test]
fn imported_lifecycles_are_rejected_precisely() {
    let program = resolve(
        r#"module test.native_cleanup_import;
permit { io.release }
@id("file.type") resource File { @id("file.drop") drop import "file.finalize"; }
@id("file.host") interface FileHost permits { io.release } {
@id("file.finalize") import fn finalize(file: own File) -> unit
    effects { io.release } failure infallible consumes file always;
}
@id("file.discard") fn discard(value: own File) -> i64 uses { io.release } { 0 }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = classify(&program, function(&program, "file.discard")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic
        .message
        .contains("imported lifecycle `file.drop`"));
    assert!(diagnostic.message.contains("`file.finalize`"));
}

#[test]
fn resource_bearing_calls_are_rejected_precisely() {
    let program = resolve(
        r#"module test.native_cleanup_call;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("token.consume") fn consume(value: own Token) -> i64 { 0 }
@id("token.forward") fn forward(value: own Token) -> i64 { consume(value) }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = classify(&program, function(&program, "token.forward")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("call execution"));
    assert!(diagnostic.message.contains("`token.consume`"));
    assert!(diagnostic.message.contains("single-frame"));
}

#[test]
fn scalar_calls_from_resource_owning_functions_are_rejected_precisely() {
    let program = resolve(
        r#"module test.native_cleanup_scalar_call;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("scalar.helper") fn helper() -> i64 { 7 }
@id("token.holding") fn holding(value: own Token) -> i64 { helper() }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let diagnostic = classify(&program, function(&program, "token.holding")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("call execution"));
    assert!(diagnostic.message.contains("`scalar.helper`"));
    assert!(diagnostic.message.contains("single-frame"));
}

#[test]
fn empty_call_commit_transitions_are_rejected_without_repair() {
    let program = resolve(SUPPORTED);
    let mut hostile = function(&program, "token.discard").clone();
    let hostile_call = hostile.body.id.clone();
    hostile.cleanup_plan.blocks[0]
        .transitions
        .push(CleanupTransition::CallCommit {
            call: hostile_call.clone(),
            arguments: Vec::new(),
        });

    let diagnostic = classify(&program, &hostile).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic
        .message
        .contains(&format!("call-commit transition `{hostile_call}`")));
    assert!(diagnostic.message.contains("single-frame"));
}

#[test]
fn projected_cleanup_places_are_rejected_without_repair() {
    let program = resolve(SUPPORTED);
    let mut hostile = function(&program, "token.discard").clone();
    hostile.cleanup_plan.entry_state.live_owned_parameters[0]
        .projections
        .push(DeclarationId::new("hostile.field"));

    let diagnostic = classify(&program, &hostile).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic
        .message
        .contains("owned entry place uses field projections"));
}

#[test]
fn generic_cleanup_slots_are_rejected_without_repair() {
    let program = resolve(SUPPORTED);
    let mut hostile = function(&program, "token.discard").clone();
    hostile.cleanup_plan.slots[0].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("token.type"),
        arguments: vec![ResolvedType::I64],
    };

    let diagnostic = classify(&program, &hostile).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("generic nominal type"));
}

#[test]
fn forged_slot_lifecycle_and_transfer_type_mismatches_are_rejected() {
    let program = resolve(
        r#"module test.native_cleanup_type_identity;
@id("alpha.type") resource Alpha { @id("alpha.drop") drop trivial; }
@id("beta.type") resource Beta { @id("beta.drop") drop trivial; }
@id("alpha.identity") fn identity(value: own Alpha) -> Alpha { value }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let original = function(&program, "alpha.identity");

    let mut lifecycle_mismatch = original.clone();
    let FieldLivenessShape::Leaf { lifecycle, .. } =
        &mut lifecycle_mismatch.cleanup_plan.slots[0].field_liveness_shape
    else {
        panic!("direct resource slot must have one leaf");
    };
    *lifecycle = DeclarationId::new("beta.drop");
    let diagnostic = classify(&program, &lifecycle_mismatch).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("lifecycle `beta.drop`"));
    assert!(diagnostic.message.contains("lifecycle `alpha.drop`"));

    let mut transfer_mismatch = original.clone();
    let temporary = transfer_mismatch
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|slot| matches!(slot.storage, StorageId::Temporary(_)))
        .expect("owned identity has body temporary storage");
    temporary.ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("beta.type"),
        arguments: Vec::new(),
    };
    let FieldLivenessShape::Leaf { lifecycle, .. } = &mut temporary.field_liveness_shape else {
        panic!("direct resource temporary must have one leaf");
    };
    *lifecycle = DeclarationId::new("beta.drop");
    let diagnostic = classify(&program, &transfer_mismatch).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert!(diagnostic.message.contains("changes resource type"));
    assert!(diagnostic.message.contains("alpha.type"));
    assert!(diagnostic.message.contains("beta.type"));
}
