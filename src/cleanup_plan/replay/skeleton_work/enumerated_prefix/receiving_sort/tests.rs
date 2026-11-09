//! Complete old heap algorithm remains the independently executable receiver.
use super::*;
// Historical direct controls retain the exact post-948 nominal layouts.
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure, Metrics};
use before_positive_bytes::{Cursor, Frame, Pair};

fn program() -> ResolvedProgram {
    let checked = crate::check(
        "module receiving_sort; @id(\"main\") fn main()->i64 {0}",
        "receiving-sort.spx",
    )
    .unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn dag(kind: ObservationDagKind) -> Rc<ObservationDag> {
    Rc::new(ObservationDag {
        ordinal: Some(SEQUENCE_MARKER),
        kind,
        pending_bound: None,
        scratch: None,
    })
}
fn item(function: &ResolvedFunction, value: bool) -> Observation {
    Rc::new(SkeletonObservation::Boolean {
        expression: function.body.id.clone(),
        value,
    })
}
fn fixture(function: &ResolvedFunction, count: usize, width: usize) -> Vec<SkeletonPath> {
    (0..count)
        .map(|index| {
            // Independently allocated equal histories, empty Seq/Cat sentinels,
            // shared siblings, and both a Seq prefix and a root tail.
            let leaf = dag(ObservationDagKind::Seq(
                None,
                (0..width).map(|_| item(function, true)).collect(),
            ));
            let prefix = dag(ObservationDagKind::Cat(vec![
                dag(ObservationDagKind::Seq(None, Vec::new())),
                leaf.clone(),
                dag(ObservationDagKind::Cat(Vec::new())),
                dag(ObservationDagKind::Seq(
                    Some(leaf),
                    vec![item(function, index % 3 == 0)],
                )),
            ]));
            SkeletonPath {
                observations: Observations {
                    prefix: Some(prefix),
                    items: if index % 4 == 0 {
                        Vec::new()
                    } else {
                        vec![item(function, index % 2 == 0)]
                    },
                },
                terminal: match index % 3 {
                    0 => SkeletonTerminal::Success,
                    1 => SkeletonTerminal::Failure,
                    _ => SkeletonTerminal::LoopLatch,
                },
            }
        })
        .rev()
        .collect()
}
fn contents(paths: &[SkeletonPath]) -> Vec<(Vec<SkeletonObservation>, SkeletonTerminal)> {
    fn append(node: &ObservationDag, out: &mut Vec<SkeletonObservation>) {
        match &node.kind {
            ObservationDagKind::Seq(prefix, items) => {
                if let Some(prefix) = prefix {
                    append(prefix, out);
                }
                out.extend(items.iter().map(|item| item.as_ref().clone()));
            }
            ObservationDagKind::Cat(children) | ObservationDagKind::Alt(children) => {
                for child in children {
                    append(child, out);
                }
            }
        }
    }
    paths
        .iter()
        .map(|path| {
            let mut out = Vec::new();
            if let Some(prefix) = &path.observations.prefix {
                append(prefix, &mut out);
            }
            out.extend(
                path.observations
                    .items
                    .iter()
                    .map(|item| item.as_ref().clone()),
            );
            (out, path.terminal)
        })
        .collect()
}
#[derive(Debug)]
struct Run {
    result: Result<(), String>,
    paths: Vec<(Vec<SkeletonObservation>, SkeletonTerminal)>,
    remaining: usize,
    skeleton_remaining: usize,
    skeleton_spent: usize,
    refused: bool,
    refusal_spent: bool,
    paid: usize,
    heap: Metrics,
    sticky: Option<String>,
}
fn run(
    current: bool,
    input: &[SkeletonPath],
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
    floor: usize,
    lent: bool,
) -> Run {
    let mut paths = input.to_vec();
    let (
        (
            result,
            snapshot,
            remaining,
            skeleton_remaining,
            skeleton_spent,
            refusal_spent,
            heap,
            sticky,
        ),
        refused,
        paid,
        _,
    ) = with_limit_usage_refusal(bytes, || {
        let mut budget = ReplayBudget::with_skeleton_limit(work);
        assert!(matches!(budget.payer, AllocationPayer::ActiveBuilder));
        budget.remaining = work;
        budget.lent_skeleton = lent;
        assert!(crate::bounded_output::set_active_floor(floor));
        let (result, heap) = measure(|| {
            if current {
                super::super::sort(&mut paths, function, &mut budget)
            } else {
                before::sort(&mut paths, function, &mut budget)
            }
        });
        let snapshot = contents(&paths);
        let result = result.map_err(|error| error.json());
        let sticky = if budget.refusal_spent || crate::bounded_output::active_refused() {
            Some(
                budget
                    .reserve_compact_bytes(function, 0, "post-sort sticky control")
                    .unwrap_err()
                    .into_diagnostic()
                    .json(),
            )
        } else {
            None
        };
        (
            result,
            snapshot,
            budget.remaining,
            budget.skeleton_remaining,
            budget.skeleton_spent,
            budget.refusal_spent,
            heap,
            sticky,
        )
    });
    Run {
        result,
        paths: snapshot,
        remaining,
        skeleton_remaining,
        skeleton_spent,
        refused,
        refusal_spent,
        paid,
        heap,
        sticky,
    }
}
fn same_work(old: &Run, new: &Run) {
    assert_eq!(old.result, new.result);
    assert_eq!(old.paths, new.paths);
    assert_eq!(old.remaining, new.remaining);
    assert_eq!(old.skeleton_remaining, new.skeleton_remaining);
    assert_eq!(old.skeleton_spent, new.skeleton_spent);
    assert_eq!(old.refusal_spent, new.refusal_spent);
    assert_eq!(old.refused, new.refused);
    assert_eq!(old.sticky, new.sticky);
}
#[test]
fn every_work_cut_keeps_complete_content_terminal_multiplicity_and_partial_permutation() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    for count in [0, 1, 2, 3, 7, 12] {
        let paths = fixture(function, count, 3);
        let old = run(false, &paths, function, usize::MAX, usize::MAX, 0, false);
        let visits = usize::MAX - old.remaining;
        for work in (0..=visits + 1).chain([usize::MAX]) {
            same_work(
                &run(false, &paths, function, work, usize::MAX, 0, false),
                &run(true, &paths, function, work, usize::MAX, 0, false),
            );
        }
        same_work(
            &run(false, &paths, function, usize::MAX, usize::MAX, 0, true),
            &run(true, &paths, function, usize::MAX, usize::MAX, 0, true),
        );
    }
}
#[test]
fn real_backings_are_reused_across_swaps_and_early_mismatches() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 32, 64);
    let old = run(false, &paths, function, usize::MAX, usize::MAX, 0, false);
    let new = run(true, &paths, function, usize::MAX, usize::MAX, 0, false);
    same_work(&old, &new);
    assert!(new.heap.allocations < old.heap.allocations);
    assert!(new.heap.allocated < old.heap.allocated);
    assert!(new.paid < old.paid, "old={old:?} new={new:?}");
    assert!(ADVANCE_CALL < CURSOR_CALL);
    assert!(2 * PREPARE_CALL + COMPARE_CALL < 2 * CURSOR_STORAGE + 2 * CURSOR_CALL);
    // Both executions use the exact same real ActiveBuilder cap. The old
    // admitted cap must still admit the replacement; the replacement's exact
    // cap is also tested against the full prior algorithm, not a byte estimate.
    let same_cap = run(true, &paths, function, usize::MAX, old.paid, 0, false);
    same_work(&old, &same_cap);
    let prior_at_new_cap = run(false, &paths, function, usize::MAX, new.paid, 0, false);
    assert!(prior_at_new_cap.result.is_err());
    assert!(prior_at_new_cap.refused || prior_at_new_cap.refusal_spent);
    assert!(!old.heap.invalid && !new.heap.invalid);
    // This window retains the budget's refusal backing. The separate complete
    // cold invocation below includes that budget and every input's final Drop.
}
#[test]
fn exact_physical_limit_floor_and_sticky_failure_keep_all_diagnostic_fields() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 7, 8);
    let healthy = run(true, &paths, function, usize::MAX, usize::MAX, 0, false);
    assert!(healthy.result.is_ok());
    for floor in [0, 1, 64] {
        let exact = run(
            true,
            &paths,
            function,
            usize::MAX,
            healthy.paid + floor,
            floor,
            false,
        );
        assert_eq!(exact.result, healthy.result);
        assert_eq!(exact.paths, healthy.paths);
        assert_eq!(exact.paid, healthy.paid);
        let short = run(
            true,
            &paths,
            function,
            usize::MAX,
            healthy.paid + floor - 1,
            floor,
            false,
        );
        assert!(short.result.is_err());
        assert!(short.refused || short.refusal_spent);
        assert!(short.sticky.is_some());
    }
    // Mixed cuts are deterministic even where the physically wider initial
    // frame and retained bank intentionally change byte refusal frontiers.
    for work in [0, 1, 3, 17, 64, usize::MAX] {
        for bytes in [0, 1, healthy.paid / 2, healthy.paid] {
            let a = run(true, &paths, function, work, bytes, 0, false);
            let b = run(true, &paths, function, work, bytes, 0, false);
            same_work(&a, &b);
            assert_eq!(a.paid, b.paid);
        }
    }
}
#[test]
fn alt_refusal_and_left_exhaustion_still_visit_the_original_right_frontier() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    let mut paths = fixture(function, 2, 0);
    paths[0].observations = Observations {
        prefix: Some(dag(ObservationDagKind::Cat(Vec::new()))),
        items: Vec::new(),
    };
    paths[1].observations = Observations {
        prefix: Some(dag(ObservationDagKind::Alt(Vec::new()))),
        items: Vec::new(),
    };
    for work in 0..16 {
        same_work(
            &run(false, &paths, function, work, usize::MAX, 0, false),
            &run(true, &paths, function, work, usize::MAX, 0, false),
        );
    }
    let old = run(false, &paths, function, usize::MAX, usize::MAX, 0, false);
    let new = run(true, &paths, function, usize::MAX, usize::MAX, 0, false);
    same_work(&old, &new);
    assert!(new
        .result
        .unwrap_err()
        .contains("cannot contain merged alternatives"));
}
#[test]
fn complete_original_source_and_executable_oracle_remain_byte_bound() {
    let _before_advance = old_advance_epoch();
    use sha2::{Digest, Sha256};
    let original = include_str!("original.rs.txt");
    let oracle = include_str!("before.rs");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "ef37260c27855f8b76325ba24ad2c4204a3228b8a6fd0cccab688fb60629d58e"
    );
    let expected = original.replacen("use super::*;", "use super::super::*;", 1)
        .replace("#[cfg(test)]\n#[path = \"enumerated_prefix/flat_probe/before.rs\"]\nmod before_flat_probe;\nmod flat_probe;", "use super::super::flat_probe;")
        .replace("#[cfg(test)]\n#[rustfmt::skip]\nmod before;\n#[cfg(test)]\nmod source_custody;\n#[cfg(test)]\nmod tests;\n", "");
    assert_eq!(oracle, expected);
    let restored = oracle.replacen("use super::super::*;", "use super::*;", 1)
        .replacen("use super::super::flat_probe;", "#[cfg(test)]\n#[path = \"enumerated_prefix/flat_probe/before.rs\"]\nmod before_flat_probe;\nmod flat_probe;", 1)
        .replacen("\npub(in crate::cleanup_plan::replay) fn extend(", "#[cfg(test)]\n#[rustfmt::skip]\nmod before;\n#[cfg(test)]\nmod source_custody;\n#[cfg(test)]\nmod tests;\n\npub(in crate::cleanup_plan::replay) fn extend(", 1);
    assert_eq!(restored, original);
}

