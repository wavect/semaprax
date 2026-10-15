//! Project-route evidence for the Bounded Language Command I/O v1 borrowed-`str`
//! argument root.
//!
//! `arg_utf8(index)` produces a `borrow str` over the single invocation-owned
//! argument arena, and checked-HIR validation must authenticate that root for a
//! local binding. The linked single-module regressions in
//! `tests/useful_data/line_filter_project_v7.rs` reassemble the committed v7
//! sources by hand, so only a manifest-driven case pins the route the CLI
//! actually takes for `check`, `run`, and `test`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ProjectExecutionOptions, ProjectExecutionOutcome,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Every committed spxgrep manifest, with the sources its manifest declares.
const COMMITTED_COMMAND_PROJECTS: &[(&str, &[&str])] = &[
    (
        "spxgrep-project",
        &["semaprax.toml", "src/app.spx", "src/tests.spx"],
    ),
    (
        "spxgrep-native-command-project",
        &["semaprax.toml", "src/app.spx", "src/tests.spx"],
    ),
    (
        "spxgrep-language-command-project",
        &["semaprax.toml", "src/app.spx", "src/tests.spx"],
    ),
    (
        "spxgrep-lines-project",
        &[
            "semaprax.toml",
            "src/app.spx",
            "src/filter.spx",
            "src/tests.spx",
        ],
    ),
];

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(example: &str, files: &[&str]) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-command-argument-borrow-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(example);
    for file in files {
        std::fs::copy(source.join(file), root.join(file)).unwrap();
    }
    Fixture(root.canonicalize().unwrap())
}

#[test]
fn every_committed_spxgrep_manifest_admits_through_the_project_check_route() {
    for (example, files) in COMMITTED_COMMAND_PROJECTS {
        let project = fixture(example, files);
        with_authenticated_project(&project.0.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        })
        .unwrap_or_else(|errors| panic!("{example} must admit on the project route: {errors:?}"));
    }
}

#[test]
fn command_argument_projects_still_execute_their_declared_tests() {
    for (example, files) in COMMITTED_COMMAND_PROJECTS {
        let project = fixture(example, files);
        let execution = with_authenticated_project(&project.0.join("semaprax.toml"), |snapshot| {
            snapshot.execute_test(&ProjectExecutionOptions::default())
        })
        .unwrap_or_else(|errors| panic!("{example} must run its tests: {errors:?}"));
        assert_eq!(
            execution.outcome(),
            &ProjectExecutionOutcome::Returned(0),
            "{example}"
        );
    }
}

#[test]
fn a_mutable_command_argument_local_still_fails_closed_on_the_project_route() {
    let (example, files) = COMMITTED_COMMAND_PROJECTS[3];
    let project = fixture(example, files);
    let app = project.0.join("src/app.spx");
    let source = std::fs::read_to_string(&app).unwrap();
    let hostile = source.replace(
        "let needle_text = arg_utf8(0usize);",
        "let mut needle_text = arg_utf8(0usize);",
    );
    assert_ne!(hostile, source, "the v7 app must bind arg_utf8 to a local");
    std::fs::write(&app, hostile).unwrap();
    let errors = with_authenticated_project(&project.0.join("semaprax.toml"), |snapshot| {
        snapshot.check()
    })
    .expect_err("a mutable borrowed-str command local must fail closed");
    assert!(
        errors.iter().any(|error| error.code == "SPX-T264"
            && error.message
                == "byte-slice alias `needle_text` must be immutable and cannot be reassigned"),
        "{errors:?}"
    );
}

#[test]
fn retained_hir_still_rejects_a_mutable_command_argument_alias_independently() {
    use semaprax::hir::{self, ResolvedExprKind, ResolvedStatement};
    let (example, files) = COMMITTED_COMMAND_PROJECTS[3];
    let project = fixture(example, files);
    let mut program = with_authenticated_project(&project.0.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.entry_program().clone())
    })
    .unwrap();
    hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "spxgrep-lines.run")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("the checked command retains its block body");
    };
    let ResolvedExprKind::If { then_branch, .. } = &mut tail.kind else {
        panic!("the checked command retains its argument-count condition");
    };
    let ResolvedExprKind::Block { statements, .. } = &mut then_branch.kind else {
        panic!("the argument branch retains its checked bindings");
    };
    let mut changed = false;
    for statement in statements {
        let ResolvedStatement::Let {
            binding, mutable, ..
        } = statement
        else {
            continue;
        };
        if binding.name != "needle_text" {
            continue;
        }
        assert!(!*mutable);
        *mutable = true;
        changed = true;
    }
    assert!(
        changed,
        "the original command-argument binding must be changed"
    );
    let error = hir::validate(&program).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "borrowed-str local alias must be immutable and unprojected"
    );
}
