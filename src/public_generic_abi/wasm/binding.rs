//! `WasmProviderBindingV1`: the physical-layer binding artifact the Core
//! Wasm adapter's `WasmProvider::open` replays against, layered on top of
//! (never modifying) [`crate::public_generic_abi::carrier::CarrierBindingV1`].
//!
//! Mirrors [`crate::public_generic_abi::native::binding::NativeProviderBindingV1`]'s
//! convention exactly — framed fields, a domain-separated digest never
//! transmitted, byte-exact `replay` — for `TargetProfile::CoreWasm` instead
//! of `TargetProfile::NativeC11`. A native binding names a C symbol; a Wasm
//! binding names an export name instead, since Wasm has no linker-visible
//! "symbol" the way a native shared object does. The two bindings are
//! deliberately independent, physical-layer-only siblings: one never wraps
//! or references the other.

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::{digest, frame, read_frame};

/// The versioned Wasm-adapter binding schema.
pub const WASM_ADAPTER_SCHEMA: &str = "semaprax.public-generic-wasm-adapter.v1";

const BINDING_DOMAIN: &[u8] = b"semaprax.public-generic-wasm-adapter.v1.binding\0";

/// Malformed Wasm provider binding bytes: framing, an unknown ABI version,
/// an unrecognized support/publication claim, or an embedded carrier
/// binding not naming `TargetProfile::CoreWasm`.
pub const MALFORMED_WASM_BINDING: &str = "SPX-PG910";
/// Independent replay found the recomputed Wasm binding preimage does not
/// equal the submitted one, or the embedded `CarrierBindingV1` differs.
pub const WASM_BINDING_REPLAY_MISMATCH: &str = "SPX-PG911";

/// The only admitted Wasm adapter ABI version in this round. A binding
/// naming any other string is malformed, not merely unsupported: the
/// generator that emits `WasmProviderBindingV1` values never produces
/// another version yet.
pub const WASM_ADAPTER_ABI_VERSION: &str = "v1";

/// A closed support/publication claim, independent of
/// [`crate::public_generic_abi::native::binding::SupportPublicationState`]
/// so the two physical adapters never share mutable state through a common
/// type. This repository's invariant is that public generic ownership
/// remains unsupported and unpublished; the type is closed to exactly that
/// value so an emitted binding can never claim otherwise by construction,
/// and a decoded claim naming anything else is rejected rather than
/// silently accepted.
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

/// The Core Wasm physical provider's binding artifact: a wrapped
/// [`CarrierBindingV1`] (descriptor identity + `TargetProfile::CoreWasm` +
/// carrier schema + opaque runtime identity) plus the physical-layer facts
/// above it: the Wasm adapter's own ABI version, the exact generated
/// provider artifact's digest, the Wasm export name of the one admitted
/// endpoint, a compiler-backend identity fact, and the closed
/// support/publication claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WasmProviderBindingV1 {
    schema: String,
    carrier_binding: CarrierBindingV1,
    wasm_adapter_abi_version: String,
    provider_artifact_digest: String,
    exported_endpoint_export_name: String,
    compiler_backend_version: String,
    support_publication_state: SupportPublicationState,
}

impl WasmProviderBindingV1 {
    /// `carrier_binding` must already name [`TargetProfile::CoreWasm`]; a
    /// binding for another target profile is a caller error, not a runtime
    /// fact this constructor decides — callers pick the wrapped binding.
    pub fn new(
        carrier_binding: CarrierBindingV1,
        provider_artifact_digest: impl Into<String>,
        exported_endpoint_export_name: impl Into<String>,
        compiler_backend_version: impl Into<String>,
    ) -> Self {
        Self {
            schema: WASM_ADAPTER_SCHEMA.to_owned(),
            carrier_binding,
            wasm_adapter_abi_version: WASM_ADAPTER_ABI_VERSION.to_owned(),
            provider_artifact_digest: provider_artifact_digest.into(),
            exported_endpoint_export_name: exported_endpoint_export_name.into(),
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

    pub fn exported_endpoint_export_name(&self) -> &str {
        &self.exported_endpoint_export_name
    }

    pub fn support_publication_state(&self) -> SupportPublicationState {
        self.support_publication_state
    }

    fn preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::new();
        frame(&mut preimage, self.schema.as_bytes());
        frame(&mut preimage, &self.carrier_binding.encode());
        frame(&mut preimage, self.wasm_adapter_abi_version.as_bytes());
        frame(&mut preimage, self.provider_artifact_digest.as_bytes());
        frame(&mut preimage, self.exported_endpoint_export_name.as_bytes());
        frame(&mut preimage, self.compiler_backend_version.as_bytes());
        frame(
            &mut preimage,
            self.support_publication_state.text().as_bytes(),
        );
        preimage
    }

    /// The domain-separated binding digest. Never transmitted; always
    /// recomputed by [`replay_wasm_provider_binding`].
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
        MALFORMED_WASM_BINDING,
        format!("not a canonical {WASM_ADAPTER_SCHEMA} binding: {subject}"),
    )
}

