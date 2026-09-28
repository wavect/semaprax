//! One authenticated append history, owned by one registered process lease.
use super::{
    codec,
    fold::{self, Phase, State},
    store::RegisteredJournalLease,
    CheckedOwnedFramePlan, OwnedFrameError as Error,
};
use crate::interpreter::resumable::owned_frame::{OwnedFrameFailure, OwnedFrameInput};
use crate::resumable_effects::source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope};
use serde_json::{json, Value};
const SCHEMA: &str = "semaprax.source-owned-frame-journal.v1";
const MAC_DOMAIN: &[u8] = b"semaprax.source-owned-frame-journal-record.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Created,
    ArgumentCommitted,
    StartReserved,
    Yielded,
    Dispatched,
    Answered,
    ResumeReserved,
    ReplayReserved,
    ReplayValidated,
    Completed,
    Failed,
    CleanupStarted,
    CleanupSettled,
    ResultClaimed,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::ArgumentCommitted => "ArgumentCommitted",
            Self::StartReserved => "StartReserved",
            Self::Yielded => "Yielded",
            Self::Dispatched => "Dispatched",
            Self::Answered => "Answered",
            Self::ResumeReserved => "ResumeReserved",
            Self::ReplayReserved => "ReplayReserved",
            Self::ReplayValidated => "ReplayValidated",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::CleanupStarted => "CleanupStarted",
            Self::CleanupSettled => "CleanupSettled",
            Self::ResultClaimed => "ResultClaimed",
        }
    }
    fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Created => &[
                "profile",
                "scope",
                "function",
                "plan_digest",
                "signature",
                "argument",
                "argument_digest",
                "max_steps",
                "max_reserved_fuel",
                "limits",
            ],
            Self::ArgumentCommitted => &["argument_digest", "storage", "leaf_flags"],
            Self::StartReserved => &["causal_sequence", "reservation", "reserved_total"],
            Self::Yielded => &[
                "causal_sequence",
                "checkpoint",
                "checkpoint_digest",
                "consumed_steps",
            ],
            Self::Dispatched => &["yielded_sequence", "checkpoint_digest", "request_digest"],
            Self::Answered => &["dispatched_sequence", "answer", "answer_digest"],
            Self::ResumeReserved => &["answered_sequence", "reservation", "reserved_total"],
            Self::ReplayReserved => &["basis", "reservation", "reserved_total"],
            Self::ReplayValidated => &["reservation_sequence", "basis", "consumed_steps"],
            Self::Completed => &[
                "causal_sequence",
                "result",
                "result_digest",
                "pending_cleanup",
                "consumed_steps",
            ],
            Self::Failed => &[
                "causal_sequence",
                "failure",
                "language_status",
                "pending_cleanup",
                "consumed_steps",
            ],
            Self::CleanupStarted => &["terminal_sequence", "cleanup_digest"],
            Self::CleanupSettled => &["cleanup_started_sequence", "receipt"],
            Self::ResultClaimed => &[
                "completed_sequence",
                "cleanup_settled_sequence",
                "result_digest",
            ],
        }
    }
    fn parse(name: &str) -> Result<Self, Error> {
        for kind in [
            Self::Created,
            Self::ArgumentCommitted,
            Self::StartReserved,
            Self::Yielded,
            Self::Dispatched,
            Self::Answered,
            Self::ResumeReserved,
            Self::ReplayReserved,
            Self::ReplayValidated,
            Self::Completed,
            Self::Failed,
            Self::CleanupStarted,
            Self::CleanupSettled,
            Self::ResultClaimed,
        ] {
            if kind.name() == name {
                return Ok(kind);
            }
        }
        Err(Error::Malformed)
    }
}
#[derive(Clone, Debug)]
pub(super) struct Record {
    pub(super) kind: Kind,
    pub(super) fields: Value,
}
impl Record {
    pub(super) fn new(kind: Kind, fields: Value) -> Result<Self, Error> {
        let row = Self { kind, fields };
        row.validate_keys()?;
        Ok(row)
    }
    pub(super) fn validate_keys(&self) -> Result<(), Error> {
        codec::keys(&self.fields, self.kind.keys())
    }
}
fn unsigned(generation: &str, sequence: u64, previous_mac: &str, record: &Record) -> Value {
    let mut row = record.fields.clone();
    let fields = row.as_object_mut().expect("typed row object");
    fields.insert("schema".into(), json!(SCHEMA));
    fields.insert("generation".into(), json!(generation));
    fields.insert("sequence".into(), json!(sequence));
    fields.insert("previous_mac".into(), json!(previous_mac));
    fields.insert("kind".into(), json!(record.kind.name()));
    row
}
fn render(
    key: &SourceCheckpointKey,
    generation: &str,
    sequence: u64,
    previous_mac: &str,
    record: &Record,
) -> Result<(Vec<u8>, String), Error> {
    record.validate_keys()?;
    let mut row = unsigned(generation, sequence, previous_mac, record);
    let mac = codec::hex(&key.authenticate(MAC_DOMAIN, &codec::canonical(&row)));
    row.as_object_mut()
        .expect("typed row")
        .insert("authentication".into(), json!(mac));
    let mut bytes = codec::canonical(&row);
    bytes.push(b'\n');
    if bytes.len() > codec::MAX_RECORD {
        return Err(Error::Capacity);
    }
    Ok((bytes, mac))
}
pub(super) struct Journal<'key> {
    pub(super) lease: RegisteredJournalLease,
    pub(super) key: &'key SourceCheckpointKey,
    pub(super) state: State,
    pub(super) previous_mac: String,
    pub(super) bytes: usize,
    pub(super) poisoned: bool,
    restoration_issued: bool,
    claim_issued: bool,
}
impl<'key> Journal<'key> {
    pub(super) fn fresh(
        lease: RegisteredJournalLease,
        key: &'key SourceCheckpointKey,
        state: State,
    ) -> Result<Self, Error> {
        lease.validate_current()?;
        Ok(Self {
            lease,
            key,
            state,
            previous_mac: "0".repeat(64),
            bytes: 0,
            poisoned: false,
            restoration_issued: false,
            claim_issued: false,
        })
    }
    pub(super) fn reopen(
        mut lease: RegisteredJournalLease,
        key: &'key SourceCheckpointKey,
        state: State,
    ) -> Result<Self, Error> {
        lease.validate_current()?;
        let bytes = lease.read()?;
        if !bytes.is_empty() && bytes.last() != Some(&b'\n') {
            return Err(Error::Malformed);
        }
        let mut journal = Self::fresh(lease, key, state)?;
        for line in bytes.split_inclusive(|byte| *byte == b'\n') {
            if line.is_empty() {
                continue;
            }
            if journal.state.records.len() >= codec::MAX_RECORDS {
                return Err(Error::Capacity);
            }
            let mut value = codec::parse(line, codec::MAX_RECORD)?;
            let kind = Kind::parse(codec::text(&value["kind"], 32)?)?;
            let mut keys = kind.keys().to_vec();
            keys.extend([
                "schema",
                "generation",
                "sequence",
                "previous_mac",
                "kind",
                "authentication",
            ]);
            codec::keys(&value, &keys)?;
            if value["schema"] != SCHEMA
                || value["sequence"].as_u64() != Some(journal.state.records.len() as u64)
                || value["previous_mac"] != journal.previous_mac
            {
                return Err(Error::Binding);
            }
            let mac = codec::text(&value["authentication"], 64)?.to_owned();
            let tag = codec::unhex(&mac, 32)?;
            if tag.len() != 32 {
                return Err(Error::Malformed);
            }
            value.as_object_mut().expect("row").remove("authentication");
            if !key.verify(MAC_DOMAIN, &codec::canonical(&value), &tag) {
                return Err(Error::Authentication);
            }
            let generation = codec::text(&value["generation"], 71)?.to_owned();
            if !codec::is_digest(&generation) {
                return Err(Error::Malformed);
            }
            let map = value.as_object_mut().expect("row");
            for key in ["schema", "generation", "sequence", "previous_mac", "kind"] {
                map.remove(key);
            }
            let record = Record::new(kind, value)?;
            let mut next = journal.state.clone();
            next.apply(key, &record)?;
            if generation != next.generation {
                return Err(Error::Binding);
            }
            let rendered = render(
                key,
                &generation,
                journal.state.records.len() as u64,
                &journal.previous_mac,
                &record,
            )?;
            if rendered.0 != line {
                return Err(Error::Malformed);
            }
            journal.state = next;
            journal.previous_mac = mac;
            journal.bytes += line.len();
        }
        Ok(journal)
    }
    pub(super) fn validate_current(&self) -> Result<(), Error> {
        self.lease.validate_current()?;
        if self.poisoned {
            return Err(Error::InDoubt);
        }
        Ok(())
    }
    /// Capacity uses rendered maximum-width candidates, never average sizes.
    /// Branches are mutually exclusive future closures, not added together.
    pub(super) fn preflight(
        &self,
        candidate: &Record,
        branches: &[Vec<Record>],
    ) -> Result<(), Error> {
        self.validate_current()?;
        let generation = if candidate.kind == Kind::Created {
            fold::generation(&candidate.fields, self.state.identity)
        } else {
            self.state.generation.clone()
        };
        let first = render(
            self.key,
            &generation,
            self.state.records.len() as u64,
            &self.previous_mac,
            candidate,
        )?
        .0
        .len();
        let mut maximum = 0usize;
        let mut rows = 0usize;
        for branch in branches {
            let mut size = 0usize;
            for row in branch {
                size = size
                    .checked_add(
                        render(self.key, &generation, u64::MAX, &"f".repeat(64), row)?
                            .0
                            .len(),
                    )
                    .ok_or(Error::Capacity)?;
            }
            maximum = maximum.max(size);
            rows = rows.max(branch.len());
        }
        if self
            .bytes
            .checked_add(first)
            .and_then(|n| n.checked_add(maximum))
            .filter(|n| *n <= codec::MAX_JOURNAL)
            .is_none()
            || self.state.records.len() + 1 + rows > codec::MAX_RECORDS
        {
            return Err(Error::Capacity);
        }
        Ok(())
    }
    pub(super) fn append(&mut self, record: Record, branches: &[Vec<Record>]) -> Result<(), Error> {
        self.preflight(&record, branches)?;
        let mut next = self.state.clone();
        next.apply(self.key, &record)?;
        let (bytes, mac) = render(
            self.key,
            &next.generation,
            self.state.records.len() as u64,
            &self.previous_mac,
            &record,
        )?;
        self.poisoned = true;
        self.lease.append(&bytes)?;
        self.bytes += bytes.len();
        self.previous_mac = mac;
        self.state = next;
        self.poisoned = false;
        Ok(())
    }
    pub(super) fn evidence(&self) -> Result<(Vec<u8>, String), Error> {
        self.validate_current()?;
        if self.state.phase == Phase::Empty {
            return Err(Error::Binding);
        }
        let terminal = self.state.terminal().map(|record| {
            let mut fields = record.fields.clone();
            fields
                .as_object_mut()
                .expect("terminal")
                .insert("kind".into(), json!(record.kind.name()));
            fields
        });
        let receipt = self
            .state
            .cleanup_settled()
            .map(|(_, row)| row.fields["receipt"].clone());
        let value = json!({"cleanup_receipt":receipt,"consumed_total":self.state.consumed_total,"generation":self.state.generation,"journal_mac":self.previous_mac,"journal_sequence":self.state.records.len() as u64-1,"plan_digest":self.state.plan.binding(),"reservation_count":self.state.reservation_count,"reserved_total":self.state.reserved_total,"result_claimed_sequence":if self.state.phase==Phase::Claimed{Some(self.state.phase_sequence)}else{None},"scope":codec::scope(&self.state.scope)?,"terminal":terminal});
        let bytes = codec::canonical(&value);
        let digest = codec::digest(b"semaprax.source-owned-frame-evidence.v1\0", &bytes);
        Ok((bytes, digest))
    }
}

