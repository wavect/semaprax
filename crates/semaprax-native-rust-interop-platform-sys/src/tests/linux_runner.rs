//! Linux runner boundary settlement and the real archive runner evidence that
//! foreign pipe holders never extend a held output.

#[cfg(target_os = "linux")]
use super::*;

#[cfg(target_os = "linux")]
#[test]
fn linux_runner_boundaries_settle_or_fail_stop_without_later_action() {
    use std::os::unix::process::ExitStatusExt as _;
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
    let source = root.join("noisy.c");
    std::fs::write(
            &source,
            "#include <stdio.h>\n#include <unistd.h>\nint main(void){FILE *f=fopen(\"leader.pid\",\"w\");if(!f)return 2;fprintf(f,\"%ld\",(long)getpid());fclose(f);if(write(1,\"x\",1)!=1)return 2;sleep(1);return 0;}\n",
        )
        .unwrap();
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    let built = Command::new(compiler)
        .env("TMPDIR", &root)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(root.join("noisy"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let current = std::env::current_exe().unwrap();
    for helper in [
        "tests::helper_linux_pipe_read_fcntl",
        "tests::helper_linux_pipe_write_fcntl",
        "tests::helper_linux_drain_fcntl",
        "tests::helper_linux_poll",
        "tests::helper_linux_read",
        "tests::helper_linux_read_conversion",
        "tests::helper_linux_waitpid",
        "tests::helper_linux_deadline",
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
        "tests::helper_linux_parent_write_close",
        "tests::helper_linux_parent_null_close",
        "tests::helper_linux_settle_close",
        "tests::helper_linux_success_read_close",
        "tests::helper_linux_wait_settlement",
        "tests::helper_linux_group_settlement",
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
            status.signal().is_some(),
            "fail-stop did not abort: {helper}"
        );
        assert!(
            !sentinel.exists(),
            "later action ran after fail-stop: {helper}"
        );
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_archive_runner_does_not_wait_for_foreign_pipe_holders_and_settles_owned_output() {
    use std::ffi::OsStr;
    use std::process::Command;

    let root = std::env::temp_dir().join(format!(
        "semaprax-sys-archive-pipe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("archiver.c");
    std::fs::write(
        &source,
        "#define _GNU_SOURCE\n#include <stdio.h>\n#include <stdlib.h>\n#include <unistd.h>\nint main(int n,char **v){if(n!=4)return 2;FILE *f=fopen(v[2],\"wb\");if(!f)return 3;if(fwrite(\"!<arch>\\n\",1,8,f)!=8||fclose(f))return 4;int ready[2];if(pipe(ready))return 5;pid_t p=fork();if(p<0)return 6;if(!p){if(close(ready[0])||setsid()<0)_exit(7);f=fopen(\"failure-holder.pid\",\"w\");if(!f)_exit(8);if(fprintf(f,\"%ld\",(long)getpid())<=0||fclose(f))_exit(9);if(write(ready[1],\"x\",1)!=1||close(ready[1]))_exit(10);for(;;)pause();}if(close(ready[1]))return 11;char byte=0;if(read(ready[0],&byte,1)!=1||byte!='x'||close(ready[0]))return 12;return 7;}\n",
    )
    .unwrap();
    let built = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source)
        .arg("-o")
        .arg(root.join("archiver"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::write_file_new(
        &directory,
        OsStr::new("module.o"),
        b"not-consumed-after-exit",
        0o600,
    )
    .unwrap();
    let archiver = super::platform::hold_executable(&directory, OsStr::new("archiver")).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    let start = std::time::Instant::now();
    let result =
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process);
    let holder = std::fs::read_to_string(root.join("failure-holder.pid")).unwrap();
    let holder = holder.parse::<libc::pid_t>().unwrap();
    let mut holder = LinuxForeignHolderGuard::new(holder);
    holder.assert_alive();
    assert!(matches!(result, Err(Error::Exit)));
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert!(!root.join("libsemaprax_native_rust_sdk.a").exists());
    holder.settle();
    drop((archiver, input, directory, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
fn compile_linux_archive_test_tool(
    root: &std::path::Path,
    name: &str,
    source: &str,
) -> std::path::PathBuf {
    use std::process::Command;

    let source_path = root.join(format!("{name}.c"));
    let output_path = root.join(name);
    std::fs::write(&source_path, source).unwrap();
    let built = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"])
        .arg(&source_path)
        .arg("-o")
        .arg(&output_path)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    output_path
}

#[cfg(target_os = "linux")]
fn linux_archive_test_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-sys-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}

#[cfg(target_os = "linux")]
struct LinuxForeignHolderGuard {
    pid: libc::pid_t,
}

#[cfg(target_os = "linux")]
impl LinuxForeignHolderGuard {
    fn new(pid: libc::pid_t) -> Self {
        assert!(pid > 1, "unsafe foreign stdout holder pid");
        assert_eq!(
            unsafe { libc::kill(pid, 0) },
            0,
            "foreign holder is not live"
        );
        Self { pid }
    }

    fn assert_alive(&self) {
        assert!(self.pid > 1, "foreign stdout holder already settled");
        assert_eq!(unsafe { libc::kill(self.pid, 0) }, 0);
    }

    fn settle(&mut self) {
        assert!(self.pid > 1, "unsafe foreign stdout holder pid");
        let _ = unsafe { libc::kill(self.pid, libc::SIGKILL) };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let alive = unsafe { libc::kill(self.pid, 0) } == 0;
            if !alive && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                self.pid = 0;
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "foreign stdout holder did not settle"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxForeignHolderGuard {
    fn drop(&mut self) {
        if self.pid > 1 {
            let _ = unsafe { libc::kill(self.pid, libc::SIGKILL) };
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_archive_seed_is_exactly_initialized_held_and_removed() {
    use std::ffi::OsStr;

    let root = linux_archive_test_root("archive-seed");
    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    super::platform::test_archive_seed_round_trip(&directory, OsStr::new("owned.a")).unwrap();
    assert!(!root.join("owned.a").exists());
    drop(directory);
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_real_archive_succeeds_without_waiting_for_foreign_pipe_holder() {
    use std::ffi::OsStr;
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    let root = linux_archive_test_root("archive-success-pipe");
    let object_source = root.join("module.c");
    std::fs::write(
        &object_source,
        "int semaprax_archive_probe(void){return 7;}\n",
    )
    .unwrap();
    let object = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2", "-c"])
        .arg(&object_source)
        .arg("-o")
        .arg(root.join("module.o"))
        .output()
        .unwrap();
    assert!(
        object.status.success(),
        "{}",
        String::from_utf8_lossy(&object.stderr)
    );
    std::fs::set_permissions(
        root.join("module.o"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let real_archiver = std::env::var("SEMAPRAX_ARCHIVER")
        .unwrap_or_else(|_| "/usr/bin/x86_64-linux-gnu-ar".to_owned());
    assert!(std::path::Path::new(&real_archiver).is_absolute());
    assert!(std::path::Path::new(&real_archiver).is_file());
    let c_archiver = real_archiver.replace('\\', "\\\\").replace('"', "\\\"");
    compile_linux_archive_test_tool(
        &root,
        "archive-wrapper",
        &format!(
            "#define _GNU_SOURCE\n#include <stdio.h>\n#include <stdlib.h>\n#include <sys/stat.h>\n#include <unistd.h>\nint main(int n,char **v){{if(n!=4)return 2;struct stat s;if(stat(v[2],&s))return 3;FILE *seed=fopen(\"seed.ino\",\"w\");if(!seed)return 4;if(fprintf(seed,\"%llu\",(unsigned long long)s.st_ino)<=0||fclose(seed))return 5;int ready[2];if(pipe(ready))return 6;pid_t p=fork();if(p<0)return 7;if(!p){{if(close(ready[0])||setsid()<0)_exit(8);FILE *f=fopen(\"holder.pid\",\"w\");if(!f)_exit(9);if(fprintf(f,\"%ld\",(long)getpid())<=0||fclose(f))_exit(10);if(write(ready[1],\"x\",1)!=1||close(ready[1]))_exit(11);for(;;)pause();}}if(close(ready[1]))return 12;char byte=0;if(read(ready[0],&byte,1)!=1||byte!='x'||close(ready[0]))return 13;v[0]=\"{c_archiver}\";execv(\"{c_archiver}\",v);return 14;}}\n"
        ),
    );

    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.o")).unwrap();
    let (input_mode, _, _) = super::platform::test_regular_file_facts(&input);
    assert_eq!(input_mode & 0o777, 0o600);
    let archiver =
        super::platform::hold_executable(&directory, OsStr::new("archive-wrapper")).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    let start = std::time::Instant::now();
    let archive =
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process);
    let holder = std::fs::read_to_string(root.join("holder.pid")).unwrap();
    let holder = holder.parse::<libc::pid_t>().unwrap();
    let mut holder = LinuxForeignHolderGuard::new(holder);
    holder.assert_alive();
    let archive = archive.unwrap();
    let seed_ino = std::fs::read_to_string(root.join("seed.ino"))
        .unwrap()
        .parse::<u64>()
        .unwrap();
    let (_, _, archive_ino) = super::platform::test_regular_file_facts(&archive);
    assert_eq!(archive_ino, seed_ino);
    super::platform::test_exact_archive_member(&archive, &input).unwrap();
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert!(root.join("libsemaprax_native_rust_sdk.a").is_file());
    holder.settle();
    drop((archive, archiver, input, directory, process));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_archive_cleanup_preserves_replaced_foreign_inode_and_fails_closed() {
    use std::ffi::OsStr;

    let root = linux_archive_test_root("archive-foreign-inode");
    compile_linux_archive_test_tool(
        &root,
        "replace-archive",
        "#define _GNU_SOURCE\n#include <stdio.h>\n#include <unistd.h>\nint main(int n,char **v){if(n!=4)return 2;if(unlink(v[2]))return 3;FILE *f=fopen(v[2],\"wb\");if(!f)return 4;if(fwrite(\"foreign-must-survive\",1,20,f)!=20||fclose(f))return 5;return 7;}\n",
    );
    std::fs::write(root.join("module.o"), b"input").unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let directory = super::platform::hold_directory(&root).unwrap();
    let input = super::platform::hold_regular_file(&directory, OsStr::new("module.o")).unwrap();
    let archiver =
        super::platform::hold_executable(&directory, OsStr::new("replace-archive")).unwrap();
    let prepared = super::platform::prepare_archive_invocation(
        OsStr::new("module.o"),
        OsStr::new("libsemaprax_native_rust_sdk.a"),
    )
    .unwrap();
    let mut process = super::platform::prepare_process_arena(1).unwrap();
    assert!(matches!(
        super::platform::archive_prepared(&archiver, &directory, &input, prepared, &mut process,),
        Err(Error::Changed)
    ));
    assert_eq!(
        std::fs::read(root.join("libsemaprax_native_rust_sdk.a")).unwrap(),
        b"foreign-must-survive"
    );
    drop((archiver, input, directory, process));
    std::fs::remove_dir_all(root).unwrap();
}
