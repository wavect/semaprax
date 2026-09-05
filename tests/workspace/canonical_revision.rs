use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, AgentDefinitions, AuthorityPolicies, ContractsAndTests,
    DependencyClosure, ProjectRevision, ProjectSemanticImage, ProjectionMetadata, SemanticProgram,
    SemanticWorkspaceRevision, SourceProjection, StableIdentityIndex, TargetProfiles,
    MAX_SEMANTIC_WORKSPACE_REVISION_BYTES, SEMANTIC_WORKSPACE_REVISION_SCHEMA,
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
    fn calculator(label: &str, core_edit: impl FnOnce(String) -> String) -> Self {
        let root = temporary(label);
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in FILES {
            let bytes = std::fs::read(example.join(file)).unwrap();
            std::fs::write(root.join(file), bytes).unwrap();
        }
        let core = std::fs::read_to_string(root.join("src/core.spx")).unwrap();
        std::fs::write(root.join("src/core.spx"), core_edit(core)).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn package(label: &str, version: &str, dependency_range: Option<&str>) -> Self {
        let fixture = Self::calculator(label, |source| source);
        let dependency = dependency_range
            .map(|range| format!("\n[dependencies]\nstd.core = \"{range}\"\n"))
            .unwrap_or_default();
        let manifest = format!(
            "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"calculator\"\nversion = \"{version}\"\n\n[modules]\nentry = \"calculator.app\"\nsources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]\ntests = [\"calculator.tests\"]\n\n[exports]\nweb = [\"calculator.add\", \"calculator.divide\", \"calculator.is-negative\", \"calculator.multiply\", \"calculator.not\", \"calculator.subtract\"]\n{dependency}"
        );
        std::fs::write(fixture.manifest(), manifest).unwrap();
        fixture
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

fn temporary(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-canonical-revision-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, entries: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                entries.insert(relative, Vec::new());
                visit(root, &path, entries);
            } else {
                entries.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn framed(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn sequence<'a>(domain: &[u8], values: impl IntoIterator<Item = &'a str>) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for value in values {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hash.finalize())
    )
}

fn canonical(mut value: Value) -> String {
    value.sort_all_objects();
    serde_json::to_string(&value).unwrap() + "\n"
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

fn remint_source_projection(document: &str) -> (String, String) {
    const NODE_DOMAIN: &[u8] =
        b"semaprax.semantic-workspace-revision.source-projection.digest.v1\0";
    const REVISION_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.digest.v1\0";
    let mut value: Value = serde_json::from_str(document).unwrap();
    value["nodes"]["source_projection"]["value"]["payload"]["files"][0]["bytes"] = json!(
        value["nodes"]["source_projection"]["value"]["payload"]["files"][0]["bytes"]
            .as_u64()
            .unwrap()
            + 1
    );
    let node = canonical(value["nodes"]["source_projection"]["value"].clone());
    let node_digest = framed(NODE_DOMAIN, node.as_bytes());
    value["nodes"]["source_projection"]["digest"] = json!(node_digest);
    value["digests"]["source_projection"] = json!(node_digest);
    let workspace_revision = sequence(
        REVISION_DOMAIN,
        [
            value["digests"]["semantic"].as_str().unwrap(),
            value["digests"]["source_projection"].as_str().unwrap(),
            value["digests"]["manifest"].as_str().unwrap(),
            value["digests"]["dependency_lock"].as_str().unwrap(),
        ],
    );
    value["workspace_revision"] = json!(workspace_revision);
    (canonical(value), workspace_revision)
}

#[test]
fn canonical_revision_components_replay_and_legacy_bytes_are_read_only() {
    const SEMANTIC_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.semantic.digest.v1\0";
    const MANIFEST_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.manifest.digest.v1\0";
    const DEPENDENCY_DOMAIN: &[u8] =
        b"semaprax.semantic-workspace-revision.dependency-lock.digest.v1\0";
    const REVISION_DOMAIN: &[u8] = b"semaprax.semantic-workspace-revision.digest.v1\0";

    let fixture = Fixture::calculator("components", |source| source);
    let revision = fixture.revision();
    let before = inventory(&fixture.0);
    let legacy = (
        revision.project_revision().to_owned(),
        revision.workspace_revision().to_owned(),
        revision.semantic_graph().to_owned(),
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision())
            .unwrap()
            .to_json()
            .to_owned(),
    );
    let workspace = revision.canonical_workspace_revision().unwrap();
    assert_eq!(
        workspace.to_json(),
        SemanticWorkspaceRevision::derive(&revision)
            .unwrap()
            .to_json()
    );
    let wire: Value = serde_json::from_str(workspace.to_json()).unwrap();
    assert_eq!(wire["schema"], SEMANTIC_WORKSPACE_REVISION_SCHEMA);

    let nodes = [
        (
            "source_projection",
            SourceProjection::SCHEMA,
            workspace.source_projection().to_json(),
            workspace.source_projection().digest(),
        ),
        (
            "semantic_program",
            SemanticProgram::SCHEMA,
            workspace.semantic_program().to_json(),
            workspace.semantic_program().digest(),
        ),
        (
            "stable_identity_index",
            StableIdentityIndex::SCHEMA,
            workspace.stable_identity_index().to_json(),
            workspace.stable_identity_index().digest(),
        ),
        (
            "dependency_closure",
            DependencyClosure::SCHEMA,
            workspace.dependency_closure().to_json(),
            workspace.dependency_closure().digest(),
        ),
        (
            "contracts_and_tests",
            ContractsAndTests::SCHEMA,
            workspace.contracts_and_tests().to_json(),
            workspace.contracts_and_tests().digest(),
        ),
        (
            "agent_definitions",
            AgentDefinitions::SCHEMA,
            workspace.agent_definitions().to_json(),
            workspace.agent_definitions().digest(),
        ),
        (
            "authority_policies",
            AuthorityPolicies::SCHEMA,
            workspace.authority_policies().to_json(),
            workspace.authority_policies().digest(),
        ),
        (
            "target_profiles",
            TargetProfiles::SCHEMA,
            workspace.target_profiles().to_json(),
            workspace.target_profiles().digest(),
        ),
        (
            "projection_metadata",
            ProjectionMetadata::SCHEMA,
            workspace.projection_metadata().to_json(),
            workspace.projection_metadata().digest(),
        ),
    ];
    assert_eq!(nodes.len(), 9);
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.3)
            .collect::<BTreeSet<_>>()
            .len(),
        9
    );
    for (name, schema, document, digest) in nodes {
        let node: Value = serde_json::from_str(document).unwrap();
        assert_eq!(node["schema"], schema);
        assert_eq!(wire["nodes"][name]["digest"], digest);
        assert_eq!(wire["nodes"][name]["value"], node);
    }

    let semantic_digest = sequence(
        SEMANTIC_DOMAIN,
        [
            workspace.semantic_program().digest(),
            workspace.stable_identity_index().digest(),
            workspace.contracts_and_tests().digest(),
            workspace.agent_definitions().digest(),
            workspace.authority_policies().digest(),
            workspace.target_profiles().digest(),
        ],
    );
    assert_eq!(workspace.semantic_digest(), semantic_digest);
    assert_eq!(
        workspace.source_projection_digest(),
        workspace.source_projection().digest()
    );
    assert_eq!(
        workspace.manifest_digest(),
        framed(
            MANIFEST_DOMAIN,
            revision.manifest().to_canonical_toml().as_bytes()
        )
    );
    assert_eq!(
        workspace.dependency_lock_digest(),
        framed(
            DEPENDENCY_DOMAIN,
            workspace.dependency_closure().to_json().as_bytes()
        )
    );
    assert_eq!(
        workspace.workspace_revision(),
        sequence(
            REVISION_DOMAIN,
            [
                workspace.semantic_digest(),
                workspace.source_projection_digest(),
                workspace.manifest_digest(),
                workspace.dependency_lock_digest(),
            ],
        )
    );

    let replay = SemanticWorkspaceRevision::replay(
        &revision,
        workspace.workspace_revision(),
        workspace.to_json().as_bytes(),
    )
    .unwrap();
    assert_eq!(replay, workspace);
    let mut tampered = workspace.to_json().as_bytes().to_vec();
    tampered[0] ^= 1;
    assert_code(
        SemanticWorkspaceRevision::replay(&revision, workspace.workspace_revision(), &tampered),
        "SPX-G222",
    );
    assert_code(
        SemanticWorkspaceRevision::replay(
            &revision,
            workspace.workspace_revision(),
            &vec![b' '; MAX_SEMANTIC_WORKSPACE_REVISION_BYTES + 1],
        ),
        "SPX-G222",
    );
    let (reminted, reminted_revision) = remint_source_projection(workspace.to_json());
    assert_code(
        SemanticWorkspaceRevision::replay(&revision, &reminted_revision, reminted.as_bytes()),
        "SPX-G223",
    );

    assert_eq!(revision.project_revision(), legacy.0);
    assert_eq!(revision.workspace_revision(), legacy.1);
    assert_eq!(revision.semantic_graph(), legacy.2);
    assert_eq!(
        ProjectSemanticImage::derive(Arc::clone(&revision), revision.project_revision())
            .unwrap()
            .to_json(),
        legacy.3
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn canonical_revision_separates_projection_comments_from_behavior() {
    let base_fixture = Fixture::calculator("base", |source| source);
    let comment_fixture = Fixture::calculator("comment", |source| {
        format!("// projection-only comment\n{source}")
    });
    let behavior_fixture = Fixture::calculator("behavior", |source| {
        source.replacen("left + right", "left - right", 1)
    });
    let base_revision = base_fixture.revision();
    let comment_revision = comment_fixture.revision();
    let behavior_revision = behavior_fixture.revision();
    let base = SemanticWorkspaceRevision::derive(&base_revision).unwrap();
    let comment = SemanticWorkspaceRevision::derive(&comment_revision).unwrap();
    let behavior = SemanticWorkspaceRevision::derive(&behavior_revision).unwrap();

    assert_eq!(base.semantic_digest(), comment.semantic_digest());
    assert_ne!(
        base.source_projection_digest(),
        comment.source_projection_digest()
    );
    assert_ne!(base.workspace_revision(), comment.workspace_revision());
    assert_ne!(base.semantic_digest(), behavior.semantic_digest());
    assert_ne!(base.workspace_revision(), behavior.workspace_revision());
    assert_code(
        SemanticWorkspaceRevision::replay(
            &behavior_revision,
            base.workspace_revision(),
            base.to_json().as_bytes(),
        ),
        "SPX-G223",
    );

    let duplicate_fixture = Fixture::calculator("duplicate-root", |source| source);
    let duplicate = SemanticWorkspaceRevision::derive(&duplicate_fixture.revision()).unwrap();
    assert_eq!(base.to_json(), duplicate.to_json());
    assert_eq!(base.workspace_revision(), duplicate.workspace_revision());
}

#[test]
fn canonical_revision_tracks_manifest_and_dependency_descriptions() {
    let manifest_a = Fixture::package("manifest-a", "0.1.0", None);
    let manifest_b = Fixture::package("manifest-b", "0.1.1", None);
    let manifest_a = SemanticWorkspaceRevision::derive(&manifest_a.revision()).unwrap();
    let manifest_b = SemanticWorkspaceRevision::derive(&manifest_b.revision()).unwrap();
    assert_eq!(manifest_a.semantic_digest(), manifest_b.semantic_digest());
    assert_eq!(
        manifest_a.dependency_lock_digest(),
        manifest_b.dependency_lock_digest()
    );
    assert_ne!(manifest_a.manifest_digest(), manifest_b.manifest_digest());
    assert_ne!(
        manifest_a.workspace_revision(),
        manifest_b.workspace_revision()
    );

    let dependency_a = Fixture::package("dependency-a", "0.1.0", Some("~0.1.0"));
    let dependency_b = Fixture::package("dependency-b", "0.1.0", Some("^0.1.0"));
    let dependency_a = SemanticWorkspaceRevision::derive(&dependency_a.revision()).unwrap();
    let dependency_b = SemanticWorkspaceRevision::derive(&dependency_b.revision()).unwrap();
    assert_eq!(
        dependency_a.semantic_digest(),
        dependency_b.semantic_digest()
    );
    assert_ne!(
        dependency_a.dependency_lock_digest(),
        dependency_b.dependency_lock_digest()
    );
    assert_ne!(
        dependency_a.manifest_digest(),
        dependency_b.manifest_digest()
    );
    assert_ne!(
        dependency_a.workspace_revision(),
        dependency_b.workspace_revision()
    );
}
