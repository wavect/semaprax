use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use semaprax::{
    diagnostic::Diagnostic, format, parse, semantic_workspace,
    semantic_workspace_structural_change, workspace, workspace_graph,
};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const MAX_PROPOSAL_BYTES: u64 = 33_554_432;
const MAX_EVIDENCE_BYTES: u64 = 1_048_576;

struct Fixture {
    root: PathBuf,
    proposal: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-workspace-structural-change-v1-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        for (path, source) in [
            ("a/provider.spx", provider()),
            ("m/consumer.spx", consumer()),
            ("n/island.spx", island()),
            ("z/entry.spx", entry()),
        ] {
            write_source(&root, path, source);
        }
        let path_set = root.join("paths.json");
        std::fs::write(
            &path_set,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"m/consumer.spx\"},{\"path\":\"n/island.spx\"},{\"path\":\"z/entry.spx\"}]}\n",
        )
        .unwrap();
        semantic_workspace::initialize(&root, &path_set).unwrap();
        let proposal = root.join("structural-change.json");
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

    fn assert_exclusive_reacquire(&self) {
        let lock = self.lock();
        FileExt::try_lock_exclusive(&lock).unwrap();
        FileExt::unlock(&lock).unwrap();
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

fn binding(root: &Path, entry: &str, path: &str) -> String {
    let graph = workspace_graph::snapshot(root, entry).unwrap();
    let module = graph
        .modules()
        .iter()
        .find(|module| module.path() == path)
        .unwrap();
    format!(
        ",\"base_source_graph_schema\":{},\"base_source_revision\":{},\"base_source_digest\":{}",
        json(module.source_graph_schema()),
        json(module.source_revision()),
        json(module.source_digest())
    )
}

fn proposal_source(root: &Path) -> String {
    let graph = workspace_graph::snapshot(root, "structural.entry").unwrap();
    let operations = [
        format!(
            "{{\"kind\":\"create\",\"path\":\"b/created.spx\",\"source\":{}}}",
            json(&canonical(created(), "b/created.spx"))
        ),
        format!(
            "{{\"kind\":\"delete\",\"path\":\"n/island.spx\"{}}}",
            binding(root, "structural.island", "n/island.spx")
        ),
        format!(
            "{{\"kind\":\"move\",\"from_path\":\"a/provider.spx\",\"to_path\":\"c/provider.spx\"{}}}",
            binding(root, "structural.entry", "a/provider.spx")
        ),
        format!(
            "{{\"kind\":\"replace\",\"path\":\"z/entry.spx\"{},\"replacement_source\":{}}}",
            binding(root, "structural.entry", "z/entry.spx"),
            json(&canonical(entry_replacement(), "z/entry.spx"))
        ),
    ];
    format!(
        "{{\"schema\":\"semaprax.workspace-semantic-structural-change.v1\",\"base_workspace_revision\":{},\"entry_module\":\"structural.entry\",\"operations\":[{}]}}\n",
        json(graph.workspace_revision()),
        operations.join(",")
    )
}

fn provider() -> &'static str {
    r#"
module structural.provider;
permit { audit.old }
@id("structural.point") record Point { @id("structural.point.value") value: i64, }
@id("structural.work") fn work(value: Point) -> i64 uses { audit.old } { value.value }
fn helper() -> i64 { 1 }
@id("structural.provider.main") fn main() -> i64 { helper() }
"#
}

fn consumer() -> &'static str {
    r#"
module structural.consumer;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
permit { audit.old, audit.new }
@id("structural.consume") fn consume() -> i64 uses { audit.old, audit.new } { work(Point { value: 3 }) }
@id("structural.consumer.main") fn main() -> i64 uses { audit.old, audit.new } { consume() }
"#
}

fn island() -> &'static str {
    r#"
module structural.island;
permit { island.old }
@id("structural.island.value") fn value() -> i64 { 1 }
@id("structural.island.main") fn main() -> i64 { value() }
"#
}

fn entry() -> &'static str {
    r#"
module structural.entry;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
use function @id("structural.consume") from structural.consumer as consume;
permit { audit.old, audit.new }
@id("structural.entry.main") fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 1 }) }
"#
}

fn entry_replacement() -> &'static str {
    r#"
