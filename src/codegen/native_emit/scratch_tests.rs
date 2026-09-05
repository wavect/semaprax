//! Synthetic process results over the real compiler scratch callsite. These
//! tests do not run Clang or prove output-binary generation or child settlement.
use super::write_and_compile_c_with_runner;
use same_file::Handle;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};

fn status(success: bool) -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        ExitStatus::from_raw(if success { 0 } else { 1 << 8 })
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt as _;
        ExitStatus::from_raw(if success { 0 } else { 1 })
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    Success,
    Failed,
    Uncertain,
    Foreign,
}

fn exercise(outcome: Outcome) {
    const SOURCE: &str = "int main(void) { return 0; }\n";
    let mut retained = None::<(PathBuf, Handle)>;
    let result =
        write_and_compile_c_with_runner(SOURCE, Path::new("unproduced-output"), false, |command| {
            assert_eq!(command.get_program(), "clang");
            let args = command.get_args().collect::<Vec<_>>();
            assert_eq!(args.len(), 9);
            for (actual, expected) in args[..6].iter().zip([
                "-std=c11",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wno-tautological-compare",
            ]) {
                assert_eq!(*actual, expected);
            }
            let source = Path::new(args[6]);
            assert!(source.is_absolute());
            assert_eq!(source.file_name().unwrap(), "source.c");
            assert_eq!(fs::read(source).unwrap(), SOURCE.as_bytes());
            assert_eq!(args[7], "-o");
            assert_eq!(args[8], "unproduced-output");
            let directory = source.parent().unwrap();
            retained = Some((
                directory.to_path_buf(),
                Handle::from_path(directory).unwrap(),
            ));
            if matches!(outcome, Outcome::Foreign) {
                fs::write(directory.join("foreign"), b"sentinel").unwrap();
            }
            if matches!(outcome, Outcome::Uncertain) {
                Err(std::io::Error::other("synthetic compiler I/O failure"))
            } else {
                Ok(Output {
                    status: status(!matches!(outcome, Outcome::Failed)),
                    stdout: Vec::new(),
                    stderr: b"synthetic compiler diagnostic".to_vec(),
                })
            }
        });
    match outcome {
        Outcome::Success | Outcome::Foreign => assert!(result.is_ok()),
        Outcome::Failed => {
            let error = result.unwrap_err();
            assert_eq!(error.code, "SPX-B102");
            assert_eq!(
                error.message,
                "native backend failed:\nsynthetic compiler diagnostic"
            );
        }
        Outcome::Uncertain => {
            let error = result.unwrap_err();
            assert_eq!(error.code, "SPX-B101");
            assert_eq!(
                error.message,
                "failed to start clang; install a C11 toolchain: synthetic compiler I/O failure"
            );
        }
    }
    let (directory, identity) = retained.expect("real compiler boundary reached");
    if matches!(outcome, Outcome::Success) {
        drop(identity);
        assert!(!directory.exists());
        return;
    }
    // The real callsite retained its directory and every byte. Fixture cleanup
    // preflights exactly those objects and never interprets synthetic status as
    // proof of real process quiescence (no process was started).
    assert_eq!(Handle::from_path(&directory).unwrap(), identity);
    let metadata = fs::symlink_metadata(&directory).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
    let mut expected = vec![("source.c", SOURCE.as_bytes())];
    if matches!(outcome, Outcome::Foreign) {
        expected.push(("foreign", b"sentinel"));
    }
    expected.sort_by_key(|(name, _)| *name);
    let mut actual = fs::read_dir(&directory)
        .unwrap()
        .map(|row| row.unwrap().file_name())
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(name, _)| std::ffi::OsString::from(*name))
            .collect::<Vec<_>>()
    );
    for (name, bytes) in &expected {
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt as _;
            assert_eq!(metadata.file_attributes() & 0x400, 0);
        }
        assert_eq!(fs::read(path).unwrap(), *bytes);
    }
    for (name, _) in expected {
        fs::remove_file(directory.join(name)).unwrap();
    }
    fs::remove_dir(directory).unwrap();
}

#[test]
fn synthetic_compiler_success_cleans_only_its_source_scratch() {
    exercise(Outcome::Success);
}

#[test]
fn synthetic_compiler_failure_retains_source_and_primary_diagnostic() {
    exercise(Outcome::Failed);
}

#[test]
fn synthetic_compiler_io_error_retains_source_and_primary_diagnostic() {
    exercise(Outcome::Uncertain);
}

#[test]
fn synthetic_compiler_success_with_foreign_inventory_preserves_success_and_files() {
    exercise(Outcome::Foreign);
}
