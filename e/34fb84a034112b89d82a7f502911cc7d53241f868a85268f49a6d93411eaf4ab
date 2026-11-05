use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use fs2::FileExt;
use semaprax::{format, parse, semantic_workspace, semantic_workspace_operations, workspace};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    proposal: PathBuf,
    proposal_source: String,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-workspace-operations-v1-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        write_source(
            &root,
            "a/provider.spx",
            "module ops.provider; @id(\"ops.answer\") fn answer()->i64{1}",
        );
        write_source(
            &root,
            "b/consumer.spx",
            "module ops.consumer; use function @id(\"ops.answer\") from ops.provider as answer; @id(\"ops.main\") fn main()->i64{answer()}",
        );
        let path_set = root.join("paths.json");
        std::fs::write(
            &path_set,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"b/consumer.spx\"}]}\n",
        )
        .unwrap();
        let revision = semantic_workspace::initialize(&root, &path_set).unwrap();
        let proposal_source = format!(
            "{{\"schema\":\"semaprax.semantic-workspace-operations.v1\",\"base_workspace_revision\":\"{revision}\",\"entry_module\":\"ops.consumer\",\"operations\":[{{\"kind\":\"rename_declaration\",\"path\":\"a/provider.spx\",\"declaration_kind\":\"function\",\"target_id\":\"ops.answer\",\"from\":\"answer\",\"to\":\"response\"}},{{\"kind\":\"rename_import_alias\",\"path\":\"b/consumer.spx\",\"import_kind\":\"function\",\"target_id\":\"ops.answer\",\"target_module\":\"ops.provider\",\"from\":\"answer\",\"to\":\"response\"}}]}}\n"
        );
        let proposal = root.join("operations.json");
        std::fs::write(&proposal, &proposal_source).unwrap();
        Self {
            root,
            proposal,
            proposal_source,
        }
    }

    fn lock(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.root.join(".semaprax-workspace/LOCK"))
            .unwrap()
    }

    fn inventory(&self) -> Vec<(String, bool, Vec<u8>)> {
        fn walk(root: &Path, path: &Path, facts: &mut Vec<(String, bool, Vec<u8>)>) {
            let mut entries = std::fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                let metadata = std::fs::symlink_metadata(&path).unwrap();
                if metadata.is_dir() {
                    facts.push((relative, true, Vec::new()));
                    walk(root, &path, facts);
                } else {
                    facts.push((relative, false, std::fs::read(path).unwrap()));
                }
            }
        }
        let mut facts = Vec::new();
        walk(&self.root, &self.root, &mut facts);
        facts
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

fn write_source(root: &Path, path: &str, source: &str) {
    let destination = root.join(path);
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    let program = parse(source, path).unwrap();
    std::fs::write(destination, format::canonical(&program)).unwrap();
}

fn run_cli(command: &str, fixture: &Fixture) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg(command)
        .arg(&fixture.root)
        .arg(&fixture.proposal)
        .output()
        .unwrap()
}

fn raw_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    super::pin_capture::observe("operations", bytes, digest)
}

#[test]
fn public_api_cli_getters_kats_and_no_write_are_exact() {
    let fixture = Fixture::new("api");
    let before = fixture.inventory();
    let bundle = semantic_workspace_operations::derive(&fixture.root, &fixture.proposal).unwrap();
    super::pin_capture::record("operations-derivation", bundle.derivation().as_bytes());
    super::pin_capture::record(
        "operations-change",
        bundle.derived_change_proposal().as_bytes(),
    );
    let document: serde_json::Value = serde_json::from_str(bundle.derivation().trim_end()).unwrap();
    assert_eq!(
        document["operations_proposal"]["bytes"],
        fixture.proposal_source.len()
    );
    assert_eq!(fixture.inventory(), before);
    assert_eq!(
        bundle.operations_proposal_digest(),
        "sha256:3c7bf340a5313907edcec41748063e8666793ee76b903bc4e691871a843544b5"
    );
    assert_eq!(
        bundle.derived_change_proposal_digest(),
        "sha256:5c7a67d42ef76b3a241c0dc98f3d8919a799d3745bb6ae54a1d0289a51ee3e86"
    );
    assert_eq!(
        bundle.derivation_digest(),
        // Parsed derivation fields and fixed-point replay independently bind
        // this re-pinned whole-document digest.
        super::pin_capture::native_wire_pin(
            "sha256:0db2a5420fe0e0ebaa4db06a16142504df48bcf0c9990f7fda9d8b55ef433dd7",
            "sha256:cb4cfb4b54025e199da0aab41389e2c698ab80bff9e56489940e12e214b2190e",
        )
    );
    assert!(bundle.derivation().ends_with('\n'));
    assert!(bundle.derived_change_proposal().ends_with('\n'));
    assert_eq!(
        semantic_workspace_operations::derivation(&fixture.root, &fixture.proposal).unwrap(),
        bundle.derivation()
    );
    assert_eq!(
        semantic_workspace_operations::derived_change_proposal(&fixture.root, &fixture.proposal)
            .unwrap(),
        bundle.derived_change_proposal()
    );
    assert_eq!(fixture.inventory(), before);
    fixture.assert_exclusive_reacquire();

    let cli_fixture = Fixture::new("cli");
    let derivation = run_cli("semantic-workspace-operations-derive", &cli_fixture);
    assert!(derivation.status.success());
    assert_eq!(derivation.stdout, bundle.derivation().as_bytes());
    assert!(derivation.stderr.is_empty());
    let change = run_cli(
        "semantic-workspace-operations-change-proposal",
        &cli_fixture,
    );
    assert!(change.status.success());
    assert_eq!(change.stdout, bundle.derived_change_proposal().as_bytes());
    assert!(change.stderr.is_empty());
}

