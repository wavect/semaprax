use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
const CONTRACT: [u8; 32] = [0x5a; 32];

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.0) else {
            return;
        };
        if metadata.file_type().is_symlink() || metadata.is_file() {
            let _ = fs::remove_file(&self.0);
        } else if metadata.is_dir() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn fixture_directory() -> FixtureDirectory {
    let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-native-provider-{}-{ordinal}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    FixtureDirectory(path)
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn request(arguments: &[(u32, u32, Vec<u8>)], invocation: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(REQUEST_MAGIC);
    push_u32(&mut bytes, WIRE_VERSION);
    push_u32(&mut bytes, HEADER_SIZE);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&CONTRACT);
    push_u64(&mut bytes, invocation);
    push_u32(&mut bytes, arguments.len() as u32);
    for (tag, index, payload) in arguments {
        push_u32(&mut bytes, *tag);
        push_u32(&mut bytes, *index);
        bytes.extend_from_slice(payload);
    }
    let length = bytes.len() as u32;
    bytes[16..20].copy_from_slice(&length.to_le_bytes());
    bytes
}

fn c_bytes(name: &str, bytes: &[u8]) -> String {
    let mut output = format!("static const uint8_t {name}[] = {{");
    for byte in bytes {
        write!(output, "0x{byte:02x},").unwrap();
    }
    output.push_str("};\n");
    output
}

fn compile_and_run(source: &str) {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let directory = fixture_directory();
    let c_path = directory.path().join("provider.c");
    let executable = directory.path().join("provider");
    fs::write(&c_path, source).unwrap();
    let compilation = Command::new("clang")
        .args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "C compilation failed:\n{}\n{}",
        String::from_utf8_lossy(&compilation.stdout),
        String::from_utf8_lossy(&compilation.stderr)
    );
    let execution = Command::new(&executable).output().unwrap();
    assert!(
        execution.status.success(),
        "provider failed with {:?}:\n{}\n{}",
        execution.status.code(),
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
}

#[test]
fn provider_target_guard_source_is_exact_for_msvc_and_gnu_models() {
    let msvc = render_provider_target_guards(ProviderTargetGuardSpec {
        includes: "",
        architecture: "defined(__x86_64__) || defined(_M_X64) || defined(_M_AMD64)",
        operating_system: "defined(_WIN32)",
        environment: "defined(_MSC_VER) && !defined(__MINGW32__) && !defined(__MINGW64__)",
        object_format: "defined(_WIN32) && !defined(__ELF__) && !defined(__MACH__)",
        pointer_width: "UINTPTR_MAX == UINT64_MAX",
        endian: ProviderEndianGuard::Little,
    });
    assert_eq!(
        msvc,
        "#if !(defined(__x86_64__) || defined(_M_X64) || defined(_M_AMD64))\n\
         # error \"SEMAPRAX callable provider architecture mismatch\"\n\
         #endif\n\
         #if !(defined(_WIN32))\n\
         # error \"SEMAPRAX callable provider operating-system mismatch\"\n\
         #endif\n\
         #if !(defined(_MSC_VER) && !defined(__MINGW32__) && !defined(__MINGW64__))\n\
         # error \"SEMAPRAX callable provider environment mismatch\"\n\
         #endif\n\
         #if !(defined(_WIN32) && !defined(__ELF__) && !defined(__MACH__))\n\
         # error \"SEMAPRAX callable provider object-format mismatch\"\n\
         #endif\n\
         #if !(UINTPTR_MAX == UINT64_MAX)\n\
         # error \"SEMAPRAX callable provider pointer-width mismatch\"\n\
         #endif\n\
         #if defined(_MSC_VER)\n\
         /* Supported MSVC architectures are intrinsically little-endian;\n\
            GNU byte-order builtins are neither required nor assumed. */\n\
         #elif defined(__BYTE_ORDER__) && defined(__ORDER_LITTLE_ENDIAN__)\n\
         # if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__\n\
         #  error \"SEMAPRAX callable provider endian mismatch\"\n\
         # endif\n\
         #else\n\
         # error \"SEMAPRAX callable provider cannot prove little endian\"\n\
         #endif\n"
    );

    let gnu = render_provider_target_guards(ProviderTargetGuardSpec {
        includes: "#include <features.h>\n",
        architecture: "defined(__x86_64__) || defined(_M_X64) || defined(_M_AMD64)",
        operating_system: "defined(__linux__) && !defined(__ANDROID__)",
        environment: "defined(__GLIBC__)",
        object_format: "defined(__ELF__) && !defined(__MACH__)",
        pointer_width: "UINTPTR_MAX == UINT64_MAX",
        endian: ProviderEndianGuard::Little,
    });
    assert!(gnu.starts_with("#include <features.h>\n#if !(defined(__x86_64__)"));
    assert!(gnu.contains("#if !(defined(__GLIBC__))"));
    assert!(gnu.contains("#if !(defined(__ELF__) && !defined(__MACH__))"));
    assert!(gnu.contains("defined(__BYTE_ORDER__)"));
    assert!(!gnu.contains("TARGET_OS_"));
}

