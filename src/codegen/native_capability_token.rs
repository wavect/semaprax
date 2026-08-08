//! Private authenticated capability-token codec for future native adapters.
//!
//! The codec is deliberately disconnected from compiler resource preflight and
//! exports no C API. A trusted runtime must supply high-quality entropy and a
//! sealed binding context before it can mint a token. OS CSPRNG integration is
//! a later runtime milestone.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable native adapter authority remains gated")
)]

use hmac::{Hmac, Mac};
use sha2::Sha256;

const TOKEN_MAGIC: &[u8; 4] = b"SPXC";
const TOKEN_VERSION: u8 = 1;
const TOKEN_BODY_BYTES: usize = 32;
const TOKEN_TAG_BYTES: usize = 32;
const TOKEN_BYTES: usize = TOKEN_BODY_BYTES + TOKEN_TAG_BYTES;
const TOKEN_AUTHENTICATION_DOMAIN: &[u8] = b"semaprax.native-capability-token.v1\0";

type HmacSha256 = Hmac<Sha256>;

const MAGIC_OFFSET: usize = 0;
const VERSION_OFFSET: usize = 4;
const KIND_OFFSET: usize = 5;
const RESERVED_OFFSET: usize = 6;
const EPOCH_OFFSET: usize = 8;
const SLOT_OFFSET: usize = 16;
const GENERATION_OFFSET: usize = 24;

/// Full-width secret key supplied only by a trusted runtime entropy source.
///
/// This type is intentionally neither `Clone` nor `Debug`. The constructor
/// checks only the codec's structural minimum (not all zero); entropy quality
/// remains the trusted runtime's obligation until OS CSPRNG integration lands.
pub(super) struct NativeCapabilitySecret([u8; 32]);

impl NativeCapabilitySecret {
    pub(super) fn from_trusted_runtime_entropy(
        bytes: [u8; 32],
    ) -> Result<Self, NativeCapabilityTokenError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(NativeCapabilityTokenError::InvalidEntropy);
        }
        Ok(Self(bytes))
    }
}

impl Drop for NativeCapabilitySecret {
    fn drop(&mut self) {
        // Best-effort hygiene only. Rust does not promise that this ordinary
        // fill survives every compiler optimization or clears key material
        // copied inside the audited HMAC implementation.
        self.0.fill(0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum NativeCapabilityKind {
    Owner = 1,
    FunctionOwnedResult = 2,
}

impl NativeCapabilityKind {
    fn from_byte(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Owner),
            2 => Some(Self::FunctionOwnedResult),
            _ => None,
        }
    }
}

/// Sealed semantic and physical scope for one adapter binding.
///
/// No field comes from token bytes. The future runtime must derive all of them
/// from admitted module/function/resource metadata and its observed binding.
pub(super) struct NativeCapabilityBinding<'a> {
    /// The descriptor-derived physical-module fingerprint already binds the
    /// physical schema/target and admitted semantic module ABI transitively.
    physical_module_fingerprint: &'a [u8; 32],
    adapter_identity: &'a [u8],
    binding_epoch: u64,
    kind: NativeCapabilityKind,
    function_template_fingerprint: Option<&'a [u8; 32]>,
    resource_identity: &'a [u8],
    lifecycle_identity: &'a [u8],
    thread_policy_identity: &'a [u8],
    /// Runtime-observed/derived binding-instance identity. This is distinct
    /// from the static thread policy and is never a raw native thread ID.
    thread_binding_identity: &'a [u8],
}

