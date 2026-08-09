//! Private C11 provider for the first physical callable-v3 proof fixtures.
//!
//! This emitter is deliberately unreachable from public compilation. It emits
//! one exact descriptor getter plus synchronous six-argument `execute` and
//! `settle` entry points for the current direct-trivial owned slice.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "callable-v3 provider remains behind SPX-B104")
)]

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};

use super::native_callable_abi_v3::NativeCallableV3Descriptor;
use super::native_callable_wire_v3::{
    candidate_receipt_capacity, execute_response_capacity, frame_capacity, ACTION_EVIDENCE_BYTES,
    DECISION_BYTES, HEADER_BYTES, HOST_RECEIPT_BYTES, VERSION,
};

const MAX_SYMBOL_BYTES: usize = 1024;
const MAX_EVENTS: u32 = 256;
const MAX_RESOURCES: u32 = 4_096;

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
    plan: ProviderV3Plan,
}

impl NativeCallableProviderV3Spec {
    pub(super) fn new(
        descriptor: NativeCallableV3Descriptor,
        plan: ProviderV3Plan,
    ) -> Result<Self, Diagnostic> {
        validate_descriptor_exact(&descriptor, &plan)?;
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
        let expected_request = 104_u32
            .checked_add(
                20_u32
                    .checked_mul(resource_count)
                    .ok_or_else(|| provider_error("request capacity overflow"))?,
            )
            .ok_or_else(|| provider_error("request capacity overflow"))?;
        if request_bytes != expected_request {
            return Err(provider_error(
                "request capacity does not match owned signature",
            ));
        }
        let response_bytes = execute_response_capacity(maximum_events)
            .map_err(|_| provider_error("response capacity overflow"))?;
        let frame_bytes = frame_capacity(resource_count)
            .map_err(|_| provider_error("frame capacity overflow"))?;
        let candidate_bytes = candidate_receipt_capacity(resource_count)
            .map_err(|_| provider_error("candidate capacity overflow"))?;
        validate_plan(&plan, resource_count, maximum_events, dictionary_entries)?;
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
            plan,
        })
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

