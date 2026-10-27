//! Ordinary Project build keeps the original authenticated loader and final recheck.
use super::*;
use crate::workspace_report::WorkspaceReportFailure;
use crate::{semantic_workspace, workspace_graph};

/// Same authority and operation order as `with_authenticated_project`, retaining
/// the acquired builder report and its borrowed first reservation location.
pub fn with_authenticated_project_reporting<T>(
    manifest_path: &Path,
    operation: impl FnOnce(&mut ProjectSnapshot) -> Result<T, Vec<Diagnostic>>,
    hints: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>,
) -> Result<T, WorkspaceReportFailure> {
    let (map_failure, outer_mapper_controls) =
        snapshot_building::prepare_mapper::<WorkspaceReportFailure, T, _>(hints);
    #[cfg(test)]
    let builder_limit = TEST_PROJECT_LIMIT.with(std::cell::Cell::get);
    #[cfg(not(test))]
    let builder_limit = workspace_graph::MAX_BUILDER_BYTES;
    let loaded = snapshot_building::load_snapshot_building_with_controls(
        manifest_path,
        ProjectHostAccess::Generic,
        |manifest, sources, loader_mapper_controls| {
            if manifest.project_profile() == ProjectProfile::SourceLocalFutureIndexedRustV1 {
                return Err(vec![Diagnostic::io(
                    "SPX-H006",
                    "source-local-future-indexed-rust.v1 requires authenticated indexed Rust selections",
                )].into());
            }
            let subtotal = controls::<T>().checked_add(outer_mapper_controls);
            let controls = match subtotal {
                Some(total) => match total.checked_add(loader_mapper_controls) {
                    Some(complete) => complete,
                    None => usize::MAX,
                },
                None => usize::MAX,
            };
            let built = build::build_owned_reporting(&manifest, sources, controls, builder_limit)?;
            Ok::<_, WorkspaceReportFailure>((
                Arc::new(ProjectRevision::from_built(manifest, built)),
                (),
            ))
        },
    );
    // A loaded snapshot has no refused builder owner. The original operation
    // retains its exact primary/drift concatenation and final held-file recheck.
    // Apply the exact CLI mapper once to BOTH loader and operation failures.
    let result = match loaded {
        Ok((snapshot, ())) => with_snapshot_operation(snapshot, operation).map_err(Into::into),
        Err(failure) => Err(failure),
    };
    result.map_err(map_failure)
}

// Whole new error/result and sidecar transport banks, acquired by the actual
// report Owner's ORIGINAL first positive control gate, before the core runs.
// Existing Project/source/output allocations remain their original obligations.
fn controls<T>() -> usize {
    use std::mem::size_of;
    const {
        let rows = [
            size_of::<Result<T, WorkspaceReportFailure>>(), // whole outer return
            size_of::<Result<T, WorkspaceReportFailure>>(), // result match receiver local
            // Shared preflight call's receiving result and the forwarding return.
            size_of::<
                Result<
                    (
                        workspace_graph::WorkspaceGraphBuild,
                        workspace_graph::current_source::SourceFacts,
                    ),
                    WorkspaceReportFailure,
                >,
            >(),
            size_of::<
                Result<
                    (
                        workspace_graph::WorkspaceGraphBuild,
                        workspace_graph::current_source::SourceFacts,
                    ),
                    WorkspaceReportFailure,
                >,
            >(),
            size_of::<Result<build::BuiltProject, WorkspaceReportFailure>>(),
            size_of::<Result<semantic_workspace::SemanticWorkspacePreflight, WorkspaceReportFailure>>(
            ),
            size_of::<Result<(Arc<ProjectRevision>, ()), WorkspaceReportFailure>>(),
            size_of::<Result<(ProjectSnapshot, ()), WorkspaceReportFailure>>(),
            // report_controls moves at each actual forwarding boundary below.
            2 * size_of::<usize>(), // builder_limit: reporting -> build_owned_reporting
            2 * size_of::<usize>(), // builder_limit: build -> preflight_owned_project
            2 * size_of::<usize>(), // builder_limit: actual/formal Some initializer
            size_of::<Option<usize>>(), // actual finite-limit argument into typed preflight
            size_of::<Option<usize>>(), // formal finite-limit argument in typed preflight
            2 * size_of::<usize>(), // reporting -> build_owned_reporting
            2 * size_of::<usize>(), // build_owned_reporting -> preflight_owned_project
            2 * size_of::<usize>(), // preflight_owned_project -> typed preflight
            2 * size_of::<usize>(), // typed preflight -> retaining-source builder
            2 * size_of::<usize>(), // retaining-source builder -> initial report builder
            2 * size_of::<usize>(), // initial report builder -> run_with_report_controls
            size_of::<usize>(),     // checked_add actual controls receiver
            size_of::<usize>(),     // checked_add formal controls receiver
            size_of::<usize>(),     // outer_mapper_controls actual operand
            size_of::<usize>(),     // outer_mapper_controls formal operand
            size_of::<Option<usize>>(), // first checked_add return
            size_of::<Option<usize>>(), // first match receiver
            size_of::<usize>(),     // first match bound total
            size_of::<usize>(),     // selected total checked_add receiver
            size_of::<usize>(),     // second checked_add formal receiver
            size_of::<usize>(),     // loader_mapper_controls actual operand
            size_of::<usize>(),     // loader_mapper_controls formal operand
            size_of::<Option<usize>>(), // second checked_add return
            size_of::<Option<usize>>(), // second match receiver
            size_of::<usize>(),     // selected complete bank local
        ];
        let mut total = 0usize;
        let mut index = 0;
        while index < rows.len() {
            total = match total.checked_add(rows[index]) {
                Some(total) => total,
                None => {
                    total = usize::MAX;
                    break;
                }
            };
            index += 1;
        }
        total
    }
}

