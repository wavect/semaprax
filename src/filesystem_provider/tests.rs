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

#[cfg(unix)]
mod physical {
    use super::*;
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
}
