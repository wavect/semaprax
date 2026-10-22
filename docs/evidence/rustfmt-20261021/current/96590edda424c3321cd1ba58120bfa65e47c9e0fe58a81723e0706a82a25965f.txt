//! Both independently compiled complete shipping generations, without a synthetic baseline.
use super::*;

fn observe(
    function: &ResolvedFunction,
    generation: usize,
    length: usize,
    shape: usize,
    units: usize,
    limit: usize,
    floor: usize,
    shared_hir: bool,
    fault: usize,
) -> Run {
    heads_run_generation(
        function, length, 3, true, fault, generation, units, limit, floor, shared_hir, true, shape,
    )
}

// Current + removed = complete fd4c + added. Every coefficient below is a
// literal actual typed bank, never an observed debit or a fabricated stage.
fn link_delta(old: &pair_scope_census::Row, r: &pair_scope_census::Row) -> (usize, usize) {
    assert!(
        old.complete && r.complete,
        "whole matched caller, no prefix price inference"
    );
    assert_eq!(
        old.identity_probes,
        r.identity_probes + old.identity_empty_probes
    );
    assert_eq!(old.shared_boundaries, r.shared_boundaries);
    // The old loop paid its loaded guard before each ordinary paired turn;
    // the fused inner loop bypasses that physical guard, retaining its work.
    assert_eq!(old.shared_checks, r.shared_checks + r.fused_turns);
    assert_eq!(certified_heads::BOUNDARY_CHECK, 3 * size_of::<bool>());
    let removed_boundary_checks =
        certified_heads::BOUNDARY_CHECK * (old.shared_checks - r.shared_checks);
    let selector_header = PAYMENT_HEADER;
    let selector_payload = size_of::<bool>() * r.empty_selectors;
    let removed_empty_probes =
        3 * size_of::<&[Rc<SkeletonObservation>]>() * old.identity_empty_probes;
    assert_eq!(
        super::super::legacy_matching(),
        before_pair_links::PAIR_MATCH
    );
    let old_links = 4 * size_of::<&Vec<usize>>()
        + 4 * size_of::<&[usize]>()
        + 4 * size_of::<usize>()
        + 3 * size_of::<bool>();
    let old_boundary = 4 * size_of::<&mut Vec<usize>>()
        + 2 * size_of::<Option<usize>>()
        + 10 * size_of::<usize>()
        + 6 * size_of::<bool>()
        + 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<Ordering>();
    let old_capture = 2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>();
    let old_turn = 4 * size_of::<usize>() + size_of::<bool>();
    let links = 2 * size_of::<&[usize]>() + 4 * size_of::<usize>() + 3 * size_of::<bool>();
    // Full current boundary already excludes the one removed right-rest
    // assignment. Do not also credit it on the removed side of this equation.
    let boundary = 2 * size_of::<&[usize]>()
        + 8 * size_of::<usize>()
        + 4 * size_of::<bool>()
        + size_of::<&[Rc<SkeletonObservation>]>()
        + size_of::<Ordering>();
    let capture = 2 * size_of::<&[Rc<SkeletonObservation>]>()
        + 4 * size_of::<usize>()
        + 6 * size_of::<std::iter::Take<std::iter::Repeat<()>>>()
        + 2 * size_of::<&mut std::iter::Take<std::iter::Repeat<()>>>()
        + 2 * size_of::<Option<()>>();
    assert_eq!(
        (links, boundary, capture),
        (
            certified_heads::BOUNDARY_LINKS,
            certified_heads::BOUNDARY_BODY,
            certified_heads::RUN_CAPTURE
        )
    );
    // The distinct complete old/new load and split packets have equal totals;
    // this is an exact formation witness, not an anonymous same-width donor.
    let old_pop = 2 * size_of::<&mut Vec<usize>>()
        + size_of::<Option<usize>>()
        + size_of::<usize>()
        + 2 * size_of::<bool>();
    let pop = 2 * size_of::<&[usize]>() + size_of::<usize>() + 2 * size_of::<bool>();
    assert_eq!(old_pop, pop);
    assert_eq!(pop, certified_heads::POP);
    assert_eq!(paired_value::PAIR_LOAD, paired_value::LOAD);
    (
        2 * (2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>()) * r.pairs
            + links * r.shared_links
            + boundary * r.shared_boundaries
            + capture * r.shared_runs
            + super::super::paired_tail_added(r)
            + PAYMENT_HEADER // actual reader's additional success-work site
            + super::super::reader_payload_added(r)
            + selector_header + selector_payload
            + final_affine::delta(r).0,
        PAYMENT_HEADER
            + old_links * r.shared_links
            + old_boundary * r.shared_boundaries
            + old_capture * r.shared_runs
            + old_turn * r.shared_turns
            + r.fused_turns
                * (super::super::legacy_matching()
                    + 2 * (paired_value::STEP + paired_value::EMPTY + paired_value::TAIL))
            + super::super::reader_removed(r)
            + removed_empty_probes
            + removed_boundary_checks
            + final_affine::delta(r).1,
    )
}

