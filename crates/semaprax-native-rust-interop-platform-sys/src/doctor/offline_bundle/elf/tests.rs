//! Independent literal metadata fixtures, deliberately not executable binaries.
use super::*;

// ELF64 little-endian ET_EXEC/x86-64, one 56-byte program header at byte 64.
const HEADER: [u8; 64] = [
    127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 62, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 56, 0, 1, 0, 0, 0, 0,
    0, 0, 0,
];

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn plain() -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    bytes.extend_from_slice(&[0; 56]); // PT_NULL: no claim that the kernel can load it.
    bytes
}

fn with_interpreters(paths: &[&[u8]]) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    put16(&mut bytes, 56, u16::try_from(paths.len()).unwrap());
    bytes.resize(64 + paths.len() * 56, 0);
    for (index, path) in paths.iter().enumerate() {
        let row = 64 + index * 56;
        bytes[row..row + 4].copy_from_slice(&[3, 0, 0, 0]); // PT_INTERP.
        let offset = bytes.len();
        put64(&mut bytes, row + 8, offset as u64);
        put64(&mut bytes, row + 32, path.len() as u64);
        bytes.extend_from_slice(path);
    }
    bytes
}

fn invalid(bytes: &[u8]) {
    assert_eq!(
        validate(bytes, DoctorOfflineArchitecture::LinuxX86_64),
        Err(Error::Invalid)
    );
}

#[test]
fn both_architectures_and_executable_types_return_borrowed_metadata() {
    for (architecture, machine) in [
        (DoctorOfflineArchitecture::LinuxX86_64, 62),
        (DoctorOfflineArchitecture::LinuxAarch64, 183),
    ] {
        for kind in [2, 3] {
            let mut bytes = plain();
            put16(&mut bytes, 16, kind);
            put16(&mut bytes, 18, machine);
            assert_eq!(validate(&bytes, architecture), Ok(None));
            let mut bytes = with_interpreters(&[b"/lib/loader.so\0"]);
            put16(&mut bytes, 16, kind);
            put16(&mut bytes, 18, machine);
            let path = validate(&bytes, architecture).unwrap().unwrap();
            assert_eq!(path, "/lib/loader.so");
            assert_eq!(path.as_ptr(), bytes[120..].as_ptr());
        }
    }
}

#[test]
fn every_required_header_field_rejects_wrong_values() {
    for offset in 0..7 {
        let mut bytes = plain();
        bytes[offset] = 0;
        invalid(&bytes);
    }
    for (offset, values) in [
        (16, &[0, 1, 4, u16::MAX][..]),
        (18, &[0, 3, 183, u16::MAX][..]),
        (52, &[0, 63, 65, u16::MAX][..]),
        (54, &[0, 55, 57, u16::MAX][..]),
        (56, &[0, 129, u16::MAX][..]),
    ] {
        for value in values {
            let mut bytes = plain();
            put16(&mut bytes, offset, *value);
            invalid(&bytes);
        }
    }
    for version in [[0, 0, 0, 0], [2, 0, 0, 0], [1, 0, 0, 1]] {
        let mut bytes = plain();
        bytes[20..24].copy_from_slice(&version);
        invalid(&bytes);
    }
    assert_eq!(
        validate(&plain(), DoctorOfflineArchitecture::LinuxAarch64),
        Err(Error::Invalid)
    );
}

#[test]
fn complete_header_and_table_ranges_are_required_without_overflow() {
    let bytes = plain();
    for length in 0..bytes.len() {
        invalid(&bytes[..length]);
    }
    for offset in [65, 120, u64::MAX - 55, u64::MAX] {
        let mut bytes = plain();
        put64(&mut bytes, 32, offset);
        invalid(&bytes);
    }
    let mut maximum = HEADER.to_vec();
    put16(&mut maximum, 56, 128);
    maximum.resize(64 + 128 * 56, 0);
    assert_eq!(
        validate(&maximum, DoctorOfflineArchitecture::LinuxX86_64),
        Ok(None)
    );
    invalid(&maximum[..maximum.len() - 1]);
    put16(&mut maximum, 56, 129);
    maximum.resize(64 + 129 * 56, 0);
    invalid(&maximum);
}

#[test]
fn interpreter_count_termination_encoding_and_absolute_path_are_checked() {
    for paths in [
        vec![&b"/a\0"[..], &b"/a\0"[..]],
        vec![&b"/a\0"[..], &b"/b\0"[..]],
    ] {
        invalid(&with_interpreters(&paths));
    }
    for path in [
        &b""[..],
        &b"/\0"[..],
        &b"/ab"[..],
        &b"ab\0"[..],
        &b"/a\0b\0"[..],
        &b"/\xff\0"[..],
    ] {
        invalid(&with_interpreters(&[path]));
    }
    // Canonical ASCII/path grammar belongs to the enclosing inventory parser.
    // The ELF helper itself only requires valid UTF-8, an absolute path and NUL framing.
    for path in ["/λ", "/../loader", "/a\nb"] {
        let mut terminated = path.as_bytes().to_vec();
        terminated.push(0);
        let bytes = with_interpreters(&[&terminated]);
        assert_eq!(
            validate(&bytes, DoctorOfflineArchitecture::LinuxX86_64),
            Ok(Some(path))
        );
    }
}

#[test]
fn interpreter_payload_boundaries_and_checked_offsets() {
    for length in [3, 1026] {
        let mut path = vec![b'a'; length];
        path[0] = b'/';
        path[length - 1] = 0;
        let bytes = with_interpreters(&[&path]);
        assert_eq!(
            validate(&bytes, DoctorOfflineArchitecture::LinuxX86_64)
                .unwrap()
                .unwrap()
                .as_bytes(),
            &path[..length - 1]
        );
        invalid(&bytes[..bytes.len() - 1]);
    }
    let mut oversized = vec![b'a'; 1027];
    oversized[0] = b'/';
    oversized[1026] = 0;
    invalid(&with_interpreters(&[&oversized]));
    for size in [0, 1, 2, 1027, u64::MAX] {
        let mut bytes = with_interpreters(&[b"/a\0"]);
        put64(&mut bytes, 64 + 32, size);
        invalid(&bytes);
    }
    for offset in [121, u64::MAX - 1, u64::MAX] {
        let mut bytes = with_interpreters(&[b"/a\0"]);
        put64(&mut bytes, 64 + 8, offset);
        invalid(&bytes);
    }
}

#[test]
fn deliberately_uninspected_fields_do_not_imply_full_elf_validation() {
    let mut bytes = plain();
    bytes[7..16].fill(255); // OSABI/ABI version and reserved identification bytes.
    bytes[24..32].fill(255); // Entry point.
    bytes[40..52].fill(255); // Section table offset and processor flags.
    bytes[58..64].fill(255); // Section table sizes/count/index.
    bytes[64..68].copy_from_slice(&[1, 0, 0, 0]); // PT_LOAD with impossible bounds.
    bytes[72..120].fill(255);
    assert_eq!(
        validate(&bytes, DoctorOfflineArchitecture::LinuxX86_64),
        Ok(None)
    );
    // No newly invented minimum e_phoff: only the complete declared range matters.
    let mut overlapping = plain();
    put64(&mut overlapping, 32, 0);
    assert_eq!(
        validate(&overlapping, DoctorOfflineArchitecture::LinuxX86_64),
        Ok(None)
    );
}
