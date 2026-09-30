//! Private additive physical profile `semaprax.authenticated-native-identity.v1`.
//!
//! Unlike the predecessor reference renderer, this entry point requires a sealed
//! descriptor and independently replays its checked program before emission.
//! The C entry point admits canonical logical frames itself. The old flattened
//! prepare operation remains a stable refusal, so bypassing a Rust wrapper cannot
//! create an input. Only flat owned Bytes identity bodies with literal boolean
//! contracts are executable in this initial profile; all other bodies fail at
//! generation, never fall back to the reversal fixture. Results retain the
//! predecessor flat export encoding; this is not a general public ABI.
//! The separate moves-v1 constructor extends only body admission to checked
//! non-allocating movement expressions; it never widens identity-v1 admission.

use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
use crate::public_generic_abi::carrier::frame::{CarrierFrameBinding, CarrierLeaf, LeafKind};
use crate::public_generic_abi::carrier::trace::Direction;
use crate::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor;
use crate::public_generic_abi::digest;

use super::binding::NativeProviderBindingV1;
use super::template::render_reference_provider;

mod allocating;
pub use allocating::{
    render_authenticated_allocating_provider, AuthenticatedNativeAllocatingArtifact,
    ALLOCATING_PROFILE,
};

pub const PROFILE: &str = "semaprax.authenticated-native-identity.v1";
pub const HEADER: &str = include_str!("authenticated_v1.h");
const SHA: &str = include_str!("authenticated_sha256.c");
const PREPARE: &str = include_str!("authenticated_prepare.c");
const PLACEHOLDER: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

pub const MOVES_PROFILE: &str = "semaprax.authenticated-native-moves.v1";

/// A separately admitted private movement-body artifact, not an identity-v1
/// artifact or a general allocating-body/public-support claim.
pub struct AuthenticatedNativeMovesArtifact {
    inner: AuthenticatedNativeIdentityArtifact,
}

impl AuthenticatedNativeMovesArtifact {
    pub fn source(&self) -> &str {
        self.inner.source()
    }
    pub fn descriptor_bytes(&self) -> &[u8] {
        self.inner.descriptor_bytes()
    }
    pub fn binding(&self) -> &NativeProviderBindingV1 {
        self.inner.binding()
    }
}

