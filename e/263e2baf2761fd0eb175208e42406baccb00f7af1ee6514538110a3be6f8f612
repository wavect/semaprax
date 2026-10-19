//! Complete immediate PLAN/HIR comparison, not an arena identity certificate.
use super::*;
fn run(
    f: &ResolvedFunction,
    old: bool,
    length: usize,
    shape: usize,
    shared: bool,
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
        true,
        fault,
        if old { 9 } else { 0 },
        work,
        bytes,
        floor,
        shared,
        !trace,
        shape,
    )
}
fn complete(f: &ResolvedFunction, old: bool, length: usize, shape: usize, shared: bool) -> Run {
    run(
        f,
        old,
        length,
        shape,
        shared,
        0,
        MAX_REPLAY_WORK_UNITS,
        usize::MAX,
        0,
        false,
    )
}
fn report(old: &Run, current: &Run) {
    assert!(old.row.complete && current.row.complete);
    assert_eq!(old.observed.0.0, current.observed.0.0);
    assert_eq!(swaps(old), swaps(current));
    assert_eq!(
        old.observed.0.1, current.observed.0.1,
        "all heap through full destruction"
    );
    assert_eq!(
        (
            old.row.final_flat,
            old.row.final_shared,
            old.row.final_steps,
            old.row.final_tails
        ),
        (
            current.row.final_flat,
            current.row.final_shared,
            current.row.final_steps,
            current.row.final_tails
        )
    );
    let (added, removed) = final_affine::delta(&current.row);
    assert_eq!(current.observed.2 + removed, old.observed.2 + added);
    eprintln!(
        "OPT907 complete old={} current={} added={} removed={} flat={} shared={} steps={} tails={}; independent content, full heap/Drop, cold losses retained; no app admission claim",
        old.observed.2,
        current.observed.2,
        added,
        removed,
        current.row.final_flat,
        current.row.final_shared,
        current.row.final_steps,
        current.row.final_tails
    );
}
#[test]
fn affine_final_hir_keeps_complete_immediate_both_routes_cold_hot_and_actual_typed_invoices() {
    let program = super::super::super::super::super::tests::program();
    let f = &program.functions[0];
    for n in [0, 1, 4, 64] {
        for shape in 0..=6 {
            for shared in [false, true] {
                report(
                    &complete(f, true, n, shape, shared),
                    &complete(f, false, n, shape, shared),
                );
            }
        }
    }
    for shared in [false, true] {
        let old = complete(f, true, 256, 0, shared);
        let current = complete(f, false, 256, 0, shared);
        report(&old, &current);
        assert!(
            current.observed.2 < old.observed.2,
            "whole dense comparison including all cold costs"
        );
    }
}
#[test]
fn affine_final_hir_keeps_all_original_work_cuts_malformed_six_fields_and_swaps() {
    let program = super::super::super::super::super::tests::program();
    let f = &program.functions[0];
    for (n, shape) in [(0, 0), (4, 3), (4, 6)] {
        for shared in [false, true] {
            let full = complete(f, true, n, shape, shared);
            assert!(full.observed.0.0.result.is_ok());
            let spent = MAX_REPLAY_WORK_UNITS - full.observed.0.0.remaining;
            for fault in 0..4 {
                for cut in 0..=spent {
                    let old = run(f, true, n, shape, shared, fault, cut, usize::MAX, 0, false);
                    let current = run(f, false, n, shape, shared, fault, cut, usize::MAX, 0, false);
                    assert_eq!(
                        old.observed.0.0, current.observed.0.0,
                        "n={n} shape={shape} shared={shared} fault={fault} work={cut}"
                    );
                    assert_eq!(swaps(&old), swaps(&current));
                }
            }
        }
    }
}
#[test]
fn affine_final_hir_actual_banks_keep_exact_short_zero_floor_sticky_and_full_drop() {
    let program = super::super::super::super::super::tests::program();
    let f = &program.functions[0];
    for shared in [false, true] {
        let full = run(
            f,
            false,
            4,
            3,
            shared,
            0,
            MAX_REPLAY_WORK_UNITS,
            usize::MAX,
            0,
            true,
        );
        assert!(full.observed.0.0.result.is_ok() && full.row.complete);
        for phase in [
            "ordinary plan retained comparison owner",
            if shared {
                "ordinary plan affine shared HIR match"
            } else {
                "ordinary plan affine flat HIR match"
            },
            "ordinary plan affine HIR segment",
            "ordinary plan affine HIR tail",
        ] {
            let event = full
                .events
                .iter()
                .find(|e| e.phase == phase && e.after.is_some())
                .unwrap();
            let end = event.after.unwrap();
            for (limit, floor) in [(end - 1, 0), (end + 37, 38)] {
                let short = run(
                    f,
                    false,
                    4,
                    3,
                    shared,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    limit,
                    floor,
                    true,
                );
                assert!(short.observed.1 && short.observed.0.0.result.is_err());
                assert!(short.observed.0.0.retry.is_some());
                assert!(!short.row.complete);
                assert_eq!(
                    short.observed.0.0.result.as_ref().unwrap_err(),
                    &primitive_failure(f, *event, limit, floor)
                );
                let first = short.observed.3.as_ref().unwrap();
                assert_eq!(
                    (first.requested, first.remaining, first.floor),
                    (end - event.before, limit - event.before, floor)
                );
                let exact = run(
                    f,
                    false,
                    4,
                    3,
                    shared,
                    0,
                    MAX_REPLAY_WORK_UNITS,
                    end,
                    0,
                    true,
                );
                if let Some(first) = exact.observed.3 {
                    assert!(first.remaining == 0);
                }
            }
        }
        let zero = run(f, false, 4, 3, shared, 0, MAX_REPLAY_WORK_UNITS, 0, 0, true);
        assert!(zero.observed.1 && zero.observed.0.0.result.is_err() && !zero.row.complete);
        let mixed = run(f, false, 4, 3, shared, 0, 0, 0, 0, true);
        assert!(mixed.observed.1 && mixed.observed.0.0.result.is_err());
        for work in [0, 1, 3, MAX_REPLAY_WORK_UNITS - full.observed.0.0.remaining] {
            for bytes in [0, full.observed.2 / 2, full.observed.2] {
                let mixed = run(f, false, 4, 3, shared, 0, work, bytes, 0, false);
                if mixed.observed.0.0.result.is_err() {
                    assert!(mixed.observed.0.0.retry.is_some());
                    assert!(!mixed.row.complete);
                }
            }
        }
    }
}
#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn affine_final_hir_feature_capture_prices_each_actual_admitted_bank_and_teardown() {
    use crate::workflow_profile::plan_comparison::{Binding, Capture, Site};
    use sha2::{Digest, Sha256};
    let program = super::super::super::super::super::tests::program();
    let f = &program.functions[0];
    for shared in [false, true] {
        let capture = Capture::begin(Binding::caller_labelled(
            "907 full independent content",
            Sha256::digest(include_bytes!("final_affine_tests.rs")).into(),
        ))
        .unwrap();
        let current = complete(f, false, 64, 3, shared);
        let report = capture.finish().unwrap();
        assert!(current.row.complete);
        for (site, count, bytes) in [
            (
                Site::FinalFlatAdmitted,
                current.row.final_flat,
                final_affine::FLAT_MATCH,
            ),
            (
                Site::FinalSharedAdmitted,
                current.row.final_shared,
                final_affine::SHARED_MATCH,
            ),
            (
                Site::FinalStepAdmitted,
                current.row.final_steps,
                final_affine::STEP + lexical_cursor::EMPTY,
            ),
            (
                Site::FinalTailAdmitted,
                current.row.final_tails,
                final_affine::TAIL,
            ),
        ] {
            assert_eq!(report.counts.at(site), count as u64);
            assert_eq!(
                report.counts.admitted_bytes[site as usize],
                (count * bytes) as u64
            );
        }
        assert_eq!(report.counts.started, report.counts.ended);
        assert_eq!(report.counts.unwound, 0);
    }
}

