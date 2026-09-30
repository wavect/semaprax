//! Bounded high-level HTTPS requests with process-local idempotency replay.
//!
//! This module owns request admission and settlement only. It grants no DNS,
//! socket, credential, retry, redirect, or persistence authority. The injected
//! adapter remains the sole physical transport capability.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest as _, Sha256};

use super::ledger::delivery_session::{
    DeliverySessionCheckpoint, DeliverySessionCheckpointRefusal, DeliverySessionCheckpointStore,
    DeliverySessionCommitment, TypedDeliveryCheckpointStore,
};
use super::*;

const RESERVED_HEADERS: [&str; 5] = [
    "content-type",
    "idempotency-key",
    "traceparent",
    "tracestate",
    "x-semaprax-delivery-id",
];
// Names with an `x-` prefix are not protected-name aliases, but are still
// explicit credential channels at this caller-controlled boundary.
const EXPLICIT_CREDENTIAL_HEADERS: [&str; 1] = ["x-api-key"];

/// One public caller-selected header. Credential-bearing header names are not
/// admitted; a trusted provider adapter may add credentials outside this
/// request value without exposing them to source or evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct HttpHeader {
    name: String,
    value: String,
}

impl HttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, Refusal> {
        let name = name.into();
        let value = value.into();
        if name != name.to_ascii_lowercase()
            || !valid_header_name(&name)
            || value.len() > MAX_HEADER_VALUE_BYTES
            || contains_control(&value)
            || RESERVED_HEADERS.contains(&name.as_str())
            || credential_header_name(&name)
        {
            return Err(Refusal::InvalidHeader);
        }
        Ok(Self { name, value })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Debug for HttpHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpHeader")
            .field("name", &self.name)
            .field("value", &"[REDACTED]")
            .finish()
    }
}

/// One bounded high-level HTTPS request. Redirects and transport retries are
/// always disabled. `idempotency_key` identifies process-local reconciliation;
/// it does not assert that a remote service implements exactly-once delivery.
pub struct HttpRequest {
    pub method: HttpMethod,
    pub endpoint: String,
    pub request_id: String,
    pub idempotency_key: String,
    pub content_type: Option<String>,
    pub headers: Vec<HttpHeader>,
    pub body: Vec<u8>,
    pub deadline_ms: u64,
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("endpoint_origin", &canonical_origin(&self.endpoint))
            .field("request_id", &"[REDACTED]")
            .field("idempotency_key", &"[REDACTED]")
            .field(
                "header_names",
                &self
                    .headers
                    .iter()
                    .map(HttpHeader::name)
                    .collect::<Vec<_>>(),
            )
            .field("body_bytes", &self.body.len())
            .field("deadline_ms", &self.deadline_ms)
            .finish()
    }
}

pub struct PreparedHttpDelivery {
    capability: OutboundCapability,
    origin: String,
    request_id: String,
    idempotency_key: String,
    identity: DeliveryIdentity,
    session_identity_digest: String,
    policy_digest: String,
    request: PreparedRequest,
}

impl fmt::Debug for PreparedHttpDelivery {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedHttpDelivery")
            .field("origin", &self.origin)
            .field("request", &self.request)
            .field("bindings", &"[REDACTED]")
            .finish()
    }
}

