//! Host-selected automatic archive publication and retention checkpointing.
//!
//! Archive publication is complete before registry mutation begins. The
//! canonical transaction is authority-neutral accountability data derived from
//! the independently recovered checkpoint/plan pair; it is not a store locator.

use std::path::Path;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{RetentionLifecycleCoordinator, RetentionLifecycleOutcome, SuccessfulRetentionReceipt};
use crate::candidate_archive_store::{
    CandidateArchiveStore, CandidateArchiveStoreReceipt, CandidateDraftArchiveStoreReceipt,
};
use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectCandidate, ProjectCandidateArchive, ProjectCandidateDraft, ProjectCandidateDraftArchive,
};
use crate::semantic_retention::{RetentionPolicy, RetentionSubject};

pub const AUTOMATIC_DURABLE_LIFECYCLE_REPLAY_RECEIPT_SCHEMA: &str =
    "semaprax.automatic-durable-candidate-draft-replay-receipt.v1";
pub const AUTOMATIC_DURABLE_LIFECYCLE_RESUME_OUTCOME_SCHEMA: &str =
    "semaprax.automatic-durable-candidate-draft-resume-outcome.v1";
pub const MAX_AUTOMATIC_DURABLE_LIFECYCLE_TRANSACTION_BYTES: usize = 65_536;
const TRANSACTION_DOMAIN: &[u8] =
    b"semaprax.automatic-durable-candidate-draft-transaction.digest.v1\0";
const NONCLAIMS: &[&str] = &[
    "no_current_source_or_checkout_freshness_claim",
    "no_candidate_or_draft_approval_or_publication_authority",
    "no_git_GC_subject_deletion_or_warm_HIR_authority",
    "branch_is_registry_local_history_identity_not_a_git_reference",
];
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One host-owned composition of two startup-selected, held roots.
pub struct AutomaticRetentionLifecycle {
    archives: CandidateArchiveStore,
    retention: RetentionLifecycleCoordinator,
}

impl AutomaticRetentionLifecycle {
    pub fn open(
        archive_root: &Path,
        registry_root: &Path,
        policy: RetentionPolicy,
        expected_cursor: Option<&str>,
    ) -> Result<Self> {
        let archives = CandidateArchiveStore::open(archive_root)?;
        let retention =
            RetentionLifecycleCoordinator::open(registry_root, policy, expected_cursor)?;
        if archives.held_root_identity() == retention.held_root_identity() {
            return Err(binding(
                "archive and retention registry roots must be distinct held directories",
            ));
        }
        Ok(Self {
            archives,
            retention,
        })
    }

    /// Publish the immutable archive first, then attempt exactly one registry
    /// generation. Registry failure never rolls back or retries the archive.
    pub fn persist_candidate(
        &mut self,
        archive: &ProjectCandidateArchive,
    ) -> Result<AutomaticCandidateLifecycleOutcome> {
        let prior = self.retention.expected_cursor_digest().map(str::to_owned);
        let receipt = self.archives.persist(archive)?;
        let lifecycle = self
            .retention
            .checkpoint(&[SuccessfulRetentionReceipt::Candidate(&receipt)]);
        let (transaction, transaction_diagnostics) = self.transaction_after_attempt(
            prior.as_deref(),
            "candidate",
            receipt.archive_digest(),
            receipt.candidate_digest(),
            receipt.base_revision(),
            &lifecycle,
        );
        Ok(AutomaticCandidateLifecycleOutcome {
            receipt,
            lifecycle,
            transaction,
            transaction_diagnostics,
            already_checkpointed: false,
        })
    }

    pub fn persist_draft(
        &mut self,
        archive: &ProjectCandidateDraftArchive,
    ) -> Result<AutomaticDraftLifecycleOutcome> {
        let prior = self.retention.expected_cursor_digest().map(str::to_owned);
        let receipt = self.archives.persist_draft(archive)?;
        let lifecycle = self
            .retention
            .checkpoint(&[SuccessfulRetentionReceipt::Draft(&receipt)]);
        let (transaction, transaction_diagnostics) = self.transaction_after_attempt(
            prior.as_deref(),
            "draft",
            receipt.archive_digest(),
            receipt.draft_digest(),
            receipt.base_revision(),
            &lifecycle,
        );
        Ok(AutomaticDraftLifecycleOutcome {
            receipt,
            lifecycle,
            transaction,
            transaction_diagnostics,
            already_checkpointed: false,
        })
    }

