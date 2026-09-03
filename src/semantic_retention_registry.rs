//! Explicit durable cursor over authority-neutral semantic-retention metadata.
//!
//! The registry serializes receipt-driven checkpoint generations in one
//! caller-selected private root. It persists the immutable checkpoint/plan pair
//! before atomically pivoting `CURRENT`. It never resolves or deletes a retained
//! subject and never restores source, candidates, drafts, approval, or authority.

#![cfg_attr(
    not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )),
    allow(
        unused_imports,
        reason = "unsupported hosts expose only fail-closed registry APIs; the held-transition imports are not invoked"
    )
)]

use std::path::Path;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::semantic_retention::{
    checkpoint_receipts, RetentionAuthority, RetentionPolicy, RetentionReceipt,
};
use crate::semantic_retention_store::{self, StoredRetentionMetadata};

#[cfg(test)]
mod tests;
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
mod unix;

pub const SEMANTIC_RETENTION_REGISTRY_CURSOR_SCHEMA: &str =
    "semaprax.semantic-retention-registry-cursor.v1";
pub const MAX_RETENTION_REGISTRY_CURSOR_BYTES: usize = 4096;

const CURSOR_DOMAIN: &[u8] = b"semaprax.semantic-retention-registry-cursor.digest.v1\0";
const NONCLAIMS: &[&str] = &[
    "cursor_selects_metadata_not_source_candidate_draft_or_image_state",
    "recovery_does_not_apply_the_pending_GC_plan_or_delete_any_subject",
    "receipt_checkpointing_grants_no_source_approval_or_publication_authority",
    "current_generation_is_registry_local_not_workspace_freshness_evidence",
    "no_clock_mtime_access_frequency_store_discovery_or_implicit_root_selection",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One authenticated registry generation. The metadata remains authority-free;
/// the explicit root used to obtain it is deliberately not retained here.
#[derive(Debug)]
pub struct RetentionRegistryState {
    metadata: StoredRetentionMetadata,
    cursor_json: String,
    cursor_digest: String,
}

/// One process-local held identity for an explicitly selected registry root.
/// It is deliberately crate-private: public callers use the lifecycle
/// coordinator, which fixes the policy and expected cursor at startup.
pub(crate) struct RetentionRegistryHandle {
    #[cfg(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    ))]
    root: unix::Root,
    #[cfg(not(all(
        unix,
        any(
            target_os = "linux",
            target_os = "android",
            target_vendor = "apple",
            target_os = "redox"
        )
    )))]
    unsupported: (),
}

impl RetentionRegistryHandle {
    pub(crate) fn open(root: &Path) -> Result<Self> {
        #[cfg(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        ))]
        {
            Ok(Self {
                root: unix::Root::open(root)?,
            })
        }
        #[cfg(not(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        )))]
        {
            let _ = root;
            Err(io(
                "retention registry requires supported Unix held-root publication",
            ))
        }
    }

    pub(crate) fn held_root_identity(&self) -> (u64, u64) {
        #[cfg(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        ))]
        {
            self.root.identity_key()
        }
        #[cfg(not(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        )))]
        {
            unreachable!("retention registry construction fails on unsupported hosts")
        }
    }

    pub(crate) fn initialize(
        &self,
        policy: RetentionPolicy,
        receipts: &[&dyn RetentionReceipt],
    ) -> Result<RetentionRegistryState> {
        require_receipts(receipts)?;
        #[cfg(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        ))]
        {
            unix::transaction_held(&self.root, |current, metadata_root| {
                if current.is_some() {
                    return Err(stale("retention registry is already initialized"));
                }
                let transition = checkpoint_receipts(None, None, 1, policy, receipts)?;
                let metadata = settle_transition(metadata_root, &transition)?;
                let cursor = Cursor::new(transition.checkpoint(), transition.plan())?;
                Ok((cursor.json.as_bytes().to_vec(), state(metadata, cursor)))
            })
        }
        #[cfg(not(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        )))]
        {
            let _ = (policy, receipts);
            Err(io(
                "retention registry requires supported Unix held-root publication",
            ))
        }
    }

