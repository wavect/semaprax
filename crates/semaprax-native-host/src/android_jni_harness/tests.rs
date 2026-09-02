use super::*;

fn table(tag: u16) -> SessionTable<u64> {
    SessionTable::new(tag, 2)
}

#[test]
fn handle_and_status_known_answers_are_exact() {
    assert_eq!(encode_handle(1, 1, 1), HANDLE_KNOWN_ANSWER);
    assert_eq!(decode_handle(HANDLE_KNOWN_ANSWER), Ok((1, 1, 1)));
    assert_eq!(android_status(1), 0x0000_002d_0000_0001);
}

#[test]
fn provider_path_must_arrive_absolute_and_byte_exact_canonical() {
    assert_eq!(
        exact_canonical_provider_path(b"relative/provider.so"),
        Err(android_status(CODE_INVALID_ARGUMENT))
    );
    let canonical = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    let canonical_text = canonical.to_str().unwrap();
    assert_eq!(
        exact_canonical_provider_path(canonical_text.as_bytes()),
        Ok(canonical.clone())
    );
    let redundant = format!("{canonical_text}/.");
    assert_eq!(
        exact_canonical_provider_path(redundant.as_bytes()),
        Err(android_status(CODE_PROVIDER_ADMISSION))
    );
}

#[test]
fn reserved_handle_shapes_fail_closed() {
    for invalid in [
        0,
        1,
        1_u64 << HANDLE_GENERATION_SHIFT,
        1_u64 << HANDLE_TAG_SHIFT,
        1_u64 << 63,
    ] {
        assert_eq!(decode_handle(invalid), Err(TableError::Invalid));
    }
}

#[test]
fn claim_restore_consume_stale_and_cross_runtime_are_exact() {
    let mut first = table(1);
    let handle = first.insert(7).unwrap();
    assert_eq!(first.claim(handle), Ok(7));
    assert_eq!(first.claim(handle), Err(TableError::Invalid));
    first.restore(handle);
    assert_eq!(first.claim(handle), Ok(7));
    first.consume(handle);
    assert_eq!(first.claim(handle), Err(TableError::Stale));
    let refreshed = first.insert(7).unwrap();
    assert_ne!(refreshed, handle);
    assert_eq!(first.claim(handle), Err(TableError::Stale));
    first.restore(refreshed);

    let mut second = table(2);
    let _ = second.insert(7).unwrap();
    assert_eq!(second.claim(refreshed), Err(TableError::CrossRuntime));
}

#[test]
fn equal_payloads_have_distinct_identity_and_capacity_is_bounded() {
    let mut table = table(1);
    let first = table.insert(9).unwrap();
    let second = table.insert(9).unwrap();
    assert_ne!(first, second);
    assert_eq!(table.insert(9), Err(TableError::Capacity));
}

#[test]
fn generation_exhaustion_retires_without_wraparound() {
    let mut table = table(1);
    table.slots[0].generation = HANDLE_FIELD_MASK as u32;
    table.slots[0].state = SlotState::Consumed;
    let handle = table.insert(5).unwrap();
    assert_eq!(decode_handle(handle).unwrap().2, 2);
    assert!(matches!(table.slots[0].state, SlotState::Retired));
}

#[test]
fn quarantine_and_drain_are_absorbing() {
    let mut table = table(1);
    let handle = table.insert(5).unwrap();
    table.claim(handle).unwrap();
    table.quarantine(handle);
    assert!(table.has_quarantine());
    assert_eq!(table.insert(6), Err(TableError::Draining));
    assert_eq!(table.claim(handle), Err(TableError::Draining));
    assert!(!table.is_empty());
}

