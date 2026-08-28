use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use semaprax::{
    format, parse, semantic_workspace, semantic_workspace_change, workspace, workspace_graph,
};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    proposal: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-workspace-change-v1-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        for (path, source) in [
            ("a/provider.spx", provider_base()),
            ("m/consumer.spx", consumer_source()),
            ("z/entry.spx", entry_base()),
        ] {
            write_source(&root, path, source);
        }
        let path_set = root.join("paths.json");
        std::fs::write(
            &path_set,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"m/consumer.spx\"},{\"path\":\"z/entry.spx\"}]}\n",
        )
        .unwrap();
        semantic_workspace::initialize(&root, &path_set).unwrap();
        let proposal = root.join("change.json");
        std::fs::write(&proposal, proposal_source(&root)).unwrap();
        Self { root, proposal }
    }

    fn lock(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.root.join(".semaprax-workspace/LOCK"))
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn canonical(source: &str, path: &str) -> String {
    format::canonical(&parse(source, path).unwrap())
}

fn write_source(root: &Path, path: &str, source: &str) {
    let destination = root.join(path);
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    std::fs::write(destination, canonical(source, path)).unwrap();
}

fn json(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

fn proposal_source(root: &Path) -> String {
    let graph = workspace_graph::snapshot(root, "change.entry").unwrap();
    let replacement = [
        (
            "a/provider.spx",
            canonical(provider_candidate(), "a/provider.spx"),
        ),
        ("z/entry.spx", canonical(entry_candidate(), "z/entry.spx")),
    ];
    let mut changes = String::new();
    for (index, (path, source)) in replacement.iter().enumerate() {
        if index != 0 {
            changes.push(',');
        }
        let module = graph
            .modules()
            .iter()
            .find(|module| module.path() == *path)
            .unwrap();
        changes.push_str(&format!(
            "{{\"path\":{},\"base_source_graph_schema\":{},\"base_source_revision\":{},\"base_source_digest\":{},\"replacement_source\":{}}}",
            json(path),
            json(module.source_graph_schema()),
            json(module.source_revision()),
            json(module.source_digest()),
            json(source),
        ));
    }
    format!(
        "{{\"schema\":\"semaprax.workspace-semantic-change.v1\",\"base_workspace_revision\":{},\"entry_module\":\"change.entry\",\"changes\":[{changes}]}}\n",
        json(graph.workspace_revision())
    )
}

fn provider_base() -> &'static str {
    r#"
module change.provider;
permit { audit.old }
@id("change.point") record Point { @id("change.point.value") value: i64, }
@id("change.work") fn work(value: Point) -> i64 uses { audit.old } { value.value }
fn helper() -> i64 { 1 }
@id("change.provider.main") fn main() -> i64 { helper() }
"#
}

fn consumer_source() -> &'static str {
    r#"
module change.consumer;
use type @id("change.point") from change.provider as Point;
use function @id("change.work") from change.provider as work;
permit { audit.old, audit.new }
@id("change.consume") fn consume() -> i64 uses { audit.old, audit.new } { work(Point { value: 3 }) }
@id("change.consumer.main") fn main() -> i64 uses { audit.old, audit.new } { consume() }
"#
}

fn entry_base() -> &'static str {
    r#"
module change.entry;
use type @id("change.point") from change.provider as Point;
use function @id("change.work") from change.provider as work;
use function @id("change.consume") from change.consumer as consume;
permit { audit.old, audit.new }
@id("change.entry.main") fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 1 }) }
"#
}

fn provider_candidate() -> &'static str {
    r#"
module change.provider;
permit { audit.new }
@id("change.point") record Metric { @id("change.point.value") value: i64, }
@id("change.work") fn task(value: Metric) -> i64 uses { audit.new } { value.value + 1 }
fn helper() -> i64 { 2 }
@id("change.provider.main") fn main() -> i64 { helper() + 1 }
"#
}

fn entry_candidate() -> &'static str {
    r#"
module change.entry;
use type @id("change.point") from change.provider as Metric;
use function @id("change.work") from change.provider as task;
use function @id("change.consume") from change.consumer as consume;
permit { audit.old, audit.new }
@id("change.entry.main") fn main() -> i64 uses { audit.new } { task(Metric { value: 2 }) }
"#
}

