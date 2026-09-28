//! Closed first-turn causal Reduce proof data. No evaluator/ACK/restore authority.
use super::reduce_inventory::{checked_step, CheckedReduceStepV8};
use super::reduce_model::{ReduceBasisV8, ReduceCleanupV8};
use super::reduce_wire::{recipe_digest, ReduceRecipeV8};
use super::SourceJournalError as Error;
use crate::resumable_effects::owned_frame::v2;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReduceTailV8 {
    Charged,
    Staged,
    CleanupInDoubt,
    CleanupSucceeded,
    FailureCleaned,
    Quarantined,
    TransferInDoubt,
    Mapped,
    Continued,
    TerminalPending,
}
/// Created only by parent glue after authenticated successful EffectRecorded,
/// whole Decision receipt and the original ordinary Reduce reservation check.
/// This inert inventory does not reproduce the physical State/Outcome owners.
pub(super) struct ReduceFoldV8 {
    binding: String,
    function: String,
    scope: Value,
    turn: u32,
    attempt: u32,
    reservation: u32,
    allowance: u64,
    effect_cleanup: u32,
    last: u32,
    tail: ReduceTailV8,
    consumed: Option<u64>,
    staged: Option<u32>,
    step: Option<CheckedReduceStepV8>,
    cleanup_started: Option<u32>,
    cleanup_settled: Option<u32>,
    cleanup: Option<v2::CheckedOwnedReduceCleanupV8>,
    transfer: Option<u32>,
    transition: Option<u32>,
    failure: Option<Value>,
}
fn require(ok: bool) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::Order)
    }
}
pub(super) struct ReduceClosureFactsV8<'a> {
    pub tail: ReduceTailV8,
    pub case: Option<&'a str>,
    pub active_operations: Option<&'a Value>,
    pub failure: bool,
}
impl ReduceFoldV8 {
    pub(super) fn after_checked_reservation(
        plan: &v2::CheckedOwnedReduceV2,
        scope: &Value,
        turn: u32,
        attempt: u32,
        reservation: u32,
        allowance: u64,
        effect_cleanup: u32,
    ) -> Result<Self, Error> {
        require(turn == 0 && effect_cleanup.checked_add(1) == Some(reservation) && allowance > 0)?;
        Ok(Self {
            binding: plan.binding().to_owned(),
            function: plan.function().id.as_str().to_owned(),
            scope: scope.clone(),
            turn,
            attempt,
            reservation,
            allowance,
            effect_cleanup,
            last: reservation,
            tail: ReduceTailV8::Charged,
            consumed: None,
            staged: None,
            step: None,
            cleanup_started: None,
            cleanup_settled: None,
            cleanup: None,
            transfer: None,
            transition: None,
            failure: None,
        })
    }
    pub(super) fn closure_facts(&self) -> ReduceClosureFactsV8<'_> {
        ReduceClosureFactsV8 {
            tail: self.tail,
            case: self.step.as_ref().map(|s| s.case()),
            active_operations: self.cleanup.as_ref().map(|c| c.active_operations()),
            failure: self.failure.is_some(),
        }
    }
    pub(super) fn tail(&self) -> ReduceTailV8 {
        self.tail
    }
    pub(super) fn failure(&self) -> Option<&Value> {
        self.failure.as_ref()
    }
    pub(super) fn consumed(&self) -> Option<u64> {
        self.consumed
    }
    fn plan(&self, plan: &v2::CheckedOwnedReduceV2, scope: Option<&Value>) -> Result<(), Error> {
        if plan.binding() != self.binding
            || plan.function().id.as_str() != self.function
            || scope.is_some_and(|s| s != &self.scope)
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
    fn coordinates(&self, seq: u32, turn: u32, attempt: u32) -> Result<(), Error> {
        require(
            turn == self.turn && attempt == self.attempt && self.last.checked_add(1) == Some(seq),
        )
    }
    fn refs(&self, reservation: u32, effect_cleanup: u32, consumed: u64) -> Result<(), Error> {
        require(
            reservation == self.reservation
                && effect_cleanup == self.effect_cleanup
                && consumed <= self.allowance
                && self.consumed.is_none_or(|old| old == consumed),
        )
    }
    /// Returns the newly recorded consumption delta; duplicate cleanup observation
    /// of Staged's consumed value returns zero, never spends/refunds reservation F.
    pub(super) fn staged(
        &mut self,
        plan: &v2::CheckedOwnedReduceV2,
        row_plan: &str,
        scope: &Value,
        seq: u32,
        turn: u32,
        attempt: u32,
        reservation: u32,
        effect_cleanup: u32,
        step: &Value,
        digest: &str,
        consumed: u64,
    ) -> Result<u64, Error> {
        if row_plan != plan.binding() {
            return Err(Error::Binding);
        }
        self.plan(plan, Some(scope))?;
        self.coordinates(seq, turn, attempt)?;
        self.refs(reservation, effect_cleanup, consumed)?;
        require(self.tail == ReduceTailV8::Charged)?;
        let checked = checked_step(plan, scope, turn, attempt, reservation, step, digest)?;
        self.step = Some(checked);
        self.staged = Some(seq);
        self.consumed = Some(consumed);
        self.last = seq;
        self.tail = ReduceTailV8::Staged;
        Ok(consumed)
    }
    pub(super) fn cleanup_started(
        &mut self,
        plan: &v2::CheckedOwnedReduceV2,
        row_plan: &str,
        scope: &Value,
        seq: u32,
        turn: u32,
        attempt: u32,
        reservation: u32,
        effect_cleanup: u32,
        basis: &ReduceBasisV8,
        digest: &str,
        consumed: u64,
        operations: &Value,
    ) -> Result<u64, Error> {
        if row_plan != plan.binding() {
            return Err(Error::Binding);
        }
        self.plan(plan, Some(scope))?;
        self.coordinates(seq, turn, attempt)?;
        self.refs(reservation, effect_cleanup, consumed)?;
        let success = matches!(basis, ReduceBasisV8::Success { .. });
        require(if success {
            self.tail == ReduceTailV8::Staged
        } else {
            self.tail == ReduceTailV8::Charged
        })?;
        if let ReduceBasisV8::Success { staged, case, .. } = basis {
            require(
                self.staged == Some(*staged)
                    && self.step.as_ref().is_some_and(|s| s.case() == case),
            )?;
        }
        let basis = serde_json::to_value(basis).map_err(|_| Error::Malformed)?;
        require(
            recipe_digest(
                ReduceRecipeV8::Basis,
                &json!({"scope":scope,"binding":plan.binding(),
            "plan":plan.binding(),"turn":turn,"attempt":attempt,"stage_reservation":reservation,
            "basis":basis}),
            )? == digest,
        )?;
        let checked = v2::validate_owned_reduce_cleanup_v8(plan, &basis, operations)
            .map_err(|_| Error::Binding)?;
        // A successful compiler-empty vector cannot be represented by fake cleanup rows.
        if success {
            require(
                checked
                    .active_operations()
                    .as_array()
                    .is_some_and(|a| !a.is_empty()),
            )?;
        }
        let delta = if self.consumed.is_none() { consumed } else { 0 };
        self.failure = checked.failure().cloned();
        self.cleanup = Some(checked);
        self.consumed = Some(consumed);
        self.cleanup_started = Some(seq);
        self.last = seq;
        self.tail = ReduceTailV8::CleanupInDoubt;
        Ok(delta)
    }
    pub(super) fn cleanup_settled(
        &mut self,
        seq: u32,
        turn: u32,
        attempt: u32,
        started: u32,
        receipt: &Value,
    ) -> Result<(), Error> {
        self.coordinates(seq, turn, attempt)?;
        require(
            self.tail == ReduceTailV8::CleanupInDoubt && self.cleanup_started == Some(started),
        )?;
        self.cleanup
            .as_ref()
            .ok_or(Error::Order)?
            .validate_receipt(receipt)
            .map_err(|_| Error::Binding)?;
        self.tail = if receipt["settlement"] != "completed" {
            ReduceTailV8::Quarantined
        } else if self.failure.is_some() {
            ReduceTailV8::FailureCleaned
        } else {
            ReduceTailV8::CleanupSucceeded
        };
        self.cleanup_settled = Some(seq);
        self.last = seq;
        Ok(())
    }
    pub(super) fn transfer_reserved(
        &mut self,
        plan: &v2::CheckedOwnedReduceV2,
        row_plan: &str,
        seq: u32,
        turn: u32,
        attempt: u32,
        reservation: u32,
        staged: u32,
        cleanup: &ReduceCleanupV8,
        case: &str,
    ) -> Result<(), Error> {
        if row_plan != plan.binding() {
            return Err(Error::Binding);
        }
        self.plan(plan, None)?;
        self.coordinates(seq, turn, attempt)?;
        require(
            reservation == self.reservation
                && self.staged == Some(staged)
                && self.step.as_ref().is_some_and(|s| s.case() == case)
                && self.failure.is_none(),
        )?;
        match cleanup {
            ReduceCleanupV8::Observed { started, settled } => require(
                self.tail == ReduceTailV8::CleanupSucceeded
                    && self.cleanup_started == Some(*started)
                    && self.cleanup_settled == Some(*settled),
            )?,
            ReduceCleanupV8::CompilerEmpty {} => {
                require(self.tail == ReduceTailV8::Staged)?;
                let matching = plan
                    .transfers()
                    .cases
                    .iter()
                    .filter(|c| c.case.as_str() == case)
                    .collect::<Vec<_>>();
                require(!matching.is_empty())?;
                // With no recorded constructor site, every possible checked site for
                // this case must prove empty. Never guess which branch executed.
                for c in matching {
                    let basis = json!({"kind":"success","staged":staged,"constructor":c.constructor.as_str(),
                        "case":case,"active_flags":c.completion_live_flags.iter().map(|f|f.0).collect::<Vec<_>>()});
                    let ops = v2::owned_wait_operations_v8(&plan.transfers().completion_cleanup)
                        .map_err(|_| Error::Binding)?;
                    let checked = v2::validate_owned_reduce_cleanup_v8(plan, &basis, &ops)
                        .map_err(|_| Error::Binding)?;
                    require(
                        checked
                            .active_operations()
                            .as_array()
                            .is_some_and(|a| a.is_empty()),
                    )?;
                }
            }
        }
        self.transfer = Some(seq);
        self.last = seq;
        self.tail = ReduceTailV8::TransferInDoubt;
        Ok(())
    }
    pub(super) fn transfer_completed(
        &mut self,
        plan: &v2::CheckedOwnedReduceV2,
        scope: &Value,
        seq: u32,
        turn: u32,
        attempt: u32,
        reserved: u32,
        target: &Value,
        digest: &str,
    ) -> Result<(), Error> {
        self.plan(plan, Some(scope))?;
        self.coordinates(seq, turn, attempt)?;
        require(self.tail == ReduceTailV8::TransferInDoubt && self.transfer == Some(reserved))?;
        let step = self.step.as_ref().ok_or(Error::Order)?;
        step.matches_target(target)?;
        require(step.transfer_digest(scope, plan, turn, attempt, reserved)? == digest)?;
        self.last = seq;
        self.tail = ReduceTailV8::Mapped;
        Ok(())
    }
    /// Only the exact frozen identity join is checked here. Terminal evidence,
    /// delivery, next-turn accounting and owner handoff remain separate gates.
    pub(super) fn transition(
        &mut self,
        plan: &v2::CheckedOwnedReduceV2,
        seq: u32,
        turn: u32,
        attempt: u32,
        case: super::super::SourceTransitionCase,
        carrier_digest: &str,
    ) -> Result<(), Error> {
        use super::super::SourceTransitionCase as Case;
        self.plan(plan, None)?;
        self.coordinates(seq, turn, attempt)?;
        require(self.tail == ReduceTailV8::Mapped)?;
        let step = self.step.as_ref().ok_or(Error::Order)?;
        let selected = match step.target()["kind"].as_str() {
            Some("continue") => Case::Continue,
            Some("suspend") => Case::Suspend,
            Some("complete") => Case::Complete,
            Some("fail") => Case::Fail,
            _ => return Err(Error::Binding),
        };
        let bytes = step.ordinary_carrier_bytes(plan)?;
        require(
            case == selected
                && crate::live_invocation::identity::digest(
                    b"semaprax.agent-step.value.v2\0",
                    &bytes,
                ) == carrier_digest,
        )?;
        self.last = seq;
        self.transition = Some(seq);
        self.tail = if case == Case::Continue {
            ReduceTailV8::Continued
        } else {
            ReduceTailV8::TerminalPending
        };
        Ok(())
    }
    /// Stop is only a causal join after successful failure cleanup; it does not
    /// validate complete terminal accounting or grant result publication.
    pub(super) fn stop(
        &mut self,
        seq: u32,
        turn: u32,
        attempt: u32,
        status: super::super::SourceStopStatus,
        reason: super::super::SourceStopReason,
    ) -> Result<(), Error> {
        self.coordinates(seq, turn, attempt)?;
        require(self.tail == ReduceTailV8::FailureCleaned)?;
        let failed = self.failure.as_ref().ok_or(Error::Order)?;
        let budget = matches!(
            failed["failure"].as_str(),
            Some("fuel_exhausted" | "call_depth_exceeded")
        );
        use super::super::{SourceStopReason as R, SourceStopStatus as S};
        require(
            (status, reason)
                == if budget {
                    (S::BudgetExhausted, R::BudgetExhausted)
                } else {
                    (S::Rejected, R::StageRefused)
                },
        )?;
        self.last = seq;
        self.tail = ReduceTailV8::TerminalPending;
        Ok(())
    }
}

