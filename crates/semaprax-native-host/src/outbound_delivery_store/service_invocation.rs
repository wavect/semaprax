//! A bounded, host-authorized library entry point wiring a decoded service
//! outbound declaration to a real durable HTTP delivery session backed by
//! [`OutboundDeliveryStore`].
//!
//! This module has no caller yet in a runnable application or service policy
//! package (no CLI command, scaffold template, or host runtime invokes it):
//! it is a bounded primitive such a caller could use, not evidence that one
//! does. Treat it as a library entry point, not as "the service is wired."
//!
//! Configuration is untrusted intent. It may name which durable store a
//! service invocation expects, but that name is data, never authority: the
//! host must independently grant a directory and select the same store
//! identity before any store, session, or adapter is constructed. A missing
//! or mismatched host grant is refused before any filesystem or network
//! effect ([`bind_service_outbound_store`]). The `HeldDirectory` the host
//! grants is the actual authority; the store-id match is only a consistency
//! check that decoded configuration cannot silently redirect a host-granted
//! directory to a different declared purpose.
//!
//! [`deliver_http_durable`] is the runnable delivery entry point. Whenever the
//! ledger's own in-memory replay check does not already short-circuit --
//! which covers both a fresh (non-restored) session and a restored session
//! that meets a new identity its checkpoint never recorded -- it commits a
//! [`PendingIntentCommit`] marker, named only by
//! [`PreparedHttpDelivery::pending_identity_key`], before ever entering the
//! adapter. That key depends only on the deployment binding, invocation id,
//! and idempotency key: never on capacity, policy, or session-restoration
//! state. This closes a gap the typed session checkpoint alone does not: its
//! own digest also depends on capacity and every in-session commitment, so a
//! restart using a different capacity, a changed policy, or simply a fresh
//! (non-restored) session for an identity that was already restored
//! elsewhere would compute a different digest, find nothing on disk under
//! that digest, and redispatch with the same idempotency key. The marker
//! commit is a single atomic create-new attempt with no preceding read: only
//! a fresh create permits dispatch, and an existing marker -- or any error
//! while creating, syncing, or rechecking one -- is `Uncertain` and never
//! enters the adapter. This also means a marker that is durably committed but
//! then never resolved (a crash before the adapter is even entered) leaves
//! that identity permanently `Uncertain`; a host must select a new
//! invocation identity to retry, exactly as the lower ledger already
//! documents for its own sticky dispositions.
//!
//! A `Replayed` outcome may itself carry an `Uncertain` disposition: replay
//! only proves the exact request was reconciled before, not that it settled.
//!
//! The marker's own durability follows the bound store's sync mode:
//! `NamespaceSynced` is required for the no-redispatch guarantee to survive
//! more than a process crash (for example, a real power loss); the default
//! `FileOnly` mode closes only the narrower process-crash window this
//! module's tests exercise.

use std::ffi::OsStr;

use semaprax::outbound_host_adapter::{
    prepare_http_delivery, DurableHttpDeliveryOutcome, DurableHttpLedgerRefusal,
    HttpDeliveryReceipt, HttpDeliverySession, HttpDeliverySessionCheckpoint,
    HttpDeliverySessionCheckpointStore, HttpDeliverySessionRestoreCapability,
    HttpDeliverySessionRestoreRefusal, HttpLedgerRefusal, HttpRequest, OutboundAdapter,
    OutboundCapability,
};
use semaprax_native_rust_interop_platform as platform;
use semaprax_native_rust_interop_platform::HeldDirectory;

use super::{
    CheckpointCommit, OutboundCheckpointKind, OutboundCheckpointSyncMode, OutboundDeliveryStore,
};

pub const SERVICE_OUTBOUND_CONFIG_SCHEMA: &str = "semaprax.native-host.service-outbound-config.v1";
const MAX_SERVICE_OUTBOUND_CONFIG_BYTES: usize = 4_096;
const MAX_STORE_ID_BYTES: usize = 64;
const CONFIG_PREFIX: &str =
    "{\"schema\":\"semaprax.native-host.service-outbound-config.v1\",\"store\":\"";
