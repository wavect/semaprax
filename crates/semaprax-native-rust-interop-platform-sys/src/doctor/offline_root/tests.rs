//! Authored preparation fixtures; literal ELF bytes are not executable tools.
use super::{Error, Plan};
use crate::{DoctorOfflineBundle, DoctorOfflineInput, DoctorOfflineTool};
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};

/// Add one canonical role entry to independent extra rows, then enter through
/// the real sealed-input acquisition and sole production inventory parser.
pub(super) fn bundle(rows: &[(&str, bool, &[u8])]) -> DoctorOfflineBundle {
    let (tag, machine) = if cfg!(target_arch = "x86_64") {
        (1, 62_u16)
    } else {
        (2, 183_u16)
    };
    let mut elf = [0_u8; 120];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2_u16.to_le_bytes());
    elf[18..20].copy_from_slice(&machine.to_le_bytes());
    elf[20..24].copy_from_slice(&1_u32.to_le_bytes());
    elf[32..40].copy_from_slice(&64_u64.to_le_bytes());
    elf[52..54].copy_from_slice(&64_u16.to_le_bytes());
    elf[54..56].copy_from_slice(&56_u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1_u16.to_le_bytes());
    elf[64..68].copy_from_slice(&1_u32.to_le_bytes());
    let mut files = rows.to_vec();
    files.push(("bin/node", true, &elf));
    files.sort_unstable_by_key(|row| row.0);
    let node = files.iter().position(|row| row.0 == "bin/node").unwrap();
    let mut bytes = b"SPXDOC1\0".to_vec();
    bytes.extend([tag, 2, 7, 0]);
    bytes.extend_from_slice(&(files.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&(node as u32).to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(b"root-v1");
    for (path, executable, payload) in files {
        bytes.extend_from_slice(&(path.len() as u16).to_le_bytes());
        bytes.extend([u8::from(executable), 0]);
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(payload);
    }
    let fd = unsafe {
        libc::memfd_create(
            c"doctor-root-fixture".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    assert!(fd >= 0, "{}", std::io::Error::last_os_error());
    let mut input = unsafe { File::from_raw_fd(fd) };
    input.write_all(&bytes).unwrap();
    assert_eq!(
        unsafe { libc::fcntl(input.as_raw_fd(), libc::F_ADD_SEALS, 15) },
        0
    );
    let snapshot = DoctorOfflineInput::acquire(&input, bytes.len()).unwrap();
    DoctorOfflineBundle::parse(snapshot, "root-v1").unwrap()
}

#[test]
fn plan_has_exact_parent_first_inventory_and_borrows_retained_payloads() {
    let bundle = bundle(&[
        ("a/x", false, b""),
        ("a-/y", false, b"x"),
        ("a/b/z", false, b"yz"),
    ]);
    let plan = Plan::prepare(&bundle, 4096).unwrap();
    assert_eq!(
        plan.directories()
            .iter()
            .map(|path| path.to_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "a-", "a/b", "bin"]
    );
    assert_eq!(
        plan.files()
            .iter()
            .map(|file| file.path.to_str().unwrap())
            .collect::<Vec<_>>(),
        ["a-/y", "a/b/z", "a/x", "bin/node"]
    );
    assert_eq!(plan.size_value().to_bytes(), b"12288");
    assert_eq!(plan.inode_value().to_bytes(), b"9");
    assert_eq!(plan.page_size(), 4096);
    for (prepared, original) in plan.files().iter().zip(bundle.files()) {
        assert_eq!(prepared.path.to_bytes(), original.path().as_bytes());
        assert_eq!(prepared.bytes, original.bytes());
        assert_eq!(prepared.bytes.as_ptr(), original.bytes().as_ptr());
        assert_eq!(prepared.executable, original.is_executable());
    }
    let node = bundle.tool(DoctorOfflineTool::Node).unwrap();
    assert_eq!(
        plan.files().last().unwrap().bytes.as_ptr(),
        node.bytes().as_ptr()
    );
}

#[test]
fn page_caps_round_each_file_independently_and_charge_empty_files_only_as_inodes() {
    for page in [4096, 8192, 16_384, 32_768, 65_536] {
        let exact = vec![0x5a; page];
        let extra = vec![0xa5; page + 1];
        let bundle = bundle(&[
            ("data/empty", false, b""),
            ("data/exact", false, &exact),
            ("data/extra", false, &extra),
            ("data/one", false, b"x"),
        ]);
        let plan = Plan::prepare(&bundle, page).unwrap();
        // node=1 page, empty=0, exact=1, extra=2, one=1.
        assert_eq!(
            plan.size_value().to_bytes(),
            (5 * page).to_string().as_bytes()
        );
        assert_eq!(plan.inode_value().to_bytes(), b"8");
    }
    let bundle = bundle(&[]);
    for invalid in [0, 1, 4095, 4097, 65_535, 65_537, usize::MAX] {
        assert!(matches!(
            Plan::prepare(&bundle, invalid),
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn many_tiny_files_charge_all_pages_not_a_single_payload_rounded_sum() {
    let paths: Vec<_> = (0..4095).map(|index| format!("data/f{index:04}")).collect();
    let rows: Vec<_> = paths
        .iter()
        .map(|path| (path.as_str(), false, b"x".as_slice()))
        .collect();
    let bundle = bundle(&rows);
    let plan = Plan::prepare(&bundle, 4096).unwrap();
    assert_eq!(plan.files().len(), 4096);
    assert_eq!(plan.directories().len(), 2);
    assert_eq!(plan.size_value().to_bytes(), b"16777216");
    assert_eq!(plan.inode_value().to_bytes(), b"4099");
}

#[test]
fn shared_deep_prefixes_are_deduplicated_but_distinct_prefixes_remain() {
    let parent = std::iter::repeat_n("a", 31).collect::<Vec<_>>().join("/");
    let first = format!("{parent}/first");
    let second = format!("{parent}/second");
    let bundle = bundle(&[
        (first.as_str(), false, b"1"),
        (second.as_str(), false, b"2"),
    ]);
    let plan = Plan::prepare(&bundle, 4096).unwrap();
    assert_eq!(plan.directories().len(), 32); // 31 shared levels and bin.
    assert_eq!(plan.inode_value().to_bytes(), b"36");
    for pair in plan.directories().windows(2) {
        assert!(pair[0].as_bytes() < pair[1].as_bytes());
    }
    assert_eq!(plan.directories()[30].to_str().unwrap(), parent);
}