/// Seeded only by the parent after its authenticated EffectFailed/Recorded and
/// complete Decision-release join. State digest is the parent's already checked
/// owned-State commitment, not a caller credential or a restored owner.
pub(super) struct FailedEffectStateFoldV8 {
    binding: String,
    scope: Value,
    turn: u32,
    attempt: u32,
    settlement: u32,
    recorded: u32,
    decision_cleanup: u32,
    failure: super::super::SourceEffectFailure,
    state_digest: String,
    operations: Value,
    started: Option<u32>,
    last: u32,
    tail: ReduceTailV8,
}
impl FailedEffectStateFoldV8 {
    pub(super) fn after_checked_effect_failure(
        binding: &v2::CheckedOwnedAgentWaitBindingV8,
        scope: &Value,
        turn: u32,
        attempt: u32,
        settlement: u32,
        recorded: u32,
        decision_cleanup: u32,
        failure: super::super::SourceEffectFailure,
        state_digest: &str,
    ) -> Result<Self, Error> {
        use super::super::SourceEffectFailure as F;
        require(
            turn == 0
                && settlement.checked_add(1) == Some(recorded)
                && recorded < decision_cleanup
                && matches!(failure, F::HandlerFailed | F::ResultLimit),
        )?;
        let actions = &binding.helper().liveness().result_disposal;
        // Whole retained flat State has every leaf live, and no selected variant
        // guards. Never infer a runtime branch from supplied receipt entries.
        require(actions.iter().all(|a| a.active_case.is_none()))?;
        let operations = v2::owned_wait_operations_v8(actions).map_err(|_| Error::Binding)?;
        Ok(Self {
            binding: binding.binding().to_owned(),
            scope: scope.clone(),
            turn,
            attempt,
            settlement,
            recorded,
            decision_cleanup,
            failure,
            state_digest: state_digest.to_owned(),
            operations,
            started: None,
            last: decision_cleanup,
            tail: ReduceTailV8::Charged,
        })
    }
    pub(super) fn tail(&self) -> ReduceTailV8 {
        self.tail
    }
    pub(super) fn operations(&self) -> &Value {
        &self.operations
    }
    pub(super) fn cleanup_started(
        &mut self,
        row_plan: &str,
        scope: &Value,
        seq: u32,
        turn: u32,
        attempt: u32,
        settlement: u32,
        recorded: u32,
        decision_cleanup: u32,
        effect_failure: &str,
        state_digest: &str,
        operations: &Value,
    ) -> Result<(), Error> {
        require(
            self.tail == ReduceTailV8::Charged
                && self.last.checked_add(1) == Some(seq)
                && turn == self.turn
                && attempt == self.attempt
                && settlement == self.settlement
                && recorded == self.recorded
                && decision_cleanup == self.decision_cleanup,
        )?;
        if row_plan != self.binding
            || scope != &self.scope
            || state_digest != self.state_digest
            || effect_failure != self.failure.as_str()
            || operations != &self.operations
        {
            return Err(Error::Binding);
        }
        self.started = Some(seq);
        self.last = seq;
        self.tail = ReduceTailV8::CleanupInDoubt;
        Ok(())
    }
    pub(super) fn cleanup_settled(
        &mut self,
        seq: u32,
        turn: u32,
        attempt: u32,
        started: u32,
        receipt: &Value,
    ) -> Result<(), Error> {
        require(
            self.tail == ReduceTailV8::CleanupInDoubt
                && self.last.checked_add(1) == Some(seq)
                && turn == self.turn
                && attempt == self.attempt
                && self.started == Some(started),
        )?;
        v2::validate_owned_wait_observed_receipt_v8(&self.operations, receipt)
            .map_err(|_| Error::Binding)?;
        self.tail = if receipt["settlement"] == "completed" {
            ReduceTailV8::FailureCleaned
        } else {
            ReduceTailV8::Quarantined
        };
        self.last = seq;
        Ok(())
    }
    pub(super) fn stop(
        &mut self,
        seq: u32,
        turn: u32,
        attempt: u32,
        status: super::super::SourceStopStatus,
        reason: super::super::SourceStopReason,
    ) -> Result<(), Error> {
        require(
            self.tail == ReduceTailV8::FailureCleaned
                && self.last.checked_add(1) == Some(seq)
                && turn == self.turn
                && attempt == self.attempt
                && status == super::super::SourceStopStatus::EffectFailed
                && reason == super::super::SourceStopReason::EffectFailed,
        )?;
        self.last = seq;
        self.tail = ReduceTailV8::TerminalPending;
        Ok(())
    }
}

#[cfg(test)]
#[path = "reduce_fold/tests.rs"]
mod tests;
