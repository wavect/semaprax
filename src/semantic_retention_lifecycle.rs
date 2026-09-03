//! Startup-selected host coordination for successful retention store receipts.
//!
//! The coordinator can advance only the explicit semantic-retention registry.
//! It receives no subject-store handle and cannot restore or delete a subject.

use std::path::Path;

use serde_json::{json, Value};

use crate::candidate_archive_store::{
    CandidateArchiveStoreReceipt, CandidateDraftArchiveStoreReceipt,
};
use crate::diagnostic::Diagnostic;
use crate::project::ImageStoreReceipt;
use crate::semantic_retention::{RetentionAuthority, RetentionPolicy, RetentionReceipt};
use crate::semantic_retention_registry;

pub const SEMANTIC_RETENTION_LIFECYCLE_REPORT_SCHEMA: &str =
    "semaprax.semantic-retention-lifecycle-report.v1";
pub const MAX_SEMANTIC_RETENTION_LIFECYCLE_REPORT_BYTES: usize = 65_536;
const MAX_RECEIPTS: usize = 96;
const MAX_DIAGNOSTICS: usize = 64;
const NONCLAIMS: &[&str] = &[
    "successful_receipt_does_not_grant_subject_store_or_restore_authority",
    "registry_checkpoint_does_not_apply_or_approve_the_GC_plan",
    "registry_failure_does_not_undo_or_deny_prior_immutable_subject_storage",
    "no_source_candidate_draft_image_approval_or_publication_state_is_changed",
    "no_implicit_root_discovery_freshness_clock_mtime_or_access_frequency",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// The only successful store receipt families admitted by this coordinator.
#[derive(Clone, Copy)]
pub enum SuccessfulRetentionReceipt<'a> {
    Image(&'a ImageStoreReceipt),
    Candidate(&'a CandidateArchiveStoreReceipt),
    Draft(&'a CandidateDraftArchiveStoreReceipt),
}

impl<'a> SuccessfulRetentionReceipt<'a> {
    fn receipt(self) -> &'a dyn RetentionReceipt {
        match self {
            Self::Image(receipt) => receipt,
            Self::Candidate(receipt) => receipt,
            Self::Draft(receipt) => receipt,
        }
    }

    fn projection(self) -> Result<Value> {
        let observation = self.receipt().retention_observation()?;
        let common = (
            observation.subject().subject_digest(),
            observation.stored_bytes(),
        );
        Ok(match self {
            Self::Image(receipt) => json!({
                "kind":"image",
                "subject_digest":common.0,
                "stored_bytes":common.1,
                "receipt_digest":receipt.receipt_digest(),
                "image_digest":receipt.image_digest(),
                "revision_store_entry":receipt.entry_digest(),
                "project_revision":receipt.project_revision(),
            }),
            Self::Candidate(receipt) => json!({
                "kind":"candidate",
                "subject_digest":common.0,
                "stored_bytes":common.1,
                "archive_digest":receipt.archive_digest(),
                "candidate_digest":receipt.candidate_digest(),
                "base_revision":receipt.base_revision(),
            }),
            Self::Draft(receipt) => json!({
                "kind":"draft",
                "subject_digest":common.0,
                "stored_bytes":common.1,
                "archive_digest":receipt.archive_digest(),
                "draft_digest":receipt.draft_digest(),
                "base_revision":receipt.base_revision(),
            }),
        })
    }

    fn ordering_key(self) -> ReceiptOrderingKey {
        match self {
            Self::Image(receipt) => (
                0,
                receipt.image_digest().to_owned(),
                receipt.entry_digest().to_owned(),
                receipt.project_revision().to_owned(),
                receipt.receipt_digest().to_owned(),
                receipt.retained_image_bytes(),
            ),
            Self::Candidate(receipt) => (
                1,
                receipt.archive_digest().to_owned(),
                receipt.candidate_digest().to_owned(),
                receipt.base_revision().to_owned(),
                String::new(),
                receipt.stored_bytes(),
            ),
            Self::Draft(receipt) => (
                2,
                receipt.archive_digest().to_owned(),
                receipt.draft_digest().to_owned(),
                receipt.base_revision().to_owned(),
                String::new(),
                receipt.stored_bytes(),
            ),
        }
    }
}

type ReceiptOrderingKey = (u8, String, String, String, String, u64);

/// One startup-fixed registry root, policy, and exact current-cursor
/// expectation. Failure poisons the coordinator; a host must recover and reopen
/// with a newly selected exact cursor before another registry attempt.
pub struct RetentionLifecycleCoordinator {
    registry: semantic_retention_registry::RetentionRegistryHandle,
    policy: RetentionPolicy,
    expected_cursor: Option<String>,
    blocked: bool,
}

impl RetentionLifecycleCoordinator {
    /// Authenticate the explicit startup expectation. `None` means the caller
    /// explicitly expects an existing private registry root with no `CURRENT`.
    pub fn open(
        root: &Path,
        policy: RetentionPolicy,
        expected_cursor: Option<&str>,
    ) -> Result<Self> {
        let registry = semantic_retention_registry::RetentionRegistryHandle::open(root)?;
        if let Some(expected) = expected_cursor {
            validate_digest(expected)?;
            let state = registry.recover()?;
            if state.cursor_digest() != expected || state.metadata().checkpoint().policy() != policy
            {
                return Err(binding(
                    "retention lifecycle startup cursor or fixed policy disagrees",
                ));
            }
        } else {
            match registry.recover() {
                Ok(_) => {
                    return Err(binding(
                        "retention lifecycle expected an uninitialized registry",
                    ))
                }
                Err(errors)
                    if errors.len() == 1
                        && errors[0].code == "SPX-G467"
                        && errors[0].message == "retention registry is not initialized" => {}
                Err(errors) => return Err(errors),
            }
        }
        Ok(Self {
            registry,
            policy,
            expected_cursor: expected_cursor.map(str::to_owned),
            blocked: false,
        })
    }

    pub fn expected_cursor_digest(&self) -> Option<&str> {
        self.expected_cursor.as_deref()
    }

    pub const fn authority(&self) -> RetentionAuthority {
        RetentionAuthority::None
    }

    /// Checkpoint one bounded batch of receipts returned by already successful
    /// immutable stores. Registry failure never changes that prior store fact.
    pub fn checkpoint(
        &mut self,
        receipts: &[SuccessfulRetentionReceipt<'_>],
    ) -> RetentionLifecycleOutcome {
        if receipts.is_empty() {
            return RetentionLifecycleOutcome::new(
                0,
                Vec::new(),
                "no_registry_attempt_invalid_receipt_inventory",
                "no_successful_receipt_batch_accepted",
                None,
                None,
                invalid("retention lifecycle requires at least one successful typed receipt"),
            );
        }
        if self.blocked {
            let mut stored_receipts = if receipts.len() <= MAX_RECEIPTS {
                receipts
                    .iter()
                    .filter_map(|receipt| receipt.projection().ok())
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            sort_receipt_projections(&mut stored_receipts);
            return RetentionLifecycleOutcome::new(
                receipts.len(),
                stored_receipts,
                "registry_attempt_blocked_reopen_required",
                "successful_receipts_precede_registry_attempt",
                None,
                None,
                poisoned("retention lifecycle is blocked after a prior registry failure"),
            );
        }
        if receipts.len() > MAX_RECEIPTS {
            return RetentionLifecycleOutcome::new(
                receipts.len(),
                Vec::new(),
                "no_registry_attempt_receipt_capacity_exceeded",
                "successful_receipts_precede_registry_attempt",
                None,
                None,
                capacity("retention lifecycle receipt inventory exceeds 96"),
            );
        }
        let (stored_receipts, projection_diagnostics) = collect_projection_results(
            receipts
                .iter()
                .copied()
                .map(|receipt| (receipt.ordering_key(), receipt.projection()))
                .collect(),
        );
        if !projection_diagnostics.is_empty() {
            return RetentionLifecycleOutcome::new(
                receipts.len(),
                stored_receipts,
                "no_registry_attempt_receipt_projection_failed",
                "successful_typed_store_receipt_was_supplied",
                None,
                None,
                projection_diagnostics,
            );
        }
        let borrowed = receipts
            .iter()
            .copied()
            .map(SuccessfulRetentionReceipt::receipt)
            .collect::<Vec<_>>();
        let result = match self.expected_cursor.as_deref() {
            Some(expected) => self.registry.advance(expected, &borrowed),
            None => self.registry.initialize(self.policy, &borrowed),
        };
        match result {
            Ok(state) => {
                self.expected_cursor = Some(state.cursor_digest().to_owned());
                let outcome = RetentionLifecycleOutcome::new(
                    receipts.len(),
                    stored_receipts,
                    "advanced",
                    "successful_receipts_precede_registry_attempt",
                    Some(state.metadata().checkpoint().sequence()),
                    Some(state.cursor_digest()),
                    Vec::new(),
                );
                if !outcome.diagnostics().is_empty() {
                    self.blocked = true;
                }
                outcome
            }
            Err(diagnostics) => {
                self.blocked = true;
                let status = failure_status(&diagnostics);
                RetentionLifecycleOutcome::new(
                    receipts.len(),
                    stored_receipts,
                    status,
                    "successful_receipts_precede_registry_attempt",
                    None,
                    None,
                    diagnostics,
                )
            }
        }
    }
}

fn sort_receipt_projections(receipts: &mut [Value]) {
    receipts.sort_by_cached_key(|receipt| {
        let mut canonical = receipt.clone();
        canonical.sort_all_objects();
        (
            receipt["subject_digest"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            receipt["kind"].as_str().unwrap_or_default().to_owned(),
            serde_json::to_vec(&canonical)
                .expect("closed successful receipt projection is JSON-encodable"),
        )
    });
}

fn collect_projection_results(
    mut projections: Vec<(ReceiptOrderingKey, Result<Value>)>,
) -> (Vec<Value>, Vec<Diagnostic>) {
    projections.sort_by(|left, right| left.0.cmp(&right.0));
    let mut receipts = Vec::with_capacity(projections.len());
    let mut diagnostics = Vec::new();
    for (_, result) in projections {
        match result {
            Ok(receipt) => receipts.push(receipt),
            Err(mut errors) => {
                errors.sort_by(|left, right| {
                    left.code
                        .cmp(right.code)
                        .then_with(|| left.message.cmp(&right.message))
                        .then_with(|| left.severity.as_str().cmp(right.severity.as_str()))
                        .then_with(|| left.path.cmp(&right.path))
                        .then_with(|| {
                            left.span
                                .map(|span| (span.start, span.end, span.line, span.column))
                                .cmp(
                                    &right
                                        .span
                                        .map(|span| (span.start, span.end, span.line, span.column)),
                                )
                        })
                        .then_with(|| left.help.cmp(&right.help))
                });
                diagnostics.extend(errors);
            }
        }
    }
    sort_receipt_projections(&mut receipts);
    (receipts, diagnostics)
}

/// One explicit outcome after typed receipts already prove immutable subject
/// storage. Registry failure is represented as data, never as a store rollback.
pub struct RetentionLifecycleOutcome {
    advanced: bool,
    sequence: Option<u64>,
    cursor_digest: Option<String>,
    diagnostics: Vec<Diagnostic>,
    json: String,
}

impl RetentionLifecycleOutcome {
    fn new(
        successful_receipt_count: usize,
        stored_receipts: Vec<Value>,
        registry_status: &'static str,
        subject_store_status: &'static str,
        sequence: Option<u64>,
        cursor_digest: Option<&str>,
        mut diagnostics: Vec<Diagnostic>,
    ) -> Self {
        if diagnostics.len() > MAX_DIAGNOSTICS {
            diagnostics = capacity("retention lifecycle diagnostic inventory exceeds 64");
        }
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        let next_action = match registry_status {
            "advanced" => "continue_with_the_returned_exact_cursor",
            "no_registry_attempt_invalid_receipt_inventory"
            | "no_registry_attempt_receipt_capacity_exceeded" => {
                "retry_with_a_bounded_nonempty_successful_receipt_batch"
            }
            "no_registry_attempt_receipt_projection_failed" => {
                "inspect_the_successful_typed_receipt_before_retry"
            }
            _ => "recover_registry_and_reopen_with_an_exact_startup_expectation",
        };
        let value = json!({
            "schema":SEMANTIC_RETENTION_LIFECYCLE_REPORT_SCHEMA,
            "successful_receipt_count":successful_receipt_count,
            "successful_store_receipts":stored_receipts,
            "subject_store_status":subject_store_status,
            "registry_cursor_status":registry_status,
            "sequence":sequence,
            "cursor_digest":cursor_digest,
            "diagnostic_codes":codes,
            "next_action":next_action,
            "authority":"none",
            "nonclaims":NONCLAIMS,
        });
        let advanced = registry_status == "advanced";
        let json = match render(value) {
            Ok(json) => json,
            Err(_) => {
                diagnostics = encoding("retention lifecycle outcome report is unavailable");
                let unavailable = if advanced {
                    "registry_cursor_advanced_report_unavailable"
                } else {
                    "registry_outcome_report_unavailable"
                };
                render(json!({
                    "schema":SEMANTIC_RETENTION_LIFECYCLE_REPORT_SCHEMA,
                    "successful_receipt_count":successful_receipt_count,
                    "successful_store_receipts":[],
                    "subject_store_status":subject_store_status,
                    "registry_cursor_status":unavailable,
                    "sequence":sequence,
                    "cursor_digest":cursor_digest,
                    "diagnostic_codes":["SPX-G484"],
                    "next_action":"recover_registry_and_reopen_with_an_exact_startup_expectation",
                    "authority":"none",
                    "nonclaims":NONCLAIMS,
                }))
                .expect("fixed retention lifecycle fallback is bounded canonical JSON")
            }
        };
        Self {
            advanced,
            sequence,
            cursor_digest: cursor_digest.map(str::to_owned),
            diagnostics,
            json,
        }
    }
    pub const fn registry_advanced(&self) -> bool {
        self.advanced
    }
    pub const fn sequence(&self) -> Option<u64> {
        self.sequence
    }
    pub fn cursor_digest(&self) -> Option<&str> {
        self.cursor_digest.as_deref()
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub const fn authority(&self) -> RetentionAuthority {
        RetentionAuthority::None
    }
}

fn failure_status(diagnostics: &[Diagnostic]) -> &'static str {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G468")
    {
        "registry_cursor_uncertain_recovery_required"
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-I371")
    {
        "registry_cursor_not_advanced_pair_publication_uncertain"
    } else if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G467")
    {
        "registry_cursor_not_advanced_stale"
    } else {
        "registry_cursor_not_advanced"
    }
}

fn render(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| encoding("retention lifecycle report cannot be encoded"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_SEMANTIC_RETENTION_LIFECYCLE_REPORT_BYTES {
        return Err(capacity("retention lifecycle report exceeds 65536 bytes"));
    }
    String::from_utf8(bytes)
        .map_err(|_| encoding("retention lifecycle report encoding is not UTF-8"))
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "retention lifecycle expected cursor is not canonical lowercase SHA-256",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G480", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G481", message)]
}
fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G482", message)]
}
fn poisoned(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G483", message)]
}
fn encoding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G484", message)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_subject_digest_rows_have_a_total_canonical_order() {
        let left = json!({
            "kind":"candidate",
            "subject_digest":"a".repeat(64),
            "stored_bytes":1,
            "archive_digest":"b".repeat(64),
            "candidate_digest":"c".repeat(64),
            "base_revision":"d".repeat(64),
        });
        let right = json!({
            "kind":"draft",
            "subject_digest":"a".repeat(64),
            "stored_bytes":1,
            "archive_digest":"b".repeat(64),
            "draft_digest":"c".repeat(64),
            "base_revision":"d".repeat(64),
        });
        let mut forward = vec![left.clone(), right.clone()];
        let mut reversed = vec![right, left];
        sort_receipt_projections(&mut forward);
        sort_receipt_projections(&mut reversed);
        assert_eq!(forward, reversed);
    }

    #[test]
    fn mixed_projection_failure_inventory_is_independent_of_input_order() {
        let success = json!({
            "kind":"candidate",
            "subject_digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "stored_bytes":1,
        });
        let first_key = (0, "a".into(), "a".into(), "a".into(), "a".into(), 1);
        let failed_key = (1, "b".into(), "b".into(), "b".into(), String::new(), 2);
        let last_key = (2, "c".into(), "c".into(), "c".into(), String::new(), 3);
        let inputs = vec![
            (last_key, Ok(success.clone())),
            (
                failed_key,
                Err(vec![
                    Diagnostic::io("SPX-G999", "later deterministic error"),
                    Diagnostic::io("SPX-G998", "earlier deterministic error"),
                ]),
            ),
            (first_key, Ok(success)),
        ];
        let mut reversed = inputs.clone();
        reversed.reverse();
        let forward = collect_projection_results(inputs);
        let backward = collect_projection_results(reversed);
        assert_eq!(forward.0, backward.0);
        assert_eq!(
            forward
                .1
                .iter()
                .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
                .collect::<Vec<_>>(),
            backward
                .1
                .iter()
                .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
                .collect::<Vec<_>>()
        );
        assert_eq!(forward.1[0].code, "SPX-G998");
    }
}
