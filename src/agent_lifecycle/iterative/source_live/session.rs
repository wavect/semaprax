//! One checked cursor coordinates source replay, stage fuel and model spend.
use super::*;
use crate::live_invocation::source_journal::*;
use crate::live_invocation::InvocationBudgetHook;
use driver::{IterativeDriver, ProposalRequest, ProposalSource};
mod model_wait;

pub(crate) struct SourceExecutionSession<'a> {
    model_wait: Option<model_wait::ModelWaitContext<'a>>,
    pub(super) sink: SourceCheckpointSink<'a>,
    pub(super) ledger: CumulativeBudgetLedger<'a>,
    clock: &'a dyn SourceInvocationClock,
    cancellation: &'a AgentCancellation,
    replay: Vec<(u32, SourceJournalEntry)>,
    cursor: usize,
    replay_pass: u32,
    pub(super) rows: Vec<SourceStageSummary>,
    pub(super) model_dispatches: u32,
    pub(super) effect_dispatches: u32,
    pub(super) selected: Option<SourceTerminalStatus>,
    pub(super) journal_error: Option<SourceJournalError>,
}
fn role(value: &str) -> Result<SourceStageRole, Vec<Diagnostic>> {
    match value {
        "initialize" => Ok(SourceStageRole::Initialize),
        "observe" => Ok(SourceStageRole::Observe),
        "authorize" => Ok(SourceStageRole::Authorize),
        "reduce" => Ok(SourceStageRole::Reduce),
        _ => Err(vec![bad("source.stage_role")]),
    }
}
fn sideband(entry: &SourceJournalEntry) -> bool {
    matches!(
        entry,
        SourceJournalEntry::ReplayStageReservation { .. }
            | SourceJournalEntry::AttemptUsage { .. }
            | SourceJournalEntry::PricedAttemptUsage(_)
            | SourceJournalEntry::PolicyAttemptUsage { .. }
    )
}
fn migration_preamble(entry: &SourceJournalEntry) -> bool {
    matches!(
        entry,
        SourceJournalEntry::MigrationOpened { .. }
            | SourceJournalEntry::MigrationEvaluationIntent { .. }
            | SourceJournalEntry::MigrationEvaluationSettled { .. }
            | SourceJournalEntry::MigrationEvaluationFailed { .. }
    )
}
impl<'a> SourceExecutionSession<'a> {
    pub(super) fn new(
        sink: SourceCheckpointSink<'a>,
        ledger: CumulativeBudgetLedger<'a>,
        clock: &'a dyn SourceInvocationClock,
        cancellation: &'a AgentCancellation,
    ) -> Self {
        let replay_pass = sink
            .journal()
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                SourceJournalEntry::ReplayStageReservation { replay, .. } => Some(*replay),
                _ => None,
            })
            .max()
            .map_or(0, |value| value.saturating_add(1));
        let replay = sink
            .journal()
            .execution_entries_v7()
            .into_iter()
            .filter_map(|(seq, entry)| match entry {
                SourceExecutionEntryV7::Ordinary(entry)
                    if !sideband(&entry) && !migration_preamble(&entry) =>
                {
                    Some((seq, entry))
                }
                _ => None,
            })
            .collect();
        Self {
            model_wait: None,
            sink,
            ledger,
            clock,
            cancellation,
            replay,
            cursor: 0,
            replay_pass,
            rows: Vec::new(),
            model_dispatches: 0,
            effect_dispatches: 0,
            selected: None,
            journal_error: None,
        }
    }
    fn refuse(&mut self, status: SourceTerminalStatus, field: &str) -> Vec<Diagnostic> {
        self.selected.get_or_insert(status);
        vec![bad(field)]
    }
    fn journal_failure(&mut self, error: SourceJournalError) -> Vec<Diagnostic> {
        self.journal_error.get_or_insert(error);
        let status = if error == SourceJournalError::Capacity {
            SourceTerminalStatus::BudgetExhausted
        } else {
            SourceTerminalStatus::Rejected
        };
        self.refuse(status, "source.checkpoint")
    }
    fn append(&mut self, entry: SourceJournalEntry) -> Result<(), Vec<Diagnostic>> {
        self.sink
            .append_at(entry, self.clock.now_millis())
            .map_err(|error| self.journal_failure(error))
    }
    pub(crate) fn guard(&mut self) -> Result<(), Vec<Diagnostic>> {
        if self.cancellation.is_cancelled() {
            return Err(self.refuse(SourceTerminalStatus::Cancelled, "source.cancelled"));
        }
        if self.clock.clock_domain() != self.sink.journal().binding().clock_domain()
            || self.clock.now_millis() < self.sink.journal().last_checked_millis()
        {
            self.journal_error.get_or_insert(SourceJournalError::Time);
            return Err(self.refuse(SourceTerminalStatus::Rejected, "source.clock"));
        }
        self.ledger.check_deadline().map_err(|error| {
            let status = if error.0 == crate::live_invocation::budget::DEADLINE_EXCEEDED {
                SourceTerminalStatus::DeadlineExceeded
            } else {
                SourceTerminalStatus::Rejected
            };
            self.refuse(status, "source.clock")
        })
    }
    fn next(&self) -> Option<&SourceJournalEntry> {
        self.replay.get(self.cursor).map(|(_, entry)| entry)
    }
    fn expect(&mut self, entry: SourceJournalEntry) -> Result<(), Vec<Diagnostic>> {
        if let Some(previous) = self.next() {
            if *previous != entry {
                return Err(self.refuse(SourceTerminalStatus::Rejected, "source.replay_mismatch"));
            }
            self.cursor += 1;
            Ok(())
        } else {
            self.append(entry)
        }
    }
    pub(super) fn opened(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.expect(SourceJournalEntry::RunOpened)
    }
    pub(crate) fn before_stage(
        &mut self,
        stage: &str,
        turn: usize,
        attempt: Option<usize>,
        fuel: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        self.guard()?;
        let role = role(stage)?;
        let original = SourceJournalEntry::StageReservation {
            turn: turn as u32,
            attempt: attempt.map(|v| v as u32),
            role,
            fuel,
        };
        if let Some((seq, entry)) = self.replay.get(self.cursor) {
            if *entry != original {
                return Err(self.refuse(SourceTerminalStatus::Rejected, "source.stage_replay"));
            }
            let reservation = SourceJournalEntry::ReplayStageReservation {
                replay: self.replay_pass,
                causal_seq: *seq,
                role,
                fuel,
            };
            self.append(reservation)?;
            self.cursor += 1;
        } else {
            self.append(original)?;
        }
        self.guard()
    }
    pub(crate) fn record_stage(&mut self, record: &StageRecord) {
        let outcome = match record.outcome() {
            "returned" => SourceStageOutcome::Returned,
            "language_failure" => SourceStageOutcome::LanguageFailure,
            "fuel_exhausted" => SourceStageOutcome::FuelExhausted,
            "call_depth_exceeded" => SourceStageOutcome::CallDepthExceeded,
            _ => SourceStageOutcome::GuardError,
        };
        if let Ok(role) = role(record.role()) {
            self.rows.push(SourceStageSummary {
                role,
                function_id: record.function_id().to_owned(),
                outcome,
                steps_used: record.steps_used,
            });
        }
    }
    pub(crate) fn observed(
        &mut self,
        turn: usize,
        state: &RetainedValue,
        observation: &RetainedValue,
        feedback: Option<&[u8]>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.expect(SourceJournalEntry::TurnObserved {
            turn: turn as u32,
            state: digest(
                b"semaprax.source-state.v2\0",
                encode_value(state).as_bytes(),
            ),
            observation: digest(
                b"semaprax.source-observation.v2\0",
                encode_value(observation).as_bytes(),
            ),
            feedback: digest(
                b"semaprax.source-feedback.v2\0",
                &feedback
                    .map(|bytes| {
                        format!(
                            "some:{}",
                            encode_value(&RetainedValue::Bytes(bytes.to_vec()))
                        )
                    })
                    .unwrap_or_else(|| "none".to_owned())
                    .into_bytes(),
            ),
        })
    }
    pub(crate) fn propose(
        &mut self,
        source: &mut dyn ProposalSource,
        request: ProposalRequest<'_>,
    ) -> Result<String, Vec<Diagnostic>> {
        if self.sink.journal().binding().policy_binding().is_some() {
            return self.propose_policy(source, request);
        }
        self.guard()?;
        self.prepare_model_wait(
            request.turn as u32,
            request.attempt as u32,
            request.observation,
        )?;
        let identity = source.checkpoint_attempt_identity(&request)?;
        let turn = request.turn as u32;
        let attempt = request.attempt as u32;
        let reservation_units = self.sink.journal().binding().reservation_units();
        let intent = if let Some(SourceJournalEntry::PricedAttemptIntent(prior)) = self.next() {
            self.sink.journal().binding().attempt_intent_at_ordinal(
                turn,
                attempt,
                identity.request_digest,
                identity.prompt_digest,
                identity.request_bytes,
                prior.money_ordinal,
            )
        } else {
            self.sink.attempt_intent(
                turn,
                attempt,
                identity.request_digest,
                identity.prompt_digest,
                identity.request_bytes,
            )
        }
        .map_err(|_| self.refuse(SourceTerminalStatus::Rejected, "source.attempt_identity"))?;
        if self.next().is_some() {
            self.expect(intent)?;
            let settled = self.next().cloned().ok_or_else(|| {
                self.refuse(SourceTerminalStatus::Rejected, "source.uncertain_attempt")
            })?;
            self.cursor += 1;
            return self.response(settled, turn, attempt);
        }
        if self.ledger.remaining() < reservation_units {
            return Err(self.refuse(SourceTerminalStatus::BudgetExhausted, "source.model_budget"));
        }
        if let SourceJournalEntry::PricedAttemptIntent(priced) = &intent {
            let totals = self
                .sink
                .priced_totals()
                .map_err(|_| {
                    self.refuse(SourceTerminalStatus::Rejected, "source.priced_accounting")
                })?
                .ok_or_else(|| {
                    self.refuse(SourceTerminalStatus::Rejected, "source.priced_accounting")
                })?;
            if priced.reserved_minor > totals.remaining_admission_minor {
                return Err(
                    self.refuse(SourceTerminalStatus::BudgetExhausted, "source.money_budget")
                );
            }
        }
        if self.sink.journal().binding().io_limits().is_some() {
            self.sink
                .preflight_at(&intent, self.clock.now_millis())
                .map_err(|error| self.journal_failure(error))?;
        }
        let start = self.sink.journal().entries().len();
        let result =
            source.propose_checkpointed(request, &mut self.sink, &mut self.ledger, self.clock);
        self.model_dispatches = self
            .model_dispatches
            .saturating_add(result.model_dispatches);
        let events: Vec<_> = self.sink.journal().entries()[start..]
            .iter()
            .filter(|e| !sideband(e))
            .cloned()
            .collect();
        if events.first() != Some(&intent) || events.len() != 2 {
            if self.sink.poisoned() {
                self.journal_error = Some(SourceJournalError::Poisoned);
            }
            // A source adapter may report one of these closed local terminal
            // conditions before it has approached a provider, so there is no
            // attempt intent or receipt to require.  Never apply such a
            // status after any claimed dispatch: a missing durable receipt
            // then remains a model failure/unknown provider outcome rather
            // than becoming a clean cancellation or budget refusal.
            let pre_dispatch_terminal = (result.model_dispatches == 0
                && self.sink.journal().entries().len() == start
                && !self.sink.poisoned())
            .then_some(result.terminal_failure)
            .flatten()
            .filter(|status| {
                matches!(
                    *status,
                    SourceTerminalStatus::Cancelled
                        | SourceTerminalStatus::DeadlineExceeded
                        | SourceTerminalStatus::BudgetExhausted
                )
            });
            if self.selected.is_none() {
                self.selected = pre_dispatch_terminal;
            }
            if self.selected.is_none() {
                self.selected = Some(SourceTerminalStatus::ModelFailed);
            }
            return Err(result.result.err().unwrap_or_else(|| {
                self.refuse(SourceTerminalStatus::Rejected, "source.attempt_receipt")
            }));
        }
        let recorded = self.response(events[1].clone(), turn, attempt);
        if self.selected.is_none() {
            self.selected = result.terminal_failure;
        }
        match (result.result, recorded) {
            (Ok(actual), Ok(stored)) if actual == stored => {
                self.guard()?;
                Ok(stored)
            }
            (Err(mut errors), _) => {
                if self.selected.is_none() {
                    if let Err(guard_errors) = self.guard() {
                        errors.extend(guard_errors);
                    }
                    self.selected
                        .get_or_insert(SourceTerminalStatus::ModelFailed);
                }
                Err(errors)
            }
            (_, Err(errors)) => Err(errors),
            _ => Err(self.refuse(SourceTerminalStatus::Rejected, "source.response_mismatch")),
        }
    }
    fn propose_policy(
        &mut self,
        source: &mut dyn ProposalSource,
        request: ProposalRequest<'_>,
    ) -> Result<String, Vec<Diagnostic>> {
        self.guard()?;
        let identity = source.checkpoint_attempt_identity(&request)?;
        let turn = request.turn as u32;
        let attempt = request.attempt as u32;
        if let Some(SourceJournalEntry::PolicyAttemptIntent(prior)) = self.next().cloned() {
            if prior.turn != turn
                || prior.attempt != attempt
                || prior.request_digest != identity.request_digest
                || prior.prompt_digest != identity.prompt_digest
                || prior.request_bytes != identity.request_bytes
            {
                return Err(self.refuse(SourceTerminalStatus::Rejected, "source.policy_replay"));
            }
            self.expect(SourceJournalEntry::PolicyAttemptIntent(prior))?;
            let settled = self.next().cloned().ok_or_else(|| {
                self.refuse(SourceTerminalStatus::Rejected, "source.uncertain_attempt")
            })?;
            self.cursor += 1;
            return self.response(settled, turn, attempt);
        }
        if self.next().is_some() {
            return Err(self.refuse(SourceTerminalStatus::Rejected, "source.policy_replay"));
        }
        let quote = source
            .policy_quote_checkpointed(&request, &identity)?
            .ok_or_else(|| self.refuse(SourceTerminalStatus::Rejected, "source.policy_quote"))?;
        let intent = self
            .sink
            .policy_attempt_intent(
                turn,
                attempt,
                identity.request_digest,
                identity.prompt_digest,
                identity.request_bytes,
                quote.clone(),
            )
            .map_err(|_| {
                self.refuse(
                    SourceTerminalStatus::BudgetExhausted,
                    "source.policy_budget",
                )
            })?;
        let entry = SourceJournalEntry::PolicyAttemptIntent(intent.clone());
        self.sink
            .preflight_at(&entry, self.clock.now_millis())
            .map_err(|error| self.journal_failure(error))?;
        let start = self.sink.journal().entries().len();
        let result = source.propose_policy_checkpointed(
            request,
            &quote,
            &intent,
            &mut self.sink,
            &mut self.ledger,
            self.clock,
        );
        self.model_dispatches = self
            .model_dispatches
            .saturating_add(result.model_dispatches);
        let events: Vec<_> = self.sink.journal().entries()[start..]
            .iter()
            .filter(|entry| !sideband(entry))
            .cloned()
            .collect();
        if events.first() != Some(&entry) || events.len() != 2 {
            if self.sink.poisoned() {
                self.journal_error = Some(SourceJournalError::Poisoned);
            }
            let pre_dispatch = (result.model_dispatches == 0
                && self.sink.journal().entries().len() == start
                && !self.sink.poisoned())
            .then_some(result.terminal_failure)
            .flatten()
            .filter(|status| {
                matches!(
                    *status,
                    SourceTerminalStatus::Cancelled
                        | SourceTerminalStatus::DeadlineExceeded
                        | SourceTerminalStatus::BudgetExhausted
                )
            });
            if self.selected.is_none() {
                self.selected = pre_dispatch;
            }
            self.selected
                .get_or_insert(SourceTerminalStatus::ModelFailed);
            return Err(result.result.err().unwrap_or_else(|| {
                self.refuse(SourceTerminalStatus::Rejected, "source.policy_receipt")
            }));
        }
        let recorded = self.response(events[1].clone(), turn, attempt);
        if self.selected.is_none() {
            self.selected = result.terminal_failure;
        }
        match (result.result, recorded) {
            (Ok(actual), Ok(stored)) if actual == stored => {
                self.guard()?;
                Ok(stored)
            }
            (Err(mut errors), _) => {
                if self.selected.is_none() {
                    self.selected = Some(SourceTerminalStatus::ModelFailed);
                }
                errors.extend(self.guard().err().unwrap_or_default());
                Err(errors)
            }
            (_, Err(errors)) => Err(errors),
            _ => Err(self.refuse(SourceTerminalStatus::Rejected, "source.response_mismatch")),
        }
    }

    fn response(
        &mut self,
        entry: SourceJournalEntry,
        turn: u32,
        attempt: u32,
    ) -> Result<String, Vec<Diagnostic>> {
        match entry {
            SourceJournalEntry::AttemptSettled {
                turn: t,
                attempt: a,
                response,
                ..
            } if t == turn && a == attempt => String::from_utf8(response).map_err(|_| {
                self.refuse(SourceTerminalStatus::ModelFailed, "source.response_utf8")
            }),
            SourceJournalEntry::AttemptFailed {
                turn: t,
                attempt: a,
                reason,
                ..
            } if t == turn && a == attempt => {
                let status = match reason {
                    SourceAttemptFailure::Cancelled => SourceTerminalStatus::Cancelled,
                    SourceAttemptFailure::DeadlineExceeded => {
                        SourceTerminalStatus::DeadlineExceeded
                    }
                    _ => SourceTerminalStatus::ModelFailed,
                };
                Err(self.refuse(status, "source.model_failed"))
            }
            _ => Err(self.refuse(SourceTerminalStatus::Rejected, "source.response_phase")),
        }
    }
    pub(crate) fn proposal_refused(
        &mut self,
        turn: usize,
        attempt: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        self.expect(SourceJournalEntry::ProposalRefused {
            turn: turn as u32,
            attempt: attempt as u32,
            reason: SourceProposalRefusal::MalformedDecode,
        })
    }
    pub(crate) fn proposal_admitted(
        &mut self,
        turn: usize,
        attempt: usize,
        canonical: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        self.expect(SourceJournalEntry::ProposalAdmitted {
            turn: turn as u32,
            attempt: attempt as u32,
            proposal_digest: digest(b"semaprax.source-proposal.v2\0", canonical.as_bytes()),
        })
    }
    pub(crate) fn authorization_refused(
        &mut self,
        turn: usize,
        attempt: usize,
        fuel_exhausted: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        self.expect(SourceJournalEntry::AuthorizationRefused {
            turn: turn as u32,
            attempt: attempt as u32,
            reason: if fuel_exhausted {
                SourceAuthorizationRefusal::Undecided
            } else {
                SourceAuthorizationRefusal::GateDenied
            },
        })
    }
    pub(crate) fn authorized(
        &mut self,
        turn: usize,
        attempt: usize,
        request: &AuthorizedRequest,
    ) -> Result<(), Vec<Diagnostic>> {
        self.expect(SourceJournalEntry::AuthorizationConsumed {
            turn: turn as u32,
            attempt: attempt as u32,
            grant_digest: request.binding().to_owned(),
        })
    }
    pub(crate) fn read(
        &mut self,
        turn: usize,
        attempt: usize,
        request: &AuthorizedRequest,
        driver: &mut dyn IterativeDriver,
    ) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
        self.guard()?;
        let turn = turn as u32;
        let attempt = attempt as u32;
        let (operation, request_digest) = match driver.source_effect_identity(request)? {
            Some(identity) => identity,
            None => (
                "agent.read".to_owned(),
                digest(
                    b"semaprax.source-effect-request.v2\0",
                    format!(
                        "{}:{}:{}",
                        request.binding(),
                        request.budget(),
                        encode_value(&RetainedValue::Bytes(request.seal().to_vec()))
                    )
                    .as_bytes(),
                ),
            ),
        };
        let replaying = self.next().is_some();
        self.expect(SourceJournalEntry::EffectIntent {
            turn,
            attempt,
            operation: operation.clone(),
            request_digest,
        })?;
        if replaying {
            let next = self.next().cloned();
            self.cursor += 1;
            return match next {
                Some(SourceJournalEntry::EffectObserved {
                    turn: t,
                    attempt: a,
                    operation: op,
                    observation,
                    ..
                }) if t == turn && a == attempt && op == operation => {
                    driver.validate_replayed_read(request, Some(&observation))?;
                    Ok(Some(observation))
                }
                Some(SourceJournalEntry::EffectFailed {
                    turn: t,
                    attempt: a,
                    operation: op,
                    reason,
                }) if t == turn && a == attempt && op == operation => {
                    driver.validate_replayed_read(request, None)?;
                    let status = match reason {
                        SourceEffectFailure::Cancelled => SourceTerminalStatus::Cancelled,
                        SourceEffectFailure::DeadlineExceeded => {
                            SourceTerminalStatus::DeadlineExceeded
                        }
                        _ => SourceTerminalStatus::EffectFailed,
                    };
                    self.selected.get_or_insert(status);
                    if status != SourceTerminalStatus::EffectFailed {
                        Err(self.refuse(status, "source.effect_failed"))
                    } else {
                        Ok(None)
                    }
                }
                _ => Err(self.refuse(SourceTerminalStatus::Rejected, "source.effect_replay")),
            };
        }
        if let Err(errors) = self.guard() {
            let reason = if self.selected == Some(SourceTerminalStatus::Cancelled) {
                SourceEffectFailure::Cancelled
            } else {
                SourceEffectFailure::DeadlineExceeded
            };
            self.append(SourceJournalEntry::EffectFailed {
                turn,
                attempt,
                operation,
                reason,
            })?;
            return Err(errors);
        }
        self.effect_dispatches += 1;
        let result = driver.read(request);
        match result {
            Ok(Some(bytes)) if bytes.len() <= MAX_SOURCE_EFFECT_BYTES => {
                self.append(SourceJournalEntry::EffectObserved {
                    turn,
                    attempt,
                    operation,
                    observation_digest: source_effect_digest(&bytes),
                    observation: bytes.clone(),
                })?;
                self.guard()?;
                Ok(Some(bytes))
            }
            result => {
                let reason = if matches!(result, Ok(Some(_))) {
                    SourceEffectFailure::ResultLimit
                } else {
                    SourceEffectFailure::HandlerFailed
                };
                self.append(SourceJournalEntry::EffectFailed {
                    turn,
                    attempt,
                    operation,
                    reason,
                })?;
                self.selected
                    .get_or_insert(SourceTerminalStatus::EffectFailed);
                match result {
                    Err(errors) => Err(errors),
                    _ => Ok(None),
                }
            }
        }
    }
    pub(crate) fn transition(
        &mut self,
        turn: usize,
        attempt: usize,
        kind: &str,
        value: &RetainedValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let case = match kind {
            "Continue" => SourceTransitionCase::Continue,
            "Complete" => SourceTransitionCase::Complete,
            "Suspend" => SourceTransitionCase::Suspend,
            "Fail" => SourceTransitionCase::Fail,
            _ => return Err(self.refuse(SourceTerminalStatus::Rejected, "source.transition")),
        };
        if case == SourceTransitionCase::Fail {
            self.selected = Some(SourceTerminalStatus::Fail);
        }
        self.expect(SourceJournalEntry::Transition {
            turn: turn as u32,
            attempt: attempt as u32,
            case,
            carrier_digest: digest(
                b"semaprax.agent-step.value.v2\0",
                encode_value(value).as_bytes(),
            ),
        })
    }
}

