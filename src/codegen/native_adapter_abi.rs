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
mod tests {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::hir::{self, DeclarationId, ExpressionId, ResolvedFunction, ResolvedProgram};
    use crate::parse;

    use super::super::{native_cleanup, native_host_contract, native_resource, native_value};
    use super::*;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    const SOURCE: &str = r#"module test.native_adapter;

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("other.type")
resource Other { @id("other.drop") drop trivial; }

@id("token.mixed")
fn mixed(first: own Token, count: i64, enabled: bool, second: own Other) -> i64 { 0 }

@id("token.identity")
fn identity(count: i64, value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum ParsedParameter {
        Scalar {
            index: u32,
            value: String,
            kind: u32,
        },
        Owned {
            index: u32,
            value: String,
            ordinal: u32,
            resource: String,
            lifecycle: String,
        },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum ParsedResult {
        ScalarI64,
        Owned {
            index: u32,
            value: String,
            ordinal: u32,
        },
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ParsedDescriptor {
        target: String,
        schema: [u8; 32],
        target_fingerprint: [u8; 32],
        physical_module: [u8; 32],
        function_template: [u8; 32],
        module: String,
        function: String,
        parameters: Vec<ParsedParameter>,
        result: ParsedResult,
    }

    struct Reader<'a> {
        bytes: &'a [u8],
        offset: usize,
    }

    impl<'a> Reader<'a> {
        fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
            let end = self
                .offset
                .checked_add(length)
                .ok_or_else(|| "descriptor offset overflow".to_owned())?;
            let value = self
                .bytes
                .get(self.offset..end)
                .ok_or_else(|| "truncated descriptor".to_owned())?;
            self.offset = end;
            Ok(value)
        }

        fn u32(&mut self) -> Result<u32, String> {
            let bytes: [u8; 4] = self
                .take(4)?
                .try_into()
                .map_err(|_| "invalid u32 width".to_owned())?;
            Ok(u32::from_le_bytes(bytes))
        }

        fn text(&mut self) -> Result<String, String> {
            let length = usize::try_from(self.u32()?)
                .map_err(|_| "text length does not fit usize".to_owned())?;
            if length == 0 {
                return Err("empty descriptor identity".to_owned());
            }
            let bytes = self.take(length)?;
            if bytes.contains(&0) {
                return Err("descriptor identity contains NUL".to_owned());
            }
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|_| "descriptor identity is not UTF-8".to_owned())
        }

        fn fingerprint(&mut self) -> Result<[u8; 32], String> {
            self.take(32)?
                .try_into()
                .map_err(|_| "invalid fingerprint width".to_owned())
        }
    }

    fn parse_descriptor(bytes: &[u8]) -> Result<ParsedDescriptor, String> {
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(MAGIC.len())? != MAGIC {
            return Err("wrong descriptor magic".to_owned());
        }
        if reader.u32()? != VERSION {
            return Err("unsupported descriptor version".to_owned());
        }
        if reader.u32()? != HEADER_SIZE {
            return Err("unsupported descriptor header size".to_owned());
        }
        let declared = usize::try_from(reader.u32()?)
            .map_err(|_| "descriptor length does not fit usize".to_owned())?;
        if declared != bytes.len() {
            return Err("descriptor total length is not exact".to_owned());
        }
        let target = reader.text()?;
        let schema = reader.fingerprint()?;
        if schema != schema_fingerprint() {
            return Err("descriptor schema fingerprint is unknown".to_owned());
        }
        let target_fingerprint = reader.fingerprint()?;
        if target_fingerprint != super::target_fingerprint(target.as_bytes()) {
            return Err("descriptor target fingerprint is inconsistent".to_owned());
        }
        let physical_module = reader.fingerprint()?;
        let function_template = reader.fingerprint()?;
        let module = reader.text()?;
        let function = reader.text()?;
        let count = usize::try_from(reader.u32()?)
            .map_err(|_| "parameter count does not fit usize".to_owned())?;
        let mut parameters = Vec::with_capacity(count.min(1024));
        let mut next_owner = 0_u32;
        for expected in 0..count {
            let tag = reader.u32()?;
            let index = reader.u32()?;
            if index != u32::try_from(expected).map_err(|_| "too many parameters".to_owned())? {
                return Err("noncanonical parameter index".to_owned());
            }
            let value = reader.text()?;
            match tag {
                PARAMETER_SCALAR => {
                    let kind = reader.u32()?;
                    if !matches!(kind, SCALAR_I64 | SCALAR_BOOL) {
                        return Err("unknown scalar kind".to_owned());
                    }
                    parameters.push(ParsedParameter::Scalar { index, value, kind });
                }
                PARAMETER_OWNED_RESOURCE => {
                    let ordinal = reader.u32()?;
                    if ordinal != next_owner {
                        return Err("noncanonical owner ordinal".to_owned());
                    }
                    next_owner = next_owner
                        .checked_add(1)
                        .ok_or_else(|| "owner ordinal overflow".to_owned())?;
                    parameters.push(ParsedParameter::Owned {
                        index,
                        value,
                        ordinal,
                        resource: reader.text()?,
                        lifecycle: reader.text()?,
                    });
                }
                _ => return Err("unknown parameter tag".to_owned()),
            }
        }
        let result = match reader.u32()? {
            RESULT_SCALAR_I64 => ParsedResult::ScalarI64,
            RESULT_OWNED_INPUT => {
                let index = reader.u32()?;
                let value = reader.text()?;
                let ordinal = reader.u32()?;
                match parameters.get(index as usize) {
                    Some(ParsedParameter::Owned {
                        index: expected_index,
                        value: expected_value,
                        ordinal: expected_ordinal,
                        ..
                    }) if *expected_index == index
                        && *expected_value == value
                        && *expected_ordinal == ordinal => {}
                    _ => return Err("owned result mapping is not exact".to_owned()),
                }
                ParsedResult::Owned {
                    index,
                    value,
                    ordinal,
                }
            }
            _ => return Err("unknown result tag".to_owned()),
        };
        if reader.offset != bytes.len() {
            return Err("descriptor contains trailing bytes".to_owned());
        }
        Ok(ParsedDescriptor {
            target,
            schema,
            target_fingerprint,
            physical_module,
            function_template,
            module,
            function,
            parameters,
            result,
        })
    }

