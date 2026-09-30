//! Fixed continued Intent funnel; the common verified append body is unchanged.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::FixedOwnedContinuedIntentAppendPermitV8;
impl<'a> AppendSessionV8<'a> {
    pub(super) fn begin_fixed_continued_intent_append(
        self,
        permit: &FixedOwnedContinuedIntentAppendPermitV8<'_, 'a>,
    ) -> Result<(PendingV8<'a>, AppendVerifiedV8, Attempting<'a>), AppendFailureV8<'a>> {
        let journal = self.journal;
        let row = permit.selected_row().clone();
        if let Err(error) = journal.validate_guard() {
            return Err(AppendFailureV8::PhysicalBeforeCandidate {
                _session: self,
                _row: row,
                error,
            });
        }
        if let Err(error) = permit
            .validate_preflight(journal)
            .and_then(|_| permit.validate_selected_prefix(journal, &self.inventory))
        {
            return Err(AppendFailureV8::PhysicalBeforeCandidate {
                _session: self,
                _row: row,
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
            self.inventory
                .prepare_fixed_continued_intent(&lease, journal, permit)
        };
        let candidate = match prepared {
            Ok(c) => c,
            Err(rejected) => return Err(reject_candidate(journal, rejected)),
        };
        let pending = candidate.into_pending();
        // Pending exists before physical preflight; every callback is outside a
        // lease borrow and the active marker. Final callback-free checks follow.
        if let Err(error) = permit
            .validate_preflight(journal)
            .and_then(|_| journal.validate_guard())
            .and_then(|_| pending.validate_fixed_continued_intent_prefix(journal, permit))
        {
            journal.quarantine();
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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::physical_append_fixed_continued_intent(&attempting, &pending, permit)
        }));
        match result {
            Ok(Ok(verified)) => Ok((pending, verified, attempting)),
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
    fn physical_append_fixed_continued_intent(
        attempting: &Attempting<'_>,
        pending: &PendingV8<'_>,
        permit: &FixedOwnedContinuedIntentAppendPermitV8<'_, '_>,
    ) -> Result<AppendVerifiedV8, SourceJournalError> {
        let journal = attempting.journal;
        journal.validate_adapter_guard()?;
        pending.validate_fixed_continued_intent_prefix(journal, permit)?;
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
            pending.validate_fixed_continued_intent_prefix(journal, permit)?;
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
        pending.validate_fixed_continued_intent_prefix(journal, permit)?;
        Ok(AppendVerifiedV8 { _sealed: () })
    }
}
