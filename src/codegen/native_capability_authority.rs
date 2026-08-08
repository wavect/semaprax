//! Private OS-backed authority for staged native capability-token mechanics.
//!
//! This authority is deliberately unreachable from compiler preflight and has
//! no public or C-facing API. It proves fail-closed entropy acquisition and
//! actual-thread binding only; it is not a callable adapter, ownership ledger,
//! retained-library handle, or module-unload protocol.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable native adapter authority remains gated")
)]

use std::thread::{self, ThreadId};

use super::native_capability_token::{
    authenticate_expected, mint, NativeCapabilityBinding, NativeCapabilityClaims,
    NativeCapabilityKind, NativeCapabilitySecret, NativeCapabilityTokenError, TOKEN_BYTES,
};

const SECRET_BYTES: usize = 32;
const EPOCH_BYTES: usize = 8;
const THREAD_NONCE_BYTES: usize = 32;
const SEED_BYTES: usize = SECRET_BYTES + EPOCH_BYTES + THREAD_NONCE_BYTES;
const THREAD_BINDING_HEX_BYTES: usize = THREAD_NONCE_BYTES * 2;

/// Immutable semantic/physical scope copied into one runtime binding.
///
/// Construction is private to the future retained adapter. Identities use the
/// same canonical nonempty, NUL-free form as the token codec.
pub(super) struct NativeCapabilityAuthorityConfig<'a> {
    pub(super) physical_module_fingerprint: &'a [u8; 32],
    pub(super) adapter_identity: &'a [u8],
    pub(super) resource_identity: &'a [u8],
    pub(super) lifecycle_identity: &'a [u8],
    pub(super) thread_policy_identity: &'a [u8],
}

/// One binding-instance authority.
///
/// The type is intentionally neither `Clone` nor `Debug`. Tokens remain
/// copyable bearer bytes, so the future synchronized ledger must still enforce
/// generation liveness, replay rejection, and exactly-once consumption.
pub(super) struct NativeCapabilityAuthority {
    secret: NativeCapabilitySecret,
    physical_module_fingerprint: [u8; 32],
    adapter_identity: Vec<u8>,
    binding_epoch: u64,
    resource_identity: Vec<u8>,
    lifecycle_identity: Vec<u8>,
    thread_policy_identity: Vec<u8>,
    thread_binding_identity: [u8; THREAD_BINDING_HEX_BYTES],
    bound_thread: ThreadId,
}

