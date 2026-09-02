//! Independent strict decoder for the metadata-only callable native descriptor v3.
//!
//! This module intentionally does not share a reader, writer, constants, or
//! semantic types with the compiler encoder or the callable-v2/proof decoders.
//! Successfully decoding these bytes grants no runtime authority, loads no
//! image, and permits no physical finalizer.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "callable descriptor v3 remains private admission metadata"
    )
)]

use std::collections::HashSet;

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"SPXNABI3";
const VERSION: u32 = 3;
const HEADER_SIZE: u32 = 20;
const LINKAGE_DYNAMIC: u32 = 1;
const LINKAGE_IOS_STATIC: u32 = 2;
const CALL_ABI_TAG: u32 = 3;
const REQUIRED_OBLIGATIONS: u32 = 0x03ff;
const OWNED_PAYLOAD_WIRE_KIND: u32 = 1;
const GRAPH_VERSION: u32 = 3;

const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_SYMBOL_BYTES: usize = 1024;
const MAX_WIRE_BYTES: u32 = 1024 * 1024;
const MAX_EVENT_COUNT: u32 = 65_536;
const MAX_DICTIONARY_BYTES: u32 = 1024 * 1024;
const MAX_DICTIONARY_ENTRIES: u32 = 65_536;
const MAX_RESOURCES: u32 = 4_096;
const MAX_CHECKPOINTS: u32 = 65_536;
const MAX_GRAPH_WORK_UNITS: u32 = 1_000_000;
const MAX_ACTIVE_FRAMES: u32 = 256;
const MAX_QUARANTINED_FRAMES: u32 = 64;
const MAX_INSTANCE_RESERVED_BYTES: u32 = 64 * 1024 * 1024;

const MIN_SCALAR_PARAMETER_BYTES: usize = 4 + 4 + 4 + 1 + 4;
const MIN_RESULT_BYTES: usize = 4;

const REQUEST_FIXED_BYTES: u32 = 104;
const REQUEST_I64_BYTES: u32 = 16;
const REQUEST_BOOL_BYTES: u32 = 12;
const REQUEST_OWNER_BYTES: u32 = 20;
const EXECUTE_RESPONSE_FIXED_BYTES: u32 = 156;
const EXECUTE_RESPONSE_EVENT_BYTES: u32 = 4;
const FRAME_FIXED_BYTES: u32 = 388;
const FRAME_RESOURCE_BYTES: u32 = 12;
const DECISION_BYTES: u32 = 172;
const ACTION_EVIDENCE_BYTES: u32 = 196;
const CANDIDATE_RECEIPT_FIXED_BYTES: u32 = 372;
const CANDIDATE_RECEIPT_RESOURCE_BYTES: u32 = 12;
const HOST_RECEIPT_BYTES: u32 = 524;
const CANONICAL_ACTIVE_FRAMES: u32 = 256;
const CANONICAL_QUARANTINED_FRAMES: u32 = 64;

const SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-descriptor-schema.v3\0";
const TARGET_DOMAIN: &[u8] = b"semaprax.native-callable-target.v3\0";
const PHYSICAL_MODULE_DOMAIN: &[u8] = b"semaprax.native-callable-physical-module.v3\0";
const GRAPH_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-graph.v3\0";
const REQUEST_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-request-schema.v3\0";
const EXECUTE_RESPONSE_SCHEMA_DOMAIN: &[u8] =
    b"semaprax.native-callable-execute-response-schema.v3\0";
const FRAME_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-frame-schema.v3\0";
const DECISION_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-decision-schema.v3\0";
const ACTION_SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-action-schema.v3\0";
const CANDIDATE_RECEIPT_SCHEMA_DOMAIN: &[u8] =
    b"semaprax.native-callable-candidate-receipt-schema.v3\0";
const COMMITTED_RECEIPT_SCHEMA_DOMAIN: &[u8] =
    b"semaprax.native-callable-committed-receipt-schema.v3\0";
const CALL_ABI_DOMAIN: &[u8] = b"semaprax.native-callable-c-abi.v3\0";
const CALL_CONTRACT_DOMAIN: &[u8] = b"semaprax.native-callable-contract.v3\0";
const SYMBOL_SEED_DOMAIN: &[u8] = b"semaprax.native-callable-symbol-seed.v3\0";
const GETTER_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-getter.v3\0";
const EXECUTE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-execute.v3\0";
const SETTLE_SYMBOL_DOMAIN: &[u8] = b"semaprax.native-callable-settle.v3\0";
const TRACE_EVIDENCE_DOMAIN: &[u8] = b"semaprax.native-recovery-trace-evidence.v1\0";

const DESCRIPTOR_SCHEMA_STATEMENT: &[u8] = b"SPXNABI3;u32le;header=20;sequential-no-offsets-no-trailing;target;linkage-profile;19-fingerprints;module;function;getter;execute;settle;abi-tag;obligations;15-capacities;signature;graph-len;graph";
const REQUEST_SCHEMA_STATEMENT: &[u8] = b"SPXNRQ03;v3;u32le;header20;total-exact;call32;invocation-u64;generation-u64;challenge32;argc;args[tag,index,payload];scalar-tag1;i64-8;bool-u32-0-or-1;owned-tag2-owner-u32-payload-u64;no-trailing";
const EXECUTE_RESPONSE_SCHEMA_STATEMENT: &[u8] = b"SPXNEX03;v3;u32le;header20;total-declared;zero-tail-to-capacity;call32;invocation-u64;generation-u64;challenge32;request-digest32;checkpoint;outcome;detail;payload-u64;event-count;ordinals;outcomes1-scalar-2-semantic-3-owned";
const FRAME_SCHEMA_STATEMENT: &[u8] = b"SPXNFR03;v3;u32le;header20;total-exact;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;return-tag;return-code;returns1-pending-2-returned-3-preexecute-host-unwind;preexecute-host-unwind-code-4294967294;checkpoint;phase;decision32;next-action;record-count;active-finalizers;resource-count;cells[state-u32,payload-u64];action-chain32;pre-candidate-frame32";
const DECISION_SCHEMA_STATEMENT: &[u8] = b"SPXNDC03;v3;u32le;header20;total172;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;decision-tag;detail;tags1-scalar-2-semantic-3-owned-4-physical-5-malformed-6-trace-7-unwind";
const ACTION_SCHEMA_STATEMENT: &[u8] = b"SPXNAC03;v3;u32le;header20;total196;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;action-index;boundary-tag;owner;payload-u64;before-state;after-state;checkpoint;tags1-start-2-complete-3-publish";
const CANDIDATE_RECEIPT_SCHEMA_STATEMENT: &[u8] = b"SPXNCR03;v3;u32le;header20;total372-plus-12r;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;frame32;decision32;action32;outcome;detail;active-finalizers-zero;disposition-count;cells[disposition-u32,payload-u64]";
const COMMITTED_RECEIPT_SCHEMA_STATEMENT: &[u8] = b"SPXHRP03;v3;u32le;header20;total524;host-only;instance32;call32;recovery32;graph32;invocation-u64;generation-u64;challenge32;request32;response32;semantic32;frame32;decision32;action32;candidate32;ledger-before32;ledger-after32;publication;detail;hmac32;separate-receipt-key;atomic-ledger-and-cache";
const CALL_ABI_STATEMENT: &[u8] = b"extern-C;getter=const-u8-ptr(void);execute=u32(const-u8-ptr,u32,u8-ptr,u32,u8-ptr,u32);settle=u32(u8-ptr,u32,const-u8-ptr,u32,u8-ptr,u32);windows-cdecl;synchronous;same-thread;no-unwind;no-longjmp;no-callbacks;no-retained-pointers;no-reentrancy";

