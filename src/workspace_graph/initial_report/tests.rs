use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};

#[test]
fn actual_initial_attempt_then_typed_policy_removes_only_real_report_allocation() {
    let observe = |current| {
        with_limit_usage_refusal(0, || {
            measure(|| {
                let scope = diagnostics::invocation_scope();
                let (attempt, overflowed, _, receipt, site) =
                    crate::bounded_output::with_limit_usage_refusal_site(0, || {
                        constructor_failure::attempt::run(
                            active_builder_limit(),
                            || -> Result<(), CoreFailure> {
                                panic!("unacquired owner must not run core")
                            },
                        )
                    });
                assert!(overflowed && receipt.is_some());
                let (result, paid) = attempt.into_parts();
                assert!(result.is_err() && paid.is_none());
                drop(result);
                let before = current_metrics().allocations;
                if current {
                    let failure = <WorkspaceReportFailure as Policy>::terminal(
                        active_builder_limit(),
                        paid,
                        receipt,
                        site,
                    );
                    assert!(failure.reported().is_none() && !failure.has_paid_report());
                    assert_eq!(current_metrics().allocations, before);
                    drop(failure);
                } else {
                    let failure = before::terminal(paid, receipt, site);
                    assert_eq!(failure.len(), 1);
                    assert_eq!(failure[0].code, "SPX-G171");
                    assert!(current_metrics().allocations > before);
                    drop(failure);
                }
                drop(scope);
            })
        })
    };
    let old = observe(false);
    let current = observe(true);
    assert_eq!((old.1, old.2, old.3), (current.1, current.2, current.3));
    for result in [&old, &current] {
        let m = &result.0 .1;
        assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
    }
    // Includes the unchanged existing child Budget bootstrap and every Drop;
    // only the exact report capture interval above asserts zero allocation.
    assert!(current.0 .1.allocated < old.0 .1.allocated);
}

#[test]
fn actual_mapper_contexts_keep_all_fields_and_original_empty_comparison_asymmetry() {
    for form in 0..3 {
        for limit in [0, 1, 31, 63, 127, 255, 511, 2047] {
            let old = with_limit_usage_refusal(limit, || {
                let scope = diagnostics::invocation_scope();
                let diagnostic = limit_error("builder_bytes", 1024);
                drop(scope);
                match form {
                    0 => before_mappers::change::map_change_builder_limit(vec![diagnostic], 1024),
                    1 => before_mappers::structural::map_candidate_builder_limit(
                        vec![diagnostic],
                        1024,
                    ),
                    _ => before_mappers::operations::map_candidate_diagnostics(
                        vec![diagnostic],
                        1024,
                        2048,
                    ),
                }
            });
            let current = with_limit_usage_refusal(limit, || {
                let scope = diagnostics::invocation_scope();
                let mut captured = diagnostics::initial::Captured::limit(1024);
                drop(scope);
                match form {
                    0 => captured
                        .map_change(1024, crate::semantic_workspace::MAX_CHANGE_BUILDER_BYTES),
                    1 => captured.map_structural(
                        1024,
                        before_mappers::structural::MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
                        before_mappers::structural::MAX_ANALYSIS_BUILDER_BYTES,
                    ),
                    _ => captured.map_operations(
                        1024,
                        before_mappers::operations::MAX_CANDIDATE_GRAPH_BUILDER_BYTES,
                        before_mappers::operations::MAX_OPERATIONS_BUILDER_BYTES,
                    ),
                }
                vec![captured.materialize_unfunded()]
            });
            assert_eq!(old, current, "form={form} limit={limit}");
        }
    }
}

