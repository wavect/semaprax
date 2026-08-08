//! Private callable-v2 settlement-proof serialization.
//!
//! This is an authority-free proof envelope. It embeds the exact callable-v2
//! descriptor and a canonical, pointer-free binary projection of the compiler's
//! settlement certificate. The embedded v2 bytes retain their existing v2
//! symbols, but the proof envelope adds no symbol, provider, capability,
//! loader operation, physical finalizer, or default public API.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "settlement proof remains compiler-private data")
)]

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedProgram};
use crate::native_settlement::{
    NativeSettlementCertificate, SettlementOutcome, SettlementProgressAction,
    SettlementResourceState,
};

use super::{emit_native_callable_admission_core, native_settlement_derivation};

const MAGIC: &[u8; 8] = b"SPXNPRF1";
const VERSION: u32 = 1;
const HEADER_SIZE: u32 = 20;
const MAX_PROOF_BYTES: usize = 64 * 1024;
const FINGERPRINT_BYTES: usize = 32;
const FIXED_PROOF_BYTES: usize = 20 + 4 * FINGERPRINT_BYTES + 4 + 4;

const GRAPH_VERSION: u32 = 1;
const STATE_LIVE: u32 = 1;
const STATE_PROVISIONAL_RESULT: u32 = 2;
const STATE_FINALIZING: u32 = 3;
const STATE_DEAD: u32 = 4;
const STATE_PUBLISHED: u32 = 5;
const OUTCOME_NONE: u32 = 0;
const OUTCOME_SCALAR_SUCCESS: u32 = 1;
const OUTCOME_SEMANTIC_FAILURE: u32 = 2;
const OUTCOME_OWNED_SUCCESS: u32 = 3;
const ACTION_FINALIZE: u32 = 1;
const ACTION_STAGE_OWNED_RESULT: u32 = 2;
const ACTION_CERTIFY_OUTCOME: u32 = 3;

const SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-schema.v1\0";
const V2_BYTES_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-v2-bytes.v1\0";
const GRAPH_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-graph.v1\0";
const ENVELOPE_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-envelope.v1\0";
const SCHEMA_STATEMENT: &[u8] = b"SPXNPRF1;u32le;header=20;body=schema32,v2_hash32,graph_hash32,envelope_hash32,v2_len32,v2_bytes,graph_len32,graph_bytes";

/// Compiler-private bytes. Possessing or decoding these bytes confers no
/// authority to load code or finalize a resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeCallableSettlementProof {
    bytes: Vec<u8>,
}

