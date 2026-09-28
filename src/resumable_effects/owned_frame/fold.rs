//! Pure closed journal grammar. No source evaluation or ownership construction.
use super::{
    checkpoint, codec,
    journal::{Kind, Record},
    store::OwnedFrameStoreIdentity,
    CheckedOwnedFramePlan, OwnedFrameError as Error,
};
use crate::cleanup_plan::StatusProducer;
use crate::conformance::NormalizedStatus;
use crate::resumable_effects::source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Empty,
    Created,
    Committed,
    Starting,
    Yielded,
    Dispatched,
    Answered,
    Resuming,
    Completed,
    Failed,
    CleanupStarted,
    CleanupSettled,
    Claimed,
}
#[derive(Clone)]
pub(super) struct State {
    pub(super) plan: CheckedOwnedFramePlan,
    pub(super) scope: SourceCheckpointScope,
    pub(super) max_steps: u64,
    pub(super) max_reserved_fuel: u64,
    pub(super) identity: OwnedFrameStoreIdentity,
    pub(super) generation: String,
    pub(super) phase: Phase,
    pub(super) records: Vec<Record>,
    pub(super) reserved_total: u64,
    pub(super) reservation_count: u64,
    pub(super) consumed_total: u64,
    pub(super) phase_sequence: u64,
    pub(super) terminal_sequence: Option<u64>,
    pub(super) replay: Option<(u64, Value)>,
    pub(super) replay_ready: bool,
}
impl State {
    pub(super) fn new(
        plan: CheckedOwnedFramePlan,
        scope: SourceCheckpointScope,
        max_steps: u64,
        max_reserved_fuel: u64,
        identity: OwnedFrameStoreIdentity,
    ) -> Result<Self, Error> {
        if max_steps == 0
            || usize::try_from(max_steps)
                .ok()
                .and_then(|steps| {
                    crate::interpreter::resumable::owned_frame::OwnedFrameBudget::new(steps).ok()
                })
                .is_none()
            || max_reserved_fuel < max_steps
        {
            return Err(Error::Fuel);
        }
        codec::scope(&scope)?;
        Ok(Self {
            plan,
            scope,
            max_steps,
            max_reserved_fuel,
            identity,
            generation: String::new(),
            phase: Phase::Empty,
            records: Vec::new(),
            reserved_total: 0,
            reservation_count: 0,
            consumed_total: 0,
            phase_sequence: 0,
            terminal_sequence: None,
            replay: None,
            replay_ready: false,
        })
    }
    pub(super) fn created(&self) -> Result<&Value, Error> {
        self.records
            .first()
            .map(|row| &row.fields)
            .ok_or(Error::Binding)
    }
    pub(super) fn argument(&self) -> Result<&Value, Error> {
        Ok(&self.created()?["argument"])
    }
    pub(super) fn argument_digest(&self) -> Result<&str, Error> {
        codec::text(&self.created()?["argument_digest"], 71)
    }
    pub(super) fn terminal(&self) -> Option<&Record> {
        self.terminal_sequence
            .and_then(|seq| self.records.get(seq as usize))
    }
    pub(super) fn yielded(&self) -> Option<(u64, &Record)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, row)| row.kind == Kind::Yielded)
            .map(|(seq, row)| (seq as u64, row))
    }
    pub(super) fn answered(&self) -> Option<(u64, &Record)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, row)| row.kind == Kind::Answered)
            .map(|(seq, row)| (seq as u64, row))
    }
    pub(super) fn cleanup_started(&self) -> Option<(u64, &Record)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, row)| row.kind == Kind::CleanupStarted)
            .map(|(seq, row)| (seq as u64, row))
    }
    pub(super) fn cleanup_settled(&self) -> Option<(u64, &Record)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, row)| row.kind == Kind::CleanupSettled)
            .map(|(seq, row)| (seq as u64, row))
    }
    pub(super) fn basis(&self) -> Result<Value, Error> {
        Ok(match self.phase {
            Phase::Committed | Phase::Starting => {
                json!({"kind":"pre_yield","created_sequence":0,"argument_committed_sequence":1,"argument_digest":self.argument_digest()?})
            }
            Phase::Yielded | Phase::Dispatched => {
                let (seq, row) = self.yielded().ok_or(Error::Binding)?;
                json!({"kind":"yielded","yielded_sequence":seq,"checkpoint_digest":row.fields["checkpoint_digest"]})
            }
            Phase::Answered | Phase::Resuming => {
                let (yielded, yielded_row) = self.yielded().ok_or(Error::Binding)?;
                let (answered, answer) = self.answered().ok_or(Error::Binding)?;
                json!({"kind":"answered","yielded_sequence":yielded,"checkpoint_digest":yielded_row.fields["checkpoint_digest"],"answered_sequence":answered,"answer_digest":answer.fields["answer_digest"]})
            }
            _ => return Err(Error::Binding),
        })
    }
    fn require(&self, phase: &[Phase]) -> Result<(), Error> {
        if phase.contains(&self.phase) {
            Ok(())
        } else {
            Err(Error::Binding)
        }
    }
    fn reference(&self, fields: &Value, key: &str, expected: u64) -> Result<(), Error> {
        if fields[key].as_u64() == Some(expected) {
            Ok(())
        } else {
            Err(Error::Binding)
        }
    }
    fn charge(&mut self, fields: &Value) -> Result<(), Error> {
        let total = self
            .reserved_total
            .checked_add(self.max_steps)
            .filter(|total| *total <= self.max_reserved_fuel)
            .ok_or(Error::Fuel)?;
        if fields["reservation"].as_u64() != Some(self.max_steps)
            || fields["reserved_total"].as_u64() != Some(total)
        {
            return Err(Error::Binding);
        }
        self.reserved_total = total;
        self.reservation_count += 1;
        Ok(())
    }
    fn consumed(&mut self, fields: &Value) -> Result<(), Error> {
        let steps = fields["consumed_steps"]
            .as_u64()
            .filter(|steps| *steps <= self.max_steps)
            .ok_or(Error::Binding)?;
        self.consumed_total = self
            .consumed_total
            .checked_add(steps)
            .ok_or(Error::Binding)?;
        Ok(())
    }
    pub(super) fn apply(
        &mut self,
        key: &SourceCheckpointKey,
        record: &Record,
    ) -> Result<(), Error> {
        let sequence = self.records.len() as u64;
        record.validate_keys()?;
        let data = &record.fields;
        if self.records.len() >= codec::MAX_RECORDS {
            return Err(Error::Capacity);
        }
        // A pending replay cannot advance authority/phase until validation;
        // repeated charged interrupted replay reservations remain legal.
        if self.replay.is_some()
            && !matches!(
                record.kind,
                Kind::ReplayReserved | Kind::ReplayValidated | Kind::Failed
            )
        {
            return Err(Error::Binding);
        }
        match record.kind {
            Kind::Created => {
                self.require(&[Phase::Empty])?;
                if data["profile"] != super::plan::PROFILE
                    || data["scope"] != codec::scope(&self.scope)?
                    || data["function"] != self.plan.function().id.as_str()
                    || data["plan_digest"] != self.plan.binding()
                    || data["signature"] != codec::signature(&self.plan)
                    || data["max_steps"].as_u64() != Some(self.max_steps)
                    || data["max_reserved_fuel"].as_u64() != Some(self.max_reserved_fuel)
                    || data["limits"] != limits()
                {
                    return Err(Error::Binding);
                }
                let input = codec::decode_input(&self.plan, &data["argument"])?;
                if codec::input(&self.plan, &input)? != data["argument"]
                    || data["argument_digest"]
                        != codec::fact_digest(
                            b"semaprax.source-owned-frame-arguments.v1\0",
                            &data["argument"],
                        )
                {
                    return Err(Error::Binding);
                }
                self.generation = generation(data, self.identity);
                self.phase = Phase::Created;
            }
            Kind::ArgumentCommitted => {
                self.require(&[Phase::Created])?;
                if data["argument_digest"] != self.argument_digest()?
                    || data["storage"] != codec::storage(&self.plan.liveness().storage)?
                    || data["leaf_flags"] != codec::leaf_flags(&self.plan)
                {
                    return Err(Error::Binding);
                }
                self.phase = Phase::Committed;
                self.phase_sequence = sequence;
            }
            Kind::StartReserved => {
                self.require(&[Phase::Committed, Phase::Starting])?;
                if self.phase == Phase::Starting && !self.replay_ready {
                    return Err(Error::Binding);
                }
                self.reference(data, "causal_sequence", self.records.len() as u64 - 1)?;
                self.charge(data)?;
                self.phase = Phase::Starting;
                self.phase_sequence = sequence;
                self.replay_ready = false;
            }
            Kind::Yielded => {
                self.require(&[Phase::Starting])?;
                self.reference(data, "causal_sequence", self.phase_sequence)?;
                self.consumed(data)?;
                let bytes = codec::text(&data["checkpoint"], codec::MAX_CHECKPOINT)?.as_bytes();
                if data["checkpoint_digest"] != checkpoint::digest(bytes) {
                    return Err(Error::Binding);
                }
                checkpoint::decode(
                    &self.plan,
                    key,
                    &self.scope,
                    self.argument_digest()?,
                    &self.generation,
                    sequence,
                    self.reserved_total,
                    bytes,
                )?;
                self.phase = Phase::Yielded;
                self.phase_sequence = sequence;
            }
            Kind::Dispatched => {
                self.require(&[Phase::Yielded])?;
                self.reference(data, "yielded_sequence", self.phase_sequence)?;
                let (_, yielded) = self.yielded().ok_or(Error::Binding)?;
                if data["checkpoint_digest"] != yielded.fields["checkpoint_digest"] {
                    return Err(Error::Binding);
                }
                let checkpoint_value = codec::parse(
                    codec::text(&yielded.fields["checkpoint"], codec::MAX_CHECKPOINT)?.as_bytes(),
                    codec::MAX_CHECKPOINT,
                )?;
                if data["request_digest"]
                    != codec::fact_digest(
                        b"semaprax.source-owned-frame-request.v1\0",
                        &checkpoint_value["request"],
                    )
                {
                    return Err(Error::Binding);
                }
                self.phase = Phase::Dispatched;
                self.phase_sequence = sequence;
            }
            Kind::Answered => {
                self.require(&[Phase::Dispatched])?;
                self.reference(data, "dispatched_sequence", self.phase_sequence)?;
                let answer = codec::decode_scalar(&data["answer"])?;
                if !crate::interpreter::resumable::owned_frame::snapshot::answer_valid(
                    &self.plan, &answer,
                ) || data["answer_digest"]
                    != codec::fact_digest(
                        b"semaprax.source-owned-frame-answer.v1\0",
                        &data["answer"],
                    )
                {
                    return Err(Error::Binding);
                }
                self.phase = Phase::Answered;
                self.phase_sequence = sequence;
            }
            Kind::ResumeReserved => {
                self.require(&[Phase::Answered, Phase::Resuming])?;
                if self.phase == Phase::Resuming && !self.replay_ready {
                    return Err(Error::Binding);
                }
                self.reference(
                    data,
                    "answered_sequence",
                    self.answered().ok_or(Error::Binding)?.0,
                )?;
                self.charge(data)?;
                self.phase = Phase::Resuming;
                self.phase_sequence = sequence;
                self.replay_ready = false;
            }
            Kind::ReplayReserved => {
                if data["basis"] != self.basis()? {
                    return Err(Error::Binding);
                }
                self.charge(data)?;
                self.replay = Some((sequence, data["basis"].clone()));
                self.replay_ready = false;
            }
            Kind::ReplayValidated => {
                let (reservation, basis) = self.replay.as_ref().ok_or(Error::Binding)?;
                self.reference(data, "reservation_sequence", *reservation)?;
                if data["basis"] != *basis {
                    return Err(Error::Binding);
                }
                self.consumed(data)?;
                self.replay = None;
                self.replay_ready = true;
            }
            Kind::Completed => {
                self.require(&[Phase::Resuming])?;
                self.reference(data, "causal_sequence", self.phase_sequence)?;
                if data["result"] != *self.argument()?
                    || data["result_digest"]
                        != codec::fact_digest(
                            b"semaprax.source-owned-frame-result.v1\0",
                            &data["result"],
                        )
                    || data["pending_cleanup"]
                        != codec::operations(&self.plan.liveness().completion_cleanup)?
                {
                    return Err(Error::Binding);
                }
                self.consumed(data)?;
                self.phase = Phase::Completed;
                self.terminal_sequence = Some(sequence);
            }
            Kind::Failed => {
                self.require(&[
                    Phase::Committed,
                    Phase::Starting,
                    Phase::Yielded,
                    Phase::Dispatched,
                    Phase::Answered,
                    Phase::Resuming,
                ])?;
                let failure = codec::text(&data["failure"], 32)?;
                if ![
                    "language_failure",
                    "fuel_exhausted",
                    "call_depth_exceeded",
                    "evaluation_rejected",
                    "handler_failed",
                    "answer_type_mismatch",
                    "host_abandoned",
                ]
                .contains(&failure)
                {
                    return Err(Error::Binding);
                }
                let evaluated = matches!(self.phase, Phase::Starting | Phase::Resuming)
                    && data["causal_sequence"].as_u64() == Some(self.phase_sequence);
                if !evaluated
                    && (data["causal_sequence"].as_u64() != Some(sequence - 1)
                        || data["consumed_steps"].as_u64() != Some(0))
                {
                    return Err(Error::Binding);
                }
                if failure == "language_failure" {
                    if !evaluated {
                        return Err(Error::Binding);
                    }
                    decode_language(&self.plan, &data["language_status"])?;
                } else if !data["language_status"].is_null() {
                    return Err(Error::Binding);
                }
                let parameter = codec::operations(&self.plan.liveness().failure_cleanup)?;
                let provisional = codec::operations(&self.plan.liveness().result_disposal)?;
                if data["pending_cleanup"] != parameter
                    && !(self.phase == Phase::Resuming && data["pending_cleanup"] == provisional)
                {
                    return Err(Error::Binding);
                }
                self.consumed(data)?;
                self.replay = None;
                self.phase = Phase::Failed;
                self.terminal_sequence = Some(sequence);
            }
            Kind::CleanupStarted => {
                self.require(&[Phase::Completed, Phase::Failed])?;
                self.reference(
                    data,
                    "terminal_sequence",
                    self.terminal_sequence.ok_or(Error::Binding)?,
                )?;
                if data["cleanup_digest"]
                    != codec::fact_digest(
                        b"semaprax.source-owned-frame-cleanup.v1\0",
                        &self.terminal().ok_or(Error::Binding)?.fields["pending_cleanup"],
                    )
                {
                    return Err(Error::Binding);
                }
                self.phase = Phase::CleanupStarted;
                self.phase_sequence = sequence;
            }
            Kind::CleanupSettled => {
                self.require(&[Phase::CleanupStarted])?;
                self.reference(data, "cleanup_started_sequence", self.phase_sequence)?;
                let terminal = self.terminal().ok_or(Error::Binding)?;
                let receipt = &data["receipt"];
                match receipt["kind"].as_str() {
                    Some("observed") => {
                        codec::keys(receipt, &["kind", "settlement", "operations"])?;
                        let operations =
                            receipt["operations"].as_array().ok_or(Error::Malformed)?;
                        let pending = terminal.fields["pending_cleanup"]
                            .as_array()
                            .ok_or(Error::Malformed)?;
                        if operations.len() != pending.len() {
                            return Err(Error::Binding);
                        }
                        let mut success = true;
                        for (operation, expected) in operations.iter().zip(pending) {
                            codec::keys(operation, &["operation", "outcome"])?;
                            if operation["operation"] != *expected {
                                return Err(Error::Binding);
                            }
                            match operation["outcome"].as_str() {
                                Some("completed") => {}
                                Some("failed") => success = false,
                                _ => return Err(Error::Malformed),
                            }
                        }
                        if receipt["settlement"] != if success { "completed" } else { "failed" } {
                            return Err(Error::Binding);
                        }
                    }
                    Some("host_confirmed") => {
                        codec::keys(receipt, &["kind", "confirmation_digest"])?;
                        if terminal.kind != Kind::Failed
                            || receipt["confirmation_digest"]
                                != codec::fact_digest(
                                    b"semaprax.source-owned-frame-confirmation.v1\0",
                                    &self.confirmation()?,
                                )
                        {
                            return Err(Error::Binding);
                        }
                    }
                    _ => return Err(Error::Malformed),
                }
                self.phase = Phase::CleanupSettled;
                self.phase_sequence = sequence;
            }
            Kind::ResultClaimed => {
                self.require(&[Phase::CleanupSettled])?;
                let terminal = self.terminal().ok_or(Error::Binding)?;
                self.reference(
                    data,
                    "completed_sequence",
                    self.terminal_sequence.ok_or(Error::Binding)?,
                )?;
                self.reference(data, "cleanup_settled_sequence", self.phase_sequence)?;
                let (_, settled) = self.cleanup_settled().ok_or(Error::Binding)?;
                if terminal.kind != Kind::Completed
                    || settled.fields["receipt"]["kind"] != "observed"
                    || settled.fields["receipt"]["settlement"] != "completed"
                    || data["result_digest"] != terminal.fields["result_digest"]
                {
                    return Err(Error::Binding);
                }
                self.phase = Phase::Claimed;
                self.phase_sequence = sequence;
            }
        }
        self.records.push(record.clone());
        Ok(())
    }
    pub(super) fn confirmation(&self) -> Result<Value, Error> {
        let (started, row) = self.cleanup_started().ok_or(Error::Binding)?;
        Ok(
            json!({"cleanup_digest":row.fields["cleanup_digest"],"cleanup_started_sequence":started,"function":self.plan.function().id.as_str(),"generation":self.generation,"scope":codec::scope(&self.scope)?,"terminal_sequence":self.terminal_sequence.ok_or(Error::Binding)?}),
        )
    }
}
pub(super) fn generation(created: &Value, identity: OwnedFrameStoreIdentity) -> String {
    codec::fact_digest(
        b"semaprax.source-owned-frame-generation.v1\0",
        &json!({"created":created,"store_identity":identity.json()}),
    )
}
pub(super) fn limits() -> Value {
    json!({"record_fields":8,"bytes_leaves":8,"bytes_per_leaf":1024,"total_bytes":8192,"stable_identity_bytes":256,"invocation_identity_bytes":128,"carrier_bytes":32768,"checkpoint_bytes":65536,"record_bytes":163840,"journal_bytes":524288,"records":64})
}
pub(super) fn decode_language(
    plan: &CheckedOwnedFramePlan,
    value: &Value,
) -> Result<NormalizedStatus, Error> {
    codec::keys(
        value,
        &["schema", "domain_id", "code", "class", "retryable"],
    )?;
    for source in &plan.function().cleanup_plan.status_sources {
        match &source.producer {
            StatusProducer::ContractFalse { phase, .. } => {
                let status = NormalizedStatus::contract(*phase);
                if codec::parse(status.to_json().as_bytes(), codec::MAX_CARRIER)? == *value {
                    return Ok(status);
                }
            }
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => {
                for case in normalized_cases {
                    let status = NormalizedStatus::arithmetic(*case);
                    if codec::parse(status.to_json().as_bytes(), codec::MAX_CARRIER)? == *value {
                        return Ok(status);
                    }
                }
            }
            StatusProducer::PropagatedCall { .. } => {}
        }
    }
    Err(Error::Binding)
}
