//! Project v6 language-command native product evidence.
//!
//! The ordinary Project CLI must select the manifest-authenticated command by
//! stable identity and publish the same bounded process adapter exercised by
//! the direct native lane. This is product wiring evidence, not a second
//! language-command implementation.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ProjectExecutionOptions, ProjectExecutionOutcome, ProjectSnapshot,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const PROJECT_FILES: &[&str] = &["semaprax.toml", "src/app.spx", "src/tests.spx"];

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
        "semaprax-project-language-command-native-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-language-command-project");
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

#[test]
fn native_executable_path_has_exactly_one_platform_extension() {
    let actual = executable(Path::new("program"));
    let expected = if std::env::consts::EXE_EXTENSION.is_empty() {
        PathBuf::from("program")
    } else {
        PathBuf::from(format!("program.{}", std::env::consts::EXE_EXTENSION))
    };
    assert_eq!(actual, expected);
    assert!(!actual.to_string_lossy().contains(".."));
}

fn build(root: &Path, output: &Path) -> Output {
    cli(
        root,
        &[
            "build",
            "semaprax.toml",
            "--target",
            "native",
            "-o",
            output.to_str().unwrap(),
        ],
    )
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

#[test]
fn project_v6_native_build_is_the_exact_bounded_language_command_product() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let project = fixture("product");
    let output = project.root.join("spxgrep");
    let built = build(&project.root, &output);
    assert!(
        built.status.success(),
        "Project v6 native build failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let executable = executable(&output);

    let binary = [0, b'n', b'e', b'e', b'd', b'l', b'e', 255];
    let matched = run(&executable, &["needle"], &binary);
    assert_eq!(matched.status.code(), Some(0));
    assert_eq!(matched.stdout, binary);
    assert!(matched.stderr.is_empty());

    let missed = run(&executable, &["absent"], &binary);
    assert_eq!(missed.status.code(), Some(1));
    assert!(missed.stdout.is_empty());
    assert_eq!(missed.stderr, b"not found\n");

    let usage = run(&executable, &[], &[]);
    assert_eq!(usage.status.code(), Some(1));
    assert!(usage.stdout.is_empty());
    assert_eq!(usage.stderr, b"usage: spxgrep <needle>\n");

    let mut exact = vec![b'a'; 65_535];
    exact[0] = b'x';
    let exact_limit = run(&executable, &["x"], &exact);
    assert_eq!(exact_limit.status.code(), Some(0));
    assert_eq!(exact_limit.stdout, exact);
    assert!(exact_limit.stderr.is_empty());

    exact.push(b'a');
    let over_limit = run(&executable, &["x"], &exact);
    assert_eq!(over_limit.status.code(), Some(2));
    assert!(over_limit.stdout.is_empty());
    assert_eq!(over_limit.stderr, b"SEMAPRAX language command failed\n");
}

#[test]
fn project_v6_native_publication_is_no_clobber_and_stable_id_selected() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let project = fixture("identity");

    // Keep the pre-refactor public Snapshot function-item surface source
    // compatible; Deref alone does not preserve these paths.
    let _ = ProjectSnapshot::manifest;
    let _ = ProjectSnapshot::sources;
    let _ = ProjectSnapshot::workspace_manifest;
    let _ = ProjectSnapshot::workspace_revision;
    let _ = ProjectSnapshot::project_revision;
    let _ = ProjectSnapshot::entry_program;
    let _ = ProjectSnapshot::test_program;
    let _ = ProjectSnapshot::semantic_graph;
    let _ = ProjectSnapshot::semantic_context;
    let _ = ProjectSnapshot::semantic_impact;
    let _ = ProjectSnapshot::check;
    let _ = ProjectSnapshot::execute_entry;
    let _ = ProjectSnapshot::execute_test;
    let _ = ProjectSnapshot::execute;
    let _ = ProjectSnapshot::build_web_inline;
    let _ = ProjectSnapshot::build_npm_inline;
    let _ = ProjectSnapshot::test_wasm_module;

    with_authenticated_project(&project.root.join("semaprax.toml"), |snapshot| {
        assert_eq!(snapshot.entry_program().entrypoint.as_str(), "main");
        assert!(!snapshot
            .entry_program()
            .functions
            .iter()
            .any(|function| function.id.as_str() == "spxgrep-language.run"));
        assert!(snapshot
            .retain_revision()
            .public_api_program()
            .functions
            .iter()
            .any(|function| function.id.as_str() == "spxgrep-language.run"));
        let executed = snapshot.execute_entry(&ProjectExecutionOptions::default())?;
        assert_eq!(executed.outcome(), &ProjectExecutionOutcome::Returned(0));
        Ok(())
    })
    .unwrap();

    let occupied = executable(&project.root.join("occupied"));
    std::fs::write(&occupied, b"foreign").unwrap();
    let rejected = build(&project.root, &occupied);
    assert!(!rejected.status.success());
    assert_eq!(std::fs::read(&occupied).unwrap(), b"foreign");

    let app = project.root.join("src/app.spx");
    let renamed = std::fs::read_to_string(&app).unwrap().replacen(
        "fn run() -> bool",
        "fn search() -> bool",
        1,
    );
    std::fs::write(&app, renamed).unwrap();
    let output = project.root.join("renamed");
    let built = build(&project.root, &output);
    assert!(
        built.status.success(),
        "stable-ID-selected build failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = run(&executable(&output), &["needle"], b"a needle b");
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"a needle b");
    assert!(result.stderr.is_empty());
}

#[test]
fn project_run_names_the_entry_and_the_command_adapters() {
    let project = fixture("run-note");
    let result = cli(&project.root, &["run", "semaprax.toml"]);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"0\n");
    assert_eq!(
        result.stderr,
        b"note: project run executes entry `main`; command function `spxgrep-language.run` is exercised by built native and web/npm adapters\n"
    );

    let json = cli(&project.root, &["run", "semaprax.toml", "--json"]);
    assert!(json.status.success());
    assert!(json.stderr.is_empty());
    let envelope: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(envelope["outcome"]["kind"], "returned");
}
