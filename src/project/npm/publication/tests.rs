//! Real verified carriers through held Unix publication; no target execution.
use super::*;
use crate::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, ProjectNpmBuild, PublicApiSubject,
    PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn carrier() -> (ProjectNpmBuild, BTreeMap<String, Vec<u8>>) {
    let source = "module publication.app;\n@id(\"publication.bytes\") fn payload(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }\n@id(\"publication.main\") fn main() -> i64 { 0 }\n";
    let program = crate::hir::resolve(&crate::check(source, "publication.spx").unwrap()).unwrap();
    let fact = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let descriptor = derive_public_api_descriptor(
        &program,
        &["publication.bytes".to_owned()],
        PublicApiSubject {
            project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            project_revision: fact,
            workspace_revision: fact,
            project_graph_digest: fact,
        },
    )
    .unwrap();
    let build = prepare_owned_data_npm_build(
        &program,
        &descriptor,
        "publication-fixture",
        "1.0.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    build.verify().unwrap();
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let artifacts: BTreeMap<_, _> = envelope["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect();
    assert_eq!(
        artifacts.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "app.wasm",
            "package.json",
            "semaprax.api.json",
            "semaprax.bindings.d.ts",
            "semaprax.bindings.js",
            "semaprax.js"
        ]
    );
    (build, artifacts)
}

enum Entry {
    Directory,
    File(Vec<u8>),
    Link(PathBuf),
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-npm-parent-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    root.canonicalize().unwrap()
}

fn add_package(
    expected: &mut BTreeMap<PathBuf, Entry>,
    directory: &Path,
    artifacts: &BTreeMap<String, Vec<u8>>,
) {
    expected.insert(directory.to_path_buf(), Entry::Directory);
    for (name, bytes) in artifacts {
        expected.insert(directory.join(name), Entry::File(bytes.clone()));
    }
}

fn verify_and_remove(
    root: &Path,
    identity: &same_file::Handle,
    expected: BTreeMap<PathBuf, Entry>,
) {
    assert_eq!(&same_file::Handle::from_path(root).unwrap(), identity);
    // Authenticate every fixed entry and complete directory inventory before
    // deleting anything. Symlinks are compared, never traversed for cleanup.
    for (relative, kind) in &expected {
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).unwrap();
        match kind {
            Entry::Directory => {
                assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
                let mut actual = fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name())
                    .collect::<Vec<_>>();
                actual.sort();
                let mut names = expected
                    .keys()
                    .filter(|candidate| {
                        candidate.as_path() != relative.as_path()
                            && candidate.parent() == Some(relative.as_path())
                    })
                    .map(|candidate| candidate.file_name().unwrap().to_os_string())
                    .collect::<Vec<_>>();
                names.sort();
                assert_eq!(actual, names);
            }
            Entry::File(bytes) => {
                assert!(metadata.is_file() && !metadata.file_type().is_symlink());
                assert_eq!(fs::read(path).unwrap(), *bytes);
            }
            Entry::Link(target) => {
                assert!(metadata.file_type().is_symlink());
                assert_eq!(fs::read_link(path).unwrap(), *target);
            }
        }
    }
    for (relative, kind) in &expected {
        if !matches!(kind, Entry::Directory) {
            fs::remove_file(root.join(relative)).unwrap();
        }
    }
    let mut directories = expected
        .iter()
        .filter_map(|(path, kind)| matches!(kind, Entry::Directory).then_some(path))
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        fs::remove_dir(root.join(directory)).unwrap();
    }
}

#[test]
fn healthy_parent_and_existing_alias_publish_exact_verified_inventory() {
    let (build, artifacts) = carrier();
    for alias in [false, true] {
        let root = fixture();
        let identity = same_file::Handle::from_path(&root).unwrap();
        fs::create_dir(root.join("parent")).unwrap();
        let mut expected = BTreeMap::from([
            (PathBuf::new(), Entry::Directory),
            (PathBuf::from("parent"), Entry::Directory),
        ]);
        let requested = if alias {
            symlink(root.join("parent"), root.join("alias")).unwrap();
            expected.insert("alias".into(), Entry::Link(root.join("parent")));
            root.join("alias/package")
        } else {
            root.join("parent/package")
        };
        build.publish(&requested).unwrap();
        add_package(&mut expected, Path::new("parent/package"), &artifacts);
        verify_and_remove(&root, &identity, expected);
    }
}