    /// Resume a candidate archive that may have been published before its
    /// registry checkpoint. The existing archive is replayed read-only.
    pub fn resume_candidate(
        &mut self,
        expected_archive: &str,
        expected_candidate: &str,
        expected_base: &str,
    ) -> Result<AutomaticCandidateResumeOutcome> {
        validate_digest(expected_base)?;
        let receipt = self
            .archives
            .replay_candidate_receipt(expected_archive, expected_candidate)?;
        if receipt.base_revision() != expected_base {
            return Err(binding("resumed candidate base revision differs"));
        }
        let subject = receipt.retention_observation()?.subject().clone();
        let state = self.recover_resume_state(&subject, expected_archive)?;
        if state.0 {
            return Ok(AutomaticCandidateResumeOutcome {
                receipt,
                lifecycle: None,
                transaction: None,
                transaction_diagnostics: Vec::new(),
                already_checkpointed: true,
                new_registry_generation: false,
                sequence: state.1,
                cursor: self.retention.expected_cursor_digest().unwrap().to_owned(),
            });
        }
        let prior = self.retention.expected_cursor_digest().map(str::to_owned);
        let lifecycle = self
            .retention
            .checkpoint(&[SuccessfulRetentionReceipt::Candidate(&receipt)]);
        let (transaction, transaction_diagnostics) = self.transaction_after_attempt(
            prior.as_deref(),
            "candidate",
            receipt.archive_digest(),
            receipt.candidate_digest(),
            receipt.base_revision(),
            &lifecycle,
        );
        let sequence = lifecycle.sequence().unwrap_or(0);
        let cursor = lifecycle.cursor_digest().unwrap_or_default().to_owned();
        let new_registry_generation = lifecycle.registry_advanced();
        Ok(AutomaticCandidateResumeOutcome {
            receipt,
            lifecycle: Some(lifecycle),
            transaction,
            transaction_diagnostics,
            already_checkpointed: false,
            new_registry_generation,
            sequence,
            cursor,
        })
    }

    pub fn resume_draft(
        &mut self,
        expected_archive: &str,
        expected_draft: &str,
        expected_base: &str,
    ) -> Result<AutomaticDraftResumeOutcome> {
        validate_digest(expected_base)?;
        let receipt = self
            .archives
            .replay_draft_receipt(expected_archive, expected_draft)?;
        if receipt.base_revision() != expected_base {
            return Err(binding("resumed draft base revision differs"));
        }
        let subject = receipt.retention_observation()?.subject().clone();
        let state = self.recover_resume_state(&subject, expected_archive)?;
        if state.0 {
            return Ok(AutomaticDraftResumeOutcome {
                receipt,
                lifecycle: None,
                transaction: None,
                transaction_diagnostics: Vec::new(),
                already_checkpointed: true,
                new_registry_generation: false,
                sequence: state.1,
                cursor: self.retention.expected_cursor_digest().unwrap().to_owned(),
            });
        }
        let prior = self.retention.expected_cursor_digest().map(str::to_owned);
        let lifecycle = self
            .retention
            .checkpoint(&[SuccessfulRetentionReceipt::Draft(&receipt)]);
        let (transaction, transaction_diagnostics) = self.transaction_after_attempt(
            prior.as_deref(),
            "draft",
            receipt.archive_digest(),
            receipt.draft_digest(),
            receipt.base_revision(),
            &lifecycle,
        );
        let sequence = lifecycle.sequence().unwrap_or(0);
        let cursor = lifecycle.cursor_digest().unwrap_or_default().to_owned();
        let new_registry_generation = lifecycle.registry_advanced();
        Ok(AutomaticDraftResumeOutcome {
            receipt,
            lifecycle: Some(lifecycle),
            transaction,
            transaction_diagnostics,
            already_checkpointed: false,
            new_registry_generation,
            sequence,
            cursor,
        })
    }

