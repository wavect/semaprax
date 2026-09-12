//! Evidence that the derived settlement obligations are the compiler's own
//! cleanup facts rather than a parallel guess, and that a disagreement fails
//! closed.

use std::path::Path;

use super::*;
use crate::hir::{self, ResolvedProgram};
use crate::parse;

const SOURCE: &str = r#"
module test.public_generic_settlement;

@id("settle.leaf")
record Leaf {
    @id("settle.leaf.head")
    head: Bytes,
    @id("settle.leaf.extra")
    extra: Bytes,
}

@id("settle.pair")
record Pair<T, U> {
    @id("settle.pair.left")
    left: T,
    @id("settle.pair.right")
    right: U,
}

@id("settle.take")
fn take(value: own Pair<Leaf, bool>) -> i64 {
    match own value {
        Pair { left: Leaf { head: payload, extra: more }, right: present } =>
            if present && byte_len(bytes_as_slice(payload)) > 0usize
                && byte_len(bytes_as_slice(more)) > 0usize { 1 } else { 0 },
    }
}

@id("settle.scalar")
fn scalar(value: i64) -> i64 { value }

@id("settle.borrowed")
fn borrowed(value: borrow Pair<Leaf, bool>) -> i64 {
    match borrow value {
        Pair { left: Leaf { head: _payload, extra: _more }, right: present } =>
            if present { 1 } else { 0 },
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    let parsed = parse(SOURCE, Path::new("settlement.spx")).unwrap();
    hir::resolve(&parsed).unwrap()
}

fn function(program: &ResolvedProgram, id: &str) -> &'static ResolvedFunction {
    // The program outlives every borrow in one test body; leaking the lookup
    // keeps the helpers readable without threading two lifetimes.
    let found = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .expect("the fixture declares the function");
    Box::leak(Box::new(found.clone()))
}

/// The obligations are the owned leaves of the instance, in the structural
/// order the grammar and the cleanup inventory agree on, and the runtime order
/// is the cleanup plan's own vector.
#[test]
fn obligations_are_the_checked_owned_leaves_in_agreed_order() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let plan = plan(&inventory, function(&program, "settle.take"), 0).unwrap();

    assert_eq!(plan.export(), "settle.take");
    assert_eq!(
        plan.instance_term(),
        "@11:settle.pair<@11:settle.leaf<>,bool>"
    );
    assert_eq!(
        plan.obligations()
            .iter()
            .map(|obligation| (obligation.index, obligation.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, "@16:settle.pair.left/@16:settle.leaf.head"),
            (1, "@16:settle.pair.left/@17:settle.leaf.extra"),
        ]
    );
    assert_eq!(
        plan.obligations()[0].fields,
        vec!["settle.pair.left", "settle.leaf.head"]
    );
    assert_eq!(
        plan.obligations()
            .iter()
            .map(|obligation| obligation.lifecycle.as_str())
            .collect::<Vec<_>>(),
        vec!["core.bytes.drop", "core.bytes.drop"],
        "every obligation states the checked lifecycle that discharges it"
    );

    assert_eq!(
        plan.release_order(),
        vec![
            "@16:settle.pair.left/@17:settle.leaf.extra",
            "@16:settle.pair.left/@16:settle.leaf.head",
        ],
        "a failed transfer releases in the exact reverse of the canonical order"
    );
    assert!(plan.digest().starts_with("sha256:"));
}

/// The transfer unit is the parameter itself, and the cleanup plan really does
/// name it whole: one live owned place with no projections. A boundary that
/// read the leaves out of the plan instead would be inventing a per-leaf
/// transfer the compiler does not perform.
#[test]
fn the_transfer_unit_is_the_whole_owned_parameter() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let function = function(&program, "settle.take");
    let plan = plan(&inventory, function, 0).unwrap();

    let live = &function.cleanup_plan.entry_state.live_owned_parameters;
    assert_eq!(live.len(), 1, "the fixture owns exactly one parameter");
    assert!(
        live[0].projections.is_empty(),
        "the entry state names the parameter whole, not per leaf"
    );
    assert_eq!(plan.transfer_unit(), function.params[0].id.as_str());
    assert_ne!(
        plan.transfer_unit(),
        plan.obligations()[0].path,
        "the transfer unit is not one of the leaves"
    );
}

/// Derivation is deterministic.
#[test]
fn derivation_is_deterministic() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let function = function(&program, "settle.take");
    assert_eq!(
        plan(&inventory, function, 0).unwrap(),
        plan(&inventory, function, 0).unwrap()
    );
}

/// Only an owned admitted instance with at least one owned leaf settles.
/// Everything else is refused rather than described with an empty plan.
#[test]
fn unsupported_parameters_fail_closed() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let cases = [
        ("settle.take", 1u32, "no parameter at that index"),
        ("settle.scalar", 0, "the parameter is not owned"),
        ("settle.borrowed", 0, "the parameter is not owned"),
    ];
    for (id, index, expected) in cases {
        let error = plan(&inventory, function(&program, id), index)
            .expect_err(&format!("{id}#{index} must fail closed"));
        assert_eq!(error.code, UNSUPPORTED_PARAMETER, "{id}#{index}");
        assert!(
            error.message.ends_with(expected),
            "{id}#{index} reported {}",
            error.message
        );
    }
}

