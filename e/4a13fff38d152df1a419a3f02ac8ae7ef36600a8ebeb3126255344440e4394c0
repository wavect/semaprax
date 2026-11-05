//! Complete d962 control; all measurements remain pending owning execution.
use super::{before_empty_signature, Canonicalizer, Diagnostic, ReplayBudget, ResolvedFunction};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}
fn graph(function: &ResolvedFunction, width: usize) -> (super::Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = super::Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    for _ in 0..width {
        let child = graph
            .end(
                function,
                &mut budget,
                sink,
                super::super::SkeletonTerminal::Failure,
            )
            .unwrap();
        for _ in 0..2 {
            graph
                .movement(
                    function,
                    &mut budget,
                    root,
                    super::Label::End(super::super::SkeletonTerminal::Success),
                    child,
                )
                .unwrap();
        }
    }
    (graph, sink, root)
}
#[test]
fn empty_signatures_match_complete_d962_ids_states_six_fields_and_all_work_frontiers() {
    let function = fixture();
    for width in 0..=4 {
        let (graph, sink, root) = graph(&function, width);
        for weight in [0, 1, 2, u64::MAX] {
            let mut a = ReplayBudget::unobserved();
            let initial = a.remaining;
            let mut old = before_empty_signature::Canonicalizer::new(&graph, sink);
            let expected = old.id(&function, vec![(root, weight)], &mut a);
            let work = initial - a.remaining;
            for limit in [0, work.saturating_sub(1), work, initial] {
                let mut a = ReplayBudget::unobserved();
                let mut b = ReplayBudget::unobserved();
                a.remaining = limit;
                b.remaining = limit;
                let mut old = before_empty_signature::Canonicalizer::new(&graph, sink);
                let mut current = Canonicalizer::new(&graph, sink);
                let before = old.id(&function, vec![(root, weight)], &mut a);
                let after = current.id(&function, vec![(root, weight)], &mut b);
                assert_eq!(
                    after, before,
                    "all diagnostic fields width={width} weight={weight} work={limit}"
                );
                if limit == initial {
                    assert_eq!(after, expected);
                }
                assert_eq!(
                    (a.remaining, a.skeleton_remaining),
                    (b.remaining, b.skeleton_remaining)
                );
                assert_eq!(
                    current.ids.iter().collect::<Vec<_>>(),
                    old.signatures().iter().collect::<Vec<_>>()
                );
                assert_eq!(
                    current.states.iter().collect::<Vec<_>>(),
                    old.states().iter().collect::<Vec<_>>()
                );
                assert!(
                    current.child_ids.is_empty()
                        && current.frames.is_empty()
                        && current.open.is_empty()
                );
            }
        }
    }
}
fn observe(
    function: &ResolvedFunction,
    graph: &super::Automaton,
    sink: usize,
    root: usize,
    limit: usize,
    old: bool,
) -> (Result<(usize, usize, Option<usize>), ()>, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                return Err(());
            }
        };
        let result = (|| {
            // Both arms execute the complete actual constructor, with identical
            // real receiver admission before any canonical owner is constructed.
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "canonical ordered dictionary owner receiving",
            )?;
            budget.reserve_array::<(usize, u64)>(function, 1, "empty signature root")?;
            let id = if old {
                let mut owner = before_empty_signature::Canonicalizer::new(graph, sink);
                owner.id(function, vec![(root, 1)], &mut budget)?
            } else {
                let mut owner = Canonicalizer::new(graph, sink);
                owner.id(function, vec![(root, 1)], &mut budget)?
            };
            Ok::<_, Diagnostic>((
                budget.physical.as_ref().unwrap().charged(),
                budget.remaining,
                id,
            ))
        })();
        assert!(
            current_metrics().allocated <= budget.physical.as_ref().unwrap().charged(),
            "all actual backing funded at every tested frontier"
        );
        let result = result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        });
        let stopped = current_metrics().allocated;
        drop(budget);
        assert_eq!(
            current_metrics().allocated,
            stopped,
            "whole Drop allocates nothing"
        );
        result
    })
}
#[test]
fn real_empty_signature_old_current_paid_heap_peak_and_exact_short_zero_drop() {
    let function = fixture();
    for width in 0..=4 {
        let (graph, sink, root) = graph(&function, width);
        let mut rows = Vec::new();
        for old in [true, false] {
            let (result, metrics) = observe(&function, &graph, sink, root, 1 << 24, old);
            let row = result.unwrap();
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!(
                "empty-signature width={width} old={old} paid={} actual={metrics:?}",
                row.0
            );
            for limit in [0, row.0 - 1, row.0] {
                let (result, actual) = observe(&function, &graph, sink, root, limit, old);
                assert_eq!(result.is_ok(), limit == row.0);
                assert!(!actual.invalid && actual.live == 0);
                assert_eq!(actual.allocated, actual.freed);
            }
            rows.push(row);
        }
        assert_eq!((rows[0].1, rows[0].2), (rows[1].1, rows[1].2));
        if width == 0 {
            assert!(
                rows[1].0 < rows[0].0,
                "real absent query/iterators, no fee exemption"
            );
        }
        // Wider cases are reported with their actual added branch receivers;
        // no universal paid/heap reduction or application fit is asserted.
    }
}
#[test]
fn empty_signature_carriers_and_complete_original_receiver_are_bound() {
    use std::mem::size_of;
    assert!(
        super::empty_signature::BRANCH_CONTROLS + super::empty_signature::EMPTY_CONTROLS
            < super::intern_arena::SignatureQuery::EXIT_CONTROLS
                + super::intern_arena::SignatureQuery::MISS_CONTROLS
    );
    assert_eq!(
        Canonicalizer::CONSTRUCTOR_CONTROLS,
        before_empty_signature::Canonicalizer::CONSTRUCTOR_CONTROLS
    );
    assert_eq!(
        size_of::<Canonicalizer<'_>>(),
        size_of::<before_empty_signature::Canonicalizer<'_>>()
    );
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/empty-signature-receiving-831-20261016/canonical_shared.before.rs.txt"
    ));
    let start = original.find("// Invoked only after preflight").unwrap();
    let end = original[start..]
        .find("\n#[cfg(test)]\nmod tests;")
        .unwrap()
        + start;
    assert!(
        include_str!("before_empty_signature.rs").contains(&original[start..end]),
        "complete immediate receiver, no old constructor calling new branch"
    );
    assert!(
        original.contains("SignatureQuery::EXIT_CONTROLS")
            && original.contains("SignatureQuery::MISS_CONTROLS")
    );
}

