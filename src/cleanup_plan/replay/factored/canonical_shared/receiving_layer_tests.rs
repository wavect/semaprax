//! Immediate831 old receiver stays complete; actual inlined boundaries qualify.
use super::super::SkeletonTerminal;
use super::before_receiving_layers;
use super::{Automaton, Canonicalizer, Diagnostic, Label, ReplayBudget, ResolvedFunction};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}
fn graph(function: &ResolvedFunction, width: usize, repeats: usize) -> (Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    let mut children = Vec::new();
    for _ in 0..width {
        children.push(
            graph
                .end(function, &mut budget, sink, SkeletonTerminal::Failure)
                .unwrap(),
        );
    }
    for child in children.into_iter().rev() {
        for _ in 0..repeats {
            graph
                .movement(
                    function,
                    &mut budget,
                    root,
                    Label::End(SkeletonTerminal::Success),
                    child,
                )
                .unwrap();
        }
    }
    (graph, sink, root)
}

#[test]
fn inline_targets_complete_immediate_receiver_keeps_states_signatures_and_every_work_frontier() {
    let function = fixture();
    for width in 0..=4 {
        for repeats in [1, 2, 4] {
            let (graph, sink, root) = graph(&function, width, repeats);
            for weight in [1, 2, u64::MAX] {
                let mut old = before_receiving_layers::Canonicalizer::new(&graph, sink);
                let mut current = Canonicalizer::new(&graph, sink);
                let mut old_budget = ReplayBudget::unobserved();
                let mut budget = ReplayBudget::unobserved();
                let old_remaining = old_budget.remaining;
                let expected = old.id(&function, vec![(root, weight)], &mut old_budget);
                let actual = current.id(&function, vec![(root, weight)], &mut budget);
                assert_eq!(
                    actual, expected,
                    "six fields: width={width} repeats={repeats} weight={weight}"
                );
                assert_eq!(budget.remaining, old_budget.remaining);
                assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
                assert_eq!(&current.ids, old.signatures());
                assert_eq!(&current.states, old.states());
                for state in current.states.keys() {
                    if state.len() == 1 {
                        assert_eq!(state.capacity(), 1);
                    }
                }
                let work = old_remaining - old_budget.remaining;
                for remaining in [0, work.saturating_sub(1), work] {
                    let mut old_budget = ReplayBudget::unobserved();
                    old_budget.remaining = remaining;
                    let mut budget = ReplayBudget::unobserved();
                    budget.remaining = remaining;
                    assert_eq!(
                        Canonicalizer::new(&graph, sink).id(
                            &function,
                            vec![(root, weight)],
                            &mut budget
                        ),
                        before_receiving_layers::Canonicalizer::new(&graph, sink).id(
                            &function,
                            vec![(root, weight)],
                            &mut old_budget
                        )
                    );
                    assert_eq!(budget.remaining, old_budget.remaining);
                    assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
                }
            }
        }
    }
}

fn observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    root: usize,
    limit: usize,
    old: bool,
) -> (Result<usize, ()>, Metrics) {
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
            budget.reserve_array::<(usize, u64)>(function, 1, "inline target root")?;
            let id = if old {
                let mut receiver = before_receiving_layers::Canonicalizer::new(graph, sink);
                receiver.id(function, vec![(root, 1)], &mut budget)?
            } else {
                let mut receiver = Canonicalizer::new(graph, sink);
                receiver.id(function, vec![(root, 1)], &mut budget)?
            };
            assert!(id.is_some());
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged())
        })();
        assert!(
            current_metrics().allocated <= budget.physical.as_ref().unwrap().charged(),
            "whole actual constructor backing remains funded, including refused frontiers"
        );
        let stopped = current_metrics().allocated;
        let result = result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        });
        drop(budget);
        assert_eq!(
            current_metrics().allocated,
            stopped,
            "post-result publication and final Drop acquire no backing"
        );
        result
    })
}
#[test]
fn inline_targets_actual_old_current_reserved_allocated_peak_and_whole_drop_are_measured() {
    let function = fixture();
    for width in 0..=4 {
        let (graph, sink, root) = graph(&function, width, 2);
        let mut tuples = Vec::new();
        for old in [true, false] {
            let (paid, physical) = observe(&function, &graph, sink, root, 1 << 24, old);
            let paid = paid.unwrap();
            assert!(!physical.invalid && physical.live == 0);
            assert_eq!(physical.allocated, physical.freed);
            assert!(physical.allocated <= paid && physical.highwater <= paid);
            tuples.push((paid, physical));
        }
        eprintln!("inline-target-layers width={width} immediate831/current={tuples:?}");
        assert!(
            tuples[1].0 < tuples[0].0,
            "actual removed Result/argument/Once layers, no fee exemption"
        );
    }
}
#[test]
fn inline_successors_every_actual_byte_cut_covers_promotion_finish_and_zero_without_post_error_drop(
) {
    let function = fixture();
    for width in [0, 1, 2, 4] {
        let (graph, sink, root) = graph(&function, width, 2);
        let (complete, _) = observe(&function, &graph, sink, root, 1 << 24, false);
        let exact = complete.unwrap();
        for limit in 0..=exact {
            let (result, physical) = observe(&function, &graph, sink, root, limit, false);
            assert_eq!(
                result.is_ok(),
                limit == exact,
                "width={width} budget={limit}/{exact}"
            );
            assert!(!physical.invalid && physical.live == 0);
            assert_eq!(physical.allocated, physical.freed);
        }
    }
}

#[test]
fn complete_immediate_receiver_helpers_and_all_seven_original_controls_remain_exact() {
    let saved = include_str!("before_receiving_layers.rs");
    let accessors = "\nimpl Canonicalizer<'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n";
    let saved = saved.strip_suffix(accessors).unwrap();
    let registrations = "#[cfg(test)]\nmod before_alias;\n#[cfg(test)]\nmod before_index;\n#[cfg(test)]\nmod before_targets;\nmod epsilon_alias;\nmod memo;\n#[cfg(test)]\nmod singleton_target_tests;\nmod successor_targets;\n";
    let restored = saved
        .replacen("use super::super::{", "use super::{", 1)
        .replacen(
            "use super::{epsilon_alias, memo, successor_targets, tests};\n",
            registrations,
            1,
        )
        + "#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\npub(super) mod app_census;\n";
    assert_eq!(restored, include_str!("../../../../../docs/evidence/singleton-target-receiving-layers-831/complete-receiver-before.rs.txt"));
    assert_eq!(include_str!("singleton_target_tests.rs"), include_str!("../../../../../docs/evidence/singleton-target-receiving-layers-831/complete-seven-controls-before.rs.txt"));
    let helpers = include_str!("successor_targets.rs")
        .replacen("#[cfg(test)]\nuse super::{", "use super::{", 1)
        .replacen("use std::collections::BTreeMap;\n#[cfg(test)]\nuse std::mem::size_of;", "use std::{collections::BTreeMap, mem::size_of};", 1)
        .replacen("// Complete prior helpers qualify old receiving boundaries only.\n#[cfg(test)]\nimpl Targets {", "impl Targets {", 1);
    assert_eq!(helpers, include_str!("../../../../../docs/evidence/singleton-target-receiving-layers-831/complete-target-helpers-before.rs.txt"));
}