#[test]
fn ios_static_target_guards_are_closed_distinct_and_stable() {
    let targets = [
        IosProviderPhysicalTarget::DeviceArm64,
        IosProviderPhysicalTarget::SimulatorArm64,
        IosProviderPhysicalTarget::SimulatorX86_64,
        IosProviderPhysicalTarget::MacCatalystArm64,
        IosProviderPhysicalTarget::MacCatalystX86_64,
    ];
    let guards = targets.map(ios_provider_target_guards);
    for (index, guard) in guards.iter().enumerate() {
        assert!(guard.starts_with("#include <TargetConditionals.h>\n"));
        assert!(guard.contains("defined(TARGET_OS_IOS) && TARGET_OS_IOS"));
        assert!(guard.contains("defined(TARGET_OS_SIMULATOR)"));
        assert!(guard.contains("defined(TARGET_OS_MACCATALYST)"));
        assert!(guard.contains("UINTPTR_MAX == UINT64_MAX"));
        assert!(guards[index + 1..].iter().all(|other| other != guard));
    }
    assert!(guards[0].contains("!TARGET_OS_SIMULATOR"));
    assert!(guards[1].contains("TARGET_OS_SIMULATOR"));
    assert!(guards[1].contains("!TARGET_OS_MACCATALYST"));
    assert!(guards[3].contains("TARGET_OS_MACCATALYST"));
    assert!(guards[3].contains("!TARGET_OS_SIMULATOR"));

    let mut hasher = Sha256::new();
    for (target, guard) in targets.into_iter().zip(guards) {
        hasher.update((target.canonical_tag().len() as u64).to_be_bytes());
        hasher.update(target.canonical_tag().as_bytes());
        hasher.update((guard.len() as u64).to_be_bytes());
        hasher.update(guard.as_bytes());
    }
    assert_eq!(
        format!("{:x}", crate::digest_hex::LowerHex(hasher.finalize())),
        "84eb82f6f26f3026fe94ee6c712a4e7add346ea8041b177a7dbd4adebe96d9b4"
    );
}

#[test]
fn android_dynamic_target_guards_are_closed_distinct_and_stable() {
    let targets = [
        AndroidProviderPhysicalTarget::Arm64,
        AndroidProviderPhysicalTarget::EmulatorX86_64,
    ];
    let guards = targets.map(android_provider_target_guards);
    for (index, guard) in guards.iter().enumerate() {
        assert!(guard.contains("defined(__linux__)"));
        assert!(guard.contains("defined(__ANDROID__)"));
        assert!(guard.contains("defined(__BIONIC__)"));
        assert!(guard.contains("!defined(__GLIBC__)"));
        assert!(guard.contains("defined(__ANDROID_API__)"));
        assert!(guard.contains("(__ANDROID_API__ >= 21)"));
        assert!(guard.contains("defined(__ELF__)"));
        assert!(guard.contains("!defined(__APPLE__)"));
        assert!(guard.contains("!defined(_WIN32)"));
        assert!(guard.contains("UINTPTR_MAX == UINT64_MAX"));
        assert!(guards[index + 1..].iter().all(|other| other != guard));
    }
    assert!(guards[0].contains("defined(__aarch64__) || defined(__arm64__)"));
    assert!(guards[0].contains("!defined(__x86_64__)"));
    assert!(guards[1].contains("defined(__x86_64__)"));
    assert!(guards[1].contains("!defined(__aarch64__)"));

    let mut hasher = Sha256::new();
    for (target, guard) in targets.into_iter().zip(guards) {
        hasher.update((target.canonical_tag().len() as u64).to_be_bytes());
        hasher.update(target.canonical_tag().as_bytes());
        hasher.update((guard.len() as u64).to_be_bytes());
        hasher.update(guard.as_bytes());
    }
    assert_eq!(
        format!("{:x}", crate::digest_hex::LowerHex(hasher.finalize())),
        "dddb21eec3f3d0fc048cd5c000c2e427a15102db3a8d96634c2651016b86926a"
    );
}