/// Parse well-formed wire bytes into a [`WasmProviderBindingV1`]. Framing,
/// schema, embedded carrier-binding, ABI-version, and support/publication
/// claim validity only; binding validation against a trusted context is
/// [`replay_wasm_provider_binding`]'s job.
pub fn decode_wasm_provider_binding(bytes: &[u8]) -> Result<WasmProviderBindingV1, Diagnostic> {
    if bytes.len() > MAX_BINDING_WIRE_BYTES {
        return Err(Diagnostic::io(
            MALFORMED_WASM_BINDING,
            format!("{WASM_ADAPTER_SCHEMA} exceeded its total wire-byte bound"),
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
    if schema != WASM_ADAPTER_SCHEMA {
        return Err(malformed("unknown Wasm adapter binding schema"));
    }
    let (carrier_binding_bytes, next_offset) = read_frame(bytes, offset, MAX_BINDING_WIRE_BYTES)
        .ok_or_else(|| malformed("truncated or oversized carrier_binding field"))?;
    offset = next_offset;
    let carrier_binding = crate::public_generic_abi::carrier::decode_binding(carrier_binding_bytes)
        .map_err(|_| malformed("embedded carrier binding does not decode"))?;
    if carrier_binding.target_profile() != TargetProfile::CoreWasm {
        return Err(malformed(
            "embedded carrier binding does not name TargetProfile::CoreWasm",
        ));
    }

    let wasm_adapter_abi_version = next_string("wasm_adapter_abi_version", &mut offset)?;
    if wasm_adapter_abi_version != WASM_ADAPTER_ABI_VERSION {
        return Err(malformed("unknown Wasm adapter ABI version"));
    }
    let provider_artifact_digest = next_string("provider_artifact_digest", &mut offset)?;
    let exported_endpoint_export_name = next_string("exported_endpoint_export_name", &mut offset)?;
    let compiler_backend_version = next_string("compiler_backend_version", &mut offset)?;
    let support_publication_text = next_string("support_publication_state", &mut offset)?;
    let support_publication_state =
        SupportPublicationState::from_text(&support_publication_text)
            .ok_or_else(|| malformed("unknown support/publication claim"))?;

    if offset != bytes.len() {
        return Err(malformed("trailing bytes after the Wasm provider binding"));
    }

    Ok(WasmProviderBindingV1 {
        schema,
        carrier_binding,
        wasm_adapter_abi_version,
        provider_artifact_digest,
        exported_endpoint_export_name,
        compiler_backend_version,
        support_publication_state,
    })
}

/// Decode `candidate` and require its preimage to equal `trusted`'s,
/// byte-for-byte. A binding for one descriptor, target, provider artifact,
/// or endpoint is never accepted against another.
pub fn replay_wasm_provider_binding(
    candidate: &[u8],
    trusted: &WasmProviderBindingV1,
) -> Result<WasmProviderBindingV1, Diagnostic> {
    let decoded = decode_wasm_provider_binding(candidate)?;
    if decoded.preimage() != trusted.preimage() {
        return Err(Diagnostic::io(
            WASM_BINDING_REPLAY_MISMATCH,
            format!("{WASM_ADAPTER_SCHEMA} independent replay does not match the trusted value"),
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests;
