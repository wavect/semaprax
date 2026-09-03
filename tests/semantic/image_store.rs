//! Source-backed image persistence/refresh evidence authored, intentionally unrun.
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    with_authenticated_project, ImageWorkspace, ProjectRevision, ProjectSemanticImage,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-image-store-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn revision(&self) -> Result<Arc<ProjectRevision>, Vec<Diagnostic>> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
    }
    fn image(&self) -> Arc<ProjectSemanticImage> {
        let revision = self.revision().unwrap();
        let expected = revision.project_revision().to_owned();
        Arc::new(ProjectSemanticImage::derive(revision, &expected).unwrap())
    }
    fn edit(&self) {
        let path = self.0.join("src/core.spx");
        let source = std::fs::read_to_string(&path)
            .unwrap()
            .replace("left + right", "left + right + 1");
        std::fs::write(
            path,
            semaprax::format::canonical(&semaprax::parse(&source, "src/core.spx").unwrap()),
        )
        .unwrap();
    }
    #[cfg(unix)]
    fn store(&self) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let root = self.0.join(".semaprax-images");
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        root
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn code<T>(result: Result<T, Vec<Diagnostic>>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(errors.iter().any(|d| d.code == expected), "{errors:?}"),
    }
}

#[test]
fn unchanged_refresh_reuses_image_arc_and_manual_edit_invalidates_reverse_import_closure() {
    let fixture = Fixture::new();
    let initial = fixture.image();
    let original = initial.image_digest().to_owned();
    let mut workspace = ImageWorkspace::new(Arc::clone(&initial));
    let unchanged = workspace
        .refresh(fixture.revision().unwrap(), &original)
        .unwrap();
    assert!(unchanged.image_reused());
    assert!(Arc::ptr_eq(workspace.image(), &initial));
    let report: Value = serde_json::from_str(unchanged.to_json()).unwrap();
    assert_eq!(report["invalidated_sources"], serde_json::json!([]));
    fixture.edit();
    let refreshed = workspace
        .refresh(fixture.revision().unwrap(), &original)
        .unwrap();
    assert!(!refreshed.image_reused());
    assert!(!Arc::ptr_eq(workspace.image(), &initial));
    let report: Value = serde_json::from_str(refreshed.to_json()).unwrap();
    assert_eq!(
        report["changed_sources"],
        serde_json::json!(["src/core.spx"])
    );
    assert_eq!(
        report["invalidated_sources"],
        serde_json::json!(["src/app.spx", "src/core.spx", "src/tests.spx"])
    );
    assert_eq!(
        report["compiler_work"],
        "complete_source_rebuild_and_image_derivation"
    );
    let retained = Arc::clone(workspace.image());
    code(
        workspace.refresh(fixture.revision().unwrap(), &original),
        "SPX-G251",
    );
    assert!(Arc::ptr_eq(workspace.image(), &retained));
    std::fs::write(fixture.0.join("src/core.spx"), "invalid source").unwrap();
    assert!(fixture.revision().is_err());
    assert!(Arc::ptr_eq(workspace.image(), &retained));
}

#[test]
fn manifest_change_conservatively_invalidates_all_sources_without_claiming_hir_reuse() {
    let fixture = Fixture::new();
    let initial = fixture.image();
    let expected = initial.image_digest().to_owned();
    let mut workspace = ImageWorkspace::new(initial);
    let path = fixture.0.join("semaprax.toml");
    let manifest = std::fs::read_to_string(&path)
        .unwrap()
        .replace("name = \"calculator\"", "name = \"calculator-refresh\"");
    std::fs::write(path, manifest).unwrap();
    let report = workspace
        .refresh(fixture.revision().unwrap(), &expected)
        .unwrap();
    let value: Value = serde_json::from_str(report.to_json()).unwrap();
    assert_eq!(value["manifest_changed"], true);
    assert_eq!(value["changed_sources"], serde_json::json!([]));
    assert_eq!(value["invalidated_sources"].as_array().unwrap().len(), 3);
}

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod disk {
    use super::*;
    use semaprax::project::{
        load_semantic_image, persist_semantic_image, MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES,
    };

    #[test]
    fn cold_load_rebuilds_exact_image_after_dropping_retained_state_and_ignores_working_copy_edits()
    {
        let fixture = Fixture::new();
        let root = fixture.store();
        let (receipt, expected, bytes) = {
            let image = fixture.image();
            let receipt = persist_semantic_image(&root, &image, image.image_digest()).unwrap();
            (
                receipt.to_json().as_bytes().to_vec(),
                image.image_digest().to_owned(),
                image.to_json().to_owned(),
            )
        };
        fixture.edit();
        let loaded = load_semantic_image(&root, &receipt, &expected).unwrap();
        assert_eq!(loaded.image_digest(), expected);
        assert_eq!(loaded.to_json(), bytes);
        assert_ne!(loaded.image_digest(), fixture.image().image_digest());
        assert!(persist_semantic_image(&root, &loaded, &expected).is_err());
    }

    #[test]
    fn corruption_deletion_and_noncanonical_receipts_never_load_as_warm_trusted_images() {
        let fixture = Fixture::new();
        let root = fixture.store();
        let image = fixture.image();
        let receipt = persist_semantic_image(&root, &image, image.image_digest()).unwrap();
        let mut malformed = receipt.to_json().as_bytes().to_vec();
        malformed.push(b'\n');
        code(
            load_semantic_image(&root, &malformed, image.image_digest()),
            "SPX-G249",
        );
        code(
            load_semantic_image(
                &root,
                &vec![b' '; MAX_SEMANTIC_IMAGE_STORE_RECEIPT_BYTES + 1],
                image.image_digest(),
            ),
            "SPX-G250",
        );
        let entry = root.join(receipt.entry_digest().strip_prefix("sha256:").unwrap());
        std::fs::write(entry.join("sources/src/core.spx"), "corrupt source").unwrap();
        assert!(
            load_semantic_image(&root, receipt.to_json().as_bytes(), image.image_digest()).is_err()
        );
        std::fs::remove_dir_all(entry).unwrap();
        assert!(
            load_semantic_image(&root, receipt.to_json().as_bytes(), image.image_digest()).is_err()
        );
    }

    #[test]
    fn compiler_locator_and_expected_image_substitution_are_rejected() {
        let fixture = Fixture::new();
        let root = fixture.store();
        let image = fixture.image();
        let receipt = persist_semantic_image(&root, &image, image.image_digest()).unwrap();
        let mut value: Value = serde_json::from_str(receipt.to_json()).unwrap();
        value["compiler"]["image_compatibility"] = serde_json::json!("unknown-compatibility");
        value.sort_all_objects();
        let modified = serde_json::to_string(&value).unwrap() + "\n";
        code(
            load_semantic_image(&root, modified.as_bytes(), image.image_digest()),
            "SPX-G249",
        );
        code(
            load_semantic_image(
                &root,
                receipt.to_json().as_bytes(),
                &format!("sha256:{}", "0".repeat(64)),
            ),
            "SPX-G251",
        );
        value = serde_json::from_str(receipt.to_json()).unwrap();
        value["revision_store"]["entry_digest"] =
            serde_json::json!(format!("sha256:{}", "0".repeat(64)));
        value.sort_all_objects();
        let modified = serde_json::to_string(&value).unwrap() + "\n";
        assert!(load_semantic_image(&root, modified.as_bytes(), image.image_digest()).is_err());
    }
}
