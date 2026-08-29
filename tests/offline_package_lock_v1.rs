//! Authored evidence for Offline Package Lock v1.
//!
//! This suite exercises exact replay, deterministic diamond ordering,
//! capability closure, optional facts, hostile digest re-minting, graph
//! confusion, limits, legacy Package Report preservation, and the stdout-only
//! CLI. It performs no fetch, registry access, dependency compilation, script,
//! target execution, source mutation, or lockfile publication.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::diagnostic::quote_json;
use semaprax::package_lock::{self, PackageLockOptions};
use semaprax::package_report::{self, PackageReportOptions};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

const SUBJECT_DOMAIN: &[u8] = b"semaprax.offline-package-subject.payload.v1\0";
const LOCK_DOMAIN: &[u8] = b"semaprax.offline-package-lock.payload.v1\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.package-report.payload.v1\0";

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn temp_path(label: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-package-lock-{label}-{}-{}.{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst),
        extension
    ))
}

fn report(module: &str) -> String {
    let path = temp_path(&module.replace('.', "-"), "spx");
    let stable_id = format!("{module}.value");
    std::fs::write(
        &path,
        format!(
            "module {module};\n\n@id({})\nfn value() -> i64 {{ 1 }}\n",
            quote_json(&stable_id)
        ),
    )
    .unwrap();
    let envelope = package_report::generate(&path, &PackageReportOptions::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    envelope
}

fn coordinate(package: &str, version: &str) -> String {
    format!(
        "{{\"package\":{},\"version\":{}}}",
        quote_json(package),
        quote_json(version)
    )
}

fn mint_subject(
    report: &str,
    version: &str,
    dependencies: &[(&str, &str)],
    capabilities: &[&str],
    licenses: &[&str],
    provenance: &[(&str, &str)],
) -> String {
    let report_value: Value = serde_json::from_str(report).unwrap();
    let package = report_value["payload"]["package"]["name"].as_str().unwrap();
    let report_digest = report_value["digest"].as_str().unwrap();
    let payload = format!(
        "{{\"schema\":\"semaprax.offline-package-subject.v1\",\"package\":{},\"version\":{},\"report\":{{\"schema\":\"semaprax.package-report.v1\",\"digest\":{},\"bytes\":{},\"envelope\":{}}},\"dependencies\":[{}],\"capabilities\":[{}],\"licenses\":[{}],\"provenance\":[{}]}}",
        quote_json(package),
        quote_json(version),
        quote_json(report_digest),
        report.len(),
        quote_json(report),
        dependencies
            .iter()
            .map(|(package, version)| coordinate(package, version))
            .collect::<Vec<_>>()
            .join(","),
        capabilities
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(","),
        licenses
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(","),
        provenance
            .iter()
            .map(|(kind, value)| format!(
                "{{\"kind\":{},\"value\":{}}}",
                quote_json(kind),
                quote_json(value)
            ))
            .collect::<Vec<_>>()
            .join(","),
    );
    format!(
        "{{\"schema\":\"semaprax.offline-package-subject.v1\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(&digest(SUBJECT_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload
    )
}

fn remint_wrapper(value: &str, domain: &[u8]) -> String {
    let marker = "\"payload\":";
    let offset = value.find(marker).unwrap() + marker.len();
    let payload = &value[offset..value.len() - 1];
    let schema = serde_json::from_str::<Value>(value).unwrap()["schema"]
        .as_str()
        .unwrap()
        .to_owned();
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(&schema),
        quote_json(&digest(domain, payload.as_bytes())),
        payload.len(),
        payload
    )
}

fn diamond_subjects() -> Vec<String> {
    let d = mint_subject(
        &report("lock.d"),
        "1.0.0",
        &[],
        &["filesystem.read"],
        &["MIT"],
        &[("source", "sha256:source-d")],
    );
    let b = mint_subject(
        &report("lock.b"),
        "1.0.0",
        &[("lock.d", "1.0.0")],
        &["network.read"],
        &[],
        &[],
    );
    let c = mint_subject(
        &report("lock.c"),
        "1.0.0",
        &[("lock.d", "1.0.0")],
        &["process"],
        &[],
        &[],
    );
    let a = mint_subject(
        &report("lock.a"),
        "2.0.0",
        &[("lock.b", "1.0.0"), ("lock.c", "1.0.0")],
        &["secrets.read"],
        &["Apache-2.0"],
        &[("repository", "https://invalid.example/lock-a")],
    );
    vec![c, a, d, b]
}

