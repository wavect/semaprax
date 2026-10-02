//! Standalone explorer CLI output remains source-free and never clobbers a destination.

use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-explore-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
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
        Self(root)
    }

    fn cli(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&self.0)
            .args(arguments)
            .output()
            .unwrap()
    }
}

fn recovery_capsule(fixture: &Fixture, intent: Value) -> (String, String) {
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        let base = snapshot.retain_revision();
        let candidate = ProjectCandidate::open(base, snapshot.project_revision())?;
        let change = SemanticChange::new(candidate.revision().project_revision(), &intent)?;
        let candidate = candidate.apply(candidate.candidate_digest(), &change)?;
        Ok((
            candidate.candidate_digest().to_owned(),
            candidate.recovery_capsule()?,
        ))
    })
    .unwrap()
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn digest(snapshot: &serde_json::Value) -> String {
    let mut canonical = snapshot.clone();
    canonical["snapshot_digest"] = serde_json::Value::Null;
    canonical.as_object_mut().unwrap().remove("snapshot_digest");
    canonical.sort_all_objects();
    let mut hash = Sha256::new();
    hash.update(b"semaprax.explorer-snapshot.v1\0");
    hash.update(canonical.to_string().as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

#[test]
fn explorer_json_is_canonical_source_free_and_carries_its_payload_digest() {
    let fixture = Fixture::new();
    let secret = "987654321";
    let core = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&core).unwrap();
    let source = source.replace("left + right", &format!("left + {secret}"));
    std::fs::write(&core, source).unwrap();

    let output = fixture.cli(&[
        "explore",
        "semaprax.toml",
        "--format",
        "json",
        "--output",
        "overview.json",
    ]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let bytes = std::fs::read(fixture.0.join("overview.json")).unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(snapshot["schema"], "semaprax.explorer-snapshot.v1");
    assert_eq!(snapshot["source_included"], false);
    assert!(snapshot.get("source_files").is_none());
    assert_eq!(snapshot["focus_sides"], serde_json::json!([]));
    assert_eq!(snapshot["snapshot_digest"], digest(&snapshot));
    assert!(!bytes
        .windows(secret.len())
        .any(|window| window == secret.as_bytes()));
    assert!(!bytes
        .windows(b"fn add".len())
        .any(|window| window == b"fn add"));
}

#[test]
fn explorer_rejects_invalid_flags_and_preserves_existing_destination() {
    let fixture = Fixture::new();
    for arguments in [
        vec![
            "explore",
            "semaprax.toml",
            "--format",
            "json",
            "--format",
            "html",
            "--output",
            "duplicate.json",
        ],
        vec![
            "explore",
            "semaprax.toml",
            "--format",
            "json",
            "--output",
            "missing-capsule.json",
            "--candidate-capsule",
            "candidate.json",
        ],
        vec![
            "explore",
            "semaprax.toml",
            "--format",
            "json",
            "--output",
            "unknown.json",
            "--unknown",
        ],
    ] {
        let output = fixture.cli(&arguments);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
    }

    let destination = fixture.0.join("existing.json");
    std::fs::write(&destination, b"foreign sentinel").unwrap();
    let output = fixture.cli(&[
        "explore",
        "semaprax.toml",
        "--format",
        "json",
        "--output",
        "existing.json",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(std::fs::read(&destination).unwrap(), b"foreign sentinel");
    assert_eq!(
        std::fs::read_dir(&fixture.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".semaprax-explore-"))
            .count(),
        0,
        "failed publication must remove its temporary artifact"
    );
}

#[test]
fn include_source_is_explicit_bounded_and_visible_in_json_and_html() {
    let fixture = Fixture::new();
    let secret = "918273645";
    let core = fixture.0.join("src/core.spx");
    let original = std::fs::read_to_string(&core).unwrap();
    let source = format!(
        "{}\n// </pre><script>window.__injected=1</script>\n",
        original.replace("left + right", &format!("left + {secret}"))
    );
    let source = semaprax::parse_canonical(&source, &core).unwrap().1;
    std::fs::write(&core, &source).unwrap();

    let json = fixture.cli(&[
        "explore",
        "semaprax.toml",
        "--format",
        "json",
        "--output",
        "included.json",
        "--include-source",
    ]);
    assert!(json.status.success(), "{json:?}");
    let bytes = std::fs::read(fixture.0.join("included.json")).unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(snapshot["source_included"], true);
    assert_eq!(snapshot["snapshot_digest"], digest(&snapshot));
    let files = snapshot["source_files"].as_array().unwrap();
    assert!(files.iter().any(|file| {
        file["side"] == "current"
            && file["path"] == "src/core.spx"
            && file["text"]
                .as_str()
                .is_some_and(|text| text.contains(secret))
    }));

    let html = fixture.cli(&[
        "explore",
        "semaprax.toml",
        "--format",
        "html",
        "--output",
        "included.html",
        "--include-source",
    ]);
    assert!(html.status.success(), "{html:?}");
    let html = std::fs::read_to_string(fixture.0.join("included.html")).unwrap();
    assert!(html.contains("Source included"));
    assert!(html.contains("complete source text"));
    assert!(html.contains(secret));
    assert!(html.contains("&lt;/pre&gt;&lt;script&gt;window.__injected=1&lt;/script&gt;"));
    assert!(!html.contains("</pre><script>window.__injected=1"));
}

#[test]
fn include_source_is_rejected_for_non_standalone_formats() {
    let fixture = Fixture::new();
    for format in ["markdown", "svg"] {
        let output = fixture.cli(&[
            "explore",
            "semaprax.toml",
            "--format",
            format,
            "--output",
            "source.out",
            "--include-source",
        ]);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(!fixture.0.join("source.out").exists());
    }
}

#[test]
fn replayed_candidate_capsules_export_rename_and_move_source_sides() {
    let renamed = Fixture::new();
    let (rename_digest, rename_capsule) = recovery_capsule(
        &renamed,
        json!({"kind":"rename_declaration","target":"calculator.add","name":"plus"}),
    );
    std::fs::write(renamed.0.join("rename.capsule"), rename_capsule).unwrap();
    let output = renamed.cli(&[
        "explore",
        "semaprax.toml",
        "--candidate-capsule",
        "rename.capsule",
        "--expect-candidate",
        &rename_digest,
        "--target",
        "calculator.add",
        "--format",
        "json",
        "--output",
        "rename.json",
        "--include-source",
    ]);
    assert!(output.status.success(), "{output:?}");
    let rename: Value =
        serde_json::from_slice(&std::fs::read(renamed.0.join("rename.json")).unwrap()).unwrap();
    assert_eq!(rename["source_included"], true);
    assert_eq!(rename["focus_sides"], json!(["candidate", "base"]));
    assert!(rename["views"]
        .as_array()
        .unwrap()
        .iter()
        .any(|view| view["query"]["side"] == "candidate"));
    let rename_sources = rename["source_files"].as_array().unwrap();
    assert!(rename_sources.iter().any(|file| file["side"] == "candidate"
        && file["path"] == "src/core.spx"
        && file["text"].as_str().unwrap().contains("fn plus(")));
    assert!(rename_sources.iter().any(|file| file["side"] == "base"
        && file["path"] == "src/core.spx"
        && file["text"].as_str().unwrap().contains("fn add(")));

    let moved = Fixture::new();
    let manifest_path = moved.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("\"calculator.add\", ", "")
        .replace(
            "\"src/tests.spx\"]",
            "\"src/support.spx\", \"src/tests.spx\"]",
        );
    std::fs::write(manifest_path, manifest).unwrap();
    let support =
        "module calculator.support;\n@id(\"calculator.support.anchor\") fn anchor() -> i64 { 0 }\n";
    let (_, support) = semaprax::parse_canonical(support, "src/support.spx").unwrap();
    std::fs::write(moved.0.join("src/support.spx"), support).unwrap();
    let (move_digest, move_capsule) = recovery_capsule(
        &moved,
        json!({"kind":"move_declaration","target":"calculator.add","destination":"calculator.support.anchor"}),
    );
    std::fs::write(moved.0.join("move.capsule"), move_capsule).unwrap();
    let output = moved.cli(&[
        "explore",
        "semaprax.toml",
        "--candidate-capsule",
        "move.capsule",
        "--expect-candidate",
        &move_digest,
        "--target",
        "calculator.add",
        "--format",
        "json",
        "--output",
        "move.json",
        "--include-source",
    ]);
    assert!(output.status.success(), "{output:?}");
    let moved: Value =
        serde_json::from_slice(&std::fs::read(moved.0.join("move.json")).unwrap()).unwrap();
    let move_sources = moved["source_files"].as_array().unwrap();
    assert!(move_sources.iter().any(|file| file["side"] == "base"
        && file["path"] == "src/core.spx"
        && file["text"].as_str().unwrap().contains("fn add(")));
    assert!(move_sources.iter().any(|file| file["side"] == "candidate"
        && file["path"] == "src/support.spx"
        && file["text"].as_str().unwrap().contains("fn add(")));
}

#[test]
fn candidate_explore_refuses_foreign_tampered_and_wrong_digest_capsules_before_output() {
    let origin = Fixture::new();
    let (candidate_digest, capsule) = recovery_capsule(
        &origin,
        json!({"kind":"rename_declaration","target":"calculator.add","name":"plus"}),
    );

    let foreign = Fixture::new();
    let foreign_core = foreign.0.join("src/core.spx");
    let source = std::fs::read_to_string(&foreign_core).unwrap();
    std::fs::write(&foreign_core, source.replace("left + right", "left + 2")).unwrap();
    std::fs::write(foreign.0.join("foreign.capsule"), &capsule).unwrap();
    let foreign_output = foreign.cli(&[
        "explore",
        "semaprax.toml",
        "--candidate-capsule",
        "foreign.capsule",
        "--expect-candidate",
        &candidate_digest,
        "--format",
        "json",
        "--output",
        "foreign.json",
    ]);
    assert_eq!(foreign_output.status.code(), Some(1), "{foreign_output:?}");
    assert!(!foreign.0.join("foreign.json").exists());

    let tampered = Fixture::new();
    let corrupted = capsule.replacen("calculator.add", "calculator.bad", 1);
    std::fs::write(tampered.0.join("tampered.capsule"), corrupted).unwrap();
    let tampered_output = tampered.cli(&[
        "explore",
        "semaprax.toml",
        "--candidate-capsule",
        "tampered.capsule",
        "--expect-candidate",
        &candidate_digest,
        "--format",
        "json",
        "--output",
        "tampered.json",
    ]);
    assert_eq!(
        tampered_output.status.code(),
        Some(1),
        "{tampered_output:?}"
    );
    assert!(!tampered.0.join("tampered.json").exists());

    let wrong_digest = Fixture::new();
    std::fs::write(wrong_digest.0.join("candidate.capsule"), capsule).unwrap();
    let wrong = format!("sha256:{}", "0".repeat(64));
    let wrong_output = wrong_digest.cli(&[
        "explore",
        "semaprax.toml",
        "--candidate-capsule",
        "candidate.capsule",
        "--expect-candidate",
        &wrong,
        "--format",
        "json",
        "--output",
        "wrong.json",
    ]);
    assert_eq!(wrong_output.status.code(), Some(1), "{wrong_output:?}");
    assert!(!wrong_digest.0.join("wrong.json").exists());
}
