
use super::super::active_targets;
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::SkeletonObservation;
use crate::test_allocation_observer::{measure, Metrics};

#[derive(Debug)]
struct Run {
    result: Result<(bool, Weighted), String>,
    remaining: usize,
    paid: usize,
    refused: bool,
    heap: Metrics,
    calls: usize,
    callbacks: usize,
}

#[allow(clippy::too_many_arguments)]
fn run(
    inline: bool,
    f: &ResolvedFunction,
    initial: &[(usize, u64)],
    inputs: &[(usize, u64)],
    work: usize,
    bytes: usize,
    floor: usize,
) -> Run {
    // Receipt preparation stays outside the measured compiler operation.
    let mut keys: std::collections::BTreeSet<_> = initial.iter().map(|x| x.0).collect();
    let callbacks_per_call: Vec<_> = inputs
        .iter()
        .map(|&(child, _)| {
            let comparisons = (usize::BITS - keys.len().leading_zeros()) as usize + 1;
            keys.insert(child);
            comparisons
        })
        .collect();
    let ((result, remaining, heap, calls, callbacks), refused, paid, _) =
        with_limit_usage_refusal(bytes, || {
            let ((result, remaining, calls, callbacks), heap) = measure(|| {
                let mut b = ReplayBudget::with_skeleton_limit(work);
                b.remaining = work;
                assert!(crate::bounded_output::set_active_floor(floor));
                let mut calls = 0;
                let mut callbacks = 0;
                let result = (|| {
                    if inline {
                        b.reserve_compact_bytes(f, PRIMITIVE_FRAMES, "inline target call frames")?;
                    }
                    let mut rows = Vec::new();
                    b.reserve_compact_vec(f, &mut rows, initial.len(), "matched target backing")?;
                    rows.extend_from_slice(initial);
                    for &(child, weight) in inputs {
                        if inline {
                            b.reserve_compact_bytes(
                                f,
                                FlatCursor::CONTROLS,
                                "inline target receivers",
                            )?;
                            let budget = &mut b;
                            let targets = &mut rows;
                            macro_rules! attempt {
                                ($operation:expr) => {
                                    $operation?
                                };
                            }
                            add_flat!(f, budget, targets, child, weight, attempt, {
                                return Ok((false, rows));
                            });
                        } else {
                            b.reserve_compact_bytes(
                                f,
                                super::super::super::compact_targets::ADD_RECEIVING,
                                "original target receivers",
                            )?;
                            if !super::super::super::compact_targets::add(
                                f, &mut b, &mut rows, child, weight,
                            )? {
                                return Ok((false, rows));
                            }
                        }
                        calls += 1;
                        callbacks += callbacks_per_call[calls - 1];
                    }
                    Ok::<_, ByteFailure>((true, rows))
                })()
                .map_err(|e| e.into_diagnostic().json());
                (result, b.remaining, calls, callbacks)
            });
            (result, remaining, heap, calls, callbacks)
        });
    Run {
        result,
        remaining,
        paid,
        refused,
        heap,
        calls,
        callbacks,
    }
}

