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