    fn recover_resume_state(
        &self,
        expected: &RetentionSubject,
        archive: &str,
    ) -> Result<(bool, u64)> {
        let state = match self.retention.registry.recover() {
            Ok(state) => state,
            Err(errors)
                if self.retention.expected_cursor_digest().is_none()
                    && errors.len() == 1
                    && errors[0].code == "SPX-G467"
                    && errors[0].message == "retention registry is not initialized" =>
            {
                return Ok((false, 0));
            }
            Err(errors) => return Err(errors),
        };
        if Some(state.cursor_digest()) != self.retention.expected_cursor_digest() {
            return Err(binding("automatic lifecycle resume cursor changed"));
        }
        let mut found = false;
        for subject in state.metadata().checkpoint().retained_subjects() {
            if subject == expected {
                found = true;
                continue;
            }
            let same_archive = match subject {
                RetentionSubject::Candidate { archive_digest, .. }
                | RetentionSubject::Draft { archive_digest, .. } => archive_digest == archive,
                RetentionSubject::Image { .. } => false,
            };
            if same_archive {
                return Err(binding(
                    "automatic lifecycle archive is retained with conflicting kind or selectors",
                ));
            }
        }
        Ok((found, state.metadata().checkpoint().sequence()))
    }

    fn transaction_after_attempt(
        &mut self,
        prior_cursor: Option<&str>,
        kind: &str,
        archive_digest: &str,
        content_digest: &str,
        base_revision: &str,
        outcome: &RetentionLifecycleOutcome,
    ) -> (Option<DurableLifecycleReplayReceipt>, Vec<Diagnostic>) {
        if !outcome.registry_advanced() {
            return (None, Vec::new());
        }
        let state = match self.retention.registry.recover() {
            Ok(state) => state,
            Err(errors) => {
                self.retention.blocked = true;
                return (None, errors);
            }
        };
        if Some(state.cursor_digest()) != self.retention.expected_cursor_digest() {
            self.retention.blocked = true;
            return (
                None,
                binding("automatic lifecycle recovered cursor differs after advancement"),
            );
        }
        match DurableLifecycleReplayReceipt::derive(
            prior_cursor,
            state.cursor_digest(),
            state.metadata().checkpoint().checkpoint_digest(),
            state.metadata().plan().plan_digest(),
            kind,
            archive_digest,
            content_digest,
            base_revision,
        ) {
            Ok(receipt) => (Some(receipt), Vec::new()),
            Err(errors) => {
                self.retention.blocked = true;
                (None, errors)
            }
        }
    }

    /// Independently replay every candidate/draft selected by CURRENT through
    /// the held archive root. Images are intentionally not restored here.
    pub fn restore_pending(&self) -> Result<Vec<RestoredPendingSubject>> {
        let state = self.retention.registry.recover()?;
        if Some(state.cursor_digest()) != self.retention.expected_cursor_digest() {
            return Err(binding(
                "automatic lifecycle startup cursor changed during recovery",
            ));
        }
        let mut restored = Vec::new();
        for subject in state.metadata().checkpoint().retained_subjects() {
            match subject {
                RetentionSubject::Candidate {
                    archive_digest,
                    candidate_digest,
                    base_revision,
                } => {
                    let candidate = self.archives.load(archive_digest, candidate_digest)?;
                    if candidate.base_revision().project_revision() != base_revision {
                        return Err(binding("recovered candidate base revision differs"));
                    }
                    restored.push(RestoredPendingSubject::Candidate(candidate));
                }
                RetentionSubject::Draft {
                    archive_digest,
                    draft_digest,
                    base_revision,
                } => {
                    let draft = self.archives.load_draft(archive_digest, draft_digest)?;
                    let replay = ProjectCandidateDraftArchive::prepare(&draft, draft_digest)?;
                    if replay.base_revision() != base_revision {
                        return Err(binding("recovered draft base revision differs"));
                    }
                    restored.push(RestoredPendingSubject::Draft(draft));
                }
                RetentionSubject::Image { .. } => {}
            }
        }
        Ok(restored)
    }
}

