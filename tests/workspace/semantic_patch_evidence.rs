use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{patch_evidence, repair, workspace, workspace_patch_evidence};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Family {
    V1,
    V2,
    V3,
}

struct Fixture {
    root: PathBuf,
    patch: PathBuf,
    evidence: PathBuf,
}

impl Fixture {
    fn new(label: &str, families: &[Family]) -> Self {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-patch-evidence-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut paths = Vec::new();
        let mut child_patches = Vec::new();
        for (index, family) in families.iter().copied().enumerate() {
            let stem = ["alpha", "beta", "gamma", "delta"][index];
            let path = format!("{stem}.spx");
            let module = format!("workspace_evidence.{stem}");
            let source = match family {
                Family::V1 | Family::V2 => canonical(
                    &format!(
                        "module {module}; @id(\"{module}.helper\") fn helper()->i64{{{index}}} @id(\"{module}.main\") fn main()->i64{{helper()}}"
                    ),
                    &path,
                ),
                Family::V3 => canonical(
                    &format!(
                        "module {module}; fn helper(value:i64)->i64{{value+1}} @id(\"{module}.caller\") fn caller(value:i64)->i64{{helper(value)}} @id(\"{module}.main\") fn main()->i64{{caller({index})}}"
                    ),
                    &path,
                ),
            };
            let source_path = root.join(&path);
            std::fs::write(&source_path, &source).unwrap();
            let child_patch = match family {
                Family::V1 => format!(
                    "base {}\nrename {module}.helper to {stem}_answer\n",
                    revision(&source, &path)
                ),
                Family::V2 => format!(
                    "schema semaprax.semantic-patch.v2\nbase {}\nrename {module}.helper to {stem}_answer\nrequire no-new-effects\n",
                    revision(&source, &path)
                ),
                Family::V3 => {
                    let query = repair::DiagnosticRepairQuery::assign_function_id(format!(
                        "auto:{module}.helper"
                    ))
                    .unwrap();
                    let repairs: serde_json::Value =
                        serde_json::from_str(&repair::query(&source_path, &query).unwrap()).unwrap();
                    let preview: serde_json::Value = serde_json::from_str(
                        &repair::instantiate(
                            &source_path,
                            repairs["repair"]["id"].as_str().unwrap(),
                            &repair::PersistentDeclarationId::new(format!("{module}.helper"))
                                .unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    preview["patch"]["source"].as_str().unwrap().to_owned()
                }
            };
            paths.push(path);
            child_patches.push(child_patch);
        }
        let path_set = root.join("paths.json");
        let files = paths
            .iter()
            .map(|path| format!("{{\"path\":{}}}", serde_json::to_string(path).unwrap()))
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &path_set,
            format!("{{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{files}]}}\n"),
        )
        .unwrap();
        let workspace_revision = workspace::initialize(&root, &path_set).unwrap();
        let patch = root.join("change.wspatch");
        let files = paths
            .iter()
            .zip(&child_patches)
            .map(|(path, patch)| {
                format!(
                    "{{\"path\":{},\"patch\":{}}}",
                    serde_json::to_string(path).unwrap(),
                    serde_json::to_string(patch).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        std::fs::write(
            &patch,
            format!(
                "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{workspace_revision}\",\"files\":[{files}]}}\n"
            ),
        )
        .unwrap();
        let evidence = root.join("evidence.json");
        Self {
            root,
            patch,
            evidence,
        }
    }

    fn control_inventory(&self) -> Vec<(String, String, Vec<u8>)> {
        let control = self.root.join(".semaprax-workspace");
        let mut output = Vec::new();
        let mut stack = vec![control.clone()];
        while let Some(directory) = stack.pop() {
            for entry in std::fs::read_dir(&directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let relative = path
                    .strip_prefix(&control)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let metadata = entry.metadata().unwrap();
                let identity = physical_identity(&path);
                let bytes = if metadata.is_file() {
                    std::fs::read(&path).unwrap()
                } else {
                    Vec::new()
                };
                output.push((relative, identity, bytes));
                if metadata.is_dir() {
                    stack.push(path);
                }
            }
        }
        output.sort();
        output
    }
}

#[cfg(unix)]
fn physical_identity(path: &std::path::Path) -> String {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = std::fs::symlink_metadata(path).unwrap();
    format!("{}:{}", metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn physical_identity(path: &std::path::Path) -> String {
    let handle = winapi_util::Handle::from_path_any(path).unwrap();
    let information = winapi_util::file::information(handle).unwrap();
    format!(
        "{}:{}",
        information.volume_serial_number(),
        information.file_index()
    )
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn canonical(source: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, path).unwrap())
}

fn revision(source: &str, path: &str) -> String {
    semaprax::graph::revision(&semaprax::parse(source, path).unwrap())
}

fn sha256(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

fn domain_digest(domain: &[u8], value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(digest.finalize())
    )
}

fn assert_exclusive_lock_available(root: &std::path::Path) {
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".semaprax-workspace/LOCK"))
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lock)
        .expect("a rejected evidence read must synchronously release LOCK");
    fs2::FileExt::unlock(&lock).unwrap();
}

#[test]
fn capsule_and_receipt_v1_v2_v3_mixed_literal_kats_and_no_write() {
    let cases: [(&str, &[Family], &str, &str); 4] = [
        (
            "v1",
            &[Family::V1, Family::V1],
            "d0f0ec9abde015cd84745d8d71b260736874b7cff8f172194d04e8ebe489c197",
            "ee310a2f848dd034c20f727f011f30db46dfe478bbc1169467dec0d57c266ae1",
        ),
        (
            "v2",
            &[Family::V2, Family::V2],
            "95b054e188a4721e03c08b94afe0963394fc0af16be42ef3bdec0990218eb9f6",
            "da2440da67c87ec0ab873599c911fc78e816d02fcd12195532ce93817a15df0b",
        ),
        (
            "v3",
            &[Family::V3, Family::V3],
            "3fc5dc57a01ce2a9d1110dfd66ec96e9def90b8bfd3e5d2328aa9d4a81da19e4",
            "b05b0516508c7850b409b1b81dedfc51c708bfbe6e73c94db77a1aadce35f757",
        ),
        (
            "mixed",
            &[Family::V1, Family::V2, Family::V3],
            "de764637af59c533feaba15dca373408cb50972f81afd3fde903f463550fde27",
            "3538b97acc1626972b0242085c87059c51b64c2ba7412172bbc2c5118f2f63c1",
        ),
    ];
    for (label, families, capsule_kat, receipt_kat) in cases {
        let fixture = Fixture::new(label, families);
        let inventory = fixture.control_inventory();
        let base_revision = workspace::snapshot(&fixture.root)
            .unwrap()
            .workspace_revision()
            .to_owned();
        let capsule = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
        assert_eq!(
            workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap(),
            capsule
        );
        assert!(capsule.ends_with('\n'));
        assert!(!capsule[..capsule.len() - 1].contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
        assert_eq!(
            value["schema"],
            "semaprax.semantic-workspace-patch-evidence.v1"
        );
        assert_eq!(value["files"].as_array().unwrap().len(), families.len());
        let workspace_patch: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&fixture.patch).unwrap()).unwrap();
        let mut child_total = 0usize;
        for (outer, patch) in value["files"]
            .as_array()
            .unwrap()
            .iter()
            .zip(workspace_patch["files"].as_array().unwrap())
        {
            assert_eq!(outer["path"], patch["path"]);
            let child_patch = fixture
                .root
                .join(format!("{}.child.spatch", outer["path"].as_str().unwrap()));
            std::fs::write(&child_patch, patch["patch"].as_str().unwrap()).unwrap();
            let direct = patch_evidence::generate(
                &fixture.root.join(outer["path"].as_str().unwrap()),
                &child_patch,
            )
            .unwrap();
            child_total = child_total.checked_add(direct.len()).unwrap();
            let direct: serde_json::Value = serde_json::from_str(&direct).unwrap();
            assert_eq!(
                outer["base_source_graph_schema"],
                direct["source_graph_schema"]
            );
            assert_eq!(
                outer["candidate_source_graph_schema"],
                direct["source_graph_schema"]
            );
            assert_eq!(outer["base_revision"], direct["base_revision"]);
            assert_eq!(outer["candidate_revision"], direct["candidate_revision"]);
            assert_eq!(outer["base_source"], direct["source"]);
            assert_eq!(outer["patch"], direct["patch"]);
            assert_eq!(outer["review"], direct["review"]);
            assert_eq!(outer["assessments"], direct["assessments"]);
            assert_eq!(outer["supporting_evidence"], direct["supporting_evidence"]);
        }
        assert_eq!(
            value["budget"]["used_total_child_patch_evidence_bytes"],
            child_total
        );
        assert_eq!(
            value["budget"]["used_workspace_evidence_bytes"],
            capsule.len()
        );
        let capsule_sha = sha256(&capsule);
        std::fs::write(&fixture.evidence, &capsule).unwrap();
        let receipt =
            workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
                .unwrap();
        let receipt_value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        assert_eq!(receipt_value["result"], "exact_replay");
        assert_eq!(
            receipt_value["workspace_patch_evidence"]["schema"],
            "semaprax.semantic-workspace-patch-evidence.v1"
        );
        assert_eq!(
            receipt_value["workspace_patch_evidence"]["digest"],
            domain_digest(
                b"semaprax.semantic-workspace-patch-evidence.artifact-digest.v1\0",
                &capsule,
            )
        );
        assert_eq!(receipt_value["files"], value["files"]);
        assert_eq!(receipt_value["workspace_patch"], value["workspace_patch"]);
        assert_eq!(
            receipt_value["workspace_preview"],
            value["workspace_preview"]
        );
        assert_eq!(
            receipt_value["budget"]["used_workspace_evidence_bytes"],
            capsule.len()
        );
        assert_eq!(
            receipt_value["budget"]["used_workspace_receipt_bytes"],
            receipt.len()
        );
        let receipt_sha = sha256(&receipt);
        assert_eq!(capsule_sha, capsule_kat, "capsule {label}");
        assert_eq!(receipt_sha, receipt_kat, "receipt {label}");
        assert!(receipt.ends_with('\n'));
        assert_eq!(fixture.control_inventory(), inventory);
        assert_eq!(
            workspace::snapshot(&fixture.root)
                .unwrap()
                .workspace_revision(),
            base_revision
        );

        let candidate_revision = value["candidate_workspace_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        let applied =
            workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
                .unwrap();
        assert_eq!(applied, candidate_revision);
        assert_eq!(
            workspace::snapshot(&fixture.root)
                .unwrap()
                .workspace_revision(),
            candidate_revision
        );
        let stale =
            workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
                .expect_err("the same evidence cannot authorize a second stale apply");
        assert_eq!(stale[0].code, "SPX-G152");

        let parity = Fixture::new(&format!("{label}-ordinary-parity"), families);
        let ordinary = workspace::apply(&parity.root, &parity.patch).unwrap();
        assert_eq!(ordinary, candidate_revision);
    }
}

#[test]
fn strict_parser_rejects_canonicality_substitution_and_receipt_confusion() {
    let fixture = Fixture::new("hostile", &[Family::V1, Family::V2]);
    let inventory = fixture.control_inventory();
    let capsule = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
    let mutations = [
        capsule.trim_end().to_owned(),
        format!("\u{feff}{capsule}"),
        capsule.replace("\n", "\r\n"),
        capsule.replacen("{\"schema\":", "{ \"schema\":", 1),
        capsule.replacen(
            "\"schema\":\"semaprax.semantic-workspace-patch-evidence.v1\",",
            "",
            1,
        ),
        capsule.replacen("{\"schema\":", "{\"extra\":0,\"schema\":", 1),
        capsule.replacen("{\"schema\":", "{\"schema\":\"duplicate\",\"schema\":", 1),
    ];
    for (index, mutation) in mutations.into_iter().enumerate() {
        std::fs::write(&fixture.evidence, mutation).unwrap();
        let error =
            workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
                .expect_err("mutation must fail");
        assert_eq!(error[0].code, "SPX-G160", "mutation {index}");
    }

    let mut reordered: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    reordered["files"].as_array_mut().unwrap().reverse();
    let reordered = format!("{}\n", serde_json::to_string(&reordered).unwrap());
    std::fs::write(&fixture.evidence, reordered).unwrap();
    assert_eq!(
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("reordered paths must fail")[0]
            .code,
        "SPX-G160"
    );

    let mut uncorrelated: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    uncorrelated["files"][0]["patch"]["schema"] =
        serde_json::Value::String("semaprax.semantic-patch.v3".to_owned());
    let uncorrelated = format!("{}\n", serde_json::to_string(&uncorrelated).unwrap());
    std::fs::write(&fixture.evidence, uncorrelated).unwrap();
    assert_eq!(
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("uncorrelated support must fail")[0]
            .code,
        "SPX-G160"
    );

    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence).unwrap();
    std::fs::write(&fixture.evidence, receipt).unwrap();
    assert_eq!(
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("receipt is not a capsule")[0]
            .code,
        "SPX-G160"
    );
    assert_eq!(
        workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("a verification receipt is not apply evidence")[0]
            .code,
        "SPX-G160"
    );
    assert_eq!(fixture.control_inventory(), inventory);

    let foreign = Fixture::new("foreign", &[Family::V1, Family::V2]);
    let foreign_patch = std::fs::read_to_string(&foreign.patch)
        .unwrap()
        .replace("alpha_answer", "alpha_foreign");
    std::fs::write(&foreign.patch, foreign_patch).unwrap();
    let foreign_capsule =
        workspace_patch_evidence::generate(&foreign.root, &foreign.patch).unwrap();
    std::fs::write(&fixture.evidence, foreign_capsule).unwrap();
    assert_eq!(
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("foreign capsule must fail")[0]
            .code,
        "SPX-G162"
    );
    assert_eq!(
        workspace_patch_evidence::apply(&fixture.root, &fixture.patch, &fixture.evidence)
            .expect_err("foreign evidence must fail before candidate staging")[0]
            .code,
        "SPX-G162"
    );
    assert_eq!(fixture.control_inventory(), inventory);
}

#[test]
fn cli_api_lf_and_arity_are_exact() {
    let fixture = Fixture::new("cli", &[Family::V1, Family::V2]);
    let expected = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
    let binary = env!("CARGO_BIN_EXE_semaprax");
    let generated = Command::new(binary)
        .args(["workspace-patch-evidence"])
        .arg(&fixture.root)
        .arg(&fixture.patch)
        .output()
        .unwrap();
    assert!(generated.status.success());
    assert_eq!(generated.stdout, expected.as_bytes());
    std::fs::write(&fixture.evidence, &expected).unwrap();
    let receipt =
        workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence).unwrap();
    let verified = Command::new(binary)
        .args(["verify-workspace-patch-evidence"])
        .arg(&fixture.root)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert!(verified.status.success());
    assert_eq!(verified.stdout, receipt.as_bytes());
    for (command, message) in [
        (
            "workspace-patch-evidence",
            "workspace-patch-evidence requires exactly <root> <patch.wspatch>\nhint: run `semaprax workspace-patch-evidence --help` for usage\n",
        ),
        (
            "verify-workspace-patch-evidence",
            "verify-workspace-patch-evidence requires exactly <root> <patch.wspatch> <evidence.json>\nhint: run `semaprax verify-workspace-patch-evidence --help` for usage\n",
        ),
    ] {
        let output = Command::new(binary).arg(command).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(String::from_utf8(output.stderr).unwrap(), message);
    }