fn sources() -> Vec<WorkspaceSource> {
    [("lib.spx","module library;\n@id(\"library.answer\") fn answer()->i64 {41}\n"),("app.spx","module app;\nuse function @id(\"library.answer\") from library as answer;\n@id(\"app.main\") fn main()->i64 {answer()+1}\n")]
        .into_iter().map(|(path,text)|WorkspaceSource{path:path.to_owned(),source:crate::format::canonical(&crate::parse(text,Path::new(path)).unwrap())}).collect()
}
#[test]
fn complete_independent_old_builder_and_real_typed_entry_keep_success_and_failures() {
    for limit in [0, 1, 127, 1023, 8191, MAX_BUILDER_BYTES] {
        let previous = ACTIVE_BUILDER_LIMIT.with(|v| v.replace(limit));
        let old = before::build_owned_inner(sources(), None, false, None);
        ACTIVE_BUILDER_LIMIT.with(|v| v.set(previous));
        let current = build_owned_retaining_sources_with_frontend_limit::<WorkspaceReportFailure>(
            sources(),
            limit,
            None,
            false,
            None,
        )
        .map_err(WorkspaceReportFailure::into_legacy_unfunded);
        match (old, current) {
            (Ok((old, _)), Ok((current, _))) => assert_eq!(
                old.source_graph_schemas().unwrap(),
                current.source_graph_schemas().unwrap()
            ),
            (Err(old), Err(current)) => assert_eq!(old, current),
            _ => panic!("old/current actual builder success differs at {limit}"),
        }
    }
    let mut duplicate = sources();
    duplicate[1].path = duplicate[0].path.clone();
    let old = match before::build_owned_inner(duplicate.clone(), None, false, None) {
        Err(error) => error,
        Ok(_) => panic!("duplicate source must fail"),
    };
    let current = match build_owned_retaining_sources_with_frontend_limit::<WorkspaceReportFailure>(
        duplicate,
        MAX_BUILDER_BYTES,
        None,
        false,
        None,
    ) {
        Err(error) => error,
        Ok(_) => panic!("duplicate source must fail"),
    };
    assert_eq!(old, current.into_legacy_unfunded());
}

#[test]
fn later_legacy_many_and_unreceived_owners_are_never_reclassified_as_paid() {
    for many in [false, true] {
        let (((), physical), refused, _, receipt) = with_limit_usage_refusal(65536, || {
            measure(|| {
                let owner = constructor_failure::receiving::Receiver::acquire().unwrap();
                let primary = Diagnostic::error(
                    "SPX-G172",
                    "primary é\0",
                    crate::ast::Span {
                        start: 3,
                        end: 8,
                        line: 2,
                        column: 4,
                    },
                )
                .at_path("provider/é.spx")
                .with_help("original help\0");
                let primary = if many {
                    constructor_failure::Failure::Many(vec![
                        primary,
                        Diagnostic::io("SPX-G173", "second dynamic error").with_help("second help"),
                    ])
                } else {
                    constructor_failure::Failure::One(primary)
                };
                let (unreceived, foreign_refused, foreign_used, foreign_first) =
                    with_limit_usage_refusal(0, || owner.receive(primary));
                assert!(!foreign_refused && foreign_used == 0 && foreign_first.is_none());
                assert!(matches!(&unreceived, CoreFailure::Unreceived { .. }));
                let before = current_metrics().allocations;
                let failure = <WorkspaceReportFailure as Policy>::core(Core(unreceived));
                assert_eq!(current_metrics().allocations, before);
                assert!(!failure.has_paid_report());
                let report = failure.reported().unwrap();
                assert_eq!(report.len(), if many { 2 } else { 1 });
                assert_eq!(report[0].code, "SPX-G172");
                assert_eq!(report[0].severity, crate::diagnostic::Severity::Error);
                assert_eq!(report[0].message, "primary é\0");
                assert_eq!(report[0].path.as_deref(), Some("provider/é.spx"));
                assert_eq!(
                    report[0].span,
                    Some(crate::ast::Span {
                        start: 3,
                        end: 8,
                        line: 2,
                        column: 4
                    })
                );
                assert_eq!(report[0].help.as_deref(), Some("original help\0"));
                if many {
                    assert_eq!(report[1].code, "SPX-G173");
                    assert_eq!(report[1].message, "second dynamic error");
                    assert_eq!(report[1].help.as_deref(), Some("second help"));
                }
                let failure = match failure.into_paid_report() {
                    Err(same) => same,
                    Ok(_) => panic!("unreceived primary is not a paid report"),
                };
                drop(failure);
            })
        });
        assert!(refused && receipt.is_some());
        assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    }
    let diagnostics = vec![
        Diagnostic::io("SPX-G172", "first"),
        Diagnostic::io("SPX-G173", "second"),
    ];
    let pointer = diagnostics.as_ptr();
    let failure = <WorkspaceReportFailure as Policy>::core(Core(CoreFailure::Legacy(diagnostics)));
    assert_eq!(failure.reported().unwrap().as_ptr(), pointer);
    assert!(!failure.has_paid_report());
    let restored = failure.into_legacy_unfunded();
    assert_eq!(restored.as_ptr(), pointer);
    assert_eq!(restored.len(), 2);
}

