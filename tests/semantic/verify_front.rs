//! Executable evidence for the unified `semaprax verify` front.
//!
//! The front selects a verifier by the capsule's `schema` and hands the same
//! operands to it, so every receipt must be byte-identical to the long-form
//! route's, and an unrecognized, unreadable, or mis-shaped capsule must fail
//! closed before any verifier runs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{
    format, graph, parse, semantic_workspace, semantic_workspace_change, workspace, workspace_graph,
};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = "module verify_front.demo;\n\n@id(\"verify_front.helper\")\nfn helper() -> i64\n{\n    7\n}\n\n@id(\"verify_front.main\")\nfn main() -> i64\n{\n    helper()\n}\n";

fn fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-verify-front-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .output()
        .unwrap()
}

fn success(arguments: &[&str]) -> String {
    let output = cli(arguments);
    assert!(
        output.status.success(),
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{arguments:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn failure(arguments: &[&str], status: i32, code: &str) {
    let output = cli(arguments);
    assert_eq!(output.status.code(), Some(status), "{arguments:?}");
    assert!(output.stdout.is_empty(), "{arguments:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(code), "{arguments:?}: {stderr}");
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn same_receipt(long: &[&str], short: &[&str]) {
    let expected = success(long);
    assert!(expected.ends_with('\n'));
    assert!(expected.contains("\"verified\":true") || expected.contains("\"schema\""));
    assert_eq!(success(short), expected);
}

#[test]
fn patch_evidence_v1_and_v2_route_by_schema() {
    let root = fixture("patch");
    let source = root.join("module.spx");
    std::fs::write(&source, SOURCE).unwrap();
    let revision = graph::revision(&parse(SOURCE, &source).unwrap());
    let v1 = root.join("v1.spatch");
    std::fs::write(
        &v1,
        format!("base {revision}\nrename verify_front.helper to seven\n"),
    )
    .unwrap();
    let v2 = root.join("v2.spatch");
    std::fs::write(
        &v2,
        format!("schema semaprax.semantic-patch.v2\nbase {revision}\nrename verify_front.helper to seven\nrequire no-new-effects\n"),
    )
    .unwrap();
    let evidence_v1 = root.join("evidence-v1.json");
    std::fs::write(
        &evidence_v1,
        success(&["patch-evidence", text(&source), text(&v1)]),
    )
    .unwrap();
    let evidence_v2 = root.join("evidence-v2.json");
    std::fs::write(
        &evidence_v2,
        success(&["patch-evidence-v2", text(&source), text(&v2)]),
    )
    .unwrap();

    same_receipt(
        &[
            "verify-patch-evidence",
            text(&source),
            text(&v1),
            text(&evidence_v1),
        ],
        &["verify", text(&source), text(&v1), text(&evidence_v1)],
    );
    same_receipt(
        &[
            "verify-patch-evidence-v2",
            text(&source),
            text(&v2),
            text(&evidence_v2),
        ],
        &["verify", text(&source), text(&v2), text(&evidence_v2)],
    );
    // The schema selects the family: a v2 capsule with the v1 patch goes to the
    // v2 verifier, whose rejection the front reproduces exactly.
    let long = cli(&[
        "verify-patch-evidence-v2",
        text(&source),
        text(&v1),
        text(&evidence_v2),
    ]);
    let short = cli(&["verify", text(&source), text(&v1), text(&evidence_v2)]);
    assert_eq!(long.status.code(), short.status.code());
    assert_eq!(long.stdout, short.stdout);
    assert_eq!(long.stderr, short.stderr);
    assert_eq!(short.status.code(), Some(1));
    // Two operands cannot select a three-operand family.
    failure(
        &["verify", text(&source), text(&evidence_v1)],
        1,
        "SPX-V201",
    );
    assert_eq!(std::fs::read_to_string(&source).unwrap(), SOURCE);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn workspace_patch_evidence_routes_by_schema() {
    let root = fixture("wspatch");
    let mut files = Vec::new();
    for (path, module) in [("alpha.spx", "alpha"), ("beta.spx", "beta")] {
        let source = SOURCE
            .replace("verify_front.demo", &format!("verify_front.{module}"))
            .replace(
                "verify_front.helper",
                &format!("verify_front.{module}.helper"),
            )
            .replace("verify_front.main", &format!("verify_front.{module}.main"));
        let canonical = format::canonical(&parse(&source, path).unwrap());
        std::fs::write(root.join(path), &canonical).unwrap();
        let child = format!(
            "base {}\nrename verify_front.{module}.helper to seven\n",
            graph::revision(&parse(&canonical, path).unwrap())
        );
        files.push(format!(
            "{{\"path\":\"{path}\",\"patch\":{}}}",
            serde_json::to_string(&child).unwrap()
        ));
    }
    let path_set = root.join("paths.json");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n",
    )
    .unwrap();
    let workspace_revision = workspace::initialize(&root, &path_set).unwrap();
    let patch = root.join("change.wspatch");
    std::fs::write(
        &patch,
        format!(
            "{{\"schema\":\"semaprax.semantic-workspace-patch.v1\",\"base_workspace_revision\":\"{workspace_revision}\",\"files\":[{}]}}\n",
            files.join(",")
        ),
    )
    .unwrap();
    let evidence = root.join("evidence.json");
    std::fs::write(
        &evidence,
        success(&["workspace-patch-evidence", text(&root), text(&patch)]),
    )
    .unwrap();
    same_receipt(
        &[
            "verify-workspace-patch-evidence",
            text(&root),
            text(&patch),
            text(&evidence),
        ],
        &["verify", text(&root), text(&patch), text(&evidence)],
    );
    std::fs::remove_dir_all(root).unwrap();
}

fn write_canonical(root: &Path, path: &str, source: &str) {
    let destination = root.join(path);
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    std::fs::write(
        destination,
        format::canonical(&parse(source, path).unwrap()),
    )
    .unwrap();
}

#[test]
fn semantic_workspace_change_evidence_routes_by_schema() {
    let root = fixture("swc");
    write_canonical(
        &root,
        "a/provider.spx",
        "module change.provider;\n@id(\"change.work\") fn work(value: i64) -> i64 { value }\n@id(\"change.provider.main\") fn main() -> i64 { work(1) }\n",
    );
    write_canonical(
        &root,
        "z/entry.spx",
        "module change.entry;\nuse function @id(\"change.work\") from change.provider as work;\n@id(\"change.entry.main\") fn main() -> i64 { work(2) }\n",
    );
    let path_set = root.join("paths.json");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/entry.spx\"}]}\n",
    )
    .unwrap();
    semantic_workspace::initialize(&root, &path_set).unwrap();
    let snapshot = workspace_graph::snapshot(&root, "change.entry").unwrap();
    let mut changes = Vec::new();
    for (path, replacement) in [
        (
            "a/provider.spx",
            "module change.provider;\n@id(\"change.work\") fn work(value: i64) -> i64 { value + 1 }\n@id(\"change.provider.main\") fn main() -> i64 { work(1) }\n",
        ),
        (
            "z/entry.spx",
            "module change.entry;\nuse function @id(\"change.work\") from change.provider as work;\n@id(\"change.entry.main\") fn main() -> i64 { work(3) }\n",
        ),
    ] {
        let module = snapshot
            .modules()
            .iter()
            .find(|module| module.path() == path)
            .unwrap();
        let replacement = format::canonical(&parse(replacement, path).unwrap());
        changes.push(format!(
            "{{\"path\":\"{path}\",\"base_source_graph_schema\":{},\"base_source_revision\":{},\"base_source_digest\":{},\"replacement_source\":{}}}",
            serde_json::to_string(module.source_graph_schema()).unwrap(),
            serde_json::to_string(module.source_revision()).unwrap(),
            serde_json::to_string(module.source_digest()).unwrap(),
            serde_json::to_string(&replacement).unwrap(),
        ));
    }
    let proposal = root.join("change.json");
    std::fs::write(
        &proposal,
        format!(
            "{{\"schema\":\"semaprax.workspace-semantic-change.v1\",\"base_workspace_revision\":{},\"entry_module\":\"change.entry\",\"changes\":[{}]}}\n",
            serde_json::to_string(snapshot.workspace_revision()).unwrap(),
            changes.join(",")
        ),
    )
    .unwrap();
    let evidence = root.join("evidence.json");
    std::fs::write(
        &evidence,
        semantic_workspace_change::evidence(&root, &proposal).unwrap(),
    )
    .unwrap();
    same_receipt(
        &[
            "verify-semantic-workspace-change-evidence",
            text(&root),
            text(&proposal),
            text(&evidence),
        ],
        &["verify", text(&root), text(&proposal), text(&evidence)],
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_image_routes_by_schema_with_two_operands() {
    let root = fixture("image");
    std::fs::create_dir(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(source.join(path), root.join(path)).unwrap();
    }
    let manifest = root.join("semaprax.toml");
    let image = root.join("image.json");
    std::fs::write(&image, success(&["project-image", text(&manifest)])).unwrap();
    same_receipt(
        &["project-image-verify", text(&manifest), text(&image)],
        &["verify", text(&manifest), text(&image)],
    );
    // Three operands cannot select the two-operand image family.
    failure(
        &["verify", text(&manifest), text(&manifest), text(&image)],
        1,
        "SPX-V201",
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrecognized_or_unreadable_capsules_fail_closed_before_any_verifier() {
    let root = fixture("closed");
    let source = root.join("module.spx");
    std::fs::write(&source, SOURCE).unwrap();
    let foreign = root.join("foreign.json");
    std::fs::write(&foreign, "{\"schema\":\"semaprax.graph.v10\"}\n").unwrap();
    failure(
        &["verify", text(&source), text(&source), text(&foreign)],
        1,
        "SPX-V201",
    );
    failure(&["verify", text(&source), text(&foreign)], 1, "SPX-V201");
    let no_schema = root.join("no-schema.json");
    std::fs::write(&no_schema, "{\"kind\":\"none\"}\n").unwrap();
    failure(
        &["verify", text(&source), text(&source), text(&no_schema)],
        1,
        "SPX-V202",
    );
    let not_json = root.join("not.json");
    std::fs::write(&not_json, "base sha256:00\n").unwrap();
    failure(
        &["verify", text(&source), text(&source), text(&not_json)],
        1,
        "SPX-V202",
    );
    failure(
        &[
            "verify",
            text(&source),
            text(&source),
            text(&root.join("missing.json")),
        ],
        1,
        "SPX-V202",
    );
    for arguments in [
        &["verify"][..],
        &["verify", "one"][..],
        &["verify", "a", "b", "c", "d"][..],
        &["verify", "a", "--json", "c"][..],
    ] {
        let output = cli(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
    }
    assert_eq!(std::fs::read_to_string(&source).unwrap(), SOURCE);
    std::fs::remove_dir_all(root).unwrap();
}