/// Credential wrapper used only inside the staged Rust runtime.
///
/// It intentionally implements neither copying nor formatting traits. This is
/// defense-in-depth against accidental logging, not a linearity guarantee: the
/// authenticated bytes remain reproducible by any holder that can read them.
pub(super) struct StagedNativeCapabilityToken([u8; TOKEN_BYTES]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeCapabilityAuthorityError {
    EntropyUnavailable,
    InvalidEntropy,
    InvalidBinding,
    WrongThread,
    Token(NativeCapabilityTokenError),
}

impl NativeCapabilityAuthority {
    /// Create an authority from exactly one operating-system random fill.
    ///
    /// Failure has no deterministic fallback. An all-zero key, zero epoch, or
    /// all-zero thread nonce is rejected instead of weakened or retried.
    pub(super) fn from_os(
        config: NativeCapabilityAuthorityConfig<'_>,
    ) -> Result<Self, NativeCapabilityAuthorityError> {
        Self::from_fill(config, |seed| getrandom::fill(seed).map_err(|_| ()))
    }

    #[cfg(test)]
    fn from_entropy_source(
        config: NativeCapabilityAuthorityConfig<'_>,
        source: &mut impl EntropySource,
    ) -> Result<Self, NativeCapabilityAuthorityError> {
        Self::from_fill(config, |seed| source.fill(seed))
    }

    fn from_fill(
        config: NativeCapabilityAuthorityConfig<'_>,
        fill: impl FnOnce(&mut [u8; SEED_BYTES]) -> Result<(), ()>,
    ) -> Result<Self, NativeCapabilityAuthorityError> {
        validate_config(&config)?;

        let mut seed = [0_u8; SEED_BYTES];
        if fill(&mut seed).is_err() {
            seed.fill(0);
            return Err(NativeCapabilityAuthorityError::EntropyUnavailable);
        }
        Self::from_seed(config, seed)
    }

    fn from_seed(
        config: NativeCapabilityAuthorityConfig<'_>,
        mut seed: [u8; SEED_BYTES],
    ) -> Result<Self, NativeCapabilityAuthorityError> {
        let mut secret_bytes = [0_u8; SECRET_BYTES];
        secret_bytes.copy_from_slice(&seed[..SECRET_BYTES]);
        let binding_epoch = u64::from_le_bytes(
            seed[SECRET_BYTES..SECRET_BYTES + EPOCH_BYTES]
                .try_into()
                .expect("authority seed epoch has a fixed width"),
        );
        let mut thread_nonce = [0_u8; THREAD_NONCE_BYTES];
        thread_nonce.copy_from_slice(&seed[SECRET_BYTES + EPOCH_BYTES..]);
        seed.fill(0);

        if secret_bytes.iter().all(|byte| *byte == 0)
            || binding_epoch == 0
            || thread_nonce.iter().all(|byte| *byte == 0)
        {
            secret_bytes.fill(0);
            thread_nonce.fill(0);
            return Err(NativeCapabilityAuthorityError::InvalidEntropy);
        }

        let secret = NativeCapabilitySecret::from_trusted_runtime_entropy(secret_bytes);
        secret_bytes.fill(0);
        let secret = secret.map_err(|_| NativeCapabilityAuthorityError::InvalidEntropy)?;
        let thread_binding_identity = encode_lower_hex(&thread_nonce);
        thread_nonce.fill(0);

        Ok(Self {
            secret,
            physical_module_fingerprint: *config.physical_module_fingerprint,
            adapter_identity: config.adapter_identity.to_vec(),
            binding_epoch,
            resource_identity: config.resource_identity.to_vec(),
            lifecycle_identity: config.lifecycle_identity.to_vec(),
            thread_policy_identity: config.thread_policy_identity.to_vec(),
            thread_binding_identity,
            bound_thread: thread::current().id(),
        })
    }

    pub(super) fn mint_owner(
        &self,
        slot: u64,
        generation: u64,
    ) -> Result<StagedNativeCapabilityToken, NativeCapabilityAuthorityError> {
        self.require_current_thread()?;
        let binding = self.binding(NativeCapabilityKind::Owner, None)?;
        mint(&self.secret, &binding, slot, generation)
            .map(StagedNativeCapabilityToken)
            .map_err(Into::into)
    }

    pub(super) fn authenticate_owner(
        &self,
        token: &StagedNativeCapabilityToken,
        expected_slot: u64,
        expected_generation: u64,
    ) -> Result<NativeCapabilityClaims, NativeCapabilityAuthorityError> {
        self.require_current_thread()?;
        let binding = self.binding(NativeCapabilityKind::Owner, None)?;
        authenticate_expected(
            &self.secret,
            &binding,
            &token.0,
            expected_slot,
            expected_generation,
        )
        .map_err(Into::into)
    }

    pub(super) fn mint_function_owned_result(
        &self,
        function_template_fingerprint: &[u8; 32],
        slot: u64,
        generation: u64,
    ) -> Result<StagedNativeCapabilityToken, NativeCapabilityAuthorityError> {
        self.require_current_thread()?;
        let binding = self.binding(
            NativeCapabilityKind::FunctionOwnedResult,
            Some(function_template_fingerprint),
        )?;
        mint(&self.secret, &binding, slot, generation)
            .map(StagedNativeCapabilityToken)
            .map_err(Into::into)
    }

    pub(super) fn authenticate_function_owned_result(
        &self,
        function_template_fingerprint: &[u8; 32],
        token: &StagedNativeCapabilityToken,
        expected_slot: u64,
        expected_generation: u64,
    ) -> Result<NativeCapabilityClaims, NativeCapabilityAuthorityError> {
        self.require_current_thread()?;
        let binding = self.binding(
            NativeCapabilityKind::FunctionOwnedResult,
            Some(function_template_fingerprint),
        )?;
        authenticate_expected(
            &self.secret,
            &binding,
            &token.0,
            expected_slot,
            expected_generation,
        )
        .map_err(Into::into)
    }

    fn require_current_thread(&self) -> Result<(), NativeCapabilityAuthorityError> {
        if thread::current().id() == self.bound_thread {
            Ok(())
        } else {
            Err(NativeCapabilityAuthorityError::WrongThread)
        }
    }

    fn binding<'a>(
        &'a self,
        kind: NativeCapabilityKind,
        function_template_fingerprint: Option<&'a [u8; 32]>,
    ) -> Result<NativeCapabilityBinding<'a>, NativeCapabilityAuthorityError> {
        NativeCapabilityBinding::from_trusted_runtime_binding(
            &self.physical_module_fingerprint,
            &self.adapter_identity,
            self.binding_epoch,
            kind,
            function_template_fingerprint,
            &self.resource_identity,
            &self.lifecycle_identity,
            &self.thread_policy_identity,
            &self.thread_binding_identity,
        )
        .map_err(Into::into)
    }
}

