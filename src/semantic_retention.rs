//! Authority-neutral retention checkpoints for derived semantic subjects.
//!
//! The checkpoint records exact image, candidate, and draft identities and
//! computes a deterministic bounded keep/evict plan. It owns no store root and
//! performs no deletion. Restoring a checkpoint authenticates metadata only;
//! every selected subject still needs its ordinary source replay.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

pub const SEMANTIC_RETENTION_CHECKPOINT_SCHEMA: &str = "semaprax.semantic-retention-checkpoint.v1";
pub const SEMANTIC_RETENTION_PLAN_SCHEMA: &str = "semaprax.semantic-retention-plan.v1";
pub const MAX_RETENTION_SUBJECTS: usize = 96;
pub const MAX_RETENTION_TRANSITION_SUBJECTS: usize = MAX_RETENTION_SUBJECTS * 2;
pub const MAX_RETENTION_CHECKPOINT_BYTES: usize = 1_048_576;
pub const MAX_RETENTION_PLAN_BYTES: usize = 1_048_576;
pub const MAX_RETENTION_SUBJECT_BYTES: u64 = 134_217_728;
pub const MAX_RETENTION_TOTAL_BYTES: u64 = 8_589_934_592;
pub const MAX_RETENTION_GENERATIONS: u64 = 32;

