use super::*;

#[test]
fn file_count_and_declared_lengths_fail_before_unbounded_allocation() {
    let executable = elf(62, None);
    let valid = node(&[("bin/node", true, &executable)], 0);
    for (count, error) in [
        (0_u32, Error::Invalid),
        (4097, Error::Limit),
        (u32::MAX, Error::Limit),
    ] {
        let mut bytes = valid.clone();
        bytes[12..16].copy_from_slice(&count.to_le_bytes());
        rejects(&bytes, error);
    }
    let paths = (0..4096).map(|i| format!("{i:04}")).collect::<Vec<_>>();
    for count in [4095, 4096] {
        let mut files = paths[..count]
            .iter()
            .map(|p| (p.as_str(), false, b"".as_slice()))
            .collect::<Vec<_>>();
        files.push(("bin/node", true, &executable));
        let bytes = node(&files, count as u32);
        if count == 4095 {
            assert!(parse(&bytes).is_ok());
        } else {
            rejects(&bytes, Error::Limit);
        }
    }
    let record = 28 + ID.len();
    for (length, error) in [
        (0_u16, Error::Invalid),
        (1025, Error::Limit),
        (u16::MAX, Error::Limit),
    ] {
        let mut bytes = valid.clone();
        bytes[record..record + 2].copy_from_slice(&length.to_le_bytes());
        rejects(&bytes, error);
    }
    let mut bytes = valid.clone();
    bytes[record + 4..record + 12].copy_from_slice(&536_870_913_u64.to_le_bytes());
    rejects(&bytes, Error::Limit);
}

fn long_path(id: usize, length: usize) -> String {
    assert!((770..=1025).contains(&length));
    // Five components, every component <=255 even at path length1025.
    format!(
        "{id:04}{}/{}/{}/{}/x",
        "a".repeat(251),
        "b".repeat(255),
        "c".repeat(255),
        "d".repeat(length - 770)
    )
}

#[test]
fn path_component_depth_and_path_byte_boundaries_are_exact() {
    let executable = elf(62, None);
    for (length, expected) in [(1024, None), (1025, Some(Error::Limit))] {
        let path = long_path(0, length);
        assert_eq!(path.len(), length);
        let bytes = node(
            &[(path.as_str(), false, b""), ("bin/node", true, &executable)],
            1,
        );
        if let Some(error) = expected {
            rejects(&bytes, error);
        } else {
            assert!(parse(&bytes).is_ok());
        }
    }
    for (length, expected) in [(255, None), (256, Some(Error::Limit))] {
        let path = format!("{}/node", "a".repeat(length));
        let bytes = node(&[(path.as_str(), true, &executable)], 0);
        if let Some(error) = expected {
            rejects(&bytes, error);
        } else {
            assert!(parse(&bytes).is_ok());
        }
    }
    for (depth, expected) in [(32, None), (33, Some(Error::Limit))] {
        let path = format!("{}/node", vec!["a"; depth - 1].join("/"));
        let bytes = node(&[(path.as_str(), true, &executable)], 0);
        if let Some(error) = expected {
            rejects(&bytes, error);
        } else {
            assert!(parse(&bytes).is_ok());
        }
    }
}

#[test]
fn cumulative_path_limit_counts_every_byte_and_does_not_alias_file_count() {
    let executable = elf(62, None);
    for extra in [0, 1] {
        let mut paths = (0..1023).map(|i| long_path(i, 1024)).collect::<Vec<_>>();
        paths.push(long_path(1023, 1016 + extra));
        assert_eq!(
            paths.iter().map(String::len).sum::<usize>() + "bin/node".len(),
            1_048_576 + extra
        );
        let mut files = paths
            .iter()
            .map(|p| (p.as_str(), false, b"".as_slice()))
            .collect::<Vec<_>>();
        files.push(("bin/node", true, &executable));
        let bytes = node(&files, 1024);
        if extra == 0 {
            assert!(parse(&bytes).is_ok());
        } else {
            rejects(&bytes, Error::Limit);
        }
    }
}
