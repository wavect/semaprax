// Held process mechanics exercised with this native Rust test binary.

use super::*;
use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

static PROCESS_SERIAL: AtomicU64 = AtomicU64::new(0);
static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());
const SURROGATE_FILTER: &str = "held_git_process_native_surrogate";
/// Generous ceiling for the tests whose subject is not the deadline. Spawning
/// and inspecting the suspended child takes seconds when the whole test binary
/// runs in parallel, and a tight bound makes those runs report a deadline error
/// instead of the outcome being asserted.
const SETTLED_DEADLINE: Duration = Duration::from_secs(60);

fn process_test_lock() -> MutexGuard<'static, ()> {
    PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct ProcessFixture {
    root: PathBuf,
    executable_path: PathBuf,
    executable: File,
    executable_metadata: Metadata,
    repository: File,
}

impl ProcessFixture {
    fn new(mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-held-git-process-{}-{}",
            std::process::id(),
            PROCESS_SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::write(root.join("surrogate-mode"), mode).unwrap();
        let executable_path = std::env::current_exe().unwrap().canonicalize().unwrap();
        let executable = open_file(&executable_path, false, false).unwrap();
        let executable_metadata = executable.metadata().unwrap();
        let repository = open_file(&root, true, false).unwrap();
        Self {
            root,
            executable_path,
            executable,
            executable_metadata,
            repository,
        }
    }

    fn run(
        &self,
        stdout_limit: usize,
        stderr_limit: usize,
        deadline: Instant,
    ) -> io::Result<(i32, Vec<u8>)> {
        platform::run_for_test(
            &self.executable_path,
            &self.executable,
            &self.executable_metadata,
            &self.repository,
            &[
                SURROGATE_FILTER,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ],
            &[],
            stdout_limit,
            stderr_limit,
            deadline,
        )
    }

    fn process_group(&self) -> rustix::process::Pid {
        let raw = std::fs::read_to_string(self.root.join("leader-pid"))
            .unwrap()
            .parse::<i32>()
            .unwrap();
        rustix::process::Pid::from_raw(raw).unwrap()
    }
}