const CHECKPOINT_DOMAIN: &[u8] = b"semaprax.semantic-retention-checkpoint.digest.v1\0";
const SUBJECT_DOMAIN: &[u8] = b"semaprax.semantic-retention-subject.digest.v1\0";
const PLAN_DOMAIN: &[u8] = b"semaprax.semantic-retention-plan.digest.v1\0";
const NONCLAIMS: &[&str] = &[
    "checkpoint_not_source_freshness_or_current_workspace_evidence",
    "checkpoint_not_validation_test_review_approval_or_publication_authority",
    "restored_identity_requires_ordinary_exact_source_and_history_replay",
    "plan_performs_no_deletion_adoption_repair_or_store_discovery",
    "caller_must_apply_evictions_through_separately_selected_store_authority",
    "old_checkpoint_bytes_remain_historical_and_never_become_current_implicitly",
    "no_clock_mtime_access_frequency_or_nondeterministic_ordering",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Closed identity of one disposable derived subject.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RetentionSubject {
    Image {
        image_digest: String,
        revision_store_entry: String,
        project_revision: String,
    },
    Candidate {
        archive_digest: String,
        candidate_digest: String,
        base_revision: String,
    },
    Draft {
        archive_digest: String,
        draft_digest: String,
        base_revision: String,
    },
}

impl RetentionSubject {
    pub fn image(
        image_digest: impl Into<String>,
        revision_store_entry: impl Into<String>,
        project_revision: impl Into<String>,
    ) -> Result<Self> {
        let subject = Self::Image {
            image_digest: image_digest.into(),
            revision_store_entry: revision_store_entry.into(),
            project_revision: project_revision.into(),
        };
        subject.validate()?;
        Ok(subject)
    }

    pub fn candidate(
        archive_digest: impl Into<String>,
        candidate_digest: impl Into<String>,
        base_revision: impl Into<String>,
    ) -> Result<Self> {
        let subject = Self::Candidate {
            archive_digest: archive_digest.into(),
            candidate_digest: candidate_digest.into(),
            base_revision: base_revision.into(),
        };
        subject.validate()?;
        Ok(subject)
    }

    pub fn draft(
        archive_digest: impl Into<String>,
        draft_digest: impl Into<String>,
        base_revision: impl Into<String>,
    ) -> Result<Self> {
        let subject = Self::Draft {
            archive_digest: archive_digest.into(),
            draft_digest: draft_digest.into(),
            base_revision: base_revision.into(),
        };
        subject.validate()?;
        Ok(subject)
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Image { .. } => "image",
            Self::Candidate { .. } => "candidate",
            Self::Draft { .. } => "draft",
        }
    }

    /// Stable metadata identity. This is not a store locator or capability.
    pub fn subject_digest(&self) -> String {
        digest(SUBJECT_DOMAIN, canonical(&self.value()).as_bytes())
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Image {
                image_digest,
                revision_store_entry,
                project_revision,
            } => {
                validate_digest(image_digest)?;
                validate_digest(revision_store_entry)?;
                validate_digest(project_revision)?;
            }
            Self::Candidate {
                archive_digest,
                candidate_digest,
                base_revision,
            } => {
                validate_digest(archive_digest)?;
                validate_digest(candidate_digest)?;
                validate_digest(base_revision)?;
            }
            Self::Draft {
                archive_digest,
                draft_digest,
                base_revision,
            } => {
                validate_digest(archive_digest)?;
                validate_digest(draft_digest)?;
                validate_digest(base_revision)?;
            }
        }
        Ok(())
    }

    fn value(&self) -> Value {
        match self {
            Self::Image {
                image_digest,
                revision_store_entry,
                project_revision,
            } => json!({
                "kind":"image",
                "image_digest":image_digest,
                "revision_store_entry":revision_store_entry,
                "project_revision":project_revision,
            }),
            Self::Candidate {
                archive_digest,
                candidate_digest,
                base_revision,
            } => json!({
                "kind":"candidate",
                "archive_digest":archive_digest,
                "candidate_digest":candidate_digest,
                "base_revision":base_revision,
            }),
            Self::Draft {
                archive_digest,
                draft_digest,
                base_revision,
            } => json!({
                "kind":"draft",
                "archive_digest":archive_digest,
                "draft_digest":draft_digest,
                "base_revision":base_revision,
            }),
        }
    }

    fn parse(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("retention subject must be an object"))?;
        let kind = text(object.get("kind"), "retention subject kind is missing")?;
        match kind {
            "image" => {
                require_keys(
                    object,
                    &[
                        "kind",
                        "image_digest",
                        "revision_store_entry",
                        "project_revision",
                    ],
                    "image retention subject has unknown or missing fields",
                )?;
                Self::image(
                    text(object.get("image_digest"), "image digest is missing")?,
                    text(
                        object.get("revision_store_entry"),
                        "revision store entry is missing",
                    )?,
                    text(
                        object.get("project_revision"),
                        "project revision is missing",
                    )?,
                )
            }
            "candidate" => {
                require_keys(
                    object,
                    &[
                        "kind",
                        "archive_digest",
                        "candidate_digest",
                        "base_revision",
                    ],
                    "candidate retention subject has unknown or missing fields",
                )?;
                Self::candidate(
                    text(object.get("archive_digest"), "archive digest is missing")?,
                    text(
                        object.get("candidate_digest"),
                        "candidate digest is missing",
                    )?,
                    text(object.get("base_revision"), "base revision is missing")?,
                )
            }
            "draft" => {
                require_keys(
                    object,
                    &["kind", "archive_digest", "draft_digest", "base_revision"],
                    "draft retention subject has unknown or missing fields",
                )?;
                Self::draft(
                    text(object.get("archive_digest"), "archive digest is missing")?,
                    text(object.get("draft_digest"), "draft digest is missing")?,
                    text(object.get("base_revision"), "base revision is missing")?,
                )
            }
            _ => Err(invalid("retention subject kind is not supported")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    max_subjects: usize,
    max_bytes: u64,
    protected_generations: u64,
}

impl RetentionPolicy {
    pub fn new(max_subjects: usize, max_bytes: u64, protected_generations: u64) -> Result<Self> {
        if max_subjects == 0 || max_subjects > MAX_RETENTION_SUBJECTS {
            return Err(capacity("retention subject limit is outside 1 through 96"));
        }
        if max_bytes == 0 || max_bytes > MAX_RETENTION_TOTAL_BYTES {
            return Err(capacity("retention byte limit is outside its fixed bound"));
        }
        if protected_generations > MAX_RETENTION_GENERATIONS {
            return Err(capacity("retention protected-generation count exceeds 32"));
        }
        Ok(Self {
            max_subjects,
            max_bytes,
            protected_generations,
        })
    }

    pub const fn max_subjects(&self) -> usize {
        self.max_subjects
    }
    pub const fn max_bytes(&self) -> u64 {
        self.max_bytes
    }
    pub const fn protected_generations(&self) -> u64 {
        self.protected_generations
    }

    fn value(self) -> Value {
        json!({
            "max_subjects":self.max_subjects,
            "max_bytes":self.max_bytes,
            "protected_generations":self.protected_generations,
            "selection":"protected_then_most_recent_exact_subject_identity",
        })
    }

    fn parse(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("retention policy must be an object"))?;
        require_keys(
            object,
            &[
                "max_subjects",
                "max_bytes",
                "protected_generations",
                "selection",
            ],
            "retention policy has unknown or missing fields",
        )?;
        if value["selection"] != "protected_then_most_recent_exact_subject_identity" {
            return Err(invalid("retention policy selection differs"));
        }
        let max_subjects = usize::try_from(number(
            object.get("max_subjects"),
            "retention subject limit is missing",
        )?)
        .map_err(|_| capacity("retention subject limit exceeds this host"))?;
        Self::new(
            max_subjects,
            number(object.get("max_bytes"), "retention byte limit is missing")?,
            number(
                object.get("protected_generations"),
                "protected generation count is missing",
            )?,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetentionObservation {
    subject: RetentionSubject,
    stored_bytes: u64,
}

impl RetentionObservation {
    pub fn new(subject: RetentionSubject, stored_bytes: u64) -> Result<Self> {
        subject.validate()?;
        if stored_bytes == 0 || stored_bytes > MAX_RETENTION_SUBJECT_BYTES {
            return Err(capacity(
                "retention subject byte count is outside its fixed bound",
            ));
        }
        Ok(Self {
            subject,
            stored_bytes,
        })
    }

    pub fn subject(&self) -> &RetentionSubject {
        &self.subject
    }
    pub const fn stored_bytes(&self) -> u64 {
        self.stored_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    subject: RetentionSubject,
    stored_bytes: u64,
    first_seen: u64,
    last_seen: u64,
}

impl Entry {
    fn value(&self) -> Value {
        json!({
            "subject_digest":self.subject.subject_digest(),
            "subject":self.subject.value(),
            "stored_bytes":self.stored_bytes,
            "first_seen":self.first_seen,
            "last_seen":self.last_seen,
        })
    }

    fn parse(value: &Value, sequence: u64) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("retention entry must be an object"))?;
        require_keys(
            object,
            &[
                "subject_digest",
                "subject",
                "stored_bytes",
                "first_seen",
                "last_seen",
            ],
            "retention entry has unknown or missing fields",
        )?;
        let subject = RetentionSubject::parse(
            object
                .get("subject")
                .ok_or_else(|| invalid("retention entry subject is missing"))?,
        )?;
        if text(
            object.get("subject_digest"),
            "retention subject digest is missing",
        )? != subject.subject_digest()
        {
            return Err(binding("retention subject digest disagrees"));
        }
        let stored_bytes = number(
            object.get("stored_bytes"),
            "retention stored byte count is missing",
        )?;
        if stored_bytes == 0 || stored_bytes > MAX_RETENTION_SUBJECT_BYTES {
            return Err(capacity(
                "retention subject byte count is outside its fixed bound",
            ));
        }
        let first_seen = number(
            object.get("first_seen"),
            "retention first-seen generation is missing",
        )?;
        let last_seen = number(
            object.get("last_seen"),
            "retention last-seen generation is missing",
        )?;
        if first_seen == 0 || first_seen > last_seen || last_seen > sequence {
            return Err(binding("retention entry generation order is invalid"));
        }
        Ok(Self {
            subject,
            stored_bytes,
            first_seen,
            last_seen,
        })
    }
}

/// Canonical recovery metadata. It carries identities, never retained content.
#[derive(Clone, Debug)]
pub struct RetentionCheckpoint {
    sequence: u64,
    previous: Option<String>,
    policy: RetentionPolicy,
    entries: Vec<Entry>,
    json: String,
    digest: String,
}

impl RetentionCheckpoint {
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn previous_checkpoint_digest(&self) -> Option<&str> {
        self.previous.as_deref()
    }
    pub const fn policy(&self) -> RetentionPolicy {
        self.policy
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn checkpoint_digest(&self) -> &str {
        &self.digest
    }
    pub fn retained_subjects(&self) -> impl ExactSizeIterator<Item = &RetentionSubject> {
        self.entries.iter().map(|entry| &entry.subject)
    }
    pub fn retained_bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.stored_bytes).sum()
    }
}

/// Pure deterministic transition. `evicted_subjects` is an instruction to a
/// separately authorized host, not proof that cleanup occurred.
#[derive(Debug)]
pub struct RetentionTransition {
    checkpoint: RetentionCheckpoint,
    plan: RetentionGarbageCollectionPlan,
}

impl RetentionTransition {
    pub fn checkpoint(&self) -> &RetentionCheckpoint {
        &self.checkpoint
    }
    pub fn into_checkpoint(self) -> RetentionCheckpoint {
        self.checkpoint
    }
    pub fn evicted_subjects(&self) -> impl ExactSizeIterator<Item = &RetentionSubject> {
        self.plan.evicted.iter()
    }
    pub fn plan_json(&self) -> &str {
        &self.plan.json
    }
    pub fn plan_digest(&self) -> &str {
        &self.plan.digest
    }
    pub fn plan(&self) -> &RetentionGarbageCollectionPlan {
        &self.plan
    }
}

/// Canonical companion to a checkpoint. Persisting both before cleanup lets a
/// host recover the exact pending eviction identities after interruption.
#[derive(Clone, Debug)]
pub struct RetentionGarbageCollectionPlan {
    predecessor: Option<String>,
    checkpoint_digest: String,
    sequence: u64,
    retained_subjects: usize,
    retained_bytes: u64,
    evicted: Vec<RetentionSubject>,
    json: String,
    digest: String,
}

impl RetentionGarbageCollectionPlan {
    pub fn predecessor_checkpoint_digest(&self) -> Option<&str> {
        self.predecessor.as_deref()
    }
    pub fn checkpoint_digest(&self) -> &str {
        &self.checkpoint_digest
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub const fn retained_subject_count(&self) -> usize {
        self.retained_subjects
    }
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
    pub fn evicted_subjects(&self) -> impl ExactSizeIterator<Item = &RetentionSubject> {
        self.evicted.iter()
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn plan_digest(&self) -> &str {
        &self.digest
    }
}

/// Merge exact observations into the prior metadata and automatically select
/// a deterministic bounded survivor set. No filesystem access occurs.
pub fn checkpoint(
    previous: Option<&RetentionCheckpoint>,
    expected_previous: Option<&str>,
    sequence: u64,
    policy: RetentionPolicy,
    observations: &[RetentionObservation],
) -> Result<RetentionTransition> {
    let expected_sequence = match previous {
        Some(checkpoint) => checkpoint
            .sequence
            .checked_add(1)
            .ok_or_else(|| capacity("retention checkpoint sequence overflow"))?,
        None => 1,
    };
    if sequence != expected_sequence {
        return Err(stale("retention checkpoint sequence is stale"));
    }
    let previous_digest = match (previous, expected_previous) {
        (None, None) => None,
        (Some(checkpoint), Some(expected)) => {
            validate_digest(expected)?;
            if checkpoint.checkpoint_digest() != expected {
                return Err(stale("retention predecessor digest is stale"));
            }
            Some(expected.to_owned())
        }
        _ => {
            return Err(stale(
                "retention predecessor checkpoint and digest must be supplied together",
            ))
        }
    };
    if observations.len() > MAX_RETENTION_SUBJECTS {
        return Err(capacity("retention observation inventory exceeds 96"));
    }

    let mut entries = previous
        .map(|checkpoint| {
            checkpoint
                .entries
                .iter()
                .cloned()
                .map(|entry| (entry.subject.subject_digest(), entry))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut observed = BTreeSet::new();
    for observation in observations {
        observation.subject.validate()?;
        let identity = observation.subject.subject_digest();
        if !observed.insert(identity.clone()) {
            return Err(invalid("retention observations repeat an exact subject"));
        }
        match entries.get_mut(&identity) {
            Some(entry) => {
                if entry.subject != observation.subject
                    || entry.stored_bytes != observation.stored_bytes
                {
                    return Err(binding(
                        "retention observation changes immutable subject accounting",
                    ));
                }
                entry.last_seen = sequence;
            }
            None => {
                entries.insert(
                    identity,
                    Entry {
                        subject: observation.subject.clone(),
                        stored_bytes: observation.stored_bytes,
                        first_seen: sequence,
                        last_seen: sequence,
                    },
                );
            }
        }
    }
    if entries.len() > MAX_RETENTION_TRANSITION_SUBJECTS {
        return Err(capacity(
            "retention transition inventory exceeds its fixed bound",
        ));
    }

    let mut ranked = entries.into_values().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .last_seen
            .cmp(&left.last_seen)
            .then_with(|| right.first_seen.cmp(&left.first_seen))
            .then_with(|| {
                left.subject
                    .subject_digest()
                    .cmp(&right.subject.subject_digest())
            })
    });
    let protected_after = sequence.saturating_sub(policy.protected_generations);
    let mut kept = Vec::new();
    let mut evicted = Vec::new();
    let mut bytes = 0_u64;
    for entry in ranked {
        let protected = policy.protected_generations != 0 && entry.last_seen > protected_after;
        let next_bytes = bytes
            .checked_add(entry.stored_bytes)
            .ok_or_else(|| capacity("retention total byte accounting overflow"))?;
        let fits = kept.len() < policy.max_subjects && next_bytes <= policy.max_bytes;
        if protected && !fits {
            return Err(capacity(
                "protected retention generations exceed the selected policy bounds",
            ));
        }
        if fits {
            bytes = next_bytes;
            kept.push(entry);
        } else {
            evicted.push(entry.subject);
        }
    }
    kept.sort_by_key(|entry| entry.subject.subject_digest());
    evicted.sort_by_key(RetentionSubject::subject_digest);
    let checkpoint = make_checkpoint(sequence, previous_digest, policy, kept)?;
    let plan = make_plan(&checkpoint, evicted)?;
    Ok(RetentionTransition { checkpoint, plan })
}

/// Restore the exact pending GC companion for one already authenticated
/// checkpoint. It still cannot delete an entry or prove that deletion ran.
pub fn restore_plan(
    bytes: &[u8],
    expected_plan: &str,
    checkpoint: &RetentionCheckpoint,
) -> Result<RetentionGarbageCollectionPlan> {
    validate_digest(expected_plan)?;
    if bytes.is_empty() || bytes.len() > MAX_RETENTION_PLAN_BYTES {
        return Err(capacity(
            "retention plan bytes are empty or exceed the fixed bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("retention plan is not bounded valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("retention plan must be an object"))?;
    require_keys(
        object,
        &[
            "schema",
            "predecessor",
            "checkpoint_digest",
            "sequence",
            "retained_subjects",
            "retained_bytes",
            "evicted",
            "effect",
            "nonclaims",
        ],
        "retention plan has unknown or missing fields",
    )?;
    if value["schema"] != SEMANTIC_RETENTION_PLAN_SCHEMA
        || value["effect"] != "none_metadata_plan_only"
        || value["nonclaims"] != json!(NONCLAIMS)
        || text(
            object.get("checkpoint_digest"),
            "retention plan checkpoint digest is missing",
        )? != checkpoint.checkpoint_digest()
        || number(object.get("sequence"), "retention plan sequence is missing")?
            != checkpoint.sequence
        || number(
            object.get("retained_subjects"),
            "retention plan retained count is missing",
        )? != checkpoint.entries.len() as u64
        || number(
            object.get("retained_bytes"),
            "retention plan retained bytes are missing",
        )? != checkpoint.retained_bytes()
    {
        return Err(binding(
            "retention plan schema or checkpoint accounting differs",
        ));
    }
    let predecessor = match object.get("predecessor") {
        Some(Value::Null) => None,
        Some(Value::String(value)) => {
            validate_digest(value)?;
            Some(value.clone())
        }
        _ => return Err(invalid("retention plan predecessor is malformed")),
    };
    if predecessor.as_deref() != checkpoint.previous_checkpoint_digest() {
        return Err(stale("retention plan predecessor is stale"));
    }
    let evicted_values = object
        .get("evicted")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("retention plan eviction inventory is missing"))?;
    if evicted_values.len() > MAX_RETENTION_TRANSITION_SUBJECTS {
        return Err(capacity(
            "retention plan eviction inventory exceeds its bound",
        ));
    }
    let retained = checkpoint
        .entries
        .iter()
        .map(|entry| entry.subject.subject_digest())
        .collect::<BTreeSet<_>>();
    let mut identities = BTreeSet::new();
    let mut evicted = Vec::with_capacity(evicted_values.len());
    for value in evicted_values {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("retention plan eviction must be an object"))?;
        require_keys(
            object,
            &["subject_digest", "subject"],
            "retention plan eviction has unknown or missing fields",
        )?;
        let subject = RetentionSubject::parse(
            object
                .get("subject")
                .ok_or_else(|| invalid("retention plan eviction subject is missing"))?,
        )?;
        let identity = subject.subject_digest();
        if text(
            object.get("subject_digest"),
            "retention plan eviction identity is missing",
        )? != identity
            || !identities.insert(identity.clone())
            || retained.contains(&identity)
        {
            return Err(binding(
                "retention plan eviction identity is repeated, retained, or mismatched",
            ));
        }
        evicted.push(subject);
    }
    if evicted
        .windows(2)
        .any(|pair| pair[0].subject_digest() >= pair[1].subject_digest())
    {
        return Err(binding("retention plan evictions are not canonical"));
    }
    let restored = make_plan(checkpoint, evicted)?;
    if restored.to_json().as_bytes() != bytes || restored.plan_digest() != expected_plan {
        return Err(binding(
            "retention plan bytes or expected identity disagree",
        ));
    }
    Ok(restored)
}

/// Restore exact canonical checkpoint metadata. The caller must retain and
/// supply the expected checkpoint and predecessor identities; accepting bytes
/// without those selectors would permit silent rollback.
pub fn restore_checkpoint(
    bytes: &[u8],
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
) -> Result<RetentionCheckpoint> {
    validate_digest(expected_checkpoint)?;
    if let Some(previous) = expected_previous {
        validate_digest(previous)?;
    }
    if bytes.is_empty() || bytes.len() > MAX_RETENTION_CHECKPOINT_BYTES {
        return Err(capacity(
            "retention checkpoint bytes are empty or exceed the fixed bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("retention checkpoint is not bounded valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("retention checkpoint must be an object"))?;
    require_keys(
        object,
        &[
            "schema",
            "sequence",
            "previous_checkpoint_digest",
            "policy",
            "entries",
            "retained_bytes",
            "nonclaims",
        ],
        "retention checkpoint has unknown or missing fields",
    )?;
    if value["schema"] != SEMANTIC_RETENTION_CHECKPOINT_SCHEMA
        || value["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid(
            "retention checkpoint schema or authority boundary differs",
        ));
    }
    let sequence = number(object.get("sequence"), "retention sequence is missing")?;
    if sequence == 0 {
        return Err(invalid("retention checkpoint sequence must be nonzero"));
    }
    let previous = match object.get("previous_checkpoint_digest") {
        Some(Value::Null) => None,
        Some(Value::String(value)) => {
            validate_digest(value)?;
            Some(value.clone())
        }
        _ => return Err(invalid("retention predecessor binding is malformed")),
    };
    if previous.as_deref() != expected_previous || (sequence == 1) != previous.is_none() {
        return Err(stale(
            "retention checkpoint predecessor or initial sequence is stale",
        ));
    }
    let policy = RetentionPolicy::parse(
        object
            .get("policy")
            .ok_or_else(|| invalid("retention policy is missing"))?,
    )?;
    let values = object
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("retention entries are missing"))?;
    if values.len() > policy.max_subjects || values.len() > MAX_RETENTION_SUBJECTS {
        return Err(capacity("retention checkpoint entry count exceeds policy"));
    }
    let mut entries = Vec::with_capacity(values.len());
    let mut identities = BTreeSet::new();
    let mut total = 0_u64;
    for value in values {
        let entry = Entry::parse(value, sequence)?;
        if !identities.insert(entry.subject.subject_digest()) {
            return Err(binding("retention checkpoint repeats a subject"));
        }
        total = total
            .checked_add(entry.stored_bytes)
            .ok_or_else(|| capacity("retention total byte accounting overflow"))?;
        entries.push(entry);
    }
    if entries
        .windows(2)
        .any(|pair| pair[0].subject.subject_digest() >= pair[1].subject.subject_digest())
        || total > policy.max_bytes
        || object.get("retained_bytes").and_then(Value::as_u64) != Some(total)
    {
        return Err(binding(
            "retention checkpoint order or retained-byte accounting disagrees",
        ));
    }
    let restored = make_checkpoint(sequence, previous, policy, entries)?;
    if restored.to_json().as_bytes() != bytes || restored.checkpoint_digest() != expected_checkpoint
    {
        return Err(binding(
            "retention checkpoint bytes or expected identity disagree",
        ));
    }
    Ok(restored)
}

fn make_checkpoint(
    sequence: u64,
    previous: Option<String>,
    policy: RetentionPolicy,
    entries: Vec<Entry>,
) -> Result<RetentionCheckpoint> {
    let retained_bytes = entries.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.stored_bytes)
            .ok_or_else(|| capacity("retention total byte accounting overflow"))
    })?;
    let value = json!({
        "schema":SEMANTIC_RETENTION_CHECKPOINT_SCHEMA,
        "sequence":sequence,
        "previous_checkpoint_digest":previous,
        "policy":policy.value(),
        "entries":entries.iter().map(Entry::value).collect::<Vec<_>>(),
        "retained_bytes":retained_bytes,
        "nonclaims":NONCLAIMS,
    });
    let json = render(value, MAX_RETENTION_CHECKPOINT_BYTES)?;
    let digest = digest(CHECKPOINT_DOMAIN, json.as_bytes());
    Ok(RetentionCheckpoint {
        sequence,
        previous,
        policy,
        entries,
        json,
        digest,
    })
}

