//! Opt-in only: run in a provisioned, serial private mapped user/mount namespace.
//! No fixture creates a namespace, attaches a mount, chroots, or executes a tool.
use super::super::tests::bundle;
use super::*;
use std::ffi::CStr;
use std::fs::File;
use std::io::Read as _;
use std::os::fd::FromRawFd as _;
use std::os::unix::fs::MetadataExt as _;

fn provisioned_page_size() -> usize {
    // This is an explicit provisioner precondition, NOT namespace attestation.
    // --test-threads=1 and no other descriptor users are additionally required.
    assert_eq!(
        std::env::var("SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT").as_deref(),
        Ok("private-user-mount-v1"),
        "requires an externally provisioned private mapped user+mount namespace"
    );
    // SAFETY: querying the provisioned host's page size changes no state.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    assert!(page_size > 0);
    page_size as usize
}

fn open_read(root: &Root, path: &CStr) -> File {
    // SAFETY: path is relative to the fixture-owned immutable detached tree.
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            path.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    assert!(fd >= 0, "open fixture: {}", std::io::Error::last_os_error());
    // SAFETY: this successful open transfers one new descriptor to File.
    unsafe { File::from_raw_fd(fd) }
}

fn assert_closed(control: &TestControl) {
    for fd in control.opened.iter().flatten() {
        // No fixture descriptor allocation occurs between failure and this check.
        assert_eq!(
            unsafe { libc::fcntl(*fd, libc::F_GETFD, 0 as libc::c_long) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }
}

#[test]
#[ignore = "requires provisioned private mapped user/mount namespace; run serially"]
fn provisioned_detached_root_bytes_modes_and_read_only() {
    let page_size = provisioned_page_size();
    let payload: Vec<u8> = (0..8199).map(|n| (n % 251) as u8).collect();
    let input = bundle(&[
        ("data/deep/binary", false, &payload),
        ("data/empty", false, b""),
    ]);
    let plan = Plan::prepare(&input, page_size).unwrap();
    let mut control = TestControl::default();
    // SAFETY: the explicitly provisioned isolated serial test process is the
    // controlled caller; the fixture does not enter production doctor dispatch.
    let old_umask = unsafe { libc::umask(0o777) };
    let result = materialize_inner(&plan, Some(&mut control));
    unsafe { libc::umask(old_umask) };
    let root = result.unwrap();
    // Independent physical oracle, not merely the production comparisons to
    // fields computed by Plan: root + bin/data/data/deep + three files.
    let mut stat = MaybeUninit::<libc::statfs64>::uninit();
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_fstatfs, root.as_raw_fd(), stat.as_mut_ptr()) },
        0
    );
    let stat = unsafe { stat.assume_init() };
    assert_eq!(stat.f_files, 7);
    assert_eq!(stat.f_blocks, (8199_usize.div_ceil(page_size) + 1) as u64);
    assert_eq!(stat.f_bsize, page_size as _);
    assert_eq!(stat.f_flags & 7, 7);
    assert_eq!(control.calls[Step::ReadOnly as usize], 1);
    assert_eq!(control.calls[Step::Verify as usize], 1);
    assert_eq!(control.calls[Step::Open as usize], plan.files().len());
    assert_eq!(
        control.calls[Step::Write as usize],
        plan.files()
            .iter()
            .map(|f| f.bytes.len().div_ceil(8192))
            .sum::<usize>()
    );
    for file in plan.files() {
        let mut reopened = open_read(&root, &file.path);
        let metadata = reopened.metadata().unwrap();
        assert!(metadata.is_file());
        assert_eq!(
            metadata.mode() & 0o7777,
            if file.executable { 0o500 } else { 0o400 }
        );
        let mut bytes = Vec::new();
        reopened.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, file.bytes);
    }
    for directory in plan.directories() {
        let reopened = open_read(&root, directory);
        let metadata = reopened.metadata().unwrap();
        assert!(metadata.is_dir());
        assert_eq!(metadata.mode() & 0o7777, 0o700);
    }
    let root_metadata = open_read(&root, c".").metadata().unwrap();
    assert_eq!(root_metadata.mode() & 0o7777, 0o700);
    for path in [c"proc", c"dev", c"etc", c"not-in-bundle"] {
        assert_eq!(
            unsafe {
                libc::openat(
                    root.as_raw_fd(),
                    path.as_ptr(),
                    libc::O_RDONLY | libc::O_CLOEXEC,
                )
            },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ENOENT)
        );
    }
    // All failures must be the actual read-only mount decision, not permissions.
    for flags in [libc::O_WRONLY, libc::O_WRONLY | libc::O_TRUNC] {
        assert_eq!(
            unsafe {
                libc::openat(
                    root.as_raw_fd(),
                    c"data/deep/binary".as_ptr(),
                    flags | libc::O_CLOEXEC,
                )
            },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EROFS)
        );
    }
    assert_eq!(
        unsafe { libc::mkdirat(root.as_raw_fd(), c"extra".as_ptr(), 0o700) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EROFS)
    );
    // Detached tree lifetime follows held references; this read handle remains
    // usable after its mount descriptor is closed, without any host attachment.
    let mut retained = open_read(&root, c"data/deep/binary");
    unsafe { root.close() };
    let mut bytes = Vec::new();
    retained.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, payload);
    drop(retained);
    assert_closed(&control);
}