#[test]
fn android_guards_reject_wrong_architecture_platform_api_and_object_format() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    fn preprocess(
        guard: &str,
        architecture: AndroidProviderPhysicalTarget,
        overrides: &[&str],
        directory: &std::path::Path,
    ) -> (bool, String) {
        fs::write(
            directory.join("stdint.h"),
            "#define UINT32_MAX 4294967295U\n\
             #define UINT64_MAX 18446744073709551615ULL\n\
             #define UINTPTR_MAX UINT64_MAX\n\
             #define __ORDER_LITTLE_ENDIAN__ 1234\n\
             #define __ORDER_BIG_ENDIAN__ 4321\n\
             #define __BYTE_ORDER__ __ORDER_LITTLE_ENDIAN__\n",
        )
        .unwrap();
        fs::write(
            directory.join("guard.c"),
            format!("#include <stdint.h>\n{guard}\nint semaprax_guard_probe;\n"),
        )
        .unwrap();
        let mut command = Command::new("clang");
        command.args([
            "-E",
            "-nostdinc",
            "-D__linux__=1",
            "-D__ANDROID__=1",
            "-D__BIONIC__=1",
            "-D__ANDROID_API__=21",
            "-D__ELF__=1",
            "-U__GLIBC__",
            "-U__APPLE__",
            "-U__MACH__",
            "-U_WIN32",
            "-U_WIN64",
            "-U_MSC_VER",
            "-U__MINGW32__",
            "-U__MINGW64__",
            "-U__i386__",
            "-U__arm__",
        ]);
        match architecture {
            AndroidProviderPhysicalTarget::Arm64 => {
                command.args(["-D__aarch64__=1", "-D__arm64__=1", "-U__x86_64__"]);
            }
            AndroidProviderPhysicalTarget::EmulatorX86_64 => {
                command.args(["-D__x86_64__=1", "-U__aarch64__", "-U__arm64__"]);
            }
        }
        command
            .args(overrides)
            .arg("-I")
            .arg(directory)
            .arg(directory.join("guard.c"));
        let output = command.output().unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    let directory = fixture_directory();
    let arm = android_provider_target_guards(AndroidProviderPhysicalTarget::Arm64);
    let x86 = android_provider_target_guards(AndroidProviderPhysicalTarget::EmulatorX86_64);
    let arm_ok = preprocess(
        &arm,
        AndroidProviderPhysicalTarget::Arm64,
        &[],
        directory.path(),
    );
    assert!(arm_ok.0, "{}", arm_ok.1);
    let x86_ok = preprocess(
        &x86,
        AndroidProviderPhysicalTarget::EmulatorX86_64,
        &[],
        directory.path(),
    );
    assert!(x86_ok.0, "{}", x86_ok.1);
    assert!(
        !preprocess(
            &arm,
            AndroidProviderPhysicalTarget::EmulatorX86_64,
            &[],
            directory.path(),
        )
        .0
    );
    assert!(
        !preprocess(
            &x86,
            AndroidProviderPhysicalTarget::Arm64,
            &[],
            directory.path(),
        )
        .0
    );
    for rejected in [
        &["-U__ANDROID__"][..],
        &["-U__BIONIC__"][..],
        &["-D__GLIBC__=1"][..],
        &["-U__ANDROID_API__"][..],
        &["-U__ANDROID_API__", "-D__ANDROID_API__=20"][..],
        &["-U__ELF__"][..],
        &["-D__APPLE__=1"][..],
        &["-D_WIN32=1"][..],
    ] {
        assert!(
            !preprocess(
                &x86,
                AndroidProviderPhysicalTarget::EmulatorX86_64,
                rejected,
                directory.path(),
            )
            .0
        );
    }
}

