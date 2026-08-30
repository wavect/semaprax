use super::*;
use std::collections::BTreeMap;

fn root() -> PathBuf {
    let path = std::env::temp_dir().join(std::format!(
        "semaprax-scratch-test-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}

// All fixtures are fixed, small and inert. Preflight the entire expected tree
// before non-recursive cleanup; a failed test retains its files for inspection.
fn finish(root: &Path, directories: &[&str], files: &[(&str, &[u8])]) {
    let mut inventory = BTreeMap::<PathBuf, Vec<std::ffi::OsString>>::new();
    inventory.insert(root.to_path_buf(), Vec::new());
    for relative in directories {
        inventory.insert(root.join(relative), Vec::new());
    }
    for relative in directories
        .iter()
        .copied()
        .chain(files.iter().map(|(name, _)| *name))
    {
        let path = root.join(relative);
        inventory
            .get_mut(path.parent().unwrap())
            .expect("fixture parent is declared")
            .push(path.file_name().unwrap().to_owned());
    }
    for (directory, expected) in &mut inventory {
        plain(directory, true).unwrap();
        let mut actual = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual, *expected);
    }
    for (relative, bytes) in files {
        let path = root.join(relative);
        plain(&path, false).unwrap();
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (relative, _) in files {
        fs::remove_file(root.join(relative)).unwrap();
    }
    let mut directories = directories
        .iter()
        .map(|name| root.join(name))
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        fs::remove_dir(directory).unwrap();
    }
    fs::remove_dir(root).unwrap();
}

#[test]
fn created_source_is_sealed_and_only_explicit_cleanup_removes_it() {
    let root = root();
    let mut scratch =
        Scratch::create_in(&root, "source.c", Some(b"source"), || "owned".into()).unwrap();
    assert_eq!(fs::read(scratch.path()).unwrap(), b"source");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            fs::metadata(&scratch.directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
    scratch.seal().unwrap();
    scratch.cleanup().unwrap();
    finish(&root, &[], &[]);
}

#[test]
fn output_is_absent_until_link_then_adopted_and_cleaned() {
    let root = root();
    let leaf = std::format!("program{}", std::env::consts::EXE_SUFFIX);
    let mut scratch = Scratch::create_in(&root, &leaf, None, || "owned".into()).unwrap();
    assert!(!scratch.path().exists());
    assert!(scratch.file_identity.is_none());
    fs::write(scratch.path(), b"linked output").unwrap();
    scratch.seal().unwrap();
    assert!(scratch.file_identity.is_some());
    scratch.cleanup().unwrap();
    finish(&root, &[], &[]);
}

#[test]
fn colliding_files_and_directories_are_not_adopted_or_modified() {
    let root = root();
    fs::write(root.join("file"), b"foreign").unwrap();
    fs::create_dir(root.join("directory")).unwrap();
    let mut candidates = ["file", "directory", "fresh"].into_iter();
    let mut scratch = Scratch::create_in(&root, "source.c", Some(b"owned"), || {
        candidates.next().unwrap().into()
    })
    .unwrap();
    scratch.seal().unwrap();
    scratch.cleanup().unwrap();
    let mut attempts = 0;
    assert!(Scratch::create_in(&root, "source.c", Some(b"never"), || {
        attempts += 1;
        "file".into()
    })
    .is_err());
    assert_eq!(attempts, MAX_ATTEMPTS);
    finish(&root, &["directory"], &[("file", b"foreign")]);
}

#[test]
fn drop_and_unsealed_cleanup_preserve_partial_outputs() {
    let root = root();
    let scratch = Scratch::create_in(&root, "program", None, || "dropped".into()).unwrap();
    fs::write(scratch.path(), b"partial").unwrap();
    drop(scratch);
    let scratch = Scratch::create_in(&root, "program", None, || "unsealed".into()).unwrap();
    fs::write(scratch.path(), b"partial").unwrap();
    assert!(scratch.cleanup().is_err());
    let mut source = Scratch::create_in(&root, "source.c", Some(b"input"), || {
        "failed-compiler".into()
    })
    .unwrap();
    source.seal().unwrap();
    drop(source);
    finish(
        &root,
        &["dropped", "unsealed", "failed-compiler"],
        &[
            ("dropped/program", b"partial"),
            ("unsealed/program", b"partial"),
            ("failed-compiler/source.c", b"input"),
        ],
    );
}

#[test]
fn foreign_inventory_stops_cleanup_before_the_owned_file_is_removed() {
    let root = root();
    let mut scratch =
        Scratch::create_in(&root, "source.c", Some(b"owned"), || "owned".into()).unwrap();
    scratch.seal().unwrap();
    fs::write(scratch.directory.join("foreign"), b"sentinel").unwrap();
    assert!(scratch.cleanup().is_err());
    finish(
        &root,
        &["owned"],
        &[("owned/source.c", b"owned"), ("owned/foreign", b"sentinel")],
    );
}

#[test]
fn invalid_leaf_names_are_rejected_before_directory_creation() {
    let root = root();
    for leaf in ["", ".", "..", "../escape", "sub/file", "file/", "file/."] {
        assert!(Scratch::create_in(&root, leaf, Some(b"never"), || "never".into()).is_err());
    }
    finish(&root, &[], &[]);
}

#[test]
fn multiply_linked_output_is_not_adopted_for_cleanup() {
    let root = root();
    fs::write(root.join("foreign"), b"sentinel").unwrap();
    let mut scratch = Scratch::create_in(&root, "program", None, || "owned".into()).unwrap();
    fs::hard_link(root.join("foreign"), scratch.path()).unwrap();
    assert!(scratch.seal().is_err());
    drop(scratch);
    finish(
        &root,
        &["owned"],
        &[("foreign", b"sentinel"), ("owned/program", b"sentinel")],
    );
}

#[cfg(unix)]
#[test]
fn replaced_file_and_directory_preserve_original_and_foreign_objects() {
    let root = root();
    let mut scratch =
        Scratch::create_in(&root, "source.c", Some(b"original"), || "owned".into()).unwrap();
    scratch.seal().unwrap();
    fs::rename(scratch.path(), root.join("displaced.c")).unwrap();
    fs::write(scratch.path(), b"foreign").unwrap();
    assert!(scratch.cleanup().is_err());
    let mut directory =
        Scratch::create_in(&root, "source.c", Some(b"original"), || "directory".into()).unwrap();
    directory.seal().unwrap();
    fs::rename(&directory.directory, root.join("moved")).unwrap();
    fs::create_dir(&directory.directory).unwrap();
    fs::write(directory.path(), b"foreign").unwrap();
    assert!(directory.cleanup().is_err());
    finish(
        &root,
        &["owned", "directory", "moved"],
        &[
            ("displaced.c", b"original"),
            ("owned/source.c", b"foreign"),
            ("directory/source.c", b"foreign"),
            ("moved/source.c", b"original"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn displaced_parent_preserves_both_trees() {
    let root = root();
    fs::create_dir(root.join("parent")).unwrap();
    let mut scratch =
        Scratch::create_in(&root.join("parent"), "source.c", Some(b"original"), || {
            "owned".into()
        })
        .unwrap();
    scratch.seal().unwrap();
    fs::rename(root.join("parent"), root.join("moved-parent")).unwrap();
    fs::create_dir(root.join("parent")).unwrap();
    fs::create_dir(root.join("parent/owned")).unwrap();
    fs::write(scratch.path(), b"foreign").unwrap();
    assert!(scratch.cleanup().is_err());
    finish(
        &root,
        &[
            "parent",
            "parent/owned",
            "moved-parent",
            "moved-parent/owned",
        ],
        &[
            ("parent/owned/source.c", b"foreign"),
            ("moved-parent/owned/source.c", b"original"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn dangling_collision_and_output_symlink_are_preserved() {
    use std::os::unix::fs::symlink;
    let root = root();
    symlink("missing", root.join("collision")).unwrap();
    assert!(Scratch::create_in(&root, "source.c", Some(b"never"), || "collision".into()).is_err());
    fs::write(root.join("foreign"), b"sentinel").unwrap();
    symlink("foreign", root.join("linked-collision")).unwrap();
    assert!(Scratch::create_in(&root, "source.c", Some(b"never"), || {
        "linked-collision".into()
    })
    .is_err());
    let mut scratch = Scratch::create_in(&root, "program", None, || "owned".into()).unwrap();
    symlink(root.join("foreign"), scratch.path()).unwrap();
    assert!(scratch.seal().is_err());
    drop(scratch);
    assert_eq!(
        fs::read_link(root.join("collision")).unwrap(),
        Path::new("missing")
    );
    assert_eq!(
        fs::read_link(root.join("owned/program")).unwrap(),
        root.join("foreign")
    );
    assert_eq!(fs::read(root.join("foreign")).unwrap(), b"sentinel");
    assert_eq!(
        fs::read_link(root.join("linked-collision")).unwrap(),
        Path::new("foreign")
    );
    plain(&root, true).unwrap();
    plain(&root.join("owned"), true).unwrap();
    let mut entries = fs::read_dir(&root)
        .unwrap()
        .map(|row| row.unwrap().file_name())
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        ["collision", "foreign", "linked-collision", "owned"].map(std::ffi::OsString::from)
    );
    assert_eq!(
        fs::read_dir(root.join("owned"))
            .unwrap()
            .map(|row| row.unwrap().file_name())
            .collect::<Vec<_>>(),
        [std::ffi::OsString::from("program")]
    );
    plain(&root.join("foreign"), false).unwrap();
    // Remove only the individually authenticated links, never their targets.
    fs::remove_file(root.join("collision")).unwrap();
    fs::remove_file(root.join("linked-collision")).unwrap();
    fs::remove_file(root.join("owned/program")).unwrap();
    finish(&root, &["owned"], &[("foreign", b"sentinel")]);
}
