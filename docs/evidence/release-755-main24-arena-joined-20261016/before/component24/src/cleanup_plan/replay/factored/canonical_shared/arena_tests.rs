//! Independent complete immediate canonical receiver and actual arena owners.
use super::{
    before_arena, Automaton, Canonicalizer, Diagnostic, Label, ReplayBudget, ResolvedFunction,
};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn ordered_interning_preserves_complete_ids_signatures_work_overflow_and_cycle() {
    let function = fixture();
    let (graph, sink, roots) = super::tests::automaton(&function);
    for root in roots.into_iter().chain([sink]) {
        for weight in [0, 1, 2, u64::MAX] {
            for work in [0, 1, 2, 4, 16, 128, 1024] {
                let mut old = before_arena::Canonicalizer::new(&graph, sink);
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
) -> (Result<(usize, usize, u64), ()>, Metrics) {
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
            if old {
                let mut owner = before_arena::Canonicalizer::new(graph, sink);
                for root in roots {
                    budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                    let id = owner.id(function, vec![(*root, 1)], &mut budget)?.unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
                }
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
            }
            Ok::<_, Diagnostic>((
                budget.physical.as_ref().unwrap().charged(),
                budget.remaining,
                checksum,
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
        let (graph, sink, roots) = corpus(&function, shape);
        let mut observations = [(0, 0, 0); 2];
        for (index, old) in [true, false].into_iter().enumerate() {
            let (result, metrics) = observe(&function, &graph, sink, &roots, 1 << 26, old);
            observations[index] = result.unwrap();
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!(
                "arena shape={shape} old={old} reserved={} actual={metrics:?}",
                observations[index].0
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
        if shape == 0 {
            assert!(
                observations[1].0 < observations[0].0,
                "whole duplicate-signature corpus, unchanged old receiver"
            );
        }
        // Unique-wide and tiny cases are reported even if new controls/spare
        // backing are unfavorable. No universal inequality is asserted.
    }
}
