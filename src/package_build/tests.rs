use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::package_lock_v2::{self, Coordinate};
use crate::package_report_v2::{self, PackageReportV2Options};
use crate::package_resolver::{self, Requirement};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    evidence: String,
    input: ResolutionInput,
    resolution_options: ResolutionOptions,
    build_options: OfflinePackageBuildOptions,
}

fn fixture() -> Fixture {
    let source = "module pkg.root;\n@id(\"pkg.add\") fn add(value: i64) -> i64 { value + 1 }\n@id(\"pkg.main\") fn main() -> i64 { add(41) }\n";
    let path = temporary_source_path();
    std::fs::write(&path, source).expect("write package-build fixture");
    let report = package_report_v2::generate(&path, &PackageReportV2Options::default())
        .expect("generate Report v2 fixture");
    std::fs::remove_file(&path).expect("remove package-build fixture");
    let subject = package_lock_v2::create_subject(
        &Coordinate {
            package: "pkg.root".to_owned(),
            version: "1.0.0".to_owned(),
        },
        &report,
        &[],
        &[],
    )
    .expect("create Subject v2 fixture");
    let input = ResolutionInput {
        requirements: vec![Requirement {
            package: "pkg.root".to_owned(),
            range: "=1.0.0".to_owned(),
        }],
        subjects: vec![subject],
        target: "wasm32".to_owned(),
        allowed_capabilities: Vec::new(),
    };
    let resolution_options = ResolutionOptions::default();
    let evidence =
        package_resolver::generate(&input, &resolution_options).expect("generate resolver fixture");
    let build_options = OfflinePackageBuildOptions::new(
        "pkg.root".to_owned(),
        vec!["pkg.add".to_owned(), "pkg.main".to_owned()],
        MAX_ARTIFACT_BYTES,
        MAX_EVIDENCE_BYTES,
    )
    .expect("valid package-build options");
    Fixture {
        evidence,
        input,
        resolution_options,
        build_options,
    }
}

fn temporary_source_path() -> PathBuf {
    let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-package-build-{}-{ordinal}.spx",
        std::process::id()
    ))
}

#[test]
fn generation_is_exact_and_independently_replayable() {
    let fixture = fixture();
    let first = generate(
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.build_options,
    )
    .unwrap();
    let second = generate(
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.build_options,
    )
    .unwrap();
    assert_eq!(first, second);
    assert!(first.manifest_json.contains(
        "\"runtime_imports\":[{\"module\":\"env\",\"name\":\"spx_add\",\"kind\":\"function\"}"
    ));
    assert!(!first.manifest_json.contains("semaprax_main"));
    let receipt = verify(
        &first,
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.build_options,
    )
    .unwrap();
    assert_eq!(receipt.root_package, "pkg.root");
    assert_eq!(receipt.packages.len(), 1);
    assert_eq!(
        receipt.artifact_bytes,
        first.module_wasm.len() + first.manifest_json.len() + first.evidence_json.len()
    );
}

#[test]
fn artifact_and_evidence_mutations_fail_exact_replay() {
    let fixture = fixture();
    let build = generate(
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.build_options,
    )
    .unwrap();
    let mut changed_wasm = build.clone();
    changed_wasm.module_wasm.push(0);
    assert_eq!(
        verify(
            &changed_wasm,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.build_options,
        )
        .unwrap_err()
        .code,
        "SPX-PB507"
    );

    let mut changed_evidence = build;
    changed_evidence.evidence_json = changed_evidence.evidence_json.replacen(
        "\"used_source_bytes\":",
        "\"used_source_bytes\":0,\"forged\":",
        1,
    );
    assert_eq!(
        verify(
            &changed_evidence,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.build_options,
        )
        .unwrap_err()
        .code,
        "SPX-PB507"
    );
}

#[test]
fn duplicate_wire_keys_are_rejected_before_replay() {
    let fixture = fixture();
    let build = generate(
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.build_options,
    )
    .unwrap();
    let mut duplicate_manifest = build.clone();
    duplicate_manifest.manifest_json = duplicate_manifest.manifest_json.replacen(
        "{\"schema\":",
        "{\"schema\":\"forged\",\"schema\":",
        1,
    );
    assert_eq!(
        verify(
            &duplicate_manifest,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.build_options,
        )
        .unwrap_err()
        .code,
        "SPX-PB506"
    );
}

#[test]
fn structurally_valid_outer_digest_drift_is_replay_not_wire_failure() {
    let fixture = fixture();
    let mut build = generate(
        &fixture.evidence,
        &fixture.input,
        &fixture.resolution_options,
        &fixture.build_options,
    )
    .unwrap();
    build.evidence_json =
        build
            .evidence_json
            .replacen("\"digest\":\"sha256:", "\"digest\":\"sha256:0", 1);
    assert_eq!(
        verify(
            &build,
            &fixture.evidence,
            &fixture.input,
            &fixture.resolution_options,
            &fixture.build_options,
        )
        .unwrap_err()
        .code,
        "SPX-PB507"
    );
}

#[test]
fn public_selection_and_authority_options_fail_closed() {
    let fixture = fixture();
    let unsorted = OfflinePackageBuildOptions::new(
        "pkg.root".to_owned(),
        vec!["pkg.main".to_owned(), "pkg.add".to_owned()],
        MAX_ARTIFACT_BYTES,
        MAX_EVIDENCE_BYTES,
    )
    .unwrap_err();
    assert_eq!(unsorted.code, "SPX-PB501");

    let mut widened = fixture.input;
    widened.allowed_capabilities = vec!["network.read".to_owned()];
    let widened_evidence = package_resolver::generate(&widened, &fixture.resolution_options)
        .expect("generate widened resolver fixture");
    assert_eq!(
        generate(
            &widened_evidence,
            &widened,
            &fixture.resolution_options,
            &fixture.build_options,
        )
        .unwrap_err()[0]
            .code,
        "SPX-PB504"
    );
}