#[test]
fn paired_capacity_grows_only_when_needed_and_retains_no_path_handles_between_calls() {
    let _before_advance = old_advance_epoch();
    use super::before_advance::Pair;
    let program = program();
    let function = &program.functions[0];
    let shallow = fixture(function, 2, 1);
    let wide = |value| SkeletonPath {
        observations: Observations {
            prefix: Some(dag(ObservationDagKind::Cat(
                (0..33)
                    .map(|_| dag(ObservationDagKind::Seq(None, vec![item(function, value)])))
                    .collect(),
            ))),
            items: Vec::new(),
        },
        terminal: SkeletonTerminal::Success,
    };
    let mut paths = vec![wide(true), wide(false)];
    let original_counts: Vec<_> = paths
        .iter()
        .map(|path| Rc::strong_count(path.observations.prefix.as_ref().unwrap()))
        .collect();
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        budget
            .reserve_compact_bytes(function, RECEIVERS, "test paired bank")
            .unwrap();
        let mut pair = Pair::new();
        pair.order(&shallow[0], &shallow[1], function, &mut budget)
            .unwrap();
        let small = pair.left.pending.capacity();
        pair.order(&paths[0], &paths[1], function, &mut budget)
            .unwrap();
        assert!(pair.left.pending.capacity() > small);
        let pointers = (pair.left.pending.as_ptr(), pair.right.pending.as_ptr());
        let capacity = (pair.left.pending.capacity(), pair.right.pending.capacity());
        for _ in 0..8 {
            paths.swap(0, 1);
            pair.order(&paths[0], &paths[1], function, &mut budget)
                .unwrap();
            assert!(pair.left.pending.is_empty() && pair.right.pending.is_empty());
            assert!(pair.left.yielded.is_none() && pair.right.yielded.is_none());
            assert_eq!(
                (pair.left.pending.as_ptr(), pair.right.pending.as_ptr()),
                pointers
            );
            assert_eq!(
                (pair.left.pending.capacity(), pair.right.pending.capacity()),
                capacity
            );
        }
        for (path, count) in paths.iter().zip(original_counts) {
            assert_eq!(
                Rc::strong_count(path.observations.prefix.as_ref().unwrap()),
                count
            );
        }
    });
    assert!(!refused);
}
#[test]
fn complete_cold_invocation_including_owned_input_drop_releases_every_backing() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    let cold = |current| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut paths = fixture(function, 32, 32);
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                if current {
                    super::super::sort(&mut paths, function, &mut budget).unwrap();
                } else {
                    before::sort(&mut paths, function, &mut budget).unwrap();
                }
                // Input construction, the complete algorithm and terminal destruction
                // share one observation. No retained external node masks final Drop.
                drop(paths);
                drop(budget);
            })
        })
    };
    let (((), old), old_refused, _, _) = cold(false);
    let (((), new), new_refused, _, _) = cold(true);
    assert!(!old_refused && !new_refused);
    assert!(!old.invalid && !new.invalid);
    assert_eq!(old.live, 0);
    assert_eq!(new.live, 0);
    assert!(new.allocations < old.allocations);
    assert!(new.allocated < old.allocated);
}
#[test]
fn unwind_releases_temporary_handles_while_original_paths_still_own_every_node() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 2, 3);
    let root = paths[0].observations.prefix.as_ref().unwrap();
    let original = Rc::strong_count(root);
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            budget
                .reserve_compact_bytes(function, RECEIVERS, "test unwind bank")
                .unwrap();
            let mut pair = Pair::new();
            pair.left
                .prepare(&paths[0].observations, function, &mut budget)
                .unwrap();
            assert!(Rc::strong_count(root) > original);
            pair.left
                .next(&paths[0].observations, function, &mut budget)
                .unwrap();
            panic!("test receiving bank unwind");
        }));
        assert!(result.is_err());
        assert_eq!(Rc::strong_count(root), original);
    });
    assert!(!refused);
}