fn inventory(root: &Path) -> Vec<(String, &'static str, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, output: &mut Vec<(String, &'static str, Vec<u8>)>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            if metadata.is_dir() {
                output.push((relative, "directory", Vec::new()));
                visit(root, &path, output);
            } else if metadata.is_file() {
                output.push((relative, "file", std::fs::read(path).unwrap()));
            } else {
                output.push((relative, "other", Vec::new()));
            }
        }
    }
    let mut output = Vec::new();
    visit(root, root, &mut output);
    output
}

fn raw_sha(source: &str) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn raw_sources(root: &Path) -> Vec<Vec<u8>> {
    ["a/provider.spx", "m/consumer.spx", "z/entry.spx"]
        .into_iter()
        .map(|path| std::fs::read(root.join(path)).unwrap())
        .collect()
}

#[test]
fn public_api_cli_kat_parity_and_opaque_getters() {
    let fixture = Fixture::new("api-cli");
    let before = inventory(&fixture.root);
    let artifacts = semantic_workspace_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let preview = semantic_workspace_change::preview(&fixture.root, &fixture.proposal).unwrap();
    let evidence = semantic_workspace_change::evidence(&fixture.root, &fixture.proposal).unwrap();
    assert_eq!(preview, artifacts.preview());
    assert_eq!(evidence, artifacts.evidence());
    assert!(preview.ends_with('\n') && evidence.ends_with('\n'));
    assert_eq!(
        raw_sha(&preview),
        "sha256:973f9c540e7bcb4ba7536815667fb638a411ee04c4f3a3b9675849cc0b69e643"
    );
    assert_eq!(
        raw_sha(&evidence),
        "sha256:7f9b62ddf9d577e7e82271831659319c1e9fb0a94370a0a36897fbc70d63e747"
    );
    for value in [
        artifacts.proposal_digest(),
        artifacts.candidate_manifest_digest(),
        artifacts.preview_digest(),
        artifacts.context_digest(),
        artifacts.impact_digest(),
        artifacts.review_digest(),
        artifacts.evidence_digest(),
    ] {
        assert!(value.starts_with("sha256:") && value.len() == 71);
    }
    assert!(!artifacts.context().is_empty());
    assert!(!artifacts.impact().is_empty());
    assert!(!artifacts.review().is_empty());
    let evidence_json: serde_json::Value = serde_json::from_str(&evidence).unwrap();
    assert_eq!(
        evidence_json["change_preview"]["digest"],
        artifacts.preview_digest()
    );
    assert_eq!(
        evidence_json["context"]["digest"],
        artifacts.context_digest()
    );
    assert_eq!(evidence_json["impact"]["digest"], artifacts.impact_digest());
    assert_eq!(evidence_json["review"]["digest"], artifacts.review_digest());
    assert!(evidence_json["nonclaims"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "no_commit_authority_in_preview_context_impact_review_or_evidence"));

    for (command, expected) in [
        ("semantic-workspace-change-preview", preview.as_str()),
        ("semantic-workspace-change-evidence", evidence.as_str()),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg(command)
            .arg(&fixture.root)
            .arg(&fixture.proposal)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
    }
    assert_eq!(inventory(&fixture.root), before);
}

