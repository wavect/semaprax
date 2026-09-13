//! Bounded source-mode causal checkpoint. This wire is distinct from the
//! generic one-request-per-turn live journal; it does not dispatch anything.
//! The caller must load the latest authoritative generation under exclusive
//! writer control. A valid older same-binding document can be replayed by a
//! party controlling storage: the chain and opaque recovery type check
//! integrity and causal shape, not freshness or authentication.
//! Provider usage counters and host receipts are not stored as charge proof.

use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::diagnostic::quote_json;

use super::identity::{digest, hex, looks_like_digest};

mod validate;
mod wire;

#[cfg(test)]
mod tests;

pub const SOURCE_JOURNAL_SCHEMA: &str = "semaprax.live-invocation.source-persisted-journal.v1";
pub const MAX_SOURCE_ENTRIES: usize = 65_536;
pub const MAX_SOURCE_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SOURCE_RESPONSE_BYTES: usize = 65_536;
pub const MAX_SOURCE_REQUEST_BYTES: usize = 65_536;
pub const MAX_SOURCE_EFFECT_BYTES: usize = 65_536;
pub const MAX_SOURCE_ATTEMPTS: u32 = 4;

const ID_DOMAIN: &[u8] = b"semaprax.live-invocation.source-id.v1\0";
const ATTEMPT_DOMAIN: &[u8] = b"semaprax.live-invocation.source-attempt.v1\0";
const RESPONSE_DOMAIN: &[u8] = b"semaprax.live-invocation.source-response.v1\0";
const EFFECT_DOMAIN: &[u8] = b"semaprax.live-invocation.source-effect.v1\0";
const PROMPT_DOMAIN: &[u8] = b"semaprax.live-invocation.source-prompt.v1\0";
const CONTEXT_DOMAIN: &[u8] = b"semaprax.live-invocation.source-context-binding.v1\0";