const CONFIG_SUFFIX: &str = "\"}";

/// Untrusted, decoded service intent. Naming a store here mints nothing: it
/// is only ever compared against a separately host-granted identity in
/// [`bind_service_outbound_store`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceOutboundConfig {
    store_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceOutboundConfigRefusal {
    TooLarge,
    Malformed,
}

impl ServiceOutboundConfig {
    /// Decode a single canonical byte spelling. There is exactly one valid
    /// encoding for a given store id; anything else, including reordered or
    /// additional fields, is refused rather than loosely parsed.
    pub fn decode(bytes: &[u8]) -> Result<Self, ServiceOutboundConfigRefusal> {
        if bytes.is_empty() || bytes.len() > MAX_SERVICE_OUTBOUND_CONFIG_BYTES {
            return Err(ServiceOutboundConfigRefusal::TooLarge);
        }
        let text =
            std::str::from_utf8(bytes).map_err(|_| ServiceOutboundConfigRefusal::Malformed)?;
        let middle = text
            .strip_prefix(CONFIG_PREFIX)
            .and_then(|rest| rest.strip_suffix(CONFIG_SUFFIX))
            .ok_or(ServiceOutboundConfigRefusal::Malformed)?;
        if middle.is_empty()
            || middle.len() > MAX_STORE_ID_BYTES
            || !middle
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(ServiceOutboundConfigRefusal::Malformed);
        }
        Ok(Self {
            store_id: middle.to_owned(),
        })
    }

    pub fn store_id(&self) -> &str {
        &self.store_id
    }
}

/// Host-only authority naming the exact store identity a caller-held
/// directory is allowed to serve. Configuration cannot construct this. The
/// directory itself remains the real authority; `store_id` is a consistency
/// label the host also controls, not an independent credential.
pub struct ServiceOutboundStoreGrant<'directory> {
    store_id: &'static str,
    directory: &'directory HeldDirectory,
    sync_mode: OutboundCheckpointSyncMode,
}