#[test]
fn complete_original_builder_mappers_and_all_current_owners_remain_authenticated() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = root.join("docs/evidence/initial-report-policy-845-20261015");
    let rows: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(packet.join("current-to-2bfd.json")).unwrap(),
    )
    .unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["source"].as_str().unwrap();
        let actual = std::fs::read_to_string(root.join(path)).unwrap();
        let mut actual = crate::source_reflows_merge::initial845_input(path, &actual);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(actual.as_bytes()))
            ),
            row["current_sha256"]
        );
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(actual.matches(new).count(), 1);
            actual = actual.replacen(new, edit["old"].as_str().unwrap(), 1);
        }
        assert_eq!(
            actual,
            std::fs::read_to_string(root.join(row["archive"].as_str().unwrap())).unwrap()
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(actual.as_bytes()))
            ),
            row["original_sha256"]
        );
    }
    let owners: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(packet.join("new-owners.json")).unwrap())
            .unwrap();
    for row in owners.as_array().unwrap() {
        let path = row["source"].as_str().unwrap();
        let actual = std::fs::read_to_string(root.join(path)).unwrap();
        let actual = crate::source_reflows_merge::initial845_input(path, &actual);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(actual.as_bytes()))
            ),
            row["sha256"]
        );
    }
    let old = std::fs::read_to_string(packet.join("before/src/workspace_graph.rs")).unwrap();
    let start = old.find("fn build_owned_inner(").unwrap();
    let end = old[start..].find("\ntype ResolvedCore").unwrap() + start;
    let body = old[start..end].replace("fn build_owned_inner(", "pub(super) fn build_owned_inner(");
    let complete_before = crate::source_reflows_merge::initial845_input(
        "src/workspace_graph/initial_report/before.rs", include_str!("before.rs"),
    );
    assert!(complete_before.contains(&body));
    let legacy = std::fs::read_to_string(packet.join("before/src/bounded_output.rs")).unwrap();
    let current = crate::source_reflows_merge::initial845_input(
        "src/bounded_output.rs",
        include_str!("../../bounded_output.rs"),
    );
    assert_eq!(
        current
            .strip_prefix("pub(crate) mod format_acceptance;\n")
            .unwrap(),
        legacy
    );
}

#[cfg(feature = "unstable-workflow-profiling")]
#[test]
fn exclusive_core_capture_keeps_complete_real_builder_outputs_and_every_original_limit() {
    use crate::workflow_profile::plan_comparison::{Binding, Capture, CoreDebitAlgorithm};
    for limit in [0, 1, 127, 1023, 8191, MAX_BUILDER_BYTES] {
        let run = || {
            build_owned_retaining_sources_with_frontend_limit::<Vec<Diagnostic>>(
                sources(),
                limit,
                None,
                false,
                None,
            )
            .map(|(build, _)| build.source_graph_schemas().unwrap())
        };
        let plain = run();
        let capture = Capture::begin(Binding::caller_labelled(
            "real initial builder parity",
            [0; 32],
        ))
        .unwrap();
        let watched = run();
        let report = capture.finish().unwrap();
        assert_eq!(
            plain, watched,
            "actual full compiler product at unchanged limit {limit}"
        );
        assert_eq!(
            report.resolved_core.algorithm,
            CoreDebitAlgorithm::InstrumentedLibraryTest
        );
        assert!(
            report.resolved_core.complete
                && !report.resolved_core.invalid
                && !report.resolved_core.lost
                && !report.resolved_core.overflow,
            "{report:?}"
        );
        for attempt in &report.resolved_core.attempts[..report.resolved_core.stored_attempts] {
            assert!(attempt.complete && !attempt.invalid);
            assert_eq!(attempt.stage_sum(), Some(attempt.consumed));
            assert_eq!(attempt.operation_sum(), Some(attempt.consumed));
        }
        if limit == MAX_BUILDER_BYTES {
            assert!(watched.is_ok());
            assert!(report.resolved_core.entered_attempts > 0);
            assert_eq!(
                report.resolved_core.final_attempt_ordinal,
                Some(report.resolved_core.entered_attempts - 1)
            );
        }
    }
}
