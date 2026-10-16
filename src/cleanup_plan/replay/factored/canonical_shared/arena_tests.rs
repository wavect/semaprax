//! Independent complete immediate canonical receiver and actual arena owners.
use super::{
    before_arena, Automaton, Canonicalizer, Diagnostic, Label, ReplayBudget, ResolvedFunction,
};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

// Actual signature dictionary branch observations. Const TLS has no heap
// backing, carries no payer, and is reset before each measured whole owner.
thread_local! {
    static SIGNATURE_LOOKUPS: std::cell::Cell<(usize, usize)> = const {
        std::cell::Cell::new((0, 0))
    };
}
pub(super) fn signature_lookup(hit: bool) {
    SIGNATURE_LOOKUPS.with(|value| {
        let (hits, misses) = value.get();
        value.set((hits + usize::from(hit), misses + usize::from(!hit)));
    });
}
fn lookups() -> (usize, usize) {
    SIGNATURE_LOOKUPS.with(std::cell::Cell::get)
}

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn ordered_interning_preserves_complete_ids_signatures_work_overflow_and_cycle() {
    let function = fixture();
    let (old_graph, old_sink, old_roots) = super::tests::automaton(&function);
    let (graph, sink, roots) = super::tests::automaton(&function);
    assert_eq!((old_sink, &old_roots), (sink, &roots));
    assert!(!std::ptr::eq(&old_graph, &graph));
    for root in roots.into_iter().chain([sink]) {
        for weight in [0, 1, 2, u64::MAX] {
            for work in [0, 1, 2, 4, 16, 128, 1024] {
                let mut old = before_arena::Canonicalizer::new(&old_graph, old_sink);
                let mut current = Canonicalizer::new(&graph, sink);
                let mut a = ReplayBudget::unobserved();
                let mut b = ReplayBudget::unobserved();
                a.remaining = work;
                b.remaining = work;
                let expected = old.id(&function, vec![(root, weight)], &mut a);
                let actual = current.id(&function, vec![(root, weight)], &mut b);
                assert_eq!(
                    actual, expected,
                    "six fields root={root} weight={weight} work={work}"
                );
                assert_eq!(
                    (a.remaining, a.skeleton_remaining),
                    (b.remaining, b.skeleton_remaining)
                );
                assert_eq!(&current.ids, old.signatures());
                assert_eq!(&current.states, old.states());
                assert!(
                    current.cursor.is_empty()
                        && current.frames.is_empty()
                        && current.open.is_empty()
                );
            }
        }
    }
    for shape in 0..3 {
        let (old_graph, old_sink, old_roots) = corpus(&function, shape);
        let (graph, sink, roots) = corpus(&function, shape);
        assert_eq!((old_sink, &old_roots), (sink, &roots));
        assert!(!std::ptr::eq(&old_graph, &graph));
        let mut old = before_arena::Canonicalizer::new(&old_graph, old_sink);
        let mut current = Canonicalizer::new(&graph, sink);
        let mut a = ReplayBudget::unobserved();
        let mut b = ReplayBudget::unobserved();
        for (old_root, root) in old_roots.into_iter().zip(roots) {
            assert_eq!(
                current.id(&function, vec![(root, 1)], &mut b),
                old.id(&function, vec![(old_root, 1)], &mut a),
            );
            assert_eq!(&current.ids, old.signatures());
            assert_eq!(&current.states, old.states());
            assert_eq!(
                (a.remaining, a.skeleton_remaining),
                (b.remaining, b.skeleton_remaining)
            );
        }
    }
    for old in [true, false] {
        failures_keep_exact_owner_and_first_receipt(old);
    }
}

// Ordinary immutable input construction is outside the canonical owner window.
// Duplicate signatures retain distinct weighted-state keys; the wide case has
// distinct real observation labels and signatures instead of padded old fees.
fn corpus(function: &ResolvedFunction, shape: usize) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    if shape == 2 {
        return (graph, sink, vec![sink]);
    }
    let mut roots = Vec::new();
    for row in 0..32 {
        let root = graph.node(function, &mut budget).unwrap();
        for column in 0..if shape == 0 { 128 } else { 24 } {
            // Duplicate corpus repeats the same full labels; unique-wide uses
            // distinct labels on each row. Both retain distinct state keys.
            let identity_row = if shape == 0 { 0 } else { row };
            let label = Label::Observation(super::Rc::new(
                super::super::super::SkeletonObservation::Boolean {
                    expression: crate::hir::ExpressionId::from_owned(format!(
                        "arena.row.{identity_row}.column.{column}"
                    )),
                    value: column % 2 == 0,
                },
            ));
            graph
                .movement(function, &mut budget, root, label, sink)
                .unwrap();
        }
        roots.push(root);
    }
    (graph, sink, roots)
}

