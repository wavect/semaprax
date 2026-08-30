//! Mandatory directory-link evidence: Unix symlinks and Windows junctions.
//! Windows uses only fixed shell words; fixture paths are passed as current_dir,
//! never interpolated into cmd syntax. Missing provisioning is a test failure.
use std::ffi::OsString;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

const TARGET: &str = "symlink-target";
const LINK: &str = "linked";
const SENTINEL: &str = "sentinel";
const BYTES: &[u8] = b"directory-link target must remain unchanged\n";

pub fn entries(root: &Path) -> Vec<OsString> {
    let mut names = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub fn create(root: &Path) -> PathBuf {
    assert!(root.is_absolute());
    assert_plain(&fs::symlink_metadata(root).unwrap(), true);
    let target = root.join(TARGET);
    let link = root.join(LINK);
    for path in [&target, &link] {
        assert_eq!(
            fs::symlink_metadata(path).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }
    let mut expected = entries(root);
    expected.extend([OsString::from(TARGET), OsString::from(LINK)]);
    expected.sort();
    fs::create_dir(&target).unwrap();
    let mut sentinel = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target.join(SENTINEL))
        .unwrap();
    sentinel.write_all(BYTES).unwrap();
    drop(sentinel);

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    #[cfg(windows)]
    {
        let working_directory = windows_working_directory(root);
        let result = std::process::Command::new("cmd")
            .args(["/d", "/c", "mklink", "/J", "linked", "symlink-target"])
            .current_dir(&working_directory)
            .output()
            .expect("Windows junction fixture requires cmd and mklink /J");
        assert!(
            result.status.success(),
            "junction creation failed: stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_intact(root);
    assert_eq!(
        entries(root),
        expected,
        "link preparation created unexpected entries"
    );
    link
}

#[cfg(windows)]
fn windows_working_directory(root: &Path) -> PathBuf {
    use std::os::windows::ffi::OsStringExt as _;
    use std::path::{Component, Prefix};

    // cmd can reject a UNC cwd and fall back elsewhere. Only ordinary local
    // absolute drive paths are admitted; canonical Windows paths commonly
    // carry a VerbatimDisk prefix, which must first lose the verbatim marker.
    let mut parts = root.components();
    let drive = match parts.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => panic!("junction fixture requires a local absolute drive path"),
        },
        _ => panic!("junction fixture requires a local absolute drive path"),
    };
    assert!(matches!(parts.next(), Some(Component::RootDir)));
    let mut directory = PathBuf::from(format!("{}:\\", char::from(drive)));
    for component in parts {
        match component {
            Component::Normal(name) => directory.push(name),
            _ => panic!("junction fixture root must be canonical"),
        }
    }
    assert_eq!(
        fs::canonicalize(&directory).unwrap(),
        fs::canonicalize(root).unwrap()
    );
    // This command is read-only. /u gives UTF-16LE for the builtin's piped
    // output, avoiding dependence on the active Windows console code page.
    let observed = std::process::Command::new("cmd")
        .args(["/d", "/u", "/c", "cd"])
        .current_dir(&directory)
        .output()
        .expect("cannot verify cmd fixture working directory");
    assert!(observed.status.success());
    assert_eq!(observed.stdout.len() % 2, 0);
    let mut units = observed
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect::<Vec<_>>();
    assert!(units.ends_with(&[13, 10]));
    units.truncate(units.len() - 2);
    assert!(!units.is_empty());
    let reported = PathBuf::from(OsString::from_wide(&units));
    assert_eq!(
        fs::canonicalize(reported).unwrap(),
        fs::canonicalize(root).unwrap(),
        "cmd must not substitute a different working directory"
    );
    directory
}

pub fn assert_intact(root: &Path) {
    assert_plain(&fs::symlink_metadata(root).unwrap(), true);
    assert_target(root);
    let link = root.join(LINK);
    let metadata = fs::symlink_metadata(&link).unwrap();
    #[cfg(unix)]
    assert!(metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_ne!(
            metadata.file_attributes() & 0x400,
            0,
            "fixture must be a reparse point"
        );
    }
    assert_eq!(
        fs::canonicalize(link).unwrap(),
        fs::canonicalize(root.join(TARGET)).unwrap()
    );
}

fn assert_target(root: &Path) {
    let target = root.join(TARGET);
    assert_plain(&fs::symlink_metadata(&target).unwrap(), true);
    assert_eq!(entries(&target), [OsString::from(SENTINEL)]);
    assert_plain(&fs::symlink_metadata(target.join(SENTINEL)).unwrap(), false);
    assert_eq!(fs::read(target.join(SENTINEL)).unwrap(), BYTES);
}

fn assert_plain(metadata: &fs::Metadata, directory: bool) {
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.is_dir(), directory);
    assert_eq!(metadata.is_file(), !directory);
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
}

pub fn remove_link(root: &Path) {
    // Preflight both the exact link and complete target inventory before this
    // single nonrecursive unlink; never pass a target tree to recursive cleanup.
    assert_intact(root);
    let mut expected = entries(root);
    expected.retain(|name| name != LINK);
    #[cfg(unix)]
    fs::remove_file(root.join(LINK)).unwrap();
    #[cfg(windows)]
    fs::remove_dir(root.join(LINK)).unwrap();
    assert_eq!(
        fs::symlink_metadata(root.join(LINK)).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
    assert_target(root);
    assert_eq!(entries(root), expected);
}
