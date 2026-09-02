use super::*;
#[test]
fn kats_are_frozen() {
    assert_eq!(encode(1, 1, 1), HANDLE_KAT);
    assert_eq!(decode(HANDLE_KAT), Ok((1, 1, 1)));
    assert_eq!(status(1), 0x0000_002d_0000_0001);
    assert_eq!(mem::size_of::<PrivateAppleSwiftEvidenceV1>(), 64);
}
#[test]
fn reserved_handles_reject() {
    for handle in [0, 1, 1 << 24, 1 << 48, 1 << 63] {
        assert_eq!(decode(handle), Err(TableError::Invalid));
    }
}
#[test]
fn stale_cross_capacity_and_generation_are_closed() {
    let mut first = Table::new(1);
    let h = first.insert(7).unwrap();
    assert_eq!(first.claim(h), Ok(7));
    first.consume(h);
    assert_eq!(first.claim(h), Err(TableError::Stale));
    let fresh = first.insert(7).unwrap();
    assert_ne!(h, fresh);
    let mut second: Table<u64> = Table::new(2);
    assert_eq!(second.claim(fresh), Err(TableError::Cross));
    for _ in 1..CAPACITY {
        first.insert(9).unwrap();
    }
    assert_eq!(first.insert(9), Err(TableError::Capacity));
}
#[test]
fn quarantine_and_close_inventory_are_absorbing() {
    let mut table = Table::new(1);
    let h = table.insert(1).unwrap();
    table.claim(h).unwrap();
    table.quarantine(h);
    assert!(table.quarantined());
    assert!(!table.empty());
    assert_eq!(table.insert(2), Err(TableError::Draining));
}

#[test]
fn equal_payloads_keep_distinct_identity_and_restore_is_exact() {
    let mut table = Table::new(1);
    let first = table.insert(7).unwrap();
    let second = table.insert(7).unwrap();
    assert_ne!(first, second);
    assert_eq!(table.claim(first), Ok(7));
    table.restore(first);
    assert_eq!(table.claim(first), Ok(7));
}

#[test]
fn generation_exhaustion_retires_without_wraparound() {
    let mut table: Table<u64> = Table::new(1);
    table.slots[0].generation = MASK as u32;
    table.slots[0].state = State::Consumed;
    let handle = table.insert(5).unwrap();
    assert_eq!(decode(handle).unwrap().2, 2);
    assert!(matches!(table.slots[0].state, State::Retired));
}

#[test]
fn evidence_and_terminal_status_kats_are_exact() {
    let evidence = PrivateAppleSwiftEvidenceV1 {
        words: [1, 7, 15, 0, 2, (1_u64 << 32) | 13, 11, 0],
    };
    assert_eq!(
        evidence.words,
        [1, 7, 15, 0, 2, 0x0000_0001_0000_000d, 11, 0]
    );
    assert_eq!(status(CODE_EVIDENCE), 0x0000_002d_8000_0004);
    assert_eq!(status(CODE_PANIC), 0x0000_002d_8000_0002);
}

#[test]
fn requires_false_witness_constants_are_frozen() {
    assert_eq!(REQUIRES_FALSE_PAYLOAD, u64::MAX);
    assert_eq!(REQUIRES_FALSE_SELECTED_ORDINAL, 1);
    assert_eq!(OWNER_GENERATION, 1);
    assert_eq!(status(CODE_WRONG_PAYLOAD), 0x0000_002d_0000_000c);
    let evidence = PrivateAppleSwiftEvidenceV1 {
        words: [
            1,
            7,
            u64::from(REQUIRES_FALSE_SELECTED_ORDINAL),
            0,
            1,
            REQUIRES_FALSE_PAYLOAD,
            0,
            0,
        ],
    };
    assert_eq!(evidence.words, [1, 7, 1, 0, 1, 0xffff_ffff_ffff_ffff, 0, 0]);
}

#[test]
fn witness_sessions_are_shaped_and_distinct_from_pairs() {
    assert_ne!(SessionShape::Pair, SessionShape::SingleWitness);
    assert_ne!(SessionShape::Pair, SessionShape::SingleOwnedResult);
    assert_ne!(SessionShape::SingleWitness, SessionShape::SingleOwnedResult);
    let witness = Session {
        shape: SessionShape::SingleWitness,
        payloads: [REQUIRES_FALSE_PAYLOAD, 0],
    };
    assert_eq!(witness.payloads[0], u64::MAX);
    assert_eq!(witness.shape, SessionShape::SingleWitness);
    let owned = Session {
        shape: SessionShape::SingleOwnedResult,
        payloads: [IDENTITY_MAX_PAYLOAD, 0],
    };
    assert_eq!(owned.payloads[0], u64::MAX);
    assert_eq!(owned.shape, SessionShape::SingleOwnedResult);
}

#[test]
fn identity_max_witness_constants_are_frozen() {
    assert_eq!(IDENTITY_MAX_PAYLOAD, u64::MAX);
    assert_eq!(IDENTITY_MAX_OWNER_ORDINAL, 0);
    assert_eq!(IDENTITY_MAX_PUBLICATIONS, 2);
    let evidence = PrivateAppleSwiftEvidenceV1 {
        words: [1, 7, IDENTITY_MAX_PUBLICATIONS, 0, 0, 0, 0, 0],
    };
    assert_eq!(evidence.words, [1, 7, 2, 0, 0, 0, 0, 0]);
}

#[test]
fn checked_add_overflow_witness_constants_are_frozen() {
    assert_eq!(CHECKED_ADD_OVERFLOW_PAYLOAD, u64::MAX);
    assert_eq!(CHECKED_ADD_OVERFLOW_I64, i64::MAX);
    assert_eq!(CHECKED_ADD_OVERFLOW_SELECTED_ORDINAL, 2);
    let evidence = PrivateAppleSwiftEvidenceV1 {
        words: [
            1,
            7,
            u64::from(CHECKED_ADD_OVERFLOW_SELECTED_ORDINAL),
            0,
            1,
            CHECKED_ADD_OVERFLOW_PAYLOAD,
            0,
            0,
        ],
    };
    assert_eq!(evidence.words, [1, 7, 2, 0, 1, 0xffff_ffff_ffff_ffff, 0, 0]);
}

#[test]
fn ensures_false_witness_constants_are_frozen() {
    assert_eq!(ENSURES_FALSE_PAYLOAD, u64::MAX);
    assert_eq!(ENSURES_FALSE_SELECTED_ORDINAL, 3);
    let evidence = PrivateAppleSwiftEvidenceV1 {
        words: [
            1,
            7,
            u64::from(ENSURES_FALSE_SELECTED_ORDINAL),
            0,
            1,
            ENSURES_FALSE_PAYLOAD,
            0,
            0,
        ],
    };
    assert_eq!(evidence.words, [1, 7, 3, 0, 1, 0xffff_ffff_ffff_ffff, 0, 0]);
}

#[test]
fn witness_shapes_are_distinct() {
    assert_ne!(SessionShape::Pair, SessionShape::CheckedAddOverflow);
    assert_ne!(
        SessionShape::SingleWitness,
        SessionShape::CheckedAddOverflow
    );
    assert_ne!(
        SessionShape::SingleOwnedResult,
        SessionShape::CheckedAddOverflow
    );
    assert_ne!(SessionShape::Pair, SessionShape::EnsuresFalse);
    assert_ne!(SessionShape::SingleWitness, SessionShape::EnsuresFalse);
    assert_ne!(SessionShape::SingleOwnedResult, SessionShape::EnsuresFalse);
    assert_ne!(SessionShape::CheckedAddOverflow, SessionShape::EnsuresFalse);
}
