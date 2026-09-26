//! The first non-test caller for [`OutboundDeliveryStore`]: a bounded,
//! host-authorized wiring from a decoded service outbound declaration to a
//! real durable typed delivery session.
//!
//! Configuration is untrusted intent. It may name which durable store a
//! service invocation expects, but that name is data, never authority: the
//! host must independently grant a directory and select the same store
//! identity before any store, session, or adapter is constructed. A missing
//! or mismatched host grant is refused before any filesystem or network
//! effect ([`bind_service_outbound_store`]).
//!
//! [`deliver_http_durable`] is the runnable delivery entry point. On a fresh
//! (non-restored) attempt it additionally probes the injected store for an
//! already-committed provisional checkpoint under the exact identity and
//! request before ever entering the adapter. This closes a gap the typed
//! session alone does not: a caller that restarts with a fresh in-memory
//! session after a crash between the provisional intent commit and the
//! terminal commit would otherwise redispatch, because a brand-new session's
//! in-memory ledger has no record of the earlier attempt. Finding that exact
//! provisional checkpoint already on disk is conservatively surfaced as
//! [`ServiceHttpDeliveryOutcome::Uncertain`] and never enters the adapter.

use semaprax::outbound_host_adapter::{
    prepare_http_delivery, DurableHttpDeliveryOutcome, DurableHttpLedgerRefusal,
    HttpDeliveryReceipt, HttpDeliverySession, HttpDeliverySessionCheckpoint,
    HttpDeliverySessionCheckpointStore, HttpDeliverySessionRestoreCapability,
    HttpDeliverySessionRestoreRefusal, HttpLedgerRefusal, HttpRequest, OutboundAdapter,
    OutboundCapability,
};
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
/// directory is allowed to serve. Configuration cannot construct this.
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
    /// Configuration named a store identity the host did not grant.
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
    Replayed(HttpDeliveryReceipt),
    /// Either the store commit itself was ambiguous, or a fresh attempt found
    /// an already-committed provisional checkpoint for this exact identity
    /// and request. Neither case enters the adapter.
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
/// redispatches an exact identity and request whose provisional checkpoint is
/// already durably present.
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

/// Wraps the real store and, on the first (provisional-intent) commit of a
/// fresh session only, checks whether that exact checkpoint is already
/// durably present. Finding it there means an earlier attempt for this exact
/// identity and request got at least as far as committing its intent; this
/// guard then reports the commit itself as uncertain so the ledger never
/// proceeds to the adapter, rather than acknowledging the (byte-identical)
/// intent again and redispatching.
struct ProvisionalProbeStore<'a, 'directory> {
    inner: &'a mut OutboundDeliveryStore<'directory>,
    probed: bool,
}

impl HttpDeliverySessionCheckpointStore for ProvisionalProbeStore<'_, '_> {
    fn commit(&mut self, checkpoint: &HttpDeliverySessionCheckpoint) -> CheckpointCommit {
        if !self.probed {
            self.probed = true;
            if self
                .inner
                .load(OutboundCheckpointKind::HttpSession, &checkpoint.digest())
                .is_ok()
            {
                return CheckpointCommit::Uncertain;
            }
        }
        HttpDeliverySessionCheckpointStore::commit(self.inner, checkpoint)
    }
}

#[cfg(test)]
#[path = "service_invocation/tests.rs"]
mod tests;
