use super::*;
use crate::conformance::{
    Retryability, StatusClass, ARITHMETIC_STATUS_DOMAIN_V1, CONTRACT_ENSURES_FALSE_CODE,
    CONTRACT_REQUIRES_FALSE_CODE, CONTRACT_STATUS_DOMAIN_V1, NORMALIZED_STATUS_SCHEMA_V1,
};

fn arena(nonce: u64, capacity: u32) -> StatusArena {
    StatusArena::new(StatusContextId::new(nonce), capacity).unwrap()
}

#[test]
fn zero_is_success_and_never_resolves_to_a_record() {
    let arena = arena(7, 1);
    assert!(StatusToken::SUCCESS.is_success());
    assert_eq!(StatusToken::SUCCESS.raw(), 0);
    assert_eq!(
        arena.resolve_local(StatusToken::SUCCESS),
        Err(StatusArenaError::SuccessHasNoRecord)
    );
}

#[test]
fn records_receive_immutable_one_based_tokens_in_insertion_order() {
    let mut arena = arena(7, 3);
    let first = arena.record_arithmetic(StatusCase::AddOverflow).unwrap();
    let second = arena.record_contract(ContractPhase::Ensures).unwrap();

    assert_eq!(first.raw(), 1);
    assert_eq!(second.raw(), 2);
    assert_eq!(
        arena.resolve(first),
        Ok(&normalize_arithmetic(StatusCase::AddOverflow))
    );
    assert_eq!(
        arena.resolve(second),
        Ok(&normalize_contract(ContractPhase::Ensures))
    );
    assert_eq!(arena.len(), 2);
}

#[test]
fn equal_records_still_receive_distinct_stable_tokens() {
    let mut arena = arena(8, 2);
    let first = arena.record_arithmetic(StatusCase::MulOverflow).unwrap();
    let second = arena.record_arithmetic(StatusCase::MulOverflow).unwrap();
    assert_eq!((first.raw(), second.raw()), (1, 2));
    assert_eq!(arena.resolve(first), arena.resolve(second));
}

#[test]
fn exhaustion_is_a_non_mutating_harness_error() {
    let mut arena = arena(9, 1);
    let first = arena.record_arithmetic(StatusCase::SubOverflow).unwrap();
    let before = arena.resolve(first).unwrap().clone();

    assert_eq!(
        arena.record_contract(ContractPhase::Requires),
        Err(StatusArenaError::Exhausted { capacity: 1 })
    );
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.resolve(first), Ok(&before));
}

#[test]
fn zero_capacity_fails_without_creating_a_language_status() {
    let mut arena = arena(10, 0);
    assert_eq!(
        arena.record_arithmetic(StatusCase::DivisionByZero),
        Err(StatusArenaError::Exhausted { capacity: 0 })
    );
    assert!(arena.is_empty());
}

#[test]
fn scoped_tokens_cannot_cross_contexts_even_when_raw_indices_match() {
    let mut left = arena(11, 1);
    let mut right = arena(12, 1);
    let left_token = left
        .record_arithmetic(StatusCase::DivisionOverflow)
        .unwrap();
    let right_token = right.record_contract(ContractPhase::Requires).unwrap();
    assert_eq!(left_token.raw(), right_token.raw());
    assert_eq!(
        right.resolve(left_token),
        Err(StatusArenaError::WrongContext {
            expected: StatusContextId::new(12),
            actual: StatusContextId::new(11),
        })
    );
    assert_eq!(
        left.resolve(right_token),
        Err(StatusArenaError::WrongContext {
            expected: StatusContextId::new(11),
            actual: StatusContextId::new(12),
        })
    );
}

#[test]
fn same_context_nonce_does_not_alias_distinct_arenas() {
    let mut first = arena(15, 1);
    let mut second = arena(15, 1);
    let first_token = first.record_arithmetic(StatusCase::AddOverflow).unwrap();
    let second_token = second.record_contract(ContractPhase::Ensures).unwrap();
    assert_eq!(first_token.raw(), second_token.raw());
    assert_eq!(
        second.resolve(first_token),
        Err(StatusArenaError::ForeignArena {
            context: StatusContextId::new(15),
        })
    );
    assert_eq!(
        first.resolve(second_token),
        Err(StatusArenaError::ForeignArena {
            context: StatusContextId::new(15),
        })
    );
}

#[test]
fn unknown_nonzero_tokens_are_rejected() {
    let arena = arena(13, 4);
    let token = StatusToken::from_raw(4);
    assert_eq!(
        arena.resolve_local(token),
        Err(StatusArenaError::UnknownToken { token })
    );
}

#[test]
fn arithmetic_normalization_uses_the_exact_v1_table() {
    let cases = [
        (StatusCase::AddOverflow, 1),
        (StatusCase::SubOverflow, 2),
        (StatusCase::MulOverflow, 3),
        (StatusCase::DivisionByZero, 4),
        (StatusCase::DivisionOverflow, 5),
        (StatusCase::RemainderByZero, 6),
        (StatusCase::RemainderOverflow, 7),
        (StatusCase::NegationOverflow, 8),
    ];
    for (case, code) in cases {
        let status = normalize_arithmetic(case);
        assert_eq!(status.schema(), NORMALIZED_STATUS_SCHEMA_V1);
        assert_eq!(status.domain_id(), ARITHMETIC_STATUS_DOMAIN_V1);
        assert_eq!(status.code(), code);
        assert_eq!(status.class(), StatusClass::Arithmetic);
        assert_eq!(status.retryability(), Retryability::Known(false));
    }
}

#[test]
fn contract_normalization_uses_phase_codes_not_ordinals() {
    let requires = normalize_contract(ContractPhase::Requires);
    let ensures = normalize_contract(ContractPhase::Ensures);
    for status in [&requires, &ensures] {
        assert_eq!(status.schema(), NORMALIZED_STATUS_SCHEMA_V1);
        assert_eq!(status.domain_id(), CONTRACT_STATUS_DOMAIN_V1);
        assert_eq!(status.class(), StatusClass::Contract);
        assert_eq!(status.retryability(), Retryability::Known(false));
    }
    assert_eq!(requires.code(), CONTRACT_REQUIRES_FALSE_CODE);
    assert_eq!(ensures.code(), CONTRACT_ENSURES_FALSE_CODE);
}

#[test]
fn arbitrary_normalized_records_round_trip_without_semantic_rewriting() {
    let mut arena = arena(14, 1);
    let status = NormalizedStatus::try_new(
        "io.error.v1",
        42,
        StatusClass::ExplicitClose,
        Retryability::Known(true),
    )
    .unwrap();
    let token = arena.record(status.clone()).unwrap();
    assert_eq!(arena.resolve(token), Ok(&status));
}
