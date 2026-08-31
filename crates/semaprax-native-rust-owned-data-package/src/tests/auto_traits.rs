//! Compile-only external consumers of actual generated SDKs. No provider is
//! linked or executed; these checks do not establish runtime thread safety.
use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PRELUDE: &str = "#![forbid(unsafe_code)]\nuse generated_sdk::{NativeRustOwnedDataSdk,CallError};\npub fn construct()->Result<NativeRustOwnedDataSdk,CallError>{NativeRustOwnedDataSdk::new()}\n";

fn generated() -> Vec<(&'static str, String, String)> {
    let target = HostTarget::current().unwrap();
    let bytes = descriptor_bytes("owned-bytes");
    let descriptor = descriptor::replay(
        &bytes,
        &descriptor_digest(&bytes),
        &["fixture.value".to_owned()],
    )
    .unwrap();
    let mut profiles = Vec::new();
    for (label, mode) in [
        ("v8-standalone", PackageMode::StandaloneEvidence),
        ("v8-project", PackageMode::ProjectV8),
    ] {
        let sources = render::render_sources(&descriptor, target, mode);
        profiles.push((label, sources.lib_rs, sources.ffi_rs));
    }
    let bytes = include_bytes!("../../../../tests/fixtures/flat_descriptor_retained_names.json");
    let descriptor = flat_descriptor::replay(
        bytes,
        &flat_descriptor_digest(bytes),
        &["api.value".to_owned()],
    )
    .unwrap();
    let sources = flat_render::render_sources(&descriptor, target);
    profiles.push(("v9-project", sources.lib_rs, sources.ffi_rs));
    let bytes = utf8_descriptor_bytes();
    let descriptor = descriptor::replay(
        &bytes,
        &utf8_descriptor_digest(&bytes),
        &["fixture.count".to_owned(), "fixture.text".to_owned()],
    )
    .unwrap();
    let sources = render::render_sources(&descriptor, target, PackageMode::ProjectV10OwnedUtf8);
    profiles.push(("v10-project", sources.lib_rs, sources.ffi_rs));
    profiles
}

pub(super) fn write_new(path: &Path, text: &str) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

pub(super) fn compiler(source: &Path, output: &Path, name: &str) -> Command {
    let mut command = Command::new("rustc");
    command
        .args([
            "--edition=2021",
            "--crate-type=lib",
            "--emit=metadata",
            "--error-format=json",
            "-Dwarnings",
            "--crate-name",
            name,
        ])
        .arg(source)
        .arg("-o")
        .arg(output);
    command
}

pub(super) fn success(output: Output, metadata: &Path) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let file = fs::symlink_metadata(metadata).unwrap();
    assert!(file.is_file() && !file.file_type().is_symlink());
    assert!(file.len() > 0);
}

fn messages(diagnostic: &Value, result: &mut String) {
    result.push_str(diagnostic["message"].as_str().unwrap());
    result.push('\n');
    for child in diagnostic["children"].as_array().unwrap() {
        messages(child, result);
    }
}

fn rejected(output: Output, consumer: &Path, metadata: &Path, bound: &str) {
    assert!(
        !output.status.success(),
        "SDK unexpectedly implements {bound}"
    );
    assert!(!metadata.exists(), "rejected consumer published metadata");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let mut matched = 0;
    for line in stderr.lines() {
        let diagnostic: Value = serde_json::from_str(line).unwrap();
        if diagnostic["level"] != "error" {
            continue;
        }
        if diagnostic["code"].is_null() {
            assert!(
                diagnostic["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("aborting due to "),
                "{stderr}"
            );
            continue;
        }
        assert_eq!(diagnostic["code"]["code"], "E0277", "{stderr}");
        assert!(
            diagnostic["spans"].as_array().unwrap().iter().any(|span| {
                span["is_primary"] == true
                    && span["line_start"] == 6
                    && span["line_end"] == 6
                    && Path::new(span["file_name"].as_str().unwrap()) == consumer
            }),
            "missing intentional consumer-bound span: {stderr}"
        );
        let mut facts = String::new();
        messages(&diagnostic, &mut facts);
        assert!(facts.contains("NativeRustOwnedDataSdk"), "{stderr}");
        assert!(facts.contains(bound), "{stderr}");
        matched += 1;
    }
    assert!(matched > 0, "no expected {bound} rejection: {stderr}");
}

#[test]
fn generated_owned_sdks_reject_send_and_sync_in_external_safe_consumers() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-sdk-auto-traits-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    // Retain the fixed four-profile fixture. No recursive cleanup and no
    // dependence on the linked provider's behavior or availability.
    for (label, library, ffi) in generated() {
        let directory = root.join(label);
        fs::create_dir(&directory).unwrap();
        let library_path = directory.join("lib.rs");
        write_new(&library_path, &library);
        write_new(&directory.join("owned_data_ffi.rs"), &ffi);
        let metadata = directory.join("libgenerated_sdk.rmeta");
        success(
            compiler(&library_path, &metadata, "generated_sdk")
                .output()
                .unwrap(),
            &metadata,
        );
        let compile = |source: &Path, output: &Path| {
            compiler(source, output, "consumer")
                .arg("--extern")
                .arg(format!("generated_sdk={}", metadata.display()))
                .output()
                .unwrap()
        };
        let healthy = directory.join("healthy.rs");
        write_new(&healthy, PRELUDE);
        let healthy_metadata = directory.join("healthy.rmeta");
        success(compile(&healthy, &healthy_metadata), &healthy_metadata);
        for bound in ["Send", "Sync"] {
            let source = format!("{PRELUDE}fn require_bound<T:{bound}>(){{}}\npub fn check(){{\nrequire_bound::<NativeRustOwnedDataSdk>();\n}}\n");
            assert_eq!(
                source.lines().nth(5),
                Some("require_bound::<NativeRustOwnedDataSdk>();")
            );
            let consumer: PathBuf = directory.join(format!("{bound}.rs"));
            write_new(&consumer, &source);
            let rejected_metadata = directory.join(format!("{bound}.rmeta"));
            rejected(
                compile(&consumer, &rejected_metadata),
                &consumer,
                &rejected_metadata,
                bound,
            );
        }
    }
}
