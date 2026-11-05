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
                super::inline_weighted::assert_states(&current.states, old.states());
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
    let saved = crate::source_reflows_merge::assembly_view(
        "842",
        "src/cleanup_plan/replay/factored/canonical_shared/before_receiving_layers.rs",
        include_str!("before_receiving_layers.rs"),
    );
    let accessors = "\nimpl Canonicalizer<'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n";
    let saved = saved.strip_suffix(accessors).unwrap();
    let registrations = "#[cfg(test)]\nmod before_alias;\n#[cfg(test)]\nmod before_index;\n#[cfg(test)]\nmod before_targets;\nmod epsilon_alias;\nmod memo;\n#[cfg(test)]\nmod singleton_target_tests;\nmod successor_targets;\n";
    let restored = saved
        .replacen("use super::super::{", "use super::{", 1)
        .replacen(
            "use super::{app_census, epsilon_alias, memo, successor_targets, tests};\n",
            registrations,
            1,
        )
        + "#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\npub(super) mod app_census;\n";
    assert_eq!(restored, include_str!("../../../../../docs/evidence/singleton-target-receiving-layers-831/complete-receiver-before.rs.txt"));
    let controls = crate::source_reflows_merge::consumer_input(
        "receiving831",
        "src/cleanup_plan/replay/factored/canonical_shared/singleton_target_tests.rs",
        include_str!("singleton_target_tests.rs"),
    );
    assert_eq!(controls, include_str!("../../../../../docs/evidence/singleton-target-receiving-layers-831/complete-seven-controls-before.rs.txt"));
    let helper_input = crate::source_reflows_merge::consumer_input(
        "receiving831",
        "src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs",
        include_str!("successor_targets.rs"),
    );
    let helpers = helper_input
        .replacen("#[cfg(test)]\nuse super::{", "use super::{", 1)
        .replacen("use std::collections::BTreeMap;\n#[cfg(test)]\nuse std::mem::size_of;", "use std::{collections::BTreeMap, mem::size_of};", 1)
        .replacen("// Complete prior helpers qualify old receiving boundaries only.\n#[cfg(test)]\nimpl Targets {", "impl Targets {", 1);
    assert_eq!(helpers, include_str!("../../../../../docs/evidence/singleton-target-receiving-layers-831/complete-target-helpers-before.rs.txt"));
}

#[test]
fn branch_closed_full_current_sources_restore_every_immediate_e93f_byte() {
    use sha2::{Digest, Sha256};
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/branch-closed-receiving-831-20261015/current-to-e93f.json"
    )))
    .unwrap();
    let sources = [
        (
            "src/cleanup_plan/replay/factored/canonical_shared.rs",
            include_str!("../canonical_shared.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-closed-receiving-831-20261015/before-0.txt"
            )),
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs",
            include_str!("ordered_weights.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-closed-receiving-831-20261015/before-1.txt"
            )),
        ),
        (
            "src/source_reflows_merge.rs",
            include_str!("../../../../source_reflows_merge.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-closed-receiving-831-20261015/before-2.txt"
            )),
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/receiving_layer_tests.rs",
            include_str!("receiving_layer_tests.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-closed-receiving-831-20261015/before-3.txt"
            )),
        ),
    ];
    assert_eq!(rows.as_array().unwrap().len(), sources.len());
    for ((path, current, original), row) in sources.into_iter().zip(rows.as_array().unwrap()) {
        let current_view = crate::source_reflows_merge::branch_closed831_input(path, current);
        let current = current_view.as_str();
        assert_eq!(row["source"], path);
        assert_eq!(
            row["current_sha256"],
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            )
        );
        assert_eq!(
            row["original_sha256"],
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            )
        );
        let edits = row["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0]["new"], current);
        assert_eq!(edits[0]["old"], original);
    }
}

