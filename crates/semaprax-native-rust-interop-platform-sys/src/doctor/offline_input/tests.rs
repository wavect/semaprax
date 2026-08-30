//! Authored kernel-descriptor fixtures; no executable/profile admission claims.
#![cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use super::*;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{FileExt, MetadataExt};

const HARD_LIMIT: usize = 512 * 1024 * 1024;
const REQUIRED: libc::c_int = 1 | 2 | 4 | 8;

fn memfd(bytes: &[u8], seals: libc::c_int) -> File {
    let raw = unsafe {
        libc::memfd_create(
            c"doctor-input-fixture".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    assert!(raw >= 0, "{}", std::io::Error::last_os_error());
    let mut file = unsafe { File::from_raw_fd(raw) };
    file.write_all(bytes).unwrap();
    assert_eq!(unsafe { libc::fcntl(raw, libc::F_ADD_SEALS, seals) }, 0);
    file.seek(SeekFrom::Start(0)).unwrap();
    file
}

fn state(file: &File) -> (libc::c_int, libc::c_int, libc::c_int, libc::off_t) {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD, 0 as libc::c_long) };
    assert!(flags >= 0, "caller descriptor was closed");
    unsafe {
        (
            flags,
            libc::fcntl(fd, libc::F_GETFL, 0 as libc::c_long),
            libc::fcntl(fd, libc::F_GET_SEALS, 0 as libc::c_long),
            libc::lseek(fd, 0, libc::SEEK_CUR),
        )
    }
}

fn acquire(
    file: &File,
    limit: usize,
    fault: Option<TestFault>,
) -> (
    Result<DoctorOfflineInput, DoctorOfflineInputError>,
    Vec<TestStage>,
) {
    let before = state(file);
    let mut control = TestControl {
        trace: Vec::new(),
        fault,
    };
    let result = DoctorOfflineInput::acquire_with_test(file, limit, &mut control);
    assert_eq!(state(file), before);
    (result, control.trace)
}

fn pre_read() -> Vec<TestStage> {
    vec![
        TestStage::Seals,
        TestStage::FileSystem,
        TestStage::Metadata,
        TestStage::Allocate,
    ]
}

#[test]
fn binary_snapshot_is_owned_and_preserves_offset_flags_seals_and_caller() {
    let bytes = (0..16_391)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let mut file = memfd(&bytes, REQUIRED);
    file.seek(SeekFrom::Start(37)).unwrap();
    let (result, trace) = acquire(&file, bytes.len(), None);
    let snapshot = result.unwrap();
    assert_eq!(snapshot.bytes(), bytes);
    let mut expected = pre_read();
    expected.extend([
        TestStage::Read {
            offset: 0,
            length: 8192,
        },
        TestStage::Read {
            offset: 8192,
            length: 8192,
        },
        TestStage::Read {
            offset: 16384,
            length: 7,
        },
    ]);
    assert_eq!(trace, expected);
    let mut first = [255];
    assert_eq!(file.read_at(&mut first, 0).unwrap(), 1);
    assert_eq!(first, [0]);
    let repeated = DoctorOfflineInput::acquire(&file, bytes.len()).unwrap();
    assert_eq!(state(&file).3, 37);
    assert_eq!(repeated.bytes(), snapshot.bytes());
    drop(file);
    assert_eq!(snapshot.bytes(), bytes);
    assert_eq!(repeated.bytes(), bytes);
}

