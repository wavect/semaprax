use super::*;

const MODULE: [u8; 32] = [0xa5; 32];
const OTHER_MODULE: [u8; 32] = [0x5a; 32];
const ADAPTER: &[u8] = b"adapter.binding.one";
const FUNCTION_TEMPLATE: &[u8; 32] = &[0x3c; 32];
const OTHER_FUNCTION_TEMPLATE: &[u8; 32] = &[0xc3; 32];
const RESOURCE: &[u8] = b"token.type";
const LIFECYCLE: &[u8] = b"token.drop";
const THREAD_POLICY: &[u8] = b"semaprax.thread-bound.v1";
const THREAD_BINDING: &[u8] = b"runtime-observed-thread-binding:fixture-one";
const EPOCH: u64 = 0x0102_0304_0506_0708;

fn test_secret(fill: u8) -> NativeCapabilitySecret {
    // Deterministic low-quality bytes are test fixtures only. Acceptance
    // here does not establish production entropy or token unforgeability.
    NativeCapabilitySecret::from_trusted_runtime_entropy([fill; 32]).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn binding<'a>(
    module: &'a [u8; 32],
    adapter: &'a [u8],
    epoch: u64,
    kind: NativeCapabilityKind,
    function_template: Option<&'a [u8; 32]>,
    resource: &'a [u8],
    lifecycle: &'a [u8],
    thread_policy: &'a [u8],
) -> NativeCapabilityBinding<'a> {
    NativeCapabilityBinding::from_trusted_runtime_binding(
        module,
        adapter,
        epoch,
        kind,
        function_template,
        resource,
        lifecycle,
        thread_policy,
        THREAD_BINDING,
    )
    .unwrap()
}

fn result_binding() -> NativeCapabilityBinding<'static> {
    binding(
        &MODULE,
        ADAPTER,
        EPOCH,
        NativeCapabilityKind::FunctionOwnedResult,
        Some(FUNCTION_TEMPLATE),
        RESOURCE,
        LIFECYCLE,
        THREAD_POLICY,
    )
}

#[test]
fn rfc_4231_hmac_sha256_case_one_is_exact() {
    let actual = audited_hmac_sha256(&[0x0b; 20], b"Hi There");
    assert_eq!(
        actual,
        [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ]
    );
}

#[test]
fn token_golden_roundtrip_and_copy_as_bearer_are_exact() {
    let secret = test_secret(0x11);
    let binding = result_binding();
    let token = mint(
        &secret,
        &binding,
        0x1112_1314_1516_1718,
        0x2122_2324_2526_2728,
    )
    .unwrap();
    assert_eq!(token.len(), 64);
    assert_eq!(
        token,
        [
            83, 80, 88, 67, 1, 2, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 24, 23, 22, 21, 20, 19, 18, 17, 40,
            39, 38, 37, 36, 35, 34, 33, 54, 13, 122, 182, 162, 220, 86, 248, 92, 32, 175, 18, 4,
            85, 211, 104, 169, 198, 253, 139, 76, 182, 131, 254, 228, 46, 10, 32, 144, 150, 176,
            240,
        ]
    );
    let copied = token;
    assert_eq!(
        authenticate(&secret, &binding, &copied).unwrap(),
        NativeCapabilityClaims {
            slot: 0x1112_1314_1516_1718,
            generation: 0x2122_2324_2526_2728,
        }
    );
}

#[test]
fn every_token_bit_is_authenticated_or_structurally_rejected() {
    let secret = test_secret(0x22);
    let binding = result_binding();
    let token = mint(&secret, &binding, 7, 9).unwrap();
    for byte in 0..TOKEN_BYTES {
        for bit in 0..8 {
            let mut hostile = token;
            hostile[byte] ^= 1 << bit;
            assert!(
                authenticate(&secret, &binding, &hostile).is_err(),
                "byte {byte} bit {bit} was not covered"
            );
        }
    }
}

