use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{repair, workspace};
use sha2::{Digest, Sha256};

static FIXTURE: AtomicU64 = AtomicU64::new(0);

const ZERO_REVISION: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let serial = FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-workspace-hostile-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    root: TempRoot,
    revision: String,
}

impl Fixture {
    fn initialized(label: &str, files: &[(&str, &str)]) -> Self {
        let root = TempRoot::new(label);
        let path_set = write_fixture_inputs(root.path(), files);
        let revision = workspace::initialize(root.path(), &path_set).unwrap();
        Self { root, revision }
    }

    fn snapshot(&self) -> workspace::WorkspaceSnapshot {
        workspace::snapshot(self.root.path()).unwrap()
    }

    fn patch_path(&self, bytes: &str) -> PathBuf {
        let path = self.root.path().join("change.wspatch");
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

fn write_fixture_inputs(root: &Path, files: &[(&str, &str)]) -> PathBuf {
    for (path, source) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, canonical(source, path.to_str().unwrap())).unwrap();
    }
    let paths = files.iter().map(|(path, _)| *path).collect::<Vec<_>>();
    let path_set = root.join("paths.json");
    std::fs::write(&path_set, path_set_json(&paths)).unwrap();
    path_set
}

fn canonical(source: &str, path: &str) -> String {
    let program = semaprax::parse(source, path).unwrap();
    semaprax::format::canonical(&program)
}

fn sources2() -> [(&'static str, &'static str); 2] {
    [
        (
            "alpha.spx",
            "module workspace.alpha; @id(\"workspace.alpha.helper\") fn helper()->i64{1} fn main()->i64{helper()}",
        ),
        (
            "beta.spx",
            "module workspace.beta; @id(\"workspace.beta.helper\") fn helper()->i64{2} fn main()->i64{helper()}",
        ),
    ]
}

fn sources3() -> [(&'static str, &'static str); 3] {
    [
        (
            "alpha.spx",
            "module workspace.alpha; @id(\"workspace.alpha.helper\") fn helper()->i64{1} fn main()->i64{helper()}",
        ),
        (
            "beta.spx",
            "module workspace.beta; @id(\"workspace.beta.helper\") fn helper()->i64{2} fn main()->i64{helper()}",
        ),
        (
            "gamma.spx",
            "module workspace.gamma; fn helper()->bool{true} @id(\"workspace.gamma.run\") fn run()->bool{helper()} fn main()->i64{1}",
        ),
    ]
}

