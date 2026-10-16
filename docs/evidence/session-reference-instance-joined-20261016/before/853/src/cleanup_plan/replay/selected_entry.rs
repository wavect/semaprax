//! A private primitive selected entry owns one cumulative reservation.
//! Independent replay reconstructs every fact from the immutable context; no
//! attached schema or certificate is an admission shortcut.
use super::*;
mod paid;
use paid::PaidReport;
pub(crate) use paid::{Acquired, PaidRefusal, Refused, ReportForm, Unpaid};
#[cfg(test)]
pub(crate) use paid::{Cause, UnpaidReason};

#[cfg(test)]
pub(crate) fn validate_retained_lexical_with_limit(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    bytes: usize,
) -> Result<(), Diagnostic> {
    validate_retained_lexical_with_reservation(program, function, PhysicalReservation::new(bytes))
        .map(|_| ())
}

#[cfg(test)]
pub(crate) fn validate_retained_lexical_with_reservation(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    physical: PhysicalReservation,
) -> Result<PhysicalReservation, Diagnostic> {
    let mut session = Session::new(program, function, physical)?;
    session.validate(function)?;
    session.finish()
}

/// Non-clone preparation owner: authentication and replay consume one ledger.
pub(crate) struct Preparation {
    budget: ReplayBudget,
    failed: bool,
    /// One first receipt: the original paid/legacy Diagnostic by move, or
    /// typed expression-fact data with shared source identity handles.
    receipt: Option<expression_facts::errors::EngineError>,
    // None is confined to legacy test constructors; production acquires both owners.
    report: Option<PaidReport>,
}
// The owned-Diagnostic constructor and retry methods below are test-only
// legacy APIs: a failed retry allocates `replay_error`. Production uses the
// inline `try_new` and the Copy-refusing `try_*`/`construct` in `paid.rs`.
impl Preparation {
    #[cfg(test)]
    pub(crate) fn new(
        function: &ResolvedFunction,
        physical: PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        Self::try_new(function, physical).map_err(|failure| {
            let (physical, reason) = failure.into_parts();
            #[allow(
                clippy::drop_non_drop,
                reason = "End this borrowed proof at the measured lifetime boundary."
            )]
            drop(physical);
            // Legacy outward Diagnostic has no separately funded report carrier.
            replay_error(function, reason.message())
        })
    }
    fn try_new(
        function: &ResolvedFunction,
        physical: PhysicalReservation,
    ) -> Result<Self, physical_budget::SetupFailure> {
        Ok(Self {
            budget: ReplayBudget::try_from_reservation(function, physical)?,
            failed: false,
            receipt: None,
            report: None,
        })
    }
    #[cfg(test)]
    pub(crate) fn with_limits(
        function: &ResolvedFunction,
        work: usize,
        bytes: usize,
    ) -> Result<Self, Diagnostic> {
        if work > MAX_REPLAY_WORK_UNITS {
            return Err(replay_error(
                function,
                "test preparation work exceeds production ceiling",
            ));
        }
        let mut preparation = Self::new(function, PhysicalReservation::new(bytes))?;
        preparation.budget.remaining = work;
        Ok(preparation)
    }
    #[cfg(test)]
    pub(crate) fn charge(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let answer = self.budget.charge(function, units, phase);
        self.failed = answer.is_err();
        answer
    }
    #[cfg(test)]
    pub(crate) fn reserve_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let answer = self.budget.reserve_array::<T>(function, count, phase);
        self.failed = answer.is_err();
        answer
    }
    #[cfg(test)]
    pub(crate) fn reserve_vec<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut Vec<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected preparation already failed",
            ));
        }
        let answer = self.budget.reserve_vec(function, values, additional, phase);
        self.failed = answer.is_err();
        answer
    }
    #[cfg(test)]
    pub(crate) fn remaining(&self) -> usize {
        self.budget.remaining
    }
    // `construct` lives in `paid.rs`: its refusal is the Copy `Refused`, and
    // work exhaustion spends the prepaid refusal text instead of formatting.
    /// Status-only owning observation, distinct from an owned-Diagnostic retry.
    /// It never restores work or replaces the held physical account.
    #[cfg(test)]
    fn probe_zero(&mut self, function: &ResolvedFunction) -> Result<(), ()> {
        if self.failed {
            return Err(());
        }
        let result = self
            .budget
            .charge(function, 0, "selected zero-unit state probe");
        self.failed = result.is_err();
        result.map_err(drop)
    }

    #[cfg(test)]
    fn usage(&self) -> (usize, usize) {
        (
            self.budget.remaining,
            self.budget.physical.as_ref().unwrap().charged(),
        )
    }

    pub(crate) fn fail(&mut self) {
        self.failed = true;
    }
}

/// One held provider, borrowed callee inventory, and cumulative work ledger.
/// Each selected member still obtains a fresh function-bound renewal lease.
pub(crate) struct Session<'a> {
    indexed: selected_callees::Context<'a>,
    budget: ReplayBudget,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "read only by the test-only legacy `finish`")
    )]
    anchor: &'a ResolvedFunction,
    failed: bool,
    report: Option<PaidReport>,
}
// `new`, `from_preparation`, `from_budget` and `finish` are test-only legacy
// owners whose refusals allocate an owned Diagnostic. The production route
// uses `try_from_preparation` and `finish_paid` (`paid.rs`).
impl<'a> Session<'a> {
    #[cfg(test)]
    pub(crate) fn new(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        physical: PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        let budget = ReplayBudget::from_reservation(anchor, physical)?;
        Self::from_budget(program, anchor, budget)
    }

