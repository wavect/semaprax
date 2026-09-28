//! Fixed physical append adapter. No owner restoration or phase grant.
use super::candidate::{InventoryV8, PendingV8};
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
    #[cfg(test)]
    panic_after_append: Cell<bool>,
    lease: RefCell<SourceOwnedWaitLeaseV8>,
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
            #[cfg(test)]
            panic_after_append: Cell::new(false),
            lease: RefCell::new(lease),
        })
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
        self.context.validate_lease(&lease)
    }
    pub(super) fn begin_session(&self) -> Result<AppendSessionV8<'_>, SourceJournalError> {
        self.validate_guard()?;
        let inventory = {
            let mut lease = self
                .lease
                .try_borrow_mut()
                .map_err(|_| SourceJournalError::Order)?;
            let bytes = lease.read().map_err(store_error)?;
            InventoryV8::recover(&self.context, &lease, &self.key, &bytes)?
        };
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
    pub(crate) fn generation(&self) -> &str {
        self.journal.context.generation()
    }
    pub(crate) fn registration(&self) -> &SourceOwnedWaitStoreRegistrationV8 {
        self.journal.context.registration()
    }
}
impl<'a> AppendSessionV8<'a> {
    pub(super) fn sequence(&self) -> usize {
        self.inventory.sequence()
    }
    pub(super) fn acknowledged_bytes(&self) -> usize {
        self.inventory.acknowledged_bytes()
    }
    pub(super) fn append(self, row: EntryV8) -> Result<Self, AppendFailureV8<'a>> {
        let journal = self.journal;
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
            Err(rejected) => {
                return Err(AppendFailureV8::CandidateRefused {
                    session: Self {
                        journal,
                        inventory: rejected.inventory,
                    },
                    row: rejected.row,
                    error: rejected.error,
                })
            }
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
fn physical_append(
    attempting: &Attempting<'_>,
    pending: &PendingV8<'_>,
) -> Result<AppendVerifiedV8, SourceJournalError> {
    let journal = attempting.journal;
    journal.validate_adapter_guard()?;
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