impl<'directory> ServiceOutboundStoreGrant<'directory> {
    /// Bind a store identity to a directory the host already holds. This
    /// performs no I/O and reads no configuration.
    pub fn from_trusted_host(
        store_id: &'static str,
        directory: &'directory HeldDirectory,
        sync_mode: OutboundCheckpointSyncMode,
    ) -> Self {
        Self {
            store_id,
            directory,
            sync_mode,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceOutboundBindingRefusal {
    /// The host granted no store for this invocation at all.
    NoHostGrant,
    /// Configuration named a store identity the host did not grant. The
    /// host's `HeldDirectory` may be perfectly valid; only the declared
    /// purpose failed this consistency check.
    AuthorityDenied,
}

/// Bind a decoded configuration to a real durable store only when the host
/// independently grants the exact store identity configuration names. A
/// config that names a store the host never granted -- including when the
/// host granted none -- is refused before any store is constructed.
pub fn bind_service_outbound_store<'directory>(
    config: &ServiceOutboundConfig,
    grant: Option<&ServiceOutboundStoreGrant<'directory>>,
) -> Result<OutboundDeliveryStore<'directory>, ServiceOutboundBindingRefusal> {
    let grant = grant.ok_or(ServiceOutboundBindingRefusal::NoHostGrant)?;
    if grant.store_id != config.store_id() {
        return Err(ServiceOutboundBindingRefusal::AuthorityDenied);
    }
    Ok(OutboundDeliveryStore::with_sync_mode(
        grant.directory,
        grant.sync_mode,
    ))
}

/// A digest and capacity the host itself durably retained after an earlier
/// completed invocation of the exact same identity and request. This is
/// evidence the caller already holds, not authority minted by this module.
pub struct PriorTerminalReference<'a> {
    pub digest: &'a str,
    pub capacity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceHttpDeliveryOutcome {
    Dispatched(HttpDeliveryReceipt),
    /// The exact identity and request reconciled before. The carried receipt
    /// may itself hold an `Uncertain` disposition: replay proves only that
    /// this request was already reconciled, not that it settled.
    Replayed(HttpDeliveryReceipt),
    /// Either the store commit itself was ambiguous, or an in-flight or
    /// already-settled attempt for this exact identity was found durably
    /// present before dispatch. Neither case enters the adapter. A durably
    /// committed marker is never cleared: if the adapter was never actually
    /// reached (a crash between committing the marker and entering it), this
    /// identity is permanently `Uncertain`, and an operator must select a
    /// new invocation identity to retry.
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceHttpDeliveryRefusal {
    InvalidRequest,
    LoadFailed,
    Session(HttpLedgerRefusal),
    Restore(HttpDeliverySessionRestoreRefusal),
    Durable(DurableHttpLedgerRefusal),
}

/// Perform one durable HTTP service delivery.
///
/// `prior_terminal` is `None` for a first attempt, or when the caller has no
/// independently retained terminal reference (including immediately after a
/// crash before one was ever produced). It is never derived from decoded
/// configuration. Passing `Some` restores the exact prior session and lets an
/// unchanged request replay without redispatch; passing `None` still never
/// redispatches an exact identity whose durable marker is already present,
/// regardless of this call's capacity or policy.
pub fn deliver_http_durable(
    store: &mut OutboundDeliveryStore<'_>,
    capacity: usize,
    prior_terminal: Option<PriorTerminalReference<'_>>,
    capability: OutboundCapability,
    request: HttpRequest,
    adapter: &mut impl OutboundAdapter,
) -> Result<ServiceHttpDeliveryOutcome, ServiceHttpDeliveryRefusal> {
    let prepared = prepare_http_delivery(capability, request)
        .map_err(|_| ServiceHttpDeliveryRefusal::InvalidRequest)?;
    let identity_key = prepared.pending_identity_key().to_owned();

    let mut session = match prior_terminal {
        Some(prior) => {
            let restore_capability = HttpDeliverySessionRestoreCapability::grant_for_trusted_host(
                prior.digest,
                prior.capacity,
            )
            .map_err(ServiceHttpDeliveryRefusal::Restore)?;
            let bytes = store
                .load(OutboundCheckpointKind::HttpSession, prior.digest)
                .map_err(|_| ServiceHttpDeliveryRefusal::LoadFailed)?;
            HttpDeliverySession::restore_authenticated(&bytes, restore_capability)
                .map_err(ServiceHttpDeliveryRefusal::Restore)?
        }
        None => HttpDeliverySession::new(capacity).map_err(ServiceHttpDeliveryRefusal::Session)?,
    };

    let mut guard = ProvisionalProbeStore {
        inner: store,
        identity_key,
        probed: false,
    };
    let outcome = session
        .reconcile_durable(prepared, &mut guard, adapter)
        .map_err(ServiceHttpDeliveryRefusal::Durable)?;
    Ok(match outcome {
        DurableHttpDeliveryOutcome::Dispatched(receipt) => {
            ServiceHttpDeliveryOutcome::Dispatched(receipt)
        }
        DurableHttpDeliveryOutcome::Replayed(receipt) => {
            ServiceHttpDeliveryOutcome::Replayed(receipt)
        }
        DurableHttpDeliveryOutcome::IntentNotCommitted
        | DurableHttpDeliveryOutcome::IntentUncertain(_)
        | DurableHttpDeliveryOutcome::SettlementUncertain(_) => {
            ServiceHttpDeliveryOutcome::Uncertain
        }
    })
}

/// The outcome of [`commit_pending_intent_marker`]. Unlike the typed
/// checkpoint store's own idempotent-existing-content acknowledgment, an
/// existing marker is never inspected or treated as a successful repeat: only
/// a genuinely fresh create is `Fresh`. This is what makes the marker safe
/// against two concurrent fresh workers racing the same identity: at most
/// one create-new can win, and the loser -- even though it would compute
/// byte-identical content -- gets `Blocked` without ever reading what is
/// already there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingIntentCommit {
    Fresh,
    Blocked,
}

const MAX_PENDING_INTENT_BYTES: usize = 128;
const PENDING_INTENT_BYTES: &[u8] = b"semaprax.outbound.pending-intent.v1\n";

fn pending_intent_filename(kind: OutboundCheckpointKind, identity_key: &str) -> Option<String> {
    let hex = identity_key.strip_prefix("sha256:")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some(format!("outbound-{}-intent-{hex}.marker", kind.label()))
}

/// Atomically commit a create-new-only marker naming one identity, entirely
/// independent of capacity, policy, or any session-restoration state. A
/// caller must not proceed to dispatch unless this returns `Fresh`.
pub(crate) fn commit_pending_intent_marker(
    directory: &HeldDirectory,
    sync_mode: OutboundCheckpointSyncMode,
    kind: OutboundCheckpointKind,
    identity_key: &str,
) -> PendingIntentCommit {
    let Some(name) = pending_intent_filename(kind, identity_key) else {
        return PendingIntentCommit::Blocked;
    };
    if platform::recheck_directory(directory).is_err() {
        return PendingIntentCommit::Blocked;
    }
    // No preceding read: the create-new attempt itself is the single atomic
    // decision point. `Err(Exists)` -- from this attempt or a concurrent
    // racer's -- is unconditionally `Blocked`, never compared or forgiven.
    if platform::write_file_new(directory, OsStr::new(&name), PENDING_INTENT_BYTES, 0o600).is_err()
    {
        return PendingIntentCommit::Blocked;
    }
    // Bind the ACK to the current namespace entry, matching the typed
    // checkpoint store's own reopen-by-name discipline, rather than trusting
    // the writer's descriptor.
    let Ok(existing) = platform::hold_regular_file_bounded_for_sync(
        directory,
        OsStr::new(&name),
        MAX_PENDING_INTENT_BYTES,
    ) else {
        return PendingIntentCommit::Blocked;
    };
    if platform::sync_regular_file(&existing).is_err() {
        return PendingIntentCommit::Blocked;
    }
    if sync_mode == OutboundCheckpointSyncMode::NamespaceSynced
        && platform::sync_directory(directory).is_err()
    {
        return PendingIntentCommit::Blocked;
    }
    if platform::recheck_regular_file_named_bounded(
        directory,
        OsStr::new(&name),
        &existing,
        MAX_PENDING_INTENT_BYTES,
    )
    .is_err()
    {
        return PendingIntentCommit::Blocked;
    }
    PendingIntentCommit::Fresh
}

/// Wraps the real store. On the first commit of a session that reaches the
/// store at all -- a fresh session, or a restored one meeting an identity its
/// checkpoint never recorded -- it commits an identity-keyed
/// [`PendingIntentCommit`] marker before delegating. Only `Fresh` lets the
/// ledger's own provisional-intent commit (and, later, the adapter) proceed.
struct ProvisionalProbeStore<'a, 'directory> {
    inner: &'a mut OutboundDeliveryStore<'directory>,
    identity_key: String,
    probed: bool,
}

impl HttpDeliverySessionCheckpointStore for ProvisionalProbeStore<'_, '_> {
    fn commit(&mut self, checkpoint: &HttpDeliverySessionCheckpoint) -> CheckpointCommit {
        if !self.probed {
            self.probed = true;
            let marker = commit_pending_intent_marker(
                self.inner.directory(),
                self.inner.sync_mode(),
                OutboundCheckpointKind::HttpSession,
                &self.identity_key,
            );
            if marker != PendingIntentCommit::Fresh {
                return CheckpointCommit::Uncertain;
            }
        }
        HttpDeliverySessionCheckpointStore::commit(self.inner, checkpoint)
    }
}

#[cfg(test)]
#[path = "service_invocation/tests.rs"]
mod tests;