    std::fs::write(&fixture.evidence, "not-json\n").unwrap();
    let rejected = Command::new(binary)
        .args(["verify-workspace-patch-evidence"])
        .arg(&fixture.root)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    let stderr = String::from_utf8(rejected.stderr).unwrap();
    assert!(stderr.contains("SPX-G160"));
    assert!(stderr.contains(
        "Semantic Workspace Patch Evidence must be one canonical JSON line with one terminal LF"
    ));

    std::fs::write(&fixture.patch, "not a workspace patch\n").unwrap();
    let rejected = Command::new(binary)
        .args(["workspace-patch-evidence"])
        .arg(&fixture.root)
        .arg(&fixture.patch)
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(String::from_utf8(rejected.stderr)
        .unwrap()
        .contains("SPX-G150"));
}

#[test]
fn evidence_bound_is_checked_before_workspace_semantic_replay() {
    let fixture = Fixture::new("bound", &[Family::V1, Family::V2]);
    let inventory = fixture.control_inventory();
    let valid_evidence = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
    let active = fixture.root.join(".semaprax-workspace/ACTIVE");
    let held_active = fixture.root.join("held-active");
    std::fs::rename(&active, &held_active).unwrap();
    std::fs::write(&fixture.evidence, vec![b'x'; 65_537]).unwrap();
    std::fs::write(&fixture.patch, b"not a workspace patch").unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("oversized evidence must fail first");
    assert_eq!(error[0].code, "SPX-G161");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Patch Evidence `max_workspace_evidence_bytes` exceeds 65536"
    );
    assert_exclusive_lock_available(&fixture.root);
    std::fs::write(&fixture.evidence, b"not-json\n").unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("malformed evidence must fail before patch and snapshot");
    assert_eq!(error[0].code, "SPX-G160");
    assert_exclusive_lock_available(&fixture.root);
    std::fs::write(&fixture.evidence, vec![0xff]).unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("non-UTF8 evidence must fail at the owned read");
    assert_eq!(error[0].code, "SPX-I213");
    assert!(error[0]
        .message
        .starts_with("cannot read Semantic Workspace Patch Evidence"));
    assert_exclusive_lock_available(&fixture.root);
    std::fs::rename(&held_active, &active).unwrap();

    std::fs::write(&fixture.evidence, valid_evidence).unwrap();
    std::fs::rename(&active, &held_active).unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("evidence route must parse patch before snapshot authentication");
    assert_eq!(error[0].code, "SPX-G150");
    assert_exclusive_lock_available(&fixture.root);
    let ordinary = workspace::preview(&fixture.root, &fixture.patch)
        .expect_err("ordinary preview preserves snapshot-first precedence");
    assert_eq!(ordinary[0].code, "SPX-G153");
    std::fs::rename(&held_active, &active).unwrap();

    std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&fixture.patch)
        .unwrap()
        .set_len(4_194_305)
        .unwrap();
    let error = workspace_patch_evidence::generate(&fixture.root, &fixture.patch)
        .expect_err("Workspace patch input bounds retain their G151 diagnostic family");
    assert_eq!(error[0].code, "SPX-G151");
    assert_eq!(error[0].message, "workspace input exceeds its byte limit");
    assert_exclusive_lock_available(&fixture.root);
    assert_eq!(fixture.control_inventory(), inventory);
}

