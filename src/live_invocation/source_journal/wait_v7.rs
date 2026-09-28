//! Opt-in single-journal model waits. Ordinary entries retain their frozen API;
//! private wait rows occupy the same immutable generation/sequence inventory.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
mod tests;
pub(super) mod wire;

pub(super) const SCHEMA: &str = "semaprax.live-invocation.source-persisted-journal.v7";
pub(crate) const SOURCE_MODEL_WAIT_CHECKPOINT_LIMIT: usize = 32_768;
const ID_DOMAIN_V7: &[u8] = b"semaprax.live-invocation.source-id.v7\0";
const WAIT_DOMAIN: &[u8] = b"semaprax.source-model-wait.attempt.v1\0";
const CHECKPOINT_DOMAIN: &[u8] = b"semaprax.source-model-wait.checkpoint.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.source-model-wait.evidence.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceModelWaitProfileV7 {
    wrapper_binding: String,
    source_revision: String,
    evaluation_fuel: usize,
}
impl SourceModelWaitProfileV7 {
    pub(crate) fn new(
        wrapper_binding: String,
        source_revision: String,
        evaluation_fuel: usize,
    ) -> Result<Self, SourceJournalError> {
        if !looks_like_digest(&wrapper_binding)
            || !looks_like_digest(&source_revision)
            || !(1..=1_000_000).contains(&evaluation_fuel)
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(Self {
            wrapper_binding,
            source_revision,
            evaluation_fuel,
        })
    }
    pub(crate) fn wrapper_binding(&self) -> &str {
        &self.wrapper_binding
    }
    pub(crate) fn source_revision(&self) -> &str {
        &self.source_revision
    }
    pub(crate) fn evaluation_fuel(&self) -> usize {
        self.evaluation_fuel
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceModelWaitPhaseV7 {
    Start,
    Resume,
}
impl SourceModelWaitPhaseV7 {
    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Resume => "resume",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceModelWaitEntryV7 {
    EvaluationReserved {
        turn: u32,
        attempt: u32,
        wait: String,
        phase: SourceModelWaitPhaseV7,
        replay_of: Option<u32>,
        fuel: usize,
    },
    Prepared {
        turn: u32,
        attempt: u32,
        wait: String,
        reservation: u32,
        observation_digest: String,
        checkpoint_digest: String,
        checkpoint: Vec<u8>,
    },
    Completed {
        turn: u32,
        attempt: u32,
        wait: String,
        reservation: u32,
        proposal_digest: String,
    },
    ReplayChecked {
        turn: u32,
        attempt: u32,
        wait: String,
        reservation: u32,
        original: u32,
        result_digest: String,
    },
}
impl SourceModelWaitEntryV7 {
    fn identity(&self) -> (u32, u32, &str) {
        match self {
            Self::EvaluationReserved {
                turn,
                attempt,
                wait,
                ..
            }
            | Self::Prepared {
                turn,
                attempt,
                wait,
                ..
            }
            | Self::Completed {
                turn,
                attempt,
                wait,
                ..
            }
            | Self::ReplayChecked {
                turn,
                attempt,
                wait,
                ..
            } => (*turn, *attempt, wait),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceExecutionEntryV7 {
    Ordinary(SourceJournalEntry),
    Wait(SourceModelWaitEntryV7),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceModelWaitStateV7 {
    pub(crate) turn: u32,
    pub(crate) attempt: u32,
    pub(crate) wait: String,
    pub(crate) start_reservation: Option<u32>,
    pub(crate) resume_reservation: Option<u32>,
    pub(crate) prepared: Option<(u32, SourceModelWaitEntryV7)>,
    pub(crate) completed: Option<(u32, SourceModelWaitEntryV7)>,
    pub(crate) reservations: Vec<(u32, SourceModelWaitEntryV7)>,
}

pub(crate) fn source_model_wait_checkpoint_digest(bytes: &[u8]) -> String {
    digest(CHECKPOINT_DOMAIN, bytes)
}
impl SourceInvocationBinding {
    pub(crate) fn with_model_wait_v7(
        mut self,
        profile: SourceModelWaitProfileV7,
    ) -> Result<Self, SourceJournalError> {
        if !matches!(&self.profile, SourceProfile::ExecutionV2 { .. })
            || self.wait.is_some()
            || self.io.is_some()
            || self.program_root.is_some()
            || self.source_revision != profile.source_revision
            || profile.evaluation_fuel
                > self
                    .max_steps_per_stage()
                    .ok_or(SourceJournalError::Binding)?
        {
            return Err(SourceJournalError::Binding);
        }
        let canonical = format!(
            "{{\"execution\":{},\"wait_binding\":{}}}",
            quote_json(self.invocation()),
            quote_json(profile.wrapper_binding())
        );
        self.invocation = digest(ID_DOMAIN_V7, canonical.as_bytes());
        self.wait = Some(profile);
        Ok(self)
    }
    pub(crate) fn model_wait_profile(&self) -> Option<&SourceModelWaitProfileV7> {
        self.wait.as_ref()
    }
    pub(crate) fn model_wait_id(
        &self,
        turn: u32,
        attempt: u32,
    ) -> Result<String, SourceJournalError> {
        let profile = self.wait.as_ref().ok_or(SourceJournalError::Binding)?;
        if turn >= self.max_iterations || attempt >= self.max_attempts {
            return Err(SourceJournalError::Binding);
        }
        let canonical = format!(
            "{{\"invocation\":{},\"turn\":{},\"attempt\":{},\"wrapper_binding\":{}}}",
            quote_json(self.invocation()),
            turn,
            attempt,
            quote_json(profile.wrapper_binding())
        );
        Ok(digest(WAIT_DOMAIN, canonical.as_bytes()))
    }
}
impl SourceJournal {
    pub(crate) fn combined_len(&self) -> usize {
        self.entries.len() + self.wait_entries.len()
    }
    pub(crate) fn execution_entries_v7(&self) -> Vec<(u32, SourceExecutionEntryV7)> {
        let mut ordinary = self.entries.iter();
        let mut waits = self.wait_entries.iter().peekable();
        (0..self.combined_len())
            .map(|seq| {
                let row = if waits
                    .peek()
                    .is_some_and(|(index, _)| *index as usize == seq)
                {
                    SourceExecutionEntryV7::Wait(waits.next().expect("peeked").1.clone())
                } else {
                    SourceExecutionEntryV7::Ordinary(
                        ordinary.next().expect("complete inventory").clone(),
                    )
                };
                (seq as u32, row)
            })
            .collect()
    }
    pub(super) fn execution_fold(&self) -> Result<execution::ExecutionFold, SourceJournalError> {
        if self.binding.wait.is_none() {
            if !self.wait_entries.is_empty() {
                return Err(SourceJournalError::Binding);
            }
            return execution::validate(&self.binding, &self.entries);
        }
        let wait_fold = fold(self)?;
        let combined = self.execution_entries_v7();
        let mut projected = Vec::new();
        let mut sequences = BTreeMap::new();
        for (seq, row) in combined {
            if let SourceExecutionEntryV7::Ordinary(mut entry) = row {
                sequences.insert(seq, projected.len() as u32);
                if let SourceJournalEntry::ReplayStageReservation { causal_seq, .. } = &mut entry {
                    *causal_seq = *sequences.get(causal_seq).ok_or(SourceJournalError::Order)?;
                }
                projected.push(entry);
            }
        }
        execution::validate_with_wait_fuel(&self.binding, &projected, wait_fold.fuel)
    }
    pub(crate) fn wait_state(
        &self,
        turn: u32,
        attempt: u32,
    ) -> Result<Option<SourceModelWaitStateV7>, SourceJournalError> {
        Ok(fold(self)?
            .states
            .remove(&(turn, attempt))
            .map(|state| state.public))
    }
    pub(crate) fn wait_fuel(&self) -> Result<u64, SourceJournalError> {
        Ok(fold(self)?.fuel)
    }
    pub(crate) fn wait_evidence(
        &self,
        terminal_digest: &str,
        ordinary_model_evidence_digest: &str,
    ) -> Result<Vec<u8>, SourceJournalError> {
        if !looks_like_digest(terminal_digest) || !looks_like_digest(ordinary_model_evidence_digest)
        {
            return Err(SourceJournalError::Malformed);
        }
        if !matches!(self.entries.last(), Some(SourceJournalEntry::TerminalSnapshot { evidence_digest, .. }) if evidence_digest == terminal_digest)
        {
            return Err(SourceJournalError::Binding);
        }
        let folded = fold(self)?;
        let mut states: Vec<_> = folded.states.values().collect();
        states.sort_by_key(|state| state.public.start_reservation);
        let mut rows = Vec::new();
        for state in states {
            let prepared = state
                .public
                .prepared
                .as_ref()
                .map(|(_, entry)| match entry {
                    SourceModelWaitEntryV7::Prepared {
                        checkpoint_digest, ..
                    } => quote_json(checkpoint_digest),
                    _ => unreachable!(),
                })
                .unwrap_or_else(|| "null".into());
            let completed = state
                .public
                .completed
                .as_ref()
                .map(|(_, entry)| match entry {
                    SourceModelWaitEntryV7::Completed {
                        proposal_digest, ..
                    } => quote_json(proposal_digest),
                    _ => unreachable!(),
                })
                .unwrap_or_else(|| "null".into());
            let reservations = state
                .public
                .reservations
                .iter()
                .map(|(seq, _)| seq.to_string())
                .collect::<Vec<_>>()
                .join(",");
            rows.push(format!("{{\"wait\":{},\"turn\":{},\"attempt\":{},\"prepared\":{},\"completed\":{},\"reservations\":[{}]}}",quote_json(&state.public.wait),state.public.turn,state.public.attempt,prepared,completed,reservations));
        }
        let profile = self
            .binding
            .wait
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        Ok(format!("{{\"schema\":\"semaprax.source-model-wait.evidence.v1\",\"terminal_evidence_digest\":{},\"ordinary_model_evidence_digest\":{},\"wrapper_binding\":{},\"invocation\":{},\"waits\":[{}],\"total_wait_fuel\":{}}}",quote_json(terminal_digest),quote_json(ordinary_model_evidence_digest),quote_json(profile.wrapper_binding()),quote_json(self.binding.invocation()),rows.join(","),folded.fuel).into_bytes())
    }
    pub(crate) fn wait_evidence_digest(
        &self,
        terminal_digest: &str,
        ordinary_model_evidence_digest: &str,
    ) -> Result<String, SourceJournalError> {
        Ok(digest(
            EVIDENCE_DOMAIN,
            &self.wait_evidence(terminal_digest, ordinary_model_evidence_digest)?,
        ))
    }
}

#[derive(Clone)]
struct State {
    public: SourceModelWaitStateV7,
    intent: bool,
    settled: bool,
    closed: bool,
    checks: BTreeSet<u32>,
    reservation_index: BTreeMap<u32, (SourceModelWaitPhaseV7, Option<u32>)>,
}
impl State {
    fn ready(&self, phase: SourceModelWaitPhaseV7) -> bool {
        let closure = match phase {
            SourceModelWaitPhaseV7::Start => &self.public.prepared,
            SourceModelWaitPhaseV7::Resume => &self.public.completed,
        };
        if closure.is_none() {
            return false;
        }
        let latest=self.public.reservations.iter().rev().find(|(_,entry)|matches!(entry,SourceModelWaitEntryV7::EvaluationReserved{phase:p,..} if *p==phase));
        latest.is_some_and(|(seq, entry)| match entry {
            SourceModelWaitEntryV7::EvaluationReserved {
                replay_of: None, ..
            } => true,
            SourceModelWaitEntryV7::EvaluationReserved {
                replay_of: Some(_), ..
            } => self.checks.contains(seq),
            _ => false,
        })
    }
}
struct WaitFold {
    states: BTreeMap<(u32, u32), State>,
    fuel: u64,
}
fn fold(journal: &SourceJournal) -> Result<WaitFold, SourceJournalError> {
    let profile = journal
        .binding
        .wait
        .as_ref()
        .ok_or(SourceJournalError::Binding)?;
    if journal.binding.io.is_some()
        || journal.binding.program_root.is_some()
        || !matches!(journal.binding.profile, SourceProfile::ExecutionV2 { .. })
    {
        return Err(SourceJournalError::Binding);
    }
    let mut result = WaitFold {
        states: BTreeMap::new(),
        fuel: 0,
    };
    let mut current: Option<(u32, u32)> = None;
    let mut forbidden = false;
    for (seq, row) in journal.execution_entries_v7() {
        match row {
            SourceExecutionEntryV7::Ordinary(entry) => match entry {
                SourceJournalEntry::TurnObserved { turn, .. } => {
                    current = Some((turn, 0));
                    forbidden = false;
                }
                SourceJournalEntry::AttemptIntent { turn, attempt, .. } => {
                    let state = result
                        .states
                        .get_mut(&(turn, attempt))
                        .ok_or(SourceJournalError::Order)?;
                    if current != Some((turn, attempt))
                        || state.intent
                        || state.closed
                        || !state.ready(SourceModelWaitPhaseV7::Start)
                    {
                        return Err(SourceJournalError::Order);
                    }
                    state.intent = true;
                    forbidden = true;
                }
                SourceJournalEntry::AttemptSettled { turn, attempt, .. } => {
                    let state = result
                        .states
                        .get_mut(&(turn, attempt))
                        .ok_or(SourceJournalError::Order)?;
                    if !state.intent || state.settled || state.closed {
                        return Err(SourceJournalError::Order);
                    }
                    state.settled = true;
                    forbidden = false;
                }
                SourceJournalEntry::AttemptFailed { turn, attempt, .. } => {
                    let state = result
                        .states
                        .get_mut(&(turn, attempt))
                        .ok_or(SourceJournalError::Order)?;
                    if !state.intent || state.settled || state.closed {
                        return Err(SourceJournalError::Order);
                    }
                    state.closed = true;
                    forbidden = false;
                }
                SourceJournalEntry::AttemptUsage { turn, attempt, .. } => {
                    let state = result
                        .states
                        .get(&(turn, attempt))
                        .ok_or(SourceJournalError::Order)?;
                    if !state.intent || state.public.resume_reservation.is_some() {
                        return Err(SourceJournalError::Order);
                    }
                }
                SourceJournalEntry::ProposalRefused { turn, attempt, .. } => {
                    let state = result
                        .states
                        .get_mut(&(turn, attempt))
                        .ok_or(SourceJournalError::Order)?;
                    if !state.settled || state.closed || state.public.resume_reservation.is_some() {
                        return Err(SourceJournalError::Order);
                    }
                    state.closed = true;
                    current = Some((
                        turn,
                        attempt.checked_add(1).ok_or(SourceJournalError::Capacity)?,
                    ));
                }
                SourceJournalEntry::ProposalAdmitted { turn, attempt, .. } => {
                    let state = result
                        .states
                        .get_mut(&(turn, attempt))
                        .ok_or(SourceJournalError::Order)?;
                    if state.closed
                        || !state.settled
                        || !state.ready(SourceModelWaitPhaseV7::Start)
                        || !state.ready(SourceModelWaitPhaseV7::Resume)
                    {
                        return Err(SourceJournalError::Order);
                    }
                    state.closed = true;
                    current = None;
                }
                SourceJournalEntry::Stop { .. } | SourceJournalEntry::TerminalSnapshot { .. } => {
                    forbidden = true;
                    current = None;
                }
                SourceJournalEntry::PricedAttemptIntent(_)
                | SourceJournalEntry::PricedAttemptUsage(_)
                | SourceJournalEntry::PolicyAttemptIntent(_)
                | SourceJournalEntry::PolicyAttemptUsage { .. } => {
                    return Err(SourceJournalError::Binding)
                }
                _ => {}
            },
            SourceExecutionEntryV7::Wait(entry) => {
                if forbidden {
                    return Err(SourceJournalError::Order);
                }
                let (turn, attempt, wait) = entry.identity();
                if journal.binding.model_wait_id(turn, attempt)? != wait {
                    return Err(SourceJournalError::Binding);
                }
                let key = (turn, attempt);
                if let SourceModelWaitEntryV7::EvaluationReserved {
                    phase: SourceModelWaitPhaseV7::Start,
                    replay_of: None,
                    ..
                } = &entry
                {
                    if current != Some(key) || result.states.contains_key(&key) {
                        return Err(SourceJournalError::Order);
                    }
                    result.states.insert(
                        key,
                        State {
                            public: SourceModelWaitStateV7 {
                                turn,
                                attempt,
                                wait: wait.to_owned(),
                                start_reservation: None,
                                resume_reservation: None,
                                prepared: None,
                                completed: None,
                                reservations: Vec::new(),
                            },
                            intent: false,
                            settled: false,
                            closed: false,
                            checks: BTreeSet::new(),
                            reservation_index: BTreeMap::new(),
                        },
                    );
                }
                let state = result
                    .states
                    .get_mut(&key)
                    .ok_or(SourceJournalError::Order)?;
                match &entry {
                    SourceModelWaitEntryV7::EvaluationReserved {
                        phase,
                        replay_of,
                        fuel,
                        ..
                    } => {
                        if *fuel != profile.evaluation_fuel || *fuel == 0 {
                            return Err(SourceJournalError::Binding);
                        }
                        let start_ready = state.ready(SourceModelWaitPhaseV7::Start);
                        let original = match phase {
                            SourceModelWaitPhaseV7::Start => &mut state.public.start_reservation,
                            SourceModelWaitPhaseV7::Resume => &mut state.public.resume_reservation,
                        };
                        if let Some(prior) = replay_of {
                            if Some(*prior) != *original || *prior >= seq {
                                return Err(SourceJournalError::Order);
                            }
                        } else {
                            if original.is_some()
                                || state.closed
                                || (*phase == SourceModelWaitPhaseV7::Resume
                                    && (!state.settled || !start_ready))
                            {
                                return Err(SourceJournalError::Order);
                            }
                            *original = Some(seq);
                        }
                        result.fuel = result
                            .fuel
                            .checked_add(*fuel as u64)
                            .ok_or(SourceJournalError::Capacity)?;
                        state.public.reservations.push((seq, entry.clone()));
                        state.reservation_index.insert(seq, (*phase, *replay_of));
                    }
                    SourceModelWaitEntryV7::Prepared {
                        reservation,
                        checkpoint_digest,
                        checkpoint,
                        observation_digest,
                        ..
                    } => {
                        if state.closed
                            || state.intent
                            || state.public.prepared.is_some()
                            || state.public.start_reservation != Some(*reservation)
                            || *reservation >= seq
                            || checkpoint.is_empty()
                            || checkpoint.len() > SOURCE_MODEL_WAIT_CHECKPOINT_LIMIT
                            || !looks_like_digest(observation_digest)
                            || source_model_wait_checkpoint_digest(checkpoint) != *checkpoint_digest
                        {
                            return Err(SourceJournalError::Order);
                        }
                        state.public.prepared = Some((seq, entry.clone()));
                    }
                    SourceModelWaitEntryV7::Completed {
                        reservation,
                        proposal_digest,
                        ..
                    } => {
                        if state.closed
                            || !state.settled
                            || state.public.completed.is_some()
                            || state.public.resume_reservation != Some(*reservation)
                            || *reservation >= seq
                            || !looks_like_digest(proposal_digest)
                        {
                            return Err(SourceJournalError::Order);
                        }
                        state.public.completed = Some((seq, entry.clone()));
                    }
                    SourceModelWaitEntryV7::ReplayChecked {
                        reservation,
                        original,
                        result_digest,
                        ..
                    } => {
                        let Some((phase, Some(_))) = state.reservation_index.get(reservation)
                        else {
                            return Err(SourceJournalError::Order);
                        };
                        let closure = match phase {
                            SourceModelWaitPhaseV7::Start => state.public.prepared.as_ref(),
                            SourceModelWaitPhaseV7::Resume => state.public.completed.as_ref(),
                        }
                        .ok_or(SourceJournalError::Order)?;
                        let expected = match &closure.1 {
                            SourceModelWaitEntryV7::Prepared {
                                checkpoint_digest, ..
                            } => checkpoint_digest,
                            SourceModelWaitEntryV7::Completed {
                                proposal_digest, ..
                            } => proposal_digest,
                            _ => unreachable!(),
                        };
                        if closure.0 != *original
                            || *reservation >= seq
                            || *original >= seq
                            || result_digest != expected
                            || state.checks.contains(reservation)
                        {
                            return Err(SourceJournalError::Order);
                        }
                        state.checks.insert(*reservation);
                    }
                }
            }
        }
    }
    Ok(result)
}

mod capacity;
pub(super) fn check_capacity(
    journal: &SourceJournal,
    document_bytes: usize,
) -> Result<(), SourceJournalError> {
    capacity::check(journal, document_bytes)
}

impl SourceCheckpointSink<'_> {
    pub(crate) fn preflight_wait_at(
        &self,
        entry: &SourceModelWaitEntryV7,
        now: i64,
    ) -> Result<(), SourceJournalError> {
        self.prepare_wait_append(entry.clone(), now).map(|_| ())
    }
    pub(crate) fn append_wait_at(
        &mut self,
        entry: SourceModelWaitEntryV7,
        now: i64,
    ) -> Result<(), SourceJournalError> {
        let (next, generation, document) = self.prepare_wait_append(entry, now)?;
        if let Err(error) = self.store.commit(generation, &document) {
            self.poisoned = true;
            return Err(SourceJournalError::Store(error));
        }
        self.journal = next;
        self.generation = generation;
        Ok(())
    }
    fn prepare_wait_append(
        &self,
        entry: SourceModelWaitEntryV7,
        now: i64,
    ) -> Result<(SourceJournal, u64, String), SourceJournalError> {
        if self.poisoned {
            return Err(SourceJournalError::Poisoned);
        }
        if self.journal.binding.wait.is_none() {
            return Err(SourceJournalError::Binding);
        }
        if now < self.journal.last_checked_millis {
            return Err(SourceJournalError::Time);
        }
        if self.journal.combined_len() >= MAX_SOURCE_ENTRIES {
            return Err(SourceJournalError::Capacity);
        }
        let mut next = self.journal.clone();
        next.wait_entries.push((next.combined_len() as u32, entry));
        next.last_checked_millis = now;
        next.execution_fold()?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(SourceJournalError::Generation)?;
        let document = super::wire::encode_envelope(&next, generation)?;
        check_capacity(&next, document.len())?;
        Ok((next, generation, document))
    }
}

impl RecoveredSourceCheckpoint {
    pub(crate) fn execution_entries_v7(&self) -> Vec<(u32, SourceExecutionEntryV7)> {
        self.journal.execution_entries_v7()
    }
    pub(crate) fn wait_state(
        &self,
        turn: u32,
        attempt: u32,
    ) -> Result<Option<SourceModelWaitStateV7>, SourceJournalError> {
        self.journal.wait_state(turn, attempt)
    }
    pub(crate) fn wait_fuel(&self) -> Result<u64, SourceJournalError> {
        self.journal.wait_fuel()
    }
    pub(crate) fn wait_evidence(
        &self,
        terminal_digest: &str,
        ordinary_model_evidence_digest: &str,
    ) -> Result<Vec<u8>, SourceJournalError> {
        self.journal
            .wait_evidence(terminal_digest, ordinary_model_evidence_digest)
    }
    pub(crate) fn wait_evidence_digest(
        &self,
        terminal_digest: &str,
        ordinary_model_evidence_digest: &str,
    ) -> Result<String, SourceJournalError> {
        self.journal
            .wait_evidence_digest(terminal_digest, ordinary_model_evidence_digest)
    }
}
