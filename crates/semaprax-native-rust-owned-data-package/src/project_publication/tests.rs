//! Physical namespace-race evidence. Hooks never exist in production builds.
use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FILES: [(&str, &[u8]); 4] = [
    ("README.md", b"readme\n"),
    ("semaprax.toml", b"manifest\n"),
    ("src/app.spx", b"app\n"),
    ("src/tests.spx", b"tests\n"),
];

#[test]
fn stage_and_output_collision_rejects_before_creating_children() {
    for output in ["stage", "STAGE"] {
        let root = fixture();
        assert!(matches!(
            NewProjectAuthority::create(&root, OsStr::new(output), OsStr::new("stage")),
            Err(NewProjectAuthorityError::Invalid)
        ));
        assert!(names(&root).is_empty());
        fs::create_dir(root.join(output)).unwrap();
        assert!(matches!(
            NewProjectAuthority::create(&root, OsStr::new(output), OsStr::new("stage")),
            Err(NewProjectAuthorityError::Exists)
        ));
        assert_eq!(names(&root), [OsString::from(output)]);
        assert!(names(&root.join(output)).is_empty());
        fs::remove_dir(root.join(output)).unwrap();
        fs::remove_dir(root).unwrap();
    }
}

fn fixture() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-new-binding-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}

fn authority(parent: &Path) -> NewProjectAuthority {
    let mut authority =
        NewProjectAuthority::create(parent, OsStr::new("project"), OsStr::new("stage")).unwrap();
    for (path, bytes) in FILES {
        authority.write(path, bytes).unwrap();
    }
    authority
}

fn names(path: &Path) -> Vec<OsString> {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    assert_no_reparse(&metadata);
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn remove_file(path: &Path, bytes: &[u8]) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    assert_no_reparse(&metadata);
    assert_eq!(fs::read(path).unwrap(), bytes);
    fs::remove_file(path).unwrap();
}

fn assert_project(path: &Path) {
    assert_eq!(names(path), ["README.md", "semaprax.toml", "src"]);
    assert_eq!(names(&path.join("src")), ["app.spx", "tests.spx"]);
    for (relative, bytes) in FILES {
        let file = path.join(relative);
        let metadata = fs::symlink_metadata(&file).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert_no_reparse(&metadata);
        assert_eq!(fs::read(file).unwrap(), bytes);
    }
}

fn assert_no_reparse(metadata: &fs::Metadata) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    #[cfg(not(windows))]
    let _ = metadata;
}

fn remove_project(path: &Path) {
    assert_project(path);
    for (relative, bytes) in FILES {
        remove_file(&path.join(relative), bytes);
    }
    fs::remove_dir(path.join("src")).unwrap();
    fs::remove_dir(path).unwrap();
}

#[test]
fn published_tree_binds_the_original_parent_and_output() {
    let root = fixture();
    let mut authority = authority(&root);
    authority.publish_and_verify(FILES).unwrap();
    assert!(authority.published);
    drop(authority);
    assert_eq!(names(&root), ["project"]);
    remove_project(&root.join("project"));
    fs::remove_dir(root).unwrap();
}

#[cfg(unix)]
#[test]
fn post_rename_output_displacement_is_changed_without_rollback() {
    for replace in [false, true] {
        let root = fixture();
        let mut authority = authority(&root);
        let target = root.clone();
        authority.after_rename = Some(Box::new(move || {
            fs::rename(target.join("project"), target.join("displaced")).unwrap();
            if replace {
                fs::create_dir(target.join("project")).unwrap();
                fs::write(target.join("project/foreign"), b"foreign\n").unwrap();
            }
        }));
        assert_eq!(
            authority.publish_and_verify(FILES),
            Err(NewProjectAuthorityError::Changed)
        );
        assert!(authority.published);
        drop(authority);
        assert_project(&root.join("displaced"));
        if replace {
            assert_eq!(names(&root), ["displaced", "project"]);
            assert_eq!(names(&root.join("project")), ["foreign"]);
            remove_file(&root.join("project/foreign"), b"foreign\n");
            fs::remove_dir(root.join("project")).unwrap();
        } else {
            assert_eq!(names(&root), ["displaced"]);
        }
        remove_project(&root.join("displaced"));
        fs::remove_dir(root).unwrap();
    }
}

#[test]
fn post_rename_unwind_cannot_recover_staging_cleanup_authority() {
    let root = fixture();
    let mut authority = authority(&root);
    authority.after_rename = Some(Box::new(|| panic!("after publication")));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        authority.publish_and_verify(FILES).unwrap();
    }));
    assert!(outcome.is_err());
    assert_eq!(names(&root), ["project"]);
    remove_project(&root.join("project"));
    fs::remove_dir(root).unwrap();
}

#[cfg(unix)]
#[test]
fn post_rename_parent_displacement_is_changed_without_rollback() {
    let root = fixture();
    let parent = root.join("parent");
    fs::create_dir(&parent).unwrap();
    let mut authority = authority(&parent);
    let target = root.clone();
    authority.after_rename = Some(Box::new(move || {
        fs::rename(target.join("parent"), target.join("displaced")).unwrap();
        fs::create_dir(target.join("parent")).unwrap();
        fs::write(target.join("parent/foreign"), b"foreign\n").unwrap();
    }));
    assert_eq!(
        authority.publish_and_verify(FILES),
        Err(NewProjectAuthorityError::Changed)
    );
    assert!(authority.published);
    drop(authority);
    assert_eq!(names(&root), ["displaced", "parent"]);
    assert_eq!(names(&root.join("displaced")), ["project"]);
    assert_project(&root.join("displaced/project"));
    assert_eq!(names(&parent), ["foreign"]);
    remove_file(&parent.join("foreign"), b"foreign\n");
    fs::remove_dir(parent).unwrap();
    remove_project(&root.join("displaced/project"));
    fs::remove_dir(root.join("displaced")).unwrap();
    fs::remove_dir(root).unwrap();
}

#[test]
fn partial_untracked_stage_is_not_adopted_for_cleanup() {
    let root = fixture();
    let mut authority =
        NewProjectAuthority::create(&root, OsStr::new("project"), OsStr::new("stage")).unwrap();
    authority.write("README.md", FILES[0].1).unwrap();
    fs::write(root.join("stage/src/foreign"), b"foreign\n").unwrap();
    assert_eq!(
        authority.publish_and_verify(FILES),
        Err(NewProjectAuthorityError::Changed)
    );
    assert!(!authority.published);
    drop(authority);
    assert_eq!(names(&root), ["stage"]);
    assert_eq!(names(&root.join("stage")), ["README.md", "src"]);
    assert_eq!(names(&root.join("stage/src")), ["foreign"]);
    remove_file(&root.join("stage/README.md"), FILES[0].1);
    remove_file(&root.join("stage/src/foreign"), b"foreign\n");
    fs::remove_dir(root.join("stage/src")).unwrap();
    fs::remove_dir(root.join("stage")).unwrap();
    fs::remove_dir(root).unwrap();
}
