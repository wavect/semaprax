//! Exact source snapshots retain their original document location and bytes.
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const REGISTRY: &str = include_str!("archived-sources.tsv");

fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut hex, "{byte:02x}").unwrap();
    }
    hex
}

fn entries() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    REGISTRY
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let columns: Vec<_> = line.split('\t').collect();
            assert_eq!(columns.len(), 3, "archive registry row: {line}");
            (columns[0], columns[1], columns[2])
        })
}

/// Only a named, byte-exact snapshot receives historical-path treatment.
/// Unknown evidence documents still require normal metadata and cataloging.
pub(super) fn origin(root: &Path, document: &Path) -> Option<PathBuf> {
    let (_, original, expected) =
        entries().find(|(archive, _, _)| root.join(archive) == document)?;
    let bytes = std::fs::read(document).unwrap();
    assert_eq!(sha256(&bytes), expected, "{}", document.display());
    let original = root.join(original);
    assert!(
        original.is_file(),
        "missing archive origin: {}",
        original.display()
    );
    Some(original)
}

#[test]
fn snapshot_registry_has_unique_canonical_paths_and_exact_existing_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = std::collections::BTreeSet::new();
    for (archive, original, digest) in entries() {
        assert!(paths.insert(archive), "duplicate archive: {archive}");
        for path in [archive, original] {
            assert!(Path::new(path)
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_))));
        }
        assert!(archive.starts_with("docs/evidence/"));
        assert!(archive.ends_with(".md"));
        assert!(original.starts_with("docs/") || original == "CHANGELOG.md");
        assert_eq!(digest.len(), 64);
        assert!(digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
        assert_eq!(origin(root, &root.join(archive)), Some(root.join(original)));
    }
    assert!(!paths.is_empty());
    assert_eq!(origin(root, &root.join("docs/QUALITY-GATES.md")), None);
    assert_eq!(
        origin(root, &root.join("docs/evidence/unregistered/README.md")),
        None
    );
}
