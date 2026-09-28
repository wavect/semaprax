//! Pure causal fold over already checked inert carriers. No restoration permits.
use super::super::{SourceStageRole, MAX_SOURCE_ENTRIES};
use super::*;
use model::{OwnedBodyV8 as Body, OwnerV8, PhaseV8};
pub(super) mod cumulative;
#[path = "effect_fold.rs"]
mod effect_fold;
mod initialization;
mod observe_settlement;
#[path = "fold/reduce.rs"]
mod reduce;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TailV8 {
    Reduce,
    Empty,
    Created,
    Opened,
    InitializeReserved,
    Initialized,
    CommittedState,
    ObserveReserved,
    ObserveSettled,
    Observed,
    WaitCreated,
    StartReserved,
    Prepared,
    ModelDispatchInDoubt,
    Settled,
    ProposalRefused,
    TransferInDoubt,
    RearmedState,
    ResumeReserved,
    Completed,
    Admitted,
    TransferReserved,
    PendingAuthorize,
    ChargedAuthorizeReplay,
    PendingReady,
    PendingRefusal,
    ResultDeliveryInDoubt,
    ReadyPair,
    EffectInDoubt,
    EffectSettlementUncommitted,
    EffectSettled,
    EffectCleanupInDoubt,
    EffectDecisionReleased,
    EffectCleanupFailed,
    EffectFailedState,
    FailedState,
    FailedDecisionThenState,
    CleanupInDoubt,
    PendingStateCleanup,
    MetadataOnly,
    Stopped,
    StopInDoubt,
    Terminal,
    TerminalInDoubt,
}
#[derive(Clone, Debug)]
struct Reservation {
    seq: u32,
    phase: PhaseV8,
    replay_of: Option<u32>,
    fuel: u64,
    closure: Option<u32>,
    consumed: Option<u64>,
}
#[derive(Clone, Debug)]
struct Wait {
    id: String,
    attempt: u32,
    created: u32,
    prepared: Option<u32>,
    completed: Option<u32>,
    retired: Option<u32>,
    observation: String,
    copy_arguments: Value,
    start_result: Option<String>,
    resume_result: Option<String>,
    proposal: Option<String>,
    proposal_value: Option<Value>,
    reservations: Vec<Reservation>,
    original_indexes: [Option<usize>; 2],
    latest_indexes: [Option<usize>; 2],
    first_funder_indexes: [Option<usize>; 2],
}
impl Wait {
    fn index(phase: PhaseV8) -> usize {
        match phase {
            PhaseV8::Start => 0,
            PhaseV8::Resume => 1,
        }
    }
    fn original(&self, phase: PhaseV8) -> Option<&Reservation> {
        self.reservations
            .get(self.original_indexes[Self::index(phase)]?)
    }
    fn latest(&self, phase: PhaseV8) -> Option<&Reservation> {
        self.reservations
            .get(self.latest_indexes[Self::index(phase)]?)
    }
    fn first_funder(&self, phase: PhaseV8) -> Option<&Reservation> {
        self.reservations
            .get(self.first_funder_indexes[Self::index(phase)]?)
    }
    fn result(&self, phase: PhaseV8) -> Option<u32> {
        match phase {
            PhaseV8::Start => self.prepared,
            PhaseV8::Resume => self.completed,
        }
    }
    fn ready(&self, phase: PhaseV8) -> bool {
        self.latest(phase).is_some_and(|r| r.closure.is_some())
    }
}

#[derive(Clone, Debug)]
struct Transfer {
    reserved: u32,
    completed: Option<u32>,
    digest: String,
    proposal: String,
}
#[derive(Clone, Debug)]
struct Decision {
    staged: u32,
    digest: String,
    granted: bool,
    ready: Option<u32>,
    grant: Option<String>,
}
#[derive(Clone, Debug)]
struct Cleanup {
    seq: u32,
    owner: OwnerV8,
    basis: u32,
    operations: Value,
    settled: bool,
    host_confirmed: bool,
    completed: bool,
}

