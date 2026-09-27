//! Private additive caller for `semaprax.authenticated-native-identity.v1`.
//! No public support, general-body admission, or new wire profile is implied.
use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::{
    carrier::{
        frame::{CarrierFrameBinding, CarrierLeaf, LeafKind},
        trace::Direction,
    },
    descriptor::verify::VerifiedPublicGenericDescriptor,
    native::{
        authenticated::{
            AuthenticatedNativeAllocatingArtifact, AuthenticatedNativeIdentityArtifact,
            AuthenticatedNativeMovesArtifact, AuthenticatedNativeNestedMovesArtifact,
            ALLOCATING_PROFILE, HEADER, MOVES_PROFILE, NESTED_MOVES_PROFILE, PROFILE,
        },
        binding::NativeProviderBindingV1,
    },
};

use super::{render, CallingConsumer, OwnedByteField, RecordShape};

/// Generate a caller only for the exact sealed descriptor/provider pair. The
/// artifact's compiler-owned constructor already restricts the body and layout
/// to flat owned Bytes identity with literal contracts. This does not admit a
/// caller-supplied shape, unverified descriptor, or an ordinary flat provider.
pub fn generate_authenticated_identity_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeIdentityArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        PROFILE,
    )
}

/// Private closed movement-body profile. Framing and settlement are identical
/// to identity-v1, but only the independently admitted moves artifact binds it.
pub fn generate_authenticated_moves_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeMovesArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        MOVES_PROFILE,
    )
}

/// Private closed movement-body profile admitting a nested owned record
/// (issue #292). Framing and settlement generation are identical to
/// `moves-v1` above -- both walk `descriptor.input_facts().owned_leaves`/
/// `result_facts().owned_leaves`, already a flat, nesting-agnostic sequence
/// of leaf paths -- but only the independently admitted nested-moves
/// artifact binds this distinct profile string.
pub fn generate_authenticated_nested_moves_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeNestedMovesArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        NESTED_MOVES_PROFILE,
    )
}

fn generate(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact_descriptor: &[u8],
    binding: &NativeProviderBindingV1,
    profile_name: &str,
) -> Result<CallingConsumer, Diagnostic> {
    if descriptor.accepted_bytes() != artifact_descriptor {
        return Err(Diagnostic::io(
            "SPX-PG803",
            "authenticated C caller descriptor/provider mismatch",
        ));
    }
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    let mut profile = input_profile(descriptor, &plan)?;
    profile.header = HEADER.replace(PROFILE, profile_name);
    Ok(CallingConsumer {
        files: vec![
            (
                super::HEADER_FILE_NAME.to_owned(),
                super::HEADER_V1.replace("\r\n", "\n"),
            ),
            (
                super::CONSUMER_HEADER_FILE_NAME.to_owned(),
                render::consumer_header(&input, &output),
            ),
            (
                super::CONSUMER_SOURCE_FILE_NAME.to_owned(),
                render::consumer_source_with_profile(
                    descriptor.accepted_bytes(),
                    &binding.encode(),
                    &input,
                    &output,
                    Some(&profile),
                ),
            ),
        ],
    })
}

/// Private reservation-backed profile; only an exact compiler-admitted artifact
/// grants this caller shape. This does not widen either predecessor profile.
pub fn generate_authenticated_allocating_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeAllocatingArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        ALLOCATING_PROFILE,
    )
}

fn input_profile(
    descriptor: &VerifiedPublicGenericDescriptor,
    plan: &CarrierFrameBinding,
) -> Result<render::InputProfile, Diagnostic> {
    let empty = plan
        .frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .map(|path| CarrierLeaf::new(path, LeafKind::Bytes, Vec::new()))
                .collect(),
        )
        .encode();
    // Reserve at most the logical wire format's 4 MiB metadata allowance.
    // Payload limits are independently enforced by the existing flat preflight.
    if empty.len() > 4 * 1024 * 1024 {
        return Err(Diagnostic::io(
            "SPX-PG802",
            "authenticated C caller metadata capacity",
        ));
    }
    let mut support = String::new();
    // Reuse the physical profile's allocation-free digest implementation, with
    // only its byte-writer dependency rebound to this translation unit's codec.
    support.push_str(
        &include_str!("../../public_generic_abi/native/authenticated_sha256.c")
            .replace("spx_pg_write_u64le", "spx_pg_ccc_write_u64le"),
    );
    array(&mut support, "spx_pg_ccc_auth_empty", &empty);
    array(
        &mut support,
        "spx_pg_ccc_auth_cleanup",
        descriptor.settlement().digest().as_bytes(),
    );
    let mut offset = 0;
    for _ in 0..6 {
        offset = next_field(&empty, offset);
    }
    // Prefix includes leaf count; the next raw field is total payload length.
    let prefix = offset + 8;
    writeln!(support, "#define SPX_PG_CCC_AUTH_PREFIX {prefix}u").unwrap();
    offset = prefix + 8;
    let mut offsets = Vec::new();
    let mut lengths = Vec::new();
    for _ in plan.leaf_paths() {
        let start = offset;
        offset = next_field(&empty, offset) + 1; // path plus canonical kind
        offsets.push(start);
        lengths.push(offset - start);
        offset += 8; // empty payload length
    }
    assert_eq!(offset + 79, empty.len());
    for (name, values) in [("offsets", offsets), ("lengths", lengths)] {
        writeln!(
            support,
            "static const size_t spx_pg_ccc_auth_{name}[FIELD_COUNT] = {{{}}};",
            values
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
        .unwrap();
    }
    support.push_str(include_str!("render/authenticated_input.c.txt"));
    Ok(render::InputProfile {
        header: HEADER.to_owned(),
        support,
        encoder: "spx_pg_ccc_encode_authenticated",
        prepare: "uint64_t generation = 0;\n    status = spx_pg_authenticated_generation_v1(consumer->provider, &generation);\n    if (status == SPX_PG_STATUS_OK) status = spx_pg_authenticated_input_prepare_v1(consumer->provider, generation, SPX_PG_AUTH_OWNERSHIP_CALLER, spx_pg_ccc_auth_cleanup, sizeof(spx_pg_ccc_auth_cleanup), carrier, carrier_len, &input_handle);",
    })
}

fn next_field(bytes: &[u8], offset: usize) -> usize {
    offset
        + 8
        + usize::try_from(u64::from_le_bytes(
            bytes[offset..offset + 8].try_into().unwrap(),
        ))
        .unwrap()
}

fn array(out: &mut String, name: &str, bytes: &[u8]) {
    writeln!(
        out,
        "static const uint8_t {name}[] = {{{}}};",
        bytes
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
    .unwrap();
}