    pub(crate) fn recover(&self) -> Result<RetentionRegistryState> {
        #[cfg(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        ))]
        {
            unix::read_held(&self.root, |current, metadata_root| {
                let cursor = Cursor::parse(current)?;
                let metadata = semantic_retention_store::load_held(
                    metadata_root,
                    &cursor.checkpoint,
                    cursor.previous.as_deref(),
                    &cursor.plan,
                )?;
                validate_metadata(&cursor, &metadata)?;
                Ok(state(metadata, cursor))
            })
        }
        #[cfg(not(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        )))]
        {
            Err(io(
                "retention registry requires supported Unix held-root publication",
            ))
        }
    }

    pub(crate) fn advance(
        &self,
        expected_cursor_digest: &str,
        receipts: &[&dyn RetentionReceipt],
    ) -> Result<RetentionRegistryState> {
        validate_digest(expected_cursor_digest)?;
        require_receipts(receipts)?;
        #[cfg(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        ))]
        {
            unix::transaction_held(&self.root, |current, metadata_root| {
                let current =
                    current.ok_or_else(|| stale("retention registry is not initialized"))?;
                let cursor = Cursor::parse(current)?;
                if cursor.digest != expected_cursor_digest {
                    return Err(stale("retention registry CURRENT selector is stale"));
                }
                let previous = semantic_retention_store::load_held(
                    metadata_root,
                    &cursor.checkpoint,
                    cursor.previous.as_deref(),
                    &cursor.plan,
                )?;
                validate_metadata(&cursor, &previous)?;
                let sequence = cursor
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| capacity("retention registry sequence overflow"))?;
                let transition = checkpoint_receipts(
                    Some(previous.checkpoint()),
                    Some(&cursor.checkpoint),
                    sequence,
                    cursor.policy,
                    receipts,
                )?;
                let metadata = settle_transition(metadata_root, &transition)?;
                let next = Cursor::new(transition.checkpoint(), transition.plan())?;
                Ok((next.json.as_bytes().to_vec(), state(metadata, next)))
            })
        }
        #[cfg(not(all(
            unix,
            any(
                target_os = "linux",
                target_os = "android",
                target_vendor = "apple",
                target_os = "redox"
            )
        )))]
        {
            let _ = receipts;
            Err(io(
                "retention registry requires supported Unix held-root publication",
            ))
        }
    }
}

impl RetentionRegistryState {
    pub fn metadata(&self) -> &StoredRetentionMetadata {
        &self.metadata
    }
    pub fn cursor_json(&self) -> &str {
        &self.cursor_json
    }
    pub fn cursor_digest(&self) -> &str {
        &self.cursor_digest
    }
    pub const fn authority(&self) -> RetentionAuthority {
        RetentionAuthority::None
    }
}

#[derive(Clone, Debug)]
struct Cursor {
    sequence: u64,
    checkpoint: String,
    previous: Option<String>,
    plan: String,
    policy: RetentionPolicy,
    json: String,
    digest: String,
}

/// Create generation one from successful store receipts. Both `root` and its
/// `metadata` child must already be private directories selected by the caller.
pub fn initialize(
    root: &Path,
    policy: RetentionPolicy,
    receipts: &[&dyn RetentionReceipt],
) -> Result<RetentionRegistryState> {
    require_receipts(receipts)?;
    RetentionRegistryHandle::open(root)?.initialize(policy, receipts)
}

/// Restore the exact pair selected by this explicit registry's canonical
/// `CURRENT` cursor. Recovery authenticates metadata only and performs no GC.
pub fn recover(root: &Path) -> Result<RetentionRegistryState> {
    RetentionRegistryHandle::open(root)?.recover()
}

/// Compare-and-swap one consecutive generation using the policy fixed by the
/// current authenticated checkpoint. A stale expected cursor fails before a new
/// pair is created. The immutable pair settles before the cursor pivot.
pub fn advance(
    root: &Path,
    expected_cursor_digest: &str,
    receipts: &[&dyn RetentionReceipt],
) -> Result<RetentionRegistryState> {
    validate_digest(expected_cursor_digest)?;
    require_receipts(receipts)?;
    RetentionRegistryHandle::open(root)?.advance(expected_cursor_digest, receipts)
}