/// Recorded consumption is a lower bound: ordinary Observe has no durable
/// consumed field. It must never be advertised as exact terminal consumption.
pub(super) struct FoldV8 {
    pub tail: TailV8,
    continuation_profile_selected: bool,
    current_turn: u32,
    pub reserved_total: u64,
    pub consumed_recorded: u64,
    pub stages: u32,
    state_basis: Option<u32>,
    state_digest: Option<String>,
    state: Option<Value>,
    observation: Option<String>,
    observe_settlement: Option<observe_settlement::ObserveSettlementFactsV8>,
    wait: Option<Wait>,
    transfer: Option<Transfer>,
    decision: Option<Decision>,
    cleanup: Option<Cleanup>,
    effect: Option<effect_fold::EffectV8>,
    reduce: Option<reduce::ReduceJournalV8>,
    failed_effect_state: Option<super::reduce_fold::FailedEffectStateFoldV8>,
    stage_originals: Vec<(u32, SourceStageRole, u64)>,
    stage_current: Option<(u32, SourceStageRole, u64)>,
    ordinary: Vec<SourceJournalEntry>,
    ordinary_sequences: Vec<u32>,
    wait_fuel: u64,
    model_usage_pending: bool,
    model_failed: bool,
    failure_selected: bool,
    cleanup_terminal: Option<Value>,
}
fn order<T>() -> Result<T, SourceJournalError> {
    Err(SourceJournalError::Order)
}
fn require(condition: bool) -> Result<(), SourceJournalError> {
    if condition {
        Ok(())
    } else {
        order()
    }
}
fn add(total: &mut u64, value: u64) -> Result<(), SourceJournalError> {
    *total = total
        .checked_add(value)
        .ok_or(SourceJournalError::Capacity)?;
    Ok(())
}
impl FoldV8 {
    fn empty() -> Self {
        Self {
            tail: TailV8::Empty,
            continuation_profile_selected: false,
            current_turn: 0,
            reserved_total: 0,
            consumed_recorded: 0,
            stages: 0,
            state_basis: None,
            state_digest: None,
            state: None,
            observation: None,
            observe_settlement: None,
            wait: None,
            transfer: None,
            decision: None,
            cleanup: None,
            effect: None,
            reduce: None,
            failed_effect_state: None,
            stage_originals: Vec::new(),
            stage_current: None,
            ordinary: Vec::new(),
            ordinary_sequences: Vec::new(),
            wait_fuel: 0,
            model_usage_pending: false,
            model_failed: false,
            failure_selected: false,
            cleanup_terminal: None,
        }
    }
    pub(super) fn reduce_fold(&self) -> Option<&super::reduce_fold::ReduceFoldV8> {
        self.reduce.as_ref().map(|r| r.fold())
    }
    /// Descriptive capacity template only; no typed inventory or owner permit.
    pub(super) fn capacity_fresh_turn(turn: u32) -> Self {
        let mut template = Self::empty();
        template.current_turn = turn;
        template.tail = TailV8::CommittedState;
        template
    }
    pub(super) fn current_turn(&self) -> u32 {
        self.current_turn
    }
    pub(super) fn failure_selected(&self) -> bool {
        self.failure_selected
    }
    pub(super) fn continuation_profile_selected(&self) -> bool {
        self.continuation_profile_selected
    }
    pub(super) fn failed_effect_state_fold(
        &self,
    ) -> Option<&super::reduce_fold::FailedEffectStateFoldV8> {
        self.failed_effect_state.as_ref()
    }
    pub(super) fn capacity_facts(&self) -> capacity::ClosureFactsV8<'_> {
        let attempt = self.wait.as_ref().map(|w| w.attempt);
        let mut intent = false;
        let mut response_closed = false;
        let mut usage_closed = false;
        for row in &self.ordinary {
            match row {
                SourceJournalEntry::AttemptIntent { attempt: a, .. } if Some(*a) == attempt => {
                    intent = true
                }
                SourceJournalEntry::AttemptSettled { attempt: a, .. }
                | SourceJournalEntry::AttemptFailed { attempt: a, .. }
                    if Some(*a) == attempt =>
                {
                    response_closed = true
                }
                SourceJournalEntry::AttemptUsage { attempt: a, .. } if Some(*a) == attempt => {
                    usage_closed = true
                }
                _ => {}
            }
        }
        capacity::ClosureFactsV8 {
            attempt,
            intent,
            model_failed: self.model_failed,
            response_closed,
            usage_closed,
            pending_historical: self.wait.as_ref().is_some_and(|w| {
                w.reservations
                    .last()
                    .is_some_and(|r| r.closure.is_none() && w.result(r.phase).is_some())
            }),
            cleanup_owner: self.cleanup.as_ref().map(|c| c.owner),
            cleanup_operations: self.cleanup.as_ref().map(|c| &c.operations),
            effect_operations: self.effect.as_ref().and_then(|e| e.operations.as_ref()),
            effect_observed: self.effect.as_ref().is_some_and(|e| e.observed),
        }
    }
    fn current_wait(
        &mut self,
        turn: u32,
        attempt: u32,
        id: &str,
    ) -> Result<&mut Wait, SourceJournalError> {
        if turn != self.current_turn {
            return order();
        }
        self.wait
            .as_mut()
            .filter(|w| w.attempt == attempt && w.id == id)
            .ok_or(SourceJournalError::Order)
    }
    fn reserve(
        &mut self,
        context: &FoldContextV8,
        fuel: u64,
        stage: bool,
        wait: bool,
    ) -> Result<(), SourceJournalError> {
        require(
            Some(fuel)
                == context
                    .ordinary
                    .max_steps_per_stage()
                    .and_then(|fuel| u64::try_from(fuel).ok()),
        )?;
        add(&mut self.reserved_total, fuel)?;
        if self.reserved_total
            > context
                .ordinary
                .max_total_steps()
                .ok_or(SourceJournalError::Binding)? as u64
        {
            return Err(SourceJournalError::Capacity);
        }
        if stage {
            self.stages = self
                .stages
                .checked_add(1)
                .ok_or(SourceJournalError::Capacity)?;
            require(self.stages <= context.ordinary.max_stages())?;
        }
        if wait {
            add(&mut self.wait_fuel, fuel)?;
        }
        Ok(())
    }
    fn consume(&mut self, consumed: u64, fuel: u64) -> Result<(), SourceJournalError> {
        require(consumed <= fuel)?;
        add(&mut self.consumed_recorded, consumed)
    }
}