fn observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    roots: &[usize],
    limit: usize,
    old: bool,
) -> (Result<(usize, usize, u64, [usize; 4]), ()>, Metrics) {
    SIGNATURE_LOOKUPS.with(|value| value.set((0, 0)));
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(value) => value,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                return Err(());
            }
        };
        let result = (|| {
            let mut checksum = 0u64;
            let counts;
            if old {
                let mut owner = before_arena::Canonicalizer::new(graph, sink);
                for root in roots {
                    budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                    let id = owner.id(function, vec![(*root, 1)], &mut budget)?.unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
                }
                let (hits, misses) = lookups();
                counts = [hits, misses, owner.signatures().len(), owner.states().len()];
            } else {
                // The same explicit shipping acquisition precedes the actual
                // new Canonicalizer/arena header constructors.
                budget.reserve_bytes(
                    function,
                    Canonicalizer::CONSTRUCTOR_CONTROLS,
                    "canonical ordered dictionary owner receiving",
                )?;
                let mut owner = Canonicalizer::new(graph, sink);
                for root in roots {
                    budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                    let id = owner.id(function, vec![(*root, 1)], &mut budget)?.unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
                }
                let (hits, misses) = lookups();
                counts = [hits, misses, owner.ids.len(), owner.states.len()];
            }
            Ok::<_, Diagnostic>((
                budget.physical.as_ref().unwrap().charged(),
                budget.remaining,
                checksum,
                counts,
            ))
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let result = result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        });
        drop(budget);
        result
    })
}