fn require_receipts(receipts: &[&dyn RetentionReceipt]) -> Result<()> {
    if receipts.is_empty() {
        return Err(invalid(
            "retention registry generation requires at least one successful store receipt",
        ));
    }
    Ok(())
}

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn settle_transition(
    root: &std::os::fd::OwnedFd,
    transition: &crate::semantic_retention::RetentionTransition,
) -> Result<StoredRetentionMetadata> {
    let checkpoint = transition.checkpoint();
    let load = || {
        semantic_retention_store::load_held(
            root,
            checkpoint.checkpoint_digest(),
            checkpoint.previous_checkpoint_digest(),
            transition.plan_digest(),
        )
    };
    if let Ok(existing) = load() {
        if existing.checkpoint().to_json() != checkpoint.to_json()
            || existing.plan().to_json() != transition.plan().to_json()
        {
            return Err(binding(
                "existing retention metadata pair differs from the derived transition",
            ));
        }
        return Ok(existing);
    }
    semantic_retention_store::persist_held(
        root,
        checkpoint,
        checkpoint.checkpoint_digest(),
        checkpoint.previous_checkpoint_digest(),
        transition.plan(),
        transition.plan_digest(),
    )?;
    load()
}

fn state(metadata: StoredRetentionMetadata, cursor: Cursor) -> RetentionRegistryState {
    RetentionRegistryState {
        metadata,
        cursor_json: cursor.json,
        cursor_digest: cursor.digest,
    }
}

fn validate_metadata(cursor: &Cursor, metadata: &StoredRetentionMetadata) -> Result<()> {
    if metadata.authority() != RetentionAuthority::None
        || metadata.checkpoint().sequence() != cursor.sequence
        || metadata.checkpoint().checkpoint_digest() != cursor.checkpoint
        || metadata.checkpoint().previous_checkpoint_digest() != cursor.previous.as_deref()
        || metadata.checkpoint().policy() != cursor.policy
        || metadata.plan().plan_digest() != cursor.plan
        || metadata.plan().checkpoint_digest() != cursor.checkpoint
        || metadata.plan().sequence() != cursor.sequence
    {
        return Err(binding(
            "retention registry cursor and restored metadata disagree",
        ));
    }
    Ok(())
}

