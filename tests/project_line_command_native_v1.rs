//! Project v7 multi-module native line-command product evidence.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PROJECT_FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/filter.spx",
    "src/tests.spx",
];

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-line-command-native-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-lines-project");
    for file in PROJECT_FILES {
        std::fs::copy(source.join(file), root.join(file)).unwrap();
    }
    Fixture(root.canonicalize().unwrap())
}

fn run(path: &Path, arguments: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(path)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

#[cfg(not(windows))]
fn run_node(path: &Path, arguments: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new("node")
        .arg(path)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn project_v7_builds_and_runs_the_real_multimodule_line_filter() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let project = fixture();
    let output = project.0.join("spxgrep-lines");
    let built = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            output.to_str().unwrap(),
        ])
        .current_dir(&project.0)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "Project v7 native build failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let executable = output.with_extension(std::env::consts::EXE_SUFFIX);

    let input = b"alpha\nbeta\0\nalphabet";
    let matched = run(&executable, &["alpha"], input);
    assert_eq!(matched.status.code(), Some(0));
    assert_eq!(matched.stdout, b"alpha\nalphabet");
    assert!(matched.stderr.is_empty());

    let missed = run(&executable, &["absent"], input);
    assert_eq!(missed.status.code(), Some(1));
    assert!(missed.stdout.is_empty());
    assert!(missed.stderr.is_empty());

    let usage = run(&executable, &[], &[]);
    assert_eq!(usage.status.code(), Some(1));
    assert!(usage.stdout.is_empty());
    assert_eq!(usage.stderr, b"usage: spxgrep-lines <needle>\n");
}

#[test]
fn project_v7_cross_module_borrow_boundary_is_slice_only_and_non_escaping() {
    let project = fixture();
    let hostile = r#"module spxgrep_lines.filter;

@id("spxgrep-lines.contains")
fn contains(input: borrow str, needle: borrow str) -> bool
{
    str_contains(input, needle)
}

"#;
    let path = project.0.join("src/filter.spx");
    let ast = semaprax::parse(hostile, &path).unwrap();
    std::fs::write(&path, semaprax::format::canonical(&ast)).unwrap();
    let output = project.0.join("must-not-publish");
    let rejected = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            output.to_str().unwrap(),
        ])
        .current_dir(&project.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("SPX-G172"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(!output.exists());
}

#[test]
fn project_v6_cannot_downgrade_a_reachable_v7_operation_closure() {
    let project = fixture();
    let manifest_path = project.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("semaprax.project.v7", "semaprax.project.v6")
        .replace("line-command-io.v1", "language-command-io.v1");
    std::fs::write(&manifest_path, manifest).unwrap();
    let output = project.0.join("must-not-publish-v6");
    let rejected = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            output.to_str().unwrap(),
        ])
        .current_dir(&project.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("SPX-W114")
            && String::from_utf8_lossy(&rejected.stderr).contains(
                "Language Command I/O v1 cannot reach byte_range, stdout_append, or stderr_append"
            ),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(!output.exists());
    assert!(!output.with_extension(std::env::consts::EXE_SUFFIX).exists());
}

#[cfg(not(windows))]
#[test]
fn project_v7_npm_package_runs_the_same_multimodule_line_filter() {
    let node = Command::new("node")
        .arg("--version")
        .output()
        .expect("Project v7 npm product evidence requires Node");
    assert!(
        node.status.success(),
        "Node is unavailable for Project v7 evidence"
    );
    let project = fixture();
    let package = project.0.join("package");
    let built = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "npm",
            "-o",
            package.to_str().unwrap(),
        ])
        .current_dir(&project.0)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "Project v7 npm build failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let adapter = package.join("semaprax.command.js");
    let input = b"alpha\nbeta\0\nalphabet";
    let matched = run_node(&adapter, &["alpha"], input);
    assert_eq!(matched.status.code(), Some(0));
    assert_eq!(matched.stdout, b"alpha\nalphabet");
    assert!(matched.stderr.is_empty());

    let missed = run_node(&adapter, &["absent"], input);
    assert_eq!(missed.status.code(), Some(1));
    assert!(missed.stdout.is_empty());
    assert!(missed.stderr.is_empty());
}

#[cfg(windows)]
#[test]
fn project_v7_npm_publication_fails_closed_without_windows_authority() {
    let project = fixture();
    let package = project.0.join("package");
    let rejected = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "build",
            "semaprax.toml",
            "--target",
            "npm",
            "-o",
            package.to_str().unwrap(),
        ])
        .current_dir(&project.0)
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains(
            "error[SPX-W120]: useful-data npm publication requires safe handle-relative Windows authority"
        ),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(!package.exists());
}