pub fn render_authenticated_moves_provider(
    program: &ResolvedProgram,
    source_revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<AuthenticatedNativeMovesArtifact, Diagnostic> {
    let (source, bridge) =
        crate::codegen::emit_public_generic_moves_bridge(program, source_revision, descriptor)?;
    Ok(AuthenticatedNativeMovesArtifact {
        inner: render_admitted(
            descriptor,
            &source,
            &bridge,
            MOVES_PROFILE,
            b"semaprax.authenticated-native-moves.v1.runtime\0",
            b"semaprax.authenticated-native-moves.v1.artifact\0",
            "spx_pg_endpoint_checked_moves_v1(",
        )?,
    })
}

/// A distinct, separately versioned physical profile
/// (`semaprax.authenticated-native-moves-nested.v1`, issue #292): `moves-v1`
/// itself ([`render_authenticated_moves_provider`]) stays exactly as
/// written above, still only a flat one-level owned-Bytes record, with
/// byte-identical output for every program it already admitted. This
/// constructor additionally admits a nested owned record (a record whose
/// fields are themselves owned-Bytes-only record instances, to a bounded
/// depth) through [`crate::codegen::emit_public_generic_nested_moves_bridge`]'s
/// own admission and C field-access codegen, rather than silently widening
/// `moves-v1`'s frozen contract in place.
pub const NESTED_MOVES_PROFILE: &str = "semaprax.authenticated-native-moves-nested.v1";

/// A separately admitted private nested-record movement-body artifact, not a
/// `moves-v1`, identity-v1, or general allocating-body/public-support claim.
pub struct AuthenticatedNativeNestedMovesArtifact {
    inner: AuthenticatedNativeIdentityArtifact,
}

impl AuthenticatedNativeNestedMovesArtifact {
    pub fn source(&self) -> &str {
        self.inner.source()
    }
    pub fn descriptor_bytes(&self) -> &[u8] {
        self.inner.descriptor_bytes()
    }
    pub fn binding(&self) -> &NativeProviderBindingV1 {
        self.inner.binding()
    }
}

pub fn render_authenticated_nested_moves_provider(
    program: &ResolvedProgram,
    source_revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<AuthenticatedNativeNestedMovesArtifact, Diagnostic> {
    let (source, bridge) = crate::codegen::emit_public_generic_nested_moves_bridge(
        program,
        source_revision,
        descriptor,
    )?;
    Ok(AuthenticatedNativeNestedMovesArtifact {
        inner: render_admitted(
            descriptor,
            &source,
            &bridge,
            NESTED_MOVES_PROFILE,
            b"semaprax.authenticated-native-moves-nested.v1.runtime\0",
            b"semaprax.authenticated-native-moves-nested.v1.artifact\0",
            "spx_pg_endpoint_checked_nested_moves_v1(",
        )?,
    })
}

/// Deterministic in-memory C artifact. No compilation, invocation or publication
/// authority is acquired by generating it.
pub struct AuthenticatedNativeIdentityArtifact {
    source: String,
    descriptor: Vec<u8>,
    binding: NativeProviderBindingV1,
}

impl AuthenticatedNativeIdentityArtifact {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn descriptor_bytes(&self) -> &[u8] {
        &self.descriptor
    }
    pub fn binding(&self) -> &NativeProviderBindingV1 {
        &self.binding
    }
}

pub fn render_authenticated_identity_provider(
    program: &ResolvedProgram,
    source_revision: &str,
    descriptor: &VerifiedPublicGenericDescriptor,
) -> Result<AuthenticatedNativeIdentityArtifact, Diagnostic> {
    // The compiler owns both legality and physical layout of the selected body.
    let (checked_source, bridge) =
        crate::codegen::emit_public_generic_identity_bridge(program, source_revision, descriptor)?;
    render_admitted(
        descriptor,
        &checked_source,
        &bridge,
        PROFILE,
        b"semaprax.authenticated-native-identity.v1.runtime\0",
        b"semaprax.authenticated-native-identity.v1.artifact\0",
        "spx_pg_endpoint_checked_identity_v1(",
    )
}

fn render_admitted(
    descriptor: &VerifiedPublicGenericDescriptor,
    checked_source: &str,
    bridge: &str,
    profile: &str,
    runtime_domain: &[u8],
    artifact_domain: &[u8],
    endpoint: &str,
) -> Result<AuthenticatedNativeIdentityArtifact, Diagnostic> {
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    let empty = plan
        .frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .map(|path| CarrierLeaf::new(path, LeafKind::Bytes, Vec::new()))
                .collect(),
        )
        .encode();
    let mut constants = String::new();
    array(&mut constants, "SPX_PG_AUTH_EMPTY_FRAME", &empty);
    array(
        &mut constants,
        "SPX_PG_AUTH_CLEANUP",
        descriptor.settlement().digest().as_bytes(),
    );
    writeln!(
        constants,
        "#define SPX_PG_AUTH_LEAF_COUNT {}u",
        plan.leaf_paths().len()
    )
    .unwrap();
    let carrier = CarrierBindingV1::new(
        descriptor.descriptor_digest(),
        TargetProfile::NativeC11,
        digest(runtime_domain, profile.as_bytes()),
    );
    let make_binding = |artifact: &str| {
        NativeProviderBindingV1::new(
            carrier.clone(),
            artifact,
            "spx_pg_authenticated_input_prepare_v1",
            env!("CARGO_PKG_VERSION"),
        )
    };
    let assemble = |binding: &NativeProviderBindingV1| -> Result<String, Diagnostic> {
        let mut source = render_reference_provider(descriptor.accepted_bytes(), binding);
        let start = source
            .find("static spx_pg_status_v1 spx_pg_endpoint_reverse_bytes_v1(")
            .ok_or_else(template_drift)?;
        let end = source[start..]
            .find("/* --- Provider binding / descriptor replay. --- */")
            .map(|offset| start + offset)
            .ok_or_else(template_drift)?;
        source.replace_range(start..end, bridge);
        let fixture_call = "spx_pg_endpoint_reverse_bytes_v1(";
        if source.matches(fixture_call).count() != 1 {
            return Err(template_drift());
        }
        source = source.replacen(fixture_call, endpoint, 1);
        let bypass = "spx_pg_status_v1 outcome = spx_pg_input_prepare_v1_impl(provider, carrier_bytes, carrier_len, out_input);";
        if source.matches(bypass).count() != 1 {
            return Err(template_drift());
        }
        source = source.replacen(bypass,
            "(void)provider; (void)carrier_bytes; (void)carrier_len;\n    if (out_input != NULL) *out_input = NULL;\n    spx_pg_status_v1 outcome = SPX_PG_STATUS_MALFORMED_CARRIER;", 1);
        let header = HEADER.replace(PROFILE, profile);
        Ok(format!("/* {profile}: unsupported, unpublished */\n{checked_source}\n{source}\n{header}\n{constants}\n{SHA}\n{PREPARE}"))
    };
    // Normalize the one artifact-digest slot to the fixed placeholder. This
    // commits the complete checked body, plan, codec and provider implementation.
    let normalized = assemble(&make_binding(PLACEHOLDER))?;
    let artifact_digest = digest(artifact_domain, normalized.as_bytes());
    let binding = make_binding(&artifact_digest);
    let source = assemble(&binding)?;
    Ok(AuthenticatedNativeIdentityArtifact {
        source,
        descriptor: descriptor.accepted_bytes().to_vec(),
        binding,
    })
}

fn array(out: &mut String, name: &str, bytes: &[u8]) {
    write!(out, "static const uint8_t {name}[] = {{").unwrap();
    for byte in bytes {
        write!(out, "0x{byte:02x},").unwrap();
    }
    writeln!(
        out,
        "}};\nstatic const size_t {name}_LEN = {};",
        bytes.len()
    )
    .unwrap();
}

fn template_drift() -> Diagnostic {
    Diagnostic::io(
        "SPX-PG902",
        "authenticated native provider template boundary changed",
    )
}
