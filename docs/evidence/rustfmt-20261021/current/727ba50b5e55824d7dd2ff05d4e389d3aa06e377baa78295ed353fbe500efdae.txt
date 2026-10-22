//! Complete immutable7760 versus the shipping lexical empty selector.
use super::certified_head_tests::heads_run_generation;
use super::*;
const SELECT: &str = "ordinary plan certified suffix empty selector";
fn observe(
    f: &ResolvedFunction,
    old: bool,
    length: usize,
    shape: usize,
    shared: bool,
    hir: bool,
    fault: usize,
    work: usize,
    bytes: usize,
    floor: usize,
    trace: bool,
) -> Run {
    heads_run_generation(
        f,
        length,
        3,
        shared,
        fault,
        if old { 8 } else { 0 },
        work,
        bytes,
        floor,
        hir,
        !trace,
        shape,
    )
}
fn full(
    f: &ResolvedFunction,
    old: bool,
    length: usize,
    shape: usize,
    shared: bool,
    hir: bool,
) -> Run {
    observe(
        f,
        old,
        length,
        shape,
        shared,
        hir,
        0,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
    )
}
fn report(old: &Run, current: &Run, label: &str) {
    let header = 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<Result<(), Diagnostic>>();
    let probe = 3 * size_of::<&[Rc<SkeletonObservation>]>();
    let selector = size_of::<bool>();
    let right_rest = size_of::<&[Rc<SkeletonObservation>]>();
    let old_body = 2 * size_of::<&[usize]>()
        + 8 * size_of::<usize>()
        + 4 * size_of::<bool>()
        + 2 * right_rest
        + size_of::<Ordering>();
    assert_eq!(certified_heads::BOUNDARY_BODY + right_rest, old_body);
    // Equal actual admitted bodies, not inferred future completed work.
    assert_eq!(old.row.shared_boundaries, current.row.shared_boundaries);
    assert_eq!(
        (header, probe, selector),
        (
            PAYMENT_HEADER,
            shared_prefix::PROBE,
            certified_heads::EMPTY_SELECT
        )
    );
    assert!(old.row.complete && current.row.complete);
    assert_eq!(old.observed.0 .0, current.observed.0 .0);
    assert_eq!(swaps(old), swaps(current));
    assert_eq!(
        old.observed.0 .1, current.observed.0 .1,
        "no new owning layout: full allocator metrics through Drop"
    );
    assert_eq!(old.row.empty_selectors, 0);
    assert_eq!(current.row.identity_empty_probes, 0);
    assert_eq!(current.row.identity_work, old.row.identity_work);
    assert_eq!(
        current.row.shared_boundary_complete,
        old.row.shared_boundary_complete
    );
    assert_eq!(current.row.shared_runs, old.row.shared_runs);
    assert_eq!(current.row.shared_publishes, old.row.shared_publishes);
    assert_eq!(
        current.row.empty_selectors,
        current.row.identity_hits + current.row.shared_boundary_complete
    );
    assert_eq!(
        current.row.empty_selectors,
        current.row.empty_selector_skips + current.row.shared_runs
    );
    assert_eq!(current.row.identity_probes, current.row.shared_runs);
    assert_eq!(
        old.row.identity_probes,
        current.row.identity_probes + old.row.identity_empty_probes
    );
    assert_eq!(
        old.row.identity_empty_probes,
        current.row.empty_selector_skips + current.row.shared_publishes
    );
    assert_eq!(
        current.observed.2
            + probe * old.row.identity_empty_probes
            + right_rest * current.row.shared_boundaries
            + final_affine::delta(&current.row).1,
        old.observed.2 + header + selector * current.row.empty_selectors
            + final_affine::delta(&current.row).0,
        "complete real old/new bills: independent selector/header plus removed actual right-rest transport"
    );
    eprintln!(
        "OPT905_906 {label}: old={} current={} selector={} skipped={} old_empty_probes={} retained_probes={} runs={} work={} old_alloc={:?} current_alloc={:?}; selector_added={} selector_header={} old_empty_removed={} right_rest_removed={}; whole comparator and Drop, cold losses retained; not whole32/application admission",
        old.observed.2,
        current.observed.2,
        current.row.empty_selectors,
        current.row.empty_selector_skips,
        old.row.identity_empty_probes,
        current.row.identity_probes,
        current.row.shared_publishes,
        current.row.identity_work,
        old.observed.0.1,
        current.observed.0.1,
        selector * current.row.empty_selectors,
        header,
        probe * old.row.identity_empty_probes,
        right_rest * current.row.shared_boundaries
    );
}
#[test]
fn lexical_empty_selector_complete_bills_keep_cold_losses_and_real_dense_net_for_both_hir_routes() {
    let p = super::super::super::super::tests::program();
    let f = &p.functions[0];
    let mut cold = false;
    let mut favorable = false;
    for hir in [false, true] {
        for length in [0, 1, 2, 8, 128] {
            for (shape, shared) in [
                (0, true),
                (0, false),
                (1, true),
                (2, true),
                (3, true),
                (4, true),
                (5, true),
                (6, true),
            ] {
                let old = full(f, true, length, shape, shared, hir);
                let current = full(f, false, length, shape, shared, hir);
                assert!(
                    current.observed.0 .0.result.is_ok(),
                    "length={length} shape={shape} shared={shared} hir={hir} result={:?}",
                    current.observed.0 .0
                );
                report(
                    &old,
                    &current,
                    &format!("length={length} shape={shape} shared={shared} hir={hir}"),
                );
                if length == 0 {
                    assert!(current.observed.2 > old.observed.2);
                    cold = true;
                }
                if length == 128 && shape == 4 {
                    assert!(
                        current.observed.2 < old.observed.2,
                        "genuine singleton chain must pay its full new cold bank before net"
                    );
                    favorable = true;
                }
                if !shared || shape == 6 {
                    assert_eq!(current.row.empty_selectors, 0);
                }
            }
        }
    }
    assert!(cold && favorable);
}
#[test]
fn lexical_empty_selector_keeps_every_original_work_cut_and_all_malformed_fallbacks() {
    let p = super::super::super::super::tests::program();
    let f = &p.functions[0];
    for hir in [false, true] {
        for (length, shape, shared) in [
            (0, 0, true),
            (1, 0, true),
            (4, 0, true),
            (4, 0, false),
            (4, 1, true),
            (4, 2, true),
            (4, 3, true),
            (4, 4, true),
            (4, 5, true),
            (4, 6, true),
        ] {
            let success = full(f, false, length, shape, shared, hir);
            assert!(success.observed.0 .0.result.is_ok());
            let used = MAX_REPLAY_WORK_UNITS - success.observed.0 .0.remaining;
            for work in 0..=used {
                let old = observe(
                    f,
                    true,
                    length,
                    shape,
                    shared,
                    hir,
                    0,
                    work,
                    usize::MAX,
                    0,
                    false,
                );
                let current = observe(
                    f,
                    false,
                    length,
                    shape,
                    shared,
                    hir,
                    0,
                    work,
                    usize::MAX,
                    0,
                    false,
                );
                assert_eq!(
                    old.observed.0 .0, current.observed.0 .0,
                    "all six sticky fields/order work={work} length={length} shape={shape}"
                );
                assert_eq!(swaps(&old), swaps(&current));
                assert!(!old.observed.1 && !current.observed.1);
                assert_eq!(current.row.identity_empty_probes, 0);
            }
        }
        for fault in 1..=8 {
            let old = observe(
                f,
                true,
                4,
                0,
                true,
                hir,
                fault,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                false,
            );
            let current = observe(
                f,
                false,
                4,
                0,
                true,
                hir,
                fault,
                MAX_REPLAY_WORK_UNITS,
                usize::MAX,
                0,
                false,
            );
            assert_eq!(
                old.observed.0 .0, current.observed.0 .0,
                "malformed/terminal/content fault={fault}"
            );
            assert_eq!(swaps(&old), swaps(&current));
        }
    }
}
#[test]
fn lexical_empty_selector_exact_short_zero_floor_and_mixed_frontiers_are_actual_admissions() {
    let p = super::super::super::super::tests::program();
    let f = &p.functions[0];
    for hir in [false, true] {
        let success = observe(
            f,
            false,
            4,
            0,
            true,
            hir,
            0,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            true,
        );
        assert!(success.observed.0 .0.result.is_ok());
        let selectors: Vec<_> = success
            .events
            .iter()
            .copied()
            .filter(|e| e.phase == SELECT)
            .collect();
        assert!(!selectors.is_empty());
        assert_eq!(selectors.len(), success.row.empty_selectors);
        for event in selectors {
            let after = event.after.unwrap();
            assert_eq!(after - event.before, size_of::<bool>());
            for (limit, floor) in [(event.before, 0), (after - 1, 0), (after, 1)] {
                let r = observe(
                    f,
                    false,
                    4,
                    0,
                    true,
                    hir,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    limit,
                    floor,
                    true,
                );
                assert!(
                    r.observed.1
                        && r.observed.0 .0.result.is_err()
                        && r.observed.0 .0.retry.is_some()
                );
                let refusal = r.observed.3.unwrap();
                assert_eq!(
                    (refusal.requested, refusal.remaining, refusal.floor),
                    (size_of::<bool>(), limit - event.before, floor)
                );
                assert!(!r.row.complete);
                assert_eq!(
                    r.row.empty_selectors,
                    r.events
                        .iter()
                        .filter(|e| e.phase == SELECT && e.after.is_some())
                        .count()
                );
            }
            let exact = observe(
                f,
                false,
                4,
                0,
                true,
                hir,
                0,
                MAX_REPLAY_WORK_UNITS,
                after,
                0,
                true,
            );
            assert!(exact
                .events
                .iter()
                .any(|e| e.phase == SELECT && e.before == event.before && e.after == Some(after)));
        }
        // Shape0 keeps all four observations in one segment: it reaches the
        // selector but never forms a boundary body. Exercise the independent
        // body frontier with four genuine singleton segments instead.
        let boundary_shape = 4;
        let body_success = observe(
            f,
            false,
            4,
            boundary_shape,
            true,
            hir,
            0,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            true,
        );
        assert!(body_success.observed.0 .0.result.is_ok() && body_success.row.complete);
        let body_events: Vec<_> = body_success
            .events
            .iter()
            .copied()
            .filter(|e| e.phase == "ordinary plan certified boundary transition")
            .collect();
        assert!(!body_events.is_empty());
        assert_eq!(body_events.len(), body_success.row.shared_boundaries);
        for event in body_events {
            let after = event.after.unwrap();
            assert_eq!(after - event.before, certified_heads::BOUNDARY_BODY);
            for (limit, floor) in [(event.before, 0), (after - 1, 0), (after, 1)] {
                let r = observe(
                    f,
                    false,
                    4,
                    boundary_shape,
                    true,
                    hir,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    limit,
                    floor,
                    true,
                );
                assert!(
                    r.observed.1
                        && r.observed.0 .0.result.is_err()
                        && r.observed.0 .0.retry.is_some()
                );
                let refusal = r.observed.3.unwrap();
                assert_eq!(
                    (refusal.requested, refusal.remaining, refusal.floor),
                    (certified_heads::BOUNDARY_BODY, limit - event.before, floor)
                );
                assert!(!r.row.complete);
            }
            let exact = observe(
                f,
                false,
                4,
                boundary_shape,
                true,
                hir,
                0,
                MAX_REPLAY_WORK_UNITS,
                after,
                0,
                true,
            );
            assert!(exact.events.iter().any(|e| e.phase == event.phase
                && e.before == event.before
                && e.after == Some(after)));
        }
        let used = MAX_REPLAY_WORK_UNITS - success.observed.0 .0.remaining;
        for old in [false, true] {
            let complete = full(f, old, 4, 0, true, hir);
            let total = complete.observed.2;
            for bytes in [0, total - 1, total] {
                let r = observe(
                    f,
                    old,
                    4,
                    0,
                    true,
                    hir,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    bytes,
                    0,
                    false,
                );
                assert_eq!(r.observed.0 .0.result.is_ok(), bytes == total);
                assert_eq!(r.observed.1, bytes != total);
                if bytes != total {
                    assert!(r.observed.0 .0.retry.is_some());
                }
            }
            // Mixed physical/work refusal is observed per actual generation;
            // the added selector deliberately changes physical first refusal.
            for work in [0, 1, used / 2, used] {
                let r = observe(f, old, 4, 0, true, hir, 0, work, total / 2, 0, false);
                assert!(r.observed.0 .0.result.is_err());
                assert!(r.observed.0 .0.retry.is_some());
                eprintln!(
                    "OPT905 mixed old={old} hir={hir} work={work} bytes={} outcome={:?} refusal={:?}",
                    total / 2,
                    r.observed.0.0,
                    r.observed.3
                );
            }
        }
    }
}
#[test]
fn lexical_empty_selector_real_producer_keeps_complete_old_current_caller_and_drop() {
    let p = super::super::super::super::tests::program();
    for f in &p.functions {
        let flat = expected(&p, f);
        let mut shared = flat.clone();
        for path in &mut shared {
            path.observations.prefix = path.observations.freeze();
            path.observations.items.clear();
        }
        for expected in [&flat, &shared] {
            let run = |old| {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let mut b = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                        let r = (|| {
                            paid!(&mut b, f, PRODUCE_CALL_CONTROLS, "905 real producer caller")?;
                            let actual = produce(f, &mut b)?;
                            paid!(
                                &mut b,
                                f,
                                COMPARE_CALL_CONTROLS,
                                "905 complete comparator caller"
                            )?;
                            if old {
                                before_empty_selector::compare(f, &mut b, actual, expected)
                            } else {
                                compare(f, &mut b, actual, expected)
                            }
                        })();
                        let out = (r.as_ref().map(|_| ()).map_err(failure), b.remaining);
                        drop(r);
                        drop(b);
                        out
                    })
                })
            };
            let old = run(true);
            let current = run(false);
            assert!(old.0 .0 .0.is_ok());
            assert_eq!(old.0 .0, current.0 .0);
            assert_eq!(old.0 .1, current.0 .1);
            balanced(old.0 .1);
            balanced(current.0 .1);
            eprintln!(
                "OPT905 full real producer old/current paid={}/{} allocator={:?}/{:?}; complete source function {:?}, not whole32/app admission",
                old.2, current.2, old.0.1, current.0.1, f.name
            );
        }
    }
}
#[test]
fn lexical_empty_selector_live_numeric_unwind_keeps_both_complete_owners_drop_balanced() {
    let p = super::super::super::super::tests::program();
    let f = &p.functions[0];
    for old in [false, true] {
        let (actual, expected) = fixture(4, 3, true, 0);
        pair_scope_census::reset();
        frontiers::reset();
        pair_scope_census::unwind_next_identity_observation();
        let (_, metrics) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut b = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let _r = if old {
                        before_empty_selector::compare(f, &mut b, actual, &expected)
                    } else {
                        compare(f, &mut b, actual, &expected)
                    };
                }));
                assert!(panic.is_err());
                drop(panic);
            })
        })
        .0;
        balanced(metrics);
        frontiers::finish();
        let (rows, lost) = pair_scope_census::finish();
        assert_eq!(lost, 0);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].ended && rows[0].unwound && !rows[0].complete);
        assert!(rows[0].identity_observations > 0);
        assert_eq!(rows[0].empty_selectors > 0, !old);
    }
}
#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn lexical_empty_selector_capture_reports_new_bool_separately_from_retained_probe_and_old_empty() {
    use crate::workflow_profile::plan_comparison::{Binding, Capture, Site};
    use sha2::{Digest, Sha256};
    let p = super::super::super::super::tests::program();
    let f = &p.functions[0];
    for length in [1, 4] {
        let capture = Capture::begin(Binding::caller_labelled(
            "905 exact certified suffix",
            Sha256::digest(include_bytes!("empty_selector_tests.rs")).into(),
        ))
        .unwrap();
        let r = full(f, false, length, 0, true, false);
        let report = capture.finish().unwrap();
        assert!(
            r.row.complete && report.counts_complete && report.detail_complete && !report.overflow
        );
        for (site, count, price) in [
            (
                Site::EmptySelectorAdmitted,
                r.row.empty_selectors,
                certified_heads::EMPTY_SELECT,
            ),
            (Site::EmptySelectorSkipped, r.row.empty_selector_skips, 0),
            (
                Site::IdentityProbeAdmitted,
                r.row.identity_probes,
                shared_prefix::PROBE,
            ),
            (Site::IdentityEmptyProbe, 0, 0),
        ] {
            assert_eq!(report.counts.at(site), count as u64);
            assert_eq!(
                report.counts.admitted_bytes[site as usize],
                (count * price) as u64
            );
        }
        if length == 1 {
            assert!(r.row.empty_selector_skips > 0);
            assert_eq!(r.row.shared_runs, 0);
        } else {
            assert!(r.row.shared_publishes > 0);
        }
    }
}
#[test]
fn lexical_empty_selector_whole_immediate_caller_four_macros_and_actual_site_expansion_stay_exact()
{
    use sha2::{Digest, Sha256};
    let caller = before_empty_selector::whole_source();
    let sha = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(&Sha256::digest(bytes)));
    // Filled from immutable Git7760, not from a projected/truncated old body.
    assert_eq!(
        (caller.len(), sha(caller.as_bytes())),
        (
            30368,
            "edcc5677574fb9255af2fd3fc1d9e48a5b017baa0ce0bae124bae6e0839bdfa1".to_owned()
        )
    );
    for (name, body, length, digest) in [
        (
            "lexical_cursor",
            include_str!("lexical_cursor_before_empty.rs"),
            6956,
            "c63b8f98af8b1fa55f7e305819efc87107477db4a92f57743365544885b4973c",
        ),
        (
            "paired_value",
            include_str!("paired_value_before_empty.rs"),
            15748,
            "0e289307a841d6ae175957fc9d0f510dc08ccf85959125748cf027b65a9b0de3",
        ),
        (
            "shared_prefix",
            include_str!("shared_prefix_before_empty.rs"),
            7420,
            "faf2391a83182d4b60d1be8561c404ac7af427fcd1f59ff2bfa45c85058244d0",
        ),
        (
            "certified_heads",
            include_str!("certified_heads_before_empty.rs"),
            33327,
            "a8f68ec06d4a418a6a4fd4b0a19ea75a411357899a32a66dcf7e5d0d46097460",
        ),
    ] {
        assert_eq!(
            (body.len(), sha(body.as_bytes())),
            (length, digest.to_owned()),
            "entire actual {name}"
        );
        let mut eof = body.to_owned();
        eof.push('\n');
        assert_ne!(sha(eof.as_bytes()), digest);
        let mut outside = body.as_bytes().to_vec();
        outside[0] ^= 1;
        assert_ne!(sha(&outside), digest);
    }
    let old = include_str!("certified_heads_before_empty.rs");
    let current = include_str!("certified_heads.rs");
    let marker = "macro_rules! certified_boundary {";
    let current_boundary = current.split_once(marker).unwrap().1;
    let anchor = "                                    $right_links = right_links_rest;\n                                    $owner.right_loaded = true;";
    let before = "                                    $right_links = right_links_rest;\n                                    $owner.right_items = rest;\n                                    $owner.right_loaded = true;";
    assert_eq!(current_boundary.matches(anchor).count(), 1);
    let restored_boundary = current_boundary.replacen(anchor, before, 1);
    assert_eq!(
        old.split_once(marker).unwrap().1,
        restored_boundary,
        "entire boundary inverse differs by exactly the deleted real right-rest assignment; all seven work calls/state effects otherwise exact"
    );
    assert!(!current_boundary.contains("$owner.right_items = rest;"));
    let right_rest = size_of::<&[Rc<SkeletonObservation>]>();
    let body_before = 2 * size_of::<&[usize]>()
        + 8 * size_of::<usize>()
        + 4 * size_of::<bool>()
        + 2 * right_rest
        + size_of::<Ordering>();
    assert_eq!(certified_heads::BOUNDARY_BODY + right_rest, body_before);
    // Both conservative remembered slots still occur in the eight scalar slots.
    assert!(include_str!("certified_heads.rs").contains("+ size_of::<[usize; 8]>()"));
    let root = include_str!("../comparison_owner.rs");
    assert_eq!(
        root.matches("certified_suffix!(").count(),
        1,
        "single actual lexical expansion receives both certificate exits"
    );
    let suffix = current
        .split_once("macro_rules! certified_suffix {")
        .unwrap()
        .1
        .split_once("macro_rules! certified_boundary {")
        .unwrap()
        .0;
    let old_suffix = old
        .split_once("macro_rules! certified_suffix {")
        .unwrap()
        .1
        .split_once("macro_rules! certified_boundary {")
        .unwrap()
        .0;
    assert_eq!(
        suffix.matches("paid!(").count(),
        old_suffix.matches("paid!(").count() + 1
    );
    assert_eq!(suffix.matches(".charge(").count(), 5);
    assert_eq!(
        size_of::<ComparisonOwner<'static>>(),
        size_of::<before_empty_selector::ComparisonOwner<'static>>()
    );
    assert_eq!(certified_heads::OWNER, 23 * PAYMENT_HEADER);
    assert_eq!(certified_heads::EMPTY_SELECT, size_of::<bool>());
    assert_eq!(
        shared_prefix::PROBE,
        3 * size_of::<&[Rc<SkeletonObservation>]>()
    );
}