pub(super) fn fold(
    context: &FoldContextV8,
    entries: &[ValidatedEntryV8],
) -> Result<FoldV8, SourceJournalError> {
    if entries.len() > MAX_SOURCE_ENTRIES {
        return Err(SourceJournalError::Capacity);
    }
    let mut fold = FoldV8::empty();
    for (index, row) in entries.iter().enumerate() {
        let seq = u32::try_from(index).map_err(|_| SourceJournalError::Capacity)?;
        require(!matches!(
            fold.tail,
            TailV8::Terminal | TailV8::TerminalInDoubt
        ))?;
        if matches!(fold.tail, TailV8::Stopped | TailV8::StopInDoubt) {
            require(matches!(
                row.entry,
                EntryV8::Ordinary(SourceJournalEntry::TerminalSnapshot { .. })
            ))?;
        }
        if fold.effect.is_some() {
            require(
                cumulative::is_next_state_commit(context, &fold, &row.entry)
                    || effect_fold::is_effect_row(&row.entry)
                    || (is_reduce_row(&row.entry)
                        && matches!(
                            fold.tail,
                            TailV8::EffectDecisionReleased
                                | TailV8::EffectFailedState
                                | TailV8::Reduce
                        )),
            )?;
        }
        match &row.entry {
            EntryV8::Owned(body) => {
                if let Body::OwnedWaitCreated { copy_arguments, .. } = body {
                    let facts = row
                        .observation
                        .as_ref()
                        .ok_or(SourceJournalError::Binding)?;
                    let Body::OwnedRunCreated { scope, binding, .. } = &context.created else {
                        return order();
                    };
                    require(
                        facts.matches(binding, scope)
                            && facts.copy_arguments() == copy_arguments
                            && fold.observation.as_deref() == Some(facts.ordinary_digest()),
                    )?;
                }
                if let Body::OwnedWaitPrepared {
                    observation_digest, ..
                } = body
                {
                    let facts = row
                        .observation
                        .as_ref()
                        .ok_or(SourceJournalError::Binding)?;
                    let Body::OwnedRunCreated { scope, binding, .. } = &context.created else {
                        return order();
                    };
                    require(
                        facts.matches(binding, scope)
                            && facts.request_digest() == observation_digest
                            && fold.observation.as_deref() == Some(facts.ordinary_digest())
                            && fold
                                .wait
                                .as_ref()
                                .is_some_and(|w| &w.copy_arguments == facts.copy_arguments()),
                    )?;
                }
                owned(context, &mut fold, body, seq)?;
            }
            EntryV8::Ordinary(entry) => ordinary(context, &mut fold, entry, seq)?,
        }
    }
    // Reuse the ordinary phase grammar without fake Initialize or migration.
    // Replay references are independently checked at their true combined seqs
    // above, then remapped only inside this inert validation projection.
    let mut projected = fold.ordinary.clone();
    for entry in &mut projected {
        if let SourceJournalEntry::ReplayStageReservation { causal_seq, .. } = entry {
            *causal_seq = u32::try_from(
                fold.ordinary_sequences
                    .iter()
                    .position(|seq| *seq == *causal_seq)
                    .ok_or(SourceJournalError::Order)?,
            )
            .map_err(|_| SourceJournalError::Capacity)?;
        }
    }
    let ordinary_fold = super::super::execution::validate_inner_seeded(
        &context.ordinary,
        &projected,
        fold.wait_fuel,
        if context.initialized_task.is_some() {
            super::super::validate::InitialStage::InitializeThenObserve
        } else {
            super::super::validate::InitialStage::ObserveOnly
        },
    )?;
    require(
        ordinary_fold.stage_fuel == fold.reserved_total && ordinary_fold.stages == fold.stages,
    )?;
    Ok(fold)
}

