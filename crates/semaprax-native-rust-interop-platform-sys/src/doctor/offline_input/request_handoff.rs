//! Real sealed snapshots into the public request author. No tools execute here.
use super::DoctorOfflineInput;
use crate::doctor::offline_worker::wire;
use crate::{DoctorOfflineBundle, DoctorOfflineBundleError, DoctorOfflineTarget};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};

fn architecture() -> u8 {
    if cfg!(target_arch = "x86_64") {
        1
    } else {
        2
    }
}

// Independent literal bundle producer. Do not use a production encoder for the
// request oracle: producer/consumer agreement alone could hide framing drift.
fn capsule(selector: &str, directory: &str, roles: u8, payload: &[u8]) -> Vec<u8> {
    let mut elf = vec![0; 120];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2u16.to_le_bytes());
    elf[18..20].copy_from_slice(&(if architecture() == 1 { 62u16 } else { 183u16 }).to_le_bytes());
    elf[20..24].copy_from_slice(&1u32.to_le_bytes());
    elf[32..40].copy_from_slice(&64u64.to_le_bytes());
    elf[52..54].copy_from_slice(&64u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1u16.to_le_bytes());
    elf[64..68].copy_from_slice(&1u32.to_le_bytes());
    elf.extend_from_slice(payload);
    let mut bytes = b"SPXDOC1\0".to_vec();
    bytes.extend_from_slice(&[architecture(), roles]);
    bytes.extend_from_slice(&(selector.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&3u32.to_le_bytes());
    for ordinal in 0..3u32 {
        let index = if roles & (1 << ordinal) != 0 {
            ordinal
        } else {
            u32::MAX
        };
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    bytes.extend_from_slice(selector.as_bytes());
    for name in ["clang", "node", "rustc"] {
        let path = format!("{directory}/{name}");
        bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&[1, 0]);
        bytes.extend_from_slice(&(elf.len() as u64).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(&elf);
    }
    bytes
}

fn seal(bytes: &[u8]) -> File {
    let fd = unsafe {
        libc::memfd_create(
            c"doctor-request-handoff".as_ptr(),
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

fn parse(bytes: &[u8], selector: &str) -> Result<DoctorOfflineBundle, DoctorOfflineBundleError> {
    let file = seal(bytes);
    let input = DoctorOfflineInput::acquire(&file, bytes.len()).unwrap();
    DoctorOfflineBundle::parse(input, selector)
}

fn literal_reply(request: &[u8]) -> Vec<u8> {
    let mut bytes = b"SPXDWR1\0".to_vec();
    bytes.extend_from_slice(&Sha256::digest(request));
    bytes.extend_from_slice(&request[12..44]);
    bytes.extend_from_slice(&request[8..12]);
    bytes.extend_from_slice(&[1, 1, 0]); // Exactly one successful Clang row.
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.push(b'x');
    bytes
}

#[test]
fn canonical_request_matches_independent_literal_layout_and_retained_bundle() {
    let bytes = capsule("a", "bin", 7, b"");
    assert_eq!(bytes.len(), 451);
    let mut file = seal(&bytes);
    let seals = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) };
    assert_eq!(seals & 15, 15);
    let input = DoctorOfflineInput::acquire(&file, bytes.len()).unwrap();
    let bundle = DoctorOfflineBundle::parse(input, "a").unwrap();
    let request = bundle
        .encode_worker_request(DoctorOfflineTarget::All, [0x11; 32])
        .unwrap();
    let mut expected = vec![
        0x53,
        0x50,
        0x58,
        0x44,
        0x57,
        0x4b,
        0x31,
        0,
        1,
        architecture(),
        3,
        7,
    ];
    expected.extend_from_slice(&[0x11; 32]);
    expected.extend_from_slice(&[0xc3, 1, 0, 0, 0, 0, 0, 0]); // Literal 451, little endian.
    expected.extend_from_slice(&Sha256::digest(&bytes));
    expected.extend_from_slice(&[1, b'a']);
    assert_eq!(request, expected);
    assert_eq!(request.len(), 86);
    assert_eq!(file.stream_position().unwrap(), 7);
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) },
        seals
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&file, bytes.len())
            .unwrap()
            .bytes(),
        bytes
    );
    drop(file);
    assert_eq!(
        bundle
            .encode_worker_request(DoctorOfflineTarget::All, [0x11; 32])
            .unwrap(),
        request
    );
    let parsed = wire::Request::parse(&request).unwrap();
    assert_eq!(parsed.roles().count(), 3);
    assert_eq!(parsed.selector, "a");
    assert_eq!(parsed.bundle_len, bytes.len());
}

