//! Descriptor-only physical ABI for admitted native resource functions.
//!
//! This private milestone intentionally exposes no callable resource API.  It
//! serializes one compiler-admitted host template into a canonical,
//! pointer-free byte descriptor and emits a C11 getter for immutable
//! library-owned storage.  Public resource lowering remains gated by
//! `SPX-B104`.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the descriptor is staged behind the native resource gate"
    )
)]

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::native_host_contract::{
    self, NativeAdapterParameterProjection, NativeAdapterResultProjection,
    NativeHostContractTemplate, NativeHostScalarKind,
};

const MAGIC: &[u8; 8] = b"SPXNABI1";
const VERSION: u32 = 1;
const HEADER_SIZE: u32 = 20;
const FINGERPRINT_BYTES: usize = 32;
const SCHEMA_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-adapter-schema.v1\0";
const TARGET_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-adapter-target.v1\0";
const PHYSICAL_MODULE_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.native-adapter-physical-module.v1\0";
const GETTER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-adapter-getter.v1\0";

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED_RESOURCE: u32 = 2;
const SCALAR_I64: u32 = 1;
const SCALAR_BOOL: u32 = 2;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

/// Exact physical descriptor and the sole symbol that may expose it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeAdapterDescriptor {
    pub(super) bytes: Vec<u8>,
    pub(super) getter_symbol: String,
}

/// Derive a canonical physical descriptor only through the sealed compiler
/// template projection. No HIR, cleanup, value planning, adapter binding, or
/// runtime authority is constructed here.
pub(super) fn derive(
    template: &NativeHostContractTemplate,
) -> Result<NativeAdapterDescriptor, Diagnostic> {
    let projection = native_host_contract::project_for_adapter_abi(template);
    let semantic_module = decode_fingerprint(
        &projection.module_abi_fingerprint,
        "semantic module ABI fingerprint",
    )?;
    let function_template = decode_fingerprint(
        &projection.function_template_fingerprint,
        "function-template fingerprint",
    )?;
    let target = physical_target_tag()?;
    let schema = schema_fingerprint();
    let target_fingerprint = target_fingerprint(target.as_bytes());
    let physical_module = physical_module_fingerprint(
        &schema,
        &target_fingerprint,
        &semantic_module,
        projection.module.as_bytes(),
    );

    let mut writer = WireWriter::new();
    writer.bytes.extend_from_slice(MAGIC);
    writer.u32(VERSION);
    writer.u32(HEADER_SIZE);
    let total_length_offset = writer.bytes.len();
    writer.u32(0);
    writer.text(&target, "physical target tag")?;
    writer.bytes.extend_from_slice(&schema);
    writer.bytes.extend_from_slice(&target_fingerprint);
    writer.bytes.extend_from_slice(&physical_module);
    writer.bytes.extend_from_slice(&function_template);
    writer.text(&projection.module, "module identity")?;
    writer.text(&projection.function, "function identity")?;
    writer.count(projection.parameters.len(), "parameter count")?;

    let mut expected_owner_ordinal = 0_usize;
    for (expected_index, parameter) in projection.parameters.iter().enumerate() {
        match parameter {
            NativeAdapterParameterProjection::Scalar {
                parameter_index,
                value_id,
                kind,
            } => {
                require_index(*parameter_index, expected_index)?;
                writer.u32(PARAMETER_SCALAR);
                writer.index(*parameter_index, "scalar parameter index")?;
                writer.text(value_id.as_str(), "scalar value identity")?;
                writer.u32(match kind {
                    NativeHostScalarKind::I64 => SCALAR_I64,
                    NativeHostScalarKind::Bool => SCALAR_BOOL,
                });
            }
            NativeAdapterParameterProjection::OwnedResource {
                parameter_index,
                value_id,
                owner_ordinal,
                resource_type,
                lifecycle,
            } => {
                require_index(*parameter_index, expected_index)?;
                require_index(*owner_ordinal, expected_owner_ordinal)?;
                expected_owner_ordinal += 1;
                writer.u32(PARAMETER_OWNED_RESOURCE);
                writer.index(*parameter_index, "resource parameter index")?;
                writer.text(value_id.as_str(), "resource value identity")?;
                writer.index(*owner_ordinal, "owner ordinal")?;
                writer.text(resource_type, "resource identity")?;
                writer.text(lifecycle, "lifecycle identity")?;
            }
        }
    }

    match &projection.result {
        NativeAdapterResultProjection::ScalarI64 => writer.u32(RESULT_SCALAR_I64),
        NativeAdapterResultProjection::OwnedInput {
            parameter_index,
            value_id,
            owner_ordinal,
        } => {
            let Some(NativeAdapterParameterProjection::OwnedResource {
                parameter_index: expected_parameter,
                value_id: expected_value,
                owner_ordinal: expected_owner,
                ..
            }) = projection.parameters.get(*parameter_index)
            else {
                return Err(adapter_error(
                    "owned result does not select an admitted resource parameter",
                ));
            };
            if expected_parameter != parameter_index
                || expected_value != value_id
                || expected_owner != owner_ordinal
            {
                return Err(adapter_error(
                    "owned result mapping disagrees with admitted parameter metadata",
                ));
            }
            writer.u32(RESULT_OWNED_INPUT);
            writer.index(*parameter_index, "owned-result parameter index")?;
            writer.text(value_id.as_str(), "owned-result value identity")?;
            writer.index(*owner_ordinal, "owned-result owner ordinal")?;
        }
    }

    let total_length = u32::try_from(writer.bytes.len())
        .map_err(|_| adapter_error("physical descriptor exceeds the u32 wire limit"))?;
    writer.bytes[total_length_offset..total_length_offset + 4]
        .copy_from_slice(&total_length.to_le_bytes());
    let getter_symbol = getter_symbol(&physical_module, &function_template);
    Ok(NativeAdapterDescriptor {
        bytes: writer.bytes,
        getter_symbol,
    })
}

