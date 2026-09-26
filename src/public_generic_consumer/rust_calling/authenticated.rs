//! Private, unsupported/unpublished authenticated native identity, moves and
//! allocating callers. The legacy generated crate, manifests and wire bytes
//! remain unchanged; identity-v1 output is byte-identical to its predecessor.
use std::fmt::Write as _;

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::{
    carrier::{
        frame::{parse_bounded, CarrierFrameBinding, CarrierLeaf, LeafKind},
        trace::Direction,
    },
    descriptor::verify::VerifiedPublicGenericDescriptor,
    native::{
        authenticated::{
            AuthenticatedNativeAllocatingArtifact, AuthenticatedNativeIdentityArtifact,
            AuthenticatedNativeMovesArtifact, ALLOCATING_PROFILE, MOVES_PROFILE, PROFILE,
        },
        binding::NativeProviderBindingV1,
    },
};

use super::{render, CallingConsumer, OwnedByteField, RecordShape};

/// Package identity of one private authenticated Rust caller profile. Each
/// profile names itself in the crate docs and the Cargo package/library names,
/// so no later profile's output can be mistaken for identity-v1's.
struct ProfileNames {
    profile: &'static str,
    package: &'static str,
    library: &'static str,
}

const IDENTITY: ProfileNames = ProfileNames {
    profile: PROFILE,
    package: "spx-pg-private-authenticated-rust-v1",
    library: "spx_pg_private_authenticated_rust_v1",
};
const MOVES: ProfileNames = ProfileNames {
    profile: MOVES_PROFILE,
    package: "spx-pg-private-authenticated-moves-rust-v1",
    library: "spx_pg_private_authenticated_moves_rust_v1",
};
const ALLOCATING: ProfileNames = ProfileNames {
    profile: ALLOCATING_PROFILE,
    package: "spx-pg-private-authenticated-allocating-rust-v1",
    library: "spx_pg_private_authenticated_allocating_rust_v1",
};

/// Emit a private caller for the exact sealed descriptor/provider pair. No
/// caller-authored shape, ordinary flat provider, shared ABI change or support
/// decision is accepted. The generated package pins the existing SHA-256 crate
/// and may be built fully offline; its name and publish=false distinguish it
/// from the ordinary caller package.
pub fn generate_authenticated_identity_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeIdentityArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        &IDENTITY,
    )
}

/// Private closed movement-body profile. Framing, admission and settlement are
/// the identity-v1 caller's; only the independently admitted moves artifact
/// binds it, and the package is separately named.
pub fn generate_authenticated_moves_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeMovesArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        &MOVES,
    )
}

/// Private reservation-backed body profile; only an exact compiler-admitted
/// allocating artifact binds it. This widens neither predecessor profile.
pub fn generate_authenticated_allocating_calling_consumer_v1(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact: &AuthenticatedNativeAllocatingArtifact,
) -> Result<CallingConsumer, Diagnostic> {
    generate(
        descriptor,
        artifact.descriptor_bytes(),
        artifact.binding(),
        &ALLOCATING,
    )
}

fn generate(
    descriptor: &VerifiedPublicGenericDescriptor,
    artifact_descriptor: &[u8],
    binding: &NativeProviderBindingV1,
    names: &ProfileNames,
) -> Result<CallingConsumer, Diagnostic> {
    if descriptor.accepted_bytes() != artifact_descriptor {
        return Err(Diagnostic::io(
            "SPX-PG803",
            "authenticated Rust caller descriptor/provider mismatch",
        ));
    }
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let mut carrier = render::carrier_rs(&input, &output);
    replace_once(
        &mut carrier,
        "encode_leaves(&leaves)",
        "crate::authenticated::encode(&encode_leaves(&leaves)?)",
    );
    let mut library = render::lib_rs();
    replace_once(
        &mut library,
        "mod carrier;",
        "mod authenticated;\nmod carrier;",
    );
    library.insert_str(
        0,
        &format!(
            "//! Private {} caller: unsupported, unpublished.\n",
            names.profile
        ),
    );
    Ok(CallingConsumer { files: vec![
        ("Cargo.toml".into(), format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nrust-version = \"{}\"\npublish = false\n\n[lib]\nname = \"{}\"\npath = \"src/lib.rs\"\n\n[dependencies]\nsha2 = \"=0.10.9\"\n\n[lints.rust]\nunsafe_code = \"deny\"\n", names.package, super::RUST_VERSION, names.library)),
        ("build.rs".into(), render::build_rs()),
        ("src/lib.rs".into(), library),
        ("src/error.rs".into(), render::error_rs()),
        ("src/descriptor.rs".into(), render::descriptor_rs(descriptor.accepted_bytes(), &binding.encode())),
        ("src/types.rs".into(), render::types_rs(&input, &output)),
        ("src/carrier.rs".into(), carrier),
        ("src/provider.rs".into(), provider()),
        ("src/authenticated.rs".into(), frame_codec(descriptor)?),
    ] })
}

