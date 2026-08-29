use crate::diagnostic::Diagnostic;

use super::error;

/// Target-neutral publication sequencer. It carries no provider handle and
/// performs no copy/drop itself; adapters advance it only after the named
/// physical step has succeeded. A failure is sticky and publication is
/// impossible until authentication, copy, and settlement all completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlatOwnedRecordSettlement {
    state: SettlementState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettlementState {
    Received,
    Authenticated,
    Copied,
    Settled,
    Published,
    Failed,
}

impl FlatOwnedRecordSettlement {
    pub const fn received() -> Self {
        Self {
            state: SettlementState::Received,
        }
    }
    pub fn authenticated(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Received, SettlementState::Authenticated)
    }
    pub fn copy_completed(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Authenticated, SettlementState::Copied)
    }
    pub fn settlement_completed(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Copied, SettlementState::Settled)
    }
    pub fn publish(&mut self) -> Result<(), Diagnostic> {
        self.advance(SettlementState::Settled, SettlementState::Published)
    }
    pub fn fail(&mut self) -> Result<(), Diagnostic> {
        if self.state == SettlementState::Published {
            return Err(error(
                "flat owned-record failure cannot replace a published result",
            ));
        }
        self.state = SettlementState::Failed;
        Ok(())
    }
    pub const fn is_published(self) -> bool {
        matches!(self.state, SettlementState::Published)
    }
    fn advance(
        &mut self,
        expected: SettlementState,
        next: SettlementState,
    ) -> Result<(), Diagnostic> {
        if self.state != expected {
            self.state = SettlementState::Failed;
            return Err(error(
                "flat owned-record publication transition is out of order",
            ));
        }
        self.state = next;
        Ok(())
    }
}
