//! Private authenticated capability-token codec for future native adapters.
//!
//! The codec is deliberately disconnected from compiler resource preflight and
//! exports no C API. Its only production caller is a private authority that
//! obtains keying material from the operating system and seals the observed
//! thread binding. Compiler preflight never constructs either type.

#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable native adapter authority remains gated")
)]

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const TOKEN_MAGIC: &[u8; 4] = b"SPXC";
const TOKEN_VERSION: u8 = 1;
const TOKEN_BODY_BYTES: usize = 32;
const TOKEN_TAG_BYTES: usize = 32;
pub(super) const TOKEN_BYTES: usize = TOKEN_BODY_BYTES + TOKEN_TAG_BYTES;
const TOKEN_AUTHENTICATION_DOMAIN: &[u8] = b"semaprax.native-capability-token.v1\0";

type HmacSha256 = Hmac<Sha256>;

const MAGIC_OFFSET: usize = 0;
const VERSION_OFFSET: usize = 4;
const KIND_OFFSET: usize = 5;
const RESERVED_OFFSET: usize = 6;
const EPOCH_OFFSET: usize = 8;
const SLOT_OFFSET: usize = 16;
const GENERATION_OFFSET: usize = 24;

/// Full-width secret key supplied only by the private runtime authority.
///
/// This type is intentionally neither `Clone` nor `Debug`. The constructor
/// checks only the codec's structural minimum (not all zero); the authority is
/// responsible for sourcing these bytes directly from the operating system.
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
#[path = "native_capability_token/tests.rs"]
mod tests;
