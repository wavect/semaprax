//! Behavioral evidence through the public replacement API. Worker identity,
//! concurrent-operation admission, and terminal handoff faults live beside the
//! worker implementation; every physical worker test shares the parent lock.

use super::*;
use crate::project::{ProjectExecutionOptions, ProjectExecutionOutcome, ProjectExecutionRole};
use std::path::PathBuf;

struct Candidate {
    root: PathBuf,
    revision: Arc<ProjectRevision>,
}

impl Candidate {
    fn new(automatic_test_helper: bool) -> Self {
        static SERIAL: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "semaprax-prepared-replacement-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for relative in ["semaprax.toml", "src/core.spx"] {
            std::fs::write(
                root.join(relative),
                std::fs::read(original.join(relative)).unwrap(),
            )
            .unwrap();
        }
        let app = r#"module calculator.app;
use function @id("calculator.add") from calculator.core as add;
@id("calculator.app.main")
fn main() -> i64 { add(47, 1) }
"#;
        let tests = if automatic_test_helper {
            r#"module calculator.tests;
fn helper() -> i64 { 1 }
@id("calculator.tests.main")
fn main() -> i64 { helper() }
"#
        } else {
            r#"module calculator.tests;
use function @id("calculator.add") from calculator.core as add;
@id("calculator.tests.main")
fn main() -> i64 { add(1, 2) }
"#
        };
        for (relative, source) in [("src/app.spx", app), ("src/tests.spx", tests)] {
            let canonical =
                crate::format::canonical(&crate::parse(source, Path::new(relative)).unwrap());
            assert_eq!(
                crate::format::canonical(&crate::parse(&canonical, Path::new(relative)).unwrap()),
                canonical
            );
            std::fs::write(root.join(relative), canonical).unwrap();
        }
        let revision = crate::project::load_snapshot(&root.join("semaprax.toml"))
            .expect("candidate must pass real complete Project admission")
            .retain_revision();
        Self { root, revision }
    }

    fn reopen(&self) -> Arc<ProjectRevision> {
        crate::project::load_snapshot(&self.root.join("semaprax.toml"))
            .unwrap()
            .retain_revision()
    }

    // Preserve failed fixtures for diagnosis. Validate the complete fixed
    // inventory before deleting anything; never recurse through a replacement.
    fn cleanup(self) {
        assert_plain(&self.root, true);
        assert_plain(&self.root.join("src"), true);
        assert_inventory(&self.root, &["semaprax.toml", "src"]);
        assert_inventory(
            &self.root.join("src"),
            &["app.spx", "core.spx", "tests.spx"],
        );
        let files = [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ];
        for relative in files {
            assert_plain(&self.root.join(relative), false);
        }
        for relative in files {
            std::fs::remove_file(self.root.join(relative)).unwrap();
        }
        std::fs::remove_dir(self.root.join("src")).unwrap();
        std::fs::remove_dir(&self.root).unwrap();
    }
}

fn assert_plain(path: &Path, directory: bool) {
    let metadata = std::fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(metadata.file_attributes() & 0x400, 0, "reparse point");
    }
    assert_eq!(metadata.is_dir(), directory);
    assert_eq!(metadata.is_file(), !directory);
}

fn assert_inventory(directory: &Path, expected: &[&str]) {
    let mut actual = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(actual, expected);
}

fn pair(
    prepared: &PreparedProjectInterpreter,
    options: &PreparedProjectExecutionOptions,
) -> [PreparedProjectExecution; 2] {
    let cancellation = ProjectExecutionCancellation::new();
    [
        prepared.execute_entry(options, &cancellation).unwrap(),
        prepared.execute_test(options, &cancellation).unwrap(),
    ]
}

fn assert_exact_pair(left: &[PreparedProjectExecution; 2], right: &[PreparedProjectExecution; 2]) {
    for (left, right) in left.iter().zip(right) {
        assert_eq!(left, right);
        assert_eq!(
            left.trace().envelope().as_bytes(),
            right.trace().envelope().as_bytes()
        );
    }
}

fn assert_legacy_parity(revision: &ProjectRevision, executions: &[PreparedProjectExecution; 2]) {
    for (role, prepared) in [ProjectExecutionRole::Entry, ProjectExecutionRole::Test]
        .into_iter()
        .zip(executions)
    {
        let options = ProjectExecutionOptions {
            max_steps: prepared.max_steps(),
            ..ProjectExecutionOptions::default()
        };
        let legacy = revision.execute(role, &options).unwrap();
        assert_eq!(legacy.steps_used(), prepared.steps_used());
        let expected = match legacy.outcome() {
            ProjectExecutionOutcome::Returned(value) => {
                ProjectPreparedExecutionOutcome::Returned(*value)
            }
            ProjectExecutionOutcome::LanguageFailure(status) => {
                ProjectPreparedExecutionOutcome::LanguageFailure(status.clone())
            }
            ProjectExecutionOutcome::FuelExhausted => {
                ProjectPreparedExecutionOutcome::FuelExhausted
            }
            ProjectExecutionOutcome::CallDepthExceeded => {
                ProjectPreparedExecutionOutcome::CallDepthExceeded
            }
        };
        assert_eq!(prepared.outcome(), &expected);
        verify_project_source_trace_against_revision(revision, prepared.trace().envelope())
            .unwrap();
    }
}