impl NativeCallableSettlementProof {
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Serialize one validated callable-v2 contract and its independently derived
/// settlement graph. Both derivations must agree on the exact trace proof.
pub(super) fn derive(
    program: &ResolvedProgram,
    function_id: &DeclarationId,
) -> Result<NativeCallableSettlementProof, Diagnostic> {
    let v2 = emit_native_callable_admission_core(program, function_id)?;
    let settlement = native_settlement_derivation::derive_native_settlement(program, function_id)?;
    require_matching_trace_certificates(
        v2.trace_path_certificate().fingerprint(),
        settlement.trace_certificate_fingerprint(),
    )?;

    let graph_budget = graph_budget(v2.descriptor().len())?;
    let graph = encode_graph(
        settlement.certificate(),
        v2.call_contract(),
        settlement.trace_certificate_fingerprint(),
        graph_budget,
    )?;
    encode_proof(v2.descriptor(), &graph)
}

fn graph_budget(v2_descriptor_bytes: usize) -> Result<usize, Diagnostic> {
    FIXED_PROOF_BYTES
        .checked_add(v2_descriptor_bytes)
        .and_then(|used| MAX_PROOF_BYTES.checked_sub(used))
        .filter(|remaining| *remaining > 0)
        .ok_or_else(|| {
            proof_error(format!(
                "callable-v2 descriptor leaves no room under the {MAX_PROOF_BYTES}-byte proof limit"
            ))
        })
}

fn require_matching_trace_certificates(
    callable_v2: [u8; FINGERPRINT_BYTES],
    settlement: [u8; FINGERPRINT_BYTES],
) -> Result<(), Diagnostic> {
    if callable_v2 != settlement {
        return Err(proof_error(
            "callable-v2 and settlement derivations disagree on the trace certificate",
        ));
    }
    Ok(())
}

fn encode_proof(
    v2_descriptor: &[u8],
    graph: &[u8],
) -> Result<NativeCallableSettlementProof, Diagnostic> {
    let available_graph_bytes = graph_budget(v2_descriptor.len())?;
    if graph.len() > available_graph_bytes {
        return Err(proof_error(format!(
            "settlement graph exceeds its {available_graph_bytes}-byte proof budget"
        )));
    }
    let v2_len = wire_u32(v2_descriptor.len(), "callable-v2 descriptor byte length")?;
    let graph_len = wire_u32(graph.len(), "settlement graph byte length")?;
    let schema = framed_fingerprint(SCHEMA_DOMAIN, SCHEMA_STATEMENT);
    let v2_fingerprint = framed_fingerprint(V2_BYTES_DOMAIN, v2_descriptor);
    let graph_fingerprint = framed_fingerprint(GRAPH_DOMAIN, graph);
    let envelope = envelope_fingerprint(
        &schema,
        &v2_fingerprint,
        &graph_fingerprint,
        v2_len,
        graph_len,
    );

    let total = usize::try_from(HEADER_SIZE)
        .expect("header size fits usize")
        .checked_add(FINGERPRINT_BYTES * 4)
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_add(v2_descriptor.len()))
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_add(graph.len()))
        .ok_or_else(|| proof_error("proof byte length overflow"))?;
    if total > MAX_PROOF_BYTES {
        return Err(proof_error(format!(
            "callable settlement proof exceeds the {MAX_PROOF_BYTES}-byte limit"
        )));
    }
    let total = wire_u32(total, "callable settlement-proof byte length")?;

    let mut bytes = Vec::with_capacity(total as usize);
    bytes.extend_from_slice(MAGIC);
    push_u32(&mut bytes, VERSION);
    push_u32(&mut bytes, HEADER_SIZE);
    push_u32(&mut bytes, total);
    bytes.extend_from_slice(&schema);
    bytes.extend_from_slice(&v2_fingerprint);
    bytes.extend_from_slice(&graph_fingerprint);
    bytes.extend_from_slice(&envelope);
    push_u32(&mut bytes, v2_len);
    bytes.extend_from_slice(v2_descriptor);
    push_u32(&mut bytes, graph_len);
    bytes.extend_from_slice(graph);
    debug_assert_eq!(bytes.len(), total as usize);

    // Fail closed if a future edit makes the encoder disagree with its own
    // canonical proof-envelope rules.
    validate_proof(&bytes)?;
    Ok(NativeCallableSettlementProof { bytes })
}

