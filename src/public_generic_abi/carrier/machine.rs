//! The call-level orchestration for [Public Generic Carrier
//! v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md): [`CarrierCallMachine`]
//! ties one whole call's [`CallLedger`] phase ledger together with every
//! argument and result [`HandleLedger`], recording a normalized
//! [`Trace`](super::trace::Trace) as it runs.
//!
//! [`CarrierCallMachine::commit_input_transfer`] and
//! [`CarrierCallMachine::commit_result`] are the executable answer to
//! [Public Generic Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#failure-settlement-and-release-order)'s
//! claim that "a call that reaches `Committed` has transferred every
//! argument handle or none of them": both validate every handle's
//! readiness with a pure, non-mutating check *before* committing any of
//! them, so a failure discovered mid-transfer (one handle not yet
//! `Initialized` while its siblings are) can never leave some handles
//! `Transferred` and others not. This directly matches the repository's "an
//! owned call stages arguments left to right and transfers them together at
//! its declared commit boundary" invariant.
//!
//! Scope: this machine implements the LOGICAL states, the phase ledger, and
//! the trace vocabulary only. It does not parse carrier bytes, does not
//! bind to a `VerifiedPublicGenericDescriptor`, and defines no physical
//! target mapping. Wiring this orchestration to a real descriptor, a parsed
//! frame, and a physical allocation adapter is #154/#155's follow-on work,
//! per [Public Generic Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#logical-versus-physical).

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::carrier::trace::{Direction, Trace, TraceLabel};
use crate::public_generic_abi::carrier::{
    transition, CallLedger, CarrierState, Event, Handle, HandleLedger, Phase, Settlement,
    CARRIER_SCHEMA, ILLEGAL_TRANSITION,
};

fn illegal_here(reason: &str) -> Diagnostic {
    Diagnostic::io(ILLEGAL_TRANSITION, format!("{CARRIER_SCHEMA}: {reason}"))
}

/// One whole call's argument or result handles: the root aggregate plus its
/// owned leaves, in the same structural order the settlement plan fixes.
/// `id` 0 is always the root; leaves are indexed `0..leaves.len()`
/// (`Handle::leaf`'s own numbering, one below the wire `id`).
#[derive(Clone, Debug)]
pub struct HandleSet {
    root: HandleLedger,
    leaves: Vec<HandleLedger>,
}

impl HandleSet {
    pub fn new(root: Handle, leaves: Vec<Handle>) -> Self {
        Self {
            root: HandleLedger::new(root),
            leaves: leaves.into_iter().map(HandleLedger::new).collect(),
        }
    }

    /// Root first, then leaves in structural order — the canonical
    /// obligation order this whole set commits and releases by.
    fn indexed_ledgers(&self) -> impl Iterator<Item = (Option<u32>, &HandleLedger)> {
        std::iter::once((None, &self.root)).chain(
            self.leaves
                .iter()
                .enumerate()
                .map(|(index, ledger)| (Some(index as u32), ledger)),
        )
    }

    fn indexed_ledgers_mut(&mut self) -> impl Iterator<Item = (Option<u32>, &mut HandleLedger)> {
        std::iter::once((None, &mut self.root)).chain(
            self.leaves
                .iter_mut()
                .enumerate()
                .map(|(index, ledger)| (Some(index as u32), ledger)),
        )
    }

    /// The canonical obligation order: root, then leaves, structural order.
    /// Release order is required to be the exact reverse of this.
    pub fn obligation_order(&self) -> Vec<Handle> {
        self.indexed_ledgers()
            .map(|(_, ledger)| ledger.handle())
            .collect()
    }

    pub fn root_state(&self) -> CarrierState {
        self.root.state()
    }

    pub fn leaf_state(&self, index: usize) -> CarrierState {
        self.leaves[index].state()
    }

    /// Every ledger, root included, is ready for `event` without mutating
    /// any of them. Used to make commit atomic: nothing is mutated unless
    /// every handle can legally make the transition.
    fn all_ready_for(&self, event: Event) -> bool {
        self.indexed_ledgers()
            .all(|(_, ledger)| transition(ledger.state(), event).is_ok())
    }
}

/// One whole call's machine: the [`CallLedger`] phase ledger, the input
/// [`HandleSet`], an optional result [`HandleSet`] once result staging
/// begins, and the normalized [`Trace`] recorded as it runs.
///
/// Fields are private and every mutation goes through a transition method,
/// per Public Generic Carrier v1's "use private fields and transition
/// methods; avoid a publicly mutable enum field" requirement.
#[derive(Clone, Debug)]
pub struct CarrierCallMachine {
    call: CallLedger,
    input: HandleSet,
    result: Option<HandleSet>,
    execution_started: bool,
    execution_finished: bool,
    trace: Trace,
}

