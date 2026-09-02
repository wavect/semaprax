//! C11 request/response provider scaffold for callable ABI v2 bundles.
//!
//! This emitter deliberately stops at a direct, translation-unit-local hook.
//! The verified value/cleanup emitter fills that hook for build-only public
//! bundles. `SPX-B104` still blocks ordinary native resource execution. There
//! is no function pointer, allocation, callback, or independently selectable
//! runtime operation in the generated surface.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::native_callable_abi::{
    CALL_RESULT_COMPLETE, CALL_RESULT_INTERNAL_FAILURE, CALL_RESULT_INVALID_REQUEST,
    CALL_RESULT_RESPONSE_CAPACITY, RESPONSE_OUTCOME_FAILURE, RESPONSE_OUTCOME_SUCCESS,
};
use crate::diagnostic::Diagnostic;

const REQUEST_MAGIC: &[u8; 8] = b"SPXNREQ1";
const RESPONSE_MAGIC: &[u8; 8] = b"SPXNRSP1";
const WIRE_VERSION: u32 = 1;
const HEADER_SIZE: u32 = 20;
const REQUEST_FIXED_BYTES: u32 = 64;
const RESPONSE_FIXED_BYTES: u32 = 68;
const REQUEST_I64_BYTES: u32 = 16;
const REQUEST_BOOL_BYTES: u32 = 12;
const REQUEST_OWNER_BYTES: u32 = 20;
const RESPONSE_SCALAR_BYTES: u32 = 12;
const RESPONSE_OWNER_BYTES: u32 = 8;
const RESPONSE_FAILURE_BYTES: u32 = 4;
const EVENT_BYTES: u32 = 4;
const MAX_CALL_WIRE_BYTES: u32 = 1024 * 1024;
const MAX_EVENT_COUNT: u32 = 65_536;
/// Audited production ceiling for the two fixed-size event arrays allocated by
/// the callable wrapper and execution hook. The protocol permits more events,
/// but this first native lane deliberately caps stack use well below that wire
/// ceiling until a non-stack backing design is verified.
pub(super) const MAX_PROVIDER_STACK_EVENTS: u32 = 256;
const MAX_SYMBOL_BYTES: usize = 1024;

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED: u32 = 2;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

const HOOK_RESULT_COMPLETE: u32 = 0;

const CODEC_PROFILE_DOMAIN: &[u8] = b"semaprax.native-callable-provider-codec.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderParameter {
    I64,
    Bool,
    Owned { owner_ordinal: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderResult {
    ScalarI64 {
        result_commit_ordinal: u32,
    },
    OwnedInput {
        owner_ordinal: u32,
        result_commit_ordinal: u32,
    },
}

/// Sealed-at-module-boundary facts consumed by the staged provider emitter.
/// Integration must construct these from the exact compiler descriptor and
/// semantic dictionary, never from adapter assertions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableProviderSpec {
    callable_symbol: String,
    call_contract: [u8; 32],
    parameters: Vec<ProviderParameter>,
    result: ProviderResult,
    max_event_count: u32,
    dictionary_entries: u32,
}

