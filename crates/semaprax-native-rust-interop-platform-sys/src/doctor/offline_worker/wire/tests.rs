use super::*;

fn request_bytes(architecture: u8, target: u8, selector: &[u8]) -> Vec<u8> {
    let mut bytes = b"SPXDWK1\0".to_vec();
    bytes.extend_from_slice(&[1, architecture, target, [4, 1, 2, 7][usize::from(target)]]);
    bytes.extend_from_slice(&[0x31; 32]);
    bytes.extend_from_slice(&123u64.to_le_bytes());
    bytes.extend_from_slice(&[0x42; 32]);
    bytes.push(selector.len() as u8);
    bytes.extend_from_slice(selector);
    bytes
}

fn request() -> Request {
    Request::parse(&request_bytes(1, 3, b"profile-1")).unwrap()
}

#[test]
fn request_fields_and_target_role_inventory_are_exact() {
    for (architecture, expected_architecture) in [
        (1, DoctorOfflineArchitecture::LinuxX86_64),
        (2, DoctorOfflineArchitecture::LinuxAarch64),
    ] {
        for (target, expected) in [
            (0, vec![(4, DoctorOfflineTool::Rustc)]),
            (1, vec![(1, DoctorOfflineTool::Clang)]),
            (2, vec![(2, DoctorOfflineTool::Node)]),
            (
                3,
                vec![
                    (1, DoctorOfflineTool::Clang),
                    (2, DoctorOfflineTool::Node),
                    (4, DoctorOfflineTool::Rustc),
                ],
            ),
        ] {
            let bytes = request_bytes(architecture, target, b"profile-1");
            let parsed = Request::parse(&bytes).unwrap();
            assert_eq!(parsed.architecture, expected_architecture);
            assert_eq!(parsed.target, target);
            assert_eq!(parsed.roles().collect::<Vec<_>>(), expected);
            assert_eq!(parsed.selector, "profile-1");
            assert_eq!(parsed.bundle_len, 123);
            assert_eq!(parsed.bundle_digest, [0x42; 32]);
            assert_eq!(parsed.nonce, [0x31; 32]);
            assert_eq!(parsed.digest.as_slice(), Sha256::digest(&bytes).as_slice());
        }
    }
}