module structural.entry;
use type @id("structural.point") from structural.provider as Point;
use function @id("structural.work") from structural.provider as work;
use function @id("structural.consume") from structural.consumer as consume;
permit { audit.old, audit.new }
@id("structural.entry.main") fn main() -> i64 uses { audit.old, audit.new } { work(Point { value: 2 }) + consume() }
"#
}

fn created() -> &'static str {
    r#"
module structural.created;
permit { created.capability }
fn helper() -> i64 { 7 }
@id("structural.created.main") fn main() -> i64 uses { created.capability } { helper() }
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
    [
        "a/provider.spx",
        "m/consumer.spx",
        "n/island.spx",
        "z/entry.spx",
    ]
    .into_iter()
    .map(|path| std::fs::read(root.join(path)).unwrap())
    .collect()
}

fn active_manifest_paths(root: &Path) -> Vec<String> {
    let active: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".semaprax-workspace/ACTIVE")).unwrap())
            .unwrap();
    let revision = active["workspace_revision"]
        .as_str()
        .unwrap()
        .strip_prefix("sha256:")
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            root.join(".semaprax-workspace/generations")
                .join(revision)
                .join("manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap().to_owned())
        .collect()
}

fn assert_failure_is_read_only<T>(
    fixture: &Fixture,
    operation: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
    code: &str,
    message: Option<&str>,
) {
    let before = inventory(&fixture.root);
    let diagnostics = match operation() {
        Ok(_) => panic!("expected failure"),
        Err(diagnostics) => diagnostics,
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, code);
    if let Some(message) = message {
        assert_eq!(diagnostics[0].message, message);
    }
    assert_eq!(inventory(&fixture.root), before);
    fixture.assert_exclusive_reacquire();
}