#[test]
fn replacement_switches_both_closures_preserves_old_revision_and_binds_traces() {
    let _serial = real_prepare_serial();
    let old = revision();
    let candidate = Candidate::new(false);
    assert_ne!(
        old.project_revision(),
        candidate.revision.project_revision()
    );
    let old_graph = old.semantic_graph().to_owned();
    let old_entry = old
        .execute_entry(&ProjectExecutionOptions::default())
        .unwrap();
    let old_test = old
        .execute_test(&ProjectExecutionOptions::default())
        .unwrap();
    let prepared = old
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let options = PreparedProjectExecutionOptions::default();
    let before = pair(&prepared, &options);
    assert_legacy_parity(&old, &before);
    assert_eq!(
        before[0].outcome(),
        &ProjectPreparedExecutionOutcome::Returned(42)
    );
    assert_eq!(
        before[1].outcome(),
        &ProjectPreparedExecutionOutcome::Returned(0)
    );

    prepared
        .replace_revision(old.project_revision(), Arc::clone(&candidate.revision))
        .unwrap();
    let after = pair(&prepared, &options);
    assert_eq!(
        after[0].outcome(),
        &ProjectPreparedExecutionOutcome::Returned(48)
    );
    assert_eq!(
        after[1].outcome(),
        &ProjectPreparedExecutionOutcome::Returned(3)
    );
    assert_legacy_parity(&candidate.revision, &after);
    assert_exact_pair(&after, &pair(&prepared, &options));
    assert_eq!(old.semantic_graph(), old_graph);
    assert_eq!(
        old.execute_entry(&ProjectExecutionOptions::default())
            .unwrap(),
        old_entry
    );
    assert_eq!(
        old.execute_test(&ProjectExecutionOptions::default())
            .unwrap(),
        old_test
    );

    for (old_trace, new_trace) in before.iter().zip(&after) {
        for (wrong_revision, trace) in [(&candidate.revision, old_trace), (&old, new_trace)] {
            verify_project_source_trace(trace.trace().envelope()).unwrap();
            assert_eq!(
                verify_project_source_trace_against_revision(
                    wrong_revision,
                    trace.trace().envelope()
                )
                .unwrap_err()
                .code,
                "SPX-F110"
            );
        }
        let rebound = remint_payload(old_trace.trace().envelope(), |payload| {
            assert!(payload.contains(old.project_revision()));
            payload.replacen(
                old.project_revision(),
                candidate.revision.project_revision(),
                1,
            )
        });
        verify_project_source_trace(&rebound).unwrap();
        assert_eq!(
            verify_project_source_trace_against_revision(&candidate.revision, &rebound)
                .unwrap_err()
                .code,
            "SPX-F110"
        );
    }
    drop(prepared);
    candidate.cleanup();
}

#[test]
fn replacement_repeats_same_candidate_and_uses_content_not_epoch_tokens() {
    let _serial = real_prepare_serial();
    let old = revision();
    let candidate = Candidate::new(false);
    let prepared = old
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let options = PreparedProjectExecutionOptions::default();
    let before = pair(&prepared, &options);
    prepared
        .replace_revision(old.project_revision(), Arc::clone(&old))
        .unwrap();
    assert_exact_pair(&before, &pair(&prepared, &options));
    for _ in 0..3 {
        prepared
            .replace_revision(old.project_revision(), Arc::clone(&candidate.revision))
            .unwrap();
        let after = pair(&prepared, &options);
        assert_eq!(
            prepared
                .replace_revision(old.project_revision(), Arc::clone(&old))
                .unwrap_err()[0]
                .code,
            "SPX-F108"
        );
        assert_exact_pair(&after, &pair(&prepared, &options));
        prepared
            .replace_revision(
                candidate.revision.project_revision(),
                Arc::clone(&candidate.revision),
            )
            .unwrap();
        assert_exact_pair(&after, &pair(&prepared, &options));
        let reopened = candidate.reopen();
        assert!(!Arc::ptr_eq(&reopened, &candidate.revision));
        assert_eq!(
            reopened.project_revision(),
            candidate.revision.project_revision()
        );
        prepared
            .replace_revision(candidate.revision.project_revision(), reopened)
            .unwrap();
        assert_exact_pair(&after, &pair(&prepared, &options));
        prepared
            .replace_revision(candidate.revision.project_revision(), Arc::clone(&old))
            .unwrap();
        assert_exact_pair(&before, &pair(&prepared, &options));
    }
    drop(prepared);
    candidate.cleanup();
}

