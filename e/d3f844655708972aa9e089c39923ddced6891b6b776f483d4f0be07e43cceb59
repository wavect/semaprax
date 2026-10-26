//! Admission is independent of the actual allocation payer. An active builder
//! does not grant any selected schema, helper lease or replay shortcut.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AdmissionMode {
    Ordinary,
    SelectedRetained,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AllocationPayer {
    LegacyUnobserved,
    ActiveBuilder,
    SelectedReservation,
    /// Independently funded test oracle account; never an admission mode and
    /// never selected retention.
    #[cfg(test)]
    Oracle(oracle::OracleLease),
}
#[cfg(test)]
mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, raw_error_metered,
    replay_error_funded, InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope,
    ReplayFailure,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    set: &BTreeSet<K>,
    key: &K,
) -> Result<(), Diagnostic> {
    if !set.contains(key) {
        budget.reserve_btree_node::<K, ()>(function, "ordinary identity tree")?;
    }
    budget.reserve_clone(function, key, "ordinary identity clone")
}
pub(super) fn insert_id(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    set: &mut BTreeSet<DeclarationId>,
    key: &DeclarationId,
) -> Result<bool, Diagnostic> {
    set_key(function, budget, set, key)?;
    Ok(set.insert(key.clone()))
}
pub(super) fn leaf_payload(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    storage: &StorageId,
    projections: &Vec<DeclarationId>,
    lifecycle: &DeclarationId,
) -> Result<(), Diagnostic> {
    budget.reserve_clone(function, storage, "ordinary leaf storage")?;
    budget.reserve_clone(function, projections, "ordinary leaf projections")?;
    budget.reserve_clone(function, lifecycle, "ordinary leaf lifecycle")
}
pub(super) fn leaf_indexes(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
    places: &BTreeSet<CleanupPlace>,
    flag: LivenessFlagId,
    leaf: &Leaf,
) -> Result<(), Diagnostic> {
    if !leaves.contains_key(&flag) {
        budget.reserve_btree_node::<LivenessFlagId, Leaf>(function, "ordinary leaf index")?;
    }
    budget.reserve_clone(function, &leaf.place, "ordinary leaf clone")?;
    budget.reserve_clone(function, &leaf.lifecycle, "ordinary leaf lifecycle clone")?;
    if !places.contains(&leaf.place) {
        budget.reserve_btree_node::<CleanupPlace, ()>(function, "ordinary leaf places")?;
    }
    Ok(())
}
impl ReplayBudget {
    #[cfg(test)]
    pub(super) fn unobserved() -> Self {
        Self {
            payer: AllocationPayer::LegacyUnobserved,
            ..Self::new()
        }
    }

    #[cfg(feature = "unstable-workflow-profiling")]
    pub(in crate::cleanup_plan::replay) fn paid_phase(
        &self,
        phase: crate::workflow_profile::plan_comparison::PaidPhase,
    ) -> crate::workflow_profile::plan_comparison::PaidPhaseGuard {
        use crate::workflow_profile::plan_comparison::{paid_phase, PhasePayer};
        paid_phase(
            phase,
            match self.payer {
                AllocationPayer::LegacyUnobserved => PhasePayer::Unobserved,
                AllocationPayer::ActiveBuilder => PhasePayer::ActiveBuilder,
                AllocationPayer::SelectedReservation => PhasePayer::Selected,
                #[cfg(test)]
                AllocationPayer::Oracle(_) => PhasePayer::Oracle,
            },
        )
    }

    pub(super) fn new() -> Self {
        Self {
            admission: AdmissionMode::Ordinary,
            payer: if crate::bounded_output::active_limit().is_some() {
                AllocationPayer::ActiveBuilder
            } else {
                AllocationPayer::LegacyUnobserved
            },
            remaining: MAX_REPLAY_WORK_UNITS,
            skeleton_remaining: 0,
            skeleton_spent: 0,
            lent_skeleton: false,
            merge_paths: false,
            physical: None,
            refusal_text: None,
            refusal_spent: false,
            builder_clone_cursor_paid: false,
        }
    }
    #[cfg(test)]
    pub(super) fn with_skeleton_limit(limit: usize) -> Self {
        Self {
            admission: AdmissionMode::Ordinary,
            payer: if crate::bounded_output::active_limit().is_some() {
                AllocationPayer::ActiveBuilder
            } else {
                AllocationPayer::LegacyUnobserved
            },
            remaining: MAX_REPLAY_WORK_UNITS,
            skeleton_remaining: limit,
            skeleton_spent: 0,
            lent_skeleton: false,
            merge_paths: false,
            physical: None,
            refusal_text: None,
            refusal_spent: false,
            builder_clone_cursor_paid: false,
        }
    }

