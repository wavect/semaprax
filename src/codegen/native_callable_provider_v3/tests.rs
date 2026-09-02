use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::conformance::{TraceEventKind, TraceOutcome, TraceResult};
use crate::owned_resource_corpus::{
    build_owned_resource_corpus_v1, OwnedResourceCorpusArgument, OwnedResourceCorpusCase,
};
use crate::semantic_trace::build_semantic_event_dictionary;

use super::super::native_callable_abi_v3;
use super::super::native_callable_wire_v3::{
    decision_digest, encode_action_evidence, encode_decision, encode_frame, encode_request,
    extend_action_chain_digest, initial_action_chain_digest, request_digest,
    response_storage_digest, ActionBoundary, ActionEvidence, ExecuteReturn, FramePhase,
    ProviderBinding, RecoveryFrame, RequestArgument, ResourceCell, ResourceState,
    SettlementDecision,
};
use super::*;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn new() -> Self {
        let ordinal = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-native-provider-v3-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binding(
    spec: &NativeCallableProviderV3Spec,
    invocation: u64,
    generation: u64,
    challenge: u8,
) -> ProviderBinding {
    ProviderBinding {
        call_contract: spec.call_contract,
        recovery_contract: spec.recovery_contract,
        settlement_graph: spec.settlement_graph,
        invocation,
        frame_generation: generation,
        provider_challenge: [challenge; 32],
    }
}

fn initial_owned_wires(binding: ProviderBinding, payloads: &[u64]) -> (Vec<u8>, Vec<u8>) {
    owned_wires_at(
        binding,
        payloads,
        &vec![ResourceState::Live; payloads.len()],
        1,
        0,
    )
}

fn preexecute_unwind_owned_wires(
    binding: ProviderBinding,
    payloads: &[u64],
    response_bytes: u32,
) -> (Vec<u8>, Vec<u8>) {
    let (request, _) = initial_owned_wires(binding, payloads);
    let resources = payloads
        .iter()
        .map(|payload| ResourceCell {
            state: ResourceState::Live,
            payload: *payload,
        })
        .collect::<Vec<_>>();
    let frame = encode_frame(&RecoveryFrame {
        binding,
        request_digest: request_digest(&request).unwrap(),
        response_digest: response_storage_digest(
            PRE_EXECUTE_HOST_UNWIND_CODE,
            &vec![0; response_bytes as usize],
        )
        .unwrap(),
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::PreExecuteHostUnwind,
        checkpoint: 1,
        phase: FramePhase::Executing,
        decision_digest: [0; 32],
        next_action_index: 0,
        action_record_count: 0,
        active_finalizers: 0,
        resources: &resources,
        action_chain_digest: [0; 32],
    })
    .unwrap();
    (request, frame)
}

fn owned_wires_at(
    binding: ProviderBinding,
    payloads: &[u64],
    states: &[ResourceState],
    checkpoint: u32,
    active_finalizers: u32,
) -> (Vec<u8>, Vec<u8>) {
    let arguments = payloads
        .iter()
        .enumerate()
        .map(|(index, payload)| RequestArgument::Owned {
            index: index as u32,
            owner_ordinal: index as u32,
            payload: *payload,
        })
        .collect::<Vec<_>>();
    let request = encode_request(binding, &arguments).unwrap();
    let digest = request_digest(&request).unwrap();
    let resources = payloads
        .iter()
        .zip(states)
        .map(|(payload, state)| ResourceCell {
            state: *state,
            payload: *payload,
        })
        .collect::<Vec<_>>();
    let frame = encode_frame(&RecoveryFrame {
        binding,
        request_digest: digest,
        response_digest: [0; 32],
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::Pending,
        checkpoint,
        phase: FramePhase::Executing,
        decision_digest: [0; 32],
        next_action_index: 0,
        action_record_count: 0,
        active_finalizers,
        resources: &resources,
        action_chain_digest: [0; 32],
    })
    .unwrap();
    (request, frame)
}

fn corpus_wires(
    binding: ProviderBinding,
    arguments: &[OwnedResourceCorpusArgument],
) -> (Vec<u8>, Vec<u8>, Vec<u64>) {
    let mut next_owner = 0_u32;
    let mut payloads = Vec::new();
    let request_arguments = arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| match *argument {
            OwnedResourceCorpusArgument::Owned(payload) => {
                let owner_ordinal = next_owner;
                next_owner += 1;
                payloads.push(payload);
                RequestArgument::Owned {
                    index: index as u32,
                    owner_ordinal,
                    payload,
                }
            }
            OwnedResourceCorpusArgument::Bool(value) => RequestArgument::Bool {
                index: index as u32,
                value,
            },
            OwnedResourceCorpusArgument::I64(value) => RequestArgument::I64 {
                index: index as u32,
                value,
            },
        })
        .collect::<Vec<_>>();
    let request = encode_request(binding, &request_arguments).unwrap();
    let digest = request_digest(&request).unwrap();
    let resources = payloads
        .iter()
        .map(|payload| ResourceCell {
            state: ResourceState::Live,
            payload: *payload,
        })
        .collect::<Vec<_>>();
    let frame = encode_frame(&RecoveryFrame {
        binding,
        request_digest: digest,
        response_digest: [0; 32],
        semantic_trace_digest: [0; 32],
        execute_return: ExecuteReturn::Pending,
        checkpoint: 1,
        phase: FramePhase::Executing,
        decision_digest: [0; 32],
        next_action_index: 0,
        action_record_count: 0,
        active_finalizers: 0,
        resources: &resources,
        action_chain_digest: [0; 32],
    })
    .unwrap();
    (request, frame, payloads)
}

fn append_array(output: &mut String, name: &str, bytes: &[u8]) {
    emit_c_array(output, name, bytes);
}

fn semantic_digest(trace: [u8; 32], ordinals: &[u32], outcome: u8) -> [u8; 32] {
    semantic_digest_exact(trace, ordinals, outcome, 0)
}

fn semantic_digest_exact(
    trace: [u8; 32],
    ordinals: &[u32],
    outcome: u8,
    selected_ordinal: u32,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.native-recovery-trace-evidence.v1\0");
    hasher.update(trace);
    hasher.update((ordinals.len() as u64).to_le_bytes());
    for ordinal in ordinals {
        hasher.update(ordinal.to_le_bytes());
    }
    hasher.update([outcome]);
    if outcome == 3 {
        hasher.update(selected_ordinal.to_le_bytes());
    }
    hasher.finalize().into()
}

fn append_u32_array(output: &mut String, name: &str, values: &[u32]) {
    let physical = if values.is_empty() { &[0][..] } else { values };
    writeln!(
        output,
        "static const uint32_t {name}[{}]={{{}}};",
        physical.len(),
        physical
            .iter()
            .map(|value| format!("UINT32_C({value})"))
            .collect::<Vec<_>>()
            .join(",")
    )
    .unwrap();
}

fn append_u64_array(output: &mut String, name: &str, values: &[u64]) {
    let physical = if values.is_empty() { &[0][..] } else { values };
    writeln!(
        output,
        "static const uint64_t {name}[{}]={{{}}};",
        physical.len(),
        physical
            .iter()
            .map(|value| format!("UINT64_C({value})"))
            .collect::<Vec<_>>()
            .join(",")
    )
    .unwrap();
}