#[test]
fn diamond_is_dependency_first_deterministic_and_exactly_replayable() {
    let subjects = diamond_subjects();
    let options = PackageLockOptions::default();
    let first = package_lock::generate(&subjects, &options).unwrap();
    let second = package_lock::generate(&subjects, &options).unwrap();
    assert_eq!(first, second);
    let value: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(value["schema"], package_lock::SCHEMA);
    assert_eq!(
        value["payload"]["packages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["package"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["lock.d", "lock.b", "lock.c", "lock.a"]
    );
    assert_eq!(
        value["payload"]["roots"],
        serde_json::json!([{"package":"lock.a","version":"2.0.0"}])
    );
    assert_eq!(
        value["payload"]["edges"][0],
        serde_json::json!({
            "dependency":{"package":"lock.b","version":"1.0.0"},
            "dependent":{"package":"lock.a","version":"2.0.0"}
        })
    );
    let a = &value["payload"]["packages"][3];
    assert_eq!(
        a["capabilities"]["closure"],
        serde_json::json!(["filesystem.read", "network.read", "process", "secrets.read"])
    );
    assert_eq!(a["licenses"], serde_json::json!(["Apache-2.0"]));
    let verified = package_lock::verify(&first, &subjects, &options).unwrap();
    assert_eq!(
        verified.packages(),
        [
            "lock.d@1.0.0",
            "lock.b@1.0.0",
            "lock.c@1.0.0",
            "lock.a@2.0.0"
        ]
        .map(str::to_owned)
    );
}

#[test]
fn subject_and_lock_tamper_fail_even_after_outer_digest_remint() {
    let report = report("lock.identity");
    let subject = mint_subject(&report, "1.0.0", &[], &[], &[], &[]);
    let forged_identity = remint_wrapper(
        &subject.replacen(
            "\"package\":\"lock.identity\"",
            "\"package\":\"lock.foreign\"",
            1,
        ),
        SUBJECT_DOMAIN,
    );
    let error = package_lock::generate(&[forged_identity], &PackageLockOptions::default())
        .unwrap_err()
        .remove(0);
    assert_eq!(error.code, "SPX-L404");

    let lock = package_lock::generate(&[subject.clone()], &PackageLockOptions::default()).unwrap();
    let forged_lock = remint_wrapper(
        &lock.replacen("\"version\":\"1.0.0\"", "\"version\":\"9.9.9\"", 1),
        LOCK_DOMAIN,
    );
    assert_eq!(
        package_lock::verify(&forged_lock, &[subject], &PackageLockOptions::default())
            .unwrap_err()
            .code,
        "SPX-L407"
    );
}

#[test]
fn report_target_remint_and_foreign_schema_are_rejected() {
    let original = report("lock.target");
    let forged_target = remint_wrapper(
        &original.replacen("\"available\":true", "\"available\":false", 1),
        REPORT_DOMAIN,
    );
    let subject = mint_subject(&forged_target, "1.0.0", &[], &[], &[], &[]);
    assert_eq!(
        package_lock::generate(&[subject], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L403"
    );

    let forged_nonclaim = remint_wrapper(
        &original.replacen("report_descriptor_only", "report_is_authority", 1),
        REPORT_DOMAIN,
    );
    let subject = mint_subject(&forged_nonclaim, "1.0.0", &[], &[], &[], &[]);
    assert_eq!(
        package_lock::generate(&[subject], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L403"
    );

    let valid = mint_subject(&original, "1.0.0", &[], &[], &[], &[]);
    let foreign = remint_wrapper(
        &valid.replacen(
            "semaprax.offline-package-subject.v1",
            "semaprax.offline-package-subject.v2",
            1,
        ),
        SUBJECT_DOMAIN,
    );
    assert!(package_lock::generate(&[foreign], &PackageLockOptions::default()).is_err());
}

#[test]
fn cycles_duplicates_foreign_and_version_confusion_fail_closed() {
    let a_report = report("lock.cycle_a");
    let b_report = report("lock.cycle_b");
    let a = mint_subject(
        &a_report,
        "1.0.0",
        &[("lock.cycle_b", "1.0.0")],
        &[],
        &[],
        &[],
    );
    let b = mint_subject(
        &b_report,
        "1.0.0",
        &[("lock.cycle_a", "1.0.0")],
        &[],
        &[],
        &[],
    );
    assert_eq!(
        package_lock::generate(&[a, b], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L405"
    );

    let plain = mint_subject(&a_report, "1.0.0", &[], &[], &[], &[]);
    assert_eq!(
        package_lock::generate(&[plain.clone(), plain], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L404"
    );

    let first_version = mint_subject(&a_report, "1.0.0", &[], &[], &[], &[]);
    let second_version = mint_subject(&a_report, "2.0.0", &[], &[], &[], &[]);
    assert_eq!(
        package_lock::generate(
            &[first_version, second_version],
            &PackageLockOptions::default()
        )
        .unwrap_err()
        .remove(0)
        .code,
        "SPX-L404"
    );

    let self_edge = mint_subject(
        &a_report,
        "1.0.0",
        &[("lock.cycle_a", "1.0.0")],
        &[],
        &[],
        &[],
    );
    assert_eq!(
        package_lock::generate(&[self_edge], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L404"
    );

    let duplicate_edge = mint_subject(
        &a_report,
        "1.0.0",
        &[("lock.cycle_b", "1.0.0"), ("lock.cycle_b", "1.0.0")],
        &[],
        &[],
        &[],
    );
    let b = mint_subject(&b_report, "1.0.0", &[], &[], &[], &[]);
    assert_eq!(
        package_lock::generate(&[duplicate_edge, b], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L404"
    );

    let foreign = mint_subject(
        &a_report,
        "1.0.0",
        &[("lock.missing", "1.0.0")],
        &[],
        &[],
        &[],
    );
    assert_eq!(
        package_lock::generate(&[foreign], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L404"
    );

    let mismatch = mint_subject(
        &a_report,
        "1.0.0",
        &[("lock.cycle_b", "2.0.0")],
        &[],
        &[],
        &[],
    );
    let b = mint_subject(&b_report, "1.0.0", &[], &[], &[], &[]);
    assert_eq!(
        package_lock::generate(&[mismatch, b], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L404"
    );
}

#[test]
fn package_report_legacy_bytes_are_unchanged_by_lock_generation() {
    let path = Path::new("examples/meaning.spx");
    let before = package_report::generate(path, &PackageReportOptions::default()).unwrap();
    let subject = mint_subject(&before, "1.0.0", &[], &[], &[], &[]);
    let _ = package_lock::generate(&[subject], &PackageLockOptions::default()).unwrap();
    let after = package_report::generate(path, &PackageReportOptions::default()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn count_and_output_limits_fail_without_partial_artifact() {
    assert_eq!(
        package_lock::generate(&[], &PackageLockOptions::default())
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L406"
    );
    let report = report("lock.limit");
    let capabilities = (0..package_lock::MAX_CAPABILITIES)
        .map(|index| format!("network.read{index:03}"))
        .collect::<Vec<_>>();
    let capability_refs = capabilities.iter().map(String::as_str).collect::<Vec<_>>();
    let subject = mint_subject(&report, "1.0.0", &[], &capability_refs, &[], &[]);
    let options = PackageLockOptions::new(4_096).unwrap();
    assert_eq!(
        package_lock::generate(&[subject], &options)
            .unwrap_err()
            .remove(0)
            .code,
        "SPX-L406"
    );
}

#[test]
fn cli_reads_explicit_subjects_and_writes_only_stdout() {
    let subject = mint_subject(&report("lock.cli"), "1.0.0", &[], &[], &[], &[]);
    let path = temp_path("subject", "json");
    std::fs::write(&path, subject).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("package-lock")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(output.stdout.ends_with(b"\n"));
    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    assert_eq!(stdout.matches('\n').count(), 1);
    assert!(stdout.contains("\"schema\":\"semaprax.offline-package-lock.v1\""));
}

#[cfg(unix)]
#[test]
fn cli_rejects_two_paths_to_the_same_held_file_identity() {
    let subject = mint_subject(&report("lock.alias"), "1.0.0", &[], &[], &[], &[]);
    let first = temp_path("alias-first", "json");
    let second = temp_path("alias-second", "json");
    std::fs::write(&first, subject).unwrap();
    std::fs::hard_link(&first, &second).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("package-lock")
        .arg(&first)
        .arg(&second)
        .output()
        .unwrap();
    std::fs::remove_file(first).unwrap();
    std::fs::remove_file(second).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-I215"));
}
