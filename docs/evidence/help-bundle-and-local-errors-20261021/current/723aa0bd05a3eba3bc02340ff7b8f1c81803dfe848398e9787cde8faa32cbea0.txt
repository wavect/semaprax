//! Selected physical allocation requests; ordinary replay keeps its exact route.
use super::*;
pub(in crate::cleanup_plan::replay) mod authentication;
mod setup;
pub(crate) use setup::{SetupFailure, SetupReason};

// Byte-payment results carry the complete fixed H006 message, including a
// borrowed oracle refusal. Public diagnostic fields are reconstructed only at
// the existing semantic Result boundary; no success path constructs them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ByteFailure {
    pub(super) code: &'static str,
    pub(super) message: std::borrow::Cow<'static, str>,
}
impl ByteFailure {
    pub(super) fn from_budget(diagnostic: Diagnostic) -> Self {
        let Diagnostic {
            code: "SPX-H006",
            severity: crate::diagnostic::Severity::Error,
            message,
            path: None,
            span: None,
            help: None,
        } = diagnostic
        else {
            panic!("byte reservation diagnostic representation changed");
        };
        Self {
            code: "SPX-H006",
            message,
        }
    }
}
impl ByteFailure {
    pub(super) fn into_diagnostic(self) -> Diagnostic {
        Diagnostic {
            code: self.code,
            severity: crate::diagnostic::Severity::Error,
            message: self.message,
            path: None,
            span: None,
            help: None,
        }
    }
}

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

    #[cfg(not(test))]
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

    #[cfg(test)]
    #[track_caller]
    pub(super) fn reserve_bytes(
        &mut self,
        function: &ResolvedFunction,
        bytes: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        let ticket = super::merge_census::before(self, function, bytes, phase);
        let result = self.reserve_bytes_without_merge_census(function, bytes, phase);
        super::merge_census::after(ticket, self, function, result.is_ok());
        result
    }

    #[cfg(test)]
    #[track_caller]
    pub(super) fn reserve_bytes_without_merge_census(
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
pub(super) const REFUSAL_PHASE_BYTES: usize = 256;
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

// Compact producers are selected only by the funded branch-copy hot paths.
impl ReplayBudget {
    #[cfg(not(test))]
    #[track_caller]
    pub(super) fn reserve_compact_bytes(
        &mut self,
        function: &ResolvedFunction,
        bytes: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
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
                        return Err(self.compact_refusal(
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
                        return Err(self.compact_refusal(
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
                Err(self.compact_refusal(
                    function,
                    "cleanup replay physical reservation exhausted during ",
                    phase,
                ))
            };
        }
        match self.prepare_builder(function) {
            Ok(()) => (),
            Err(error) => return Err(ByteFailure::from_budget(error)),
        };
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
            Err(self.compact_refusal(
                function,
                "cleanup replay physical reservation exhausted during ",
                phase,
            ))
        }
    }

    #[cfg(test)]
    #[track_caller]
    pub(super) fn reserve_compact_bytes(
        &mut self,
        function: &ResolvedFunction,
        bytes: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
        let ticket = super::merge_census::before(self, function, bytes, phase);
        let result = self.reserve_compact_bytes_without_merge_census(function, bytes, phase);
        super::merge_census::after(ticket, self, function, result.is_ok());
        result
    }

    #[cfg(test)]
    #[track_caller]
    pub(super) fn reserve_compact_bytes_without_merge_census(
        &mut self,
        function: &ResolvedFunction,
        bytes: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
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
                        return Err(self.compact_refusal(
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
                        return Err(self.compact_refusal(
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
                Err(self.compact_refusal(
                    function,
                    "cleanup replay physical reservation exhausted during ",
                    phase,
                ))
            };
        }
        match self.prepare_builder(function) {
            Ok(()) => (),
            Err(error) => return Err(ByteFailure::from_budget(error)),
        };
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
            Err(self.compact_refusal(
                function,
                "cleanup replay physical reservation exhausted during ",
                phase,
            ))
        }
    }

    #[track_caller]
    pub(super) fn reserve_compact_clone<T: ClonePayload>(
        &mut self,
        function: &ResolvedFunction,
        value: &T,
        phase: &str,
    ) -> Result<(), ByteFailure> {
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
            match self.prepare_builder(function) {
                Ok(()) => (),
                Err(error) => return Err(ByteFailure::from_budget(error)),
            };
        } else {
            match self.prepare_clone_cursor(function) {
                Ok(()) => (),
                Err(error) => return Err(ByteFailure::from_budget(error)),
            };
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
            self.compact_refusal(
                function,
                "cleanup replay clone payload exceeds the physical bound",
                "",
            )
        })?;
        self.reserve_compact_bytes(function, bytes, phase)
    }

    #[track_caller]
    pub(super) fn reserve_compact_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
        if !self.pays_allocations() {
            return Ok(());
        }
        let bytes = count.checked_mul(std::mem::size_of::<T>()).ok_or_else(|| {
            self.compact_refusal(
                function,
                "cleanup replay physical array layout overflowed",
                "",
            )
        })?;
        self.reserve_compact_bytes(function, bytes, phase)
    }
}

