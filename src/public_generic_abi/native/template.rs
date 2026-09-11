//! Renders one compilable C11 translation unit for the native physical
//! adapter: the versioned header (`spx_pg_v1.h`), the trusted descriptor and
//! provider-binding byte constants a real `VerifiedPublicGenericDescriptor`
//! and [`NativeProviderBindingV1`] supply, and the hand-authored reference
//! body (`provider_body.c`) — see [`super`] for the full scope note.
//!
//! Nothing here decides legality: `render_reference_provider` is a pure,
//! deterministic string-composition function over already-trusted bytes.
//! Whether those bytes are trustworthy is [`crate::public_generic_abi::descriptor::verify`]'s
//! job, exercised once by the caller before this function ever runs.

use std::fmt::Write as _;

use super::binding::NativeProviderBindingV1;

/// The versioned ABI header text, exactly as compiled into every rendered
/// provider. Exposed so a test can also compile it completely alone (a
/// required ABI-contract test: "header compiles as C11").
pub const HEADER_V1: &str = include_str!("spx_pg_v1.h");

const BODY_V1: &str = include_str!("provider_body.c");

/// One fully compilable provider translation unit: `HEADER_V1`, then the
/// trusted `descriptor_bytes`/`binding.encode()` rendered as C byte-array
/// constants, then `BODY_V1`. Byte-deterministic: identical inputs render
/// identical bytes.
pub fn render_reference_provider(
    descriptor_bytes: &[u8],
    binding: &NativeProviderBindingV1,
) -> String {
    let binding_bytes = binding.encode();
    let mut source = String::with_capacity(
        HEADER_V1.len()
            + BODY_V1.len()
            + descriptor_bytes.len() * 6
            + binding_bytes.len() * 6
            + 512,
    );
    source.push_str(HEADER_V1);
    source.push('\n');
    source.push_str(
        "/* Trusted constants substituted by src/public_generic_abi/native/template.rs from a\n\
         * real VerifiedPublicGenericDescriptor and NativeProviderBindingV1. Never hand-edited. */\n",
    );
    write_byte_array(
        &mut source,
        "SPX_PG_TRUSTED_DESCRIPTOR_BYTES",
        "SPX_PG_TRUSTED_DESCRIPTOR_LEN",
        descriptor_bytes,
    );
    write_byte_array(
        &mut source,
        "SPX_PG_TRUSTED_BINDING_BYTES",
        "SPX_PG_TRUSTED_BINDING_LEN",
        &binding_bytes,
    );
    source.push('\n');
    source.push_str(BODY_V1);
    source
}

fn write_byte_array(source: &mut String, bytes_name: &str, len_name: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        writeln!(source, "static const uint8_t {bytes_name}[] = {{0}};").unwrap();
    } else {
        source.push_str("static const uint8_t ");
        source.push_str(bytes_name);
        source.push_str("[] = {");
        for (index, byte) in bytes.iter().enumerate() {
            if index != 0 {
                source.push(',');
            }
            write!(source, "0x{byte:02x}").unwrap();
        }
        source.push_str("};\n");
    }
    writeln!(source, "static const size_t {len_name} = {};", bytes.len()).unwrap();
}

#[cfg(test)]
mod tests;