fn owned(
    context: &FoldContextV8,
    f: &mut FoldV8,
    b: &Body,
    seq: u32,
) -> Result<(), SourceJournalError> {
    observe_settlement::validate_cleanup(context, f, b)?;
    if observe_settlement::settle(context, f, b, seq)? {
        return Ok(());
    }
    if cumulative::commit_next_state(context, f, b, seq)? {
        return Ok(());
    }
    if reduce::owned(context, f, b, seq)? {
        return Ok(());
    }
    match b {
        Body::OwnedRunCreated { .. } => {
            require(f.tail == TailV8::Empty && b == &context.created)?;
            f.tail = TailV8::Created;
        }
        Body::OwnedContinuationProfileSelected { .. } => {
            cumulative::select_profile(context, f, b, seq)?;
        }
        Body::OwnedInitializationCommitted { .. } => initialization::commit(context, f, b)?,
        Body::OwnedObserveSettled { .. }
        | Body::OwnedReduceStaged { .. }
        | Body::OwnedReduceCleanupStarted { .. }
        | Body::OwnedReduceCleanupSettled { .. }
        | Body::OwnedStepTransferReserved { .. }
        | Body::OwnedStepTransferCompleted { .. }
        | Body::OwnedEffectFailureStateCleanupStarted { .. }
        | Body::OwnedEffectFailureStateCleanupSettled { .. } => return order(),
        Body::OwnedStateCommitted {
            turn,
            state,
            argument_digest,
            cleanup_plan_digest,
        } => {
            require(
                f.tail
                    == if context.initialized_task.is_some() {
                        TailV8::Initialized
                    } else {
                        TailV8::Opened
                    }
                    && *turn == f.current_turn
                    && cleanup_plan_digest == &context.cleanup_plan_digest,
            )?;
            require(
                crate::live_invocation::identity::digest(
                    b"semaprax.source-owned-frame-args.v2\0",
                    &wire::canonical(state),
                ) == *argument_digest,
            )?;
            if context.initialized_task.is_some() {
                require(
                    f.state.as_ref() == Some(state)
                        && f.state_digest.as_ref() == Some(argument_digest),
                )?;
            }
            f.state = Some(state.clone());
            f.state_digest = Some(argument_digest.clone());
            f.state_basis = Some(seq);
            f.tail = TailV8::CommittedState;
        }
        Body::OwnedWaitCreated {
            turn,
            attempt,
            wait,
            plan_digest,
            cleanup_plan_digest,
            signature,
            argument_digest,
            copy_arguments,
            copy_arguments_digest,
        } => {
            require(
                *turn == f.current_turn
                    && matches!(f.tail, TailV8::Observed | TailV8::RearmedState)
                    && !f.failure_selected,
            )?;
            require(f.wait.as_ref().map_or(*attempt == 0, |w| {
                w.attempt.checked_add(1) == Some(*attempt)
            }))?;
            require(
                *attempt < context.ordinary.max_attempts()
                    && plan_digest == &context.plan_digest
                    && cleanup_plan_digest == &context.cleanup_plan_digest
                    && signature == &context.signature
                    && f.state_digest.as_ref() == Some(argument_digest),
            )?;
            require(
                crate::live_invocation::identity::digest(
                    b"semaprax.source-owned-frame-copy-args.v2\0",
                    &wire::canonical(copy_arguments),
                ) == *copy_arguments_digest,
            )?;
            let (execution, binding) = match &context.created {
                Body::OwnedRunCreated {
                    execution, binding, ..
                } => (execution, binding),
                _ => return order(),
            };
            let invocation = wire::recipe_digest(
                wire::RecipeV8::Invocation,
                &serde_json::json!({"execution":execution,"owned_wait_binding":binding}),
            )?;
            require(
                wire::recipe_digest(
                    wire::RecipeV8::Attempt,
                    &serde_json::json!({"invocation":invocation,"turn":turn,"attempt":attempt,"binding":binding}),
                )? == *wait,
            )?;
            f.wait = Some(Wait {
                id: wait.clone(),
                attempt: *attempt,
                created: seq,
                prepared: None,
                completed: None,
                retired: None,
                observation: String::new(),
                copy_arguments: copy_arguments.clone(),
                start_result: None,
                resume_result: None,
                proposal: None,
                proposal_value: None,
                reservations: Vec::new(),
                original_indexes: [None; 2],
                latest_indexes: [None; 2],
                first_funder_indexes: [None; 2],
            });
            f.tail = TailV8::WaitCreated;
        }
        Body::OwnedWaitReserved {
            turn,
            attempt,
            wait,
            phase,
            replay_of,
            fuel,
        } => {
            require(!f.failure_selected && !f.model_failed)?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            if let Some(original) = replay_of {
                let original_row = w.original(*phase).ok_or(SourceJournalError::Order)?;
                require(original_row.seq == *original && w.retired.is_none())?;
                if *phase == PhaseV8::Resume {
                    let start = w.latest(PhaseV8::Start).ok_or(SourceJournalError::Order)?;
                    let previous_resume =
                        w.latest(PhaseV8::Resume).ok_or(SourceJournalError::Order)?;
                    require(
                        start.replay_of.is_some()
                            && start.closure.is_some()
                            && start.seq > previous_resume.seq,
                    )?;
                }
                let has_result = w.result(*phase).is_some();
                if has_result {
                    require(matches!(
                        f.tail,
                        TailV8::Prepared
                            | TailV8::Settled
                            | TailV8::ResumeReserved
                            | TailV8::Completed
                            | TailV8::Admitted
                    ))?;
                } else {
                    require(match phase {
                        PhaseV8::Start => f.tail == TailV8::StartReserved,
                        PhaseV8::Resume => f.tail == TailV8::ResumeReserved,
                    })?;
                }
            } else {
                require(w.original(*phase).is_none())?;
                match phase {
                    PhaseV8::Start => require(f.tail == TailV8::WaitCreated)?,
                    PhaseV8::Resume => {
                        require(f.tail == TailV8::Settled && !f.model_usage_pending)?;
                        require(f.wait.as_ref().is_some_and(|w| w.ready(PhaseV8::Start)))?;
                    }
                }
                f.tail = if *phase == PhaseV8::Start {
                    TailV8::StartReserved
                } else {
                    TailV8::ResumeReserved
                };
            }
            f.reserve(context, *fuel, false, true)?;
            let w = f.wait.as_mut().expect("matched wait");
            let index = w.reservations.len();
            w.latest_indexes[Wait::index(*phase)] = Some(index);
            if replay_of.is_none() {
                w.original_indexes[Wait::index(*phase)] = Some(index);
            }
            w.reservations.push(Reservation {
                seq,
                phase: *phase,
                replay_of: *replay_of,
                fuel: *fuel,
                closure: None,
                consumed: None,
            });
        }
        Body::OwnedWaitPrepared {
            turn,
            attempt,
            wait,
            reservation,
            observation_digest,
            checkpoint_digest,
            checkpoint,
            consumed,
        } => {
            require(f.tail == TailV8::StartReserved)?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            require(w.prepared.is_none())?;
            let r = w
                .latest(PhaseV8::Start)
                .filter(|r| r.seq == *reservation && r.closure.is_none())
                .ok_or(SourceJournalError::Order)?
                .clone();
            let bytes = crate::live_invocation::identity::unhex(checkpoint)
                .ok_or(SourceJournalError::Malformed)?;
            require(wire::checkpoint_bytes_digest(&bytes)? == *checkpoint_digest)?;
            require(w.reservations.last().is_some_and(|last| last.seq == r.seq))?;
            f.consume(*consumed, r.fuel)?;
            let w = f.wait.as_mut().expect("matched wait");
            let index =
                w.latest_indexes[Wait::index(PhaseV8::Start)].ok_or(SourceJournalError::Order)?;
            let r = w
                .reservations
                .get_mut(index)
                .ok_or(SourceJournalError::Order)?;
            r.closure = Some(seq);
            r.consumed = Some(*consumed);
            w.prepared = Some(seq);
            w.first_funder_indexes[Wait::index(PhaseV8::Start)] = Some(index);
            w.observation = observation_digest.clone();
            w.start_result = Some(checkpoint_digest.clone());
            f.state_basis = Some(seq);
            f.tail = TailV8::Prepared;
        }
        Body::OwnedWaitCompleted {
            turn,
            attempt,
            wait,
            reservation,
            proposal,
            proposal_digest,
            result_digest,
            consumed,
        } => {
            require(f.tail == TailV8::ResumeReserved)?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            require(w.ready(PhaseV8::Start) && w.completed.is_none())?;
            let r = w
                .latest(PhaseV8::Resume)
                .filter(|r| r.seq == *reservation && r.closure.is_none())
                .ok_or(SourceJournalError::Order)?
                .clone();
            require(
                w.reservations.last().is_some_and(|last| last.seq == r.seq)
                    && w.latest(PhaseV8::Start)
                        .is_some_and(|start| start.seq < r.seq),
            )?;
            f.consume(*consumed, r.fuel)?;
            let w = f.wait.as_mut().expect("matched wait");
            let index =
                w.latest_indexes[Wait::index(PhaseV8::Resume)].ok_or(SourceJournalError::Order)?;
            let r = w
                .reservations
                .get_mut(index)
                .ok_or(SourceJournalError::Order)?;
            r.closure = Some(seq);
            r.consumed = Some(*consumed);
            w.completed = Some(seq);
            w.first_funder_indexes[Wait::index(PhaseV8::Resume)] = Some(index);
            w.resume_result = Some(result_digest.clone());
            w.proposal = Some(proposal_digest.clone());
            w.proposal_value = Some(proposal.clone());
            f.state_basis = Some(seq);
            f.tail = TailV8::Completed;
        }
        Body::OwnedWaitReplayChecked {
            turn,
            attempt,
            wait,
            reservation,
            original,
            result_digest,
            consumed,
        } => {
            require(matches!(
                f.tail,
                TailV8::Prepared
                    | TailV8::Settled
                    | TailV8::ResumeReserved
                    | TailV8::Completed
                    | TailV8::Admitted
            ))?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            let r = w
                .reservations
                .last()
                .filter(|r| r.seq == *reservation && r.replay_of.is_some() && r.closure.is_none())
                .ok_or(SourceJournalError::Order)?
                .clone();
            let prior = w.original(r.phase).ok_or(SourceJournalError::Order)?;
            require(prior.seq == r.replay_of.expect("replay"))?;
            let funded = w.first_funder(r.phase).ok_or(SourceJournalError::Order)?;
            require(
                funded.phase == r.phase
                    && funded.closure == Some(*original)
                    && (funded.seq == prior.seq || funded.replay_of == Some(prior.seq)),
            )?;
            // Start and Resume closures carry different immutable result hashes.
            let expected = if r.phase == PhaseV8::Start {
                w.prepared
            } else {
                w.completed
            };
            require(expected == Some(*original))?;
            // Closure digest lookup is tied to the exact causal body by caller.
            // The result is additionally checked against the referenced row below.
            require(
                if r.phase == PhaseV8::Start {
                    w.start_result.as_ref()
                } else {
                    w.resume_result.as_ref()
                } == Some(result_digest),
            )?;
            f.consume(*consumed, r.fuel)?;
            let w = f.wait.as_mut().expect("matched wait");
            let r = w.reservations.last_mut().expect("matched reservation");
            r.closure = Some(seq);
            r.consumed = Some(*consumed);
        }
        Body::OwnedWaitFailed {
            turn,
            attempt,
            wait,
            reservation,
            status,
            consumed,
        } => {
            require(
                !matches!(
                    f.tail,
                    TailV8::ModelDispatchInDoubt
                        | TailV8::TransferInDoubt
                        | TailV8::TransferReserved
                        | TailV8::ResultDeliveryInDoubt
                ) && !f.failure_selected,
            )?;
            require(!f.model_usage_pending)?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            if let Some(funded) = reservation {
                let r = w
                    .reservations
                    .last()
                    .filter(|r| r.seq == *funded && r.closure.is_none())
                    .ok_or(SourceJournalError::Order)?
                    .clone();
                f.consume(*consumed, r.fuel)?;
                let r = f
                    .wait
                    .as_mut()
                    .expect("matched wait")
                    .reservations
                    .last_mut()
                    .expect("reservation");
                r.closure = Some(seq);
                r.consumed = Some(*consumed);
            } else {
                require(
                    *consumed == 0 && w.reservations.last().is_none_or(|r| r.closure.is_some()),
                )?;
            }
            f.failure_selected = true;
            f.cleanup_terminal = Some(status.clone());
            f.tail = TailV8::FailedState;
        }
        Body::OwnedWaitRetired {
            turn,
            attempt,
            wait,
            prepared,
            state_digest,
            observation_digest,
        } => {
            require(
                f.tail == TailV8::ProposalRefused && f.state_digest.as_ref() == Some(state_digest),
            )?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            require(
                w.prepared == Some(*prepared)
                    && w.completed.is_none()
                    && w.original(PhaseV8::Resume).is_none()
                    && w.observation == *observation_digest
                    && w.retired.is_none(),
            )?;
            f.wait.as_mut().expect("matched wait").retired = Some(seq);
            f.state_basis = None;
            f.tail = TailV8::TransferInDoubt;
        }
        Body::OwnedStateRearmed {
            turn,
            attempt,
            wait,
            retired,
            state,
            state_digest,
            observation_digest,
            observation,
        } => {
            require(
                f.tail == TailV8::TransferInDoubt
                    && f.state.as_ref() == Some(state)
                    && f.state_digest.as_ref() == Some(state_digest),
            )?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            require(
                w.retired == Some(*retired)
                    && w.observation == *observation_digest
                    && w.copy_arguments.get(0).and_then(|v| v.get("value")) == Some(observation),
            )?;
            f.state_basis = Some(seq);
            f.tail = TailV8::RearmedState;
        }
        Body::OwnedStateTransferReserved {
            turn,
            attempt,
            wait,
            from,
            to,
            state_digest,
            proposal_digest,
            transfer_digest,
        } => {
            require(
                f.tail == TailV8::Admitted
                    && *turn == f.current_turn
                    && from == &context.helper
                    && to == &context.authorize
                    && f.state_digest.as_ref() == Some(state_digest),
            )?;
            let w = f.current_wait(*turn, *attempt, wait)?;
            require(
                w.ready(PhaseV8::Start)
                    && w.ready(PhaseV8::Resume)
                    && w.proposal.as_ref() == Some(proposal_digest),
            )?;
            let Body::OwnedRunCreated { scope, .. } = &context.created else {
                return order();
            };
            let generation = wire::generation_digest_from_created(&context.created)?;
            require(
                wire::recipe_digest(
                    wire::RecipeV8::Transfer,
                    &serde_json::json!({"scope":scope,"generation":generation,"turn":turn,"attempt":attempt,"wait":wait,"from":from,"to":to,"state_digest":state_digest,"proposal_digest":proposal_digest}),
                )? == *transfer_digest,
            )?;
            f.transfer = Some(Transfer {
                reserved: seq,
                completed: None,
                digest: transfer_digest.clone(),
                proposal: proposal_digest.clone(),
            });
            f.state_basis = None;
            f.tail = TailV8::TransferReserved;
        }
        Body::OwnedStateTransferCompleted {
            turn,
            attempt,
            wait,
            reservation,
            state,
            state_digest,
            proposal,
            proposal_digest,
            transfer_digest,
        } => {
            require(
                f.tail == TailV8::TransferReserved
                    && f.state.as_ref() == Some(state)
                    && f.state_digest.as_ref() == Some(state_digest),
            )?;
            require(
                f.current_wait(*turn, *attempt, wait)?
                    .proposal_value
                    .as_ref()
                    == Some(proposal),
            )?;
            let t = f
                .transfer
                .as_mut()
                .filter(|t| {
                    t.reserved == *reservation
                        && t.digest == *transfer_digest
                        && t.proposal == *proposal_digest
                        && t.completed.is_none()
                })
                .ok_or(SourceJournalError::Order)?;
            t.completed = Some(seq);
            f.state_basis = Some(seq);
            f.tail = TailV8::PendingAuthorize;
        }
        Body::OwnedAuthorizationStaged {
            turn,
            attempt,
            stage_reservation,
            transfer,
            state_digest,
            proposal_digest,
            decision,
            decision_digest,
            consumed,
        } => {
            require(
                f.tail == TailV8::ChargedAuthorizeReplay
                    && *turn == f.current_turn
                    && f.state_digest.as_ref() == Some(state_digest),
            )?;
            require(
                f.wait.as_ref().is_some_and(|w| w.attempt == *attempt)
                    && f.transfer.as_ref().is_some_and(|t| {
                        t.completed == Some(*transfer) && t.proposal == *proposal_digest
                    }),
            )?;
            let r = f
                .stage_current
                .filter(|(s, role, _)| {
                    *s == *stage_reservation && *role == SourceStageRole::Authorize
                })
                .ok_or(SourceJournalError::Order)?;
            f.consume(*consumed, r.2)?;
            let Body::OwnedRunCreated { scope, .. } = &context.created else {
                return order();
            };
            require(
                wire::recipe_digest(
                    wire::RecipeV8::Decision,
                    &serde_json::json!({"scope":scope,"turn":turn,"attempt":attempt,"authorize":context.authorize,"decision":decision}),
                )? == *decision_digest,
            )?;
            let case = decision
                .get("case")
                .and_then(Value::as_str)
                .ok_or(SourceJournalError::Malformed)?;
            require(case == context.granted || case == context.refused)?;
            let granted = case == context.granted;
            f.decision = Some(Decision {
                staged: seq,
                digest: decision_digest.clone(),
                granted,
                ready: None,
                grant: None,
            });
            f.tail = if granted {
                TailV8::PendingReady
            } else {
                TailV8::PendingRefusal
            };
        }
        Body::OwnedAuthorizationReady {
            turn,
            attempt,
            staged,
            state_digest,
            decision_digest,
            grant_digest,
        } => {
            require(
                f.tail == TailV8::PendingReady
                    && *turn == f.current_turn
                    && f.wait.as_ref().is_some_and(|w| w.attempt == *attempt)
                    && f.state_digest.as_ref() == Some(state_digest)
                    && !f.failure_selected,
            )?;
            let d = f
                .decision
                .as_mut()
                .filter(|d| {
                    d.staged == *staged
                        && d.granted
                        && d.digest == *decision_digest
                        && d.ready.is_none()
                })
                .ok_or(SourceJournalError::Order)?;
            d.ready = Some(seq);
            d.grant = Some(grant_digest.clone());
            f.tail = TailV8::ResultDeliveryInDoubt;
        }
        Body::OwnedEffectSettlementRecorded { .. }
        | Body::OwnedEffectDecisionCleanupStarted { .. }
        | Body::OwnedEffectDecisionCleanupSettled { .. } => effect_fold::owned(context, f, b, seq)?,
        Body::OwnedCleanupStarted {
            turn,
            attempt,
            wait,
            owner,
            basis,
            terminal,
            operations,
            operations_digest,
        } => {
            require(
                *turn == f.current_turn
                    && f.effect.is_none()
                    && !matches!(
                        f.tail,
                        TailV8::ResultDeliveryInDoubt
                            | TailV8::ModelDispatchInDoubt
                            | TailV8::CleanupInDoubt
                            | TailV8::TransferReserved
                            | TailV8::TransferInDoubt
                    )
                    && f.cleanup.as_ref().is_none_or(|c| c.settled),
            )?;
            require(attempt.map_or(f.wait.is_none(), |a| {
                f.wait
                    .as_ref()
                    .is_some_and(|w| w.attempt == a && wait.as_ref() == Some(&w.id))
            }))?;
            require(
                wire::recipe_digest(
                    wire::RecipeV8::Operations,
                    &serde_json::json!({"owner":owner,"basis":basis,"terminal":terminal,"operations":operations}),
                )? == *operations_digest,
            )?;
            // Pre-wait Observe and pre-Staged authorize failures have no
            // ordinary failure body. The checked actual obligation first pins
            // its sticky failure here, before any Stop or physical operation.
            if !f.failure_selected {
                require(matches!(
                    f.tail,
                    TailV8::ObserveReserved | TailV8::ChargedAuthorizeReplay
                ))?;
                f.failure_selected = true;
            }
            if let Some(selected) = &f.cleanup_terminal {
                require(selected == terminal)?;
            } else {
                f.cleanup_terminal = Some(terminal.clone());
            }
            match owner {
                OwnerV8::State => {
                    require(
                        f.state_basis == Some(*basis)
                            && f.decision
                                .as_ref()
                                .is_none_or(|d| !d.granted && context.refused_cleanup_empty),
                    )?;
                    f.state_basis = None;
                    f.decision = None;
                }
                OwnerV8::Decision => {
                    // A failed partial constructor is never a full Decision.
                    // Its compiler-validated obligation names the newest funded
                    // authorize execution, not an invented Staged snapshot.
                    require(
                        f.decision.as_ref().is_some_and(|d| d.staged == *basis)
                            || (f.decision.is_none()
                                && f.tail == TailV8::ChargedAuthorizeReplay
                                && f.stage_current.is_some_and(|(s, role, _)| {
                                    s == *basis && role == SourceStageRole::Authorize
                                })),
                    )?;
                }
            }
            f.cleanup = Some(Cleanup {
                seq,
                owner: *owner,
                basis: *basis,
                operations: operations.clone(),
                settled: false,
                host_confirmed: false,
                completed: false,
            });
            f.tail = TailV8::CleanupInDoubt;
        }
        Body::OwnedCleanupSettled {
            turn,
            attempt,
            wait,
            owner,
            started,
            receipt,
            receipt_digest,
        } => {
            require(
                f.tail == TailV8::CleanupInDoubt
                    && *turn == f.current_turn
                    && attempt.map_or(f.wait.is_none(), |a| {
                        f.wait
                            .as_ref()
                            .is_some_and(|w| w.attempt == a && wait.as_ref() == Some(&w.id))
                    }),
            )?;
            require(wire::recipe_digest(wire::RecipeV8::Receipt, receipt)? == *receipt_digest)?;
            let c = f
                .cleanup
                .as_mut()
                .filter(|c| c.seq == *started && c.owner == *owner && !c.settled)
                .ok_or(SourceJournalError::Order)?;
            let kind = receipt
                .get("kind")
                .and_then(Value::as_str)
                .ok_or(SourceJournalError::Malformed)?;
            require(matches!(kind, "observed" | "host_confirmed"))?;
            c.settled = true;
            c.host_confirmed = kind == "host_confirmed";
            c.completed = receipt["settlement"] == "completed";
            match owner {
                OwnerV8::State => {
                    f.state = None;
                    f.state_digest = None;
                    f.tail = TailV8::MetadataOnly;
                }
                OwnerV8::Decision => {
                    f.decision = None;
                    if c.host_confirmed {
                        f.state_basis = None;
                        f.tail = TailV8::CleanupInDoubt;
                    } else {
                        f.tail = TailV8::PendingStateCleanup;
                    }
                }
            }
        }
    }
    Ok(())
}