fn altered(f: &ResolvedFunction, old: bool, alteration: usize, shared: bool) -> Run {
    let (mut actual, mut expected) = fixture(4, 3, true, 0);
    match alteration {
        0 => {
            actual.paths.clear();
            expected.clear();
        }
        1 => {
            expected[0].observations.items.clear();
        }
        2 => {
            expected[0].observations.items.push(observation(999, true));
        }
        3 => {
            expected[0].terminal = SkeletonTerminal::Failure;
        }
        4 => {
            expected.pop();
        }
        5 => {
            expected.push(expected[0].clone());
        }
        _ => unreachable!("closed independent content attacks"),
    }
    expected.sort();
    if shared {
        for path in &mut expected {
            path.observations.prefix = path.observations.freeze();
            path.observations.items.clear();
        }
    }
    pair_scope_census::reset();
    frontiers::reset_swaps();
    let observed = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
            let result = (|| {
                paid!(
                    &mut budget,
                    f,
                    COMPARE_CALL_CONTROLS,
                    "907 complete altered comparison"
                )?;
                if old {
                    before_final_affine::compare(f, &mut budget, actual, &expected)
                } else {
                    compare(f, &mut budget, actual, &expected)
                }
            })();
            let outcome = Outcome {
                result: result.as_ref().map(|_| ()).map_err(failure),
                retry: None,
                remaining: budget.remaining,
                skeleton_spent: budget.skeleton_spent,
            };
            drop(result);
            drop(budget);
            outcome
        })
    });
    balanced(observed.0.1);
    let events = frontiers::finish();
    let (rows, lost) = pair_scope_census::finish();
    assert_eq!(lost, 0);
    assert_eq!(rows.len(), 1);
    Run {
        observed,
        events,
        row: rows[0],
    }
}
#[test]
fn affine_final_hir_retains_independent_content_exhaustion_and_terminal_mismatches_and_cold_loss() {
    let program = super::super::super::super::super::tests::program();
    let f = &program.functions[0];
    for shared in [false, true] {
        let old = altered(f, true, 0, shared);
        let current = altered(f, false, 0, shared);
        report(&old, &current);
        assert!(
            current.observed.2 > old.observed.2,
            "real zero-path cold regression is retained"
        );
        for attack in 1..=5 {
            let old = altered(f, true, attack, shared);
            let current = altered(f, false, attack, shared);
            assert!(old.observed.0.0.result.is_err() && current.observed.0.0.result.is_err());
            assert_eq!(
                old.observed.0.0, current.observed.0.0,
                "shared={shared} independent content attack={attack}"
            );
            assert_eq!(swaps(&old), swaps(&current));
            assert!(!old.row.complete && !current.row.complete);
        }
    }
}
#[test]
fn affine_final_hir_keeps_typed_foreign_restored_authority_and_unwind_full_drop() {
    // Existing complete authority owner exercises the actual current compare,
    // valid/missing/foreign Oracle admission and sticky refusal after restore.
    super::super::shared_identity_replay_typed_oracle_missing_foreign_and_restored_scope_keeps_ambient_unspent();
    let program = super::super::super::super::super::tests::program();
    let f = &program.functions[0];
    for old in [false, true] {
        for shared in [false, true] {
            let (actual, mut expected) = fixture(4, 3, true, 0);
            if shared {
                for p in &mut expected {
                    p.observations.prefix = p.observations.freeze();
                    p.observations.items.clear();
                }
            }
            pair_scope_census::reset();
            frontiers::reset_swaps();
            pair_scope_census::unwind_next_final_round();
            let ((unwound, heap), refused, _, receipt) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let outcome =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                let mut budget =
                                    ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                                paid!(
                                    &mut budget,
                                    f,
                                    COMPARE_CALL_CONTROLS,
                                    "907 unwind comparator receiver"
                                )
                                .unwrap();
                                if old {
                                    before_final_affine::compare(f, &mut budget, actual, &expected)
                                        .unwrap();
                                } else {
                                    compare(f, &mut budget, actual, &expected).unwrap();
                                }
                            }));
                        let unwound = outcome.is_err();
                        drop(outcome);
                        unwound
                    })
                });
            assert!(unwound && !refused && receipt.is_none());
            balanced(heap);
            frontiers::finish();
            let (rows, lost) = pair_scope_census::finish();
            assert_eq!(lost, 0);
            assert_eq!(rows.len(), 1);
            assert!(rows[0].ended && rows[0].unwound && !rows[0].complete);
            assert_eq!(rows[0].final_flat + rows[0].final_shared, 1);
        }
    }
}

