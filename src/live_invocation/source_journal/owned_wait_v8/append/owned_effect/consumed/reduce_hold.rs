//! Exclusive future Reduce funding acquired only from the live Consumed owner.
//! No fuel charge, Intent, host, matching-Reduce ACK debit or recovery producer.
use super::*;

/// The actual owner is retained first; credit never exists as a detached token.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct HeldOwnedAuthorizationConsumedV8<
    'j,
> {
    owner: VerifiedOwnedAuthorizationConsumedV8<'j>,
    hold: ProspectiveOwnedReduceHoldV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ProspectiveOwnedReduceHoldV8<
    'j,
> {
    journal: &'j SourceOwnedWaitJournalV8,
    identity: u64,
}
impl Drop for ProspectiveOwnedReduceHoldV8<'_> {
    fn drop(&mut self) {
        // No refund, replacement or terminal retirement exists in this packet.
        self.journal.poisoned.set(true);
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ReduceHoldRejectionV8<'j> {
    _owner: VerifiedOwnedAuthorizationConsumedV8<'j>,
    error: SourceJournalError,
}
impl ProspectiveOwnedReduceHoldV8<'_> {
    /// Borrow-only Consumed-phase guard, never an owner/ACK/token producer.
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_guard(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        sequence: usize,
        acknowledged_bytes: usize,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            // Refuse before reading a foreign container; retire our own lineage.
            if !std::ptr::eq(self.journal, journal) {
                return Err(SourceJournalError::Binding);
            }
            journal.validate_guard()?;
            let current = journal.begin_session()?;
            let (reserved, stages, turn, attempt) = current.inventory.prospective_reduce_facts()?;
            let ordinary = journal.context.ordinary();
            let (_, execution) = journal
                .context
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let fuel = u64::try_from(execution.evaluation_fuel())
                .map_err(|_| SourceJournalError::Capacity)?;
            if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
                return Err(SourceJournalError::Binding);
            }
            funding(
                reserved,
                stages,
                fuel,
                u64::try_from(
                    ordinary
                        .max_total_steps()
                        .ok_or(SourceJournalError::Binding)?,
                )
                .map_err(|_| SourceJournalError::Capacity)?,
                ordinary.max_stages(),
            )?;
            {
                let registry = journal
                    .prospective_reduce
                    .try_borrow()
                    .map_err(|_| SourceJournalError::Order)?;
                let record = registry.as_ref().ok_or(SourceJournalError::Binding)?;
                if record.identity != self.identity
                    || record.turn != turn
                    || record.attempt != attempt
                    || record.fuel != fuel
                    || record.sequence != sequence
                    || record.bytes != acknowledged_bytes
                    || current.sequence() != sequence
                    || current.acknowledged_bytes() != acknowledged_bytes
                    || record.authentication != current.inventory.authentication_tail()
                {
                    return Err(SourceJournalError::Binding);
                }
            }
            // No registry borrow crosses this final physical guard.
            journal.validate_guard()
        })();
        result.inspect_err(|_| self.journal.poisoned.set(true))
    }
}
impl HeldOwnedAuthorizationConsumedV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let guard = || {
            self.hold.validate_guard(
                self.owner.session.journal,
                self.owner.session.sequence(),
                self.owner.session.acknowledged_bytes(),
            )
        };
        let result = (|| {
            self.owner.validate_live()?;
            guard()?;
            self.owner.validate_live()?;
            // Policy/clock callbacks have ended; re-read the physical prefix.
            guard()
        })();
        result.inspect_err(|_| self.hold.journal.poisoned.set(true))
    }
}

fn funding(
    reserved: u64,
    stages: u32,
    fuel: u64,
    total: u64,
    max_stages: u32,
) -> Result<(), SourceJournalError> {
    if reserved.checked_add(fuel).is_none_or(|n| n > total)
        || stages.checked_add(1).is_none_or(|n| n > max_stages)
    {
        return Err(SourceJournalError::Capacity);
    }
    Ok(())
}
pub(super) fn reserve<'j>(
    owner: VerifiedOwnedAuthorizationConsumedV8<'j>,
) -> Result<HeldOwnedAuthorizationConsumedV8<'j>, ReduceHoldRejectionV8<'j>> {
    let journal = owner.session.journal;
    let prepared = (|| {
        journal.validate_guard()?;
        if journal
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?
            .is_some()
        {
            return Err(SourceJournalError::Order);
        }
        owner.validate_live()?;
        // Read the actual physical prefix again, not the envelope's old Vec.
        let current = journal.begin_session()?;
        owner.witness.successor.validate_current()?;
        if current.sequence() != owner.session.sequence()
            || current.acknowledged_bytes() != owner.session.acknowledged_bytes()
            || current.inventory.authentication_tail()
                != owner.session.inventory.authentication_tail()
        {
            journal.poisoned.set(true);
            return Err(SourceJournalError::Order);
        }
        let (reserved, stages, turn, attempt) = current.inventory.prospective_reduce_facts()?;
        let ordinary = journal.context.ordinary();
        let (_, execution) = journal
            .context
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let fuel =
            u64::try_from(execution.evaluation_fuel()).map_err(|_| SourceJournalError::Capacity)?;
        if Some(execution.evaluation_fuel()) != ordinary.max_steps_per_stage() {
            return Err(SourceJournalError::Binding);
        }
        funding(
            reserved,
            stages,
            fuel,
            u64::try_from(
                ordinary
                    .max_total_steps()
                    .ok_or(SourceJournalError::Binding)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?,
            ordinary.max_stages(),
        )?;
        let authentication = current.inventory.authentication_tail().to_owned();
        // All policy/runtime callbacks finish before the registry's final insert.
        owner.validate_live()?;
        owner.witness.successor.validate_current()?;
        journal.validate_guard()?;
        let mut registry = journal
            .prospective_reduce
            .try_borrow_mut()
            .map_err(|_| SourceJournalError::Order)?;
        if journal.append_active.get() || registry.is_some() {
            return Err(SourceJournalError::Order);
        }
        let identity = journal
            .prospective_reduce_identity
            .get()
            .checked_add(1)
            .ok_or(SourceJournalError::Capacity)?;
        // Callback-free, borrow-held recheck/increment/insert is indivisible.
        journal.prospective_reduce_identity.set(identity);
        *registry = Some(ProspectiveReduceRegistryV8 {
            identity,
            sequence: current.sequence(),
            bytes: current.acknowledged_bytes(),
            authentication,
            turn,
            attempt,
            fuel,
        });
        Ok(identity)
    })();
    match prepared {
        Ok(identity) => Ok(HeldOwnedAuthorizationConsumedV8 {
            owner,
            hold: ProspectiveOwnedReduceHoldV8 { journal, identity },
        }),
        Err(error) => Err(ReduceHoldRejectionV8 {
            _owner: owner,
            error,
        }),
    }
}

#[cfg(test)]
mod tests;
