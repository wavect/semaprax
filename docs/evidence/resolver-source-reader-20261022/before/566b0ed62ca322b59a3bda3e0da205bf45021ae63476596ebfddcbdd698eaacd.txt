//! Arena edges against the formatting encoders, plus storage/work controls.
use super::*;
use crate::bounded_output::{active_refused, with_limit_usage, with_limit_usage_refusal};
use crate::hir::{DeclarationId, FunctionInstanceId};
use crate::test_allocation_observer::measure;

const INDICES: [usize; 8] = [0, 9, 10, 99, 100, 999, 1000, u32::MAX as usize];

fn owners() -> [FunctionExecutionId; 3] {
    [
        FunctionExecutionId::Monomorphic(DeclarationId::new("app.main")),
        FunctionExecutionId::Monomorphic(DeclarationId::new("λ.函数")),
        FunctionExecutionId::Generic(FunctionInstanceId("λ-instance".to_owned())),
    ]
}

/// Every closed edge spelling, built independently of `Step` formatting.
fn every_edge() -> Vec<(Step, String)> {
    let mut edges = Vec::new();
    for lead in Seg::ALL {
        if lead != Seg::None {
            edges.push((Step::from(lead), lead.text().to_owned()));
        }
        for index in INDICES {
            for trail in Seg::ALL {
                let text = [lead.text(), &index.to_string(), trail.text()].concat();
                edges.push((lead.at(index).then(trail), text));
            }
        }
    }
    edges
}

#[test]
fn every_edge_spelling_matches_the_formatting_encoders() {
    for root in ["body", "", "contract.λ.requires.0"] {
        let mut paths = PathArena::new(root);
        for (step, edge) in every_edge() {
            paths.release_above(0);
            let node = paths.child_at(1, 1, PathId::ROOT, step);
            let expected = format!("{root}{edge}");
            assert_eq!(paths.at(node).materialize(), expected);
            assert_eq!(ExprPath::from(root).materialize_with(&[step]), expected);
            for owner in owners() {
                let old = ExpressionId::new(&owner, &expected);
                let streamed = paths.at(node).expression_id(&owner);
                assert_eq!(streamed.as_str(), old.as_str());
                assert_eq!(
                    streamed.owned_allocation_bytes(),
                    old.owned_allocation_bytes()
                );
                let suffixed = paths.at(PathId::ROOT).expression_id_with(&owner, &[step]);
                assert_eq!(suffixed, old);
                let text = ExprPath::from(root).expression_id_with(&owner, &[step]);
                assert_eq!(text, old);
                // ValueId equality also compares the cached fingerprint.
                let local = ValueId::local(&owner, &expected);
                assert_eq!(paths.at(node).value_local_with(&owner, &[]), local);
                assert_eq!(
                    ExprPath::from(root).value_local_with(&owner, &[step]),
                    local
                );
            }
        }
    }
}

#[test]
fn opposing_decimal_widths_keep_exact_value_identity_order() {
    let owner = &owners()[0];
    let mut paths = PathArena::new("body");
    let older = paths
        .at(PathId::ROOT)
        .value_local_with(owner, &[Seg::Statement.at(999)]);
    let newer = paths
        .at(PathId::ROOT)
        .value_local_with(owner, &[Seg::Statement.at(1000)]);
    assert_eq!(older, ValueId::local(owner, "body.s999"));
    assert_eq!(newer, ValueId::local(owner, "body.s1000"));
    assert!(newer < older, "decimal path-length 10 precedes 9");
    let node = paths.child_at(1, 1, PathId::ROOT, Seg::Statement.at(1000));
    assert_eq!(
        paths.at(node).value_local_with(owner, &[]).cmp(&older),
        newer.cmp(&older)
    );
}

#[test]
fn deep_prefix_walks_once_per_edge_and_allocates_only_final_backing() {
    const DEPTH: usize = 2048;
    let owner = &owners()[2];
    let mut paths = PathArena::new("body");
    let mut expected = "body".to_owned();
    let mut node = PathId::ROOT;
    for height in 1..=DEPTH {
        let step = match height % 3 {
            0 => Step::from(Seg::Left),
            1 => Seg::Arg.at(height),
            _ => Seg::Arm.at(height).then(Seg::Guard),
        };
        expected.push_str(&step.to_string());
        node = paths.child_at(height, DEPTH, node, step);
    }
    let before = paths.work();
    assert_eq!(before.nodes_created, DEPTH);
    assert_eq!(paths.owned_capacity(), DEPTH * PATH_NODE_BYTES);
    // Every doubling from 4 to DEPTH nodes was debited before allocation.
    assert_eq!(before.node_bytes_charged, (2 * DEPTH - 4) * PATH_NODE_BYTES);

    let total = streamed::scoped_len(owner, "expression", expected.len()).unwrap();
    let (id, metrics) = measure(|| paths.at(node).expression_id(owner));
    assert_eq!(id, ExpressionId::new(owner, &expected));
    assert!(!metrics.invalid);
    assert_eq!(metrics.reallocations, 0);
    assert_eq!(
        metrics.allocations, 2,
        "identity backing and shared carrier"
    );
    assert_eq!(
        metrics.allocated,
        total + ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES
    );
    let after = paths.work();
    assert_eq!(after.parent_steps - before.parent_steps, DEPTH);
    assert_eq!(
        after.path_bytes_written - before.path_bytes_written,
        expected.len()
    );
}