#[test]
fn public_arity_locking_i216_and_mode_separation_are_fail_closed() {
    for (command, message) in [
        (
            "semantic-workspace-operations-derive",
            "semantic-workspace-operations-derive requires exactly <root> <proposal.json>\nhint: run `semaprax semantic-workspace-operations-derive --help` for usage\n",
        ),
        (
            "semantic-workspace-operations-change-proposal",
            "semantic-workspace-operations-change-proposal requires exactly <root> <proposal.json>\nhint: run `semaprax semantic-workspace-operations-change-proposal --help` for usage\n",
        ),
    ] {
        for args in [Vec::<&str>::new(), vec!["one"], vec!["one", "two", "three"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
                .arg(command)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert_eq!(String::from_utf8(output.stderr).unwrap(), message);
        }
    }

    let fixture = Fixture::new("locking");
    let before = fixture.inventory();
    let shared = fixture.lock();
    FileExt::try_lock_shared(&shared).unwrap();
    assert!(semantic_workspace_operations::derive(&fixture.root, &fixture.proposal).is_ok());
    FileExt::unlock(&shared).unwrap();
    let exclusive = fixture.lock();
    FileExt::try_lock_exclusive(&exclusive).unwrap();
    let diagnostics = semantic_workspace_operations::derive(&fixture.root, &fixture.proposal)
        .err()
        .unwrap();
    assert_eq!(diagnostics[0].code, "SPX-I210");
    FileExt::unlock(&exclusive).unwrap();
    assert_eq!(fixture.inventory(), before);
    fixture.assert_exclusive_reacquire();

    let missing = fixture.root.join("missing.json");
    let diagnostics = semantic_workspace_operations::derive(&fixture.root, &missing)
        .err()
        .unwrap();
    assert_eq!(diagnostics[0].code, "SPX-I216");
    assert_eq!(
        diagnostics[0].message,
        "could not read Semantic Workspace Operations proposal: open failed"
    );
    fixture.assert_exclusive_reacquire();

    for command in [
        "semantic-workspace-operations-derive",
        "semantic-workspace-operations-change-proposal",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg(command)
            .arg(&fixture.root)
            .arg(&missing)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "error[SPX-I216]: could not read Semantic Workspace Operations proposal: open failed\n"
        );
    }

    let ordinary_root = std::env::temp_dir().join(format!(
        "semaprax-ordinary-operations-mode-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&ordinary_root).unwrap();
    write_source(
        &ordinary_root,
        "a/provider.spx",
        "module ordinary.provider; @id(\"ordinary.provider.main\") fn main()->i64{0}",
    );
    write_source(
        &ordinary_root,
        "z/app.spx",
        "module ordinary.app; @id(\"ordinary.app.main\") fn main()->i64{1}",
    );
    let ordinary_path_set = ordinary_root.join("paths.json");
    std::fs::write(
        &ordinary_path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
    )
    .unwrap();
    workspace::initialize(&ordinary_root, &ordinary_path_set).unwrap();
    let diagnostics = semantic_workspace_operations::derive(&ordinary_root, &fixture.proposal)
        .err()
        .unwrap();
    assert_eq!(diagnostics[0].code, "SPX-G174");
    let _ = std::fs::remove_dir_all(ordinary_root);
}

#[test]
fn public_operations_evidence_verify_apply_api_cli_are_exact() {
    let fixture = Fixture::new("public-evidence");
    let before = fixture.inventory();
    let bundle =
        semantic_workspace_operations::generate_evidence(&fixture.root, &fixture.proposal).unwrap();
    super::pin_capture::record("operations-derivation", bundle.derivation().as_bytes());
    super::pin_capture::record(
        "operations-child-evidence",
        bundle.workspace_change_evidence().as_bytes(),
    );
    super::pin_capture::record(
        "operations-evidence",
        bundle.operations_evidence().as_bytes(),
    );
    assert_eq!(
        bundle.operations_proposal_digest(),
        "sha256:3c7bf340a5313907edcec41748063e8666793ee76b903bc4e691871a843544b5"
    );
    assert_eq!(
        bundle.derivation_digest(),
        // Parsed derivation fields and fixed-point replay independently bind
        // this re-pinned whole-document digest.
        super::pin_capture::native_wire_pin(
            "sha256:0db2a5420fe0e0ebaa4db06a16142504df48bcf0c9990f7fda9d8b55ef433dd7",
            "sha256:cb4cfb4b54025e199da0aab41389e2c698ab80bff9e56489940e12e214b2190e",
        )
    );
    assert_eq!(
        bundle.derived_change_proposal_digest(),
        "sha256:5c7a67d42ef76b3a241c0dc98f3d8919a799d3745bb6ae54a1d0289a51ee3e86"
    );
    assert_eq!(
        raw_sha256(bundle.workspace_change_evidence().as_bytes()),
        // Evidence reference and replay assertions independently bind this
        // re-pinned whole-document digest.
        super::pin_capture::native_wire_pin(
            "sha256:b6ef2b450daa6c9a09e730e28a3079976f00fa9b37b2a6abe26e17fa91f81138",
            "sha256:14c2cfca49446f591a3fa98fedd33a4e5ed10eb8f71a99645a06a59822bd566a",
        )
    );
    assert_eq!(
        raw_sha256(bundle.operations_evidence().as_bytes()),
        // Evidence reference and replay assertions independently bind this
        // re-pinned whole-document digest.
        super::pin_capture::native_wire_pin(
            "sha256:d675be31765bba093f04c3b0a89216a4ad9e1472bfffa63574b003d2e94f78bb",
            "sha256:b5d1f25cef9e0a2b2d315c254c732f6847a9578b10b30e8697cb4d6792e94b68",
        )
    );
    assert!(bundle
        .workspace_change_evidence_digest()
        .starts_with("sha256:"));
    assert!(bundle.operations_evidence_digest().starts_with("sha256:"));
    assert!(bundle.derivation().ends_with('\n'));
    assert!(bundle.derived_change_proposal().ends_with('\n'));
    assert_eq!(fixture.inventory(), before);
    assert_eq!(
        semantic_workspace_operations::evidence(&fixture.root, &fixture.proposal).unwrap(),
        bundle.operations_evidence()
    );
    let cli_fixture = Fixture::new("public-evidence-cli");
    let cli = run_cli("semantic-workspace-operations-evidence", &cli_fixture);
    assert!(cli.status.success());
    assert_eq!(cli.stdout, bundle.operations_evidence().as_bytes());
    assert!(cli.stderr.is_empty());

    let evidence_path = fixture.root.join("operations-evidence.json");
    std::fs::write(&evidence_path, bundle.operations_evidence()).unwrap();
    let verification =
        semantic_workspace_operations::verify(&fixture.root, &fixture.proposal, &evidence_path)
            .unwrap();
    assert_eq!(
        raw_sha256(verification.as_bytes()),
        // Parsed receipt fields independently bind this re-pinned digest.
        super::pin_capture::native_wire_pin(
            "sha256:6d2fa7c230113135205b36e9fa1bf873e0020ff061b22795d21dad70060d65c7",
            "sha256:fa792d1b1297a76d886b1d6d145d6608c7234ff189d36d959563be3f6c4de67a",
        )
    );
    let cli_verify = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("verify-semantic-workspace-operations-evidence")
        .arg(&fixture.root)
        .arg(&fixture.proposal)
        .arg(&evidence_path)
        .output()
        .unwrap();
    assert!(cli_verify.status.success());
    assert_eq!(cli_verify.stdout, verification.as_bytes());
    assert!(cli_verify.stderr.is_empty());

    let apply_fixture = Fixture::new("public-apply");
    let apply_bundle = semantic_workspace_operations::generate_evidence(
        &apply_fixture.root,
        &apply_fixture.proposal,
    )
    .unwrap();
    let apply_evidence = apply_fixture.root.join("operations-evidence.json");
    std::fs::write(&apply_evidence, apply_bundle.operations_evidence()).unwrap();
    let application = semantic_workspace_operations::apply(
        &apply_fixture.root,
        &apply_fixture.proposal,
        &apply_evidence,
    )
    .unwrap();
    assert_eq!(
        raw_sha256(application.as_bytes()),
        // Parsed receipt fields independently bind this re-pinned digest.
        super::pin_capture::native_wire_pin(
            "sha256:2b21276016d0af2f15503b82d42a8ec9e462590597fed808d2273f7131ccc049",
            "sha256:cce730f3d5fdecc08f6d62c519c981590269a317e257593c53cb3179a03dd017",
        )
    );
    apply_fixture.assert_exclusive_reacquire();

    let cli_apply_fixture = Fixture::new("public-apply-cli");
    let cli_apply_bundle = semantic_workspace_operations::generate_evidence(
        &cli_apply_fixture.root,
        &cli_apply_fixture.proposal,
    )
    .unwrap();
    let cli_apply_evidence = cli_apply_fixture.root.join("operations-evidence.json");
    std::fs::write(&cli_apply_evidence, cli_apply_bundle.operations_evidence()).unwrap();
    let cli_apply = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("apply-semantic-workspace-operations-evidence")
        .arg(&cli_apply_fixture.root)
        .arg(&cli_apply_fixture.proposal)
        .arg(&cli_apply_evidence)
        .output()
        .unwrap();
    assert!(cli_apply.status.success());
    assert_eq!(cli_apply.stdout, application.as_bytes());
    assert!(cli_apply.stderr.is_empty());
}

#[test]
fn public_operations_evidence_cli_arity_help_and_errors_are_exact() {
    for (command, message) in [
        (
            "semantic-workspace-operations-evidence",
            "semantic-workspace-operations-evidence requires exactly <root> <proposal.json>\nhint: run `semaprax semantic-workspace-operations-evidence --help` for usage\n",
        ),
        (
            "verify-semantic-workspace-operations-evidence",
            "verify-semantic-workspace-operations-evidence requires exactly <root> <proposal.json> <evidence.json>\nhint: run `semaprax verify-semantic-workspace-operations-evidence --help` for usage\n",
        ),
        (
            "apply-semantic-workspace-operations-evidence",
            "apply-semantic-workspace-operations-evidence requires exactly <root> <proposal.json> <evidence.json>\nhint: run `semaprax apply-semantic-workspace-operations-evidence --help` for usage\n",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg(command)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), message);
    }

    // The exhaustive command catalog lives under `help all`; the bare and
    // `--help` pages are the guided one-screen overview.
    let help = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["help", "all"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert_eq!(
        help.lines()
            .filter(|line| line.contains("semantic-workspace-operations-"))
            .collect::<Vec<_>>(),
        [
            "semaprax semantic-workspace-operations-derive <root> <proposal.json>",
            "semaprax semantic-workspace-operations-change-proposal <root> <proposal.json>",
            "semaprax semantic-workspace-operations-evidence <root> <proposal.json>",
            "semaprax verify-semantic-workspace-operations-evidence <root> <proposal.json> <evidence.json>",
            "semaprax apply-semantic-workspace-operations-evidence <root> <proposal.json> <evidence.json>",
        ]
    );
}

#[test]
fn external_consumer_cannot_construct_format_clone_or_reach_private_operations_authority() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-operations-external-surface-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let manifest_root = env!("CARGO_MANIFEST_DIR").replace('\\', "\\\\");
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            r#"[package]
name = "semaprax-operations-external-surface"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
semaprax = {{ path = "{manifest_root}", default-features = false }}
"#
        ),
    )
    .unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        r#"use semaprax::semantic_workspace_operations::{apply_with_hook, derive_with_hook, parse_proposal, OperationsDerivePoint, OperationsEvidencePoint, SemanticWorkspaceOperationsDerivation, SemanticWorkspaceOperationsEvidenceArtifacts};

