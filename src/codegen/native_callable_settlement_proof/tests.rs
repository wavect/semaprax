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
        native_settlement_derivation::derive_native_settlement(&corpus.program, &function).unwrap();
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
        native_settlement_derivation::derive_native_settlement(&corpus.program, &function).unwrap();
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
        native_settlement_derivation::derive_native_settlement(&corpus.program, &function).unwrap();
    let graph = encode_graph(
        settlement.certificate(),
        [0x51; FINGERPRINT_BYTES],
        [0x52; FINGERPRINT_BYTES],
        MAX_PROOF_BYTES,
    )
    .unwrap();
    assert_eq!(
        hex(&Sha256::digest(&graph)),
        "60c7b31750a6571e77a5e8568d6ea4ee3ce3e599681b88776a846aa5e43e7e32"
    );
}