pub struct AutomaticCandidateLifecycleOutcome {
    receipt: CandidateArchiveStoreReceipt,
    lifecycle: RetentionLifecycleOutcome,
    transaction: Option<DurableLifecycleReplayReceipt>,
    transaction_diagnostics: Vec<Diagnostic>,
    already_checkpointed: bool,
}

impl AutomaticCandidateLifecycleOutcome {
    pub fn receipt(&self) -> &CandidateArchiveStoreReceipt {
        &self.receipt
    }
    pub fn lifecycle(&self) -> &RetentionLifecycleOutcome {
        &self.lifecycle
    }
    pub fn transaction(&self) -> Option<&DurableLifecycleReplayReceipt> {
        self.transaction.as_ref()
    }
    pub fn recovery_required(&self) -> bool {
        !self.already_checkpointed
            && (!self.lifecycle.registry_advanced() || !self.transaction_diagnostics.is_empty())
    }
    pub const fn already_checkpointed(&self) -> bool {
        self.already_checkpointed
    }
    pub fn transaction_diagnostics(&self) -> &[Diagnostic] {
        &self.transaction_diagnostics
    }
}

pub struct AutomaticDraftLifecycleOutcome {
    receipt: CandidateDraftArchiveStoreReceipt,
    lifecycle: RetentionLifecycleOutcome,
    transaction: Option<DurableLifecycleReplayReceipt>,
    transaction_diagnostics: Vec<Diagnostic>,
    already_checkpointed: bool,
}

pub struct AutomaticCandidateResumeOutcome {
    receipt: CandidateArchiveStoreReceipt,
    lifecycle: Option<RetentionLifecycleOutcome>,
    transaction: Option<DurableLifecycleReplayReceipt>,
    transaction_diagnostics: Vec<Diagnostic>,
    already_checkpointed: bool,
    new_registry_generation: bool,
    sequence: u64,
    cursor: String,
}