#[test]
fn every_required_seal_subset_and_future_write_only_are_distinguished() {
    for seals in 0..=15 {
        let file = memfd(b"\0\xffsealed", seals);
        let (result, trace) = acquire(&file, 32, None);
        if seals == REQUIRED {
            assert_eq!(result.unwrap().bytes(), b"\0\xffsealed");
            let mut expected = pre_read();
            expected.push(TestStage::Read {
                offset: 0,
                length: 8,
            });
            assert_eq!(trace, expected);
        } else {
            assert_eq!(
                result.unwrap_err(),
                DoctorOfflineInputError::Invalid,
                "seals={seals}"
            );
            assert_eq!(trace, [TestStage::Seals]);
        }
    }
    // FUTURE_WRITE (16) leaves previously writable mappings possible: not WRITE (8).
    let file = memfd(b"future", 1 | 2 | 4 | 16);
    let (result, trace) = acquire(&file, 32, None);
    assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Invalid);
    assert_eq!(trace, [TestStage::Seals]);
    let stronger = memfd(b"extra-seals", REQUIRED | 16);
    assert_eq!(
        DoctorOfflineInput::acquire(&stronger, 32).unwrap().bytes(),
        b"extra-seals"
    );
}

#[test]
fn writable_shared_mapping_prevents_write_seal_until_unmapped() {
    let file = memfd(&[0; 4096], 2 | 4);
    let address = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            4096,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            file.as_raw_fd(),
            0,
        )
    };
    assert_ne!(address, libc::MAP_FAILED);
    struct Mapping(*mut libc::c_void);
    impl Drop for Mapping {
        fn drop(&mut self) {
            assert_eq!(unsafe { libc::munmap(self.0, 4096) }, 0);
        }
    }
    let mapping = Mapping(address);
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, 8) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBUSY)
    );
    let (result, trace) = acquire(&file, 4096, None);
    assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Invalid);
    assert_eq!(trace, [TestStage::Seals]);
    drop(mapping);
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, 1 | 8) },
        0
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&file, 4096).unwrap().bytes(),
        &[0; 4096]
    );
}

#[test]
fn ordinary_descriptors_reject_at_seals_without_metadata_or_reads() {
    let root = std::env::temp_dir().join(format!("semaprax-offline-input-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let root_identity = std::fs::symlink_metadata(&root).unwrap();
    let path = root.join("ordinary");
    let mut ordinary = OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    ordinary.write_all(b"ordinary-input-sentinel").unwrap();
    let identity = ordinary.metadata().unwrap();
    let directory = File::open(&root).unwrap();
    use std::os::unix::ffi::OsStrExt as _;
    let cpath = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    let raw = unsafe { libc::open(cpath.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    assert!(raw >= 0);
    let path_only = unsafe { File::from_raw_fd(raw) };
    let mut pipe = [-1; 2];
    assert_eq!(
        unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) },
        0
    );
    let pipe = pipe.map(|fd| unsafe { File::from_raw_fd(fd) });
    let mut sockets = [-1; 2];
    assert_eq!(
        unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                0,
                sockets.as_mut_ptr(),
            )
        },
        0
    );
    let sockets = sockets.map(|fd| unsafe { File::from_raw_fd(fd) });
    for file in [
        &ordinary,
        &directory,
        &pipe[0],
        &pipe[1],
        &sockets[0],
        &sockets[1],
    ] {
        let (result, trace) = acquire(file, 32, None);
        assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Invalid);
        assert_eq!(trace, [TestStage::Seals]);
    }
    // Linux rejects F_GET_SEALS on O_PATH with EBADF, not unsupported-kind EINVAL.
    let (result, trace) = acquire(&path_only, 32, None);
    assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Io);
    assert_eq!(trace, [TestStage::Seals]);
    // No recursive cleanup: verify the exact owned root/file before removing either.
    let names = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(names, [std::ffi::OsString::from("ordinary")]);
    let current = std::fs::symlink_metadata(&path).unwrap();
    assert!(current.is_file() && !current.file_type().is_symlink());
    assert_eq!(
        (current.dev(), current.ino()),
        (identity.dev(), identity.ino())
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"ordinary-input-sentinel");
    let current = std::fs::symlink_metadata(&root).unwrap();
    assert!(current.is_dir() && !current.file_type().is_symlink());
    assert_eq!(
        (current.dev(), current.ino()),
        (root_identity.dev(), root_identity.ino())
    );
    drop((ordinary, directory, path_only, pipe, sockets));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn limits_precede_allocation_and_sparse_oversize_never_reads() {
    let file = memfd(b"exact", REQUIRED);
    for (limit, expected) in [
        (0, DoctorOfflineInputError::Invalid),
        (HARD_LIMIT + 1, DoctorOfflineInputError::Limit),
        (usize::MAX, DoctorOfflineInputError::Limit),
    ] {
        let (result, trace) = acquire(&file, limit, None);
        assert_eq!(result.unwrap_err(), expected);
        assert!(trace.is_empty());
    }
    assert_eq!(
        DoctorOfflineInput::acquire(&file, 5).unwrap().bytes(),
        b"exact"
    );
    let (result, trace) = acquire(&file, 4, None);
    assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Limit);
    assert_eq!(
        trace,
        [TestStage::Seals, TestStage::FileSystem, TestStage::Metadata]
    );
    let sparse = memfd(b"", 0);
    sparse.set_len((HARD_LIMIT + 1) as u64).unwrap();
    assert_eq!(
        unsafe { libc::fcntl(sparse.as_raw_fd(), libc::F_ADD_SEALS, REQUIRED) },
        0
    );
    let (result, trace) = acquire(&sparse, HARD_LIMIT, None);
    assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Limit);
    assert_eq!(
        trace,
        [TestStage::Seals, TestStage::FileSystem, TestStage::Metadata]
    );
    let empty = memfd(b"", REQUIRED);
    let (result, trace) = acquire(&empty, 1, None);
    assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Invalid);
    assert_eq!(
        trace,
        [TestStage::Seals, TestStage::FileSystem, TestStage::Metadata]
    );
}