// This reports the full immediate b742 method, not an inflated historical fee.
fn borrowed_single_observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    roots: &[usize],
    old: bool,
    limit: usize,
) -> (
    Result<(usize, usize, usize, usize, usize, u64), ()>,
    Metrics,
) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                return Err(());
            }
        };
        let outcome = (|| {
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "single signature complete owner",
            )?;
            let mut owner = Canonicalizer::new(graph, sink);
            budget.reserve_bytes(
                function,
                super::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "single signature held receiver",
            )?;
            let mut receiving =
                super::closure_receiving::ClosureReceiving::from_prepaid(function, &budget);
            let initial = budget.remaining;
            let mut checksum = 0_u64;
            for root in roots {
                budget.reserve_array::<(usize, u64)>(function, 1, "single signature root")?;
                let id = if old {
                    owner.before_borrowed_single_id_with_receiving(
                        function,
                        vec![(*root, 1)],
                        &mut budget,
                        &mut receiving,
                    )
                } else {
                    owner.id_with_receiving(function, vec![(*root, 1)], &mut budget, &mut receiving)
                }?
                .expect("acyclic exact one-label graph");
                checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
            }
            Ok::<_, Diagnostic>((
                budget.physical.as_ref().unwrap().charged(),
                initial - budget.remaining,
                owner.states.len(),
                owner.ids.len(),
                owner.child_ids.capacity(),
                checksum,
            ))
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        let outcome = outcome.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
        });
        drop(budget);
        assert_eq!(
            current_metrics().allocated,
            stopped,
            "complete singleton signature Drop allocates nothing"
        );
        outcome
    })
}

