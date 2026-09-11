//! Operation-surface regressions for the SPX-AI-019 owned-record collection
//! profile (issue #118, acceptance criteria AC-2, AC-3 and AC-4).
//!
//! Every fixture below is a real parsed program driven through
//! `crate::check` (the independent source verifier) and `crate::hir::resolve`
//! (which also builds the loan plan and the cleanup plan), so a positive case
//! is genuine end-to-end admission and a negative case is a real diagnostic
//! rather than a hand-built intermediate representation asserting against
//! itself.
//!
//! The profile is deliberately admitted only through the front end: no
//! ordinary execution target implements its carrier yet (SPX-AI-020, issue
//! #119), and `ordinary_execution_targets_refuse_the_profile` pins that each
//! target refuses it with its own stable diagnostic instead of emitting a
//! broken carrier.

use crate::cleanup::FieldLivenessShape;
use crate::cleanup_plan::CleanupTransition;
use crate::hir::ResolvedProgram;

/// `quantity: i64`, the exact catalog-normalizer shape.
const DECLARATION: &str = r#"
module owned_record_collection.ops;
@id("owned_record_collection.ops.item") record Item {
  @id("owned_record_collection.ops.item.id") id: Bytes,
  @id("owned_record_collection.ops.item.label") label: Bytes,
  @id("owned_record_collection.ops.item.quantity") quantity: i64,
}
"#;

/// The same admitted shape with a `usize` Copy field, so a fixture can put a
/// borrowing `vec_len` call in the record's own Copy slot.
const USIZE_DECLARATION: &str = r#"
module owned_record_collection.usizeops;
@id("owned_record_collection.usizeops.item") record Item {
  @id("owned_record_collection.usizeops.item.id") id: Bytes,
  @id("owned_record_collection.usizeops.item.label") label: Bytes,
  @id("owned_record_collection.usizeops.item.quantity") quantity: usize,
}
"#;

fn program(declaration: &str, body: &str) -> String {
    let module = if declaration == DECLARATION {
        "owned_record_collection.ops"
    } else {
        "owned_record_collection.usizeops"
    };
    format!("{declaration}{body}\n@id(\"{module}.main\") fn main()->i64 {{ 0 }}\n")
}

/// Source verification and resolution of one fixture, both of which must
/// agree: an independently checked source program must resolve.
fn admitted(declaration: &str, body: &str) -> ResolvedProgram {
    let source = program(declaration, body);
    crate::check(&source, "owned-record-collection-ops.spx")
        .expect("the source verifier must admit the profile");
    let parsed = crate::parse(
        &source,
        std::path::Path::new("owned-record-collection-ops.spx"),
    )
    .unwrap();
    let resolved = crate::hir::resolve(&parsed).expect("HIR must admit the profile");
    crate::hir::validate(&resolved).expect("resolved HIR must validate");
    resolved
}

/// Every diagnostic code the source verifier and the resolver each report for
/// one fixture. Both are asserted so a refusal can never be source-only or
/// HIR-only.
fn refusal_codes(declaration: &str, body: &str) -> (Vec<String>, Vec<String>) {
    let source = program(declaration, body);
    let verified = crate::check(&source, "owned-record-collection-ops.spx")
        .err()
        .map(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.code.to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let parsed = crate::parse(
        &source,
        std::path::Path::new("owned-record-collection-ops.spx"),
    )
    .unwrap();
    let resolved = crate::hir::resolve(&parsed)
        .err()
        .map(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.code.to_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (verified, resolved)
}

fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a crate::hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .expect("the fixture function must resolve")
}

const BUILD: &str = r#"@id("owned_record_collection.ops.build") fn build()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let item=Item{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:7};
    let filled=vec_push<Item>(items,item);
    vec_len<Item>(filled)
}"#;

#[test]
fn with_capacity_len_capacity_and_clear_resolve_over_the_admitted_record_element() {
    let resolved = admitted(
        DECLARATION,
        r#"@id("owned_record_collection.ops.count") fn count()->usize {
    let items=vec_with_capacity<Item>(4usize);
    let seen=vec_len<Item>(items);
    let room=vec_capacity<Item>(items);
    let empty=vec_clear<Item>(items);
    seen + room
}"#,
    );
    assert!(super::program_uses_profile(&resolved));
    let item = crate::hir::ResolvedType::Nominal {
        declaration: crate::hir::DeclarationId::new("owned_record_collection.ops.item"),
        arguments: Vec::new(),
    };
    assert!(super::is_admitted_owned_record_collection_element(
        &resolved.declarations,
        &item
    ));
    assert!(super::is_owned_record_vec_type(
        &resolved.declarations,
        &crate::vec_ops::resolved_vec(item)
    ));
}

