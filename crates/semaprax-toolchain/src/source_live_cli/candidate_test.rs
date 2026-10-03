//! Host-selected, bounded, read-only candidate-test capability.
//!
//! This module deliberately exposes only an immutable `ProjectCandidate` to
//! the host observer. It carries no publication handle, process authority,
//! source writer, or Git capability. The observation bytes are canonicalized
//! and bounded before they can become provider feedback.

use semaprax::agent_runtime_v2::{OfflineRepairPreview, SourceModelAdapterIdentity};
use semaprax::digest_hex::LowerHex;
use semaprax::live_invocation::source_journal::{
    RecoveredSourceCheckpoint, SourceEffectFailure, SourceJournalEntry,
};
use semaprax::project::{CandidateTestPolicy, ProjectCandidate};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const MAX_CAPABILITY_ID_BYTES: usize = 240;
pub const MAX_CANDIDATE_TEST_OBSERVATION_BYTES: usize = 4096;
const MAX_CANDIDATE_TEST_DETAIL_BYTES: usize = 1024;
pub const CANDIDATE_TEST_SCHEMA: &str = "semaprax.source-live-cli.candidate-test-observation.v1";
const CANDIDATE_TEST_BINDING_DOMAIN: &[u8] = b"semaprax.source-live-cli.repair-candidate-test.v1\0";
const CANDIDATE_TEST_FEEDBACK_DOMAIN: &[u8] =
    b"semaprax.source-live-cli.repair-candidate-test-feedback.v1\0";
pub(super) const REPAIR_TEST_CAPABILITY_ID: &str = "semaprax.source-live-cli.repair-tested.v1";
const REPAIR_TEST_MAX_STEPS: usize = 100_000;
const REPAIR_TEST_MAX_EXECUTION_BYTES: usize = 65_536;
const REPAIR_TEST_MAX_REPORT_BYTES: usize = 262_144;

/// Fixed, interpreter-only test observer for the separately selected repair
/// host profile. Its policy is deliberately not an operand or configuration
/// field, so a repair request cannot widen test authority.
pub(super) struct FixedCandidateTestObserver {
    policy: CandidateTestPolicy,
}

impl FixedCandidateTestObserver {
    pub(super) fn new() -> Self {
        Self {
            policy: CandidateTestPolicy::new(
                REPAIR_TEST_MAX_STEPS,
                REPAIR_TEST_MAX_EXECUTION_BYTES,
                REPAIR_TEST_MAX_REPORT_BYTES,
            )
            .expect("the fixed repair-tested policy is within candidate-test bounds"),
        }
    }
}

impl CandidateTestObserver for FixedCandidateTestObserver {
    fn observe(
        &mut self,
        _: &CandidateTestCapability,
        subject: &CandidateTestSubject<'_>,
    ) -> Result<CandidateTestObservation, CandidateTestObservationError> {
        let (status, detail) = match subject
            .candidate()
            .execute_tests(subject.candidate_revision(), &self.policy)
        {
            Ok(report) => (
                if report.passed() { "passed" } else { "failed" },
                format!(
                    "candidate-test-report={} policy=repair-tested-v1",
                    report.report_digest()
                ),
            ),
            Err(_) => (
                "refused",
                "candidate-test interpreter replay or execution refused".to_owned(),
            ),
        };
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema": CANDIDATE_TEST_SCHEMA,
            "capability": subject.capability(),
            "candidate_revision": subject.candidate_revision(),
            "base_project_revision": subject.base_project_revision(),
            "source_revision": subject.source_revision(),
            "status": status,
            "detail": detail,
        }))
        .map_err(|_| CandidateTestObservationError)?;
        CandidateTestObservation::try_from_bytes(&bytes)
    }
}

/// Opaque host-selected authority for one bounded candidate-test observation.
pub struct CandidateTestCapability {
    identity: String,
    _sealed: CandidateTestSeal,
}

struct CandidateTestSeal;

impl CandidateTestCapability {
    pub fn host_selected(identity: impl Into<String>) -> Result<Self, String> {
        let identity = identity.into();
        if identity.is_empty()
            || identity.len() > MAX_CAPABILITY_ID_BYTES
            || identity.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err("candidate-test capability identity is invalid".to_owned());
        }
        Ok(Self {
            identity,
            _sealed: CandidateTestSeal,
        })
    }

    fn identity(&self) -> &str {
        &self.identity
    }
}