impl CarrierCallMachine {
    pub fn new(root: Handle, leaves: Vec<Handle>) -> Self {
        Self {
            call: CallLedger::new(),
            input: HandleSet::new(root, leaves),
            result: None,
            execution_started: false,
            execution_finished: false,
            trace: Trace::new(),
        }
    }

    pub fn phase(&self) -> Phase {
        self.call.phase()
    }

    pub fn settlement(&self) -> Option<Settlement> {
        self.call.settlement()
    }

    pub fn input(&self) -> &HandleSet {
        &self.input
    }

    pub fn input_mut(&mut self) -> &mut HandleSet {
        &mut self.input
    }

    pub fn result(&self) -> Option<&HandleSet> {
        self.result.as_ref()
    }

    pub fn result_mut(&mut self) -> Option<&mut HandleSet> {
        self.result.as_mut()
    }

    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    /// `Preparing -> Validated`. Non-committing: the call may still be
    /// abandoned with no owned handle ever reaching `Transferred`.
    pub fn validate(&mut self) -> Result<(), Diagnostic> {
        self.call.advance(Phase::Validated)?;
        self.trace.record(
            TraceLabel::FrameValidated,
            Direction::Input,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    /// Fill every input handle (`Created -> Initialized`), root then
    /// leaves, recording an allocation/copy event triple for each.
    pub fn prepare_input(&mut self) -> Result<(), Diagnostic> {
        Self::fill_and_trace(&mut self.input, &mut self.trace, Direction::Input, false)?;
        self.trace.record(
            TraceLabel::InputValuePrepared,
            Direction::Input,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    fn fill_and_trace(
        set: &mut HandleSet,
        trace: &mut Trace,
        direction: Direction,
        is_result: bool,
    ) -> Result<(), Diagnostic> {
        let (started, committed) = if is_result {
            (
                TraceLabel::ResultLeafAllocationStarted,
                TraceLabel::ResultLeafAllocationCommitted,
            )
        } else {
            (
                TraceLabel::LeafAllocationStarted,
                TraceLabel::LeafAllocationCommitted,
            )
        };
        for (leaf, ledger) in set.indexed_ledgers_mut() {
            let before = ledger.state();
            trace.record(started, direction, leaf, Some(before), None, None);
            ledger.apply(Event::Fill)?;
            let after = ledger.state();
            trace.record(committed, direction, leaf, Some(before), Some(after), None);
            if !is_result {
                trace.record(
                    TraceLabel::LeafPayloadCopied,
                    direction,
                    leaf,
                    Some(after),
                    Some(after),
                    None,
                );
            }
        }
        Ok(())
    }

    /// The one atomic commit point: every input handle's `Initialized ->
    /// Transferred` transition happens here, together, or none of them do.
    ///
    /// A precondition failure (any handle not `Initialized` — including a
    /// handle already `Transferred` from an earlier call, which is exactly
    /// how a double transfer is refused) mutates nothing: no handle
    /// changes state and the phase does not advance, so a failure
    /// discovered here is indistinguishable from a failure discovered one
    /// instant earlier, before any commit was attempted.
    pub fn commit_input_transfer(&mut self) -> Result<(), Diagnostic> {
        if self.call.phase() != Phase::Validated {
            return Err(illegal_here(
                "input transfer cannot commit outside the Validated phase",
            ));
        }
        if !self.input.all_ready_for(Event::Commit) {
            return Err(illegal_here(
                "input transfer cannot commit: not every handle is Initialized",
            ));
        }
        self.call.advance(Phase::Committed)?;
        for (_, ledger) in self.input.indexed_ledgers_mut() {
            ledger.apply(Event::Commit)?;
        }
        self.trace.record(
            TraceLabel::InputTransferCommitted,
            Direction::Input,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    /// Mark the checked function's execution boundary. Legal only after
    /// input transfer has committed and before result staging begins.
    pub fn begin_execution(&mut self) -> Result<(), Diagnostic> {
        if self.call.phase() != Phase::Committed || self.execution_started {
            return Err(illegal_here(
                "execution can only begin once, after input transfer commits",
            ));
        }
        self.execution_started = true;
        self.trace.record(
            TraceLabel::ExecutionStarted,
            Direction::Input,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    pub fn finish_execution(&mut self) -> Result<(), Diagnostic> {
        if !self.execution_started || self.execution_finished {
            return Err(illegal_here(
                "execution cannot finish before it begins, or twice",
            ));
        }
        self.execution_finished = true;
        self.trace.record(
            TraceLabel::ExecutionFinished,
            Direction::Input,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    /// Start staging a result. Legal only after execution has finished and
    /// only once per call.
    pub fn begin_result(&mut self, root: Handle, leaves: Vec<Handle>) -> Result<(), Diagnostic> {
        if !self.execution_finished || self.result.is_some() {
            return Err(illegal_here(
                "result staging can only begin once, after execution finishes",
            ));
        }
        self.result = Some(HandleSet::new(root, leaves));
        Ok(())
    }

    /// Fill every result handle privately (`Created -> Initialized`). The
    /// consumer sees no result leaf before [`Self::commit_result`].
    pub fn prepare_result(&mut self) -> Result<(), Diagnostic> {
        let result = self
            .result
            .as_mut()
            .ok_or_else(|| illegal_here("result staging has not begun"))?;
        Self::fill_and_trace(result, &mut self.trace, Direction::Result, true)?;
        self.trace.record(
            TraceLabel::ResultValuePrepared,
            Direction::Result,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    /// The one atomic result-commit point, exposing the result handle set
    /// to the consumer only as a whole. Same all-or-nothing precondition
    /// discipline as [`Self::commit_input_transfer`].
    pub fn commit_result(&mut self) -> Result<(), Diagnostic> {
        let result = self
            .result
            .as_mut()
            .ok_or_else(|| illegal_here("result staging has not begun"))?;
        if !result.all_ready_for(Event::Commit) {
            return Err(illegal_here(
                "result cannot commit: not every result handle is Initialized",
            ));
        }
        for (_, ledger) in result.indexed_ledgers_mut() {
            ledger.apply(Event::Commit)?;
        }
        self.trace.record(
            TraceLabel::ResultCommit,
            Direction::Result,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    /// Select the terminal settlement outcome. Sticky: [`CallLedger::settle`]
    /// rejects a later, different outcome — this is how "cleanup attempting
    /// to overwrite a sticky failure status" is refused. When no earlier
    /// failure exists, the first cleanup/release failure may legally become
    /// the terminal status, matching Public Generic Carrier v1's sticky-
    /// failure rule.
    pub fn settle(&mut self, outcome: Settlement) -> Result<(), Diagnostic> {
        self.call.settle(outcome)?;
        self.trace.record(
            TraceLabel::TerminalStatus,
            Direction::Input,
            None,
            None,
            None,
            Some(outcome),
        );
        Ok(())
    }

    /// Release every handle in `set` in the exact reverse of its
    /// obligation order, applying the release event appropriate to
    /// `transferred` (whether the set's handles already committed before
    /// the failure). Recorded as one [`TraceLabel::LeafRelease`] event per
    /// handle plus one whole-carrier [`TraceLabel::CarrierRelease`].
    fn release_set(
        set: &mut HandleSet,
        trace: &mut Trace,
        direction: Direction,
        transferred: bool,
    ) -> Result<(), Diagnostic> {
        let event = if transferred {
            Event::ReleaseAfterTransfer
        } else {
            Event::ReleaseBeforeTransfer
        };
        let mut reversed: Vec<(Option<u32>, &mut HandleLedger)> =
            set.indexed_ledgers_mut().collect();
        reversed.reverse();
        for (leaf, ledger) in reversed {
            let before = ledger.state();
            ledger.apply(event)?;
            let after = ledger.state();
            trace.record(
                TraceLabel::LeafRelease,
                direction,
                leaf,
                Some(before),
                Some(after),
                None,
            );
        }
        trace.record(
            TraceLabel::CarrierRelease,
            direction,
            None,
            None,
            None,
            None,
        );
        Ok(())
    }

    /// Release the input set after a failure discovered before commit
    /// (every handle still `Created`/`Initialized`).
    pub fn release_input_before_transfer(&mut self) -> Result<(), Diagnostic> {
        Self::release_set(&mut self.input, &mut self.trace, Direction::Input, false)
    }

    /// Release the input set after a failure discovered after commit but
    /// before it was consumed (every handle already `Transferred`).
    pub fn release_input_after_transfer(&mut self) -> Result<(), Diagnostic> {
        Self::release_set(&mut self.input, &mut self.trace, Direction::Input, true)
    }

    /// Release a staged-but-uncommitted result set after a failure during
    /// result construction: only the leaves actually completed are
    /// released, in exact reverse order — callers pass the partially
    /// filled set as-is, since `HandleLedger` state already reflects
    /// exactly which leaves reached `Initialized`.
    pub fn release_result_before_commit(&mut self) -> Result<(), Diagnostic> {
        let result = self
            .result
            .as_mut()
            .ok_or_else(|| illegal_here("result staging has not begun"))?;
        Self::release_set(result, &mut self.trace, Direction::Result, false)
    }
}

#[cfg(test)]
mod tests;