pub(super) fn whole_before(case: usize, source: &str) -> Result<Vec<u8>, &'static str> {
    use sha2::{Digest, Sha256};
    let table: serde_json::Value =
        serde_json::from_str(include_str!("final_affine_inverse.json")).unwrap();
    let row = &table["cases"][case];
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    if source.len() as u64 != row["current_length"].as_u64().unwrap()
        || digest(source.as_bytes()) != row["current_sha256"].as_str().unwrap()
    {
        return Err("whole current source drift");
    }
    let mut restored = source.as_bytes().to_vec();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let end = edit["end"].as_u64().unwrap() as usize;
        if restored.get(start..end) != Some(edit["new"].as_str().unwrap().as_bytes()) {
            return Err("literal source mismatch");
        }
        restored.splice(
            start..end,
            edit["old"].as_str().unwrap().as_bytes().iter().copied(),
        );
    }
    if restored.len() as u64 != row["before_length"].as_u64().unwrap()
        || digest(&restored) != row["before_sha256"].as_str().unwrap()
    {
        return Err("whole old source mismatch");
    }
    Ok(restored)
}
#[test]
fn affine_final_hir_preserves_entire_immediate_comparator_four_macros_and_all_old_source_guards() {
    let inputs = [
        include_str!("../comparison_owner.rs"),
        include_str!("before_final_affine.rs"),
        include_str!("lexical_cursor_before_final_affine.rs"),
        include_str!("paired_value_before_final_affine.rs"),
        include_str!("shared_prefix_before_final_affine.rs"),
        include_str!("certified_heads_before_final_affine.rs"),
    ];
    for (case, input) in inputs.into_iter().enumerate() {
        let before = whole_before(case, input).unwrap();
        if case <= 1 {
            assert_eq!(before, whole_before(0, inputs[0]).unwrap());
        } else {
            assert_eq!(
                before,
                input.as_bytes(),
                "entire authentic old macro, no rewritten tariff/body"
            );
        }
        assert!(
            whole_before(case, &format!("{input} ")).is_err(),
            "EOF/extent drift"
        );
        assert!(
            whole_before(case, &input[..input.len() - 1]).is_err(),
            "truncation"
        );
        let mut changed = input.as_bytes().to_vec();
        changed[0] ^= 1;
        assert!(
            whole_before(case, std::str::from_utf8(&changed).unwrap()).is_err(),
            "outside any edit"
        );
        if case <= 1 {
            assert!(
                whole_before(case, std::str::from_utf8(&before).unwrap()).is_err(),
                "already old is not current authority"
            );
        }
    }
    assert_eq!(
        include_bytes!("lexical_cursor.rs"),
        include_bytes!("lexical_cursor_before_final_affine.rs")
    );
    assert_eq!(
        include_bytes!("paired_value.rs"),
        include_bytes!("paired_value_before_final_affine.rs")
    );
    assert_eq!(
        include_bytes!("shared_prefix.rs"),
        include_bytes!("shared_prefix_before_final_affine.rs")
    );
    assert_eq!(
        include_bytes!("certified_heads.rs"),
        include_bytes!("certified_heads_before_final_affine.rs")
    );
}