impl Drop for ProcessFixture {
    fn drop(&mut self) {
        // Best effort: a destructor that panics while a failing assertion is
        // already unwinding aborts the whole binary, which discards the
        // captured panic and leaves no failing test name in the report.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn assert_group_quiescent(group: rustix::process::Pid) {
    let error = rustix::process::test_kill_process_group(group)
        .expect_err("held runner returned with a live process group");
    assert_eq!(error.raw_os_error(), libc::ESRCH);
}

#[test]
#[ignore = "invoked only as the held native process by this module"]
fn held_git_process_native_surrogate() {
    let Ok(mode) = std::fs::read_to_string("surrogate-mode") else {
        return;
    };
    match mode.as_str() {
        "exact" => {
            std::io::stdout().write_all(b"exact-output").unwrap();
            std::io::stdout().flush().unwrap();
            std::process::exit(23);
        }
        "isolation" => {
            let mut inherited_environment = std::env::vars_os()
                .map(|(key, value)| {
                    format!("{}={}", key.to_string_lossy(), value.to_string_lossy())
                })
                .collect::<Vec<_>>();
            inherited_environment.sort();
            let secret_fd = std::fs::read_to_string("secret-fd").unwrap();
            let inherited_secret = std::fs::read(format!("/dev/fd/{secret_fd}"))
                .is_ok_and(|bytes| bytes == b"must not cross exec");
            std::fs::write(
                "isolation-observation",
                format!(
                    "environment={}\nsecret_fd_visible={inherited_secret}\n",
                    inherited_environment.join("|")
                ),
            )
            .unwrap();
            std::process::exit(17);
        }
        "overflow" | "stderr-overflow" | "deadline" | "success-descendant" => {
            std::fs::write("leader-pid", std::process::id().to_string()).unwrap();
            let mut descendant = Command::new("/bin/sleep").arg("60").spawn().unwrap();
            std::fs::write("descendant-pid", descendant.id().to_string()).unwrap();
            if mode == "overflow" {
                std::io::stdout().write_all(&[b'x'; 8192]).unwrap();
                std::io::stdout().flush().unwrap();
            } else if mode == "stderr-overflow" {
                std::io::stderr().write_all(&[b'x'; 8192]).unwrap();
                std::io::stderr().flush().unwrap();
            } else if mode == "success-descendant" {
                std::process::exit(0);
            }
            let _ = descendant.wait();
            std::process::exit(19);
        }
        other => panic!("unknown held process surrogate mode {other}"),
    }
}

#[test]
fn held_runner_preserves_exact_status_and_stdout() {
    let _serial = process_test_lock();
    let fixture = ProcessFixture::new("exact");
    let (status, output) = fixture
        .run(4096, 4096, Instant::now() + SETTLED_DEADLINE)
        .unwrap();
    assert_eq!(status, 23);
    assert!(output.ends_with(b"exact-output"), "{output:?}");
    assert_eq!(
        output
            .windows(b"exact-output".len())
            .filter(|window| *window == b"exact-output")
            .count(),
        1
    );
}

#[test]
fn held_runner_executes_the_real_git_image() {
    let _serial = process_test_lock();
    let mut fixture = ProcessFixture::new("exact");
    fixture.executable_path = std::env::var_os("SEMAPRAX_TEST_GIT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/git"))
        .canonicalize()
        .unwrap();
    fixture.executable = open_file(&fixture.executable_path, false, false).unwrap();
    fixture.executable_metadata = fixture.executable.metadata().unwrap();
    let (status, output) = platform::run_for_test(
        &fixture.executable_path,
        &fixture.executable,
        &fixture.executable_metadata,
        &fixture.repository,
        &["--version"],
        &[],
        4096,
        4096,
        Instant::now() + SETTLED_DEADLINE,
    )
    .unwrap();
    assert_eq!(status, 0);
    assert!(output.starts_with(b"git version "), "{output:?}");
}

#[test]
fn held_runner_never_executes_a_replacement_path() {
    let _serial = process_test_lock();
    let mut fixture = ProcessFixture::new("exact");
    let trusted = fixture.root.join("trusted-runner");
    let retained = fixture.root.join("retained-runner");
    std::fs::copy(&fixture.executable_path, &trusted).unwrap();
    let mode = std::fs::metadata(&fixture.executable_path)
        .unwrap()
        .permissions()
        .mode();
    std::fs::set_permissions(&trusted, std::fs::Permissions::from_mode(mode)).unwrap();
    fixture.executable_path = trusted.clone();
    fixture.executable = open_file(&trusted, false, false).unwrap();
    fixture.executable_metadata = fixture.executable.metadata().unwrap();

    std::fs::rename(&trusted, &retained).unwrap();
    std::fs::copy("/usr/bin/false", &trusted).unwrap();
    std::fs::set_permissions(&trusted, std::fs::Permissions::from_mode(0o755)).unwrap();
    let result = fixture.run(4096, 4096, Instant::now() + SETTLED_DEADLINE);
    match result {
        Ok((status, output)) => {
            assert_eq!(status, 23);
            assert!(output.ends_with(b"exact-output"), "{output:?}");
        }
        #[cfg(target_os = "macos")]
        Err(error) => assert_eq!(
            error.to_string(),
            "suspended child did not map the held Git executable exactly once"
        ),
        #[cfg(not(target_os = "macos"))]
        Err(error) => panic!("held executable must run on this platform: {error}"),
    }
    assert_eq!(std::fs::read(&trusted).unwrap(), std::fs::read("/usr/bin/false").unwrap());
}

