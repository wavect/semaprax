//! Generates a real, standalone C++17 move-only *calling* consumer that
//! WRAPS the generated C11 calling consumer (issue #158), answering issue
//! #159.
//!
//! This does not reimplement the codec, the lifecycle, or the pairing check.
//! Every file [`c_calling::generate_c_calling_consumer`] emits is reused
//! byte-for-byte (this generator never restates
//! [`super::c_calling`]/[`super::c_calling::render`]'s templates or the
//! frozen native ABI header), and the generated C++ header
//! (`include/semaprax_public_generic_v1.hpp`) `#include`s the generated
//! `spx_pg_calling_consumer.h` exactly as issue #158's own module
//! documentation anticipated ("Issue #159's C++17 consumer is expected to
//! `extern "C"`-include this exact header and link against the compiled
//! `spx_pg_calling_consumer.c` rather than inventing a second ABI") and
//! calls only its declared public functions. No native ABI type
//! (`spx_pg_provider_v1` and friends) is ever named by this generator's own
//! output.
//!
//! **Move-only is the crux.** `Provider` and `Output` (the two owning C++
//! types this generator emits) delete their copy constructor/assignment,
//! are `noexcept` move-constructible/assignable, and release their sole
//! owned handle at most once, in a `noexcept` destructor, via a private
//! idempotent `reset()`/free helper — restated once here rather than in
//! every render function's own doc comment; see
//! [`render::provider_class_declaration`] and [`render::output_class`] for
//! the exact generated code.
//!
//! **Scope, stated once**, restating [`super::c_calling`]'s own: [Public
//! Generic Boundary Profile
//! v1](../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) admits exactly one
//! owned aggregate input parameter and exactly one owned aggregate result in
//! v1, so this generator emits exactly two concrete C++ types, `Input` (an
//! ordinary aggregate of `std::vector<std::uint8_t>` leaves, consumed by
//! value) and `Output` (a move-only RAII wrapper around the generated C
//! `spx_pg_output` value), each a flat sequence of owned-bytes leaves; a
//! Copy scalar or nested-record leaf is future work tracked by the same
//! #119 prerequisite [`super::c_calling`] and [`super::rust_calling`] name,
//! not a limitation invented here.
//!
//! Determinism and authority: like [`super::c_calling::generate_c_calling_consumer`],
//! this is a pure function from already-trusted bytes to source text. It
//! reads no file, starts no process, and uses no network; the C++ program it
//! emits is what actually calls native code, once compiled and executed by
//! a caller.

use crate::public_generic_abi::native::binding::NativeProviderBindingV1;

use super::c_calling;
use super::rust_calling::{OwnedByteField, RecordShape, ShapeError};

/// The generated C++ field name for one owned leaf, matching
/// [`super::c_calling`]'s and [`super::rust_calling`]'s own `field_<hex-identity>`
/// scheme exactly (all three generators derive from
/// [`super::identifier`]), so the same descriptor produces aligned field
/// names across every generated language.
fn field_name(field: &OwnedByteField) -> String {
    format!("field_{}", super::identifier(&field.identity))
}

/// One generated file's relative path and deterministic contents, in
/// emission order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallingConsumer {
    files: Vec<(String, String)>,
}

impl CallingConsumer {
    pub fn files(&self) -> &[(String, String)] {
        &self.files
    }
}

/// The generated public C++ header: the move-only RAII wrapper. Depends only
/// on the C11 consumer header this package also carries and a fixed set of
/// C++17 standard headers.
pub const WRAPPER_HEADER_FILE_NAME: &str = "include/semaprax_public_generic_v1.hpp";
/// The generated executable test driver: real execution evidence, including
/// the RAII/move-only compile-time and runtime contract, not a mere compile
/// check.
pub const ROUND_TRIP_FILE_NAME: &str = "test/round_trip.cpp";