impl NativeCallableProviderSpec {
    pub(super) fn new(
        callable_symbol: String,
        call_contract: [u8; 32],
        parameters: Vec<ProviderParameter>,
        result: ProviderResult,
        max_event_count: u32,
        dictionary_entries: u32,
    ) -> Result<Self, Diagnostic> {
        if !is_c_symbol(&callable_symbol) || callable_symbol.len() > MAX_SYMBOL_BYTES {
            return Err(provider_error(
                "callable symbol is not a bounded C identifier",
            ));
        }
        if call_contract.iter().all(|byte| *byte == 0) {
            return Err(provider_error("call-contract fingerprint is uninitialized"));
        }
        if max_event_count == 0 || max_event_count > MAX_PROVIDER_STACK_EVENTS {
            return Err(provider_error("maximum event count is outside callable v2"));
        }
        if dictionary_entries == 0 || dictionary_entries > MAX_EVENT_COUNT {
            return Err(provider_error(
                "dictionary entry count is outside callable v2",
            ));
        }
        let mut next_owner = 0_u32;
        for parameter in &parameters {
            if let ProviderParameter::Owned { owner_ordinal } = parameter {
                if *owner_ordinal != next_owner {
                    return Err(provider_error("owned parameter ordinals are not dense"));
                }
                next_owner = next_owner
                    .checked_add(1)
                    .ok_or_else(|| provider_error("owned parameter ordinal overflow"))?;
            }
        }
        match result {
            ProviderResult::ScalarI64 {
                result_commit_ordinal,
            } => {
                require_dictionary_ordinal(result_commit_ordinal, dictionary_entries)?;
            }
            ProviderResult::OwnedInput {
                owner_ordinal,
                result_commit_ordinal,
            } => {
                if owner_ordinal >= next_owner {
                    return Err(provider_error(
                        "owned result does not select an owned parameter",
                    ));
                }
                require_dictionary_ordinal(result_commit_ordinal, dictionary_entries)?;
            }
        }
        request_capacity(&parameters)?;
        response_capacity(result, max_event_count)?;
        Ok(Self {
            callable_symbol,
            call_contract,
            parameters,
            result,
            max_event_count,
            dictionary_entries,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableProvider {
    pub(super) source: String,
    pub(super) hook_symbol: String,
    pub(super) target_guards: String,
    pub(super) codec_profile_fingerprint: [u8; 32],
    pub(super) request_bytes: u32,
    pub(super) response_bytes: u32,
}

pub(super) fn emit(
    spec: &NativeCallableProviderSpec,
) -> Result<NativeCallableProvider, Diagnostic> {
    // Revalidate derived capacities so no future alternate constructor can
    // bypass the exact wire equations.
    let request_bytes = request_capacity(&spec.parameters)?;
    let response_bytes = response_capacity(spec.result, spec.max_event_count)?;
    let hook_symbol = format!("{}_generated_hook", spec.callable_symbol);
    if hook_symbol.len() > MAX_SYMBOL_BYTES || !is_c_symbol(&hook_symbol) {
        return Err(provider_error(
            "generated hook symbol is not a bounded C identifier",
        ));
    }
    let target_guards = provider_target_guards()?;

    let mut source = String::new();
    emit_prelude(
        &mut source,
        spec,
        &hook_symbol,
        request_bytes,
        response_bytes,
        &target_guards,
    );
    emit_callable(
        &mut source,
        spec,
        &hook_symbol,
        request_bytes,
        response_bytes,
    )?;
    Ok(NativeCallableProvider {
        source,
        hook_symbol,
        target_guards,
        codec_profile_fingerprint: codec_profile_fingerprint(),
        request_bytes,
        response_bytes,
    })
}

fn emit_prelude(
    output: &mut String,
    spec: &NativeCallableProviderSpec,
    hook_symbol: &str,
    request_bytes: u32,
    response_bytes: u32,
    target_guards: &str,
) {
    output.push_str(
        "/* semaprax.native-callable-provider.v1; build-only, execution behind SPX-B104 */\n\
         #include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\n#include <string.h>\n\
         ",
    );
    output.push_str(target_guards);
    output.push_str(
        "#if defined(__cplusplus)\n#define SPX_PROVIDER_STATIC_ASSERT(c, m) static_assert((c), m)\nextern \"C\" {\n\
         #else\n#define SPX_PROVIDER_STATIC_ASSERT(c, m) _Static_assert((c), m)\n#endif\n\
         #if defined(_WIN32)\n#define SPX_PROVIDER_API __declspec(dllexport)\n#define SPX_PROVIDER_CALL __cdecl\n\
         #elif defined(__GNUC__) || defined(__clang__)\n#define SPX_PROVIDER_API __attribute__((visibility(\"default\")))\n#define SPX_PROVIDER_CALL\n\
         #else\n#define SPX_PROVIDER_API\n#define SPX_PROVIDER_CALL\n#endif\n\
         SPX_PROVIDER_STATIC_ASSERT(sizeof(uint8_t) == 1, \"SEMAPRAX requires exact uint8_t\");\n\
         SPX_PROVIDER_STATIC_ASSERT(sizeof(uint32_t) == 4, \"SEMAPRAX requires exact uint32_t\");\n\
         SPX_PROVIDER_STATIC_ASSERT(sizeof(uint64_t) == 8, \"SEMAPRAX requires exact uint64_t\");\n\
         SPX_PROVIDER_STATIC_ASSERT(sizeof(int64_t) == 8, \"SEMAPRAX requires exact int64_t\");\n",
    );
    writeln!(
        output,
        "#define SPX_PROVIDER_MAX_EVENTS UINT32_C({})",
        spec.max_event_count
    )
    .expect("writing cannot fail");
    writeln!(
        output,
        "#define SPX_PROVIDER_DICTIONARY_ENTRIES UINT32_C({})",
        spec.dictionary_entries
    )
    .expect("writing cannot fail");
    writeln!(
        output,
        "#define SPX_PROVIDER_REQUEST_BYTES UINT32_C({request_bytes})"
    )
    .expect("writing cannot fail");
    writeln!(
        output,
        "#define SPX_PROVIDER_RESPONSE_BYTES UINT32_C({response_bytes})"
    )
    .expect("writing cannot fail");
    for (name, value) in [
        ("SPX_CALL_COMPLETE", CALL_RESULT_COMPLETE),
        ("SPX_CALL_INVALID_REQUEST", CALL_RESULT_INVALID_REQUEST),
        ("SPX_CALL_RESPONSE_CAPACITY", CALL_RESULT_RESPONSE_CAPACITY),
        ("SPX_CALL_INTERNAL_FAILURE", CALL_RESULT_INTERNAL_FAILURE),
        ("SPX_OUTCOME_SUCCESS", RESPONSE_OUTCOME_SUCCESS),
        ("SPX_OUTCOME_FAILURE", RESPONSE_OUTCOME_FAILURE),
    ] {
        writeln!(output, "#define {name} UINT32_C({value})").expect("writing cannot fail");
    }
    output.push_str(
        "struct spx_provider_execution {\n\
         uint32_t outcome;\nint64_t scalar_result;\nuint32_t owned_result_ordinal;\n\
         uint32_t selected_failure_ordinal;\nuint32_t event_count;\n\
         uint32_t event_ordinals[SPX_PROVIDER_MAX_EVENTS];\n};\n",
    );
    write!(
        output,
        "static uint32_t SPX_PROVIDER_CALL {hook_symbol}(uint64_t spx_invocation"
    )
    .expect("writing cannot fail");
    for (index, parameter) in spec.parameters.iter().enumerate() {
        output.push_str(", ");
        let c_type = match parameter {
            ProviderParameter::I64 => "int64_t",
            ProviderParameter::Bool => "bool",
            ProviderParameter::Owned { .. } => "uint64_t",
        };
        write!(output, "{c_type} spx_arg_{index}").expect("writing cannot fail");
    }
    output.push_str(", struct spx_provider_execution *spx_execution);\n");
    output.push_str(
        "static inline uint32_t spx_load_u32(const uint8_t *p) {\n\
             return ((uint32_t)p[0]) | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);\n}\n\
         static inline uint64_t spx_load_u64(const uint8_t *p) {\n\
             return ((uint64_t)spx_load_u32(p)) | ((uint64_t)spx_load_u32(p + 4) << 32);\n}\n\
         static inline void spx_store_u32(uint8_t *p, uint32_t v) {\n\
             p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24);\n}\n\
         static inline bool spx_take(const uint8_t *base, uint32_t limit, uint32_t *offset, uint32_t amount, const uint8_t **field) {\n\
             if (*offset > limit || amount > limit - *offset) return false;\n\
             *field = base + *offset; *offset += amount; return true;\n}\n",
    );
    if spec
        .parameters
        .iter()
        .any(|parameter| matches!(parameter, ProviderParameter::I64))
    {
        output.push_str(
            "static inline int64_t spx_load_i64(const uint8_t *p) {\n\
                 uint64_t bits = spx_load_u64(p); int64_t value; memcpy(&value, &bits, sizeof(value)); return value;\n}\n",
        );
    }
    if matches!(spec.result, ProviderResult::ScalarI64 { .. }) {
        output.push_str(
            "static inline void spx_store_u64(uint8_t *p, uint64_t v) {\n\
                 spx_store_u32(p, (uint32_t)v); spx_store_u32(p + 4, (uint32_t)(v >> 32));\n}\n\
             static inline void spx_store_i64(uint8_t *p, int64_t v) {\n\
                 uint64_t bits; memcpy(&bits, &v, sizeof(bits)); spx_store_u64(p, bits);\n}\n",
        );
    }
}

#[derive(Clone, Copy)]
struct ProviderTargetGuardSpec {
    includes: &'static str,
    architecture: &'static str,
    operating_system: &'static str,
    environment: &'static str,
    object_format: &'static str,
    pointer_width: &'static str,
    endian: ProviderEndianGuard,
}

#[derive(Clone, Copy)]
enum ProviderEndianGuard {
    Little,
    Big,
}

/// Closed physical iOS-family target selector for private statically linked
/// callable-v3 fixtures. It is deliberately separate from the build target so
/// cross-target fixture generation never inherits ambient compiler cfgs.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IosProviderPhysicalTarget {
    DeviceArm64,
    SimulatorArm64,
    SimulatorX86_64,
    MacCatalystArm64,
    MacCatalystX86_64,
}

/// Closed physical Android target selector for private dynamically loaded
/// callable-v3 fixtures. The role names identify the evidence lanes; the
/// authenticated physical tags remain architecture/OS/environment/object
/// identities and never depend on the host that generates the fixture.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AndroidProviderPhysicalTarget {
    Arm64,
    EmulatorX86_64,
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
impl AndroidProviderPhysicalTarget {
    pub(super) const fn canonical_tag(self) -> &'static str {
        match self {
            Self::Arm64 => "aarch64-android-android-elf-ptr64-little-callable-v3",
            Self::EmulatorX86_64 => "x86_64-android-android-elf-ptr64-little-callable-v3",
        }
    }
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
impl IosProviderPhysicalTarget {
    pub(super) const fn canonical_tag(self) -> &'static str {
        match self {
            Self::DeviceArm64 => "aarch64-ios-device-apple-macho-ptr64-little-callable-v3",
            Self::SimulatorArm64 => "aarch64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            Self::SimulatorX86_64 => "x86_64-ios-simulator-apple-macho-ptr64-little-callable-v3",
            Self::MacCatalystArm64 => "aarch64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
            Self::MacCatalystX86_64 => "x86_64-ios-catalyst-apple-macho-ptr64-little-callable-v3",
        }
    }
}

/// Emit the exact C preprocessor proof paired with one iOS-static descriptor.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
pub(super) fn ios_provider_target_guards(target: IosProviderPhysicalTarget) -> String {
    let architecture = match target {
        IosProviderPhysicalTarget::DeviceArm64
        | IosProviderPhysicalTarget::SimulatorArm64
        | IosProviderPhysicalTarget::MacCatalystArm64 => {
            "(defined(__aarch64__) || defined(__arm64__)) && !defined(__x86_64__) && !defined(__i386__)"
        }
        IosProviderPhysicalTarget::SimulatorX86_64
        | IosProviderPhysicalTarget::MacCatalystX86_64 => {
            "defined(__x86_64__) && !defined(__aarch64__) && !defined(__arm64__) && !defined(__arm__)"
        }
    };
    let operating_system = match target {
        IosProviderPhysicalTarget::DeviceArm64 => {
            "defined(__APPLE__) && defined(__MACH__) && defined(TARGET_OS_IOS) && TARGET_OS_IOS && defined(TARGET_OS_SIMULATOR) && !TARGET_OS_SIMULATOR && defined(TARGET_OS_MACCATALYST) && !TARGET_OS_MACCATALYST"
        }
        IosProviderPhysicalTarget::SimulatorArm64
        | IosProviderPhysicalTarget::SimulatorX86_64 => {
            "defined(__APPLE__) && defined(__MACH__) && defined(TARGET_OS_IOS) && TARGET_OS_IOS && defined(TARGET_OS_SIMULATOR) && TARGET_OS_SIMULATOR && defined(TARGET_OS_MACCATALYST) && !TARGET_OS_MACCATALYST"
        }
        IosProviderPhysicalTarget::MacCatalystArm64
        | IosProviderPhysicalTarget::MacCatalystX86_64 => {
            "defined(__APPLE__) && defined(__MACH__) && defined(TARGET_OS_IOS) && TARGET_OS_IOS && defined(TARGET_OS_SIMULATOR) && !TARGET_OS_SIMULATOR && defined(TARGET_OS_MACCATALYST) && TARGET_OS_MACCATALYST"
        }
    };
    render_provider_target_guards(ProviderTargetGuardSpec {
        includes: "#include <TargetConditionals.h>\n",
        architecture,
        operating_system,
        environment: "defined(__APPLE__) && !defined(_WIN32)",
        object_format: "defined(__MACH__) && !defined(__ELF__)",
        pointer_width: "UINTPTR_MAX == UINT64_MAX",
        endian: ProviderEndianGuard::Little,
    })
}

/// Emit the exact C preprocessor proof paired with one private Android
/// dynamic-image descriptor. API 21 is the first 64-bit Android API level and
/// is treated as a minimum compatibility floor, not as part of the target tag.
#[cfg(any(test, feature = "unstable-native-host-internal"))]
pub(super) fn android_provider_target_guards(target: AndroidProviderPhysicalTarget) -> String {
    let architecture = match target {
        AndroidProviderPhysicalTarget::Arm64 => {
            "(defined(__aarch64__) || defined(__arm64__)) && !defined(__x86_64__) && !defined(__i386__) && !defined(__arm__)"
        }
        AndroidProviderPhysicalTarget::EmulatorX86_64 => {
            "defined(__x86_64__) && !defined(__aarch64__) && !defined(__arm64__) && !defined(__arm__) && !defined(__i386__)"
        }
    };
    render_provider_target_guards(ProviderTargetGuardSpec {
        includes: "",
        architecture,
        operating_system:
            "defined(__linux__) && defined(__ANDROID__) && !defined(__APPLE__) && !defined(_WIN32)",
        environment: "defined(__ANDROID__) && defined(__BIONIC__) && !defined(__GLIBC__) && defined(__ANDROID_API__) && (__ANDROID_API__ >= 21)",
        object_format: "defined(__ELF__) && !defined(__MACH__) && !defined(_WIN32)",
        pointer_width: "UINTPTR_MAX == UINT64_MAX",
        endian: ProviderEndianGuard::Little,
    })
}

/// Emit a preprocessor proof that the C compiler is materializing the exact
/// physical target authenticated by callable descriptor v2. Unsupported or
/// unprovable Rust targets fail during provider derivation; a mismatched C
/// compiler fails before producing an object file. These guards are not
/// Windows loader, dependency-collision, or runtime-call evidence.
pub(super) fn provider_target_guards() -> Result<String, Diagnostic> {
    let architecture = if cfg!(target_arch = "x86_64") {
        "defined(__x86_64__) || defined(_M_X64) || defined(_M_AMD64)"
    } else if cfg!(target_arch = "x86") {
        "defined(__i386__) || defined(_M_IX86)"
    } else if cfg!(target_arch = "aarch64") {
        "defined(__aarch64__) || defined(__arm64__) || defined(_M_ARM64)"
    } else if cfg!(target_arch = "arm") {
        "defined(__arm__) || defined(_M_ARM)"
    } else if cfg!(target_arch = "riscv64") {
        "defined(__riscv) && defined(__riscv_xlen) && (__riscv_xlen == 64)"
    } else {
        return Err(provider_error(
            "target guard does not support this Rust architecture",
        ));
    };
    let (includes, operating_system) = if cfg!(windows) {
        ("", "defined(_WIN32)")
    } else if cfg!(target_os = "macos") {
        (
            "#include <TargetConditionals.h>\n",
            "defined(__APPLE__) && defined(__MACH__) && TARGET_OS_OSX",
        )
    } else if cfg!(target_os = "ios") {
        (
            "#include <TargetConditionals.h>\n",
            "defined(__APPLE__) && defined(__MACH__) && TARGET_OS_IOS",
        )
    } else if cfg!(target_os = "linux") {
        (
            "#include <features.h>\n",
            "defined(__linux__) && !defined(__ANDROID__)",
        )
    } else {
        return Err(provider_error(
            "target guard does not support this Rust operating system",
        ));
    };
    let environment = if cfg!(target_env = "msvc") {
        "defined(_MSC_VER) && !defined(__MINGW32__) && !defined(__MINGW64__)"
    } else if cfg!(all(windows, target_env = "gnu")) {
        "!defined(_MSC_VER) && (defined(__MINGW32__) || defined(__MINGW64__))"
    } else if cfg!(all(target_os = "linux", target_env = "gnu")) {
        "defined(__GLIBC__)"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "defined(__APPLE__)"
    } else {
        return Err(provider_error(
            "target guard cannot prove this Rust target environment",
        ));
    };
    let object_format = if cfg!(windows) {
        "defined(_WIN32) && !defined(__ELF__) && !defined(__MACH__)"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "defined(__MACH__) && !defined(__ELF__)"
    } else if cfg!(target_os = "linux") {
        "defined(__ELF__) && !defined(__MACH__)"
    } else {
        return Err(provider_error(
            "target guard cannot prove this Rust target object format",
        ));
    };
    let pointer_width = match usize::BITS {
        32 => "UINTPTR_MAX == UINT32_MAX",
        64 => "UINTPTR_MAX == UINT64_MAX",
        _ => {
            return Err(provider_error(
                "target guard does not support this Rust pointer width",
            ))
        }
    };
    let endian = if cfg!(target_endian = "little") {
        ProviderEndianGuard::Little
    } else if cfg!(target_endian = "big") {
        ProviderEndianGuard::Big
    } else {
        return Err(provider_error(
            "target guard does not support this Rust byte order",
        ));
    };
    Ok(render_provider_target_guards(ProviderTargetGuardSpec {
        includes,
        architecture,
        operating_system,
        environment,
        object_format,
        pointer_width,
        endian,
    }))
}

fn render_provider_target_guards(spec: ProviderTargetGuardSpec) -> String {
    let endian = match spec.endian {
        ProviderEndianGuard::Little => {
            "#if defined(_MSC_VER)\n\
             /* Supported MSVC architectures are intrinsically little-endian;\n\
                GNU byte-order builtins are neither required nor assumed. */\n\
             #elif defined(__BYTE_ORDER__) && defined(__ORDER_LITTLE_ENDIAN__)\n\
             # if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__\n\
             #  error \"SEMAPRAX callable provider endian mismatch\"\n\
             # endif\n\
             #else\n\
             # error \"SEMAPRAX callable provider cannot prove little endian\"\n\
             #endif\n"
        }
        ProviderEndianGuard::Big => {
            "#if defined(_MSC_VER)\n\
             # error \"SEMAPRAX callable provider endian mismatch\"\n\
             #elif !defined(__BYTE_ORDER__) || !defined(__ORDER_BIG_ENDIAN__) || (__BYTE_ORDER__ != __ORDER_BIG_ENDIAN__)\n\
             # error \"SEMAPRAX callable provider cannot prove big endian\"\n\
             #endif\n"
        }
    };
    format!(
        "{}#if !({})\n\
         # error \"SEMAPRAX callable provider architecture mismatch\"\n\
         #endif\n\
         #if !({})\n\
         # error \"SEMAPRAX callable provider operating-system mismatch\"\n\
         #endif\n\
         #if !({})\n\
         # error \"SEMAPRAX callable provider environment mismatch\"\n\
         #endif\n\
         #if !({})\n\
         # error \"SEMAPRAX callable provider object-format mismatch\"\n\
         #endif\n\
         #if !({})\n\
         # error \"SEMAPRAX callable provider pointer-width mismatch\"\n\
         #endif\n\
         {}",
        spec.includes,
        spec.architecture,
        spec.operating_system,
        spec.environment,
        spec.object_format,
        spec.pointer_width,
        endian
    )
}

fn emit_callable(
    output: &mut String,
    spec: &NativeCallableProviderSpec,
    hook_symbol: &str,
    request_bytes: u32,
    response_bytes: u32,
) -> Result<(), Diagnostic> {
    writeln!(
        output,
        "SPX_PROVIDER_API uint32_t SPX_PROVIDER_CALL {}(const uint8_t *request, uint32_t request_len, uint8_t *response, uint32_t response_capacity) {{",
        spec.callable_symbol
    )
    .expect("writing cannot fail");
    writeln!(
        output,
        "    if (request == NULL || response == NULL || request_len != UINT32_C({request_bytes})) return SPX_CALL_INVALID_REQUEST;"
    )
    .expect("writing cannot fail");
    writeln!(
        output,
        "    if (response_capacity != UINT32_C({response_bytes})) return SPX_CALL_RESPONSE_CAPACITY;"
    )
    .expect("writing cannot fail");
    output.push_str(
        "    if (memcmp(request, \"SPXNREQ1\", 8) != 0\n\
             || spx_load_u32(request + 8) != UINT32_C(1)\n\
             || spx_load_u32(request + 12) != UINT32_C(20)\n\
             || spx_load_u32(request + 16) != request_len) return SPX_CALL_INVALID_REQUEST;\n",
    );
    output.push_str("    static const uint8_t spx_call_contract[32] = {");
    for byte in spec.call_contract {
        write!(output, "0x{byte:02x},").expect("writing cannot fail");
    }
    output.push_str("};\n");
    output.push_str("    uint64_t spx_invocation = spx_load_u64(request + 52);\n");
    writeln!(
        output,
        "    if (memcmp(request + 20, spx_call_contract, 32) != 0 || spx_invocation == UINT64_C(0) || spx_load_u32(request + 60) != UINT32_C({})) return SPX_CALL_INVALID_REQUEST;",
        spec.parameters.len()
    )
    .expect("writing cannot fail");
    output.push_str(
        "    uint32_t spx_offset = UINT32_C(64);\n    const uint8_t *spx_field = NULL;\n",
    );
    for (index, parameter) in spec.parameters.iter().enumerate() {
        output.push_str("    if (!spx_take(request, request_len, &spx_offset, UINT32_C(8), &spx_field)) return SPX_CALL_INVALID_REQUEST;\n");
        let tag = if matches!(parameter, ProviderParameter::Owned { .. }) {
            PARAMETER_OWNED
        } else {
            PARAMETER_SCALAR
        };
        writeln!(
            output,
            "    if (spx_load_u32(spx_field) != UINT32_C({tag}) || spx_load_u32(spx_field + 4) != UINT32_C({index})) return SPX_CALL_INVALID_REQUEST;"
        )
        .expect("writing cannot fail");
        match parameter {
            ProviderParameter::I64 => {
                output.push_str("    if (!spx_take(request, request_len, &spx_offset, UINT32_C(8), &spx_field)) return SPX_CALL_INVALID_REQUEST;\n");
                writeln!(
                    output,
                    "    int64_t spx_arg_{index} = spx_load_i64(spx_field);"
                )
                .expect("writing cannot fail");
            }
            ProviderParameter::Bool => {
                output.push_str("    if (!spx_take(request, request_len, &spx_offset, UINT32_C(4), &spx_field)) return SPX_CALL_INVALID_REQUEST;\n");
                writeln!(
                    output,
                    "    uint32_t spx_bool_{index} = spx_load_u32(spx_field);"
                )
                .expect("writing cannot fail");
                writeln!(
                    output,
                    "    if (spx_bool_{index} > UINT32_C(1)) return SPX_CALL_INVALID_REQUEST;"
                )
                .expect("writing cannot fail");
                writeln!(
                    output,
                    "    bool spx_arg_{index} = spx_bool_{index} != UINT32_C(0);"
                )
                .expect("writing cannot fail");
            }
            ProviderParameter::Owned { owner_ordinal } => {
                output.push_str("    if (!spx_take(request, request_len, &spx_offset, UINT32_C(12), &spx_field)) return SPX_CALL_INVALID_REQUEST;\n");
                writeln!(output, "    if (spx_load_u32(spx_field) != UINT32_C({owner_ordinal})) return SPX_CALL_INVALID_REQUEST;")
                    .expect("writing cannot fail");
                writeln!(
                    output,
                    "    uint64_t spx_arg_{index} = spx_load_u64(spx_field + 4);"
                )
                .expect("writing cannot fail");
            }
        }
    }
    output.push_str("    if (spx_offset != request_len) return SPX_CALL_INVALID_REQUEST;\n");
    output.push_str(
        "    struct spx_provider_execution spx_execution = {0};\n    uint32_t spx_hook_result = ",
    );
    write!(output, "{hook_symbol}(spx_invocation").expect("writing cannot fail");
    for index in 0..spec.parameters.len() {
        output.push_str(", ");
        write!(output, "spx_arg_{index}").expect("writing cannot fail");
    }
    output.push_str(", &spx_execution);\n");
    writeln!(
        output,
        "    if (spx_hook_result != UINT32_C({HOOK_RESULT_COMPLETE})) return SPX_CALL_INTERNAL_FAILURE;"
    )
    .expect("writing cannot fail");
    output.push_str("    if (spx_execution.event_count == UINT32_C(0) || spx_execution.event_count > SPX_PROVIDER_MAX_EVENTS) return SPX_CALL_INTERNAL_FAILURE;\n");
    output.push_str("    for (uint32_t i = 0; i < spx_execution.event_count; ++i) { if (spx_execution.event_ordinals[i] == UINT32_C(0) || spx_execution.event_ordinals[i] > SPX_PROVIDER_DICTIONARY_ENTRIES) return SPX_CALL_INTERNAL_FAILURE; }\n");
    output.push_str("    uint32_t spx_payload_bytes = UINT32_C(0);\n");
    match spec.result {
        ProviderResult::ScalarI64 {
            result_commit_ordinal,
        } => {
            writeln!(output, "    if (spx_execution.outcome == SPX_OUTCOME_SUCCESS) {{ if (spx_execution.event_ordinals[spx_execution.event_count - 1] != UINT32_C({result_commit_ordinal})) return SPX_CALL_INTERNAL_FAILURE; spx_payload_bytes = UINT32_C(12); }}")
                .expect("writing cannot fail");
        }
        ProviderResult::OwnedInput {
            owner_ordinal,
            result_commit_ordinal,
        } => {
            writeln!(output, "    if (spx_execution.outcome == SPX_OUTCOME_SUCCESS) {{ if (spx_execution.owned_result_ordinal != UINT32_C({owner_ordinal}) || spx_execution.event_ordinals[spx_execution.event_count - 1] != UINT32_C({result_commit_ordinal})) return SPX_CALL_INTERNAL_FAILURE; spx_payload_bytes = UINT32_C(8); }}")
                .expect("writing cannot fail");
        }
    }
    output.push_str(
        "    if (spx_execution.outcome == SPX_OUTCOME_FAILURE) {\n\
             if (spx_execution.selected_failure_ordinal == UINT32_C(0) || spx_execution.selected_failure_ordinal > SPX_PROVIDER_DICTIONARY_ENTRIES) return SPX_CALL_INTERNAL_FAILURE;\n\
             bool spx_selected_seen = false;\n\
             for (uint32_t i = 0; i < spx_execution.event_count; ++i) {\n\
                 if (spx_execution.event_ordinals[i] == spx_execution.selected_failure_ordinal) spx_selected_seen = true;\n\
             }\n\
             if (!spx_selected_seen) return SPX_CALL_INTERNAL_FAILURE;\n\
             spx_payload_bytes = UINT32_C(4);\n\
         } else if (spx_execution.outcome != SPX_OUTCOME_SUCCESS) return SPX_CALL_INTERNAL_FAILURE;\n",
    );
    output.push_str("    uint32_t spx_total = UINT32_C(68) + spx_payload_bytes + UINT32_C(4) * spx_execution.event_count;\n    if (spx_total > response_capacity) return SPX_CALL_INTERNAL_FAILURE;\n");
    output.push_str("    memcpy(response, \"SPXNRSP1\", 8); spx_store_u32(response + 8, UINT32_C(1)); spx_store_u32(response + 12, UINT32_C(20)); spx_store_u32(response + 16, spx_total); memcpy(response + 20, spx_call_contract, 32); memcpy(response + 52, request + 52, 8); spx_store_u32(response + 60, spx_execution.outcome); spx_store_u32(response + 64, spx_execution.event_count);\n");
    output.push_str("    uint32_t spx_write = UINT32_C(68);\n");
    match spec.result {
        ProviderResult::ScalarI64 { .. } => {
            output.push_str("    if (spx_execution.outcome == SPX_OUTCOME_SUCCESS) { spx_store_u32(response + spx_write, UINT32_C(1)); spx_store_i64(response + spx_write + 4, spx_execution.scalar_result); spx_write += UINT32_C(12); } else { spx_store_u32(response + spx_write, spx_execution.selected_failure_ordinal); spx_write += UINT32_C(4); }\n");
        }
        ProviderResult::OwnedInput { .. } => {
            output.push_str("    if (spx_execution.outcome == SPX_OUTCOME_SUCCESS) { spx_store_u32(response + spx_write, UINT32_C(2)); spx_store_u32(response + spx_write + 4, spx_execution.owned_result_ordinal); spx_write += UINT32_C(8); } else { spx_store_u32(response + spx_write, spx_execution.selected_failure_ordinal); spx_write += UINT32_C(4); }\n");
        }
    }
    output.push_str("    for (uint32_t i = 0; i < spx_execution.event_count; ++i) { spx_store_u32(response + spx_write, spx_execution.event_ordinals[i]); spx_write += UINT32_C(4); }\n    if (spx_write != spx_total) return SPX_CALL_INTERNAL_FAILURE;\n    return SPX_CALL_COMPLETE;\n}\n#if defined(__cplusplus)\n}\n#endif\n");
    Ok(())
}

fn request_capacity(parameters: &[ProviderParameter]) -> Result<u32, Diagnostic> {
    let mut total = REQUEST_FIXED_BYTES;
    for parameter in parameters {
        total = total
            .checked_add(match parameter {
                ProviderParameter::I64 => REQUEST_I64_BYTES,
                ProviderParameter::Bool => REQUEST_BOOL_BYTES,
                ProviderParameter::Owned { .. } => REQUEST_OWNER_BYTES,
            })
            .ok_or_else(|| provider_error("request capacity overflows u32"))?;
    }
    require_capacity(total, "request")
}

fn response_capacity(result: ProviderResult, max_events: u32) -> Result<u32, Diagnostic> {
    let success = match result {
        ProviderResult::ScalarI64 { .. } => RESPONSE_SCALAR_BYTES,
        ProviderResult::OwnedInput { .. } => RESPONSE_OWNER_BYTES,
    };
    let events = max_events
        .checked_mul(EVENT_BYTES)
        .ok_or_else(|| provider_error("response event capacity overflows u32"))?;
    let total = RESPONSE_FIXED_BYTES
        .checked_add(success.max(RESPONSE_FAILURE_BYTES))
        .and_then(|value| value.checked_add(events))
        .ok_or_else(|| provider_error("response capacity overflows u32"))?;
    require_capacity(total, "response")
}

fn require_capacity(value: u32, context: &str) -> Result<u32, Diagnostic> {
    if value == 0 || value > MAX_CALL_WIRE_BYTES {
        return Err(provider_error(format!(
            "{context} capacity is outside callable v2"
        )));
    }
    Ok(value)
}

fn require_dictionary_ordinal(ordinal: u32, entries: u32) -> Result<(), Diagnostic> {
    if ordinal == 0 || ordinal > entries {
        return Err(provider_error("semantic ordinal is outside the dictionary"));
    }
    Ok(())
}

fn is_c_symbol(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn codec_profile_fingerprint() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CODEC_PROFILE_DOMAIN);
    for value in [
        WIRE_VERSION,
        HEADER_SIZE,
        REQUEST_FIXED_BYTES,
        RESPONSE_FIXED_BYTES,
        REQUEST_I64_BYTES,
        REQUEST_BOOL_BYTES,
        REQUEST_OWNER_BYTES,
        RESPONSE_SCALAR_BYTES,
        RESPONSE_OWNER_BYTES,
        RESPONSE_FAILURE_BYTES,
        EVENT_BYTES,
        MAX_PROVIDER_STACK_EVENTS,
        PARAMETER_SCALAR,
        PARAMETER_OWNED,
        RESULT_SCALAR_I64,
        RESULT_OWNED_INPUT,
        CALL_RESULT_COMPLETE,
        CALL_RESULT_INVALID_REQUEST,
        CALL_RESULT_RESPONSE_CAPACITY,
        CALL_RESULT_INTERNAL_FAILURE,
        RESPONSE_OUTCOME_SUCCESS,
        RESPONSE_OUTCOME_FAILURE,
        HOOK_RESULT_COMPLETE,
    ] {
        hasher.update(value.to_le_bytes());
    }
    for bytes in [
        REQUEST_MAGIC.as_slice(),
        RESPONSE_MAGIC.as_slice(),
        b"compile-time-physical-target-guard-profile-v1;arch;os;env;object;pointer;endian;msvc-without-gnu-byte-order-builtins",
        b"direct-generated-hook;invocation-first;no-allocation;no-function-pointer;response-unchanged-before-complete",
    ] {
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    hasher.finalize().into()
}

fn provider_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("native callable provider: {}", message.into()),
    )
}

#[cfg(test)]
#[path = "native_callable_provider/tests.rs"]
mod tests;