#[test]
fn cli_arity_and_proposal_io_are_exact() {
    let fixture = Fixture::new("cli-errors");
    for command in [
        "semantic-workspace-change-preview",
        "semantic-workspace-change-evidence",
    ] {
        for arguments in [vec![command], vec![command, "root", "proposal", "extra"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
                .args(arguments)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert_eq!(
                String::from_utf8(output.stderr).unwrap(),
                format!("{command} requires exactly <root> <proposal.json>\n")
            );
        }
    }
    let missing = fixture.root.join("missing.json");
    let error = semantic_workspace_change::preview(&fixture.root, &missing).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-I214");
    assert_eq!(
        error[0].message,
        "could not read Semantic Workspace Change proposal: open failed"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("semantic-workspace-change-preview")
        .arg(&fixture.root)
        .arg(missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "error[SPX-I214]: could not read Semantic Workspace Change proposal: open failed\n"
    );
}

#[test]
fn shared_lock_no_write_and_immediate_exclusive_reacquire() {
    let fixture = Fixture::new("shared-lock");
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    FileExt::lock_shared(&shared).unwrap();
    let competing = fixture.lock();
    assert!(competing.try_lock_exclusive().is_err());
    let artifacts = semantic_workspace_change::generate(&fixture.root, &fixture.proposal).unwrap();
    assert!(!artifacts.preview().is_empty());
    assert_eq!(inventory(&fixture.root), before);
    FileExt::unlock(&shared).unwrap();
    competing.try_lock_exclusive().unwrap();
    FileExt::unlock(&competing).unwrap();
}

#[test]
fn ordinary_workspace_mode_is_rejected_without_writes() {
    let fixture = Fixture::new("ordinary-proposal-source");
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let ordinary = std::env::temp_dir().join(format!(
        "semaprax-semantic-workspace-change-ordinary-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir(&ordinary).unwrap();
    for (path, source) in [
        (
            "a/provider.spx",
            "module ordinary.provider; @id(\"ordinary.provider.main\") fn main()->i64{0}",
        ),
        (
            "z/app.spx",
            "module ordinary.app; @id(\"ordinary.app.main\") fn main()->i64{1}",
        ),
    ] {
        write_source(&ordinary, path, source);
    }
    let path_set = ordinary.join("paths.json");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
    )
    .unwrap();
    workspace::initialize(&ordinary, &path_set).unwrap();
    let before = inventory(&ordinary);
    let error = semantic_workspace_change::preview(&ordinary, &fixture.proposal).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G174");
    assert_eq!(
        error[0].message,
        "managed workspace is not a semaprax.workspace-semantic-root.v1 workspace"
    );
    assert_eq!(inventory(&ordinary), before);
    std::fs::remove_dir_all(&ordinary).unwrap();
}

#[test]
fn hostile_proposal_inputs_fail_exactly_and_release_the_shared_lock() {
    let fixture = Fixture::new("hostile-proposal-inputs");
    let directory = fixture.root.join("proposal-directory");
    std::fs::create_dir(&directory).unwrap();
    let invalid_utf8 = fixture.root.join("proposal-invalid-utf8.json");
    std::fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
    let sparse_over = fixture.root.join("proposal-sparse-over.json");
    File::create(&sparse_over)
        .unwrap()
        .set_len(33_554_433)
        .unwrap();
    #[cfg(windows)]
    let directory_message = "could not read Semantic Workspace Change proposal: open failed";
    #[cfg(not(windows))]
    let directory_message =
        "could not read Semantic Workspace Change proposal: input is not a regular file";

    for (path, code, message) in [
        (directory.as_path(), "SPX-I214", directory_message),
        (
            invalid_utf8.as_path(),
            "SPX-I214",
            "could not read Semantic Workspace Change proposal: input is not UTF-8",
        ),
        (
            sparse_over.as_path(),
            "SPX-G183",
            "Semantic Workspace Change `proposal_bytes` exceeds 33554432",
        ),
    ] {
        for api in 0..3 {
            let before = inventory(&fixture.root);
            let result = match api {
                0 => semantic_workspace_change::generate(&fixture.root, path).map(|_| ()),
                1 => semantic_workspace_change::preview(&fixture.root, path).map(|_| ()),
                2 => semantic_workspace_change::evidence(&fixture.root, path).map(|_| ()),
                _ => unreachable!(),
            };
            let error = result.unwrap_err();
            assert_eq!(error.len(), 1);
            assert_eq!(error[0].code, code);
            assert_eq!(error[0].message, message);
            assert_eq!(inventory(&fixture.root), before);
            let lock = fixture.lock();
            lock.try_lock_exclusive().unwrap();
            FileExt::unlock(&lock).unwrap();
        }
    }
}

fn replace_digest_character(source: &str, field: &str) -> String {
    let field_index = source.find(field).unwrap();
    let digest_index = field_index + source[field_index..].find("sha256:").unwrap() + 7;
    let mut bytes = source.as_bytes().to_vec();
    bytes[digest_index] = if bytes[digest_index] == b'0' {
        b'1'
    } else {
        b'0'
    };
    String::from_utf8(bytes).unwrap()
}

#[test]
fn verification_receipt_api_cli_kat_shared_lock_and_no_write() {
    let fixture = Fixture::new("verification-receipt");
    let artifacts = semantic_workspace_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    FileExt::lock_shared(&shared).unwrap();
    let competing = fixture.lock();
    assert!(competing.try_lock_exclusive().is_err());
    let receipt =
        semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &evidence_path)
            .unwrap();
    assert!(receipt.ends_with('\n'));
    assert!(!receipt[..receipt.len() - 1].contains('\n'));
    assert_eq!(
        raw_sha(&receipt),
        "sha256:f66fea1b1873dbae8418c67f8612a7698907ec15cdfde7c5a2cbdf82fe53b196"
    );
    let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.workspace-semantic-change-evidence-verification.v1"
    );
    assert_eq!(value["result"], "exact_replay");
    assert_eq!(value["proposal"]["digest"], artifacts.proposal_digest());
    assert_eq!(
        value["change_preview"]["digest"],
        artifacts.preview_digest()
    );
    assert_eq!(value["context"]["digest"], artifacts.context_digest());
    assert_eq!(value["impact"]["digest"], artifacts.impact_digest());
    assert_eq!(value["review"]["digest"], artifacts.review_digest());
    assert_eq!(
        value["workspace_change_evidence"]["digest"],
        artifacts.evidence_digest()
    );
    assert_eq!(
        value["workspace_change_evidence"]["bytes"],
        artifacts.evidence().len()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("verify-semantic-workspace-change-evidence")
        .arg(&fixture.root)
        .arg(&fixture.proposal)
        .arg(&evidence_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, receipt.as_bytes());
    assert_eq!(inventory(&fixture.root), before);
    FileExt::unlock(&shared).unwrap();
    competing.try_lock_exclusive().unwrap();
    FileExt::unlock(&competing).unwrap();
}

#[test]
fn evidence_parser_replay_confusion_and_read_hostiles_fail_closed() {
    let fixture = Fixture::new("verification-hostiles");
    let evidence = semantic_workspace_change::evidence(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, &evidence).unwrap();
    let receipt =
        semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &evidence_path)
            .unwrap();

    for arguments in [
        vec!["verify-semantic-workspace-change-evidence"],
        vec![
            "verify-semantic-workspace-change-evidence",
            "root",
            "proposal",
            "evidence",
            "extra",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "verify-semantic-workspace-change-evidence requires exactly <root> <proposal.json> <evidence.json>\n"
        );
    }

    for (proposal, submitted, message) in [
        (
            fixture.root.join("missing-proposal.json"),
            evidence_path.clone(),
            "could not read Semantic Workspace Change proposal: open failed",
        ),
        (
            fixture.proposal.clone(),
            fixture.root.join("missing-evidence.json"),
            "could not read Semantic Workspace Change Evidence: open failed",
        ),
    ] {
        let error =
            semantic_workspace_change::verify(&fixture.root, &proposal, &submitted).unwrap_err();
        assert_eq!(error[0].code, "SPX-I214");
        assert_eq!(error[0].message, message);
        let lock = fixture.lock();
        lock.try_lock_exclusive().unwrap();
        FileExt::unlock(&lock).unwrap();
    }

    let body = evidence.trim_end_matches('\n');
    let first = body.find(',').unwrap();
    let second = first + 1 + body[first + 1..].find(',').unwrap();
    let reordered = format!(
        "{{{},{},{}\n",
        &body[first + 1..second],
        &body[1..first],
        &body[second + 1..]
    );
    let format_cases = [
        evidence.trim_end_matches('\n').to_owned(),
        evidence.replace('\n', "\r\n"),
        format!("\u{feff}{evidence}"),
        evidence.replacen(
            "{\"schema\":",
            "{\"extra\":0,\"schema\":",
            1,
        ),
        evidence.replacen(
            "{\"schema\":\"semaprax.workspace-semantic-change-evidence.v1\",",
            "{\"schema\":\"semaprax.workspace-semantic-change-evidence.v1\",\"schema\":\"semaprax.workspace-semantic-change-evidence.v1\",",
            1,
        ),
        evidence.replacen(",\"entry_module\":\"change.entry\"", "", 1),
        reordered,
    ];
    for (index, hostile) in format_cases.into_iter().enumerate() {
        let path = fixture.root.join(format!("format-{index}.json"));
        std::fs::write(&path, hostile).unwrap();
        let error =
            semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &path).unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G185");
    }

    for (index, hostile) in [
        evidence.replace(
            "\"entry_module\":\"change.entry\"",
            "\"entry_module\":\"change.entra\"",
        ),
        replace_digest_character(&evidence, "\"proposal\""),
        replace_digest_character(&evidence, "\"change_preview\""),
        replace_digest_character(&evidence, "\"candidate_source_digest\""),
        evidence.replacen("\"max_managed_files\":16", "\"max_managed_files\":15", 1),
        evidence.replacen(
            "not_signature_or_authenticated_provenance",
            "not_signature_or_authenticated_provenancf",
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let path = fixture.root.join(format!("replay-{index}.json"));
        std::fs::write(&path, hostile).unwrap();
        let error =
            semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &path).unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G187");
    }

    let receipt_path = fixture.root.join("receipt-as-evidence.json");
    std::fs::write(&receipt_path, receipt).unwrap();
    let error = semantic_workspace_change::verify(&fixture.root, &fixture.proposal, &receipt_path)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G185");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Change Evidence must be one canonical JSON line with one terminal LF: receipt and capsule schemas are confused"
    );

    let directory = fixture.root.join("evidence-directory");
    std::fs::create_dir(&directory).unwrap();
    let invalid = fixture.root.join("evidence-invalid.json");
    std::fs::write(&invalid, [0xff]).unwrap();
    let sparse = fixture.root.join("evidence-sparse.json");
    File::create(&sparse).unwrap().set_len(1_048_577).unwrap();
    #[cfg(windows)]
    let directory_message = "could not read Semantic Workspace Change Evidence: open failed";
    #[cfg(not(windows))]
    let directory_message =
        "could not read Semantic Workspace Change Evidence: input is not a regular file";
    for (path, code, message) in [
        (directory.as_path(), "SPX-I214", directory_message),
        (
            invalid.as_path(),
            "SPX-I214",
            "could not read Semantic Workspace Change Evidence: input is not UTF-8",
        ),
        (
            sparse.as_path(),
            "SPX-G183",
            "Semantic Workspace Change `evidence_bytes` exceeds 1048576",
        ),
    ] {
        let error =
            semantic_workspace_change::verify(&fixture.root, &fixture.proposal, path).unwrap_err();
        assert_eq!(error[0].code, code);
        assert_eq!(error[0].message, message);
        let lock = fixture.lock();
        lock.try_lock_exclusive().unwrap();
        FileExt::unlock(&lock).unwrap();
    }
}

