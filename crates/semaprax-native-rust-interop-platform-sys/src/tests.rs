//! Source-locked hostile evidence for the OS quarantine.
//!
//! The production text constants below are the audit subject: every submodule
//! that pins an ordering, count, or forbidden construct reads them, so they
//! must cover the whole platform module including its submodules.
//!
//! Cases that fork this test binary by exact path stay in this root, because
//! their `--exact tests::<name>` filters name the module path.
const COMMON_SOURCE: &str = include_str!("lib.rs");
const UNIX_SOURCE: &str = concat!(
    include_str!("unix.rs"),
    include_str!("unix/plans.rs"),
    include_str!("unix/primitives.rs"),
    include_str!("unix/handles.rs"),
    include_str!("unix/inventory.rs"),
    include_str!("unix/process.rs"),
    include_str!("unix/archive.rs"),
);
const WINDOWS_SOURCE: &str = concat!(
    include_str!("windows.rs"),
    include_str!("windows/plans.rs"),
    include_str!("windows/handles.rs"),
    include_str!("windows/inventory.rs"),
    include_str!("windows/process.rs"),
    include_str!("windows/invocations.rs"),
);

fn production_sources() -> String {
    [COMMON_SOURCE, UNIX_SOURCE, WINDOWS_SOURCE].concat()
}

use super::*;
use super::{enter_prepared_file_syscalls, Error, TEST_PREPARED_FILE_SYSCALL_ENTRIES};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use super::{set_test_settlement_failures, TestSettlementFailure};
use std::sync::atomic::Ordering;

mod archive_admission;
mod inventory;
mod linux_runner;
mod source_contracts;
mod spawn_settlement;
mod windows_archive;

#[cfg(target_os = "linux")]
fn linux_runner_failure_helper(
    points: &[TestSettlementFailure],
    expected: Option<Error>,
    sentinel: &str,
) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(points);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("noisy")).unwrap();
    let result =
        super::platform::execute_harness_with_output_limit(&executable, &directory, 65_536);
    if let Some(expected) = expected {
        assert_eq!(result, Err(expected));
    }
    std::fs::write(root.join(sentinel), b"returned").unwrap();
}

#[cfg(target_os = "linux")]
macro_rules! linux_runner_helper {
        ($name:ident, [$($point:ident),+], $expected:expr, $sentinel:literal) => {
            #[test]
            fn $name() {
                linux_runner_failure_helper(
                    &[$(TestSettlementFailure::$point),+],
                    $expected,
                    $sentinel,
                );
            }
        };
    }

#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_pipe_read_fcntl,
    [UnixPipeReadFcntl],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_pipe_write_fcntl,
    [UnixPipeWriteFcntl],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_drain_fcntl,
    [UnixDrainFcntl],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(helper_linux_poll, [UnixPoll], Some(Error::Spawn), "settled");
#[cfg(target_os = "linux")]
linux_runner_helper!(helper_linux_read, [UnixRead], Some(Error::Spawn), "settled");
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_read_conversion,
    [UnixReadConversion],
    Some(Error::OutputLimit),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_waitpid,
    [UnixWaitpid],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_deadline,
    [UnixDeadline],
    Some(Error::Spawn),
    "settled"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_parent_write_close,
    [UnixParentWriteClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_parent_null_close,
    [UnixParentNullClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_settle_close,
    [UnixDrainFcntl, UnixSettleClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_success_read_close,
    [UnixSuccessReadClose],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_wait_settlement,
    [UnixDrainFcntl, UnixWait],
    None,
    "post-fail-stop"
);
#[cfg(target_os = "linux")]
linux_runner_helper!(
    helper_linux_group_settlement,
    [UnixDrainFcntl, UnixGroup],
    None,
    "post-fail-stop"
);

#[cfg(target_os = "macos")]
fn darwin_failure_helper(points: &[TestSettlementFailure]) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(points);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("quiet")).unwrap();
    let _ = super::platform::execute_harness(&executable, &directory);
    std::fs::write(root.join("post-fail-stop"), b"returned").unwrap();
}

