use semaprax::project::derive_project_scaffold;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn invoke(arguments: &[&str]) -> (Output, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-scaffold-cli-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(&root)
        .output()
        .unwrap();
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    (output, root)
}

fn expected(name: &str) -> Vec<u8> {
    let artifact = derive_project_scaffold(name, "calculator").unwrap();
    artifact.canonical_bytes()
}

#[test]
fn public_cli_prints_only_the_exact_replayable_capsule() {
    for arguments in [
        &["project-scaffold", "--name", "demo-project"][..],
        &[
            "project-scaffold",
            "--template",
            "calculator",
            "--name",
            "demo-project",
        ][..],
        &[
            "project-scaffold",
            "--name",
            "demo-project",
            "--template",
            "calculator",
        ][..],
    ] {
        let (output, root) = invoke(arguments);
        assert!(output.status.success(), "{arguments:?}");
        assert!(output.stderr.is_empty(), "{arguments:?}");
        assert_eq!(output.stdout, expected("demo-project"), "{arguments:?}");
        std::fs::remove_dir(root).unwrap();
    }
}

#[test]
fn malformed_options_fail_before_capsule_output_or_activity() {
    for arguments in [
        &["project-scaffold"][..],
        &["project-scaffold", "--name"][..],
        &["project-scaffold", "--template"][..],
        &["project-scaffold", "--unknown", "value"][..],
        &["project-scaffold", "--name", "one", "--name", "two"][..],
        &[
            "project-scaffold",
            "--template",
            "calculator",
            "--template",
            "calculator",
            "--name",
            "demo-project",
        ][..],
    ] {
        let (output, root) = invoke(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(!output.stderr.is_empty(), "{arguments:?}");
        std::fs::remove_dir(root).unwrap();
    }
}

#[test]
fn semantic_and_capacity_failures_publish_no_partial_document() {
    for (name, code) in [
        ("Bad_Name".to_owned(), "SPX-J115"),
        ("a".repeat(65), "SPX-J116"),
    ] {
        let (output, root) = invoke(&["project-scaffold", "--name", &name]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(code), "{stderr}");
        std::fs::remove_dir(root).unwrap();
    }
}
