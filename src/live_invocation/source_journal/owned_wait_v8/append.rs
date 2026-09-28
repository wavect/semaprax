//! Fixed physical append adapter. No owner restoration or phase grant.
use super::candidate::{CandidateRejectionV8, InventoryV8, PendingV8};
use super::*;
use crate::resumable_effects::owned_frame::{
    SourceOwnedWaitLeaseV8, SourceOwnedWaitStoreRegistrationV8,
};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

/// Owns the one held lock. Neither the lease nor its file can be extracted.
pub(crate) struct SourceOwnedWaitJournalV8 {
    context: Arc<CheckedOwnedWaitJournalContextV8>,
    key: SourceCheckpointKey,
    poisoned: Cell<bool>,
    append_active: Cell<bool>,
    prospective_reduce: RefCell<Option<ProspectiveReduceRegistryV8>>,
    prospective_reduce_identity: Cell<u64>,
    #[cfg(test)]
    panic_after_append: Cell<bool>,
    lease: RefCell<SourceOwnedWaitLeaseV8>,
}
struct ProspectiveReduceRegistryV8 {
    phase: owned_effect::OwnedReduceHoldPhaseV8,
    identity: u64,
    sequence: usize,
    bytes: usize,
    authentication: String,
    turn: u32,
    attempt: u32,
    fuel: u64,
}
/// Move-only guard retained by physical backing holders.
pub(crate) struct HeldOwnedWaitStoreV8<'a> {
    journal: &'a SourceOwnedWaitJournalV8,
}
pub(super) struct AppendSessionV8<'a> {
    journal: &'a SourceOwnedWaitJournalV8,
    inventory: InventoryV8<'a>,
}
pub(super) enum AppendFailureV8<'a> {
    PhysicalBeforeCandidate {
        _session: AppendSessionV8<'a>,
        _row: EntryV8,
        error: SourceJournalError,
    },
    CandidateRefused {
        session: AppendSessionV8<'a>,
        row: EntryV8,
        error: SourceJournalError,
    },
    PrewriteRefused {
        _journal: &'a SourceOwnedWaitJournalV8,
        _pending: PendingV8<'a>,
        error: SourceJournalError,
    },
    InDoubt {
        _journal: &'a SourceOwnedWaitJournalV8,
        _pending: PendingV8<'a>,
        error: SourceJournalError,
    },
}
/// Private construction is confined to the fixed post-write verification path.
pub(super) struct AppendVerifiedV8 {
    _sealed: (),
}
struct Attempting<'a> {
    journal: &'a SourceOwnedWaitJournalV8,
    attempted: Cell<bool>,
    complete: Cell<bool>,
}
impl Drop for Attempting<'_> {
    fn drop(&mut self) {
        if !self.complete.get() {
            self.journal.poisoned.set(true);
        }
        self.journal.append_active.set(false);
    }
}
impl SourceOwnedWaitJournalV8 {
    pub(crate) fn open(
        context: Arc<CheckedOwnedWaitJournalContextV8>,
        key: SourceCheckpointKey,
        lease: SourceOwnedWaitLeaseV8,
    ) -> Result<Self, SourceJournalError> {
        context.validate_lease(&lease)?;
        Ok(Self {
            context,
            key,
            poisoned: Cell::new(false),
            append_active: Cell::new(false),
            prospective_reduce: RefCell::new(None),
            prospective_reduce_identity: Cell::new(0),
            #[cfg(test)]
            panic_after_append: Cell::new(false),
            lease: RefCell::new(lease),
        })
    }
    pub(super) fn context(&self) -> &CheckedOwnedWaitJournalContextV8 {
        &self.context
    }
    pub(crate) fn hold(&self) -> Result<HeldOwnedWaitStoreV8<'_>, SourceJournalError> {
        self.validate_guard()?;
        Ok(HeldOwnedWaitStoreV8 { journal: self })
    }
    fn validate_guard(&self) -> Result<(), SourceJournalError> {
        if self.append_active.get() {
            return Err(SourceJournalError::Order);
        }
        self.validate_adapter_guard()
    }
    // The fixed adapter may validate while holding its own active marker.
    fn validate_adapter_guard(&self) -> Result<(), SourceJournalError> {
        if self.poisoned.get() {
            return Err(SourceJournalError::Poisoned);
        }
        let lease = self
            .lease
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?;
        self.context.validate_lease(&lease).inspect_err(|_| {
            self.poisoned.set(true);
        })
    }
    // Acquisition-only packet: no closed live phase route can bypass this.
    fn validate_generic_write(&self) -> Result<(), SourceJournalError> {
        if self
            .prospective_reduce
            .try_borrow()
            .map_err(|_| SourceJournalError::Order)?
            .is_some()
        {
            return Err(SourceJournalError::Order);
        }
        Ok(())
    }
    pub(super) fn begin_session(&self) -> Result<AppendSessionV8<'_>, SourceJournalError> {
        self.validate_guard()?;
        let recovered = (|| {
            let mut lease = self
                .lease
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let bytes = lease.read().map_err(store_error)?;
            InventoryV8::recover(&self.context, &lease, &self.key, &bytes)
        })();
        let inventory = recovered.inspect_err(|_| self.poisoned.set(true))?;
        self.validate_guard()?;
        Ok(AppendSessionV8 {
            journal: self,
            inventory,
        })
    }
}
impl HeldOwnedWaitStoreV8<'_> {
    pub(crate) fn validate_guard(&self) -> Result<(), SourceJournalError> {
        self.journal.validate_guard()
    }
    /// Proof-only current-tail check; this never returns a key, session or owner.
    pub(crate) fn validate_prefix(
        &self,
        sequence: usize,
        bytes: usize,
    ) -> Result<(), SourceJournalError> {
        self.validate_guard()?;
        let current = self.journal.begin_session()?;
        if current.sequence() != sequence || current.acknowledged_bytes() != bytes {
            self.journal.poisoned.set(true);
            return Err(SourceJournalError::Order);
        }
        self.validate_guard()
    }
    /// Permanent quarantine of an actual callback failure; no authority is minted.
    pub(crate) fn quarantine(&self) {
        self.journal.poisoned.set(true);
    }
    pub(crate) fn generation(&self) -> &str {
        self.journal.context.generation()
    }
    pub(crate) fn registration(&self) -> &SourceOwnedWaitStoreRegistrationV8 {
        self.journal.context.registration()
    }
}
impl<'a> AppendSessionV8<'a> {
    #[cfg(test)]
    pub(super) fn fold_for_live_test(&self) -> super::fold::FoldV8 {
        self.inventory.fold_for_live_test()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal, journal)
    }
    pub(super) fn sequence(&self) -> usize {
        self.inventory.sequence()
    }
    pub(super) fn acknowledged_bytes(&self) -> usize {
        self.inventory.acknowledged_bytes()
    }
    pub(super) fn append(self, row: EntryV8) -> Result<Self, AppendFailureV8<'a>> {
        let journal = self.journal;
        if let Err(error) = journal.validate_guard() {
            return Err(AppendFailureV8::PhysicalBeforeCandidate {
                _session: self,
                _row: row,
                error,
            });
        }
        if let Err(error) = journal.validate_generic_write() {
            return Err(AppendFailureV8::CandidateRefused {
                session: self,
                row,
                error,
            });
        }
        let prepared = {
            let lease = match journal.lease.try_borrow() {
                Ok(lease) => lease,
                Err(_) => {
                    return Err(AppendFailureV8::CandidateRefused {
                        session: self,
                        row,
                        error: SourceJournalError::Order,
                    })
                }
            };
            self.inventory.prepare(&lease, row)
        };
        let candidate = match prepared {
            Ok(candidate) => candidate,
            Err(rejected) => return Err(reject_candidate(journal, rejected)),
        };
        let pending = candidate.into_pending();
        // Pending exists before the first append-adapter physical call.
        if let Err(error) = journal.validate_guard() {
            journal.poisoned.set(true);
            return Err(AppendFailureV8::PrewriteRefused {
                _journal: journal,
                _pending: pending,
                error,
            });
        }
        journal.append_active.set(true);
        let attempting = Attempting {
            journal,
            attempted: Cell::new(false),
            complete: Cell::new(false),
        };
        let verified = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            physical_append(&attempting, &pending)
        }));
        match verified {
            Ok(Ok(witness)) => {
                let inventory = pending.acknowledge_verified(witness);
                attempting.complete.set(true);
                Ok(Self { journal, inventory })
            }
            Ok(Err(error)) if !attempting.attempted.get() => {
                Err(AppendFailureV8::PrewriteRefused {
                    _journal: journal,
                    _pending: pending,
                    error,
                })
            }
            Ok(Err(error)) => Err(AppendFailureV8::InDoubt {
                _journal: journal,
                _pending: pending,
                error,
            }),
            Err(_) => Err(AppendFailureV8::InDoubt {
                _journal: journal,
                _pending: pending,
                error: SourceJournalError::Uncertain,
            }),
        }
    }
}
fn reject_candidate<'a>(
    journal: &'a SourceOwnedWaitJournalV8,
    rejected: CandidateRejectionV8<'a>,
) -> AppendFailureV8<'a> {
    let physical = if rejected.physical {
        journal.poisoned.set(true);
        Some(rejected.error)
    } else {
        journal.validate_guard().err()
    };
    let session = AppendSessionV8 {
        journal,
        inventory: rejected.inventory,
    };
    if let Some(error) = physical {
        AppendFailureV8::PhysicalBeforeCandidate {
            _session: session,
            _row: rejected.row,
            error,
        }
    } else {
        AppendFailureV8::CandidateRefused {
            session,
            row: rejected.row,
            error: rejected.error,
        }
    }
}

