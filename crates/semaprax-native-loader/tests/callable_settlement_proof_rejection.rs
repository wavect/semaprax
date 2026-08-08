use semaprax_native_loader::{open_admitted_callable_exact, open_admitted_exact, OpenError};

#[test]
fn settlement_proof_is_rejected_by_both_loaders_before_any_library_open() {
    let mut proof = Vec::new();
    proof.extend_from_slice(b"SPXNPRF1");
    proof.extend_from_slice(&1_u32.to_le_bytes());
    proof.extend_from_slice(&20_u32.to_le_bytes());
    proof.extend_from_slice(&20_u32.to_le_bytes());
    let absent_library = std::env::current_dir()
        .unwrap()
        .join("semaprax-callable-proof-must-not-be-opened");

    // SAFETY: Input validation rejects proof envelopes before path
    // canonicalization, loading, symbol lookup, or invocation, so no foreign
    // code is reached.
    let descriptor_only =
        unsafe { open_admitted_exact(&absent_library, b"semaprax_settlement_proof_v1", &proof) };
    let callable = unsafe {
        open_admitted_callable_exact(
            &absent_library,
            b"semaprax_settlement_proof_v1",
            b"semaprax_callable_proof_v1",
            &proof,
        )
    };

    assert!(matches!(
        descriptor_only,
        Err(OpenError::SettlementProofEnvelopeNotLoadable)
    ));
    assert!(matches!(
        callable,
        Err(OpenError::SettlementProofEnvelopeNotLoadable)
    ));
}