impl AutomaticCandidateResumeOutcome {
    pub fn receipt(&self) -> &CandidateArchiveStoreReceipt {
        &self.receipt
    }
    pub fn lifecycle(&self) -> Option<&RetentionLifecycleOutcome> {
        self.lifecycle.as_ref()
    }
    pub fn transaction(&self) -> Option<&DurableLifecycleReplayReceipt> {
        self.transaction.as_ref()
    }
    pub const fn already_checkpointed(&self) -> bool {
        self.already_checkpointed
    }
    pub fn recovery_required(&self) -> bool {
        (!self.already_checkpointed
            && self
                .lifecycle
                .as_ref()
                .is_none_or(|value| !value.registry_advanced()))
            || !self.transaction_diagnostics.is_empty()
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn cursor_digest(&self) -> Option<&str> {
        (!self.cursor.is_empty()).then_some(self.cursor.as_str())
    }
    pub fn to_json(&self) -> Result<String> {
        resume_report(
            "candidate",
            self.receipt.archive_digest(),
            self.receipt.candidate_digest(),
            self.receipt.base_revision(),
            self.sequence,
            self.cursor_digest(),
            self.already_checkpointed,
            self.new_registry_generation,
            self.recovery_required(),
        )
    }
}

pub struct AutomaticDraftResumeOutcome {
    receipt: CandidateDraftArchiveStoreReceipt,
    lifecycle: Option<RetentionLifecycleOutcome>,
    transaction: Option<DurableLifecycleReplayReceipt>,
    transaction_diagnostics: Vec<Diagnostic>,
    already_checkpointed: bool,
    new_registry_generation: bool,
    sequence: u64,
    cursor: String,
}

impl AutomaticDraftResumeOutcome {
    pub fn receipt(&self) -> &CandidateDraftArchiveStoreReceipt {
        &self.receipt
    }
    pub fn lifecycle(&self) -> Option<&RetentionLifecycleOutcome> {
        self.lifecycle.as_ref()
    }
    pub fn transaction(&self) -> Option<&DurableLifecycleReplayReceipt> {
        self.transaction.as_ref()
    }
    pub const fn already_checkpointed(&self) -> bool {
        self.already_checkpointed
    }
    pub fn recovery_required(&self) -> bool {
        (!self.already_checkpointed
            && self
                .lifecycle
                .as_ref()
                .is_none_or(|value| !value.registry_advanced()))
            || !self.transaction_diagnostics.is_empty()
    }
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn cursor_digest(&self) -> Option<&str> {
        (!self.cursor.is_empty()).then_some(self.cursor.as_str())
    }
    pub fn to_json(&self) -> Result<String> {
        resume_report(
            "draft",
            self.receipt.archive_digest(),
            self.receipt.draft_digest(),
            self.receipt.base_revision(),
            self.sequence,
            self.cursor_digest(),
            self.already_checkpointed,
            self.new_registry_generation,
            self.recovery_required(),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn resume_report(
    kind: &str,
    archive: &str,
    content: &str,
    base: &str,
    sequence: u64,
    cursor: Option<&str>,
    already: bool,
    new_generation: bool,
    recovery: bool,
) -> Result<String> {
    render(json!({
        "schema":AUTOMATIC_DURABLE_LIFECYCLE_RESUME_OUTCOME_SCHEMA,
        "kind":kind,
        "archive_digest":archive,
        "content_digest":content,
        "base_revision":base,
        "sequence":sequence,
        "cursor_digest":cursor,
        "status":if already { "already_checkpointed" } else if recovery { "checkpoint_recovery_required" } else { "checkpointed" },
        "archive_write_performed":false,
        "new_registry_generation":new_generation,
        "authority":"none",
        "nonclaims":NONCLAIMS,
    }))
}

impl AutomaticDraftLifecycleOutcome {
    pub fn receipt(&self) -> &CandidateDraftArchiveStoreReceipt {
        &self.receipt
    }
    pub fn lifecycle(&self) -> &RetentionLifecycleOutcome {
        &self.lifecycle
    }
    pub fn transaction(&self) -> Option<&DurableLifecycleReplayReceipt> {
        self.transaction.as_ref()
    }
    pub fn recovery_required(&self) -> bool {
        !self.already_checkpointed
            && (!self.lifecycle.registry_advanced() || !self.transaction_diagnostics.is_empty())
    }
    pub const fn already_checkpointed(&self) -> bool {
        self.already_checkpointed
    }
    pub fn transaction_diagnostics(&self) -> &[Diagnostic] {
        &self.transaction_diagnostics
    }
}

pub enum RestoredPendingSubject {
    Candidate(ProjectCandidate),
    Draft(ProjectCandidateDraft),
}

/// Canonical binding rederived from the exact CURRENT-selected metadata pair.
pub struct DurableLifecycleReplayReceipt {
    json: String,
    digest: String,
}

impl DurableLifecycleReplayReceipt {
    #[allow(clippy::too_many_arguments)]
    fn derive(
        prior_cursor: Option<&str>,
        cursor: &str,
        checkpoint: &str,
        plan: &str,
        kind: &str,
        archive: &str,
        content: &str,
        base: &str,
    ) -> Result<Self> {
        for selector in [
            Some(cursor),
            Some(checkpoint),
            Some(plan),
            Some(archive),
            Some(content),
            Some(base),
            prior_cursor,
        ]
        .into_iter()
        .flatten()
        {
            validate_digest(selector)?;
        }
        if kind != "candidate" && kind != "draft" {
            return Err(binding("automatic lifecycle kind is not supported"));
        }
        let branch = format!(
            "{kind}/{}",
            content
                .strip_prefix("sha256:")
                .ok_or_else(|| binding("automatic lifecycle content digest is malformed"))?
        );
        let value = json!({
            "schema":AUTOMATIC_DURABLE_LIFECYCLE_REPLAY_RECEIPT_SCHEMA,
            "prior_cursor_digest":prior_cursor,
            "cursor_digest":cursor,
            "checkpoint_digest":checkpoint,
            "plan_digest":plan,
            "subject":{"kind":kind,"archive_digest":archive,"content_digest":content,"base_revision":base},
            "branch":branch,
            "operation_kind":format!("archive_{kind}_then_checkpoint"),
            "authority":"none",
            "nonclaims":NONCLAIMS,
        });
        let json = render(value)?;
        let digest = digest(json.as_bytes());
        Ok(Self { json, digest })
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn transaction_digest(&self) -> &str {
        &self.digest
    }

    pub fn restore(bytes: &[u8], expected_digest: &str) -> Result<Self> {
        validate_digest(expected_digest)?;
        if bytes.is_empty() || bytes.len() > MAX_AUTOMATIC_DURABLE_LIFECYCLE_TRANSACTION_BYTES {
            return Err(capacity(
                "automatic lifecycle replay receipt has invalid size",
            ));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| binding("automatic lifecycle replay receipt is not JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| binding("automatic lifecycle replay receipt must be an object"))?;
        let keys = [
            "authority",
            "branch",
            "checkpoint_digest",
            "cursor_digest",
            "nonclaims",
            "operation_kind",
            "plan_digest",
            "prior_cursor_digest",
            "schema",
            "subject",
        ];
        if object.len() != keys.len()
            || keys.iter().any(|key| !object.contains_key(*key))
            || value["schema"] != AUTOMATIC_DURABLE_LIFECYCLE_REPLAY_RECEIPT_SCHEMA
            || value["authority"] != "none"
            || value["nonclaims"] != json!(NONCLAIMS)
        {
            return Err(binding("automatic lifecycle replay receipt schema differs"));
        }
        let subject = value["subject"]
            .as_object()
            .filter(|value| value.len() == 4)
            .ok_or_else(|| binding("automatic lifecycle replay subject is malformed"))?;
        let kind = field(&value, "operation_kind")?
            .strip_prefix("archive_")
            .and_then(|value| value.strip_suffix("_then_checkpoint"))
            .ok_or_else(|| binding("automatic lifecycle operation kind is malformed"))?;
        if kind != "candidate" && kind != "draft"
            || subject.get("kind").and_then(Value::as_str) != Some(kind)
        {
            return Err(binding(
                "automatic lifecycle subject and operation kinds disagree",
            ));
        }
        let prior = match &value["prior_cursor_digest"] {
            Value::Null => None,
            Value::String(value) => Some(value.as_str()),
            _ => return Err(binding("automatic lifecycle prior cursor is malformed")),
        };
        let restored = Self::derive(
            prior,
            field(&value, "cursor_digest")?,
            field(&value, "checkpoint_digest")?,
            field(&value, "plan_digest")?,
            kind,
            subject
                .get("archive_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| binding("automatic lifecycle archive digest is missing"))?,
            subject
                .get("content_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| binding("automatic lifecycle content digest is missing"))?,
            subject
                .get("base_revision")
                .and_then(Value::as_str)
                .ok_or_else(|| binding("automatic lifecycle base revision is missing"))?,
        )?;
        if restored.json.as_bytes() != bytes || restored.digest != expected_digest {
            return Err(binding(
                "automatic lifecycle replay receipt exact bytes or digest disagree",
            ));
        }
        Ok(restored)
    }
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .ok_or_else(|| binding(format!("automatic lifecycle {name} is missing")))
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(binding(
            "automatic lifecycle selector is not canonical lowercase SHA-256",
        ));
    }
    Ok(())
}

fn render(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| binding("automatic lifecycle transaction cannot be encoded"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_AUTOMATIC_DURABLE_LIFECYCLE_TRANSACTION_BYTES {
        return Err(capacity(
            "automatic lifecycle transaction exceeds 65536 bytes",
        ));
    }
    String::from_utf8(bytes).map_err(|_| binding("automatic lifecycle transaction is not UTF-8"))
}

fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(TRANSACTION_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G490", message)]
}

fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G491", message)]
}
