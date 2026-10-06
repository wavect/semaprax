//! Tests for the optional `installers` inventory of the release manifest and
//! for the legacy (v0.8.0) three-platform manifest shape.

use std::fs;

use super::*;

const V080_MANIFEST: &str = include_str!("fixtures/v0.8.0-release-manifest.json");
const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

fn archive_file_name(platform: &str) -> String {
    let extension = if platform.contains("windows") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("semaprax-v9.9.9-{platform}.{extension}")
}

fn current_manifest(installers: Option<&str>) -> String {
    let entries: Vec<String> = ARCHIVE_PLATFORMS
        .iter()
        .map(|platform| {
            let name = archive_file_name(platform);
            format!(
                r#"{{"name": "{name}", "platform": "{platform}", "size": 10, "digest": "{DIGEST}"}}"#
            )
        })
        .collect();
    let artifacts = entries.join(",\n    ");
    let installers = installers
        .map(|value| format!(",\n  \"installers\": {value}"))
        .unwrap_or_default();
    format!(
        r#"{{
  "schema": "semaprax.release-manifest.v1",
  "version": "9.9.9",
  "tag": "v9.9.9",
  "commit": "{COMMIT}",
  "prerelease": false,
  "required_checks": ["alpha"],
  "changelog_section_digest": "{DIGEST}",
  "artifacts": [
    {artifacts}
  ]{installers}
}}"#
    )
}

fn installer_json(name: &str, size: usize, digest: &str) -> String {
    format!(r#"{{"name": "{name}", "size": {size}, "digest": "{digest}"}}"#)
}

#[test]
fn real_v080_manifest_without_installers_still_parses() {
    let manifest = parse_manifest(V080_MANIFEST.as_bytes()).expect("v0.8.0 shape must parse");
    assert_eq!(manifest.version, "0.8.0");
    assert_eq!(manifest.artifacts.len(), LEGACY_ARCHIVE_PLATFORMS.len());
    assert!(manifest.installers.is_empty());
}

#[test]
fn legacy_three_platform_set_is_rejected_for_newer_versions() {
    let newer = V080_MANIFEST
        .replace("v0.8.0", "v0.9.0")
        .replace("0.8.0", "0.9.0");
    let error = parse_manifest(newer.as_bytes())
        .expect_err("a post-0.8.0 manifest must inventory every admitted platform");
    assert!(error.message.contains("missing"));
}

#[test]
fn manifest_without_installers_key_parses_for_current_platforms() {
    let manifest = parse_manifest(current_manifest(None).as_bytes()).unwrap();
    assert_eq!(manifest.artifacts.len(), ARCHIVE_PLATFORMS.len());
    assert!(manifest.installers.is_empty());
}

#[test]
fn manifest_with_installers_parses_and_sorts_by_name() {
    let installers = format!(
        "[{}, {}]",
        installer_json("install.sh", 7, DIGEST),
        installer_json("install.ps1", 9, DIGEST)
    );
    let manifest = parse_manifest(current_manifest(Some(&installers)).as_bytes()).unwrap();
    let names: Vec<&str> = manifest
        .installers
        .iter()
        .map(|installer| installer.name.as_str())
        .collect();
    assert_eq!(names, ["install.ps1", "install.sh"]);
    assert_eq!(manifest.installers[0].size, 9);
}

#[test]
fn unknown_duplicate_or_malformed_installers_are_rejected() {
    for (description, installers) in [
        (
            "unknown name",
            format!("[{}]", installer_json("evil.sh", 7, DIGEST)),
        ),
        (
            "duplicate name",
            format!(
                "[{}, {}]",
                installer_json("install.sh", 7, DIGEST),
                installer_json("install.sh", 7, DIGEST)
            ),
        ),
        (
            "bad digest",
            format!("[{}]", installer_json("install.sh", 7, "sha256:AB")),
        ),
        (
            "extra key",
            format!(r#"[{{"name": "install.sh", "size": 7, "digest": "{DIGEST}", "url": "x"}}]"#),
        ),
        ("not an array", "{}".to_owned()),
    ] {
        parse_manifest(current_manifest(Some(&installers)).as_bytes()).expect_err(description);
    }
}

#[test]
fn on_disk_verification_rehashes_installers() {
    let dir = std::env::temp_dir().join(format!(
        "spx-release-installers-test-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let archive = b"0123456789";
    let archive_digest = sha256_digest(archive);
    let script = b"#!/bin/sh\necho hi\n";
    let script_digest = sha256_digest(script);
    let manifest = current_manifest(Some(&format!(
        "[{}]",
        installer_json("install.sh", script.len(), &script_digest)
    )))
    .replace(
        &format!("\"size\": 10, \"digest\": \"{DIGEST}\""),
        &format!("\"size\": 10, \"digest\": \"{archive_digest}\""),
    );
    for platform in ARCHIVE_PLATFORMS {
        fs::write(dir.join(archive_file_name(platform)), archive).unwrap();
    }
    fs::write(dir.join("install.sh"), script).unwrap();
    verify_manifest_artifacts_on_disk(manifest.as_bytes(), &dir)
        .expect("archives and installer agree with the manifest");

    fs::write(dir.join("install.sh"), b"#!/bin/sh\necho HI\n").unwrap();
    let error = verify_manifest_artifacts_on_disk(manifest.as_bytes(), &dir)
        .expect_err("a changed installer byte must be rejected");
    assert!(error.message.contains("install.sh"));
    assert!(error.message.contains("digest"));

    fs::remove_file(dir.join("install.sh")).unwrap();
    verify_manifest_artifacts_on_disk(manifest.as_bytes(), &dir)
        .expect_err("a listed installer that is absent must be rejected");
    fs::remove_dir_all(&dir).ok();
}