fn terminal_status(status: IterativeStatus) -> SourceTerminalStatus {
    match status {
        IterativeStatus::Complete => SourceTerminalStatus::Complete,
        IterativeStatus::Suspend => SourceTerminalStatus::Suspend,
        IterativeStatus::Fail => SourceTerminalStatus::Fail,
        IterativeStatus::Rejected => SourceTerminalStatus::Rejected,
        IterativeStatus::ModelFailed => SourceTerminalStatus::ModelFailed,
        IterativeStatus::EffectFailed => SourceTerminalStatus::EffectFailed,
        IterativeStatus::Cancelled => SourceTerminalStatus::Cancelled,
        IterativeStatus::BudgetExhausted => SourceTerminalStatus::BudgetExhausted,
    }
}
fn stop_pair(status: SourceTerminalStatus) -> Option<(SourceStopStatus, SourceStopReason)> {
    Some(match status {
        SourceTerminalStatus::Rejected => {
            (SourceStopStatus::Rejected, SourceStopReason::StageRefused)
        }
        SourceTerminalStatus::ModelFailed => {
            (SourceStopStatus::ModelFailed, SourceStopReason::ModelFailed)
        }
        SourceTerminalStatus::EffectFailed => (
            SourceStopStatus::EffectFailed,
            SourceStopReason::EffectFailed,
        ),
        SourceTerminalStatus::Cancelled => {
            (SourceStopStatus::Cancelled, SourceStopReason::Cancelled)
        }
        SourceTerminalStatus::BudgetExhausted => (
            SourceStopStatus::BudgetExhausted,
            SourceStopReason::BudgetExhausted,
        ),
        SourceTerminalStatus::DeadlineExceeded => (
            SourceStopStatus::DeadlineExceeded,
            SourceStopReason::DeadlineExceeded,
        ),
        _ => return None,
    })
}
impl SourceExecutionSession<'_> {
    pub(super) fn failure(
        self,
        run: Option<IterativeRun>,
        diagnostics: Vec<Diagnostic>,
    ) -> SourceLiveFailure {
        SourceLiveFailure {
            diagnostics,
            selected: self.selected,
            checked_run: run,
            checkpoint: self.sink.checkpoint().ok(),
            stage_rows: self.rows,
            model_dispatches: self.model_dispatches,
            effect_dispatches: self.effect_dispatches,
            journal_error: self.journal_error,
        }
    }
    pub(super) fn complete(
        mut self,
        run: Option<IterativeRun>,
        selected: Option<SourceTerminalStatus>,
        mut diagnostics: Vec<Diagnostic>,
        effect_accounting: Option<crate::agent_lifecycle::iterative::driver::EffectAccounting>,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let candidate = self
            .selected
            .or(selected)
            .or_else(|| run.as_ref().map(|r| terminal_status(r.status())));
        if matches!(
            candidate,
            Some(SourceTerminalStatus::Complete | SourceTerminalStatus::Suspend)
        ) {
            self.selected = None;
            if let Err(errors) = self.guard() {
                diagnostics.extend(errors);
            }
        }
        let status = self
            .selected
            .or(selected)
            .or_else(|| run.as_ref().map(|r| terminal_status(r.status())))
            .unwrap_or(SourceTerminalStatus::Rejected);
        self.selected = Some(status);
        if self.sink.poisoned() {
            self.journal_error
                .get_or_insert(SourceJournalError::Poisoned);
            return Err(self.failure(run, diagnostics));
        }
        // Never publish a fresh checked value after a mismatching replay.
        if self.cursor < self.replay.len()
            && !matches!(
                status,
                SourceTerminalStatus::Cancelled
                    | SourceTerminalStatus::DeadlineExceeded
                    | SourceTerminalStatus::BudgetExhausted
            )
            && !matches!(
                self.sink.journal().entries().last(),
                Some(SourceJournalEntry::Stop { .. })
            )
        {
            diagnostics.push(bad("source.replay_incomplete"));
            return Err(self.failure(run, diagnostics));
        }
        let last = self
            .sink
            .journal()
            .entries()
            .iter()
            .rev()
            .find(|e| !sideband(e) && !matches!(e, SourceJournalEntry::StageReservation { .. }))
            .cloned();
        let (turn, attempt) = match &last {
            Some(SourceJournalEntry::RunOpened) | None => (None, None),
            Some(SourceJournalEntry::TurnObserved { turn, .. }) => (Some(*turn), None),
            Some(SourceJournalEntry::Transition {
                turn,
                case: SourceTransitionCase::Continue,
                ..
            }) => (Some(*turn + 1), None),
            Some(SourceJournalEntry::Stop { turn, attempt, .. }) => (*turn, *attempt),
            Some(
                SourceJournalEntry::AttemptIntent { turn, attempt, .. }
                | SourceJournalEntry::AttemptSettled { turn, attempt, .. }
                | SourceJournalEntry::AttemptFailed { turn, attempt, .. }
                | SourceJournalEntry::ProposalRefused { turn, attempt, .. }
                | SourceJournalEntry::ProposalAdmitted { turn, attempt, .. }
                | SourceJournalEntry::AuthorizationConsumed { turn, attempt, .. }
                | SourceJournalEntry::AuthorizationRefused { turn, attempt, .. }
                | SourceJournalEntry::EffectIntent { turn, attempt, .. }
                | SourceJournalEntry::EffectObserved { turn, attempt, .. }
                | SourceJournalEntry::EffectFailed { turn, attempt, .. }
                | SourceJournalEntry::Transition { turn, attempt, .. },
            ) => (Some(*turn), Some(*attempt)),
            _ => {
                diagnostics.push(bad("source.terminal_scope"));
                return Err(self.failure(run, diagnostics));
            }
        };
        if !matches!(last, Some(SourceJournalEntry::Stop { .. })) {
            if let Some((stop, reason)) = stop_pair(status) {
                if let Err(errors) = self.append(SourceJournalEntry::Stop {
                    turn,
                    attempt,
                    status: stop,
                    reason,
                }) {
                    diagnostics.extend(errors);
                    return Err(self.failure(run, diagnostics));
                }
            }
        }
        let carrier = run
            .as_ref()
            .and_then(|r| r.value())
            .filter(|_| stop_pair(status).is_none())
            .map(|v| encode_value(v).into_bytes());
        let mut evidence = SourceTerminalEvidenceInput {
            completed_stages: self.rows.len() as u32,
            omitted_stage_rows: 0,
            stage_rows: self.rows.clone(),
            checked_run_evidence: run
                .as_ref()
                .filter(|r| terminal_status(r.status()) == status)
                .map(|r| r.evidence().as_bytes().to_vec()),
        };
        let entry = loop {
            match self
                .sink
                .terminal_snapshot_entry(turn, status, carrier.clone(), evidence.clone())
            {
                Ok(entry) => break entry,
                Err(SourceJournalError::Capacity) if evidence.checked_run_evidence.is_some() => {
                    evidence.checked_run_evidence = None
                }
                Err(SourceJournalError::Capacity) if !evidence.stage_rows.is_empty() => {
                    let keep = evidence.stage_rows.len() / 2;
                    evidence.stage_rows.truncate(keep);
                    evidence.omitted_stage_rows = evidence.completed_stages - keep as u32;
                }
                Err(error) => {
                    diagnostics.extend(self.journal_failure(error));
                    return Err(self.failure(run, diagnostics));
                }
            }
        };
        if matches!(
            status,
            SourceTerminalStatus::Complete | SourceTerminalStatus::Suspend
        ) {
            self.selected = None;
            if let Err(errors) = self.guard() {
                diagnostics.extend(errors);
                return self.complete(run, Some(status), diagnostics, effect_accounting);
            }
            self.selected = Some(status);
        }
        if let Err(errors) = self.append(entry) {
            diagnostics.extend(errors);
            return Err(self.failure(run, diagnostics));
        }
        if matches!(
            status,
            SourceTerminalStatus::Complete | SourceTerminalStatus::Suspend
        ) {
            self.selected = None;
            if let Err(errors) = self.guard() {
                diagnostics.extend(errors);
            } else {
                self.selected = Some(status);
            }
        }
        if !diagnostics.is_empty() {
            return Err(self.failure(run, diagnostics));
        }
        let checkpoint = match self.sink.checkpoint() {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                diagnostics.extend(self.journal_failure(error));
                return Err(self.failure(run, diagnostics));
            }
        };
        Ok(SourceLiveOutcome {
            checked_run: run,
            checkpoint,
            model_dispatches: self.model_dispatches,
            effect_dispatches: self.effect_dispatches,
            effect_accounting,
        })
    }
}
