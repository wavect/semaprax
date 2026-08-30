//! Forced extended-class rejection followed by the real native legacy call.
//! Per-request controls exist only in this crate's unit-test compilation.
use super::*;
use std::ffi::OsString;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Destination {
    Absent,
    File,
    Directory,
}

fn checked_metadata(path: &Path) -> std::fs::Metadata {
    use std::os::windows::fs::MetadataExt as _;
    let metadata = std::fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    metadata
}

fn check_directory(path: &Path, expected: &[&str]) {
    assert!(checked_metadata(path).is_dir());
    let mut observed = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    observed.sort();
    let mut names = expected
        .iter()
        .map(|name| OsString::from(*name))
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(observed, names);
}

fn check_file(path: &Path, expected: &[u8]) {
    assert!(checked_metadata(path).is_file());
    assert_eq!(std::fs::read(path).unwrap(), expected);
}

fn exercise(destination: Destination) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-legacy-rename-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)
        .expect("Windows rename fixture requires a writable local temporary directory");
    let root = root.canonicalize().unwrap();
    let parent = hold_directory(&root).unwrap();
    let stage = create_directory_new(&parent, OsStr::new("stage"), 0o700).unwrap();
    std::fs::write(root.join("stage/owned"), b"owned stage bytes").unwrap();
    let output = root.join("output");
    match destination {
        Destination::Absent => {}
        Destination::File => std::fs::write(&output, b"foreign file bytes").unwrap(),
        Destination::Directory => {
            std::fs::create_dir(&output).unwrap();
            std::fs::write(output.join("foreign"), b"foreign directory bytes").unwrap();
        }
    }
    let mut name = prepare_relative_name_arena(5).unwrap();
    set_relative_name_arena(&mut name, OsStr::new("stage")).unwrap();
    let mut prepared = prepare_publish_directory(OsStr::new("output")).unwrap();
    prepared.force_extended_rejection = true;
    let capacity = prepared_publish_directory_owned_capacity(&prepared);
    let result =
        publish_directory_new_prepared(&mut prepared, &parent, &stage, &name, OsStr::new("output"));
    assert_eq!(
        prepared.observed_extended_flags,
        Some(windows_sys::Win32::System::WindowsProgramming::FILE_RENAME_FLAG_POSIX_SEMANTICS)
    );
    // Observe both the complete union field and the actual first byte which
    // the legacy information class consumes as BOOLEAN ReplaceIfExists.
    assert_eq!(prepared.observed_legacy_flags, Some((0, 0)));
    assert_eq!(
        prepared_publish_directory_owned_capacity(&prepared),
        capacity
    );
    assert_eq!(prepared_publish_directory_remaining(&prepared), 0);
    assert_eq!(
        publish_directory_new_prepared(&mut prepared, &parent, &stage, &name, OsStr::new("output")),
        Err(Error::Invalid)
    );
    match destination {
        Destination::Absent => {
            assert_eq!(result, Ok(()));
            assert!(!root.join("stage").exists());
            assert_eq!(
                std::fs::read(output.join("owned")).unwrap(),
                b"owned stage bytes"
            );
        }
        Destination::File => {
            assert_eq!(result, Err(Error::Exists));
            assert_eq!(std::fs::read(&output).unwrap(), b"foreign file bytes");
            assert_eq!(
                std::fs::read(root.join("stage/owned")).unwrap(),
                b"owned stage bytes"
            );
        }
        Destination::Directory => {
            assert_eq!(result, Err(Error::Exists));
            assert_eq!(
                std::fs::read(output.join("foreign")).unwrap(),
                b"foreign directory bytes"
            );
            assert_eq!(
                std::fs::read(root.join("stage/owned")).unwrap(),
                b"owned stage bytes"
            );
        }
    }
    drop(stage);
    drop(parent);
    // Validate the complete fixed removal plan before deleting any entry.
    // Failure retains the fixture for inspection; never recurse or broaden.
    match destination {
        Destination::Absent => {
            check_directory(&root, &["output"]);
            check_directory(&output, &["owned"]);
            check_file(&output.join("owned"), b"owned stage bytes");
        }
        Destination::File => {
            check_directory(&root, &["output", "stage"]);
            check_directory(&root.join("stage"), &["owned"]);
            check_file(&output, b"foreign file bytes");
            check_file(&root.join("stage/owned"), b"owned stage bytes");
        }
        Destination::Directory => {
            check_directory(&root, &["output", "stage"]);
            check_directory(&root.join("stage"), &["owned"]);
            check_directory(&output, &["foreign"]);
            check_file(&output.join("foreign"), b"foreign directory bytes");
            check_file(&root.join("stage/owned"), b"owned stage bytes");
        }
    }
    match destination {
        Destination::Absent => {
            std::fs::remove_file(output.join("owned")).unwrap();
            std::fs::remove_dir(output).unwrap();
        }
        Destination::File => {
            std::fs::remove_file(output).unwrap();
            std::fs::remove_file(root.join("stage/owned")).unwrap();
            std::fs::remove_dir(root.join("stage")).unwrap();
        }
        Destination::Directory => {
            std::fs::remove_file(output.join("foreign")).unwrap();
            std::fs::remove_dir(output).unwrap();
            std::fs::remove_file(root.join("stage/owned")).unwrap();
            std::fs::remove_dir(root.join("stage")).unwrap();
        }
    }
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn legacy_fallback_publishes_with_zero_replace_policy() {
    exercise(Destination::Absent);
}

#[test]
fn legacy_fallback_preserves_existing_regular_file() {
    exercise(Destination::File);
}

#[test]
fn legacy_fallback_preserves_existing_directory() {
    exercise(Destination::Directory);
}
