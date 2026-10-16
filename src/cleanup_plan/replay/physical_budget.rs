//! Selected physical allocation requests; ordinary replay keeps its exact route.
use super::*;
mod setup;
pub(in crate::cleanup_plan::replay) mod authentication;
pub(crate) use setup::{SetupFailure, SetupReason};

impl ReplayBudget {
    pub(super) fn with_physical(
        function: &ResolvedFunction,
        bytes: usize,
    ) -> Result<Self, Diagnostic> {
        Self::from_reservation(function, physical_storage::PhysicalReservation::new(bytes))
    }

    pub(super) fn from_reservation(
        function: &ResolvedFunction,
        physical: physical_storage::PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        Self::try_from_reservation(function, physical).map_err(|failure| {
            let (_, reason) = failure.into_parts();
            // Compatibility boundary: this legacy wrapper constructs an owned
            // Diagnostic without an outer paid carrier. try_from_reservation
            // itself publishes only an inline reason and the failed ledger.
            replay_error(function, reason.message())
        })
    }

    #[track_caller]
    pub(super) fn reserve_bytes(
        &mut self,
        function: &ResolvedFunction,
        bytes: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if bytes == 0 {
            // A zero request constructs nothing, so it must not bootstrap the
            // refusal String. It consults the same payer, sticky refusal,
            // reserved floor and refusal_spent state that a positive request
            // would, then authenticates only. Positive requests below keep
            // their original bootstrap and charges byte-for-byte.
            let admitted = match self.payer {
                AllocationPayer::LegacyUnobserved => true,
                AllocationPayer::ActiveBuilder => {
                    if self.refusal_spent || crate::bounded_output::active_refused() {
                        return Err(self.budget_refusal(
                            function,
                            "ordinary builder already failed",
                            "",
                        ));
                    }
                    crate::bounded_output::reserve_active_required(0)
                }
                #[cfg(test)]
                AllocationPayer::Oracle(lease) => {
                    if self.refusal_spent || lease.refused() {
                        return Err(self.budget_refusal(
                            function,
                            "ordinary builder already failed",
                            "",
                        ));
                    }
                    lease.charge(0).is_ok()
                }
                AllocationPayer::SelectedReservation => self
                    .physical
                    .as_deref_mut()
                    .expect("selected reservation payer")
                    .reserve(0)
                    .is_ok(),
            };
            return if admitted {
                Ok(())
            } else {
                Err(self.budget_refusal(
                    function,
                    "cleanup replay physical reservation exhausted during ",
                    phase,
                ))
            };
        }
        self.prepare_builder(function)?;
        let accepted = match self.payer {
            AllocationPayer::LegacyUnobserved => true,
            AllocationPayer::ActiveBuilder => {
                !self.refusal_spent
                    && !crate::bounded_output::active_refused()
                    && crate::bounded_output::reserve_active_required(bytes)
            }
            #[cfg(test)]
            AllocationPayer::Oracle(lease) => {
                !self.refusal_spent && !lease.refused() && lease.charge(bytes).is_ok()
            }
            AllocationPayer::SelectedReservation => self
                .physical
                .as_deref_mut()
                .expect("selected reservation payer")
                .reserve(bytes)
                .is_ok(),
        };
        if accepted {
            Ok(())
        } else {
            Err(self.budget_refusal(
                function,
                "cleanup replay physical reservation exhausted during ",
                phase,
            ))
        }
    }