fn compile_and_run(source: &str, optimization: &str) {
    compile_and_run_labeled(source, optimization, "provider");
}

fn compile_and_run_labeled(source: &str, optimization: &str, label: &str) {
    let directory = FixtureDirectory::new();
    let c_path = directory.path().join("provider.c");
    let executable = directory.path().join("provider");
    fs::write(&c_path, source).unwrap();
    let sanitizers_required = match std::env::var("SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS") {
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) if value == "1" => true,
        Ok(value) => {
            panic!("SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS must be exactly `1`, got `{value}`")
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS must be Unicode `1`")
        }
    };
    let clang = if sanitizers_required {
        std::env::var_os("CLANG")
            .expect("SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS=1 requires an explicit CLANG path")
    } else {
        std::env::var_os("CLANG").unwrap_or_else(|| "clang".into())
    };
    if sanitizers_required {
        let version = Command::new(&clang).arg("--version").output().unwrap();
        assert!(
            version.status.success()
                && String::from_utf8_lossy(&version.stdout)
                    .to_ascii_lowercase()
                    .contains("clang"),
            "callable-v3 sanitizer gate requires Clang"
        );
    }
    let mut compiler = Command::new(clang);
    compiler.args([
        "-std=c11",
        optimization,
        "-Wall",
        "-Wextra",
        "-Werror",
        "-pedantic",
    ]);
    if sanitizers_required {
        compiler.args([
            "-fsanitize=address,undefined",
            "-fno-omit-frame-pointer",
            "-fno-sanitize-recover=all",
        ]);
    }
    let compile = compiler
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "provider `{label}` compile failed:\n{}\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );
    let mut runner = Command::new(&executable);
    if sanitizers_required {
        let asan = strict_sanitizer_options(
            "ASAN_OPTIONS",
            "detect_leaks=0:halt_on_error=1:abort_on_error=1",
        );
        let ubsan = strict_sanitizer_options("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        runner.env("ASAN_OPTIONS", asan);
        runner.env("UBSAN_OPTIONS", ubsan);
    }
    let run = runner.output().unwrap();
    assert!(
        run.status.success(),
        "provider `{label}` failed {:?}:\n{}\n{}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

fn strict_sanitizer_options(name: &str, default: &str) -> String {
    match std::env::var(name) {
        Err(std::env::VarError::NotPresent) => default.to_owned(),
        Ok(value)
            if value.split(':').any(|field| field == "halt_on_error=1")
                && (name != "ASAN_OPTIONS"
                    || value.split(':').any(|field| field == "abort_on_error=1")) =>
        {
            value
        }
        Ok(value) => panic!(
            "{name} must preserve strict halt{} semantics, got `{value}`",
            if name == "ASAN_OPTIONS" { "/abort" } else { "" }
        ),
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("{name} must be valid Unicode strict sanitizer options")
        }
    }
}

fn spec(function_id: &str, plan: ProviderV3Plan) -> NativeCallableProviderV3Spec {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = corpus
        .program
        .functions
        .iter()
        .find(|function| function.id.as_str() == function_id)
        .unwrap();
    let descriptor = native_callable_abi_v3::derive(&corpus.program, &function.id).unwrap();
    NativeCallableProviderV3Spec::new(descriptor, plan).unwrap()
}

#[test]
fn ios_static_descriptors_and_provider_guards_are_exactly_paired() {
    use super::super::native_callable_provider::IosProviderPhysicalTarget;

    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = DeclarationId::new("token.discard-two");
    let pairs = [
        IosProviderPhysicalTarget::DeviceArm64,
        IosProviderPhysicalTarget::SimulatorArm64,
        IosProviderPhysicalTarget::SimulatorX86_64,
        IosProviderPhysicalTarget::MacCatalystArm64,
        IosProviderPhysicalTarget::MacCatalystX86_64,
    ];
    let plan = ProviderV3Plan::ScalarDiscard {
        scalar_result: 0,
        finalizer_order: vec![1, 0],
        completed_checkpoints: vec![2, 3],
        semantic_ordinals: vec![1, 2, 3, 4, 5],
    };
    let mut emitted = Vec::new();
    for (index, target) in pairs.into_iter().enumerate() {
        let descriptor = native_callable_abi_v3::derive_ios_static_for_target(
            &corpus.program,
            &function,
            target.canonical_tag(),
        )
        .unwrap();
        let wrong_target = pairs[(index + 1) % pairs.len()];
        let mismatch = NativeCallableProviderV3Spec::new_ios_static(
            descriptor.clone(),
            plan.clone(),
            wrong_target,
        )
        .unwrap_err();
        assert_eq!(mismatch.code, "SPX-I107");
        assert!(mismatch
            .message
            .contains("target guard does not match descriptor"));

        let spec =
            NativeCallableProviderV3Spec::new_ios_static(descriptor.clone(), plan.clone(), target)
                .unwrap();
        let source = emit(&spec).unwrap().source;
        assert!(source
            .contains(&super::super::native_callable_provider::ios_provider_target_guards(target)));
        assert!(source.contains(&descriptor.getter_symbol));
        assert!(source.contains(&descriptor.execute_symbol));
        assert!(source.contains(&descriptor.settle_symbol));
        assert!(emitted
            .iter()
            .all(|(bytes, prior_source): &(Vec<u8>, String)| {
                bytes != &descriptor.bytes && prior_source != &source
            }));
        emitted.push((descriptor.bytes, source));
    }
}

#[test]
fn android_dynamic_descriptors_and_provider_guards_are_exactly_paired() {
    use super::super::native_callable_provider::AndroidProviderPhysicalTarget;

    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = DeclarationId::new("token.discard-two");
    let pairs = [
        AndroidProviderPhysicalTarget::Arm64,
        AndroidProviderPhysicalTarget::EmulatorX86_64,
    ];
    let plan = ProviderV3Plan::ScalarDiscard {
        scalar_result: 0,
        finalizer_order: vec![1, 0],
        completed_checkpoints: vec![2, 3],
        semantic_ordinals: vec![1, 2, 3, 4, 5],
    };
    let mut emitted = Vec::new();
    for (index, target) in pairs.into_iter().enumerate() {
        let descriptor = native_callable_abi_v3::derive_dynamic_for_target(
            &corpus.program,
            &function,
            target.canonical_tag(),
        )
        .unwrap();
        let wrong_target = pairs[(index + 1) % pairs.len()];
        let mismatch = NativeCallableProviderV3Spec::new_android_dynamic(
            descriptor.clone(),
            plan.clone(),
            wrong_target,
        )
        .unwrap_err();
        assert_eq!(mismatch.code, "SPX-I107");
        assert!(mismatch
            .message
            .contains("target guard does not match descriptor"));

        let spec = NativeCallableProviderV3Spec::new_android_dynamic(
            descriptor.clone(),
            plan.clone(),
            target,
        )
        .unwrap();
        let source = emit(&spec).unwrap().source;
        assert!(source.contains(
            &super::super::native_callable_provider::android_provider_target_guards(target)
        ));
        assert!(source.contains(&descriptor.getter_symbol));
        assert!(source.contains(&descriptor.execute_symbol));
        assert!(source.contains(&descriptor.settle_symbol));
        assert!(emitted
            .iter()
            .all(|(bytes, prior_source): &(Vec<u8>, String)| {
                bytes != &descriptor.bytes && prior_source != &source
            }));
        emitted.push((descriptor.bytes, source));
    }

    let ios_descriptor = native_callable_abi_v3::derive_ios_static_for_target(
        &corpus.program,
        &function,
        super::super::native_callable_provider::IosProviderPhysicalTarget::DeviceArm64
            .canonical_tag(),
    )
    .unwrap();
    assert!(NativeCallableProviderV3Spec::new_android_dynamic(
        ios_descriptor,
        plan,
        AndroidProviderPhysicalTarget::Arm64,
    )
    .is_err());
}

fn graph_spec(
    program: &ResolvedProgram,
    case: &OwnedResourceCorpusCase,
) -> NativeCallableProviderV3Spec {
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == case.function_id)
        .unwrap();
    let dictionary = build_semantic_event_dictionary(program, &function.id).unwrap();
    let semantic_ordinals = case
        .reference
        .events
        .iter()
        .map(|event| dictionary.ordinal_for(&event.event).unwrap())
        .collect::<Vec<_>>();
    let outcome = match &case.reference.outcome {
        TraceOutcome::Success {
            result: TraceResult::I64(value),
        } => ProviderV3Outcome::Scalar { value: *value },
        TraceOutcome::Success {
            result: TraceResult::Owned { .. },
        } => ProviderV3Outcome::Owned {
            owner_ordinal: case.expected_owned_result_ordinal.unwrap() as u32,
        },
        TraceOutcome::Failure { .. } => {
            let selected_ordinal = case
                .reference
                .events
                .iter()
                .find_map(|event| {
                    matches!(event.event, TraceEventKind::SelectFailure { .. })
                        .then(|| dictionary.ordinal_for(&event.event).unwrap())
                })
                .unwrap();
            ProviderV3Outcome::SemanticFailure { selected_ordinal }
        }
        TraceOutcome::Success { .. } => panic!("corpus result is outside callable v3"),
    };
    let scalar_arguments = case
        .arguments
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| match argument {
            OwnedResourceCorpusArgument::Owned(_) => None,
            OwnedResourceCorpusArgument::Bool(value) => Some(ProviderV3ScalarArgument {
                parameter_index: index as u32,
                value: ProviderV3ScalarValue::Bool(*value),
            }),
            OwnedResourceCorpusArgument::I64(value) => Some(ProviderV3ScalarArgument {
                parameter_index: index as u32,
                value: ProviderV3ScalarValue::I64(*value),
            }),
        })
        .collect();
    let descriptor = native_callable_abi_v3::derive(program, &function.id).unwrap();
    NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::GraphWitness {
            scalar_arguments,
            outcome,
            semantic_ordinals,
        },
    )
    .unwrap()
}