#[test]
fn ios_simulator_and_catalyst_guards_reject_each_others_target_conditionals() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    fn preprocess(guard: &str, simulator: bool, directory: &std::path::Path) -> (bool, String) {
        fs::write(
            directory.join("TargetConditionals.h"),
            format!(
                "#define TARGET_OS_IOS 1\n#define TARGET_OS_SIMULATOR {}\n#define TARGET_OS_MACCATALYST {}\n",
                u8::from(simulator),
                u8::from(!simulator),
            ),
        )
        .unwrap();
        fs::write(
            directory.join("stdint.h"),
            "#define UINT32_MAX 4294967295U\n\
             #define UINT64_MAX 18446744073709551615ULL\n\
             #define UINTPTR_MAX UINT64_MAX\n\
             #ifndef __ORDER_LITTLE_ENDIAN__\n\
             #define __ORDER_LITTLE_ENDIAN__ 1234\n\
             #endif\n\
             #ifndef __BYTE_ORDER__\n\
             #define __BYTE_ORDER__ __ORDER_LITTLE_ENDIAN__\n\
             #endif\n",
        )
        .unwrap();
        fs::write(
            directory.join("guard.c"),
            format!("#include <stdint.h>\n{guard}\nint semaprax_guard_probe;\n"),
        )
        .unwrap();
        let output = Command::new("clang")
            .args([
                "-E",
                "-nostdinc",
                "-D__x86_64__=1",
                "-D__APPLE__=1",
                "-D__MACH__=1",
                "-U__aarch64__",
                "-U__arm64__",
                "-U__arm__",
                "-U__ELF__",
                "-U_WIN32",
                "-U_WIN64",
                "-U_MSC_VER",
                "-U__MINGW32__",
                "-U__MINGW64__",
                "-I",
            ])
            .arg(directory)
            .arg(directory.join("guard.c"))
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    let directory = fixture_directory();
    let simulator = ios_provider_target_guards(IosProviderPhysicalTarget::SimulatorX86_64);
    let catalyst = ios_provider_target_guards(IosProviderPhysicalTarget::MacCatalystX86_64);
    let simulator_on_simulator = preprocess(&simulator, true, directory.path());
    assert!(simulator_on_simulator.0, "{}", simulator_on_simulator.1);
    assert!(!preprocess(&catalyst, true, directory.path()).0);
    let catalyst_on_catalyst = preprocess(&catalyst, false, directory.path());
    assert!(catalyst_on_catalyst.0, "{}", catalyst_on_catalyst.1);
    assert!(!preprocess(&simulator, false, directory.path()).0);
}

#[test]
fn provider_target_guard_rejects_a_deliberate_source_mismatch() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let spec = NativeCallableProviderSpec::new(
        "spx_target_mismatch".to_owned(),
        CONTRACT,
        Vec::new(),
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        1,
        1,
    )
    .unwrap();
    let provider = emit(&spec).unwrap();
    let mut source = provider.source.replacen("#if !(", "#if 1 || !(", 1);
    writeln!(source, "static uint32_t SPX_PROVIDER_CALL {}(uint64_t invocation, struct spx_provider_execution *out) {{ (void)invocation; out->outcome = SPX_OUTCOME_SUCCESS; out->scalar_result = INT64_C(0); out->event_count = UINT32_C(1); out->event_ordinals[0] = UINT32_C(1); return UINT32_C(0); }}", provider.hook_symbol).unwrap();
    let directory = fixture_directory();
    let c_path = directory.path().join("mismatch.c");
    let object = directory.path().join("mismatch.o");
    fs::write(&c_path, source).unwrap();
    let compilation = Command::new("clang")
        .args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-c",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(!compilation.status.success());
    assert!(String::from_utf8_lossy(&compilation.stderr)
        .contains("SEMAPRAX callable provider architecture mismatch"));
}

