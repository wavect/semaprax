//! Serialized closure maxima for inert effect history; no dispatch/ACK authority.
use super::*;

pub(super) const MAX_EVIDENCE_BYTES: usize = 1_475;
pub(super) const MAX_RESULT_WIRE_BYTES: usize = 65_536;

pub(super) struct EffectRoomsV8 {
    pub ready: RoomV8,
    pub intent: RoomV8,
    pub settlement: RoomV8,
    pub recorded: RoomV8,
    pub cleanup: RoomV8,
}
/// Each component is a complete maximum canonical row, including LF and chain
/// envelope. Alternative settlement outcomes are bounded without summing them.
pub(super) fn rooms(
    max: &templates::Maxima,
    state_cleanup: RoomV8,
) -> Result<EffectRoomsV8, SourceJournalError> {
    let fixed = fixed_rooms()?;
    let started = row(json!({
        "kind":"owned_effect_decision_cleanup_started", "turn":u32::MAX,
        "attempt":u32::MAX, "staged":u32::MAX, "ready":u32::MAX,
        "consumed":u32::MAX, "intent":u32::MAX, "settlement":u32::MAX,
        "recorded":u32::MAX, "decision_digest":hash(),
        "operations":max.effect_operations, "operations_digest":hash()
    }))?;
    let cleanup = started
        .add(receipt(&max.effect_operations)?)?
        .add(state_cleanup)?;
    let recorded = fixed.recorded.add(cleanup)?;
    let settlement = fixed.settlement.add(recorded)?;
    let intent = fixed.intent.add(settlement)?;
    Ok(EffectRoomsV8 {
        ready: intent,
        intent: settlement,
        settlement: recorded,
        recorded: cleanup,
        cleanup: state_cleanup,
    })
}
// These rows contain only frozen schema constants and maximum coordinate/payload
// widths. Retain their sizes, never their serialized payloads or a live verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FixedEffectRoomsV8 {
    intent: RoomV8,
    settlement: RoomV8,
    recorded: RoomV8,
}
fn fixed_rooms() -> Result<FixedEffectRoomsV8, SourceJournalError> {
    static FIXED: std::sync::OnceLock<Result<FixedEffectRoomsV8, SourceJournalError>> =
        std::sync::OnceLock::new();
    *FIXED.get_or_init(fixed_rooms_uncached)
}
fn fixed_rooms_uncached() -> Result<FixedEffectRoomsV8, SourceJournalError> {
    let intent = ordinary(SourceJournalEntry::EffectIntent {
        turn: u32::MAX,
        attempt: u32::MAX,
        operation: "a".repeat(240),
        request_digest: hash(),
    })?;
    let observed = ordinary(SourceJournalEntry::EffectObserved {
        turn: u32::MAX,
        attempt: u32::MAX,
        operation: "a".repeat(240),
        observation: vec![255; super::super::super::MAX_SOURCE_EFFECT_BYTES],
        observation_digest: hash(),
    })?;
    let failed = ordinary(SourceJournalEntry::EffectFailed {
        turn: u32::MAX,
        attempt: u32::MAX,
        operation: "a".repeat(240),
        reason: super::super::super::SourceEffectFailure::DeadlineExceeded,
    })?;
    let recorded = row(json!({
        "kind":"owned_effect_settlement_recorded", "turn":u32::MAX,
        "attempt":u32::MAX, "intent":u32::MAX, "settlement":u32::MAX,
        "evidence":"ff".repeat(MAX_EVIDENCE_BYTES), "evidence_digest":hash(),
        "result_wire":"ff".repeat(MAX_RESULT_WIRE_BYTES)
    }))?;
    Ok(FixedEffectRoomsV8 {
        intent,
        settlement: observed.either(failed),
        recorded,
    })
}

/// Used only after ACKed Started; preserve the selected actual compiler vector.
pub(super) fn receipt(operations: &Value) -> Result<RoomV8, SourceJournalError> {
    row(json!({
        "kind":"owned_effect_decision_cleanup_settled", "turn":u32::MAX,
        "attempt":u32::MAX, "started":u32::MAX,
        "receipt":templates::receipt(operations)?, "receipt_digest":hash()
    }))
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, unix))]
mod phase_tests;

#[cfg(test)]
fn check_edge(before: RoomV8, encoded: usize, after: RoomV8) {
    let used = super::super::super::MAX_SOURCE_DOCUMENT_BYTES - before.bytes;
    before.check(used, 0).unwrap();
    assert_eq!(before.check(used + 1, 0), Err(SourceJournalError::Capacity));
    after.check(used + encoded, 1).unwrap();
    assert!(before.bytes >= encoded + after.bytes);
    assert!(before.rows >= 1 + after.rows);
}