/// Emit a standalone header with one pointer-returning descriptor getter.
/// The wire prefix carries the total size; there is no caller-owned out slot.
pub(super) fn emit_header(descriptor: &NativeAdapterDescriptor) -> String {
    let guard = format!("{}_H", descriptor.getter_symbol.to_ascii_uppercase());
    format!(
        "#ifndef {guard}\n#define {guard}\n\
         #include <limits.h>\n#include <stdint.h>\n\
         #if defined(__cplusplus)\n#define SPX_ADAPTER_STATIC_ASSERT(c, m) static_assert((c), m)\nextern \"C\" {{\n#else\n#define SPX_ADAPTER_STATIC_ASSERT(c, m) _Static_assert((c), m)\n#endif\n\
         SPX_ADAPTER_STATIC_ASSERT(CHAR_BIT == 8, \"SEMAPRAX adapter requires 8-bit bytes\");\n\
         SPX_ADAPTER_STATIC_ASSERT(sizeof(uint8_t) == 1, \"SEMAPRAX adapter requires exact uint8_t\");\n\
         SPX_ADAPTER_STATIC_ASSERT(sizeof(uint32_t) == 4, \"SEMAPRAX adapter requires exact uint32_t\");\n\
         SPX_ADAPTER_STATIC_ASSERT(sizeof(uint64_t) == 8, \"SEMAPRAX adapter requires exact uint64_t\");\n\
         SPX_ADAPTER_STATIC_ASSERT(sizeof(int64_t) == 8, \"SEMAPRAX adapter requires exact int64_t\");\n\
         #if defined(_WIN32)\n\
         # if defined(SPX_ADAPTER_DESCRIPTOR_BUILD)\n\
         #  define SPX_ADAPTER_API __declspec(dllexport)\n\
         # elif defined(SPX_ADAPTER_DESCRIPTOR_DLL)\n\
         #  define SPX_ADAPTER_API __declspec(dllimport)\n\
         # else\n\
         #  define SPX_ADAPTER_API\n\
         # endif\n\
         # define SPX_ADAPTER_CALL __cdecl\n\
         #elif defined(__GNUC__) || defined(__clang__)\n\
         # define SPX_ADAPTER_API __attribute__((visibility(\"default\")))\n\
         # define SPX_ADAPTER_CALL\n\
         #else\n\
         # define SPX_ADAPTER_API\n#define SPX_ADAPTER_CALL\n#endif\n\
         SPX_ADAPTER_API const unsigned char *SPX_ADAPTER_CALL {symbol}(void);\n\
         #if defined(__cplusplus)\n}}\n#endif\n\
         #undef SPX_ADAPTER_STATIC_ASSERT\n\
         #endif /* {guard} */\n",
        symbol = descriptor.getter_symbol
    )
}