fn encode_graph(
    certificate: &NativeSettlementCertificate,
    source_v2_call_contract: [u8; FINGERPRINT_BYTES],
    trace_path_certificate_fingerprint: [u8; FINGERPRINT_BYTES],
    byte_budget: usize,
) -> Result<Vec<u8>, Diagnostic> {
    for (fingerprint, label) in [
        (
            source_v2_call_contract,
            "source callable-v2 call-contract fingerprint",
        ),
        (
            trace_path_certificate_fingerprint,
            "trace-path certificate fingerprint",
        ),
    ] {
        if fingerprint.iter().all(|byte| *byte == 0) {
            return Err(proof_error(format!("{label} must be nonzero")));
        }
    }
    let mut graph = GraphWriter::new(byte_budget);
    graph.u32(GRAPH_VERSION)?;
    push_text(
        &mut graph,
        certificate.function().as_str(),
        "settlement function identity",
    )?;
    graph.raw(&certificate.recovery_contract())?;
    graph.raw(&source_v2_call_contract)?;
    graph.raw(&trace_path_certificate_fingerprint)?;
    push_count(
        &mut graph,
        certificate.resource_count(),
        "settlement resource count",
    )?;
    push_count(
        &mut graph,
        certificate.checkpoints().len(),
        "settlement checkpoint count",
    )?;
    for checkpoint in certificate.checkpoints() {
        graph.u32(checkpoint.checkpoint())?;
        push_count(
            &mut graph,
            checkpoint.resources().len(),
            "checkpoint resource-state count",
        )?;
        for state in checkpoint.resources() {
            graph.u32(match state {
                SettlementResourceState::Live => STATE_LIVE,
                SettlementResourceState::ProvisionalResult => STATE_PROVISIONAL_RESULT,
                SettlementResourceState::Finalizing => STATE_FINALIZING,
                SettlementResourceState::Dead => STATE_DEAD,
                SettlementResourceState::Published => STATE_PUBLISHED,
            })?;
        }
        match checkpoint.normal_outcome() {
            None => graph.u32(OUTCOME_NONE)?,
            Some(SettlementOutcome::ScalarSuccess) => {
                graph.u32(OUTCOME_SCALAR_SUCCESS)?;
            }
            Some(SettlementOutcome::SemanticFailure) => {
                graph.u32(OUTCOME_SEMANTIC_FAILURE)?;
            }
            Some(SettlementOutcome::OwnedSuccess { owner_ordinal }) => {
                graph.u32(OUTCOME_OWNED_SUCCESS)?;
                graph.u32(owner_ordinal)?;
            }
        }
        push_ordinals(
            &mut graph,
            checkpoint.abort_cleanup_order(),
            "abort cleanup order",
        )?;
        push_ordinals(
            &mut graph,
            checkpoint.accept_cleanup_order(),
            "accept cleanup order",
        )?;
    }
    push_ordinals(
        &mut graph,
        certificate.start_checkpoints(),
        "settlement start checkpoints",
    )?;
    push_count(
        &mut graph,
        certificate.progress_edges().len(),
        "settlement progress-edge count",
    )?;
    for edge in certificate.progress_edges() {
        graph.u32(edge.from())?;
        graph.u32(edge.to())?;
        match edge.action() {
            SettlementProgressAction::Finalize { owner_ordinal } => {
                graph.u32(ACTION_FINALIZE)?;
                graph.u32(owner_ordinal)?;
            }
            SettlementProgressAction::StageOwnedResult { owner_ordinal } => {
                graph.u32(ACTION_STAGE_OWNED_RESULT)?;
                graph.u32(owner_ordinal)?;
            }
            SettlementProgressAction::CertifyOutcome { trace_evidence } => {
                graph.u32(ACTION_CERTIFY_OUTCOME)?;
                graph.raw(&trace_evidence)?;
            }
        }
    }
    Ok(graph.finish())
}

struct GraphWriter {
    bytes: Vec<u8>,
    byte_budget: usize,
}

impl GraphWriter {
    fn new(byte_budget: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(byte_budget),
            byte_budget,
        }
    }

    fn raw(&mut self, value: &[u8]) -> Result<(), Diagnostic> {
        let next = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| proof_error("settlement graph byte length overflow"))?;
        if next > self.byte_budget {
            return Err(proof_error(format!(
                "settlement graph exceeds its {}-byte proof budget",
                self.byte_budget
            )));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u32(&mut self, value: u32) -> Result<(), Diagnostic> {
        self.raw(&value.to_le_bytes())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_ordinals(bytes: &mut GraphWriter, values: &[u32], label: &str) -> Result<(), Diagnostic> {
    push_count(bytes, values.len(), label)?;
    for value in values {
        bytes.u32(*value)?;
    }
    Ok(())
}

fn push_count(bytes: &mut GraphWriter, count: usize, label: &str) -> Result<(), Diagnostic> {
    bytes.u32(wire_u32(count, label)?)
}

fn push_text(bytes: &mut GraphWriter, value: &str, label: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.as_bytes().contains(&0) {
        return Err(proof_error(format!(
            "{label} must be nonempty and NUL-free"
        )));
    }
    push_count(bytes, value.len(), label)?;
    bytes.raw(value.as_bytes())
}

fn wire_u32(value: usize, label: &str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| proof_error(format!("{label} exceeds u32")))
}

fn framed_fingerprint(domain: &[u8], payload: &[u8]) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_be_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

fn envelope_fingerprint(
    schema: &[u8; FINGERPRINT_BYTES],
    v2: &[u8; FINGERPRINT_BYTES],
    graph: &[u8; FINGERPRINT_BYTES],
    v2_len: u32,
    graph_len: u32,
) -> [u8; FINGERPRINT_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(ENVELOPE_DOMAIN);
    for field in [schema.as_slice(), v2.as_slice(), graph.as_slice()] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    for field in [v2_len.to_le_bytes(), graph_len.to_le_bytes()] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field);
    }
    hasher.finalize().into()
}