#[test]
fn held_runner_clears_environment_and_non_whitelisted_descriptors() {
    let _serial = process_test_lock();
    #[cfg(target_os = "macos")]
    let expected_environment = format!(
        "environment=GIT_ATTR_NOSYSTEM=1|GIT_CONFIG_GLOBAL=/dev/null|GIT_CONFIG_NOSYSTEM=1|GIT_CONFIG_SYSTEM=/dev/null|GIT_NO_LAZY_FETCH=1|GIT_NO_REPLACE_OBJECTS=1|GIT_OPTIONAL_LOCKS=0|GIT_TERMINAL_PROMPT=0|LC_ALL=C|__CF_USER_TEXT_ENCODING=0x{:X}:0:0\nsecret_fd_visible=false\n",
        rustix::process::getuid().as_raw()
    );
    #[cfg(not(target_os = "macos"))]
    let expected_environment = "environment=GIT_ATTR_NOSYSTEM=1|GIT_CONFIG_GLOBAL=/dev/null|GIT_CONFIG_NOSYSTEM=1|GIT_CONFIG_SYSTEM=/dev/null|GIT_NO_LAZY_FETCH=1|GIT_NO_REPLACE_OBJECTS=1|GIT_OPTIONAL_LOCKS=0|GIT_TERMINAL_PROMPT=0|LC_ALL=C\nsecret_fd_visible=false\n";

    let fixture = ProcessFixture::new("isolation");
    let secret_path = fixture.root.join("ambient-secret");
    std::fs::write(&secret_path, b"must not cross exec").unwrap();
    let secret: OwnedFd = rustix::fs::open(
        &secret_path,
        rustix::fs::OFlags::RDONLY,
        rustix::fs::Mode::empty(),
    )
    .unwrap();
    std::fs::remove_file(&secret_path).unwrap();
    std::fs::write(
        fixture.root.join("secret-fd"),
        secret.as_raw_fd().to_string(),
    )
    .unwrap();

    let (status, _) = fixture
        .run(4096, 4096, Instant::now() + SETTLED_DEADLINE)
        .unwrap();
    assert_eq!(status, 17);
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("isolation-observation")).unwrap(),
        expected_environment
    );
    drop(secret);
}

#[test]
fn output_overflow_settles_the_entire_owned_process_group() {
    let _serial = process_test_lock();
    let fixture = ProcessFixture::new("overflow");
    let error = fixture
        .run(4096, 4096, Instant::now() + SETTLED_DEADLINE)
        .expect_err("surrogate output must exceed the exact bound");
    assert_eq!(error.to_string(), "Git stdout exceeded byte bound");
    assert_group_quiescent(fixture.process_group());
}

#[test]
fn stderr_overflow_settles_the_entire_owned_process_group() {
    let _serial = process_test_lock();
    let fixture = ProcessFixture::new("stderr-overflow");
    let error = fixture
        .run(4096, 4096, Instant::now() + SETTLED_DEADLINE)
        .expect_err("surrogate stderr must exceed the exact bound");
    assert_eq!(error.to_string(), "Git stderr exceeded byte bound");
    assert_group_quiescent(fixture.process_group());
}

#[test]
fn successful_leader_cannot_leave_a_live_descendant() {
    let _serial = process_test_lock();
    let fixture = ProcessFixture::new("success-descendant");
    let (status, _) = fixture
        .run(4096, 4096, Instant::now() + SETTLED_DEADLINE)
        .unwrap();
    assert_eq!(status, 0);
    assert_group_quiescent(fixture.process_group());
}

#[test]
fn deadline_settles_the_entire_owned_process_group() {
    let _serial = process_test_lock();
    let fixture = ProcessFixture::new("deadline");
    let error = fixture
        .run(4096, 4096, Instant::now() + Duration::from_secs(1))
        .expect_err("blocked surrogate must reach the exact deadline");
    assert_eq!(error.to_string(), "Git host deadline exceeded");
    assert_group_quiescent(fixture.process_group());
}
