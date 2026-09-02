//! Private OS-backed authority for staged native capability-token mechanics.
//!
//! This authority is deliberately unreachable from compiler preflight and has
//! no public or C-facing API. In tests it additionally requires a fake-backed
//! module lease and retains that exact allocation through every staged
//! credential wrapper. It is not a callable adapter, ownership ledger,
//! platform loader handle, physical module pin, or unload protocol.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable native adapter authority remains gated")
)]

use std::thread::{self, ThreadId};

use super::native_capability_token::{
    authenticate_expected, mint, NativeCapabilityBinding, NativeCapabilityClaims,
    NativeCapabilityKind, NativeCapabilitySecret, NativeCapabilityTokenError, TOKEN_BYTES,
};
use super::native_module_lease::{NativeModuleLease, NativeModuleLeaseError};

const SECRET_BYTES: usize = 32;
const EPOCH_BYTES: usize = 8;
const THREAD_NONCE_BYTES: usize = 32;
const SEED_BYTES: usize = SECRET_BYTES + EPOCH_BYTES + THREAD_NONCE_BYTES;
const THREAD_BINDING_HEX_BYTES: usize = THREAD_NONCE_BYTES * 2;

/// Immutable semantic scope copied into one runtime binding.
///
/// The physical-module fingerprint is intentionally available only through
/// the exact module-instance lease. Construction remains private to the future
/// retained adapter. Identities use the same canonical nonempty, NUL-free form
/// as the token codec.
pub(super) struct NativeCapabilityAuthorityConfig<'a> {
    pub(super) module_lease: NativeModuleLease,
    pub(super) adapter_identity: &'a [u8],
    pub(super) resource_identity: &'a [u8],
    pub(super) lifecycle_identity: &'a [u8],
    pub(super) thread_policy_identity: &'a [u8],
}

/// One binding-instance authority.
///
/// The type is intentionally neither `Clone` nor `Debug`. Tokens remain
/// copyable bearer bytes, so the future synchronized ledger must still enforce
/// generation liveness, replay rejection, and exactly-once consumption. The
/// lease topology proves strong-reference retention against a fake pin only;
/// it does not prove that executable code remains mapped.
pub(super) struct NativeCapabilityAuthority {
    secret: NativeCapabilitySecret,
    module_lease: NativeModuleLease,
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
pub(super) struct StagedNativeCapabilityToken {
    bytes: [u8; TOKEN_BYTES],
    module_lease: NativeModuleLease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeCapabilityAuthorityError {
    EntropyUnavailable,
    InvalidEntropy,
    InvalidBinding,
    WrongThread,
    ModuleLease(NativeModuleLeaseError),
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
            module_lease: config.module_lease,
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
        let module_lease = self.module_lease.retain_current_process()?;
        let binding = self.binding(NativeCapabilityKind::Owner, None)?;
        mint(&self.secret, &binding, slot, generation)
            .map(|bytes| StagedNativeCapabilityToken {
                bytes,
                module_lease,
            })
            .map_err(Into::into)
    }

    pub(super) fn authenticate_owner(
        &self,
        token: &StagedNativeCapabilityToken,
        expected_slot: u64,
        expected_generation: u64,
    ) -> Result<NativeCapabilityClaims, NativeCapabilityAuthorityError> {
        self.require_current_thread()?;
        let operation_lease = self.module_lease.retain_current_process()?;
        if !operation_lease.is_same_instance(&token.module_lease) {
            return Err(NativeCapabilityAuthorityError::ModuleLease(
                NativeModuleLeaseError::WrongModuleInstance,
            ));
        }
        let binding = self.binding(NativeCapabilityKind::Owner, None)?;
        authenticate_expected(
            &self.secret,
            &binding,
            &token.bytes,
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
        let module_lease = self.module_lease.retain_current_process()?;
        let binding = self.binding(
            NativeCapabilityKind::FunctionOwnedResult,
            Some(function_template_fingerprint),
        )?;
        mint(&self.secret, &binding, slot, generation)
            .map(|bytes| StagedNativeCapabilityToken {
                bytes,
                module_lease,
            })
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
        let operation_lease = self.module_lease.retain_current_process()?;
        if !operation_lease.is_same_instance(&token.module_lease) {
            return Err(NativeCapabilityAuthorityError::ModuleLease(
                NativeModuleLeaseError::WrongModuleInstance,
            ));
        }
        let binding = self.binding(
            NativeCapabilityKind::FunctionOwnedResult,
            Some(function_template_fingerprint),
        )?;
        authenticate_expected(
            &self.secret,
            &binding,
            &token.bytes,
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
            self.module_lease.physical_module_fingerprint(),
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

impl From<NativeModuleLeaseError> for NativeCapabilityAuthorityError {
    fn from(value: NativeModuleLeaseError) -> Self {
        Self::ModuleLease(value)
    }
}

#[cfg(test)]
trait EntropySource {
    fn fill(&mut self, destination: &mut [u8]) -> Result<(), ()>;
}

fn validate_config(
    config: &NativeCapabilityAuthorityConfig<'_>,
) -> Result<(), NativeCapabilityAuthorityError> {
    let validation_lease = config.module_lease.retain_current_process()?;
    NativeCapabilityBinding::from_trusted_runtime_binding(
        validation_lease.physical_module_fingerprint(),
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
#[path = "native_capability_authority/tests.rs"]
mod tests;
