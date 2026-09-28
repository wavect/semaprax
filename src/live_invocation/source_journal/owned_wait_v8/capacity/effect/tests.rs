//! Size proof tests only: no physical effect, ACK, cleanup, or Outcome authority.
use super::*;

fn check_edge(before: RoomV8, encoded: usize, after: RoomV8) {
    let used = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES - before.bytes;
    before.check(used, 0).unwrap();
    assert_eq!(before.check(used + 1, 0), Err(SourceJournalError::Capacity));
    after.check(used + encoded, 1).unwrap();
    assert!(before.bytes >= encoded + after.bytes);
    assert!(before.rows >= 1 + after.rows);
}

#[test]
fn effect_closure_preserves_room_across_maximum_serialized_phase_edges() {
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, _lease, _key| {
        let max = templates::maxima(context.fold()).unwrap();
        let terminal = RoomV8 {
            bytes: super::super::super::super::execution::TERMINAL_ROOM_BYTES,
            rows: 2,
        };
        let state = cleanup(&max, OwnerV8::State, &max.state_operations)
            .unwrap()
            .add(terminal)
            .unwrap()
            .either(
                cleanup(&max, OwnerV8::State, &max.result_operations)
                    .unwrap()
                    .add(terminal)
                    .unwrap(),
            );
        let bound = rooms(&max, state).unwrap();
        assert!(bound.ready.bytes > 2 * MAX_RESULT_WIRE_BYTES);
        assert_eq!(bound.ready.rows, state.rows + 5);
        let intent = ordinary(SourceJournalEntry::EffectIntent {
            turn: u32::MAX,
            attempt: u32::MAX,
            operation: "a".repeat(240),
            request_digest: hash(),
        })
        .unwrap();
        check_edge(bound.ready, intent.bytes, bound.intent);
        let observed = ordinary(SourceJournalEntry::EffectObserved {
            turn: u32::MAX,
            attempt: u32::MAX,
            operation: "a".repeat(240),
            observation: vec![255; super::super::super::super::MAX_SOURCE_EFFECT_BYTES],
            observation_digest: hash(),
        })
        .unwrap();
        check_edge(bound.intent, observed.bytes, bound.settlement);
        let recorded = row(
            json!({"kind":"owned_effect_settlement_recorded","turn":u32::MAX,"attempt":u32::MAX,
            "intent":u32::MAX,"settlement":u32::MAX,"evidence":"ff".repeat(MAX_EVIDENCE_BYTES),
            "evidence_digest":hash(),"result_wire":"ff".repeat(MAX_RESULT_WIRE_BYTES)}),
        )
        .unwrap();
        check_edge(bound.settlement, recorded.bytes, bound.recorded);
        let started = row(json!({"kind":"owned_effect_decision_cleanup_started","turn":u32::MAX,"attempt":u32::MAX,
            "staged":u32::MAX,"ready":u32::MAX,"consumed":u32::MAX,"intent":u32::MAX,"settlement":u32::MAX,
            "recorded":u32::MAX,"decision_digest":hash(),"operations":max.decision_operations,"operations_digest":hash()})).unwrap();
        let after_started = receipt(&max.decision_operations)
            .unwrap()
            .add(state)
            .unwrap();
        check_edge(bound.recorded, started.bytes, after_started);
        check_edge(
            after_started,
            receipt(&max.decision_operations).unwrap().bytes,
            state,
        );
        assert_eq!(
            bound.cleanup, state,
            "Decision release preserves future State failure closure"
        );
    });
}