#[test]
fn scalar_provider_strictly_decodes_and_preserves_response_on_rejection() {
    let spec = NativeCallableProviderSpec::new(
        "spx_test_scalar_call_v2".to_owned(),
        CONTRACT,
        vec![
            ProviderParameter::I64,
            ProviderParameter::Bool,
            ProviderParameter::Owned { owner_ordinal: 0 },
        ],
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 3,
        },
        4,
        8,
    )
    .unwrap();
    let provider = emit(&spec).unwrap();
    let mut i64_payload = i64::MIN.to_le_bytes().to_vec();
    let mut bool_payload = Vec::new();
    push_u32(&mut bool_payload, 1);
    let mut owned_payload = Vec::new();
    push_u32(&mut owned_payload, 0);
    push_u64(&mut owned_payload, u64::MAX);
    let canonical = request(
        &[
            (PARAMETER_SCALAR, 0, std::mem::take(&mut i64_payload)),
            (PARAMETER_SCALAR, 1, bool_payload),
            (PARAMETER_OWNED, 2, owned_payload),
        ],
        9,
    );
    assert_eq!(canonical.len(), 112);
    let mut source = provider.source;
    source.push_str(&c_bytes("canonical_request", &canonical));
    writeln!(source, "static uint32_t SPX_PROVIDER_CALL {}(uint64_t invocation, int64_t a, bool b, uint64_t c, struct spx_provider_execution *out) {{ if (invocation != UINT64_C(9) || a != INT64_MIN || !b || c != UINT64_MAX) return UINT32_C(9); out->outcome = SPX_OUTCOME_SUCCESS; out->scalar_result = -INT64_C(7); out->event_count = UINT32_C(3); out->event_ordinals[0] = UINT32_C(1); out->event_ordinals[1] = UINT32_C(2); out->event_ordinals[2] = UINT32_C(3); return UINT32_C(0); }}", provider.hook_symbol).unwrap();
    source.push_str(
        "static int unchanged(const uint8_t *p, size_t n) { for (size_t i = 0; i < n; ++i) if (p[i] != UINT8_C(0xa5)) return 0; return 1; }\n\
         int main(void) {\n\
         uint8_t response[SPX_PROVIDER_RESPONSE_BYTES]; uint8_t hostile[sizeof(canonical_request)];\n\
         memset(response, 0xa5, sizeof(response));\n\
         if (spx_test_scalar_call_v2(canonical_request, sizeof(canonical_request), response, sizeof(response)) != SPX_CALL_COMPLETE) return 1;\n\
         if (memcmp(response, \"SPXNRSP1\", 8) != 0 || spx_load_u32(response + 60) != SPX_OUTCOME_SUCCESS || spx_load_u32(response + 64) != 3 || spx_load_u32(response + 68) != 1 || spx_load_i64(response + 72) != -INT64_C(7) || spx_load_u32(response + 80) != 1 || spx_load_u32(response + 84) != 2 || spx_load_u32(response + 88) != 3) return 2;\n\
         for (uint32_t which = 0; which < UINT32_C(12); ++which) { memcpy(hostile, canonical_request, sizeof(hostile));\n\
           if (which == 0) hostile[0] ^= 1; else if (which == 1) hostile[8] ^= 1; else if (which == 2) hostile[12] ^= 1; else if (which == 3) hostile[16] ^= 1; else if (which == 4) hostile[20] ^= 1; else if (which == 5) memset(hostile + 52, 0, 8); else if (which == 6) hostile[60] ^= 1; else if (which == 7) hostile[64] ^= 1; else if (which == 8) hostile[84] = 2; else if (which == 9) hostile[88] = 2; else if (which == 10) hostile[92] ^= 1; else hostile[100] = 1;\n\
           memset(response, 0xa5, sizeof(response)); if (spx_test_scalar_call_v2(hostile, sizeof(hostile), response, sizeof(response)) != SPX_CALL_INVALID_REQUEST || !unchanged(response, sizeof(response))) return 10 + (int)which; }\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_scalar_call_v2(canonical_request, sizeof(canonical_request) - 1, response, sizeof(response)) != SPX_CALL_INVALID_REQUEST || !unchanged(response, sizeof(response))) return 30;\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_scalar_call_v2(canonical_request, sizeof(canonical_request), response, sizeof(response) - 1) != SPX_CALL_RESPONSE_CAPACITY || !unchanged(response, sizeof(response))) return 31;\n\
         return 0; }\n",
    );
    compile_and_run(&source);
}