#[test]
fn singleton_shared_sort_never_acquires_a_cursor_or_receiving_bank() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    let paths = fixture(function, 1, 8);
    // One original flat-probe work unit admits this nonflat singleton. The
    // shared heapsort has no comparisons, no swaps and no traversal work.
    let old = run(false, &paths, function, 1, usize::MAX, 0, false);
    let new = run(true, &paths, function, 1, usize::MAX, 0, false);
    same_work(&old, &new);
    assert!(new.result.is_ok());
    assert_eq!(new.remaining, 0);
    assert_eq!(new.paid, old.paid + TRIVIAL_GUARD);
    assert_eq!(new.heap, old.heap);
    let exact = run(true, &paths, function, 1, new.paid, 0, false);
    assert_eq!(exact.result, new.result);
    assert_eq!(exact.paths, new.paths);
    assert_eq!(exact.paid, new.paid);
    assert_eq!(exact.remaining, 0);
}

#[test]
fn every_deep_empty_frame_and_warm_cat_child_attempt_renews_its_actual_invoice() {
    let _before_advance = old_advance_epoch();
    let program = program();
    let function = &program.functions[0];
    for depth in [0, 1, 17, 257] {
        for children in [0, 1, 33, 129] {
            let mut prefix = dag(ObservationDagKind::Cat(
                (0..children)
                    .map(|_| dag(ObservationDagKind::Seq(None, Vec::new())))
                    .collect(),
            ));
            for _ in 0..depth {
                prefix = dag(ObservationDagKind::Seq(Some(prefix), Vec::new()));
            }
            let observations = Observations {
                prefix: Some(prefix),
                items: Vec::new(),
            };
            // Each Seq has a node and an exhausted item frame. The Cat node
            // and exhausted root-items frame add two; pop also attempts None.
            let frames = 2 * depth + 2 * children + 2;
            let attempts = frames + 1;
            let child_attempts = children + 1;
            let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                budget
                    .reserve_compact_bytes(
                        function,
                        RECEIVERS + PREPARE_CALL + ADVANCE_CALL,
                        "test structural cursor ownership",
                    )
                    .unwrap();
                let mut cursor = Cursor::new();
                cursor
                    .prepare(&observations, function, &mut budget)
                    .unwrap();
                cursor.next(&observations, function, &mut budget).unwrap();
                assert!(cursor.pending.is_empty() && cursor.yielded.is_none());
                budget
                    .reserve_compact_bytes(
                        function,
                        PREPARE_CALL,
                        "test warm structural cursor prepare",
                    )
                    .unwrap();
                cursor
                    .prepare(&observations, function, &mut budget)
                    .unwrap();
                let capacity = cursor.pending.capacity();
                let pointer = cursor.pending.as_ptr();
                let bytes = crate::bounded_output::active_remaining().unwrap();
                let work = budget.remaining;
                let ((), heap) = measure(|| {
                    budget
                        .reserve_compact_bytes(
                            function,
                            ADVANCE_CALL,
                            "test warm structural cursor invocation",
                        )
                        .unwrap();
                    cursor.next(&observations, function, &mut budget).unwrap();
                });
                assert!(!heap.invalid);
                assert_eq!(heap.allocated, 0, "all pending backing is already warm");
                assert_eq!(cursor.pending.capacity(), capacity);
                assert_eq!(cursor.pending.as_ptr(), pointer);
                assert_eq!(work - budget.remaining, frames);
                assert_eq!(
                    bytes - crate::bounded_output::active_remaining().unwrap(),
                    ADVANCE_CALL
                        + attempts * FRAME_COMMON
                        + ROOT_FRAME
                        + (depth + children) * SEQ_FRAME
                        + (depth + children + 1) * NODE_FRAME
                        + child_attempts * CAT_CHILD_STEP
                        + (depth + children + 1) * NODE_STEP
                );
                assert!(
                    cursor.pending.is_empty()
                        && cursor.current.is_none()
                        && cursor.yielded.is_none()
                );

                before_frame_pricing::verify_original_warm_invoice(
                    &observations,
                    function,
                    frames,
                    attempts,
                    child_attempts,
                );

                let mut old_budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                old_budget
                    .reserve_bytes(
                        function,
                        CURSOR_STORAGE + CURSOR_CALL,
                        "test original structural cursor ownership",
                    )
                    .unwrap();
                let mut old = before::Cursor::new();
                old.prepare(&observations, function, &mut old_budget)
                    .unwrap();
                old_budget
                    .reserve_bytes(
                        function,
                        CURSOR_CALL,
                        "test original structural cursor invocation",
                    )
                    .unwrap();
                let old_work = old_budget.remaining;
                assert!(old.next(function, &mut old_budget).unwrap().is_none());
                assert_eq!(old_work - old_budget.remaining, frames);
            });
            assert!(!refused);
        }
    }
}