impl<'a> NativeCapabilityBinding<'a> {
    #[allow(clippy::too_many_arguments)]
    /// Validate the structural shape of a runtime-supplied binding.
    ///
    /// This constructor does not prove binding-epoch uniqueness or retain the
    /// admitted module. Those remain obligations of the future runtime ledger.
    pub(super) fn from_trusted_runtime_binding(
        physical_module_fingerprint: &'a [u8; 32],
        adapter_identity: &'a [u8],
        binding_epoch: u64,
        kind: NativeCapabilityKind,
        function_template_fingerprint: Option<&'a [u8; 32]>,
        resource_identity: &'a [u8],
        lifecycle_identity: &'a [u8],
        thread_policy_identity: &'a [u8],
        thread_binding_identity: &'a [u8],
    ) -> Result<Self, NativeCapabilityTokenError> {
        if binding_epoch == 0 || fingerprint_is_uninitialized(physical_module_fingerprint) {
            return Err(NativeCapabilityTokenError::InvalidBinding);
        }
        for identity in [
            adapter_identity,
            resource_identity,
            lifecycle_identity,
            thread_policy_identity,
            thread_binding_identity,
        ] {
            require_identity(identity)?;
        }
        if function_template_fingerprint.is_some_and(fingerprint_is_uninitialized) {
            return Err(NativeCapabilityTokenError::InvalidBinding);
        }
        match (kind, function_template_fingerprint) {
            (NativeCapabilityKind::Owner, None)
            | (NativeCapabilityKind::FunctionOwnedResult, Some(_)) => {}
            (NativeCapabilityKind::Owner, Some(_))
            | (NativeCapabilityKind::FunctionOwnedResult, None) => {
                return Err(NativeCapabilityTokenError::InvalidBinding)
            }
        }
        Ok(Self {
            physical_module_fingerprint,
            adapter_identity,
            binding_epoch,
            kind,
            function_template_fingerprint,
            resource_identity,
            lifecycle_identity,
            thread_policy_identity,
            thread_binding_identity,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeCapabilityClaims {
    pub(super) slot: u64,
    pub(super) generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeCapabilityTokenError {
    InvalidEntropy,
    InvalidBinding,
    InvalidLength,
    InvalidMagic,
    UnsupportedVersion,
    UnsupportedKind,
    NonCanonicalReserved,
    ZeroBindingEpoch,
    ZeroSlot,
    ZeroGeneration,
    AuthenticationFailed,
}

/// Mint one canonical 64-byte bearer capability.
pub(super) fn mint(
    secret: &NativeCapabilitySecret,
    binding: &NativeCapabilityBinding<'_>,
    slot: u64,
    generation: u64,
) -> Result<[u8; TOKEN_BYTES], NativeCapabilityTokenError> {
    if slot == 0 {
        return Err(NativeCapabilityTokenError::ZeroSlot);
    }
    if generation == 0 {
        return Err(NativeCapabilityTokenError::ZeroGeneration);
    }
    let mut token = [0_u8; TOKEN_BYTES];
    token[MAGIC_OFFSET..MAGIC_OFFSET + TOKEN_MAGIC.len()].copy_from_slice(TOKEN_MAGIC);
    token[VERSION_OFFSET] = TOKEN_VERSION;
    token[KIND_OFFSET] = binding.kind as u8;
    token[EPOCH_OFFSET..EPOCH_OFFSET + 8].copy_from_slice(&binding.binding_epoch.to_le_bytes());
    token[SLOT_OFFSET..SLOT_OFFSET + 8].copy_from_slice(&slot.to_le_bytes());
    token[GENERATION_OFFSET..GENERATION_OFFSET + 8].copy_from_slice(&generation.to_le_bytes());
    let tag = authentication_tag(secret, binding, &token[..TOKEN_BODY_BYTES]);
    token[TOKEN_BODY_BYTES..].copy_from_slice(&tag);
    Ok(token)
}

/// Authenticate a slice and return claims only after RustCrypto's full-tag
/// `Mac::verify_slice` succeeds. Only tag verification has the library's
/// constant-time guarantee; structural parsing and this API as a whole do not.
pub(super) fn authenticate(
    secret: &NativeCapabilitySecret,
    binding: &NativeCapabilityBinding<'_>,
    token: &[u8],
) -> Result<NativeCapabilityClaims, NativeCapabilityTokenError> {
    if token.len() != TOKEN_BYTES {
        return Err(NativeCapabilityTokenError::InvalidLength);
    }
    if &token[MAGIC_OFFSET..MAGIC_OFFSET + TOKEN_MAGIC.len()] != TOKEN_MAGIC {
        return Err(NativeCapabilityTokenError::InvalidMagic);
    }
    if token[VERSION_OFFSET] != TOKEN_VERSION {
        return Err(NativeCapabilityTokenError::UnsupportedVersion);
    }
    let kind = NativeCapabilityKind::from_byte(token[KIND_OFFSET])
        .ok_or(NativeCapabilityTokenError::UnsupportedKind)?;
    if token[RESERVED_OFFSET..EPOCH_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(NativeCapabilityTokenError::NonCanonicalReserved);
    }
    let epoch = read_u64(token, EPOCH_OFFSET);
    if epoch == 0 {
        return Err(NativeCapabilityTokenError::ZeroBindingEpoch);
    }
    let slot = read_u64(token, SLOT_OFFSET);
    if slot == 0 {
        return Err(NativeCapabilityTokenError::ZeroSlot);
    }
    let generation = read_u64(token, GENERATION_OFFSET);
    if generation == 0 {
        return Err(NativeCapabilityTokenError::ZeroGeneration);
    }

    verify_authentication_tag(
        secret,
        binding,
        &token[..TOKEN_BODY_BYTES],
        &token[TOKEN_BODY_BYTES..],
    )?;
    if epoch != binding.binding_epoch || kind != binding.kind {
        return Err(NativeCapabilityTokenError::AuthenticationFailed);
    }
    Ok(NativeCapabilityClaims { slot, generation })
}

/// Authenticate and compare the ledger's expected slot/generation. A genuine
/// but stale bearer receives the same generic authentication failure as any
/// other context mismatch.
pub(super) fn authenticate_expected(
    secret: &NativeCapabilitySecret,
    binding: &NativeCapabilityBinding<'_>,
    token: &[u8],
    expected_slot: u64,
    expected_generation: u64,
) -> Result<NativeCapabilityClaims, NativeCapabilityTokenError> {
    let claims = authenticate(secret, binding, token)?;
    let difference = (claims.slot ^ expected_slot) | (claims.generation ^ expected_generation);
    if difference != 0 {
        return Err(NativeCapabilityTokenError::AuthenticationFailed);
    }
    Ok(claims)
}

fn authentication_tag(
    secret: &NativeCapabilitySecret,
    binding: &NativeCapabilityBinding<'_>,
    body: &[u8],
) -> [u8; TOKEN_TAG_BYTES] {
    audited_hmac_sha256(&secret.0, &authentication_message(binding, body))
}

fn verify_authentication_tag(
    secret: &NativeCapabilitySecret,
    binding: &NativeCapabilityBinding<'_>,
    body: &[u8],
    tag: &[u8],
) -> Result<(), NativeCapabilityTokenError> {
    let mut mac = HmacSha256::new_from_slice(&secret.0)
        .expect("SHA-256 HMAC accepts the fixed 32-byte capability secret");
    mac.update(&authentication_message(binding, body));
    mac.verify_slice(tag)
        .map_err(|_| NativeCapabilityTokenError::AuthenticationFailed)
}

/// Build the bounded authenticated transcript from trusted binding context.
///
/// This allocation is groundwork only. It must be removed or replaced with a
/// preallocated buffer before any allocation-free callable preflight exists.
fn authentication_message(binding: &NativeCapabilityBinding<'_>, body: &[u8]) -> Vec<u8> {
    let mut message = Vec::new();
    frame(&mut message, b"domain", TOKEN_AUTHENTICATION_DOMAIN);
    frame(
        &mut message,
        b"physical-module-fingerprint",
        binding.physical_module_fingerprint,
    );
    frame(&mut message, b"adapter-identity", binding.adapter_identity);
    frame(
        &mut message,
        b"binding-epoch",
        &binding.binding_epoch.to_le_bytes(),
    );
    frame(&mut message, b"token-kind", &[binding.kind as u8]);
    frame(
        &mut message,
        b"function-template-fingerprint",
        binding
            .function_template_fingerprint
            .map_or(&[], |fingerprint| fingerprint),
    );
    frame(
        &mut message,
        b"resource-identity",
        binding.resource_identity,
    );
    frame(
        &mut message,
        b"lifecycle-identity",
        binding.lifecycle_identity,
    );
    frame(
        &mut message,
        b"thread-policy-identity",
        binding.thread_policy_identity,
    );
    frame(
        &mut message,
        b"thread-binding-identity",
        binding.thread_binding_identity,
    );
    frame(&mut message, b"canonical-token-body", body);
    message
}

fn frame(output: &mut Vec<u8>, label: &[u8], value: &[u8]) {
    output.extend_from_slice(&(label.len() as u64).to_be_bytes());
    output.extend_from_slice(label);
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

/// Thin audited helper retained so the exact RFC 4231 KAT exercises the same
/// RustCrypto `Hmac<Sha256>` implementation used for capability minting.
fn audited_hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of every size");
    mac.update(message);
    mac.finalize().into_bytes().into()
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("token structural offsets are fixed"),
    )
}

fn require_identity(value: &[u8]) -> Result<(), NativeCapabilityTokenError> {
    if value.is_empty() || value.contains(&0) {
        Err(NativeCapabilityTokenError::InvalidBinding)
    } else {
        Ok(())
    }
}

fn fingerprint_is_uninitialized(fingerprint: &[u8; 32]) -> bool {
    fingerprint.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
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
                83, 80, 88, 67, 1, 2, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 24, 23, 22, 21, 20, 19, 18, 17,
                40, 39, 38, 37, 36, 35, 34, 33, 54, 13, 122, 182, 162, 220, 86, 248, 92, 32, 175,
                18, 4, 85, 211, 104, 169, 198, 253, 139, 76, 182, 131, 254, 228, 46, 10, 32, 144,
                150, 176, 240,
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
                83, 80, 88, 67, 1, 1, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 29, 0, 0, 0, 0, 0, 0, 0, 31, 0,
                0, 0, 0, 0, 0, 0, 215, 205, 166, 64, 249, 53, 136, 200, 191, 32, 122, 106, 28, 155,
                187, 236, 214, 141, 252, 149, 246, 12, 115, 220, 193, 54, 173, 172, 78, 150, 6,
                251,
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
}
