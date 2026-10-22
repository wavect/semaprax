//! Real shipping tails; complete pre-fusion and pure-main generations stay compiled.
use super::*;
fn tail_run(
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
fn delta(old: &pair_scope_census::Row, row: &pair_scope_census::Row) -> (usize, usize) {
    assert!(
        old.complete && row.complete,
        "complete invoice only, never a refused prefix"
    );
    assert_eq!(
        old.identity_probes,
        row.identity_probes + old.identity_empty_probes
    );
    assert_eq!(old.shared_boundaries, row.shared_boundaries);
    // The authentic old paired loop enters the loaded-guard payment on every
    // turn, even for ordinary tails. The fused inner loop retains all logical
    // work but never re-enters that three-boolean boundary guard.
    assert_eq!(old.shared_checks, row.shared_checks + row.fused_turns);
    assert_eq!(certified_heads::BOUNDARY_CHECK, 3 * size_of::<bool>());
    let removed_boundary_checks =
        certified_heads::BOUNDARY_CHECK * (old.shared_checks - row.shared_checks);
    let selector_header = PAYMENT_HEADER;
    let selector_payload = size_of::<bool>() * row.empty_selectors;
    let removed_empty_probes =
        (3 * size_of::<&[Rc<SkeletonObservation>]>()) * old.identity_empty_probes;
    let removed_right_rest = size_of::<&[Rc<SkeletonObservation>]>() * row.shared_boundaries;
    let current = super::super::paired_tail_added(row) + size_of::<ComparisonOwner<'static>>()
        - size_of::<before_paired_tail::ComparisonOwner<'static>>()
        + PAYMENT_HEADER
        + super::super::reader_payload_added(row)
        + selector_header
        + selector_payload
        + final_affine::delta(row).0;
    let removed = row.fused_turns
        * (super::super::legacy_matching()
            + 2 * (paired_value::STEP + paired_value::EMPTY + paired_value::TAIL))
        + super::super::reader_removed(row)
        + removed_empty_probes
        + removed_right_rest
        + removed_boundary_checks
        + final_affine::delta(row).1;
    (current, removed)
}
#[test]
fn paired_tail_composition_keeps_complete_real_generations_and_typed_invoices() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let mut reached = false;
    for length in [0, 1, 2, 4, 64] {
        for shape in [0, 1, 2, 3, 4, 5, 6] {
            for shared_hir in [false, true] {
                let old = tail_run(
                    function,
                    5,
                    length,
                    shape,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    shared_hir,
                    true,
                );
                let current = tail_run(
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
                let main = tail_run(
                    function,
                    6,
                    length,
                    shape,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    usize::MAX,
                    0,
                    shared_hir,
                    true,
                );
                assert!(
                    old.observed.0.0.result.is_ok()
                        && current.observed.0.0.result.is_ok()
                        && main.observed.0.0.result.is_ok(),
                    "length={length} shape={shape} shared_hir={shared_hir}; old={:?} current={:?} pure_main={:?}; rows old={:?} current={:?} pure_main={:?}",
                    old.observed.0.0,
                    current.observed.0.0,
                    main.observed.0.0,
                    old.row,
                    current.row,
                    main.row
                );
                assert_eq!(old.observed.0 .0, current.observed.0 .0);
                assert_eq!(main.observed.0 .0, current.observed.0 .0);
                assert_eq!(swaps(&old), swaps(&current));
                assert_eq!(swaps(&main), swaps(&current));
                assert!(old.row.complete && main.row.complete && current.row.complete);
                assert_eq!(old.row.pairs, current.row.pairs);
                assert_eq!(
                    old.row.value_steps,
                    current.row.value_steps + 2 * current.row.fused_turns
                );
                assert_eq!(
                    old.row.value_tails,
                    current.row.value_tails + 2 * current.row.fused_turns
                );
                assert_eq!(
                    old.row.pair_matches_admitted,
                    current.row.pair_matches_admitted + current.row.fused_turns
                );
                let (added, removed) = delta(&old.row, &current.row);
                assert_eq!(current.observed.2 + removed, old.observed.2 + added);
                if shape == 6 && length == 64 {
                    assert!(current.row.fused_turns > 0);
                    assert!(
                        current.observed.2 < old.observed.2,
                        "the genuine long ordinary tails must amortize the complete cold bank"
                    );
                    reached = true;
                }
                eprintln!(
                    "paired-tail length={length} shape={shape} shared_hir={shared_hir} old8102={} pure_main={} current={} selection={} marks={} turns={} work={}; complete unit fixture, not app admission",
                    old.observed.2,
                    main.observed.2,
                    current.observed.2,
                    current.row.mask_checks,
                    current.row.tail_marks,
                    current.row.fused_turns,
                    current.row.tail_work
                );
            }
        }
    }
    assert!(reached);
}
#[test]
fn paired_tail_composition_preserves_every_individual_work_cut_and_six_field_refusal() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    for (shape, fault) in [
        (6, 0),
        (6, 4),
        (6, 5),
        (6, 6),
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (0, 7),
        (0, 8),
    ] {
        let full = tail_run(
            function,
            5,
            4,
            shape,
            fault,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            false,
            true,
        );
        for units in 0..=MAX_REPLAY_WORK_UNITS - full.observed.0 .0.remaining {
            let old = tail_run(
                function,
                5,
                4,
                shape,
                fault,
                units,
                usize::MAX,
                0,
                false,
                true,
            );
            let current = tail_run(
                function,
                0,
                4,
                shape,
                fault,
                units,
                usize::MAX,
                0,
                false,
                true,
            );
            let main = tail_run(
                function,
                6,
                4,
                shape,
                fault,
                units,
                usize::MAX,
                0,
                false,
                true,
            );
            assert_eq!(
                old.observed.0 .0, current.observed.0 .0,
                "shape={shape} fault={fault} units={units}"
            );
            assert_eq!(
                main.observed.0 .0, current.observed.0 .0,
                "pure main shape={shape} fault={fault} units={units}"
            );
            assert_eq!(swaps(&old), swaps(&current));
            assert_eq!(swaps(&main), swaps(&current));
        }
    }
}
#[test]
fn paired_tail_composition_exact_short_zero_floor_and_sticky_banks_keep_full_drop() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let success = tail_run(
        function,
        0,
        4,
        6,
        0,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
        false,
    );
    assert!(success.observed.0 .0.result.is_ok() && success.row.complete);
    for (phase, count, price) in [
        (
            "ordinary plan paired tail selection",
            success.row.mask_checks,
            paired_value::TAIL_SELECT,
        ),
        (
            "ordinary plan paired tail turn",
            success.row.fused_turns,
            paired_value::TAIL_TURN,
        ),
    ] {
        let selected: Vec<_> = success.events.iter().filter(|e| e.phase == phase).collect();
        assert!(count > 0);
        assert_eq!(selected.len(), count);
        assert_eq!(
            selected
                .iter()
                .map(|e| e.after.unwrap() - e.before)
                .sum::<usize>(),
            count * price
        );
        for event in selected {
            let after = event.after.unwrap();
            for (limit, floor) in [(event.before, 0), (after - 1, 0), (after, 1), (0, 0)] {
                let r = tail_run(
                    function,
                    0,
                    4,
                    6,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    limit,
                    floor,
                    false,
                    false,
                );
                assert!(r.observed.1 && r.observed.0 .0.result.is_err() && !r.row.complete);
                assert!(r.observed.0 .0.retry.is_some());
                if limit != 0 {
                    let refusal = r.observed.3.unwrap();
                    assert_eq!(refusal.requested, price);
                    assert_eq!(refusal.remaining, limit - event.before);
                    assert_eq!(refusal.floor, floor);
                }
            }
            let exact = tail_run(
                function,
                0,
                4,
                6,
                0,
                MAX_REPLAY_WORK_UNITS,
                after,
                0,
                false,
                false,
            );
            assert!(exact
                .events
                .iter()
                .any(|e| e.phase == phase && e.before == event.before && e.after == Some(after)));
        }
    }
}
#[test]
fn paired_tail_composition_restores_all_complete_old_roots_macros_and_main_test_bodies() {
    use sha2::{Digest, Sha256};
    let old = before_paired_tail::whole_source();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "96c1ec72eb31b61e171ed8dfe44ba030c0a86b579852a4afb604272bf6633512"
    );
    before_main_b692::whole_source();
    for (source, sha) in [
        (
            include_str!("lexical_cursor_paired_tail.rs"),
            "c63b8f98af8b1fa55f7e305819efc87107477db4a92f57743365544885b4973c",
        ),
        (
            include_str!("paired_value_paired_tail.rs"),
            "40d1e9db7ef14c4791fcad0476fe1abcf2732022fd2d2a89e5b9b811ed5b6437",
        ),
        (
            include_str!("shared_prefix_paired_tail.rs"),
            "faf2391a83182d4b60d1be8561c404ac7af427fcd1f59ff2bfa45c85058244d0",
        ),
        (
            include_str!("certified_heads_paired_tail.rs"),
            "9467284ebc12b7a48380d41f9d3d7005bbbcb707fff02fe81b4e48527402f76d",
        ),
        (
            include_str!("lexical_cursor_main_b692.rs"),
            "b54e7239b1e5c1f5a2dbe914d8ad4dbf17b343784d10d8cb1b1e450baf626f6c",
        ),
    ] {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            sha
        );
    }
    let mut tests = include_str!("main_b692_pair_scope_tests.rs").to_owned();
    assert_eq!(tests.matches("use super::super::before_main_b692::{compare, PAYMENT_HEADER, PATH_INDEX, PAIR_SCOPE, PAIR_PRIMITIVE, ComparisonOwner, Step};\n").count(),1);
    tests=tests.replacen("use super::super::before_main_b692::{compare, PAYMENT_HEADER, PATH_INDEX, PAIR_SCOPE, PAIR_PRIMITIVE, ComparisonOwner, Step};\n","",1);
    assert_eq!(
        tests
            .matches("#[path = \"main_b692_paired_tails.rs\"]\nmod paired_tails;")
            .count(),
        1
    );
    tests = tests.replacen(
        "#[path = \"main_b692_paired_tails.rs\"]\nmod paired_tails;",
        "#[path = \"pair_scope_tests/paired_tails.rs\"]\nmod paired_tails;",
        1,
    );
    assert_eq!(
        tests
            .matches("    type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;")
            .count(),
        1
    );
    tests = tests.replacen(
        "    type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;",
        "    use lexical_cursor::SharedUnitDriver;",
        1,
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(tests.as_bytes()))
        ),
        "76ad602250fc7e4c0fa09714b35266c3ee738c2eba0136092fb5111eec092dae"
    );
    let child = include_str!("main_b692_paired_tails.rs");
    let after = "include_str!(\"main_b692_paired_tails.rs\")";
    assert_eq!(child.matches(after).count(), 1);
    let child = child.replacen(after, "include_str!(\"paired_tails.rs\")", 1);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(child.as_bytes()))
        ),
        "cceea03772195a341b852575b1aa39c22ba5958ebed27ad29a5f2823b7f2fb4a"
    );
}
#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn paired_tail_composition_feature_counts_actual_admitted_banks_and_each_work_frontier() {
    use crate::workflow_profile::plan_comparison::{Binding, Capture, Site};
    use sha2::{Digest, Sha256};
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let capture = Capture::begin(Binding::caller_labelled(
        "paired tail genuine fixture",
        Sha256::digest(include_bytes!("paired_tail_composition_tests.rs")).into(),
    ))
    .unwrap();
    let r = tail_run(
        function,
        0,
        4,
        6,
        0,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
        true,
    );
    let report = capture.finish().unwrap();
    assert!(r.row.complete && r.observed.0 .0.result.is_ok());
    assert!(
        report.counts_complete
            && report.detail_complete
            && !report.overflow
            && report.row_count == 1
    );
    for (site, n, bytes) in [
        (
            Site::TailMaskAdmitted,
            r.row.mask_checks,
            paired_value::TAIL_SELECT,
        ),
        (
            Site::TailMarkAdmitted,
            r.row.tail_marks,
            paired_value::TAIL_MARK,
        ),
        (
            Site::TailTurnAdmitted,
            r.row.fused_turns,
            paired_value::TAIL_TURN,
        ),
        (Site::TailWork, r.row.tail_work, 0),
        (Site::BorrowedRightStep, r.row.pair_right_steps, 0),
        (Site::BorrowedRightTail, r.row.pair_right_tails, 0),
    ] {
        assert!(n > 0);
        assert_eq!(report.counts.at(site), n as u64);
        assert_eq!(
            report.counts.admitted_bytes[site as usize],
            (n * bytes) as u64
        );
    }
    let mut partial_work = false;
    for units in 0..=(MAX_REPLAY_WORK_UNITS - r.observed.0 .0.remaining) {
        let capture =
            Capture::begin(Binding::caller_labelled("paired tail work-cut", [0; 32])).unwrap();
        let prefix = tail_run(function, 0, 4, 6, 0, units, usize::MAX, 0, false, true);
        let report = capture.finish().unwrap();
        assert!(report.counts_complete && report.detail_complete && !report.overflow);
        for (site, count, amount) in [
            (
                Site::TailMaskAdmitted,
                prefix.row.mask_checks,
                paired_value::TAIL_SELECT,
            ),
            (
                Site::TailMarkAdmitted,
                prefix.row.tail_marks,
                paired_value::TAIL_MARK,
            ),
            (
                Site::TailTurnAdmitted,
                prefix.row.fused_turns,
                paired_value::TAIL_TURN,
            ),
            (Site::TailWork, prefix.row.tail_work, 0),
            (Site::BorrowedRightStep, prefix.row.pair_right_steps, 0),
            (Site::BorrowedRightTail, prefix.row.pair_right_tails, 0),
        ] {
            assert_eq!(report.counts.at(site), count as u64);
            assert_eq!(
                report.counts.admitted_bytes[site as usize],
                (count * amount) as u64
            );
        }
        partial_work |= prefix.row.tail_work > 0 && prefix.observed.0 .0.result.is_err();
    }
    assert!(
        partial_work,
        "real refused prefix must retain every individually completed tail work event"
    );
}

#[test]
fn paired_tail_live_unwind_keeps_all_owned_storage_and_reports_only_reached_work() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    let (mut actual, expected) = fixture(4, 1, true, 0);
    let common = actual.arena[0].items.clone();
    for path in &mut actual.paths {
        let mut tail = common.clone();
        tail.extend(path.history.tail.iter().cloned());
        path.history.previous = None;
        path.history.tail = tail;
    }
    drop(common);
    pair_scope_census::reset();
    frontiers::reset();
    pair_scope_census::unwind_next_tail_work();
    let ((unwound, metrics), refused, _, receipt) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                paid!(
                    &mut budget,
                    function,
                    COMPARE_CALL_CONTROLS,
                    "paired tail unwind receiver"
                )
                .unwrap();
                compare(function, &mut budget, actual, &expected).unwrap();
            }));
            let unwound = outcome.is_err();
            drop(outcome);
            unwound
        })
    });
    assert!(unwound && !refused && receipt.is_none());
    balanced(metrics);
    frontiers::finish();
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].ended && rows[0].unwound && !rows[0].complete);
    assert_eq!(rows[0].fused_turns, 1);
    assert_eq!(rows[0].tail_work, 1);
}