const PARAMETER_SCALAR: u32 = 1;
const PARAMETER_OWNED_RESOURCE: u32 = 2;
const SCALAR_I64: u32 = 1;
const SCALAR_BOOL: u32 = 2;
const RESULT_SCALAR_I64: u32 = 1;
const RESULT_OWNED_INPUT: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Linkage {
    Dynamic,
    IosStatic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fingerprints {
    pub(crate) schema: [u8; 32],
    pub(crate) target: [u8; 32],
    pub(crate) semantic_module: [u8; 32],
    pub(crate) physical_module: [u8; 32],
    pub(crate) function_template: [u8; 32],
    pub(crate) execution_cleanup: [u8; 32],
    pub(crate) event_dictionary: [u8; 32],
    pub(crate) trace_path_certificate: [u8; 32],
    pub(crate) recovery_contract: [u8; 32],
    pub(crate) settlement_graph: [u8; 32],
    pub(crate) request_schema: [u8; 32],
    pub(crate) execute_response_schema: [u8; 32],
    pub(crate) frame_schema: [u8; 32],
    pub(crate) decision_schema: [u8; 32],
    pub(crate) action_schema: [u8; 32],
    pub(crate) candidate_receipt_schema: [u8; 32],
    pub(crate) committed_receipt_schema: [u8; 32],
    pub(crate) call_abi: [u8; 32],
    pub(crate) call_contract: [u8; 32],
}

impl Fingerprints {
    fn iter(&self) -> impl Iterator<Item = &[u8; 32]> {
        [
            &self.schema,
            &self.target,
            &self.semantic_module,
            &self.physical_module,
            &self.function_template,
            &self.execution_cleanup,
            &self.event_dictionary,
            &self.trace_path_certificate,
            &self.recovery_contract,
            &self.settlement_graph,
            &self.request_schema,
            &self.execute_response_schema,
            &self.frame_schema,
            &self.decision_schema,
            &self.action_schema,
            &self.candidate_receipt_schema,
            &self.committed_receipt_schema,
            &self.call_abi,
            &self.call_contract,
        ]
        .into_iter()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarKind {
    I64,
    Bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Parameter {
    Scalar {
        index: usize,
        value: String,
        kind: ScalarKind,
    },
    Owned {
        index: usize,
        value: String,
        owner_ordinal: usize,
        resource: String,
        lifecycle: String,
        payload_wire_kind: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultShape {
    ScalarI64,
    OwnedInput {
        parameter_index: usize,
        owner_ordinal: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Capacities {
    pub(crate) request: u32,
    pub(crate) execute_response: u32,
    pub(crate) frame: u32,
    pub(crate) decision: u32,
    pub(crate) action_evidence: u32,
    pub(crate) candidate_receipt: u32,
    pub(crate) event_count: u32,
    pub(crate) dictionary_bytes: u32,
    pub(crate) dictionary_entries: u32,
    pub(crate) resource_count: u32,
    pub(crate) checkpoint_count: u32,
    pub(crate) graph_work_units: u32,
    pub(crate) active_frames: u32,
    pub(crate) quarantined_frames: u32,
    pub(crate) instance_reserved_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceState {
    Live,
    ProvisionalResult,
    Finalizing,
    Dead,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Outcome {
    ScalarSuccess,
    SemanticFailure,
    OwnedSuccess(u32),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TraceOutcome {
    ScalarSuccess,
    OwnedSuccess,
    Failure { selected_ordinal: u32 },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct TraceEvidence {
    pub(crate) digest: [u8; 32],
    pub(crate) ordinals: Vec<u32>,
    pub(crate) outcome: TraceOutcome,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Action {
    Finalize(u32),
    StageOwnedResult(u32),
    CertifyOutcome(TraceEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Checkpoint {
    pub(crate) id: u32,
    pub(crate) resources: Vec<ResourceState>,
    pub(crate) outcome: Option<Outcome>,
    pub(crate) abort_order: Vec<u32>,
    pub(crate) accept_order: Vec<u32>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct Edge {
    pub(crate) from: u32,
    pub(crate) to: u32,
    pub(crate) action: Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettlementGraph {
    pub(crate) function: String,
    pub(crate) recovery_contract: [u8; 32],
    pub(crate) execution_cleanup: [u8; 32],
    pub(crate) trace_path_certificate: [u8; 32],
    pub(crate) resource_count: usize,
    pub(crate) checkpoints: Vec<Checkpoint>,
    pub(crate) starts: Vec<u32>,
    pub(crate) edges: Vec<Edge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) target: String,
    pub(crate) linkage: Linkage,
    pub(crate) fingerprints: Fingerprints,
    pub(crate) module: String,
    pub(crate) function: String,
    pub(crate) getter_symbol: String,
    pub(crate) execute_symbol: String,
    pub(crate) settle_symbol: String,
    pub(crate) call_abi_tag: u32,
    pub(crate) obligations: u32,
    pub(crate) capacities: Capacities,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) result: ResultShape,
    pub(crate) graph: SettlementGraph,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorError {
    Malformed,
    UnsupportedSchema,
    WrongTarget,
    NonCanonical,
    ArtifactMismatch,
}

impl Descriptor {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, DescriptorError> {
        let target = current_target_tag()?;
        let linkage = if cfg!(target_os = "ios") {
            Linkage::IosStatic
        } else {
            Linkage::Dynamic
        };
        Self::parse_for_target(bytes, &target, linkage)
    }

    pub(crate) fn parse_for_target(
        bytes: &[u8],
        expected_target: &str,
        expected_linkage: Linkage,
    ) -> Result<Self, DescriptorError> {
        if bytes.len() > MAX_DESCRIPTOR_BYTES || bytes.len() < HEADER_SIZE as usize {
            return Err(DescriptorError::Malformed);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != MAGIC || reader.u32()? != VERSION || reader.u32()? != HEADER_SIZE {
            return Err(DescriptorError::UnsupportedSchema);
        }
        if reader.usize()? != bytes.len() {
            return Err(DescriptorError::Malformed);
        }
        let target = reader.text(MAX_TEXT_BYTES)?;
        if target != expected_target {
            return Err(DescriptorError::WrongTarget);
        }
        let linkage = match reader.u32()? {
            LINKAGE_DYNAMIC => Linkage::Dynamic,
            LINKAGE_IOS_STATIC => Linkage::IosStatic,
            _ => return Err(DescriptorError::UnsupportedSchema),
        };
        if linkage != expected_linkage {
            return Err(DescriptorError::WrongTarget);
        }
        let fingerprints = Fingerprints {
            schema: reader.fingerprint()?,
            target: reader.fingerprint()?,
            semantic_module: reader.fingerprint()?,
            physical_module: reader.fingerprint()?,
            function_template: reader.fingerprint()?,
            execution_cleanup: reader.fingerprint()?,
            event_dictionary: reader.fingerprint()?,
            trace_path_certificate: reader.fingerprint()?,
            recovery_contract: reader.fingerprint()?,
            settlement_graph: reader.fingerprint()?,
            request_schema: reader.fingerprint()?,
            execute_response_schema: reader.fingerprint()?,
            frame_schema: reader.fingerprint()?,
            decision_schema: reader.fingerprint()?,
            action_schema: reader.fingerprint()?,
            candidate_receipt_schema: reader.fingerprint()?,
            committed_receipt_schema: reader.fingerprint()?,
            call_abi: reader.fingerprint()?,
            call_contract: reader.fingerprint()?,
        };
        if fingerprints.schema != schema_fingerprint() {
            return Err(DescriptorError::UnsupportedSchema);
        }
        if fingerprints.target != target_fingerprint(target.as_bytes()) {
            return Err(DescriptorError::WrongTarget);
        }
        if fingerprints.iter().any(|value| *value == [0; 32]) {
            return Err(DescriptorError::NonCanonical);
        }

        let module = reader.text(MAX_TEXT_BYTES)?;
        let function = reader.text(MAX_TEXT_BYTES)?;
        if fingerprints.physical_module
            != physical_module_fingerprint(
                &fingerprints.schema,
                &fingerprints.target,
                &fingerprints.semantic_module,
                module.as_bytes(),
                linkage,
            )
        {
            return Err(DescriptorError::NonCanonical);
        }
        let getter_symbol = reader.text(MAX_SYMBOL_BYTES)?;
        let execute_symbol = reader.text(MAX_SYMBOL_BYTES)?;
        let settle_symbol = reader.text(MAX_SYMBOL_BYTES)?;
        let symbols = [&getter_symbol, &execute_symbol, &settle_symbol];
        if symbols.iter().any(|symbol| !is_c_symbol(symbol))
            || symbols[0] == symbols[1]
            || symbols[0] == symbols[2]
            || symbols[1] == symbols[2]
        {
            return Err(DescriptorError::NonCanonical);
        }

        let call_abi_tag = reader.u32()?;
        if call_abi_tag != CALL_ABI_TAG {
            return Err(DescriptorError::UnsupportedSchema);
        }
        let obligations = reader.u32()?;
        if obligations != REQUIRED_OBLIGATIONS {
            return Err(DescriptorError::NonCanonical);
        }
        if fingerprints.request_schema != request_schema_fingerprint()
            || fingerprints.execute_response_schema != execute_response_schema_fingerprint()
            || fingerprints.frame_schema != frame_schema_fingerprint()
            || fingerprints.decision_schema != decision_schema_fingerprint()
            || fingerprints.action_schema != action_schema_fingerprint()
            || fingerprints.candidate_receipt_schema != candidate_receipt_schema_fingerprint()
            || fingerprints.committed_receipt_schema != committed_receipt_schema_fingerprint()
            || fingerprints.call_abi != call_abi_fingerprint()
            || fingerprints.candidate_receipt_schema == fingerprints.committed_receipt_schema
        {
            return Err(DescriptorError::UnsupportedSchema);
        }

        let capacities = Capacities {
            request: reader.u32()?,
            execute_response: reader.u32()?,
            frame: reader.u32()?,
            decision: reader.u32()?,
            action_evidence: reader.u32()?,
            candidate_receipt: reader.u32()?,
            event_count: reader.u32()?,
            dictionary_bytes: reader.u32()?,
            dictionary_entries: reader.u32()?,
            resource_count: reader.u32()?,
            checkpoint_count: reader.u32()?,
            graph_work_units: reader.u32()?,
            active_frames: reader.u32()?,
            quarantined_frames: reader.u32()?,
            instance_reserved_bytes: reader.u32()?,
        };
        validate_capacity_ceilings(&capacities)?;

        let parameter_count = reader.usize()?;
        let maximum_structural_count =
            reader.remaining().saturating_sub(MIN_RESULT_BYTES + 4) / MIN_SCALAR_PARAMETER_BYTES;
        if parameter_count > maximum_structural_count {
            return Err(DescriptorError::NonCanonical);
        }
        let mut parameters = Vec::with_capacity(parameter_count);
        let mut values = HashSet::with_capacity(parameter_count);
        let mut next_owner = 0_usize;
        for expected_index in 0..parameter_count {
            let tag = reader.u32()?;
            let index = reader.usize()?;
            if index != expected_index {
                return Err(DescriptorError::NonCanonical);
            }
            let value = reader.text(MAX_TEXT_BYTES)?;
            if !values.insert(value.clone()) {
                return Err(DescriptorError::NonCanonical);
            }
            match tag {
                PARAMETER_SCALAR => {
                    let kind = match reader.u32()? {
                        SCALAR_I64 => ScalarKind::I64,
                        SCALAR_BOOL => ScalarKind::Bool,
                        _ => return Err(DescriptorError::NonCanonical),
                    };
                    parameters.push(Parameter::Scalar { index, value, kind });
                }
                PARAMETER_OWNED_RESOURCE => {
                    let owner_ordinal = reader.usize()?;
                    if owner_ordinal != next_owner {
                        return Err(DescriptorError::NonCanonical);
                    }
                    next_owner = next_owner
                        .checked_add(1)
                        .ok_or(DescriptorError::Malformed)?;
                    let resource = reader.text(MAX_TEXT_BYTES)?;
                    let lifecycle = reader.text(MAX_TEXT_BYTES)?;
                    let payload_wire_kind = reader.u32()?;
                    if payload_wire_kind != OWNED_PAYLOAD_WIRE_KIND {
                        return Err(DescriptorError::UnsupportedSchema);
                    }
                    parameters.push(Parameter::Owned {
                        index,
                        value,
                        owner_ordinal,
                        resource,
                        lifecycle,
                        payload_wire_kind,
                    });
                }
                _ => return Err(DescriptorError::NonCanonical),
            }
        }
        let result = match reader.u32()? {
            RESULT_SCALAR_I64 => ResultShape::ScalarI64,
            RESULT_OWNED_INPUT => {
                let parameter_index = reader.usize()?;
                let value = reader.text(MAX_TEXT_BYTES)?;
                let owner_ordinal = reader.usize()?;
                let Some(Parameter::Owned {
                    index,
                    value: expected_value,
                    owner_ordinal: expected_ordinal,
                    ..
                }) = parameters.get(parameter_index)
                else {
                    return Err(DescriptorError::NonCanonical);
                };
                if *index != parameter_index
                    || *expected_value != value
                    || *expected_ordinal != owner_ordinal
                {
                    return Err(DescriptorError::NonCanonical);
                }
                ResultShape::OwnedInput {
                    parameter_index,
                    owner_ordinal,
                }
            }
            _ => return Err(DescriptorError::NonCanonical),
        };
        validate_exact_capacities(&capacities, &parameters)?;

        let graph_len = reader.usize()?;
        if graph_len == 0 || graph_len > reader.remaining() {
            return Err(DescriptorError::NonCanonical);
        }
        let graph_bytes = reader.take(graph_len)?;
        if !reader.is_finished() {
            return Err(DescriptorError::Malformed);
        }
        if fingerprints.settlement_graph != graph_fingerprint(graph_bytes) {
            return Err(DescriptorError::ArtifactMismatch);
        }
        let graph = SettlementGraph::parse(graph_bytes, &capacities)?;
        if encode_graph(&graph)? != graph_bytes {
            return Err(DescriptorError::NonCanonical);
        }
        validate_cross_bindings(
            &fingerprints,
            &function,
            &parameters,
            &result,
            &capacities,
            &graph,
        )?;

        let expected_contract = call_contract_fingerprint(
            &target,
            linkage,
            &fingerprints,
            &module,
            &function,
            &capacities,
            &parameters,
            &result,
        );
        if fingerprints.call_contract != expected_contract {
            return Err(DescriptorError::ArtifactMismatch);
        }
        let expected_symbols = derive_symbols(&fingerprints);
        if (
            getter_symbol.as_str(),
            execute_symbol.as_str(),
            settle_symbol.as_str(),
        ) != (
            expected_symbols.0.as_str(),
            expected_symbols.1.as_str(),
            expected_symbols.2.as_str(),
        ) {
            return Err(DescriptorError::NonCanonical);
        }

        let descriptor = Self {
            target,
            linkage,
            fingerprints,
            module,
            function,
            getter_symbol,
            execute_symbol,
            settle_symbol,
            call_abi_tag,
            obligations,
            capacities,
            parameters,
            result,
            graph,
        };
        if encode_descriptor(&descriptor)? != bytes {
            return Err(DescriptorError::NonCanonical);
        }
        Ok(descriptor)
    }
}

impl SettlementGraph {
    fn parse(bytes: &[u8], capacities: &Capacities) -> Result<Self, DescriptorError> {
        let mut reader = Reader::new(bytes);
        if reader.u32()? != GRAPH_VERSION {
            return Err(DescriptorError::UnsupportedSchema);
        }
        let function = reader.text(MAX_TEXT_BYTES)?;
        let recovery_contract = reader.fingerprint()?;
        let execution_cleanup = reader.fingerprint()?;
        let trace_path_certificate = reader.fingerprint()?;
        if [recovery_contract, execution_cleanup, trace_path_certificate].contains(&[0; 32]) {
            return Err(DescriptorError::NonCanonical);
        }
        let resource_count = reader.usize()?;
        let checkpoint_count = reader.usize()?;
        if resource_count == 0
            || resource_count != capacities.resource_count as usize
            || checkpoint_count == 0
            || checkpoint_count != capacities.checkpoint_count as usize
        {
            return Err(DescriptorError::NonCanonical);
        }
        let base_work = resource_count
            .checked_mul(checkpoint_count)
            .ok_or(DescriptorError::Malformed)?;
        let minimum_checkpoint = 20_usize
            .checked_add(
                resource_count
                    .checked_mul(4)
                    .ok_or(DescriptorError::Malformed)?,
            )
            .ok_or(DescriptorError::Malformed)?;
        if checkpoint_count > reader.remaining() / minimum_checkpoint {
            return Err(DescriptorError::Malformed);
        }
        let mut checkpoints = Vec::with_capacity(checkpoint_count);
        for index in 0..checkpoint_count {
            let id = reader.u32()?;
            if id != u32::try_from(index + 1).map_err(|_| DescriptorError::Malformed)? {
                return Err(DescriptorError::NonCanonical);
            }
            if reader.usize()? != resource_count {
                return Err(DescriptorError::NonCanonical);
            }
            let mut resources = Vec::with_capacity(resource_count);
            for _ in 0..resource_count {
                resources.push(match reader.u32()? {
                    1 => ResourceState::Live,
                    2 => ResourceState::ProvisionalResult,
                    3 => ResourceState::Finalizing,
                    4 => ResourceState::Dead,
                    5 => ResourceState::Published,
                    _ => return Err(DescriptorError::NonCanonical),
                });
            }
            let outcome = match reader.u32()? {
                0 => None,
                1 => Some(Outcome::ScalarSuccess),
                2 => Some(Outcome::SemanticFailure),
                3 => Some(Outcome::OwnedSuccess(reader.u32()?)),
                _ => return Err(DescriptorError::NonCanonical),
            };
            let abort_order = reader.ordinals(resource_count)?;
            let accept_order = reader.ordinals(resource_count)?;
            let checkpoint = Checkpoint {
                id,
                resources,
                outcome,
                abort_order,
                accept_order,
            };
            validate_checkpoint(&checkpoint, resource_count)?;
            checkpoints.push(checkpoint);
        }
        let starts = reader.ordinals(checkpoint_count)?;
        let edge_count = reader.usize()?;
        if base_work != capacities.graph_work_units as usize || edge_count > reader.remaining() / 12
        {
            return Err(DescriptorError::NonCanonical);
        }
        let mut edges = Vec::with_capacity(edge_count);
        for _ in 0..edge_count {
            let from = reader.u32()?;
            let to = reader.u32()?;
            let action = match reader.u32()? {
                1 => Action::Finalize(reader.u32()?),
                2 => Action::StageOwnedResult(reader.u32()?),
                3 => {
                    let digest = reader.fingerprint()?;
                    let ordinal_count = reader.usize()?;
                    let maximum_structural_count = reader.remaining().saturating_sub(4) / 4;
                    if ordinal_count > capacities.event_count as usize
                        || ordinal_count > maximum_structural_count
                    {
                        return Err(DescriptorError::NonCanonical);
                    }
                    let mut ordinals = Vec::with_capacity(ordinal_count);
                    for _ in 0..ordinal_count {
                        let ordinal = reader.u32()?;
                        if ordinal == 0 || ordinal > capacities.dictionary_entries {
                            return Err(DescriptorError::NonCanonical);
                        }
                        ordinals.push(ordinal);
                    }
                    let outcome = match reader.u32()? {
                        1 => TraceOutcome::ScalarSuccess,
                        2 => TraceOutcome::OwnedSuccess,
                        3 => {
                            let selected_ordinal = reader.u32()?;
                            if selected_ordinal == 0
                                || selected_ordinal > capacities.dictionary_entries
                                || !ordinals.contains(&selected_ordinal)
                            {
                                return Err(DescriptorError::NonCanonical);
                            }
                            TraceOutcome::Failure { selected_ordinal }
                        }
                        _ => return Err(DescriptorError::NonCanonical),
                    };
                    let expected =
                        trace_evidence_fingerprint(&trace_path_certificate, &ordinals, outcome);
                    if digest == [0; 32] || digest != expected {
                        return Err(DescriptorError::ArtifactMismatch);
                    }
                    Action::CertifyOutcome(TraceEvidence {
                        digest,
                        ordinals,
                        outcome,
                    })
                }
                _ => return Err(DescriptorError::NonCanonical),
            };
            edges.push(Edge { from, to, action });
        }
        if !reader.is_finished() {
            return Err(DescriptorError::Malformed);
        }
        validate_progress(&checkpoints, &starts, &edges)?;
        Ok(Self {
            function,
            recovery_contract,
            execution_cleanup,
            trace_path_certificate,
            resource_count,
            checkpoints,
            starts,
            edges,
        })
    }
}

fn validate_checkpoint(
    checkpoint: &Checkpoint,
    resource_count: usize,
) -> Result<(), DescriptorError> {
    if checkpoint.resources.len() != resource_count
        || checkpoint
            .resources
            .iter()
            .any(|state| matches!(state, ResourceState::Finalizing | ResourceState::Published))
    {
        return Err(DescriptorError::NonCanonical);
    }
    let provisional = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter_map(|(ordinal, state)| {
            (*state == ResourceState::ProvisionalResult).then_some(ordinal as u32)
        })
        .collect::<Vec<_>>();
    if provisional.len() > 1 {
        return Err(DescriptorError::NonCanonical);
    }
    let abort_required = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter_map(|(ordinal, state)| (*state != ResourceState::Dead).then_some(ordinal as u32))
        .collect::<HashSet<_>>();
    validate_exact_order(&checkpoint.abort_order, &abort_required)?;
    let accept_required = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter_map(|(ordinal, state)| (*state == ResourceState::Live).then_some(ordinal as u32))
        .collect::<HashSet<_>>();
    match checkpoint.outcome {
        None if checkpoint.accept_order.is_empty() => {}
        None => return Err(DescriptorError::NonCanonical),
        Some(Outcome::ScalarSuccess | Outcome::SemanticFailure) if provisional.is_empty() => {
            validate_exact_order(&checkpoint.accept_order, &accept_required)?;
        }
        Some(Outcome::OwnedSuccess(owner)) if provisional.as_slice() == [owner] => {
            validate_exact_order(&checkpoint.accept_order, &accept_required)?;
        }
        Some(_) => return Err(DescriptorError::NonCanonical),
    }
    Ok(())
}

fn validate_exact_order(order: &[u32], required: &HashSet<u32>) -> Result<(), DescriptorError> {
    let actual = order.iter().copied().collect::<HashSet<_>>();
    if actual.len() != order.len() || actual != *required {
        return Err(DescriptorError::NonCanonical);
    }
    Ok(())
}

fn validate_progress(
    checkpoints: &[Checkpoint],
    starts: &[u32],
    edges: &[Edge],
) -> Result<(), DescriptorError> {
    if starts != [1]
        || checkpoints[0].outcome.is_some()
        || checkpoints[0]
            .resources
            .iter()
            .any(|state| *state != ResourceState::Live)
    {
        return Err(DescriptorError::NonCanonical);
    }
    let mut seen_edges = HashSet::new();
    let mut seen_actions = HashSet::new();
    let mut reachable = HashSet::from([1_u32]);
    let mut outgoing = HashSet::new();
    for edge in edges {
        if edge.from == 0
            || edge.to == 0
            || edge.from >= edge.to
            || edge.to as usize > checkpoints.len()
            || !seen_edges.insert(edge.clone())
            || !seen_actions.insert((edge.from, edge.action.clone()))
            || !reachable.contains(&edge.from)
        {
            return Err(DescriptorError::NonCanonical);
        }
        let from = &checkpoints[(edge.from - 1) as usize];
        let to = &checkpoints[(edge.to - 1) as usize];
        if from.outcome.is_some() || !valid_transition(from, to, &edge.action) {
            return Err(DescriptorError::NonCanonical);
        }
        outgoing.insert(edge.from);
        reachable.insert(edge.to);
    }
    if reachable.len() != checkpoints.len()
        || checkpoints
            .iter()
            .any(|checkpoint| checkpoint.outcome.is_none() != outgoing.contains(&checkpoint.id))
    {
        return Err(DescriptorError::NonCanonical);
    }
    Ok(())
}

fn valid_transition(from: &Checkpoint, to: &Checkpoint, action: &Action) -> bool {
    match action {
        Action::Finalize(owner) => {
            let Some(position) = from
                .abort_order
                .iter()
                .position(|candidate| candidate == owner)
            else {
                return false;
            };
            let prefix_only_provisional = from.abort_order[..position].iter().all(|ordinal| {
                from.resources[*ordinal as usize] == ResourceState::ProvisionalResult
            });
            let mut expected_abort = from.abort_order.clone();
            expected_abort.remove(position);
            let state_transition =
                changed_state(from, to, *owner, ResourceState::Live, ResourceState::Dead)
                    || changed_state(
                        from,
                        to,
                        *owner,
                        ResourceState::ProvisionalResult,
                        ResourceState::Dead,
                    );
            to.outcome.is_none()
                && state_transition
                && from.accept_order.is_empty()
                && to.accept_order.is_empty()
                && prefix_only_provisional
                && to.abort_order == expected_abort
        }
        Action::StageOwnedResult(owner) => {
            to.outcome.is_none()
                && changed_state(
                    from,
                    to,
                    *owner,
                    ResourceState::Live,
                    ResourceState::ProvisionalResult,
                )
                && from.accept_order.is_empty()
                && to.accept_order.is_empty()
                && from.abort_order == to.abort_order
        }
        Action::CertifyOutcome(trace) => {
            let expected_accept = to
                .abort_order
                .iter()
                .copied()
                .filter(|ordinal| to.resources[*ordinal as usize] == ResourceState::Live)
                .collect::<Vec<_>>();
            from.outcome.is_none()
                && to.outcome.is_some()
                && from.resources == to.resources
                && from.accept_order.is_empty()
                && from.abort_order == to.abort_order
                && to.accept_order == expected_accept
                && trace.digest != [0; 32]
                && matches!(
                    (to.outcome, trace.outcome),
                    (Some(Outcome::ScalarSuccess), TraceOutcome::ScalarSuccess)
                        | (Some(Outcome::OwnedSuccess(_)), TraceOutcome::OwnedSuccess)
                        | (Some(Outcome::SemanticFailure), TraceOutcome::Failure { .. })
                )
        }
    }
}

fn changed_state(
    from: &Checkpoint,
    to: &Checkpoint,
    owner: u32,
    expected_from: ResourceState,
    expected_to: ResourceState,
) -> bool {
    let Ok(owner) = usize::try_from(owner) else {
        return false;
    };
    owner < from.resources.len()
        && from.resources.len() == to.resources.len()
        && from
            .resources
            .iter()
            .zip(&to.resources)
            .enumerate()
            .all(|(index, (left, right))| {
                if index == owner {
                    *left == expected_from && *right == expected_to
                } else {
                    left == right
                }
            })
}

fn validate_capacity_ceilings(capacities: &Capacities) -> Result<(), DescriptorError> {
    if [
        capacities.request,
        capacities.execute_response,
        capacities.frame,
        capacities.decision,
        capacities.action_evidence,
        capacities.candidate_receipt,
    ]
    .iter()
    .any(|value| *value == 0 || *value > MAX_WIRE_BYTES)
        || capacities.event_count == 0
        || capacities.event_count > MAX_EVENT_COUNT
        || capacities.dictionary_bytes == 0
        || capacities.dictionary_bytes > MAX_DICTIONARY_BYTES
        || capacities.dictionary_entries == 0
        || capacities.dictionary_entries > MAX_DICTIONARY_ENTRIES
        || capacities.resource_count == 0
        || capacities.resource_count > MAX_RESOURCES
        || capacities.checkpoint_count == 0
        || capacities.checkpoint_count > MAX_CHECKPOINTS
        || capacities.graph_work_units == 0
        || capacities.graph_work_units > MAX_GRAPH_WORK_UNITS
        || capacities.active_frames == 0
        || capacities.active_frames > MAX_ACTIVE_FRAMES
        || capacities.quarantined_frames == 0
        || capacities.quarantined_frames > MAX_QUARANTINED_FRAMES
        || capacities.instance_reserved_bytes == 0
        || capacities.instance_reserved_bytes > MAX_INSTANCE_RESERVED_BYTES
    {
        return Err(DescriptorError::NonCanonical);
    }
    Ok(())
}

fn validate_exact_capacities(
    capacities: &Capacities,
    parameters: &[Parameter],
) -> Result<(), DescriptorError> {
    let mut request = REQUEST_FIXED_BYTES;
    for parameter in parameters {
        request = request
            .checked_add(match parameter {
                Parameter::Scalar {
                    kind: ScalarKind::I64,
                    ..
                } => REQUEST_I64_BYTES,
                Parameter::Scalar {
                    kind: ScalarKind::Bool,
                    ..
                } => REQUEST_BOOL_BYTES,
                Parameter::Owned { .. } => REQUEST_OWNER_BYTES,
            })
            .ok_or(DescriptorError::Malformed)?;
    }
    let execute_response = capacities
        .event_count
        .checked_mul(EXECUTE_RESPONSE_EVENT_BYTES)
        .and_then(|events| EXECUTE_RESPONSE_FIXED_BYTES.checked_add(events))
        .ok_or(DescriptorError::Malformed)?;
    let frame = capacities
        .resource_count
        .checked_mul(FRAME_RESOURCE_BYTES)
        .and_then(|resources| FRAME_FIXED_BYTES.checked_add(resources))
        .ok_or(DescriptorError::Malformed)?;
    let candidate_receipt = capacities
        .resource_count
        .checked_mul(CANDIDATE_RECEIPT_RESOURCE_BYTES)
        .and_then(|resources| CANDIDATE_RECEIPT_FIXED_BYTES.checked_add(resources))
        .ok_or(DescriptorError::Malformed)?;
    let per_active = request
        .checked_add(execute_response)
        .and_then(|value| value.checked_add(frame))
        .and_then(|value| value.checked_add(DECISION_BYTES))
        .and_then(|value| value.checked_add(ACTION_EVIDENCE_BYTES))
        .and_then(|value| value.checked_add(candidate_receipt))
        .and_then(|value| value.checked_add(HOST_RECEIPT_BYTES))
        .ok_or(DescriptorError::Malformed)?;
    let retained_frames = CANONICAL_ACTIVE_FRAMES
        .checked_add(CANONICAL_QUARANTINED_FRAMES)
        .ok_or(DescriptorError::Malformed)?;
    let instance_reserved = retained_frames
        .checked_mul(per_active)
        .ok_or(DescriptorError::Malformed)?;
    if capacities.request != request
        || capacities.execute_response != execute_response
        || capacities.frame != frame
        || capacities.decision != DECISION_BYTES
        || capacities.action_evidence != ACTION_EVIDENCE_BYTES
        || capacities.candidate_receipt != candidate_receipt
        || capacities.active_frames != CANONICAL_ACTIVE_FRAMES
        || capacities.quarantined_frames != CANONICAL_QUARANTINED_FRAMES
        || capacities.instance_reserved_bytes != instance_reserved
    {
        return Err(DescriptorError::NonCanonical);
    }
    Ok(())
}

fn validate_cross_bindings(
    fingerprints: &Fingerprints,
    function: &str,
    parameters: &[Parameter],
    result: &ResultShape,
    capacities: &Capacities,
    graph: &SettlementGraph,
) -> Result<(), DescriptorError> {
    if graph.function != function
        || graph.recovery_contract != fingerprints.recovery_contract
        || graph.execution_cleanup != fingerprints.execution_cleanup
        || graph.trace_path_certificate != fingerprints.trace_path_certificate
    {
        return Err(DescriptorError::ArtifactMismatch);
    }
    let owned_count = parameters
        .iter()
        .filter(|parameter| matches!(parameter, Parameter::Owned { .. }))
        .count();
    if owned_count != graph.resource_count || owned_count != capacities.resource_count as usize {
        return Err(DescriptorError::ArtifactMismatch);
    }
    for checkpoint in &graph.checkpoints {
        match (*result, checkpoint.outcome) {
            (_, None | Some(Outcome::SemanticFailure)) => {}
            (ResultShape::ScalarI64, Some(Outcome::ScalarSuccess)) => {}
            (
                ResultShape::OwnedInput { owner_ordinal, .. },
                Some(Outcome::OwnedSuccess(graph_owner)),
            ) if owner_ordinal == graph_owner as usize => {}
            _ => return Err(DescriptorError::ArtifactMismatch),
        }
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DescriptorError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(DescriptorError::Malformed)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(DescriptorError::Malformed)?;
        self.offset = end;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<u32, DescriptorError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| DescriptorError::Malformed)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn usize(&mut self) -> Result<usize, DescriptorError> {
        usize::try_from(self.u32()?).map_err(|_| DescriptorError::Malformed)
    }

    fn fingerprint(&mut self) -> Result<[u8; 32], DescriptorError> {
        self.take(32)?
            .try_into()
            .map_err(|_| DescriptorError::Malformed)
    }

    fn text(&mut self, max: usize) -> Result<String, DescriptorError> {
        let length = self.usize()?;
        if length == 0 || length > max {
            return Err(DescriptorError::NonCanonical);
        }
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes).map_err(|_| DescriptorError::Malformed)?;
        if value.contains('\0') {
            return Err(DescriptorError::NonCanonical);
        }
        Ok(value.to_owned())
    }

    fn ordinals(&mut self, max: usize) -> Result<Vec<u32>, DescriptorError> {
        let count = self.usize()?;
        if count > max || count > self.remaining() / 4 {
            return Err(DescriptorError::NonCanonical);
        }
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            let ordinal = self.u32()?;
            if ordinal as usize >= max {
                return Err(DescriptorError::NonCanonical);
            }
            values.push(ordinal);
        }
        Ok(values)
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), DescriptorError> {
        self.u32(u32::try_from(value).map_err(|_| DescriptorError::Malformed)?);
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), DescriptorError> {
        self.usize(value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }

    fn ordinals(&mut self, values: &[u32]) -> Result<(), DescriptorError> {
        self.usize(values.len())?;
        for value in values {
            self.u32(*value);
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(crate) fn encode_graph(graph: &SettlementGraph) -> Result<Vec<u8>, DescriptorError> {
    let mut writer = Writer::new();
    writer.u32(GRAPH_VERSION);
    writer.text(&graph.function)?;
    writer.bytes(&graph.recovery_contract);
    writer.bytes(&graph.execution_cleanup);
    writer.bytes(&graph.trace_path_certificate);
    writer.usize(graph.resource_count)?;
    writer.usize(graph.checkpoints.len())?;
    for checkpoint in &graph.checkpoints {
        writer.u32(checkpoint.id);
        writer.usize(checkpoint.resources.len())?;
        for state in &checkpoint.resources {
            writer.u32(match state {
                ResourceState::Live => 1,
                ResourceState::ProvisionalResult => 2,
                ResourceState::Finalizing => 3,
                ResourceState::Dead => 4,
                ResourceState::Published => 5,
            });
        }
        match checkpoint.outcome {
            None => writer.u32(0),
            Some(Outcome::ScalarSuccess) => writer.u32(1),
            Some(Outcome::SemanticFailure) => writer.u32(2),
            Some(Outcome::OwnedSuccess(owner)) => {
                writer.u32(3);
                writer.u32(owner);
            }
        }
        writer.ordinals(&checkpoint.abort_order)?;
        writer.ordinals(&checkpoint.accept_order)?;
    }
    writer.ordinals(&graph.starts)?;
    writer.usize(graph.edges.len())?;
    for edge in &graph.edges {
        writer.u32(edge.from);
        writer.u32(edge.to);
        match &edge.action {
            Action::Finalize(owner) => {
                writer.u32(1);
                writer.u32(*owner);
            }
            Action::StageOwnedResult(owner) => {
                writer.u32(2);
                writer.u32(*owner);
            }
            Action::CertifyOutcome(evidence) => {
                writer.u32(3);
                writer.bytes(&evidence.digest);
                writer.usize(evidence.ordinals.len())?;
                for ordinal in &evidence.ordinals {
                    writer.u32(*ordinal);
                }
                match evidence.outcome {
                    TraceOutcome::ScalarSuccess => writer.u32(1),
                    TraceOutcome::OwnedSuccess => writer.u32(2),
                    TraceOutcome::Failure { selected_ordinal } => {
                        writer.u32(3);
                        writer.u32(selected_ordinal);
                    }
                }
            }
        }
    }
    Ok(writer.finish())
}

pub(crate) fn encode_descriptor(descriptor: &Descriptor) -> Result<Vec<u8>, DescriptorError> {
    let mut writer = Writer::new();
    writer.bytes(MAGIC);
    writer.u32(VERSION);
    writer.u32(HEADER_SIZE);
    writer.u32(0);
    writer.text(&descriptor.target)?;
    writer.u32(linkage_tag(descriptor.linkage));
    for fingerprint in descriptor.fingerprints.iter() {
        writer.bytes(fingerprint);
    }
    writer.text(&descriptor.module)?;
    writer.text(&descriptor.function)?;
    writer.text(&descriptor.getter_symbol)?;
    writer.text(&descriptor.execute_symbol)?;
    writer.text(&descriptor.settle_symbol)?;
    writer.u32(descriptor.call_abi_tag);
    writer.u32(descriptor.obligations);
    for capacity in capacity_values(&descriptor.capacities) {
        writer.u32(capacity);
    }
    encode_signature(&mut writer, &descriptor.parameters, &descriptor.result)?;
    let graph = encode_graph(&descriptor.graph)?;
    writer.usize(graph.len())?;
    writer.bytes(&graph);
    let mut bytes = writer.finish();
    let total = u32::try_from(bytes.len()).map_err(|_| DescriptorError::Malformed)?;
    bytes[16..20].copy_from_slice(&total.to_le_bytes());
    Ok(bytes)
}

fn encode_signature(
    writer: &mut Writer,
    parameters: &[Parameter],
    result: &ResultShape,
) -> Result<(), DescriptorError> {
    writer.usize(parameters.len())?;
    for parameter in parameters {
        match parameter {
            Parameter::Scalar { index, value, kind } => {
                writer.u32(PARAMETER_SCALAR);
                writer.usize(*index)?;
                writer.text(value)?;
                writer.u32(match kind {
                    ScalarKind::I64 => SCALAR_I64,
                    ScalarKind::Bool => SCALAR_BOOL,
                });
            }
            Parameter::Owned {
                index,
                value,
                owner_ordinal,
                resource,
                lifecycle,
                payload_wire_kind,
            } => {
                writer.u32(PARAMETER_OWNED_RESOURCE);
                writer.usize(*index)?;
                writer.text(value)?;
                writer.usize(*owner_ordinal)?;
                writer.text(resource)?;
                writer.text(lifecycle)?;
                writer.u32(*payload_wire_kind);
            }
        }
    }
    match result {
        ResultShape::ScalarI64 => writer.u32(RESULT_SCALAR_I64),
        ResultShape::OwnedInput {
            parameter_index,
            owner_ordinal,
        } => {
            writer.u32(RESULT_OWNED_INPUT);
            writer.usize(*parameter_index)?;
            let value = match &parameters[*parameter_index] {
                Parameter::Owned { value, .. } => value,
                Parameter::Scalar { .. } => return Err(DescriptorError::NonCanonical),
            };
            writer.text(value)?;
            writer.usize(*owner_ordinal)?;
        }
    }
    Ok(())
}

fn current_target_tag() -> Result<String, DescriptorError> {
    let endian = if cfg!(target_endian = "little") {
        "little"
    } else {
        "big"
    };
    let environment = if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "apple"
    } else {
        return Err(DescriptorError::WrongTarget);
    };
    let object = if cfg!(windows) {
        "coff"
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        "macho"
    } else if cfg!(any(target_os = "linux", target_os = "android")) {
        "elf"
    } else {
        return Err(DescriptorError::WrongTarget);
    };
    if cfg!(target_os = "ios") {
        let variant = if cfg!(target_abi = "macabi") {
            "catalyst"
        } else if cfg!(target_abi = "sim") {
            "simulator"
        } else {
            "device"
        };
        Ok(format!(
            "{}-ios-{variant}-{environment}-{object}-ptr{}-{endian}-callable-v3",
            std::env::consts::ARCH,
            usize::BITS
        ))
    } else {
        Ok(format!(
            "{}-{}-{environment}-{object}-ptr{}-{endian}-callable-v3",
            std::env::consts::ARCH,
            std::env::consts::OS,
            usize::BITS
        ))
    }
}

const fn linkage_tag(linkage: Linkage) -> u32 {
    match linkage {
        Linkage::Dynamic => LINKAGE_DYNAMIC,
        Linkage::IosStatic => LINKAGE_IOS_STATIC,
    }
}

fn schema_fingerprint() -> [u8; 32] {
    domain_hash(SCHEMA_DOMAIN, &[DESCRIPTOR_SCHEMA_STATEMENT])
}

fn target_fingerprint(target: &[u8]) -> [u8; 32] {
    domain_hash(TARGET_DOMAIN, &[target])
}

fn physical_module_fingerprint(
    schema: &[u8; 32],
    target: &[u8; 32],
    semantic_module: &[u8; 32],
    module: &[u8],
    linkage: Linkage,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PHYSICAL_MODULE_DOMAIN);
    for field in [schema.as_slice(), target, semantic_module, module] {
        hash_field(&mut hasher, field);
    }
    hash_u32(&mut hasher, linkage_tag(linkage));
    hasher.finalize().into()
}

fn graph_fingerprint(bytes: &[u8]) -> [u8; 32] {
    domain_hash(GRAPH_DOMAIN, &[bytes])
}

fn trace_evidence_fingerprint(
    trace_certificate: &[u8; 32],
    ordinals: &[u32],
    outcome: TraceOutcome,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TRACE_EVIDENCE_DOMAIN);
    hasher.update(trace_certificate);
    hasher.update((ordinals.len() as u64).to_le_bytes());
    for ordinal in ordinals {
        hash_u32(&mut hasher, *ordinal);
    }
    match outcome {
        TraceOutcome::ScalarSuccess => hasher.update([1]),
        TraceOutcome::OwnedSuccess => hasher.update([2]),
        TraceOutcome::Failure { selected_ordinal } => {
            hasher.update([3]);
            hash_u32(&mut hasher, selected_ordinal);
        }
    }
    hasher.finalize().into()
}

fn request_schema_fingerprint() -> [u8; 32] {
    domain_hash(REQUEST_SCHEMA_DOMAIN, &[REQUEST_SCHEMA_STATEMENT])
}

fn execute_response_schema_fingerprint() -> [u8; 32] {
    domain_hash(
        EXECUTE_RESPONSE_SCHEMA_DOMAIN,
        &[EXECUTE_RESPONSE_SCHEMA_STATEMENT],
    )
}

fn frame_schema_fingerprint() -> [u8; 32] {
    domain_hash(FRAME_SCHEMA_DOMAIN, &[FRAME_SCHEMA_STATEMENT])
}

fn decision_schema_fingerprint() -> [u8; 32] {
    domain_hash(DECISION_SCHEMA_DOMAIN, &[DECISION_SCHEMA_STATEMENT])
}

fn action_schema_fingerprint() -> [u8; 32] {
    domain_hash(ACTION_SCHEMA_DOMAIN, &[ACTION_SCHEMA_STATEMENT])
}

fn candidate_receipt_schema_fingerprint() -> [u8; 32] {
    domain_hash(
        CANDIDATE_RECEIPT_SCHEMA_DOMAIN,
        &[CANDIDATE_RECEIPT_SCHEMA_STATEMENT],
    )
}

fn committed_receipt_schema_fingerprint() -> [u8; 32] {
    domain_hash(
        COMMITTED_RECEIPT_SCHEMA_DOMAIN,
        &[COMMITTED_RECEIPT_SCHEMA_STATEMENT],
    )
}

fn call_abi_fingerprint() -> [u8; 32] {
    domain_hash(CALL_ABI_DOMAIN, &[CALL_ABI_STATEMENT])
}

#[allow(clippy::too_many_arguments)]
fn call_contract_fingerprint(
    target: &str,
    linkage: Linkage,
    fingerprints: &Fingerprints,
    module: &str,
    function: &str,
    capacities: &Capacities,
    parameters: &[Parameter],
    result: &ResultShape,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CALL_CONTRACT_DOMAIN);
    for bytes in [
        target.as_bytes(),
        fingerprints.schema.as_slice(),
        &fingerprints.target,
        &fingerprints.semantic_module,
        &fingerprints.physical_module,
        &fingerprints.function_template,
        &fingerprints.execution_cleanup,
        &fingerprints.event_dictionary,
        &fingerprints.trace_path_certificate,
        &fingerprints.recovery_contract,
        &fingerprints.settlement_graph,
        &fingerprints.request_schema,
        &fingerprints.execute_response_schema,
        &fingerprints.frame_schema,
        &fingerprints.decision_schema,
        &fingerprints.action_schema,
        &fingerprints.candidate_receipt_schema,
        &fingerprints.committed_receipt_schema,
        &fingerprints.call_abi,
        module.as_bytes(),
        function.as_bytes(),
    ] {
        hash_field(&mut hasher, bytes);
    }
    hash_u32(&mut hasher, linkage_tag(linkage));
    hash_u32(&mut hasher, CALL_ABI_TAG);
    hash_u32(&mut hasher, REQUIRED_OBLIGATIONS);
    for capacity in capacity_values(capacities) {
        hash_u32(&mut hasher, capacity);
    }
    hash_signature(&mut hasher, parameters, result);
    hasher.finalize().into()
}

fn hash_signature(hasher: &mut Sha256, parameters: &[Parameter], result: &ResultShape) {
    hash_u32(hasher, parameters.len() as u32);
    for parameter in parameters {
        match parameter {
            Parameter::Scalar { index, value, kind } => {
                hash_u32(hasher, PARAMETER_SCALAR);
                hash_u32(hasher, *index as u32);
                hash_field(hasher, value.as_bytes());
                hash_u32(
                    hasher,
                    match kind {
                        ScalarKind::I64 => SCALAR_I64,
                        ScalarKind::Bool => SCALAR_BOOL,
                    },
                );
            }
            Parameter::Owned {
                index,
                value,
                owner_ordinal,
                resource,
                lifecycle,
                payload_wire_kind,
            } => {
                hash_u32(hasher, PARAMETER_OWNED_RESOURCE);
                hash_u32(hasher, *index as u32);
                hash_field(hasher, value.as_bytes());
                hash_u32(hasher, *owner_ordinal as u32);
                hash_field(hasher, resource.as_bytes());
                hash_field(hasher, lifecycle.as_bytes());
                hash_u32(hasher, *payload_wire_kind);
            }
        }
    }
    match result {
        ResultShape::ScalarI64 => hash_u32(hasher, RESULT_SCALAR_I64),
        ResultShape::OwnedInput {
            parameter_index,
            owner_ordinal,
        } => {
            hash_u32(hasher, RESULT_OWNED_INPUT);
            hash_u32(hasher, *parameter_index as u32);
            let value = match &parameters[*parameter_index] {
                Parameter::Owned { value, .. } => value,
                Parameter::Scalar { .. } => return,
            };
            hash_field(hasher, value.as_bytes());
            hash_u32(hasher, *owner_ordinal as u32);
        }
    }
}

fn derive_symbols(fingerprints: &Fingerprints) -> (String, String, String) {
    let mut hasher = Sha256::new();
    hasher.update(SYMBOL_SEED_DOMAIN);
    for fingerprint in [
        &fingerprints.physical_module,
        &fingerprints.function_template,
        &fingerprints.recovery_contract,
        &fingerprints.settlement_graph,
        &fingerprints.request_schema,
        &fingerprints.execute_response_schema,
        &fingerprints.frame_schema,
        &fingerprints.decision_schema,
        &fingerprints.action_schema,
        &fingerprints.candidate_receipt_schema,
        &fingerprints.committed_receipt_schema,
        &fingerprints.call_abi,
        &fingerprints.call_contract,
    ] {
        hash_field(&mut hasher, fingerprint);
    }
    let seed: [u8; 32] = hasher.finalize().into();
    (
        derive_symbol(GETTER_SYMBOL_DOMAIN, &seed, "descriptor_v3"),
        derive_symbol(EXECUTE_SYMBOL_DOMAIN, &seed, "execute_v3"),
        derive_symbol(SETTLE_SYMBOL_DOMAIN, &seed, "settle_v3"),
    )
}

fn derive_symbol(domain: &[u8], seed: &[u8; 32], suffix: &str) -> String {
    let digest = domain_hash(domain, &[seed]);
    let mut symbol = String::with_capacity(4 + 48 + 1 + suffix.len());
    symbol.push_str("spx_");
    for byte in &digest[..24] {
        use std::fmt::Write as _;
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.push('_');
    symbol.push_str(suffix);
    symbol
}

fn capacity_values(capacities: &Capacities) -> [u32; 15] {
    [
        capacities.request,
        capacities.execute_response,
        capacities.frame,
        capacities.decision,
        capacities.action_evidence,
        capacities.candidate_receipt,
        capacities.event_count,
        capacities.dictionary_bytes,
        capacities.dictionary_entries,
        capacities.resource_count,
        capacities.checkpoint_count,
        capacities.graph_work_units,
        capacities.active_frames,
        capacities.quarantined_frames,
        capacities.instance_reserved_bytes,
    ]
}

fn is_c_symbol(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn domain_hash(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for field in fields {
        hash_field(&mut hasher, field);
    }
    hasher.finalize().into()
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hash_u32(hasher: &mut Sha256, value: u32) {
    hasher.update(value.to_le_bytes());
}

#[cfg(test)]
#[path = "descriptor_v3/tests.rs"]
mod tests;
