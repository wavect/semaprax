//! Genuine2c borrowed readers composed with certified heads and fused tails.
use super::*;
fn main487_delta(r: &pair_scope_census::Row, expected: &[SkeletonPath]) -> (usize, usize) {
    use super::super::super::main487_pair_scope_tests as old;
    let flat_paths = expected
        .iter()
        .filter(|p| p.observations.prefix.is_none())
        .count();
    let flat_rounds: usize = expected
        .iter()
        .filter(|p| p.observations.prefix.is_none())
        .map(|p| p.observations.items.len() + 1)
        .sum();
    let removed_view = 2 * size_of::<&Observations>() + size_of::<&[Rc<SkeletonObservation>]>();
    let retained_view = 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>();
    let (old_tail, tail, tail_view) = old::tail_receiver_prices();
    let (step, load, setup, limit) = old::prepared_link_prices();
    let (all_primitives, reachable_primitives) = old::reachable_primitive_prices(expected.len());
    let (shared_removed, shared_retained) = old::shared_step_prices(expected.len(), r);
    (
        PAIR_SCOPE * r.pairs
            + PAIR_PRIMITIVE
            + PAYMENT_HEADER
            + retained_view * flat_paths
            + tail_view * (2 * r.pairs + expected.len())
            + setup
            + limit * (2 * r.pairs + expected.len())
            + reachable_primitives
            + shared_retained,
        4 * PATH_INDEX * r.rounds
            + removed_view * flat_rounds
            + (old_tail - tail) * r.tails
            + step * r.steps
            + load * r.loads
            + all_primitives
            + shared_removed,
    )
}
fn run(
    function: &ResolvedFunction,
    generation: usize,
    length: usize,
    shape: usize,
    fault: usize,
    units: usize,
    limit: usize,
    floor: usize,
    shared_hir: bool,
    swaps_only: bool,
) -> Run {
    heads_run_generation(
        function, length, 3, true, fault, generation, units, limit, floor, shared_hir, swaps_only,
        shape,
    )
}
#[test]
fn borrowed_reader_composition_keeps_both_real_generations_complete_and_reconciles_every_typed_bill(
) {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    for length in [0, 1, 4, 64] {
        for shape in 0..=6 {
            for shared_hir in [false, true] {
                let old = run(
                    function,
                    7,
                    length,
                    shape,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    shared_hir,
                    true,
                );
                let current = run(
                    function,
                    0,
                    length,
                    shape,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    shared_hir,
                    true,
                );
                assert_eq!(old.observed.0 .0, current.observed.0 .0);
                assert!(
                    old.observed.0.0.result.is_ok() && old.row.complete && current.row.complete,
                    "length={length} shape={shape} shared_hir={shared_hir}; pure2c={:?} current={:?}; rows pure2c={:?} current={:?}",
                    old.observed.0.0, current.observed.0.0, old.row, current.row
                );
                assert_eq!(swaps(&old), swaps(&current));
                assert_eq!(old.row.pairs, current.row.pairs);
                let (actual, mut expected) = fixture(length, 3, true, 0);
                if shape == 5 {
                    expected.clear();
                    for path in actual.paths.iter().take(2) {
                        let mut values = actual.arena[0].items.clone();
                        values.extend(path.history.tail.iter().cloned());
                        expected.push(SkeletonPath {
                            observations: values.into(),
                            terminal: path.terminal,
                        });
                    }
                    expected.sort();
                }
                if shared_hir {
                    for path in &mut expected {
                        path.observations.prefix = path.observations.freeze();
                        path.observations.items.clear();
                    }
                }
                let expected = &expected;
                let old_delta = main487_delta(&old.row, expected);
                let current_delta = current883_delta(&current.row, expected);
                assert_eq!(
                    current.observed.2 + current_delta.1 + old_delta.0,
                    old.observed.2 + old_delta.1 + current_delta.0,
                    "complete actual2c/current equations, length={length} shape={shape} shared={shared_hir}"
                );
                assert_eq!(
                    paired_value::LEFT_YIELD_AND_RECEIVE,
                    2 * size_of::<&Rc<SkeletonObservation>>()
                );
                assert!(paired_value::LEFT_YIELD_AND_RECEIVE <= paired_value::LEFT_RETIRED_AS_REF);
                eprintln!(
                    "OPT487 composed complete length={length} shape={shape} shared_hir={shared_hir} pure2c={} current={} rightsteps={} righttails={} matchbanks={}; full old/new typed bills, losses retained, not an app-fit measurement",
                    old.observed.2,
                    current.observed.2,
                    current.row.pair_right_steps,
                    current.row.pair_right_tails,
                    current.row.pair_matches_admitted
                );
            }
        }
    }
}
#[test]
fn borrowed_reader_composition_keeps_every_work_cut_six_fields_terminal_and_swap_order() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    for (shape, fault) in [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
        (6, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (6, 4),
        (6, 5),
        (6, 6),
    ] {
        let complete = run(
            function,
            7,
            4,
            shape,
            fault,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            false,
            true,
        );
        // This comparator spends ordinary work, not the reserved skeleton loan.
        for units in 0..=MAX_REPLAY_WORK_UNITS - complete.observed.0 .0.remaining {
            for shared_hir in [false, true] {
                let old = run(
                    function,
                    7,
                    4,
                    shape,
                    fault,
                    units,
                    usize::MAX,
                    0,
                    shared_hir,
                    true,
                );
                let current = run(
                    function,
                    0,
                    4,
                    shape,
                    fault,
                    units,
                    usize::MAX,
                    0,
                    shared_hir,
                    true,
                );
                assert_eq!(
                    old.observed.0 .0, current.observed.0 .0,
                    "all six diagnostic fields/work, shape={shape} fault={fault} units={units}"
                );
                assert_eq!(swaps(&old), swaps(&current));
            }
        }
    }
}
#[test]
fn borrowed_right_and_content_frontiers_are_real_exact_short_zero_floor_sticky_and_full_drop() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut reached = [false; 3];
    for shape in [0, 1, 3, 6] {
        let complete = run(
            function,
            0,
            4,
            shape,
            0,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            false,
            false,
        );
        assert!(
            complete.observed.0 .0.result.is_ok() && complete.row.complete,
            "shape={shape}; outcome={:?}; row={:?}",
            complete.observed.0 .0,
            complete.row
        );
        for (i, price, count) in [
            (0, paired_value::RIGHT_STEP, complete.row.pair_right_steps),
            (
                1,
                paired_value::RIGHT_TAIL + paired_value::TAIL_MARK,
                complete.row.pair_right_tails,
            ),
            (2, PAIR_MATCH, complete.row.pair_matches_admitted),
        ] {
            let events: Vec<_> = complete
                .events
                .iter()
                .filter(|e| {
                    e.phase == "ordinary plan history paired cursor next"
                        && e.after == e.before.checked_add(price)
                })
                .collect();
            assert_eq!(events.len(), count);
            reached[i] |= count != 0;
            for event in events {
                let after = event.after.unwrap();
                for (limit, floor) in [(event.before, 0), (after - 1, 0), (after, 1), (0, 0)] {
                    let prefix = run(
                        function,
                        0,
                        4,
                        shape,
                        0,
                        MAX_REPLAY_WORK_UNITS,
                        limit,
                        floor,
                        false,
                        false,
                    );
                    assert!(prefix.observed.1 && prefix.observed.0 .0.result.is_err());
                    assert!(prefix.observed.0 .0.retry.is_some());
                    assert!(!prefix.row.complete && !prefix.observed.0 .1.invalid);
                    assert_eq!(prefix.observed.0 .1.live, 0);
                    assert_eq!(prefix.observed.0 .1.allocated, prefix.observed.0 .1.freed);
                    if limit != 0 {
                        assert_eq!(
                            prefix.observed.0 .0.result.as_ref().unwrap_err(),
                            &primitive_failure(function, *event, limit, floor)
                        );
                        assert_eq!(prefix.observed.3.unwrap().requested, price);
                    }
                }
                let exact = run(
                    function,
                    0,
                    4,
                    shape,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    after,
                    0,
                    false,
                    false,
                );
                assert!(exact.observed.2 >= after);
                assert!(!exact.observed.0 .1.invalid && exact.observed.0 .1.live == 0);
                assert_eq!(exact.observed.0 .1.allocated, exact.observed.0 .1.freed);
            }
        }
    }
    assert!(reached.into_iter().all(|v| v));
}
#[test]
fn borrowed_reader_sources_keep_whole_pure2c_root_macro_and_all_original_pair_controls() {
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let root = before_main487::whole_source();
    assert_eq!(
        (root.len(), hash(root.as_bytes())),
        (
            22396,
            "23469f3a0ec012a0975f50e627811aa4874fdf4215b6a1c734b1c93a7a043955".to_owned()
        )
    );
    assert_eq!(
        hash(include_bytes!("lexical_cursor_main487.rs")),
        "b66e1c1f47583c8c687e352be56bcb01e49da9456b81c3e36862a359416e5583"
    );
    let mut tests = include_str!("main487_pair_scope_tests.rs").to_owned();
    for (after, before) in [
        (
            "use super::super::before_main487::{compare, PAYMENT_HEADER, PATH_INDEX, PAIR_SCOPE, PAIR_PRIMITIVE, ComparisonOwner, Step};\n",
            "",
        ),
        (
            "#[allow(clippy::duplicate_mod, reason = \"complete incoming2c tail owner independently compiled against genuine old comparator\")]\n#[path = \"main_b692_paired_tails.rs\"]",
            "#[path = \"pair_scope_tests/paired_tails.rs\"]",
        ),
        (
            "    type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;",
            "    use lexical_cursor::SharedUnitDriver;",
        ),
        (
            "pub(super) fn tail_receiver_prices(",
            "fn tail_receiver_prices(",
        ),
        (
            "pub(super) fn prepared_link_prices(",
            "fn prepared_link_prices(",
        ),
        (
            "pub(super) fn reachable_primitive_prices(",
            "fn reachable_primitive_prices(",
        ),
        (
            "pub(super) fn shared_step_prices(",
            "fn shared_step_prices(",
        ),
    ] {
        assert_eq!(tests.matches(after).count(), 1);
        tests = tests.replacen(after, before, 1);
    }
    assert_eq!(
        hash(tests.as_bytes()),
        "b46e7bfec30cd08b716e6070975651a38d235b0a8ad3ffd851d7534fc0b94d52"
    );
    assert_eq!(paired_value::ADDITIONAL_PRIMITIVES, 6 * PAYMENT_HEADER);
    assert_eq!(
        paired_value::LEFT_RETIRED_AS_REF,
        2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>()
    );
    assert_eq!(
        paired_value::LEFT_YIELD_AND_RECEIVE,
        2 * size_of::<&Rc<SkeletonObservation>>()
    );
    let current = include_str!("paired_value.rs");
    let body = current
        .split_once("macro_rules! borrowed_right_value {")
        .unwrap()
        .1
        .split_once("// Five actual new primitive sites:")
        .unwrap()
        .0;
    assert!(!body.contains("as_ref("));
    assert!(!body.contains("Some::<"));
    assert!(!body.contains("Rc::clone"));
    assert!(include_str!("../comparison_owner.rs").contains("borrowed_right_value!("));
}