#[test]
fn request_rejects_every_truncated_prefix_trailing_fields_and_unknown_enums() {
    let bytes = request_bytes(1, 3, b"profile-1");
    for end in 0..bytes.len() {
        assert!(Request::parse(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(Request::parse(&trailing).unwrap_err(), Error::Invalid);
    for offset in 0..12 {
        let mut corrupt = bytes.clone();
        corrupt[offset] ^= 0x80;
        assert_eq!(
            Request::parse(&corrupt).unwrap_err(),
            Error::Invalid,
            "offset {offset}"
        );
    }
    for target in 0..4 {
        for role in 0..=u8::MAX {
            let mut corrupt = request_bytes(1, target, b"p");
            corrupt[11] = role;
            assert_eq!(
                Request::parse(&corrupt).is_ok(),
                role == [4, 1, 2, 7][usize::from(target)]
            );
        }
    }
    let mut zero_nonce = bytes;
    zero_nonce[12..44].fill(0);
    assert_eq!(Request::parse(&zero_nonce).unwrap_err(), Error::Invalid);
}

#[test]
fn request_selector_and_bundle_bounds_are_closed() {
    for selector in [
        b"".as_slice(),
        b"A",
        b"0a",
        b"-a",
        b"a/b",
        b"a.b",
        b"a_b",
        b"a\0",
        b"a\xff",
    ] {
        assert_eq!(
            Request::parse(&request_bytes(1, 0, selector)).unwrap_err(),
            Error::Invalid
        );
    }
    let maximum = request_bytes(2, 2, &[b'a'; 64]);
    assert_eq!(maximum.len(), MAX_REQUEST_BYTES);
    assert_eq!(Request::parse(&maximum).unwrap().selector.len(), 64);
    assert_eq!(
        Request::parse(&request_bytes(2, 2, &[b'a'; 65])).unwrap_err(),
        Error::Limit
    );
    let mut length = request_bytes(1, 1, b"p");
    for (value, expected) in [
        (0, Some(Error::Invalid)),
        (1, None),
        (DOCTOR_OFFLINE_INPUT_MAX_BYTES as u64, None),
        (
            DOCTOR_OFFLINE_INPUT_MAX_BYTES as u64 + 1,
            Some(Error::Limit),
        ),
        (u64::MAX, Some(Error::Limit)),
    ] {
        length[44..52].copy_from_slice(&value.to_le_bytes());
        assert_eq!(Request::parse(&length).err(), expected);
    }
}

#[test]
fn reply_canonical_bytes_and_every_error_are_round_tripped() {
    let request = request();
    let rows = vec![
        (1, Ok(vec![0, 0xff, b'x'])),
        (2, Err(ProbeError::Timeout)),
        (4, Ok(Vec::new())),
    ];
    let reply = encode_reply(&request, &rows).unwrap();
    let mut expected = b"SPXDWR1\0".to_vec();
    expected.extend_from_slice(&request.digest);
    expected.extend_from_slice(&[0x31; 32]);
    expected.extend_from_slice(&[1, 1, 3, 7, 3]);
    expected.extend_from_slice(&[
        1, 0, 3, 0, 0, 0, 0, 0xff, b'x', 2, 6, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0,
    ]);
    assert_eq!(reply, expected);
    assert_eq!(validate_reply(&request, &reply).unwrap(), rows);
    let request = Request::parse(&request_bytes(1, 1, b"p")).unwrap();
    for (status, error) in [
        (1, ProbeError::Invalid),
        (2, ProbeError::Unsupported),
        (3, ProbeError::Spawn),
        (4, ProbeError::Exit),
        (5, ProbeError::OutputLimit),
        (6, ProbeError::Timeout),
        (7, ProbeError::Io),
    ] {
        let rows = vec![(1, Err(error))];
        let bytes = encode_reply(&request, &rows).unwrap();
        assert_eq!(bytes[78], status);
        assert_eq!(&bytes[79..], &[0, 0, 0, 0]);
        assert_eq!(validate_reply(&request, &bytes).unwrap(), rows);
    }
}

#[test]
fn reply_rejects_binding_mutations_truncation_and_trailing_bytes() {
    let request = request();
    let rows = vec![(1, Ok(vec![1])), (2, Ok(vec![2])), (4, Ok(vec![3]))];
    let bytes = encode_reply(&request, &rows).unwrap();
    for end in 0..bytes.len() {
        assert!(
            validate_reply(&request, &bytes[..end]).is_err(),
            "prefix {end}"
        );
    }
    for offset in 0..REPLY_HEADER {
        let mut corrupt = bytes.clone();
        corrupt[offset] ^= 0x80;
        assert_eq!(
            validate_reply(&request, &corrupt).unwrap_err(),
            Error::Invalid,
            "offset {offset}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        validate_reply(&request, &trailing).unwrap_err(),
        Error::Invalid
    );
    for offset in [12, 44, 52, 85] {
        let mut foreign = request_bytes(1, 3, b"profile-1");
        foreign[offset] ^= 1;
        let foreign = Request::parse(&foreign).unwrap();
        assert_eq!(
            validate_reply(&foreign, &bytes).unwrap_err(),
            Error::Invalid
        );
    }
    for (architecture, target, selector) in [
        (2, 3, b"profile-1".as_slice()),
        (1, 0, b"profile-1"),
        (1, 3, b"other"),
    ] {
        let foreign = Request::parse(&request_bytes(architecture, target, selector)).unwrap();
        assert_eq!(
            validate_reply(&foreign, &bytes).unwrap_err(),
            Error::Invalid
        );
    }
}

#[test]
fn reply_requires_exact_role_order_status_and_empty_failure_payloads() {
    let request = request();
    let rows = vec![
        (1, Ok(Vec::new())),
        (2, Ok(Vec::new())),
        (4, Ok(Vec::new())),
    ];
    assert_eq!(
        encode_reply(&request, &rows[..2]).unwrap_err(),
        Error::Invalid
    );
    let mut extra = rows.clone();
    extra.push((4, Ok(Vec::new())));
    assert_eq!(encode_reply(&request, &extra).unwrap_err(), Error::Invalid);
    for roles in [[2, 1, 4], [1, 1, 4], [1, 2, 2], [1, 2, 8]] {
        let changed = roles.map(|role| (role, Ok(Vec::new())));
        assert_eq!(
            encode_reply(&request, &changed).unwrap_err(),
            Error::Invalid
        );
        let mut corrupt = encode_reply(&request, &rows).unwrap();
        for (offset, role) in [77, 83, 89].into_iter().zip(roles) {
            corrupt[offset] = role;
        }
        assert_eq!(
            validate_reply(&request, &corrupt).unwrap_err(),
            Error::Invalid
        );
    }
    let mut unknown = encode_reply(&request, &rows).unwrap();
    unknown[78] = 8;
    assert_eq!(
        validate_reply(&request, &unknown).unwrap_err(),
        Error::Invalid
    );
    let single = Request::parse(&request_bytes(1, 1, b"p")).unwrap();
    let mut failure = encode_reply(&single, &[(1, Ok(vec![0x55]))]).unwrap();
    failure[78] = 1;
    assert_eq!(
        validate_reply(&single, &failure).unwrap_err(),
        Error::Invalid
    );
}

#[test]
fn reply_payload_and_total_bounds_reject_before_payload_copy() {
    let request = request();
    for length in [65_535, 65_536] {
        let rows = [1, 2, 4].map(|role| (role, Ok(vec![role; length])));
        let reply = encode_reply(&request, &rows).unwrap();
        assert_eq!(reply.len(), 77 + 3 * (6 + length));
        assert!(reply.len() <= MAX_REPLY_BYTES);
        assert_eq!(validate_reply(&request, &reply).unwrap(), rows);
    }
    let oversized = [1, 2, 4].map(|role| (role, Ok(vec![role; 65_537])));
    assert_eq!(
        encode_reply(&request, &oversized).unwrap_err(),
        Error::Limit
    );
    let rows = [1, 2, 4].map(|role| (role, Ok(Vec::new())));
    for length in [65_537u32, u32::MAX] {
        let mut corrupt = encode_reply(&request, &rows).unwrap();
        corrupt[79..83].copy_from_slice(&length.to_le_bytes());
        assert_eq!(
            validate_reply(&request, &corrupt).unwrap_err(),
            Error::Limit
        );
    }
    assert_eq!(
        validate_reply(&request, &vec![0; MAX_REPLY_BYTES + 1]).unwrap_err(),
        Error::Limit
    );
}
