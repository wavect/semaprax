use super::*;
#[test]
fn owned_reduce_capacity_reserves_actual_stage_and_all_checked_case_closures() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
        |context, _lease, _key| {
            let context = context.fold();
            let rooms = rooms(context).unwrap();
            let plan = v2::compile_owned_reduce_v2(&context.checked_binding).unwrap();
            assert_eq!(rooms.cases.len(), plan.transfers().cases.len());
            assert_eq!(rooms.cases.len(), 2);
            let stage = ordinary(SourceJournalEntry::StageReservation {
                turn: 0,
                attempt: Some(u32::MAX),
                role: super::super::super::super::SourceStageRole::Reduce,
                fuel: context.ordinary.max_steps_per_stage().unwrap(),
            })
            .unwrap();
            let limit = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES;
            let used = limit - rooms.before_stage.bytes;
            rooms.before_stage.check(used, 0).unwrap();
            assert_eq!(
                rooms.before_stage.check(used + 1, 0),
                Err(SourceJournalError::Capacity)
            );
            // Full original stage ACK consumes serialized room without borrowing from
            // any subsequent Step/failure settlement. Its allowance is still spent F.
            rooms.charged.check(used + stage.bytes, 1).unwrap();
            assert_eq!(rooms.before_stage.rows, rooms.charged.rows + 1);
            for c in &rooms.cases {
                assert!(c.staged.rows >= c.transfer.rows);
                assert!(c.transfer.bytes > c.completed.bytes);
                assert!(c.transfer.rows > c.completed.rows);
                assert!(rooms.charged.bytes > c.staged.bytes);
                assert!(rooms.charged.rows > c.staged.rows);
                let used = limit - c.transfer.bytes;
                c.transfer.check(used, 0).unwrap();
                assert_eq!(
                    c.transfer.check(used + 1, 0),
                    Err(SourceJournalError::Capacity)
                );
                c.completed.check(limit - c.completed.bytes, 0).unwrap();
                assert_eq!(
                    c.completed.check(limit - c.completed.bytes + 1, 0),
                    Err(SourceJournalError::Capacity)
                );
            }
            assert!(rooms.failed_state.rows >= 4);
            let before = rooms.after_effect();
            assert!(
                before.bytes >= rooms.before_stage.bytes
                    && before.bytes >= rooms.failed_state.bytes
            );
            assert!(
                before.rows >= rooms.before_stage.rows && before.rows >= rooms.failed_state.rows
            );
            rooms
                .failed_state
                .check(limit - rooms.failed_state.bytes, 0)
                .unwrap();
            assert_eq!(
                rooms
                    .failed_state
                    .check(limit - rooms.failed_state.bytes + 1, 0),
                Err(SourceJournalError::Capacity)
            );
            before.check(limit - before.bytes, 0).unwrap();
            assert_eq!(
                before.check(limit - before.bytes + 1, 0),
                Err(SourceJournalError::Capacity)
            );
        },
    );
}

#[test]
fn failed_state_started_ack_preserves_exact_reserved_closure_edge() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
        |context, _lease, _key| {
            let context = context.fold();
            let plan = v2::compile_owned_reduce_v2(&context.checked_binding).unwrap();
            let operations = v2::owned_wait_operations_v8(
                &context.checked_binding.helper().liveness().result_disposal,
            )
            .unwrap();
            let started = row(json!({
                "kind":"owned_effect_failure_state_cleanup_started","turn":0,
                "attempt":u32::MAX,"plan":plan.binding(),"settlement":u32::MAX,
                "recorded":u32::MAX,"decision_cleanup_settled":u32::MAX,
                "effect_failure":"handler_failed","state_digest":hash(),
                "operations":operations
            }))
            .unwrap();
            // Independently render the frozen receipt shape: it has no Decision
            // digest. The acknowledged Started row must consume only its own room.
            let settled = row(json!({
                "kind":"owned_effect_failure_state_cleanup_settled","turn":0,
                "attempt":u32::MAX,"started":u32::MAX,
                "receipt":templates::receipt(&operations).unwrap()
            }))
            .unwrap();
            assert_eq!(failed_state_receipt(&operations).unwrap(), settled);
            let remaining = failed_state_receipt(&operations)
                .unwrap()
                .add(terminal())
                .unwrap();
            let before = rooms(context).unwrap().failed_state;
            assert_eq!(before, started.add(remaining).unwrap());
            let byte_limit = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES;
            let row_limit = super::super::super::super::MAX_SOURCE_ENTRIES;
            let used_bytes = byte_limit - before.bytes;
            let used_rows = row_limit - before.rows;
            before.check(used_bytes, used_rows).unwrap();
            remaining
                .check(used_bytes + started.bytes, used_rows + started.rows)
                .unwrap();
            assert_eq!(
                remaining.check(used_bytes + started.bytes + 1, used_rows + started.rows),
                Err(SourceJournalError::Capacity)
            );
            assert_eq!(
                remaining.check(used_bytes + started.bytes, used_rows + started.rows + 1),
                Err(SourceJournalError::Capacity)
            );
        },
    );
}