#[test]
#[ignore = "requires provisioned private mapped user/mount namespace; run serially"]
fn provisioned_wrong_page_cost_stops_before_tree_writes() {
    let page_size = provisioned_page_size();
    let input = bundle(&[("data/value", false, b"x")]);
    let wrong_size = if page_size == 4096 { 8192 } else { 4096 };
    let plan = Plan::prepare(&input, wrong_size).unwrap();
    let mut control = TestControl::default();
    assert_eq!(
        materialize_inner(&plan, Some(&mut control)).unwrap_err(),
        Error::Io
    );
    assert_eq!(control.calls[Step::Inspect as usize], 1);
    assert_eq!(control.calls[Step::Directory as usize], 0);
    assert_eq!(control.calls[Step::Open as usize], 0);
    assert_eq!(control.calls[Step::Write as usize], 0);
    assert_closed(&control);
}

#[test]
#[ignore = "requires provisioned private mapped user/mount namespace; run serially"]
fn provisioned_setup_and_exact_write_failures_return_no_root() {
    let page_size = provisioned_page_size();
    let bytes = [7; 8193];
    let input = bundle(&[("data/value", false, &bytes)]);
    let plan = Plan::prepare(&input, page_size).unwrap();
    // Calibrate actual successful materialization before injecting any outcome.
    let root = unsafe { materialize(&plan) }.unwrap();
    unsafe { root.close() };
    let cases = [
        (Step::FsOpen, 1),
        (Step::Configure, 1),
        (Step::Configure, 2),
        (Step::Configure, 3),
        (Step::Create, 1),
        (Step::Mount, 1),
        (Step::Inspect, 1),
        (Step::Directory, 1),
        (Step::DirectoryMode, 1),
        (Step::Open, 1),
        (Step::Write, 1),
        (Step::Write, 2),
        (Step::FileMode, 1),
        (Step::ReadOnly, 1),
        (Step::Verify, 1),
    ];
    for (step, call) in cases {
        let mut control = TestControl {
            fault: Some((step, call)),
            ..TestControl::default()
        };
        assert_eq!(
            materialize_inner(&plan, Some(&mut control)).unwrap_err(),
            Error::Io
        );
        assert_eq!(control.calls[step as usize], call);
        if step != Step::Verify {
            assert_eq!(control.calls[Step::Verify as usize], 0);
        }
        assert_closed(&control);
    }
    // Zero/short output takes the same no-retry failure branch as -1, including
    // EINTR. These are injected syscall outcomes, not delivered-signal evidence.
    for result in [0, 1] {
        let mut control = TestControl {
            fault: Some((Step::Write, 2)),
            write_result: Some(result),
            ..TestControl::default()
        };
        assert_eq!(
            materialize_inner(&plan, Some(&mut control)).unwrap_err(),
            Error::Io
        );
        assert_eq!(control.calls[Step::Write as usize], 2);
        assert_eq!(control.calls[Step::ReadOnly as usize], 0);
        assert_closed(&control);
    }
}

#[test]
#[ignore = "requires provisioned private mapped user/mount namespace; run serially"]
fn provisioned_metadata_mismatches_feed_actual_admission() {
    let page_size = provisioned_page_size();
    let input = bundle(&[("data/value", false, b"x")]);
    let plan = Plan::prepare(&input, page_size).unwrap();
    let root = unsafe { materialize(&plan) }.unwrap();
    unsafe { root.close() };
    for final_check in [false, true] {
        for fault in [
            Corrupt::Type,
            Corrupt::PageSize,
            Corrupt::Blocks,
            Corrupt::Inodes,
            Corrupt::Flags,
        ] {
            if !final_check && matches!(fault, Corrupt::Flags) {
                continue;
            }
            let mut control = TestControl {
                corrupt: Some((final_check, fault)),
                ..TestControl::default()
            };
            assert_eq!(
                materialize_inner(&plan, Some(&mut control)).unwrap_err(),
                Error::Io
            );
            if !final_check {
                assert_eq!(control.calls[Step::Directory as usize], 0);
                assert_eq!(control.calls[Step::Open as usize], 0);
            } else {
                assert_eq!(control.calls[Step::Verify as usize], 1);
            }
            assert_closed(&control);
        }
    }
}

#[test]
#[ignore = "requires provisioned private mapped user/mount namespace; run serially"]
fn provisioned_close_uncertainty_is_fail_stop() {
    let page_size = provisioned_page_size();
    let input = bundle(&[]);
    let plan = Plan::prepare(&input, page_size).unwrap();
    let root = unsafe { materialize(&plan) }.unwrap();
    unsafe { root.close() };
    if std::env::var_os("SEMAPRAX_DOCTOR_ROOT_CLOSE_FAULT").is_some() {
        let mut control = TestControl {
            close_failure: true,
            ..TestControl::default()
        };
        let _unexpected = materialize_inner(&plan, Some(&mut control));
        panic!("uncertain close returned to its caller");
    }
    // This reexecutes only this test inside the already provisioned namespace;
    // it does not bootstrap a namespace or execute a bundled tool. The test
    // process descriptor table must itself satisfy the controlled-FD prerequisite.
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "doctor::offline_root::linux::tests::provisioned_close_uncertainty_is_fail_stop",
            "--ignored",
            "--test-threads=1",
        ])
        .env("SEMAPRAX_DOCTOR_ROOT_CLOSE_FAULT", "1")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(126));
}
