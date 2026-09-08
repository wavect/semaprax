use super::*;

#[test]
fn relative_byte_paths_have_an_exact_closed_grammar() {
    for path in [
        b"".as_slice(),
        b"/a",
        b"a/",
        b"a//b",
        b".",
        b"..",
        b"a/../b",
        b"a/./b",
        b"a\\b",
        b"C:a",
        b"a\0b",
    ] {
        assert_eq!(
            validate_path(path),
            Err(FileFailure::InvalidPath),
            "{path:?}"
        );
    }
    for path in [b"a".as_slice(), b"a/b", b".hidden", b"a...b", &[255u8]] {
        assert_eq!(validate_path(path), Ok(()));
    }
    assert!(validate_path(&vec![b'a'; MAX_PATH_BYTES]).is_ok());
    assert_eq!(
        validate_path(&vec![b'a'; MAX_PATH_BYTES + 1]),
        Err(FileFailure::InvalidPath)
    );
}

#[test]
fn fixture_read_create_new_and_denial_preserve_exact_bytes() {
    let mut fixture = FixtureFileProvider::new([(b"in".to_vec(), vec![0, 255, 10])], true).unwrap();
    assert_eq!(fixture.read(b"in", 2), Err(FileFailure::CapacityExceeded));
    assert_eq!(fixture.read(b"in", 3), Ok(vec![0, 255, 10]));
    assert_eq!(fixture.write_new(b"out", &[255, 0]), Ok(2));
    assert_eq!(
        fixture.write_new(b"out", b"wrong"),
        Err(FileFailure::AlreadyExists)
    );
    assert_eq!(fixture.read(b"out", 2), Ok(vec![255, 0]));
    assert_eq!(fixture.write_new(b"empty", &[]), Ok(0));
    assert_eq!(fixture.read(b"empty", 0), Ok(vec![]));
    assert_eq!(fixture.write_new(b"nested/child", b"x"), Ok(1));
    assert_eq!(fixture.list(b"nested", 16), Ok(b"child\0".to_vec()));
    assert_eq!(
        fixture.write_new(b"nested", b"wrong"),
        Err(FileFailure::AlreadyExists)
    );
    fixture.settle();
    assert_eq!(fixture.settlements(), 1);
    assert_eq!(fixture.read(b"missing", 8), Err(FileFailure::NotFound));
    let mut denied = DeniedFileProvider;
    assert_eq!(denied.read(b"in", 3), Err(FileFailure::AuthorityDenied));
    assert_eq!(
        denied.write_new(b"out", b"data"),
        Err(FileFailure::AuthorityDenied)
    );
    let mut readonly = FixtureFileProvider::new([], false).unwrap();
    assert_eq!(
        readonly.write_new(b"out", b"data"),
        Err(FileFailure::AuthorityDenied)
    );
    assert!(readonly.files().is_empty());
}

#[test]
fn fixture_capacity_and_duplicate_inventory_fail_before_use() {
    assert!(matches!(
        FixtureFileProvider::new([(b"x".to_vec(), vec![0; MAX_FILE_BYTES + 1])], true),
        Err(FileFailure::CapacityExceeded)
    ));
    assert!(matches!(
        FixtureFileProvider::new([(b"x".to_vec(), vec![]), (b"x".to_vec(), vec![])], true),
        Err(FileFailure::AlreadyExists)
    ));
}

#[test]
fn fixture_v2_metadata_listing_directories_removal_and_atomic_replacement() {
    let mut fixture = FixtureFileProvider::new(
        [
            (b"z".to_vec(), vec![1]),
            (b"tree/a".to_vec(), vec![2]),
            (b"tree/\xff".to_vec(), vec![3]),
        ],
        true,
    )
    .unwrap();
    assert_eq!(
        fixture.stat(b""),
        Ok(FileMetadata {
            kind: FileKind::Directory,
            size: 0
        })
    );
    assert_eq!(
        fixture.stat(b"tree/a"),
        Ok(FileMetadata {
            kind: FileKind::File,
            size: 1
        })
    );
    assert_eq!(fixture.list(b"", 64), Ok(b"tree\0z\0".to_vec()));
    assert_eq!(fixture.list(b"tree", 64), Ok(vec![b'a', 0, 255, 0]));
    assert_eq!(fixture.list(b"z", 64), Err(FileFailure::InvalidFileType));
    assert_eq!(fixture.list(b"tree", 3), Err(FileFailure::CapacityExceeded));
    assert_eq!(fixture.create_dir(b"empty"), Ok(0));
    assert_eq!(
        fixture.stat(b"empty"),
        Ok(FileMetadata {
            kind: FileKind::Directory,
            size: 0
        })
    );
    assert_eq!(fixture.remove(b"tree"), Err(FileFailure::IoFailure));
    assert_eq!(fixture.remove(b"empty"), Ok(0));
    assert_eq!(fixture.write_atomic(b"z", b"new"), Ok(3));
    assert_eq!(fixture.read(b"z", 3), Ok(b"new".to_vec()));
    assert_eq!(
        fixture.write_atomic(b"tree", b"x"),
        Err(FileFailure::InvalidFileType)
    );
    assert_eq!(
        fixture.write_atomic(b"missing/x", b"x"),
        Err(FileFailure::NotFound)
    );
    assert_eq!(
        fixture.read(b"z", MAX_FILE_BYTES + 1),
        Err(FileFailure::CapacityExceeded)
    );
    assert_eq!(fixture.read(b"z", 3), Ok(b"new".to_vec()));
}