#[test]
fn malformed_stale_and_inadmissible_replacements_preserve_both_exact_old_traces() {
    let _serial = real_prepare_serial();
    let old = revision();
    let candidate = Candidate::new(true);
    // The scalar Project linker admits an automatic nonentry identity, but
    // prepared and legacy execution require explicit identities throughout
    // the selected closure. The candidate is genuine admitted Project HIR.
    candidate.revision.check().unwrap();
    let test_program = candidate.revision.test_program();
    let helper = test_program
        .functions
        .iter()
        .find(|function| function.name == "helper")
        .unwrap();
    assert_eq!(
        test_program
            .declarations
            .declaration(&helper.id)
            .unwrap()
            .identity_origin,
        crate::hir::IdentityOrigin::Automatic
    );
    let rejected = candidate
        .revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .err()
        .expect("automatic helper must not widen prepared admission");
    assert_eq!(rejected[0].code, "SPX-F107");
    assert_eq!(
        candidate
            .revision
            .execute_test(&ProjectExecutionOptions::default())
            .unwrap_err()[0]
            .code,
        "SPX-F102"
    );
    let prepared = old
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let options = PreparedProjectExecutionOptions::default();
    let before = pair(&prepared, &options);
    let malformed = [
        String::new(),
        format!("sha256:{}", "a".repeat(63)),
        format!("sha256:{}", "a".repeat(65)),
        format!("sha256:{}", "A".repeat(64)),
        format!("sha256:{}", "g".repeat(64)),
        format!("SHA256:{}", "a".repeat(64)),
        format!(" {}", old.project_revision()),
        format!("{}\n", old.project_revision()),
    ];
    for expected in malformed
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(candidate.revision.project_revision()))
    {
        assert_eq!(
            prepared
                .replace_revision(expected, Arc::clone(&candidate.revision))
                .unwrap_err()[0]
                .code,
            "SPX-F108"
        );
        assert_exact_pair(&before, &pair(&prepared, &options));
    }
    for _ in 0..2 {
        assert_eq!(
            prepared
                .replace_revision(old.project_revision(), Arc::clone(&candidate.revision))
                .unwrap_err()[0]
                .code,
            "SPX-F107"
        );
        assert_exact_pair(&before, &pair(&prepared, &options));
    }
    assert_legacy_parity(&old, &pair(&prepared, &options));
    drop(prepared);
    candidate.cleanup();
}

#[test]
fn replacement_preserves_cancellation_trace_saturation_fuel_and_original_ceilings() {
    let _serial = real_prepare_serial();
    let old = revision();
    let candidate = Candidate::new(false);
    let ceilings =
        PreparedProjectInterpreterOptions::new(MIN_PROJECT_SOURCE_TRACE_BYTES, 2).unwrap();
    let prepared = old.prepare_interpreter(ceilings).unwrap();
    let options = PreparedProjectExecutionOptions::new(
        interpreter::DEFAULT_MAX_STEPS,
        MIN_PROJECT_SOURCE_TRACE_BYTES,
        2,
    )
    .unwrap();
    let cancellation = ProjectExecutionCancellation::new();
    cancellation.cancel();
    assert_eq!(
        prepared
            .execute_entry(&options, &cancellation)
            .unwrap()
            .steps_used(),
        0
    );
    prepared
        .replace_revision(old.project_revision(), Arc::clone(&candidate.revision))
        .unwrap();
    assert!(cancellation.is_cancelled());
    for role in [ProjectExecutionRole::Entry, ProjectExecutionRole::Test] {
        let cancelled = prepared.execute(role, &options, &cancellation).unwrap();
        assert_eq!(
            cancelled.outcome(),
            &ProjectPreparedExecutionOutcome::Cancelled { before_step: 1 }
        );
        assert_eq!(cancelled.steps_used(), 0);
        assert_eq!(cancelled.trace().recorded_events(), 0);
        verify_project_source_trace_against_revision(
            &candidate.revision,
            cancelled.trace().envelope(),
        )
        .unwrap();
    }
    let saturated = pair(&prepared, &options);
    for execution in &saturated {
        assert_eq!(execution.trace().recorded_events(), 2);
        assert!(execution.trace().truncated());
    }
    assert_legacy_parity(&candidate.revision, &saturated);
    for excessive in [
        PreparedProjectExecutionOptions {
            max_trace_bytes: MIN_PROJECT_SOURCE_TRACE_BYTES + 1,
            ..options
        },
        PreparedProjectExecutionOptions {
            max_trace_events: 3,
            ..options
        },
    ] {
        assert_eq!(
            prepared
                .execute_entry(&excessive, &ProjectExecutionCancellation::new())
                .unwrap_err()[0]
                .code,
            "SPX-F108"
        );
        assert_exact_pair(&saturated, &pair(&prepared, &options));
    }
    let exhausted = pair(
        &prepared,
        &PreparedProjectExecutionOptions {
            max_steps: 1,
            ..options
        },
    );
    for execution in &exhausted {
        assert_eq!(
            execution.outcome(),
            &ProjectPreparedExecutionOutcome::FuelExhausted
        );
        assert_eq!(execution.steps_used(), 1);
    }
    assert_legacy_parity(&candidate.revision, &exhausted);
    assert_exact_pair(&saturated, &pair(&prepared, &options));
    drop(prepared);
    candidate.cleanup();
}