fn make_plan(
    checkpoint: &RetentionCheckpoint,
    evicted: Vec<RetentionSubject>,
) -> Result<RetentionGarbageCollectionPlan> {
    let value = json!({
        "schema":SEMANTIC_RETENTION_PLAN_SCHEMA,
        "predecessor":checkpoint.previous_checkpoint_digest(),
        "checkpoint_digest":checkpoint.checkpoint_digest(),
        "sequence":checkpoint.sequence,
        "retained_subjects":checkpoint.entries.len(),
        "retained_bytes":checkpoint.retained_bytes(),
        "evicted":evicted.iter().map(|subject| json!({
            "subject_digest":subject.subject_digest(),
            "subject":subject.value(),
        })).collect::<Vec<_>>(),
        "effect":"none_metadata_plan_only",
        "nonclaims":NONCLAIMS,
    });
    let json = render(value, MAX_RETENTION_PLAN_BYTES)?;
    let digest = digest(PLAN_DOMAIN, json.as_bytes());
    Ok(RetentionGarbageCollectionPlan {
        predecessor: checkpoint.previous.clone(),
        checkpoint_digest: checkpoint.digest.clone(),
        sequence: checkpoint.sequence,
        retained_subjects: checkpoint.entries.len(),
        retained_bytes: checkpoint.retained_bytes(),
        evicted,
        json,
        digest,
    })
}

fn canonical(value: &Value) -> String {
    serde_json::to_string(value).expect("bounded retention value is serializable")
}

fn render(value: Value, max: usize) -> Result<String> {
    let mut rendered = canonical(&value);
    rendered.push('\n');
    if rendered.len() > max {
        return Err(capacity(
            "retention canonical output exceeds its byte bound",
        ));
    }
    Ok(rendered)
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "retention identity must be canonical lowercase SHA256",
        ));
    }
    Ok(())
}

fn require_keys(
    object: &serde_json::Map<String, Value>,
    expected: &[&str],
    message: &'static str,
) -> Result<()> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(message));
    }
    Ok(())
}

fn text<'a>(value: Option<&'a Value>, message: &'static str) -> Result<&'a str> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(message))
}

fn number(value: Option<&Value>, message: &'static str) -> Result<u64> {
    value
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(message))
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G420", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G421", message)]
}
fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G422", message)]
}
fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G423", message)]
}