    /// Explicit opt-in: pays only the leased oracle account. Ordinary
    /// constructors above never choose this payer.
    #[cfg(test)]
    pub(super) fn with_skeleton_limit_oracle(limit: usize, lease: OracleLease) -> Self {
        Self {
            payer: AllocationPayer::Oracle(lease),
            ..Self::with_skeleton_limit(limit)
        }
    }

    /// Cumulative oracle work debit, charged before the original per-receiver
    /// ceiling is applied; that ceiling and its refusal text are unchanged. A
    /// refused or sticky account is a refusal here, never a skipped debit.
    pub(super) fn charge_oracle_work(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if let AllocationPayer::Oracle(lease) = self.payer {
            if lease.charge_work(units).is_err() {
                return Err(self.budget_refusal(
                    function,
                    "oracle account work allowance refused during ",
                    phase,
                ));
            }
        }
        let _ = (function, units, phase);
        Ok(())
    }

    /// The explicitly opted account, if this budget pays it.
    #[cfg(test)]
    pub(super) fn oracle_lease(&self) -> Option<OracleLease> {
        match self.payer {
            AllocationPayer::Oracle(lease) => Some(lease),
            _ => None,
        }
    }

    pub(super) fn selected(&self) -> bool {
        self.admission == AdmissionMode::SelectedRetained
    }
    pub(super) fn pays_allocations(&self) -> bool {
        self.payer != AllocationPayer::LegacyUnobserved
    }
    /// Payers whose refusal text and clone cursor are prepaid on a builder-like
    /// ledger: the production ACTIVE builder, or the leased oracle account.
    pub(super) fn pays_builder(&self) -> bool {
        match self.payer {
            AllocationPayer::ActiveBuilder => true,
            #[cfg(test)]
            AllocationPayer::Oracle(_) => true,
            _ => false,
        }
    }
    fn builder_refused(&self) -> bool {
        match self.payer {
            #[cfg(test)]
            AllocationPayer::Oracle(lease) => lease.refused(),
            _ => crate::bounded_output::active_refused(),
        }
    }
    /// Debit the builder-like ledger for `bytes` before construction. Oracle
    /// refusal never touches production ACTIVE.
    pub(super) fn reserve_builder(&self, bytes: usize) -> bool {
        match self.payer {
            #[cfg(test)]
            AllocationPayer::Oracle(lease) => lease.charge(bytes).is_ok(),
            _ => crate::bounded_output::reserve_active_required(bytes),
        }
    }
    /// The fixed type-clone census cursor is shared across clone preflights;
    /// pay its exact stack layout before any preflight can initialize it.
    pub(super) fn prepare_clone_cursor(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), Diagnostic> {
        if self.pays_builder() && !self.builder_clone_cursor_paid {
            self.reserve_array::<Option<(&ResolvedType, usize)>>(
                function,
                514,
                "ordinary clone census cursor",
            )?;
            self.builder_clone_cursor_paid = true;
        }
        Ok(())
    }

    /// Reserve the first refusal's backing on the existing builder before any
    /// replay allocation. Initial refusal is inline, never an unfunded string.
    pub(super) fn prepare_builder(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), Diagnostic> {
        if !self.pays_builder() {
            return Ok(());
        }
        if self.refusal_spent || self.builder_refused() {
            return Err(self.budget_refusal(function, "ordinary builder already failed", ""));
        }
        let Some(capacity) = function.id.as_str().len().checked_add(512) else {
            return Err(self.budget_refusal(
                function,
                "ordinary replay refusal layout overflowed",
                "",
            ));
        };
        if self
            .refusal_text
            .as_ref()
            .is_some_and(|text| text.capacity() >= capacity)
        {
            return Ok(());
        }
        // A later function may have a longer current ID. Charge its new
        // complete backing while the earlier buffer is still held.
        if !self.reserve_builder(capacity) {
            return Err(self.budget_refusal(
                function,
                "ordinary replay refusal reservation exhausted",
                "",
            ));
        }
        let text = String::with_capacity(capacity);
        if text.capacity() != capacity {
            return Err(self.budget_refusal(
                function,
                "ordinary replay refusal capacity differs",
                "",
            ));
        }
        self.refusal_text = Some(text);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