#[test]
fn empty_signature_keeps_sticky_function_budget_and_restored_owner_denial() {
    let function = fixture();
    let wrong = function.clone();
    let (graph, sink, root) = graph(&function, 0);
    for attack in 0..2 {
        let mut verdicts = Vec::new();
        for old in [true, false] {
            let mut budget = ReplayBudget::unobserved();
            let mut foreign = ReplayBudget::unobserved();
            macro_rules! exercise {
                ($constructor:expr) => {{
                    let mut owner = $constructor;
                    assert!(owner
                        .id(&function, vec![(root, 1)], &mut budget)
                        .unwrap()
                        .is_some());
                    let work = budget.remaining;
                    let foreign_work = foreign.remaining;
                    let first = if attack == 0 {
                        owner.id(&wrong, vec![(root, 1)], &mut budget).unwrap_err()
                    } else {
                        owner
                            .id(&function, vec![(root, 1)], &mut foreign)
                            .unwrap_err()
                    };
                    assert_eq!(budget.remaining, work);
                    assert_eq!(foreign.remaining, foreign_work);
                    let restored = owner
                        .id(&function, vec![(root, 1)], &mut budget)
                        .unwrap_err();
                    assert_eq!(
                        budget.remaining, work,
                        "restoration cannot revive retained authority"
                    );
                    (first, restored)
                }};
            }
            verdicts.push(if old {
                exercise!(before_empty_signature::Canonicalizer::new(&graph, sink))
            } else {
                exercise!(Canonicalizer::new(&graph, sink))
            });
        }
        assert_eq!(verdicts[0], verdicts[1], "all six original denial fields");
    }
}