#[test]
fn application_receipt_api_cli_kat_fixed_point_and_raw_no_write() {
    let fixture = Fixture::new("application-receipt-api");
    let raw_before = raw_sources(&fixture.root);
    let artifacts = semantic_workspace_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let receipt =
        semantic_workspace_change::apply(&fixture.root, &fixture.proposal, &evidence_path).unwrap();
    assert!(receipt.ends_with('\n'));
    assert!(!receipt[..receipt.len() - 1].contains('\n'));
    assert_eq!(
        raw_sha(&receipt),
        "sha256:a7b4d68e459d54bd59c9b515dd46e5b406d86e7f27c3cef8a89cc61e39095650"
    );
    let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.workspace-semantic-change-evidence-application.v1"
    );
    assert_eq!(value["result"], "applied");
    assert_eq!(value["proposal"]["digest"], artifacts.proposal_digest());
    assert_eq!(
        value["workspace_change_evidence"]["digest"],
        artifacts.evidence_digest()
    );
    assert_eq!(
        value["workspace_change_evidence"]["bytes"],
        artifacts.evidence().len()
    );
    assert_eq!(value["budget"]["used_receipt_bytes"], receipt.len());
    let used_total = value["budget"]["used_proposal_bytes"].as_u64().unwrap()
        + value["budget"]["used_change_preview_bytes"]
            .as_u64()
            .unwrap()
        + value["budget"]["used_context_bytes"].as_u64().unwrap()
        + value["budget"]["used_impact_bytes"].as_u64().unwrap()
        + value["budget"]["used_review_bytes"].as_u64().unwrap()
        + value["budget"]["used_evidence_bytes"].as_u64().unwrap()
        + value["budget"]["used_receipt_bytes"].as_u64().unwrap();
    assert_eq!(value["budget"]["used_total_artifact_bytes"], used_total);
    assert_eq!(raw_sources(&fixture.root), raw_before);
    let lock = fixture.lock();
    lock.try_lock_exclusive().unwrap();
    FileExt::unlock(&lock).unwrap();

    let cli = Fixture::new("application-receipt-cli");
    let cli_raw_before = raw_sources(&cli.root);
    let cli_evidence = semantic_workspace_change::evidence(&cli.root, &cli.proposal).unwrap();
    let cli_evidence_path = cli.root.join("evidence.json");
    std::fs::write(&cli_evidence_path, cli_evidence).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("apply-semantic-workspace-change-evidence")
        .arg(&cli.root)
        .arg(&cli.proposal)
        .arg(&cli_evidence_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, receipt.as_bytes());
    assert_eq!(raw_sources(&cli.root), cli_raw_before);
    let lock = cli.lock();
    lock.try_lock_exclusive().unwrap();
    FileExt::unlock(&lock).unwrap();
}

