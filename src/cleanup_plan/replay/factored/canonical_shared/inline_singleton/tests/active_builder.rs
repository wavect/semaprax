//! Whole current general component compared with active-only activation.
// This alias is checked against the actual retained producer below.
type Successors = (u64, Vec<(Label, SharedWeighted)>);
use super::*;
fn invoke_active(
    canonical: &mut Canonicalizer<'_>,
    function: &ResolvedFunction,
    root: Weighted,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    // Preserve the old entry's owner rejection before a fresh test-only slot
    // can query or charge a foreign payer. A valid call still pays acquisition.
    if canonical.closures.preflight_owned(function, budget) {
        if canonical.closures.failed() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
    } else {
        if canonical.closures.failed() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        // Keep the first-cold failure before invocation-slot acquisition.
        // The outer id Result is unchanged; the inner auth Result is absent.
        activate_closure!(canonical, function, budget, error, {
            return Err(error);
        });
    }
    // Existing received owner place: no copied tag, bool or reference.
    // scratch_paid=true makes cold_dead_node false by its own definition.
    if let Canonicalizer {
        scratch_paid: false,
        ..
    } = canonical
    {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                Canonicalizer::COLD_ROOT_CONTROLS,
                "factored cold terminal root predicate",
            )?;
        }
        if root.len() == 1 && root[0].1 == 1 && canonical.cold_dead_node(root[0].0) {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    Canonicalizer::COLD_RESULT_CONTROLS,
                    "factored cold terminal result receiving",
                )?;
            }
            return canonical.cold_dead_id(function, root, budget);
        }
    }
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
            "factored held closure invocation controls",
        )?;
    }
    let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
    canonical.id_with_active_receiving(function, root, budget, &mut receiving)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Matched whole-owner predecessor and compact transport controls use identical actual calls."
)]
fn run(
    compact: bool,
    f: &ResolvedFunction,
    a: &Automaton,
    sink: usize,
    root: usize,
    weight: u64,
    limit: usize,
    work: usize,
    floor: usize,
) -> Run {
    Canonicalizer::active_finish_calls(true);
    Canonicalizer::active_avoided_label_nodes(true);
    Canonicalizer::active_initial_label_maps(true);
    let ((result, remaining, skeleton, spent, heap, shapes, successors), refused, paid, _) =
        with_limit_usage_refusal(limit, || {
            let ((result, remaining, skeleton, spent, shapes, successors), heap) = measure(|| {
                let mut b = ReplayBudget::with_skeleton_limit(work);
                b.remaining = work;
                let observer = old_census::observe(f, &b);
                let shapes = shape_census::observe(f, &b);
                assert!(crate::bounded_output::set_active_floor(floor));
                let result = (|| {
                    b.reserve_bytes(
                        f,
                        Canonicalizer::CONSTRUCTOR_CONTROLS + NEW_CALL,
                        "canonical ordered dictionary owner receiving",
                    )?;
                    let mut c = Canonicalizer::new(a, sink);
                    b.reserve_array::<(usize, u64)>(f, 1, "889 matched root")?;
                    if compact {
                        invoke_active(&mut c, f, vec![(root, weight)], &mut b)
                    } else {
                        c.id_compact(f, vec![(root, weight)], &mut b)
                    }
                })()
                .map_err(failure);
                let row = shapes.finish(matches!(result, Ok(Some(_))));
                let successor = old_census::take();
                drop(shapes);
                drop(observer);
                (
                    result,
                    b.remaining,
                    b.skeleton_remaining,
                    b.skeleton_spent,
                    row,
                    successor,
                )
            });
            (result, remaining, skeleton, spent, heap, shapes, successors)
        });
    Run {
        result,
        remaining,
        skeleton,
        spent,
        paid,
        refused,
        heap,
        shapes,
        successors,
    }
}

