//! Caller-declared deployment-contract coverage evidence, authored and unrun.

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{with_authenticated_project, ProjectCandidate};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DECLARATION_SCHEMA: &str = "semaprax.project-candidate-deployment-contract-declaration.v1";
const EVIDENCE_SCHEMA: &str = "semaprax.project-candidate-deployment-contract-evidence.v1";
const DOMAIN: &[u8] = b"semaprax.project-candidate-deployment-contract-declaration.v1\0";

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-deployment-contract-evidence-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }

    fn source_bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn canonical(mut value: Value) -> Vec<u8> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    bytes
}

fn digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

fn declaration(candidate: &ProjectCandidate, exports: Value) -> (Vec<u8>, String) {
    let bytes = canonical(json!({
        "schema":DECLARATION_SCHEMA,
        "candidate_revision":candidate.candidate_digest(),
        "manifest_exports":exports,
        "configuration":[
            {"key":"API_BASE_URL","type":"string","required":true},
            {"key":"DEPLOYMENT_REGION","type":"string","required":false},
            {"key":"MAX_RETRIES","type":"integer","required":true}
        ]
    }));
    let digest = digest(&bytes);
    (bytes, digest)
}

fn area<'a>(value: &'a Value, name: &str) -> &'a Value {
    let rows = value["areas"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["area"] == name)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    rows[0]
}

fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    let diagnostics = result.err().expect("hostile declaration was admitted");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected),
        "{diagnostics:?}"
    );
}

#[test]
fn canonical_declaration_marks_only_deployment_configuration_partial() {
    let fixture = Fixture::new();
    let disk = fixture.source_bytes();
    let candidate = fixture.candidate();
    let candidate_json = candidate.to_json().to_owned();
    let exports = json!(candidate.revision().manifest().web_exports());
    let (bytes, declaration_digest) = declaration(&candidate, exports.clone());
    let report: Value = serde_json::from_str(
        &candidate
            .analysis_deployment_contract_evidence(
                candidate.candidate_digest(),
                &bytes,
                &declaration_digest,
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(report["schema"], EVIDENCE_SCHEMA);
    assert_eq!(report["candidate_revision"], candidate.candidate_digest());
    assert_eq!(
        area(&report, "deployment_configuration")["status"],
        "partial"
    );
    for name in [
        "generated_file_provenance",
        "generated_artifacts",
        "external_api_behavior",
        "runtime_environment",
        "external_consumers",
    ] {
        assert_eq!(area(&report, name)["status"], "not_inspected");
    }
    let attachment = &report["deployment_contract_declaration"];
    assert_eq!(attachment["digest"], declaration_digest);
    assert_eq!(attachment["bytes"], bytes.len());
    assert_eq!(
        attachment["canonical_json"],
        std::str::from_utf8(&bytes).unwrap()
    );
    assert_eq!(attachment["manifest_exports"], exports);
    assert_eq!(attachment["environment_observation"], false);
    assert_eq!(attachment["deployment_authority"], false);
    assert_eq!(report["source_authority"], false);
    assert_eq!(report["external_io"], false);
    assert_eq!(report["execution"], false);
    assert!(report["nonclaims"].as_array().unwrap().contains(&json!(
        "no_filesystem_network_secret_input_or_locator_authority"
    )));
    let blind = report["blind_spots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["domain"] == "deployment_configuration")
        .unwrap();
    assert_eq!(blind["evidence_status"], "partial");
    assert_eq!(candidate.to_json(), candidate_json);
    assert_eq!(fixture.source_bytes(), disk);
}

#[test]
fn declaration_digest_candidate_and_canonical_bytes_fail_closed_on_tamper() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    let exports = json!(candidate.revision().manifest().web_exports());
    let (bytes, declaration_digest) = declaration(&candidate, exports.clone());

    let mut changed = bytes.clone();
    let position = changed.iter().position(|byte| *byte == b'A').unwrap();
    changed[position] = b'B';
    code(
        candidate.analysis_deployment_contract_evidence(
            candidate.candidate_digest(),
            &changed,
            &declaration_digest,
        ),
        "SPX-G426",
    );

    let mut value: Value = serde_json::from_slice(&bytes).unwrap();
    value["candidate_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    let rebound = canonical(value);
    code(
        candidate.analysis_deployment_contract_evidence(
            candidate.candidate_digest(),
            &rebound,
            &digest(&rebound),
        ),
        "SPX-G426",
    );

    let mut noncanonical = bytes.clone();
    noncanonical.insert(0, b' ');
    code(
        candidate.analysis_deployment_contract_evidence(
            candidate.candidate_digest(),
            &noncanonical,
            &digest(&noncanonical),
        ),
        "SPX-G424",
    );
}

#[test]
fn unknown_or_repeated_export_never_becomes_declared_deployment_evidence() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate();
    for exports in [
        json!(["unknown.external.export"]),
        json!([
            candidate.revision().manifest().web_exports()[0],
            candidate.revision().manifest().web_exports()[0]
        ]),
    ] {
        let (bytes, declaration_digest) = declaration(&candidate, exports);
        code(
            candidate.analysis_deployment_contract_evidence(
                candidate.candidate_digest(),
                &bytes,
                &declaration_digest,
            ),
            "SPX-G426",
        );
    }
}