// Same sticky logical-work producer and operation order as ReplayBudget::charge_skeleton.
impl ReplayBudget {
    pub(super) fn charge_compact_skeleton(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
        match self.prepare_builder(function) {
            Ok(()) => (),
            Err(error) => return Err(ByteFailure::from_budget(error)),
        };
        if self.refusal_spent {
            return Err(self.compact_refusal(
                function,
                "selected replay budget already failed",
                "",
            ));
        }
        match self.charge_oracle_work(function, units, phase) {
            Ok(()) => (),
            Err(error) => return Err(ByteFailure::from_budget(error)),
        };
        let spent = self.skeleton_spent.checked_add(units).ok_or_else(|| {
            self.compact_refusal(
                function,
                "cleanup replay skeleton debit count overflowed",
                "",
            )
        })?;
        self.skeleton_remaining = self.skeleton_remaining.checked_sub(units).ok_or_else(|| {
            self.compact_refusal(
                function,
                "cleanup replay work budget exhausted during ",
                phase,
            )
        })?;
        self.skeleton_spent = spent;
        Ok(())
    }
}

// Direct private refusal preserves the complete original status/message producer.
impl ReplayBudget {
    pub(super) fn compact_refusal(
        &mut self,
        function: &ResolvedFunction,
        reason: &str,
        phase: &str,
    ) -> ByteFailure {
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
            return ByteFailure::from_budget(replay_error(
                function,
                format_args!("{reason}{phase}"),
            ));
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
            return ByteFailure {
                code: "SPX-H006",
                message: std::borrow::Cow::Owned(String::new()),
            };
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
        ByteFailure {
            code: "SPX-H006",
            message: std::borrow::Cow::Owned(text),
        }
    }
}

#[cfg(test)]
mod byte_failure_tests {
    use super::*;
    use crate::test_allocation_observer::measure;
    use std::borrow::Cow;

    #[test]
    fn compact_byte_error_preserves_all_public_fields_message_ownership_and_zero_backing() {
        for message in [
            Cow::Borrowed(""),
            Cow::Borrowed("borrowed oracle refusal"),
            Cow::Owned("quoted \"text\", NUL\0, LF\n, UTF-8 λ".to_owned()),
        ] {
            let diagnostic = Diagnostic {
                code: "SPX-H006",
                severity: crate::diagnostic::Severity::Error,
                message,
                path: None,
                span: None,
                help: None,
            };
            let json = diagnostic.json();
            let pointer = diagnostic.message.as_ptr();
            let capacity = diagnostic.message_owned_capacity();
            let borrowed = matches!(&diagnostic.message, Cow::Borrowed(_));
            let (restored, metrics) =
                measure(|| ByteFailure::from_budget(diagnostic).into_diagnostic());
            assert_eq!(restored.json(), json);
            assert_eq!(restored.message.as_ptr(), pointer);
            assert_eq!(restored.message_owned_capacity(), capacity);
            assert_eq!(matches!(&restored.message, Cow::Borrowed(_)), borrowed);
            assert_eq!(metrics, crate::test_allocation_observer::Metrics::default());
        }
    }

    #[test]
    fn compact_byte_error_refuses_extended_diagnostic_fields_instead_of_discarding_them() {
        for field in 0..5 {
            let mut diagnostic = Diagnostic::io("SPX-H006", "original refusal");
            match field {
                0 => diagnostic.code = "SPX-H999",
                1 => diagnostic.severity = crate::diagnostic::Severity::Warning,
                2 => diagnostic.path = Some("other.spx".into()),
                3 => {
                    diagnostic.span = Some(crate::ast::Span {
                        line: 2,
                        column: 3,
                        start: 4,
                        end: 5,
                    })
                }
                4 => diagnostic.help = Some("extra field".into()),
                _ => unreachable!(),
            }
            assert!(std::panic::catch_unwind(|| ByteFailure::from_budget(diagnostic)).is_err());
        }
    }

    #[test]
    fn compact_byte_error_result_layout_is_smaller_than_the_unchanged_public_diagnostic() {
        use std::mem::{align_of, size_of};
        assert!(size_of::<ByteFailure>() < size_of::<Diagnostic>());
        assert!(align_of::<ByteFailure>() <= align_of::<Diagnostic>());
        assert!(size_of::<Result<(), ByteFailure>>() < size_of::<Result<(), Diagnostic>>());
        assert!(size_of::<Result<usize, ByteFailure>>() < size_of::<Result<usize, Diagnostic>>());
    }
}

// Only the selected local callers use this conversion. No global From impl
// participates in unrelated Result inference; successful calls retain ByteFailure.
macro_rules! compact_try {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => return Err(error.into_diagnostic()),
        }
    };
}
pub(super) use compact_try;
