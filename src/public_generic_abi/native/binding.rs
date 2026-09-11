//! `NativeProviderBindingV1`: the physical-layer binding artifact the native
//! C11 adapter's `spx_pg_provider_open_v1` replays against, layered on top
//! of (never modifying) [`crate::public_generic_abi::carrier::CarrierBindingV1`].
//!
//! A [`crate::public_generic_abi::carrier::CarrierBindingV1`] already binds
//! a descriptor identity, a `TargetProfile`, the carrier schema version, and
//! an opaque `runtime_identity` — everything the LOGICAL carrier needs. The
//! PHYSICAL native adapter additionally needs facts no logical carrier
//! should ever carry: the native adapter's own ABI version, the exact
//! generated provider artifact's digest, the C symbol identity of the one
//! admitted endpoint, and a closed support/publication claim. This module
//! adds exactly those facts and folds the wrapped `CarrierBindingV1` into
//! its own preimage unchanged, so a binding for one descriptor, target,
//! provider artifact, or endpoint is never accepted against another.

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::{digest, frame, read_frame};

/// The versioned native-adapter binding schema.
pub const NATIVE_ADAPTER_SCHEMA: &str = "semaprax.public-generic-native-adapter.v1";

const BINDING_DOMAIN: &[u8] = b"semaprax.public-generic-native-adapter.v1.binding\0";

/// Malformed native provider binding bytes: framing, an unknown ABI
/// version, or an unrecognized support/publication claim.
pub const MALFORMED_NATIVE_BINDING: &str = "SPX-PG901";
/// Independent replay found the recomputed native binding preimage does not
/// equal the submitted one, or the embedded `CarrierBindingV1` differs.
pub const NATIVE_BINDING_REPLAY_MISMATCH: &str = "SPX-PG902";

/// The only admitted native adapter ABI version in this round. A binding
/// naming any other string is malformed, not merely unsupported: the
/// generator that emits `NativeProviderBindingV1` values never produces
/// another version yet.
pub const NATIVE_ADAPTER_ABI_VERSION: &str = "v1";

/// A closed support/publication claim. This repository's invariant is that
/// public generic ownership remains unsupported and unpublished; the type is
/// closed to exactly that value so an emitted binding can never claim
/// otherwise by construction, and a decoded claim naming anything else is
/// rejected rather than silently accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportPublicationState {
    UnsupportedUnpublished,
}

impl SupportPublicationState {
    fn text(self) -> &'static str {
        match self {
            Self::UnsupportedUnpublished => "unsupported-unpublished",
        }
    }

    fn from_text(text: &str) -> Option<Self> {
        match text {
            "unsupported-unpublished" => Some(Self::UnsupportedUnpublished),
            _ => None,
        }
    }
}

/// The native C11 physical provider's binding artifact: a wrapped
/// [`CarrierBindingV1`] (descriptor identity + `TargetProfile::NativeC11` +
/// carrier schema + opaque runtime identity) plus the physical-layer facts
/// above it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeProviderBindingV1 {
    schema: String,
    carrier_binding: CarrierBindingV1,
    native_adapter_abi_version: String,
    provider_artifact_digest: String,
    exported_endpoint_symbol: String,
    compiler_backend_version: String,
    support_publication_state: SupportPublicationState,
}

impl NativeProviderBindingV1 {
    /// `carrier_binding` must already name [`TargetProfile::NativeC11`]; a
    /// binding for another target profile is a caller error, not a runtime
    /// fact this constructor decides — callers pick the wrapped binding.
    pub fn new(
        carrier_binding: CarrierBindingV1,
        provider_artifact_digest: impl Into<String>,
        exported_endpoint_symbol: impl Into<String>,
        compiler_backend_version: impl Into<String>,
    ) -> Self {
        Self {
            schema: NATIVE_ADAPTER_SCHEMA.to_owned(),
            carrier_binding,
            native_adapter_abi_version: NATIVE_ADAPTER_ABI_VERSION.to_owned(),
            provider_artifact_digest: provider_artifact_digest.into(),
            exported_endpoint_symbol: exported_endpoint_symbol.into(),
            compiler_backend_version: compiler_backend_version.into(),
            support_publication_state: SupportPublicationState::UnsupportedUnpublished,
        }
    }

    pub fn carrier_binding(&self) -> &CarrierBindingV1 {
        &self.carrier_binding
    }

    pub fn target_profile(&self) -> TargetProfile {
        self.carrier_binding.target_profile()
    }

    pub fn provider_artifact_digest(&self) -> &str {
        &self.provider_artifact_digest
    }

    pub fn exported_endpoint_symbol(&self) -> &str {
        &self.exported_endpoint_symbol
    }

    pub fn support_publication_state(&self) -> SupportPublicationState {
        self.support_publication_state
    }