#[test]
fn injected_stage_failures_stop_without_later_actions() {
    let file = memfd(b"payload", REQUIRED);
    for (fault, error, stage_count) in [
        (TestFault::Seals, DoctorOfflineInputError::Io, 1),
        (TestFault::FileSystem, DoctorOfflineInputError::Io, 2),
        (
            TestFault::WrongFileSystem,
            DoctorOfflineInputError::Invalid,
            2,
        ),
        (TestFault::Metadata, DoctorOfflineInputError::Io, 3),
        (TestFault::NonRegular, DoctorOfflineInputError::Invalid, 3),
        (TestFault::NegativeSize, DoctorOfflineInputError::Invalid, 3),
        (TestFault::Allocation, DoctorOfflineInputError::Io, 4),
    ] {
        let (result, trace) = acquire(&file, 32, Some(fault));
        assert_eq!(result.unwrap_err(), error);
        assert_eq!(trace, pre_read()[..stage_count]);
        assert_eq!(
            DoctorOfflineInput::acquire(&file, 32).unwrap().bytes(),
            b"payload"
        );
    }
}

#[test]
fn short_zero_interrupted_and_failed_reads_publish_nothing_and_never_retry() {
    let bytes = vec![0xa5; 16_391];
    let file = memfd(&bytes, REQUIRED);
    for call in [1, 2, 3] {
        for outcome in [
            TestReadFault::Short,
            TestReadFault::Zero,
            TestReadFault::Interrupted,
            TestReadFault::Io,
        ] {
            let (result, trace) =
                acquire(&file, bytes.len(), Some(TestFault::Read { call, outcome }));
            assert_eq!(result.unwrap_err(), DoctorOfflineInputError::Io);
            let mut expected = pre_read();
            expected.extend(
                [
                    TestStage::Read {
                        offset: 0,
                        length: 8192,
                    },
                    TestStage::Read {
                        offset: 8192,
                        length: 8192,
                    },
                    TestStage::Read {
                        offset: 16384,
                        length: 7,
                    },
                ]
                .into_iter()
                .take(call),
            );
            assert_eq!(trace, expected);
            assert_eq!(
                DoctorOfflineInput::acquire(&file, bytes.len())
                    .unwrap()
                    .bytes(),
                bytes
            );
        }
    }
}