#[test]
fn scalar_two_owner_provider_is_durable_strict_and_idempotent_at_o0_o2() {
    let plan = ProviderV3Plan::ScalarDiscard {
        scalar_result: 0,
        finalizer_order: vec![1, 0],
        completed_checkpoints: vec![2, 3],
        semantic_ordinals: vec![1, 2, 3, 4, 5],
    };
    let spec = spec("token.discard-two", plan);
    let emitted = emit(&spec).unwrap();
    let primary_binding = binding(&spec, 9, 11, 4);
    let (request, frame) = initial_owned_wires(primary_binding, &[41, 73]);
    let accept = encode_decision(primary_binding, SettlementDecision::AcceptScalar).unwrap();
    let conflict = encode_decision(primary_binding, SettlementDecision::AbortMalformed).unwrap();
    let expected_semantic = semantic_digest(spec.trace_path_certificate, &[1, 2, 3, 4, 5], 1);
    let expected_accept_chain =
        initial_action_chain_digest(decision_digest(&accept).unwrap(), 0).unwrap();
    let initial_abort_binding = binding(&spec, 41, 51, 8);
    let mid_abort_binding = binding(&spec, 42, 52, 9);
    let uncertain_binding = binding(&spec, 43, 53, 10);
    let (_, initial_abort_frame) = initial_owned_wires(initial_abort_binding, &[201, 203]);
    let (_, preexecute_abort_frame) =
        preexecute_unwind_owned_wires(initial_abort_binding, &[201, 203], spec.response_bytes);
    let (_, mid_abort_frame) = owned_wires_at(
        mid_abort_binding,
        &[211, 223],
        &[ResourceState::Live, ResourceState::Dead],
        2,
        0,
    );
    let (_, uncertain_frame) = owned_wires_at(
        uncertain_binding,
        &[227, 229],
        &[ResourceState::Live, ResourceState::Finalizing],
        1,
        1,
    );
    let initial_abort =
        encode_decision(initial_abort_binding, SettlementDecision::AbortHostUnwind).unwrap();
    let mid_abort =
        encode_decision(mid_abort_binding, SettlementDecision::AbortHostUnwind).unwrap();
    let uncertain_abort =
        encode_decision(uncertain_binding, SettlementDecision::AbortHostUnwind).unwrap();
    let mut source = emitted.source;
    writeln!(source, "#define spx_fixture_descriptor_v3 {}\n#define spx_fixture_execute_v3 {}\n#define spx_fixture_settle_v3 {}", spec.getter_symbol, spec.execute_symbol, spec.settle_symbol).unwrap();
    append_array(&mut source, "canonical_request", &request);
    append_array(&mut source, "initial_frame", &frame);
    append_array(&mut source, "accept_decision", &accept);
    append_array(&mut source, "conflict_decision", &conflict);
    append_array(&mut source, "expected_semantic", &expected_semantic);
    append_array(&mut source, "expected_accept_chain", &expected_accept_chain);
    append_array(&mut source, "initial_abort_frame", &initial_abort_frame);
    append_array(
        &mut source,
        "preexecute_abort_frame",
        &preexecute_abort_frame,
    );
    append_array(&mut source, "mid_abort_frame", &mid_abort_frame);
    append_array(&mut source, "uncertain_frame", &uncertain_frame);
    append_array(&mut source, "initial_abort_decision", &initial_abort);
    append_array(&mut source, "mid_abort_decision", &mid_abort);
    append_array(&mut source, "uncertain_abort_decision", &uncertain_abort);
    writeln!(
        source,
        "#define expected_terminal_checkpoint UINT32_C({})",
        spec.plan.terminal_checkpoint
    )
    .unwrap();
    source.push_str(
        r#"
static uint32_t finalizer_count=0,finalizer_order[8]; static uint64_t finalizer_payload[8];
static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){if(finalizer_count<8){finalizer_order[finalizer_count]=owner;finalizer_payload[finalizer_count]=payload;}finalizer_count++;}
int main(void){
 uint8_t request[sizeof(canonical_request)],frame[sizeof(initial_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(initial_frame)],hostile_frame[sizeof(initial_frame)],abort_frame[sizeof(initial_frame)];
 memcpy(request,canonical_request,sizeof(request));memcpy(frame,initial_frame,sizeof(frame));memset(response,0xa5,sizeof(response));
 request[104]^=1;if(spx_fixture_execute_v3(request,sizeof(request),frame,sizeof(frame),response,sizeof(response))!=1||finalizer_count!=0)return 1;request[104]^=1;
 memcpy(hostile_frame,initial_frame,sizeof(hostile_frame));hostile_frame[132]^=1;spx_v3_refresh_frame(hostile_frame);if(spx_fixture_execute_v3(request,sizeof(request),hostile_frame,sizeof(hostile_frame),response,sizeof(response))!=1||finalizer_count!=0)return 2;
 memcpy(hostile_frame,initial_frame,sizeof(hostile_frame));hostile_frame[196]=1;spx_v3_refresh_frame(hostile_frame);if(spx_fixture_execute_v3(request,sizeof(request),hostile_frame,sizeof(hostile_frame),response,sizeof(response))!=1||finalizer_count!=0)return 10;
 memcpy(hostile_frame,initial_frame,sizeof(hostile_frame));if(spx_fixture_execute_v3(request,sizeof(request),hostile_frame,sizeof(hostile_frame),hostile_frame+20,sizeof(response))!=1||finalizer_count!=0)return 11;
 memcpy(frame,initial_frame,sizeof(frame));if(spx_fixture_execute_v3(request,sizeof(request),frame,sizeof(frame),response,sizeof(response))!=0)return 3;
 if(finalizer_count!=2||finalizer_order[0]!=1||finalizer_order[1]!=0||finalizer_payload[0]!=73||finalizer_payload[1]!=41||spx_v3_load_u32(frame+268)!=expected_terminal_checkpoint||spx_v3_load_u32(frame+324)!=4||spx_v3_load_u32(frame+336)!=4||memcmp(frame+228,expected_semantic,32))return 4;
 if(spx_fixture_settle_v3(frame,sizeof(frame),accept_decision,sizeof(accept_decision),candidate,sizeof(candidate))!=0)return 5;
 if(spx_v3_load_u32(frame+272)!=4||spx_v3_load_u32(candidate+356)!=1||spx_v3_load_u32(candidate+372)!=1||spx_v3_load_u32(candidate+384)!=1||memcmp(candidate+228,frame+228,32)||memcmp(candidate+324,expected_accept_chain,32))return 6;
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),accept_decision,sizeof(accept_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=2)return 7;
 memcpy(saved_frame,frame,sizeof(frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),conflict_decision,sizeof(conflict_decision),candidate,sizeof(candidate))!=3||finalizer_count!=2||memcmp(frame,saved_frame,sizeof(frame))||memcmp(candidate,saved,sizeof(candidate)))return 8;
 if(memcmp(spx_fixture_descriptor_v3(),spx_v3_descriptor,sizeof(spx_v3_descriptor)))return 9;
 memcpy(abort_frame,initial_abort_frame,sizeof(abort_frame));memcpy(saved_frame,abort_frame,sizeof(abort_frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),initial_abort_decision,sizeof(initial_abort_decision),candidate,sizeof(candidate))!=3||finalizer_count!=2||memcmp(abort_frame,saved_frame,sizeof(abort_frame))||memcmp(candidate,saved,sizeof(candidate)))return 12;
 memcpy(abort_frame,preexecute_abort_frame,sizeof(abort_frame));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),initial_abort_decision,sizeof(initial_abort_decision),candidate,sizeof(candidate))!=0||finalizer_count!=4||finalizer_order[2]!=1||finalizer_order[3]!=0||finalizer_payload[2]!=203||finalizer_payload[3]!=201||spx_v3_load_u32(candidate+356)!=4||spx_v3_load_u32(candidate+372)!=1||spx_v3_load_u32(candidate+384)!=1)return 15;
 memcpy(abort_frame,mid_abort_frame,sizeof(abort_frame));memcpy(saved_frame,abort_frame,sizeof(abort_frame));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),mid_abort_decision,sizeof(mid_abort_decision),candidate,sizeof(candidate))!=3||finalizer_count!=4||memcmp(abort_frame,saved_frame,sizeof(abort_frame))||memcmp(candidate,saved,sizeof(candidate)))return 13;
 memcpy(abort_frame,uncertain_frame,sizeof(abort_frame));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),uncertain_abort_decision,sizeof(uncertain_abort_decision),candidate,sizeof(candidate))!=1||finalizer_count!=4)return 14;
 return 0;}