#[cfg(target_os = "macos")]
fn darwin_returning_failure_helper(point: TestSettlementFailure, expected: Error) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(&[point]);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new("quiet")).unwrap();
    assert_eq!(
        super::platform::execute_harness(&executable, &directory),
        Err(expected)
    );
    std::fs::write(root.join("post-return"), b"returned").unwrap();
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_actions_destroy() {
    darwin_failure_helper(&[TestSettlementFailure::DarwinActionsDestroy]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_attributes_destroy() {
    darwin_failure_helper(&[TestSettlementFailure::DarwinAttributesDestroy]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_attest_settlement_fail_stop() {
    darwin_failure_helper(&[
        TestSettlementFailure::DarwinAttest,
        TestSettlementFailure::UnixWait,
    ]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_sigcont_settlement_fail_stop() {
    darwin_failure_helper(&[
        TestSettlementFailure::DarwinSigcont,
        TestSettlementFailure::UnixGroup,
    ]);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_attest_returns_changed_after_settlement() {
    darwin_returning_failure_helper(TestSettlementFailure::DarwinAttest, Error::Changed);
}

#[cfg(target_os = "macos")]
#[test]
fn helper_darwin_sigcont_returns_spawn_after_settlement() {
    darwin_returning_failure_helper(TestSettlementFailure::DarwinSigcont, Error::Spawn);
}

#[cfg(target_os = "windows")]
fn windows_runner_failure_helper(
    points: &[TestSettlementFailure],
    executable_name: &str,
    expected: Option<Error>,
    bounded_output: bool,
    sentinel: &str,
) {
    let Some(root) = std::env::var_os("SEMAPRAX_SYS_TEST_HELPER_ROOT") else {
        return;
    };
    set_test_settlement_failures(points);
    let root = std::path::PathBuf::from(root);
    let directory = super::platform::hold_directory(&root).unwrap();
    let executable =
        super::platform::hold_executable(&directory, std::ffi::OsStr::new(executable_name))
            .unwrap();
    let result = if bounded_output {
        super::platform::clang_version(&executable, &directory, 64).map(|_| ())
    } else {
        super::platform::execute_harness(&executable, &directory)
    };
    if let Some(expected) = expected {
        assert_eq!(result, Err(expected));
    }
    std::fs::write(root.join(sentinel), b"returned").unwrap();
}

#[cfg(target_os = "windows")]
macro_rules! windows_runner_helper {
        ($name:ident, [$($point:ident),+], $exe:literal, $expected:expr, $bounded:expr, $sentinel:literal) => {
            #[test]
            fn $name() {
                windows_runner_failure_helper(
                    &[$(TestSettlementFailure::$point),+],
                    $exe,
                    $expected,
                    $bounded,
                    $sentinel,
                );
            }
        };
    }

#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_image,
    [WindowsImage],
    "quiet.exe",
    Some(Error::Changed),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_assign,
    [WindowsAssign],
    "quiet.exe",
    Some(Error::Changed),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_resume,
    [WindowsResume],
    "quiet.exe",
    Some(Error::Spawn),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_peek,
    [WindowsPeek],
    "quiet.exe",
    Some(Error::Spawn),
    false,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_read,
    [WindowsRead],
    "output.exe",
    Some(Error::Spawn),
    true,
    "settled"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_unassigned_fail_stop,
    [WindowsImage, WindowsTerminateProcess],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_wait_unassigned_fail_stop,
    [WindowsImage, WindowsWaitUnassigned],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_terminate_job_fail_stop,
    [WindowsPeek, WindowsTerminateJob],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);
#[cfg(target_os = "windows")]
windows_runner_helper!(
    helper_windows_query_job_fail_stop,
    [WindowsPeek, WindowsQueryJob],
    "quiet.exe",
    None,
    false,
    "post-fail-stop"
);

#[cfg(target_os = "linux")]
fn inventory_record(name: &[u8], inode: u64) -> Vec<u8> {
    let length = (19 + name.len() + 1 + 7) & !7;
    let mut bytes = vec![0_u8; length];
    bytes[..8].copy_from_slice(&inode.to_ne_bytes());
    bytes[16..18].copy_from_slice(&u16::try_from(length).unwrap().to_ne_bytes());
    bytes[18] = 8;
    bytes[19..19 + name.len()].copy_from_slice(name);
    bytes
}

#[cfg(unix)]
fn with_inventory_fixture(
    root: &std::path::Path,
    action: impl FnOnce(
        &super::platform::Directory,
        &super::platform::PreparedDiscardNames<1>,
        &super::platform::RegularFile,
        &mut super::platform::PreparedInventoryExact<1>,
    ),
) {
    use std::ffi::OsStr;

    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).unwrap();
    let directory = super::platform::hold_directory(root).unwrap();
    let names = super::platform::prepare_discard_names([OsStr::new("a")]).unwrap();
    let file = super::platform::write_file_new_prepared(&directory, &names, 0, b"inventory", 0o600)
        .unwrap();
    let mut prepared = super::platform::prepare_inventory_exact(&names).unwrap();
    action(&directory, &names, &file, &mut prepared);
    drop((prepared, file, names, directory));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "macos")]
fn inventory_record(name: &[u8], inode: u64) -> Vec<u8> {
    let length = (21 + name.len() + 1 + 3) & !3;
    let mut bytes = vec![0_u8; length];
    bytes[..8].copy_from_slice(&inode.to_ne_bytes());
    bytes[16..18].copy_from_slice(&u16::try_from(length).unwrap().to_ne_bytes());
    bytes[18..20].copy_from_slice(&u16::try_from(name.len()).unwrap().to_ne_bytes());
    bytes[20] = 8;
    bytes[21..21 + name.len()].copy_from_slice(name);
    bytes
}

#[cfg(unix)]
#[test]
fn prepared_inventory_rebound_close_failure_child() {
    let Ok(root) = std::env::var("SEMAPRAX_INVENTORY_CLOSE_FAILURE_ROOT") else {
        return;
    };
    let root = std::path::Path::new(&root);
    with_inventory_fixture(root, |directory, names, file, prepared| {
        super::platform::test_inventory_exact_failures(prepared, false, false, true, true);
        let _ = super::platform::inventory_exact_prepared(prepared, directory, names, [Some(file)]);
        std::fs::write(root.join("later-action"), b"must not exist").unwrap();
    });
}
