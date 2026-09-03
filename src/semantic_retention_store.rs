//! Explicit immutable persistence for one authenticated retention checkpoint
//! and its exact pending GC plan. Stored metadata cannot apply that plan.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::semantic_retention::{
    restore_checkpoint, restore_plan, RetentionAuthority, RetentionCheckpoint,
    RetentionGarbageCollectionPlan, MAX_RETENTION_CHECKPOINT_BYTES, MAX_RETENTION_PLAN_BYTES,
};

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

pub const MAX_RETENTION_METADATA_STORE_ENTRIES: usize = 32;
pub const MAX_RETENTION_METADATA_STORE_PATH_BYTES: usize = 4096;
pub const MAX_RETENTION_METADATA_STORE_PATH_DEPTH: usize = 64;
pub const RETENTION_METADATA_STORE_COMPATIBILITY: &str =
    "semaprax.semantic-retention-metadata-store.v1";

const MAGIC: &[u8; 8] = b"SPXRET01";
const DIGEST_BYTES: usize = 71;
const LENGTH_BYTES: usize = 8;
const ENVELOPE_OVERHEAD: usize = MAGIC.len() + DIGEST_BYTES * 2 + LENGTH_BYTES * 2;
pub(crate) const MAX_RETENTION_METADATA_ENVELOPE_BYTES: usize =
    ENVELOPE_OVERHEAD + MAX_RETENTION_CHECKPOINT_BYTES + MAX_RETENTION_PLAN_BYTES;
const ENVELOPE_DOMAIN: &[u8] = b"semaprax.semantic-retention-metadata-store.envelope.v1\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Immutable publication receipt. It carries identities and accounting only,
/// never a root, handle, delete capability, or retained-subject authority.
#[derive(Debug)]
pub struct RetentionMetadataStoreReceipt {
    checkpoint: String,
    plan: String,
    envelope: String,
    envelope_bytes: usize,
}

impl RetentionMetadataStoreReceipt {
    pub fn checkpoint_digest(&self) -> &str {
        &self.checkpoint
    }
    pub fn plan_digest(&self) -> &str {
        &self.plan
    }
    pub fn envelope_digest(&self) -> &str {
        &self.envelope
    }
    pub const fn envelope_bytes(&self) -> usize {
        self.envelope_bytes
    }
    pub const fn authority(&self) -> RetentionAuthority {
        RetentionAuthority::None
    }
}

/// Restored canonical metadata. The contained plan remains metadata-only and
/// cannot resolve, delete, approve, or publish any retained subject.
#[derive(Debug)]
pub struct StoredRetentionMetadata {
    checkpoint: RetentionCheckpoint,
    plan: RetentionGarbageCollectionPlan,
}

impl StoredRetentionMetadata {
    pub fn checkpoint(&self) -> &RetentionCheckpoint {
        &self.checkpoint
    }
    pub fn plan(&self) -> &RetentionGarbageCollectionPlan {
        &self.plan
    }
    pub const fn authority(&self) -> RetentionAuthority {
        RetentionAuthority::None
    }
}

/// Publish one already authenticated canonical checkpoint/plan pair through a
/// single immutable no-replace pivot. The explicit root must already exist.
pub fn persist(
    root: &Path,
    checkpoint: &RetentionCheckpoint,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    plan: &RetentionGarbageCollectionPlan,
    expected_plan: &str,
) -> Result<RetentionMetadataStoreReceipt> {
    let (bytes, receipt) = prepare(
        checkpoint,
        expected_checkpoint,
        expected_previous,
        plan,
        expected_plan,
    )?;
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
        unix::persist(root, expected_checkpoint, expected_plan, &bytes)?;
        Ok(receipt)
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
        let _ = (root, bytes, receipt);
        Err(io(
            "retention metadata store requires supported Unix no-replace publication",
        ))
    }
}