/// A `let` with an explicit `Vec<Item>` annotation must resolve, not just an
/// inferred one: the source verifier admits the annotation, so the resolver's
/// independent generic-argument rule has to admit it too or the two
/// projections disagree.
#[test]
fn an_explicitly_annotated_carrier_binding_resolves() {
    let resolved = admitted(
        DECLARATION,
        r#"@id("owned_record_collection.ops.annotated") fn annotated()->usize {
    let items: Vec<Item> = vec_with_capacity<Item>(2usize);
    vec_len<Item>(items)
}"#,
    );
    assert!(super::program_uses_profile(&resolved));
}

/// AC-2. The push stages its vector and its record left to right and transfers
/// both at the one declared commit boundary, and the record's own two owned
/// `Bytes` leaves move as a unit inside that single argument slot.
#[test]
fn push_stages_arguments_left_to_right_and_transfers_them_at_the_commit_boundary() {
    let resolved = admitted(DECLARATION, BUILD);
    let build = function(&resolved, "owned_record_collection.ops.build");
    let mut staged: Vec<usize> = Vec::new();
    let mut committed = 0usize;
    for block in &build.cleanup_plan.blocks {
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Transfer { destination, .. } => {
                    if let crate::cleanup_plan::StorageId::CallArgument {
                        parameter_index, ..
                    } = &destination.storage
                    {
                        staged.push(usize::try_from(*parameter_index).unwrap());
                    }
                }
                CleanupTransition::CallCommit { arguments, .. } if !arguments.is_empty() => {
                    committed += 1;
                    assert_eq!(
                        arguments
                            .iter()
                            .map(|argument| argument.parameter_index)
                            .collect::<Vec<_>>(),
                        vec![0, 1],
                        "one commit boundary transfers both staged arguments together"
                    );
                }
                _ => {}
            }
        }
    }
    assert_eq!(staged, vec![0, 1], "arguments stage left to right");
    assert_eq!(committed, 1, "exactly one owned commit boundary");
}

/// AC-2. A pushed record is consumed at that commit boundary, so reusing the
/// moved binding is an ownership diagnostic in both projections.
#[test]
fn a_pushed_record_cannot_be_reused() {
    let (verified, resolved) = refusal_codes(
        DECLARATION,
        r#"@id("owned_record_collection.ops.reuse") fn reuse()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let item=Item{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:7};
    let filled=vec_push<Item>(items,item);
    let again=vec_push<Item>(filled,item);
    vec_len<Item>(again)
}"#,
    );
    assert!(verified.contains(&"SPX-O101".to_owned()), "{verified:?}");
    assert!(resolved.contains(&"SPX-O101".to_owned()), "{resolved:?}");
}

/// AC-3. A borrowing read of the carrier after a consuming mutation of the
/// same place is refused before lowering.
#[test]
fn a_borrow_after_a_consuming_mutation_is_refused() {
    let (verified, resolved) = refusal_codes(
        DECLARATION,
        r#"@id("owned_record_collection.ops.stale") fn stale()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let empty=vec_clear<Item>(items);
    vec_len<Item>(items)
}"#,
    );
    assert!(verified.contains(&"SPX-O101".to_owned()), "{verified:?}");
    assert!(resolved.contains(&"SPX-O101".to_owned()), "{resolved:?}");
}

/// AC-3, the live case. `vec_push` stages its vector argument first, so a
/// borrowing `vec_len` of that same place inside the *second* argument is a
/// borrow that is live across the consuming mutation. Left-to-right staging is
/// what makes it observable, and it is refused before lowering.
#[test]
fn a_borrow_live_across_a_consuming_push_is_refused() {
    let (verified, resolved) = refusal_codes(
        USIZE_DECLARATION,
        r#"@id("owned_record_collection.usizeops.stage") fn stage()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let filled=vec_push<Item>(items,Item{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:vec_len<Item>(items)});
    vec_len<Item>(filled)
}"#,
    );
    assert!(verified.contains(&"SPX-O101".to_owned()), "{verified:?}");
    assert!(resolved.contains(&"SPX-O101".to_owned()), "{resolved:?}");
}

