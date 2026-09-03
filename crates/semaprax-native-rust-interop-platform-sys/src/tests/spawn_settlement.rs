//! Darwin spawn-resource and Windows job settlement drivers that fork the test
//! binary into the per-point helper cases.

#[cfg(target_os = "macos")]
#[test]
fn darwin_spawn_resource_destroy_uncertainty_fail_stops_without_later_action() {
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::Command;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-sys-darwin-destroy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("quiet.c");
    std::fs::write(&source, "int main(void){return 0;}\n").unwrap();
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "/usr/bin/cc".into());
    let built = Command::new(compiler)
        .env_clear()
        .env("TMPDIR", &root)
        .env("PATH", "/usr/bin:/bin")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(root.join("quiet"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let current = std::env::current_exe().unwrap();
    for helper in [
        "tests::helper_darwin_attest_returns_changed_after_settlement",
        "tests::helper_darwin_sigcont_returns_spawn_after_settlement",
    ] {
        let sentinel = root.join("post-return");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(
            status.success(),
            "settled operation did not return: {helper}"
        );
        assert!(
            sentinel.exists(),
            "post-return sentinel missing after settled operation: {helper}"
        );
    }
    for helper in [
        "tests::helper_darwin_actions_destroy",
        "tests::helper_darwin_attributes_destroy",
        "tests::helper_darwin_attest_settlement_fail_stop",
        "tests::helper_darwin_sigcont_settlement_fail_stop",
    ] {
        let sentinel = root.join("post-fail-stop");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(!status.success(), "destroy uncertainty returned: {helper}");
        assert!(
            status.signal().is_some(),
            "destroy uncertainty did not abort: {helper}"
        );
        assert!(
            !sentinel.exists(),
            "later action ran after destroy uncertainty"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "windows")]
#[test]
fn windows_runner_failures_use_only_explicit_test_state() {
    use std::process::Command;

    let parent = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = parent.join(format!(
        "semaprax-sys-runner-boundaries-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    for (name, source) in [
            ("quiet", "int main(void){return 0;}\n"),
            (
                "output",
                "#include <windows.h>\n#include <stdio.h>\nint main(void){fputs(\"x\",stdout);fflush(stdout);Sleep(30000);return 0;}\n",
            ),
            (
                "handle_probe",
                "#include <windows.h>\n#include <stdint.h>\n#include <stdlib.h>\n#include <string.h>\nstatic int nibble(char c){if(c>='0'&&c<='9')return c-'0';if(c>='a'&&c<='f')return c-'a'+10;return -1;}\nint main(int argc,char **argv){if(argc!=4)return 7;char *end=0;uintptr_t handle=(uintptr_t)_strtoui64(argv[1],&end,10);if(!end||*end)return 6;end=0;uint64_t volume=(uint64_t)_strtoui64(argv[2],&end,10);if(!end||*end||strlen(argv[3])!=32)return 6;if(getenv(\"PATH\")!=0)return 8;FILE_ID_INFO info;if(!GetFileInformationByHandleEx((HANDLE)handle,FileIdInfo,&info,sizeof(info)))return 0;if(info.VolumeSerialNumber!=volume)return 0;for(size_t i=0;i<16;i++){int high=nibble(argv[3][i*2]);int low=nibble(argv[3][i*2+1]);if(high<0||low<0)return 6;if(info.FileId.Identifier[i]!=(unsigned char)((high<<4)|low))return 0;}return 9;}\n",
            ),
        ] {
            let source_path = root.join(format!("{name}.c"));
            std::fs::write(&source_path, source).unwrap();
            let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
            let built = Command::new(compiler)
                .env("TMP", &root)
                .env("TEMP", &root)
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-D_CRT_SECURE_NO_WARNINGS",
                    "-O2",
                ])
                .arg(&source_path)
                .arg("-o")
                .arg(root.join(format!("{name}.exe")))
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
        }
    use std::fmt::Write as _;
    use std::os::windows::io::AsRawHandle as _;
    let sentinel = root.join("unlisted-handle");
    std::fs::write(&sentinel, b"must not be inherited").unwrap();
    let inherited = std::fs::File::open(&sentinel).unwrap();
    let raw = inherited.as_raw_handle();
    assert_ne!(
        unsafe {
            windows_sys::Win32::Foundation::SetHandleInformation(
                raw.cast(),
                windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
                windows_sys::Win32::Foundation::HANDLE_FLAG_INHERIT,
            )
        },
        0
    );
    let mut identity = windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO::default();
    assert_ne!(
        unsafe {
            windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandleEx(
                raw.cast(),
                windows_sys::Win32::Storage::FileSystem::FileIdInfo,
                (&mut identity as *mut windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO)
                    .cast(),
                u32::try_from(std::mem::size_of::<
                    windows_sys::Win32::Storage::FileSystem::FILE_ID_INFO,
                >())
                .unwrap(),
            )
        },
        0
    );
    let mut file_id = String::with_capacity(32);
    for byte in identity.FileId.Identifier {
        write!(&mut file_id, "{byte:02x}").unwrap();
    }
    let arguments = [
        (raw as usize).to_string(),
        identity.VolumeSerialNumber.to_string(),
        file_id,
    ];
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("handle_probe.exe"))
            .unwrap();
    super::platform::execute_harness_with_arguments(&executable, &directory, &arguments).unwrap();
    drop(executable);
    drop(directory);
    drop(inherited);
    let current = std::env::current_exe().unwrap();
    for helper in [
        "tests::helper_windows_image",
        "tests::helper_windows_assign",
        "tests::helper_windows_resume",
        "tests::helper_windows_peek",
        "tests::helper_windows_read",
    ] {
        let sentinel = root.join("settled");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(status.success(), "settled boundary failed: {helper}");
        assert!(
            sentinel.exists(),
            "settled boundary did not return: {helper}"
        );
    }
    for helper in [
        "tests::helper_windows_unassigned_fail_stop",
        "tests::helper_windows_wait_unassigned_fail_stop",
        "tests::helper_windows_terminate_job_fail_stop",
        "tests::helper_windows_query_job_fail_stop",
    ] {
        let sentinel = root.join("post-fail-stop");
        let _ = std::fs::remove_file(&sentinel);
        let status = Command::new(&current)
            .env("SEMAPRAX_SYS_TEST_HELPER_ROOT", &root)
            .args(["--exact", helper, "--nocapture"])
            .status()
            .unwrap();
        assert!(!status.success(), "fail-stop boundary returned: {helper}");
        assert!(
            !sentinel.exists(),
            "later action ran after fail-stop: {helper}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}