"#,
    );
    for optimization in ["-O0", "-O2"] {
        compile_and_run(&source, optimization);
    }
}

#[test]
fn all_fourteen_graph_witness_specs_are_unique_and_bounded() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    assert_eq!(corpus.cases.len(), 14);
    for case in &corpus.cases {
        let spec = graph_spec(&corpus.program, case);
        assert!(spec.plan.terminal_checkpoint > 1, "{}", case.scenario_id);
        assert_eq!(
            spec.plan.semantic_ordinals.len(),
            case.reference.events.len(),
            "{}",
            case.scenario_id
        );
        let emitted = emit(&spec).unwrap();
        assert!(!emitted.source.contains("malloc("), "{}", case.scenario_id);
        assert!(!emitted.source.contains("calloc("), "{}", case.scenario_id);
        assert!(!emitted.source.contains("realloc("), "{}", case.scenario_id);
        assert!(!emitted.source.contains("free("), "{}", case.scenario_id);
    }
}

#[test]
fn authoritative_fourteen_case_graph_providers_execute_and_settle_at_o0_o2() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    assert_eq!(corpus.cases.len(), 14);
    for (case_index, case) in corpus.cases.iter().enumerate() {
        let spec = graph_spec(&corpus.program, case);
        let emitted = emit(&spec).unwrap();
        let primary_binding = binding(&spec, 1_000 + case_index as u64, 2_000, 31);
        let abort_binding = binding(&spec, 3_000 + case_index as u64, 4_000, 47);
        let (request, frame, payloads) = corpus_wires(primary_binding, case.arguments.as_slice());
        let (_, abort_frame, _) = corpus_wires(abort_binding, case.arguments.as_slice());
        let decision = match spec.plan.outcome {
            ProviderV3Outcome::Scalar { .. } => {
                encode_decision(primary_binding, SettlementDecision::AcceptScalar).unwrap()
            }
            ProviderV3Outcome::SemanticFailure { .. } => {
                encode_decision(primary_binding, SettlementDecision::AcceptSemanticFailure).unwrap()
            }
            ProviderV3Outcome::Owned { owner_ordinal } => encode_decision(
                primary_binding,
                SettlementDecision::AcceptOwned { owner_ordinal },
            )
            .unwrap(),
        };
        let abort_decision =
            encode_decision(abort_binding, SettlementDecision::AbortHostUnwind).unwrap();
        let execute_finalizers = spec
            .plan
            .execute_actions
            .iter()
            .filter_map(|action| match action {
                ProviderV3ExecuteAction::Finalize { owner_ordinal, .. } => Some(*owner_ordinal),
                ProviderV3ExecuteAction::Stage { .. } => None,
            })
            .collect::<Vec<_>>();
        let execute_payloads = execute_finalizers
            .iter()
            .map(|owner| payloads[*owner as usize])
            .collect::<Vec<_>>();
        let (outcome_tag, outcome_detail, outcome_payload, semantic_class, candidate_tag) =
            match spec.plan.outcome {
                ProviderV3Outcome::Scalar { value } => (1, 0, value as u64, 1, 1),
                ProviderV3Outcome::SemanticFailure { selected_ordinal } => {
                    (2, selected_ordinal, 0, 3, 2)
                }
                ProviderV3Outcome::Owned { owner_ordinal } => {
                    (3, owner_ordinal, payloads[owner_ordinal as usize], 2, 3)
                }
            };
        let expected_semantic = semantic_digest_exact(
            spec.trace_path_certificate,
            &spec.plan.semantic_ordinals,
            semantic_class,
            outcome_detail,
        );
        let terminal = spec
            .plan
            .checkpoints
            .iter()
            .find(|checkpoint| checkpoint.id == spec.plan.terminal_checkpoint)
            .unwrap();
        let published = match spec.plan.outcome {
            ProviderV3Outcome::Owned { owner_ordinal } => Some(owner_ordinal),
            ProviderV3Outcome::Scalar { .. } | ProviderV3Outcome::SemanticFailure { .. } => None,
        };
        let accept_action_count = terminal.accept_cleanup.len() + usize::from(published.is_some());
        let mut expected_accept_chain = initial_action_chain_digest(
            decision_digest(&decision).unwrap(),
            accept_action_count as u64,
        )
        .unwrap();
        let mut record_index = 0_u64;
        for (action_index, owner_ordinal) in terminal.accept_cleanup.iter().enumerate() {
            let before = match terminal.states[*owner_ordinal as usize] {
                1 => ResourceState::Live,
                2 => ResourceState::ProvisionalResult,
                _ => panic!("accept cleanup starts from a non-live owner"),
            };
            for (boundary, from, after) in [
                (
                    ActionBoundary::FinalizerStarted,
                    before,
                    ResourceState::Finalizing,
                ),
                (
                    ActionBoundary::FinalizerCompleted,
                    ResourceState::Finalizing,
                    ResourceState::Dead,
                ),
            ] {
                let evidence = encode_action_evidence(ActionEvidence {
                    binding: primary_binding,
                    action_index: action_index as u32,
                    boundary,
                    owner_ordinal: *owner_ordinal,
                    payload: payloads[*owner_ordinal as usize],
                    before: from,
                    after,
                    checkpoint: terminal.id,
                })
                .unwrap();
                expected_accept_chain =
                    extend_action_chain_digest(expected_accept_chain, record_index, &evidence)
                        .unwrap();
                record_index += 1;
            }
        }
        if let Some(owner_ordinal) = published {
            let evidence = encode_action_evidence(ActionEvidence {
                binding: primary_binding,
                action_index: terminal.accept_cleanup.len() as u32,
                boundary: ActionBoundary::Publish,
                owner_ordinal,
                payload: payloads[owner_ordinal as usize],
                before: ResourceState::ProvisionalResult,
                after: ResourceState::Published,
                checkpoint: terminal.id,
            })
            .unwrap();
            expected_accept_chain =
                extend_action_chain_digest(expected_accept_chain, record_index, &evidence).unwrap();
        }
        let mut source = emitted.source;
        writeln!(
            source,
            "#define spx_fixture_execute_v3 {}\n#define spx_fixture_settle_v3 {}",
            spec.execute_symbol, spec.settle_symbol
        )
        .unwrap();
        append_array(&mut source, "case_request", &request);
        append_array(&mut source, "case_frame", &frame);
        append_array(&mut source, "case_decision", &decision);
        append_array(&mut source, "case_abort_frame", &abort_frame);
        append_array(&mut source, "case_abort_decision", &abort_decision);
        append_array(&mut source, "case_expected_semantic", &expected_semantic);
        append_array(
            &mut source,
            "case_expected_accept_chain",
            &expected_accept_chain,
        );
        append_u32_array(&mut source, "case_execute_order", &execute_finalizers);
        append_u64_array(&mut source, "case_execute_payload", &execute_payloads);
        writeln!(
            source,
            "#define CASE_EXECUTE_COUNT UINT32_C({})\n#define CASE_EVENT_COUNT UINT32_C({})\n#define CASE_OUTCOME_TAG UINT32_C({outcome_tag})\n#define CASE_OUTCOME_DETAIL UINT32_C({outcome_detail})\n#define CASE_OUTCOME_PAYLOAD UINT64_C({outcome_payload})\n#define CASE_CANDIDATE_TAG UINT32_C({candidate_tag})\n#define CASE_TERMINAL_CHECKPOINT UINT32_C({})",
            execute_finalizers.len(),
            spec.plan.semantic_ordinals.len(),
            spec.plan.terminal_checkpoint,
        )
        .unwrap();

        let mut hostile = case.arguments.clone();
        let hostile_wires = hostile
            .iter_mut()
            .find_map(|argument| match argument {
                OwnedResourceCorpusArgument::Bool(value) => {
                    *value = !*value;
                    Some(())
                }
                OwnedResourceCorpusArgument::I64(value) => {
                    *value = value.wrapping_add(1);
                    Some(())
                }
                OwnedResourceCorpusArgument::Owned(_) => None,
            })
            .map(|()| {
                let hostile_binding = binding(&spec, 5_000 + case_index as u64, 6_000, 63);
                corpus_wires(hostile_binding, &hostile)
            });
        if let Some((hostile_request, hostile_frame, _)) = hostile_wires {
            append_array(&mut source, "case_hostile_request", &hostile_request);
            append_array(&mut source, "case_hostile_frame", &hostile_frame);
            source.push_str("#define CASE_HAS_HOSTILE_SCALAR UINT32_C(1)\n");
        } else {
            append_array(&mut source, "case_hostile_request", &request);
            append_array(&mut source, "case_hostile_frame", &frame);
            source.push_str("#define CASE_HAS_HOSTILE_SCALAR UINT32_C(0)\n");
        }
        source.push_str(
            r#"
static uint32_t finalizer_count=0,finalizer_order[16];static uint64_t finalizer_payload[16];
static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){if(finalizer_count<16){finalizer_order[finalizer_count]=owner;finalizer_payload[finalizer_count]=payload;}finalizer_count++;}
int main(void){
 uint8_t frame[sizeof(case_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],abort_frame[sizeof(case_abort_frame)],saved_abort_frame[sizeof(case_abort_frame)],hostile_frame[sizeof(case_hostile_frame)];
 if(CASE_HAS_HOSTILE_SCALAR){memcpy(hostile_frame,case_hostile_frame,sizeof(hostile_frame));if(spx_fixture_execute_v3(case_hostile_request,sizeof(case_hostile_request),hostile_frame,sizeof(hostile_frame),response,sizeof(response))!=1||finalizer_count!=0)return 1;}
 memcpy(frame,case_frame,sizeof(frame));if(spx_fixture_execute_v3(case_request,sizeof(case_request),frame,sizeof(frame),response,sizeof(response))!=0)return 2;
 if(spx_v3_load_u32(frame+268)!=CASE_TERMINAL_CHECKPOINT||spx_v3_load_u32(response+132)!=CASE_TERMINAL_CHECKPOINT||spx_v3_load_u32(response+136)!=CASE_OUTCOME_TAG||spx_v3_load_u32(response+140)!=CASE_OUTCOME_DETAIL||spx_v3_load_u64(response+144)!=CASE_OUTCOME_PAYLOAD||spx_v3_load_u32(response+152)!=CASE_EVENT_COUNT||memcmp(frame+228,case_expected_semantic,32))return 3;
 if(finalizer_count!=CASE_EXECUTE_COUNT)return 4;for(uint32_t i=0;i<CASE_EXECUTE_COUNT;i++)if(finalizer_order[i]!=case_execute_order[i]||finalizer_payload[i]!=case_execute_payload[i])return 5;
 if(spx_fixture_settle_v3(frame,sizeof(frame),case_decision,sizeof(case_decision),candidate,sizeof(candidate))!=0||spx_v3_load_u32(candidate+356)!=CASE_CANDIDATE_TAG||spx_v3_load_u32(candidate+360)!=(CASE_CANDIDATE_TAG==3?CASE_OUTCOME_DETAIL:0)||spx_v3_load_u32(candidate+368)!=SPX_V3_RESOURCE_COUNT||memcmp(candidate+228,case_expected_semantic,32)||memcmp(candidate+324,case_expected_accept_chain,32))return 6;
 for(uint32_t owner=0;owner<SPX_V3_RESOURCE_COUNT;owner++){uint32_t disposition=spx_v3_load_u32(candidate+372+12*owner);uint32_t expected=CASE_CANDIDATE_TAG==3&&owner==CASE_OUTCOME_DETAIL?2:1;if(disposition!=expected||spx_v3_load_u64(candidate+376+12*owner)!=spx_v3_load_u64(frame+spx_v3_cell(owner)+4))return 7;}
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),case_decision,sizeof(case_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=CASE_EXECUTE_COUNT)return 8;
 memcpy(abort_frame,case_abort_frame,sizeof(abort_frame));memcpy(saved_abort_frame,abort_frame,sizeof(abort_frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),case_abort_decision,sizeof(case_abort_decision),candidate,sizeof(candidate))!=3||memcmp(abort_frame,saved_abort_frame,sizeof(abort_frame))||memcmp(candidate,saved,sizeof(candidate))||finalizer_count!=CASE_EXECUTE_COUNT)return 9;
 return 0;}