impl Cursor {
    fn new(
        checkpoint: &crate::semantic_retention::RetentionCheckpoint,
        plan: &crate::semantic_retention::RetentionGarbageCollectionPlan,
    ) -> Result<Self> {
        if checkpoint.authority() != RetentionAuthority::None
            || plan.authority() != RetentionAuthority::None
            || plan.checkpoint_digest() != checkpoint.checkpoint_digest()
            || plan.sequence() != checkpoint.sequence()
            || plan.predecessor_checkpoint_digest() != checkpoint.previous_checkpoint_digest()
        {
            return Err(binding(
                "retention registry transition metadata bindings disagree",
            ));
        }
        let mut cursor = Self {
            sequence: checkpoint.sequence(),
            checkpoint: checkpoint.checkpoint_digest().to_owned(),
            previous: checkpoint.previous_checkpoint_digest().map(str::to_owned),
            plan: plan.plan_digest().to_owned(),
            policy: checkpoint.policy(),
            json: String::new(),
            digest: String::new(),
        };
        cursor.json = cursor.render()?;
        cursor.digest = digest(cursor.json.as_bytes());
        Ok(cursor)
    }

    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_RETENTION_REGISTRY_CURSOR_BYTES {
            return Err(capacity(
                "retention registry cursor bytes are empty or exceed 4096",
            ));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("retention registry cursor is not valid JSON"))?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("retention registry cursor must be an object"))?;
        require_keys(
            object,
            &[
                "schema",
                "sequence",
                "checkpoint_digest",
                "previous_checkpoint_digest",
                "plan_digest",
                "policy",
                "authority",
                "nonclaims",
            ],
        )?;
        if value["schema"] != SEMANTIC_RETENTION_REGISTRY_CURSOR_SCHEMA
            || value["authority"] != "none"
            || value["nonclaims"] != json!(NONCLAIMS)
        {
            return Err(binding(
                "retention registry cursor schema or authority boundary differs",
            ));
        }
        let sequence = object["sequence"]
            .as_u64()
            .filter(|value| *value != 0)
            .ok_or_else(|| invalid("retention registry sequence is invalid"))?;
        let checkpoint = digest_field(object, "checkpoint_digest")?.to_owned();
        let plan = digest_field(object, "plan_digest")?.to_owned();
        let previous = match object.get("previous_checkpoint_digest") {
            Some(Value::Null) => None,
            Some(Value::String(value)) => {
                validate_digest(value)?;
                Some(value.clone())
            }
            _ => return Err(invalid("retention registry predecessor is malformed")),
        };
        if (sequence == 1) != previous.is_none() {
            return Err(binding(
                "retention registry predecessor and sequence disagree",
            ));
        }
        let policy = RetentionPolicy::new(
            policy_number(&value["policy"], "max_subjects")?
                .try_into()
                .map_err(|_| capacity("retention registry subject bound exceeds this host"))?,
            policy_number(&value["policy"], "max_bytes")?,
            policy_number(&value["policy"], "protected_generations")?,
        )?;
        let cursor = Self {
            sequence,
            checkpoint,
            previous,
            plan,
            policy,
            json: render(value.clone())?,
            digest: digest(bytes),
        };
        if cursor.json.as_bytes() != bytes || cursor.value() != value {
            return Err(binding(
                "retention registry cursor is not exact canonical JSON",
            ));
        }
        Ok(cursor)
    }

    fn value(&self) -> Value {
        json!({
            "schema":SEMANTIC_RETENTION_REGISTRY_CURSOR_SCHEMA,
            "sequence":self.sequence,
            "checkpoint_digest":self.checkpoint,
            "previous_checkpoint_digest":self.previous,
            "plan_digest":self.plan,
            "policy":{
                "max_subjects":self.policy.max_subjects(),
                "max_bytes":self.policy.max_bytes(),
                "protected_generations":self.policy.protected_generations(),
            },
            "authority":"none",
            "nonclaims":NONCLAIMS,
        })
    }

    fn render(&self) -> Result<String> {
        render(self.value())
    }
}

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn validate_stage_relationship(
    metadata_root: &std::os::fd::OwnedFd,
    current: Option<&[u8]>,
    stage: &[u8],
) -> Result<()> {
    let stage = Cursor::parse(stage)?;
    let metadata = semantic_retention_store::load_held(
        metadata_root,
        &stage.checkpoint,
        stage.previous.as_deref(),
        &stage.plan,
    )?;
    validate_metadata(&stage, &metadata)?;
    let Some(current) = current else {
        if stage.sequence == 1 && stage.previous.is_none() {
            return Ok(());
        }
        return Err(binding(
            "retention registry initial stage is not an exact generation-one cursor",
        ));
    };
    let current = Cursor::parse(current)?;
    let stage_is_next = current
        .sequence
        .checked_add(1)
        .is_some_and(|sequence| sequence == stage.sequence)
        && stage.previous.as_deref() == Some(current.checkpoint.as_str())
        && stage.policy == current.policy;
    let stage_is_predecessor = stage
        .sequence
        .checked_add(1)
        .is_some_and(|sequence| sequence == current.sequence)
        && current.previous.as_deref() == Some(stage.checkpoint.as_str())
        && stage.policy == current.policy;
    if !stage_is_next && !stage_is_predecessor {
        return Err(binding(
            "retention registry stage is neither the consecutive next cursor nor CURRENT's exact predecessor",
        ));
    }
    Ok(())
}

fn policy_number(value: &Value, field: &str) -> Result<u64> {
    let object = value
        .as_object()
        .filter(|object| object.len() == 3)
        .ok_or_else(|| invalid("retention registry policy is malformed"))?;
    if !["max_subjects", "max_bytes", "protected_generations"]
        .iter()
        .all(|key| object.contains_key(*key))
    {
        return Err(invalid("retention registry policy is malformed"));
    }
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("retention registry policy number is malformed"))
}

fn digest_field<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("retention registry digest is missing"))?;
    validate_digest(value)?;
    Ok(value)
}

fn require_keys(object: &Map<String, Value>, keys: &[&str]) -> Result<()> {
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(
            "retention registry cursor has unknown or missing fields",
        ));
    }
    Ok(())
}

fn render(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("retention registry cursor cannot be encoded"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_RETENTION_REGISTRY_CURSOR_BYTES {
        return Err(capacity("retention registry cursor exceeds 4096 bytes"));
    }
    String::from_utf8(bytes).map_err(|_| invalid("retention registry cursor is not UTF-8"))
}

fn digest(bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(CURSOR_DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "retention registry selector is not canonical lowercase SHA-256",
        ));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G464", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G465", message)]
}
fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G466", message)]
}
fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G467", message)]
}
fn io(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G468", message)]
}

#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
fn post_pivot(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G468", message)]
}