fn path_set_json(paths: &[&str]) -> String {
    format!(
        "{{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{}]}}\n",
        paths
            .iter()
            .map(|path| format!("{{\"path\":{}}}", serde_json::to_string(path).unwrap()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn workspace_patch(base: &str, files: &[(&str, String)]) -> String {
    format!(
        "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":{base},\"files\":[{}]}}\n",
        files
            .iter()
            .map(|(path, patch)| format!(
                "{{\"path\":{},\"patch\":{}}}",
                serde_json::to_string(path).unwrap(),
                serde_json::to_string(patch).unwrap()
            ))
            .collect::<Vec<_>>()
            .join(","),
        base = serde_json::to_string(base).unwrap()
    )
}

fn source_revision<'a>(snapshot: &'a workspace::WorkspaceSnapshot, path: &str) -> &'a str {
    snapshot
        .files()
        .iter()
        .find(|file| file.path() == path)
        .unwrap()
        .source_revision()
}

fn rename_patch(revision: &str, stable_id: &str, new_name: &str) -> String {
    format!("base {revision}\nrename {stable_id} to {new_name}\n")
}

fn rename_patch_v2(revision: &str, stable_id: &str, new_name: &str) -> String {
    format!(
        "schema semaprax.semantic-patch.v2\nbase {revision}\nrename {stable_id} to {new_name}\n"
    )
}

fn sha256(bytes: &[u8]) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn first_code<T>(result: Result<T, Vec<semaprax::diagnostic::Diagnostic>>) -> String {
    result.err().expect("hostile input must fail")[0]
        .code
        .to_owned()
}

#[test]
fn path_set_canonical_json_schema_and_portable_path_matrix_is_closed() {
    let cases = [
        ("bom", "\u{feff}{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n".to_owned()),
        ("crlf", path_set_json(&["alpha.spx", "beta.spx"]).replace('\n', "\r\n")),
        ("missing-lf", path_set_json(&["alpha.spx", "beta.spx"]).trim_end().to_owned()),
        ("extra-lf", format!("{}\n", path_set_json(&["alpha.spx", "beta.spx"]))),
        ("leading-space", format!(" {}", path_set_json(&["alpha.spx", "beta.spx"]))),
        ("wrong-schema", "{\"schema\":\"semaprax.workspace-manifest.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n".to_owned()),
        ("reordered", "{\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}],\"schema\":\"semaprax.workspace-path-set.v1\"}\n".to_owned()),
        ("duplicate-key", "{\"schema\":\"semaprax.workspace-path-set.v1\",\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n".to_owned()),
        ("extra-key", "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}],\"extra\":0}\n".to_owned()),
        ("unsorted", path_set_json(&["beta.spx", "alpha.spx"])),
        ("duplicate-path", path_set_json(&["alpha.spx", "alpha.spx"])),
    ];
    for (label, bytes) in cases {
        let root = TempRoot::new(label);
        let path_set = root.path().join("paths.json");
        std::fs::write(&path_set, bytes).unwrap();
        assert_eq!(
            first_code(workspace::initialize(root.path(), &path_set)),
            "SPX-G150",
            "{label}"
        );
        assert!(!root.path().join(".semaprax-workspace").exists(), "{label}");
    }

    let long_segment = format!("{}.spx", "a".repeat(61));
    let deep = format!("{}/file.spx", vec!["a"; 16].join("/"));
    let long_path = format!(
        "{}/{}/{}/{}/file.spx",
        "a".repeat(60),
        "b".repeat(60),
        "c".repeat(60),
        "d".repeat(60)
    );
    let invalid = vec![
        "Alpha.spx".to_owned(),
        "ümlaut.spx".to_owned(),
        "nested\\beta.spx".to_owned(),
        "c:beta.spx".to_owned(),
        "/alpha.spx".to_owned(),
        "./alpha.spx".to_owned(),
        "nested/../alpha.spx".to_owned(),
        "nested//alpha.spx".to_owned(),
        "con.spx".to_owned(),
        "folder./alpha.spx".to_owned(),
        long_segment,
        deep,
        long_path,
    ];
    for (index, hostile) in invalid.into_iter().enumerate() {
        let root = TempRoot::new(&format!("path-{index}"));
        let path_set = root.path().join("paths.json");
        std::fs::write(&path_set, path_set_json(&["alpha.spx", &hostile])).unwrap();
        assert_eq!(
            first_code(workspace::initialize(root.path(), &path_set)),
            "SPX-G150",
            "{hostile}"
        );
        assert!(!root.path().join(".semaprax-workspace").exists());
    }
}

#[test]
fn active_and_workspace_patch_strict_canonical_matrix_fails_closed() {
    for (label, mutate) in [
        ("active-bom", 0usize),
        ("active-crlf", 1),
        ("active-missing-lf", 2),
        ("active-reordered", 3),
        ("active-wrong-schema", 4),
        ("active-extra", 5),
    ] {
        let fixture = Fixture::initialized(label, &sources2());
        let active = fixture.root.path().join(".semaprax-workspace/ACTIVE");
        let canonical = std::fs::read_to_string(&active).unwrap();
        let hostile = match mutate {
            0 => format!("\u{feff}{canonical}"),
            1 => canonical.replace('\n', "\r\n"),
            2 => canonical.trim_end().to_owned(),
            3 => format!(
                "{{\"workspace_revision\":{},\"schema\":\"semaprax.workspace-root.v1\"}}\n",
                serde_json::to_string(&fixture.revision).unwrap()
            ),
            4 => canonical.replace(
                "semaprax.workspace-root.v1",
                "semaprax.workspace-snapshot.v1",
            ),
            5 => canonical.replacen('}', ",\"extra\":0}", 1),
            _ => unreachable!(),
        };
        std::fs::write(active, hostile).unwrap();
        assert_eq!(
            first_code(workspace::snapshot(fixture.root.path())),
            "SPX-G150",
            "{label}"
        );
    }

    let fixture = Fixture::initialized("patch-canonical", &sources2());
    let snapshot = fixture.snapshot();
    let alpha = rename_patch(
        source_revision(&snapshot, "alpha.spx"),
        "workspace.alpha.helper",
        "renamed_alpha",
    );
    let beta = rename_patch(
        source_revision(&snapshot, "beta.spx"),
        "workspace.beta.helper",
        "renamed_beta",
    );
    let canonical = workspace_patch(
        &fixture.revision,
        &[("alpha.spx", alpha.clone()), ("beta.spx", beta.clone())],
    );
    let reordered = format!(
        "{{\"base_workspace_revision\":{},\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"files\":[{{\"path\":\"alpha.spx\",\"patch\":{}}},{{\"path\":\"beta.spx\",\"patch\":{}}}]}}\n",
        serde_json::to_string(&fixture.revision).unwrap(),
        serde_json::to_string(&alpha).unwrap(),
        serde_json::to_string(&beta).unwrap()
    );
    let wrong_schema = canonical.replace(
        "semaprax.semantic-workspace-patch.v1",
        "semaprax.workspace-path-set.v1",
    );
    let embedded_noncanonical = workspace_patch(
        &fixture.revision,
        &[
            ("alpha.spx", alpha.replace("rename ", "rename  ")),
            ("beta.spx", beta),
        ],
    );
    for (label, hostile) in [
        ("bom", format!("\u{feff}{canonical}")),
        ("crlf", canonical.replace('\n', "\r\n")),
        ("missing-lf", canonical.trim_end().to_owned()),
        ("extra-lf", format!("{canonical}\n")),
        ("leading-space", format!(" {canonical}")),
        ("wrong-schema", wrong_schema),
        ("reordered", reordered),
        (
            "extra-key",
            canonical.replacen(",\"files\"", ",\"extra\":0,\"files\"", 1),
        ),
        (
            "duplicate-key",
            canonical.replacen(
                "{\"schema\":",
                "{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"schema\":",
                1,
            ),
        ),
        ("embedded-noncanonical", embedded_noncanonical),
    ] {
        let path = fixture.patch_path(&hostile);
        assert_eq!(
            first_code(workspace::preview(fixture.root.path(), &path)),
            "SPX-G150",
            "{label}"
        );
    }
}

#[test]
fn stale_noop_and_changed_subset_have_exact_diagnostics_and_budget() {
    let fixture = Fixture::initialized("semantic-boundaries", &sources3());
    let snapshot = fixture.snapshot();
    let alpha_revision = source_revision(&snapshot, "alpha.spx");
    let beta_revision = source_revision(&snapshot, "beta.spx");
    let valid_files = [
        (
            "alpha.spx",
            rename_patch(alpha_revision, "workspace.alpha.helper", "renamed_alpha"),
        ),
        (
            "beta.spx",
            rename_patch(beta_revision, "workspace.beta.helper", "renamed_beta"),
        ),
    ];

    let stale = workspace_patch(ZERO_REVISION, &valid_files);
    let stale_path = fixture.patch_path(&stale);
    assert_eq!(
        first_code(workspace::preview(fixture.root.path(), &stale_path)),
        "SPX-G152"
    );

    let noop = workspace_patch(
        &fixture.revision,
        &[
            (
                "alpha.spx",
                rename_patch(alpha_revision, "workspace.alpha.helper", "helper"),
            ),
            ("beta.spx", valid_files[1].1.clone()),
        ],
    );
    let noop_path = fixture.patch_path(&noop);
    assert_eq!(
        first_code(workspace::preview(fixture.root.path(), &noop_path)),
        "SPX-G153"
    );

    let outside = workspace_patch(
        &fixture.revision,
        &[
            ("alpha.spx", valid_files[0].1.clone()),
            ("outside.spx", valid_files[1].1.clone()),
        ],
    );
    let outside_path = fixture.patch_path(&outside);
    assert_eq!(
        first_code(workspace::preview(fixture.root.path(), &outside_path)),
        "SPX-G153"
    );

    let valid = workspace_patch(&fixture.revision, &valid_files);
    let valid_path = fixture.patch_path(&valid);
    let preview = workspace::preview(fixture.root.path(), &valid_path).unwrap();
    assert!(preview.contains("\"used_managed_files\":3"));
    assert!(preview.contains("\"used_changed_files\":2"));
    assert!(preview.contains("\"used_operations\":2"));
    assert!(!preview.ends_with('\n'));
}

#[test]
fn snapshot_and_mixed_v1_v2_v3_preview_have_literal_sha_kats_and_replay() {
    let fixture = Fixture::initialized("mixed-kat", &sources3());
    let snapshot = fixture.snapshot();
    let snapshot_json = snapshot.to_json();
    assert!(!snapshot_json.ends_with('\n'));
    assert_eq!(snapshot_json, snapshot.to_json());
    assert_eq!(
        snapshot_json,
        workspace::snapshot(fixture.root.path()).unwrap().to_json()
    );
    assert_eq!(
        sha256(snapshot_json.as_bytes()),
        "dfd35db518d0a8d94b83702dd1d2760ce9340b5875e0960ac573f84474c223b5"
    );

    let alpha = rename_patch(
        source_revision(&snapshot, "alpha.spx"),
        "workspace.alpha.helper",
        "renamed_alpha",
    );
    let beta = rename_patch_v2(
        source_revision(&snapshot, "beta.spx"),
        "workspace.beta.helper",
        "renamed_beta",
    );
    let gamma_source = fixture.root.path().join("gamma.spx");
    let query =
        repair::DiagnosticRepairQuery::assign_function_id("auto:workspace.gamma.helper").unwrap();
    let report = repair::query(&gamma_source, &query).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    let instantiated = repair::instantiate(
        &gamma_source,
        report["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new("workspace.gamma.helper").unwrap(),
    )
    .unwrap();
    let instantiated: serde_json::Value = serde_json::from_str(&instantiated).unwrap();
    let gamma = instantiated["patch"]["source"].as_str().unwrap().to_owned();

    let patch = workspace_patch(
        &fixture.revision,
        &[
            ("alpha.spx", alpha),
            ("beta.spx", beta),
            ("gamma.spx", gamma),
        ],
    );
    let patch_path = fixture.patch_path(&patch);
    let preview = workspace::preview(fixture.root.path(), &patch_path).unwrap();
    assert!(!preview.ends_with('\n'));
    assert_eq!(
        preview,
        workspace::preview(fixture.root.path(), &patch_path).unwrap()
    );
    assert!(preview.contains("\"patch_schema\":\"semaprax.semantic-patch.v1\""));
    assert!(preview.contains("\"patch_schema\":\"semaprax.semantic-patch.v2\""));
    assert!(preview.contains("\"patch_schema\":\"semaprax.semantic-patch.v3\""));
    assert_eq!(
        sha256(preview.as_bytes()),
        "3cbd8d22bc26069387ac8ebce72ca590f095cbaa193b04bdef041e4c06beced1"
    );
    let candidate_revision = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        workspace::apply(fixture.root.path(), &patch_path).unwrap(),
        candidate_revision
    );
    assert_eq!(
        workspace::snapshot(fixture.root.path())
            .unwrap()
            .workspace_revision(),
        candidate_revision
    );
    assert_eq!(
        workspace::apply(fixture.root.path(), &patch_path).unwrap_err()[0].code,
        "SPX-G152"
    );
}

#[test]
fn workspace_cli_has_exact_arity_and_api_byte_projection() {
    let binary = env!("CARGO_BIN_EXE_semaprax");
    for (arguments, message) in [
        (
            vec!["workspace-init"],
            "workspace-init requires exactly <root> <path-set.json>\nhint: run `semaprax workspace-init --help` for usage\n",
        ),
        (
            vec!["workspace-init", "root", "paths", "extra"],
            "workspace-init requires exactly <root> <path-set.json>\nhint: run `semaprax workspace-init --help` for usage\n",
        ),
        (
            vec!["workspace-snapshot"],
            "workspace-snapshot requires exactly <root>\nhint: run `semaprax workspace-snapshot --help` for usage\n",
        ),
        (
            vec!["workspace-snapshot", "root", "extra"],
            "workspace-snapshot requires exactly <root>\nhint: run `semaprax workspace-snapshot --help` for usage\n",
        ),
        (
            vec!["workspace-preview", "root"],
            "workspace-preview requires exactly <root> <patch.wspatch>\nhint: run `semaprax workspace-preview --help` for usage\n",
        ),
        (
            vec!["workspace-preview", "root", "patch", "extra"],
            "workspace-preview requires exactly <root> <patch.wspatch>\nhint: run `semaprax workspace-preview --help` for usage\n",
        ),
        (
            vec!["workspace-apply", "root"],
            "workspace-apply requires exactly <root> <patch.wspatch>\nhint: run `semaprax workspace-apply --help` for usage\n",
        ),
        (
            vec!["workspace-apply", "root", "patch", "extra"],
            "workspace-apply requires exactly <root> <patch.wspatch>\nhint: run `semaprax workspace-apply --help` for usage\n",
        ),
    ] {
        let output = Command::new(binary).args(arguments).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), message);
    }

    let root = TempRoot::new("cli");
    let path_set = write_fixture_inputs(root.path(), &sources2());
    let init = Command::new(binary)
        .arg("workspace-init")
        .arg(root.path())
        .arg(&path_set)
        .output()
        .unwrap();
    assert!(init.status.success());
    assert!(init.stderr.is_empty());
    let revision = workspace::snapshot(root.path())
        .unwrap()
        .workspace_revision()
        .to_owned();
    assert_eq!(
        String::from_utf8(init.stdout).unwrap(),
        format!("initialized semantic workspace; workspace is {revision}\n")
    );

    let snapshot = workspace::snapshot(root.path()).unwrap();
    let snapshot_cli = Command::new(binary)
        .arg("workspace-snapshot")
        .arg(root.path())
        .output()
        .unwrap();
    assert!(snapshot_cli.status.success());
    assert!(snapshot_cli.stderr.is_empty());
    assert_eq!(
        String::from_utf8(snapshot_cli.stdout).unwrap(),
        format!("{}\n", snapshot.to_json())
    );

    let patch = workspace_patch(
        &revision,
        &[
            (
                "alpha.spx",
                rename_patch(
                    source_revision(&snapshot, "alpha.spx"),
                    "workspace.alpha.helper",
                    "renamed_alpha",
                ),
            ),
            (
                "beta.spx",
                rename_patch(
                    source_revision(&snapshot, "beta.spx"),
                    "workspace.beta.helper",
                    "renamed_beta",
                ),
            ),
        ],
    );
    let patch_path = root.path().join("change.wspatch");
    std::fs::write(&patch_path, patch).unwrap();
    let preview = workspace::preview(root.path(), &patch_path).unwrap();
    let preview_cli = Command::new(binary)
        .arg("workspace-preview")
        .arg(root.path())
        .arg(&patch_path)
        .output()
        .unwrap();
    assert!(preview_cli.status.success());
    assert!(preview_cli.stderr.is_empty());
    assert_eq!(
        String::from_utf8(preview_cli.stdout).unwrap(),
        format!("{preview}\n")
    );
    let candidate_revision = serde_json::from_str::<serde_json::Value>(&preview).unwrap()
        ["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();

    let apply = Command::new(binary)
        .arg("workspace-apply")
        .arg(root.path())
        .arg(&patch_path)
        .output()
        .unwrap();
    assert!(apply.status.success());
    assert!(apply.stderr.is_empty());
    assert_eq!(
        String::from_utf8(apply.stdout).unwrap(),
        format!("applied semantic workspace transaction; workspace is now {candidate_revision}\n")
    );

    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for usage in [
        "semaprax workspace-init <root> <path-set.json>",
        "semaprax workspace-snapshot <root>",
        "semaprax workspace-preview <root> <patch.wspatch>",
        "semaprax workspace-apply <root> <patch.wspatch>",
    ] {
        assert!(help.contains(usage));
    }
}