#[test]
fn wide_sibling_edges_reuse_one_live_slot() {
    let owner = &owners()[0];
    let mut paths = PathArena::new("body");
    let call = paths.child_at(1, 4, PathId::ROOT, Seg::Statement.at(0).then(Seg::Value));
    for index in 0..10_000 {
        // ChildNext at position 1 is popped: its previous argument is dead.
        paths.release_above(1);
        let argument = paths.child_at(2, 4, call, Seg::Arg.at(index));
        let expected = format!("body.s0.value.arg.{index}");
        assert_eq!(
            paths.at(argument).expression_id(owner),
            ExpressionId::new(owner, &expected)
        );
        assert_eq!(paths.live_nodes(), 2);
    }
    let work = paths.work();
    assert_eq!(work.high_water_nodes, 2);
    assert_eq!(work.nodes_released, 9_999);
    assert_eq!(paths.owned_capacity(), 4 * PATH_NODE_BYTES);
    assert_eq!(work.node_bytes_charged, 4 * PATH_NODE_BYTES);
}

#[test]
fn release_keeps_every_handle_held_at_or_below_the_frame_height() {
    let mut paths = PathArena::new("body");
    let parent = paths.child_at(1, 4, PathId::ROOT, Seg::Tail.into());
    let dead = paths.child_at(2, 4, parent, Seg::Left.into());
    assert_eq!(paths.at(dead).materialize(), "body.tail.left");
    paths.release_above(1);
    let reused = paths.child_at(2, 4, parent, Seg::Right.into());
    assert_eq!(reused, dead, "the released slot is reused");
    assert_eq!(paths.at(reused).materialize(), "body.tail.right");
    assert_eq!(paths.at(parent).materialize(), "body.tail");
    paths.release_above(0);
    assert_eq!(paths.live_nodes(), 0);
    assert_eq!(paths.at(PathId::ROOT).materialize(), "body");
}

#[test]
fn growth_is_charged_before_allocation_and_covers_replacement_overlap() {
    let first = 4 * PATH_NODE_BYTES;
    let second = 8 * PATH_NODE_BYTES;
    let spelled = "body.value.value.value.value.value".len();
    let ((text, metrics), overflowed, used) = with_limit_usage(first + second + spelled, || {
        measure(|| {
            let mut paths = PathArena::new("body");
            let mut node = PathId::ROOT;
            for height in 1..=5 {
                let slots = if height <= 4 { 4 } else { 8 };
                node = paths.child_at(height, slots, node, Seg::Value.into());
            }
            let text = paths.at(node).materialize();
            drop(paths);
            text
        })
    });
    assert!(!overflowed);
    assert_eq!(text, "body.value.value.value.value.value");
    assert!(!metrics.invalid);
    assert_eq!(metrics.reallocations, 1);
    // Every requested backing, including the replaced node buffer, was
    // debited before allocation; the debit covers the observed overlap.
    assert_eq!(used, first + second + spelled);
    assert_eq!(metrics.allocated, used);
    assert_eq!(metrics.highwater, first + second);
    assert_eq!(metrics.live, text.capacity());
}

#[test]
fn exact_and_one_short_growth_limits_with_sticky_zero_unit_retry() {
    let bytes = 4 * PATH_NODE_BYTES;
    let (node, overflowed, used) = with_limit_usage(bytes, || {
        let mut paths = PathArena::new("body");
        paths.child_at(1, 4, PathId::ROOT, Seg::Value.into())
    });
    assert!(!overflowed);
    assert_eq!(used, bytes);
    assert_ne!(node, PathId::ROOT);

    let ((paths, refused, retried, metrics), overflowed, used, refusal) =
        with_limit_usage_refusal(bytes - 1, || {
            let (outcome, metrics) = measure(|| {
                let mut paths = PathArena::new("body");
                let refused = paths.child_at(1, 4, PathId::ROOT, Seg::Value.into());
                // Zero-unit retry: no further charge, still refused.
                let retried = paths.child_at(2, 4, refused, Seg::Left.into());
                (paths, refused, retried)
            });
            assert!(active_refused());
            (outcome.0, outcome.1, outcome.2, metrics)
        });
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(metrics.allocations, 0);
    assert_eq!((refused, retried), (PathId::REFUSED, PathId::REFUSED));
    let refusal = refusal.unwrap();
    assert_eq!((refusal.requested, refusal.remaining), (bytes, bytes - 1));
    // A refused coordinate spells the empty text the formatting route left.
    let owner = &owners()[0];
    assert_eq!(
        paths.at(retried).materialize_with(&[Seg::Value.into()]),
        ".value"
    );
    assert_eq!(
        paths.at(retried).expression_id(owner),
        ExpressionId::new(owner, "")
    );
}

