//! Complete immediate receiver and actual current unit publication boundaries.
use super::{before_unit, Automaton, Canonicalizer, Diagnostic, ReplayBudget, ResolvedFunction};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn unified_closure_slot_keeps_complete_immediate_ids_work_and_all_six_failure_fields() {
    let function = fixture();
    let (graph, sink, roots) = super::tests::automaton(&function);
    for root in roots.into_iter().chain([sink]) {
        for weight in [0, 1, 2, u64::MAX] {
            for work in [0, 1, 2, 4, 16, 128, 1024] {
                let mut old = before_unit::Canonicalizer::new(&graph, sink);
                let mut current = Canonicalizer::new(&graph, sink);
                let mut a = ReplayBudget::unobserved();
                let mut b = ReplayBudget::unobserved();
                a.remaining = work;
                b.remaining = work;
                let expected = old.id(&function, vec![(root, weight)], &mut a);
                let actual = current.id(&function, vec![(root, weight)], &mut b);
                assert_eq!(actual, expected, "root={root} weight={weight} work={work}");
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
            budget.reserve_array::<(usize, u64)>(function, 1, "unified invocation test root")?;
            let id = if old {
                let mut receiver = before_unit::Canonicalizer::new(graph, sink);
                receiver.id(function, vec![(root, 1)], &mut budget)?
            } else {
                let mut receiver = Canonicalizer::new(graph, sink);
                receiver.id(function, vec![(root, 1)], &mut budget)?
            };
            assert!(id.is_some());
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged())
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let stopped = current_metrics().allocated;
        let result = result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        });
        drop(budget);
        assert_eq!(current_metrics().allocated, stopped);
        result
    })
}

/// Requested bytes of an intern arena's backing for `nodes` insertions: the
/// first block holds FIRST_CAPACITY nodes, then each growth reserves the whole
/// replay_growth_capacity target while the old block remains charged.
fn arena_backing(nodes: usize, node_bytes: usize) -> (usize, usize) {
    let (mut capacity, mut bytes, mut growths) = (0, 0, 0);
    for len in 0..nodes {
        if len == capacity {
            let additional = if capacity == 0 {
                super::intern_arena::FIRST_CAPACITY
            } else {
                1
            };
            capacity = crate::cleanup_plan::replay::physical_storage::replay_growth_capacity(
                capacity,
                len + additional,
            )
            .unwrap();
            bytes += capacity * node_bytes;
            growths += 1;
        }
    }
    (bytes, growths)
}

