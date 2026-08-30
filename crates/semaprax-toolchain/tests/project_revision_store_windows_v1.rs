//! Explicit provisioned-host evidence; never run implicitly by ordinary CI.
#![cfg(windows)]

use std::path::{Path, PathBuf};

use semaprax::{project, project_revision_store as store};

#[test]
#[ignore = "requires empty private fixed-local-NTFS SEMAPRAX_WINDOWS_REVISION_STORE_TEST_ROOT with no short aliases"]
fn windows_project_roundtrip_replays_identity_and_preserves_existing_entry() {
    let root = PathBuf::from(
        std::env::var_os("SEMAPRAX_WINDOWS_REVISION_STORE_TEST_ROOT")
            .expect("provision a dedicated empty, exact-DACL, alias-free NTFS test root"),
    );
    assert!(root.is_absolute(), "test store root must be absolute");
    assert_eq!(
        std::fs::read_dir(&root)
            .expect("existing provisioned test root")
            .count(),
        0,
        "never adopt a nonempty test store"
    );
    eprintln!(
        "Windows Project Revision Store fixture is retained at {}",
        root.display()
    );

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/calculator-project/semaprax.toml");
    let revision =
        project::with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();
    let expected = revision.project_revision();
    let legacy = store::identify(&revision, expected).unwrap();
    let locator = store::identify_windows(&revision, expected).unwrap();
    assert_ne!(legacy.entry_digest(), locator.entry_digest());
    assert_eq!(
        store::persist(&root, &revision, expected).unwrap_err()[0].code,
        "SPX-I215"
    );
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);

    let receipt = semaprax_toolchain::persist_windows(&root, &revision, expected).unwrap();
    assert_eq!(receipt.entry_digest(), locator.entry_digest());
    let loaded = semaprax_toolchain::load_windows(&root, receipt.entry_digest(), expected).unwrap();
    assert_eq!(loaded.project_revision(), expected);
    assert_eq!(loaded.workspace_revision(), revision.workspace_revision());
    assert_eq!(
        loaded.semantic_graph_digest(),
        revision.semantic_graph_digest()
    );
    assert_eq!(
        loaded.manifest().to_canonical_toml(),
        revision.manifest().to_canonical_toml()
    );
    assert_eq!(
        loaded
            .sources()
            .iter()
            .map(|source| (source.path(), source.source()))
            .collect::<Vec<_>>(),
        revision
            .sources()
            .iter()
            .map(|source| (source.path(), source.source()))
            .collect::<Vec<_>>()
    );

    let entry = root.join(receipt.entry_digest().strip_prefix("sha256:").unwrap());
    let before = std::fs::read(entry.join("entry.json")).unwrap();
    assert!(String::from_utf8(before.clone())
        .unwrap()
        .contains(store::PROJECT_REVISION_STORE_WINDOWS_ENTRY_SCHEMA));
    assert_eq!(
        semaprax_toolchain::persist_windows(&root, &revision, expected).unwrap_err()[0].code,
        "SPX-G193"
    );
    assert_eq!(std::fs::read(entry.join("entry.json")).unwrap(), before);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    let stale = format!("sha256:{}", "0".repeat(64));
    assert_ne!(stale, expected);
    assert_eq!(
        semaprax_toolchain::load_windows(&root, receipt.entry_digest(), &stale)
            .err()
            .unwrap()[0]
            .code,
        "SPX-G192"
    );
    assert_eq!(std::fs::read(entry.join("entry.json")).unwrap(), before);
    // No cleanup: the host retains the exact owned fixture for inspection.
}
