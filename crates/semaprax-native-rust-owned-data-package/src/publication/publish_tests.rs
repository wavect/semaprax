//! Physical seven-file namespace evidence; no compiler or archiver is run.
//! Injected observations exercise the real publication transition, not SDK ABI.
use super::*;
use std::fs;

const FILES: [(&str, &[u8]); 7] = [
    ("Cargo.toml", b"cargo input\n"),
    ("build.rs", b"build input\n"),
    ("descriptor.json", b"descriptor input\n"),
    ("lib.rs", b"safe input\n"),
    ("owned_data_ffi.rs", b"ffi input\n"),
    ("module.a", b"archive input\n"),
    ("manifest.json", b"manifest input\n"),
];

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-publish-boundary-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        STAGE_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained publication boundary fixture: {}", root.display());
    root
}

fn plain(path: &Path, directory: bool) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.is_dir(), directory);
    if !directory {
        assert!(metadata.is_file());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
}

fn names(path: &Path) -> Vec<OsString> {
    plain(path, true);
    let mut names = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn assert_package(path: &Path) {
    let mut expected = FILES.map(|(name, _)| OsString::from(name));
    expected.sort();
    assert_eq!(names(path), expected);
    for (name, bytes) in FILES {
        let path = path.join(name);
        plain(&path, false);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

fn label(point: PublicationPoint) -> &'static str {
    match point {
        PublicationPoint::BeforeSettlement => "before-settlement",
        PublicationPoint::AfterSettlement => "after-settlement",
        PublicationPoint::AfterRename => "after-rename",
    }
}

fn assert_stage_path(root: &Path, stage: &Path) {
    assert_eq!(stage.parent(), Some(root));
    assert!(stage
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with(".semaprax-owned-data-sdk-"));
}

#[test]
fn complete_package_reopens_exactly_and_retry_cannot_clobber() {
    let root = fixture();
    let output = root.join("sdk");
    let authority = PublicationAuthority::new(&output).unwrap();
    let published = publish_package(&authority, FILES).unwrap();
    verify_published(&authority, &published, FILES).unwrap();
    drop(published);
    assert_eq!(
        publish_package(&authority, FILES).err(),
        Some(PackageError::publication())
    );
    drop(authority);
    assert_eq!(names(&root), ["sdk"]);
    assert_package(&output);
}

#[test]
fn pre_settlement_failure_discards_only_its_exact_stage() {
    let root = fixture();
    let authority = PublicationAuthority::new(&root.join("sdk")).unwrap();
    let mut events = Vec::new();
    let mut observed_stage = None;
    let result = publish_package_inner(&authority, FILES, |point, stage| {
        let point = label(point);
        events.push(point);
        assert_eq!(point, "before-settlement");
        observed_stage = Some(stage.to_path_buf());
        Err(PackageError::provider())
    });
    assert_eq!(result.err(), Some(PackageError::provider()));
    drop(authority);
    assert_eq!(events, ["before-settlement"]);
    let stage = observed_stage.unwrap();
    assert_stage_path(&root, &stage);
    assert!(names(&root).is_empty());
    assert!(!stage.exists());
}

#[test]
fn post_transition_failures_preserve_complete_inventory() {
    for failure in ["after-settlement", "after-rename"] {
        let root = fixture();
        let output = root.join("sdk");
        let authority = PublicationAuthority::new(&output).unwrap();
        let mut events = Vec::new();
        let mut observed_stage = None;
        let result = publish_package_inner(&authority, FILES, |point, stage| {
            let point = label(point);
            events.push(point);
            observed_stage = Some(stage.to_path_buf());
            if point == failure {
                Err(PackageError::provider())
            } else {
                Ok(())
            }
        });
        assert_eq!(result.err(), Some(PackageError::provider()));
        drop(authority);
        let stage = observed_stage.unwrap();
        assert_stage_path(&root, &stage);
        if failure == "after-settlement" {
            assert_eq!(events, ["before-settlement", "after-settlement"]);
            assert_eq!(names(&root), [stage.file_name().unwrap().to_os_string()]);
            assert_package(&stage);
        } else {
            assert_eq!(
                events,
                ["before-settlement", "after-settlement", "after-rename"]
            );
            assert_eq!(names(&root), ["sdk"]);
            assert_package(&output);
        }
    }
}

#[test]
fn post_settlement_rename_collision_preserves_foreign_output_and_stage() {
    let root = fixture();
    let output = root.join("sdk");
    let authority = PublicationAuthority::new(&output).unwrap();
    let mut events = Vec::new();
    let mut observed_stage = None;
    let result = publish_package_inner(&authority, FILES, |point, stage| {
        let point = label(point);
        events.push(point);
        observed_stage = Some(stage.to_path_buf());
        if point == "after-settlement" {
            fs::create_dir(&output).unwrap();
            fs::write(output.join("foreign"), b"foreign sentinel\n").unwrap();
        }
        Ok(())
    });
    assert_eq!(result.err(), Some(PackageError::publication()));
    drop(authority);
    assert_eq!(events, ["before-settlement", "after-settlement"]);
    let stage = observed_stage.unwrap();
    assert_stage_path(&root, &stage);
    let mut expected = vec![stage.file_name().unwrap().to_os_string(), "sdk".into()];
    expected.sort();
    assert_eq!(names(&root), expected);
    assert_package(&stage);
    assert_eq!(names(&output), ["foreign"]);
    plain(&output.join("foreign"), false);
    assert_eq!(
        fs::read(output.join("foreign")).unwrap(),
        b"foreign sentinel\n"
    );
}

#[cfg(unix)]
#[test]
fn published_package_moved_back_to_stage_is_not_rolled_back() {
    let root = fixture();
    let output = root.join("sdk");
    let authority = PublicationAuthority::new(&output).unwrap();
    let mut events = Vec::new();
    let mut observed_stage = None;
    let result = publish_package_inner(&authority, FILES, |point, stage| {
        let point = label(point);
        events.push(point);
        observed_stage = Some(stage.to_path_buf());
        if point == "after-rename" {
            // Keep the exact same authenticated directory and seven files.
            // Old unconditional discard would accept it at the former name.
            fs::rename(&output, stage).unwrap();
            fs::create_dir(&output).unwrap();
            fs::write(output.join("foreign"), b"replacement output\n").unwrap();
        }
        Ok(())
    });
    assert_eq!(result.err(), Some(PackageError::publication()));
    drop(authority);
    assert_eq!(
        events,
        ["before-settlement", "after-settlement", "after-rename"]
    );
    let stage = observed_stage.unwrap();
    assert_stage_path(&root, &stage);
    let mut expected = vec![stage.file_name().unwrap().to_os_string(), "sdk".into()];
    expected.sort();
    assert_eq!(names(&root), expected);
    assert_package(&stage);
    assert_eq!(names(&output), ["foreign"]);
    plain(&output.join("foreign"), false);
    assert_eq!(
        fs::read(output.join("foreign")).unwrap(),
        b"replacement output\n"
    );
}
