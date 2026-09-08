//! Draft authority-free per-operation journal. Not registered until live hooks pass.
use crate::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use crate::diagnostic::Diagnostic;
use crate::interpreter::retained_call::RetainedValue;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
mod codec;
#[cfg(test)]
mod tests;
mod value;

pub(crate) fn encode_retained_value(value: &RetainedValue) -> Result<Value, Diagnostic> {
    value::encode(value)
}

pub(crate) fn decode_retained_value(value: &Value) -> Result<RetainedValue, Diagnostic> {
    value::decode(value)
}

pub const CHECKPOINT_SCHEMA: &str = "semaprax.agent-operation-checkpoint.v2";
const MAX_BYTES: usize = 2_097_152;
const MAX_ENTRIES: usize = 4096;
fn rejected(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G584",
        format!("Agent operation checkpoint invariant failed: {field}"),
    )
}
fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.agent-operation-checkpoint.v2\0");
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
fn hash_valid(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointIdentity {
    pub execution_revision: String,
    pub invocation: String,
    pub registry: String,
    pub program_root: String,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct CheckpointUsage {
    pub calls: u64,
    pub argument_bytes: u64,
    pub result_bytes: u64,
    pub reserved_fuel: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointLimits {
    pub calls: u64,
    pub argument_bytes: u64,
    pub result_bytes: u64,
    pub total_bytes: u64,
    pub reserved_fuel: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectContext {
    pub turn: u64,
    pub operation: String,
    pub effect: String,
    pub authorization_binding: String,
    pub state: RetainedValue,
    pub proposal: String,
    pub arguments: Vec<(String, RetainedValue)>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalEvent {
    /// Reserved before a retained stage, including every replayed stage.
    StageReservation {
        turn: u64,
        stage: String,
        fuel: u64,
    },
    Intent(EffectContext),
    Observed {
        context: EffectContext,
        result: Vec<(String, RetainedValue)>,
        failure: Option<String>,
    },
    Transition {
        context: EffectContext,
        transition: String,
        value: RetainedValue,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct JournalEntry {
    generation: u64,
    prior_digest: String,
    usage: CheckpointUsage,
    event: JournalEvent,
    digest: String,
}

/// Parsed bytes supply no dispatch authority. The caller must independently
/// bind identity and replay the retained producer before interpreting a row.
pub struct OperationCheckpoint {
    identity: CheckpointIdentity,
    limits: CheckpointLimits,
    entries: Vec<JournalEntry>,
    poisoned: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDisposition {
    Fresh,
    ReplayObserved,
    ContinueAfterTransition,
    Terminal,
    UncertainIntent,
}
impl OperationCheckpoint {
    pub fn new(identity: CheckpointIdentity, limits: CheckpointLimits) -> Result<Self, Diagnostic> {
        for value in [
            &identity.execution_revision,
            &identity.invocation,
            &identity.registry,
            &identity.program_root,
        ] {
            if !hash_valid(value) {
                return Err(rejected("identity"));
            }
        }
        Ok(Self {
            identity,
            limits,
            entries: Vec::new(),
            poisoned: false,
        })
    }
    pub fn identity(&self) -> &CheckpointIdentity {
        &self.identity
    }
    pub fn limits(&self) -> CheckpointLimits {
        self.limits
    }
    pub fn usage(&self) -> CheckpointUsage {
        self.entries.last().map(|e| e.usage).unwrap_or_default()
    }
    pub fn generation(&self) -> u64 {
        self.entries.len() as u64
    }
    pub fn events(&self) -> impl Iterator<Item = &JournalEvent> {
        self.entries.iter().map(|entry| &entry.event)
    }
    pub fn digest(&self) -> String {
        self.entries
            .last()
            .map(|e| e.digest.clone())
            .unwrap_or_else(|| digest(codec::binding(self).to_string().as_bytes()))
    }
    pub fn canonical_json(&self) -> String {
        codec::encode(self)
    }
    pub fn decode(document: &str, expected: &CheckpointIdentity) -> Result<Self, Diagnostic> {
        codec::decode(document, expected)
    }
    /// Require the live caller's exact effective limits as well as identity.
    pub fn decode_with_limits(
        document: &str,
        expected: &CheckpointIdentity,
        limits: CheckpointLimits,
    ) -> Result<Self, Diagnostic> {
        let journal = Self::decode(document, expected)?;
        if journal.limits != limits {
            return Err(rejected("limits.substitution"));
        }
        Ok(journal)
    }
    pub fn recovery_disposition(&self) -> RecoveryDisposition {
        match self
            .entries
            .iter()
            .rev()
            .find_map(|entry| match &entry.event {
                JournalEvent::StageReservation { .. } => None,
                event => Some(event),
            }) {
            None => RecoveryDisposition::Fresh,
            Some(JournalEvent::Intent(_)) => RecoveryDisposition::UncertainIntent,
            Some(JournalEvent::Observed { .. }) => RecoveryDisposition::ReplayObserved,
            Some(JournalEvent::Transition { transition, .. }) if transition == "Continue" => {
                RecoveryDisposition::ContinueAfterTransition
            }
            Some(JournalEvent::Transition { .. }) => RecoveryDisposition::Terminal,
            _ => unreachable!(),
        }
    }
    /// Atomic store acknowledgement is required before the caller continues.
    /// Any store error poisons this in-memory journal: lost acknowledgements
    /// cannot be retried into a second host dispatch using the same object.
    pub fn persist(
        &mut self,
        event: JournalEvent,
        usage: CheckpointUsage,
        store: &mut dyn CheckpointStore,
    ) -> Result<(), Diagnostic> {
        if self.poisoned {
            return Err(rejected("uncertain_store"));
        }
        let entry = self.make_entry(event, usage)?;
        self.entries.push(entry);
        let document = self.canonical_json();
        if document.len() > MAX_BYTES {
            self.entries.pop();
            return Err(rejected("bytes"));
        }
        if let Err(CheckpointStoreError) = store.commit(self.generation(), &document) {
            self.poisoned = true;
            return Err(rejected("uncertain_store"));
        }
        Ok(())
    }
    fn make_entry(
        &self,
        event: JournalEvent,
        usage: CheckpointUsage,
    ) -> Result<JournalEntry, Diagnostic> {
        if self.entries.len() >= MAX_ENTRIES {
            return Err(rejected("entries"));
        }
        self.validate_event(&event, usage)?;
        let generation = self.generation() + 1;
        let prior_digest = self.digest();
        let event_json = codec::event(&event)?;
        let hash = digest(json!({"binding":codec::binding(self),"generation":generation,"prior_digest":prior_digest,"usage":codec::usage(usage),"event":event_json}).to_string().as_bytes());
        Ok(JournalEntry {
            generation,
            prior_digest,
            usage,
            event,
            digest: hash,
        })
    }
    fn validate_event(
        &self,
        event: &JournalEvent,
        usage: CheckpointUsage,
    ) -> Result<(), Diagnostic> {
        let prior = self.usage();
        if usage.calls < prior.calls
            || usage.argument_bytes < prior.argument_bytes
            || usage.result_bytes < prior.result_bytes
            || usage.reserved_fuel < prior.reserved_fuel
        {
            return Err(rejected("usage.refund"));
        }
        let last = self
            .entries
            .iter()
            .rev()
            .find_map(|entry| match &entry.event {
                JournalEvent::StageReservation { .. } => None,
                event => Some(event),
            });
        let failed_observation = matches!(
            event,
            JournalEvent::Observed {
                failure: Some(_),
                ..
            }
        );
        let replay_failed_observation = matches!(event, JournalEvent::StageReservation { .. })
            && matches!(
                last,
                Some(JournalEvent::Observed {
                    failure: Some(_),
                    ..
                })
            );
        if usage.calls > self.limits.calls
            || usage.argument_bytes > self.limits.argument_bytes
            || usage.reserved_fuel > self.limits.reserved_fuel
            || (!(failed_observation || replay_failed_observation)
                && (usage.result_bytes > self.limits.result_bytes
                    || usage
                        .argument_bytes
                        .checked_add(usage.result_bytes)
                        .is_none_or(|v| v > self.limits.total_bytes)))
        {
            return Err(rejected("usage.limit"));
        }
        match event {
            JournalEvent::StageReservation { turn, stage, fuel } => {
                if *turn > 4096
                    || !["initialize", "observe", "authorize", "reduce"].contains(&stage.as_str())
                    || *fuel == 0
                    || prior.reserved_fuel.checked_add(*fuel) != Some(usage.reserved_fuel)
                    || usage.calls != prior.calls
                    || usage.argument_bytes != prior.argument_bytes
                    || usage.result_bytes != prior.result_bytes
                {
                    return Err(rejected("stage.reservation"));
                }
            }
            JournalEvent::Intent(context) => {
                if prior.calls.checked_add(1) != Some(usage.calls)
                    || prior
                        .argument_bytes
                        .checked_add(value::transport_bytes(&context.arguments)?)
                        != Some(usage.argument_bytes)
                    || usage.result_bytes != prior.result_bytes
                    || usage.reserved_fuel != prior.reserved_fuel
                {
                    return Err(rejected("intent.usage"));
                }
                match last {
                    None if context.turn == 0 => {}
                    Some(JournalEvent::Transition {
                        context: before,
                        transition,
                        ..
                    }) if transition == "Continue"
                        && before.turn.checked_add(1) == Some(context.turn) => {}
                    _ => return Err(rejected("intent.order")),
                }
            }
            JournalEvent::Observed {
                context,
                result,
                failure,
            } => {
                if !matches!(last, Some(JournalEvent::Intent(before)) if before == context)
                    || usage.calls != prior.calls
                    || usage.argument_bytes != prior.argument_bytes
                    || usage.reserved_fuel != prior.reserved_fuel
                {
                    return Err(rejected("observed.order"));
                }
                if let Some(reason) = failure {
                    let charge = usage.result_bytes - prior.result_bytes;
                    if !result.is_empty()
                        || ![
                            "handler_failed",
                            "result_fields",
                            "result_type",
                            "result_scalar",
                            "result_field_budget",
                            "result_budget",
                            "result_measurement",
                        ]
                        .contains(&reason.as_str())
                        || (reason == "handler_failed" && charge != 0)
                        || (reason != "handler_failed" && !(1..=65537).contains(&charge))
                    {
                        return Err(rejected("observed.failure"));
                    }
                } else if result.is_empty()
                    || prior
                        .result_bytes
                        .checked_add(value::transport_bytes(result)?)
                        != Some(usage.result_bytes)
                {
                    return Err(rejected("observed.charge"));
                }
            }
            JournalEvent::Transition {
                context,
                transition,
                value,
            } => {
                if !matches!(last, Some(JournalEvent::Observed { context: before, failure: None, .. }) if before == context)
                    || usage != prior
                    || !["Continue", "Complete", "Suspend", "Fail"].contains(&transition.as_str())
                {
                    return Err(rejected("transition.order"));
                }
                if if transition == "Fail" {
                    !matches!(value, RetainedValue::I64(_))
                } else {
                    !matches!(value, RetainedValue::Record(_))
                } {
                    return Err(rejected("transition.value"));
                }
            }
        }
        codec::event(event)?;
        Ok(())
    }
}