#[test]
fn unified_closure_slot_reports_whole_immediate_costs_and_exact_short_zero_drop() {
    use super::intern_arena::{Arena, Node, Signature};
    use crate::cleanup_plan::replay::merge_census::phase_census;
    use std::mem::size_of;
    let function = fixture();
    let (graph, sink, roots) = super::tests::automaton(&function);
    for root in [roots[0], roots[3], roots[4], sink] {
        let mut paid = [0; 2];
        let mut censuses = Vec::new();
        for (index, old) in [true, false].into_iter().enumerate() {
            let ((result, metrics), census) =
                phase_census(|| observe(&function, &graph, sink, root, 1 << 24, old));
            assert!(census.complete());
            paid[index] = result.unwrap();
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!(
                "unified-closure root={root} old={old} reserved={} physical={metrics:?}",
                paid[index]
            );
            for (phase, bytes, count) in census.phases() {
                eprintln!("unified-closure root={root} old={old} phase={phase} bytes={bytes} count={count}");
            }
            for cut in [0, paid[index] - 1, paid[index]] {
                let (result, metrics) = observe(&function, &graph, sink, root, cut, old);
                assert_eq!(result.is_ok(), cut == paid[index]);
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
            censuses.push(census);
        }
        // Current named phases equal their constants times the structural
        // insertion counts of an independent current receiver on this root.
        let mut receiver = Canonicalizer::new(&graph, sink);
        assert!(receiver
            .id(&function, vec![(root, 1)], &mut ReplayBudget::unobserved())
            .unwrap()
            .is_some());
        let (signatures, states) = (receiver.ids.len(), receiver.states.len());
        let current = &censuses[1];
        assert_eq!(
            current.phase("canonical arena node receiving"),
            (
                signatures * Arena::<Signature>::INSERT_CONTROLS
                    + states * Arena::<super::SharedWeighted>::INSERT_CONTROLS,
                signatures + states
            )
        );
        let (signature_backing, signature_growths) =
            arena_backing(signatures, size_of::<Node<Signature>>());
        let (state_backing, state_growths) =
            arena_backing(states, size_of::<Node<super::SharedWeighted>>());
        // Every insertion authenticates its backing; each growth also requests it.
        assert_eq!(
            current.phase("canonical ordered arena backing"),
            (
                signature_backing + state_backing,
                signatures + states + signature_growths + state_growths
            )
        );
        // The whole reservation difference is exactly the sum of per-phase
        // differences: every byte either route requests is a censused phase.
        let phase_net = current.total() as isize - censuses[0].total() as isize;
        assert_eq!(
            paid[1] as isize - paid[0] as isize,
            phase_net,
            "root={root}"
        );
        // The unified slot's own saving, at the phases it changed (fac23f62f):
        // the duplicated retained-memo owner and its failure Result are gone,
        // and the held producer/take controls shrink. Whole-route totals also
        // include later, independently gated changes (the four-node intern
        // arena start trades a few bytes on two-node closures for fewer
        // allocation events), so they are pinned by the per-phase equation
        // above, never by a whole-route inequality.
        let slot = |census: &crate::cleanup_plan::replay::merge_census::PhaseCensus| {
            [
                "factored retained memo controls and failure receiving",
                "factored held closure producer and take controls",
            ]
            .into_iter()
            .map(|phase| census.phase(phase).0)
            .sum::<usize>()
        };
        let (old_slot, new_slot) = (slot(&censuses[0]), slot(current));
        assert!(new_slot <= old_slot, "root={root}");
        if old_slot != 0 {
            assert!(
                new_slot < old_slot,
                "actual duplicated owner and Result removed; old remains complete"
            );
        }
    }
}

#[test]
fn unified_closure_slot_keeps_full_immediate_sources_and_old_receiver_bodies_bound() {
    use sha2::{Digest, Sha256};
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"));
    let start = old.find("type SharedWeighted =").unwrap();
    let end = old[start..].find("\n#[cfg(test)]\nmod tests;").unwrap() + start;
    let copy = include_str!("before_unit.rs");
    let copied_start = copy.find("type SharedWeighted =").unwrap();
    assert_eq!(
        &copy[copied_start..copied_start + end - start],
        &old[start..end]
    );
    assert_eq!(include_str!("before_unit/empty_memo.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared__empty_memo.rs.before.txt")));
    assert_eq!(include_str!("before_unit/empty_memo/retained.rs"), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared__empty_memo__retained.rs.before.txt")));
    let old_slot = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared__closure_receiving.rs.before.txt"));
    let registration = old_slot
        .find("\n#[cfg(test)]\npub(in crate::cleanup_plan::replay) mod source;")
        .unwrap();
    assert_eq!(
        include_str!("before_unit/closure_receiving.rs"),
        &old_slot[..registration + 1]
    );
    let full_inputs = [
        ("CHANGELOG.md", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/CHANGELOG.md.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("docs/ARCHITECTURE.md", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/ARCHITECTURE.md")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/docs__ARCHITECTURE.md.before.txt"))),
        ("src/cleanup_plan/replay/factored/canonical_shared.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared__closure_receiving.rs.before.txt"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__cleanup_plan__replay__factored__canonical_shared__empty_memo.rs.before.txt"))),
        ("src/source_reflows_merge.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/src__source_reflows_merge.rs.before.txt"))),
        ("tests/hir_module_boundaries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/tests__hir_module_boundaries.rs.before.txt"))),
        ("tests/workspace/semantic_graph/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/closure-unit-receiving-831-next-20261016/tests__workspace__semantic_graph__source_inventory.rs.before.txt"))),
    ];
    for (path, current, original) in full_inputs {
        let projected = crate::source_reflows_merge::unit831_input(path, current);
        assert_eq!(projected, original);
    }
    let owners: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/closure-unit-receiving-831-next-20261016/new-owners.json"
    )))
    .unwrap();
    let sources = [
        ("before_unit.rs", include_str!("before_unit.rs")),
        (
            "before_unit/empty_memo.rs",
            include_str!("before_unit/empty_memo.rs"),
        ),
        (
            "before_unit/empty_memo/retained.rs",
            include_str!("before_unit/empty_memo/retained.rs"),
        ),
        (
            "before_unit/closure_receiving.rs",
            include_str!("before_unit/closure_receiving.rs"),
        ),
        (
            "unit_receiving_tests.rs",
            include_str!("unit_receiving_tests.rs"),
        ),
    ];
    assert_eq!(owners.as_array().unwrap().len(), sources.len());
    for (suffix, text) in sources {
        let path = format!("src/cleanup_plan/replay/factored/canonical_shared/{suffix}");
        let owner = owners
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == path)
            .unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            ),
            owner["sha256"]
        );
    }
}
