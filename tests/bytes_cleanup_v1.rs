use std::path::Path;

use semaprax::cleanup::FieldLivenessShape;
use semaprax::cleanup_plan::{CleanupResultSource, ExitContinuation, StatusProducer};
use semaprax::hir::{self, DeclarationId, ResolvedProgram};
use semaprax::parse;

const SOURCE: &str = r#"
module test.bytes_cleanup;

@id("bytes.make")
fn make(value: borrow Slice<u8>) -> Bytes {
    bytes_copy(value)
}

@id("bytes.forward")
fn forward(value: own Bytes) -> Bytes {
    value
}

@id("bytes.consume")
fn consume(value: own Bytes) -> i64 {
    1
}

@id("bytes.copy-consume")
fn copy_consume(value: borrow Slice<u8>) -> i64 {
    consume(bytes_copy(value))
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn resolved() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("bytes-cleanup.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

#[test]
fn bytes_is_exactly_one_compiler_owned_cleanup_leaf_in_every_storage_epoch() {
    let program = resolved();
    for id in [
        "bytes.make",
        "bytes.forward",
        "bytes.consume",
        "bytes.copy-consume",
    ] {
        let function = function(&program, id);
        let mut inventory_bytes_slots = 0usize;
        for slot in &function.cleanup.slots {
            if slot.ty != hir::ResolvedType::Bytes {
                continue;
            }
            inventory_bytes_slots += 1;
            let FieldLivenessShape::Leaf { lifecycle, .. } = &slot.shape else {
                panic!("Bytes inventory slot is not one direct leaf");
            };
            assert_eq!(lifecycle.as_str(), "core.bytes.drop");
        }
        assert!(
            inventory_bytes_slots > 0,
            "{id} has no Bytes inventory slot"
        );
        let mut plan_bytes_slots = 0usize;
        for slot in &function.cleanup_plan.slots {
            if slot.ty != hir::ResolvedType::Bytes {
                continue;
            }
            plan_bytes_slots += 1;
            let FieldLivenessShape::Leaf { lifecycle, .. } = &slot.field_liveness_shape else {
                panic!("Bytes plan slot is not one direct leaf");
            };
            assert_eq!(lifecycle.as_str(), "core.bytes.drop");
        }
        assert!(plan_bytes_slots > 0, "{id} has no Bytes plan slot");
    }

    let forward = function(&program, "bytes.forward");
    assert!(forward.cleanup_plan.entry_state.live_owned_parameters.len() == 1);
    assert!(forward.cleanup_plan.exits.iter().any(|exit| matches!(
        exit.continuation,
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { .. }
        }
    )));

    let consume = function(&program, "bytes.consume");
    assert!(consume
        .cleanup_plan
        .exits
        .iter()
        .filter(|exit| matches!(exit.continuation, ExitContinuation::CommitResult { .. }))
        .all(|exit| exit.finalize_in_order.len() == 1));
}

#[test]
fn compiler_owned_byte_operations_are_infallible_cleanup_steps() {
    let program = resolved();
    for id in ["bytes.make", "bytes.copy-consume"] {
        let function = function(&program, id);
        assert!(!function.cleanup_plan.status_sources.iter().any(|source| {
            matches!(
                &source.producer,
                StatusProducer::PropagatedCall { callee }
                    if callee.as_str() == "core.bytes.copy"
            )
        }));
    }
}

#[test]
fn forged_bytes_lifecycle_is_rejected_by_independent_replay() {
    let mut program = resolved();
    let make = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "bytes.make")
        .unwrap();
    let slot = make
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|slot| slot.ty == hir::ResolvedType::Bytes)
        .unwrap();
    let FieldLivenessShape::Leaf { lifecycle, .. } = &mut slot.field_liveness_shape else {
        panic!("Bytes plan slot is not one direct leaf");
    };
    *lifecycle = DeclarationId::new("hostile.bytes.drop");
    assert!(hir::validate(&program).is_err());
}
