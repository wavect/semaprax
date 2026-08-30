//! Structural byte fixtures only; these are not runnable tool distributions.
use super::*;

#[path = "tests/interpreters.rs"]
mod interpreters;
#[path = "tests/limits.rs"]
mod limits;

const ID: &str = "fixture-v1";
const NONE: u32 = u32::MAX;
const X64: DoctorOfflineArchitecture = DoctorOfflineArchitecture::LinuxX86_64;
const ARM: DoctorOfflineArchitecture = DoctorOfflineArchitecture::LinuxAarch64;
type Error = DoctorOfflineBundleError;

fn elf(machine: u16, interpreter: Option<&str>) -> Vec<u8> {
    let phnum: u16 = if interpreter.is_some() { 2 } else { 1 };
    let mut bytes = vec![0; 64 + usize::from(phnum) * 56];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&machine.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&phnum.to_le_bytes());
    bytes[64..68].copy_from_slice(&1_u32.to_le_bytes());
    if let Some(path) = interpreter {
        let offset = bytes.len() as u64;
        bytes[120..124].copy_from_slice(&3_u32.to_le_bytes());
        bytes[128..136].copy_from_slice(&offset.to_le_bytes());
        bytes[152..160].copy_from_slice(&(path.len() as u64 + 1).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
    }
    bytes
}