/// The bounded operation surface. `get` would be an ambiguous copy-returning
/// read of an owned value; `set` and `reserve_exact` are outside the
/// enumerated profile. All three keep the existing stable refusal rather than
/// reaching any backend.
#[test]
fn get_set_and_reserve_exact_stay_refused_with_a_stable_diagnostic() {
    for body in [
        r#"@id("owned_record_collection.ops.read") fn read()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let item=vec_get<Item>(items,0usize);
    vec_len<Item>(items)
}"#,
        r#"@id("owned_record_collection.ops.replace") fn replace()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let item=Item{id:bytes_zeroed(1usize),label:bytes_zeroed(2usize),quantity:7};
    let updated=vec_set<Item>(items,0usize,item);
    vec_len<Item>(updated)
}"#,
        r#"@id("owned_record_collection.ops.reserve") fn reserve()->usize {
    let items=vec_with_capacity<Item>(2usize);
    let bigger=vec_reserve_exact<Item>(items,4usize);
    vec_len<Item>(bigger)
}"#,
    ] {
        let (verified, resolved) = refusal_codes(DECLARATION, body);
        assert!(verified.contains(&"SPX-T281".to_owned()), "{verified:?}");
        assert!(!resolved.is_empty(), "resolution must refuse it too");
    }
    for op in crate::vec_ops::ALL {
        assert_eq!(
            super::admits_vec_operation(op),
            !matches!(
                op,
                crate::vec_ops::VecOp::Get
                    | crate::vec_ops::VecOp::Set
                    | crate::vec_ops::VecOp::ReserveExact
            ),
            "unexpected admission for {}",
            op.name()
        );
    }
}