#[test]
fn replaced_parent_or_ancestor_is_rejected_with_both_trees_retained() {
    let (build, artifacts) = carrier();
    for ancestor in [false, true] {
        for foreign_output in [false, true] {
            let root = fixture();
            let identity = same_file::Handle::from_path(&root).unwrap();
            fs::create_dir(root.join("anchor")).unwrap();
            fs::create_dir(root.join("anchor/parent")).unwrap();
            let requested = root.join("anchor/parent/package");
            let hook_root = root.clone();
            set_test_after_create(Box::new(move || {
                let replaced = hook_root.join(if ancestor { "anchor" } else { "anchor/parent" });
                fs::rename(&replaced, hook_root.join("displaced")).unwrap();
                fs::create_dir(&replaced).unwrap();
                if ancestor {
                    fs::create_dir(hook_root.join("anchor/parent")).unwrap();
                }
                fs::write(hook_root.join("anchor/parent/foreign"), b"foreign parent\n").unwrap();
                if foreign_output {
                    fs::create_dir(hook_root.join("anchor/parent/package")).unwrap();
                    fs::write(
                        hook_root.join("anchor/parent/package/marker"),
                        b"foreign output\n",
                    )
                    .unwrap();
                }
            }));
            let error = build.publish(&requested).unwrap_err();
            assert_eq!(error.code, "SPX-W120");
            assert_eq!(
                error.message,
                "npm package parent identity changed during publication"
            );
            let mut expected = BTreeMap::from([
                (PathBuf::new(), Entry::Directory),
                (PathBuf::from("anchor"), Entry::Directory),
                (PathBuf::from("anchor/parent"), Entry::Directory),
                (PathBuf::from("displaced"), Entry::Directory),
                (
                    PathBuf::from("anchor/parent/foreign"),
                    Entry::File(b"foreign parent\n".to_vec()),
                ),
            ]);
            if ancestor {
                expected.insert("displaced/parent".into(), Entry::Directory);
            }
            add_package(
                &mut expected,
                Path::new(if ancestor {
                    "displaced/parent/package"
                } else {
                    "displaced/package"
                }),
                &artifacts,
            );
            if foreign_output {
                expected.insert("anchor/parent/package".into(), Entry::Directory);
                expected.insert(
                    "anchor/parent/package/marker".into(),
                    Entry::File(b"foreign output\n".to_vec()),
                );
            } else {
                assert_eq!(
                    fs::symlink_metadata(&requested).unwrap_err().kind(),
                    std::io::ErrorKind::NotFound
                );
            }
            verify_and_remove(&root, &identity, expected);
        }
    }
}

#[test]
fn retargeted_original_alias_keeps_the_existing_path_check_and_both_trees() {
    let (build, artifacts) = carrier();
    let root = fixture();
    let identity = same_file::Handle::from_path(&root).unwrap();
    for name in ["parent", "other"] {
        fs::create_dir(root.join(name)).unwrap();
    }
    symlink(root.join("parent"), root.join("alias")).unwrap();
    fs::write(root.join("other/foreign"), b"foreign\n").unwrap();
    let hook_root = root.clone();
    set_test_after_create(Box::new(move || {
        assert_eq!(
            fs::read_link(hook_root.join("alias")).unwrap(),
            hook_root.join("parent")
        );
        fs::remove_file(hook_root.join("alias")).unwrap();
        symlink(hook_root.join("other"), hook_root.join("alias")).unwrap();
    }));
    let error = build.publish(&root.join("alias/package")).unwrap_err();
    assert_eq!(error.code, "SPX-W120");
    assert_eq!(
        error.message,
        "npm package parent identity changed during publication"
    );
    assert_eq!(
        fs::symlink_metadata(root.join("alias/package"))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
    let mut expected = BTreeMap::from([
        (PathBuf::new(), Entry::Directory),
        (PathBuf::from("parent"), Entry::Directory),
        (PathBuf::from("other"), Entry::Directory),
        (PathBuf::from("alias"), Entry::Link(root.join("other"))),
        (
            PathBuf::from("other/foreign"),
            Entry::File(b"foreign\n".to_vec()),
        ),
    ]);
    add_package(&mut expected, Path::new("parent/package"), &artifacts);
    verify_and_remove(&root, &identity, expected);
}
