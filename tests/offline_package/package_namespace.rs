//! Executable evidence for the `semaprax package` namespace: each subcommand
//! is exactly its long-form route, so stdout, stderr, and status agree, and
//! the namespace itself fails closed on a missing or unknown subcommand.

use std::path::PathBuf;
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(root())
        .output()
        .unwrap()
}

/// The usage recovery hint names the verb as typed (`package --help` rather
/// than `package-report --help`), so it is the one stderr line allowed to
/// differ.
fn without_hint(stderr: &[u8]) -> String {
    String::from_utf8(stderr.to_vec())
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("hint: run `semaprax "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn same(short: &[&str], long: &[&str]) {
    let short_output = cli(short);
    let long_output = cli(long);
    assert_eq!(
        short_output.status.code(),
        long_output.status.code(),
        "{short:?}"
    );
    assert_eq!(short_output.stdout, long_output.stdout, "{short:?}");
    assert_eq!(
        without_hint(&short_output.stderr),
        without_hint(&long_output.stderr),
        "{short:?}"
    );
}

#[test]
fn package_subcommands_are_their_long_forms() {
    let report = cli(&["package", "report", "examples/meaning.spx"]);
    assert!(
        report.status.success(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    assert!(!report.stdout.is_empty());
    same(
        &["package", "report", "examples/meaning.spx"],
        &["package-report", "examples/meaning.spx"],
    );
    same(
        &[
            "package",
            "report",
            "examples/meaning.spx",
            "--max-bytes",
            "4096",
        ],
        &[
            "package-report",
            "examples/meaning.spx",
            "--max-bytes",
            "4096",
        ],
    );
    same(
        &[
            "package",
            "report",
            "examples/meaning.spx",
            "--max-bytes",
            "64",
        ],
        &[
            "package-report",
            "examples/meaning.spx",
            "--max-bytes",
            "64",
        ],
    );
    same(&["package", "lock"], &["package-lock"]);
    same(
        &["package", "resolve", "examples/meaning.spx"],
        &["package-resolve", "examples/meaning.spx"],
    );
}

#[test]
fn package_namespace_fails_closed() {
    for arguments in [
        &["package"][..],
        &["package", "build"][..],
        &["package", "--report", "examples/meaning.spx"][..],
        &["package", "package-report", "examples/meaning.spx"][..],
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("package accepts exactly"));
    }
}
