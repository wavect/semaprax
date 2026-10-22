//! Private path-local provenance for same-owner renewal; never serialized.
use crate::cleanup::LivenessFlagId;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RenewalHistory {
    pub(super) history: Vec<LivenessFlagId>,
    // Original leaf -> current staged epoch. Staging never debits history.
    epochs: Vec<(LivenessFlagId, LivenessFlagId)>,
    protected: Option<LivenessFlagId>,
}
impl std::ops::Deref for RenewalHistory {
    type Target = Vec<LivenessFlagId>;
    fn deref(&self) -> &Self::Target {
        &self.history
    }
}
impl RenewalHistory {
    pub(super) fn new(history: Vec<LivenessFlagId>, protected: Option<LivenessFlagId>) -> Self {
        let epochs = if protected.is_some() {
            history.iter().map(|flag| (*flag, *flag)).collect()
        } else {
            Vec::new()
        };
        Self {
            history,
            epochs,
            protected,
        }
    }
    pub(super) fn stage(
        &mut self,
        mut destination: impl FnMut(LivenessFlagId) -> Option<LivenessFlagId>,
    ) {
        for (_, current) in &mut self.epochs {
            if let Some(next) = destination(*current) {
                *current = next;
            }
        }
    }
    pub(super) fn commit(&mut self, consumed: &BTreeSet<LivenessFlagId>) {
        if self.epochs.is_empty() {
            return;
        }
        let protected = self.protected;
        let mut epochs = self.epochs.iter();
        self.history.retain(|original| {
            let (expected, current) = epochs
                .next()
                .expect("reserved provenance accompanies each original leaf");
            debug_assert_eq!(original, expected);
            Some(*original) == protected || !consumed.contains(current)
        });
        self.epochs.retain(|(original, current)| {
            Some(*original) == protected || !consumed.contains(current)
        });
    }
    pub(super) fn provenance_units(&self) -> usize {
        self.epochs
            .len()
            .saturating_mul(2)
            .saturating_add(usize::from(self.protected.is_some()))
    }
    pub(super) fn retained_units(&self) -> usize {
        self.history.len().saturating_add(self.provenance_units())
    }
    /// Actual buffers created by Clone; protected is inline, not a work byte.
    pub(super) fn clone_payload_bytes(&self) -> Option<usize> {
        self.history
            .len()
            .checked_mul(std::mem::size_of::<LivenessFlagId>())?
            .checked_add(
                self.epochs
                    .len()
                    .checked_mul(std::mem::size_of::<(LivenessFlagId, LivenessFlagId)>())?,
            )
    }
    #[cfg(test)]
    pub(super) fn owned_capacity(&self) -> usize {
        self.history.capacity() * std::mem::size_of::<LivenessFlagId>()
            + self.epochs.capacity() * std::mem::size_of::<(LivenessFlagId, LivenessFlagId)>()
    }
}

// Private storage access for the selected paid twin. No proof, source or
// replay authority is supplied; caller admits this exact borrow ABI first.
impl RenewalHistory {
    pub(super) fn selected_parts(
        &mut self,
    ) -> (
        &mut Vec<LivenessFlagId>,
        &mut Vec<(LivenessFlagId, LivenessFlagId)>,
        Option<LivenessFlagId>,
    ) {
        (&mut self.history, &mut self.epochs, self.protected)
    }
    pub(super) fn from_selected_parts(
        history: Vec<LivenessFlagId>,
        epochs: Vec<(LivenessFlagId, LivenessFlagId)>,
        protected: Option<LivenessFlagId>,
    ) -> Self {
        Self {
            history,
            epochs,
            protected,
        }
    }
}