"#,
        );
        for optimization in ["-O0", "-O2"] {
            compile_and_run_labeled(&source, optimization, case.scenario_id);
        }
    }
}

#[test]
fn owned_identity_accept_abort_and_finalizing_uncertainty_are_exact_at_o0_o2() {
    let plan = ProviderV3Plan::OwnedIdentity {
        owner_ordinal: 0,
        staged_checkpoint: 2,
        semantic_ordinals: vec![1, 2, 3],
    };
    let spec = spec("token.identity", plan);
    let emitted = emit(&spec).unwrap();
    let first_binding = binding(&spec, 21, 31, 6);
    let second_binding = binding(&spec, 22, 32, 7);
    let (request, frame) = initial_owned_wires(first_binding, &[99]);
    let (abort_request, abort_frame) = initial_owned_wires(second_binding, &[101]);
    let accept = encode_decision(
        first_binding,
        SettlementDecision::AcceptOwned { owner_ordinal: 0 },
    )
    .unwrap();
    let abort = encode_decision(second_binding, SettlementDecision::AbortMalformed).unwrap();
    let expected_semantic = semantic_digest(spec.trace_path_certificate, &[1, 2, 3], 2);
    let accept_decision_digest = decision_digest(&accept).unwrap();
    let accept_seed = initial_action_chain_digest(accept_decision_digest, 1).unwrap();
    let accept_action = encode_action_evidence(ActionEvidence {
        binding: first_binding,
        action_index: 0,
        boundary: ActionBoundary::Publish,
        owner_ordinal: 0,
        payload: 99,
        before: ResourceState::ProvisionalResult,
        after: ResourceState::Published,
        checkpoint: spec.plan.terminal_checkpoint,
    })
    .unwrap();
    let expected_accept_chain = extend_action_chain_digest(accept_seed, 0, &accept_action).unwrap();
    let abort_decision_digest = decision_digest(&abort).unwrap();
    let abort_seed = initial_action_chain_digest(abort_decision_digest, 1).unwrap();
    let abort_started = encode_action_evidence(ActionEvidence {
        binding: second_binding,
        action_index: 0,
        boundary: ActionBoundary::FinalizerStarted,
        owner_ordinal: 0,
        payload: 101,
        before: ResourceState::ProvisionalResult,
        after: ResourceState::Finalizing,
        checkpoint: spec.plan.terminal_checkpoint,
    })
    .unwrap();
    let abort_completed = encode_action_evidence(ActionEvidence {
        binding: second_binding,
        action_index: 0,
        boundary: ActionBoundary::FinalizerCompleted,
        owner_ordinal: 0,
        payload: 101,
        before: ResourceState::Finalizing,
        after: ResourceState::Dead,
        checkpoint: spec.plan.terminal_checkpoint,
    })
    .unwrap();
    let abort_after_started = extend_action_chain_digest(abort_seed, 0, &abort_started).unwrap();
    let expected_abort_chain =
        extend_action_chain_digest(abort_after_started, 1, &abort_completed).unwrap();
    let mut source = emitted.source;
    writeln!(source, "#define spx_fixture_descriptor_v3 {}\n#define spx_fixture_execute_v3 {}\n#define spx_fixture_settle_v3 {}", spec.getter_symbol, spec.execute_symbol, spec.settle_symbol).unwrap();
    append_array(&mut source, "identity_request", &request);
    append_array(&mut source, "identity_frame", &frame);
    append_array(&mut source, "abort_request", &abort_request);
    append_array(&mut source, "abort_frame", &abort_frame);
    append_array(&mut source, "accept_decision", &accept);
    append_array(&mut source, "abort_decision", &abort);
    append_array(&mut source, "expected_semantic", &expected_semantic);
    append_array(&mut source, "expected_accept_chain", &expected_accept_chain);
    append_array(&mut source, "expected_abort_chain", &expected_abort_chain);
    source.push_str(
        r#"
static uint32_t finalizer_count=0;static uint64_t finalized_payload=0;
static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){if(owner!=0)return;finalizer_count++;finalized_payload=payload;}
int main(void){
 uint8_t frame[sizeof(identity_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],other[sizeof(abort_frame)],forged[sizeof(abort_frame)],saved_frame[sizeof(abort_frame)],other_response[SPX_V3_RESPONSE_BYTES],uncertain[sizeof(abort_frame)],dh[32],zero[32]={0};
 memcpy(frame,identity_frame,sizeof(frame));if(spx_fixture_execute_v3(identity_request,sizeof(identity_request),frame,sizeof(frame),response,sizeof(response))!=0||spx_v3_load_u32(frame+324)!=2)return 1;
 if(memcmp(frame+228,expected_semantic,32)||spx_fixture_settle_v3(frame,sizeof(frame),accept_decision,sizeof(accept_decision),candidate,sizeof(candidate))!=0||finalizer_count!=0||spx_v3_load_u32(frame+324)!=5||spx_v3_load_u32(candidate+356)!=3||spx_v3_load_u32(candidate+372)!=2||spx_v3_load_u64(candidate+376)!=99||memcmp(candidate+228,frame+228,32)||memcmp(candidate+324,expected_accept_chain,32))return 2;
 memcpy(saved,candidate,sizeof(saved));spx_v3_store_u32(frame+272,3);spx_v3_refresh_frame(frame);if(spx_fixture_settle_v3(frame,sizeof(frame),accept_decision,sizeof(accept_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=0)return 7;
 memcpy(other,abort_frame,sizeof(other));if(spx_fixture_execute_v3(abort_request,sizeof(abort_request),other,sizeof(other),other_response,sizeof(other_response))!=0)return 3;
 spx_v3_decision_digest(abort_decision,dh);memset(other+228,0,32);memcpy(other+276,dh,32);spx_v3_action_seed(other,dh,1);spx_v3_store_u32(other+272,2);spx_v3_refresh_frame(other);memcpy(forged,other,sizeof(forged));forged[spx_v3_action_digest_offset()]^=1;spx_v3_refresh_frame(forged);memcpy(saved_frame,forged,sizeof(forged));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(forged,sizeof(forged),abort_decision,sizeof(abort_decision),candidate,sizeof(candidate))!=3||finalizer_count!=0||memcmp(forged,saved_frame,sizeof(forged))||memcmp(candidate,saved,sizeof(candidate)))return 8;if(spx_fixture_settle_v3(other,sizeof(other),abort_decision,sizeof(abort_decision),candidate,sizeof(candidate))!=0||finalizer_count!=1||finalized_payload!=101||spx_v3_load_u32(other+324)!=4||spx_v3_load_u32(candidate+356)!=4||spx_v3_load_u32(candidate+372)!=1||memcmp(other+228,zero,32)||memcmp(candidate+228,zero,32)||memcmp(candidate+324,expected_abort_chain,32))return 4;
 memcpy(uncertain,abort_frame,sizeof(uncertain));if(spx_fixture_execute_v3(abort_request,sizeof(abort_request),uncertain,sizeof(uncertain),other_response,sizeof(other_response))!=0)return 5;spx_v3_decision_digest(abort_decision,dh);memcpy(uncertain+276,dh,32);spx_v3_action_seed(uncertain,dh,1);spx_v3_store_u32(uncertain+272,3);spx_v3_store_u32(uncertain+316,1);spx_v3_store_u32(uncertain+324,3);spx_v3_refresh_frame(uncertain);if(spx_fixture_settle_v3(uncertain,sizeof(uncertain),abort_decision,sizeof(abort_decision),candidate,sizeof(candidate))!=1||finalizer_count!=1)return 6;
 return 0;}
"#,
    );
    for optimization in ["-O0", "-O2"] {
        compile_and_run(&source, optimization);
    }
}

#[test]
fn physical_failure_injection_and_durable_settlement_boundaries_are_exact_at_o0_o2() {
    fn fixture(
        fault: ProviderV3TestFault,
        invocation: u64,
        decision: SettlementDecision,
        body: &str,
    ) -> String {
        let plan = ProviderV3Plan::OwnedIdentity {
            owner_ordinal: 0,
            staged_checkpoint: 2,
            semantic_ordinals: vec![1, 2, 3],
        };
        let spec = spec("token.identity", plan).with_test_fault(fault).unwrap();
        let provider_binding = binding(&spec, invocation, invocation + 100, invocation as u8);
        let (request, frame) = initial_owned_wires(provider_binding, &[900 + invocation]);
        let decision = encode_decision(provider_binding, decision).unwrap();
        let conflict =
            encode_decision(provider_binding, SettlementDecision::AbortHostUnwind).unwrap();
        let mut source = emit(&spec).unwrap().source;
        writeln!(
            source,
            "#define spx_fixture_execute_v3 {}\n#define spx_fixture_settle_v3 {}",
            spec.execute_symbol, spec.settle_symbol
        )
        .unwrap();
        append_array(&mut source, "fixture_request", &request);
        append_array(&mut source, "fixture_frame", &frame);
        append_array(&mut source, "fixture_decision", &decision);
        append_array(&mut source, "fixture_conflict", &conflict);
        source.push_str(
            r#"
static uint32_t finalizer_count=0;static uint64_t finalized_payload=0;
static void spx_v3_generated_finalize(uint32_t owner,uint64_t payload){if(owner==0){finalizer_count++;finalized_payload=payload;}}
"#,
        );
        source.push_str(body);
        source
    }

    let physical = fixture(
        ProviderV3TestFault::PhysicalFailure {
            checkpoint: 2,
            code: 71,
        },
        61,
        SettlementDecision::AbortPhysical { code: 71 },
        r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)],pending[sizeof(fixture_frame)],zero[32]={0};
 memcpy(frame,fixture_frame,sizeof(frame));memset(response,0xa5,sizeof(response));
 if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=71)return 1;
 if(spx_v3_load_u32(frame+260)!=2||spx_v3_load_u32(frame+264)!=71||spx_v3_load_u32(frame+268)!=2||spx_v3_load_u32(frame+324)!=2||spx_v3_zero(frame+196,32)||memcmp(frame+228,zero,32)||finalizer_count!=0)return 2;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||finalizer_count!=1||finalized_payload!=961||spx_v3_load_u32(frame+324)!=4||spx_v3_load_u32(candidate+356)!=4)return 3;
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 4;
 memcpy(saved_frame,frame,sizeof(frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_conflict,sizeof(fixture_conflict),candidate,sizeof(candidate))!=3||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 5;
 memcpy(pending,fixture_frame,sizeof(pending));memcpy(saved_frame,pending,sizeof(pending));memset(candidate,0x5a,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(pending,sizeof(pending),fixture_conflict,sizeof(fixture_conflict),candidate,sizeof(candidate))!=3||memcmp(saved_frame,pending,sizeof(pending))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 6;
 return 0;}
"#,
    );

    let malformed_response = fixture(
        ProviderV3TestFault::MalformedResponse { offset: 0 },
        62,
        SettlementDecision::AbortMalformed,
        r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)];
 memcpy(frame,fixture_frame,sizeof(frame));if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=0||response[0]!='R')return 1;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||finalizer_count!=1||finalized_payload!=962||spx_v3_load_u32(candidate+356)!=4)return 2;
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 3;
 memcpy(saved_frame,frame,sizeof(frame));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_conflict,sizeof(fixture_conflict),candidate,sizeof(candidate))!=3||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 4;
 return 0;}
