use std::collections::BTreeSet;
use std::path::Path;

use semaprax::codegen::emit_native_callable_settlement_proof;
use semaprax::hir::{self, DeclarationId};
use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

use super::*;

const SOURCE: &str = r#"module test.callable_proof_host;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.consume")
fn consume(value: own Token) -> i64 {
    7
}

@id("token.keep")
fn keep(value: own Token) -> Token {
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn compiler_proof(source: &str, function: &str) -> Vec<u8> {
    let parsed = semaprax::parse(source, Path::new("callable-proof-host.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    emit_native_callable_settlement_proof(&resolved, &DeclarationId::new(function))
        .unwrap()
        .bytes()
        .to_vec()
}

fn canonical() -> Vec<u8> {
    compiler_proof(SOURCE, "token.consume")
}

fn components(proof: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let v2_len = u32::from_le_bytes(proof[148..152].try_into().unwrap()) as usize;
    let v2_start = 152;
    let v2_end = v2_start + v2_len;
    let graph_len = u32::from_le_bytes(proof[v2_end..v2_end + 4].try_into().unwrap()) as usize;
    let graph_start = v2_end + 4;
    (
        proof[v2_start..v2_end].to_vec(),
        proof[graph_start..graph_start + graph_len].to_vec(),
    )
}

fn envelope(v2: &[u8], graph: &[u8]) -> Vec<u8> {
    let schema = schema_fingerprint();
    let v2_fingerprint = payload_fingerprint(V2_BYTES_DOMAIN, v2);
    let graph_fingerprint = payload_fingerprint(GRAPH_DOMAIN, graph);
    let envelope_fingerprint = envelope_fingerprint_for(
        &schema,
        &v2_fingerprint,
        &graph_fingerprint,
        v2.len(),
        graph.len(),
    )
    .unwrap();
    let total = 20 + 4 * 32 + 4 + v2.len() + 4 + graph.len();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&HEADER_SIZE.to_le_bytes());
    bytes.extend_from_slice(&(total as u32).to_le_bytes());
    bytes.extend_from_slice(&schema);
    bytes.extend_from_slice(&v2_fingerprint);
    bytes.extend_from_slice(&graph_fingerprint);
    bytes.extend_from_slice(&envelope_fingerprint);
    bytes.extend_from_slice(&(v2.len() as u32).to_le_bytes());
    bytes.extend_from_slice(v2);
    bytes.extend_from_slice(&(graph.len() as u32).to_le_bytes());
    bytes.extend_from_slice(graph);
    bytes
}

fn decoded_components() -> (Vec<u8>, SettlementGraph) {
    let proof = canonical();
    let (v2, graph) = components(&proof);
    (v2, SettlementGraph::parse(&graph).unwrap())
}

#[test]
fn independently_accepts_exact_compiler_proof() {
    let first = canonical();
    let second = canonical();
    assert_eq!(first, second);
    assert_eq!(&first[..8], b"SPXNPRF1");
    let bound = BoundSettlementProof::parse(&first).unwrap();
    assert_eq!(bound.callable_v2.function, "token.consume");
    assert_eq!(bound.graph.function, "token.consume");
    assert_eq!(
        bound.graph.source_v2_call_contract,
        bound.callable_v2.fingerprints.call_contract
    );
    assert_eq!(
        bound.graph.trace_path_certificate_fingerprint,
        bound.callable_v2.fingerprints.trace_path_certificate
    );
    assert_eq!(bound.graph.starts, [1]);
    assert!(bound
        .graph
        .edges
        .iter()
        .any(|edge| matches!(edge.action, Action::Finalize(0))));
    assert!(bound
        .graph
        .edges
        .iter()
        .any(|edge| matches!(edge.action, Action::CertifyOutcome(_))));
    assert_eq!(bound.proof_bytes, first);
}

#[test]
fn independently_accepts_owned_result_staging() {
    let owned = compiler_proof(SOURCE, "token.keep");
    let bound = BoundSettlementProof::parse(&owned).unwrap();
    assert!(matches!(
        bound.callable_v2.result,
        ResultShape::OwnedInput {
            owner_ordinal: 0,
            ..
        }
    ));
    assert!(bound
        .graph
        .edges
        .iter()
        .any(|edge| matches!(edge.action, Action::StageOwnedResult(0))));
    assert!(bound
        .graph
        .checkpoints
        .iter()
        .any(|checkpoint| checkpoint.outcome == Some(Outcome::OwnedSuccess(0))));
}

#[test]
fn independently_accepts_all_fourteen_authoritative_corpus_cases() {
    let corpus = build_owned_resource_corpus_v1().unwrap();
    assert_eq!(corpus.cases.len(), 14);
    let mut functions = BTreeSet::new();
    let mut outcomes = BTreeSet::new();
    for case in &corpus.cases {
        let proof = emit_native_callable_settlement_proof(
            &corpus.program,
            &DeclarationId::new(case.function_id),
        )
        .unwrap();
        let bound = BoundSettlementProof::parse(proof.bytes()).unwrap();
        assert_eq!(bound.callable_v2.function, case.function_id);
        functions.insert(case.function_id);
        for checkpoint in &bound.graph.checkpoints {
            if let Some(outcome) = checkpoint.outcome {
                outcomes.insert(match outcome {
                    Outcome::ScalarSuccess => 1,
                    Outcome::SemanticFailure => 2,
                    Outcome::OwnedSuccess(_) => 3,
                });
            }
        }
    }
    assert_eq!(functions.len(), 7);
    assert_eq!(outcomes, BTreeSet::from([1, 2, 3]));
}

#[test]
fn rejects_every_prefix_truncation_trailing_byte_and_single_byte_mutation() {
    let bytes = canonical();
    for length in 0..bytes.len() {
        assert!(
            BoundSettlementProof::parse(&bytes[..length]).is_err(),
            "accepted prefix length {length}"
        );
    }
    for trailing in [0_u8, 1, 0x7f, 0xff] {
        let mut hostile = bytes.clone();
        hostile.push(trailing);
        assert!(BoundSettlementProof::parse(&hostile).is_err());
    }
    for offset in 0..bytes.len() {
        let mut hostile = bytes.clone();
        hostile[offset] ^= 1;
        assert!(
            BoundSettlementProof::parse(&hostile).is_err(),
            "accepted mutation at {offset}"
        );
    }
}

#[test]
fn rejects_rehashed_hostile_graph_semantics_and_binding_zeros() {
    let (v2, canonical_graph) = decoded_components();
    let mut cases = Vec::new();
    let mut function = canonical_graph.clone();
    function.function = "token.other".to_owned();
    cases.push(function);
    let mut recovery = canonical_graph.clone();
    recovery.recovery_contract = [0; 32];
    cases.push(recovery);
    let mut call_contract = canonical_graph.clone();
    call_contract.source_v2_call_contract = [0; 32];
    cases.push(call_contract);
    let mut trace = canonical_graph.clone();
    trace.trace_path_certificate_fingerprint = [0; 32];
    cases.push(trace);
    let mut resources = canonical_graph.clone();
    resources.resource_count += 1;
    cases.push(resources);
    let mut checkpoint = canonical_graph.clone();
    checkpoint.checkpoints[0].id = 2;
    cases.push(checkpoint);
    let mut state = canonical_graph.clone();
    state.checkpoints[0].resources[0] = ResourceState::Finalizing;
    cases.push(state);
    let mut abort_order = canonical_graph.clone();
    abort_order.checkpoints[0].abort_order.clear();
    cases.push(abort_order);
    let mut start = canonical_graph.clone();
    start.starts = vec![2];
    cases.push(start);
    let mut edge = canonical_graph.clone();
    edge.edges[0].to = edge.edges[0].from;
    cases.push(edge);
    let mut trace_evidence = canonical_graph;
    let certify = trace_evidence
        .edges
        .iter_mut()
        .find(|edge| matches!(edge.action, Action::CertifyOutcome(_)))
        .unwrap();
    certify.action = Action::CertifyOutcome([0; 32]);
    cases.push(trace_evidence);

    for hostile_graph in cases {
        let hostile = envelope(&v2, &encode_graph(&hostile_graph).unwrap());
        assert!(BoundSettlementProof::parse(&hostile).is_err());
    }
}

#[test]
fn rejects_rehashed_unknown_tags_invalid_text_and_hostile_counts() {
    let proof = canonical();
    let (v2, canonical_graph) = components(&proof);
    let function_start = 8;
    let function_len = "token.consume".len();
    let recovery_start = function_start + function_len;
    let resource_count = recovery_start + 32 + 32 + 32;
    let checkpoint_count = resource_count + 4;
    let first_state_tag = checkpoint_count + 4 + 4 + 4;

    let mut cases = Vec::new();
    let mut bad_utf8 = canonical_graph.clone();
    bad_utf8[function_start] = 0xff;
    cases.push(bad_utf8);
    let mut nul = canonical_graph.clone();
    nul[function_start] = 0;
    cases.push(nul);
    let mut resource_overflow = canonical_graph.clone();
    resource_overflow[resource_count..resource_count + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(resource_overflow);
    let mut checkpoint_overflow = canonical_graph.clone();
    checkpoint_overflow[checkpoint_count..checkpoint_count + 4]
        .copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(checkpoint_overflow);
    let mut unknown_state = canonical_graph;
    unknown_state[first_state_tag..first_state_tag + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(unknown_state);

    for graph in cases {
        assert!(BoundSettlementProof::parse(&envelope(&v2, &graph)).is_err());
    }
}

#[test]
fn rejects_rehashed_same_shape_cross_module_graph_swap() {
    let other_module = SOURCE.replace(
        "module test.callable_proof_host;",
        "module test.callable_proof_other;",
    );
    let left = canonical();
    let right = compiler_proof(&other_module, "token.consume");
    let (left_v2, _) = components(&left);
    let (_, right_graph) = components(&right);
    assert_eq!(
        BoundSettlementProof::parse(&envelope(&left_v2, &right_graph)),
        Err(SettlementProofError::ArtifactMismatch)
    );
}

#[test]
fn rejects_rehashed_same_module_function_changed_trace_graph_swap() {
    let changed_trace = SOURCE.replace(
        "fn consume(value: own Token) -> i64 {\n    7\n}",
        "fn consume(value: own Token) -> i64\nrequires true\n{\n    7\n}",
    );
    let left = canonical();
    let right = compiler_proof(&changed_trace, "token.consume");
    let (left_v2, _) = components(&left);
    let (_, right_graph) = components(&right);
    let left_descriptor = DescriptorV2::parse(&left_v2).unwrap();
    let right_v2 = components(&right).0;
    let right_descriptor = DescriptorV2::parse(&right_v2).unwrap();
    assert_ne!(
        left_descriptor.fingerprints.trace_path_certificate,
        right_descriptor.fingerprints.trace_path_certificate
    );
    assert_eq!(
        BoundSettlementProof::parse(&envelope(&left_v2, &right_graph)),
        Err(SettlementProofError::ArtifactMismatch)
    );
}

#[test]
fn rejects_proof_over_exact_global_cap() {
    let hostile = vec![0_u8; MAX_PROOF_BYTES + 1];
    assert_eq!(
        BoundSettlementProof::parse(&hostile),
        Err(SettlementProofError::Malformed)
    );
}