/// All caller-selected policy, program and task inputs to one source run.
/// The resulting binding is opaque; a recovery caller must supply it again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInvocationSeed {
    pub lifecycle_digest: String,
    pub source_revision: String,
    pub deployment_binding: String,
    pub task: Vec<u8>,
    pub task_budget: i64,
    pub proposal_schema_digest: String,
    pub response_limit: usize,
    pub max_iterations: u32,
    pub max_stages: u32,
    pub max_attempts: u32,
    pub max_steps_per_stage: usize,
    pub max_total_steps: usize,
    pub ceiling: i64,
    pub reservation_units: i64,
    pub unit: String,
    pub clock_domain: String,
    pub initial_millis: i64,
    pub deadline_millis: i64,
    pub program_root: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInvocationBinding {
    invocation: String,
    proposal_source: String,
    response_limit: usize,
    max_iterations: u32,
    max_stages: u32,
    max_attempts: u32,
    ceiling: i64,
    reservation_units: i64,
    unit: String,
    clock_domain: String,
    initial_millis: i64,
    deadline_millis: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceJournalError {
    Binding,
    Malformed,
    Capacity,
    Time,
    Order,
    Chain,
    Generation,
    Store(CheckpointStoreError),
    Poisoned,
    Uncertain,
}

impl SourceInvocationBinding {
    pub fn bind(seed: SourceInvocationSeed) -> Result<Self, SourceJournalError> {
        let digests = [
            &seed.lifecycle_digest,
            &seed.source_revision,
            &seed.deployment_binding,
            &seed.proposal_schema_digest,
        ];
        if !digests.iter().all(|value| looks_like_digest(value))
            || seed.task.len() > 65_536
            || seed.response_limit == 0
            || seed.response_limit > MAX_SOURCE_RESPONSE_BYTES
            || !(1..=4_096).contains(&seed.max_iterations)
            || !(1..=12_289).contains(&seed.max_stages)
            || !(1..=MAX_SOURCE_ATTEMPTS).contains(&seed.max_attempts)
            || seed.max_steps_per_stage == 0
            || seed.max_steps_per_stage > 1_000_000
            || seed.max_total_steps == 0
            || seed.max_total_steps > 1_000_000_000
            || seed.ceiling < 0
            || seed.reservation_units <= 0
            || !valid_token(&seed.unit, 64)
            || !valid_token(&seed.clock_domain, 128)
            || seed.initial_millis < 0
            || seed.deadline_millis <= seed.initial_millis
            || seed
                .program_root
                .as_ref()
                .is_some_and(|root| root.len() > 4_096 || root.contains('\0'))
        {
            return Err(SourceJournalError::Binding);
        }
        let root = seed
            .program_root
            .as_deref()
            .map(quote_json)
            .unwrap_or_else(|| "null".to_owned());
        let canonical = format!(
            "{{\"schema\":\"semaprax.live-invocation.source-binding.v1\",\"lifecycle\":{},\"source_revision\":{},\"deployment\":{},\"task\":{},\"task_budget\":{},\"proposal_schema\":{},\"response_limit\":{},\"max_iterations\":{},\"max_stages\":{},\"max_attempts\":{},\"max_steps_per_stage\":{},\"max_total_steps\":{},\"ceiling\":{},\"reservation_units\":{},\"unit\":{},\"clock_domain\":{},\"initial_millis\":{},\"deadline_millis\":{},\"program_root\":{}}}",
            quote_json(&seed.lifecycle_digest),
            quote_json(&seed.source_revision),
            quote_json(&seed.deployment_binding),
            quote_json(&hex(&seed.task)),
            seed.task_budget,
            quote_json(&seed.proposal_schema_digest),
            seed.response_limit,
            seed.max_iterations,
            seed.max_stages,
            seed.max_attempts,
            seed.max_steps_per_stage,
            seed.max_total_steps,
            seed.ceiling,
            seed.reservation_units,
            quote_json(&seed.unit),
            quote_json(&seed.clock_domain),
            seed.initial_millis,
            seed.deadline_millis,
            root,
        );
        Ok(Self {
            invocation: digest(ID_DOMAIN, canonical.as_bytes()),
            proposal_source: proposal_source_digest(
                &seed.source_revision,
                &seed.deployment_binding,
                &seed.task,
                seed.task_budget,
                &seed.proposal_schema_digest,
            ),
            response_limit: seed.response_limit,
            max_iterations: seed.max_iterations,
            max_stages: seed.max_stages,
            max_attempts: seed.max_attempts,
            ceiling: seed.ceiling,
            reservation_units: seed.reservation_units,
            unit: seed.unit,
            clock_domain: seed.clock_domain,
            initial_millis: seed.initial_millis,
            deadline_millis: seed.deadline_millis,
        })
    }

    pub fn invocation(&self) -> &str {
        &self.invocation
    }
    pub const fn response_limit(&self) -> usize {
        self.response_limit
    }
    pub const fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
    pub const fn ceiling(&self) -> i64 {
        self.ceiling
    }
    pub const fn reservation_units(&self) -> i64 {
        self.reservation_units
    }
    pub fn unit(&self) -> &str {
        &self.unit
    }
    pub fn clock_domain(&self) -> &str {
        &self.clock_domain
    }
    pub const fn initial_millis(&self) -> i64 {
        self.initial_millis
    }
    pub const fn deadline_millis(&self) -> i64 {
        self.deadline_millis
    }

    /// Checks the bind-time inputs available to a host proposal adapter.
    /// The checked lifecycle driver still owns lifecycle, stage-limit and
    /// ProgramRoot binding; this does not authenticate arbitrary source text.
    pub fn matches_proposal_source(
        &self,
        source_revision: &str,
        deployment_binding: &str,
        task: &[u8],
        task_budget: i64,
        proposal_schema_digest: &str,
    ) -> bool {
        task.len() <= MAX_SOURCE_REQUEST_BYTES
            && [source_revision, deployment_binding, proposal_schema_digest]
                .into_iter()
                .all(looks_like_digest)
            && self.proposal_source
                == proposal_source_digest(
                    source_revision,
                    deployment_binding,
                    task,
                    task_budget,
                    proposal_schema_digest,
                )
    }

    /// Binds a particular retry to the exact model request and prompt.
    pub fn attempt_digest(
        &self,
        turn: u32,
        attempt: u32,
        request_digest: &str,
        prompt_digest: &str,
        request_bytes: usize,
    ) -> String {
        let canonical = format!(
            "{{\"invocation\":{},\"turn\":{},\"attempt\":{},\"request\":{},\"prompt\":{},\"request_bytes\":{},\"reserved_units\":{},\"response_limit\":{},\"deadline_millis\":{}}}",
            quote_json(&self.invocation), turn, attempt, quote_json(request_digest),
            quote_json(prompt_digest), request_bytes, self.reservation_units, self.response_limit,
            self.deadline_millis,
        );
        digest(ATTEMPT_DOMAIN, canonical.as_bytes())
    }
}

fn proposal_source_digest(
    source_revision: &str,
    deployment_binding: &str,
    task: &[u8],
    task_budget: i64,
    proposal_schema_digest: &str,
) -> String {
    let canonical = format!(
        "[{}, {}, {}, {}, {}]",
        quote_json(source_revision),
        quote_json(deployment_binding),
        quote_json(&hex(task)),
        task_budget,
        quote_json(proposal_schema_digest),
    );
    digest(CONTEXT_DOMAIN, canonical.as_bytes())
}

fn valid_token(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
}

macro_rules! closed_tags {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }
            fn parse(value: &str) -> Option<Self> {
                match value { $($wire => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

closed_tags!(SourceAttemptFailure {
    Timeout => "timeout", Cancelled => "cancelled", CapacityExceeded => "capacity_exceeded",
    ProviderError => "provider_error", MalformedResponse => "malformed_response",
    Refused => "refused", DeadlineExceeded => "deadline_exceeded"
});
closed_tags!(SourceProposalRefusal {
    MalformedDecode => "malformed_decode", InvalidUtf8 => "invalid_utf8",
    DeadlineExceeded => "deadline_exceeded", ProjectionFailed => "projection_failed"
});
closed_tags!(SourceAuthorizationRefusal {
    GateDenied => "gate_denied", Undecided => "undecided",
    Cancelled => "cancelled", DeadlineExceeded => "deadline_exceeded"
});
closed_tags!(SourceEffectFailure {
    HandlerFailed => "handler_failed", ResultLimit => "result_limit",
    Cancelled => "cancelled", DeadlineExceeded => "deadline_exceeded"
});
closed_tags!(SourceTransitionCase {
    Continue => "continue", Complete => "complete", Suspend => "suspend", Fail => "fail"
});
closed_tags!(SourceStopStatus {
    Rejected => "rejected", ModelFailed => "model_failed", EffectFailed => "effect_failed",
    Cancelled => "cancelled", BudgetExhausted => "budget_exhausted",
    DeadlineExceeded => "deadline_exceeded"
});
closed_tags!(SourceStopReason {
    StageRefused => "stage_refused", ModelFailed => "model_failed",
    EffectFailed => "effect_failed", Cancelled => "cancelled",
    BudgetExhausted => "budget_exhausted", DeadlineExceeded => "deadline_exceeded"
});
closed_tags!(SourceTerminalStatus {
    Complete => "complete", Suspend => "suspend", Fail => "fail",
    Rejected => "rejected", ModelFailed => "model_failed", EffectFailed => "effect_failed",
    Cancelled => "cancelled", BudgetExhausted => "budget_exhausted",
    DeadlineExceeded => "deadline_exceeded"
});

impl SourceStopReason {
    fn status(self) -> SourceStopStatus {
        match self {
            Self::StageRefused => SourceStopStatus::Rejected,
            Self::ModelFailed => SourceStopStatus::ModelFailed,
            Self::EffectFailed => SourceStopStatus::EffectFailed,
            Self::Cancelled => SourceStopStatus::Cancelled,
            Self::BudgetExhausted => SourceStopStatus::BudgetExhausted,
            Self::DeadlineExceeded => SourceStopStatus::DeadlineExceeded,
        }
    }
}
impl From<SourceStopStatus> for SourceTerminalStatus {
    fn from(value: SourceStopStatus) -> Self {
        match value {
            SourceStopStatus::Rejected => Self::Rejected,
            SourceStopStatus::ModelFailed => Self::ModelFailed,
            SourceStopStatus::EffectFailed => Self::EffectFailed,
            SourceStopStatus::Cancelled => Self::Cancelled,
            SourceStopStatus::BudgetExhausted => Self::BudgetExhausted,
            SourceStopStatus::DeadlineExceeded => Self::DeadlineExceeded,
        }
    }
}

/// Typed source phases. Response and effect bytes are retained only where
/// replay needs them; error tags cannot carry provider text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceJournalEntry {
    RunOpened,
    TurnObserved {
        turn: u32,
        state: String,
        observation: String,
        feedback: String,
    },
    AttemptIntent {
        turn: u32,
        attempt: u32,
        attempt_digest: String,
        request_digest: String,
        prompt_digest: String,
        request_bytes: usize,
        reserved_units: i64,
        response_limit: usize,
    },
    AttemptSettled {
        turn: u32,
        attempt: u32,
        response: Vec<u8>,
        response_digest: String,
    },
    AttemptFailed {
        turn: u32,
        attempt: u32,
        reason: SourceAttemptFailure,
        attempted_bytes: usize,
    },
    ProposalRefused {
        turn: u32,
        attempt: u32,
        reason: SourceProposalRefusal,
    },
    ProposalAdmitted {
        turn: u32,
        attempt: u32,
        proposal_digest: String,
    },
    AuthorizationConsumed {
        turn: u32,
        attempt: u32,
        grant_digest: String,
    },
    AuthorizationRefused {
        turn: u32,
        attempt: u32,
        reason: SourceAuthorizationRefusal,
    },
    EffectIntent {
        turn: u32,
        attempt: u32,
        operation: String,
        request_digest: String,
    },
    EffectObserved {
        turn: u32,
        attempt: u32,
        operation: String,
        observation: Vec<u8>,
        observation_digest: String,
    },
    EffectFailed {
        turn: u32,
        attempt: u32,
        operation: String,
        reason: SourceEffectFailure,
    },
    Transition {
        turn: u32,
        attempt: u32,
        case: SourceTransitionCase,
        carrier_digest: String,
    },
    Stop {
        turn: Option<u32>,
        attempt: Option<u32>,
        status: SourceStopStatus,
        reason: SourceStopReason,
    },
    TerminalOutcome {
        turn: Option<u32>,
        status: SourceTerminalStatus,
        carrier_digest: Option<String>,
    },
}

pub fn source_response_digest(bytes: &[u8]) -> String {
    digest(RESPONSE_DOMAIN, bytes)
}
pub fn source_effect_digest(bytes: &[u8]) -> String {
    digest(EFFECT_DOMAIN, bytes)
}
pub fn source_prompt_digest(bytes: &[u8]) -> String {
    digest(PROMPT_DOMAIN, bytes)
}

#[derive(Clone, Debug)]
pub struct SourceJournal {
    binding: SourceInvocationBinding,
    entries: Vec<SourceJournalEntry>,
    last_checked_millis: i64,
}

impl SourceJournal {
    pub fn new(binding: SourceInvocationBinding) -> Self {
        let last_checked_millis = binding.initial_millis;
        Self {
            binding,
            entries: Vec::new(),
            last_checked_millis,
        }
    }
    pub fn binding(&self) -> &SourceInvocationBinding {
        &self.binding
    }
    pub fn entries(&self) -> &[SourceJournalEntry] {
        &self.entries
    }
    pub const fn last_checked_millis(&self) -> i64 {
        self.last_checked_millis
    }

    fn candidate(&self, entry: SourceJournalEntry, now: i64) -> Result<Self, SourceJournalError> {
        if now < self.last_checked_millis {
            return Err(SourceJournalError::Time);
        }
        if self.entries.len() >= MAX_SOURCE_ENTRIES {
            return Err(SourceJournalError::Capacity);
        }
        let mut next = self.clone();
        next.entries.push(entry);
        next.last_checked_millis = now;
        validate::validate(&next.binding, &next.entries)?;
        Ok(next)
    }
}

/// Owns the sole source journal write cursor. Any failed store acknowledgement
/// poisons the cursor, even if the store may actually have committed.
pub struct SourceCheckpointSink<'a> {
    store: &'a mut dyn CheckpointStore,
    journal: SourceJournal,
    generation: u64,
    poisoned: bool,
}

impl<'a> SourceCheckpointSink<'a> {
    pub fn new(store: &'a mut dyn CheckpointStore, binding: SourceInvocationBinding) -> Self {
        Self {
            store,
            journal: SourceJournal::new(binding),
            generation: 0,
            poisoned: false,
        }
    }
    pub fn resume(
        store: &'a mut dyn CheckpointStore,
        recovered: RecoveredSourceCheckpoint,
    ) -> Result<Self, SourceJournalError> {
        if recovered.is_uncertain() {
            return Err(SourceJournalError::Uncertain);
        }
        Ok(Self {
            store,
            journal: recovered.journal,
            generation: recovered.generation,
            poisoned: false,
        })
    }
    pub fn append_at(
        &mut self,
        entry: SourceJournalEntry,
        now: i64,
    ) -> Result<(), SourceJournalError> {
        let (next, generation, document) = self.prepare_append(entry, now)?;
        if let Err(error) = self.store.commit(generation, &document) {
            self.poisoned = true;
            return Err(SourceJournalError::Store(error));
        }
        self.journal = next;
        self.generation = generation;
        Ok(())
    }

    /// Checks phase, clock and bounded settlement capacity before a caller
    /// reserves budget. No store write or dispatch permission is produced;
    /// `append_at` revalidates and must acknowledge the actual intent first.
    pub fn preflight_at(
        &self,
        entry: &SourceJournalEntry,
        now: i64,
    ) -> Result<(), SourceJournalError> {
        self.prepare_append(entry.clone(), now).map(|_| ())
    }

    fn prepare_append(
        &self,
        entry: SourceJournalEntry,
        now: i64,
    ) -> Result<(SourceJournal, u64, String), SourceJournalError> {
        if self.poisoned {
            return Err(SourceJournalError::Poisoned);
        }
        let next = self.journal.candidate(entry, now)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(SourceJournalError::Generation)?;
        let document = wire::encode_envelope(&next, generation)?;
        // An intent is unusable if its worst-case bounded result cannot be
        // checkpointed. Reserve room before granting a physical dispatch.
        let (future_bytes, future_entries) = match next.entries.last() {
            Some(SourceJournalEntry::AttemptIntent { response_limit, .. }) => {
                (response_limit.saturating_mul(2).saturating_add(4_096), 5)
            }
            Some(SourceJournalEntry::EffectIntent { .. }) => (
                MAX_SOURCE_EFFECT_BYTES
                    .saturating_mul(2)
                    .saturating_add(4_096),
                3,
            ),
            _ => (0, 0),
        };
        if document
            .len()
            .checked_add(future_bytes)
            .is_none_or(|size| size > MAX_SOURCE_DOCUMENT_BYTES)
            || next
                .entries
                .len()
                .checked_add(future_entries)
                .is_none_or(|count| count > MAX_SOURCE_ENTRIES)
        {
            return Err(SourceJournalError::Capacity);
        }
        Ok((next, generation, document))
    }
    pub fn journal(&self) -> &SourceJournal {
        &self.journal
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub const fn poisoned(&self) -> bool {
        self.poisoned
    }
}

/// Validated, identity-bound source checkpoint. Its private fields prevent a
/// caller from inventing committed charges for `CumulativeBudgetLedger`.
#[derive(Clone, Debug)]
pub struct RecoveredSourceCheckpoint {
    journal: SourceJournal,
    generation: u64,
    chain: String,
    committed_reserved_units: i64,
}

impl RecoveredSourceCheckpoint {
    pub fn entries(&self) -> &[SourceJournalEntry] {
        self.journal.entries()
    }
    pub fn invocation(&self) -> &str {
        self.journal.binding.invocation()
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub fn chain(&self) -> &str {
        &self.chain
    }
    pub const fn ceiling(&self) -> i64 {
        self.journal.binding.ceiling
    }
    pub const fn reservation_units(&self) -> i64 {
        self.journal.binding.reservation_units
    }
    pub fn unit(&self) -> &str {
        self.journal.binding.unit()
    }
    pub fn clock_domain(&self) -> &str {
        self.journal.binding.clock_domain()
    }
    pub const fn deadline_millis(&self) -> i64 {
        self.journal.binding.deadline_millis
    }
    pub const fn last_checked_millis(&self) -> i64 {
        self.journal.last_checked_millis
    }
    pub const fn committed_reserved_units(&self) -> i64 {
        self.committed_reserved_units
    }
    pub fn is_uncertain(&self) -> bool {
        matches!(
            self.journal.entries.last(),
            Some(
                SourceJournalEntry::AttemptIntent { .. } | SourceJournalEntry::EffectIntent { .. }
            )
        )
    }
}

pub fn recover_source_checkpoint(
    document: &str,
    expected: &SourceInvocationBinding,
) -> Result<RecoveredSourceCheckpoint, SourceJournalError> {
    let (journal, generation, chain) = wire::decode_envelope(document, expected)?;
    let committed_reserved_units = validate::validate(expected, journal.entries())?;
    Ok(RecoveredSourceCheckpoint {
        journal,
        generation,
        chain,
        committed_reserved_units,
    })
}