fn validate_proof(bytes: &[u8]) -> Result<(), Diagnostic> {
    if bytes.len() > MAX_PROOF_BYTES || bytes.len() < HEADER_SIZE as usize + 136 {
        return Err(proof_error("proof length is outside canonical bounds"));
    }
    if &bytes[..8] != MAGIC {
        return Err(proof_error("proof magic is not SPXNPRF1"));
    }
    let version = read_u32(bytes, 8)?;
    let header = read_u32(bytes, 12)?;
    let declared = read_u32(bytes, 16)?;
    if version != VERSION || header != HEADER_SIZE || declared as usize != bytes.len() {
        return Err(proof_error("proof envelope is not canonical"));
    }
    let schema: [u8; 32] = bytes[20..52]
        .try_into()
        .expect("validated proof has fixed fingerprint fields");
    let v2_fingerprint: [u8; 32] = bytes[52..84].try_into().expect("fixed fingerprint field");
    let graph_fingerprint: [u8; 32] = bytes[84..116].try_into().expect("fixed fingerprint field");
    let envelope: [u8; 32] = bytes[116..148].try_into().expect("fixed fingerprint field");
    if schema != framed_fingerprint(SCHEMA_DOMAIN, SCHEMA_STATEMENT) {
        return Err(proof_error("proof schema fingerprint mismatch"));
    }
    let v2_len = read_u32(bytes, 148)?;
    let v2_start = 152_usize;
    let v2_end = v2_start
        .checked_add(v2_len as usize)
        .ok_or_else(|| proof_error("callable-v2 descriptor length overflow"))?;
    let graph_len_offset = v2_end;
    let graph_start = graph_len_offset
        .checked_add(4)
        .ok_or_else(|| proof_error("settlement graph offset overflow"))?;
    if graph_start > bytes.len() {
        return Err(proof_error("callable-v2 descriptor is truncated"));
    }
    let graph_len = read_u32(bytes, graph_len_offset)?;
    let graph_end = graph_start
        .checked_add(graph_len as usize)
        .ok_or_else(|| proof_error("settlement graph length overflow"))?;
    if graph_end != bytes.len() {
        return Err(proof_error("settlement graph length is not exact"));
    }
    if v2_len == 0 || graph_len == 0 {
        return Err(proof_error("embedded artifacts must be nonempty"));
    }
    if v2_fingerprint != framed_fingerprint(V2_BYTES_DOMAIN, &bytes[v2_start..v2_end]) {
        return Err(proof_error("callable-v2 descriptor fingerprint mismatch"));
    }
    if graph_fingerprint != framed_fingerprint(GRAPH_DOMAIN, &bytes[graph_start..graph_end]) {
        return Err(proof_error("settlement graph fingerprint mismatch"));
    }
    if envelope
        != envelope_fingerprint(
            &schema,
            &v2_fingerprint,
            &graph_fingerprint,
            v2_len,
            graph_len,
        )
    {
        return Err(proof_error("proof envelope fingerprint mismatch"));
    }
    Ok(())
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| proof_error("proof offset overflow"))?;
    let field = bytes
        .get(offset..end)
        .ok_or_else(|| proof_error("proof is truncated"))?;
    Ok(u32::from_le_bytes(
        field.try_into().expect("four-byte slice"),
    ))
}