fn emit_prelude(
    output: &mut String,
    spec: &NativeCallableProviderV3Spec,
) -> Result<(), Diagnostic> {
    output.push_str("/* semaprax.native-callable-provider.v3; private; SPX-B104 closed */\n#include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\n#include <string.h>\n");
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
        ("SPX_V3_MAX_EVENTS", spec.maximum_events),
        ("SPX_V3_DICTIONARY_ENTRIES", spec.dictionary_entries),
    ] {
        writeln!(output, "#define {name} UINT32_C({value})").expect("string write");
    }
    writeln!(
        output,
        "#define SPX_V3_OWNED_IDENTITY {}",
        u32::from(matches!(spec.plan, ProviderV3Plan::OwnedIdentity { .. }))
    )
    .expect("string write");
    emit_c_array(output, "spx_v3_descriptor", &spec.descriptor);
    emit_c_array(output, "spx_v3_call_contract", &spec.call_contract);
    emit_c_array(output, "spx_v3_recovery_contract", &spec.recovery_contract);
    emit_c_array(output, "spx_v3_settlement_graph", &spec.settlement_graph);
    emit_c_array(
        output,
        "spx_v3_trace_path_certificate",
        &spec.trace_path_certificate,
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
    output.push_str("  uint64_t payloads[SPX_V3_RESOURCE_COUNT]; uint8_t request_hash[32];\n  if (!spx_v3_validate_execute_inputs(request,request_len,frame,frame_len,response,response_len,payloads,request_hash)) return UINT32_C(1);\n  memset(response,0,response_len);\n");
    match &spec.plan {
        ProviderV3Plan::ScalarDiscard {
            scalar_result,
            finalizer_order,
            completed_checkpoints,
            semantic_ordinals,
        } => {
            for (owner, checkpoint) in finalizer_order.iter().zip(completed_checkpoints) {
                writeln!(output, "  if (!spx_v3_begin_finalizer(frame,UINT32_C({owner}))) return UINT32_C(2);\n  spx_v3_generated_finalize(UINT32_C({owner}),payloads[{owner}]);\n  if (!spx_v3_complete_finalizer(frame,UINT32_C({owner}),UINT32_C({checkpoint}))) return UINT32_C(2);").expect("string write");
            }
            emit_response_header(
                output,
                semantic_ordinals.len(),
                "1",
                "0",
                &scalar_result.to_string(),
            );
            for ordinal in semantic_ordinals {
                writeln!(output, "  spx_v3_store_u32(response+spx_write,UINT32_C({ordinal})); spx_write+=UINT32_C(4);").expect("string write");
            }
        }
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal,
            staged_checkpoint,
            semantic_ordinals,
        } => {
            writeln!(output, "  if (!spx_v3_stage_owned(frame,UINT32_C({owner_ordinal}),UINT32_C({staged_checkpoint}))) return UINT32_C(2);").expect("string write");
            emit_response_header(
                output,
                semantic_ordinals.len(),
                "3",
                &owner_ordinal.to_string(),
                &format!("payloads[{owner_ordinal}]"),
            );
            for ordinal in semantic_ordinals {
                writeln!(output, "  spx_v3_store_u32(response+spx_write,UINT32_C({ordinal})); spx_write+=UINT32_C(4);").expect("string write");
            }
        }
    }
    output.push_str("  if (spx_write != spx_total) return UINT32_C(2);\n  spx_v3_response_digest(UINT32_C(0),response,frame+196); spx_v3_semantic_digest(response,frame+228); spx_v3_store_u32(frame+260,UINT32_C(2)); spx_v3_store_u32(frame+264,UINT32_C(0)); spx_v3_refresh_frame(frame);\n  return UINT32_C(0);\n}\n");
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
    writeln!(output, "SPX_V3_API uint32_t SPX_V3_CALL {}(uint8_t *frame,uint32_t frame_len,const uint8_t *decision,uint32_t decision_len,uint8_t *candidate,uint32_t candidate_len) {{", spec.settle_symbol).expect("string write");
    output.push_str("  uint8_t decision_hash[32]; uint32_t decision_tag,decision_detail,phase;\n  if (!spx_v3_validate_settle_inputs(frame,frame_len,decision,decision_len,candidate,candidate_len,decision_hash,&decision_tag,&decision_detail)) return UINT32_C(1);\n  phase=spx_v3_load_u32(frame+272);\n  if (phase==UINT32_C(4)) { if (memcmp(frame+276,decision_hash,32)!=0) return UINT32_C(3); return spx_v3_emit_candidate(frame,decision_tag,decision_detail,candidate); }\n  if (phase!=UINT32_C(1)&&phase!=UINT32_C(2)&&phase!=UINT32_C(3)) return UINT32_C(3);\n  if (decision_tag<UINT32_C(4) && (spx_v3_load_u32(frame+260)!=UINT32_C(2)||spx_v3_load_u32(frame+264)!=UINT32_C(0)||spx_v3_zero(frame+196,32)||spx_v3_zero(frame+228,32))) return UINT32_C(3);\n  if (decision_tag==UINT32_C(4) && (spx_v3_load_u32(frame+260)!=UINT32_C(2)||spx_v3_load_u32(frame+264)!=decision_detail)) return UINT32_C(3);\n  if ((decision_tag==UINT32_C(5)||decision_tag==UINT32_C(6)) && (spx_v3_load_u32(frame+260)!=UINT32_C(2)||spx_v3_load_u32(frame+264)!=UINT32_C(0))) return UINT32_C(3);\n");
    match spec.plan {
        ProviderV3Plan::ScalarDiscard {
            ref finalizer_order,
            ..
        } => {
            writeln!(output, "  static const uint32_t spx_order[{}]={{ {} }}; uint32_t completed,expected,next; uint8_t replay[SPX_V3_FRAME_BYTES];", finalizer_order.len(), finalizer_order.iter().map(|owner| format!("UINT32_C({owner})")).collect::<Vec<_>>().join(",")).expect("string write");
            writeln!(output, "  if (decision_tag!=UINT32_C(1) && decision_tag<UINT32_C(4)) return UINT32_C(3);\n  if (spx_v3_load_u32(frame+268)<1||spx_v3_load_u32(frame+268)>UINT32_C({})) return UINT32_C(3); completed=spx_v3_load_u32(frame+268)-1;\n  if (decision_tag==UINT32_C(1)&&completed!=UINT32_C({})) return UINT32_C(3); expected=decision_tag==UINT32_C(1)?0:UINT32_C({})-completed; next=spx_v3_load_u32(frame+308); if(next>expected)return UINT32_C(3);", finalizer_order.len()+1, finalizer_order.len(), finalizer_order.len()).expect("string write");
            output.push_str("  for(uint32_t i=0;i<SPX_V3_RESOURCE_COUNT;i++){uint32_t state=spx_v3_load_u32(frame+spx_v3_cell(spx_order[i]));uint32_t want=i<completed+next?UINT32_C(4):UINT32_C(1);if(state!=want)return UINT32_C(3);}\n  if (phase==UINT32_C(1)) { if(next!=0||spx_v3_load_u32(frame+312)!=0)return UINT32_C(3); if (decision_tag>=UINT32_C(4)) memset(frame+228,0,32); if (!spx_v3_lock_decision(frame,decision_hash,(uint64_t)expected)) return UINT32_C(3); } else { if (memcmp(frame+276,decision_hash,32)!=0) return UINT32_C(3); if(phase==UINT32_C(2)){if(next!=0||spx_v3_load_u32(frame+312)!=0||!spx_v3_valid_action_seed(frame,decision_hash,(uint64_t)expected))return UINT32_C(3);}else{if(spx_v3_load_u32(frame+312)!=2*next)return UINT32_C(3);memcpy(replay,frame,SPX_V3_FRAME_BYTES);spx_v3_action_seed(replay,decision_hash,(uint64_t)expected);spx_v3_store_u32(replay+312,0);for(uint32_t i=0;i<next;i++){uint32_t owner=spx_order[completed+i];spx_v3_action_step(replay,i,1,owner,1,3);spx_v3_action_step(replay,i,2,owner,3,4);}if(memcmp(replay+spx_v3_action_digest_offset(),frame+spx_v3_action_digest_offset(),32))return UINT32_C(3);}}\n  for(uint32_t i=next;i<expected;i++){if(!spx_v3_settlement_finalize(frame,i,spx_order[completed+i]))return UINT32_C(3);}\n");
        }
        ProviderV3Plan::OwnedIdentity { owner_ordinal, .. } => {
            writeln!(output, "  uint32_t next=spx_v3_load_u32(frame+308),before;uint8_t replay[SPX_V3_FRAME_BYTES];\n  if (decision_tag<UINT32_C(4) && (decision_tag!=UINT32_C(3)||decision_detail!=UINT32_C({owner_ordinal}))) return UINT32_C(3); if(next>1)return UINT32_C(3); before=spx_v3_load_u32(frame+268)==1?UINT32_C(1):(spx_v3_load_u32(frame+268)==2?UINT32_C(2):0);if(before==0||(decision_tag==UINT32_C(3)&&before!=UINT32_C(2)))return UINT32_C(3);\n  if(next==0&&spx_v3_load_u32(frame+spx_v3_cell(UINT32_C({owner_ordinal})))!=before)return UINT32_C(3);if(next==1&&spx_v3_load_u32(frame+spx_v3_cell(UINT32_C({owner_ordinal})) )!=(decision_tag==UINT32_C(3)?UINT32_C(5):UINT32_C(4)))return UINT32_C(3);\n  if (phase==UINT32_C(1)) {{if(next!=0||spx_v3_load_u32(frame+312)!=0)return UINT32_C(3); if (decision_tag>=UINT32_C(4)) memset(frame+228,0,32); if (!spx_v3_lock_decision(frame,decision_hash,UINT64_C(1))) return UINT32_C(3); }} else {{if (memcmp(frame+276,decision_hash,32)!=0) return UINT32_C(3);if(phase==UINT32_C(2)){{if(next!=0||spx_v3_load_u32(frame+312)!=0||!spx_v3_valid_action_seed(frame,decision_hash,UINT64_C(1)))return UINT32_C(3);}}else{{uint32_t records=decision_tag==UINT32_C(3)?1:2;if(next!=1||spx_v3_load_u32(frame+312)!=records)return UINT32_C(3);memcpy(replay,frame,SPX_V3_FRAME_BYTES);spx_v3_action_seed(replay,decision_hash,UINT64_C(1));spx_v3_store_u32(replay+312,0);if(decision_tag==UINT32_C(3))spx_v3_action_step(replay,0,3,UINT32_C({owner_ordinal}),2,5);else{{spx_v3_action_step(replay,0,1,UINT32_C({owner_ordinal}),before,3);spx_v3_action_step(replay,0,2,UINT32_C({owner_ordinal}),3,4);}}if(memcmp(replay+spx_v3_action_digest_offset(),frame+spx_v3_action_digest_offset(),32))return UINT32_C(3);}}}}\n  if(next==0){{if (decision_tag==UINT32_C(3)) {{ if (!spx_v3_publish(frame,UINT32_C({owner_ordinal}))) return UINT32_C(3); }} else {{ if (!spx_v3_settlement_finalize(frame,0,UINT32_C({owner_ordinal}))) return UINT32_C(3); }} }}").expect("string write");
        }
    }
    output.push_str("  spx_v3_store_u32(frame+272,UINT32_C(4)); spx_v3_refresh_frame(frame);\n  return spx_v3_emit_candidate(frame,decision_tag,decision_detail,candidate);\n}\n");
}