#[test]
fn application_cli_arity_contention_and_receipt_confusion_are_fail_closed() {
    for arguments in [
        vec!["apply-semantic-workspace-change-evidence"],
        vec![
            "apply-semantic-workspace-change-evidence",
            "root",
            "proposal",
            "evidence",
            "extra",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "apply-semantic-workspace-change-evidence requires exactly <root> <proposal.json> <evidence.json>\n"
        );
    }

    let fixture = Fixture::new("application-contention");
    let evidence = semantic_workspace_change::evidence(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, evidence).unwrap();
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    FileExt::lock_shared(&shared).unwrap();
    let error = semantic_workspace_change::apply(&fixture.root, &fixture.proposal, &evidence_path)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-I210");
    assert_eq!(error[0].message, "workspace LOCK is busy");
    assert_eq!(inventory(&fixture.root), before);
    FileExt::unlock(&shared).unwrap();
    let lock = fixture.lock();
    lock.try_lock_exclusive().unwrap();
    FileExt::unlock(&lock).unwrap();

    let confusion = Fixture::new("application-receipt-confusion");
    let evidence =
        semantic_workspace_change::evidence(&confusion.root, &confusion.proposal).unwrap();
    let evidence_path = confusion.root.join("evidence.json");
    std::fs::write(&evidence_path, &evidence).unwrap();
    let verification =
        semantic_workspace_change::verify(&confusion.root, &confusion.proposal, &evidence_path)
            .unwrap();
    let receipt_path = confusion.root.join("verification-receipt.json");
    std::fs::write(&receipt_path, verification).unwrap();
    let before = inventory(&confusion.root);
    let error =
        semantic_workspace_change::apply(&confusion.root, &confusion.proposal, &receipt_path)
            .unwrap_err();
    assert_eq!(error[0].code, "SPX-G185");
    assert_eq!(inventory(&confusion.root), before);
    let lock = confusion.lock();
    lock.try_lock_exclusive().unwrap();
    FileExt::unlock(&lock).unwrap();
}

#[test]
fn second_application_is_stale_and_preserves_the_committed_generation() {
    let fixture = Fixture::new("application-stale-second");
    let evidence = semantic_workspace_change::evidence(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, evidence).unwrap();
    semantic_workspace_change::apply(&fixture.root, &fixture.proposal, &evidence_path).unwrap();
    let committed = inventory(&fixture.root);
    let error = semantic_workspace_change::apply(&fixture.root, &fixture.proposal, &evidence_path)
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-G182");
    assert_eq!(inventory(&fixture.root), committed);
    let lock = fixture.lock();
    lock.try_lock_exclusive().unwrap();
    FileExt::unlock(&lock).unwrap();
}
