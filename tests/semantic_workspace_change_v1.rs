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
    format!("sha256:{:x}", Sha256::digest(source.as_bytes()))
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
        "sha256:fbfba16e8c3a822b65e59b2a16e2f28393b6d9d9552bcc95fa1363e2599ff8fc"
    );
    assert_eq!(
        raw_sha(&evidence),
        "sha256:0c5393cb128adc8223a82b7181229cb2c18cb495d714949ccc2dfba07b4402b0"
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

    for (path, code, message) in [
        (
            directory.as_path(),
            "SPX-I214",
            "could not read Semantic Workspace Change proposal: input is not a regular file",
        ),
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