    #[cfg(test)]
    pub(crate) fn from_preparation(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        preparation: Preparation,
    ) -> Result<Self, Diagnostic> {
        if preparation.failed {
            return Err(replay_error(anchor, "selected preparation already failed"));
        }
        Self::try_from_preparation(program, anchor, preparation).map_err(|(preparation, error)| {
            drop(preparation);
            error
        })
    }

    /// Preserve the exact cumulative owner and first diagnostic on construction
    /// failure. The borrowed callee index is built once; no preflight is replayed.
    pub(crate) fn try_from_preparation(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        mut preparation: Preparation,
    ) -> Result<Self, (Preparation, Diagnostic)> {
        if preparation.failed {
            let error = preparation.budget.budget_refusal(
                anchor,
                "selected preparation already failed",
                "",
            );
            return Err((preparation, error));
        }
        let indexed = (|| {
            preparation.budget.reserve_array::<Self>(
                anchor,
                1,
                "selected module replay session",
            )?;
            selected_callees::Context::new(program, anchor, &mut preparation.budget)
        })();
        match indexed {
            Ok(indexed) => Ok(Self {
                indexed,
                budget: preparation.budget,
                report: preparation.report,
                anchor,
                failed: false,
            }),
            Err(error) => {
                preparation.failed = true;
                Err((preparation, error))
            }
        }
    }

    #[cfg(test)]
    fn from_budget(
        program: &'a dyn ReplayContext,
        anchor: &'a ResolvedFunction,
        budget: ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        Self::from_preparation(
            program,
            anchor,
            Preparation {
                budget,
                failed: false,
                receipt: None,
                report: None,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn validate(&mut self, function: &ResolvedFunction) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        // Keep refusal sticky without publishing a diagnostic from Drop or
        // allowing another member to restart the spent work allowance.
        self.failed = true;
        require_entry(function)?;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        lexical_replay::validate_with_index(&context, function, &mut self.budget, None)?;
        self.failed = false;
        Ok(())
    }

    /// Partial typed engine boundary: expression-fact and four structural
    /// families retain sealed semantic data. Other children keep Diagnostics.
    pub(crate) fn validate_paid(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), expression_facts::errors::EngineError> {
        if self.failed {
            return Err(expression_facts::errors::EngineError::Failed);
        }
        self.failed = true;
        let context =
            selected_context::Context::new_paid(&self.indexed, function, &mut self.budget, true)?;
        lexical_replay::validate_paid(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }

    pub(crate) fn validate_scalar_paid(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), expression_facts::errors::EngineError> {
        if self.failed {
            return Err(expression_facts::errors::EngineError::Failed);
        }
        self.failed = true;
        let context =
            selected_context::Context::new_paid(&self.indexed, function, &mut self.budget, false)?;
        scalar_replay::validate_paid(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn validate_paid_original(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), expression_facts::errors::EngineError> {
        if self.failed {
            return Err(expression_facts::errors::EngineError::Failed);
        }
        self.failed = true;
        require_entry(function)?;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        lexical_replay::validate_paid(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn validate_scalar_paid_original(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), expression_facts::errors::EngineError> {
        if self.failed {
            return Err(expression_facts::errors::EngineError::Failed);
        }
        self.failed = true;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        scalar_replay::validate_paid(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn metrics(&self) -> (usize, usize) {
        (
            self.budget.remaining,
            self.budget
                .physical
                .as_ref()
                .expect("selected backing retained")
                .charged(),
        )
    }
    #[cfg(test)]
    pub(crate) fn validate_scalar(
        &mut self,
        function: &ResolvedFunction,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        self.failed = true;
        let context = selected_context::Context::new(&self.indexed, function, &mut self.budget)?;
        scalar_replay::validate(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }
    /// Read-only owning-test observation; no constructor or work credits escape.
    #[cfg(test)]
    pub(crate) fn usage(&self) -> (usize, usize) {
        (
            self.budget.remaining,
            self.budget
                .physical
                .as_ref()
                .expect("held selected reservation")
                .charged(),
        )
    }
    #[cfg(test)]
    pub(crate) fn finish(mut self) -> Result<PhysicalReservation, Diagnostic> {
        if self.failed {
            return Err(replay_error(
                self.anchor,
                "selected module replay already failed",
            ));
        }
        // The function lease has dropped; the shared inventory drops with this
        // session. Returning the debit grants neither a work reset nor a refund.
        self.budget
            .physical
            .take()
            .map(|physical| *physical)
            .ok_or_else(|| {
                replay_error(self.anchor, "selected replay lost its physical reservation")
            })
    }
}

#[cfg(test)]
mod tests;

fn require_entry(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if !crate::hir::lexical_exit::requires(function)?
        || !matches!(
            function.cleanup_plan.schema,
            super::super::CLEANUP_PLAN_SCHEMA_V18 | super::super::CLEANUP_PLAN_SCHEMA_V19
        )
    {
        return Err(replay_error(
            function,
            "selected retained lexical replay requires an actual lexical function",
        ));
    }
    Ok(())
}
/// Whole-entry unit oracle uses the same constructors and proof owner as the
/// selected production entry, with independently supplied exact work/bytes.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn validate_with_limits(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
) -> Result<(usize, usize), Diagnostic> {
    require_entry(function)?;
    let mut budget = ReplayBudget::with_physical(function, bytes)?;
    budget.remaining = work;
    let mut session = Session::from_budget(program, function, budget)?;
    session.validate(function)?;
    let spent = work
        .checked_sub(session.budget.remaining)
        .expect("work never increases");
    let charged = session
        .budget
        .physical
        .as_ref()
        .expect("selected reservation retained")
        .charged();
    // Drop the whole budget and all constructors inside the caller's observer.
    drop(session);
    Ok((spent, charged))
}

#[cfg(test)]
mod borrowed;