#[test]
fn borrowed_pair_links_keep_both_whole_generations_and_all_original_source_bytes() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        size_of::<ComparisonOwner<'static>>(),
        size_of::<before_pair_links::ComparisonOwner<'static>>()
    );
    before_pair_links::whole_source();
    before_main001::whole_source();
    for (source, sha) in [
        (
            include_str!("lexical_cursor_pair_links.rs"),
            "c63b8f98af8b1fa55f7e305819efc87107477db4a92f57743365544885b4973c",
        ),
        (
            include_str!("paired_value_pair_links.rs"),
            "f68af76308eb729dd805fd1453d23e738fa6cf3aa4ab9bbbe9518d72323c341e",
        ),
        (
            include_str!("shared_prefix_pair_links.rs"),
            "faf2391a83182d4b60d1be8561c404ac7af427fcd1f59ff2bfa45c85058244d0",
        ),
        (
            include_str!("certified_heads_pair_links.rs"),
            "4f77590d854ac3982ec412d797fe559b92ab781896b26e8b3fee1299064529ae",
        ),
        (
            include_str!("lexical_cursor_main001.rs"),
            "f70a5ec7456c2b0e922b302aa194f8303575e1f7e794a9370ee409744538a5d3",
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
    let mut old_tests = include_str!("main001_pair_scope_tests.rs").to_owned();
    let import = "use super::super::before_main001::{compare, PAYMENT_HEADER, PATH_INDEX, PAIR_SCOPE, PAIR_PRIMITIVE};\n";
    assert_eq!(old_tests.matches(import).count(), 1);
    old_tests = old_tests.replacen(import, "", 1);
    for name in [
        "tail_receiver_prices",
        "prepared_link_prices",
        "reachable_primitive_prices",
        "shared_step_prices",
    ] {
        let after = format!("pub(super) fn {name}(");
        assert_eq!(old_tests.matches(&after).count(), 1);
        old_tests = old_tests.replacen(&after, &format!("fn {name}("), 1);
    }
    let driver = "    type SharedUnitDriver = std::iter::Take<std::iter::Repeat<()>>;";
    assert_eq!(old_tests.matches(driver).count(), 1);
    old_tests = old_tests.replacen(driver, "    use lexical_cursor::SharedUnitDriver;", 1);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old_tests.as_bytes()))
        ),
        "8484121abb7afd47a3baf21d1eb36875622f31ef72e65a867728b7af1ec34d26"
    );
}