impl PreparedHttpDelivery {
    /// A stable identity marker independent of policy, capacity, or session
    /// restoration state: unlike the full typed session-checkpoint digest, it
    /// depends only on the deployment binding, invocation id, and idempotency
    /// key. A trusted host may use it to maintain its own atomic "an attempt
    /// for this exact identity already reached the durable store" marker
    /// before any session or capacity-bearing state exists, so a fresh
    /// session built with a different capacity or policy -- or simply not
    /// restored at all -- cannot bypass a durable in-flight or settled
    /// attempt for the same deployment/invocation/idempotency-key triple.
    /// This is not a capability or evidence; it is a stable name only.
    pub fn pending_identity_key(&self) -> &str {
        &self.session_identity_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpDeliveryReceipt {
    evidence: DeliveryEvidence,
    replayed: bool,
}

impl HttpDeliveryReceipt {
    pub fn evidence(&self) -> &DeliveryEvidence {
        &self.evidence
    }

    pub fn was_replayed(&self) -> bool {
        self.replayed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpLedgerRefusal {
    Ledger(LedgerRefusal),
    PolicyChanged,
    ReplayBindingUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpDeliverySessionCheckpoint {
    inner: DeliverySessionCheckpoint,
}

impl HttpDeliverySessionCheckpoint {
    pub fn render(&self) -> String {
        self.inner.render()
    }

    pub fn digest(&self) -> String {
        self.inner.digest()
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

pub trait HttpDeliverySessionCheckpointStore {
    fn commit(&mut self, checkpoint: &HttpDeliverySessionCheckpoint) -> CheckpointCommit;
}

pub struct HttpDeliverySessionRestoreCapability {
    expected_digest: String,
    expected_capacity: usize,
}

impl HttpDeliverySessionRestoreCapability {
    pub fn grant_for_trusted_host(
        expected_digest: impl Into<String>,
        expected_capacity: usize,
    ) -> Result<Self, HttpDeliverySessionRestoreRefusal> {
        let expected_digest = expected_digest.into();
        if !valid_sha256(&expected_digest)
            || expected_capacity == 0
            || expected_capacity > MAX_LEDGER_ENTRIES
        {
            return Err(HttpDeliverySessionRestoreRefusal::InvalidCapability);
        }
        Ok(Self {
            expected_digest,
            expected_capacity,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpDeliverySessionRestoreRefusal {
    InvalidCapability,
    Checkpoint(DeliverySessionCheckpointRefusal),
    CapacityMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableHttpDeliveryOutcome {
    Dispatched(HttpDeliveryReceipt),
    Replayed(HttpDeliveryReceipt),
    IntentNotCommitted,
    IntentUncertain(HttpDeliveryReceipt),
    SettlementUncertain(HttpDeliveryReceipt),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableHttpLedgerRefusal {
    Session(HttpLedgerRefusal),
    Durable(DurableLedgerRefusal),
}

impl From<LedgerRefusal> for HttpLedgerRefusal {
    fn from(value: LedgerRefusal) -> Self {
        Self::Ledger(value)
    }
}

/// Process-local disposition reconciliation for high-level HTTPS requests.
/// Provider response bytes are deliberately not stored or replayed.
pub struct HttpDeliverySession {
    ledger: HostDeliveryLedger,
    policy_commitments: BTreeMap<String, DeliverySessionCommitment>,
}

impl HttpDeliverySession {
    pub fn new(capacity: usize) -> Result<Self, HttpLedgerRefusal> {
        Ok(Self {
            ledger: HostDeliveryLedger::new(capacity)?,
            policy_commitments: BTreeMap::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.ledger.len()
    }

    pub fn checkpoint(&self) -> Result<LedgerCheckpoint, LedgerCheckpointRefusal> {
        self.ledger.checkpoint()
    }

    pub fn session_checkpoint(
        &self,
    ) -> Result<HttpDeliverySessionCheckpoint, DeliverySessionCheckpointRefusal> {
        Ok(HttpDeliverySessionCheckpoint {
            inner: DeliverySessionCheckpoint::from_session(
                "http",
                &self.ledger,
                &self.policy_commitments,
            )?,
        })
    }

    pub fn restore_authenticated(
        bytes: &[u8],
        capability: HttpDeliverySessionRestoreCapability,
    ) -> Result<Self, HttpDeliverySessionRestoreRefusal> {
        let checkpoint =
            DeliverySessionCheckpoint::decode(bytes, &capability.expected_digest, "http")
                .map_err(HttpDeliverySessionRestoreRefusal::Checkpoint)?;
        if checkpoint.capacity() != capability.expected_capacity {
            return Err(HttpDeliverySessionRestoreRefusal::CapacityMismatch);
        }
        let (ledger, policy_commitments) = checkpoint
            .restore()
            .map_err(HttpDeliverySessionRestoreRefusal::Checkpoint)?;
        Ok(Self {
            ledger,
            policy_commitments,
        })
    }

    pub fn verify_checkpoint(
        &self,
        checkpoint: &LedgerCheckpoint,
    ) -> Result<(), LedgerCheckpointRefusal> {
        checkpoint.verify_against(&self.ledger)
    }

    pub fn reconcile(
        &mut self,
        prepared: PreparedHttpDelivery,
        adapter: &mut impl OutboundAdapter,
    ) -> Result<HttpDeliveryReceipt, HttpLedgerRefusal> {
        if let Some(existing) = self
            .policy_commitments
            .get(&prepared.session_identity_digest)
        {
            if existing.policy != prepared.policy_digest {
                return Err(HttpLedgerRefusal::PolicyChanged);
            }
        }
        let inserted_commitment = if self
            .policy_commitments
            .contains_key(&prepared.session_identity_digest)
        {
            false
        } else {
            self.policy_commitments.insert(
                prepared.session_identity_digest.clone(),
                DeliverySessionCommitment {
                    policy: prepared.policy_digest.clone(),
                    request: request_digest(&prepared.request),
                },
            );
            true
        };
        let outcome =
            match self
                .ledger
                .reconcile(prepared.identity.clone(), &prepared.request, |request| {
                    let observation = adapter.send(request);
                    settlement_disposition(request, &observation)
                }) {
                Ok(outcome) => outcome,
                Err(refusal) => {
                    if inserted_commitment {
                        self.policy_commitments
                            .remove(&prepared.session_identity_digest);
                    }
                    return Err(refusal.into());
                }
            };
        let replayed = !outcome.was_dispatched();
        if replayed
            && !self
                .policy_commitments
                .contains_key(&prepared.session_identity_digest)
        {
            return Err(HttpLedgerRefusal::ReplayBindingUnavailable);
        }
        let disposition = outcome.record().disposition().clone();
        Ok(HttpDeliveryReceipt {
            evidence: delivery_evidence(
                prepared.capability,
                prepared.origin,
                prepared.request_id,
                prepared.idempotency_key,
                prepared.request,
                disposition,
            ),
            replayed,
        })
    }

    pub fn reconcile_durable(
        &mut self,
        prepared: PreparedHttpDelivery,
        store: &mut impl HttpDeliverySessionCheckpointStore,
        adapter: &mut impl OutboundAdapter,
    ) -> Result<DurableHttpDeliveryOutcome, DurableHttpLedgerRefusal> {
        if policy_digest(&prepared.capability.policy) != prepared.policy_digest {
            return Err(DurableHttpLedgerRefusal::Session(
                HttpLedgerRefusal::PolicyChanged,
            ));
        }
        if let Some(existing) = self
            .policy_commitments
            .get(&prepared.session_identity_digest)
        {
            if existing.policy != prepared.policy_digest {
                return Err(DurableHttpLedgerRefusal::Session(
                    HttpLedgerRefusal::PolicyChanged,
                ));
            }
        }
        let inserted = if self
            .policy_commitments
            .contains_key(&prepared.session_identity_digest)
        {
            false
        } else {
            self.policy_commitments.insert(
                prepared.session_identity_digest.clone(),
                DeliverySessionCommitment {
                    policy: prepared.policy_digest.clone(),
                    request: request_digest(&prepared.request),
                },
            );
            true
        };
        let mut wrapper = HttpTypedCheckpointStore { store };
        let mut typed_store = TypedDeliveryCheckpointStore {
            store: &mut wrapper,
            kind: "http",
            commitments: &self.policy_commitments,
        };
        let outcome = self.ledger.reconcile_durable(
            prepared.identity.clone(),
            &prepared.request,
            &mut typed_store,
            |request| {
                let observation = adapter.send(request);
                settlement_disposition(request, &observation)
            },
        );
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                if inserted {
                    self.policy_commitments
                        .remove(&prepared.session_identity_digest);
                }
                return Err(DurableHttpLedgerRefusal::Durable(error));
            }
        };
        if matches!(outcome, DurableLedgerOutcome::IntentNotCommitted) && inserted {
            self.policy_commitments
                .remove(&prepared.session_identity_digest);
        }
        Ok(match outcome {
            DurableLedgerOutcome::Dispatched(record) => DurableHttpDeliveryOutcome::Dispatched(
                durable_http_receipt(prepared, record, false),
            ),
            DurableLedgerOutcome::Replayed(record) => {
                DurableHttpDeliveryOutcome::Replayed(durable_http_receipt(prepared, record, true))
            }
            DurableLedgerOutcome::IntentNotCommitted => {
                DurableHttpDeliveryOutcome::IntentNotCommitted
            }
            DurableLedgerOutcome::IntentUncertain(record) => {
                DurableHttpDeliveryOutcome::IntentUncertain(durable_http_receipt(
                    prepared, record, false,
                ))
            }
            DurableLedgerOutcome::SettlementUncertain(record) => {
                DurableHttpDeliveryOutcome::SettlementUncertain(durable_http_receipt(
                    prepared, record, false,
                ))
            }
        })
    }
}

struct HttpTypedCheckpointStore<'a, Store> {
    store: &'a mut Store,
}

impl<Store: HttpDeliverySessionCheckpointStore> DeliverySessionCheckpointStore
    for HttpTypedCheckpointStore<'_, Store>
{
    fn commit(&mut self, checkpoint: &DeliverySessionCheckpoint) -> CheckpointCommit {
        self.store.commit(&HttpDeliverySessionCheckpoint {
            inner: checkpoint.clone(),
        })
    }
}

fn durable_http_receipt(
    prepared: PreparedHttpDelivery,
    record: LedgerRecord,
    replayed: bool,
) -> HttpDeliveryReceipt {
    HttpDeliveryReceipt {
        evidence: delivery_evidence(
            prepared.capability,
            prepared.origin,
            prepared.request_id,
            prepared.idempotency_key,
            prepared.request,
            record.disposition().clone(),
        ),
        replayed,
    }
}

/// Validate the complete request before it can reach a transport.
pub fn prepare_http_delivery(
    capability: OutboundCapability,
    request: HttpRequest,
) -> Result<PreparedHttpDelivery, Refusal> {
    prepare_http_delivery_with_trace_context(capability, request, None, None)
}

pub(super) fn prepare_http_delivery_with_trace_context(
    capability: OutboundCapability,
    request: HttpRequest,
    trace_context: Option<&TraceContext>,
    tracestate: Option<&TraceState>,
) -> Result<PreparedHttpDelivery, Refusal> {
    let origin = validate_http(
        &capability.policy,
        &request,
        trace_context.is_some(),
        tracestate.is_some(),
    )?;
    let identity = DeliveryIdentity::new(
        capability.deployment_binding.clone(),
        capability.invocation_id.clone(),
        request.idempotency_key.clone(),
    )
    .map_err(|_| Refusal::InvalidIdentity)?;
    let session_identity_digest = session_identity_digest(
        &capability.deployment_binding,
        &capability.invocation_id,
        &request.idempotency_key,
    );
    let policy_digest = policy_digest(&capability.policy);
    let request_id = request.request_id.clone();
    let idempotency_key = request.idempotency_key.clone();
    let max_response_bytes = capability.policy.max_response_bytes;
    let prepared = into_prepared(request, max_response_bytes, trace_context, tracestate);
    Ok(PreparedHttpDelivery {
        capability,
        origin,
        request_id,
        idempotency_key,
        identity,
        session_identity_digest,
        policy_digest,
        request: prepared,
    })
}

/// Validate and attempt exactly one request. The adapter contract disables
/// redirects and retries; any failure after entry is settled as uncertain.
pub fn deliver_http(
    capability: OutboundCapability,
    request: HttpRequest,
    adapter: &mut impl OutboundAdapter,
) -> Result<DeliveryResult, Refusal> {
    deliver_http_with_trace_context(capability, request, None, None, adapter)
}

pub(super) fn deliver_http_with_trace_context(
    capability: OutboundCapability,
    request: HttpRequest,
    trace_context: Option<&TraceContext>,
    tracestate: Option<&TraceState>,
    adapter: &mut impl OutboundAdapter,
) -> Result<DeliveryResult, Refusal> {
    let origin = validate_http(
        &capability.policy,
        &request,
        trace_context.is_some(),
        tracestate.is_some(),
    )?;
    let request_id = request.request_id.clone();
    let idempotency_key = request.idempotency_key.clone();
    let max_response_bytes = capability.policy.max_response_bytes;
    let prepared = into_prepared(request, max_response_bytes, trace_context, tracestate);
    let observation = adapter.send(&prepared);
    Ok(settle(
        capability,
        origin,
        request_id,
        idempotency_key,
        prepared,
        observation,
    ))
}

fn validate_http(
    policy: &OutboundPolicy,
    request: &HttpRequest,
    has_trace_context: bool,
    has_tracestate: bool,
) -> Result<String, Refusal> {
    let origin = validate_common(policy, &request.endpoint, request.deadline_ms)?;
    if !valid_identity(&request.request_id) || !valid_identity(&request.idempotency_key) {
        return Err(Refusal::InvalidIdentity);
    }
    if request.body.len() > policy.max_request_bytes {
        return Err(Refusal::RequestTooLarge);
    }
    if request.method == HttpMethod::Get && !request.body.is_empty() {
        return Err(Refusal::InvalidHeader);
    }
    if (has_tracestate && !has_trace_context)
        || request.headers.len()
            + usize::from(request.content_type.is_some())
            + 2
            + usize::from(has_trace_context)
            + usize::from(has_tracestate)
            > MAX_HEADERS
    {
        return Err(Refusal::InvalidHeader);
    }
    if request.content_type.as_ref().is_some_and(|value| {
        value.is_empty() || value.len() > 128 || contains_control(value) || value.contains(' ')
    }) {
        return Err(Refusal::InvalidContentType);
    }
    let mut names = BTreeSet::new();
    if request.headers.iter().any(|header| {
        !names.insert(header.name.as_str())
            || RESERVED_HEADERS.contains(&header.name.as_str())
            || credential_header_name(&header.name)
    }) {
        return Err(Refusal::InvalidHeader);
    }
    Ok(origin)
}

/// Reject both the HTTP-specific credential channels and the shared closed
/// protected-name vocabulary. This keeps caller-selected HTTP headers from
/// becoming a bypass around the export/redaction boundary while preserving the
/// deliberately exact (not substring-based) protected-name policy.
fn credential_header_name(name: &str) -> bool {
    EXPLICIT_CREDENTIAL_HEADERS.contains(&name) || protected_names::is_protected(name)
}

fn into_prepared(
    request: HttpRequest,
    max_response_bytes: usize,
    trace_context: Option<&TraceContext>,
    tracestate: Option<&TraceState>,
) -> PreparedRequest {
    let mut headers = Vec::with_capacity(
        request.headers.len()
            + 3
            + usize::from(trace_context.is_some())
            + usize::from(tracestate.is_some()),
    );
    headers.push(("idempotency-key".into(), request.idempotency_key));
    headers.push(("x-semaprax-delivery-id".into(), request.request_id));
    if let Some(content_type) = request.content_type {
        headers.push(("content-type".into(), content_type));
    }
    if let Some(context) = trace_context {
        headers.push(("traceparent".into(), context.traceparent()));
    }
    if let Some(tracestate) = tracestate {
        headers.push(("tracestate".into(), tracestate.as_header_value().into()));
    }
    headers.extend(
        request
            .headers
            .into_iter()
            .map(|header| (header.name, header.value)),
    );
    PreparedRequest {
        method: request.method,
        endpoint: request.endpoint,
        headers,
        body: request.body,
        deadline_ms: request.deadline_ms,
        max_redirects: 0,
        max_response_bytes,
    }
}

fn session_identity_digest(
    deployment_binding: &str,
    invocation_id: &str,
    idempotency_key: &str,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.outbound.http-session.identity.v1\0");
    for part in [deployment_binding, invocation_id, idempotency_key] {
        digest_part(&mut hash, part.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn policy_digest(policy: &OutboundPolicy) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.outbound.http-session.policy.v1\0");
    digest_part(&mut hash, policy.policy_id.as_bytes());
    hash.update((policy.allowed_origins.len() as u64).to_le_bytes());
    for origin in &policy.allowed_origins {
        digest_part(&mut hash, origin.as_bytes());
    }
    hash.update((policy.max_request_bytes as u64).to_le_bytes());
    hash.update((policy.max_response_bytes as u64).to_le_bytes());
    hash.update(policy.max_deadline_ms.to_le_bytes());
    hash.update((policy.max_export_fields as u64).to_le_bytes());
    hash.update((policy.max_export_labels as u64).to_le_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

#[cfg(test)]
mod tests;
