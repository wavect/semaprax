//! Real sealed-input handoff into the exact source-included parser body.
//! This is not cross-crate runtime linkage or a runnable tool fixture.
#[path = "../../../../semaprax-native-rust-interop-platform/src/doctor_offline_bundle.rs"]
mod bundle;

use super::DoctorOfflineInput;
use bundle::{
    DoctorOfflineArchitecture, DoctorOfflineBundle, DoctorOfflineBundleError, DoctorOfflineTool,
};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};

fn capsule() -> (Vec<u8>, Vec<u8>, DoctorOfflineArchitecture) {
    let (tag, machine, arch) = if cfg!(target_arch = "x86_64") {
        (1_u8, 62_u16, DoctorOfflineArchitecture::LinuxX86_64)
    } else {
        (2_u8, 183_u16, DoctorOfflineArchitecture::LinuxAarch64)
    };
    let mut elf = vec![0; 120];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2_u16.to_le_bytes());
    elf[18..20].copy_from_slice(&machine.to_le_bytes());
    elf[20..24].copy_from_slice(&1_u32.to_le_bytes());
    elf[32..40].copy_from_slice(&64_u64.to_le_bytes());
    elf[52..54].copy_from_slice(&64_u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56_u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1_u16.to_le_bytes());
    elf[64..68].copy_from_slice(&1_u32.to_le_bytes());
    let mut bytes = b"SPXDOC1\0".to_vec();
    bytes.extend([tag, 2, 10, 0, 1, 0, 0, 0]);
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(b"handoff-v1");
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend([1, 0]);
    bytes.extend_from_slice(&120_u64.to_le_bytes());
    bytes.extend_from_slice(b"bin/node");
    bytes.extend_from_slice(&elf);
    (bytes, elf, arch)
}

fn seal(bytes: &[u8]) -> File {
    let fd = unsafe {
        libc::memfd_create(
            c"doctor-bundle-handoff".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    assert!(fd >= 0, "{}", std::io::Error::last_os_error());
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).unwrap();
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, 15) }, 0);
    file.seek(SeekFrom::Start(7)).unwrap();
    file
}

#[test]
fn sealed_snapshot_public_parser_body_retains_zero_copy_views_after_file_drop() {
    let (bytes, elf, arch) = capsule();
    let mut file = seal(&bytes);
    let input = DoctorOfflineInput::acquire(&file, bytes.len()).unwrap();
    let base = input.bytes().as_ptr() as usize;
    let parsed = DoctorOfflineBundle::parse(input, "handoff-v1").unwrap();
    assert_eq!(file.stream_position().unwrap(), 7);
    assert!(unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD, 0 as libc::c_long) } >= 0);
    drop(file);
    assert_eq!(parsed.selector(), "handoff-v1");
    assert_eq!(parsed.architecture(), arch);
    assert_eq!(parsed.files().len(), 1);
    let node = parsed.tool(DoctorOfflineTool::Node).unwrap();
    assert_eq!(node.path(), "bin/node");
    assert_eq!(node.bytes(), elf);
    assert!(node.is_executable());
    assert_eq!(node.path().as_ptr() as usize, base + 50);
    assert_eq!(node.bytes().as_ptr() as usize, base + 58);
    assert!(parsed.tool(DoctorOfflineTool::Clang).is_none());
    assert!(parsed.tool(DoctorOfflineTool::Rustc).is_none());
    assert_eq!(parsed.files().next().unwrap().bytes(), node.bytes());
}

#[test]
fn sealed_bytes_do_not_override_selector_or_compiled_architecture_binding() {
    let (mut bytes, _, _) = capsule();
    let file = seal(&bytes);
    let input = DoctorOfflineInput::acquire(&file, bytes.len()).unwrap();
    assert_eq!(
        DoctorOfflineBundle::parse(input, "other-v1").unwrap_err(),
        DoctorOfflineBundleError::SelectorMismatch
    );
    bytes[8] = if bytes[8] == 1 { 2 } else { 1 };
    let file = seal(&bytes);
    let input = DoctorOfflineInput::acquire(&file, bytes.len()).unwrap();
    assert_eq!(
        DoctorOfflineBundle::parse(input, "handoff-v1").unwrap_err(),
        DoctorOfflineBundleError::ArchitectureMismatch
    );
}
