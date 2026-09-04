use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::hir::ResolvedType;
use crate::{hir, parse};
use sha2::{Digest, Sha256};

use super::*;

static NEXT_CALLABLE_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct CallableFixture(PathBuf);

impl CallableFixture {
    fn create() -> Self {
        let ordinal = NEXT_CALLABLE_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-integrated-callable-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for CallableFixture {
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

#[test]
fn generic_record_c_symbols_bind_the_full_concrete_instance() {
    let declaration = DeclarationId::new("test.phantom");
    let i64_instance = ResolvedType::Nominal {
        declaration: declaration.clone(),
        arguments: vec![ResolvedType::I64],
    };
    let bool_instance = ResolvedType::Nominal {
        declaration: declaration.clone(),
        arguments: vec![ResolvedType::Bool],
    };
    let i64_symbol = c_record_symbol(&i64_instance);
    let bool_symbol = c_record_symbol(&bool_instance);
    assert_ne!(i64_symbol, bool_symbol);
    assert!(i64_symbol.starts_with("spx_record_746573742e7068616e746f6d_"));
    assert!(bool_symbol.starts_with("spx_record_746573742e7068616e746f6d_"));
    assert_eq!(
        c_record_symbol(&ResolvedType::Nominal {
            declaration,
            arguments: Vec::new(),
        }),
        "spx_record_746573742e7068616e746f6d"
    );
}

const RESOURCE_SOURCE: &str = r#"module test.native_resource_types;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

const CALLABLE_EXECUTION_SOURCE: &str = r#"module test.native_callable_execution;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.discard")
fn discard(value: own Token) -> i64 { 0 }

@id("token.discard-two")
fn discard_two(first: own Token, second: own Token) -> i64 { 0 }

@id("token.requires")
fn requires_guard(value: own Token, allowed: bool) -> i64
    requires allowed
{
    0
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("token.checked")
fn checked(value: own Token, number: i64) -> i64
    requires number >= 0
{
    number + 1
}

@id("token.choose-second")
fn choose_second(first: own Token, count: i64, second: own Token) -> Token
    requires count >= 0
{
    second
}

@id("token.ensures-false")
fn ensures_false(value: own Token) -> Token
    ensures false
{
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

enum CallableArgument {
    I64(i64),
    Bool(bool),
    Owned { ordinal: u32, payload: u64 },
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn callable_request(
    artifact: &NativeCallableAdmissionArtifact,
    invocation: u64,
    arguments: &[CallableArgument],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"SPXNREQ1");
    push_u32(&mut bytes, 1);
    push_u32(&mut bytes, 20);
    push_u32(&mut bytes, 0);
    bytes.extend_from_slice(&artifact.call_contract());
    push_u64(&mut bytes, invocation);
    push_u32(&mut bytes, arguments.len() as u32);
    for (index, argument) in arguments.iter().enumerate() {
        match argument {
            CallableArgument::I64(value) => {
                push_u32(&mut bytes, 1);
                push_u32(&mut bytes, index as u32);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            CallableArgument::Bool(value) => {
                push_u32(&mut bytes, 1);
                push_u32(&mut bytes, index as u32);
                push_u32(&mut bytes, u32::from(*value));
            }
            CallableArgument::Owned { ordinal, payload } => {
                push_u32(&mut bytes, 2);
                push_u32(&mut bytes, index as u32);
                push_u32(&mut bytes, *ordinal);
                push_u64(&mut bytes, *payload);
            }
        }
    }
    let length = u32::try_from(bytes.len()).unwrap();
    bytes[16..20].copy_from_slice(&length.to_le_bytes());
    assert_eq!(length, artifact.max_request_bytes());
    bytes
}

fn c_byte_array(name: &str, bytes: &[u8]) -> String {
    let mut output = format!("static const uint8_t {name}[] = {{");
    for byte in bytes {
        write!(output, "0x{byte:02x},").unwrap();
    }
    output.push_str("};\n");
    output
}

fn provider_harness(artifact: &NativeCallableAdmissionArtifact, requests: &[Vec<u8>]) -> String {
    let mut source = artifact.provider_source().to_owned();
    source.push_str("#if defined(_WIN32)\n#include <fcntl.h>\n#include <io.h>\n#endif\n");
    for (index, request) in requests.iter().enumerate() {
        source.push_str(&c_byte_array(&format!("spx_request_{index}"), request));
    }
    source.push_str("int main(void) {\n");
    source.push_str(
            "#if defined(_WIN32)\n    if (_setmode(_fileno(stdout), _O_BINARY) == -1) return 89;\n#endif\n",
        );
    writeln!(
        source,
        "    if (memcmp({}(), \"SPXNABI2\", UINT32_C(8)) != 0) return 90;",
        artifact.getter_symbol()
    )
    .unwrap();
    writeln!(
        source,
        "    uint8_t response[UINT32_C({})];",
        artifact.max_response_bytes()
    )
    .unwrap();
    for index in 0..requests.len() {
        source.push_str("    memset(response, 0xa5, sizeof(response));\n");
        writeln!(source, "    if ({}(spx_request_{index}, (uint32_t)sizeof(spx_request_{index}), response, (uint32_t)sizeof(response)) != SPX_CALL_COMPLETE) return {};", artifact.callable_symbol(), 100 + index).unwrap();
        writeln!(source, "    uint32_t declared_{index} = spx_load_u32(response + UINT32_C(16)); if (declared_{index} > (uint32_t)sizeof(response) || fwrite(response, UINT32_C(1), declared_{index}, stdout) != declared_{index}) return {};", 120 + index).unwrap();
    }
    source.push_str("    return 0;\n}\n");
    source
}

fn compile_and_run_provider(
    artifact: &NativeCallableAdmissionArtifact,
    requests: &[Vec<u8>],
    optimization: &str,
    sanitizers: bool,
) -> Vec<u8> {
    let fixture = CallableFixture::create();
    let source_path = fixture.0.join("provider.c");
    let executable = fixture.0.join("provider");
    fs::write(&source_path, provider_harness(artifact, requests)).unwrap();
    let mut command = Command::new("clang");
    command.args([
        "-std=c11",
        "-pedantic-errors",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fvisibility=hidden",
        optimization,
    ]);
    if sanitizers {
        command.args([
            "-fsanitize=address,undefined",
            "-fno-omit-frame-pointer",
            "-fno-sanitize-recover=all",
        ]);
    }
    let compiled = command
        .arg(&source_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "integrated provider compilation ({optimization}, sanitizers={sanitizers}) failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run = Command::new(&executable)
        .env("ASAN_OPTIONS", "detect_leaks=0:halt_on_error=1")
        .env("UBSAN_OPTIONS", "halt_on_error=1")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "integrated provider execution failed with {}:\n{}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    run.stdout
}

fn split_responses(bytes: &[u8]) -> Vec<&[u8]> {
    let mut responses = Vec::new();
    let mut offset = 0_usize;
    while offset < bytes.len() {
        assert!(bytes.len() - offset >= 20);
        let declared =
            u32::from_le_bytes(bytes[offset + 16..offset + 20].try_into().unwrap()) as usize;
        assert!(declared >= 68 && declared <= bytes.len() - offset);
        responses.push(&bytes[offset..offset + declared]);
        offset += declared;
    }
    responses
}

fn response_word(response: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(response[offset..offset + 4].try_into().unwrap())
}

fn resolved_resource_program() -> ResolvedProgram {
    let parsed = parse(
        RESOURCE_SOURCE,
        Path::new("native-resource-type-selection.spx"),
    )
    .unwrap();
    hir::resolve(&parsed).unwrap()
}

#[test]
fn private_ios_static_fixture_facade_binds_descriptor_source_and_symbols() {
    let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
    let function = DeclarationId::new("token.discard-two");
    let targets = [
        PrivateNativeCallableV3IosTarget::DeviceArm64,
        PrivateNativeCallableV3IosTarget::SimulatorArm64,
        PrivateNativeCallableV3IosTarget::SimulatorX86_64,
        PrivateNativeCallableV3IosTarget::MacCatalystArm64,
        PrivateNativeCallableV3IosTarget::MacCatalystX86_64,
    ];
    let mut artifacts = Vec::new();
    for target in targets {
        let artifact = emit_private_native_callable_v3_ios_fixture(
            &corpus.program,
            &function,
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .unwrap();
        let metadata =
            emit_private_native_callable_v3_ios_descriptor(&corpus.program, &function, target)
                .unwrap();
        assert_eq!(artifact.descriptor(), metadata.bytes());
        assert_eq!(artifact.getter_symbol(), metadata.getter_symbol());
        assert_eq!(artifact.execute_symbol(), metadata.execute_symbol());
        assert_eq!(artifact.settle_symbol(), metadata.settle_symbol());
        assert!(artifact.source().contains(artifact.getter_symbol()));
        assert!(artifact.source().contains(artifact.execute_symbol()));
        assert!(artifact.source().contains(artifact.settle_symbol()));
        assert!(artifacts
            .iter()
            .all(|prior: &Vec<u8>| prior.as_slice() != artifact.descriptor()));
        artifacts.push(artifact.descriptor().to_vec());
    }
}

#[test]
fn private_android_dynamic_fixture_facade_binds_descriptor_source_and_symbols() {
    let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
    let function = DeclarationId::new("token.discard-two");
    let targets = [
        PrivateNativeCallableV3AndroidTarget::Arm64,
        PrivateNativeCallableV3AndroidTarget::X86_64,
    ];
    let mut artifacts = Vec::new();
    for target in targets {
        let first = emit_private_native_callable_v3_android_fixture(
            &corpus.program,
            &function,
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .unwrap();
        let second = emit_private_native_callable_v3_android_fixture(
            &corpus.program,
            &function,
            PrivateNativeCallableV3Fixture::ScalarDiscardTwo,
            target,
        )
        .unwrap();
        let metadata =
            emit_private_native_callable_v3_android_descriptor(&corpus.program, &function, target)
                .unwrap();
        assert_eq!(first.descriptor(), metadata.bytes());
        assert_eq!(first.getter_symbol(), metadata.getter_symbol());
        assert_eq!(first.execute_symbol(), metadata.execute_symbol());
        assert_eq!(first.settle_symbol(), metadata.settle_symbol());
        assert_eq!(first.descriptor(), second.descriptor());
        assert_eq!(first.source(), second.source());
        assert!(first.source().contains(first.getter_symbol()));
        assert!(first.source().contains(first.execute_symbol()));
        assert!(first.source().contains(first.settle_symbol()));
        assert!(artifacts
            .iter()
            .all(|prior: &Vec<u8>| prior.as_slice() != first.descriptor()));
        artifacts.push(first.descriptor().to_vec());
    }
}

#[test]
fn android_corpus_failure_fixture_is_deterministic_and_target_bound() {
    let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
    let case = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "requires-false")
        .expect("requires-false corpus case");
    let function = DeclarationId::new(case.function_id);
    let first = emit_private_native_callable_v3_android_corpus_fixture(
        &corpus.program,
        &function,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
        PrivateNativeCallableV3AndroidTarget::X86_64,
    )
    .unwrap();
    let second = emit_private_native_callable_v3_android_corpus_fixture(
        &corpus.program,
        &function,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
        PrivateNativeCallableV3AndroidTarget::X86_64,
    )
    .unwrap();
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.source(), second.source());
    assert!(first.source().contains(first.getter_symbol()));
    assert!(first.source().contains(first.execute_symbol()));
    assert!(first.source().contains(first.settle_symbol()));
    let arm64 = emit_private_native_callable_v3_android_corpus_fixture(
        &corpus.program,
        &function,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
        PrivateNativeCallableV3AndroidTarget::Arm64,
    )
    .unwrap();
    assert_ne!(first.descriptor(), arm64.descriptor());
}

#[test]
fn ios_corpus_failure_fixture_is_deterministic_and_target_bound() {
    let corpus = crate::owned_resource_corpus::build_owned_resource_corpus_v1().unwrap();
    let case = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "requires-false")
        .expect("requires-false corpus case");
    let function = DeclarationId::new(case.function_id);
    let first = emit_private_native_callable_v3_ios_corpus_fixture(
        &corpus.program,
        &function,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
        PrivateNativeCallableV3IosTarget::SimulatorArm64,
    )
    .unwrap();
    let second = emit_private_native_callable_v3_ios_corpus_fixture(
        &corpus.program,
        &function,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
        PrivateNativeCallableV3IosTarget::SimulatorArm64,
    )
    .unwrap();
    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.source(), second.source());
    assert!(first.source().contains(first.getter_symbol()));
    assert!(first.source().contains(first.execute_symbol()));
    assert!(first.source().contains(first.settle_symbol()));
    let device = emit_private_native_callable_v3_ios_corpus_fixture(
        &corpus.program,
        &function,
        &case.arguments,
        case.expected_owned_result_ordinal,
        &case.reference,
        PrivateNativeCallableV3IosTarget::DeviceArm64,
    )
    .unwrap();
    assert_ne!(first.descriptor(), device.descriptor());
}

#[test]
fn scalar_c_output_matches_the_committed_pre_resource_template_baseline() {
    let parsed = parse(
        r#"module test.native_scalar_baseline;

@id("math.increment")
fn increment(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { increment(41) }
"#,
        Path::new("native-scalar-baseline.spx"),
    )
    .unwrap();
    let generated = emit_c(&parsed).unwrap();
    let digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(generated.as_bytes()))
    );
    assert_eq!(
        digest,
        "4d8c464e5bff19262d1c240c3b770ee3d031bfbbed5c3e6a8c3da9b3654f3f2c"
    );
}

#[test]
fn direct_resource_parameters_and_results_use_the_stable_wrapper_type() {
    let program = resolved_resource_program();
    let resource_abi = native_resource::build_resource_abi(&program).unwrap();
    let functions = function_index(&program).unwrap();
    let wrapper = &resource_abi.resources[0].c_type;
    let mut output = String::new();
    emit_native_prelude(&mut output, &resource_abi, &program);
    emit_function_prototypes(&mut output, &program, &functions, &resource_abi).unwrap();

    let identity_symbol = c_function_symbol(&DeclarationId::new("token.identity"));
    let prototype = output
        .lines()
        .find(|line| line.contains(&identity_symbol))
        .unwrap();
    assert!(prototype.contains(&format!(
        "{identity_symbol}(struct spx_context *spx_ctx, {wrapper}, {wrapper} *spx_result_out);"
    )));
    assert!(!prototype.contains("void *"));
    assert!(
        output
            .find("/* semaprax.native-resource-abi.v1 */")
            .unwrap()
            < output.find(&identity_symbol).unwrap()
    );

    let mut second = String::new();
    emit_native_prelude(&mut second, &resource_abi, &program);
    emit_function_prototypes(&mut second, &program, &functions, &resource_abi).unwrap();
    assert_eq!(output, second);
}

#[test]
fn public_resource_emission_runs_preflight_but_remains_b104_gated() {
    let parsed = parse(
        RESOURCE_SOURCE,
        Path::new("native-resource-public-gate.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let resource_abi = native_resource::build_resource_abi(&program).unwrap();
    let functions = function_index(&program).unwrap();
    preflight_resource_lowering(&program, &functions, &resource_abi, &HashMap::new()).unwrap();

    let diagnostic = emit_c(&parsed).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-B104");
    assert_eq!(
        diagnostic.message,
        "native resource lowering requires lifecycle declarations and the verified cleanup ABI"
    );
}

#[test]
fn callable_v2_admission_is_deterministic_and_does_not_open_b104() {
    let program = resolved_resource_program();
    let function = DeclarationId::new("token.identity");
    let first = emit_native_callable_admission(&program, &function).unwrap();
    let second = emit_native_callable_admission(&program, &function).unwrap();

    assert_eq!(first.descriptor(), second.descriptor());
    assert_eq!(first.getter_symbol(), second.getter_symbol());
    assert_eq!(first.callable_symbol(), second.callable_symbol());
    assert_ne!(first.getter_symbol(), first.callable_symbol());
    assert_eq!(&first.descriptor()[..8], b"SPXNABI2");
    assert_eq!(first.max_request_bytes(), 84);
    assert!(first.max_response_bytes() > 68);
    assert!(first.event_dictionary().contains("token.identity"));

    let parsed = parse(
        RESOURCE_SOURCE,
        Path::new("native-callable-v2-public-gate.spx"),
    )
    .unwrap();
    let public = emit_c(&parsed).unwrap_err();
    assert_eq!(public.code, "SPX-B104");
}

#[test]
fn callable_v2_integrated_provider_is_strict_c11() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = resolved_resource_program();
    let artifact =
        emit_native_callable_admission(&program, &DeclarationId::new("token.identity")).unwrap();
    let fixture = CallableFixture::create();
    let source = fixture.0.join("provider.c");
    let object = fixture.0.join("provider.o");
    fs::write(&source, artifact.provider_source()).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-pedantic-errors",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-O2",
            "-fvisibility=hidden",
            "-c",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "integrated provider C11 compilation failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}

#[test]
fn callable_v2_production_provider_is_strict_cc_at_o0_and_o2() {
    #[cfg(windows)]
    let compiler_name = "clang";
    #[cfg(not(windows))]
    let compiler_name = "cc";
    if Command::new(compiler_name)
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let parsed = parse(
        CALLABLE_EXECUTION_SOURCE,
        Path::new("native-callable-strict-cc.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let artifact =
        emit_native_callable_admission(&program, &DeclarationId::new("token.checked")).unwrap();
    let fixture = CallableFixture::create();
    let source = fixture.0.join("provider.c");
    fs::write(&source, artifact.provider_source()).unwrap();

    for optimization in ["-O0", "-O2"] {
        let object = fixture.0.join(format!(
            "provider-{}.o",
            optimization.trim_start_matches('-').to_ascii_lowercase()
        ));
        let compiled = Command::new(compiler_name)
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
                optimization,
                "-fvisibility=hidden",
                "-c",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "production provider strict {compiler_name} compilation failed at {optimization}:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

#[test]
fn callable_v2_wrong_physical_owned_result_fails_without_touching_response() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(
        CALLABLE_EXECUTION_SOURCE,
        Path::new("native-callable-physical-result-integrity.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let artifact =
        emit_native_callable_admission(&program, &DeclarationId::new("token.identity")).unwrap();
    let request = callable_request(
        &artifact,
        201,
        &[CallableArgument::Owned {
            ordinal: 0,
            payload: u64::MAX,
        }],
    );

    // Establish that the unmodified provider emits the authenticated owned
    // result-commit for this exact request.
    let baseline =
        compile_and_run_provider(&artifact, std::slice::from_ref(&request), "-O2", false);
    let baseline = split_responses(&baseline)[0];
    assert_eq!(response_word(baseline, 60), 1);
    assert_eq!(response_word(baseline, 68), 2);
    assert_eq!(response_word(baseline, 72), 0);

    // Model a lowering defect after the verified body has already emitted
    // its valid semantic trace: corrupt only the physical result payload.
    // The generated hook must detect the disagreement before the wrapper
    // writes even one response byte.
    let marker = "    if (spx_trace.length == UINT32_C(0) ||";
    assert_eq!(artifact.provider_source().matches(marker).count(), 1);
    let mut hostile = artifact.provider_source().replacen(
            marker,
            "    spx_result.payload ^= (uintptr_t)UINT32_C(1);\n    if (spx_trace.length == UINT32_C(0) ||",
            1,
        );
    hostile.push_str(&c_byte_array("spx_physical_mismatch_request", &request));
    writeln!(
            hostile,
            "static int spx_response_unchanged(const uint8_t *response, size_t length) {{ for (size_t i = 0; i < length; ++i) if (response[i] != UINT8_C(0xa5)) return 0; return 1; }}\nint main(void) {{ uint8_t response[UINT32_C({})]; memset(response, 0xa5, sizeof(response)); uint32_t physical = {}(spx_physical_mismatch_request, (uint32_t)sizeof(spx_physical_mismatch_request), response, (uint32_t)sizeof(response)); if (physical != SPX_CALL_INTERNAL_FAILURE || !spx_response_unchanged(response, sizeof(response))) return 1; return 0; }}",
            artifact.max_response_bytes(),
            artifact.callable_symbol()
        )
        .unwrap();

    for optimization in ["-O0", "-O2"] {
        let fixture = CallableFixture::create();
        let source = fixture.0.join("provider.c");
        let executable = fixture.0.join("provider");
        fs::write(&source, &hostile).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-pedantic-errors",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-fvisibility=hidden",
                optimization,
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "physical-result mismatch fixture compilation ({optimization}) failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&executable).output().unwrap();
        assert!(
            run.status.success(),
            "physical-result mismatch fixture ({optimization}) failed with {}:\n{}",
            run.status,
            String::from_utf8_lossy(&run.stderr)
        );
    }
}

#[test]
fn callable_v2_integrated_scalar_owned_success_failure_are_o0_o2_exact() {
    let sanitizers_required =
        std::env::var("SEMAPRAX_REQUIRE_NATIVE_SANITIZERS").is_ok_and(|value| value == "1");
    if Command::new("clang").arg("--version").output().is_err() {
        assert!(
            !sanitizers_required,
            "clang is required for sanitizer evidence"
        );
        return;
    }
    let parsed = parse(
        CALLABLE_EXECUTION_SOURCE,
        Path::new("native-callable-execution.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();

    let requires =
        emit_native_callable_admission(&program, &DeclarationId::new("token.requires")).unwrap();
    let requires_again =
        emit_native_callable_admission(&program, &DeclarationId::new("token.requires")).unwrap();
    assert_eq!(requires.provider_source(), requires_again.provider_source());
    assert_eq!(
        requires.semantic_event_dictionary().canonical_json(),
        requires.event_dictionary()
    );
    let requires_requests = vec![
        callable_request(
            &requires,
            101,
            &[
                CallableArgument::Owned {
                    ordinal: 0,
                    payload: u64::MAX,
                },
                CallableArgument::Bool(true),
            ],
        ),
        callable_request(
            &requires,
            102,
            &[
                CallableArgument::Owned {
                    ordinal: 0,
                    payload: 0,
                },
                CallableArgument::Bool(false),
            ],
        ),
    ];
    let requires_o0 = compile_and_run_provider(&requires, &requires_requests, "-O0", false);
    let requires_o2 = compile_and_run_provider(&requires, &requires_requests, "-O2", false);
    assert_eq!(requires_o0, requires_o2);
    let responses = split_responses(&requires_o2);
    assert_eq!(responses.len(), 2);
    assert_eq!(&responses[0][..8], b"SPXNRSP1");
    assert_eq!(response_word(responses[0], 60), 1);
    assert_eq!(response_word(responses[1], 60), 2);
    assert_eq!(
        u64::from_le_bytes(responses[0][52..60].try_into().unwrap()),
        101
    );
    assert_eq!(
        u64::from_le_bytes(responses[1][52..60].try_into().unwrap()),
        102
    );
    let selected = response_word(responses[1], 68);
    let failure_events = (0..response_word(responses[1], 64) as usize)
        .map(|index| response_word(responses[1], 72 + index * 4))
        .collect::<Vec<_>>();
    assert!(failure_events.contains(&selected));

    let checked =
        emit_native_callable_admission(&program, &DeclarationId::new("token.checked")).unwrap();
    let checked_requests = vec![
        callable_request(
            &checked,
            105,
            &[
                CallableArgument::Owned {
                    ordinal: 0,
                    payload: 0,
                },
                CallableArgument::I64(41),
            ],
        ),
        callable_request(
            &checked,
            106,
            &[
                CallableArgument::Owned {
                    ordinal: 0,
                    payload: u64::MAX,
                },
                CallableArgument::I64(i64::MAX),
            ],
        ),
    ];
    let checked_o0 = compile_and_run_provider(&checked, &checked_requests, "-O0", false);
    let checked_o2 = compile_and_run_provider(&checked, &checked_requests, "-O2", false);
    assert_eq!(checked_o0, checked_o2);
    let checked_responses = split_responses(&checked_o2);
    assert_eq!(response_word(checked_responses[0], 60), 1);
    assert_eq!(
        i64::from_le_bytes(checked_responses[0][72..80].try_into().unwrap()),
        42
    );
    assert_eq!(response_word(checked_responses[1], 60), 2);

    let identity =
        emit_native_callable_admission(&program, &DeclarationId::new("token.identity")).unwrap();
    let identity_requests = vec![callable_request(
        &identity,
        103,
        &[CallableArgument::Owned {
            ordinal: 0,
            payload: u64::MAX,
        }],
    )];
    let identity_o0 = compile_and_run_provider(&identity, &identity_requests, "-O0", false);
    let identity_o2 = compile_and_run_provider(&identity, &identity_requests, "-O2", false);
    assert_eq!(identity_o0, identity_o2);
    let response = split_responses(&identity_o2)[0];
    assert_eq!(response_word(response, 60), 1);
    assert_eq!(response_word(response, 68), 2);
    assert_eq!(response_word(response, 72), 0);

    let ensures =
        emit_native_callable_admission(&program, &DeclarationId::new("token.ensures-false"))
            .unwrap();
    let ensures_requests = vec![callable_request(
        &ensures,
        104,
        &[CallableArgument::Owned {
            ordinal: 0,
            payload: u64::MAX,
        }],
    )];
    let ensures_o0 = compile_and_run_provider(&ensures, &ensures_requests, "-O0", false);
    let ensures_o2 = compile_and_run_provider(&ensures, &ensures_requests, "-O2", false);
    assert_eq!(ensures_o0, ensures_o2);
    assert_eq!(response_word(split_responses(&ensures_o2)[0], 60), 2);

    if sanitizers_required {
        assert_eq!(
            compile_and_run_provider(&requires, &requires_requests, "-O1", true),
            requires_o2
        );
        assert_eq!(
            compile_and_run_provider(&identity, &identity_requests, "-O1", true),
            identity_o2
        );
        assert_eq!(
            compile_and_run_provider(&checked, &checked_requests, "-O1", true),
            checked_o2
        );
        assert_eq!(
            compile_and_run_provider(&ensures, &ensures_requests, "-O1", true),
            ensures_o2
        );
    }

    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                requires.normalized_execution_projection().as_bytes()
            ))
        ),
        "e5802548830ebc278bfd727a91fecebd763c81d5729374d85a4bded1e0dbf83c"
    );
}

#[test]
fn callable_v2_shared_library_exports_only_getter_and_callable() {
    if Command::new("clang").arg("--version").output().is_err()
        || Command::new("nm").arg("--version").output().is_err()
    {
        return;
    }
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return;
    }
    let parsed = parse(
        CALLABLE_EXECUTION_SOURCE,
        Path::new("native-callable-exports.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let artifact =
        emit_native_callable_admission(&program, &DeclarationId::new("token.identity")).unwrap();
    let fixture = CallableFixture::create();
    let source = fixture.0.join("provider.c");
    let library = fixture.0.join(if cfg!(target_os = "macos") {
        "provider.dylib"
    } else {
        "provider.so"
    });
    fs::write(&source, artifact.provider_source()).unwrap();
    let mut compile = Command::new("clang");
    compile.args([
        "-std=c11",
        "-pedantic-errors",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-O2",
        "-fvisibility=hidden",
    ]);
    if cfg!(target_os = "macos") {
        compile.arg("-dynamiclib");
    } else {
        compile.args(["-shared", "-fPIC"]);
    }
    let compiled = compile
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "callable shared-library compilation failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let symbols = if cfg!(target_os = "macos") {
        Command::new("nm").args(["-gU"]).arg(&library).output()
    } else {
        Command::new("nm")
            .args(["-D", "--defined-only"])
            .arg(&library)
            .output()
    }
    .unwrap();
    assert!(symbols.status.success());
    let mut actual = String::from_utf8(symbols.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .map(|symbol| {
            if cfg!(target_os = "macos") {
                symbol.strip_prefix('_').unwrap_or(symbol).to_owned()
            } else {
                symbol.to_owned()
            }
        })
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = vec![
        artifact.callable_symbol().to_owned(),
        artifact.getter_symbol().to_owned(),
    ];
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn callable_v2_authoritative_fourteen_case_corpus_has_production_providers() {
    let parsed = parse(
        CALLABLE_EXECUTION_SOURCE,
        Path::new("native-callable-authoritative-corpus.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let scenario_functions = [
        ("discard-zero", "token.discard"),
        ("discard-max", "token.discard"),
        ("discard-two-reverse", "token.discard-two"),
        ("requires-false", "token.requires"),
        ("requires-true", "token.requires"),
        ("checked-success", "token.checked"),
        ("checked-add-overflow", "token.checked"),
        ("checked-precondition-false", "token.checked"),
        ("identity-zero", "token.identity"),
        ("identity-max", "token.identity"),
        ("choose-second-zero-max", "token.choose-second"),
        ("choose-second-zero-zero", "token.choose-second"),
        ("choose-second-requires-false", "token.choose-second"),
        ("ensures-false", "token.ensures-false"),
    ];
    assert_eq!(
        scenario_functions.map(|(scenario, _)| scenario),
        crate::semantic_trace::OWNED_RESOURCE_CORPUS_V1_SCENARIOS
    );
    for (_, function) in scenario_functions {
        let artifact =
            emit_native_callable_admission(&program, &DeclarationId::new(function)).unwrap();
        assert!(artifact
            .provider_source()
            .contains(artifact.callable_symbol()));
        assert!(artifact
            .provider_source()
            .contains(artifact.getter_symbol()));
        assert_eq!(
            artifact.semantic_event_dictionary().canonical_json(),
            artifact.event_dictionary()
        );
    }
}

#[test]
fn callable_v2_admission_changes_with_checked_semantics() {
    let baseline = resolved_resource_program();
    let changed_source = RESOURCE_SOURCE.replace(
        "fn identity(value: own Token) -> Token { value }",
        "fn identity(value: own Token) -> Token ensures false { value }",
    );
    let changed = parse(
        &changed_source,
        Path::new("native-callable-v2-semantic-delta.spx"),
    )
    .unwrap();
    let changed = hir::resolve(&changed).unwrap();
    let function = DeclarationId::new("token.identity");
    let baseline = emit_native_callable_admission(&baseline, &function).unwrap();
    let changed = emit_native_callable_admission(&changed, &function).unwrap();

    assert_ne!(baseline.descriptor(), changed.descriptor());
    assert_ne!(baseline.getter_symbol(), changed.getter_symbol());
    assert_ne!(baseline.callable_symbol(), changed.callable_symbol());
    assert_ne!(baseline.event_dictionary(), changed.event_dictionary());
}

#[test]
fn resource_value_preflight_rejects_unstaged_borrow_without_changing_public_gate() {
    let source = RESOURCE_SOURCE.replace(
        "@id(\"token.identity\")\nfn identity(value: own Token) -> Token { value }",
        "@id(\"token.observe\")\nfn observe(value: borrow Token) -> i64 { 0 }",
    );
    let parsed = parse(
        &source,
        Path::new("native-resource-borrow-value-preflight.spx"),
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let resource_abi = native_resource::build_resource_abi(&program).unwrap();
    let functions = function_index(&program).unwrap();
    let preflight =
        preflight_resource_lowering(&program, &functions, &resource_abi, &HashMap::new())
            .unwrap_err();
    assert_eq!(preflight.code, "SPX-B104");
    assert!(preflight.message.contains("resource parameter"));

    let public = emit_c(&parsed).unwrap_err();
    let gate = resource_lowering_gate();
    assert_eq!(public.code, gate.code);
    assert_eq!(public.message, gate.message);
}

#[test]
fn c_literals_preserve_utf8_bytes_without_exposing_trigraphs() {
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }

    let escaped = c_string("??/λ\n\r\t\\\"\u{7f}");
    assert_eq!(escaped, "\\077\\077/\\316\\273\\n\\r\\t\\\\\\\"\\177");
    assert!(!escaped.contains("??"));
    assert!(escaped.is_ascii());
    assert_eq!(
        c_i64(i64::MIN),
        "(-INT64_C(9223372036854775807) - INT64_C(1))"
    );
    assert_eq!(c_i64(-42), "-INT64_C(42)");
    assert_eq!(c_i64(42), "INT64_C(42)");

    let source = format!(
            "#include <stddef.h>\n\
             #include <stdint.h>\n\
             static const unsigned char value[] = \"{escaped}\";\n\
             static const unsigned char expected[] = {{0x3f, 0x3f, 0x2f, 0xce, 0xbb, 0x0a, 0x0d, 0x09, 0x5c, 0x22, 0x7f, 0x00}};\n\
             static const int64_t minimum = {};\n\
             static const int64_t negative = {};\n\
             int main(void) {{\n\
                 if (sizeof(value) != sizeof(expected)) return 1;\n\
                 for (size_t index = 0; index < sizeof(expected); ++index) {{\n\
                     if (value[index] != expected[index]) return 2;\n\
                 }}\n\
                 if (minimum != INT64_MIN || negative != -INT64_C(42)) return 3;\n\
                 return 0;\n\
             }}\n",
            c_i64(i64::MIN),
            c_i64(-42),
        );
    assert!(!source.contains("??"));

    let suffix = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let c_path = std::env::temp_dir().join(format!(
        "semaprax-c-string-{}-{suffix}.c",
        std::process::id()
    ));
    let binary = std::env::temp_dir().join(format!(
        "semaprax-c-string-{}-{suffix}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::write(&c_path, source).expect("write strict C string fixture");
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wtrigraphs",
            "-Wimplicitly-unsigned-literal",
        ])
        .arg(&c_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("clang was available during the version probe");
    let _ = std::fs::remove_file(&c_path);
    assert!(
        compiled.status.success(),
        "strict C11 compilation failed:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let run = Command::new(&binary)
        .output()
        .expect("run C string fixture");
    let _ = std::fs::remove_file(&binary);
    assert!(
        run.status.success(),
        "C string fixture exited {}",
        run.status
    );
}