/// Generate one C++17 calling consumer for `descriptor_bytes` and `binding`,
/// admitting exactly `input`/`output` as the one owned input parameter and
/// one owned result [Public Generic Boundary Profile
/// v1](../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) admits in v1.
///
/// Internally generates the exact same C11 calling consumer
/// [`c_calling::generate_c_calling_consumer`] would for the same arguments
/// (never a second, drifting copy of it) and adds exactly two new files: the
/// C++ wrapper header and its executable round-trip test.
///
/// Deterministic: the same arguments always produce byte-identical files.
pub fn generate_cxx_calling_consumer(
    descriptor_bytes: &[u8],
    binding: &NativeProviderBindingV1,
    input: &RecordShape,
    output: &RecordShape,
) -> Result<CallingConsumer, ShapeError> {
    let c_consumer =
        c_calling::generate_c_calling_consumer(descriptor_bytes, binding, input, output)?;
    let mut files: Vec<(String, String)> = c_consumer.files().to_vec();
    files.push((
        WRAPPER_HEADER_FILE_NAME.to_owned(),
        render::wrapper_header(input, output),
    ));
    files.push((
        ROUND_TRIP_FILE_NAME.to_owned(),
        render::round_trip_cpp(input, output),
    ));
    Ok(CallingConsumer { files })
}

/// Private additive caller for `semaprax.authenticated-native-identity.v1`:
/// unsupported and unpublished. The sealed C generator authenticates the exact
/// descriptor/artifact pair and owns all framing, generation and settlement.
/// Only the unchanged move-only C++ wrapper is added, using trusted leaf order.
/// The legacy generator and its reversal-fixture round-trip driver are unchanged.
pub fn generate_authenticated_identity_calling_consumer_v1(
    descriptor: &crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor,
    artifact: &crate::public_generic_abi::native::authenticated::AuthenticatedNativeIdentityArtifact,
) -> Result<CallingConsumer, crate::diagnostic::Diagnostic> {
    let c_consumer =
        c_calling::generate_authenticated_identity_calling_consumer_v1(descriptor, artifact)?;
    Ok(wrap_authenticated(descriptor, &c_consumer))
}

/// Private movement-body profile; delegates all codec, binding and settlement
/// work to the sealed C generator without changing the move-only wrapper.
pub fn generate_authenticated_moves_calling_consumer_v1(
    descriptor: &crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor,
    artifact: &crate::public_generic_abi::native::authenticated::AuthenticatedNativeMovesArtifact,
) -> Result<CallingConsumer, crate::diagnostic::Diagnostic> {
    let c_consumer =
        c_calling::generate_authenticated_moves_calling_consumer_v1(descriptor, artifact)?;
    Ok(wrap_authenticated(descriptor, &c_consumer))
}

fn wrap_authenticated(
    descriptor: &crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor,
    c_consumer: &c_calling::CallingConsumer,
) -> CallingConsumer {
    let shape = |paths: &[String]| {
        RecordShape::new(paths.iter().cloned().map(OwnedByteField::new).collect())
    };
    let input = shape(&descriptor.input_facts().owned_leaves);
    let output = shape(&descriptor.result_facts().owned_leaves);
    let mut files = c_consumer.files().to_vec();
    files.push((
        WRAPPER_HEADER_FILE_NAME.to_owned(),
        render::wrapper_header(&input, &output),
    ));
    CallingConsumer { files }
}

/// Private nested-owned-record movement-body profile (issue #292 / #288
/// follow-on): delegates all codec, admission and settlement work to the
/// sealed C generator without changing the move-only wrapper. `Input`/
/// `Output` remain the same flat sequence of owned-`Bytes` leaves the wrapper
/// has always emitted -- [`super::rust_calling::RecordShape`]'s own leaf
/// paths are already flat and nesting-agnostic -- so nesting changes neither
/// this generator nor the wrapper it emits, only the sealed C provider/caller
/// codec underneath it.
pub fn generate_authenticated_nested_moves_calling_consumer_v1(
    descriptor: &crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor,
    artifact: &crate::public_generic_abi::native::authenticated::AuthenticatedNativeNestedMovesArtifact,
) -> Result<CallingConsumer, crate::diagnostic::Diagnostic> {
    let c_consumer =
        c_calling::generate_authenticated_nested_moves_calling_consumer_v1(descriptor, artifact)?;
    Ok(wrap_authenticated(descriptor, &c_consumer))
}

/// Private reservation-backed profile, reusing the sealed C caller and the
/// unchanged move-only C++ wrapper without another framing/ownership codec.
pub fn generate_authenticated_allocating_calling_consumer_v1(
    descriptor: &crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor,
    artifact: &crate::public_generic_abi::native::authenticated::AuthenticatedNativeAllocatingArtifact,
) -> Result<CallingConsumer, crate::diagnostic::Diagnostic> {
    let c_consumer =
        c_calling::generate_authenticated_allocating_calling_consumer_v1(descriptor, artifact)?;
    Ok(wrap_authenticated(descriptor, &c_consumer))
}

mod render;

#[cfg(test)]
mod tests;