#[test]
fn owned_provider_encodes_success_failure_and_contains_hook_failure() {
    let spec = NativeCallableProviderSpec::new(
        "spx_test_owned_call_v2".to_owned(),
        CONTRACT,
        vec![
            ProviderParameter::Owned { owner_ordinal: 0 },
            ProviderParameter::Bool,
        ],
        ProviderResult::OwnedInput {
            owner_ordinal: 0,
            result_commit_ordinal: 4,
        },
        4,
        8,
    )
    .unwrap();
    let provider = emit(&spec).unwrap();
    let mut owner = Vec::new();
    push_u32(&mut owner, 0);
    push_u64(&mut owner, 77);
    let mut yes = Vec::new();
    push_u32(&mut yes, 1);
    let mut no = Vec::new();
    push_u32(&mut no, 0);
    let success = request(
        &[
            (PARAMETER_OWNED, 0, owner.clone()),
            (PARAMETER_SCALAR, 1, yes),
        ],
        10,
    );
    let failure = request(
        &[
            (PARAMETER_OWNED, 0, owner.clone()),
            (PARAMETER_SCALAR, 1, no),
        ],
        11,
    );
    owner[4..12].copy_from_slice(&u64::MAX.to_le_bytes());
    let internal = request(
        &[
            (PARAMETER_OWNED, 0, owner),
            (PARAMETER_SCALAR, 1, vec![1, 0, 0, 0]),
        ],
        12,
    );
    let mut bad_commit_owner = Vec::new();
    push_u32(&mut bad_commit_owner, 0);
    push_u64(&mut bad_commit_owner, 78);
    let bad_commit = request(
        &[
            (PARAMETER_OWNED, 0, bad_commit_owner),
            (PARAMETER_SCALAR, 1, vec![1, 0, 0, 0]),
        ],
        13,
    );
    let mut bad_failure_owner = Vec::new();
    push_u32(&mut bad_failure_owner, 0);
    push_u64(&mut bad_failure_owner, 79);
    let bad_failure = request(
        &[
            (PARAMETER_OWNED, 0, bad_failure_owner),
            (PARAMETER_SCALAR, 1, vec![0, 0, 0, 0]),
        ],
        14,
    );
    let mut source = provider.source;
    source.push_str(&c_bytes("success_request", &success));
    source.push_str(&c_bytes("failure_request", &failure));
    source.push_str(&c_bytes("internal_request", &internal));
    source.push_str(&c_bytes("bad_commit_request", &bad_commit));
    source.push_str(&c_bytes("bad_failure_request", &bad_failure));
    writeln!(source, "static uint32_t SPX_PROVIDER_CALL {}(uint64_t invocation, uint64_t value, bool success, struct spx_provider_execution *out) {{ if (invocation < UINT64_C(10) || invocation > UINT64_C(14)) return UINT32_C(8); if (value == UINT64_MAX) return UINT32_C(7); if (value == UINT64_C(78)) {{ out->outcome = SPX_OUTCOME_SUCCESS; out->owned_result_ordinal = UINT32_C(0); out->event_count = UINT32_C(1); out->event_ordinals[0] = UINT32_C(3); return UINT32_C(0); }} if (value == UINT64_C(79)) {{ out->outcome = SPX_OUTCOME_FAILURE; out->selected_failure_ordinal = UINT32_C(2); out->event_count = UINT32_C(1); out->event_ordinals[0] = UINT32_C(3); return UINT32_C(0); }} if (success) {{ out->outcome = SPX_OUTCOME_SUCCESS; out->owned_result_ordinal = UINT32_C(0); out->event_count = UINT32_C(2); out->event_ordinals[0] = UINT32_C(1); out->event_ordinals[1] = UINT32_C(4); }} else {{ out->outcome = SPX_OUTCOME_FAILURE; out->selected_failure_ordinal = UINT32_C(2); out->event_count = UINT32_C(3); out->event_ordinals[0] = UINT32_C(2); out->event_ordinals[1] = UINT32_C(3); out->event_ordinals[2] = UINT32_C(4); }} return UINT32_C(0); }}", provider.hook_symbol).unwrap();
    source.push_str(
        "static int unchanged(const uint8_t *p, size_t n) { for (size_t i = 0; i < n; ++i) if (p[i] != UINT8_C(0xa5)) return 0; return 1; }\n\
         int main(void) { uint8_t response[SPX_PROVIDER_RESPONSE_BYTES];\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_owned_call_v2(success_request, sizeof(success_request), response, sizeof(response)) != 0 || spx_load_u32(response + 60) != 1 || spx_load_u32(response + 68) != 2 || spx_load_u32(response + 72) != 0) return 1;\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_owned_call_v2(failure_request, sizeof(failure_request), response, sizeof(response)) != 0 || spx_load_u32(response + 60) != 2 || spx_load_u32(response + 68) != 2 || spx_load_u32(response + 72) != 2) return 2;\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_owned_call_v2(internal_request, sizeof(internal_request), response, sizeof(response)) != SPX_CALL_INTERNAL_FAILURE || !unchanged(response, sizeof(response))) return 3;\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_owned_call_v2(bad_commit_request, sizeof(bad_commit_request), response, sizeof(response)) != SPX_CALL_INTERNAL_FAILURE || !unchanged(response, sizeof(response))) return 4;\n\
         memset(response, 0xa5, sizeof(response)); if (spx_test_owned_call_v2(bad_failure_request, sizeof(bad_failure_request), response, sizeof(response)) != SPX_CALL_INTERNAL_FAILURE || !unchanged(response, sizeof(response))) return 5;\n\
         return 0; }\n",
    );
    compile_and_run(&source);
}