fn ordinary(
    context: &FoldContextV8,
    f: &mut FoldV8,
    e: &SourceJournalEntry,
    seq: u32,
) -> Result<(), SourceJournalError> {
    use SourceJournalEntry as E;
    if reduce::ordinary(context, f, e, seq)? {
        f.ordinary_sequences.push(seq);
        f.ordinary.push(e.clone());
        return Ok(());
    }
    match e {
        E::RunOpened => {
            require(
                f.tail == TailV8::Created
                    && f.continuation_profile_selected == context.cumulative_initialization,
            )?;
            f.tail = TailV8::Opened;
        }
        E::StageReservation {
            turn,
            attempt,
            role,
            fuel,
        } => {
            require(*turn == f.current_turn && !f.failure_selected)?;
            match role {
                SourceStageRole::Initialize => {
                    require(
                        context.initialized_task.is_some()
                            && f.tail == TailV8::Opened
                            && attempt.is_none(),
                    )?;
                    f.tail = TailV8::InitializeReserved;
                }
                SourceStageRole::Observe => {
                    require(f.tail == TailV8::CommittedState && attempt.is_none())?;
                    f.tail = TailV8::ObserveReserved;
                }
                SourceStageRole::Authorize => {
                    require(
                        f.tail == TailV8::PendingAuthorize
                            && f.wait.as_ref().is_some_and(|w| Some(w.attempt) == *attempt),
                    )?;
                    f.tail = TailV8::ChargedAuthorizeReplay;
                }
                _ => return order(),
            }
            f.reserve(context, *fuel as u64, true, false)?;
            f.stage_originals.push((seq, *role, *fuel as u64));
            f.stage_current = Some((seq, *role, *fuel as u64));
        }
        E::ReplayStageReservation {
            causal_seq,
            role,
            fuel,
            ..
        } => {
            require(
                !f.failure_selected
                    && f.effect.is_none()
                    && !matches!(
                        f.tail,
                        TailV8::ModelDispatchInDoubt
                            | TailV8::TransferInDoubt
                            | TailV8::TransferReserved
                            | TailV8::CleanupInDoubt
                            | TailV8::ResultDeliveryInDoubt
                            | TailV8::MetadataOnly
                            | TailV8::Stopped
                    ),
            )?;
            require(
                f.stage_originals
                    .iter()
                    .any(|(s, r, f)| s == causal_seq && r == role && *f == *fuel as u64),
            )?;
            f.reserve(context, *fuel as u64, false, false)?;
            if *role == SourceStageRole::Authorize {
                require(f.tail == TailV8::ChargedAuthorizeReplay)?;
                f.stage_current = Some((seq, *role, *fuel as u64));
            }
        }
        E::TurnObserved {
            turn,
            state,
            observation,
            ..
        } => {
            require(
                *turn == f.current_turn
                    && f.state_basis.is_some()
                    && crate::live_invocation::identity::looks_like_digest(state),
            )?;
            observe_settlement::validate_observed(context, f, seq, state, observation)?;
            f.observation = Some(observation.clone());
            f.stage_current = None;
            f.tail = TailV8::Observed;
        }
        E::AttemptIntent { turn, attempt, .. } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::Prepared
                    && !f.failure_selected
                    && f.wait
                        .as_ref()
                        .is_some_and(|w| w.attempt == *attempt && w.ready(PhaseV8::Start)),
            )?;
            f.tail = TailV8::ModelDispatchInDoubt;
        }
        E::AttemptSettled { turn, attempt, .. } | E::AttemptFailed { turn, attempt, .. } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::ModelDispatchInDoubt
                    && f.wait.as_ref().is_some_and(|w| w.attempt == *attempt),
            )?;
            f.model_failed = matches!(e, E::AttemptFailed { .. });
            f.model_usage_pending = true;
            f.tail = TailV8::Settled;
        }
        E::AttemptUsage { turn, attempt, .. } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::Settled
                    && f.model_usage_pending
                    && f.wait.as_ref().is_some_and(|w| w.attempt == *attempt),
            )?;
            f.model_usage_pending = false;
        }
        E::ProposalRefused { turn, attempt, .. } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::Settled
                    && !f.model_usage_pending
                    && !f.model_failed
                    && f.wait.as_ref().is_some_and(|w| {
                        w.attempt == *attempt
                            && w.ready(PhaseV8::Start)
                            && w.original(PhaseV8::Resume).is_none()
                    }),
            )?;
            f.tail = TailV8::ProposalRefused;
        }
        E::ProposalAdmitted {
            turn,
            attempt,
            proposal_digest,
        } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::Completed
                    && !f.model_failed
                    && f.wait.as_ref().is_some_and(|w| {
                        w.attempt == *attempt
                            && w.ready(PhaseV8::Start)
                            && w.ready(PhaseV8::Resume)
                            && w.proposal.as_ref() == Some(proposal_digest)
                    }),
            )?;
            f.tail = TailV8::Admitted;
        }
        E::AuthorizationConsumed {
            turn,
            attempt,
            grant_digest,
        } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::ResultDeliveryInDoubt
                    && !f.failure_selected
                    && f.wait.as_ref().is_some_and(|w| w.attempt == *attempt)
                    && f.decision.as_ref().is_some_and(|d| {
                        d.granted && d.ready.is_some() && d.grant.as_ref() == Some(grant_digest)
                    }),
            )?;
            f.effect = Some(effect_fold::EffectV8::consumed(seq));
            f.tail = TailV8::ReadyPair;
        }
        E::EffectIntent { .. } | E::EffectObserved { .. } | E::EffectFailed { .. } => {
            effect_fold::ordinary(f, e, seq)?;
        }
        E::AuthorizationRefused { turn, attempt, .. } => {
            require(
                *turn == f.current_turn
                    && f.tail == TailV8::PendingRefusal
                    && f.wait.as_ref().is_some_and(|w| w.attempt == *attempt)
                    && f.decision.as_ref().is_some_and(|d| !d.granted),
            )?;
            f.failure_selected = true;
            f.tail = TailV8::FailedDecisionThenState;
        }
        E::Stop { status, reason, .. } => {
            require(f.effect.is_none())?;
            observe_settlement::validate_stop(f, *status, *reason)?;
            if f.tail == TailV8::MetadataOnly && f.state_basis.is_none() && f.decision.is_none() {
                f.tail = TailV8::Stopped;
            } else {
                // Historical pre-cleanup Stop is never a restoration basis.
                // A producer is separately forbidden from creating this tail.
                f.state_basis = None;
                f.state = None;
                f.state_digest = None;
                f.decision = None;
                f.transfer = None;
                f.tail = TailV8::StopInDoubt;
            }
        }
        E::TerminalSnapshot { .. } => {
            f.tail = match f.tail {
                TailV8::Stopped => TailV8::Terminal,
                TailV8::StopInDoubt => TailV8::TerminalInDoubt,
                _ => return order(),
            };
        }
        // Effect/reduce/Step/next-turn and all other versioned profiles remain
        // unsupported by this first partial proof-data grammar.
        _ => return order(),
    }
    f.ordinary_sequences.push(seq);
    f.ordinary.push(e.clone());
    Ok(())
}

