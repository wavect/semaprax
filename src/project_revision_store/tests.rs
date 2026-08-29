use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::project_revision_store::unix::StorePoint;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
    store: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let ordinal = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-project-revision-store-{label}-{}-{ordinal}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let store = directory.join("store");
        std::fs::create_dir(&store).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self { directory, store }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn revision() -> std::sync::Arc<ProjectRevision> {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
    crate::project::load_snapshot(&manifest)
        .unwrap()
        .retain_revision()
}

fn prepared(revision: &ProjectRevision) -> PreparedEntry {
    PreparedEntry::from_revision(revision, revision.project_revision()).unwrap()
}

#[test]
fn exact_revision_round_trips_through_one_content_addressed_entry() {
    let fixture = Fixture::new("round-trip");
    let revision = revision();
    let before_manifest = revision.manifest().to_canonical_toml();
    let before_sources = revision
        .sources()
        .iter()
        .map(|source| (source.path().to_owned(), source.source().to_owned()))
        .collect::<Vec<_>>();
    let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
    let loaded = load(
        &fixture.store,
        receipt.entry_digest(),
        receipt.project_revision(),
    )
    .unwrap();
    assert_eq!(loaded.project_revision(), revision.project_revision());
    assert_eq!(loaded.workspace_revision(), revision.workspace_revision());
    assert_eq!(
        loaded.semantic_graph_digest(),
        revision.semantic_graph_digest()
    );
    assert_eq!(loaded.manifest().to_canonical_toml(), before_manifest);
    assert_eq!(
        loaded
            .sources()
            .iter()
            .map(|source| (source.path().to_owned(), source.source().to_owned()))
            .collect::<Vec<_>>(),
        before_sources
    );
    assert_eq!(std::fs::read_dir(&fixture.store).unwrap().count(), 1);
}

#[test]
fn stale_subject_rejects_before_the_first_store_effect() {
    let fixture = Fixture::new("stale");
    let revision = revision();
    let stale = format!("sha256:{}", "0".repeat(64));
    let error = persist(&fixture.store, &revision, &stale).unwrap_err();
    assert_eq!(error[0].code, "SPX-G192");
    assert_eq!(std::fs::read_dir(&fixture.store).unwrap().count(), 0);
}

#[test]
fn stale_load_subject_rejects_without_mutating_the_entry() {
    let fixture = Fixture::new("stale-load");
    let revision = revision();
    let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
    let entry = fixture
        .store
        .join(receipt.entry_digest().trim_start_matches("sha256:"));
    let before = std::fs::read(entry.join("entry.json")).unwrap();
    let stale = format!("sha256:{}", "0".repeat(64));
    let error = load(&fixture.store, receipt.entry_digest(), &stale)
        .err()
        .expect("stale load subject must reject");
    assert_eq!(error[0].code, "SPX-G192");
    assert_eq!(std::fs::read(entry.join("entry.json")).unwrap(), before);
}