"#,
    );

    let malformed_frame = fixture(
        ProviderV3TestFault::MalformedFrame { offset: 0 },
        63,
        SettlementDecision::AbortMalformed,
        r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)];
 (void)fixture_conflict;
 memcpy(frame,fixture_frame,sizeof(frame));if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=0||frame[0]!='R')return 1;
 memcpy(saved_frame,frame,sizeof(frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=1||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=0)return 2;
 return 0;}
"#,
    );

    let malformed_candidate = fixture(
        ProviderV3TestFault::MalformedCandidate { offset: 0 },
        64,
        SettlementDecision::AbortMalformed,
        r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)];
 (void)fixture_conflict;
 memcpy(frame,fixture_frame,sizeof(frame));if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=0)return 1;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||candidate[0]!='R'||finalizer_count!=1)return 2;
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 3;
 memcpy(saved_frame,frame,sizeof(frame));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_conflict,sizeof(fixture_conflict),candidate,sizeof(candidate))!=3||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 4;
 return 0;}
"#,
    );

    let boundary_body = |boundary: u32| {
        if boundary == 1 {
            r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)];
 (void)fixture_conflict;
 memcpy(frame,fixture_frame,sizeof(frame));if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=0)return 1;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=3||spx_v3_load_u32(frame+316)!=1||spx_v3_load_u32(frame+324)!=3||finalizer_count!=0)return 2;
 memcpy(saved_frame,frame,sizeof(frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=1||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=0)return 3;
 return 0;}
