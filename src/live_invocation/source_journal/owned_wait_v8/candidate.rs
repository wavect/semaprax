//! Consuming inert append choreography. No sink, File, runtime owner, or ACK factory.
use super::*;
use crate::resumable_effects::owned_frame::SourceOwnedWaitLeaseV8;

enum ContextV8<'a> {
    Checked(&'a CheckedOwnedWaitJournalContextV8),
    #[cfg(test)]
    Synthetic(&'a FoldContextV8),
}
impl ContextV8<'_> {
    fn fold(&self) -> &FoldContextV8 {
        match self {
            Self::Checked(context) => context.fold(),
            #[cfg(test)]
            Self::Synthetic(context) => context,
        }
    }
}
/// Exact acknowledged inventory; no caller-shaped validated entries are admitted.
pub(super) struct InventoryV8<'a> {
    context: ContextV8<'a>,
    key: &'a SourceCheckpointKey,
    entries: Vec<ValidatedEntryV8>,
    invocation: String,
    generation: String,
    mac: String,
    document: Vec<u8>,
}
pub(super) struct CandidateV8<'a> {
    inventory: InventoryV8<'a>,
    row: ValidatedEntryV8,
    encoded: Vec<u8>,
    successor_mac: String,
}
pub(super) struct PendingV8<'a>(CandidateV8<'a>);
/// Append uncertainty permanently retires the retained data; no retry/extraction.
pub(super) struct PoisonedV8<'a> {
    _pending: PendingV8<'a>,
}
pub(super) struct CandidateRejectionV8<'a> {
    pub inventory: InventoryV8<'a>,
    pub row: EntryV8,
    pub error: SourceJournalError,
    pub physical: bool,
}
pub(super) struct AckRejectionV8<'a> {
    pub error: SourceJournalError,
    _poisoned: PoisonedV8<'a>,
}
/// No production constructor. Only a separately leased trusted physical adapter
/// may mint an ACK after the same-store append/sync acknowledgment.
pub(super) struct TrustedAppendAckV8 {
    invocation: String,
    generation: String,
    predecessor_seq: usize,
    predecessor_mac: String,
    successor_mac: String,
    encoded_bytes: usize,
}
impl<'a> InventoryV8<'a> {
    pub(super) fn recover(
        context: &'a CheckedOwnedWaitJournalContextV8,
        lease: &SourceOwnedWaitLeaseV8,
        key: &'a SourceCheckpointKey,
        document: &[u8],
    ) -> Result<Self, SourceJournalError> {
        let checked = inventory::checked_inventory_v8(context, lease, key, document)?;
        let (entries, mac) = checked.into_parts();
        Ok(Self {
            context: ContextV8::Checked(context),
            key,
            entries,
            invocation: context.ordinary().invocation().to_owned(),
            generation: context.generation().to_owned(),
            mac,
            document: document.to_vec(),
        })
    }
    pub(super) fn fresh(
        context: &'a CheckedOwnedWaitJournalContextV8,
        lease: &SourceOwnedWaitLeaseV8,
        key: &'a SourceCheckpointKey,
    ) -> Result<Self, SourceJournalError> {
        Self::recover(context, lease, key, &[])
    }
    #[cfg(test)]
    pub(super) fn fold_for_live_test(&self) -> fold::FoldV8 {
        fold::fold(self.context.fold(), &self.entries).expect("actual ACKed inventory")
    }
    pub(super) fn live_start_checkpoint_basis(
        &self,
        observation: &crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8,
    ) -> Result<(u64, u64, String), SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        if folded.tail != fold::TailV8::StartReserved {
            return Err(SourceJournalError::Order);
        }
        let [.., created, reserved] = self.entries.as_slice() else {
            return Err(SourceJournalError::Order);
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved {
            turn: 0,
            attempt: 0,
            wait,
            phase: model::PhaseV8::Start,
            replay_of: None,
            ..
        }) = &reserved.entry
        else {
            return Err(SourceJournalError::Order);
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCreated {
            turn: 0,
            attempt: 0,
            wait: created_wait,
            argument_digest,
            copy_arguments,
            ..
        }) = &created.entry
        else {
            return Err(SourceJournalError::Order);
        };
        if wait != created_wait || copy_arguments != observation.copy_arguments() {
            return Err(SourceJournalError::Binding);
        }
        Ok((
            folded.reserved_total,
            folded.consumed_recorded,
            argument_digest.clone(),
        ))
    }
    pub(super) fn sequence(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn acknowledged_bytes(&self) -> usize {
        self.document.len()
    }
    pub(super) fn authentication_tail(&self) -> &str {
        &self.mac
    }
    pub(super) fn prepare(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        row: EntryV8,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        #[cfg(test)]
        {
            self.prepare_inner(row, Some(lease), None)
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(row, Some(lease))
        }
    }
    fn prepare_inner(
        mut self,
        row: EntryV8,
        lease: Option<&SourceOwnedWaitLeaseV8>,
        #[cfg(test)] synthetic: Option<ValidatedEntryV8>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let mut physical = false;
        let result = (|| {
            let context = self.context.fold();
            let expected = ExpectedRowV8 {
                invocation: &self.invocation,
                generation: &self.generation,
                seq: u32::try_from(self.entries.len()).map_err(|_| SourceJournalError::Capacity)?,
                prev_mac: &self.mac,
                ordinary: &context.ordinary,
            };
            let encoded = wire::encode(&row, &expected, self.key)?;
            let (checked, successor_mac) = match &self.context {
                ContextV8::Checked(context) => {
                    let lease = lease.ok_or(SourceJournalError::Binding)?;
                    // No supplied proof pairs: authenticate the original ACK
                    // prefix and derive this row's facts from that same history.
                    let checked = inventory::checked_candidate_inventory_tagged_v8(
                        context,
                        lease,
                        self.key,
                        &self.document,
                        &encoded,
                    )
                    .map_err(|failure| {
                        physical =
                            matches!(&failure, inventory::InventoryValidationErrorV8::Physical(_));
                        failure.error()
                    })?;
                    let (mut entries, mac) = checked.into_parts();
                    if entries.len() != self.entries.len() + 1 {
                        return Err(SourceJournalError::Binding);
                    }
                    (entries.pop().ok_or(SourceJournalError::Binding)?, mac)
                }
                #[cfg(test)]
                ContextV8::Synthetic(_) => {
                    let checked = synthetic.ok_or(SourceJournalError::Binding)?;
                    if checked.entry != row {
                        return Err(SourceJournalError::Binding);
                    }
                    let envelope = wire::parse(&encoded[..encoded.len() - 1])?;
                    (
                        checked,
                        envelope["authentication"]
                            .as_str()
                            .ok_or(SourceJournalError::Malformed)?
                            .to_owned(),
                    )
                }
            };
            let previous = fold::fold(context, &self.entries)?;
            fold::validate_producer_transition(&previous, &checked)?;
            self.entries.push(checked);
            let next = fold::fold(context, &self.entries);
            let row = self.entries.pop().expect("prospective row retained");
            let next = next?;
            let bytes = self
                .document
                .len()
                .checked_add(encoded.len())
                .ok_or(SourceJournalError::Capacity)?;
            capacity::outstanding(context, &next)?.check(bytes, self.entries.len() + 1)?;
            // Allocate the future ACK backing before Pending can expose bytes.
            self.document
                .try_reserve(encoded.len())
                .map_err(|_| SourceJournalError::Capacity)?;
            Ok((row, encoded, successor_mac))
        })();
        match result {
            Ok((row, encoded, successor_mac)) => Ok(CandidateV8 {
                inventory: self,
                row,
                encoded,
                successor_mac,
            }),
            Err(error) => Err(CandidateRejectionV8 {
                inventory: self,
                row,
                error,
                physical,
            }),
        }
    }
    #[cfg(test)]
    pub(super) fn synthetic_fresh(
        context: &'a FoldContextV8,
        key: &'a SourceCheckpointKey,
    ) -> Result<Self, SourceJournalError> {
        Self::synthetic_recover(context, key, &[], Vec::new())
    }
    #[cfg(test)]
    pub(super) fn synthetic_recover(
        context: &'a FoldContextV8,
        key: &'a SourceCheckpointKey,
        document: &[u8],
        entries: Vec<ValidatedEntryV8>,
    ) -> Result<Self, SourceJournalError> {
        let model::OwnedBodyV8::OwnedRunCreated {
            scope,
            execution,
            binding,
            store_identity,
            limits,
            ..
        } = &context.created
        else {
            return Err(SourceJournalError::Binding);
        };
        let invocation = wire::recipe_digest(
            wire::RecipeV8::Invocation,
            &serde_json::json!({"execution":execution,"owned_wait_binding":binding}),
        )?;
        let generation = wire::recipe_digest(
            wire::RecipeV8::Generation,
            &serde_json::json!({"scope":scope,"execution":execution,"binding":binding,"store_identity":store_identity,"limits":limits}),
        )?;
        let zero = "0".repeat(64);
        let decoded = wire::decode_inventory(
            document,
            &ExpectedRowV8 {
                invocation: &invocation,
                generation: &generation,
                seq: 0,
                prev_mac: &zero,
                ordinary: &context.ordinary,
            },
            key,
        )?;
        if decoded.len() != entries.len()
            || decoded.iter().zip(&entries).any(|(a, b)| a != &b.entry)
        {
            return Err(SourceJournalError::Binding);
        }
        fold::fold(context, &entries)?;
        let mac = if document.is_empty() {
            zero
        } else {
            let last = document
                .strip_suffix(b"\n")
                .ok_or(SourceJournalError::Malformed)?
                .rsplit(|b| *b == b'\n')
                .next()
                .ok_or(SourceJournalError::Malformed)?;
            wire::parse(last)?["authentication"]
                .as_str()
                .ok_or(SourceJournalError::Malformed)?
                .to_owned()
        };
        Ok(Self {
            context: ContextV8::Synthetic(context),
            key,
            entries,
            invocation,
            generation,
            mac,
            document: document.to_vec(),
        })
    }
    #[cfg(test)]
    pub(super) fn synthetic_prepare(
        self,
        row: ValidatedEntryV8,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        self.prepare_inner(row.entry.clone(), None, Some(row))
    }
}
impl<'a> CandidateV8<'a> {
    /// This consuming transition must happen before the first physical call.
    pub(super) fn into_pending(self) -> PendingV8<'a> {
        PendingV8(self)
    }
}
impl<'a> PendingV8<'a> {
    /// Only Pending exposes bytes to the future adapter; Candidate cannot I/O.
    pub(super) fn bytes(&self) -> &[u8] {
        &self.0.encoded
    }
    pub(super) fn check_prefix(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
        bytes: &[u8],
    ) -> Result<(), SourceJournalError> {
        let inventory = &self.0.inventory;
        if bytes != inventory.document {
            return Err(SourceJournalError::Binding);
        }
        let ContextV8::Checked(context) = &inventory.context else {
            return Err(SourceJournalError::Binding);
        };
        let checked = super::inventory::checked_inventory_v8(context, lease, inventory.key, bytes)?;
        let (entries, mac) = checked.into_parts();
        if entries.len() != inventory.entries.len() || mac != inventory.mac {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    pub(super) fn check_written(
        &self,
        lease: &SourceOwnedWaitLeaseV8,
        bytes: &[u8],
    ) -> Result<(), SourceJournalError> {
        let inventory = &self.0.inventory;
        let total = inventory
            .document
            .len()
            .checked_add(self.0.encoded.len())
            .ok_or(SourceJournalError::Capacity)?;
        if bytes.len() != total
            || !bytes.starts_with(&inventory.document)
            || bytes[inventory.document.len()..] != self.0.encoded
        {
            return Err(SourceJournalError::Binding);
        }
        let ContextV8::Checked(context) = &inventory.context else {
            return Err(SourceJournalError::Binding);
        };
        let checked = super::inventory::checked_inventory_v8(context, lease, inventory.key, bytes)?;
        let (entries, mac) = checked.into_parts();
        if entries.len() != inventory.entries.len() + 1 || mac != self.0.successor_mac {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
    /// Only the fixed adapter can construct this sealed postappend witness.
    pub(super) fn acknowledge_verified(
        self,
        _verified: super::append::AppendVerifiedV8,
    ) -> InventoryV8<'a> {
        self.finish_ack()
    }
    pub(super) fn poison(self) -> PoisonedV8<'a> {
        PoisonedV8 { _pending: self }
    }
    pub(super) fn acknowledge(
        self,
        ack: TrustedAppendAckV8,
    ) -> Result<InventoryV8<'a>, AckRejectionV8<'a>> {
        let candidate = &self.0;
        if ack.invocation != candidate.inventory.invocation
            || ack.generation != candidate.inventory.generation
            || ack.predecessor_seq != candidate.inventory.entries.len()
            || ack.predecessor_mac != candidate.inventory.mac
            || ack.successor_mac != candidate.successor_mac
            || ack.encoded_bytes != candidate.encoded.len()
        {
            return Err(AckRejectionV8 {
                error: SourceJournalError::Binding,
                _poisoned: self.poison(),
            });
        }
        Ok(self.finish_ack())
    }
    fn finish_ack(self) -> InventoryV8<'a> {
        let CandidateV8 {
            mut inventory,
            row,
            encoded,
            successor_mac,
        } = self.0;
        inventory.document.extend_from_slice(&encoded); // reserved before Pending
        inventory.mac = successor_mac;
        inventory.entries.push(row); // capacity retained by the preflight push/pop
        inventory
    }
    #[cfg(test)]
    pub(super) fn synthetic_ack_for_inert_test(&self) -> TrustedAppendAckV8 {
        let candidate = &self.0;
        TrustedAppendAckV8 {
            invocation: candidate.inventory.invocation.clone(),
            generation: candidate.inventory.generation.clone(),
            predecessor_seq: candidate.inventory.entries.len(),
            predecessor_mac: candidate.inventory.mac.clone(),
            successor_mac: candidate.successor_mac.clone(),
            encoded_bytes: candidate.encoded.len(),
        }
    }
}

#[cfg(test)]
impl TrustedAppendAckV8 {
    pub(super) fn alter_predecessor_for_inert_test(&mut self) {
        self.predecessor_seq += 1;
    }
}