    fn preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::new();
        frame(&mut preimage, self.schema.as_bytes());
        frame(&mut preimage, &self.carrier_binding.encode());
        frame(&mut preimage, self.native_adapter_abi_version.as_bytes());
        frame(&mut preimage, self.provider_artifact_digest.as_bytes());
        frame(&mut preimage, self.exported_endpoint_symbol.as_bytes());
        frame(&mut preimage, self.compiler_backend_version.as_bytes());
        frame(
            &mut preimage,
            self.support_publication_state.text().as_bytes(),
        );
        preimage
    }

    /// The domain-separated binding digest. Never transmitted; always
    /// recomputed by [`replay_native_provider_binding`].
    pub fn binding_digest(&self) -> String {
        digest(BINDING_DOMAIN, &self.preimage())
    }

    /// Canonical wire bytes: the identity preimage, matching
    /// `CarrierBindingV1::encode`'s own convention.
    pub fn encode(&self) -> Vec<u8> {
        self.preimage()
    }
}

const MAX_BINDING_FIELD_BYTES: usize = 64 * 1024;
const MAX_BINDING_WIRE_BYTES: usize = 256 * 1024;

fn malformed(subject: &str) -> Diagnostic {
    Diagnostic::io(
        MALFORMED_NATIVE_BINDING,
        format!("not a canonical {NATIVE_ADAPTER_SCHEMA} binding: {subject}"),
    )
}

/// Parse well-formed wire bytes into a [`NativeProviderBindingV1`]. Framing,
/// schema, embedded carrier-binding, ABI-version, and support/publication
/// claim validity only; binding validation against a trusted context is
/// [`replay_native_provider_binding`]'s job.
pub fn decode_native_provider_binding(bytes: &[u8]) -> Result<NativeProviderBindingV1, Diagnostic> {
    if bytes.len() > MAX_BINDING_WIRE_BYTES {
        return Err(Diagnostic::io(
            MALFORMED_NATIVE_BINDING,
            format!("{NATIVE_ADAPTER_SCHEMA} exceeded its total wire-byte bound"),
        ));
    }
    let mut offset = 0usize;

    let next_string = |name: &'static str, offset: &mut usize| -> Result<String, Diagnostic> {
        let (field, next_offset) = read_frame(bytes, *offset, MAX_BINDING_FIELD_BYTES)
            .ok_or_else(|| malformed(&format!("truncated or oversized {name} field")))?;
        *offset = next_offset;
        String::from_utf8(field.to_vec()).map_err(|_| malformed(&format!("{name} is not UTF-8")))
    };

    let schema = next_string("schema", &mut offset)?;
    if schema != NATIVE_ADAPTER_SCHEMA {
        return Err(malformed("unknown native adapter binding schema"));
    }
    let (carrier_binding_bytes, next_offset) = read_frame(bytes, offset, MAX_BINDING_WIRE_BYTES)
        .ok_or_else(|| malformed("truncated or oversized carrier_binding field"))?;
    offset = next_offset;
    let carrier_binding = crate::public_generic_abi::carrier::decode_binding(carrier_binding_bytes)
        .map_err(|_| malformed("embedded carrier binding does not decode"))?;
    if carrier_binding.target_profile() != TargetProfile::NativeC11 {
        return Err(malformed(
            "embedded carrier binding does not name TargetProfile::NativeC11",
        ));
    }

    let native_adapter_abi_version = next_string("native_adapter_abi_version", &mut offset)?;
    if native_adapter_abi_version != NATIVE_ADAPTER_ABI_VERSION {
        return Err(malformed("unknown native adapter ABI version"));
    }
    let provider_artifact_digest = next_string("provider_artifact_digest", &mut offset)?;
    let exported_endpoint_symbol = next_string("exported_endpoint_symbol", &mut offset)?;
    let compiler_backend_version = next_string("compiler_backend_version", &mut offset)?;
    let support_publication_text = next_string("support_publication_state", &mut offset)?;
    let support_publication_state =
        SupportPublicationState::from_text(&support_publication_text)
            .ok_or_else(|| malformed("unknown support/publication claim"))?;

    if offset != bytes.len() {
        return Err(malformed(
            "trailing bytes after the native provider binding",
        ));
    }

    Ok(NativeProviderBindingV1 {
        schema,
        carrier_binding,
        native_adapter_abi_version,
        provider_artifact_digest,
        exported_endpoint_symbol,
        compiler_backend_version,
        support_publication_state,
    })
}

/// Decode `candidate` and require its preimage to equal `trusted`'s,
/// byte-for-byte. A binding for one descriptor, target, provider artifact,
/// or endpoint is never accepted against another.
pub fn replay_native_provider_binding(
    candidate: &[u8],
    trusted: &NativeProviderBindingV1,
) -> Result<NativeProviderBindingV1, Diagnostic> {
    let decoded = decode_native_provider_binding(candidate)?;
    if decoded.preimage() != trusted.preimage() {
        return Err(Diagnostic::io(
            NATIVE_BINDING_REPLAY_MISMATCH,
            format!("{NATIVE_ADAPTER_SCHEMA} independent replay does not match the trusted value"),
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests;