fn validate_descriptor_exact(
    descriptor: &NativeCallableV3Descriptor,
    plan: &ProviderV3Plan,
) -> Result<(), Diagnostic> {
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
    let parameters = descriptor_u32(bytes, &mut at)?;
    if parameters != descriptor.resource_count {
        return Err(provider_error(
            "provider tranche requires all-owned parameters",
        ));
    }
    for expected in 0..parameters {
        if descriptor_u32(bytes, &mut at)? != 2 || descriptor_u32(bytes, &mut at)? != expected {
            return Err(provider_error("descriptor owned parameter is noncanonical"));
        }
        let _value = descriptor_text(bytes, &mut at)?;
        if descriptor_u32(bytes, &mut at)? != expected {
            return Err(provider_error("descriptor owner ordinal is noncanonical"));
        }
        let _resource = descriptor_text(bytes, &mut at)?;
        let _lifecycle = descriptor_text(bytes, &mut at)?;
        if descriptor_u32(bytes, &mut at)? != 1 {
            return Err(provider_error(
                "descriptor payload wire kind is unsupported",
            ));
        }
    }
    match plan {
        ProviderV3Plan::ScalarDiscard { .. } => {
            if descriptor_u32(bytes, &mut at)? != 1 {
                return Err(provider_error("descriptor result is not scalar i64"));
            }
        }
        ProviderV3Plan::OwnedIdentity { owner_ordinal, .. } => {
            if descriptor_u32(bytes, &mut at)? != 2
                || descriptor_u32(bytes, &mut at)? != *owner_ordinal
            {
                return Err(provider_error("descriptor owned result diverges"));
            }
            let _value = descriptor_text(bytes, &mut at)?;
            if descriptor_u32(bytes, &mut at)? != *owner_ordinal {
                return Err(provider_error("descriptor owned-result ordinal diverges"));
            }
        }
    }
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
    validate_plan_against_graph(graph, plan, descriptor.resource_count)?;
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GraphAction {
    Finalize(u32),
    Stage(u32),
    Certify(Vec<u32>, u32),
}

fn validate_plan_against_graph(
    graph: &[u8],
    plan: &ProviderV3Plan,
    resources: u32,
) -> Result<(), Diagnostic> {
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
    let mut outcomes = Vec::new();
    for _ in 0..checkpoint_count {
        let id = descriptor_u32(graph, &mut at)?;
        if descriptor_u32(graph, &mut at)? != resources {
            return Err(provider_error("settlement checkpoint width diverges"));
        }
        for _ in 0..resources {
            if !(1..=5).contains(&descriptor_u32(graph, &mut at)?) {
                return Err(provider_error("settlement resource state is invalid"));
            }
        }
        let outcome = descriptor_u32(graph, &mut at)?;
        let detail = if outcome == 3 {
            descriptor_u32(graph, &mut at)?
        } else {
            0
        };
        outcomes.push((id, outcome, detail));
        let _abort = graph_ordinals(graph, &mut at)?;
        let _accept = graph_ordinals(graph, &mut at)?;
    }
    let starts = graph_ordinals(graph, &mut at)?;
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
                if outcome == 3 {
                    let _selected = descriptor_u32(graph, &mut at)?;
                } else if !matches!(outcome, 1 | 2) {
                    return Err(provider_error("trace witness outcome is invalid"));
                }
                GraphAction::Certify(ordinals, outcome)
            }
            _ => return Err(provider_error("settlement graph action is invalid")),
        };
        edges.push((from, to, action));
    }
    if at != graph.len() || starts.is_empty() {
        return Err(provider_error("settlement graph is not exact"));
    }
    let wanted = match plan {
        ProviderV3Plan::ScalarDiscard { .. } => (1, 0),
        ProviderV3Plan::OwnedIdentity { owner_ordinal, .. } => (3, *owner_ordinal),
    };
    let mut paths = Vec::new();
    for start in starts {
        collect_paths(
            start,
            wanted,
            &outcomes,
            &edges,
            &mut Vec::new(),
            &mut Vec::new(),
            &mut paths,
        );
    }
    paths.sort();
    paths.dedup();
    if paths.len() != 1 {
        return Err(provider_error("settlement graph normal path is not unique"));
    }
    let path = &paths[0];
    match plan {
        ProviderV3Plan::ScalarDiscard {
            scalar_result,
            finalizer_order,
            completed_checkpoints,
            semantic_ordinals,
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
            let witnesses = path
                .iter()
                .filter_map(|(action, _)| match action {
                    GraphAction::Certify(ordinals, 1) => Some(ordinals),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if *scalar_result != 0
                || &actual_order != finalizer_order
                || &actual_checkpoints != completed_checkpoints
                || witnesses.as_slice() != [semantic_ordinals]
            {
                return Err(provider_error(format!(
                    "scalar provider plan diverges from graph: order={actual_order:?} checkpoints={actual_checkpoints:?} witnesses={witnesses:?}"
                )));
            }
        }
        ProviderV3Plan::OwnedIdentity {
            owner_ordinal,
            staged_checkpoint,
            semantic_ordinals,
        } => {
            let stages = path
                .iter()
                .filter_map(|(action, checkpoint)| match action {
                    GraphAction::Stage(owner) => Some((*owner, *checkpoint)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let witnesses = path
                .iter()
                .filter_map(|(action, _)| match action {
                    GraphAction::Certify(ordinals, 2) => Some(ordinals),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if stages.as_slice() != [(*owner_ordinal, *staged_checkpoint)]
                || witnesses.as_slice() != [semantic_ordinals]
            {
                return Err(provider_error(format!(
                    "owned provider plan diverges from graph: stages={stages:?} witnesses={witnesses:?}"
                )));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_paths(
    checkpoint: u32,
    wanted: (u32, u32),
    outcomes: &[(u32, u32, u32)],
    edges: &[(u32, u32, GraphAction)],
    visited: &mut Vec<u32>,
    path: &mut Vec<(GraphAction, u32)>,
    paths: &mut Vec<Vec<(GraphAction, u32)>>,
) {
    if visited.contains(&checkpoint) || visited.len() > outcomes.len() {
        return;
    }
    if outcomes.contains(&(checkpoint, wanted.0, wanted.1)) {
        paths.push(path.clone());
        return;
    }
    visited.push(checkpoint);
    for (_, to, action) in edges.iter().filter(|(from, _, _)| *from == checkpoint) {
        path.push((action.clone(), *to));
        collect_paths(*to, wanted, outcomes, edges, visited, path, paths);
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
static void spx_v3_semantic_digest(const uint8_t *response,uint8_t out[32]){struct spx_v3_sha s;uint8_t le[8],outcome;uint32_t count=spx_v3_load_u32(response+152),tag=spx_v3_load_u32(response+136);spx_v3_store_u64(le,(uint64_t)count);spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_trace_domain,(uint32_t)sizeof(spx_v3_trace_domain));spx_v3_sha_update(&s,spx_v3_trace_path_certificate,32);spx_v3_sha_update(&s,le,8);for(uint32_t i=0;i<count;i++)spx_v3_sha_update(&s,response+156+4*i,4);outcome=(uint8_t)(tag==1?1:(tag==3?2:3));spx_v3_sha_update(&s,&outcome,1);if(outcome==3){spx_v3_store_u32(le,spx_v3_load_u32(response+140));spx_v3_sha_update(&s,le,4);}spx_v3_sha_final(&s,out);}
static void spx_v3_decision_digest(const uint8_t *decision,uint8_t out[32]){spx_v3_framed_digest(spx_v3_decision_domain,(uint32_t)sizeof(spx_v3_decision_domain),decision,SPX_V3_DECISION_BYTES,out);}
static bool spx_v3_common(const uint8_t *p,uint32_t n,const char magic[8]){return p!=NULL&&n>=20&&memcmp(p,magic,8)==0&&spx_v3_load_u32(p+8)==3&&spx_v3_load_u32(p+12)==20&&spx_v3_load_u32(p+16)==n;}
static bool spx_v3_validate_execute_inputs(const uint8_t *request,uint32_t request_len,uint8_t *frame,uint32_t frame_len,uint8_t *response,uint32_t response_len,uint64_t payloads[SPX_V3_RESOURCE_COUNT],uint8_t request_hash[32]){
 if(response==NULL||response_len!=SPX_V3_RESPONSE_BYTES||!spx_v3_common(request,request_len,"SPXNRQ03")||request_len!=SPX_V3_REQUEST_BYTES||!spx_v3_common(frame,frame_len,"SPXNFR03")||frame_len!=SPX_V3_FRAME_BYTES||!spx_v3_valid_frame_digest(frame)||!spx_v3_disjoint(request,request_len,frame,frame_len)||!spx_v3_disjoint(request,request_len,response,response_len)||!spx_v3_disjoint(frame,frame_len,response,response_len))return false;
 if(memcmp(request+20,spx_v3_call_contract,32)||memcmp(frame+20,spx_v3_call_contract,32)||memcmp(frame+52,spx_v3_recovery_contract,32)||memcmp(frame+84,spx_v3_settlement_graph,32)||memcmp(request+52,frame+116,48)||spx_v3_load_u64(request+52)==0||spx_v3_load_u64(request+60)==0||spx_v3_zero(request+68,32)||spx_v3_load_u32(request+100)!=SPX_V3_RESOURCE_COUNT||!spx_v3_zero(frame+196,64)||spx_v3_load_u32(frame+260)!=1||spx_v3_load_u32(frame+264)!=0||spx_v3_load_u32(frame+268)!=1||spx_v3_load_u32(frame+272)!=1||!spx_v3_zero(frame+276,32)||spx_v3_load_u32(frame+308)!=0||spx_v3_load_u32(frame+312)!=0||spx_v3_load_u32(frame+316)!=0||spx_v3_load_u32(frame+320)!=SPX_V3_RESOURCE_COUNT||!spx_v3_zero(frame+spx_v3_action_digest_offset(),32))return false;
 spx_v3_request_digest(request,request_hash);if(memcmp(frame+164,request_hash,32)!=0)return false;
 for(uint32_t i=0;i<SPX_V3_RESOURCE_COUNT;i++){uint32_t q=104+20*i,c=spx_v3_cell(i);if(spx_v3_load_u32(request+q)!=2||spx_v3_load_u32(request+q+4)!=i||spx_v3_load_u32(request+q+8)!=i||spx_v3_load_u32(frame+c)!=1)return false;payloads[i]=spx_v3_load_u64(request+q+12);if(spx_v3_load_u64(frame+c+4)!=payloads[i])return false;}return true;
}
#if !SPX_V3_OWNED_IDENTITY
static bool spx_v3_begin_finalizer(uint8_t *frame,uint32_t owner){uint32_t c=spx_v3_cell(owner);if(owner>=SPX_V3_RESOURCE_COUNT||spx_v3_load_u32(frame+c)!=1)return false;spx_v3_store_u32(frame+c,3);spx_v3_store_u32(frame+316,1);spx_v3_refresh_frame(frame);return true;}
static bool spx_v3_complete_finalizer(uint8_t *frame,uint32_t owner,uint32_t checkpoint){uint32_t c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+c)!=3)return false;spx_v3_store_u32(frame+c,4);spx_v3_store_u32(frame+316,0);spx_v3_store_u32(frame+268,checkpoint);spx_v3_refresh_frame(frame);return true;}
#else
static bool spx_v3_stage_owned(uint8_t *frame,uint32_t owner,uint32_t checkpoint){uint32_t c=spx_v3_cell(owner);if(owner>=SPX_V3_RESOURCE_COUNT||spx_v3_load_u32(frame+c)!=1)return false;spx_v3_store_u32(frame+c,2);spx_v3_store_u32(frame+268,checkpoint);spx_v3_refresh_frame(frame);return true;}
#endif
static void spx_v3_action_seed_digest(const uint8_t decision[32],uint64_t count,uint8_t out[32]){struct spx_v3_sha s;uint8_t le[8];spx_v3_store_u64(le,count);spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_action_seed_domain,(uint32_t)sizeof(spx_v3_action_seed_domain));spx_v3_sha_update(&s,decision,32);spx_v3_sha_update(&s,le,8);spx_v3_sha_final(&s,out);}
static void spx_v3_action_seed(uint8_t *frame,const uint8_t decision[32],uint64_t count){spx_v3_action_seed_digest(decision,count,frame+spx_v3_action_digest_offset());}
static bool spx_v3_valid_action_seed(const uint8_t *frame,const uint8_t decision[32],uint64_t count){uint8_t expected[32];spx_v3_action_seed_digest(decision,count,expected);return memcmp(frame+spx_v3_action_digest_offset(),expected,32)==0;}
static bool spx_v3_lock_decision(uint8_t *frame,const uint8_t decision[32],uint64_t actions){memcpy(frame+276,decision,32);spx_v3_action_seed(frame,decision,actions);spx_v3_store_u32(frame+272,2);spx_v3_refresh_frame(frame);return true;}
static void spx_v3_action_step(uint8_t *frame,uint32_t action,uint32_t boundary,uint32_t owner,uint32_t before,uint32_t after){uint8_t record[SPX_V3_ACTION_BYTES],next[32],le[8];uint64_t j=spx_v3_load_u32(frame+312);spx_v3_store_u64(le,j);memset(record,0,sizeof(record));memcpy(record,"SPXNAC03",8);spx_v3_store_u32(record+8,3);spx_v3_store_u32(record+12,20);spx_v3_store_u32(record+16,SPX_V3_ACTION_BYTES);memcpy(record+20,frame+20,144);spx_v3_store_u32(record+164,action);spx_v3_store_u32(record+168,boundary);spx_v3_store_u32(record+172,owner);spx_v3_store_u64(record+176,spx_v3_load_u64(frame+spx_v3_cell(owner)+4));spx_v3_store_u32(record+184,before);spx_v3_store_u32(record+188,after);spx_v3_store_u32(record+192,spx_v3_load_u32(frame+268));struct spx_v3_sha s;spx_v3_sha_init(&s);spx_v3_sha_update(&s,spx_v3_action_step_domain,(uint32_t)sizeof(spx_v3_action_step_domain));spx_v3_sha_update(&s,frame+spx_v3_action_digest_offset(),32);spx_v3_sha_update(&s,le,8);spx_v3_hash_field(&s,record,SPX_V3_ACTION_BYTES);spx_v3_sha_final(&s,next);memcpy(frame+spx_v3_action_digest_offset(),next,32);spx_v3_store_u32(frame+312,(uint32_t)(j+1));}
#if SPX_V3_OWNED_IDENTITY
static bool spx_v3_publish(uint8_t *frame,uint32_t owner){uint32_t c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+c)!=2)return false;spx_v3_store_u32(frame+272,3);spx_v3_action_step(frame,0,3,owner,2,5);spx_v3_store_u32(frame+c,5);spx_v3_store_u32(frame+308,1);spx_v3_refresh_frame(frame);return true;}
#endif
static bool spx_v3_settlement_finalize(uint8_t *frame,uint32_t action,uint32_t owner){uint32_t c=spx_v3_cell(owner);if(spx_v3_load_u32(frame+308)!=action||(spx_v3_load_u32(frame+c)!=1&&spx_v3_load_u32(frame+c)!=2))return false;uint32_t before=spx_v3_load_u32(frame+c);spx_v3_store_u32(frame+272,3);spx_v3_store_u32(frame+c,3);spx_v3_store_u32(frame+316,1);spx_v3_action_step(frame,action,1,owner,before,3);spx_v3_refresh_frame(frame);spx_v3_generated_finalize(owner,spx_v3_load_u64(frame+c+4));spx_v3_action_step(frame,action,2,owner,3,4);spx_v3_store_u32(frame+c,4);spx_v3_store_u32(frame+316,0);spx_v3_store_u32(frame+308,action+1);spx_v3_refresh_frame(frame);return true;}
static bool spx_v3_validate_settle_inputs(uint8_t *frame,uint32_t frame_len,const uint8_t *decision,uint32_t decision_len,uint8_t *candidate,uint32_t candidate_len,uint8_t decision_hash[32],uint32_t *tag,uint32_t *detail){if(candidate==NULL||candidate_len!=SPX_V3_CANDIDATE_BYTES||!spx_v3_common(frame,frame_len,"SPXNFR03")||frame_len!=SPX_V3_FRAME_BYTES||!spx_v3_valid_frame_digest(frame)||!spx_v3_common(decision,decision_len,"SPXNDC03")||decision_len!=SPX_V3_DECISION_BYTES||!spx_v3_disjoint(frame,frame_len,decision,decision_len)||!spx_v3_disjoint(frame,frame_len,candidate,candidate_len)||!spx_v3_disjoint(decision,decision_len,candidate,candidate_len)||memcmp(frame+20,decision+20,144)!=0||spx_v3_load_u32(frame+316)!=0)return false;*tag=spx_v3_load_u32(decision+164);*detail=spx_v3_load_u32(decision+168);if(*tag<1||*tag>7||((*tag==1||*tag==2||*tag==5||*tag==6||*tag==7)&&*detail!=0)||(*tag==4&&*detail==0))return false;spx_v3_decision_digest(decision,decision_hash);return true;}
static uint32_t spx_v3_emit_candidate(uint8_t *frame,uint32_t tag,uint32_t detail,uint8_t *candidate){uint32_t outcome=tag==1?1:(tag==2?2:(tag==3?3:4));memset(candidate,0,SPX_V3_CANDIDATE_BYTES);memcpy(candidate,"SPXNCR03",8);spx_v3_store_u32(candidate+8,3);spx_v3_store_u32(candidate+12,20);spx_v3_store_u32(candidate+16,SPX_V3_CANDIDATE_BYTES);memcpy(candidate+20,frame+20,144);memcpy(candidate+164,frame+164,64);if(outcome==4)memset(candidate+228,0,32);else memcpy(candidate+228,frame+228,32);memcpy(candidate+260,frame+spx_v3_frame_digest_offset(),32);memcpy(candidate+292,frame+276,32);memcpy(candidate+324,frame+spx_v3_action_digest_offset(),32);spx_v3_store_u32(candidate+356,outcome);spx_v3_store_u32(candidate+360,outcome==3?detail:0);spx_v3_store_u32(candidate+364,0);spx_v3_store_u32(candidate+368,SPX_V3_RESOURCE_COUNT);for(uint32_t i=0;i<SPX_V3_RESOURCE_COUNT;i++){uint32_t c=spx_v3_cell(i),q=372+12*i,s=spx_v3_load_u32(frame+c);if(s!=4&&s!=5)return UINT32_C(3);spx_v3_store_u32(candidate+q,s==4?1:2);spx_v3_store_u64(candidate+q+4,spx_v3_load_u64(frame+c+4));}return UINT32_C(0);}
"#;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::super::native_callable_abi_v3;
    use super::super::native_callable_wire_v3::{
        decision_digest, encode_action_evidence, encode_decision, encode_frame, encode_request,
        extend_action_chain_digest, initial_action_chain_digest, request_digest, ActionBoundary,
        ActionEvidence, ExecuteReturn, FramePhase, ProviderBinding, RecoveryFrame, RequestArgument,
        ResourceCell, ResourceState, SettlementDecision,
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

    fn append_array(output: &mut String, name: &str, bytes: &[u8]) {
        emit_c_array(output, name, bytes);
    }

    fn semantic_digest(trace: [u8; 32], ordinals: &[u32], outcome: u8) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"semaprax.native-recovery-trace-evidence.v1\0");
        hasher.update(trace);
        hasher.update((ordinals.len() as u64).to_le_bytes());
        for ordinal in ordinals {
            hasher.update(ordinal.to_le_bytes());
        }
        hasher.update([outcome]);
        hasher.finalize().into()
    }

    fn compile_and_run(source: &str, optimization: &str) {
        let directory = FixtureDirectory::new();
        let c_path = directory.path().join("provider.c");
        let executable = directory.path().join("provider");
        fs::write(&c_path, source).unwrap();
        let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
        let compile = Command::new(clang)
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-pedantic",
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "provider compile failed:\n{}\n{}",
            String::from_utf8_lossy(&compile.stdout),
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(&executable).output().unwrap();
        assert!(
            run.status.success(),
            "provider failed {:?}:\n{}\n{}",
            run.status.code(),
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
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
        let conflict =
            encode_decision(primary_binding, SettlementDecision::AbortMalformed).unwrap();
        let expected_semantic = semantic_digest(spec.trace_path_certificate, &[1, 2, 3, 4, 5], 1);
        let expected_accept_chain =
            initial_action_chain_digest(decision_digest(&accept).unwrap(), 0).unwrap();
        let initial_abort_binding = binding(&spec, 41, 51, 8);
        let mid_abort_binding = binding(&spec, 42, 52, 9);
        let uncertain_binding = binding(&spec, 43, 53, 10);
        let (_, initial_abort_frame) = initial_owned_wires(initial_abort_binding, &[201, 203]);
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
        append_array(&mut source, "mid_abort_frame", &mid_abort_frame);
        append_array(&mut source, "uncertain_frame", &uncertain_frame);
        append_array(&mut source, "initial_abort_decision", &initial_abort);
        append_array(&mut source, "mid_abort_decision", &mid_abort);
        append_array(&mut source, "uncertain_abort_decision", &uncertain_abort);
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
 if(finalizer_count!=2||finalizer_order[0]!=1||finalizer_order[1]!=0||finalizer_payload[0]!=73||finalizer_payload[1]!=41||spx_v3_load_u32(frame+268)!=3||spx_v3_load_u32(frame+324)!=4||spx_v3_load_u32(frame+336)!=4||memcmp(frame+228,expected_semantic,32))return 4;
 if(spx_fixture_settle_v3(frame,sizeof(frame),accept_decision,sizeof(accept_decision),candidate,sizeof(candidate))!=0)return 5;
 if(spx_v3_load_u32(frame+272)!=4||spx_v3_load_u32(candidate+356)!=1||spx_v3_load_u32(candidate+372)!=1||spx_v3_load_u32(candidate+384)!=1||memcmp(candidate+228,frame+228,32)||memcmp(candidate+324,expected_accept_chain,32))return 6;
 memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),accept_decision,sizeof(accept_decision),candidate,sizeof(candidate))!=0||memcmp(saved,candidate,sizeof(saved))||finalizer_count!=2)return 7;
 memcpy(saved_frame,frame,sizeof(frame));memset(candidate,0xa5,sizeof(candidate));memcpy(saved,candidate,sizeof(saved));if(spx_fixture_settle_v3(frame,sizeof(frame),conflict_decision,sizeof(conflict_decision),candidate,sizeof(candidate))!=3||finalizer_count!=2||memcmp(frame,saved_frame,sizeof(frame))||memcmp(candidate,saved,sizeof(candidate)))return 8;
 if(memcmp(spx_fixture_descriptor_v3(),spx_v3_descriptor,sizeof(spx_v3_descriptor)))return 9;
 memcpy(abort_frame,initial_abort_frame,sizeof(abort_frame));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),initial_abort_decision,sizeof(initial_abort_decision),candidate,sizeof(candidate))!=0||finalizer_count!=4||finalizer_order[2]!=1||finalizer_payload[2]!=203||finalizer_order[3]!=0||finalizer_payload[3]!=201)return 12;
 memcpy(abort_frame,mid_abort_frame,sizeof(abort_frame));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),mid_abort_decision,sizeof(mid_abort_decision),candidate,sizeof(candidate))!=0||finalizer_count!=5||finalizer_order[4]!=0||finalizer_payload[4]!=211)return 13;
 memcpy(abort_frame,uncertain_frame,sizeof(abort_frame));if(spx_fixture_settle_v3(abort_frame,sizeof(abort_frame),uncertain_abort_decision,sizeof(uncertain_abort_decision),candidate,sizeof(candidate))!=1||finalizer_count!=5)return 14;
 return 0;}
"#,
        );
        for optimization in ["-O0", "-O2"] {
            compile_and_run(&source, optimization);
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
            checkpoint: 2,
        })
        .unwrap();
        let expected_accept_chain =
            extend_action_chain_digest(accept_seed, 0, &accept_action).unwrap();
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
            checkpoint: 2,
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
            checkpoint: 2,
        })
        .unwrap();
        let abort_after_started =
            extend_action_chain_digest(abort_seed, 0, &abort_started).unwrap();
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
    }
}
