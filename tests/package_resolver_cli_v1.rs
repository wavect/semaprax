use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-package-resolver-cli-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command.arg("package-resolve");
    command
}

fn minimally_shaped(path: &std::path::Path) -> Command {
    let mut command = command();
    command
        .arg(path)
        .arg("--require")
        .arg("example.package:=1.0.0")
        .arg("--target")
        .arg("native64");
    command
}

#[test]
fn help_adds_only_the_frozen_package_resolve_usage_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("semaprax package-lock <subject.json>... [--max-bytes N]"));
    assert!(stdout.contains("semaprax package-resolve <subject.json>... --require <package>:<range> [--require ...] --target <native64|wasm32> [--allow-capability <capability>]... [--max-bytes N]"));
}

#[test]
fn exact_subject_resolves_to_one_canonical_stdout_line() {
    let report = package_report_v2::generate(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/meaning.spx"),
        &PackageReportV2Options::default(),
    )
    .unwrap();
    let coordinate = Coordinate {
        package: "examples.meaning".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let subject = package_lock_v2::create_subject(&coordinate, &report, &[], &[]).unwrap();
    let path = temp_path("valid-subject");
    std::fs::write(&path, subject).unwrap();
    let output = command()
        .arg(&path)
        .arg("--require")
        .arg("examples.meaning:=1.0.0")
        .arg("--target")
        .arg("native64")
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("\"schema\":\"semaprax.offline-package-resolution-evidence.v1\""));
}

#[test]
fn grouped_grammar_rejects_before_opening_any_subject() {
    let absent = temp_path("absent");
    for arguments in [
        vec![absent.display().to_string()],
        vec![
            absent.display().to_string(),
            "--target".into(),
            "native64".into(),
        ],
        vec![
            absent.display().to_string(),
            "--require".into(),
            "example.package:=1.0.0".into(),
            "--target".into(),
            "native64".into(),
            "--require".into(),
            "later.package:=1.0.0".into(),
        ],
        vec![
            absent.display().to_string(),
            "--require".into(),
            "example.package:=1.0.0".into(),
            "--target".into(),
            "native64".into(),
            "--max-bytes".into(),
            "4096".into(),
            "--max-bytes".into(),
            "4096".into(),
        ],
    ] {
        let output = command().args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
    }
}

#[test]
fn malformed_ranges_targets_names_and_order_reject_before_open() {
    let absent = temp_path("grammar-absent");
    for (requirement, target) in [
        ("bad:name:=1.0.0", "native64"),
        ("example.package:1.0.0", "native64"),
        ("example.package:^0.0.4294967295", "native64"),
        ("example.package:=01.0.0", "native64"),
        ("example.package:=1.0.0", "unknown"),
    ] {
        let output = command()
            .arg(&absent)
            .arg("--require")
            .arg(requirement)
            .arg("--target")
            .arg(target)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
    }
}

#[test]
fn non_regular_and_invalid_utf8_inputs_fail_with_io_diagnostic() {
    let directory = temp_path("directory");
    std::fs::create_dir(&directory).unwrap();
    let output = minimally_shaped(&directory).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
    std::fs::remove_dir(&directory).unwrap();

    let invalid = temp_path("invalid-utf8");
    std::fs::write(&invalid, [0xff]).unwrap();
    let output = minimally_shaped(&invalid).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
    std::fs::remove_file(invalid).unwrap();
}

#[test]
fn declared_per_file_and_cumulative_bounds_reject_before_reads() {
    const MAX_SUBJECT_BYTES: u64 = 17 * 1024 * 1024;
    let oversized = temp_path("oversized");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_SUBJECT_BYTES + 1)
        .unwrap();
    let output = minimally_shaped(&oversized).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-PR505"));
    std::fs::remove_file(oversized).unwrap();

    let paths = (0..8)
        .map(|index| {
            let path = temp_path(&format!("cumulative-{index}"));
            std::fs::File::create(&path)
                .unwrap()
                .set_len(MAX_SUBJECT_BYTES)
                .unwrap();
            path
        })
        .collect::<Vec<_>>();
    let output = command()
        .args(&paths)
        .arg("--require")
        .arg("example.package:=1.0.0")
        .arg("--target")
        .arg("native64")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-PR505"));
    for path in paths {
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn symlink_leaf_fails_closed() {
    use std::os::unix::fs::symlink;

    let source = temp_path("source");
    let alias = temp_path("alias");
    std::fs::write(&source, b"{}").unwrap();
    symlink(&source, &alias).unwrap();
    let output = minimally_shaped(&alias).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
    std::fs::remove_file(&alias).unwrap();

    std::fs::remove_file(source).unwrap();
}

#[test]
fn same_file_aliases_fail_closed() {
    let source = temp_path("hardlink-source");
    let alias = temp_path("hardlink-alias");
    std::fs::write(&source, b"{}").unwrap();
    std::fs::hard_link(&source, &alias).unwrap();
    let output = command()
        .arg(&source)
        .arg(&alias)
        .arg("--require")
        .arg("example.package:=1.0.0")
        .arg("--target")
        .arg("native64")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
    std::fs::remove_file(source).unwrap();
    std::fs::remove_file(alias).unwrap();
}

#[test]
fn zero_and_sixty_five_subjects_are_usage_failures() {
    let output = command()
        .arg("--require")
        .arg("example.package:=1.0.0")
        .arg("--target")
        .arg("native64")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));

    let arguments = (0..65)
        .map(|index| format!("missing-{index}"))
        .chain([
            "--require".to_owned(),
            "example.package:=1.0.0".to_owned(),
            "--target".to_owned(),
            "native64".to_owned(),
        ])
        .collect::<Vec<_>>();
    let output = command().args(arguments).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
}