#[test]
fn evidence_file_cardinality_and_json_depth_boundaries_are_exact() {
    let fixture = Fixture::new("parser-bounds", &[Family::V1, Family::V2]);
    let inventory = fixture.control_inventory();
    let capsule = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();

    let one_file = {
        let mut changed = value.clone();
        changed["files"] = serde_json::Value::Array(vec![value["files"][0].clone()]);
        format!("{}\n", serde_json::to_string(&changed).unwrap())
    };
    std::fs::write(&fixture.evidence, one_file).unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("one file is outside the closed workspace evidence schema");
    assert_eq!(error[0].code, "SPX-G160");

    let seventeen_files = {
        let mut changed = value;
        changed["files"] = serde_json::Value::Array(vec![changed["files"][0].clone(); 17]);
        format!("{}\n", serde_json::to_string(&changed).unwrap())
    };
    std::fs::write(&fixture.evidence, seventeen_files).unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("seventeen files exceed the outer changed-file limit");
    assert_eq!(error[0].code, "SPX-G161");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Patch Evidence `max_changed_files` exceeds 16"
    );

    std::fs::write(&fixture.evidence, "[[[[[[[[0]]]]]]]]\n").unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("depth eight is structurally admitted but not a capsule");
    assert_eq!(error[0].code, "SPX-G160");
    std::fs::write(&fixture.evidence, "[[[[[[[[[0]]]]]]]]]\n").unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &fixture.evidence)
        .expect_err("depth nine exceeds the JSON depth limit");
    assert_eq!(error[0].code, "SPX-G161");
    assert_eq!(
        error[0].message,
        "Semantic Workspace Patch Evidence `max_json_depth` exceeds 8"
    );

    let directory = fixture.root.join("evidence-directory");
    std::fs::create_dir(&directory).unwrap();
    let error = workspace_patch_evidence::verify(&fixture.root, &fixture.patch, &directory)
        .expect_err("nonregular evidence must fail at the I213 read boundary");
    assert_eq!(error[0].code, "SPX-I213");
    assert!(error[0]
        .message
        .starts_with("cannot read Semantic Workspace Patch Evidence"));

    let workspace_patch: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture.patch).unwrap()).unwrap();
    std::fs::write(
        &fixture.patch,
        format!(
            "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":{},\"files\":[{{\"path\":{},\"patch\":{}}}]}}\n",
            serde_json::to_string(&workspace_patch["base_workspace_revision"]).unwrap(),
            serde_json::to_string(&workspace_patch["files"][0]["path"]).unwrap(),
            serde_json::to_string(&workspace_patch["files"][0]["patch"]).unwrap(),
        ),
    )
    .unwrap();
    let error = workspace_patch_evidence::generate(&fixture.root, &fixture.patch)
        .expect_err("a one-child workspace evidence capsule is outside the closed schema");
    assert_eq!(error[0].code, "SPX-G160");
    assert!(error[0]
        .message
        .contains("workspace evidence file cardinality is outside the closed schema"));
    assert_eq!(fixture.control_inventory(), inventory);
}

#[test]
fn shared_lock_contention_fails_without_workspace_writes() {
    let fixture = Fixture::new("lock", &[Family::V1, Family::V2]);
    let inventory = fixture.control_inventory();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.root.join(".semaprax-workspace/LOCK"))
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    let error = workspace_patch_evidence::generate(&fixture.root, &fixture.patch)
        .expect_err("an exclusive workspace writer excludes evidence generation");
    assert_eq!(error[0].code, "SPX-I210");
    fs2::FileExt::unlock(&lock).unwrap();
    assert_eq!(fixture.control_inventory(), inventory);

    let capsule = workspace_patch_evidence::generate(&fixture.root, &fixture.patch).unwrap();
    assert!(!capsule.is_empty());
    assert_eq!(fixture.control_inventory(), inventory);
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    fs2::FileExt::unlock(&lock).unwrap();
}
