use super::super::{
    create_executable_with_test, TestControl, TestFault, TestStage, TestWriteFault,
};
use super::*;
use crate::DoctorOfflineInput;
use std::fs::File;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, MetadataExt};

mod faults;

// Literal Linux UAPI bits; additional kernel seals may legitimately coexist.
const REQUIRED_SEALS: i32 = 1 | 2 | 4 | 8 | 0x20;

fn completed_stages(length: usize) -> Vec<TestStage> {
    let mut stages = vec![TestStage::Create, TestStage::Mode];
    for offset in (0..length).step_by(8192) {
        stages.push(TestStage::Write {
            offset,
            length: (length - offset).min(8192),
        });
    }
    stages.extend([
        TestStage::Seal,
        TestStage::GetSeals,
        TestStage::Metadata,
        TestStage::GetFlags,
        TestStage::Snapshot,
        TestStage::Compare,
    ]);
    stages
}

fn assert_file(file: &mut File, bytes: &[u8]) {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0);
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
    let seals = unsafe { libc::fcntl(fd, libc::F_GET_SEALS) };
    assert!(seals >= 0);
    assert_eq!(seals & REQUIRED_SEALS, REQUIRED_SEALS);
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.mode() & 0o7777, 0o500);
    assert_eq!(metadata.len(), bytes.len() as u64);
    assert_eq!(file.stream_position().unwrap(), 0);
    assert_eq!(
        DoctorOfflineInput::acquire(file, bytes.len())
            .unwrap()
            .bytes(),
        bytes
    );
    assert_eq!(file.stream_position().unwrap(), 0);
}

#[test]
fn real_creation_calibrates_bounded_writes_and_returns_independent_exact_storage() {
    for length in [120, 8192, 8193] {
        let bytes = image(length);
        let mut control = TestControl::default();
        let (mut file, snapshot) =
            create_executable_with_test(&bytes, length, &mut control).unwrap();
        assert_eq!(control.events, completed_stages(length));
        assert_eq!(snapshot.bytes(), bytes);
        assert_file(&mut file, &bytes);
        let (mut repeated, retained) = create_doctor_offline_executable(&bytes, length).unwrap();
        assert_ne!(file.as_raw_fd(), repeated.as_raw_fd());
        assert_ne!(
            file.metadata().unwrap().ino(),
            repeated.metadata().unwrap().ino()
        );
        assert_file(&mut repeated, &bytes);
        drop(file);
        drop(repeated);
        assert_eq!(snapshot.bytes(), bytes);
        assert_eq!(retained.bytes(), bytes);
    }
    // A maximum permitted caller limit is not a 512-MiB payload claim.
    let bytes = image(120);
    let (mut file, _) =
        create_doctor_offline_executable(&bytes, DOCTOR_OFFLINE_INPUT_MAX_BYTES).unwrap();
    assert_file(&mut file, &bytes);
}

#[test]
fn sealed_storage_denies_mutation_and_execute_bit_changes_without_changing_bytes() {
    let bytes = image(8192);
    let (mut file, snapshot) = create_doctor_offline_executable(&bytes, bytes.len()).unwrap();
    assert!(file.write_at(b"x", 0).is_err());
    assert!(file.set_len(0).is_err());
    assert!(file.set_len(8193).is_err());
    let mapped = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            8192,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            file.as_raw_fd(),
            0,
        )
    };
    if mapped != libc::MAP_FAILED {
        assert_eq!(unsafe { libc::munmap(mapped, 8192) }, 0);
        panic!("executable carrier admitted a writable shared mapping");
    }
    // F_SEAL_EXEC freezes X bits, not every permission/metadata bit.
    for mode in [0o400, 0o510, 0o501, 0o511] {
        assert_eq!(unsafe { libc::fchmod(file.as_raw_fd(), mode) }, -1);
        assert_file(&mut file, &bytes);
    }
    assert_eq!(snapshot.bytes(), bytes);
}

fn interpreter(path: &[u8]) -> Vec<u8> {
    let mut bytes = image(120);
    bytes[64..68].copy_from_slice(&3u32.to_le_bytes());
    bytes[72..80].copy_from_slice(&120u64.to_le_bytes());
    bytes[96..104].copy_from_slice(&(path.len() as u64).to_le_bytes());
    bytes.extend_from_slice(path);
    bytes
}

#[test]
fn malformed_native_elf_and_interpreter_metadata_reject_before_creation() {
    let valid = image(120);
    let mut wrong_arch = valid.clone();
    let foreign = if cfg!(target_arch = "x86_64") {
        183u16
    } else {
        62u16
    };
    wrong_arch[18..20].copy_from_slice(&foreign.to_le_bytes());
    let mut wrong_class = valid.clone();
    wrong_class[4] = 1;
    let mut overflow_table = valid.clone();
    overflow_table[32..40].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut missing_table = valid.clone();
    missing_table[56..58].copy_from_slice(&0u16.to_le_bytes());
    for bytes in [
        b"#!/bin/sh\nexit 0\n".to_vec(),
        valid[..63].to_vec(),
        wrong_arch,
        wrong_class,
        overflow_table,
        missing_table,
        interpreter(b"relative\0"),
        interpreter(b"/missing-terminator"),
        interpreter(b"/a\0b\0"),
    ] {
        let mut control = TestControl::default();
        assert_eq!(
            create_executable_with_test(&bytes, bytes.len(), &mut control).unwrap_err(),
            Error::Invalid
        );
        assert!(control.events.is_empty());
    }
}

#[test]
fn valid_interpreter_metadata_is_storage_only_not_path_lookup_or_loadability() {
    let bytes = interpreter(b"/semaprax-fixture-no-loader-lookup\0");
    let (mut file, snapshot) = create_doctor_offline_executable(&bytes, bytes.len()).unwrap();
    assert_file(&mut file, &bytes);
    assert_eq!(snapshot.bytes(), bytes);
    // No exec, existence check, or claim that this interpreter can be loaded.
}