    fn program(source: &str) -> ResolvedProgram {
        let parsed = parse(source, Path::new("native-adapter.spx")).unwrap();
        hir::resolve(&parsed).unwrap()
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|candidate| candidate.id.as_str() == id)
            .unwrap()
    }

    fn descriptor(source: &str, id: &str) -> NativeAdapterDescriptor {
        let program = program(source);
        let function = function(&program, id);
        let abi = native_resource::build_resource_abi(&program).unwrap();
        let cleanup = native_cleanup::classify(&program, function).unwrap();
        let values = native_value::plan(
            &program,
            function,
            &cleanup,
            &abi,
            &HashMap::<ExpressionId, String>::new(),
        )
        .unwrap();
        let template = native_host_contract::derive_from_admitted(
            &program,
            &DeclarationId::new(id),
            &abi,
            &cleanup,
            &values,
        )
        .unwrap();
        derive(&template).unwrap()
    }

    #[test]
    fn descriptor_round_trips_complete_ordered_signature_and_result_metadata() {
        let mixed = descriptor(SOURCE, "token.mixed");
        let parsed = parse_descriptor(&mixed.bytes).unwrap();
        assert_eq!(parsed.target, physical_target_tag().unwrap());
        assert_eq!(parsed.parameters.len(), 4);
        assert!(matches!(
            &parsed.parameters[..],
            [
                ParsedParameter::Owned {
                    index: 0,
                    ordinal: 0,
                    ..
                },
                ParsedParameter::Scalar {
                    index: 1,
                    kind: SCALAR_I64,
                    ..
                },
                ParsedParameter::Scalar {
                    index: 2,
                    kind: SCALAR_BOOL,
                    ..
                },
                ParsedParameter::Owned {
                    index: 3,
                    ordinal: 1,
                    ..
                }
            ]
        ));
        assert_eq!(parsed.result, ParsedResult::ScalarI64);
        assert_eq!(
            u32::from_le_bytes(mixed.bytes[16..20].try_into().unwrap()) as usize,
            mixed.bytes.len()
        );
        assert_eq!(
            u32::from_le_bytes(mixed.bytes[12..16].try_into().unwrap()),
            HEADER_SIZE
        );

        let identity = descriptor(SOURCE, "token.identity");
        let parsed = parse_descriptor(&identity.bytes).unwrap();
        assert!(matches!(
            parsed.result,
            ParsedResult::Owned {
                index: 1,
                ordinal: 0,
                ..
            }
        ));
    }

    #[test]
    fn display_and_whitespace_do_not_change_bytes_but_physical_abi_changes_do() {
        let baseline = descriptor(SOURCE, "token.identity");
        let renamed = format!(
            "\n{}",
            SOURCE.replace("fn identity(", "fn renamed_identity(")
        );
        assert_eq!(baseline, descriptor(&renamed, "token.identity"));

        let scalar_changed = SOURCE.replace(
            "fn identity(count: i64, value: own Token)",
            "fn identity(count: bool, value: own Token)",
        );
        assert_ne!(baseline, descriptor(&scalar_changed, "token.identity"));

        let lifecycle_changed = SOURCE.replace("token.drop", "token.drop.v2");
        assert_ne!(baseline, descriptor(&lifecycle_changed, "token.identity"));
    }

    #[test]
    fn same_module_functions_have_distinct_deterministic_getters_and_ordered_bytes() {
        let identity = descriptor(SOURCE, "token.identity");
        let mixed = descriptor(SOURCE, "token.mixed");
        assert_ne!(identity.getter_symbol, mixed.getter_symbol);
        assert_ne!(identity.bytes, mixed.bytes);
        assert_eq!(identity, descriptor(SOURCE, "token.identity"));
        assert_eq!(mixed, descriptor(SOURCE, "token.mixed"));

        let parsed = parse_descriptor(&mixed.bytes).unwrap();
        assert_eq!(
            parsed
                .parameters
                .iter()
                .map(|parameter| match parameter {
                    ParsedParameter::Scalar { index, .. }
                    | ParsedParameter::Owned { index, .. } => *index,
                })
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
    }

    #[test]
    fn hostile_wire_inputs_fail_closed_without_repairs() {
        let descriptor = descriptor(SOURCE, "token.mixed");
        for length in 0..descriptor.bytes.len() {
            assert!(parse_descriptor(&descriptor.bytes[..length]).is_err());
        }
        let mut trailing = descriptor.bytes.clone();
        trailing.push(0);
        assert!(parse_descriptor(&trailing).is_err());

        for (offset, replacement) in [(0, 0_u8), (8, 2_u8), (12, 0_u8), (16, 0_u8)] {
            let mut hostile = descriptor.bytes.clone();
            hostile[offset] = replacement;
            assert!(parse_descriptor(&hostile).is_err());
        }

        let mut reader = Reader {
            bytes: &descriptor.bytes,
            offset: HEADER_SIZE as usize,
        };
        let _ = reader.text().unwrap();
        let _ = reader.take(32 * 4).unwrap();
        let _ = reader.text().unwrap();
        let _ = reader.text().unwrap();
        let count = reader.u32().unwrap();
        assert!(count > 0);
        let first_tag = reader.offset;
        let mut unknown_tag = descriptor.bytes.clone();
        unknown_tag[first_tag..first_tag + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_descriptor(&unknown_tag).is_err());

        let mut wrong_index = descriptor.bytes.clone();
        wrong_index[first_tag + 4..first_tag + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_descriptor(&wrong_index).is_err());

        let mut wrong_schema = descriptor.bytes.clone();
        let target_length = u32::from_le_bytes(wrong_schema[20..24].try_into().unwrap()) as usize;
        wrong_schema[24 + target_length] ^= 0x80;
        assert!(parse_descriptor(&wrong_schema).is_err());
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn create() -> Self {
            let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "semaprax-native-adapter-{}-{suffix}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            if std::fs::symlink_metadata(&self.path).is_ok() {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    fn compile(command: &mut Command, context: &str) {
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{context} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn strict_separate_c_and_cpp_translation_units_link_and_inspect_descriptor() {
        if Command::new("clang").arg("--version").output().is_err()
            || Command::new("clang++").arg("--version").output().is_err()
        {
            return;
        }
        let descriptor = descriptor(SOURCE, "token.identity");
        let directory = TestDirectory::create();
        let header = directory.path.join("adapter.h");
        let provider = directory.path.join("provider.c");
        let consumer = directory.path.join("consumer.c");
        let cpp = directory.path.join("consumer.cpp");
        std::fs::write(&header, emit_header(&descriptor)).unwrap();
        let provider_source = emit_source(&descriptor, "adapter.h").unwrap();
        std::fs::write(&provider, &provider_source).unwrap();
        std::fs::write(
            &consumer,
            format!(
                "#include <string.h>\n#pragma pack(push, 1)\n#include \"adapter.h\"\n#pragma pack(pop)\n\
                 extern int spx_cpp_inspect(void);\n\
                 static uint32_t read_u32(const unsigned char *p) {{\n\
                 return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);\n}}\n\
                 int main(void) {{\n\
                 const unsigned char *p = {symbol}();\n\
                 if (p == (const unsigned char *)0 || memcmp(p, \"SPXNABI1\", 8) != 0) return 1;\n\
                 if (read_u32(p + 8) != UINT32_C(1) || read_u32(p + 12) != UINT32_C(20) || read_u32(p + 16) != UINT32_C({length})) return 2;\n\
                 if (p != {symbol}()) return 3;\n\
                 return spx_cpp_inspect();\n}}\n",
                symbol = descriptor.getter_symbol,
                length = descriptor.bytes.len()
            ),
        )
        .unwrap();
        std::fs::write(
            &cpp,
            format!(
                "#pragma pack(push, 16)\n#include \"adapter.h\"\n#pragma pack(pop)\n\
                 extern \"C\" int spx_cpp_inspect(void) {{ return {symbol}() == nullptr ? 4 : 0; }}\n",
                symbol = descriptor.getter_symbol
            ),
        )
        .unwrap();

        let provider_object = directory.path.join("provider.o");
        let consumer_object = directory.path.join("consumer.o");
        let cpp_object = directory.path.join("consumer_cpp.o");
        let executable = directory.path.join(if cfg!(windows) {
            "adapter_test.exe"
        } else {
            "adapter_test"
        });
        compile(
            Command::new("clang")
                .arg("-std=c11")
                .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
                .arg("-fvisibility=hidden")
                .arg("-I")
                .arg(&directory.path)
                .arg("-c")
                .arg(&provider)
                .arg("-o")
                .arg(&provider_object),
            "provider compile",
        );

        let mut mismatched_lines = provider_source
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let guard_error = mismatched_lines
            .iter()
            .position(|line| line.contains("provider architecture mismatch"))
            .unwrap();
        assert!(guard_error > 0);
        mismatched_lines[guard_error - 1] =
            "#if !(defined(SPX_DELIBERATELY_WRONG_ARCHITECTURE))".to_owned();
        let mismatched_provider = directory.path.join("mismatched_provider.c");
        std::fs::write(&mismatched_provider, mismatched_lines.join("\n")).unwrap();
        let mismatch = Command::new("clang")
            .arg("-std=c11")
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
            .arg("-I")
            .arg(&directory.path)
            .arg("-c")
            .arg(&mismatched_provider)
            .arg("-o")
            .arg(directory.path.join("mismatched_provider.o"))
            .output()
            .unwrap();
        assert!(
            !mismatch.status.success(),
            "mismatched target guard compiled"
        );
        assert!(String::from_utf8_lossy(&mismatch.stderr)
            .contains("SEMAPRAX descriptor provider architecture mismatch"));
        compile(
            Command::new("clang")
                .arg("-std=c11")
                .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
                .arg("-I")
                .arg(&directory.path)
                .arg("-c")
                .arg(&consumer)
                .arg("-o")
                .arg(&consumer_object),
            "C consumer compile",
        );
        compile(
            Command::new("clang++")
                .arg("-std=c++17")
                .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
                .arg("-I")
                .arg(&directory.path)
                .arg("-c")
                .arg(&cpp)
                .arg("-o")
                .arg(&cpp_object),
            "C++ consumer compile",
        );
        compile(
            Command::new("clang++")
                .arg(&provider_object)
                .arg(&consumer_object)
                .arg(&cpp_object)
                .arg("-o")
                .arg(&executable),
            "descriptor link",
        );
        let executed = Command::new(&executable).output().unwrap();
        assert!(
            executed.status.success(),
            "descriptor consumer failed: {}",
            String::from_utf8_lossy(&executed.stderr)
        );

        if let Ok(symbols) = Command::new("nm").arg("-g").arg(&provider_object).output() {
            if symbols.status.success() {
                let symbols = String::from_utf8_lossy(&symbols.stdout);
                let adapter_symbols = symbols
                    .lines()
                    .filter(|line| line.contains("spx_"))
                    .collect::<Vec<_>>();
                assert_eq!(adapter_symbols.len(), 1, "unexpected exports: {symbols}");
                assert!(adapter_symbols[0].contains(&descriptor.getter_symbol));
            }
        }
    }

    #[test]
    fn shared_library_exports_only_getter_and_dynamic_consumer_runs() {
        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }
        let descriptor = descriptor(SOURCE, "token.identity");
        let directory = TestDirectory::create();
        let header = directory.path.join("adapter.h");
        let provider = directory.path.join("provider.c");
        let consumer = directory.path.join("dynamic_consumer.c");
        std::fs::write(&header, emit_header(&descriptor)).unwrap();
        std::fs::write(&provider, emit_source(&descriptor, "adapter.h").unwrap()).unwrap();
        std::fs::write(
            &consumer,
            format!(
                "#if defined(_WIN32)\n#define SPX_ADAPTER_DESCRIPTOR_DLL 1\n#endif\n\
                 #include <string.h>\n#include \"adapter.h\"\n\
                 static uint32_t read_u32(const unsigned char *p) {{\n\
                 return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);\n}}\n\
                 int main(void) {{\n\
                 const unsigned char *p = {symbol}();\n\
                 if (p == (const unsigned char *)0 || memcmp(p, \"SPXNABI1\", 8) != 0) return 1;\n\
                 if (read_u32(p + 8) != UINT32_C(1) || read_u32(p + 12) != UINT32_C(20) || read_u32(p + 16) != UINT32_C({length})) return 2;\n\
                 return p == {symbol}() ? 0 : 3;\n}}\n",
                symbol = descriptor.getter_symbol,
                length = descriptor.bytes.len()
            ),
        )
        .unwrap();

        let executable = directory.path.join(if cfg!(windows) {
            "dynamic_consumer.exe"
        } else {
            "dynamic_consumer"
        });
        let library = if cfg!(windows) {
            directory.path.join("adapter.dll")
        } else if cfg!(target_os = "macos") {
            directory.path.join("libadapter.dylib")
        } else {
            directory.path.join("libadapter.so")
        };
        let import_library = directory.path.join("adapter.lib");

        let mut shared = Command::new("clang");
        shared
            .arg("-std=c11")
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
            .arg("-fvisibility=hidden")
            .arg("-I")
            .arg(&directory.path);
        if cfg!(target_os = "macos") {
            shared.arg("-dynamiclib").arg("-fPIC");
        } else {
            shared.arg("-shared");
            if !cfg!(windows) {
                shared.arg("-fPIC");
            }
        }
        shared.arg(&provider).arg("-o").arg(&library);
        if cfg!(windows) {
            shared.arg(format!("-Wl,/implib:{}", import_library.display()));
        }
        compile(&mut shared, "shared descriptor provider build");

        assert_dynamic_export_allowlist(&library, &descriptor.getter_symbol);

        let mut consumer_compile = Command::new("clang");
        consumer_compile
            .arg("-std=c11")
            .args(["-Wall", "-Wextra", "-Werror", "-pedantic"])
            .arg("-I")
            .arg(&directory.path)
            .arg(&consumer);
        if cfg!(windows) {
            consumer_compile.arg(&import_library);
        } else {
            consumer_compile
                .arg("-L")
                .arg(&directory.path)
                .arg("-ladapter")
                .arg(format!("-Wl,-rpath,{}", directory.path.display()));
        }
        consumer_compile.arg("-o").arg(&executable);
        compile(&mut consumer_compile, "dynamic descriptor consumer build");
        let executed = Command::new(&executable)
            .current_dir(&directory.path)
            .output()
            .unwrap();
        assert!(
            executed.status.success(),
            "dynamic descriptor consumer failed:\n{}\n{}",
            String::from_utf8_lossy(&executed.stdout),
            String::from_utf8_lossy(&executed.stderr)
        );
    }

    fn assert_dynamic_export_allowlist(library: &Path, getter: &str) {
        if cfg!(windows) {
            if let Ok(output) = Command::new("llvm-readobj")
                .arg("--coff-exports")
                .arg(library)
                .output()
            {
                if output.status.success() {
                    let names = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter_map(|line| line.trim().strip_prefix("Name: "))
                        .map(str::to_owned)
                        .collect::<Vec<_>>();
                    assert_eq!(names, vec![getter.to_owned()]);
                    return;
                }
            }
            if let Ok(output) = Command::new("dumpbin")
                .arg("/exports")
                .arg(library)
                .output()
            {
                if output.status.success() {
                    let names = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter_map(|line| {
                            let columns = line.split_whitespace().collect::<Vec<_>>();
                            (columns.len() == 4
                                && columns[0].bytes().all(|byte| byte.is_ascii_digit())
                                && columns[1].bytes().all(|byte| byte.is_ascii_hexdigit())
                                && columns[2].bytes().all(|byte| byte.is_ascii_hexdigit()))
                            .then(|| columns[3].to_owned())
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(names, vec![getter.to_owned()]);
                }
            }
            return;
        }

        let mut command = Command::new("nm");
        if cfg!(target_os = "macos") {
            command.args(["-gU", "-j"]);
        } else {
            command.args(["-D", "--defined-only", "--format=posix"]);
        }
        let Ok(output) = command.arg(library).output() else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let names = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let name = if cfg!(target_os = "macos") {
                    line.trim().trim_start_matches('_')
                } else {
                    line.split_whitespace().next().unwrap_or_default()
                };
                (!name.is_empty()).then(|| name.to_owned())
            })
            .collect::<Vec<_>>();
        assert_eq!(names, vec![getter.to_owned()]);
    }
}
