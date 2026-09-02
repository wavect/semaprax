use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::package_lock_v2::{self, Coordinate};
use semaprax::package_report_v2::{self, PackageReportV2Options};
use semaprax::package_resolution_snapshot::{self, ResolutionSnapshot};
use semaprax::package_resolver::{self, Requirement, ResolutionInput, ResolutionOptions};
use sha2::{Digest as _, Sha256};

const INPUT_DOMAIN: &[u8] = b"semaprax.offline-package-resolution-input.v1\0";
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn report(package: &str) -> String {
    report_from_source(
        package,
        &format!("module {package};\n@id(\"{package}.main\")\nfn main() -> i64 {{ 42 }}\n"),
    )
}

fn report_from_source(package: &str, source: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "semaprax-lock-snapshot-{}-{}-{package}.spx",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let report = package_report_v2::generate(&path, &PackageReportV2Options::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    report
}

fn coordinate(package: &str, version: &str) -> Coordinate {
    Coordinate {
        package: package.to_owned(),
        version: version.to_owned(),
    }
}

fn subject(
    report: &str,
    package: &str,
    version: &str,
    dependencies: &[Coordinate],
    capabilities: &[String],
) -> String {
    package_lock_v2::create_subject(
        &coordinate(package, version),
        report,
        dependencies,
        capabilities,
    )
    .unwrap()
}

fn input(
    requirements: &[(&str, &str)],
    subjects: Vec<String>,
    target: &str,
    allowed_capabilities: &[String],
) -> ResolutionInput {
    ResolutionInput {
        requirements: requirements
            .iter()
            .map(|(package, range)| Requirement {
                package: (*package).to_owned(),
                range: (*range).to_owned(),
            })
            .collect(),
        subjects,
        target: target.to_owned(),
        allowed_capabilities: allowed_capabilities.to_vec(),
    }
}

fn generate(input: &ResolutionInput) -> String {
    package_resolver::generate(input, &ResolutionOptions::default()).unwrap()
}

fn payload(envelope: &str) -> &str {
    let start = envelope.find("\"payload\":").unwrap() + "\"payload\":".len();
    &envelope[start..envelope.len() - 1]
}

fn remint(schema: &str, domain: &[u8], payload: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    format!(
        "{{\"schema\":\"{schema}\",\"digest\":\"sha256:{:x}\",\"bytes\":{},\"payload\":{payload}}}",
        semaprax::digest_hex::LowerHex(hasher.finalize()),
        payload.len()
    )
}

fn fixture(value: i64) -> (semaprax::package_resolver::ResolutionInput, String) {
    let report = report_from_source(
        "app.main",
        &format!("module app.main;\n@id(\"app.main.main\")\nfn main() -> i64 {{ {value} }}\n"),
    );
    let subject = subject(&report, "app.main", "1.0.0", &[], &[]);
    let input = input(&[("app.main", "=1.0.0")], vec![subject], "wasm32", &[]);
    let evidence = generate(&input);
    (input, evidence)
}

#[test]
fn exact_snapshot_replays_and_catalog_permutation_is_canonical() {
    let alpha_report = report("app.alpha");
    let beta_report = report("lib.beta");
    let beta = subject(&beta_report, "lib.beta", "1.0.0", &[], &[]);
    let alpha = subject(
        &alpha_report,
        "app.alpha",
        "1.0.0",
        &[coordinate("lib.beta", "1.0.0")],
        &[],
    );
    let first = input(
        &[("app.alpha", "=1.0.0")],
        vec![alpha.clone(), beta.clone()],
        "wasm32",
        &[],
    );
    let second = input(&[("app.alpha", "=1.0.0")], vec![beta, alpha], "wasm32", &[]);
    let first_evidence = generate(&first);
    let second_evidence = generate(&second);
    assert_eq!(first_evidence, second_evidence);
    let first_snapshot = package_resolution_snapshot::generate(
        &first,
        &ResolutionOptions::default(),
        &first_evidence,
    )
    .unwrap();
    let second_snapshot = package_resolution_snapshot::generate(
        &second,
        &ResolutionOptions::default(),
        &second_evidence,
    )
    .unwrap();
    assert_eq!(first_snapshot, second_snapshot);
    assert_eq!(
        package_resolution_snapshot::verify(&first_snapshot)
            .unwrap()
            .lock,
        first_snapshot.lock_json
    );
}

#[test]
fn every_snapshot_member_is_exact_and_cross_pairs_fail_closed() {
    let (first_input, first_evidence) = fixture(41);
    let (second_input, second_evidence) = fixture(42);
    let first = package_resolution_snapshot::generate(
        &first_input,
        &ResolutionOptions::default(),
        &first_evidence,
    )
    .unwrap();
    let second = package_resolution_snapshot::generate(
        &second_input,
        &ResolutionOptions::default(),
        &second_evidence,
    )
    .unwrap();

    for field in 0..3 {
        let mut changed = first.clone();
        let bytes = match field {
            0 => &mut changed.input_json,
            1 => &mut changed.resolution_evidence_json,
            _ => &mut changed.lock_json,
        };
        bytes.push(' ');
        assert!(package_resolution_snapshot::verify(&changed).is_err());

        let mut truncated = first.clone();
        let bytes = match field {
            0 => &mut truncated.input_json,
            1 => &mut truncated.resolution_evidence_json,
            _ => &mut truncated.lock_json,
        };
        bytes.pop();
        assert!(package_resolution_snapshot::verify(&truncated).is_err());
    }

    let crossed = ResolutionSnapshot {
        input_json: first.input_json.clone(),
        resolution_evidence_json: second.resolution_evidence_json.clone(),
        lock_json: second.lock_json.clone(),
    };
    assert_eq!(
        package_resolution_snapshot::verify(&crossed)
            .unwrap_err()
            .code,
        "SPX-PK505"
    );
}

#[test]
fn reminted_noncanonical_and_hostile_raw_subjects_are_rejected() {
    let (input, evidence) = fixture(41);
    let snapshot =
        package_resolution_snapshot::generate(&input, &ResolutionOptions::default(), &evidence)
            .unwrap();
    let payload = payload(&snapshot.input_json);

    let inserted = payload.replacen("\"target\":", "\"unknown\":0,\"target\":", 1);
    let hostile = ResolutionSnapshot {
        input_json: remint(
            package_resolution_snapshot::INPUT_SCHEMA,
            INPUT_DOMAIN,
            &inserted,
        ),
        ..snapshot.clone()
    };
    assert_eq!(
        package_resolution_snapshot::verify(&hostile)
            .unwrap_err()
            .code,
        "SPX-PK504"
    );

    let subject_marker = "\"subjects\":[{";
    let duplicate_subject_key = payload.replacen(
        subject_marker,
        "\"subjects\":[{\"schema\":\"duplicate\",",
        1,
    );
    let hostile = ResolutionSnapshot {
        input_json: remint(
            package_resolution_snapshot::INPUT_SCHEMA,
            INPUT_DOMAIN,
            &duplicate_subject_key,
        ),
        ..snapshot.clone()
    };
    assert!(package_resolution_snapshot::verify(&hostile).is_err());

    let nested = format!(
        "{}0{}",
        "[".repeat(semaprax::package_lock_v2::MAX_JSON_DEPTH + 9),
        "]".repeat(semaprax::package_lock_v2::MAX_JSON_DEPTH + 9)
    );
    let deep_subject = payload.replacen(
        subject_marker,
        &format!("{subject_marker}\"deep\":{nested},"),
        1,
    );
    let hostile = ResolutionSnapshot {
        input_json: remint(
            package_resolution_snapshot::INPUT_SCHEMA,
            INPUT_DOMAIN,
            &deep_subject,
        ),
        ..snapshot.clone()
    };
    assert_eq!(
        package_resolution_snapshot::verify(&hostile)
            .unwrap_err()
            .code,
        "SPX-PK504"
    );

    let trailing_subject = payload.replacen(
        "}],\"resolution_max_bytes\":",
        "}0],\"resolution_max_bytes\":",
        1,
    );
    let hostile = ResolutionSnapshot {
        input_json: remint(
            package_resolution_snapshot::INPUT_SCHEMA,
            INPUT_DOMAIN,
            &trailing_subject,
        ),
        ..snapshot.clone()
    };
    assert_eq!(
        package_resolution_snapshot::verify(&hostile)
            .unwrap_err()
            .code,
        "SPX-PK504"
    );

    let whitespace = payload.replacen("\"subjects\":[", "\"subjects\":[ ", 1);
    let hostile = ResolutionSnapshot {
        input_json: remint(
            package_resolution_snapshot::INPUT_SCHEMA,
            INPUT_DOMAIN,
            &whitespace,
        ),
        ..snapshot
    };
    assert!(package_resolution_snapshot::verify(&hostile).is_err());
}

#[test]
fn cumulative_and_component_plus_one_bounds_fail_before_replay() {
    let oversized = ResolutionSnapshot {
        input_json: "x".repeat(package_resolution_snapshot::MAX_INPUT_BYTES + 1),
        resolution_evidence_json: String::new(),
        lock_json: String::new(),
    };
    assert_eq!(
        package_resolution_snapshot::verify(&oversized)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
    let oversized = ResolutionSnapshot {
        input_json: String::new(),
        resolution_evidence_json: "x".repeat(semaprax::package_resolver::MAX_OUTPUT_BYTES + 1),
        lock_json: String::new(),
    };
    assert_eq!(
        package_resolution_snapshot::verify(&oversized)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
    let oversized = ResolutionSnapshot {
        input_json: String::new(),
        resolution_evidence_json: String::new(),
        lock_json: "x".repeat(semaprax::package_lock_v2::MAX_OUTPUT_BYTES + 1),
    };
    assert_eq!(
        package_resolution_snapshot::verify(&oversized)
            .unwrap_err()
            .code,
        "SPX-PK503"
    );
}