#[test]
fn borrowed_single_complete_immediate_method_keeps_unique_and_duplicate_hits_and_reports_full_drop()
{
    let function = fixture();
    for width in 1..=4 {
        let (graph, sink, root) = graph(&function, width, 2);
        let mut reports = Vec::new();
        for old in [true, false] {
            let (result, heap) =
                borrowed_single_observe(&function, &graph, sink, &[root], old, 1 << 26);
            let receipt = result.unwrap();
            assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
            assert!(heap.allocated <= receipt.0 && heap.highwater <= receipt.0);
            assert_eq!(
                receipt.4 == 0,
                !old,
                "only the direct one-label path omits child-ID backing"
            );
            reports.push((receipt, heap));
        }
        assert_eq!(
            (
                reports[0].0 .1,
                reports[0].0 .2,
                reports[0].0 .3,
                reports[0].0 .5
            ),
            (
                reports[1].0 .1,
                reports[1].0 .2,
                reports[1].0 .3,
                reports[1].0 .5
            )
        );
        eprintln!("borrowed-single unique width={width} immediate/current={reports:?}");
    }
    let mut setup = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(&function, &mut setup).unwrap();
    let first = graph
        .end(
            &function,
            &mut setup,
            sink,
            super::super::SkeletonTerminal::Success,
        )
        .unwrap();
    let second = graph
        .end(
            &function,
            &mut setup,
            sink,
            super::super::SkeletonTerminal::Success,
        )
        .unwrap();
    let roots = [first, second, first, second];
    let mut reports = Vec::new();
    for old in [true, false] {
        let (result, heap) = borrowed_single_observe(&function, &graph, sink, &roots, old, 1 << 26);
        let exact = result.unwrap();
        assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
        assert_eq!(
            (exact.2, exact.3),
            (3, 2),
            "distinct completed states share one equal nonempty signature"
        );
        for limit in [0, exact.0 - 1, exact.0] {
            let (result, physical) =
                borrowed_single_observe(&function, &graph, sink, &roots, old, limit);
            assert_eq!(result.is_ok(), limit == exact.0);
            if let Ok(actual) = result {
                assert_eq!(actual, exact);
            }
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
        reports.push((exact, heap));
    }
    assert_eq!(
        (
            reports[0].0 .1,
            reports[0].0 .2,
            reports[0].0 .3,
            reports[0].0 .5
        ),
        (
            reports[1].0 .1,
            reports[1].0 .2,
            reports[1].0 .3,
            reports[1].0 .5
        )
    );
    eprintln!("borrowed-single duplicate and repeated hits immediate/current={reports:?}");
}

#[test]
fn borrowed_single_real_two_label_general_route_keeps_full_old_maps_and_reports_conservative_overhead(
) {
    let function = fixture();
    let mut setup = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(&function, &mut setup).unwrap();
    let dead = graph.node(&function, &mut setup).unwrap();
    let root = graph.node(&function, &mut setup).unwrap();
    graph
        .movement(
            &function,
            &mut setup,
            root,
            Label::End(super::super::SkeletonTerminal::Success),
            sink,
        )
        .unwrap();
    graph
        .movement(
            &function,
            &mut setup,
            root,
            Label::End(super::super::SkeletonTerminal::Failure),
            dead,
        )
        .unwrap();
    let mut old = Canonicalizer::new(&graph, sink);
    let mut current = Canonicalizer::new(&graph, sink);
    let mut a = ReplayBudget::unobserved();
    let mut b = ReplayBudget::unobserved();
    let mut old_receiving = super::closure_receiving::ClosureReceiving::from_prepaid(&function, &a);
    let mut receiving = super::closure_receiving::ClosureReceiving::from_prepaid(&function, &b);
    assert_eq!(
        old.before_borrowed_single_id_with_receiving(
            &function,
            vec![(root, 1)],
            &mut a,
            &mut old_receiving
        ),
        current.id_with_receiving(&function, vec![(root, 1)], &mut b, &mut receiving)
    );
    assert_eq!(
        (a.remaining, a.skeleton_remaining, a.skeleton_spent),
        (b.remaining, b.skeleton_remaining, b.skeleton_spent)
    );
    assert!(old.states.iter().eq(current.states.iter()));
    assert!(old.ids.iter().eq(current.ids.iter()));
    assert_eq!(current.states.len(), 3);
    assert!(current.ids.iter().any(|((_, edges), _)| edges.len() == 2));
    let mut reports = Vec::new();
    for old in [true, false] {
        let (result, heap) =
            borrowed_single_observe(&function, &graph, sink, &[root], old, 1 << 26);
        let full = result.unwrap();
        for limit in [0, full.0 - 1, full.0] {
            let (result, physical) =
                borrowed_single_observe(&function, &graph, sink, &[root], old, limit);
            assert_eq!(result.is_ok(), limit == full.0);
            if let Ok(actual) = result {
                assert_eq!(actual, full);
            }
            assert!(
                !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
            );
        }
        reports.push((full, heap));
    }
    assert_eq!(
        (
            reports[0].0 .1,
            reports[0].0 .2,
            reports[0].0 .3,
            reports[0].0 .5
        ),
        (
            reports[1].0 .1,
            reports[1].0 .2,
            reports[1].0 .3,
            reports[1].0 .5
        )
    );
    assert_eq!(
        reports[0].1, reports[1].1,
        "this graph never takes a one-label Exit; backing and Drop are identical"
    );
    assert!(
        reports[1].0 .0 > reports[0].0 .0,
        "the complete general path exposes the new conservative Exit overhead"
    );
    eprintln!("borrowed-single actual two-label general immediate/current={reports:?}");
}

// Complete identical original arena corpus, including duplicate128 labels,
// unique24 labels and empty-only shape. The original strict arena/210 owners
// remain byte-exact; this is additional immediate886 composition coverage.
fn arity_dispatch_corpus(
    function: &ResolvedFunction,
    shape: usize,
) -> (Automaton, usize, Vec<usize>) {
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

fn arity_dispatch_observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    roots: &[usize],
    old: bool,
    limit: usize,
) -> (
    Result<(usize, usize, usize, usize, usize, u64), ()>,
    Metrics,
) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                return Err(());
            }
        };
        let outcome = (|| {
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "single signature complete owner",
            )?;
            let mut owner = Canonicalizer::new(graph, sink);
            budget.reserve_bytes(
                function,
                super::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "single signature held receiver",
            )?;
            let mut receiving =
                super::closure_receiving::ClosureReceiving::from_prepaid(function, &budget);
            let initial = budget.remaining;
            let mut checksum = 0_u64;
            for root in roots {
                budget.reserve_array::<(usize, u64)>(function, 1, "single signature root")?;
                let id = if old {
                    owner.before_arity_dispatch_id_with_receiving(
                        function,
                        vec![(*root, 1)],
                        &mut budget,
                        &mut receiving,
                    )
                } else {
                    owner.id_with_receiving(function, vec![(*root, 1)], &mut budget, &mut receiving)
                }?
                .expect("acyclic exact one-label graph");
                checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
            }
            Ok::<_, Diagnostic>((
                budget.physical.as_ref().unwrap().charged(),
                initial - budget.remaining,
                owner.states.len(),
                owner.ids.len(),
                owner.child_ids.capacity(),
                checksum,
            ))
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        let outcome = outcome.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
        });
        drop(budget);
        assert_eq!(
            current_metrics().allocated,
            stopped,
            "complete singleton signature Drop allocates nothing"
        );
        outcome
    })
}