fn require_clone<T: Clone>() {}
fn require_debug<T: std::fmt::Debug>() {}

fn main() {
    require_clone::<SemanticWorkspaceOperationsDerivation>();
    require_debug::<SemanticWorkspaceOperationsDerivation>();
    require_clone::<SemanticWorkspaceOperationsEvidenceArtifacts>();
    require_debug::<SemanticWorkspaceOperationsEvidenceArtifacts>();
    let _ = SemanticWorkspaceOperationsDerivation {
        operations_proposal_digest: String::new(),
        derived_change_proposal: String::new(),
        derived_change_proposal_digest: String::new(),
        derivation: String::new(),
        derivation_digest: String::new(),
    };
    let _ = derive_with_hook;
    let _ = apply_with_hook;
    let _ = parse_proposal;
    let _ = std::mem::size_of::<OperationsDerivePoint>();
    let _ = std::mem::size_of::<OperationsEvidencePoint>();
}
"#,
    )
    .unwrap();
    let checked = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["check", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .unwrap();
    assert!(!checked.status.success());
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(stderr.contains("derive_with_hook"));
    assert!(stderr.contains("parse_proposal"));
    assert!(stderr.contains("OperationsDerivePoint"));
    assert!(stderr.contains("OperationsEvidencePoint"));
    assert!(stderr.contains("apply_with_hook"));
    assert!(stderr.contains("Clone"));
    assert!(stderr.contains("Debug"));
    assert!(stderr.contains("private"));
    let _ = std::fs::remove_dir_all(root);
}

#[path = "semantic_operations/reporting.rs"]
mod reporting;