/// Load one exact selected pair and invoke the ordinary canonical checkpoint
/// and plan restorers while the file, inventory, lock, and root chain are held.
pub fn load(
    root: &Path,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    expected_plan: &str,
) -> Result<StoredRetentionMetadata> {
    validate_digest(expected_checkpoint)?;
    validate_digest(expected_plan)?;
    if let Some(previous) = expected_previous {
        validate_digest(previous)?;
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
    {
        unix::load(root, expected_checkpoint, expected_plan, |bytes| {
            decode(bytes, expected_checkpoint, expected_previous, expected_plan)
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
            "retention metadata store requires supported Unix held-file input",
        ))
    }
}

/// Registry-only persistence through an already authenticated, held metadata
/// directory. This does not expose a reusable path or handle outside the crate.
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
pub(crate) fn persist_held(
    root: impl std::os::fd::AsFd,
    checkpoint: &RetentionCheckpoint,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    plan: &RetentionGarbageCollectionPlan,
    expected_plan: &str,
) -> Result<RetentionMetadataStoreReceipt> {
    let (bytes, receipt) = prepare(
        checkpoint,
        expected_checkpoint,
        expected_previous,
        plan,
        expected_plan,
    )?;
    unix::persist_held(root, expected_checkpoint, expected_plan, &bytes)?;
    Ok(receipt)
}

/// Registry-only exact restoration through the same held metadata directory.
#[cfg(all(
    unix,
    any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "redox"
    )
))]
pub(crate) fn load_held(
    root: impl std::os::fd::AsFd,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    expected_plan: &str,
) -> Result<StoredRetentionMetadata> {
    validate_digest(expected_checkpoint)?;
    validate_digest(expected_plan)?;
    if let Some(previous) = expected_previous {
        validate_digest(previous)?;
    }
    unix::load_held(root, expected_checkpoint, expected_plan, |bytes| {
        decode(bytes, expected_checkpoint, expected_previous, expected_plan)
    })
}

fn prepare(
    checkpoint: &RetentionCheckpoint,
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    plan: &RetentionGarbageCollectionPlan,
    expected_plan: &str,
) -> Result<(Vec<u8>, RetentionMetadataStoreReceipt)> {
    validate_digest(expected_checkpoint)?;
    validate_digest(expected_plan)?;
    if checkpoint.checkpoint_digest() != expected_checkpoint
        || plan.plan_digest() != expected_plan
        || plan.checkpoint_digest() != expected_checkpoint
    {
        return Err(binding(
            "retention metadata objects disagree with the expected selectors",
        ));
    }
    let restored_checkpoint = restore_checkpoint(
        checkpoint.to_json().as_bytes(),
        expected_checkpoint,
        expected_previous,
    )?;
    let restored_plan = restore_plan(
        plan.to_json().as_bytes(),
        expected_plan,
        &restored_checkpoint,
    )?;
    if restored_checkpoint.authority() != RetentionAuthority::None
        || restored_plan.authority() != RetentionAuthority::None
        || restored_checkpoint.to_json() != checkpoint.to_json()
        || restored_plan.to_json() != plan.to_json()
    {
        return Err(binding(
            "retention metadata ordinary restoration changed the selected pair",
        ));
    }
    let bytes = encode(
        expected_checkpoint,
        expected_plan,
        checkpoint.to_json().as_bytes(),
        plan.to_json().as_bytes(),
    )?;
    let receipt = RetentionMetadataStoreReceipt {
        checkpoint: expected_checkpoint.to_owned(),
        plan: expected_plan.to_owned(),
        envelope: hash(&bytes),
        envelope_bytes: bytes.len(),
    };
    Ok((bytes, receipt))
}