#[test]
fn structural_noncanonical_forms_fail_before_claims_exist() {
    assert_eq!(
        NativeCapabilitySecret::from_trusted_runtime_entropy([0; 32]).err(),
        Some(NativeCapabilityTokenError::InvalidEntropy)
    );
    let secret = test_secret(0x33);
    let binding = result_binding();
    let token = mint(&secret, &binding, 3, 4).unwrap();
    for length in 0..TOKEN_BYTES {
        assert_eq!(
            authenticate(&secret, &binding, &token[..length]),
            Err(NativeCapabilityTokenError::InvalidLength)
        );
    }
    let mut overlong = token.to_vec();
    overlong.push(0);
    assert_eq!(
        authenticate(&secret, &binding, &overlong),
        Err(NativeCapabilityTokenError::InvalidLength)
    );

    let cases = [
        (MAGIC_OFFSET, 0_u8, NativeCapabilityTokenError::InvalidMagic),
        (
            VERSION_OFFSET,
            TOKEN_VERSION + 1,
            NativeCapabilityTokenError::UnsupportedVersion,
        ),
        (
            KIND_OFFSET,
            0xff,
            NativeCapabilityTokenError::UnsupportedKind,
        ),
        (
            RESERVED_OFFSET,
            1,
            NativeCapabilityTokenError::NonCanonicalReserved,
        ),
        (
            RESERVED_OFFSET + 1,
            1,
            NativeCapabilityTokenError::NonCanonicalReserved,
        ),
    ];
    for (offset, value, expected) in cases {
        let mut hostile = token;
        hostile[offset] = value;
        assert_eq!(authenticate(&secret, &binding, &hostile), Err(expected));
    }
    for (offset, expected) in [
        (EPOCH_OFFSET, NativeCapabilityTokenError::ZeroBindingEpoch),
        (SLOT_OFFSET, NativeCapabilityTokenError::ZeroSlot),
        (
            GENERATION_OFFSET,
            NativeCapabilityTokenError::ZeroGeneration,
        ),
    ] {
        let mut hostile = token;
        hostile[offset..offset + 8].fill(0);
        assert_eq!(authenticate(&secret, &binding, &hostile), Err(expected));
    }
    assert_eq!(
        mint(&secret, &binding, 0, 1),
        Err(NativeCapabilityTokenError::ZeroSlot)
    );
    assert_eq!(
        mint(&secret, &binding, 1, 0),
        Err(NativeCapabilityTokenError::ZeroGeneration)
    );
}

#[test]
fn all_sealed_context_dimensions_and_secrets_are_bound() {
    let secret = test_secret(0x44);
    let other_secret = test_secret(0x45);
    let canonical = result_binding();
    let token = mint(&secret, &canonical, 11, 13).unwrap();

    let other_adapter = b"adapter.binding.two";
    let other_resource = b"other.type";
    let other_lifecycle = b"other.drop";
    let other_thread = b"semaprax.other-thread-policy.v1";
    let other_thread_binding = b"runtime-observed-thread-binding:fixture-two";
    let contexts = [
        binding(
            &OTHER_MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
        ),
        binding(
            &MODULE,
            other_adapter,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
        ),
        binding(
            &MODULE,
            ADAPTER,
            EPOCH + 1,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
        ),
        binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(OTHER_FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
        ),
        binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            other_resource,
            LIFECYCLE,
            THREAD_POLICY,
        ),
        binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            other_lifecycle,
            THREAD_POLICY,
        ),
        binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            other_thread,
        ),
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
            other_thread_binding,
        )
        .unwrap(),
    ];
    assert_eq!(
        authenticate(&other_secret, &canonical, &token),
        Err(NativeCapabilityTokenError::AuthenticationFailed)
    );
    for context in &contexts {
        assert_eq!(
            authenticate(&secret, context, &token),
            Err(NativeCapabilityTokenError::AuthenticationFailed)
        );
    }

    let owner_context = binding(
        &MODULE,
        ADAPTER,
        EPOCH,
        NativeCapabilityKind::Owner,
        None,
        RESOURCE,
        LIFECYCLE,
        THREAD_POLICY,
    );
    assert_eq!(
        authenticate(&secret, &owner_context, &token),
        Err(NativeCapabilityTokenError::AuthenticationFailed)
    );
}

#[test]
fn stale_generation_and_full_tag_mutation_use_generic_authentication_failure() {
    let secret = test_secret(0x55);
    let binding = result_binding();
    let stale = mint(&secret, &binding, 17, 1).unwrap();
    let current = mint(&secret, &binding, 17, 2).unwrap();
    assert_eq!(
        authenticate_expected(&secret, &binding, &stale, 17, 2),
        Err(NativeCapabilityTokenError::AuthenticationFailed)
    );
    assert_eq!(
        authenticate_expected(&secret, &binding, &current, 17, 2).unwrap(),
        NativeCapabilityClaims {
            slot: 17,
            generation: 2,
        }
    );
    for tag_byte in TOKEN_BODY_BYTES..TOKEN_BYTES {
        let mut hostile = current;
        hostile[tag_byte] ^= 0x80;
        assert_eq!(
            authenticate(&secret, &binding, &hostile),
            Err(NativeCapabilityTokenError::AuthenticationFailed)
        );
    }
}

#[test]
fn owner_token_authenticates_across_compatible_function_call_contexts() {
    let secret = test_secret(0x5a);
    let owner_binding_for_call = |caller_template: &[u8; 32]| {
        assert!(!fingerprint_is_uninitialized(caller_template));
        // Owner authority is module/resource scoped, so compatible caller
        // templates are deliberately checked outside and omitted here.
        binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::Owner,
            None,
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
        )
    };
    assert_ne!(FUNCTION_TEMPLATE, OTHER_FUNCTION_TEMPLATE);
    let call_a = owner_binding_for_call(FUNCTION_TEMPLATE);
    let call_b = owner_binding_for_call(OTHER_FUNCTION_TEMPLATE);
    let token = mint(&secret, &call_a, 29, 31).unwrap();
    assert_eq!(
        token,
        [
            83, 80, 88, 67, 1, 1, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 29, 0, 0, 0, 0, 0, 0, 0, 31, 0, 0,
            0, 0, 0, 0, 0, 215, 205, 166, 64, 249, 53, 136, 200, 191, 32, 122, 106, 28, 155, 187,
            236, 214, 141, 252, 149, 246, 12, 115, 220, 193, 54, 173, 172, 78, 150, 6, 251,
        ]
    );
    assert_eq!(
        authenticate(&secret, &call_b, &token).unwrap(),
        NativeCapabilityClaims {
            slot: 29,
            generation: 31,
        }
    );
}

