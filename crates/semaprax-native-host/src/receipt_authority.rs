//! Host-only receipt authentication and per-frame provider challenges.
//!
//! This authority is deliberately distinct from owner capability authority.
//! One and only one 64-byte operating-system fill creates it: the first half
//! is the receipt/MAC key and the second is the exact-instance binding. Invalid
//! entropy fails closed; there is no retry or deterministic fallback.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable-v3 physical settlement remains private and unwired"
)]

use std::num::NonZeroU64;

use crate::callable_wire_v3::{
    CandidateReceipt, HostCommittedReceipt, ReceiptMacKey, WireError, HOST_RECEIPT_BYTES,
};

const KEY_BYTES: usize = 32;
const INSTANCE_BYTES: usize = 32;
const SEED_BYTES: usize = KEY_BYTES + INSTANCE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReceiptAuthorityError {
    EntropyUnavailable,
    InvalidEntropy,
    WrongCallContract,
    Authentication,
}

/// Unformattable host-only key authority for one exact admitted image.
///
/// There is intentionally no `Clone` or `Debug` implementation. The wrapped
/// key is zeroized by `ReceiptMacKey::drop`.
pub(crate) struct ReceiptAuthority {
    key: ReceiptMacKey,
    instance_binding: [u8; INSTANCE_BYTES],
    instance_nonce: NonZeroU64,
}

impl ReceiptAuthority {
    pub(crate) fn from_os(instance_nonce: NonZeroU64) -> Result<Self, ReceiptAuthorityError> {
        Self::from_fill(instance_nonce, getrandom::fill)
    }

    fn from_fill(
        instance_nonce: NonZeroU64,
        mut fill: impl FnMut(&mut [u8]) -> Result<(), getrandom::Error>,
    ) -> Result<Self, ReceiptAuthorityError> {
        let mut seed = [0_u8; SEED_BYTES];
        if fill(&mut seed).is_err() {
            seed.fill(0);
            return Err(ReceiptAuthorityError::EntropyUnavailable);
        }
        Self::from_seed(instance_nonce, &mut seed)
    }

    fn from_seed(
        instance_nonce: NonZeroU64,
        seed: &mut [u8; SEED_BYTES],
    ) -> Result<Self, ReceiptAuthorityError> {
        let mut key_bytes = [0_u8; KEY_BYTES];
        key_bytes.copy_from_slice(&seed[..KEY_BYTES]);
        let mut instance_binding = [0_u8; INSTANCE_BYTES];
        instance_binding.copy_from_slice(&seed[KEY_BYTES..]);
        seed.fill(0);

        if key_bytes == [0; KEY_BYTES]
            || instance_binding == [0; INSTANCE_BYTES]
            || key_bytes == instance_binding
        {
            key_bytes.fill(0);
            instance_binding.fill(0);
            return Err(ReceiptAuthorityError::InvalidEntropy);
        }
        let key = ReceiptMacKey::from_runtime_bytes(key_bytes)
            .map_err(|_| ReceiptAuthorityError::InvalidEntropy)?;
        key_bytes.fill(0);
        Ok(Self {
            key,
            instance_binding,
            instance_nonce,
        })
    }

    pub(crate) const fn instance_binding(&self) -> [u8; INSTANCE_BYTES] {
        self.instance_binding
    }

    pub(crate) const fn instance_nonce(&self) -> NonZeroU64 {
        self.instance_nonce
    }

    /// Derive the non-provider-selectable challenge for one reserved frame.
    pub(crate) fn provider_challenge(
        &self,
        call_contract: [u8; 32],
        invocation: NonZeroU64,
        frame_generation: NonZeroU64,
    ) -> Result<[u8; 32], ReceiptAuthorityError> {
        if call_contract == [0; 32] {
            return Err(ReceiptAuthorityError::WrongCallContract);
        }
        self.key
            .provider_challenge(
                self.instance_binding,
                self.instance_nonce,
                call_contract,
                invocation,
                frame_generation,
            )
            .map_err(ReceiptAuthorityError::from)
    }

    pub(crate) fn authenticate_receipt(
        &self,
        candidate: &CandidateReceipt,
        candidate_digest: [u8; 32],
        ledger_before: [u8; 32],
        ledger_after: [u8; 32],
    ) -> Result<[u8; HOST_RECEIPT_BYTES], ReceiptAuthorityError> {
        let receipt = HostCommittedReceipt::authenticate(
            &self.key,
            self.instance_binding,
            candidate,
            candidate_digest,
            ledger_before,
            ledger_after,
        )?;
        receipt.encode_fixed().map_err(ReceiptAuthorityError::from)
    }

    pub(crate) fn verify_receipt(
        &self,
        bytes: &[u8],
        descriptor: &crate::descriptor_v3::Descriptor,
        candidate: &CandidateReceipt,
        candidate_digest: [u8; 32],
        ledger_before: [u8; 32],
        ledger_after: [u8; 32],
    ) -> Result<HostCommittedReceipt, ReceiptAuthorityError> {
        HostCommittedReceipt::parse_and_verify_precomputed(
            bytes,
            &self.key,
            descriptor,
            self.instance_binding,
            candidate,
            candidate_digest,
            ledger_before,
            ledger_after,
        )
        .map_err(ReceiptAuthorityError::from)
    }
}

impl From<WireError> for ReceiptAuthorityError {
    fn from(_: WireError) -> Self {
        Self::Authentication
    }
}

#[cfg(test)]
#[path = "receipt_authority/tests.rs"]
mod tests;