#[test]
fn provider_plan_bounds_and_codec_profile_are_deterministic() {
    assert!(NativeCallableProviderSpec::new(
        "bad-symbol!".to_owned(),
        CONTRACT,
        Vec::new(),
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        1,
        1,
    )
    .is_err());
    assert!(NativeCallableProviderSpec::new(
        "spx_stack_boundary".to_owned(),
        CONTRACT,
        Vec::new(),
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        MAX_PROVIDER_STACK_EVENTS,
        1,
    )
    .is_ok());
    assert!(NativeCallableProviderSpec::new(
        "spx_stack_overflow".to_owned(),
        CONTRACT,
        Vec::new(),
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        MAX_PROVIDER_STACK_EVENTS + 1,
        1,
    )
    .is_err());
    assert!(NativeCallableProviderSpec::new(
        "spx_test".to_owned(),
        [0; 32],
        Vec::new(),
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        1,
        1,
    )
    .is_err());
    assert!(NativeCallableProviderSpec::new(
        "spx_test".to_owned(),
        CONTRACT,
        vec![ProviderParameter::Owned { owner_ordinal: 1 }],
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        1,
        1,
    )
    .is_err());
    assert_eq!(
        codec_profile_fingerprint(),
        [
            0x82, 0xa0, 0x77, 0xdb, 0xf8, 0x17, 0x62, 0x8a, 0x2c, 0x02, 0x11, 0xab, 0x8e, 0x67,
            0xcf, 0xb1, 0x08, 0x5c, 0x4b, 0x8e, 0x4f, 0x0c, 0xbb, 0x6f, 0xa1, 0x0a, 0xd4, 0xd9,
            0x1f, 0x78, 0x0d, 0x2b,
        ]
    );
    let spec = NativeCallableProviderSpec::new(
        "spx_preprocessor_balance".to_owned(),
        CONTRACT,
        Vec::new(),
        ProviderResult::ScalarI64 {
            result_commit_ordinal: 1,
        },
        1,
        1,
    )
    .unwrap();
    let source = emit(&spec).unwrap().source;
    let target_guards = provider_target_guards().unwrap();
    assert_eq!(source.matches(&target_guards).count(), 1);
    assert!(source.find(&target_guards).unwrap() < source.find("SPX_PROVIDER_API").unwrap());
    assert_eq!(source.matches("#if defined(_WIN32)").count(), 1);
    assert_eq!(
        source.matches("#if ").count(),
        source.matches("#endif").count()
    );
}