#[test]
fn materialization_charges_exact_text_once() {
    let mut paths = PathArena::new("body");
    let node = paths.child_at(1, 1, PathId::ROOT, Seg::Arm.at(10).then(Seg::Record));
    let expected = "body.arm.10.record";
    let ((text, metrics), overflowed, used) =
        with_limit_usage(expected.len(), || measure(|| paths.at(node).materialize()));
    assert!(!overflowed);
    assert_eq!(used, expected.len());
    assert_eq!(text, expected);
    assert_eq!(
        (metrics.allocations, metrics.allocated),
        (1, expected.len())
    );
    let (text, overflowed, used) =
        with_limit_usage(expected.len() - 1, || paths.at(node).materialize());
    assert!(overflowed);
    assert_eq!((text.as_str(), used), ("", 0));
}

#[test]
fn unaddressable_coordinates_refuse_instead_of_truncating() {
    let ((paths, node), overflowed, _) = with_limit_usage(1 << 20, || {
        let mut paths = PathArena::new("body");
        let height = u32::MAX as usize + 1;
        let node = paths.child_at(height, 1, PathId::ROOT, Seg::Value.into());
        (paths, node)
    });
    assert!(overflowed);
    assert_eq!(node, PathId::REFUSED);
    assert_eq!(paths.live_nodes(), 0);
    let (node, overflowed, _) = with_limit_usage(1 << 20, || {
        let mut paths = PathArena::new("body");
        paths.child_at(1, 1, PathId::ROOT, Seg::Arg.at(u32::MAX as usize + 1))
    });
    assert!(overflowed);
    assert_eq!(node, PathId::REFUSED);
}

#[test]
fn resolver_frame_slot_bound_includes_one_arena_node() {
    assert_eq!(PATH_NODE_BYTES, 20);
    assert_eq!(
        crate::hir::RESOLVER_FRAME_BYTES,
        std::mem::size_of::<crate::hir::borrowed_root_oracle::resolve_expr_frame::Frame<'static>>() + PATH_NODE_BYTES
    );
    assert_eq!(std::mem::size_of::<PathId>(), 4);
}

#[test]
fn allocator_capacity_mismatch_refuses_before_publishing_an_arena_node() {
    // Supply actual backing outside the observation: this injects allocator
    // discrepancy into the real post-allocation receiver, not a mock allocator.
    for (actual, requested, slots) in [(5, 4, 5), (5, 5, 4), (3, 4, 4)] {
        let mut paths = PathArena::new("body");
        paths.nodes = Vec::with_capacity(actual);
        assert_eq!(paths.nodes.capacity(), actual);
        let parent = paths.child_at(1, actual, PathId::ROOT, Seg::Value.into());
        assert_eq!(paths.at(parent).materialize(), "body.value");
        let parent_pointer = paths.nodes.as_ptr();
        let (((accepted, retried), metrics), overflowed, used) = with_limit_usage(1024, || {
            measure(|| {
                assert!(crate::bounded_output::reserve_active_required(
                    requested * 20
                ));
                let accepted = paths.admit_growth(requested, slots);
                let retried = paths.child_at(2, slots, parent, Seg::Left.into());
                (accepted, retried)
            })
        });
        assert!(!accepted);
        assert!(overflowed);
        assert_eq!(retried, PathId::REFUSED);
        assert_eq!(paths.live_nodes(), 1);
        assert_eq!(paths.nodes.as_ptr(), parent_pointer);
        assert_eq!(paths.at(parent).materialize(), "body.value");
        paths.release_above(0);
        assert_eq!(paths.live_nodes(), 0);
        assert_eq!(used, actual.max(requested) * 20);
        assert_eq!(metrics.allocations, 0);
    }
    // Delta reconciliation itself may refuse; it never grants a handle or
    // hides an allocation behind the requested-capacity forecast.
    let mut paths = PathArena::new("body");
    paths.nodes = Vec::with_capacity(5);
    let (accepted, overflowed, used, refusal) = with_limit_usage_refusal(99, || {
        assert!(crate::bounded_output::reserve_active_required(80));
        paths.admit_growth(4, 5)
    });
    assert!(!accepted);
    assert!(overflowed);
    assert_eq!(used, 80);
    let refusal = refusal.unwrap();
    assert_eq!((refusal.requested, refusal.remaining), (20, 19));
    assert_eq!(
        paths.child_at(1, 5, PathId::ROOT, Seg::Left.into()),
        PathId::REFUSED
    );
    assert_eq!(paths.live_nodes(), 0);
}
