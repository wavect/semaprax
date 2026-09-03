//! Prepared inventory record parsing, bounded failure paths, and the
//! rebound-close fail-stop driver.

#[cfg(unix)]
use super::*;

#[cfg(unix)]
#[test]
fn prepared_inventory_record_parser_rejects_malformed_and_stale_bytes() {
    use super::platform::test_parse_inventory_records;

    let valid = inventory_record(b"a", 7);
    assert_eq!(
        test_parse_inventory_records(&valid, &[(b"a".as_slice(), 7)]),
        Ok(())
    );

    let header = if cfg!(target_os = "macos") { 21 } else { 19 };
    assert!(test_parse_inventory_records(&vec![0_u8; header - 1], &[]).is_err());
    for record_length in [0_u16, 8, 21, u16::try_from(valid.len() + 8).unwrap()] {
        let mut malformed = valid.clone();
        malformed[16..18].copy_from_slice(&record_length.to_ne_bytes());
        assert!(test_parse_inventory_records(&malformed, &[(b"a".as_slice(), 7)]).is_err());
    }

    let mut missing_nul = valid.clone();
    let terminator = header + 1;
    missing_nul[terminator..].fill(0xff);
    assert!(test_parse_inventory_records(&missing_nul, &[(b"a".as_slice(), 7)]).is_err());

    let early_nul = inventory_record(b"a\0late", 7);
    #[cfg(target_os = "linux")]
    assert_eq!(
        test_parse_inventory_records(&early_nul, &[(b"a".as_slice(), 7)]),
        Ok(())
    );
    #[cfg(target_os = "macos")]
    assert!(test_parse_inventory_records(&early_nul, &[(b"a".as_slice(), 7)]).is_err());

    let mut nonzero_padding = valid.clone();
    nonzero_padding[terminator + 1..].fill(0xa5);
    assert_eq!(
        test_parse_inventory_records(&nonzero_padding, &[(b"a".as_slice(), 7)]),
        Ok(())
    );

    let mut poisoned_tail = valid.clone();
    poisoned_tail.extend_from_slice(&[0xff; 3]);
    assert!(test_parse_inventory_records(&poisoned_tail, &[(b"a".as_slice(), 7)]).is_err());

    let mut duplicate = valid.clone();
    duplicate.extend_from_slice(&valid);
    assert!(test_parse_inventory_records(&duplicate, &[(b"a".as_slice(), 7)]).is_err());
    assert!(test_parse_inventory_records(
        &inventory_record(b"unknown", 7),
        &[(b"a".as_slice(), 7)]
    )
    .is_err());
    #[cfg(target_os = "linux")]
    assert!(
        test_parse_inventory_records(&inventory_record(b"a", 0), &[(b"a".as_slice(), 0)]).is_err()
    );

    #[cfg(target_os = "macos")]
    {
        let mut with_tombstone = inventory_record(b"a", 7);
        with_tombstone.extend_from_slice(&inventory_record(b"", 0));
        with_tombstone.extend_from_slice(&inventory_record(b"b", 8));
        assert_eq!(
            test_parse_inventory_records(
                &with_tombstone,
                &[(b"a".as_slice(), 7), (b"b".as_slice(), 8)]
            ),
            Ok(())
        );
        let overlong = inventory_record(&vec![b'a'; 1024], 7);
        assert!(test_parse_inventory_records(&overlong, &[]).is_err());
    }
}

#[cfg(unix)]
#[test]
fn prepared_inventory_seek_reset_and_authentication_failures_are_bounded() {
    let base = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!("semaprax-inventory-failure-{}", std::process::id()));
    for (suffix, failures, expected_scans) in [
        ("initial", (true, false, false, false), 0),
        ("reset", (false, true, false, false), 1),
        ("authentication", (false, false, true, false), 1),
    ] {
        let root = base.join(suffix);
        with_inventory_fixture(&root, |directory, names, file, prepared| {
            super::platform::test_inventory_exact_failures(
                prepared, failures.0, failures.1, failures.2, failures.3,
            );
            assert!(super::platform::inventory_exact_prepared(
                prepared,
                directory,
                names,
                [Some(file)]
            )
            .is_err());
            assert_eq!(
                super::platform::test_inventory_exact_scan_entries(prepared),
                expected_scans
            );
            assert_eq!(
                super::platform::prepared_inventory_exact_remaining(prepared),
                1
            );
        });
    }
    let _ = std::fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn prepared_inventory_rebound_close_failure_is_fail_stop() {
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-inventory-close-failure-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("tests::prepared_inventory_rebound_close_failure_child")
        .arg("--nocapture")
        .env("SEMAPRAX_INVENTORY_CLOSE_FAILURE_ROOT", &root)
        .status()
        .unwrap();
    assert!(!status.success());
    assert!(!root.join("later-action").exists());
    let _ = std::fs::remove_dir_all(root);
}