#[test]
fn complete_immediate_old_bank_and_original_structural_invoice_are_source_bound() {
    let _before_advance = old_advance_epoch();
    use sha2::{Digest, Sha256};
    let captured = include_str!("before_frame_pricing.rs.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(captured.as_bytes()))
        ),
        "fb4060791d4d5b729e06411e1177b8e923dea4084c1904c1ce9785c33b800eea"
    );
    let compiled = include_str!("before_frame_pricing.rs");
    let (body, shim) = compiled
        .split_once("\n// TEST_ONLY_ORIGINAL_WARM_INVOICE\n")
        .unwrap();
    let (prefix, _) = captured.split_once("\n#[cfg(test)]\n#[allow(").unwrap();
    assert_eq!(
        body.replace("use super::super::*;", "use super::*;"),
        prefix
    );
    assert!(shim.contains("ADVANCE_CALL + attempts * FRAME_STEP + child_attempts * CAT_CHILD_STEP"));
    let original_tests = include_str!("tests.before_frame_pricing.rs.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original_tests.as_bytes()))
        ),
        "7a0f709097a80bb15783c0d031716847bcb49d8cf749f3fc701855618c6220c4"
    );
    assert!(original_tests
        .contains("ADVANCE_CALL + attempts * FRAME_STEP + child_attempts * CAT_CHILD_STEP"));
}

#[path = "branch_frames.rs"]
mod branch_frames;

#[path = "lexical_advance.rs"]
mod lexical_advance;

#[path = "positive_bytes.rs"]
mod positive_bytes;
