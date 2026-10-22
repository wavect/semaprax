use super::*;
use crate::cleanup_plan::replay::merge_census::{failure, Failure};
use crate::test_allocation_observer::{current_metrics, measure};

fn graph(function: &ResolvedFunction) -> (Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let child = graph
        .end(
            function,
            &mut budget,
            sink,
            crate::cleanup_plan::replay::SkeletonTerminal::Failure,
        )
        .unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    for _ in 0..2 {
        graph
            .movement(
                function,
                &mut budget,
                root,
                Label::End(crate::cleanup_plan::replay::SkeletonTerminal::Success),
                child,
            )
            .unwrap();
    }
    (graph, sink, root)
}

#[test]
fn literal_one_query_matches_complete_owned_order_including_empty_and_longer_signatures() {
    use intern_arena::Query;
    let success = Label::End(crate::cleanup_plan::replay::SkeletonTerminal::Success);
    let failure = Label::End(crate::cleanup_plan::replay::SkeletonTerminal::Failure);
    for accepted in [0, 1, u64::MAX] {
        for child in [0, 1, usize::MAX] {
            let query = OneQuery {
                accepted,
                label: &success,
                child: &child,
            };
            let owned = (accepted, vec![(success.clone(), child)]);
            for stored in [
                (0, vec![]),
                (u64::MAX, vec![]),
                (accepted, vec![(failure.clone(), child)]),
                (accepted, vec![(success.clone(), 0)]),
                (accepted, vec![(success.clone(), child)]),
                (
                    accepted,
                    vec![(success.clone(), child), (failure.clone(), child)],
                ),
            ] {
                assert_eq!(query.compare(&stored), owned.cmp(&stored));
            }
        }
    }
}

fn actual(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    root: usize,
    limit: usize,
) -> Result<Option<usize>, Failure> {
    let (result, heap) = measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => return Err(failure(&error)),
        };
        let result = (|| {
            budget.reserve_array::<(usize, u64)>(function, 1, "borrowed singleton oracle root")?;
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "borrowed singleton complete physical owner",
            )?;
            Canonicalizer::new(graph, sink).id(function, vec![(root, 1)], &mut budget)
        })()
        .map_err(|error| failure(&error));
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        drop(budget);
        assert_eq!(current_metrics().allocated, stopped);
        result
    });
    assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
    result
}
fn primitive(function: &ResolvedFunction, event: Event, limit: usize) -> Result<(), Failure> {
    let mut budget =
        ReplayBudget::with_physical(function, limit).map_err(|error| failure(&error))?;
    let initial = budget.physical.as_ref().unwrap().charged();
    budget
        .reserve_bytes(
            function,
            event.paid - initial,
            "independent primitive prefix",
        )
        .map_err(|error| failure(&error))?;
    budget.remaining = event.remaining;
    budget.skeleton_remaining = event.skeleton_remaining;
    budget.skeleton_spent = event.skeleton_spent;
    budget
        .reserve_bytes(
            function,
            event.requested,
            "canonical lexical exit receiving",
        )
        .map_err(|error| failure(&error))
}

#[test]
fn one_atomic_exit_site_pays_full_cold_and_warm_primitive_and_preserves_six_field_frontiers() {
    let function = crate::cleanup_plan::replay::factored::lexical_plan_tests::fixture(
        crate::cleanup_plan::replay::factored::lexical_plan_tests::WHILE,
    )
    .1;
    let (graph, sink, root) = graph(&function);
    EVENTS.with(|slot| slot.set((true, [None; 16], 0)));
    let result = actual(&function, &graph, sink, root, 1 << 26);
    let (_, events, count) = EVENTS.with(|slot| slot.replace((false, [None; 16], 0)));
    assert!(result.is_ok());
    assert_eq!(
        count, 2,
        "exact two one-label Exits; empty uses its complete original route"
    );
    assert_eq!(
        PRIMITIVE,
        2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
            + 2 * size_of::<Result<(), Diagnostic>>()
    );
    for (index, event) in events[..count].iter().enumerate() {
        let event = event.unwrap();
        assert_eq!(event.cold, index == 0);
        assert_eq!(event.requested, if event.cold { COLD } else { WARM });
        for admitted in [0, event.requested - 1] {
            let limit = event.paid + admitted;
            let actual = actual(&function, &graph, sink, root, limit).unwrap_err();
            let expected = primitive(&function, event, limit).unwrap_err();
            assert_eq!(
                actual, expected,
                "all six fields use the actual new primitive request"
            );
        }
        assert!(primitive(&function, event, event.paid + event.requested).is_ok());
    }
}

#[test]
fn empty_and_real_two_label_routes_never_acquire_singleton_compound_or_primitive_bank() {
    let function = crate::cleanup_plan::replay::factored::lexical_plan_tests::fixture(
        crate::cleanup_plan::replay::factored::lexical_plan_tests::WHILE,
    )
    .1;
    let mut setup = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(&function, &mut setup).unwrap();
    let root = graph.node(&function, &mut setup).unwrap();
    for terminal in [
        crate::cleanup_plan::replay::SkeletonTerminal::Success,
        crate::cleanup_plan::replay::SkeletonTerminal::Failure,
    ] {
        graph
            .movement(&function, &mut setup, root, Label::End(terminal), sink)
            .unwrap();
    }
    for selected in [sink, root] {
        EVENTS.with(|slot| slot.set((true, [None; 16], 0)));
        let result = actual(&function, &graph, sink, selected, 1 << 26);
        let (_, _, count) = EVENTS.with(|slot| slot.replace((false, [None; 16], 0)));
        assert!(result.is_ok());
        assert_eq!(
            count, 0,
            "empty/general use only complete original branch/query/miss frontiers"
        );
    }
}
