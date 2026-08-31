//! Structural/storage admission only, never a physical no-child-created claim.
use super::*;
use crate::{
    create_doctor_offline_input, encode_doctor_offline_bundle, DoctorOfflineBundleEntry,
    DoctorOfflineBundleRoles, DoctorOfflineTarget,
};
use std::io::Write;

fn image(interpreter: Option<&str>) -> Vec<u8> {
    let mut bytes = vec![0; 120];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    let machine = if cfg!(target_arch = "x86_64") {
        62u16
    } else {
        183u16
    };
    bytes[18..20].copy_from_slice(&machine.to_le_bytes());
    bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
    bytes[32..40].copy_from_slice(&64u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&1u16.to_le_bytes());
    if let Some(path) = interpreter {
        bytes[64..68].copy_from_slice(&3u32.to_le_bytes());
        bytes[72..80].copy_from_slice(&120u64.to_le_bytes());
        bytes[96..104].copy_from_slice(&((path.len() + 1) as u64).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn executable(bytes: &[u8], exec_seal: bool, setid: bool) -> File {
    let fd = unsafe {
        libc::memfd_create(
            c"launcher-admission-fixture".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | libc::MFD_EXEC,
        )
    };
    assert!(fd >= 0, "strict executable memfd support is required");
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).unwrap();
    assert_eq!(
        unsafe { libc::fchmod(fd, if setid { 0o4700 } else { 0o700 }) },
        0
    );
    let seals = libc::F_SEAL_SEAL
        | libc::F_SEAL_WRITE
        | libc::F_SEAL_GROW
        | libc::F_SEAL_SHRINK
        | if exec_seal { libc::F_SEAL_EXEC } else { 0 };
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) }, 0);
    file
}

fn inputs() -> (File, File, Vec<u8>) {
    let tool = image(None);
    let bytes = encode_doctor_offline_bundle(
        native(),
        "launcher-v1",
        &[DoctorOfflineBundleEntry {
            path: "bin/clang",
            bytes: &tool,
            executable: true,
        }],
        DoctorOfflineBundleRoles {
            clang: Some(0),
            node: None,
            rustc: None,
        },
        DOCTOR_OFFLINE_INPUT_MAX_BYTES,
    )
    .unwrap();
    let (bundle_file, input) = create_doctor_offline_input(&bytes, bytes.len()).unwrap();
    let bundle = DoctorOfflineBundle::parse(input, "launcher-v1").unwrap();
    let request = bundle
        .encode_worker_request(DoctorOfflineTarget::Native, [9; 32])
        .unwrap();
    let (request_file, _) = create_doctor_offline_input(&request, request.len()).unwrap();
    (request_file, bundle_file, request)
}

#[test]
fn both_images_require_sealed_executable_native_elf_and_preserve_inputs() {
    let (request, bundle, request_bytes) = inputs();
    let bytes = image(None);
    let worker = executable(&bytes, true, false);
    let collector = executable(&bytes, true, false);
    assert_eq!(
        validate_files(&request, &bundle, &worker, &collector),
        Ok(())
    );
    let mut wrong_arch = bytes.clone();
    wrong_arch[18..20].copy_from_slice(
        &if cfg!(target_arch = "x86_64") {
            183u16
        } else {
            62u16
        }
        .to_le_bytes(),
    );
    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 1;
    let bad = [
        executable(&wrong_arch, true, false),
        executable(&wrong_magic, true, false),
        executable(b"#!/bin/sh\nexit 0\n", true, false),
        executable(&bytes[..63], true, false),
        executable(&bytes, false, false),
        executable(&bytes, true, true),
        create_doctor_offline_input(&bytes, bytes.len()).unwrap().0,
    ];
    for file in &bad {
        assert!(validate_files(&request, &bundle, file, &collector).is_err());
        assert!(validate_files(&request, &bundle, &worker, file).is_err());
    }
    assert_eq!(
        validate_files(&request, &bundle, &worker, &collector),
        Ok(())
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&request, 149).unwrap().bytes(),
        request_bytes
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&worker, bytes.len())
            .unwrap()
            .bytes(),
        bytes
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&collector, bytes.len())
            .unwrap()
            .bytes(),
        bytes
    );
}

#[test]
fn exact_request_binding_and_required_roles_reject_before_launch() {
    let (request_file, bundle, request) = inputs();
    let bytes = image(None);
    let worker = executable(&bytes, true, false);
    let collector = executable(&bytes, true, false);
    for index in [9, 44, 52, 85] {
        let mut wrong = request.clone();
        wrong[index] ^= 1;
        let (file, _) = create_doctor_offline_input(&wrong, wrong.len()).unwrap();
        assert!(validate_files(&file, &bundle, &worker, &collector).is_err());
    }
    let mut all = request.clone();
    all[10] = 3;
    all[11] = 7;
    let (file, _) = create_doctor_offline_input(&all, all.len()).unwrap();
    assert!(validate_files(&file, &bundle, &worker, &collector).is_err());
    assert_eq!(
        validate_files(&request_file, &bundle, &worker, &collector),
        Ok(())
    );
}

#[test]
fn structural_interpreter_acceptance_is_not_loadability_or_loader_admission() {
    let bytes = image(Some("/semaprax-launcher-fixture-missing-loader"));
    let file = executable(&bytes, true, false);
    // Deliberately no filesystem lookup: the structural validator cannot prove
    // whether this interpreter exists or is approved. The physical negative
    // fixture separately observes a missing loader in its provisioned context.
    assert_eq!(validate_image(&file), Ok(()));
}
