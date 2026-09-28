//! Consuming inert append choreography. No sink, File, runtime owner, or ACK factory.
use super::*;
use crate::live_invocation::identity::looks_like_digest;

/// Exact acknowledged inventory; not Clone. Future recovery must obtain its
/// checked entries from the authoritative store/context join, not raw JSON.
pub(super) struct InventoryV8<'a> {
    context: FoldContextV8,
    key: &'a SourceCheckpointKey,
    entries: Vec<ValidatedEntryV8>,
    invocation: String,
    generation: String,
    mac: String,
    bytes: usize,
}
pub(super) struct CandidateV8<'a> {
    inventory: InventoryV8<'a>,
    row: ValidatedEntryV8,
    encoded: Vec<u8>,
    successor_mac: String,
}
pub(super) struct PendingV8<'a>(CandidateV8<'a>);
/// Retains all inert state permanently after append uncertainty. No retry or
/// inventory extraction exists, including after an invalid alleged ACK.
pub(super) struct PoisonedV8<'a> {
    _pending: PendingV8<'a>,
}
pub(super) struct CandidateRejectionV8<'a> {
    pub inventory: InventoryV8<'a>,
    pub row: ValidatedEntryV8,
    pub error: SourceJournalError,
}
pub(super) struct AckRejectionV8<'a> {
    pub error: SourceJournalError,
    _poisoned: PoisonedV8<'a>,
}
/// No production constructor. The physical adapter must mint this only after
/// its trusted same-store append/sync acknowledgment, under a separate lease.
pub(super) struct TrustedAppendAckV8 {
    invocation: String,
    generation: String,
    predecessor_seq: usize,
    predecessor_mac: String,
    successor_mac: String,
    encoded_bytes: usize,
}
impl<'a> InventoryV8<'a> {
    pub(super) fn fresh(
        context: FoldContextV8,
        key: &'a SourceCheckpointKey,
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
        if !looks_like_digest(&invocation) || !looks_like_digest(&generation) {
            return Err(SourceJournalError::Binding);
        }
        Ok(Self {
            context,
            key,
            entries: Vec::new(),
            invocation,
            generation,
            mac: "0".repeat(64),
            bytes: 0,
        })
    }
    pub(super) fn sequence(&self) -> usize {
        self.entries.len()
    }
    pub(super) fn acknowledged_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn prepare(
        mut self,
        row: ValidatedEntryV8,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let encoded = (|| {
            let previous = fold::fold(&self.context, &self.entries)?;
            fold::validate_producer_transition(&previous, &row)?;
            let expected = ExpectedRowV8 {
                invocation: &self.invocation,
                generation: &self.generation,
                seq: u32::try_from(self.entries.len()).map_err(|_| SourceJournalError::Capacity)?,
                prev_mac: &self.mac,
                ordinary: &self.context.ordinary,
            };
            wire::encode(&row.entry, &expected, self.key)
        })();
        let encoded = match encoded {
            Ok(encoded) => encoded,
            Err(error) => {
                return Err(CandidateRejectionV8 {
                    inventory: self,
                    row,
                    error,
                })
            }
        };
        self.entries.push(row);
        let result = (|| {
            let next = fold::fold(&self.context, &self.entries)?;
            let bytes = self
                .bytes
                .checked_add(encoded.len())
                .ok_or(SourceJournalError::Capacity)?;
            capacity::outstanding(&self.context, &next)?.check(bytes, self.entries.len())?;
            let envelope = wire::parse(
                encoded
                    .strip_suffix(b"\n")
                    .ok_or(SourceJournalError::Malformed)?,
            )?;
            Ok(envelope["authentication"]
                .as_str()
                .ok_or(SourceJournalError::Malformed)?
                .to_owned())
        })();
        let row = self.entries.pop().expect("candidate row retained");
        match result {
            Ok(successor_mac) => Ok(CandidateV8 {
                inventory: self,
                row,
                encoded,
                successor_mac,
            }),
            Err(error) => Err(CandidateRejectionV8 {
                inventory: self,
                row,
                error,
            }),
        }
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
        let CandidateV8 {
            mut inventory,
            row,
            encoded,
            successor_mac,
        } = self.0;
        inventory.bytes += encoded.len(); // checked before creating Candidate
        inventory.mac = successor_mac;
        inventory.entries.push(row);
        Ok(inventory)
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