#[test]
fn panic_poisoning_quarantines_every_claimed_session() {
    let mut table = SessionTable::new(1, 3);
    let first = table.insert(11_u64).unwrap();
    let second = table.insert(13_u64).unwrap();
    let live = table.insert(17_u64).unwrap();
    assert_eq!(table.claim(first), Ok(11));
    assert_eq!(table.claim(second), Ok(13));

    table.quarantine_all_claimed();

    assert!(table.has_quarantine());
    assert_eq!(table.claim(first), Err(TableError::Draining));
    assert_eq!(table.claim(second), Err(TableError::Draining));
    assert_eq!(table.claim(live), Err(TableError::Draining));
    assert_eq!(table.insert(19), Err(TableError::Draining));
    assert!(matches!(table.slots[0].state, SlotState::Quarantined));
    assert!(matches!(table.slots[1].state, SlotState::Quarantined));
    assert!(matches!(table.slots[2].state, SlotState::Live(17)));
}

#[test]
fn boundary_poisoning_quarantines_live_and_claimed_sessions() {
    let mut table = SessionTable::new(1, 3);
    let first = table.insert(11_u64).unwrap();
    let _live = table.insert(13_u64).unwrap();
    assert_eq!(table.claim(first), Ok(11));

    table.quarantine_all_active();

    assert!(matches!(table.slots[0].state, SlotState::Quarantined));
    assert!(matches!(table.slots[1].state, SlotState::Quarantined));
    assert!(table.has_quarantine());
    assert!(table.draining);
}

#[test]
fn exact_output_abi_remains_frozen() {
    assert_eq!(mem::size_of::<PrivateAndroidJniEvidenceV1>(), 40);
    assert_eq!(
        mem::offset_of!(PrivateAndroidJniEvidenceV1, module_instance_id),
        8
    );
    assert_eq!(
        mem::offset_of!(PrivateAndroidJniEvidenceV1, host_state_flags),
        32
    );
}

#[test]
fn owner_pair_counter_exhaustion_is_nonmutating() {
    let mut next = u64::MAX - 1;
    assert_eq!(
        take_owner_pair(&mut next),
        Err(android_status(CODE_CAPACITY))
    );
    assert_eq!(next, u64::MAX - 1);
    let mut next = 7;
    assert_eq!(take_owner_pair(&mut next), Ok([7, 8]));
    assert_eq!(next, 9);
}

#[test]
fn owner_single_counter_exhaustion_is_nonmutating() {
    let mut next = u64::MAX;
    assert_eq!(
        take_owner_slot(&mut next),
        Err(android_status(CODE_CAPACITY))
    );
    assert_eq!(next, u64::MAX);
    let mut next = 7;
    assert_eq!(take_owner_slot(&mut next), Ok(7));
    assert_eq!(next, 8);
}

#[test]
fn requires_false_witness_constants_are_frozen() {
    assert_eq!(REQUIRES_FALSE_PAYLOAD, u64::MAX);
    assert_eq!(REQUIRES_FALSE_SELECTED_ORDINAL, 1);
    assert_eq!(OWNER_GENERATION, 1);
}

#[test]
fn identity_max_witness_constants_are_frozen() {
    assert_eq!(IDENTITY_MAX_PAYLOAD, u64::MAX);
    assert_eq!(IDENTITY_MAX_OWNER_ORDINAL, 0);
    assert_eq!(IDENTITY_MAX_PUBLICATIONS, 2);
}

#[test]
fn rejected_close_precheck_does_not_drain_live_table() {
    let mut table = table(1);
    let handle = table.insert(7).unwrap();
    assert!(!table.is_empty());
    assert!(!table.draining);
    assert_eq!(table.claim(handle), Ok(7));
    table.restore(handle);
    assert!(!table.draining);
}

#[test]
fn checked_add_overflow_witness_constants_are_frozen() {
    assert_eq!(CHECKED_ADD_OVERFLOW_PAYLOAD, u64::MAX);
    assert_eq!(CHECKED_ADD_OVERFLOW_I64, i64::MAX);
    assert_eq!(CHECKED_ADD_OVERFLOW_SELECTED_ORDINAL, 2);
    assert_eq!(OWNER_GENERATION, 1);
}

#[test]
fn ensures_false_witness_constants_are_frozen() {
    assert_eq!(ENSURES_FALSE_PAYLOAD, u64::MAX);
    assert_eq!(ENSURES_FALSE_SELECTED_ORDINAL, 3);
    assert_eq!(OWNER_GENERATION, 1);
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