#[test]
fn deterministic_arbitrary_byte_corpus_never_panics_or_authenticates() {
    let secret = test_secret(0x71);
    let binding = result_binding();
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    for length in 0..=128 {
        for sample in 0..16_u64 {
            let mut hostile = vec![0_u8; length];
            for byte in &mut hostile {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state.wrapping_add(sample) as u8;
            }
            assert!(authenticate(&secret, &binding, &hostile).is_err());
        }
    }
}

#[test]
fn maximum_epoch_slot_and_generation_roundtrip() {
    let secret = test_secret(0x5b);
    let binding = binding(
        &MODULE,
        ADAPTER,
        u64::MAX,
        NativeCapabilityKind::FunctionOwnedResult,
        Some(FUNCTION_TEMPLATE),
        RESOURCE,
        LIFECYCLE,
        THREAD_POLICY,
    );
    let token = mint(&secret, &binding, u64::MAX, u64::MAX).unwrap();
    assert_eq!(
        authenticate_expected(&secret, &binding, &token, u64::MAX, u64::MAX,).unwrap(),
        NativeCapabilityClaims {
            slot: u64::MAX,
            generation: u64::MAX,
        }
    );
}

#[test]
fn canonical_body_contains_only_public_codec_fields() {
    let secret = test_secret(0x66);
    let binding = result_binding();
    let token = mint(&secret, &binding, 0xdead_beef, 0x0102_0304).unwrap();
    let body = &token[..TOKEN_BODY_BYTES];
    assert_eq!(&body[..4], TOKEN_MAGIC);
    assert_eq!(body[VERSION_OFFSET], TOKEN_VERSION);
    assert_eq!(
        body[KIND_OFFSET],
        NativeCapabilityKind::FunctionOwnedResult as u8
    );
    assert_eq!(&body[RESERVED_OFFSET..EPOCH_OFFSET], &[0, 0]);
    for sensitive in [
        ADAPTER,
        FUNCTION_TEMPLATE,
        RESOURCE,
        LIFECYCLE,
        THREAD_POLICY,
        THREAD_BINDING,
    ] {
        assert!(
            !body
                .windows(sensitive.len())
                .any(|window| window == sensitive),
            "sensitive context leaked into canonical body"
        );
    }
    assert!(!body.windows(MODULE.len()).any(|window| window == MODULE));
}

#[test]
fn binding_construction_rejects_missing_or_misplaced_semantic_scope() {
    let zero_fingerprint = [0_u8; 32];
    assert!(matches!(
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &zero_fingerprint,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::Owner,
            None,
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
            THREAD_BINDING,
        ),
        Err(NativeCapabilityTokenError::InvalidBinding)
    ));
    assert!(matches!(
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            Some(&zero_fingerprint),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
            THREAD_BINDING,
        ),
        Err(NativeCapabilityTokenError::InvalidBinding)
    ));
    assert!(matches!(
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &MODULE,
            ADAPTER,
            0,
            NativeCapabilityKind::Owner,
            None,
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
            THREAD_BINDING,
        ),
        Err(NativeCapabilityTokenError::InvalidBinding)
    ));
    for invalid in [b"".as_slice(), b"bad\0identity".as_slice()] {
        assert!(matches!(
            NativeCapabilityBinding::from_trusted_runtime_binding(
                &MODULE,
                invalid,
                EPOCH,
                NativeCapabilityKind::Owner,
                None,
                RESOURCE,
                LIFECYCLE,
                THREAD_POLICY,
                THREAD_BINDING,
            ),
            Err(NativeCapabilityTokenError::InvalidBinding)
        ));
        assert!(matches!(
            NativeCapabilityBinding::from_trusted_runtime_binding(
                &MODULE,
                ADAPTER,
                EPOCH,
                NativeCapabilityKind::Owner,
                None,
                RESOURCE,
                LIFECYCLE,
                THREAD_POLICY,
                invalid,
            ),
            Err(NativeCapabilityTokenError::InvalidBinding)
        ));
    }
    assert!(matches!(
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::Owner,
            Some(FUNCTION_TEMPLATE),
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
            THREAD_BINDING,
        ),
        Err(NativeCapabilityTokenError::InvalidBinding)
    ));
    assert!(matches!(
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &MODULE,
            ADAPTER,
            EPOCH,
            NativeCapabilityKind::FunctionOwnedResult,
            None,
            RESOURCE,
            LIFECYCLE,
            THREAD_POLICY,
            THREAD_BINDING,
        ),
        Err(NativeCapabilityTokenError::InvalidBinding)
    ));
}
