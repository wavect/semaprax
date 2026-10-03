use super::*;

// Original arithmetic, independently rebuilt without the retained forecast.
fn uncached_future(context: &FoldContextV8, folded: &FoldV8) -> Result<RoomV8, SourceJournalError> {
    let Some(remaining) = fold::cumulative::remaining_turns(context, folded)? else {
        return Ok(RoomV8::default());
    };
    let max = templates::maxima(context)?;
    let one = fresh_turn_room_uncached(context, &max)?;
    let turns = RoomV8 {
        bytes: one
            .bytes
            .checked_mul(remaining as usize)
            .ok_or(SourceJournalError::Capacity)?,
        rows: one
            .rows
            .checked_mul(remaining as usize)
            .ok_or(SourceJournalError::Capacity)?,
    };
    cleanup(&max, OwnerV8::State, &max.state_operations)?
        .add(RoomV8 {
            bytes: super::super::super::super::execution::TERMINAL_ROOM_BYTES,
            rows: 2,
        })?
        .add(turns)
}

#[test]
fn owned_future_template_cache_preserves_remaining_turns_and_exact_capacity_edges() {
    let mut context = super::super::super::fold::tests::context();
    context.cumulative_initialization = true;
    context.ordinary.max_iterations = 3;
    for turn in 0..3 {
        let folded = FoldV8::capacity_fresh_turn(turn);
        let expected = uncached_future(&context, &folded).unwrap();
        for _ in 0..2 {
            assert_eq!(future(&context, &folded).unwrap(), expected);
        }
        let byte_limit = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES;
        let row_limit = super::super::super::super::MAX_SOURCE_ENTRIES;
        expected
            .check(byte_limit - expected.bytes, row_limit - expected.rows)
            .unwrap();
        assert_eq!(
            expected.check(byte_limit - expected.bytes + 1, row_limit - expected.rows),
            Err(SourceJournalError::Capacity)
        );
        assert_eq!(
            expected.check(byte_limit - expected.bytes, row_limit - expected.rows + 1),
            Err(SourceJournalError::Capacity)
        );
    }
    let invalid = FoldV8::capacity_fresh_turn(3);
    assert_eq!(
        future(&context, &invalid),
        Err(SourceJournalError::Capacity)
    );
    assert_eq!(
        future(&context, &invalid),
        uncached_future(&context, &invalid)
    );
}

#[test]
fn owned_future_template_cache_recomputes_changed_response_bound_and_preserves_refusal() {
    let mut context = super::super::super::fold::tests::context();
    context.cumulative_initialization = true;
    let folded = FoldV8::capacity_fresh_turn(0);
    let old = future(&context, &folded).unwrap();
    context.ordinary.response_limit += 1;
    let fresh = uncached_future(&context, &folded).unwrap();
    assert!(fresh.bytes > old.bytes);
    assert_eq!(future(&context, &folded).unwrap(), fresh);
    context.ordinary.profile = crate::live_invocation::source_journal::SourceProfile::PrimitiveV1;
    assert_eq!(future(&context, &folded), Err(SourceJournalError::Binding));
    assert_eq!(
        future(&context, &folded),
        uncached_future(&context, &folded)
    );
    context.future_templates.reset();
    assert!(context.future_templates.entry.borrow().is_none());
}

#[cfg(unix)]
#[test]
fn owned_future_template_cache_rejects_crossed_proof_and_resets_at_profile_selection() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
        |context, lease, _key| {
            let max = templates::maxima(context.fold()).unwrap();
            fresh_turn_room(context.fold(), &max).unwrap();
            assert!(context.fold().future_templates.entry.borrow().is_some());
            let plan = std::sync::Arc::clone(context.fold().checked_reduce().unwrap());
            let context = context.with_cumulative_initialization(&lease).unwrap();
            assert!(context.fold().future_templates.entry.borrow().is_none());
            let folded = FoldV8::capacity_fresh_turn(0);
            assert_eq!(
                future(context.fold(), &folded),
                uncached_future(context.fold(), &folded)
            );
            let mut crossed = super::super::super::fold::tests::context();
            crossed.cumulative_initialization = true;
            let max = templates::maxima(&crossed).unwrap();
            fresh_turn_room(&crossed, &max).unwrap();
            crossed.checked_reduce = Ok(plan);
            assert_eq!(
                fresh_turn_room(&crossed, &max),
                Err(SourceJournalError::Binding)
            );
        },
    );
}

#[test]
fn owned_maxima_template_cache_reuses_its_binding_and_preserves_crossed_proof_refusal() {
    let mut context = super::super::super::fold::tests::context();
    let expected = templates::maxima(&context).unwrap();
    for _ in 0..3 {
        assert_eq!(templates::maxima(&context).unwrap(), expected);
    }
    let cached = context.maxima_templates.entry.borrow();
    let (binding, retained) = cached.as_ref().unwrap();
    assert!(std::sync::Arc::ptr_eq(binding, &context.checked_binding));
    assert_eq!(retained.as_ref().unwrap(), &expected);
    drop(cached);

    // A capacity proof retained for a different checked binding cannot turn a
    // crossed Reduce proof into an admissible continued profile.
    let crossed = super::super::super::fold::tests::context();
    context.checked_binding = std::sync::Arc::clone(&crossed.checked_binding);
    context.cumulative_initialization = true;
    assert_eq!(
        future(&context, &FoldV8::capacity_fresh_turn(0)),
        Err(SourceJournalError::Binding)
    );
}
