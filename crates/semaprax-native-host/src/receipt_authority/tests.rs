use std::fmt::Write;
use std::num::NonZeroU64;

use sha2::{Digest, Sha256};

use super::*;
use crate::callable_wire_v3::{
    CallIdentity, CandidateOutcome, CandidateReceipt, Disposition, DispositionCell,
    RecoveryIdentity,
};

fn authority() -> ReceiptAuthority {
    let mut seed = [0_u8; SEED_BYTES];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = u8::try_from(index + 1).unwrap();
    }
    ReceiptAuthority::from_seed(NonZeroU64::new(91).unwrap(), &mut seed).unwrap()
}

fn candidate(challenge: [u8; 32]) -> CandidateReceipt {
    CandidateReceipt {
        identity: RecoveryIdentity {
            call: CallIdentity {
                call_contract: [0x31; 32],
                invocation: NonZeroU64::new(7).unwrap(),
                frame_generation: NonZeroU64::new(11).unwrap(),
                provider_challenge: challenge,
            },
            recovery_contract: [0x32; 32],
            settlement_graph: [0x33; 32],
        },
        request_digest: [0x41; 32],
        response_storage_digest: [0x42; 32],
        semantic_trace_digest: [0; 32],
        frame_digest: [0x43; 32],
        decision_digest: [0x44; 32],
        action_evidence_digest: [0x45; 32],
        outcome: CandidateOutcome::Abort,
        active_finalizers: 0,
        dispositions: vec![DispositionCell {
            disposition: Disposition::Dead,
            payload: 0x0102_0304_0506_0708,
        }],
    }
}

#[test]
fn deterministic_challenge_and_exact_receipt_kat() {
    let authority = authority();
    let invocation = NonZeroU64::new(7).unwrap();
    let generation = NonZeroU64::new(11).unwrap();
    let challenge = authority
        .provider_challenge([0x31; 32], invocation, generation)
        .unwrap();
    assert_eq!(
        hex(&challenge),
        "677cd5775a7cd54a60dcd3bc7c1c8b36cc4323cbeef09ae18dd8104ca570dc9b"
    );
    let candidate = candidate(challenge);
    let digest = crate::callable_wire_v3::candidate_digest(&candidate.encode());
    let receipt = authority
        .authenticate_receipt(&candidate, digest, [0x51; 32], [0x52; 32])
        .unwrap();
    assert_eq!(receipt.len(), 524);
    assert_eq!(
        hex(&Sha256::digest(receipt)),
        "0a76ade3cf435c207aa1a52a8e09ad7771c0a74f5729e3b82f8413ca78e7dac4"
    );
}

#[test]
fn invalid_halves_fail_without_fallback() {
    for mut seed in [[0_u8; 64], [7_u8; 64]] {
        assert!(matches!(
            ReceiptAuthority::from_seed(NonZeroU64::new(91).unwrap(), &mut seed),
            Err(ReceiptAuthorityError::InvalidEntropy)
        ));
        assert_eq!(seed, [0; 64]);
    }
    let mut zero_key = [9_u8; 64];
    zero_key[..32].fill(0);
    assert!(ReceiptAuthority::from_seed(NonZeroU64::new(91).unwrap(), &mut zero_key).is_err());
    assert_eq!(zero_key, [0; 64]);
    let mut zero_instance = [9_u8; 64];
    zero_instance[32..].fill(0);
    assert!(ReceiptAuthority::from_seed(NonZeroU64::new(91).unwrap(), &mut zero_instance).is_err());
    assert_eq!(zero_instance, [0; 64]);
}

#[test]
fn entropy_source_is_called_once_with_exactly_sixty_four_bytes() {
    let mut calls = 0;
    let authority = ReceiptAuthority::from_fill(NonZeroU64::new(91).unwrap(), |destination| {
        calls += 1;
        assert_eq!(destination.len(), 64);
        for (index, byte) in destination.iter_mut().enumerate() {
            *byte = u8::try_from(index + 1).unwrap();
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(
        authority.instance_binding(),
        [
            33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54,
            55, 56, 57, 58, 59, 60, 61, 62, 63, 64,
        ]
    );
}

#[test]
fn challenges_bind_every_identity_field() {
    let authority = authority();
    let baseline = authority
        .provider_challenge(
            [3; 32],
            NonZeroU64::new(5).unwrap(),
            NonZeroU64::new(7).unwrap(),
        )
        .unwrap();
    assert_ne!(
        baseline,
        authority
            .provider_challenge(
                [4; 32],
                NonZeroU64::new(5).unwrap(),
                NonZeroU64::new(7).unwrap()
            )
            .unwrap()
    );
    assert_ne!(
        baseline,
        authority
            .provider_challenge(
                [3; 32],
                NonZeroU64::new(6).unwrap(),
                NonZeroU64::new(7).unwrap()
            )
            .unwrap()
    );
    assert_ne!(
        baseline,
        authority
            .provider_challenge(
                [3; 32],
                NonZeroU64::new(5).unwrap(),
                NonZeroU64::new(8).unwrap()
            )
            .unwrap()
    );
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        write!(output, "{byte:02x}").unwrap();
        output
    })
}