"#
        } else if boundary == 2 {
            r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)];
 (void)fixture_conflict;
 memcpy(frame,fixture_frame,sizeof(frame));if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=0)return 1;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=3||spx_v3_load_u32(frame+316)!=1||spx_v3_load_u32(frame+324)!=3||finalizer_count!=1||finalized_payload!=972)return 2;
 memcpy(saved_frame,frame,sizeof(frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=1||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1||finalized_payload!=972)return 3;
 return 0;}
"#
        } else {
            r#"
int main(void){
 uint8_t frame[sizeof(fixture_frame)],response[SPX_V3_RESPONSE_BYTES],candidate[SPX_V3_CANDIDATE_BYTES],saved[SPX_V3_CANDIDATE_BYTES],saved_frame[sizeof(fixture_frame)];
 (void)fixture_conflict;
 memcpy(frame,fixture_frame,sizeof(frame));if(spx_fixture_execute_v3(fixture_request,sizeof(fixture_request),frame,sizeof(frame),response,sizeof(response))!=0)return 1;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=3||finalizer_count!=1||spx_v3_load_u32(frame+316)!=0||spx_v3_load_u32(frame+308)!=1||spx_v3_load_u32(frame+324)!=4)return 2;
 if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||finalizer_count!=1||spx_v3_load_u32(candidate+356)!=4)return 3;
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_decision,sizeof(fixture_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 4;
 memcpy(saved_frame,frame,sizeof(frame));if(spx_fixture_settle_v3(frame,sizeof(frame),fixture_conflict,sizeof(fixture_conflict),candidate,sizeof(candidate))!=3||memcmp(saved_frame,frame,sizeof(frame))||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=1)return 5;
 return 0;}