#[test]
fn fixture_v2_invalid_operations_cannot_mutate_inventory() {
    let mut fixture =
        FixtureFileProvider::new([(b"keep".to_vec(), b"old".to_vec())], true).unwrap();
    for path in [b"".as_slice(), b"/bad", b"bad/", b"a/../b", b"a\0b"] {
        assert_eq!(fixture.create_dir(path), Err(FileFailure::InvalidPath));
        assert_eq!(fixture.remove(path), Err(FileFailure::InvalidPath));
        assert_eq!(
            fixture.write_atomic(path, b"new"),
            Err(FileFailure::InvalidPath)
        );
    }
    assert_eq!(
        fixture.write_atomic(b"keep", &vec![0; MAX_FILE_BYTES + 1]),
        Err(FileFailure::CapacityExceeded)
    );
    assert_eq!(fixture.read(b"keep", 3), Ok(b"old".to_vec()));
}

#[cfg(unix)]
mod physical {
    use super::*;
    #[cfg(target_os = "linux")]
    use std::ffi::OsStr;
    #[cfg(target_os = "linux")]
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "semaprax-file-provider-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn physical_roundtrip_zero_capacity_and_no_overwrite() {
        let root = Scratch::new();
        std::fs::create_dir(root.0.join("nested")).unwrap();
        let mut provider = ScopedFileProvider::open(&root.0, FileAccess::ReadWrite).unwrap();
        assert_eq!(provider.write_new(b"nested/file", &[0, 255, 10]), Ok(3));
        assert_eq!(provider.read(b"nested/file", 3), Ok(vec![0, 255, 10]));
        assert_eq!(
            provider.read(b"nested/file", 2),
            Err(FileFailure::CapacityExceeded)
        );
        assert_eq!(
            provider.write_new(b"nested/file", b"overwrite"),
            Err(FileFailure::AlreadyExists)
        );
        assert_eq!(
            std::fs::read(root.0.join("nested/file")).unwrap(),
            [0, 255, 10]
        );
        assert_eq!(provider.write_new(b"empty", &[]), Ok(0));
        assert_eq!(provider.read(b"empty", 0), Ok(vec![]));
        let mut readonly = ScopedFileProvider::open(&root.0, FileAccess::ReadOnly).unwrap();
        assert_eq!(
            readonly.write_new(b"denied", b"x"),
            Err(FileFailure::AuthorityDenied)
        );
        assert!(!root.0.join("denied").exists());
        let mut writeonly = ScopedFileProvider::open(&root.0, FileAccess::WriteOnly).unwrap();
        assert_eq!(
            writeonly.read(b"empty", 0),
            Err(FileFailure::AuthorityDenied)
        );
    }

    #[test]
    fn symlinks_directories_and_traversal_cannot_escape_authority() {
        let root = Scratch::new();
        let outside = Scratch::new();
        std::fs::write(outside.0.join("secret"), b"outside").unwrap();
        symlink(&outside.0, root.0.join("redirect")).unwrap();
        symlink(outside.0.join("secret"), root.0.join("leaf")).unwrap();
        let mut provider = ScopedFileProvider::open(&root.0, FileAccess::ReadWrite).unwrap();
        assert_eq!(
            provider.read(b"redirect/secret", 20),
            Err(FileFailure::InvalidFileType)
        );
        assert_eq!(
            provider.read(b"leaf", 20),
            Err(FileFailure::InvalidFileType)
        );
        assert_eq!(
            provider.write_new(b"redirect/new", b"x"),
            Err(FileFailure::InvalidFileType)
        );
        assert_eq!(
            provider.write_new(b"leaf", b"x"),
            Err(FileFailure::AlreadyExists)
        );
        assert_eq!(
            provider.read(b"../secret", 20),
            Err(FileFailure::InvalidPath)
        );
        assert!(!outside.0.join("new").exists());
        assert_eq!(std::fs::read(outside.0.join("secret")).unwrap(), b"outside");
        std::fs::create_dir(root.0.join("directory")).unwrap();
        assert_eq!(
            provider.read(b"directory", 20),
            Err(FileFailure::InvalidFileType)
        );
    }

    #[test]
    fn retained_root_descriptor_survives_path_replacement() {
        let outer = Scratch::new();
        let original = outer.0.join("root");
        let moved = outer.0.join("retained");
        std::fs::create_dir(&original).unwrap();
        std::fs::write(original.join("file"), b"original").unwrap();
        let mut provider = ScopedFileProvider::open(&original, FileAccess::ReadWrite).unwrap();
        std::fs::rename(&original, &moved).unwrap();
        std::fs::create_dir(&original).unwrap();
        std::fs::write(original.join("file"), b"replacement").unwrap();
        assert_eq!(provider.read(b"file", 20), Ok(b"original".to_vec()));
        assert_eq!(provider.write_new(b"new", b"retained"), Ok(8));
        assert!(moved.join("new").exists());
        assert!(!original.join("new").exists());
    }

    #[test]
    fn physical_v2_metadata_listing_directories_removal_and_atomic_replacement() {
        let root = Scratch::new();
        std::fs::create_dir(root.0.join("tree")).unwrap();
        std::fs::write(root.0.join("z"), b"z").unwrap();
        std::fs::write(root.0.join("tree/a"), b"a").unwrap();
        // macOS filesystems reject arbitrary non-UTF-8 names. The fixture
        // test above covers raw bytes on every platform; retain that physical
        // assertion where the host filesystem admits the byte name.
        #[cfg(target_os = "linux")]
        std::fs::write(root.0.join("tree").join(OsStr::from_bytes(&[255])), b"x").unwrap();
        #[cfg(not(target_os = "linux"))]
        std::fs::write(root.0.join("tree/b"), b"x").unwrap();
        let mut provider = ScopedFileProvider::open(&root.0, FileAccess::ReadWrite).unwrap();
        assert_eq!(
            provider.stat(b""),
            Ok(FileMetadata {
                kind: FileKind::Directory,
                size: 0
            })
        );
        assert_eq!(
            provider.stat(b"z"),
            Ok(FileMetadata {
                kind: FileKind::File,
                size: 1
            })
        );
        assert_eq!(provider.list(b"", 64), Ok(b"tree\0z\0".to_vec()));
        #[cfg(target_os = "linux")]
        assert_eq!(provider.list(b"tree", 64), Ok(vec![b'a', 0, 255, 0]));
        #[cfg(not(target_os = "linux"))]
        assert_eq!(provider.list(b"tree", 64), Ok(b"a\0b\0".to_vec()));
        assert_eq!(provider.list(b"z", 64), Err(FileFailure::InvalidFileType));
        assert_eq!(
            provider.list(b"tree", 3),
            Err(FileFailure::CapacityExceeded)
        );
        assert_eq!(provider.create_dir(b"empty"), Ok(0));
        assert_eq!(provider.remove(b"tree"), Err(FileFailure::IoFailure));
        assert_eq!(provider.remove(b"empty"), Ok(0));
        assert_eq!(provider.write_atomic(b"z", b"replacement"), Ok(11));
        assert_eq!(provider.read(b"z", 11), Ok(b"replacement".to_vec()));
    }

    #[test]
    fn physical_v2_never_follows_or_replaces_symlinks_and_invalid_inputs_do_not_mutate() {
        let root = Scratch::new();
        let outside = Scratch::new();
        std::fs::write(root.0.join("keep"), b"old").unwrap();
        std::fs::write(outside.0.join("target"), b"outside").unwrap();
        symlink(outside.0.join("target"), root.0.join("link")).unwrap();
        let mut provider = ScopedFileProvider::open(&root.0, FileAccess::ReadWrite).unwrap();
        assert_eq!(provider.stat(b"link"), Err(FileFailure::InvalidFileType));
        assert_eq!(provider.remove(b"link"), Err(FileFailure::InvalidFileType));
        assert_eq!(
            provider.write_atomic(b"link", b"changed"),
            Err(FileFailure::InvalidFileType)
        );
        for path in [b"".as_slice(), b"../target", b"bad/", b"a\0b"] {
            assert_eq!(
                provider.write_atomic(path, b"changed"),
                Err(FileFailure::InvalidPath)
            );
        }
        assert_eq!(
            provider.write_atomic(b"keep", &vec![0; MAX_FILE_BYTES + 1]),
            Err(FileFailure::CapacityExceeded)
        );
        assert_eq!(provider.read(b"keep", 3), Ok(b"old".to_vec()));
        assert!(root.0.join("link").is_symlink());
        assert_eq!(std::fs::read(outside.0.join("target")).unwrap(), b"outside");
    }
}
