use super::*;
fn root(ids: &[u32]) -> Value {
    Value::Record(Arc::new(OwnedRecordValue {
        record: hir::DeclarationId::new("test.provenance"),
        fields: ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                (
                    hir::DeclarationId::new(format!("field.{i}")),
                    Value::Bytes(OwnedBytesValue {
                        allocation: *id,
                        bytes: Arc::from(vec![0u8]),
                    }),
                )
            })
            .collect(),
    }))
}
#[test]
fn owned_frame_v2_provenance_sparse_state_dead_seal_and_fresh_outcome_keep_high_water() {
    let mut state = root(&[1, 2]);
    let mut provenance = OwnedAllocationProvenanceV2::fresh(&state).unwrap();
    let seal = root(&[3]);
    provenance.record_frame(&[&state, &seal], 3).unwrap();
    assert_eq!(provenance.seed(&[&state, &seal]).unwrap(), 3);
    drop(seal);
    let Value::Record(r) = &mut state else {
        panic!()
    };
    drop(
        Arc::get_mut(r)
            .unwrap()
            .fields
            .remove(&hir::DeclarationId::new("field.0")),
    );
    assert_eq!(
        provenance.seed(&[&state]).unwrap(),
        3,
        "dead IDs remain reserved"
    );
    let outcome = root(&[4]);
    provenance.record_frame(&[&state, &outcome], 4).unwrap();
    assert_eq!(provenance.seed(&[&state, &outcome]).unwrap(), 4);
    drop(outcome);
    let next_seal = root(&[5]);
    provenance.record_frame(&[&state, &next_seal], 5).unwrap();
    assert_eq!(provenance.seed(&[&state, &next_seal]).unwrap(), 5);
    assert!(provenance.record_frame(&[&state, &next_seal], 4).is_err());
}
#[test]
fn owned_frame_v2_provenance_different_backing_duplicate_zero_missing_and_reused_dead_id_refuse() {
    let state = root(&[1]);
    let mut provenance = OwnedAllocationProvenanceV2::fresh(&state).unwrap();
    assert!(
        provenance.seed(&[&root(&[1])]).is_err(),
        "same ID different backing"
    );
    assert!(
        provenance.seed(&[&state, &root(&[1])]).is_err(),
        "duplicate live ID"
    );
    assert!(OwnedAllocationProvenanceV2::fresh(&root(&[0])).is_err());
    assert!(
        OwnedAllocationProvenanceV2::fresh(&root(&[2])).is_err(),
        "fresh sparse namespace"
    );
    assert!(provenance.seed(&[]).is_err(), "live owner omitted");
    let seal = root(&[2]);
    provenance.record_frame(&[&state, &seal], 2).unwrap();
    drop(seal);
    assert!(
        provenance.record_frame(&[&state, &root(&[2])], 3).is_err(),
        "dead ID cannot be reminted"
    );
    let foreign = root(&[3]);
    assert!(
        provenance.record_frame(&[&state, &foreign], 2).is_err(),
        "outside actual evaluator high-water"
    );
    assert_eq!(provenance.seed(&[&state]).unwrap(), 2);
}

#[test]
fn owned_frame_v2_provenance_exhausted_high_water_refuses_real_source_allocation() {
    let source = r#"
module owned.provenance.exhaustion;
@id("make") fn make()->Bytes { let seal = [1u8]; bytes_copy(array_as_slice(seal)) }
@id("main") fn main()->i64 { 0 }
"#;
    let program =
        hir::resolve(&crate::check(source, "provenance-exhaustion.spx").unwrap()).unwrap();
    let state = root(&[1]);
    let mut provenance = OwnedAllocationProvenanceV2::fresh(&state).unwrap();
    provenance.next = u32::MAX; // private pure boundary control, no restore grant
    let functions = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&functions),
        BTreeMap::new(),
        &program.declarations,
        100,
        0,
        PreparedCancellation::Never,
    );
    evaluator.next_byte_allocation = provenance.seed(&[&state]).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "make")
        .unwrap();
    assert!(evaluator.call_frame(function, vec![], 0).is_err());
    assert_eq!(evaluator.next_byte_allocation, u32::MAX);
    provenance
        .record_frame(&[&state], evaluator.next_byte_allocation)
        .unwrap();
    assert!(provenance.validate(&[&state]));
}

#[test]
fn owned_frame_v2_accepted_result_mints_after_dead_ids_and_refuses_before_allocation() {
    let state = root(&[1]);
    let mut token = OwnedAllocationProvenanceV2::fresh(&state).unwrap();
    let seal = root(&[2]);
    token.record_frame(&[&state, &seal], 2).unwrap();
    drop(seal);
    let outcome = token.mint_accepted_bytes(&[&state], vec![0, 1, 0]).unwrap();
    let Value::Bytes(bytes) = &outcome else {
        panic!()
    };
    assert_eq!(bytes.allocation, 3);
    assert_eq!(bytes.bytes.as_ref(), &[0, 1, 0]);
    assert_eq!(Arc::strong_count(&bytes.bytes), 1);
    assert_eq!(token.seed(&[&state, &outcome]).unwrap(), 3);
    let before = token.next;
    let payload = vec![2; 1025];
    assert_eq!(
        token
            .mint_accepted_bytes(&[&state, &outcome], payload.clone())
            .unwrap_err()
            .0,
        payload
    );
    assert_eq!(token.next, before);
    assert!(
        token.mint_accepted_bytes(&[&state], vec![]).is_err(),
        "live Outcome cannot be omitted"
    );
    assert_eq!(token.next, before);
    drop(outcome);
    let empty = token.mint_accepted_bytes(&[&state], vec![]).unwrap();
    let Value::Bytes(bytes) = &empty else {
        panic!()
    };
    assert_eq!(bytes.allocation, 4);
    assert!(bytes.bytes.is_empty());
    token.next = u32::MAX;
    assert_eq!(
        token
            .mint_accepted_bytes(&[&state, &empty], vec![7])
            .unwrap_err()
            .0,
        vec![7]
    );
    assert_eq!(token.next, u32::MAX);
}