#[test]
fn every_target_requires_its_exact_roles_without_downgrading_or_selecting_extra_tools() {
    for available in 1..=7 {
        let bytes = capsule("a", "bin", available, b"");
        let bundle = parse(&bytes, "a").unwrap();
        for (target, code, required) in [
            (DoctorOfflineTarget::Contributor, 0, 4),
            (DoctorOfflineTarget::Native, 1, 1),
            (DoctorOfflineTarget::Web, 2, 2),
            (DoctorOfflineTarget::All, 3, 7),
        ] {
            let result = bundle.encode_worker_request(target, [1; 32]);
            if available & required != required {
                assert_eq!(result, Err(DoctorOfflineBundleError::Invalid));
            } else {
                let request = result.unwrap();
                assert_eq!(&request[8..12], &[1, architecture(), code, required]);
                assert_eq!(
                    wire::Request::parse(&request).unwrap().roles().count(),
                    required.count_ones() as usize
                );
            }
        }
    }
}

#[test]
fn zero_nonce_rejects_and_every_nonzero_position_and_selector_boundary_is_preserved() {
    for selector in ["a".to_owned(), "a".repeat(64)] {
        let bundle = parse(&capsule(&selector, "bin", 7, b""), &selector).unwrap();
        assert_eq!(
            bundle.encode_worker_request(DoctorOfflineTarget::All, [0; 32]),
            Err(DoctorOfflineBundleError::Invalid)
        );
        for index in 0..32 {
            let mut nonce = [0; 32];
            nonce[index] = 1;
            let request = bundle
                .encode_worker_request(DoctorOfflineTarget::All, nonce)
                .unwrap();
            assert_eq!(&request[12..44], &nonce);
            assert_eq!(request.len(), 85 + selector.len());
            assert_eq!(request[84] as usize, selector.len());
            assert_eq!(&request[85..], selector.as_bytes());
            assert!(wire::Request::parse(&request).is_ok());
        }
    }
}

#[test]
fn payload_path_selector_and_nonce_changes_reject_cross_bound_reply_replay() {
    let original = parse(&capsule("a", "bin", 7, b"x"), "a")
        .unwrap()
        .encode_worker_request(DoctorOfflineTarget::Native, [1; 32])
        .unwrap();
    let reply = literal_reply(&original);
    assert!(wire::validate_reply(&wire::Request::parse(&original).unwrap(), &reply).is_ok());
    for (selector, directory, payload, nonce) in [
        ("a", "bin", b"y", [1; 32]),
        ("a", "lib", b"x", [1; 32]),
        ("b", "bin", b"x", [1; 32]),
        ("a", "bin", b"x", [2; 32]),
    ] {
        let changed = parse(&capsule(selector, directory, 7, payload), selector)
            .unwrap()
            .encode_worker_request(DoctorOfflineTarget::Native, nonce)
            .unwrap();
        assert_eq!(changed.len(), original.len());
        assert_ne!(changed, original);
        assert!(wire::validate_reply(&wire::Request::parse(&changed).unwrap(), &reply).is_err());
    }
}

#[test]
fn architecture_mismatch_and_shared_role_entry_still_reject_before_request_authoring() {
    let mut bytes = capsule("a", "bin", 7, b"");
    bytes[8] = if architecture() == 1 { 2 } else { 1 };
    assert_eq!(
        parse(&bytes, "a").unwrap_err(),
        DoctorOfflineBundleError::ArchitectureMismatch
    );
    let mut bytes = capsule("a", "bin", 7, b"");
    bytes[20..24].copy_from_slice(&0u32.to_le_bytes()); // Node aliases Clang, invalid basename.
    assert_eq!(
        parse(&bytes, "a").unwrap_err(),
        DoctorOfflineBundleError::Invalid
    );
    // Identical file bytes under distinct role-specific basenames remain valid.
    let bundle = parse(&capsule("a", "bin", 7, b"same"), "a").unwrap();
    assert!(bundle
        .encode_worker_request(DoctorOfflineTarget::All, [1; 32])
        .is_ok());
}