#[test]
fn full_graphs_keep_ids_work_heap_and_exact_removed_successor_receivers() {
    type ActualProducer = fn(
        &mut Canonicalizer<'static>,
        &ResolvedFunction,
        &SharedWeighted,
        &mut ReplayBudget,
        &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<Successors>, Diagnostic>;
    let _actual_producer_result: ActualProducer =
        Canonicalizer::<'static>::successors_with_receiving;
    let f = function();
    let removed = 2 * size_of::<(
        &ResolvedFunction,
        &mut ReplayBudget,
        &Automaton,
        &ordered_weights::OrderedWeights,
        u64,
    )>() + 2 * size_of::<Result<Option<Successors>, Diagnostic>>()
        + size_of::<Option<Successors>>()
        + Canonicalizer::ACTIVE_REMOVED_LABEL_CALLS;
    for length in [0, 1, 4, 17] {
        for width in [0, 1, 2, 4, 12, 32] {
            for weight in [1, 2, u64::MAX] {
                let (a, sink, root) = graph(&f, length, width, false);
                let old = run(
                    false,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    64 * 1024 * 1024,
                    MAX_REPLAY_WORK_UNITS,
                    0,
                );
                let new = run(
                    true,
                    &f,
                    &a,
                    sink,
                    root,
                    weight,
                    64 * 1024 * 1024,
                    MAX_REPLAY_WORK_UNITS,
                    0,
                );
                same(&old, &new);
                assert_eq!(old.refused, new.refused);
                assert_eq!(old.heap, new.heap);
                if matches!(new.result, Ok(Some(_))) {
                    assert!(old.successors.complete && new.successors.complete);
                    let totals = |r: &Run| {
                        r.successors
                            .cells
                            .iter()
                            .flat_map(|v| v.iter())
                            .flat_map(|v| v.iter())
                            .fold((0, 0, 0), |(n, bytes, banks), c| {
                                (
                                    n + c.attempts,
                                    bytes + c.requested,
                                    banks + c.primitive_banks,
                                )
                            })
                    };
                    let (old_count, old_requests, old_banks) = totals(&old);
                    let (new_count, new_requests, new_banks) = totals(&new);
                    assert_eq!(old_count, new_count);
                    assert_eq!(old_banks, new_banks);
                    let removed_finish = Canonicalizer::active_finish_calls(false)
                        * Canonicalizer::ACTIVE_REMOVED_WEIGHTED_TRANSPORT;
                    let avoided_label_nodes = Canonicalizer::active_avoided_label_nodes(false)
                        * crate::cleanup_plan::replay::physical_storage::btree_node_upper::<
                            &Label,
                            paid_many::Targets,
                        >()
                        .unwrap();
                    let initial_leaf_links = Canonicalizer::active_initial_label_maps(false)
                        * Canonicalizer::ACTIVE_FIRST_LABEL_LINK_BYTES;
                    let retained = new_banks
                        * (Canonicalizer::ACTIVE_TARGET_FRAMES
                            + Canonicalizer::ACTIVE_APPEND_LABEL_FRAMES);
                    let removed_nodes =
                        new_banks * Canonicalizer::ACTIVE_REMOVED_TARGET_NODE_FRAMES;
                    // The fixed census records the entry request only. The
                    // moved weighted finish and node backing are body requests;
                    // they belong to the complete paid-byte identity below.
                    assert_eq!(
                        new_requests + new_count * removed + removed_nodes,
                        old_requests + retained,
                        "entry length={length} width={width} weight={weight}"
                    );
                    assert_eq!(
                        new.paid
                            + new_count * removed
                            + removed_nodes
                            + removed_finish
                            + avoided_label_nodes
                            + initial_leaf_links,
                        old.paid + retained,
                        "total length={length} width={width} weight={weight}"
                    );
                }
            }
        }
    }
}

#[test]
fn active_work_cuts_cycles_exact_short_zero_and_floor_keep_original_controls() {
    let f = function();
    let (a, sink, root) = graph(&f, 2, 1, false);
    let full = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        64 * 1024 * 1024,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert!(matches!(full.result, Ok(Some(_))) && !full.refused);
    for work in 0..=MAX_REPLAY_WORK_UNITS - full.remaining {
        same(
            &run(false, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0),
            &run(true, &f, &a, sink, root, 1, 64 * 1024 * 1024, work, 0),
        );
    }
    let exact = run(
        true,
        &f,
        &a,
        sink,
        root,
        1,
        full.paid,
        MAX_REPLAY_WORK_UNITS,
        0,
    );
    assert_eq!(exact.result, full.result);
    assert!(!exact.refused);
    balanced(&exact);
    for (limit, floor) in [
        (0, 0),
        (full.paid - 1, 0),
        (64 * 1024 * 1024, 64 * 1024 * 1024),
    ] {
        let short = run(
            true,
            &f,
            &a,
            sink,
            root,
            1,
            limit,
            MAX_REPLAY_WORK_UNITS,
            floor,
        );
        assert!(short.result.is_err() && short.refused);
        balanced(&short);
    }
    let (a, sink, root) = graph(&f, 1, 1, true);
    same(
        &run(
            false,
            &f,
            &a,
            sink,
            root,
            1,
            64 * 1024 * 1024,
            MAX_REPLAY_WORK_UNITS,
            0,
        ),
        &run(
            true,
            &f,
            &a,
            sink,
            root,
            1,
            64 * 1024 * 1024,
            MAX_REPLAY_WORK_UNITS,
            0,
        ),
    );
}
