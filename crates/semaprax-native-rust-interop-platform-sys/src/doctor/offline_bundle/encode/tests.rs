//! Literal format/structural evidence, never sealing or execution evidence.
use super::*;

fn elf(architecture: DoctorOfflineArchitecture, interpreter: Option<&str>) -> Vec<u8> {
    let mut bytes = vec![0; 120];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
    let machine = match architecture {
        DoctorOfflineArchitecture::LinuxX86_64 => 62u16,
        DoctorOfflineArchitecture::LinuxAarch64 => 183u16,
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
    } else {
        bytes[64..68].copy_from_slice(&1u32.to_le_bytes());
    }
    bytes
}
fn entry<'a>(path: &'a str, bytes: &'a [u8], executable: bool) -> DoctorOfflineBundleEntry<'a> {
    DoctorOfflineBundleEntry {
        path,
        bytes,
        executable,
    }
}
fn clang(index: usize) -> DoctorOfflineBundleRoles {
    DoctorOfflineBundleRoles {
        clang: Some(index),
        node: None,
        rustc: None,
    }
}
fn encode(
    entries: &[DoctorOfflineBundleEntry<'_>],
    roles: DoctorOfflineBundleRoles,
) -> Result<Vec<u8>, Error> {
    encode_doctor_offline_bundle(
        DoctorOfflineArchitecture::LinuxX86_64,
        "p",
        entries,
        roles,
        DOCTOR_OFFLINE_INPUT_MAX_BYTES,
    )
}

#[test]
fn literal_header_records_and_both_architectures_match_exact_bytes() {
    for (architecture, tag) in [
        (DoctorOfflineArchitecture::LinuxX86_64, 1),
        (DoctorOfflineArchitecture::LinuxAarch64, 2),
    ] {
        let image = elf(architecture, None);
        let records = [
            entry("bin/clang", &image, true),
            entry("data/empty", b"", false),
        ];
        let mut expected = b"SPXDOC1\0".to_vec();
        expected.extend_from_slice(&[tag, 1, 1, 0, 2, 0, 0, 0]);
        expected.extend_from_slice(&[0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255]);
        expected.push(b'p');
        expected.extend_from_slice(&[9, 0, 1, 0, 120, 0, 0, 0, 0, 0, 0, 0]);
        expected.extend_from_slice(b"bin/clang");
        expected.extend_from_slice(&image);
        expected.extend_from_slice(&[10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        expected.extend_from_slice(b"data/empty");
        let result =
            encode_doctor_offline_bundle(architecture, "p", &records, clang(0), expected.len())
                .unwrap();
        assert_eq!(result, expected);
        assert_eq!(
            encode_doctor_offline_bundle(architecture, "p", &records, clang(0), expected.len())
                .unwrap(),
            result
        );
        assert_eq!(
            encode_doctor_offline_bundle(architecture, "p", &records, clang(0), expected.len() - 1),
            Err(Error::Limit)
        );
    }
}

#[test]
fn all_seven_role_masks_preserve_explicit_indices_and_absence() {
    let image = elf(DoctorOfflineArchitecture::LinuxX86_64, None);
    let entries = [
        entry("bin/clang", &image, true),
        entry("bin/node", &image, true),
        entry("bin/rustc", &image, true),
    ];
    for mask in 1u8..=7 {
        let roles = DoctorOfflineBundleRoles {
            clang: (mask & 1 != 0).then_some(0),
            node: (mask & 2 != 0).then_some(1),
            rustc: (mask & 4 != 0).then_some(2),
        };
        let bytes = encode(&entries, roles).unwrap();
        assert_eq!(bytes[9], mask);
        for index in 0..3 {
            let start = 16 + 4 * index;
            let actual = u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap());
            assert_eq!(
                actual,
                if mask & (1 << index) != 0 {
                    index as u32
                } else {
                    u32::MAX
                }
            );
        }
    }
}

#[test]
fn lowered_limit_is_checked_before_inputs_and_header_scalars_do_not_truncate() {
    let invalid = [entry("", b"", false)];
    assert_eq!(
        encode_doctor_offline_bundle(
            DoctorOfflineArchitecture::LinuxX86_64,
            "INVALID",
            &invalid,
            DoctorOfflineBundleRoles::default(),
            0
        ),
        Err(Error::Invalid)
    );
    assert_eq!(
        encode_doctor_offline_bundle(
            DoctorOfflineArchitecture::LinuxX86_64,
            "INVALID",
            &invalid,
            DoctorOfflineBundleRoles::default(),
            DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1
        ),
        Err(Error::Limit)
    );
    assert_eq!(encode(&[], clang(0)), Err(Error::Invalid));
    assert_eq!(encode(&invalid, clang(usize::MAX)), Err(Error::Invalid));
    assert_eq!(
        encode(&invalid, DoctorOfflineBundleRoles::default()),
        Err(Error::Invalid)
    );
    assert_eq!(encode(&invalid, clang(0)), Err(Error::Invalid));
    let overlong = "a".repeat(1025);
    assert_eq!(
        encode(&[entry(&overlong, b"", false)], clang(0)),
        Err(Error::Limit)
    );
    let image = elf(DoctorOfflineArchitecture::LinuxX86_64, None);
    let valid = [entry("bin/clang", &image, true)];
    for selector in [
        String::new(),
        "a".repeat(65),
        "Bad".to_owned(),
        "a/b".to_owned(),
    ] {
        assert_eq!(
            encode_doctor_offline_bundle(
                DoctorOfflineArchitecture::LinuxX86_64,
                &selector,
                &valid,
                clang(0),
                DOCTOR_OFFLINE_INPUT_MAX_BYTES
            ),
            Err(Error::Invalid)
        );
    }
    let selector = "a".repeat(64);
    let bytes = encode_doctor_offline_bundle(
        DoctorOfflineArchitecture::LinuxX86_64,
        &selector,
        &valid,
        clang(0),
        DOCTOR_OFFLINE_INPUT_MAX_BYTES,
    )
    .unwrap();
    assert_eq!(&bytes[10..12], &[64, 0]);
    assert_eq!(&bytes[28..92], selector.as_bytes());
}

#[test]
fn complete_validator_rejects_bad_paths_order_prefixes_and_role_meaning() {
    let image = elf(DoctorOfflineArchitecture::LinuxX86_64, None);
    for path in [
        "/bin/clang",
        "bin//clang",
        "bin/../clang",
        "bin\\clang",
        "bin/é",
        "bin:clang",
    ] {
        let entries = [entry(path, &image, true)];
        assert!(prepare("p", &entries, clang(0), DOCTOR_OFFLINE_INPUT_MAX_BYTES).is_ok());
        assert_eq!(encode(&entries, clang(0)), Err(Error::Invalid));
    }
    let component = format!("{}/clang", "a".repeat(256));
    assert_eq!(
        encode(&[entry(&component, &image, true)], clang(0)),
        Err(Error::Limit)
    );
    let deep = format!("{}clang", "a/".repeat(32));
    assert_eq!(
        encode(&[entry(&deep, &image, true)], clang(0)),
        Err(Error::Limit)
    );
    let reversed = [
        entry("data/x", b"", false),
        entry("bin/clang", &image, true),
    ];
    assert_eq!(encode(&reversed, clang(1)), Err(Error::Invalid));
    assert_eq!(reversed[0].path, "data/x"); // No sorting/repair of caller inventory.
    let duplicate = [
        entry("bin/clang", &image, true),
        entry("bin/clang", &image, true),
    ];
    assert_eq!(encode(&duplicate, clang(0)), Err(Error::Invalid));
    let nonadjacent = [
        entry("a", b"", false),
        entry("a-", b"", false),
        entry("a/x", b"", false),
        entry("bin/clang", &image, true),
    ];
    assert_eq!(encode(&nonadjacent, clang(3)), Err(Error::Invalid));
    assert_eq!(
        encode(&[entry("bin/clang", &image, false)], clang(0)),
        Err(Error::Invalid)
    );
    assert_eq!(
        encode(&[entry("bin/wrong", &image, true)], clang(0)),
        Err(Error::Invalid)
    );
    assert_eq!(
        encode(
            &[entry("bin/clang", &image, true)],
            DoctorOfflineBundleRoles {
                clang: Some(0),
                node: Some(0),
                rustc: None
            }
        ),
        Err(Error::Invalid)
    );
}

#[test]
fn complete_validator_checks_unselected_executables_and_interpreter_closure() {
    let arch = DoctorOfflineArchitecture::LinuxX86_64;
    let plain = elf(arch, None);
    assert_eq!(
        encode(
            &[
                entry("bin/clang", &plain, true),
                entry("z/unselected", b"bad ELF", true)
            ],
            clang(0)
        ),
        Err(Error::Invalid)
    );
    let wrong_arch = elf(DoctorOfflineArchitecture::LinuxAarch64, None);
    assert_eq!(
        encode(&[entry("bin/clang", &wrong_arch, true)], clang(0)),
        Err(Error::Invalid)
    );
    let dynamic = elf(arch, Some("/lib/ld"));
    assert!(encode(
        &[
            entry("bin/clang", &dynamic, true),
            entry("lib/ld", &plain, true)
        ],
        clang(0)
    )
    .is_ok());
    assert_eq!(
        encode(&[entry("bin/clang", &dynamic, true)], clang(0)),
        Err(Error::Invalid)
    );
    assert_eq!(
        encode(
            &[
                entry("bin/clang", &dynamic, true),
                entry("lib/ld", &plain, false)
            ],
            clang(0)
        ),
        Err(Error::Invalid)
    );
    let chained = elf(arch, Some("/lib/last"));
    assert_eq!(
        encode(
            &[
                entry("bin/clang", &dynamic, true),
                entry("lib/last", &plain, true),
                entry("lib/ld", &chained, true)
            ],
            clang(0)
        ),
        Err(Error::Invalid)
    );
}

#[test]
fn file_count_ceiling_accepts_exactly_4096_and_rejects_next_before_emission() {
    let image = elf(DoctorOfflineArchitecture::LinuxX86_64, None);
    let names = (0..4095)
        .map(|index| format!("data/{index:04}"))
        .collect::<Vec<_>>();
    let mut entries = vec![entry("bin/clang", &image, true)];
    entries.extend(names.iter().map(|path| entry(path, b"", false)));
    let bytes = encode(&entries, clang(0)).unwrap();
    assert_eq!(&bytes[12..16], &4096u32.to_le_bytes());
    entries.push(entry("z/extra", b"", false));
    assert_eq!(encode(&entries, clang(0)), Err(Error::Limit));
}

#[test]
fn length_only_preflight_covers_exact_carrier_path_totals_and_overflow() {
    let mut total = 29;
    let mut paths = 0;
    let content = DOCTOR_OFFLINE_INPUT_MAX_BYTES - 29 - 12 - 9;
    assert_eq!(
        account_record(
            &mut total,
            &mut paths,
            9,
            content,
            DOCTOR_OFFLINE_INPUT_MAX_BYTES
        ),
        Ok(())
    );
    assert_eq!(total, DOCTOR_OFFLINE_INPUT_MAX_BYTES);
    let prior = (total, paths);
    assert_eq!(
        account_record(&mut total, &mut paths, 1, 0, DOCTOR_OFFLINE_INPUT_MAX_BYTES),
        Err(Error::Limit)
    );
    assert_eq!((total, paths), prior);
    total = 0;
    paths = 0;
    for _ in 0..1024 {
        account_record(
            &mut total,
            &mut paths,
            1024,
            0,
            DOCTOR_OFFLINE_INPUT_MAX_BYTES,
        )
        .unwrap();
    }
    assert_eq!(paths, 1_048_576);
    let prior = (total, paths);
    assert_eq!(
        account_record(&mut total, &mut paths, 1, 0, DOCTOR_OFFLINE_INPUT_MAX_BYTES),
        Err(Error::Limit)
    );
    assert_eq!((total, paths), prior);
    total = usize::MAX;
    paths = 0;
    assert_eq!(
        account_record(&mut total, &mut paths, 1, 0, DOCTOR_OFFLINE_INPUT_MAX_BYTES),
        Err(Error::Limit)
    );
    assert_eq!(total, usize::MAX);
    total = 0;
    paths = usize::MAX;
    assert_eq!(
        account_record(&mut total, &mut paths, 1, 0, DOCTOR_OFFLINE_INPUT_MAX_BYTES),
        Err(Error::Limit)
    );
    assert_eq!(paths, usize::MAX);
}