#[test]
fn arity_dispatch_complete_886_corpus_preserves_all_shapes_work_and_drop_with_exact_general_role_delta(
) {
    let function = fixture();
    for shape in 0..3 {
        let (graph, sink, roots) = arity_dispatch_corpus(&function, shape);
        let mut reports = Vec::new();
        for old in [true, false] {
            let (result, heap) =
                arity_dispatch_observe(&function, &graph, sink, &roots, old, 1 << 26);
            let full = result.unwrap();
            assert!(!heap.invalid && heap.live == 0 && heap.allocated == heap.freed);
            for limit in [0, full.0 - 1, full.0] {
                let (cut, physical) =
                    arity_dispatch_observe(&function, &graph, sink, &roots, old, limit);
                assert_eq!(cut.is_ok(), limit == full.0);
                if let Ok(actual) = cut {
                    assert_eq!(actual, full);
                }
                assert!(
                    !physical.invalid && physical.live == 0 && physical.allocated == physical.freed
                );
            }
            reports.push((full, heap));
        }
        assert_eq!(
            (
                reports[0].0 .1,
                reports[0].0 .2,
                reports[0].0 .3,
                reports[0].0 .4,
                reports[0].0 .5
            ),
            (
                reports[1].0 .1,
                reports[1].0 .2,
                reports[1].0 .3,
                reports[1].0 .4,
                reports[1].0 .5
            )
        );
        assert_eq!(
            reports[0].1, reports[1].1,
            "dispatch removes no actual backing; full allocation/peak/Drop tuple is exact"
        );
        // Independent graph/source counts: every root is distinct, no one-edge
        // signature exists, and each state publishes exactly one completed Exit.
        let mut logical = Canonicalizer::new(&graph, sink);
        let mut budget = ReplayBudget::unobserved();
        let mut held = super::closure_receiving::ClosureReceiving::from_prepaid(&function, &budget);
        for root in &roots {
            logical
                .id_with_receiving(&function, vec![(*root, 1)], &mut budget, &mut held)
                .unwrap();
        }
        assert!(logical.ids.iter().all(|((_, edges), _)| edges.len() != 1));
        let activations = roots.len();
        let exits = logical.states.len();
        assert!(exits >= activations);
        let old_calls = activations
            * (Canonicalizer::BEFORE_ARITY_BORROWED_EXIT_PRIMITIVE
                + Canonicalizer::BEFORE_ARITY_BORROWED_EXIT_WARM)
            + (exits - activations) * Canonicalizer::BEFORE_ARITY_BORROWED_EXIT_WARM
            + activations * Canonicalizer::BEFORE_ARITY_BORROWED_EXIT_LOCAL;
        let current_calls = activations * Canonicalizer::BORROWED_EXIT_LOCAL
            + exits * super::empty_signature::BRANCH_CONTROLS;
        assert_eq!(reports[0].0.0 + current_calls, reports[1].0.0 + old_calls,
            "complete independently counted actual admission/retained-place equation, never measured subtraction");
        assert!(
            reports[1].0 .0 < reports[0].0 .0,
            "all three actual complete corpora remove unused singleton admissions"
        );
        eprintln!("arity-dispatch shape={shape} complete886/current={reports:?} activations={activations} exits={exits}");
    }
}
