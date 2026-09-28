//! Authenticated accounting lineage. Only inventory's authenticated decoder
//! route creates this builder; pure check_entries never exports this proof.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    accounting::CheckedTargetAccountingV8, CheckedOwnedEffectSettlementV8,
};

/// Proof data only: no ledger mutation, lease, dispatch or owner restoration.
pub(super) struct CheckedAccountingPrefixV8<'a> {
    context: &'a CheckedOwnedWaitJournalContextV8,
    program_root: String,
    policy_epoch: u64,
    invocation: String,
    execution: String,
    binding: String,
    generation: String,
    prefix_bytes: usize,
    prefix_rows: usize,
    prefix_mac: String,
    exchanges: Vec<(usize, usize, usize, CheckedTargetAccountingV8)>,
}
impl CheckedAccountingPrefixV8<'_> {
    pub(super) fn matches(
        &self,
        context: &CheckedOwnedWaitJournalContextV8,
        bytes: usize,
        rows: usize,
        mac: &str,
    ) -> bool {
        std::ptr::eq(self.context, context)
            && self.program_root == context.registration().expected_facts().scope.program_root()
            && self.policy_epoch == context.registration().expected_facts().scope.policy_epoch()
            && self.invocation == context.ordinary().invocation()
            && self.execution == context.registration().expected_facts().execution
            && self.binding == context.registration().expected_facts().binding
            && self.generation == context.generation()
            && self.prefix_bytes == bytes
            && self.prefix_rows == rows
            && self.prefix_mac == mac
    }
    pub(super) fn previous(&self) -> Option<&CheckedTargetAccountingV8> {
        self.exchanges.last().map(|e| &e.3)
    }
}

pub(super) struct AccountingBuilderV8<'a> {
    proof: CheckedAccountingPrefixV8<'a>,
    boundaries: Vec<(usize, String)>,
}
impl<'a> AccountingBuilderV8<'a> {
    /// Caller must already have authenticated EVERY row via decode_inventory.
    /// This parses only already-authenticated envelope metadata, not a second
    /// MAC verifier or a history supplied by a caller with a prefix index.
    pub(super) fn authenticated(
        context: &'a CheckedOwnedWaitJournalContextV8,
        bytes: &[u8],
    ) -> Result<Self, Error> {
        let mut end = 0usize;
        let mut boundaries = Vec::new();
        for line in bytes.split_inclusive(|b| *b == b'\n') {
            if line.last() != Some(&b'\n') {
                return Err(Error::Malformed);
            }
            end = end.checked_add(line.len()).ok_or(Error::Capacity)?;
            let envelope = wire::parse(&line[..line.len() - 1])?;
            let mac = envelope["authentication"]
                .as_str()
                .ok_or(Error::Malformed)?;
            boundaries.push((end, mac.to_owned()));
        }
        Ok(Self {
            proof: CheckedAccountingPrefixV8 {
                context,
                program_root: context
                    .registration()
                    .expected_facts()
                    .scope
                    .program_root()
                    .into(),
                policy_epoch: context.registration().expected_facts().scope.policy_epoch(),
                invocation: context.ordinary().invocation().into(),
                execution: context.registration().expected_facts().execution.clone(),
                binding: context.registration().expected_facts().binding.clone(),
                generation: context.generation().into(),
                prefix_bytes: 0,
                prefix_rows: 0,
                prefix_mac: "0".repeat(64),
                exchanges: Vec::new(),
            },
            boundaries,
        })
    }
    pub(super) fn preceding(&self) -> Option<&CheckedTargetAccountingV8> {
        self.proof.previous()
    }
    /// Called only AFTER exact request, settlement, target evidence, ordinary
    /// payload and true causal references have all passed the existing checker.
    pub(super) fn record(
        &mut self,
        recorded: usize,
        intent: usize,
        settlement: usize,
        facts: &CheckedOwnedEffectSettlementV8,
    ) -> Result<(), Error> {
        if self.proof.prefix_rows != recorded
            || intent.checked_add(1) != Some(settlement)
            || settlement.checked_add(1) != Some(recorded)
            || self.proof.exchanges.last().is_some_and(|e| e.2 >= intent)
        {
            return Err(Error::Binding);
        }
        // Current grammar is one first effect. A repeated record cannot reset
        // this accumulator; cumulative source admission remains a successor.
        if self.preceding().is_some() {
            return Err(Error::Binding);
        }
        self.proof.exchanges.push((
            intent,
            settlement,
            recorded,
            facts.accounting_proof().clone(),
        ));
        Ok(())
    }
    pub(super) fn row_checked(&mut self, rows: usize) -> Result<(), Error> {
        if rows
            != self
                .proof
                .prefix_rows
                .checked_add(1)
                .ok_or(Error::Capacity)?
        {
            return Err(Error::Binding);
        }
        let (bytes, mac) = self.boundaries.get(rows - 1).ok_or(Error::Binding)?;
        self.proof.prefix_rows = rows;
        self.proof.prefix_bytes = *bytes;
        self.proof.prefix_mac = mac.clone();
        Ok(())
    }
    /// Called only AFTER final fold and the physical lease postguard.
    pub(super) fn finish(self) -> Result<CheckedAccountingPrefixV8<'a>, Error> {
        if self.proof.prefix_rows != self.boundaries.len() {
            return Err(Error::Binding);
        }
        Ok(self.proof)
    }
}

#[cfg(test)]
mod tests;
