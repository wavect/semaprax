//! Public Project v1 native executable publication evidence.
//!
//! One authenticated manifest set publishes its linked entry closure as one
//! native executable through the same linked HIR that Web publication and the
//! internal lowering-equivalence evidence consume. Publication is explicit,
//! create-new, and rechecks every held input before and after the boundary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project;

const PROJECT_FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture(label: &str) -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-native-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for file in PROJECT_FILES {
        std::fs::copy(source.join(file), root.join(file)).unwrap();
    }
    Fixture {
        root: root.canonicalize().unwrap(),
    }
}

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root)
        .output()
        .unwrap()
}

fn executable(path: &Path) -> PathBuf {
    path.with_extension(std::env::consts::EXE_EXTENSION)
}

fn run_stdout(path: &Path) -> String {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "execution failed: {path:?}");
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn explicit_native_targets_publish_and_run_the_linked_entry_closure() {
    let project = fixture("cli");
    let output = project.root.join("published");
    let result = cli(
        &project.root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            output.to_str().unwrap(),
        ],
    );
    assert!(
        result.status.success(),
        "native project build failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        format!(
            "built project native executable {}\n",
            executable(&output).display()
        )
    );
    assert_eq!(run_stdout(&executable(&output)), "42");

    let replay_fixture = fixture("cli-replay");
    let replay_output = replay_fixture.root.join("published");
    let replayed = cli(
        &replay_fixture.root,
        &[
            "build",
            "--manifest-path",
            replay_fixture.root.join("semaprax.toml").to_str().unwrap(),
            "--target",
            "native",
            "-o",
            replay_output.to_str().unwrap(),
        ],
    );
    assert!(
        replayed.status.success(),
        "replay native project build failed: {}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    assert_eq!(run_stdout(&executable(&replay_output)), "42");
}

#[test]
fn post_publication_drift_is_uncertain_but_preserves_the_executable() {
    let project = fixture("drift");
    let output = project.root.join("uncertain");
    let error =
        project::with_authenticated_project(&project.root.join("semaprax.toml"), |snapshot| {
            snapshot.build_native(&executable(&output))?;
            std::fs::write(project.root.join("src/app.spx"), "changed").unwrap();
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-J103");
    assert!(error[0].message.contains("native executable"));
    assert_eq!(run_stdout(&executable(&output)), "42");
}

#[test]
fn stable_id_display_rename_preserves_published_native_behavior() {
    let project = fixture("rename");
    std::fs::write(
        project.root.join("src/core.spx"),
        std::fs::read_to_string(project.root.join("src/core.spx"))
            .unwrap()
            .replace("fn add(", "fn sum("),
    )
    .unwrap();
    let output = project.root.join("renamed");
    let result = cli(
        &project.root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            output.to_str().unwrap(),
        ],
    );
    assert!(
        result.status.success(),
        "renamed native project build failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(run_stdout(&executable(&output)), "42");
}

#[test]
fn concurrent_native_builds_have_exactly_one_create_new_winner() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let project = fixture("concurrent");
    let output = project.root.join("published");
    let spawn = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
        command
            .args([
                "build",
                "semaprax.toml",
                "--target",
                "native",
                "-o",
                output.to_str().unwrap(),
            ])
            .current_dir(&project.root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        command.spawn().unwrap()
    };
    let first = spawn();
    let second = spawn();
    let results = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status.success())
            .count(),
        1
    );
    let loser = results
        .iter()
        .find(|result| !result.status.success())
        .unwrap();
    assert_eq!(loser.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&loser.stderr).contains("SPX-I307"));
    assert_eq!(run_stdout(&executable(&output)), "42");
}