/// AC-1 stays closed under the new operation surface: a structurally similar
/// record with a fourth field is not admitted as an element, so its `Vec` is
/// not the profile's carrier and its calls are refused.
#[test]
fn a_structurally_similar_record_is_still_refused_by_the_operation_surface() {
    let source = r#"
module owned_record_collection.foreign;
@id("owned_record_collection.foreign.item") record Item {
  @id("owned_record_collection.foreign.item.id") id: Bytes,
  @id("owned_record_collection.foreign.item.label") label: Bytes,
  @id("owned_record_collection.foreign.item.quantity") quantity: i64,
  @id("owned_record_collection.foreign.item.extra") extra: i64,
}
@id("owned_record_collection.foreign.count") fn count()->usize {
    let items=vec_with_capacity<Item>(2usize);
    vec_len<Item>(items)
}
@id("owned_record_collection.foreign.main") fn main()->i64 { 0 }
"#;
    let verified = crate::check(source, "owned-record-collection-foreign.spx")
        .err()
        .expect("a foreign record shape must be refused");
    assert!(verified
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T281"));
    let parsed = crate::parse(
        source,
        std::path::Path::new("owned-record-collection-foreign.spx"),
    )
    .unwrap();
    assert!(
        crate::hir::resolve(&parsed).is_err(),
        "HIR must refuse it independently"
    );
}

/// AC-4. Cleanup is canonical and per leaf: the record contributes one leaf per
/// owned `Bytes` field in authored declaration order with the compiler-owned
/// `core.bytes.drop` lifecycle and no leaf for its Copy field, and the carrier
/// contributes exactly one `core.vec.drop` leaf. Nothing here is sorted or
/// repaired; the order is the record's structural metadata.
#[test]
fn cleanup_derives_canonical_per_leaf_record_cleanup_and_one_carrier_leaf() {
    let resolved = admitted(DECLARATION, BUILD);
    let build = function(&resolved, "owned_record_collection.ops.build");
    let mut carrier_leaves = 0usize;
    let mut record_slots = 0usize;
    for slot in &build.cleanup_plan.slots {
        match &slot.field_liveness_shape {
            FieldLivenessShape::Leaf { lifecycle, .. }
                if super::is_owned_record_vec_type(&resolved.declarations, &slot.ty) =>
            {
                assert_eq!(lifecycle.as_str(), crate::cleanup::VEC_DROP_LIFECYCLE_ID);
                carrier_leaves += 1;
            }
            FieldLivenessShape::Record {
                declaration,
                fields,
            } => {
                assert_eq!(declaration.as_str(), "owned_record_collection.ops.item");
                record_slots += 1;
                assert_eq!(
                    fields
                        .iter()
                        .map(|field| (field.field.as_str().to_owned(), field.field_index))
                        .collect::<Vec<_>>(),
                    vec![
                        ("owned_record_collection.ops.item.id".to_owned(), 0),
                        ("owned_record_collection.ops.item.label".to_owned(), 1),
                        ("owned_record_collection.ops.item.quantity".to_owned(), 2),
                    ],
                    "cleanup inventory order is the authored field order"
                );
                for (index, field) in fields.iter().enumerate() {
                    match (&field.shape, index) {
                        (FieldLivenessShape::Leaf { lifecycle, .. }, 0 | 1) => {
                            assert_eq!(lifecycle.as_str(), crate::cleanup::BYTES_DROP_LIFECYCLE_ID);
                        }
                        (FieldLivenessShape::NoDrop, 2) => {}
                        (shape, index) => panic!("unexpected field {index} shape {shape:?}"),
                    }
                }
            }
            _ => {}
        }
    }
    assert!(carrier_leaves > 0, "the carrier must own a drop leaf");
    assert!(record_slots > 0, "the record must expand per leaf");
}

/// AC-4. Every fallible owned call on this profile publishes a failure edge
/// that selects a status; selection is sticky, so cleanup that follows never
/// replaces it.
#[test]
fn fallible_owned_calls_select_a_sticky_failure_status() {
    let resolved = admitted(DECLARATION, BUILD);
    let build = function(&resolved, "owned_record_collection.ops.build");
    let selections = build
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter(|transition| matches!(transition, CleanupTransition::SelectFailure { .. }))
        .count();
    assert!(
        selections >= 2,
        "with_capacity and push each publish one failure selection, saw {selections}"
    );
    assert!(!build.cleanup_plan.status_sources.is_empty());
}

/// The profile is admitted by the front end and by nothing else. Each ordinary
/// execution target refuses it with its own stable diagnostic, so "no backend
/// executes the new shape" is a compile-time diagnostic rather than a broken
/// carrier or a backend accident.
#[test]
fn ordinary_execution_targets_refuse_the_profile() {
    let resolved = admitted(DECLARATION, BUILD);
    let native = crate::codegen::emit_hir_c(&resolved).expect_err("native must refuse");
    assert_eq!(native.code, super::NATIVE_TARGET_CODE);
    let wasm = crate::wasm::emit_resolved_module(&resolved).expect_err("Wasm must refuse");
    assert_eq!(wasm.code, super::WASM_TARGET_CODE);
    let interpreted = crate::interpreter::evaluate_resolved_owned_data(
        &resolved,
        "owned_record_collection.ops.build",
        &[],
        16,
    )
    .expect_err("the interpreter must refuse");
    assert_eq!(interpreted[0].code, super::INTERPRETER_TARGET_CODE);
    assert_eq!(native.code, "SPX-B115");
    assert_eq!(wasm.code, "SPX-W125");
    assert_eq!(interpreted[0].code, "SPX-F107");
}

/// Both projections stay stable over the new surface: canonical source is a
/// fixed point and the semantic graph projects the program without error.
#[test]
fn canonical_source_and_graph_projections_are_stable() {
    let source = program(DECLARATION, BUILD);
    let parsed = crate::parse(
        &source,
        std::path::Path::new("owned-record-collection-ops.spx"),
    )
    .unwrap();
    let canonical = crate::format::canonical(&parsed);
    let reparsed = crate::parse(
        &canonical,
        std::path::Path::new("owned-record-collection-ops.spx"),
    )
    .unwrap();
    assert_eq!(crate::format::canonical(&reparsed), canonical);
    let graph = crate::graph::to_json(&parsed).expect("the graph must project the profile");
    assert!(graph.contains("core.vec.push"));
    assert_eq!(
        crate::graph::to_json(&reparsed).expect("canonical source projects the same graph"),
        graph
    );
}
