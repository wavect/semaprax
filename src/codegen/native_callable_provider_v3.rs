//! Private C11 provider for the bounded physical callable-v3 proof corpus.
//!
//! This emitter is deliberately unreachable from public compilation. It emits
//! one exact descriptor getter plus synchronous six-argument `execute` and
//! `settle` entry points. Corpus scenarios seal their scalar inputs while all
//! ownership paths, checkpoints, and settlement permutations come from the
//! authenticated descriptor graph.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable-v3 provider remains behind SPX-B104")
)]

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::owned_resource_corpus::OwnedResourceCorpusArgument;

use super::native_callable_abi_v3::NativeCallableV3Descriptor;
use super::native_callable_wire_v3::{
    candidate_receipt_capacity, execute_response_capacity, frame_capacity, ACTION_EVIDENCE_BYTES,
    DECISION_BYTES, HEADER_BYTES, HOST_RECEIPT_BYTES, PRE_EXECUTE_HOST_UNWIND_CODE, VERSION,
};

const MAX_SYMBOL_BYTES: usize = 1024;
const MAX_EVENTS: u32 = 256;
const MAX_RESOURCES: u32 = 4_096;
const TEST_FAULT_DISABLED: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProviderV3FaultConfig {
    physical_failure_checkpoint: u32,
    physical_failure_code: u32,
    malformed_response_offset: u32,
    malformed_frame_offset: u32,
    malformed_candidate_offset: u32,
    finalizer_action: u32,
    finalizer_boundary: u32,
}