    #[track_caller]
    pub(super) fn reserve_clone<T: ClonePayload>(
        &mut self,
        function: &ResolvedFunction,
        value: &T,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.pays_allocations() {
            return Ok(());
        }
        let without_type_cursor = self.payer == AllocationPayer::ActiveBuilder
            && !self.selected()
            && !value.clone_needs_type_cursor();
        if without_type_cursor {
            // These preflights construct no recursive type cursor. Authenticate
            // and fund refusal text first, but leave the cursor unpaid so a
            // later recursive clone must still acquire its complete backing.
            self.prepare_builder(function)?;
        } else {
            self.prepare_clone_cursor(function)?;
        }
        #[cfg(test)]
        let payload = if without_type_cursor {
            value.clone_payload_bytes_without_type_cursor()
        } else {
            match self.oracle_lease() {
                Some(lease) => value
                    .clone_payload_bytes_with_work(&mut |units| lease.charge_work(units).is_ok()),
                None => value.clone_payload_bytes(),
            }
        };
        #[cfg(not(test))]
        let payload = if without_type_cursor {
            value.clone_payload_bytes_without_type_cursor()
        } else {
            value.clone_payload_bytes()
        };
        let bytes = payload.ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay clone payload exceeds the physical bound",
                "",
            )
        })?;
        self.reserve_bytes(function, bytes, phase)
    }

    #[track_caller]
    pub(super) fn reserve_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.pays_allocations() {
            return Ok(());
        }
        let bytes = count.checked_mul(std::mem::size_of::<T>()).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay physical array layout overflowed",
                "",
            )
        })?;
        self.reserve_bytes(function, bytes, phase)
    }

    #[track_caller]
    pub(super) fn reserve_btree_node<K, V>(
        &mut self,
        function: &ResolvedFunction,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.pays_allocations() {
            return Ok(());
        }
        let bytes = physical_storage::btree_node_upper::<K, V>().ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay physical tree layout overflowed",
                "",
            )
        })?;
        self.reserve_bytes(function, bytes, phase)
    }

    #[track_caller]
    pub(super) fn reserve_deque<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut VecDeque<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.pays_allocations() {
            return Ok(());
        }
        self.reserve_bytes(function, 0, phase)?;
        let required = values.len().checked_add(additional).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay physical queue length overflowed",
                "",
            )
        })?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = physical_storage::replay_growth_capacity(values.capacity(), required)
            .ok_or_else(|| {
                self.budget_refusal(
                    function,
                    "cleanup replay physical queue capacity overflowed",
                    "",
                )
            })?;
        self.reserve_array::<T>(function, target, phase)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| {
                self.budget_refusal(
                    function,
                    "cleanup replay physical queue allocation failed",
                    "",
                )
            })?;
        if values.capacity() != target {
            return Err(self.budget_refusal(
                function,
                "cleanup replay physical queue capacity disagrees with its reservation",
                "",
            ));
        }
        Ok(())
    }

    /// The old buffer remains fully charged while its replacement is allocated.
    /// Selected growth is geometric; ordinary replay retains its original allocator
    /// growth and performs no extra scans, reservations or allocator calls here.
    #[track_caller]
    pub(super) fn reserve_vec<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut Vec<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if !self.pays_allocations() {
            return Ok(());
        }
        self.reserve_bytes(function, 0, phase)?;
        let required = values.len().checked_add(additional).ok_or_else(|| {
            self.budget_refusal(
                function,
                "cleanup replay physical vector length overflowed",
                "",
            )
        })?;
        if required <= values.capacity() {
            return Ok(());
        }
        let target = physical_storage::replay_growth_capacity(values.capacity(), required)
            .ok_or_else(|| {
                self.budget_refusal(
                    function,
                    "cleanup replay physical vector capacity overflowed",
                    "",
                )
            })?;
        self.reserve_array::<T>(function, target, phase)?;
        values
            .try_reserve_exact(target - values.len())
            .map_err(|_| {
                self.budget_refusal(
                    function,
                    "cleanup replay physical vector allocation failed",
                    "",
                )
            })?;
        if values.capacity() != target {
            return Err(self.budget_refusal(
                function,
                "cleanup replay physical vector capacity disagrees with its reservation",
                "",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod queue_tests;

// Covers the fixed replay prefix, the longest budget failure description and
// every internal operation label. No authored bytes other than the exact held
// function ID are copied here. Oversized internal labels select a fixed
// invariant description rather than allocating beyond this paid backing.
const REFUSAL_PHASE_BYTES: usize = 256;
const REFUSAL_FIXED_BYTES: usize = 512;
impl ReplayBudget {
    pub(super) fn budget_refusal(
        &mut self,
        function: &ResolvedFunction,
        reason: &str,
        phase: &str,
    ) -> Diagnostic {
        #[cfg(test)]
        if let Some(lease) = self.oracle_lease().filter(|lease| !lease.refused()) {
            // The diagnostic keeps its complete dynamic reason. The inline
            // account stores a static category only when no prior refusal
            // has selected the failure; byte/work/authentication stay first.
            lease.fail_physical("oracle replay budget refused physical construction");
        }

        if self.payer == AllocationPayer::LegacyUnobserved
            || (self.payer == AllocationPayer::SelectedReservation
                && self.refusal_text.is_none()
                && !self.refusal_spent)
        {
            // Detached test ledgers retain their previous unadmitted route;
            // only from_reservation establishes a prepaid selected budget.
            return replay_error(function, format_args!("{reason}{phase}"));
        }
        self.refusal_spent = true;
        self.remaining = 0;
        self.skeleton_remaining = 0;
        if let Some(physical) = self.physical.as_deref_mut() {
            physical.reject();
        }
        let Some(mut text) = self.refusal_text.take() else {
            // A selected budget is sticky; no second diagnostic allocation can
            // spend a consumed reservation. The stable error identity remains.
            return Diagnostic::io("SPX-H006", String::new());
        };
        let phase = if phase.len() <= REFUSAL_PHASE_BYTES {
            phase
        } else {
            "internal operation label exceeds the prepaid refusal bound"
        };
        let pieces = [
            "cleanup plan for function `",
            function.id.as_str(),
            "` failed independent replay: ",
            reason,
            phase,
        ];
        let required = pieces
            .iter()
            .try_fold(0usize, |n, part| n.checked_add(part.len()));
        if required.is_some_and(|n| n <= text.capacity()) {
            for part in pieces {
                text.push_str(part);
            }
        } else {
            text.push_str("cleanup replay internal refusal text exceeds its prepaid backing");
        }
        Diagnostic::io("SPX-H006", text)
    }
}