/// Exact read-only subject, including the immutable candidate itself.
pub struct CandidateTestSubject<'candidate> {
    capability: String,
    candidate_revision: String,
    base_project_revision: String,
    source_revision: String,
    candidate: &'candidate ProjectCandidate,
}

impl CandidateTestSubject<'_> {
    pub fn capability(&self) -> &str {
        &self.capability
    }

    pub fn candidate_revision(&self) -> &str {
        &self.candidate_revision
    }

    pub fn base_project_revision(&self) -> &str {
        &self.base_project_revision
    }

    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    pub fn candidate(&self) -> &ProjectCandidate {
        self.candidate
    }
}

/// Sole host callback; returned bytes are bounded observation data, not a command.
pub trait CandidateTestObserver {
    fn observe(
        &mut self,
        capability: &CandidateTestCapability,
        subject: &CandidateTestSubject<'_>,
    ) -> Result<CandidateTestObservation, CandidateTestObservationError>;
}

/// Borrowed embedding boundary; JSON operands cannot select test authority.
pub struct CandidateTestHost<'a> {
    pub(crate) capability: CandidateTestCapability,
    observer: &'a mut dyn CandidateTestObserver,
}

impl<'a> CandidateTestHost<'a> {
    pub fn new(
        capability: CandidateTestCapability,
        observer: &'a mut dyn CandidateTestObserver,
    ) -> Self {
        Self {
            capability,
            observer,
        }
    }

    pub(crate) fn observe(
        &mut self,
        subject: &CandidateTestSubject<'_>,
    ) -> Result<CandidateTestObservation, CandidateTestObservationError> {
        self.observer.observe(&self.capability, subject)
    }
}

/// Fixed-size observation storage. The callback cannot return an unbounded
/// allocation for validation; malformed content is still rejected later.
#[derive(Clone)]
pub struct CandidateTestObservation {
    bytes: [u8; MAX_CANDIDATE_TEST_OBSERVATION_BYTES],
    len: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateTestObservationError;

impl CandidateTestObservation {
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, CandidateTestObservationError> {
        if bytes.is_empty() || bytes.len() > MAX_CANDIDATE_TEST_OBSERVATION_BYTES {
            return Err(CandidateTestObservationError);
        }
        let mut storage = [0; MAX_CANDIDATE_TEST_OBSERVATION_BYTES];
        storage[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            bytes: storage,
            len: bytes.len() as u16,
        })
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum CandidateTestStatus {
    Passed,
    Failed,
    Refused,
}

impl CandidateTestStatus {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "refused" => Some(Self::Refused),
            _ => None,
        }
    }

    pub(super) fn text(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Refused => "refused",
        }
    }
}

#[derive(Clone)]
pub(crate) struct CandidateTestEvidence {
    pub(super) canonical: String,
    pub(super) status: CandidateTestStatus,
    pub(super) feedback_code: i64,
}

/// Terminal replay reports only the journaled typed feedback identity.
#[derive(Clone, Copy)]
pub(crate) struct ReplayedCandidateTestEvidence {
    pub(super) status: CandidateTestStatus,
    pub(super) feedback_code: i64,
}

pub(super) fn replayed_candidate_test_evidence(
    checkpoint: &RecoveredSourceCheckpoint,
    corrected_operation_id: &str,
    result_id: &str,
    candidate_test_selected: bool,
) -> Option<ReplayedCandidateTestEvidence> {
    if !candidate_test_selected {
        return None;
    }
    let entry = checkpoint
        .entries()
        .iter()
        .rev()
        .find(|entry| match entry {
            SourceJournalEntry::EffectObserved { operation, .. }
            | SourceJournalEntry::EffectFailed { operation, .. } => {
                operation == corrected_operation_id
            }
            _ => false,
        })?;
    let observation = match entry {
        SourceJournalEntry::EffectFailed {
            reason: SourceEffectFailure::HandlerFailed,
            ..
        } => {
            return Some(ReplayedCandidateTestEvidence {
                status: CandidateTestStatus::Refused,
                feedback_code: i64::MIN,
            })
        }
        SourceJournalEntry::EffectObserved { observation, .. } => observation.as_slice(),
        _ => return None,
    };
    let value: Value = serde_json::from_slice(observation).ok()?;
    let object = value.as_object()?;
    let fields = object.get("fields")?.as_array()?;
    let [field] = fields.as_slice() else {
        return None;
    };
    let pair = field.as_array()?;
    let [id, feedback] = pair.as_slice() else {
        return None;
    };
    if object.len() != 2
        || object.get("schema")?.as_str()? != "semaprax.agent-effect-fields.v1"
        || id.as_str()? != result_id
    {
        return None;
    }
    let feedback_text = feedback.as_str()?;
    let expected = format!(
        "{{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[{},{}]]}}\n",
        serde_json::to_string(result_id).ok()?,
        serde_json::to_string(feedback_text).ok()?,
    );
    if observation != expected.as_bytes() {
        return None;
    }
    let feedback_code = feedback_text.parse::<i64>().ok()?;
    let status = match feedback_code {
        i64::MIN => CandidateTestStatus::Refused,
        code if code < 0 => CandidateTestStatus::Failed,
        _ => CandidateTestStatus::Passed,
    };
    Some(ReplayedCandidateTestEvidence {
        status,
        feedback_code,
    })
}