#[test]
fn inline_targets_match_complete_old_weights_work_heap_and_exact_removed_transport() {
    let checked = crate::check(
        "module inline_targets; @id(\"main\") fn main()->i64 {0}",
        "targets.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let f = &program.functions[0];
    let initial = [(2, 1), (8, 2)];
    let inputs = [(8, 3), (5, 7), (1, 11), (9, 13), (5, 17), (2, 19)];
    let old = run(false, f, &initial, &inputs, usize::MAX, 64 * 1024 * 1024, 0);
    let new = run(true, f, &initial, &inputs, usize::MAX, 64 * 1024 * 1024, 0);
    assert_eq!(old.result, new.result);
    assert!(matches!(new.result, Ok((true, _))));
    assert_eq!(old.remaining, new.remaining);
    assert_eq!(old.heap, new.heap);
    assert!(!old.refused && !new.refused);
    assert_eq!((old.calls, old.callbacks), (new.calls, new.callbacks));
    let removed_callback = size_of::<&(usize, u64)>() + 2 * size_of::<usize>();
    assert_eq!(
        new.paid
            + new.calls * super::super::super::compact_targets::ADD_RECEIVING
            + new.callbacks * removed_callback,
        old.paid + PRIMITIVE_FRAMES + new.calls * FlatCursor::CONTROLS,
    );
    assert!(new.paid < old.paid);
}

#[test]
fn inline_targets_keep_all_work_cuts_overflow_exact_short_zero_and_reserved_floor() {
    let checked = crate::check(
        "module inline_target_cuts; @id(\"main\") fn main()->i64 {0}",
        "cuts.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let f = &program.functions[0];
    let initial = [(2, 1), (8, 2)];
    let inputs = [(8, 3), (5, 7), (1, 11), (9, 13), (5, 17), (2, 19)];
    for work in 0..40 {
        let old = run(false, f, &initial, &inputs, work, 64 * 1024 * 1024, 0);
        let new = run(true, f, &initial, &inputs, work, 64 * 1024 * 1024, 0);
        assert_eq!(old.result, new.result, "work={work}");
        assert_eq!(old.remaining, new.remaining);
        assert_eq!(old.heap, new.heap);
    }
    let old = run(
        false,
        f,
        &[(2, u64::MAX), (8, 2)],
        &[(2, 1)],
        usize::MAX,
        64 * 1024 * 1024,
        0,
    );
    let new = run(
        true,
        f,
        &[(2, u64::MAX), (8, 2)],
        &[(2, 1)],
        usize::MAX,
        64 * 1024 * 1024,
        0,
    );
    assert_eq!(old.result, new.result);
    assert!(matches!(new.result, Ok((false, _))));
    assert_eq!(old.heap, new.heap);
    for inline in [false, true] {
        let full = run(
            inline,
            f,
            &initial,
            &inputs,
            usize::MAX,
            64 * 1024 * 1024,
            0,
        );
        let exact = run(inline, f, &initial, &inputs, usize::MAX, full.paid, 0);
        assert_eq!(exact.result, full.result);
        assert!(!exact.refused);
        for (bytes, floor) in [(0, 0), (full.paid - 1, 0), (full.paid, 1)] {
            let refused = run(inline, f, &initial, &inputs, usize::MAX, bytes, floor);
            assert!(refused.result.is_err() && refused.refused);
        }
    }
}

#[test]
fn moved_weighted_finish_keeps_complete_old_owner_heap_and_exact_typed_transport() {
    let checked = crate::check(
        "module weighted_finish; @id(\"main\") fn main()->i64 {0}",
        "finish.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let f = &program.functions[0];
    let run = |compact: bool, length: usize, bytes: usize, floor: usize| {
        let ((result, remaining, heap), refused, paid, _) = with_limit_usage_refusal(bytes, || {
            let ((result, remaining), heap) = measure(|| {
                let mut b = ReplayBudget::with_skeleton_limit(usize::MAX);
                assert!(crate::bounded_output::set_active_floor(floor));
                let result = (|| {
                    let mut rows = Weighted::new();
                    b.reserve_compact_vec(f, &mut rows, length, "matched weighted backing")
                        .map_err(|e| e.into_diagnostic())?;
                    for index in 0..length {
                        rows.push((index, (index + 1) as u64));
                    }
                    if compact {
                        b.reserve_compact_bytes(
                            f,
                            FINISH_RECEIVING,
                            "matched weighted finish receivers",
                        )
                        .map_err(|e| e.into_diagnostic())?;
                        finish_weighted(f, &mut b, rows).map_err(|e| e.into_diagnostic())
                    } else {
                        b.reserve_bytes(
                            f,
                            paid_many::FINISH_RECEIVING,
                            "matched weighted finish receivers",
                        )?;
                        own_weighted(f, &mut b, rows)
                    }
                })();
                (result.map_err(|error| error.code), b.remaining)
            });
            (result, remaining, heap)
        });
        (result, remaining, heap, refused, paid)
    };
    let removed = Canonicalizer::ACTIVE_REMOVED_WEIGHTED_TRANSPORT;
    for length in [0, 1, 2, 4, 17, 65] {
        let old = run(false, length, 64 * 1024 * 1024, 0);
        let new = run(true, length, 64 * 1024 * 1024, 0);
        assert_eq!(old.0, new.0);
        assert!(new.0.is_ok());
        assert_eq!(old.1, new.1);
        assert_eq!(old.2, new.2);
        assert!(!old.3 && !new.3);
        assert_eq!(new.4 + removed, old.4);
        let value = new.0.as_ref().unwrap();
        assert!(matches!(value.as_ref(), SmallWeighted::Many(_)));
        assert_eq!(value.len(), length);
        for compact in [false, true] {
            let full = run(compact, length, 64 * 1024 * 1024, 0);
            let exact = run(compact, length, full.4, 0);
            assert_eq!(exact.0, full.0);
            assert_eq!(exact.2, full.2);
            assert!(!exact.3);
            for (bytes, floor) in [(0, 0), (full.4 - 1, 0), (full.4, 1)] {
                let short = run(compact, length, bytes, floor);
                assert!(short.0.is_err() && short.3);
                assert_eq!(short.2.allocated, short.2.freed);
            }
        }
    }
}

#[test]
fn append_label_bound_covers_actual_allocations_at_every_prefix_and_split_order() {
    // Actual compiler label/target types; no proxy IR or layout is used.
    // This isolates the map's allocator bytes. The complete actual owner,
    // helper ABI, work and refusal receipts are exercised by active_builder.
    let checked = crate::check(
        "module label_heap; @id(\"main\") fn main()->i64 {0}",
        "labels.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let f = &program.functions[0];
    let labels: Vec<_> = (0..1024)
        .map(|arm| {
            Label::Observation(Rc::new(SkeletonObservation::ArmSelected {
                scrutinee: f.body.id.clone(),
                arm,
                selected: true,
            }))
        })
        .collect();
    let node = crate::cleanup_plan::replay::physical_storage::btree_node_upper::<
        &Label,
        paid_many::Targets,
    >()
    .unwrap();
    let counts: Vec<_> = (0..=128)
        .chain([129, 255, 256, 511, 512, 1023, 1024])
        .collect();
    for order in 0..4 {
        let indices: Vec<_> = (0..1024)
            .map(|i| match order {
                0 => i,
                1 => 1023 - i,
                2 => (i * 641) % 1024,
                _ => {
                    if i % 2 == 0 {
                        i / 2
                    } else {
                        1023 - i / 2
                    }
                }
            })
            .collect();
        for &count in &counts {
            let ((len, heap), refused, _, _) = with_limit_usage_refusal(64 * 1024 * 1024, || {
                let mut b = ReplayBudget::with_skeleton_limit(usize::MAX);
                b.reserve_compact_bytes(
                    f,
                    APPEND_LABEL_FRAMES + super::super::legacy_payments::ONE_PASS_NODE_ABI,
                    "actual append label helper frames",
                )
                .unwrap();
                measure(|| {
                    let mut map = BTreeMap::new();
                    for &index in &indices[..count] {
                        append_label_node(f, &mut b, map.len(), "actual append label node")
                            .unwrap();
                        assert!(map
                            .insert(
                                &labels[index],
                                paid_many::Targets::One {
                                    child: index,
                                    weight: 1
                                }
                            )
                            .is_none());
                    }
                    let len = map.len();
                    drop(map);
                    len
                })
            });
            let nodes = if count == 0 { 0 } else { 1 + (count - 1) / 5 };
            assert_eq!(len, count);
            assert!(!refused && !heap.invalid);
            assert_eq!(heap.live, 0);
            assert_eq!(heap.allocated, heap.freed);
            assert!(
                heap.allocated <= nodes * node,
                "order={order} count={count}: {heap:?} bound={}",
                nodes * node
            );
        }
    }
}
