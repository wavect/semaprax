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
}
/// Created only by parent glue after authenticated successful EffectRecorded,
/// whole Decision receipt and the original ordinary Reduce reservation check.
/// This inert inventory does not reproduce the physical State/Outcome owners.
pub(super) struct ReduceFoldV8 {
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
    failure: Option<Value>,
}
fn require(ok: bool) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::Order)
    }
}
impl ReduceFoldV8 {
    pub(super) fn after_checked_reservation(
        turn: u32,
        attempt: u32,
        reservation: u32,
        allowance: u64,
        effect_cleanup: u32,
    ) -> Result<Self, Error> {
        require(turn == 0 && effect_cleanup.checked_add(1) == Some(reservation) && allowance > 0)?;
        Ok(Self {
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
            failure: None,
        })
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
        seq: u32,
        turn: u32,
        attempt: u32,
        reservation: u32,
        staged: u32,
        cleanup: &ReduceCleanupV8,
        case: &str,
    ) -> Result<(), Error> {
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
            ReduceCleanupV8::CompilerEmpty => {
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
                    let ops = v2::owned_wait_operations_v8(&plan.transfers().completion_cleanup);
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
        self.coordinates(seq, turn, attempt)?;
        require(self.tail == ReduceTailV8::TransferInDoubt && self.transfer == Some(reserved))?;
        let step = self.step.as_ref().ok_or(Error::Order)?;
        step.matches_target(target)?;
        require(step.transfer_digest(scope, plan, turn, attempt, reserved)? == digest)?;
        self.last = seq;
        self.tail = ReduceTailV8::Mapped;
        Ok(())
    }
}

#[cfg(test)]
#[path = "reduce_fold/tests.rs"]
mod tests;