#[test]
fn public_api_getters_cli_and_whole_document_kats_are_exact() {
    let fixture = Fixture::new("api-cli-kat");
    let before = inventory(&fixture.root);
    let artifacts =
        semantic_workspace_structural_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let preview =
        semantic_workspace_structural_change::preview(&fixture.root, &fixture.proposal).unwrap();
    let evidence =
        semantic_workspace_structural_change::evidence(&fixture.root, &fixture.proposal).unwrap();
    assert_eq!(preview, artifacts.preview());
    assert_eq!(evidence, artifacts.evidence());
    assert_eq!(
        [
            raw_sha(artifacts.preview()),
            raw_sha(artifacts.context()),
            raw_sha(artifacts.impact()),
            raw_sha(artifacts.review()),
            raw_sha(artifacts.evidence()),
        ],
        [
            "sha256:1bf3eadefd58b3fa92c06e830979ba782b5ede6563f70a0ae7eeff5ca41e76d0",
            "sha256:fd06527f1ae53b3e38218f419cef8e48a723319e05962106f38ecaa7b4561a3d",
            "sha256:8adf3902746a7d8d316e67f015ddd4f103b04fed8f5b8d42bd726ddcac46c57f",
            "sha256:252c99954b7e0e82b288df2d536c9d413e875a3d1373fcb492b9414ea5a43809",
            "sha256:c163c425df9f6fefb354989453d7770174637c0aca5d1cad7f0f0cc7e56d2dac",
        ]
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
    let evidence_value: serde_json::Value = serde_json::from_str(artifacts.evidence()).unwrap();
    assert_eq!(
        evidence_value["proposal"]["digest"],
        artifacts.proposal_digest()
    );
    assert_eq!(
        evidence_value["candidate_manifest"]["digest"],
        artifacts.candidate_manifest_digest()
    );
    assert_eq!(
        evidence_value["structural_change_preview"]["digest"],
        artifacts.preview_digest()
    );
    assert_eq!(
        evidence_value["context"]["digest"],
        artifacts.context_digest()
    );
    assert_eq!(
        evidence_value["impact"]["digest"],
        artifacts.impact_digest()
    );
    assert_eq!(
        evidence_value["review"]["digest"],
        artifacts.review_digest()
    );

    for (command, expected) in [
        (
            "semantic-workspace-structural-change-preview",
            preview.as_str(),
        ),
        (
            "semantic-workspace-structural-change-evidence",
            evidence.as_str(),
        ),
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
fn public_verification_receipt_is_exact_shared_locked_and_read_only() {
    let fixture = Fixture::new("verify");
    let artifacts =
        semantic_workspace_structural_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    FileExt::lock_shared(&shared).unwrap();
    let competing = fixture.lock();
    assert!(FileExt::try_lock_exclusive(&competing).is_err());
    let receipt = semantic_workspace_structural_change::verify(
        &fixture.root,
        &fixture.proposal,
        &evidence_path,
    )
    .unwrap();
    assert_eq!(
        raw_sha(&receipt),
        "sha256:d2c4441326bf311c2593f58a80008c79790f65ecf8868519add6a7fe509b766e"
    );
    let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.workspace-semantic-structural-change-evidence-verification.v1"
    );
    assert_eq!(value["result"], "exact_replay");
    assert_eq!(
        value["workspace_structural_change_evidence"]["digest"],
        artifacts.evidence_digest()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("verify-semantic-workspace-structural-change-evidence")
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
    FileExt::try_lock_exclusive(&competing).unwrap();
    FileExt::unlock(&competing).unwrap();
}

#[test]
fn public_input_hostiles_confusion_and_mode_separation_fail_without_writes() {
    let fixture = Fixture::new("hostiles");
    let proposal_dir = fixture.root.join("proposal-dir");
    std::fs::create_dir(&proposal_dir).unwrap();
    #[cfg(windows)]
    let proposal_directory_message =
        "could not read Semantic Workspace Structural Change proposal: open failed";
    #[cfg(not(windows))]
    let proposal_directory_message =
        "could not read Semantic Workspace Structural Change proposal: input is not a regular file";
    assert_failure_is_read_only(
        &fixture,
        || semantic_workspace_structural_change::preview(&fixture.root, &proposal_dir),
        "SPX-I215",
        Some(proposal_directory_message),
    );
    let invalid_proposal = fixture.root.join("invalid-proposal.json");
    std::fs::write(&invalid_proposal, [0xff]).unwrap();
    assert_failure_is_read_only(
        &fixture,
        || semantic_workspace_structural_change::preview(&fixture.root, &invalid_proposal),
        "SPX-I215",
        Some("could not read Semantic Workspace Structural Change proposal: input is not UTF-8"),
    );
    let oversized_proposal = fixture.root.join("oversized-proposal.json");
    File::create(&oversized_proposal)
        .unwrap()
        .set_len(MAX_PROPOSAL_BYTES + 1)
        .unwrap();
    assert_failure_is_read_only(
        &fixture,
        || semantic_workspace_structural_change::preview(&fixture.root, &oversized_proposal),
        "SPX-G191",
        None,
    );

    let artifacts =
        semantic_workspace_structural_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let receipt = semantic_workspace_structural_change::verify(
        &fixture.root,
        &fixture.proposal,
        &evidence_path,
    )
    .unwrap();
    std::fs::write(&evidence_path, receipt).unwrap();
    assert_failure_is_read_only(
        &fixture,
        || {
            semantic_workspace_structural_change::verify(
                &fixture.root,
                &fixture.proposal,
                &evidence_path,
            )
        },
        "SPX-G193",
        None,
    );
    let evidence_dir = fixture.root.join("evidence-dir");
    std::fs::create_dir(&evidence_dir).unwrap();
    #[cfg(windows)]
    let evidence_directory_message =
        "could not read Semantic Workspace Structural Change Evidence: open failed";
    #[cfg(not(windows))]
    let evidence_directory_message =
        "could not read Semantic Workspace Structural Change Evidence: input is not a regular file";
    assert_failure_is_read_only(
        &fixture,
        || {
            semantic_workspace_structural_change::verify(
                &fixture.root,
                &fixture.proposal,
                &evidence_dir,
            )
        },
        "SPX-I215",
        Some(evidence_directory_message),
    );
    let invalid_evidence = fixture.root.join("invalid-evidence.json");
    std::fs::write(&invalid_evidence, [0xff]).unwrap();
    assert_failure_is_read_only(
        &fixture,
        || {
            semantic_workspace_structural_change::verify(
                &fixture.root,
                &fixture.proposal,
                &invalid_evidence,
            )
        },
        "SPX-I215",
        Some("could not read Semantic Workspace Structural Change Evidence: input is not UTF-8"),
    );
    let oversized_evidence = fixture.root.join("oversized-evidence.json");
    File::create(&oversized_evidence)
        .unwrap()
        .set_len(MAX_EVIDENCE_BYTES + 1)
        .unwrap();
    assert_failure_is_read_only(
        &fixture,
        || {
            semantic_workspace_structural_change::verify(
                &fixture.root,
                &fixture.proposal,
                &oversized_evidence,
            )
        },
        "SPX-G191",
        None,
    );

    let ordinary_serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let ordinary = std::env::temp_dir().join(format!(
        "semaprax-structural-change-ordinary-{}-{ordinary_serial}",
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
    let diagnostics =
        semantic_workspace_structural_change::preview(&ordinary, &fixture.proposal).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-G174");
    assert_eq!(inventory(&ordinary), before);
    std::fs::remove_dir_all(&ordinary).unwrap();
}

#[test]
fn cli_arity_is_exact_for_every_structural_command() {
    for (command, stderr) in [
        (
            "semantic-workspace-structural-change-preview",
            "semantic-workspace-structural-change-preview requires exactly <root> <proposal.json>\n",
        ),
        (
            "semantic-workspace-structural-change-evidence",
            "semantic-workspace-structural-change-evidence requires exactly <root> <proposal.json>\n",
        ),
        (
            "verify-semantic-workspace-structural-change-evidence",
            "verify-semantic-workspace-structural-change-evidence requires exactly <root> <proposal.json> <evidence.json>\n",
        ),
        (
            "apply-semantic-workspace-structural-change-evidence",
            "apply-semantic-workspace-structural-change-evidence requires exactly <root> <proposal.json> <evidence.json>\n",
        ),
    ] {
        for arguments in [vec![command], vec![command, "a", "b", "c", "d"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
                .args(arguments)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, stderr.as_bytes());
        }
    }
}

#[test]
fn public_application_receipt_api_cli_kat_and_candidate_inventory_are_exact() {
    let fixture = Fixture::new("apply-api");
    let raw_before = raw_sources(&fixture.root);
    let artifacts =
        semantic_workspace_structural_change::generate(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let receipt = semantic_workspace_structural_change::apply(
        &fixture.root,
        &fixture.proposal,
        &evidence_path,
    )
    .unwrap();
    assert!(receipt.ends_with('\n'));
    assert!(!receipt[..receipt.len() - 1].contains('\n'));
    assert_eq!(
        raw_sha(&receipt),
        "sha256:bc38f7d5f8cd90bed9dfdee6bceda0533258c7df64c74a95f42f0a53df41edb8"
    );
    let value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.workspace-semantic-structural-change-evidence-application.v1"
    );
    assert_eq!(value["result"], "applied");
    assert_eq!(value["proposal"]["digest"], artifacts.proposal_digest());
    assert_eq!(
        value["workspace_structural_change_evidence"]["digest"],
        artifacts.evidence_digest()
    );
    assert_eq!(
        value["workspace_structural_change_evidence"]["bytes"],
        artifacts.evidence().len()
    );
    assert_eq!(value["budget"]["used_receipt_bytes"], receipt.len());
    assert_eq!(raw_sources(&fixture.root), raw_before);
    assert_eq!(
        active_manifest_paths(&fixture.root),
        [
            "b/created.spx",
            "c/provider.spx",
            "m/consumer.spx",
            "z/entry.spx",
        ]
    );
    fixture.assert_exclusive_reacquire();

    let cli = Fixture::new("apply-cli");
    let cli_raw_before = raw_sources(&cli.root);
    let cli_evidence =
        semantic_workspace_structural_change::evidence(&cli.root, &cli.proposal).unwrap();
    let cli_evidence_path = cli.root.join("evidence.json");
    std::fs::write(&cli_evidence_path, cli_evidence).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("apply-semantic-workspace-structural-change-evidence")
        .arg(&cli.root)
        .arg(&cli.proposal)
        .arg(&cli_evidence_path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, receipt.as_bytes());
    assert_eq!(raw_sources(&cli.root), cli_raw_before);
    assert_eq!(
        active_manifest_paths(&cli.root),
        [
            "b/created.spx",
            "c/provider.spx",
            "m/consumer.spx",
            "z/entry.spx",
        ]
    );
    cli.assert_exclusive_reacquire();
}

#[test]
fn public_application_contention_input_precedence_and_receipt_confusion_are_exact() {
    let contention = Fixture::new("apply-contention");
    let before = inventory(&contention.root);
    let shared = contention.lock();
    FileExt::lock_shared(&shared).unwrap();
    let diagnostics = semantic_workspace_structural_change::apply(
        &contention.root,
        &contention.root.join("missing-proposal.json"),
        &contention.root.join("missing-evidence.json"),
    )
    .unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-I210");
    assert_eq!(diagnostics[0].message, "workspace LOCK is busy");
    assert_eq!(inventory(&contention.root), before);
    FileExt::unlock(&shared).unwrap();
    contention.assert_exclusive_reacquire();

    let precedence = Fixture::new("apply-input-precedence");
    assert_failure_is_read_only(
        &precedence,
        || {
            semantic_workspace_structural_change::apply(
                &precedence.root,
                &precedence.root.join("missing-proposal.json"),
                &precedence.root.join("missing-evidence.json"),
            )
        },
        "SPX-I215",
        Some("could not read Semantic Workspace Structural Change proposal: open failed"),
    );
    let malformed_proposal = precedence.root.join("malformed-proposal.json");
    let malformed_evidence = precedence.root.join("malformed-evidence.json");
    std::fs::write(&malformed_proposal, "{}\n").unwrap();
    std::fs::write(&malformed_evidence, "{}\n").unwrap();
    assert_failure_is_read_only(
        &precedence,
        || {
            semantic_workspace_structural_change::apply(
                &precedence.root,
                &malformed_proposal,
                &malformed_evidence,
            )
        },
        "SPX-G193",
        Some(
            "Semantic Workspace Structural Change Evidence must be one canonical JSON line with one terminal LF: value type is invalid",
        ),
    );

    let confusion = Fixture::new("apply-receipt-confusion");
    let artifacts =
        semantic_workspace_structural_change::generate(&confusion.root, &confusion.proposal)
            .unwrap();
    let evidence_path = confusion.root.join("evidence.json");
    std::fs::write(&evidence_path, artifacts.evidence()).unwrap();
    let verification = semantic_workspace_structural_change::verify(
        &confusion.root,
        &confusion.proposal,
        &evidence_path,
    )
    .unwrap();
    let receipt_path = confusion.root.join("verification-receipt.json");
    std::fs::write(&receipt_path, verification).unwrap();
    assert_failure_is_read_only(
        &confusion,
        || {
            semantic_workspace_structural_change::apply(
                &confusion.root,
                &confusion.proposal,
                &receipt_path,
            )
        },
        "SPX-G193",
        Some(
            "Semantic Workspace Structural Change Evidence must be one canonical JSON line with one terminal LF",
        ),
    );
}

#[test]
fn public_second_application_is_stale_and_preserves_the_committed_generation() {
    let fixture = Fixture::new("apply-stale-second");
    let raw_before = raw_sources(&fixture.root);
    let evidence =
        semantic_workspace_structural_change::evidence(&fixture.root, &fixture.proposal).unwrap();
    let evidence_path = fixture.root.join("evidence.json");
    std::fs::write(&evidence_path, evidence).unwrap();
    semantic_workspace_structural_change::apply(&fixture.root, &fixture.proposal, &evidence_path)
        .unwrap();
    let committed = inventory(&fixture.root);
    let diagnostics = semantic_workspace_structural_change::apply(
        &fixture.root,
        &fixture.proposal,
        &evidence_path,
    )
    .unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-G189");
    assert_eq!(
        diagnostics[0].message,
        "Semantic Workspace Structural Change base workspace revision is stale"
    );
    assert_eq!(inventory(&fixture.root), committed);
    assert_eq!(raw_sources(&fixture.root), raw_before);
    assert_eq!(
        active_manifest_paths(&fixture.root),
        [
            "b/created.spx",
            "c/provider.spx",
            "m/consumer.spx",
            "z/entry.spx",
        ]
    );
    fixture.assert_exclusive_reacquire();
}