"#
        }
    };
    let boundaries = [1_u32, 2, 3].map(|boundary| {
        fixture(
            ProviderV3TestFault::FinalizerInterruption {
                action: 0,
                boundary,
            },
            70 + u64::from(boundary),
            SettlementDecision::AbortMalformed,
            boundary_body(boundary),
        )
    });

    let cases = [
        ("physical", &physical),
        ("malformed-response", &malformed_response),
        ("malformed-frame", &malformed_frame),
        ("malformed-candidate", &malformed_candidate),
        ("finalizer-start", &boundaries[0]),
        ("finalizer-effect", &boundaries[1]),
        ("finalizer-complete", &boundaries[2]),
    ];
    for (label, source) in cases {
        for optimization in ["-O0", "-O2"] {
            compile_and_run_labeled(source, optimization, label);
        }
    }
}

#[test]
fn provider_spec_rejects_noncanonical_plan_symbols_and_capacities() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    let function = corpus
        .program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.identity")
        .unwrap();
    let mut descriptor = native_callable_abi_v3::derive(&corpus.program, &function.id).unwrap();
    descriptor.getter_symbol = "1bad".to_owned();
    let bad = NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal: 0,
            staged_checkpoint: 2,
            semantic_ordinals: vec![1, 2, 3],
        },
    );
    assert!(bad.is_err());

    let descriptor = native_callable_abi_v3::derive(&corpus.program, &function.id).unwrap();
    let excessive_events = vec![1; descriptor.maximum_events as usize + 1];
    assert!(NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal: 0,
            staged_checkpoint: 2,
            semantic_ordinals: excessive_events,
        },
    )
    .is_err());

    assert!(validate_plan(
        &ProviderV3Plan::OwnedIdentity {
            owner_ordinal: 0,
            staged_checkpoint: 2,
            semantic_ordinals: vec![1],
        },
        2,
        1,
        1,
    )
    .is_err());

    let mut descriptor = native_callable_abi_v3::derive(&corpus.program, &function.id).unwrap();
    descriptor.bytes.truncate(HEADER_BYTES as usize);
    assert!(NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal: 0,
            staged_checkpoint: 2,
            semantic_ordinals: vec![1],
        },
    )
    .is_err());

    assert!(derive_private_artifact(
        &corpus.program,
        &function.id,
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal: 0,
            staged_checkpoint: 2,
            semantic_ordinals: vec![1, 2, 3],
        },
    )
    .is_ok());

    let discard_two = corpus
        .program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "token.discard-two")
        .unwrap();
    let descriptor = native_callable_abi_v3::derive(&corpus.program, &discard_two.id).unwrap();
    assert!(NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::ScalarDiscard {
            scalar_result: 0,
            finalizer_order: vec![0, 1],
            completed_checkpoints: vec![2, 3],
            semantic_ordinals: vec![1, 2, 3, 4, 5],
        },
    )
    .is_err());

    let checked_case = corpus
        .cases
        .iter()
        .find(|case| case.scenario_id == "checked-success")
        .unwrap();
    let good = graph_spec(&corpus.program, checked_case);
    let mut reversed = good.plan.semantic_ordinals.clone();
    reversed.reverse();
    let descriptor = native_callable_abi_v3::derive(
        &corpus.program,
        &DeclarationId::new(checked_case.function_id),
    )
    .unwrap();
    assert!(NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::GraphWitness {
            scalar_arguments: good.plan.scalar_arguments.clone(),
            outcome: good.plan.outcome,
            semantic_ordinals: reversed,
        },
    )
    .is_err());

    let descriptor = native_callable_abi_v3::derive(
        &corpus.program,
        &DeclarationId::new(checked_case.function_id),
    )
    .unwrap();
    assert!(NativeCallableProviderV3Spec::new(
        descriptor,
        ProviderV3Plan::GraphWitness {
            scalar_arguments: vec![ProviderV3ScalarArgument {
                parameter_index: 1,
                value: ProviderV3ScalarValue::Bool(true),
            }],
            outcome: good.plan.outcome,
            semantic_ordinals: good.plan.semantic_ordinals,
        },
    )
    .is_err());
}