fn encode(
    checkpoint_digest: &str,
    plan_digest: &str,
    checkpoint: &[u8],
    plan: &[u8],
) -> Result<Vec<u8>> {
    if checkpoint.is_empty()
        || checkpoint.len() > MAX_RETENTION_CHECKPOINT_BYTES
        || plan.is_empty()
        || plan.len() > MAX_RETENTION_PLAN_BYTES
    {
        return Err(capacity(
            "retention metadata component bytes are outside their fixed bounds",
        ));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(ENVELOPE_OVERHEAD + checkpoint.len() + plan.len())
        .map_err(|_| capacity("cannot reserve bounded retention metadata envelope"))?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(checkpoint_digest.as_bytes());
    bytes.extend_from_slice(plan_digest.as_bytes());
    bytes.extend_from_slice(&(checkpoint.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&(plan.len() as u64).to_le_bytes());
    bytes.extend_from_slice(checkpoint);
    bytes.extend_from_slice(plan);
    Ok(bytes)
}

fn decode(
    bytes: &[u8],
    expected_checkpoint: &str,
    expected_previous: Option<&str>,
    expected_plan: &str,
) -> Result<StoredRetentionMetadata> {
    if bytes.len() < ENVELOPE_OVERHEAD || bytes.len() > MAX_RETENTION_METADATA_ENVELOPE_BYTES {
        return Err(capacity(
            "retention metadata envelope length is outside its fixed bound",
        ));
    }
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err(invalid("retention metadata envelope magic differs"));
    }
    let checkpoint_start = MAGIC.len();
    let plan_start = checkpoint_start + DIGEST_BYTES;
    let lengths_start = plan_start + DIGEST_BYTES;
    let checkpoint_digest = text(&bytes[checkpoint_start..plan_start])?;
    let plan_digest = text(&bytes[plan_start..lengths_start])?;
    if checkpoint_digest != expected_checkpoint || plan_digest != expected_plan {
        return Err(binding("retention metadata envelope selectors disagree"));
    }
    let checkpoint_len = length(&bytes[lengths_start..lengths_start + LENGTH_BYTES])?;
    let plan_len = length(&bytes[lengths_start + LENGTH_BYTES..lengths_start + LENGTH_BYTES * 2])?;
    if checkpoint_len == 0
        || checkpoint_len > MAX_RETENTION_CHECKPOINT_BYTES
        || plan_len == 0
        || plan_len > MAX_RETENTION_PLAN_BYTES
    {
        return Err(capacity(
            "retention metadata component length is outside its fixed bound",
        ));
    }
    let payload_start = ENVELOPE_OVERHEAD;
    let checkpoint_end = payload_start
        .checked_add(checkpoint_len)
        .ok_or_else(|| capacity("retention metadata checkpoint length overflow"))?;
    let plan_end = checkpoint_end
        .checked_add(plan_len)
        .ok_or_else(|| capacity("retention metadata plan length overflow"))?;
    if plan_end != bytes.len() {
        return Err(binding(
            "retention metadata envelope lengths or trailing bytes disagree",
        ));
    }
    let checkpoint = restore_checkpoint(
        &bytes[payload_start..checkpoint_end],
        expected_checkpoint,
        expected_previous,
    )?;
    let plan = restore_plan(&bytes[checkpoint_end..plan_end], expected_plan, &checkpoint)?;
    Ok(StoredRetentionMetadata { checkpoint, plan })
}

pub(crate) fn pair_name(checkpoint: &str, plan: &str) -> Result<String> {
    Ok(format!(
        "{}-{}.spxr",
        digest_hex(checkpoint)?,
        digest_hex(plan)?
    ))
}

fn text(bytes: &[u8]) -> Result<&str> {
    let value = std::str::from_utf8(bytes)
        .map_err(|_| invalid("retention metadata selector is not UTF-8"))?;
    validate_digest(value)?;
    Ok(value)
}

fn length(bytes: &[u8]) -> Result<usize> {
    let array: [u8; LENGTH_BYTES] = bytes
        .try_into()
        .map_err(|_| invalid("retention metadata length field is malformed"))?;
    usize::try_from(u64::from_le_bytes(array))
        .map_err(|_| capacity("retention metadata length exceeds this host"))
}

fn validate_digest(value: &str) -> Result<()> {
    digest_hex(value).map(|_| ())
}

fn digest_hex(value: &str) -> Result<&str> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| invalid("retention metadata selector requires sha256 syntax"))?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "retention metadata selector is not canonical lowercase SHA-256",
        ));
    }
    Ok(hex)
}

fn hash(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(ENVELOPE_DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G427", message)]
}
fn capacity(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G428", message)]
}
fn binding(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G429", message)]
}
fn io(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-I370", message)]
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
    vec![Diagnostic::io("SPX-I371", message)]
}
