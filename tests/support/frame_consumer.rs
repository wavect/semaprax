//! Prepare only the unchanged external Rust frame consumer's four input files.
//! The caller owns a quiescent, canonical empty directory and its sibling SDK.
//! These pathname checks are not held authority or protection against races.
//! No compiler is selected or run, and no input, SDK, or target is deleted.

use std::fs::{self, Metadata, OpenOptions};
use std::io::Write as _;
use std::path::Path;

const SDK_PATH: &str = "../frame-payload-generated-sdk";
const MANIFEST: &str = include_str!("../../examples/frame-payload-rust/Cargo.toml");
const LOCK: &[u8] = include_bytes!("../../examples/frame-payload-rust/Cargo.lock");
const SOURCE: &[u8] = include_bytes!("../../examples/frame-payload-rust/src/main.rs");

pub(crate) fn prepare(root: &Path, label: &str, corpus: &[u8]) {
    let manifest = manifest(label);
    assert_root(root);
    assert_names(root, &[]);
    fs::create_dir(root.join("src")).unwrap();
    for (name, bytes) in inputs(&manifest, corpus) {
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(root.join(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
    assert_unchanged(root, label, corpus);
}

pub(crate) fn assert_unchanged(root: &Path, label: &str, corpus: &[u8]) {
    let manifest = manifest(label);
    assert_root(root);
    assert_names(root, &["Cargo.toml", "Cargo.lock", "corpus.json", "src"]);
    let source_directory = root.join("src");
    assert_directory(&source_directory);
    assert_names(&source_directory, &["main.rs"]);
    // Authenticate every leaf's current type and exact size before reading any
    // bytes. This is a fixed test inventory, not filesystem discovery authority.
    for (name, bytes) in inputs(&manifest, corpus) {
        let metadata = fs::symlink_metadata(root.join(name)).unwrap();
        assert_plain(&metadata);
        assert!(metadata.is_file(), "{name} must remain a regular file");
        assert_eq!(metadata.len(), bytes.len() as u64, "{name} length");
    }
    for (name, bytes) in inputs(&manifest, corpus) {
        assert_eq!(fs::read(root.join(name)).unwrap(), bytes, "{name} bytes");
    }
}

fn manifest(label: &str) -> String {
    assert!(
        matches!(label, "before" | "after"),
        "unknown frame subject label"
    );
    assert_eq!(MANIFEST.matches(SDK_PATH).count(), 1);
    MANIFEST.replace(SDK_PATH, &format!("../{label}-generated-sdk"))
}

fn inputs<'a>(manifest: &'a str, corpus: &'a [u8]) -> [(&'static str, &'a [u8]); 4] {
    [
        ("Cargo.toml", manifest.as_bytes()),
        ("Cargo.lock", LOCK),
        ("src/main.rs", SOURCE),
        ("corpus.json", corpus),
    ]
}

fn assert_root(root: &Path) {
    assert!(root.is_absolute(), "consumer root must be absolute");
    assert_directory(root);
    assert_eq!(
        root.canonicalize().unwrap(),
        root,
        "consumer root must be canonical"
    );
}

fn assert_directory(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert_plain(&metadata);
    assert!(metadata.is_dir(), "{} must be a directory", path.display());
}

fn assert_plain(metadata: &Metadata) {
    assert!(!metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0, "reparse point");
    }
}

fn assert_names(root: &Path, expected: &[&str]) {
    let mut actual = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const BASE: &[u8] = include_bytes!("../../examples/frame-payload-project/corpus.json");
    const SUPPLEMENT: &[u8] = include_bytes!("../frame_payload_product_v1/adversarial.json");
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn empty_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "semaprax-frame-consumer-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        eprintln!("retained frame consumer helper fixture: {}", root.display());
        root
    }

    #[test]
    fn preparation_preserves_exact_inputs_for_both_labels_and_corpora() {
        for (label, dependency) in [
            ("before", "../before-generated-sdk"),
            ("after", "../after-generated-sdk"),
        ] {
            for corpus in [BASE, SUPPLEMENT] {
                let root = empty_root();
                prepare(&root, label, corpus);
                assert_unchanged(&root, label, corpus);
                let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
                assert_eq!(manifest.matches(dependency).count(), 1);
                // Invert only the literal substitution and compare the whole
                // checked-in manifest, including its lock/dependency policy.
                assert_eq!(manifest.replace(dependency, SDK_PATH), MANIFEST);
                assert_eq!(fs::read(root.join("Cargo.lock")).unwrap(), LOCK);
                assert_eq!(fs::read(root.join("src/main.rs")).unwrap(), SOURCE);
                assert_eq!(fs::read(root.join("corpus.json")).unwrap(), corpus);
            }
        }
    }

    #[test]
    fn invalid_labels_and_nonempty_roots_reject_before_writes() {
        let root = empty_root();
        for label in ["baseline", "../before", "before/other", ""] {
            assert!(std::panic::catch_unwind(|| prepare(&root, label, BASE)).is_err());
            assert_names(&root, &[]);
        }
        let sentinel = root.join("sentinel");
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&sentinel)
            .unwrap()
            .write_all(b"retained caller bytes")
            .unwrap();
        assert!(std::panic::catch_unwind(|| prepare(&root, "before", BASE)).is_err());
        assert_names(&root, &["sentinel"]);
        assert_eq!(fs::read(sentinel).unwrap(), b"retained caller bytes");
    }

    #[test]
    fn post_execution_check_rejects_extra_or_changed_inputs_without_repair() {
        for name in ["Cargo.toml", "Cargo.lock", "src/main.rs", "corpus.json"] {
            let root = empty_root();
            prepare(&root, "before", BASE);
            let path = root.join(name);
            let mut changed = fs::read(&path).unwrap();
            assert!(!changed.is_empty());
            changed[0] ^= 1; // Same size: length alone cannot explain rejection.
            fs::write(&path, &changed).unwrap();
            assert!(std::panic::catch_unwind(|| assert_unchanged(&root, "before", BASE)).is_err());
            assert_eq!(fs::read(path).unwrap(), changed, "no repair of {name}");
        }
        let root = empty_root();
        prepare(&root, "after", SUPPLEMENT);
        let extra = root.join("unexpected");
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&extra)
            .unwrap()
            .write_all(b"retained extra input")
            .unwrap();
        assert!(std::panic::catch_unwind(|| assert_unchanged(&root, "after", SUPPLEMENT)).is_err());
        assert_eq!(fs::read(extra).unwrap(), b"retained extra input");
        assert_eq!(fs::read(root.join("corpus.json")).unwrap(), SUPPLEMENT);
    }
}