/// Emit the immutable provider translation unit. Visibility may default to
/// hidden; the header annotates only the descriptor getter for export.
pub(super) fn emit_source(
    descriptor: &NativeAdapterDescriptor,
    header_name: &str,
) -> Result<String, Diagnostic> {
    if header_name.is_empty()
        || !header_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(adapter_error(
            "descriptor header name is not a safe basename",
        ));
    }
    let mut output = format!(
        "#define SPX_ADAPTER_DESCRIPTOR_BUILD 1\n#include \"{header_name}\"\n\
         {}\nstatic const unsigned char spx_adapter_descriptor_bytes[] = {{\n",
        provider_target_guards()?
    );
    for chunk in descriptor.bytes.chunks(12) {
        output.push_str("    ");
        for byte in chunk {
            write!(output, "0x{byte:02x}, ").expect("writing to a string cannot fail");
        }
        output.push('\n');
    }
    output.push_str("};\n");
    writeln!(
        output,
        "const unsigned char *SPX_ADAPTER_CALL {}(void) {{",
        descriptor.getter_symbol
    )
    .expect("writing to a string cannot fail");
    output.push_str("    return spx_adapter_descriptor_bytes;\n}\n");
    Ok(output)
}

/// Exhaustive provider-side proof that the C compiler which materializes the
/// immutable blob targets the same physical ABI encoded by Rust.  Consumers
/// may inspect the descriptor anywhere, but this first milestone does not
/// claim external cross-target provider generation.
fn provider_target_guards() -> Result<String, Diagnostic> {
    let architecture = if cfg!(target_arch = "x86_64") {
        "defined(__x86_64__) || defined(_M_X64)"
    } else if cfg!(target_arch = "x86") {
        "defined(__i386__) || defined(_M_IX86)"
    } else if cfg!(target_arch = "aarch64") {
        "defined(__aarch64__) || defined(__arm64__) || defined(_M_ARM64)"
    } else if cfg!(target_arch = "arm") {
        "defined(__arm__) || defined(_M_ARM)"
    } else if cfg!(target_arch = "riscv64") {
        "defined(__riscv) && defined(__riscv_xlen) && (__riscv_xlen == 64)"
    } else {
        return Err(adapter_error(
            "provider guard does not support this Rust target architecture",
        ));
    };
    let operating_system = if cfg!(windows) {
        "defined(_WIN32)"
    } else if cfg!(target_os = "macos") {
        "defined(__APPLE__) && defined(__MACH__) && TARGET_OS_OSX"
    } else if cfg!(target_os = "ios") {
        "defined(__APPLE__) && defined(__MACH__) && TARGET_OS_IOS"
    } else if cfg!(target_os = "linux") {
        "defined(__linux__) && !defined(__ANDROID__)"
    } else {
        return Err(adapter_error(
            "provider guard does not support this Rust target operating system",
        ));
    };
    let environment = if cfg!(target_env = "msvc") {
        "defined(_MSC_VER) && !defined(__MINGW32__)"
    } else if cfg!(all(windows, target_env = "gnu")) {
        "defined(__MINGW32__)"
    } else if cfg!(all(target_os = "linux", target_env = "gnu")) {
        "defined(__GLIBC__)"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "defined(__APPLE__)"
    } else {
        return Err(adapter_error(
            "provider guard cannot prove this Rust target environment",
        ));
    };
    let object_format = if cfg!(windows) {
        "defined(_WIN32)"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "defined(__MACH__)"
    } else if cfg!(target_os = "linux") {
        "defined(__ELF__)"
    } else {
        return Err(adapter_error(
            "provider guard cannot prove this Rust target object format",
        ));
    };
    let pointer_width = match usize::BITS {
        32 => "UINTPTR_MAX == UINT32_MAX",
        64 => "UINTPTR_MAX == UINT64_MAX",
        _ => {
            return Err(adapter_error(
                "provider guard does not support this Rust pointer width",
            ))
        }
    };
    let endian_guard = if cfg!(target_endian = "little") {
        "#if defined(__BYTE_ORDER__)\n\
         # if !defined(__ORDER_LITTLE_ENDIAN__) || (__BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__)\n\
         #  error \"SEMAPRAX descriptor provider endian mismatch\"\n\
         # endif\n\
         #elif !defined(_WIN32)\n\
         # error \"SEMAPRAX descriptor provider cannot prove little endian\"\n\
         #endif"
    } else {
        "#if !defined(__BYTE_ORDER__) || !defined(__ORDER_BIG_ENDIAN__) || (__BYTE_ORDER__ != __ORDER_BIG_ENDIAN__)\n\
         # error \"SEMAPRAX descriptor provider endian mismatch\"\n\
         #endif"
    };
    let apple_include = if cfg!(any(target_os = "macos", target_os = "ios")) {
        "#include <TargetConditionals.h>\n"
    } else {
        ""
    };
    Ok(format!(
        "{apple_include}#if !({architecture})\n\
         # error \"SEMAPRAX descriptor provider architecture mismatch\"\n\
         #endif\n\
         #if !({operating_system})\n\
         # error \"SEMAPRAX descriptor provider operating-system mismatch\"\n\
         #endif\n\
         #if !({environment})\n\
         # error \"SEMAPRAX descriptor provider environment mismatch\"\n\
         #endif\n\
         #if !({object_format})\n\
         # error \"SEMAPRAX descriptor provider object-format mismatch\"\n\
         #endif\n\
         #if !({pointer_width})\n\
         # error \"SEMAPRAX descriptor provider pointer-width mismatch\"\n\
         #endif\n\
         {endian_guard}\n"
    ))
}