fn physical_append(
    attempting: &Attempting<'_>,
    pending: &PendingV8<'_>,
) -> Result<AppendVerifiedV8, SourceJournalError> {
    let journal = attempting.journal;
    journal.validate_adapter_guard()?;
    journal.validate_generic_write()?;
    {
        let mut lease = journal
            .lease
            .try_borrow_mut()
            .map_err(|_| SourceJournalError::Order)?;
        lease
            .validate_append_authorized(journal.context.registration())
            .map_err(store_error)?;
        let bytes = lease.read().map_err(store_error)?;
        pending.check_prefix(&lease, &bytes)?;
        lease
            .validate_append_authorized(journal.context.registration())
            .map_err(store_error)?;
        journal.validate_generic_write()?;
        attempting.attempted.set(true);
        lease.append(pending.bytes()).map_err(store_error)?;
    }
    #[cfg(test)]
    if journal.panic_after_append.get() {
        panic!("closed postappend panic fault");
    }
    journal.validate_adapter_guard()?;
    {
        let mut lease = journal
            .lease
            .try_borrow_mut()
            .map_err(|_| SourceJournalError::Order)?;
        let bytes = lease.read().map_err(store_error)?;
        pending.check_written(&lease, &bytes)?;
    }
    journal.validate_adapter_guard()?;
    journal.validate_generic_write()?;
    Ok(AppendVerifiedV8 { _sealed: () })
}

fn store_error(
    error: crate::resumable_effects::owned_frame::OwnedFrameError,
) -> SourceJournalError {
    use crate::resumable_effects::owned_frame::OwnedFrameError;
    match error {
        OwnedFrameError::Capacity => SourceJournalError::Capacity,
        OwnedFrameError::InDoubt | OwnedFrameError::Storage => SourceJournalError::Uncertain,
        _ => SourceJournalError::Binding,
    }
}

#[cfg(all(test, unix))]
mod tests;

mod checkpoint;

// Only the fixed child may mint a live Ready successor witness.
pub(super) mod owned_effect;