fn proof_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(
        "SPX-I105",
        format!("native callable settlement proof: {}", message.into()),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fmt::Write as _;

    use crate::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().fold(
            String::with_capacity(bytes.len() * 2),
            |mut output, byte| {
                write!(output, "{byte:02x}").expect("writing to a string cannot fail");
                output
            },
        )
    }

    #[test]
    fn all_fourteen_corpus_cases_derive_identical_bounded_bytes() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        assert_eq!(corpus.cases.len(), 14);
        let mut exact_by_function = BTreeSet::new();
        for case in &corpus.cases {
            let function = DeclarationId::new(case.function_id);
            let first = derive(&corpus.program, &function).unwrap();
            let second = derive(&corpus.program, &function).unwrap();
            assert_eq!(first, second, "{}", case.scenario_id);
            assert!(first.bytes().len() <= MAX_PROOF_BYTES);
            validate_proof(first.bytes()).unwrap();
            exact_by_function.insert((case.function_id, first.bytes().to_vec()));
        }
        assert_eq!(exact_by_function.len(), 7);
    }

    #[test]
    fn proof_embeds_exact_v2_and_binary_graph_without_new_authority_fields() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let function = DeclarationId::new("token.discard-two");
        let proof = derive(&corpus.program, &function).unwrap();
        let v2 = emit_native_callable_admission_core(&corpus.program, &function).unwrap();
        let bytes = proof.bytes();
        let v2_len = read_u32(bytes, 148).unwrap() as usize;
        assert_eq!(&bytes[152..152 + v2_len], v2.descriptor());
        let graph_start = 156 + v2_len;
        assert_eq!(
            read_u32(bytes, graph_start - 4).unwrap(),
            (bytes.len() - graph_start) as u32
        );
        assert_eq!(read_u32(bytes, graph_start).unwrap(), GRAPH_VERSION);
        let function_bytes = read_u32(bytes, graph_start + 4).unwrap() as usize;
        let recovery_start = graph_start + 8 + function_bytes;
        let call_contract_start = recovery_start + FINGERPRINT_BYTES;
        let trace_start = call_contract_start + FINGERPRINT_BYTES;
        let settlement =
            native_settlement_derivation::derive_native_settlement(&corpus.program, &function)
                .unwrap();
        assert_eq!(
            &bytes[recovery_start..call_contract_start],
            &settlement.recovery_contract_fingerprint()
        );
        assert_eq!(
            &bytes[call_contract_start..trace_start],
            &v2.call_contract()
        );
        assert_eq!(
            &bytes[trace_start..trace_start + FINGERPRINT_BYTES],
            &settlement.trace_certificate_fingerprint()
        );
        assert!(!bytes[graph_start..].starts_with(b"{"));
        assert!(!bytes[graph_start..]
            .windows(b"callable_symbol".len())
            .any(|window| window == b"callable_symbol"));
        assert!(!bytes[graph_start..]
            .windows(b"semaprax_native_callable".len())
            .any(|window| window == b"semaprax_native_callable"));
        assert!(!bytes[graph_start..]
            .windows(b"capability".len())
            .any(|window| window == b"capability"));
    }

    #[test]
    fn every_proof_byte_mutation_or_truncation_fails_closed() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        let proof = derive(&corpus.program, &DeclarationId::new("token.discard-two")).unwrap();
        for length in 0..proof.bytes().len() {
            assert!(validate_proof(&proof.bytes()[..length]).is_err());
        }
        for index in 0..proof.bytes().len() {
            let mut mutated = proof.bytes().to_vec();
            mutated[index] ^= 0x01;
            assert!(validate_proof(&mutated).is_err(), "accepted byte {index}");
        }
        let mut trailing = proof.bytes().to_vec();
        trailing.push(0);
        assert!(validate_proof(&trailing).is_err());
    }

    #[test]
    fn proof_limit_and_empty_artifacts_are_composition_errors() {
        assert_eq!(encode_proof(&[], &[1]).unwrap_err().code, "SPX-I105");
        assert_eq!(encode_proof(&[1], &[]).unwrap_err().code, "SPX-I105");
        let boundary_graph = vec![1; MAX_PROOF_BYTES - 157];
        assert_eq!(
            encode_proof(&[1], &boundary_graph).unwrap().bytes().len(),
            MAX_PROOF_BYTES
        );
        let too_large = vec![1; MAX_PROOF_BYTES - 156];
        assert_eq!(encode_proof(&[1], &too_large).unwrap_err().code, "SPX-I105");
    }

    #[test]
    fn graph_writer_accepts_the_exact_budget_and_rejects_the_first_extra_byte() {
        let mut writer = GraphWriter::new(4);
        writer.u32(0x0403_0201).unwrap();
        assert_eq!(writer.bytes, [1, 2, 3, 4]);
        assert_eq!(writer.raw(&[5]).unwrap_err().code, "SPX-I105");
        assert_eq!(writer.bytes, [1, 2, 3, 4]);

        assert_eq!(graph_budget(MAX_PROOF_BYTES).unwrap_err().code, "SPX-I105");
        assert_eq!(
            graph_budget(MAX_PROOF_BYTES - FIXED_PROOF_BYTES - 1).unwrap(),
            1
        );

        let corpus = build_owned_resource_corpus_v1().unwrap();
        let function = DeclarationId::new("token.discard-two");
        let v2 = emit_native_callable_admission_core(&corpus.program, &function).unwrap();
        let settlement =
            native_settlement_derivation::derive_native_settlement(&corpus.program, &function)
                .unwrap();
        let exact = encode_graph(
            settlement.certificate(),
            v2.call_contract(),
            settlement.trace_certificate_fingerprint(),
            MAX_PROOF_BYTES,
        )
        .unwrap();
        assert_eq!(
            encode_graph(
                settlement.certificate(),
                v2.call_contract(),
                settlement.trace_certificate_fingerprint(),
                exact.len(),
            )
            .unwrap(),
            exact
        );
        assert_eq!(
            encode_graph(
                settlement.certificate(),
                v2.call_contract(),
                settlement.trace_certificate_fingerprint(),
                exact.len() - 1,
            )
            .unwrap_err()
            .code,
            "SPX-I105"
        );
    }

    #[test]
    fn trace_mismatch_is_a_composition_error() {
        assert!(require_matching_trace_certificates([7; 32], [7; 32]).is_ok());
        assert_eq!(
            require_matching_trace_certificates([7; 32], [8; 32])
                .unwrap_err()
                .code,
            "SPX-I105"
        );

        let corpus = build_owned_resource_corpus_v1().unwrap();
        let settlement = native_settlement_derivation::derive_native_settlement(
            &corpus.program,
            &DeclarationId::new("token.discard-two"),
        )
        .unwrap();
        assert_eq!(
            encode_graph(settlement.certificate(), [0; 32], [1; 32], 4096)
                .unwrap_err()
                .code,
            "SPX-I105"
        );
        assert_eq!(
            encode_graph(settlement.certificate(), [1; 32], [0; 32], 4096)
                .unwrap_err()
                .code,
            "SPX-I105"
        );
    }

    #[test]
    fn proof_and_discard_two_graph_have_exact_known_answers() {
        let fixture = encode_proof(b"SPXNABI2fixture", b"\x01\0\0\0graph").unwrap();
        assert_eq!(fixture.bytes().len(), 180);
        assert_eq!(
            hex(&Sha256::digest(fixture.bytes())),
            "fffcad5a28200f01c533f0f8ff45f5b2169c1936e861df01c3dae5a68de1d7b3"
        );

        let corpus = build_owned_resource_corpus_v1().unwrap();
        let function = DeclarationId::new("token.discard-two");
        let settlement =
            native_settlement_derivation::derive_native_settlement(&corpus.program, &function)
                .unwrap();
        let graph = encode_graph(
            settlement.certificate(),
            [0x51; FINGERPRINT_BYTES],
            [0x52; FINGERPRINT_BYTES],
            MAX_PROOF_BYTES,
        )
        .unwrap();
        assert_eq!(
            hex(&Sha256::digest(&graph)),
            "a54630347381e7709ccc4ed499056b372557a2ea8aa3f2894720fb5f5357831e"
        );
    }
}