fn require_index(actual: usize, expected: usize) -> Result<(), Diagnostic> {
    if actual == expected {
        Ok(())
    } else {
        Err(adapter_error(format!(
            "noncanonical physical descriptor index {actual}; expected {expected}"
        )))
    }
}

fn physical_target_tag() -> Result<String, Diagnostic> {
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    let environment = if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "apple"
    } else {
        return Err(adapter_error("physical target environment is unknown"));
    };
    let object_format = if cfg!(target_family = "wasm") {
        "wasm"
    } else if cfg!(windows) {
        "coff"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "macho"
    } else if cfg!(unix) {
        "elf"
    } else {
        return Err(adapter_error("physical target object format is unknown"));
    };
    let getter_abi = if cfg!(windows) {
        "descriptor-getter-cdecl"
    } else {
        "descriptor-getter-c"
    };
    Ok(format!(
        "{}-{}-{environment}-{object_format}-ptr{}-{endian}-{getter_abi}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        usize::BITS
    ))
}

fn schema_fingerprint() -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, MAGIC);
    hasher.update(VERSION.to_le_bytes());
    hasher.update(HEADER_SIZE.to_le_bytes());
    for tag in [
        PARAMETER_SCALAR,
        PARAMETER_OWNED_RESOURCE,
        SCALAR_I64,
        SCALAR_BOOL,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
    ] {
        hasher.update(tag.to_le_bytes());
    }
    hasher.update(b"u32le-lengths;utf8-identities;ordered-signature;pointer-free");
    hasher.finalize().into()
}

fn target_fingerprint(target: &[u8]) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(TARGET_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, target);
    hasher.finalize().into()
}

fn physical_module_fingerprint(
    schema: &[u8; FINGERPRINT_BYTES],
    target: &[u8; FINGERPRINT_BYTES],
    semantic_module: &[u8; FINGERPRINT_BYTES],
    module: &[u8],
) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(PHYSICAL_MODULE_FINGERPRINT_DOMAIN);
    hash_field(&mut hasher, schema);
    hash_field(&mut hasher, target);
    hash_field(&mut hasher, semantic_module);
    hash_field(&mut hasher, module);
    hasher.finalize().into()
}

fn getter_symbol(
    physical_module: &[u8; FINGERPRINT_BYTES],
    function_template: &[u8; FINGERPRINT_BYTES],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(GETTER_SYMBOL_DOMAIN);
    hash_field(&mut hasher, physical_module);
    hash_field(&mut hasher, function_template);
    let digest = hasher.finalize();
    let mut symbol = String::from("spx_");
    for byte in &digest[..24] {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.push_str("_adapter_descriptor_v1");
    symbol
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn decode_fingerprint(value: &str, context: &str) -> Result<[u8; 32], Diagnostic> {
    if value.len() != FINGERPRINT_BYTES * 2
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(adapter_error(format!(
            "{context} is not canonical lowercase SHA-256"
        )));
    }
    let mut decoded = [0_u8; FINGERPRINT_BYTES];
    for (index, slot) in decoded.iter_mut().enumerate() {
        let offset = index * 2;
        *slot = u8::from_str_radix(&value[offset..offset + 2], 16)
            .map_err(|_| adapter_error(format!("{context} contains invalid hexadecimal")))?;
    }
    Ok(decoded)
}

struct WireWriter {
    bytes: Vec<u8>,
}

impl WireWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn count(&mut self, value: usize, context: &str) -> Result<(), Diagnostic> {
        self.u32(
            u32::try_from(value)
                .map_err(|_| adapter_error(format!("{context} exceeds the u32 wire limit")))?,
        );
        Ok(())
    }

    fn index(&mut self, value: usize, context: &str) -> Result<(), Diagnostic> {
        self.count(value, context)
    }

    fn text(&mut self, value: &str, context: &str) -> Result<(), Diagnostic> {
        if value.is_empty() || value.contains('\0') {
            return Err(adapter_error(format!("{context} is empty or contains NUL")));
        }
        self.count(value.len(), context)?;
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }
}

fn adapter_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native adapter descriptor: {}", message.into()),
    )
}

#[cfg(test)]
#[path = "native_adapter_abi/tests.rs"]
mod tests;