/// A grammar-derived order that disagreed with the checked cleanup facts would
/// be worse than none, so the disagreement is a refusal. Removing a leaf from
/// the retained cleanup inventory stands in for any way the two could drift.
#[test]
fn a_disagreement_with_the_cleanup_facts_is_refused() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let mut mutated = function(&program, "settle.take").clone();
    let slot = mutated
        .cleanup
        .slots
        .iter_mut()
        .find(|slot| {
            matches!(
                slot.origin,
                CleanupStorageOrigin::Parameter {
                    parameter_index: 0,
                    ..
                }
            )
        })
        .expect("the owned parameter has a storage slot");
    if let FieldLivenessShape::Record { fields, .. } = &mut slot.shape {
        // Remove the owning field, not the Copy one: dropping `right: bool`
        // would leave the owned-leaf count untouched and prove nothing.
        fields.remove(0);
    } else {
        panic!("the owned parameter's shape is a record");
    }
    let error =
        plan(&inventory, &mutated, 0).expect_err("a shorter cleanup inventory must be refused");
    assert_eq!(error.code, SETTLEMENT_DISAGREEMENT);
    assert!(
        error.message.ends_with("the owned-leaf counts differ"),
        "got {}",
        error.message
    );
}

/// A liveness flag that names a different place is a refusal: an obligation
/// whose flag does not match it has no stated way to be discharged.
#[test]
fn a_relabelled_liveness_flag_is_refused() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let mut mutated = function(&program, "settle.take").clone();
    let storage = mutated
        .cleanup
        .slots
        .iter()
        .find(|slot| {
            matches!(
                slot.origin,
                CleanupStorageOrigin::Parameter {
                    parameter_index: 0,
                    ..
                }
            )
        })
        .map(|slot| slot.id)
        .unwrap();
    let flag = mutated
        .cleanup
        .flags
        .iter_mut()
        .find(|flag| flag.place.storage == storage)
        .expect("the owned parameter has liveness flags");
    flag.place.projections.pop();
    let error = plan(&inventory, &mutated, 0).expect_err("a relabelled flag must be refused");
    assert_eq!(error.code, SETTLEMENT_DISAGREEMENT);
    assert!(
        error.message.contains("liveness flag 0 does not name"),
        "got {}",
        error.message
    );
}

/// A projected (not whole) live owned parameter is a *transfer-unit*
/// disagreement, distinct from every other case in this file, which are all
/// cleanup-*inventory* disagreements (leaf paths, counts, and liveness
/// flags). Removing the projection would leave this obligation-derivation
/// path unable to fail on a plan that names the parameter piecewise; if the
/// `whole.projections.is_empty()` check in `plan` were deleted, this
/// mutation would be silently accepted instead of refused, so this asserts
/// the check itself rather than only its error code.
#[test]
fn a_projected_transfer_unit_is_a_transfer_unit_disagreement() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let mut mutated = function(&program, "settle.take").clone();
    let live = &mut mutated.cleanup_plan.entry_state.live_owned_parameters;
    assert_eq!(live.len(), 1, "the fixture owns exactly one parameter");
    live[0]
        .projections
        .push(crate::hir::DeclarationId::new("settle.pair.left"));

    let error =
        plan(&inventory, &mutated, 0).expect_err("a projected transfer unit must be refused");
    assert_eq!(error.code, TRANSFER_UNIT_DISAGREEMENT);
    assert_ne!(
        error.code, SETTLEMENT_DISAGREEMENT,
        "a transfer-unit disagreement must not collapse into the inventory code"
    );
    assert!(
        error
            .message
            .ends_with("the cleanup plan's live owned parameter is projected rather than whole"),
        "got {}",
        error.message
    );
}

/// A cleanup plan that names the owned parameter zero times (rather than
/// exactly once) is the sibling transfer-unit disagreement: same code, a
/// different way to fail the same `let [whole] = live.as_slice() else`
/// pattern match. If that match were replaced by anything that tolerated an
/// empty slice, this mutation would stop being refused.
#[test]
fn a_transfer_unit_named_zero_times_is_a_transfer_unit_disagreement() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let mut mutated = function(&program, "settle.take").clone();
    mutated
        .cleanup_plan
        .entry_state
        .live_owned_parameters
        .clear();

    let error =
        plan(&inventory, &mutated, 0).expect_err("an absent transfer unit must be refused");
    assert_eq!(error.code, TRANSFER_UNIT_DISAGREEMENT);
    assert!(
        error
            .message
            .ends_with("the cleanup plan does not name the owned parameter exactly once"),
        "got {}",
        error.message
    );
}

/// A storage slot whose type is not the parameter's type is a refusal too: the
/// plan is bound to one exact instance, not to whatever the slot holds.
#[test]
fn a_retyped_storage_slot_is_refused() {
    let program = program();
    let inventory = TypeInventory::of(&program);
    let mut mutated = function(&program, "settle.take").clone();
    let slot = mutated
        .cleanup
        .slots
        .iter_mut()
        .find(|slot| {
            matches!(
                slot.origin,
                CleanupStorageOrigin::Parameter {
                    parameter_index: 0,
                    ..
                }
            )
        })
        .unwrap();
    slot.ty = ResolvedType::Bytes;
    let error = plan(&inventory, &mutated, 0).expect_err("a retyped slot must be refused");
    assert_eq!(error.code, SETTLEMENT_DISAGREEMENT);
    assert!(
        error
            .message
            .ends_with("the cleanup storage type is not the parameter type"),
        "got {}",
        error.message
    );
}