pub(super) fn candidate_test_bound_identity(
    mut identity: SourceModelAdapterIdentity,
    capability: Option<&CandidateTestCapability>,
    target: &str,
    source_revision: &str,
) -> SourceModelAdapterIdentity {
    let Some(capability) = capability else {
        return identity;
    };
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_TEST_BINDING_DOMAIN);
    hasher.update(identity.adapter_identity.as_bytes());
    hasher.update(b"\0");
    hasher.update(capability.identity().as_bytes());
    hasher.update(b"\0");
    hasher.update(target.as_bytes());
    hasher.update(b"\0");
    hasher.update(source_revision.as_bytes());
    identity.adapter_identity = format!(
        "{}:candidate-test:sha256:{:x}",
        identity.adapter_identity,
        LowerHex(hasher.finalize())
    );
    identity
}

pub(super) fn candidate_test_subject<'candidate>(
    capability: &CandidateTestCapability,
    preview: &'candidate OfflineRepairPreview,
    source_revision: &str,
) -> CandidateTestSubject<'candidate> {
    CandidateTestSubject {
        capability: capability.identity().to_owned(),
        candidate_revision: preview.candidate().candidate_digest().to_owned(),
        base_project_revision: preview
            .candidate()
            .base_revision()
            .project_revision()
            .to_owned(),
        source_revision: source_revision.to_owned(),
        candidate: preview.candidate(),
    }
}

pub(crate) fn candidate_test_evidence(
    observation: CandidateTestObservation,
    subject: &CandidateTestSubject,
) -> Result<CandidateTestEvidence, ()> {
    let bytes = observation.as_bytes();
    let value: Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    let canonical = serde_json::to_vec(&value).map_err(|_| ())?;
    if bytes != canonical.as_slice() && bytes.strip_suffix(b"\n") != Some(canonical.as_slice()) {
        return Err(());
    }
    let object = value.as_object().ok_or(())?;
    const KEYS: [&str; 7] = [
        "base_project_revision",
        "candidate_revision",
        "capability",
        "detail",
        "schema",
        "source_revision",
        "status",
    ];
    if object.len() != KEYS.len() || !KEYS.iter().all(|key| object.contains_key(*key)) {
        return Err(());
    }
    if observation_text(object, "schema")? != CANDIDATE_TEST_SCHEMA
        || observation_text(object, "capability")? != subject.capability()
        || observation_text(object, "candidate_revision")? != subject.candidate_revision()
        || observation_text(object, "base_project_revision")? != subject.base_project_revision()
        || observation_text(object, "source_revision")? != subject.source_revision()
    {
        return Err(());
    }
    let detail = observation_text(object, "detail")?;
    if detail.is_empty()
        || detail.len() > MAX_CANDIDATE_TEST_DETAIL_BYTES
        || detail.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(());
    }
    let status = CandidateTestStatus::parse(observation_text(object, "status")?).ok_or(())?;
    let canonical = String::from_utf8(canonical).map_err(|_| ())?;
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_TEST_FEEDBACK_DOMAIN);
    hasher.update(canonical.as_bytes());
    let magnitude =
        i64::from_be_bytes(hasher.finalize()[..8].try_into().map_err(|_| ())?) & i64::MAX;
    let magnitude = magnitude.max(1);
    let feedback_code = match status {
        CandidateTestStatus::Passed => magnitude,
        CandidateTestStatus::Failed => -magnitude,
        CandidateTestStatus::Refused => i64::MIN,
    };
    Ok(CandidateTestEvidence {
        canonical,
        status,
        feedback_code,
    })
}

fn observation_text<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, ()> {
    map.get(key).and_then(Value::as_str).ok_or(())
}