impl From<NativeCapabilityTokenError> for NativeCapabilityAuthorityError {
    fn from(value: NativeCapabilityTokenError) -> Self {
        Self::Token(value)
    }
}

#[cfg(test)]
trait EntropySource {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()>;
}

fn validate_config(
    config: &NativeCapabilityAuthorityConfig<'_>,
) -> Result<(), NativeCapabilityAuthorityError> {
    NativeCapabilityBinding::from_trusted_runtime_binding(
        config.physical_module_fingerprint,
        config.adapter_identity,
        1,
        NativeCapabilityKind::Owner,
        None,
        config.resource_identity,
        config.lifecycle_identity,
        config.thread_policy_identity,
        b"authority-config-validation",
    )
    .map(|_| ())
    .map_err(|_| NativeCapabilityAuthorityError::InvalidBinding)
}

fn encode_lower_hex(input: &[u8; THREAD_NONCE_BYTES]) -> [u8; THREAD_BINDING_HEX_BYTES] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = [0_u8; THREAD_BINDING_HEX_BYTES];
    for (index, byte) in input.iter().copied().enumerate() {
        output[index * 2] = HEX[usize::from(byte >> 4)];
        output[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_not_impl {
        ($type:ty, $trait:path) => {{
            trait AmbiguousIfImplemented<Marker> {
                fn probe() {}
            }
            impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}
            struct Implemented;
            impl<T: ?Sized + $trait> AmbiguousIfImplemented<Implemented> for T {}
            let _ = <$type as AmbiguousIfImplemented<_>>::probe;
        }};
    }

    const MODULE: [u8; 32] = [0xa5; 32];
    const OTHER_MODULE: [u8; 32] = [0x5a; 32];
    const FUNCTION: [u8; 32] = [0x3c; 32];
    const OTHER_FUNCTION: [u8; 32] = [0xc3; 32];

    fn config(module: &[u8; 32]) -> NativeCapabilityAuthorityConfig<'_> {
        NativeCapabilityAuthorityConfig {
            physical_module_fingerprint: module,
            adapter_identity: b"adapter.binding.one",
            resource_identity: b"token.type",
            lifecycle_identity: b"token.drop",
            thread_policy_identity: b"semaprax.thread-bound.v1",
        }
    }

    fn canonical_seed() -> [u8; SEED_BYTES] {
        let mut seed = [0_u8; SEED_BYTES];
        seed[..SECRET_BYTES].fill(0x11);
        seed[SECRET_BYTES..SECRET_BYTES + EPOCH_BYTES]
            .copy_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
        for (index, byte) in seed[SECRET_BYTES + EPOCH_BYTES..].iter_mut().enumerate() {
            *byte = u8::try_from(index + 1).unwrap();
        }
        seed
    }

    struct FixtureEntropy {
        seed: [u8; SEED_BYTES],
        fail: bool,
        calls: usize,
    }

    impl FixtureEntropy {
        fn valid() -> Self {
            Self {
                seed: canonical_seed(),
                fail: false,
                calls: 0,
            }
        }
    }

    impl EntropySource for FixtureEntropy {
        fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()> {
            self.calls += 1;
            if self.fail {
                destination[..7].fill(0xee);
                return Err(());
            }
            assert_eq!(destination.len(), SEED_BYTES);
            destination.copy_from_slice(&self.seed);
            Ok(())
        }
    }

    fn fixture_authority(module: &[u8; 32]) -> (NativeCapabilityAuthority, FixtureEntropy) {
        let mut entropy = FixtureEntropy::valid();
        let authority =
            NativeCapabilityAuthority::from_entropy_source(config(module), &mut entropy).unwrap();
        (authority, entropy)
    }

    #[test]
    fn deterministic_authority_owner_and_result_paths_are_exact() {
        let (authority, entropy) = fixture_authority(&MODULE);
        assert_eq!(entropy.calls, 1);

        let owner = authority.mint_owner(29, 31).unwrap();
        assert_eq!(&owner.0[8..16], &0x0102_0304_0506_0708_u64.to_le_bytes());
        assert_eq!(
            owner.0,
            [
                83, 80, 88, 67, 1, 1, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 29, 0, 0, 0, 0, 0, 0, 0, 31, 0,
                0, 0, 0, 0, 0, 0, 11, 242, 82, 255, 7, 18, 225, 221, 190, 217, 150, 23, 192, 222,
                39, 200, 72, 152, 6, 234, 90, 248, 79, 8, 172, 169, 184, 231, 7, 127, 196, 128,
            ]
        );
        assert_eq!(
            authority.authenticate_owner(&owner, 29, 31).unwrap(),
            NativeCapabilityClaims {
                slot: 29,
                generation: 31
            }
        );

        let result = authority
            .mint_function_owned_result(&FUNCTION, 37, 41)
            .unwrap();
        assert_eq!(
            result.0,
            [
                83, 80, 88, 67, 1, 2, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 37, 0, 0, 0, 0, 0, 0, 0, 41, 0,
                0, 0, 0, 0, 0, 0, 15, 143, 187, 123, 45, 168, 91, 115, 195, 110, 159, 219, 180, 2,
                234, 96, 219, 77, 97, 172, 240, 245, 219, 13, 252, 103, 218, 187, 157, 36, 123,
                197,
            ]
        );
        assert_eq!(
            authority
                .authenticate_function_owned_result(&FUNCTION, &result, 37, 41)
                .unwrap(),
            NativeCapabilityClaims {
                slot: 37,
                generation: 41
            }
        );
        assert_eq!(
            authority.authenticate_function_owned_result(&OTHER_FUNCTION, &result, 37, 41),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::AuthenticationFailed
            ))
        );
        assert_eq!(
            authority.authenticate_owner(&result, 37, 41),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::AuthenticationFailed
            ))
        );
        assert!(matches!(
            authority.mint_owner(0, 1),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::ZeroSlot
            ))
        ));
        assert!(matches!(
            authority.mint_owner(1, 0),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::ZeroGeneration
            ))
        ));
        assert!(matches!(
            authority.mint_function_owned_result(&FUNCTION, 0, 1),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::ZeroSlot
            ))
        ));
        assert!(matches!(
            authority.mint_function_owned_result(&FUNCTION, 1, 0),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::ZeroGeneration
            ))
        ));
    }

    #[test]
    fn entropy_failure_and_every_structural_zero_fail_closed_after_one_fill() {
        let mut unavailable = FixtureEntropy {
            fail: true,
            ..FixtureEntropy::valid()
        };
        assert!(matches!(
            NativeCapabilityAuthority::from_entropy_source(config(&MODULE), &mut unavailable),
            Err(NativeCapabilityAuthorityError::EntropyUnavailable)
        ));
        assert_eq!(unavailable.calls, 1);

        for range in [
            0..SECRET_BYTES,
            SECRET_BYTES..SECRET_BYTES + EPOCH_BYTES,
            SECRET_BYTES + EPOCH_BYTES..SEED_BYTES,
        ] {
            let mut entropy = FixtureEntropy::valid();
            entropy.seed[range].fill(0);
            assert!(matches!(
                NativeCapabilityAuthority::from_entropy_source(config(&MODULE), &mut entropy),
                Err(NativeCapabilityAuthorityError::InvalidEntropy)
            ));
            assert_eq!(entropy.calls, 1);
        }
    }

    #[test]
    fn invalid_static_binding_is_rejected_before_entropy_is_requested() {
        let zero_module = [0_u8; 32];
        let mut entropy = FixtureEntropy::valid();
        assert!(matches!(
            NativeCapabilityAuthority::from_entropy_source(config(&zero_module), &mut entropy),
            Err(NativeCapabilityAuthorityError::InvalidBinding)
        ));
        assert_eq!(entropy.calls, 0);

        let mut invalid = config(&MODULE);
        invalid.adapter_identity = b"bad\0adapter";
        assert!(matches!(
            NativeCapabilityAuthority::from_entropy_source(invalid, &mut entropy),
            Err(NativeCapabilityAuthorityError::InvalidBinding)
        ));
        assert_eq!(entropy.calls, 0);

        for dimension in 0..4 {
            let mut invalid = config(&MODULE);
            match dimension {
                0 => invalid.adapter_identity = b"",
                1 => invalid.resource_identity = b"bad\0resource",
                2 => invalid.lifecycle_identity = b"",
                3 => invalid.thread_policy_identity = b"bad\0thread-policy",
                _ => unreachable!(),
            }
            assert!(matches!(
                NativeCapabilityAuthority::from_entropy_source(invalid, &mut entropy),
                Err(NativeCapabilityAuthorityError::InvalidBinding)
            ));
            assert_eq!(entropy.calls, 0);
        }
    }

    #[test]
    fn actual_thread_is_checked_before_every_mint_and_authentication_path() {
        let (authority, _) = fixture_authority(&MODULE);
        let owner = authority.mint_owner(3, 5).unwrap();
        let result = authority
            .mint_function_owned_result(&FUNCTION, 7, 11)
            .unwrap();

        thread::scope(|scope| {
            scope.spawn(|| {
                let structurally_invalid = StagedNativeCapabilityToken([0_u8; TOKEN_BYTES]);
                assert!(matches!(
                    authority.mint_owner(3, 5),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                ));
                assert!(matches!(
                    authority.mint_owner(0, 0),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                ));
                assert_eq!(
                    authority.authenticate_owner(&owner, 3, 5),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                );
                assert_eq!(
                    authority.authenticate_owner(&structurally_invalid, 3, 5),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                );
                assert!(matches!(
                    authority.mint_function_owned_result(&FUNCTION, 7, 11),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                ));
                assert!(matches!(
                    authority.mint_function_owned_result(&FUNCTION, 0, 0),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                ));
                assert_eq!(
                    authority.authenticate_function_owned_result(&FUNCTION, &result, 7, 11),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                );
                assert_eq!(
                    authority.authenticate_function_owned_result(
                        &FUNCTION,
                        &structurally_invalid,
                        7,
                        11,
                    ),
                    Err(NativeCapabilityAuthorityError::WrongThread)
                );
            });
        });

        assert!(authority.authenticate_owner(&owner, 3, 5).is_ok());
        assert!(authority
            .authenticate_function_owned_result(&FUNCTION, &result, 7, 11)
            .is_ok());
    }

    #[test]
    fn every_random_binding_dimension_and_physical_module_is_authenticated() {
        let (first, _) = fixture_authority(&MODULE);
        let (other_module, _) = fixture_authority(&OTHER_MODULE);
        let token = first.mint_owner(13, 17).unwrap();

        assert!(matches!(
            other_module.authenticate_owner(&token, 13, 17),
            Err(NativeCapabilityAuthorityError::Token(
                NativeCapabilityTokenError::AuthenticationFailed
            ))
        ));

        for offset in [0, SECRET_BYTES, SECRET_BYTES + EPOCH_BYTES] {
            let mut entropy = FixtureEntropy::valid();
            entropy.seed[offset] ^= 0x80;
            let replacement =
                NativeCapabilityAuthority::from_entropy_source(config(&MODULE), &mut entropy)
                    .unwrap();
            assert!(matches!(
                replacement.authenticate_owner(&token, 13, 17),
                Err(NativeCapabilityAuthorityError::Token(
                    NativeCapabilityTokenError::AuthenticationFailed
                ))
            ));
        }

        let contexts = [
            (
                b"adapter.binding.two".as_slice(),
                b"token.type".as_slice(),
                b"token.drop".as_slice(),
                b"semaprax.thread-bound.v1".as_slice(),
            ),
            (
                b"adapter.binding.one".as_slice(),
                b"other.type".as_slice(),
                b"token.drop".as_slice(),
                b"semaprax.thread-bound.v1".as_slice(),
            ),
            (
                b"adapter.binding.one".as_slice(),
                b"token.type".as_slice(),
                b"other.drop".as_slice(),
                b"semaprax.thread-bound.v1".as_slice(),
            ),
            (
                b"adapter.binding.one".as_slice(),
                b"token.type".as_slice(),
                b"token.drop".as_slice(),
                b"semaprax.other-thread-policy.v1".as_slice(),
            ),
        ];
        for (adapter, resource, lifecycle, thread_policy) in contexts {
            let mut changed = config(&MODULE);
            changed.adapter_identity = adapter;
            changed.resource_identity = resource;
            changed.lifecycle_identity = lifecycle;
            changed.thread_policy_identity = thread_policy;
            let mut entropy = FixtureEntropy::valid();
            let replacement =
                NativeCapabilityAuthority::from_entropy_source(changed, &mut entropy).unwrap();
            assert!(matches!(
                replacement.authenticate_owner(&token, 13, 17),
                Err(NativeCapabilityAuthorityError::Token(
                    NativeCapabilityTokenError::AuthenticationFailed
                ))
            ));
        }
    }

    #[test]
    fn catastrophic_full_entropy_repeat_is_an_explicit_nonclaim() {
        let (first, _) = fixture_authority(&MODULE);
        let (repeated, _) = fixture_authority(&MODULE);
        let token = first.mint_owner(19, 23).unwrap();

        // Exact RNG+context repetition produces the same authority. Production
        // safety is conditional on the operating-system CSPRNG, not a proof of
        // mathematical uniqueness or a substitute for module retention.
        assert!(repeated.authenticate_owner(&token, 19, 23).is_ok());
    }

    #[test]
    fn os_authority_smoke_uses_the_native_entropy_path() {
        let authority = NativeCapabilityAuthority::from_os(config(&MODULE)).unwrap();
        let token = authority.mint_owner(43, 47).unwrap();
        assert!(authority.authenticate_owner(&token, 43, 47).is_ok());
    }

    #[test]
    fn authority_auto_traits_are_deliberate_for_dynamic_thread_rejection() {
        fn assert_send_and_sync<T: Send + Sync>() {}
        assert_send_and_sync::<NativeCapabilityAuthority>();
    }

    #[test]
    fn secrets_authorities_and_credentials_exclude_copying_and_formatting_traits() {
        assert_not_impl!(NativeCapabilitySecret, Clone);
        assert_not_impl!(NativeCapabilitySecret, std::fmt::Debug);
        assert_not_impl!(NativeCapabilitySecret, std::fmt::Display);
        assert_not_impl!(NativeCapabilityAuthority, Clone);
        assert_not_impl!(NativeCapabilityAuthority, std::fmt::Debug);
        assert_not_impl!(NativeCapabilityAuthority, std::fmt::Display);
        assert_not_impl!(NativeCapabilityAuthority, Default);
        assert_not_impl!(StagedNativeCapabilityToken, Clone);
        assert_not_impl!(StagedNativeCapabilityToken, Copy);
        assert_not_impl!(StagedNativeCapabilityToken, std::fmt::Debug);
        assert_not_impl!(StagedNativeCapabilityToken, std::fmt::Display);
        assert_not_impl!(StagedNativeCapabilityToken, Default);
    }

    #[test]
    fn stable_errors_do_not_format_credentials_or_binding_context() {
        let known_secret_hex = "1111111111111111111111111111111111111111111111111111111111111111";
        let known_thread_binding =
            "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        for error in [
            NativeCapabilityAuthorityError::EntropyUnavailable,
            NativeCapabilityAuthorityError::InvalidEntropy,
            NativeCapabilityAuthorityError::InvalidBinding,
            NativeCapabilityAuthorityError::WrongThread,
            NativeCapabilityAuthorityError::Token(NativeCapabilityTokenError::AuthenticationFailed),
        ] {
            let rendered = format!("{error:?}");
            for sensitive in [
                known_secret_hex,
                known_thread_binding,
                "adapter.binding.one",
                "token.type",
                "ThreadId",
                "SPXC",
            ] {
                assert!(!rendered.contains(sensitive));
            }
        }
    }

    #[test]
    fn lower_hex_thread_binding_is_canonical_and_nul_free() {
        let mut input = [0_u8; THREAD_NONCE_BYTES];
        input[0] = 0x01;
        input[1] = 0xaf;
        input[31] = 0xfe;
        let encoded = encode_lower_hex(&input);
        assert_eq!(&encoded[..4], b"01af");
        assert_eq!(&encoded[62..], b"fe");
        assert!(encoded
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)));
        assert!(!encoded.contains(&0));
    }
}
