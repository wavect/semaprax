use super::with_native_executable_suffix;
use std::path::{Path, PathBuf};

#[test]
fn native_output_without_extension_has_exactly_one_platform_suffix() {
    for spelling in [
        "program",
        "dist/program",
        "dist with spaces/program",
        ".program",
    ] {
        let path = PathBuf::from(spelling);
        let actual = with_native_executable_suffix(path.clone());
        let mut expected = path.into_os_string();
        expected.push(std::env::consts::EXE_SUFFIX);
        assert_eq!(actual.as_os_str(), expected);
        #[cfg(windows)]
        {
            assert_eq!(actual.extension().unwrap(), "exe");
            assert!(!actual.to_string_lossy().ends_with("..exe"));
        }
    }
}

#[test]
fn native_output_with_an_existing_extension_is_preserved() {
    for spelling in [
        "program.exe",
        "program.out",
        "program.",
        "dist/program.custom",
    ] {
        assert_eq!(
            with_native_executable_suffix(PathBuf::from(spelling)),
            Path::new(spelling)
        );
    }
}

#[cfg(unix)]
#[test]
fn native_output_preserves_non_utf8_paths_on_extension_free_hosts() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(b"program-\xff".to_vec()));
    assert_eq!(with_native_executable_suffix(path.clone()), path);
}