#[cfg(test)]
thread_local! {
    // Owning tests use the ORIGINAL private finite builder-limit route, never a
    // forced refusal/fallback bit. Production always supplies the unchanged max.
    static TEST_PROJECT_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(workspace_graph::MAX_BUILDER_BYTES) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct RestoreLimit(usize);
    impl Drop for RestoreLimit {
        fn drop(&mut self) {
            TEST_PROJECT_LIMIT.with(|limit| limit.set(self.0));
        }
    }

    #[test]
    fn real_project_refusal_keeps_paid_report_through_both_hint_mappers_and_writer() {
        let root = super::super::tests::fixture();
        let manifest = root.join(MANIFEST_FILE);
        // Real admitted scalar calls, parsed and canonicalized by the ordinary
        // compiler. No manually invalid certificate, stopped bit or fake receipt.
        let mut source = String::from("module calculator.app;\nuse function @id(\"calculator.add\") from calculator.core as add;\n@id(\"calculator.app.main\") fn main() -> i64 {\n");
        use std::fmt::Write as _;
        for index in 0..96 {
            writeln!(&mut source, "let value{index} = add({index}, 1);").unwrap();
        }
        source.push_str("add(19, 23)\n}\n");
        let canonical =
            crate::format::canonical(&crate::parse(&source, Path::new("z/app.spx")).unwrap());
        std::fs::write(root.join("z/app.spx"), canonical).unwrap();
        let old =
            TEST_PROJECT_LIMIT.with(|limit| limit.replace(workspace_graph::MAX_BUILDER_BYTES));
        let _restore = RestoreLimit(old);
        // Establish that the whole real source route is admitted at the unchanged max.
        with_authenticated_project_reporting(
            &manifest,
            |snapshot| snapshot.check(),
            |report| report,
        )
        .unwrap();
        let mut found_paid = false;
        for shift in (0..=12).rev() {
            let limit = workspace_graph::MAX_BUILDER_BYTES >> shift;
            TEST_PROJECT_LIMIT.with(|slot| slot.set(limit));
            let hints = Cell::new(0usize);
            let original_pointer = Cell::new(0usize);
            let mut output = String::with_capacity(8192); // Caller-owned output, before refusal.
            let ((), allocation) = crate::test_allocation_observer::measure(|| {
                let result = with_authenticated_project_reporting(
                    &manifest,
                    |snapshot| snapshot.check(),
                    |report| {
                        hints.set(hints.get() + 1);
                        original_pointer.set(report.as_ptr() as usize);
                        report
                    },
                );
                match result {
                    Ok(()) => assert_eq!(hints.get(), 0),
                    Err(failure) => {
                        if failure.reported().is_some() {
                            assert_eq!(
                                hints.get(),
                                1,
                                "one outer hint after the unchanged loader hints"
                            );
                        } else {
                            assert_eq!(hints.get(), 0, "original inline Initial owner bypasses allocating legacy hint mappers");
                        }
                        let before = crate::test_allocation_observer::current_metrics().allocated;
                        failure.write_lines(&mut output, true).unwrap();
                        assert_eq!(
                            crate::test_allocation_observer::current_metrics().allocated,
                            before
                        );
                        if output.contains(" Reservation caller: ") && failure.reported().is_some()
                        {
                            let report = failure.reported().unwrap();
                            assert_eq!(report.as_ptr() as usize, original_pointer.get());
                            assert_eq!(report.len(), 1);
                            assert_eq!(report[0].code, "SPX-G171");
                            assert_eq!(report[0].severity, crate::diagnostic::Severity::Error);
                            assert!(report[0].path.is_none() && report[0].span.is_none());
                            assert!(report[0]
                                .help
                                .as_ref()
                                .unwrap()
                                .contains("first sticky ledger refusal"));
                            assert!(!report[0]
                                .help
                                .as_ref()
                                .unwrap()
                                .contains("Reservation caller:"));
                            found_paid = true;
                        }
                        drop(failure);
                    }
                }
            });
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if found_paid {
                break;
            }
        }
        assert!(found_paid, "finite original-limit sweep must observe an actual acquired Project report, not only an inline fallback");
        std::fs::remove_dir_all(root).unwrap();
    }
}
