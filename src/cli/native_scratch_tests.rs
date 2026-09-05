//! Rejection evidence only: no compiler or child process is needed. The
//! implementation's pre-scratch ordering is not a physical allocation trace.
use super::{checked, codegen, load, run_native_source};
use same_file::Handle;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::Path;

// Same admitted native-import syntax as tests/native_rust_interop_v1.rs.
// Ordinary emit_c rejects the import even though this main does not call it.
const UNSUPPORTED_NATIVE: &str = r#"module scratch.native_import;
@id("scratch.host") interface Host permits {} {
    @id("scratch.host.invert")
    import rust fn invert(value: bool) -> bool effects {} failure infallible;
}
@id("scratch.main") fn main() -> i64 { 0 }
"#;

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

fn write_new(path: &Path, bytes: &[u8]) {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

#[test]
fn rejected_source_preserves_the_former_predictable_run_path() {
    let temp = std::env::temp_dir();
    let old_output = temp.join(format!("semaprax-run-{}", std::process::id()));
    const SENTINEL: &[u8] = b"foreign legacy run-path sentinel\n";
    // Never overwrite a pre-existing file, directory, or dangling link. Such a
    // collision fails the fixture explicitly rather than weakening its oracle.
    write_new(&old_output, SENTINEL);
    let sentinel_identity = Handle::from_path(&old_output).unwrap();
    let root = temp.join(format!(
        "semaprax-cli-run-rejection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir(&root).unwrap();
    let root_identity = Handle::from_path(&root).unwrap();
    let invalid = root.join("invalid.spx");
    let unsupported = root.join("unsupported.spx");
    write_new(&invalid, b"");
    write_new(&unsupported, UNSUPPORTED_NATIVE.as_bytes());
    let source_identities = [
        Handle::from_path(&invalid).unwrap(),
        Handle::from_path(&unsupported).unwrap(),
    ];

    let invalid_errors = load(&invalid).unwrap_err();
    assert_eq!(invalid_errors.len(), 1);
    assert_eq!(invalid_errors[0].code, "SPX-P104");
    assert_eq!(run_native_source(&invalid), Err(1));
    assert_eq!(Handle::from_path(&old_output).unwrap(), sentinel_identity);
    assert_eq!(fs::read(&old_output).unwrap(), SENTINEL);

    let program = checked(&unsupported).expect("verified source reaches native emission");
    let error = codegen::emit_c(&program).unwrap_err();
    assert_eq!(error.code, "SPX-B103");
    assert_eq!(
        error.message,
        "native Rust imports are unavailable for the ordinary native target"
    );
    assert_eq!(run_native_source(&unsupported), Err(1));
    assert_eq!(Handle::from_path(&old_output).unwrap(), sentinel_identity);
    assert_eq!(fs::read(&old_output).unwrap(), SENTINEL);

    // Authenticate the complete fixed fixture before any nonrecursive cleanup.
    plain(&old_output, false);
    plain(&root, true);
    assert_eq!(Handle::from_path(&root).unwrap(), root_identity);
    let mut names = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["invalid.spx", "unsupported.spx"]);
    for ((path, expected), identity) in [
        (&invalid, b"".as_slice()),
        (&unsupported, UNSUPPORTED_NATIVE.as_bytes()),
    ]
    .into_iter()
    .zip(&source_identities)
    {
        plain(path, false);
        assert_eq!(&Handle::from_path(path).unwrap(), identity);
        assert_eq!(fs::read(path).unwrap(), expected);
    }
    drop(source_identities);
    drop(root_identity);
    drop(sentinel_identity);
    fs::remove_file(invalid).unwrap();
    fs::remove_file(unsupported).unwrap();
    fs::remove_file(old_output).unwrap();
    fs::remove_dir(root).unwrap();
}