pub(crate) struct OwnedFrameClaimPermit<'lease> {
    lease: &'lease RegisteredJournalLease,
    plan_digest: String,
}
impl OwnedFrameClaimPermit<'_> {
    pub(crate) fn consume(self, plan: &CheckedOwnedFramePlan) -> Result<(), Error> {
        self.lease.validate_current()?;
        if self.plan_digest != plan.binding() {
            return Err(Error::Binding);
        }
        Ok(())
    }
}
impl Journal<'_> {
    pub(super) fn claim_permit(&mut self) -> Result<OwnedFrameClaimPermit<'_>, Error> {
        self.validate_current()?;
        if self.state.phase != Phase::Claimed || self.claim_issued {
            return Err(Error::Binding);
        }
        self.claim_issued = true;
        Ok(OwnedFrameClaimPermit {
            lease: &self.lease,
            plan_digest: self.state.plan.binding().to_owned(),
        })
    }
}

// Only this authenticated journal can issue an interpreter restoration permit.
// Its borrowed held lease outlives the one consuming materialization call.
pub(crate) struct OwnedFrameRestorePermit<'lease> {
    lease: &'lease RegisteredJournalLease,
    plan_digest: String,
    input: OwnedFrameInput,
    kind: RestorationKind,
}
pub(crate) enum RestorationKind {
    PreYield,
    Parked,
    Terminal {
        failure: Option<OwnedFrameFailure>,
        provisional: bool,
    },
}
impl OwnedFrameRestorePermit<'_> {
    pub(crate) fn consume(
        self,
        plan: &CheckedOwnedFramePlan,
    ) -> Result<(OwnedFrameInput, RestorationKind), Error> {
        self.lease.validate_current()?;
        if self.plan_digest != plan.binding() {
            return Err(Error::Binding);
        }
        crate::interpreter::resumable::owned_frame::snapshot::validate_input(plan, &self.input)
            .map_err(|_| Error::Binding)?;
        Ok((self.input, self.kind))
    }
}
impl Journal<'_> {
    pub(super) fn restore_permit(&mut self) -> Result<OwnedFrameRestorePermit<'_>, Error> {
        self.validate_current()?;
        if self.restoration_issued {
            return Err(Error::Binding);
        }
        let input = codec::decode_input(&self.state.plan, self.state.argument()?)?;
        let kind = match self.state.phase {
            Phase::Committed | Phase::Starting => RestorationKind::PreYield,
            Phase::Yielded | Phase::Dispatched | Phase::Answered | Phase::Resuming => {
                RestorationKind::Parked
            }
            Phase::Completed => RestorationKind::Terminal {
                failure: None,
                provisional: true,
            },
            Phase::Failed => {
                let fields = &self.state.terminal().ok_or(Error::Binding)?.fields;
                let provisional = fields["pending_cleanup"]
                    == codec::operations(&self.state.plan.liveness().result_disposal)?;
                let failure = decode_failure(&self.state.plan, fields)?;
                RestorationKind::Terminal {
                    failure: Some(failure),
                    provisional,
                }
            }
            _ => return Err(Error::Binding),
        };
        self.restoration_issued = true;
        Ok(OwnedFrameRestorePermit {
            lease: &self.lease,
            plan_digest: self.state.plan.binding().to_owned(),
            input,
            kind,
        })
    }
}
pub(super) fn decode_failure(
    plan: &CheckedOwnedFramePlan,
    fields: &Value,
) -> Result<OwnedFrameFailure, Error> {
    Ok(match fields["failure"].as_str() {
        Some("language_failure") => {
            OwnedFrameFailure::Language(fold::decode_language(plan, &fields["language_status"])?)
        }
        Some("fuel_exhausted") => OwnedFrameFailure::FuelExhausted,
        Some("host_abandoned") => OwnedFrameFailure::HostAbandoned,
        Some("answer_type_mismatch") => OwnedFrameFailure::AnswerTypeMismatch,
        Some("handler_failed") => OwnedFrameFailure::HandlerFailed,
        Some("call_depth_exceeded") => OwnedFrameFailure::CallDepthExceeded,
        Some("evaluation_rejected") => OwnedFrameFailure::EvaluationRejected,
        _ => return Err(Error::Binding),
    })
}

#[cfg(all(test, unix))]
pub(super) mod tests;