fn encode(id: &str, arch: u8, mask: u8, roles: [u32; 3], files: &[(&str, bool, &[u8])]) -> Vec<u8> {
    let mut bytes = b"SPXDOC1\0".to_vec();
    bytes.extend([arch, mask]);
    bytes.extend_from_slice(&(id.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for index in roles {
        bytes.extend_from_slice(&index.to_le_bytes());
    }
    bytes.extend_from_slice(id.as_bytes());
    for (path, executable, content) in files {
        bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        bytes.extend([u8::from(*executable), 0]);
        bytes.extend_from_slice(&(content.len() as u64).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(content);
    }
    bytes
}

fn node(files: &[(&str, bool, &[u8])], index: u32) -> Vec<u8> {
    encode(ID, 1, 2, [NONE, index, NONE], files)
}

fn parse(bytes: &[u8]) -> Result<wire::Index, Error> {
    wire::parse(bytes, ID, X64)
}

fn rejects(bytes: &[u8], expected: Error) {
    assert_eq!(parse(bytes).err(), Some(expected));
}

#[test]
fn exact_header_and_all_role_ranges_are_preserved_without_copying_payloads() {
    for (tag, arch, machine) in [(1, X64, 62), (2, ARM, 183)] {
        let executable = elf(machine, None);
        let bytes = encode(
            ID,
            tag,
            7,
            [0, 1, 2],
            &[
                ("bin/clang", true, &executable),
                ("bin/node", true, &executable),
                ("bin/rustc", true, &executable),
                ("etc/empty", false, b""),
                ("lib/blob", false, b"\0\xff\x7fELF"),
            ],
        );
        assert_eq!(
            &bytes[..28],
            &[
                83, 80, 88, 68, 79, 67, 49, 0, tag, 7, 10, 0, 5, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
                2, 0, 0, 0,
            ]
        );
        let index = wire::parse(&bytes, ID, arch).unwrap();
        assert_eq!(&bytes[index.selector], ID.as_bytes());
        assert_eq!(index.architecture, arch);
        assert_eq!(index.tools, [Some(0), Some(1), Some(2)]);
        assert_eq!(index.files.len(), 5);
        for (row, path) in index.files.iter().zip([
            "bin/clang",
            "bin/node",
            "bin/rustc",
            "etc/empty",
            "lib/blob",
        ]) {
            assert_eq!(&bytes[row.path.clone()], path.as_bytes());
            assert!(row.path.end <= row.content.start);
        }
        for file in &index.files[..3] {
            assert!(file.executable);
            assert_eq!(&bytes[file.content.clone()], executable);
        }
        assert!(!index.files[4].executable);
        assert_eq!(&bytes[index.files[4].content.clone()], b"\0\xff\x7fELF");
    }
}

#[test]
fn all_truncated_prefixes_trailing_bytes_and_unknown_header_fields_reject() {
    let executable = elf(62, None);
    let valid = node(&[("bin/node", true, &executable)], 0);
    assert!(parse(&valid).is_ok());
    for end in 0..valid.len() {
        assert!(parse(&valid[..end]).is_err(), "prefix {end}");
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    rejects(&trailing, Error::Invalid);
    for offset in 0..8 {
        let mut bad = valid.clone();
        bad[offset] ^= 0x80;
        rejects(&bad, Error::Invalid);
    }
    for (offset, value) in [(8, 0), (8, 3), (9, 0), (9, 8), (9, 255)] {
        let mut bad = valid.clone();
        bad[offset] = value;
        rejects(&bad, Error::Invalid);
    }
    let record = 28 + ID.len();
    for (offset, value) in [(record + 2, 2), (record + 3, 1)] {
        let mut bad = valid.clone();
        bad[offset] = value;
        rejects(&bad, Error::Invalid);
    }
    let mut overflow = valid.clone();
    overflow[record + 4..record + 12].copy_from_slice(&u64::MAX.to_le_bytes());
    rejects(&overflow, Error::Limit);
}

#[test]
fn selector_and_architecture_are_exact_bindings_not_admission_authority() {
    let executable = elf(62, None);
    let valid = node(&[("bin/node", true, &executable)], 0);
    for caller in ["", "A", "../fixture-v1", "a_b", "a\0b", "é"] {
        assert_eq!(wire::parse(&[], caller, X64).err(), Some(Error::Invalid));
        assert_eq!(wire::parse(&valid, caller, X64).err(), Some(Error::Invalid));
    }
    assert_eq!(
        wire::parse(&valid, "other-v1", X64).err(),
        Some(Error::SelectorMismatch)
    );
    assert_eq!(
        wire::parse(&valid, ID, ARM).err(),
        Some(Error::ArchitectureMismatch)
    );
    for id in ["a".to_owned(), "a".repeat(64)] {
        let bytes = encode(
            &id,
            1,
            2,
            [NONE, 0, NONE],
            &[("bin/node", true, &executable)],
        );
        assert!(wire::parse(&bytes, &id, X64).is_ok());
    }
    let oversized = "a".repeat(65);
    assert_eq!(
        wire::parse(&valid, &oversized, X64).err(),
        Some(Error::Invalid)
    );
    let invalid = encode(
        "bad_id",
        1,
        2,
        [NONE, 0, NONE],
        &[("bin/node", true, &executable)],
    );
    rejects(&invalid, Error::Invalid);
}

#[test]
fn role_masks_indices_names_and_executable_flags_are_not_inferred() {
    let executable = elf(62, None);
    for (mask, roles) in [
        (2, [0, 0, NONE]),
        (2, [NONE, NONE, NONE]),
        (2, [NONE, 1, NONE]),
        (7, [0, 0, 0]),
        (1, [0, NONE, NONE]),
    ] {
        rejects(
            &encode(ID, 1, mask, roles, &[("bin/node", true, &executable)]),
            Error::Invalid,
        );
    }
    for path in ["bin/Node", "bin/node.exe", "bin/node-22", "node-other"] {
        rejects(&node(&[(path, true, &executable)], 0), Error::Invalid);
    }
    rejects(
        &node(&[("bin/node", false, &executable)], 0),
        Error::Invalid,
    );
    rejects(
        &node(&[("bin/node", true, b"#!/bin/sh\n")], 0),
        Error::Invalid,
    );
    rejects(
        &node(
            &[("bin/node", true, &executable), ("other", true, b"bad")],
            0,
        ),
        Error::Invalid,
    );
}

#[test]
fn grammar_order_duplicates_and_nonadjacent_file_directory_collisions_reject() {
    let executable = elf(62, None);
    for path in [
        "", "/a", "a/", "a//b", ".", "..", "a/./b", "a/../b", "a\\b", "a:b", "a b", "a\nb", "a\0b",
        "é",
    ] {
        let mut files = vec![
            (path, false, b"".as_slice()),
            ("bin/node", true, executable.as_slice()),
        ];
        files.sort_by_key(|file| file.0);
        let role = files.iter().position(|file| file.0 == "bin/node").unwrap() as u32;
        rejects(&node(&files, role), Error::Invalid);
    }
    for files in [
        vec![
            ("bin/node", true, executable.as_slice()),
            ("a", false, b"".as_slice()),
        ],
        vec![
            ("bin/node", true, executable.as_slice()),
            ("bin/node", true, executable.as_slice()),
        ],
        vec![
            ("a", false, b"".as_slice()),
            ("a-", false, b"".as_slice()),
            ("a/x", false, b"".as_slice()),
            ("bin/node", true, executable.as_slice()),
        ],
    ] {
        let role = files.iter().position(|file| file.0 == "bin/node").unwrap() as u32;
        rejects(&node(&files, role), Error::Invalid);
    }
    let files = [
        ("a-", false, b"".as_slice()),
        ("a/x", false, b"".as_slice()),
        ("bin/node", true, executable.as_slice()),
    ];
    assert!(parse(&node(&files, 2)).is_ok());
}

#[test]
fn public_surface_requires_owned_sealed_input_and_exposes_read_only_views() {
    let _: fn(DoctorOfflineInput, &str) -> Result<DoctorOfflineBundle, Error> =
        DoctorOfflineBundle::parse;
    let _: fn(&DoctorOfflineBundle) -> &str = DoctorOfflineBundle::selector;
    let _: fn(&DoctorOfflineBundle) -> DoctorOfflineArchitecture =
        DoctorOfflineBundle::architecture;
}