#[test]
fn exact_destination_collision_is_no_clobber_and_preserves_bytes() {
    let fixture = Fixture::new("collision");
    let revision = revision();
    let first = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
    let marker = fixture
        .store
        .join(first.entry_digest().trim_start_matches("sha256:"))
        .join("entry.json");
    let before = std::fs::read(&marker).unwrap();
    let error = persist(&fixture.store, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert_eq!(std::fs::read(marker).unwrap(), before);
}

#[test]
fn unrelated_retained_metadata_is_authenticated_once_per_invocation() {
    let fixture = Fixture::new("retained-once");
    let revision = revision();
    persist(&fixture.store, &revision, revision.project_revision()).unwrap();

    let mut candidate = prepared(&revision);
    candidate.project_graph_digest = format!("sha256:{}", "0".repeat(64));
    candidate.entry_json = render_entry_fixed_point(&candidate).unwrap();
    candidate.entry_digest = framed_digest(ENTRY_DIGEST_DOMAIN, &candidate.entry_json);
    unix::reset_retained_metadata_authentications();
    let error = unix::persist_with_hook(&fixture.store, &candidate, |_, _| Ok(()))
        .expect_err("foreign candidate graph binding must reject");
    assert!(matches!(error[0].code, "SPX-G192" | "SPX-G193"));
    assert_eq!(unix::retained_metadata_authentications(), 1);
}

#[test]
fn foreign_root_bytes_fail_closed_before_stage_creation() {
    let fixture = Fixture::new("foreign-root");
    let revision = revision();
    std::fs::write(fixture.store.join("foreign"), b"owned by somebody else").unwrap();
    let error = persist(&fixture.store, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert_eq!(
        std::fs::read(fixture.store.join("foreign")).unwrap(),
        b"owned by somebody else"
    );
}

#[test]
fn owned_partial_stage_is_preserved_and_blocks_future_adoption() {
    let fixture = Fixture::new("partial-stage");
    let revision = revision();
    let prepared = prepared(&revision);
    let error = unix::persist_with_hook(&fixture.store, &prepared, |point, _| {
        if point == StorePoint::AfterStageWrite {
            return Err(std::io::Error::other("injected stop"));
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I215");
    let stage = fixture
        .store
        .join(format!(".stage-{}", prepared.entry_hex()));
    assert!(stage.join("entry.json").is_file());
    let later = persist(&fixture.store, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(later[0].code, "SPX-G193");
    assert!(stage.join("entry.json").is_file());
}

#[test]
fn staged_byte_drift_is_rejected_and_never_published_or_deleted() {
    let fixture = Fixture::new("stage-drift");
    let revision = revision();
    let prepared = prepared(&revision);
    let stage = fixture
        .store
        .join(format!(".stage-{}", prepared.entry_hex()));
    let error = unix::persist_with_hook(&fixture.store, &prepared, |point, _| {
        if point == StorePoint::BeforePublish {
            std::fs::write(stage.join("entry.json"), b"{}\n")?;
        }
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error[0].code, "SPX-G190" | "SPX-G193"));
    assert!(stage.is_dir());
    assert!(!fixture.store.join(prepared.entry_hex()).exists());
}

#[test]
fn same_byte_stage_child_replacement_is_identity_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("same-byte-child");
    let revision = revision();
    let prepared = prepared(&revision);
    let stage = fixture
        .store
        .join(format!(".stage-{}", prepared.entry_hex()));
    let displaced = fixture.directory.join("displaced-entry.json");
    let error = unix::persist_with_hook(&fixture.store, &prepared, |point, _| {
        if point == StorePoint::BeforePublish {
            let path = stage.join("entry.json");
            let bytes = std::fs::read(&path)?;
            std::fs::rename(&path, &displaced)?;
            std::fs::write(&path, bytes)?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert_eq!(std::fs::read(displaced).unwrap(), prepared.entry_json);
    assert!(!fixture.store.join(prepared.entry_hex()).exists());
}

#[test]
fn non_absolute_and_non_normalized_roots_reject_before_effect() {
    let revision = revision();
    for root in [
        PathBuf::from("relative"),
        std::env::temp_dir().join("../tmp"),
    ] {
        let error = persist(&root, &revision, revision.project_revision()).unwrap_err();
        assert_eq!(error[0].code, "SPX-G193");
    }
}

#[test]
fn store_root_rejects_non_private_permissions_before_effect() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("root-permissions");
    let revision = revision();
    std::fs::set_permissions(&fixture.store, std::fs::Permissions::from_mode(0o750)).unwrap();
    let error = persist(&fixture.store, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert_eq!(std::fs::read_dir(&fixture.store).unwrap().count(), 0);
}

#[test]
fn symlink_root_and_same_path_root_substitution_fail_closed() {
    use std::os::unix::fs::symlink;

    let revision = revision();
    let symlink_fixture = Fixture::new("root-symlink");
    let alias = symlink_fixture.directory.join("store-alias");
    symlink(&symlink_fixture.store, &alias).unwrap();
    let error = persist(&alias, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert_eq!(
        std::fs::read_dir(&symlink_fixture.store).unwrap().count(),
        0
    );

    let fixture = Fixture::new("root-substitution");
    let prepared = prepared(&revision);
    let displaced = fixture.directory.join("displaced-store");
    let error = unix::persist_with_hook(&fixture.store, &prepared, |point, root| {
        if point == StorePoint::AfterStageCreate {
            std::fs::rename(root, &displaced)?;
            std::fs::create_dir(root)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert!(displaced
        .join(format!(".stage-{}", prepared.entry_hex()))
        .is_dir());
    assert_eq!(std::fs::read_dir(&fixture.store).unwrap().count(), 0);
}

#[test]
fn same_name_stage_substitution_is_identity_rejected_with_foreign_bytes_preserved() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("stage-substitution");
    let revision = revision();
    let prepared = prepared(&revision);
    let stage = fixture
        .store
        .join(format!(".stage-{}", prepared.entry_hex()));
    let displaced = fixture.store.join("displaced-foreign-stage");
    let error = unix::persist_with_hook(&fixture.store, &prepared, |point, _| {
        if point == StorePoint::BeforePublish {
            std::fs::rename(&stage, &displaced)?;
            symlink(&displaced, &stage)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert!(displaced.join("entry.json").is_file());
    assert!(std::fs::symlink_metadata(stage)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn post_pivot_uncertainty_preserves_a_complete_loadable_entry() {
    let fixture = Fixture::new("post-pivot");
    let revision = revision();
    let prepared = prepared(&revision);
    let error = unix::persist_with_hook(&fixture.store, &prepared, |point, _| {
        if point == StorePoint::AfterPublish {
            return Err(std::io::Error::other("injected uncertainty"));
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I216");
    assert!(fixture.store.join(prepared.entry_hex()).is_dir());
    let loaded = load(
        &fixture.store,
        &prepared.entry_digest,
        &prepared.project_revision,
    )
    .unwrap();
    assert_eq!(loaded.project_revision(), revision.project_revision());
}

#[test]
fn truncation_and_nested_foreign_inventory_both_fail_closed() {
    let revision = revision();
    for mutation in ["truncate", "foreign"] {
        let fixture = Fixture::new(mutation);
        let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
        let entry = fixture
            .store
            .join(receipt.entry_digest().trim_start_matches("sha256:"));
        if mutation == "truncate" {
            std::fs::write(entry.join("entry.json"), b"{").unwrap();
        } else {
            std::fs::write(entry.join("sources/foreign.spx"), b"foreign").unwrap();
        }
        let error = load(
            &fixture.store,
            receipt.entry_digest(),
            receipt.project_revision(),
        )
        .err()
        .expect("mutated store entry must reject");
        assert!(matches!(error[0].code, "SPX-G190" | "SPX-G193"));
        assert!(entry.exists());
    }
}

#[test]
fn every_stored_byte_and_digest_binding_is_replayed_not_trusted() {
    let revision = revision();
    for mutation in [
        "manifest",
        "workspace",
        "source",
        "graph-digest",
        "permissions",
    ] {
        let fixture = Fixture::new(mutation);
        let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
        let entry = fixture
            .store
            .join(receipt.entry_digest().trim_start_matches("sha256:"));
        match mutation {
            "manifest" => {
                let mut bytes = std::fs::read(entry.join("semaprax.toml")).unwrap();
                bytes.extend_from_slice(b" ");
                std::fs::write(entry.join("semaprax.toml"), bytes).unwrap();
            }
            "workspace" => {
                let mut bytes = std::fs::read(entry.join("workspace-manifest.json")).unwrap();
                bytes.extend_from_slice(b" ");
                std::fs::write(entry.join("workspace-manifest.json"), bytes).unwrap();
            }
            "source" => {
                let source = entry.join("sources/src/app.spx");
                let mut bytes = std::fs::read(&source).unwrap();
                bytes.extend_from_slice(b" ");
                std::fs::write(source, bytes).unwrap();
            }
            "graph-digest" => {
                let metadata = entry.join("entry.json");
                let source = std::fs::read_to_string(&metadata).unwrap();
                let replaced = source.replacen(
                    revision.semantic_graph_digest(),
                    &format!("sha256:{}", "0".repeat(64)),
                    1,
                );
                assert_ne!(source, replaced);
                std::fs::write(metadata, replaced).unwrap();
            }
            "permissions" => {
                use std::os::unix::fs::PermissionsExt;

                let source = entry.join("sources/src/app.spx");
                std::fs::set_permissions(source, std::fs::Permissions::from_mode(0o644)).unwrap();
            }
            _ => unreachable!(),
        }
        let error = load(
            &fixture.store,
            receipt.entry_digest(),
            receipt.project_revision(),
        )
        .err()
        .expect("mutated binding must reject");
        assert!(matches!(
            error[0].code,
            "SPX-G190" | "SPX-G191" | "SPX-G192" | "SPX-G193"
        ));
        assert!(entry.exists());
    }
}

#[test]
fn canonical_entry_and_digest_are_deterministic_and_depth_is_closed() {
    let revision = revision();
    let first = prepared(&revision);
    let second = prepared(&revision);
    assert_eq!(first.entry_json, second.entry_json);
    assert_eq!(first.entry_digest, second.entry_digest);
    let depth_plus_one = (0..=MAX_STORE_SOURCE_PATH_DEPTH)
        .map(|index| {
            if index == MAX_STORE_SOURCE_PATH_DEPTH {
                "x.spx".to_owned()
            } else {
                "x".to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let error = validate_source_path(&depth_plus_one).unwrap_err();
    assert_eq!(error[0].code, "SPX-G190");
    assert!(unix::require_publication_capacity(MAX_STORE_ENTRIES - 1).is_ok());
    let error = unix::require_publication_capacity(MAX_STORE_ENTRIES).unwrap_err();
    assert_eq!(error[0].code, "SPX-G191");
}

#[test]
fn every_frozen_numeric_limit_accepts_exact_and_rejects_plus_one() {
    for (field, maximum) in [
        ("manifest_bytes", MAX_STORE_MANIFEST_BYTES),
        (
            "workspace_manifest_bytes",
            MAX_STORE_WORKSPACE_MANIFEST_BYTES,
        ),
        ("sources", MAX_STORE_SOURCES),
        ("total_source_bytes", MAX_STORE_TOTAL_SOURCE_BYTES),
        ("entry_json_bytes", MAX_STORE_ENTRY_JSON_BYTES),
        ("inventory_entries", MAX_STORE_INVENTORY_ENTRIES),
    ] {
        assert!(require_max(field, maximum, maximum).is_ok());
        assert_eq!(
            require_max(field, maximum + 1, maximum).unwrap_err()[0].code,
            "SPX-G191"
        );
    }
    let exact = format!(
        "{}/{}/{}/{}.spx",
        "x".repeat(60),
        "x".repeat(60),
        "x".repeat(60),
        "x".repeat(MAX_STORE_SOURCE_PATH_BYTES - 187),
    );
    assert_eq!(exact.len(), MAX_STORE_SOURCE_PATH_BYTES);
    assert!(validate_source_path(&exact).is_ok());
    let error = validate_source_path(&format!("x{exact}")).unwrap_err();
    assert_eq!(error[0].code, "SPX-G190");
}

#[test]
fn selected_entry_workspace_fact_failure_stays_inside_store_diagnostics() {
    let revision = revision();
    let mut prepared = prepared(&revision);
    prepared.sources[0].source_graph_schema = "foreign.workspace.graph".to_owned();
    prepared.entry_json = render_entry_fixed_point(&prepared).unwrap();
    prepared.entry_digest = framed_digest(ENTRY_DIGEST_DOMAIN, &prepared.entry_json);
    let stored = StoredEntry {
        entry_json: prepared.entry_json.clone(),
        manifest: prepared.manifest.clone(),
        workspace_manifest: prepared.workspace_manifest.clone(),
        sources: prepared
            .sources
            .iter()
            .map(|source| (source.path.clone(), source.source.clone()))
            .collect(),
    };
    let error = replay_stored(stored, &prepared.entry_digest, &prepared.project_revision)
        .err()
        .expect("foreign selected Workspace facts must reject");
    assert_eq!(error[0].code, "SPX-G192");
}

#[test]
fn source_hardlink_and_symlink_substitution_are_rejected() {
    use std::os::unix::fs::symlink;

    let revision = revision();
    for mutation in ["hardlink", "symlink"] {
        let fixture = Fixture::new(mutation);
        let receipt = persist(&fixture.store, &revision, revision.project_revision()).unwrap();
        let entry = fixture
            .store
            .join(receipt.entry_digest().trim_start_matches("sha256:"));
        let source = entry.join("sources/src/app.spx");
        let displaced = entry.join("sources/src/app.displaced");
        std::fs::rename(&source, &displaced).unwrap();
        if mutation == "hardlink" {
            std::fs::hard_link(&displaced, &source).unwrap();
        } else {
            symlink(&displaced, &source).unwrap();
        }
        let error = load(
            &fixture.store,
            receipt.entry_digest(),
            receipt.project_revision(),
        )
        .err()
        .expect("substituted store entry must reject");
        assert_eq!(error[0].code, "SPX-G193");
        assert!(displaced.exists());
    }
}

#[test]
fn existing_foreign_digest_directory_is_collision_not_adoption() {
    let fixture = Fixture::new("foreign-collision");
    let revision = revision();
    let prepared = prepared(&revision);
    let destination = fixture.store.join(prepared.entry_hex());
    std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("foreign"), b"preserve").unwrap();
    let error = persist(&fixture.store, &revision, revision.project_revision()).unwrap_err();
    assert_eq!(error[0].code, "SPX-G193");
    assert_eq!(
        std::fs::read(destination.join("foreign")).unwrap(),
        b"preserve"
    );
}

#[test]
fn legacy_project_and_transport_sources_gain_no_store_dependency() {
    for manifest in [
        include_str!("../../examples/calculator-project/semaprax.toml"),
        include_str!("../../examples/config-validator-project/semaprax.toml"),
        include_str!("../../examples/binary-frame-project/semaprax.toml"),
        include_str!("../../examples/spxgrep-project/semaprax.toml"),
        include_str!("../../examples/spxgrep-native-command-project/semaprax.toml"),
        include_str!("../../examples/spxgrep-language-command-project/semaprax.toml"),
        include_str!("../../examples/spxgrep-lines-project/semaprax.toml"),
        include_str!("../../examples/frame-payload-project/semaprax.toml"),
    ] {
        assert_eq!(
            ProjectManifest::parse(manifest)
                .unwrap()
                .to_canonical_toml(),
            manifest
        );
    }
    for transport_source in [
        include_str!("../project_transport/mod.rs"),
        include_str!("../project_transport/session.rs"),
        include_str!("../project_transport/session/workflow.rs"),
        include_str!("../bin/semapraxd.rs"),
    ] {
        assert!(!transport_source.contains("project_revision_store"));
    }
}
