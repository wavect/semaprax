//! The engine-neutral normalized trace vocabulary for [Public Generic
//! Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#the-normalized-trace):
//! deterministic lifecycle events with no payload or secret bytes, so the
//! interpreter, native C11, and Core Wasm adapters can each emit their own
//! target-local events and still be compared through this shared,
//! normalized sequence.
//!
//! [`Trace`] is append-only and ordinal-numbered: the sequence itself is the
//! evidence, never summarized or reordered downstream, matching the
//! repository's "cleanup-plan vectors are canonical runtime order" invariant
//! applied to trace evidence.

use crate::public_generic_abi::carrier::{CarrierState, Settlement};

/// Which side of the boundary one traced event belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Direction {
    Input,
    Result,
}

/// The closed, engine-neutral event vocabulary. Every variant name matches
/// [Public Generic Carrier v1's normalized trace
/// vocabulary](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#the-normalized-trace)
/// exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TraceLabel {
    FrameValidated,
    LeafAllocationStarted,
    LeafAllocationCommitted,
    LeafPayloadCopied,
    InputValuePrepared,
    InputTransferCommitted,
    ExecutionStarted,
    ExecutionFinished,
    ResultLeafAllocationStarted,
    ResultLeafAllocationCommitted,
    ResultValuePrepared,
    ResultCommit,
    LeafRelease,
    CarrierRelease,
    TerminalStatus,
}

/// One normalized event. Carries only deterministic identity/lifecycle
/// fields: no host pointer, native offset, Wasm address, payload byte,
/// random nonce, wall-clock time, or process identifier — matching [Public
/// Generic Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md)'s
/// canonical-bytes bar applied to trace evidence rather than wire bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceEvent {
    /// Position in the recorded sequence. Never reused, never reordered.
    pub ordinal: u32,
    pub label: TraceLabel,
    pub direction: Direction,
    /// The structural leaf index (`Handle::leaf`'s `index` argument), or
    /// `None` for a whole-carrier / whole-value event such as a commit
    /// marker.
    pub leaf: Option<u32>,
    pub before: Option<CarrierState>,
    pub after: Option<CarrierState>,
    /// Set only on the terminal event and on a settlement attempt; absent
    /// otherwise.
    pub status: Option<Settlement>,
}

/// An append-only, ordinal-numbered recorder of [`TraceEvent`]s for one
/// call. Nothing here allocates, transfers, or releases a real value — see
/// [`super::machine`] for the orchestration that drives both the state
/// machine and this recorder together.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    events: Vec<TraceEvent>,
}

impl Trace {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &mut self,
        label: TraceLabel,
        direction: Direction,
        leaf: Option<u32>,
        before: Option<CarrierState>,
        after: Option<CarrierState>,
        status: Option<Settlement>,
    ) {
        let ordinal = self.events.len() as u32;
        self.events.push(TraceEvent {
            ordinal,
            label,
            direction,
            leaf,
            before,
            after,
            status,
        });
    }

    /// The recorded sequence, in the exact order it was produced.
    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_assigns_sequential_ordinals_and_preserves_order() {
        let mut trace = Trace::new();
        trace.record(
            TraceLabel::FrameValidated,
            Direction::Input,
            None,
            None,
            None,
            None,
        );
        trace.record(
            TraceLabel::LeafAllocationStarted,
            Direction::Input,
            Some(0),
            Some(CarrierState::Created),
            None,
            None,
        );
        trace.record(
            TraceLabel::TerminalStatus,
            Direction::Input,
            None,
            None,
            None,
            Some(Settlement::Success),
        );

        let events = trace.events();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].ordinal, 0);
        assert_eq!(events[1].ordinal, 1);
        assert_eq!(events[2].ordinal, 2);
        assert_eq!(events[0].label, TraceLabel::FrameValidated);
        assert_eq!(events[1].leaf, Some(0));
        assert_eq!(events[2].status, Some(Settlement::Success));
    }

    #[test]
    fn trace_carries_no_payload_bytes_by_construction() {
        // TraceEvent's fields are exhaustively identities/enums/small
        // integers; there is no `Vec<u8>` or `String` field a payload could
        // hide in. This test pins that shape so a future edit that adds a
        // payload-carrying field is forced to touch this assertion.
        fn assert_no_payload_field(_: TraceEvent) {}
        let event = TraceEvent {
            ordinal: 0,
            label: TraceLabel::CarrierRelease,
            direction: Direction::Result,
            leaf: None,
            before: None,
            after: None,
            status: None,
        };
        assert_no_payload_field(event);
    }
}