impl Default for ProviderV3FaultConfig {
    fn default() -> Self {
        Self {
            physical_failure_checkpoint: TEST_FAULT_DISABLED,
            physical_failure_code: 0,
            malformed_response_offset: TEST_FAULT_DISABLED,
            malformed_frame_offset: TEST_FAULT_DISABLED,
            malformed_candidate_offset: TEST_FAULT_DISABLED,
            finalizer_action: TEST_FAULT_DISABLED,
            finalizer_boundary: 0,
        }
    }
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderV3TestFault {
    PhysicalFailure { checkpoint: u32, code: u32 },
    MalformedResponse { offset: u32 },
    MalformedFrame { offset: u32 },
    MalformedCandidate { offset: u32 },
    FinalizerInterruption { action: u32, boundary: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ProviderV3Plan {
    ScalarDiscard {
        scalar_result: i64,
        finalizer_order: Vec<u32>,
        completed_checkpoints: Vec<u32>,
        semantic_ordinals: Vec<u32>,
    },
    OwnedIdentity {
        owner_ordinal: u32,
        staged_checkpoint: u32,
        semantic_ordinals: Vec<u32>,
    },
    /// One exact corpus scenario. Scalar values are part of the sealed
    /// provider projection; physical ownership actions are not. They are
    /// recovered exclusively from the authenticated settlement graph.
    GraphWitness {
        scalar_arguments: Vec<ProviderV3ScalarArgument>,
        outcome: ProviderV3Outcome,
        semantic_ordinals: Vec<u32>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderV3ScalarValue {
    I64(i64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProviderV3ScalarArgument {
    pub(super) parameter_index: u32,
    pub(super) value: ProviderV3ScalarValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProviderV3Outcome {
    Scalar { value: i64 },
    SemanticFailure { selected_ordinal: u32 },
    Owned { owner_ordinal: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderV3ParameterKind {
    I64,
    Bool,
    Owned { owner_ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProviderV3Parameter {
    index: u32,
    kind: ProviderV3ParameterKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderV3Result {
    ScalarI64,
    Owned { owner_ordinal: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderV3ExecuteAction {
    Finalize { owner_ordinal: u32, checkpoint: u32 },
    Stage { owner_ordinal: u32, checkpoint: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProviderV3Checkpoint {
    id: u32,
    states: Vec<u32>,
    outcome: Option<ProviderV3OutcomeClass>,
    abort_cleanup: Vec<u32>,
    accept_cleanup: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProviderV3OutcomeClass {
    Scalar,
    SemanticFailure,
    Owned { owner_ordinal: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedProviderV3Plan {
    scalar_arguments: Vec<ProviderV3ScalarArgument>,
    outcome: ProviderV3Outcome,
    semantic_ordinals: Vec<u32>,
    execute_actions: Vec<ProviderV3ExecuteAction>,
    checkpoints: Vec<ProviderV3Checkpoint>,
    terminal_checkpoint: u32,
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BoundProviderPhysicalTarget {
    IosStatic(super::native_callable_provider::IosProviderPhysicalTarget),
    AndroidDynamic(super::native_callable_provider::AndroidProviderPhysicalTarget),
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
impl BoundProviderPhysicalTarget {
    fn canonical_tag(self) -> &'static str {
        match self {
            Self::IosStatic(target) => target.canonical_tag(),
            Self::AndroidDynamic(target) => target.canonical_tag(),
        }
    }

    const fn linkage_profile(self) -> u32 {
        match self {
            Self::IosStatic(_) => 2,
            Self::AndroidDynamic(_) => 1,
        }
    }

    fn guards(self) -> String {
        match self {
            Self::IosStatic(target) => {
                super::native_callable_provider::ios_provider_target_guards(target)
            }
            Self::AndroidDynamic(target) => {
                super::native_callable_provider::android_provider_target_guards(target)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableProviderV3Spec {
    descriptor: Vec<u8>,
    getter_symbol: String,
    execute_symbol: String,
    settle_symbol: String,
    call_contract: [u8; 32],
    recovery_contract: [u8; 32],
    settlement_graph: [u8; 32],
    trace_path_certificate: [u8; 32],
    request_bytes: u32,
    response_bytes: u32,
    frame_bytes: u32,
    decision_bytes: u32,
    action_bytes: u32,
    candidate_bytes: u32,
    resource_count: u32,
    maximum_events: u32,
    dictionary_entries: u32,
    parameters: Vec<ProviderV3Parameter>,
    plan: ResolvedProviderV3Plan,
    fault: ProviderV3FaultConfig,
    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    bound_target: Option<BoundProviderPhysicalTarget>,
}

impl NativeCallableProviderV3Spec {
    pub(super) fn new(
        descriptor: NativeCallableV3Descriptor,
        plan: ProviderV3Plan,
    ) -> Result<Self, Diagnostic> {
        let (parameters, _result, plan) = validate_descriptor_exact(&descriptor, &plan)?;
        let NativeCallableV3Descriptor {
            bytes: descriptor,
            getter_symbol,
            execute_symbol,
            settle_symbol,
            call_contract,
            recovery_contract,
            settlement_graph,
            trace_path_certificate,
            request_bytes,
            maximum_events,
            dictionary_entries,
            resource_count,
        } = descriptor;
        for symbol in [&getter_symbol, &execute_symbol, &settle_symbol] {
            if !is_c_symbol(symbol) || symbol.len() > MAX_SYMBOL_BYTES {
                return Err(provider_error(
                    "provider symbol is not a bounded C identifier",
                ));
            }
        }
        if getter_symbol == execute_symbol
            || getter_symbol == settle_symbol
            || execute_symbol == settle_symbol
        {
            return Err(provider_error("provider symbols are not pairwise distinct"));
        }
        if descriptor.len() < HEADER_BYTES as usize
            || descriptor.get(..8) != Some(b"SPXNABI3")
            || read_u32(&descriptor, 8)? != VERSION
            || read_u32(&descriptor, 12)? != HEADER_BYTES
            || read_u32(&descriptor, 16)? as usize != descriptor.len()
        {
            return Err(provider_error(
                "descriptor bytes are not canonical SPXNABI3",
            ));
        }
        if [
            call_contract,
            recovery_contract,
            settlement_graph,
            trace_path_certificate,
        ]
        .iter()
        .any(|digest| digest.iter().all(|byte| *byte == 0))
        {
            return Err(provider_error("provider binding is uninitialized"));
        }
        if resource_count == 0
            || resource_count > MAX_RESOURCES
            || maximum_events == 0
            || maximum_events > MAX_EVENTS
            || dictionary_entries == 0
        {
            return Err(provider_error(
                "provider count is outside the first-v3 bound",
            ));
        }
        let response_bytes = execute_response_capacity(maximum_events)
            .map_err(|_| provider_error("response capacity overflow"))?;
        let frame_bytes = frame_capacity(resource_count)
            .map_err(|_| provider_error("frame capacity overflow"))?;
        let candidate_bytes = candidate_receipt_capacity(resource_count)
            .map_err(|_| provider_error("candidate capacity overflow"))?;
        Ok(Self {
            descriptor,
            getter_symbol,
            execute_symbol,
            settle_symbol,
            call_contract,
            recovery_contract,
            settlement_graph,
            trace_path_certificate,
            request_bytes,
            response_bytes,
            frame_bytes,
            decision_bytes: DECISION_BYTES,
            action_bytes: ACTION_EVIDENCE_BYTES,
            candidate_bytes,
            resource_count,
            maximum_events,
            dictionary_entries,
            parameters,
            plan,
            fault: ProviderV3FaultConfig::default(),
            #[cfg(any(test, feature = "unstable-native-host-internal"))]
            bound_target: None,
        })
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    pub(super) fn new_ios_static(
        descriptor: NativeCallableV3Descriptor,
        plan: ProviderV3Plan,
        target: super::native_callable_provider::IosProviderPhysicalTarget,
    ) -> Result<Self, Diagnostic> {
        Self::new_target_bound(
            descriptor,
            plan,
            BoundProviderPhysicalTarget::IosStatic(target),
        )
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    pub(super) fn new_android_dynamic(
        descriptor: NativeCallableV3Descriptor,
        plan: ProviderV3Plan,
        target: super::native_callable_provider::AndroidProviderPhysicalTarget,
    ) -> Result<Self, Diagnostic> {
        Self::new_target_bound(
            descriptor,
            plan,
            BoundProviderPhysicalTarget::AndroidDynamic(target),
        )
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    fn new_target_bound(
        descriptor: NativeCallableV3Descriptor,
        plan: ProviderV3Plan,
        target: BoundProviderPhysicalTarget,
    ) -> Result<Self, Diagnostic> {
        if !descriptor_has_target_and_profile(
            &descriptor.bytes,
            target.canonical_tag(),
            target.linkage_profile(),
        )? {
            return Err(provider_error(
                "target-bound provider target guard does not match descriptor",
            ));
        }
        let mut spec = Self::new(descriptor, plan)?;
        spec.bound_target = Some(target);
        Ok(spec)
    }

    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    pub(super) fn with_test_fault(
        mut self,
        fault: ProviderV3TestFault,
    ) -> Result<Self, Diagnostic> {
        match fault {
            ProviderV3TestFault::PhysicalFailure { checkpoint, code } => {
                if code == 0
                    || !self
                        .plan
                        .checkpoints
                        .iter()
                        .any(|candidate| candidate.id == checkpoint)
                {
                    return Err(provider_error(
                        "physical-failure injection is not a nonzero certified checkpoint",
                    ));
                }
                self.fault.physical_failure_checkpoint = checkpoint;
                self.fault.physical_failure_code = code;
            }
            ProviderV3TestFault::MalformedResponse { offset } => {
                if offset >= self.response_bytes {
                    return Err(provider_error(
                        "malformed-response injection is outside response storage",
                    ));
                }
                self.fault.malformed_response_offset = offset;
            }
            ProviderV3TestFault::MalformedFrame { offset } => {
                if offset >= self.frame_bytes {
                    return Err(provider_error(
                        "malformed-frame injection is outside frame storage",
                    ));
                }
                self.fault.malformed_frame_offset = offset;
            }
            ProviderV3TestFault::MalformedCandidate { offset } => {
                if offset >= self.candidate_bytes {
                    return Err(provider_error(
                        "malformed-candidate injection is outside candidate storage",
                    ));
                }
                self.fault.malformed_candidate_offset = offset;
            }
            ProviderV3TestFault::FinalizerInterruption { action, boundary } => {
                if !(1..=3).contains(&boundary) || action >= self.resource_count {
                    return Err(provider_error(
                        "finalizer interruption is outside the bounded action/boundary range",
                    ));
                }
                self.fault.finalizer_action = action;
                self.fault.finalizer_boundary = boundary;
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableProviderV3 {
    pub(super) source: String,
}

pub(super) fn emit(
    spec: &NativeCallableProviderV3Spec,
) -> Result<NativeCallableProviderV3, Diagnostic> {
    let mut source = String::new();
    emit_prelude(&mut source, spec)?;
    emit_execute(&mut source, spec);
    emit_settle(&mut source, spec);
    source.push_str("#if defined(__cplusplus)\n}\n#endif\n");
    Ok(NativeCallableProviderV3 { source })
}

/// Private artifact path: derive the exact ABI descriptor and seal the
/// provider source around that same value. This remains unreachable from the
/// public compiler while SPX-B104 is closed.
#[allow(dead_code, reason = "private composition remains behind SPX-B104")]
pub(super) fn derive_private_artifact(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    plan: ProviderV3Plan,
) -> Result<NativeCallableProviderV3, Diagnostic> {
    let descriptor = super::native_callable_abi_v3::derive(program, function_id)?;
    emit(&NativeCallableProviderV3Spec::new(descriptor, plan)?)
}

/// Rebuild one sealed corpus witness from the stable function identity,
/// canonical arguments, and independently executed semantic trace. Cleanup
/// actions are intentionally absent: the authenticated graph remains their
/// sole authority when `NativeCallableProviderV3Spec` validates this plan.
pub(super) fn corpus_witness_plan(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
    arguments: &[OwnedResourceCorpusArgument],
    expected_owned_result_ordinal: Option<usize>,
    reference: &crate::conformance::ConformanceTrace,
) -> Result<ProviderV3Plan, Diagnostic> {
    let function = program
        .functions
        .iter()
        .find(|function| &function.id == function_id)
        .ok_or_else(|| provider_error("corpus function identity is not in the program"))?;
    let dictionary = crate::semantic_trace::build_semantic_event_dictionary(program, &function.id)?;
    let semantic_ordinals = reference
        .events
        .iter()
        .map(|event| {
            dictionary.ordinal_for(&event.event).ok_or_else(|| {
                provider_error("corpus event is absent from the semantic dictionary")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outcome = match &reference.outcome {
        crate::conformance::TraceOutcome::Success {
            result: crate::conformance::TraceResult::I64(value),
        } => ProviderV3Outcome::Scalar { value: *value },
        crate::conformance::TraceOutcome::Success {
            result: crate::conformance::TraceResult::Owned { .. },
        } => ProviderV3Outcome::Owned {
            owner_ordinal: u32::try_from(expected_owned_result_ordinal.ok_or_else(|| {
                provider_error("owned corpus outcome has no expected owner ordinal")
            })?)
            .map_err(|_| provider_error("owned corpus result ordinal exceeds u32"))?,
        },
        crate::conformance::TraceOutcome::Failure { .. } => {
            let selected_ordinal = reference
                .events
                .iter()
                .find_map(|event| {
                    matches!(
                        event.event,
                        crate::conformance::TraceEventKind::SelectFailure { .. }
                    )
                    .then(|| dictionary.ordinal_for(&event.event))
                    .flatten()
                })
                .ok_or_else(|| provider_error("failure corpus trace has no selected status"))?;
            ProviderV3Outcome::SemanticFailure { selected_ordinal }
        }
        crate::conformance::TraceOutcome::Success { .. } => {
            return Err(provider_error("corpus result is outside callable v3"));
        }
    };
    let scalar_arguments = arguments
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
    Ok(ProviderV3Plan::GraphWitness {
        scalar_arguments,
        outcome,
        semantic_ordinals,
    })
}

fn emit_prelude(
    output: &mut String,
    spec: &NativeCallableProviderV3Spec,
) -> Result<(), Diagnostic> {
    output.push_str("/* semaprax.native-callable-provider.v3; private; SPX-B104 closed */\n#include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\n#include <string.h>\n");
    #[cfg(any(test, feature = "unstable-native-host-internal"))]
    if let Some(target) = spec.bound_target {
        output.push_str(&target.guards());
    } else {
        output.push_str(&super::native_callable_provider::provider_target_guards()?);
    }
    #[cfg(not(any(test, feature = "unstable-native-host-internal")))]
    output.push_str(&super::native_callable_provider::provider_target_guards()?);
    output.push_str("#if defined(__cplusplus)\n#define SPX_V3_STATIC_ASSERT(c,m) static_assert((c),m)\nextern \"C\" {\n#else\n#define SPX_V3_STATIC_ASSERT(c,m) _Static_assert((c),m)\n#endif\n#if defined(_WIN32)\n#define SPX_V3_API __declspec(dllexport)\n#define SPX_V3_CALL __cdecl\n#elif defined(__GNUC__) || defined(__clang__)\n#define SPX_V3_API __attribute__((visibility(\"default\")))\n#define SPX_V3_CALL\n#else\n#define SPX_V3_API\n#define SPX_V3_CALL\n#endif\nSPX_V3_STATIC_ASSERT(sizeof(uint32_t)==4,\"u32\");\nSPX_V3_STATIC_ASSERT(sizeof(uint64_t)==8,\"u64\");\n");
    for (name, value) in [
        ("SPX_V3_REQUEST_BYTES", spec.request_bytes),
        ("SPX_V3_RESPONSE_BYTES", spec.response_bytes),
        ("SPX_V3_FRAME_BYTES", spec.frame_bytes),
        ("SPX_V3_DECISION_BYTES", spec.decision_bytes),
        ("SPX_V3_ACTION_BYTES", spec.action_bytes),
        ("SPX_V3_CANDIDATE_BYTES", spec.candidate_bytes),
        ("SPX_V3_RESOURCE_COUNT", spec.resource_count),
        ("SPX_V3_PARAMETER_COUNT", spec.parameters.len() as u32),
        ("SPX_V3_MAX_EVENTS", spec.maximum_events),
        ("SPX_V3_DICTIONARY_ENTRIES", spec.dictionary_entries),
        (
            "SPX_V3_PREEXECUTE_UNWIND_CODE",
            PRE_EXECUTE_HOST_UNWIND_CODE,
        ),
        (
            "SPX_V3_FAULT_PHYSICAL_CHECKPOINT",
            spec.fault.physical_failure_checkpoint,
        ),
        (
            "SPX_V3_FAULT_PHYSICAL_CODE",
            spec.fault.physical_failure_code,
        ),
        (
            "SPX_V3_FAULT_RESPONSE_OFFSET",
            spec.fault.malformed_response_offset,
        ),
        (
            "SPX_V3_FAULT_FRAME_OFFSET",
            spec.fault.malformed_frame_offset,
        ),
        (
            "SPX_V3_FAULT_CANDIDATE_OFFSET",
            spec.fault.malformed_candidate_offset,
        ),
        ("SPX_V3_FAULT_FINALIZER_ACTION", spec.fault.finalizer_action),
        (
            "SPX_V3_FAULT_FINALIZER_BOUNDARY",
            spec.fault.finalizer_boundary,
        ),
    ] {
        writeln!(output, "#define {name} UINT32_C({value})").expect("string write");
    }
    output.push_str("static const uint32_t spx_v3_parameter_kind[SPX_V3_PARAMETER_COUNT]={");
    for parameter in &spec.parameters {
        let kind = match parameter.kind {
            ProviderV3ParameterKind::I64 => 1,
            ProviderV3ParameterKind::Bool => 2,
            ProviderV3ParameterKind::Owned { .. } => 3,
        };
        write!(output, "UINT32_C({kind}),").expect("string write");
    }
    output.push_str("};\nstatic const uint32_t spx_v3_parameter_owner[SPX_V3_PARAMETER_COUNT]={");
    for parameter in &spec.parameters {
        let owner = match parameter.kind {
            ProviderV3ParameterKind::Owned { owner_ordinal } => owner_ordinal,
            ProviderV3ParameterKind::I64 | ProviderV3ParameterKind::Bool => u32::MAX,
        };
        write!(output, "UINT32_C({owner}),").expect("string write");
    }
    output.push_str("};\n");
    emit_c_array(output, "spx_v3_descriptor", &spec.descriptor);
    emit_c_array(output, "spx_v3_call_contract", &spec.call_contract);
    emit_c_array(output, "spx_v3_recovery_contract", &spec.recovery_contract);
    emit_c_array(output, "spx_v3_settlement_graph", &spec.settlement_graph);
    emit_c_array(
        output,
        "spx_v3_trace_path_certificate",
        &spec.trace_path_certificate,
    );
    let unwind_response_digest = super::native_callable_wire_v3::response_storage_digest(
        PRE_EXECUTE_HOST_UNWIND_CODE,
        &vec![0; spec.response_bytes as usize],
    )
    .expect("validated response capacity has a canonical unwind digest");
    emit_c_array(
        output,
        "spx_v3_preexecute_unwind_response_digest",
        &unwind_response_digest,
    );
    for (name, domain) in [
        (
            "spx_v3_request_domain",
            b"semaprax.native-callable-request-digest.v3\0".as_slice(),
        ),
        (
            "spx_v3_response_domain",
            b"semaprax.native-callable-execute-response-storage-digest.v3\0".as_slice(),
        ),
        (
            "spx_v3_decision_domain",
            b"semaprax.native-callable-decision-digest.v3\0".as_slice(),
        ),
        (
            "spx_v3_action_seed_domain",
            b"semaprax.native-callable-action-chain-seed.v3\0".as_slice(),
        ),
        (
            "spx_v3_frame_domain",
            b"semaprax.native-callable-pre-candidate-frame-digest.v3\0".as_slice(),
        ),
        (
            "spx_v3_trace_domain",
            b"semaprax.native-recovery-trace-evidence.v1\0".as_slice(),
        ),
    ] {
        emit_c_array(output, name, domain);
    }
    emit_c_array(
        output,
        "spx_v3_action_step_domain",
        b"semaprax.native-callable-action-chain-step.v3\0",
    );
    output.push_str(SHA256_C);
    output.push_str(
        "static void spx_v3_generated_finalize(uint32_t owner_ordinal, uint64_t payload);\n",
    );
    output.push_str(HELPERS_C);
    writeln!(
        output,
        "SPX_V3_API const uint8_t *SPX_V3_CALL {}(void) {{ return spx_v3_descriptor; }}",
        spec.getter_symbol
    )
    .expect("string write");
    Ok(())
}

fn emit_execute(output: &mut String, spec: &NativeCallableProviderV3Spec) {
    writeln!(output, "SPX_V3_API uint32_t SPX_V3_CALL {}(const uint8_t *request,uint32_t request_len,uint8_t *frame,uint32_t frame_len,uint8_t *response,uint32_t response_len) {{", spec.execute_symbol).expect("string write");
    output.push_str("  uint64_t payloads[SPX_V3_RESOURCE_COUNT],arguments[SPX_V3_PARAMETER_COUNT]; uint8_t request_hash[32];\n  (void)spx_v3_begin_finalizer;(void)spx_v3_complete_finalizer;(void)spx_v3_stage_owned;\n  if (!spx_v3_validate_execute_inputs(request,request_len,frame,frame_len,response,response_len,payloads,arguments,request_hash)) return UINT32_C(1);\n");
    for argument in &spec.plan.scalar_arguments {
        let bits = match argument.value {
            ProviderV3ScalarValue::I64(value) => value as u64,
            ProviderV3ScalarValue::Bool(value) => u64::from(value),
        };
        writeln!(
            output,
            "  if(arguments[UINT32_C({})]!=UINT64_C({bits})) return UINT32_C(1);",
            argument.parameter_index
        )
        .expect("string write");
    }
    output.push_str("  memset(response,0,response_len);\n  { uint32_t spx_fault=spx_v3_maybe_physical_failure(frame,response,response_len,UINT32_C(1)); if(spx_fault!=0)return spx_fault; }\n");
    for action in &spec.plan.execute_actions {
        match *action {
            ProviderV3ExecuteAction::Finalize {
                owner_ordinal,
                checkpoint,
            } => {
                writeln!(output, "  if (!spx_v3_begin_finalizer(frame,UINT32_C({owner_ordinal}))) return UINT32_C(2);\n  spx_v3_generated_finalize(UINT32_C({owner_ordinal}),payloads[{owner_ordinal}]);\n  if (!spx_v3_complete_finalizer(frame,UINT32_C({owner_ordinal}),UINT32_C({checkpoint}))) return UINT32_C(2);\n  {{ uint32_t spx_fault=spx_v3_maybe_physical_failure(frame,response,response_len,UINT32_C({checkpoint})); if(spx_fault!=0)return spx_fault; }}").expect("string write");
            }
            ProviderV3ExecuteAction::Stage {
                owner_ordinal,
                checkpoint,
            } => {
                writeln!(output, "  if (!spx_v3_stage_owned(frame,UINT32_C({owner_ordinal}),UINT32_C({checkpoint}))) return UINT32_C(2);\n  {{ uint32_t spx_fault=spx_v3_maybe_physical_failure(frame,response,response_len,UINT32_C({checkpoint})); if(spx_fault!=0)return spx_fault; }}").expect("string write");
            }
        }
    }
    writeln!(
        output,
        "  spx_v3_store_u32(frame+268,UINT32_C({}));\n  {{ uint32_t spx_fault=spx_v3_maybe_physical_failure(frame,response,response_len,UINT32_C({})); if(spx_fault!=0)return spx_fault; }}",
        spec.plan.terminal_checkpoint,
        spec.plan.terminal_checkpoint
    )
    .expect("string write");
    let (tag, detail, payload) = match spec.plan.outcome {
        ProviderV3Outcome::Scalar { value } => (1, 0, value.to_string()),
        ProviderV3Outcome::SemanticFailure { selected_ordinal } => {
            (2, selected_ordinal, "0".to_owned())
        }
        ProviderV3Outcome::Owned { owner_ordinal } => {
            (3, owner_ordinal, format!("payloads[{owner_ordinal}]"))
        }
    };
    emit_response_header(
        output,
        spec.plan.semantic_ordinals.len(),
        &tag.to_string(),
        &detail.to_string(),
        &payload,
    );
    for ordinal in &spec.plan.semantic_ordinals {
        writeln!(
            output,
            "  spx_v3_store_u32(response+spx_write,UINT32_C({ordinal})); spx_write+=UINT32_C(4);"
        )
        .expect("string write");
    }
    output.push_str("  if (spx_write != spx_total) return UINT32_C(2);\n  spx_v3_response_digest(UINT32_C(0),response,frame+196); spx_v3_semantic_digest(response,frame+228); spx_v3_store_u32(frame+260,UINT32_C(2)); spx_v3_store_u32(frame+264,UINT32_C(0)); spx_v3_refresh_frame(frame);\n  if(SPX_V3_FAULT_RESPONSE_OFFSET!=UINT32_MAX)response[SPX_V3_FAULT_RESPONSE_OFFSET]^=UINT8_C(1);\n  if(SPX_V3_FAULT_FRAME_OFFSET!=UINT32_MAX)frame[SPX_V3_FAULT_FRAME_OFFSET]^=UINT8_C(1);\n  return UINT32_C(0);\n}\n");
}

fn emit_response_header(
    output: &mut String,
    event_count: usize,
    outcome: &str,
    detail: &str,
    payload: &str,
) {
    writeln!(output, "  uint32_t spx_total=UINT32_C(156)+UINT32_C(4)*UINT32_C({event_count}); uint32_t spx_write=UINT32_C(156);\n  memcpy(response,\"SPXNEX03\",8); spx_v3_store_u32(response+8,UINT32_C(3)); spx_v3_store_u32(response+12,UINT32_C(20)); spx_v3_store_u32(response+16,spx_total);\n  memcpy(response+20,spx_v3_call_contract,32); memcpy(response+52,request+52,48); memcpy(response+100,request_hash,32); spx_v3_store_u32(response+132,spx_v3_load_u32(frame+268)); spx_v3_store_u32(response+136,UINT32_C({outcome})); spx_v3_store_u32(response+140,UINT32_C({detail})); spx_v3_store_u64(response+144,(uint64_t)({payload})); spx_v3_store_u32(response+152,UINT32_C({event_count}));").expect("string write");
}

fn emit_settle(output: &mut String, spec: &NativeCallableProviderV3Spec) {
    let checkpoints = &spec.plan.checkpoints;
    writeln!(
        output,
        "static const uint32_t spx_v3_checkpoint_count=UINT32_C({});",
        checkpoints.len()
    )
    .expect("string write");
    for (name, values) in [
        (
            "spx_v3_checkpoint_id",
            checkpoints.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        ),
        (
            "spx_v3_checkpoint_outcome",
            checkpoints
                .iter()
                .map(|entry| match entry.outcome {
                    None => 0,
                    Some(ProviderV3OutcomeClass::Scalar) => 1,
                    Some(ProviderV3OutcomeClass::SemanticFailure) => 2,
                    Some(ProviderV3OutcomeClass::Owned { .. }) => 3,
                })
                .collect::<Vec<_>>(),
        ),
        (
            "spx_v3_checkpoint_detail",
            checkpoints
                .iter()
                .map(|entry| match entry.outcome {
                    Some(ProviderV3OutcomeClass::Owned { owner_ordinal }) => owner_ordinal,
                    None
                    | Some(
                        ProviderV3OutcomeClass::Scalar | ProviderV3OutcomeClass::SemanticFailure,
                    ) => 0,
                })
                .collect::<Vec<_>>(),
        ),
        (
            "spx_v3_abort_count",
            checkpoints
                .iter()
                .map(|entry| entry.abort_cleanup.len() as u32)
                .collect::<Vec<_>>(),
        ),
        (
            "spx_v3_accept_count",
            checkpoints
                .iter()
                .map(|entry| entry.accept_cleanup.len() as u32)
                .collect::<Vec<_>>(),
        ),
    ] {
        writeln!(
            output,
            "static const uint32_t {name}[{}]={{{}}};",
            values.len(),
            values
                .iter()
                .map(|value| format!("UINT32_C({value})"))
                .collect::<Vec<_>>()
                .join(",")
        )
        .expect("string write");
    }
    for (name, values) in [
        (
            "spx_v3_checkpoint_state",
            checkpoints
                .iter()
                .flat_map(|entry| entry.states.iter().copied())
                .collect::<Vec<_>>(),
        ),
        (
            "spx_v3_abort_order",
            checkpoints
                .iter()
                .flat_map(|entry| {
                    entry
                        .abort_cleanup
                        .iter()
                        .copied()
                        .chain(std::iter::repeat_n(
                            0,
                            spec.resource_count as usize - entry.abort_cleanup.len(),
                        ))
                })
                .collect::<Vec<_>>(),
        ),
        (
            "spx_v3_accept_order",
            checkpoints
                .iter()
                .flat_map(|entry| {
                    entry
                        .accept_cleanup
                        .iter()
                        .copied()
                        .chain(std::iter::repeat_n(
                            0,
                            spec.resource_count as usize - entry.accept_cleanup.len(),
                        ))
                })
                .collect::<Vec<_>>(),
        ),
    ] {
        writeln!(
            output,
            "static const uint32_t {name}[{}]={{{}}};",
            values.len(),
            values
                .iter()
                .map(|value| format!("UINT32_C({value})"))
                .collect::<Vec<_>>()
                .join(",")
        )
        .expect("string write");
    }
    writeln!(output, "SPX_V3_API uint32_t SPX_V3_CALL {}(uint8_t *frame,uint32_t frame_len,const uint8_t *decision,uint32_t decision_len,uint8_t *candidate,uint32_t candidate_len) {{", spec.settle_symbol).expect("string write");
    output.push_str(r#"
  uint8_t decision_hash[32],replay[SPX_V3_FRAME_BYTES];uint32_t decision_tag,decision_detail,phase,checkpoint_index=UINT32_MAX,next,action_count=0,record_count=0;uint32_t action_kind[SPX_V3_RESOURCE_COUNT+1],action_owner[SPX_V3_RESOURCE_COUNT+1];
  if(!spx_v3_validate_settle_inputs(frame,frame_len,decision,decision_len,candidate,candidate_len,decision_hash,&decision_tag,&decision_detail))return UINT32_C(1);
  phase=spx_v3_load_u32(frame+272);if(phase==UINT32_C(4)){if(memcmp(frame+276,decision_hash,32))return UINT32_C(3);return spx_v3_emit_candidate_with_fault(frame,decision_tag,decision_detail,candidate);}if(phase!=1&&phase!=2&&phase!=3)return UINT32_C(3);
  for(uint32_t i=0;i<spx_v3_checkpoint_count;i++){if(spx_v3_checkpoint_id[i]==spx_v3_load_u32(frame+268)){if(checkpoint_index!=UINT32_MAX)return UINT32_C(3);checkpoint_index=i;}}
  if(checkpoint_index==UINT32_MAX)return UINT32_C(3);
  if(decision_tag<4){if(spx_v3_load_u32(frame+260)!=2||spx_v3_load_u32(frame+264)!=0||spx_v3_zero(frame+196,32)||spx_v3_zero(frame+228,32)||decision_tag!=spx_v3_checkpoint_outcome[checkpoint_index]||(decision_tag==3&&decision_detail!=spx_v3_checkpoint_detail[checkpoint_index]))return UINT32_C(3);for(uint32_t i=0;i<spx_v3_accept_count[checkpoint_index];i++){action_kind[action_count]=1;action_owner[action_count++]=spx_v3_accept_order[checkpoint_index*SPX_V3_RESOURCE_COUNT+i];}if(decision_tag==3){action_kind[action_count]=2;action_owner[action_count++]=decision_detail;}}
  else{if(decision_tag==4&&(spx_v3_load_u32(frame+260)!=2||spx_v3_load_u32(frame+264)!=decision_detail||spx_v3_zero(frame+196,32)))return UINT32_C(3);if((decision_tag==5||decision_tag==6)&&(spx_v3_load_u32(frame+260)!=2||spx_v3_load_u32(frame+264)!=0||spx_v3_zero(frame+196,32)))return UINT32_C(3);if(decision_tag==7&&(spx_v3_load_u32(frame+260)!=3||spx_v3_load_u32(frame+264)!=SPX_V3_PREEXECUTE_UNWIND_CODE||memcmp(frame+196,spx_v3_preexecute_unwind_response_digest,32)||!spx_v3_zero(frame+228,32)))return UINT32_C(3);for(uint32_t i=0;i<spx_v3_abort_count[checkpoint_index];i++){action_kind[action_count]=1;action_owner[action_count++]=spx_v3_abort_order[checkpoint_index*SPX_V3_RESOURCE_COUNT+i];}}
  next=spx_v3_load_u32(frame+308);if(next>action_count)return UINT32_C(3);for(uint32_t owner=0;owner<SPX_V3_RESOURCE_COUNT;owner++){uint32_t want=spx_v3_checkpoint_state[checkpoint_index*SPX_V3_RESOURCE_COUNT+owner];for(uint32_t i=0;i<next;i++)if(action_owner[i]==owner)want=action_kind[i]==1?4:5;if(spx_v3_load_u32(frame+spx_v3_cell(owner))!=want)return UINT32_C(3);}
  for(uint32_t i=0;i<next;i++)record_count+=action_kind[i]==1?2:1;
  if(phase==1){if(next!=0||spx_v3_load_u32(frame+312)!=0)return UINT32_C(3);if(decision_tag>=4)memset(frame+228,0,32);if(!spx_v3_lock_decision(frame,decision_hash,(uint64_t)action_count))return UINT32_C(3);}else{if(memcmp(frame+276,decision_hash,32))return UINT32_C(3);if(phase==2){if(next!=0||spx_v3_load_u32(frame+312)!=0||!spx_v3_valid_action_seed(frame,decision_hash,(uint64_t)action_count))return UINT32_C(3);}else{if(spx_v3_load_u32(frame+312)!=record_count)return UINT32_C(3);memcpy(replay,frame,SPX_V3_FRAME_BYTES);spx_v3_action_seed(replay,decision_hash,(uint64_t)action_count);spx_v3_store_u32(replay+312,0);for(uint32_t i=0;i<next;i++){uint32_t owner=action_owner[i],before=spx_v3_checkpoint_state[checkpoint_index*SPX_V3_RESOURCE_COUNT+owner];if(action_kind[i]==1){spx_v3_action_step(replay,i,1,owner,before,3);spx_v3_action_step(replay,i,2,owner,3,4);}else spx_v3_action_step(replay,i,3,owner,2,5);}if(memcmp(replay+spx_v3_action_digest_offset(),frame+spx_v3_action_digest_offset(),32))return UINT32_C(3);}}
  for(uint32_t i=next;i<action_count;i++){if(action_kind[i]==1){if(!spx_v3_settlement_finalize(frame,i,action_owner[i]))return UINT32_C(3);}else if(!spx_v3_publish(frame,i,action_owner[i]))return UINT32_C(3);}
  spx_v3_store_u32(frame+272,UINT32_C(4));spx_v3_refresh_frame(frame);return spx_v3_emit_candidate_with_fault(frame,decision_tag,decision_detail,candidate);
}
"#);
}

fn validate_descriptor_exact(
    descriptor: &NativeCallableV3Descriptor,
    plan: &ProviderV3Plan,
) -> Result<
    (
        Vec<ProviderV3Parameter>,
        ProviderV3Result,
        ResolvedProviderV3Plan,
    ),
    Diagnostic,
> {
    let bytes = &descriptor.bytes;
    if bytes.len() < HEADER_BYTES as usize
        || bytes.get(..8) != Some(b"SPXNABI3")
        || read_u32(bytes, 8)? != VERSION
        || read_u32(bytes, 12)? != HEADER_BYTES
        || read_u32(bytes, 16)? as usize != bytes.len()
    {
        return Err(provider_error("descriptor header is not exact SPXNABI3"));
    }
    let mut at = HEADER_BYTES as usize;
    let _target = descriptor_text(bytes, &mut at)?;
    let _linkage = descriptor_u32(bytes, &mut at)?;
    let mut fingerprints = [[0_u8; 32]; 19];
    for fingerprint in &mut fingerprints {
        fingerprint.copy_from_slice(descriptor_bytes(bytes, &mut at, 32)?);
        if fingerprint.iter().all(|byte| *byte == 0) {
            return Err(provider_error("descriptor fingerprint is uninitialized"));
        }
    }
    if fingerprints[7] != descriptor.trace_path_certificate
        || fingerprints[8] != descriptor.recovery_contract
        || fingerprints[9] != descriptor.settlement_graph
        || fingerprints[18] != descriptor.call_contract
    {
        return Err(provider_error("descriptor fingerprint metadata diverges"));
    }
    let _module = descriptor_text(bytes, &mut at)?;
    let _function = descriptor_text(bytes, &mut at)?;
    let getter = descriptor_text(bytes, &mut at)?;
    let execute = descriptor_text(bytes, &mut at)?;
    let settle = descriptor_text(bytes, &mut at)?;
    if getter != descriptor.getter_symbol
        || execute != descriptor.execute_symbol
        || settle != descriptor.settle_symbol
        || descriptor_u32(bytes, &mut at)? != 3
        || descriptor_u32(bytes, &mut at)? != 0x03ff
    {
        return Err(provider_error(
            "descriptor symbols or ABI obligations diverge",
        ));
    }
    let mut capacities = [0_u32; 15];
    for capacity in &mut capacities {
        *capacity = descriptor_u32(bytes, &mut at)?;
    }
    let retained = capacities[0]
        .checked_add(capacities[1])
        .and_then(|value| value.checked_add(capacities[2]))
        .and_then(|value| value.checked_add(capacities[3]))
        .and_then(|value| value.checked_add(capacities[4]))
        .and_then(|value| value.checked_add(capacities[5]))
        .and_then(|value| value.checked_add(HOST_RECEIPT_BYTES))
        .and_then(|value| value.checked_mul(320))
        .ok_or_else(|| provider_error("descriptor reserve capacity overflow"))?;
    if capacities[0] != descriptor.request_bytes
        || capacities[1]
            != execute_response_capacity(descriptor.maximum_events)
                .map_err(|_| provider_error("descriptor response capacity overflow"))?
        || capacities[2]
            != frame_capacity(descriptor.resource_count)
                .map_err(|_| provider_error("descriptor frame capacity overflow"))?
        || capacities[3] != DECISION_BYTES
        || capacities[4] != ACTION_EVIDENCE_BYTES
        || capacities[5]
            != candidate_receipt_capacity(descriptor.resource_count)
                .map_err(|_| provider_error("descriptor candidate capacity overflow"))?
        || capacities[6] != descriptor.maximum_events
        || capacities[8] != descriptor.dictionary_entries
        || capacities[9] != descriptor.resource_count
        || capacities[12] != 256
        || capacities[13] != 64
        || capacities[14] != retained
    {
        return Err(provider_error("descriptor capacities diverge"));
    }
    let parameter_count = descriptor_u32(bytes, &mut at)?;
    let mut parameters = Vec::with_capacity(parameter_count as usize);
    let mut next_owner = 0_u32;
    let mut expected_request = 104_u32;
    for expected in 0..parameter_count {
        let tag = descriptor_u32(bytes, &mut at)?;
        if descriptor_u32(bytes, &mut at)? != expected {
            return Err(provider_error("descriptor parameter index is noncanonical"));
        }
        let _value = descriptor_text(bytes, &mut at)?;
        let (kind, increment) = match tag {
            1 => match descriptor_u32(bytes, &mut at)? {
                1 => (ProviderV3ParameterKind::I64, 16_u32),
                2 => (ProviderV3ParameterKind::Bool, 12_u32),
                _ => return Err(provider_error("descriptor scalar wire kind is unsupported")),
            },
            2 => {
                let owner_ordinal = descriptor_u32(bytes, &mut at)?;
                if owner_ordinal != next_owner {
                    return Err(provider_error("descriptor owner ordinal is noncanonical"));
                }
                next_owner = next_owner
                    .checked_add(1)
                    .ok_or_else(|| provider_error("descriptor owner ordinal overflow"))?;
                let _resource = descriptor_text(bytes, &mut at)?;
                let _lifecycle = descriptor_text(bytes, &mut at)?;
                if descriptor_u32(bytes, &mut at)? != 1 {
                    return Err(provider_error(
                        "descriptor payload wire kind is unsupported",
                    ));
                }
                (ProviderV3ParameterKind::Owned { owner_ordinal }, 20_u32)
            }
            _ => return Err(provider_error("descriptor parameter kind is unsupported")),
        };
        expected_request = expected_request
            .checked_add(increment)
            .ok_or_else(|| provider_error("request capacity overflow"))?;
        parameters.push(ProviderV3Parameter {
            index: expected,
            kind,
        });
    }
    if next_owner != descriptor.resource_count || expected_request != descriptor.request_bytes {
        return Err(provider_error(
            "descriptor request capacity or owned signature diverges",
        ));
    }
    let result = match descriptor_u32(bytes, &mut at)? {
        1 => ProviderV3Result::ScalarI64,
        2 => {
            let parameter_index = descriptor_u32(bytes, &mut at)?;
            let _value = descriptor_text(bytes, &mut at)?;
            let owner_ordinal = descriptor_u32(bytes, &mut at)?;
            if !matches!(
                parameters.get(parameter_index as usize),
                Some(ProviderV3Parameter {
                    index,
                    kind: ProviderV3ParameterKind::Owned { owner_ordinal: admitted },
                }) if *index == parameter_index && *admitted == owner_ordinal
            ) {
                return Err(provider_error("descriptor owned result diverges"));
            }
            ProviderV3Result::Owned { owner_ordinal }
        }
        _ => return Err(provider_error("descriptor result kind is unsupported")),
    };
    let graph_len = descriptor_u32(bytes, &mut at)? as usize;
    let graph = descriptor_bytes(bytes, &mut at, graph_len)?;
    if at != bytes.len() {
        return Err(provider_error("descriptor has trailing bytes"));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.native-callable-settlement-graph.v3\0");
    hasher.update((graph.len() as u64).to_be_bytes());
    hasher.update(graph);
    if <[u8; 32]>::from(hasher.finalize()) != descriptor.settlement_graph {
        return Err(provider_error(
            "descriptor settlement graph digest diverges",
        ));
    }
    let resolved = validate_plan_against_graph(
        graph,
        plan,
        descriptor.resource_count,
        &parameters,
        result,
        descriptor.maximum_events,
        descriptor.dictionary_entries,
    )?;
    Ok((parameters, result, resolved))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GraphAction {
    Finalize(u32),
    Stage(u32),
    Certify {
        ordinals: Vec<u32>,
        outcome: u32,
        detail: u32,
    },
}

fn validate_plan_against_graph(
    graph: &[u8],
    plan: &ProviderV3Plan,
    resources: u32,
    parameters: &[ProviderV3Parameter],
    result: ProviderV3Result,
    maximum_events: u32,
    dictionary_entries: u32,
) -> Result<ResolvedProviderV3Plan, Diagnostic> {
    let mut at = 0;
    if descriptor_u32(graph, &mut at)? != 3 {
        return Err(provider_error("settlement graph version diverges"));
    }
    let _function = descriptor_text(graph, &mut at)?;
    let _bindings = descriptor_bytes(graph, &mut at, 96)?;
    if descriptor_u32(graph, &mut at)? != resources {
        return Err(provider_error("settlement graph resource count diverges"));
    }
    let checkpoint_count = descriptor_u32(graph, &mut at)?;
    let mut checkpoints = Vec::with_capacity(checkpoint_count as usize);
    for expected_id in 1..=checkpoint_count {
        let id = descriptor_u32(graph, &mut at)?;
        if id != expected_id {
            return Err(provider_error("settlement checkpoints are not dense"));
        }
        if descriptor_u32(graph, &mut at)? != resources {
            return Err(provider_error("settlement checkpoint width diverges"));
        }
        let mut states = Vec::with_capacity(resources as usize);
        for _ in 0..resources {
            let state = descriptor_u32(graph, &mut at)?;
            if !matches!(state, 1 | 2 | 4) {
                return Err(provider_error("settlement resource state is invalid"));
            }
            states.push(state);
        }
        let outcome = match descriptor_u32(graph, &mut at)? {
            0 => None,
            1 => Some(ProviderV3OutcomeClass::Scalar),
            2 => Some(ProviderV3OutcomeClass::SemanticFailure),
            3 => Some(ProviderV3OutcomeClass::Owned {
                owner_ordinal: descriptor_u32(graph, &mut at)?,
            }),
            _ => return Err(provider_error("settlement checkpoint outcome is invalid")),
        };
        let abort_cleanup = graph_ordinals(graph, &mut at)?;
        let accept_cleanup = graph_ordinals(graph, &mut at)?;
        validate_checkpoint(resources, &states, outcome, &abort_cleanup, &accept_cleanup)?;
        checkpoints.push(ProviderV3Checkpoint {
            id,
            states,
            outcome,
            abort_cleanup,
            accept_cleanup,
        });
    }
    let starts = graph_ordinals(graph, &mut at)?;
    if starts
        .iter()
        .any(|start| *start == 0 || *start > checkpoint_count)
    {
        return Err(provider_error("settlement start checkpoint is invalid"));
    }
    let edge_count = descriptor_u32(graph, &mut at)?;
    let mut edges = Vec::new();
    for _ in 0..edge_count {
        let from = descriptor_u32(graph, &mut at)?;
        let to = descriptor_u32(graph, &mut at)?;
        let action = match descriptor_u32(graph, &mut at)? {
            1 => GraphAction::Finalize(descriptor_u32(graph, &mut at)?),
            2 => GraphAction::Stage(descriptor_u32(graph, &mut at)?),
            3 => {
                let _digest = descriptor_bytes(graph, &mut at, 32)?;
                let ordinals = graph_ordinals(graph, &mut at)?;
                let outcome = descriptor_u32(graph, &mut at)?;
                let detail = if outcome == 3 {
                    descriptor_u32(graph, &mut at)?
                } else if !matches!(outcome, 1 | 2) {
                    return Err(provider_error("trace witness outcome is invalid"));
                } else {
                    0
                };
                GraphAction::Certify {
                    ordinals,
                    outcome,
                    detail,
                }
            }
            _ => return Err(provider_error("settlement graph action is invalid")),
        };
        edges.push((from, to, action));
    }
    if at != graph.len() || starts.is_empty() {
        return Err(provider_error("settlement graph is not exact"));
    }
    let (scalar_arguments, requested_outcome, semantic_ordinals) = match plan {
        ProviderV3Plan::ScalarDiscard {
            scalar_result,
            semantic_ordinals,
            ..
        } => (
            Vec::new(),
            ProviderV3Outcome::Scalar {
                value: *scalar_result,
            },
            semantic_ordinals.clone(),
        ),
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal,
            semantic_ordinals,
            ..
        } => (
            Vec::new(),
            ProviderV3Outcome::Owned {
                owner_ordinal: *owner_ordinal,
            },
            semantic_ordinals.clone(),
        ),
        ProviderV3Plan::GraphWitness {
            scalar_arguments,
            outcome,
            semantic_ordinals,
        } => (
            scalar_arguments.clone(),
            *outcome,
            semantic_ordinals.clone(),
        ),
    };
    validate_scalar_arguments(parameters, &scalar_arguments)?;
    if semantic_ordinals.is_empty()
        || semantic_ordinals.len() > maximum_events as usize
        || semantic_ordinals
            .iter()
            .any(|ordinal| *ordinal == 0 || *ordinal > dictionary_entries)
    {
        return Err(provider_error("semantic ordinals are outside dictionary"));
    }
    match (result, requested_outcome) {
        (ProviderV3Result::ScalarI64, ProviderV3Outcome::Scalar { .. })
        | (ProviderV3Result::ScalarI64, ProviderV3Outcome::SemanticFailure { .. }) => {}
        (
            ProviderV3Result::Owned {
                owner_ordinal: expected,
            },
            ProviderV3Outcome::Owned { owner_ordinal },
        ) if expected == owner_ordinal => {}
        (ProviderV3Result::Owned { .. }, ProviderV3Outcome::SemanticFailure { .. }) => {}
        _ => {
            return Err(provider_error(
                "provider outcome diverges from descriptor result",
            ))
        }
    }
    let (wanted_checkpoint, wanted_trace) = match requested_outcome {
        ProviderV3Outcome::Scalar { .. } => ((1, 0), (1, 0)),
        ProviderV3Outcome::SemanticFailure { selected_ordinal } => ((2, 0), (3, selected_ordinal)),
        ProviderV3Outcome::Owned { owner_ordinal } => ((3, owner_ordinal), (2, 0)),
    };
    let mut paths = Vec::new();
    for start in starts {
        collect_paths(
            start,
            wanted_checkpoint,
            &checkpoints,
            &edges,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut paths,
        );
    }
    paths.retain(|path| {
        path.iter().any(|(action, _)| {
            matches!(
                action,
                GraphAction::Certify { ordinals, outcome, detail }
                    if ordinals == &semantic_ordinals
                        && *outcome == wanted_trace.0
                        && *detail == wanted_trace.1
            )
        })
    });
    paths.sort();
    paths.dedup();
    if paths.len() != 1 {
        return Err(provider_error("settlement graph normal path is not unique"));
    }
    let path = &paths[0];
    let terminal_checkpoint = path
        .last()
        .map(|(_, checkpoint)| *checkpoint)
        .ok_or_else(|| provider_error("settlement graph path is empty"))?;
    let witnesses = path
        .iter()
        .filter_map(|(action, _)| match action {
            GraphAction::Certify {
                ordinals,
                outcome,
                detail,
            } => Some((ordinals, *outcome, *detail)),
            GraphAction::Finalize(_) | GraphAction::Stage(_) => None,
        })
        .collect::<Vec<_>>();
    if witnesses.as_slice() != [(&semantic_ordinals, wanted_trace.0, wanted_trace.1)]
        || !matches!(path.last(), Some((GraphAction::Certify { .. }, _)))
    {
        return Err(provider_error("provider trace witness diverges from graph"));
    }
    let execute_actions = path
        .iter()
        .filter_map(|(action, checkpoint)| match action {
            GraphAction::Finalize(owner_ordinal) => Some(ProviderV3ExecuteAction::Finalize {
                owner_ordinal: *owner_ordinal,
                checkpoint: *checkpoint,
            }),
            GraphAction::Stage(owner_ordinal) => Some(ProviderV3ExecuteAction::Stage {
                owner_ordinal: *owner_ordinal,
                checkpoint: *checkpoint,
            }),
            GraphAction::Certify { .. } => None,
        })
        .collect::<Vec<_>>();
    match plan {
        ProviderV3Plan::ScalarDiscard {
            scalar_result,
            finalizer_order,
            completed_checkpoints,
            semantic_ordinals: _,
        } => {
            let actual_order = path
                .iter()
                .filter_map(|(action, _)| match action {
                    GraphAction::Finalize(owner) => Some(*owner),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let actual_checkpoints = path
                .iter()
                .filter_map(|(action, checkpoint)| {
                    matches!(action, GraphAction::Finalize(_)).then_some(*checkpoint)
                })
                .collect::<Vec<_>>();
            if *scalar_result != 0
                || &actual_order != finalizer_order
                || &actual_checkpoints != completed_checkpoints
            {
                return Err(provider_error(format!(
                    "scalar provider plan diverges from graph: order={actual_order:?} checkpoints={actual_checkpoints:?}"
                )));
            }
        }
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal,
            staged_checkpoint,
            semantic_ordinals: _,
        } => {
            let stages = path
                .iter()
                .filter_map(|(action, checkpoint)| match action {
                    GraphAction::Stage(owner) => Some((*owner, *checkpoint)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if stages.as_slice() != [(*owner_ordinal, *staged_checkpoint)] {
                return Err(provider_error(format!(
                    "owned provider plan diverges from graph: stages={stages:?}"
                )));
            }
        }
        ProviderV3Plan::GraphWitness { .. } => {}
    }
    Ok(ResolvedProviderV3Plan {
        scalar_arguments,
        outcome: requested_outcome,
        semantic_ordinals,
        execute_actions,
        checkpoints,
        terminal_checkpoint,
    })
}

fn validate_checkpoint(
    resources: u32,
    states: &[u32],
    outcome: Option<ProviderV3OutcomeClass>,
    abort_cleanup: &[u32],
    accept_cleanup: &[u32],
) -> Result<(), Diagnostic> {
    let exact = |ordinals: &[u32], wanted: Vec<u32>| {
        let mut actual = ordinals.to_vec();
        actual.sort_unstable();
        actual.dedup();
        actual == wanted && ordinals.len() == actual.len()
    };
    let non_dead = states
        .iter()
        .enumerate()
        .filter_map(|(owner, state)| (*state != 4).then_some(owner as u32))
        .collect::<Vec<_>>();
    if !exact(abort_cleanup, non_dead) {
        return Err(provider_error("checkpoint abort cleanup is not exact"));
    }
    let live = states
        .iter()
        .enumerate()
        .filter_map(|(owner, state)| (*state == 1).then_some(owner as u32))
        .collect::<Vec<_>>();
    let wanted_accept = if outcome.is_some() { live } else { Vec::new() };
    if accept_cleanup.len() > resources as usize || !exact(accept_cleanup, wanted_accept) {
        return Err(provider_error("checkpoint accept cleanup is not exact"));
    }
    let provisional = states
        .iter()
        .enumerate()
        .filter_map(|(owner, state)| (*state == 2).then_some(owner as u32))
        .collect::<Vec<_>>();
    match outcome {
        None if accept_cleanup.is_empty() && provisional.len() <= 1 => {}
        Some(ProviderV3OutcomeClass::Owned { owner_ordinal })
            if provisional.as_slice() == [owner_ordinal] => {}
        Some(ProviderV3OutcomeClass::Scalar | ProviderV3OutcomeClass::SemanticFailure)
            if provisional.is_empty() => {}
        _ => {
            return Err(provider_error(
                "checkpoint outcome and resource states diverge",
            ))
        }
    }
    Ok(())
}

fn validate_scalar_arguments(
    parameters: &[ProviderV3Parameter],
    arguments: &[ProviderV3ScalarArgument],
) -> Result<(), Diagnostic> {
    let scalar_parameters = parameters
        .iter()
        .filter(|parameter| !matches!(parameter.kind, ProviderV3ParameterKind::Owned { .. }))
        .collect::<Vec<_>>();
    if arguments.len() != scalar_parameters.len() {
        return Err(provider_error("scenario scalar argument count diverges"));
    }
    for (argument, parameter) in arguments.iter().zip(scalar_parameters) {
        if argument.parameter_index != parameter.index
            || !matches!(
                (argument.value, parameter.kind),
                (ProviderV3ScalarValue::I64(_), ProviderV3ParameterKind::I64)
                    | (
                        ProviderV3ScalarValue::Bool(_),
                        ProviderV3ParameterKind::Bool
                    )
            )
        {
            return Err(provider_error(
                "scenario scalar argument diverges from signature",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_paths(
    checkpoint: u32,
    wanted: (u32, u32),
    checkpoints: &[ProviderV3Checkpoint],
    edges: &[(u32, u32, GraphAction)],
    visited: &mut Vec<u32>,
    path: &mut Vec<(GraphAction, u32)>,
    paths: &mut Vec<Vec<(GraphAction, u32)>>,
) {
    if visited.contains(&checkpoint) || visited.len() > checkpoints.len() {
        return;
    }
    let wanted_outcome = match wanted {
        (1, 0) => Some(ProviderV3OutcomeClass::Scalar),
        (2, _) => Some(ProviderV3OutcomeClass::SemanticFailure),
        (3, owner_ordinal) => Some(ProviderV3OutcomeClass::Owned { owner_ordinal }),
        _ => None,
    };
    if checkpoints
        .iter()
        .any(|entry| entry.id == checkpoint && entry.outcome == wanted_outcome)
    {
        paths.push(path.clone());
        return;
    }
    visited.push(checkpoint);
    for (_, to, action) in edges.iter().filter(|(from, _, _)| *from == checkpoint) {
        path.push((action.clone(), *to));
        collect_paths(*to, wanted, checkpoints, edges, visited, path, paths);
        path.pop();
    }
    visited.pop();
}

fn graph_ordinals(graph: &[u8], at: &mut usize) -> Result<Vec<u32>, Diagnostic> {
    let count = descriptor_u32(graph, at)?;
    (0..count).map(|_| descriptor_u32(graph, at)).collect()
}

fn descriptor_bytes<'a>(
    bytes: &'a [u8],
    at: &mut usize,
    count: usize,
) -> Result<&'a [u8], Diagnostic> {
    let end = at
        .checked_add(count)
        .ok_or_else(|| provider_error("descriptor offset overflow"))?;
    let value = bytes
        .get(*at..end)
        .ok_or_else(|| provider_error("descriptor is truncated"))?;
    *at = end;
    Ok(value)
}

fn descriptor_u32(bytes: &[u8], at: &mut usize) -> Result<u32, Diagnostic> {
    let value = descriptor_bytes(bytes, at, 4)?;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| {
        provider_error("descriptor u32 is malformed")
    })?))
}

fn descriptor_text<'a>(bytes: &'a [u8], at: &mut usize) -> Result<&'a str, Diagnostic> {
    let count = descriptor_u32(bytes, at)? as usize;
    let value = descriptor_bytes(bytes, at, count)?;
    let text =
        std::str::from_utf8(value).map_err(|_| provider_error("descriptor text is not UTF-8"))?;
    if text.is_empty() || text.as_bytes().contains(&0) {
        return Err(provider_error("descriptor text is not canonical"));
    }
    Ok(text)
}

fn validate_plan(
    plan: &ProviderV3Plan,
    resource_count: u32,
    maximum_events: u32,
    dictionary_entries: u32,
) -> Result<(), Diagnostic> {
    let ordinals = match plan {
        ProviderV3Plan::ScalarDiscard {
            finalizer_order,
            completed_checkpoints,
            semantic_ordinals,
            ..
        } => {
            if finalizer_order.len() != resource_count as usize
                || completed_checkpoints.len() != finalizer_order.len()
                || finalizer_order
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    != (0..resource_count).collect()
                || completed_checkpoints
                    .iter()
                    .enumerate()
                    .any(|(index, checkpoint)| *checkpoint != index as u32 + 2)
            {
                return Err(provider_error("scalar cleanup plan is noncanonical"));
            }
            semantic_ordinals
        }
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal,
            staged_checkpoint,
            semantic_ordinals,
        } => {
            if resource_count != 1 || *owner_ordinal != 0 || *staged_checkpoint != 2 {
                return Err(provider_error("owned-identity plan is noncanonical"));
            }
            semantic_ordinals
        }
        ProviderV3Plan::GraphWitness {
            semantic_ordinals, ..
        } => semantic_ordinals,
    };
    if ordinals.is_empty()
        || ordinals.len() > maximum_events as usize
        || ordinals
            .iter()
            .any(|ordinal| *ordinal == 0 || *ordinal > dictionary_entries)
    {
        return Err(provider_error("semantic ordinals are outside dictionary"));
    }
    Ok(())
}

#[cfg(any(test, feature = "unstable-native-host-internal"))]
fn descriptor_has_target_and_profile(
    bytes: &[u8],
    expected_target: &str,
    expected_profile: u32,
) -> Result<bool, Diagnostic> {
    if bytes.get(..8) != Some(b"SPXNABI3") || read_u32(bytes, 12)? != HEADER_BYTES {
        return Ok(false);
    }
    let target_len = usize::try_from(read_u32(bytes, HEADER_BYTES as usize)?)
        .map_err(|_| provider_error("descriptor target length exceeds usize"))?;
    let target_start = HEADER_BYTES as usize + 4;
    let Some(target_end) = target_start.checked_add(target_len) else {
        return Ok(false);
    };
    let Some(linkage_end) = target_end.checked_add(4) else {
        return Ok(false);
    };
    if linkage_end > bytes.len() {
        return Ok(false);
    }
    Ok(
        bytes.get(target_start..target_end) == Some(expected_target.as_bytes())
            && read_u32(bytes, target_end)? == expected_profile,
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| provider_error("descriptor is truncated"))?;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| {
        provider_error("descriptor field is malformed")
    })?))
}

fn is_c_symbol(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'A'..=b'Z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn emit_c_array(output: &mut String, name: &str, bytes: &[u8]) {
    writeln!(output, "static const uint8_t {name}[{}]={{", bytes.len()).expect("string write");
    for chunk in bytes.chunks(16) {
        for byte in chunk {
            write!(output, "UINT8_C({byte}),").expect("string write");
        }
        output.push('\n');
    }
    output.push_str("};\n");
}

fn provider_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-I107",
        format!("native callable provider v3: {}", message.into()),
    )
}

const SHA256_C: &str = r#"
struct spx_v3_sha { uint32_t h[8]; uint64_t bits; uint8_t block[64]; uint32_t used; };
static uint32_t spx_v3_rotr(uint32_t x,uint32_t n){return (x>>n)|(x<<(32-n));}
static uint32_t spx_v3_be32(const uint8_t *p){return ((uint32_t)p[0]<<24)|((uint32_t)p[1]<<16)|((uint32_t)p[2]<<8)|(uint32_t)p[3];}
static void spx_v3_sha_block(struct spx_v3_sha *s,const uint8_t *p){
 static const uint32_t k[64]={0x428a2f98U,0x71374491U,0xb5c0fbcfU,0xe9b5dba5U,0x3956c25bU,0x59f111f1U,0x923f82a4U,0xab1c5ed5U,0xd807aa98U,0x12835b01U,0x243185beU,0x550c7dc3U,0x72be5d74U,0x80deb1feU,0x9bdc06a7U,0xc19bf174U,0xe49b69c1U,0xefbe4786U,0x0fc19dc6U,0x240ca1ccU,0x2de92c6fU,0x4a7484aaU,0x5cb0a9dcU,0x76f988daU,0x983e5152U,0xa831c66dU,0xb00327c8U,0xbf597fc7U,0xc6e00bf3U,0xd5a79147U,0x06ca6351U,0x14292967U,0x27b70a85U,0x2e1b2138U,0x4d2c6dfcU,0x53380d13U,0x650a7354U,0x766a0abbU,0x81c2c92eU,0x92722c85U,0xa2bfe8a1U,0xa81a664bU,0xc24b8b70U,0xc76c51a3U,0xd192e819U,0xd6990624U,0xf40e3585U,0x106aa070U,0x19a4c116U,0x1e376c08U,0x2748774cU,0x34b0bcb5U,0x391c0cb3U,0x4ed8aa4aU,0x5b9cca4fU,0x682e6ff3U,0x748f82eeU,0x78a5636fU,0x84c87814U,0x8cc70208U,0x90befffaU,0xa4506cebU,0xbef9a3f7U,0xc67178f2U};
 uint32_t w[64],a,b,c,d,e,f,g,h; for(uint32_t i=0;i<16;i++)w[i]=spx_v3_be32(p+4*i); for(uint32_t i=16;i<64;i++){uint32_t x=w[i-15],y=w[i-2];w[i]=(spx_v3_rotr(y,17)^spx_v3_rotr(y,19)^(y>>10))+w[i-7]+(spx_v3_rotr(x,7)^spx_v3_rotr(x,18)^(x>>3))+w[i-16];}
 a=s->h[0];b=s->h[1];c=s->h[2];d=s->h[3];e=s->h[4];f=s->h[5];g=s->h[6];h=s->h[7]; for(uint32_t i=0;i<64;i++){uint32_t t1=h+(spx_v3_rotr(e,6)^spx_v3_rotr(e,11)^spx_v3_rotr(e,25))+((e&f)^((~e)&g))+k[i]+w[i];uint32_t t2=(spx_v3_rotr(a,2)^spx_v3_rotr(a,13)^spx_v3_rotr(a,22))+((a&b)^(a&c)^(b&c));h=g;g=f;f=e;e=d+t1;d=c;c=b;b=a;a=t1+t2;} s->h[0]+=a;s->h[1]+=b;s->h[2]+=c;s->h[3]+=d;s->h[4]+=e;s->h[5]+=f;s->h[6]+=g;s->h[7]+=h;
}
static void spx_v3_sha_init(struct spx_v3_sha *s){static const uint32_t h[8]={0x6a09e667U,0xbb67ae85U,0x3c6ef372U,0xa54ff53aU,0x510e527fU,0x9b05688cU,0x1f83d9abU,0x5be0cd19U};memcpy(s->h,h,sizeof(h));s->bits=0;s->used=0;}
static void spx_v3_sha_update(struct spx_v3_sha *s,const uint8_t *p,uint32_t n){s->bits+=(uint64_t)n*UINT64_C(8);while(n){uint32_t take=UINT32_C(64)-s->used;if(take>n)take=n;memcpy(s->block+s->used,p,take);s->used+=take;p+=take;n-=take;if(s->used==UINT32_C(64)){spx_v3_sha_block(s,s->block);s->used=0;}}}
static void spx_v3_sha_final(struct spx_v3_sha *s,uint8_t out[32]){uint64_t bits=s->bits;s->block[s->used++]=UINT8_C(0x80);if(s->used>56){memset(s->block+s->used,0,64-s->used);spx_v3_sha_block(s,s->block);s->used=0;}memset(s->block+s->used,0,56-s->used);for(uint32_t i=0;i<8;i++)s->block[56+i]=(uint8_t)(bits>>(56-8*i));spx_v3_sha_block(s,s->block);for(uint32_t i=0;i<8;i++){out[4*i]=(uint8_t)(s->h[i]>>24);out[4*i+1]=(uint8_t)(s->h[i]>>16);out[4*i+2]=(uint8_t)(s->h[i]>>8);out[4*i+3]=(uint8_t)s->h[i];}}
static void spx_v3_hash_field(struct spx_v3_sha *s,const uint8_t *p,uint32_t n){uint8_t len[8];for(uint32_t i=0;i<8;i++)len[i]=(uint8_t)((uint64_t)n>>(56-8*i));spx_v3_sha_update(s,len,8);spx_v3_sha_update(s,p,n);}
static void spx_v3_framed_digest(const uint8_t *domain,uint32_t domain_len,const uint8_t *p,uint32_t n,uint8_t out[32]){struct spx_v3_sha s;spx_v3_sha_init(&s);spx_v3_sha_update(&s,domain,domain_len);spx_v3_hash_field(&s,p,n);spx_v3_sha_final(&s,out);}
"#;

const HELPERS_C: &str = r#"
static uint32_t spx_v3_load_u32(const uint8_t *p){uint32_t v;memcpy(&v,p,4);return v;}
static uint64_t spx_v3_load_u64(const uint8_t *p){uint64_t v;memcpy(&v,p,8);return v;}
static void spx_v3_store_u32(uint8_t *p,uint32_t v){memcpy(p,&v,4);}
static void spx_v3_store_u64(uint8_t *p,uint64_t v){memcpy(p,&v,8);}
static bool spx_v3_zero(const uint8_t *p,uint32_t n){uint8_t v=0;for(uint32_t i=0;i<n;i++)v=(uint8_t)(v|p[i]);return v==0;}
static bool spx_v3_disjoint(const uint8_t *a,uint32_t an,const uint8_t *b,uint32_t bn){uintptr_t x=(uintptr_t)a,y=(uintptr_t)b;if(x>UINTPTR_MAX-an||y>UINTPTR_MAX-bn)return false;return x+an<=y||y+bn<=x;}
static uint32_t spx_v3_cell(uint32_t owner){return UINT32_C(324)+UINT32_C(12)*owner;}
static uint32_t spx_v3_action_digest_offset(void){return UINT32_C(324)+UINT32_C(12)*SPX_V3_RESOURCE_COUNT;}
static uint32_t spx_v3_frame_digest_offset(void){return spx_v3_action_digest_offset()+UINT32_C(32);}
static void spx_v3_refresh_frame(uint8_t *frame){uint8_t d[32];spx_v3_framed_digest(spx_v3_frame_domain,(uint32_t)sizeof(spx_v3_frame_domain),frame,SPX_V3_FRAME_BYTES-UINT32_C(32),d);memcpy(frame+spx_v3_frame_digest_offset(),d,32);}
static bool spx_v3_valid_frame_digest(const uint8_t *frame){uint8_t d[32];spx_v3_framed_digest(spx_v3_frame_domain,(uint32_t)sizeof(spx_v3_frame_domain),frame,SPX_V3_FRAME_BYTES-UINT32_C(32),d);return memcmp(frame+spx_v3_frame_digest_offset(),d,32)==0;}
static void spx_v3_request_digest(const uint8_t *request,uint8_t out[32]){spx_v3_framed_digest(spx_v3_request_domain,(uint32_t)sizeof(spx_v3_request_domain),request,SPX_V3_REQUEST_BYTES,out);}
static void spx_v3_response_digest(uint32_t code,const uint8_t *response,uint8_t out[32]){struct spx_v3_sha s;uint8_t le[4];spx_v3_store_u32(le,code);spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_response_domain,(uint32_t)sizeof(spx_v3_response_domain));spx_v3_sha_update(&s,le,4);spx_v3_hash_field(&s,response,SPX_V3_RESPONSE_BYTES);spx_v3_sha_final(&s,out);}
static uint32_t spx_v3_maybe_physical_failure(uint8_t *frame,const uint8_t *response,uint32_t response_len,uint32_t checkpoint){uint32_t code=SPX_V3_FAULT_PHYSICAL_CODE;if(checkpoint!=SPX_V3_FAULT_PHYSICAL_CHECKPOINT)return UINT32_C(0);if(code==0||response_len!=SPX_V3_RESPONSE_BYTES||spx_v3_load_u32(frame+268)!=checkpoint||spx_v3_load_u32(frame+316)!=0)return UINT32_C(2);spx_v3_response_digest(code,response,frame+196);memset(frame+228,0,32);spx_v3_store_u32(frame+260,UINT32_C(2));spx_v3_store_u32(frame+264,code);spx_v3_refresh_frame(frame);return code;}
static void spx_v3_semantic_digest(const uint8_t *response,uint8_t out[32]){struct spx_v3_sha s;uint8_t le[8],outcome;uint32_t count=spx_v3_load_u32(response+152),tag=spx_v3_load_u32(response+136);spx_v3_store_u64(le,(uint64_t)count);spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_trace_domain,(uint32_t)sizeof(spx_v3_trace_domain));spx_v3_sha_update(&s,spx_v3_trace_path_certificate,32);spx_v3_sha_update(&s,le,8);for(uint32_t i=0;i<count;i++)spx_v3_sha_update(&s,response+156+4*i,4);outcome=(uint8_t)(tag==1?1:(tag==3?2:3));spx_v3_sha_update(&s,&outcome,1);if(outcome==3){spx_v3_store_u32(le,spx_v3_load_u32(response+140));spx_v3_sha_update(&s,le,4);}spx_v3_sha_final(&s,out);}
static void spx_v3_decision_digest(const uint8_t *decision,uint8_t out[32]){spx_v3_framed_digest(spx_v3_decision_domain,(uint32_t)sizeof(spx_v3_decision_domain),decision,SPX_V3_DECISION_BYTES,out);}
static bool spx_v3_common(const uint8_t *p,uint32_t n,const char magic[8]){return p!=NULL&&n>=20&&memcmp(p,magic,8)==0&&spx_v3_load_u32(p+8)==3&&spx_v3_load_u32(p+12)==20&&spx_v3_load_u32(p+16)==n;}
static bool spx_v3_validate_execute_inputs(const uint8_t *request,uint32_t request_len,uint8_t *frame,uint32_t frame_len,uint8_t *response,uint32_t response_len,uint64_t payloads[SPX_V3_RESOURCE_COUNT],uint64_t arguments[SPX_V3_PARAMETER_COUNT],uint8_t request_hash[32]){
 if(response==NULL||response_len!=SPX_V3_RESPONSE_BYTES||!spx_v3_common(request,request_len,"SPXNRQ03")||request_len!=SPX_V3_REQUEST_BYTES||!spx_v3_common(frame,frame_len,"SPXNFR03")||frame_len!=SPX_V3_FRAME_BYTES||!spx_v3_valid_frame_digest(frame)||!spx_v3_disjoint(request,request_len,frame,frame_len)||!spx_v3_disjoint(request,request_len,response,response_len)||!spx_v3_disjoint(frame,frame_len,response,response_len))return false;
 if(memcmp(request+20,spx_v3_call_contract,32)||memcmp(frame+20,spx_v3_call_contract,32)||memcmp(frame+52,spx_v3_recovery_contract,32)||memcmp(frame+84,spx_v3_settlement_graph,32)||memcmp(request+52,frame+116,48)||spx_v3_load_u64(request+52)==0||spx_v3_load_u64(request+60)==0||spx_v3_zero(request+68,32)||spx_v3_load_u32(request+100)!=SPX_V3_PARAMETER_COUNT||!spx_v3_zero(frame+196,64)||spx_v3_load_u32(frame+260)!=1||spx_v3_load_u32(frame+264)!=0||spx_v3_load_u32(frame+268)!=1||spx_v3_load_u32(frame+272)!=1||!spx_v3_zero(frame+276,32)||spx_v3_load_u32(frame+308)!=0||spx_v3_load_u32(frame+312)!=0||spx_v3_load_u32(frame+316)!=0||spx_v3_load_u32(frame+320)!=SPX_V3_RESOURCE_COUNT||!spx_v3_zero(frame+spx_v3_action_digest_offset(),32))return false;
 spx_v3_request_digest(request,request_hash);if(memcmp(frame+164,request_hash,32)!=0)return false;
 uint32_t q=104,owners=0;for(uint32_t i=0;i<SPX_V3_PARAMETER_COUNT;i++){uint32_t kind=spx_v3_parameter_kind[i],bytes=kind==1?16:(kind==2?12:20);if(q>request_len||bytes>request_len-q||spx_v3_load_u32(request+q)!=(kind==3?2:1)||spx_v3_load_u32(request+q+4)!=i)return false;if(kind==1){arguments[i]=spx_v3_load_u64(request+q+8);}else if(kind==2){uint32_t value=spx_v3_load_u32(request+q+8);if(value>1)return false;arguments[i]=value;}else{uint32_t owner=spx_v3_load_u32(request+q+8),c;if(owner!=spx_v3_parameter_owner[i]||owner!=owners||owner>=SPX_V3_RESOURCE_COUNT)return false;c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+c)!=1)return false;payloads[owner]=spx_v3_load_u64(request+q+12);arguments[i]=payloads[owner];if(spx_v3_load_u64(frame+c+4)!=payloads[owner])return false;owners++;}q+=bytes;}return q==request_len&&owners==SPX_V3_RESOURCE_COUNT;
}
static bool spx_v3_begin_finalizer(uint8_t *frame,uint32_t owner){uint32_t c=spx_v3_cell(owner),state;if(owner>=SPX_V3_RESOURCE_COUNT)return false;state=spx_v3_load_u32(frame+c);if(state!=1&&state!=2)return false;spx_v3_store_u32(frame+c,3);spx_v3_store_u32(frame+316,1);spx_v3_refresh_frame(frame);return true;}
static bool spx_v3_complete_finalizer(uint8_t *frame,uint32_t owner,uint32_t checkpoint){uint32_t c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+c)!=3)return false;spx_v3_store_u32(frame+c,4);spx_v3_store_u32(frame+316,0);spx_v3_store_u32(frame+268,checkpoint);spx_v3_refresh_frame(frame);return true;}
static bool spx_v3_stage_owned(uint8_t *frame,uint32_t owner,uint32_t checkpoint){uint32_t c=spx_v3_cell(owner);if(owner>=SPX_V3_RESOURCE_COUNT||spx_v3_load_u32(frame+c)!=1)return false;spx_v3_store_u32(frame+c,2);spx_v3_store_u32(frame+268,checkpoint);spx_v3_refresh_frame(frame);return true;}
static void spx_v3_action_seed_digest(const uint8_t decision[32],uint64_t count,uint8_t out[32]){struct spx_v3_sha s;uint8_t le[8];spx_v3_store_u64(le,count);spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_action_seed_domain,(uint32_t)sizeof(spx_v3_action_seed_domain));spx_v3_sha_update(&s,decision,32);spx_v3_sha_update(&s,le,8);spx_v3_sha_final(&s,out);}
static void spx_v3_action_seed(uint8_t *frame,const uint8_t decision[32],uint64_t count){spx_v3_action_seed_digest(decision,count,frame+spx_v3_action_digest_offset());}
static bool spx_v3_valid_action_seed(const uint8_t *frame,const uint8_t decision[32],uint64_t count){uint8_t expected[32];spx_v3_action_seed_digest(decision,count,expected);return memcmp(frame+spx_v3_action_digest_offset(),expected,32)==0;}
static bool spx_v3_lock_decision(uint8_t *frame,const uint8_t decision[32],uint64_t actions){memcpy(frame+276,decision,32);spx_v3_action_seed(frame,decision,actions);spx_v3_store_u32(frame+272,2);spx_v3_refresh_frame(frame);return true;}
static void spx_v3_action_step(uint8_t *frame,uint32_t action,uint32_t boundary,uint32_t owner,uint32_t before,uint32_t after){uint8_t record[SPX_V3_ACTION_BYTES],next[32],le[8];uint64_t j=spx_v3_load_u32(frame+312);spx_v3_store_u64(le,j);memset(record,0,sizeof(record));memcpy(record,"SPXNAC03",8);spx_v3_store_u32(record+8,3);spx_v3_store_u32(record+12,20);spx_v3_store_u32(record+16,SPX_V3_ACTION_BYTES);memcpy(record+20,frame+20,144);spx_v3_store_u32(record+164,action);spx_v3_store_u32(record+168,boundary);spx_v3_store_u32(record+172,owner);spx_v3_store_u64(record+176,spx_v3_load_u64(frame+spx_v3_cell(owner)+4));spx_v3_store_u32(record+184,before);spx_v3_store_u32(record+188,after);spx_v3_store_u32(record+192,spx_v3_load_u32(frame+268));struct spx_v3_sha s;spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_action_step_domain,(uint32_t)sizeof(spx_v3_action_step_domain));spx_v3_sha_update(&s,frame+spx_v3_action_digest_offset(),32);spx_v3_sha_update(&s,le,8);spx_v3_hash_field(&s,record,SPX_V3_ACTION_BYTES);spx_v3_sha_final(&s,next);memcpy(frame+spx_v3_action_digest_offset(),next,32);spx_v3_store_u32(frame+312,(uint32_t)(j+1));}
static bool spx_v3_publish(uint8_t *frame,uint32_t action,uint32_t owner){uint32_t c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+308)!=action||spx_v3_load_u32(frame+c)!=2)return false;spx_v3_store_u32(frame+272,3);spx_v3_action_step(frame,action,3,owner,2,5);spx_v3_store_u32(frame+c,5);spx_v3_store_u32(frame+308,action+1);spx_v3_refresh_frame(frame);return true;}
static bool spx_v3_settlement_finalize(uint8_t *frame,uint32_t action,uint32_t owner){uint32_t c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+308)!=action||(spx_v3_load_u32(frame+c)!=1&&spx_v3_load_u32(frame+c)!=2))return false;uint32_t before=spx_v3_load_u32(frame+c);spx_v3_store_u32(frame+272,3);spx_v3_store_u32(frame+c,3);spx_v3_store_u32(frame+316,1);spx_v3_action_step(frame,action,1,owner,before,3);spx_v3_refresh_frame(frame);if(action==SPX_V3_FAULT_FINALIZER_ACTION&&SPX_V3_FAULT_FINALIZER_BOUNDARY==UINT32_C(1))return false;spx_v3_generated_finalize(owner,spx_v3_load_u64(frame+c+4));if(action==SPX_V3_FAULT_FINALIZER_ACTION&&SPX_V3_FAULT_FINALIZER_BOUNDARY==UINT32_C(2))return false;spx_v3_action_step(frame,action,2,owner,3,4);spx_v3_store_u32(frame+c,4);spx_v3_store_u32(frame+316,0);spx_v3_store_u32(frame+308,action+1);spx_v3_refresh_frame(frame);if(action==SPX_V3_FAULT_FINALIZER_ACTION&&SPX_V3_FAULT_FINALIZER_BOUNDARY==UINT32_C(3))return false;return true;}
static bool spx_v3_validate_settle_inputs(uint8_t *frame,uint32_t frame_len,const uint8_t *decision,uint32_t decision_len,uint8_t *candidate,uint32_t candidate_len,uint8_t decision_hash[32],uint32_t *tag,uint32_t *detail){if(candidate==NULL||candidate_len!=SPX_V3_CANDIDATE_BYTES||!spx_v3_common(frame,frame_len,"SPXNFR03")||frame_len!=SPX_V3_FRAME_BYTES||!spx_v3_valid_frame_digest(frame)||!spx_v3_common(decision,decision_len,"SPXNDC03")||decision_len!=SPX_V3_DECISION_BYTES||!spx_v3_disjoint(frame,frame_len,decision,decision_len)||!spx_v3_disjoint(frame,frame_len,candidate,candidate_len)||!spx_v3_disjoint(decision,decision_len,candidate,candidate_len)||memcmp(frame+20,decision+20,144)!=0||spx_v3_load_u32(frame+316)!=0)return false;*tag=spx_v3_load_u32(decision+164);*detail=spx_v3_load_u32(decision+168);if(*tag<1||*tag>7||((*tag==1||*tag==2||*tag==5||*tag==6||*tag==7)&&*detail!=0)||(*tag==4&&*detail==0))return false;spx_v3_decision_digest(decision,decision_hash);return true;}
static uint32_t spx_v3_emit_candidate(uint8_t *frame,uint32_t tag,uint32_t detail,uint8_t *candidate){uint32_t outcome=tag==1?1:(tag==2?2:(tag==3?3:4));memset(candidate,0,SPX_V3_CANDIDATE_BYTES);memcpy(candidate,"SPXNCR03",8);spx_v3_store_u32(candidate+8,3);spx_v3_store_u32(candidate+12,20);spx_v3_store_u32(candidate+16,SPX_V3_CANDIDATE_BYTES);memcpy(candidate+20,frame+20,144);memcpy(candidate+164,frame+164,64);if(outcome==4)memset(candidate+228,0,32);else memcpy(candidate+228,frame+228,32);memcpy(candidate+260,frame+spx_v3_frame_digest_offset(),32);memcpy(candidate+292,frame+276,32);memcpy(candidate+324,frame+spx_v3_action_digest_offset(),32);spx_v3_store_u32(candidate+356,outcome);spx_v3_store_u32(candidate+360,outcome==3?detail:0);spx_v3_store_u32(candidate+364,0);spx_v3_store_u32(candidate+368,SPX_V3_RESOURCE_COUNT);for(uint32_t i=0;i<SPX_V3_RESOURCE_COUNT;i++){uint32_t c=spx_v3_cell(i),q=372+12*i,s=spx_v3_load_u32(frame+c);if(s!=4&&s!=5)return UINT32_C(3);spx_v3_store_u32(candidate+q,s==4?1:2);spx_v3_store_u64(candidate+q+4,spx_v3_load_u64(frame+c+4));}return UINT32_C(0);}
static uint32_t spx_v3_emit_candidate_with_fault(uint8_t *frame,uint32_t tag,uint32_t detail,uint8_t *candidate){uint32_t result=spx_v3_emit_candidate(frame,tag,detail,candidate);if(result==0&&SPX_V3_FAULT_CANDIDATE_OFFSET!=UINT32_MAX)candidate[SPX_V3_FAULT_CANDIDATE_OFFSET]^=UINT8_C(1);return result;}
"#;

#[cfg(test)]
#[path = "native_callable_provider_v3/tests.rs"]
mod tests;