#[test]
fn whole_arena_owner_reports_duplicate_unique_wide_costs_growth_refusal_and_complete_drop() {
    let function = fixture();
    for shape in 0..3 {
        let mut observations = [(0, 0, 0, [0; 4]); 2];
        for (index, old) in [true, false].into_iter().enumerate() {
            // Separate construction for each whole receiver; no graph/Rc roots
            // or label allocation is shared between the old/current arms.
            let (graph, sink, roots) = corpus(&function, shape);
            let (result, metrics) = observe(&function, &graph, sink, &roots, 1 << 26, old);
            observations[index] = result.unwrap();
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!(
                "arena shape={shape} old={old} reserved={} hits={} misses={} signature_nodes={} state_nodes={} actual={metrics:?}",
                observations[index].0, observations[index].3[0], observations[index].3[1],
                observations[index].3[2], observations[index].3[3]
            );
            let paid = observations[index].0;
            for limit in [0, paid / 2, paid - 1, paid] {
                let (result, metrics) = observe(&function, &graph, sink, &roots, limit, old);
                assert_eq!(result.is_ok(), limit == paid);
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
        }
        assert_eq!(observations[0].1, observations[1].1, "exact original work");
        assert_eq!(observations[0].2, observations[1].2, "insertion ordinals");
        assert_eq!(
            observations[0].3, observations[1].3,
            "actual lookup and node counts"
        );
        let [hits, misses, signatures, states] = observations[1].3;
        assert_eq!(
            misses, signatures,
            "one actual signature insertion per miss"
        );
        assert_eq!(
            hits + misses,
            states,
            "one actual signature lookup per published state"
        );
        if shape == 0 {
            assert!(hits > 0 && misses > 0 && states > signatures);
            assert!(
                observations[1].0 < observations[0].0,
                "whole duplicate-signature corpus, unchanged old receiver"
            );
        }
        if shape == 1 {
            assert_eq!(
                hits, 0,
                "all-unique wide signatures must not fabricate hits"
            );
            assert!(misses > 1 && states == signatures);
        }
        // Unique-wide and tiny cases are reported even if new controls/spare
        // backing are unfavorable. No universal inequality is asserted.
    }
}

// Exercise the actual dictionary owner after a real completed insertion. The
// rejected calls must not read either dictionary or touch a foreign payer, and
// restoring the original function/ledger cannot revive a poisoned owner.
fn failures_keep_exact_owner_and_first_receipt(old: bool) {
    use crate::bounded_output::{
        active_remaining, reserve_active_required, with_limit_usage_refusal,
        with_limit_usage_refusal_site,
    };
    let function = fixture();
    let wrong = function.clone();
    for attack in 0..4 {
        let (graph, sink, roots) = corpus(&function, 1);
        let ((expected, allocation), refused, _, first, site) =
            with_limit_usage_refusal_site(1 << 26, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let mut foreign = ReplayBudget::new();
                    if !old {
                        budget
                            .reserve_bytes(
                                &function,
                                Canonicalizer::CONSTRUCTOR_CONTROLS,
                                "canonical ordered dictionary owner receiving",
                            )
                            .unwrap();
                    }
                    macro_rules! exercise {
                        ($owner:ident = $constructor:expr; $counts:expr) => {{
                            let mut $owner = $constructor;
                            budget
                                .reserve_array::<(usize, u64)>(&function, 1, "arena bound root")
                                .unwrap();
                            assert!($owner
                                .id(&function, vec![(roots[0], 1)], &mut budget)
                                .unwrap()
                                .is_some());
                            // Both call inputs are real prepaid owners before a
                            // deliberately failed/stale authority is selected.
                            budget
                                .reserve_array::<(usize, u64)>(&function, 1, "arena rejected root")
                                .unwrap();
                            let input = vec![(roots[1], 1)];
                            budget
                                .reserve_array::<(usize, u64)>(&function, 1, "arena restored root")
                                .unwrap();
                            let retry = vec![(roots[1], 1)];
                            let counts = $counts;
                            assert!(counts.0 > 0 && counts.1 > 0);
                            let queries = lookups();
                            let work = budget.remaining;
                            let mut expected = None;
                            if attack == 0 {
                                let remaining = active_remaining().unwrap();
                                let requested = remaining.checked_add(1).unwrap();
                                assert!(!reserve_active_required(requested));
                                expected = Some((requested, remaining));
                            }
                            let paid = active_remaining();
                            let allocated = current_metrics().allocated;
                            let error = if attack == 1 {
                                $owner.id(&wrong, input, &mut budget)
                            } else if attack == 2 {
                                $owner.id(&function, input, &mut foreign)
                            } else if attack == 3 {
                                let (result, child_refused, child_paid, child_first) =
                                    with_limit_usage_refusal(1 << 26, || {
                                        let at = current_metrics().allocated;
                                        let result = $owner.id(&function, input, &mut budget);
                                        assert_eq!(current_metrics().allocated, at);
                                        result
                                    });
                                assert!(!child_refused && child_paid == 0 && child_first.is_none());
                                result
                            } else {
                                $owner.id(&function, input, &mut budget)
                            };
                            assert_eq!(error, Err(Diagnostic::io("SPX-H006", String::new())));
                            assert_eq!(active_remaining(), paid);
                            if attack != 3 {
                                assert_eq!(current_metrics().allocated, allocated);
                            }
                            // The nested scope itself owns one ordinary TLS
                            // budget allocation; the rejected call inside it
                            // was checked separately above.
                            let allocated = current_metrics().allocated;
                            assert_eq!(budget.remaining, work);
                            assert_eq!($counts, counts);
                            assert_eq!(lookups(), queries);
                            assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                            assert_eq!(
                                $owner.id(&function, retry, &mut budget),
                                Err(Diagnostic::io("SPX-H006", String::new())),
                            );
                            assert_eq!(active_remaining(), paid);
                            assert_eq!(current_metrics().allocated, allocated);
                            assert_eq!(budget.remaining, work);
                            assert_eq!($counts, counts);
                            assert_eq!(lookups(), queries);
                            drop($owner);
                            expected
                        }};
                    }
                    let expected = if old {
                        exercise!(owner = before_arena::Canonicalizer::new(&graph, sink);
                            (owner.signatures().len(), owner.states().len()))
                    } else {
                        exercise!(owner = Canonicalizer::new(&graph, sink);
                            (owner.ids.len(), owner.states.len()))
                    };
                    drop(foreign);
                    drop(budget);
                    expected
                })
            });
        if let Some((requested, remaining)) = expected {
            assert!(refused && site.is_some());
            let first = first.unwrap();
            assert_eq!((first.requested, first.remaining), (requested, remaining));
            assert_eq!(first.floor, 0);
            assert_eq!(first.stage, None);
        }
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn vec_borrowed_queries_and_zipped_misses_keep_full_old_key_order() {
    use super::intern_arena::{Query, SignatureQuery};
    use std::mem::size_of;
    struct PreviousQuery<'a> {
        accepted: u64,
        successors: &'a [(Label, super::SharedWeighted)],
        children: &'a [usize],
    }
    let previous_controls = 2 * size_of::<PreviousQuery<'_>>()
        + 2 * size_of::<&Vec<(Label, super::SharedWeighted)>>()
        + 2 * size_of::<&Vec<usize>>()
        + size_of::<&[(Label, super::SharedWeighted)]>()
        + size_of::<&[usize]>();
    assert!(SignatureQuery::CONTROLS < previous_controls);
    let function = fixture();
    for shape in 0..3 {
        let (graph, sink, roots) = corpus(&function, shape);
        let mut owner = Canonicalizer::new(&graph, sink);
        let mut budget = ReplayBudget::unobserved();
        for root in roots {
            owner.id(&function, vec![(root, 1)], &mut budget).unwrap();
        }
        for (key, _) in owner.ids.iter() {
            let successors: Vec<_> = key.1.iter().map(|(label, _)| {
                (label.clone(), super::Rc::new(super::SmallWeighted::One([(sink, 1)])))
            }).collect();
            let children: Vec<_> = key.1.iter().map(|(_, id)| *id).collect();
            let previous = PreviousQuery {
                accepted: key.0, successors: &successors, children: &children,
            };
            assert_eq!(previous.accepted, key.0);
            assert_eq!(previous.successors.len(), previous.children.len());
            let query = SignatureQuery { accepted: key.0, successors: &successors, children: &children };
            for (stored, _) in owner.ids.iter() {
                assert_eq!(query.compare(stored), key.cmp(stored));
            }
            let expected: Vec<_> = successors.iter().enumerate()
                .map(|(index, (label, _))| (label.clone(), children[index])).collect();
            let actual: Vec<_> = successors.into_iter().zip(&children)
                .map(|((label, _), id)| (label, *id)).collect();
            assert_eq!(actual, expected);
            assert_eq!(actual, key.1);
        }
    }
}