fn provider() -> String {
    let mut source = render::provider_rs();
    replace_once(
        &mut source,
        "    extern \"C\" {",
        include_str!("render/authenticated_ffi_rs.txt"),
    );
    replace_once(
        &mut source,
        "        let mut input_handle: *mut ffi::RawValue = std::ptr::null_mut();",
        include_str!("render/authenticated_generation_rs.txt"),
    );
    replace_once(&mut source, "ffi::spx_pg_input_prepare_v1(\n                self.handle,",
        "ffi::spx_pg_authenticated_input_prepare_v1(\n                self.handle,\n                generation,\n                0, // caller-owned ticket; the profile admits no provider-owned input\n                crate::authenticated::CLEANUP.as_ptr(),\n                crate::authenticated::CLEANUP.len(),");
    replace_once(
        &mut source,
        "ffi::SPX_PG_STATUS_MALFORMED_CARRIER =>",
        "ffi::SPX_PG_STATUS_MALFORMED_CARRIER | ffi::SPX_PG_AUTH_STATUS_REPLAY_MISMATCH =>",
    );
    source
}

fn frame_codec(descriptor: &VerifiedPublicGenericDescriptor) -> Result<String, Diagnostic> {
    let plan = CarrierFrameBinding::from_verified_descriptor(descriptor, Direction::Input);
    let empty = plan
        .frame_with_leaves(
            plan.leaf_paths()
                .iter()
                .map(|path| CarrierLeaf::new(path, LeafKind::Bytes, Vec::new()))
                .collect(),
        )
        .encode();
    plan.validate_frame(&parse_bounded(&empty)?)?;
    if empty.len() > 4 * 1024 * 1024 {
        return Err(Diagnostic::io(
            "SPX-PG802",
            "authenticated Rust caller metadata capacity",
        ));
    }
    let mut out = String::from(
        "//! Private canonical input encoder; result decoding remains the frozen flat profile.\n",
    );
    writeln!(out, "const EMPTY: &[u8] = &{empty:?};").unwrap();
    writeln!(
        out,
        "pub(crate) const CLEANUP: &[u8] = &{:?};",
        descriptor.settlement().digest().as_bytes()
    )
    .unwrap();
    let mut offset = 0;
    for _ in 0..6 {
        offset = next_field(&empty, offset);
    }
    let prefix = offset + 8;
    writeln!(out, "const PREFIX: usize = {prefix};").unwrap();
    offset = prefix + 8;
    let mut metadata = Vec::new();
    for _ in plan.leaf_paths() {
        let start = offset;
        offset = next_field(&empty, offset) + 1;
        metadata.push((start, offset - start));
        offset += 8;
    }
    assert_eq!(offset + 79, empty.len());
    writeln!(out, "const METADATA: &[(usize, usize)] = &{metadata:?};").unwrap();
    out.push_str(include_str!("render/authenticated_frame_rs.txt"));
    Ok(out.replace("\r\n", "\n"))
}

fn next_field(bytes: &[u8], offset: usize) -> usize {
    offset
        + 8
        + usize::try_from(u64::from_le_bytes(
            bytes[offset..offset + 8].try_into().unwrap(),
        ))
        .unwrap()
}

fn replace_once(source: &mut String, old: &str, new: &str) {
    assert_eq!(
        source.matches(old).count(),
        1,
        "authenticated Rust caller template drift"
    );
    *source = source.replacen(old, &new.replace("\r\n", "\n"), 1);
}