#[test]
fn borrowed_pair_links_and_unit_driver_keep_complete_invoices_six_fields_work_swaps_and_drop() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    for hir in [false, true] {
        for (length, shape) in [
            (0, 0),
            (1, 0),
            (8, 0),
            (32, 0),
            (8, 1),
            (8, 2),
            (8, 3),
            (8, 4),
            (8, 5),
            (1, 6), // Genuine ordinary tails enter the fused inner loop.
        ] {
            let old = observe(
                function,
                3,
                length,
                shape,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir,
                0,
            );
            let current = observe(
                function,
                0,
                length,
                shape,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir,
                0,
            );
            let incoming = observe(
                function,
                4,
                length,
                shape,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                hir,
                0,
            );
            assert_eq!(old.observed.0 .0, current.observed.0 .0);
            assert_eq!(incoming.observed.0 .0, current.observed.0 .0);
            assert!(current.observed.0 .0.result.is_ok());
            assert!(old.row.complete && current.row.complete && incoming.row.complete);
            assert_eq!(old.observed.0 .1, current.observed.0 .1);
            assert_eq!(swaps(&old), swaps(&current));
            assert_eq!(
                (
                    old.row.shared_runs,
                    old.row.shared_turns,
                    old.row.shared_links,
                    old.row.shared_boundaries
                ),
                (
                    current.row.shared_runs,
                    current.row.shared_turns,
                    current.row.shared_links,
                    current.row.shared_boundaries
                )
            );
            let (added, removed) = link_delta(&old.row, &current.row);
            assert_eq!(
                current.observed.2 + removed,
                old.observed.2 + added,
                "whole invoices length={length} shape={shape} HIR={hir}"
            );
            eprintln!(
                "OPT pair-links whole fd4c/current/pure001={}/{}/{} length={length} shape={shape}; no universal or application claim",
                old.observed.2, current.observed.2, incoming.observed.2
            );
            let spent = MAX_REPLAY_WORK_UNITS - old.observed.0 .0.remaining;
            for fault in 0..=3 {
                for cut in 0..=spent {
                    let old = observe(function, 3, length, shape, cut, usize::MAX, 0, hir, fault);
                    let current =
                        observe(function, 0, length, shape, cut, usize::MAX, 0, hir, fault);
                    let incoming =
                        observe(function, 4, length, shape, cut, usize::MAX, 0, hir, fault);
                    assert_eq!(
                        old.observed.0 .0, current.observed.0 .0,
                        "fd4c six fields/work cut={cut} fault={fault}"
                    );
                    assert_eq!(
                        incoming.observed.0 .0, current.observed.0 .0,
                        "main001 six fields/work cut={cut} fault={fault}"
                    );
                    assert_eq!(swaps(&old), swaps(&current));
                }
            }
        }
    }
}

#[test]
fn borrowed_pair_link_and_unit_driver_receiving_keeps_exact_short_zero_floor_sticky_drop() {
    let program = super::super::super::super::super::tests::program();
    let function = &program.functions[0];
    for generation in [0, 3, 4] {
        for shape in [0, 2, 4, 5] {
            let success = heads_run_generation(
                function,
                8,
                3,
                true,
                0,
                generation,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                false,
                false,
                shape,
            );
            assert!(success.observed.0 .0.result.is_ok());
            let total = success.observed.2;
            for (limit, floor, pass) in [
                (total, 0, true),
                (total - 1, 0, false),
                (0, 0, false),
                (total, 1, false),
            ] {
                let r = observe(
                    function,
                    generation,
                    8,
                    shape,
                    MAX_REPLAY_WORK_UNITS,
                    limit,
                    floor,
                    false,
                    0,
                );
                assert_eq!(r.observed.0 .0.result.is_ok(), pass);
                assert_eq!(r.observed.1, !pass);
                if !pass {
                    assert!(r.observed.0 .0.retry.is_some());
                }
            }
            for event in &success.events {
                if !(event.phase.contains("pair-local paths")
                    || event.phase.contains("link pop")
                    || event.phase.contains("boundary")
                    || event.phase.contains("numeric run"))
                {
                    continue;
                }
                let Some(after) = event.after.filter(|&after| after > event.before) else {
                    continue;
                };
                for (limit, floor) in [(event.before, 0), (after - 1, 0), (after, 1)] {
                    let r = observe(
                        function,
                        generation,
                        8,
                        shape,
                        MAX_REPLAY_WORK_UNITS,
                        limit,
                        floor,
                        false,
                        0,
                    );
                    assert!(r.observed.1 && r.observed.0 .0.retry.is_some());
                    assert_eq!(
                        r.observed.0 .0.result.as_ref().unwrap_err(),
                        &primitive_failure(function, *event, limit, floor)
                    );
                }
            }
        }
    }
}
