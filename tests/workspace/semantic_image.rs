//! Authored read-only image evidence; execution is deliberately left to CI.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ProjectRevision, ProjectSemanticImage, MAX_SEMANTIC_IMAGE_BYTES,
};
use semaprax::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FILES: &[&str] = &[
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-semantic-image-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in FILES {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.manifest(), |snapshot| Ok(snapshot.retain_revision()))
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                entries.insert(path.strip_prefix(root).unwrap().to_path_buf(), Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    match result {
        Ok(_) => panic!("expected {code}, operation succeeded"),
        Err(errors) => assert!(errors.iter().any(|error| error.code == code), "{errors:?}"),
    }
}

fn derive(revision: &Arc<ProjectRevision>) -> ProjectSemanticImage {
    ProjectSemanticImage::derive(Arc::clone(revision), revision.project_revision()).unwrap()
}

fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.semantic-workspace-image.digest.v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    let hex = hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

#[test]
fn exact_image_binds_declared_sources_graph_and_typed_indexes_without_writes() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let image = derive(&revision);
    let wire: Value = serde_json::from_str(image.to_json()).unwrap();
    assert_eq!(wire["schema"], "semaprax.semantic-workspace-image.v1");
    assert_eq!(wire["compiler"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        wire["compiler"]["image_compatibility"],
        "semaprax.semantic-workspace-image-compatibility.v1"
    );
    assert_eq!(wire["project_revision"], revision.project_revision());
    assert_eq!(wire["workspace_revision"], revision.workspace_revision());
    assert_eq!(
        wire["canonical_manifest"],
        revision.manifest().to_canonical_toml()
    );
    assert_eq!(
        wire["canonical_workspace_manifest"],
        revision.workspace_manifest()
    );
    assert_eq!(
        wire["project_graph_digest"],
        revision.semantic_graph_digest()
    );
    assert_eq!(
        wire["project_graph"],
        serde_json::from_str::<Value>(revision.semantic_graph()).unwrap()
    );
    let expected_sources = revision
        .sources()
        .iter()
        .map(|source| {
            json!({
                "path": source.path(),
                "source_graph_schema": source.source_graph_schema(),
                "source_revision": source.source_revision(),
                "source_digest": source.source_digest(),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(wire["sources"], json!(expected_sources));
    let stable_ids = wire["indexes"]["stable_ids"].as_array().unwrap();
    assert!(stable_ids
        .iter()
        .any(|entry| entry["id"] == "calculator.add"));
    let declarations = wire["project_graph"]["declarations"].as_array().unwrap();
    assert_eq!(stable_ids.len(), declarations.len());
    for entry in stable_ids {
        let ordinal = entry["graph_declaration"].as_u64().unwrap() as usize;
        assert_eq!(entry["id"], declarations[ordinal]["id"]);
    }
    let edges = wire["project_graph"]["edges"].as_array().unwrap();
    for (name, endpoint) in [("forward", "caller"), ("reverse", "target")] {
        let adjacency = wire["indexes"][name].as_array().unwrap();
        assert!(!adjacency.is_empty());
        let mut seen = vec![0; edges.len()];
        for entry in adjacency {
            for ordinal in entry["edges"].as_array().unwrap() {
                let ordinal = ordinal.as_u64().unwrap() as usize;
                seen[ordinal] += 1;
                let edge = &edges[ordinal];
                if edge["kind"] == "call" {
                    assert_eq!(entry["node"]["kind"], "declaration");
                    assert_eq!(entry["node"]["id"], edge[endpoint]);
                }
            }
        }
        assert!(seen.iter().all(|count| *count == 1));
    }
    assert!(image.to_json().ends_with('\n'));
    assert!(!image.to_json().ends_with("\n\n"));
    assert_eq!(image.image_digest(), digest(image.to_json().as_bytes()));
    assert_eq!(image.to_json(), derive(&revision).to_json());
    let other_root = Fixture::new();
    assert_eq!(image.to_json(), derive(&other_root.revision()).to_json());
    let replay = ProjectSemanticImage::replay(
        Arc::clone(&revision),
        revision.project_revision(),
        image.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(image.image_digest(), replay.image_digest());
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn symbol_and_analysis_use_existing_revision_indexes_and_image_selection() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let image = derive(&revision);
    let symbol = image
        .symbol(image.image_digest(), "calculator.add")
        .unwrap();
    assert!(!symbol.ends_with('\n'));
    let symbol: Value = serde_json::from_str(&symbol).unwrap();
    assert_eq!(
        symbol["schema"],
        "semaprax.semantic-workspace-image-symbol.v1"
    );
    assert_eq!(symbol["image_revision"], image.image_digest());
    assert_eq!(symbol["symbol"]["id"], "calculator.add");
    assert_eq!(symbol["symbol"]["name"], "add");
    assert_eq!(symbol["symbol"]["path"], "src/core.spx");
    assert_eq!(symbol["symbol"]["edge_scope"], "six_cross_file_families");
    assert!(
        symbol["symbol"]["direct_cross_file_callers"]
            .as_u64()
            .unwrap()
            > 0
    );
    let kind = WorkspaceAnalysisTargetKind::Declaration;
    assert_eq!(
        image
            .context(
                image.image_digest(),
                kind,
                "calculator.add",
                WorkspaceContextOptions::default()
            )
            .unwrap(),
        revision
            .semantic_context(kind, "calculator.add", WorkspaceContextOptions::default())
            .unwrap()
    );
    assert_eq!(
        image
            .impact(
                image.image_digest(),
                kind,
                "calculator.add",
                WorkspaceImpactOptions::default()
            )
            .unwrap(),
        revision
            .semantic_impact(kind, "calculator.add", WorkspaceImpactOptions::default())
            .unwrap()
    );
    let stale = format!("sha256:{}", "0".repeat(64));
    assert_code(image.symbol(&stale, "calculator.add"), "SPX-G221");
    assert_code(
        image.context(
            &stale,
            kind,
            "calculator.add",
            WorkspaceContextOptions::default(),
        ),
        "SPX-G221",
    );
    assert_code(
        image.impact(
            &stale,
            kind,
            "calculator.add",
            WorkspaceImpactOptions::default(),
        ),
        "SPX-G221",
    );
    for id in ["", "unknown.declaration", "invalid\0id"] {
        assert_code(image.symbol(image.image_digest(), id), "SPX-G219");
    }
    assert_code(image.symbol("bad-digest", "calculator.add"), "SPX-G219");
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn replay_rejects_noncanonical_tampered_reminted_stale_and_oversized_inputs() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let image = derive(&revision);
    let replay = |bytes: &[u8]| {
        ProjectSemanticImage::replay(Arc::clone(&revision), revision.project_revision(), bytes)
    };
    let canonical = image.to_json();
    assert_code(
        replay(canonical.trim_end_matches('\n').as_bytes()),
        "SPX-G221",
    );
    assert_code(replay(format!("{canonical}\n").as_bytes()), "SPX-G221");
    let value: Value = serde_json::from_str(canonical).unwrap();
    assert_code(
        replay(serde_json::to_string_pretty(&value).unwrap().as_bytes()),
        "SPX-G221",
    );
    assert_code(replay(b"{not-json}"), "SPX-G221");
    assert_code(replay(&[0xff]), "SPX-G221");
    let mut changed = value;
    changed["sources"][0]["source_digest"] = json!(format!("sha256:{}", "0".repeat(64)));
    let reminted = format!("{}\n", serde_json::to_string(&changed).unwrap());
    let forged_digest = digest(reminted.as_bytes());
    assert_ne!(forged_digest, image.image_digest());
    assert_code(replay(reminted.as_bytes()), "SPX-G221");
    assert_code(image.symbol(&forged_digest, "calculator.add"), "SPX-G221");
    assert_code(replay(&vec![b' '; MAX_SEMANTIC_IMAGE_BYTES]), "SPX-G221");
    assert_code(
        replay(&vec![b' '; MAX_SEMANTIC_IMAGE_BYTES + 1]),
        "SPX-G220",
    );
    let stale = format!("sha256:{}", "0".repeat(64));
    assert_code(
        ProjectSemanticImage::derive(Arc::clone(&revision), &stale),
        "SPX-G221",
    );
    assert_code(
        ProjectSemanticImage::replay(Arc::clone(&revision), &stale, canonical.as_bytes()),
        "SPX-G221",
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn old_image_remains_immutable_but_live_held_snapshot_rejects_source_drift() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let image = derive(&revision);
    let original = image.to_json().to_owned();
    let core = fixture.0.join("src/core.spx");
    let source = std::fs::read_to_string(&core).unwrap();
    let changed = source.replace("fn add(", "fn sum(");
    assert_ne!(source, changed);
    let mut expected_after = inventory(&fixture.0);
    expected_after.insert(PathBuf::from("src/core.spx"), changed.as_bytes().to_vec());
    let result = with_authenticated_project(&fixture.manifest(), |snapshot| {
        let retained = snapshot.retain_revision();
        let derived =
            ProjectSemanticImage::derive(Arc::clone(&retained), retained.project_revision())?;
        std::fs::write(&core, &changed).unwrap();
        Ok(derived)
    });
    assert_code(result, "SPX-J102");
    assert_eq!(image.to_json(), original);
    assert!(image
        .symbol(image.image_digest(), "calculator.add")
        .unwrap()
        .contains("\"name\":\"add\""));
    let new_revision = fixture.revision();
    let new_image = derive(&new_revision);
    assert_ne!(new_image.image_digest(), image.image_digest());
    assert_code(
        ProjectSemanticImage::replay(
            Arc::clone(&new_revision),
            new_revision.project_revision(),
            original.as_bytes(),
        ),
        "SPX-G221",
    );
    assert_eq!(inventory(&fixture.0), expected_after);
}