#[cfg(test)]
#[path = "fold_tests.rs"]
pub(super) mod tests;

/// Recovery recognizes uncertain historical tails; the candidate producer must
/// invoke this additional gate before extending a checked prefix.
pub(super) fn validate_producer_transition(
    previous: &FoldV8,
    row: &ValidatedEntryV8,
) -> Result<(), SourceJournalError> {
    require(!effect_fold::is_effect_row(&row.entry))?;
    require(!matches!(
        row.entry,
        EntryV8::Owned(Body::OwnedObserveSettled { .. })
    ))?;
    require(
        !(previous.reduce.is_some()
            && matches!(row.entry, EntryV8::Owned(Body::OwnedStateCommitted { .. }))),
    )?;
    require(
        !is_reduce_row(&row.entry)
            || (previous.reduce.is_none()
                && previous.failed_effect_state.is_none()
                && matches!(
                    row.entry,
                    EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
                )),
    )?;
    if matches!(
        row.entry,
        EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
    ) {
        require(previous.tail == TailV8::MetadataOnly)?;
    }
    require(!matches!(
        previous.tail,
        TailV8::StopInDoubt | TailV8::TerminalInDoubt
    ))
}

fn is_reduce_row(row: &EntryV8) -> bool {
    matches!(
        row,
        EntryV8::Owned(
            Body::OwnedReduceStaged { .. }
                | Body::OwnedReduceCleanupStarted { .. }
                | Body::OwnedReduceCleanupSettled { .. }
                | Body::OwnedStepTransferReserved { .. }
                | Body::OwnedStepTransferCompleted { .. }
                | Body::OwnedEffectFailureStateCleanupStarted { .. }
                | Body::OwnedEffectFailureStateCleanupSettled { .. }
        ) | EntryV8::Ordinary(
            SourceJournalEntry::StageReservation {
                role: SourceStageRole::Reduce,
                ..
            } | SourceJournalEntry::Transition { .. }
                | SourceJournalEntry::Stop { .. }
        )
    )
}
