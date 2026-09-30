//! Durable outbound delivery for the reference service.
//!
//! Job completion emits one webhook delivery through R17's production entry
//! point
//! ([`deliver_http_durable`](crate::outbound_delivery_store::service_invocation::deliver_http_durable)),
//! making this module its first real (non-test) caller. The delivery target
//! is the decoded telemetry intent's exact endpoint origin under the fixed
//! `/v1/events` route -- the same closed route
//! [`TelemetryCollectorTarget`](semaprax::outbound_host_adapter::TelemetryCollectorTarget)
//! derives, named here because that route constant has no host accessor.
//! Transport runs over the existing TLS client path
//! ([`TcpNetworkProvider::connect_tls`](semaprax::network_provider::TcpNetworkProvider::connect_tls)),
//! so no new TLS code or dependency is introduced.
//!
//! Identity discipline: one job completes once, so the durable triple is
//! `(deployment_binding, "job-<id>", "completion")`, all stable across
//! restarts. `prior_terminal` is always `None`: R17's identity-keyed marker
//! alone prevents redispatch, so no retained session digest is needed and a
//! crash between commit and response can never duplicate the delivery.

use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use semaprax::network_provider::{NetworkProvider, TcpNetworkProvider};
use semaprax::outbound_host_adapter::{
    AdapterFailure, AdapterObservation, DeliveryDisposition, HttpDeliveryReceipt, HttpHeader,
    HttpMethod, HttpRequest, OutboundAdapter, OutboundCapability, OutboundPolicy, PreparedRequest,
    Refusal,
};
use sha2::Sha256;

use crate::outbound_delivery_store::service_invocation::{
    deliver_http_durable, ServiceHttpDeliveryOutcome, ServiceHttpDeliveryRefusal,
};
use crate::outbound_delivery_store::OutboundDeliveryStore;

use super::json::{self, JsonValue};
use super::state::WebhookSettlement;

type HmacSha256 = Hmac<Sha256>;

/// The fixed telemetry events route. This duplicates the closed
/// `/v1/events` constant owned by `outbound_host_adapter::collector`: the
/// reference host may post telemetry-shaped webhooks only there, never to a
/// caller-selected path.
pub const TELEMETRY_EVENTS_PATH: &str = "/v1/events";
/// The only envelope admitted by the bounded `semaprax-json-events` profile.
///
/// This is a closed event envelope, not an OTLP signal or a caller-defined
/// JSON webhook. The decoded configuration selects the profile and fixed
/// route; it cannot supply another schema or path.
const EVENT_SCHEMA: &str = "semaprax.json-event.v1";
const WEBHOOK_EVENT: &str = "job.completed";
const WEBHOOK_CONTENT_TYPE: &str = "application/json";
const EVENT_SCHEMA_HEADER: &str = "x-semaprax-event-schema";
const DELIVERY_CAPACITY: usize = 64;
const DELIVERY_DEADLINE_MS: u64 = 10_000;
const POLICY_ID: &str = "reference-service-outbound-v1";
const POLICY_REQUEST_BYTES: usize = 8_192;
const POLICY_RESPONSE_BYTES: usize = 8_192;
const POLICY_EXPORT_FIELDS: usize = 4;
const POLICY_EXPORT_LABELS: usize = 4;
const INVOCATION_PREFIX: &str = "job-";
const COMPLETION_IDEMPOTENCY_KEY: &str = "completion";
const MAX_STATUS_LINE_BYTES: usize = 512;
const MAX_RESPONSE_HEADER_BYTES: usize = 16_384;

/// Stable refusal categories for webhook delivery preparation. Transport
/// outcomes are never refusals: they settle into [`WebhookSettlement`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryRefusal {
    InvalidPolicy,
    InvalidRequest,
    StoreUnavailable,
}

impl From<Refusal> for DeliveryRefusal {
    fn from(_: Refusal) -> Self {
        Self::InvalidPolicy
    }
}

impl From<ServiceHttpDeliveryRefusal> for DeliveryRefusal {
    fn from(refusal: ServiceHttpDeliveryRefusal) -> Self {
        match refusal {
            ServiceHttpDeliveryRefusal::InvalidRequest => Self::InvalidRequest,
            _ => Self::StoreUnavailable,
        }
    }
}

/// One HTTPS adapter over the existing provider TLS client. It issues at
/// most the one prepared request: no redirects, no retries, no proxy, and a
/// bounded response read. Anything unexpected after entry is reported as a
/// post-start failure so the ledger settles uncertain, never silently lost.
#[derive(Default)]
pub struct ProviderHttpsAdapter {
    provider: TcpNetworkProvider,
}

impl ProviderHttpsAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl OutboundAdapter for ProviderHttpsAdapter {
    fn send(&mut self, request: &PreparedRequest) -> AdapterObservation {
        if request.max_redirects() != 0 {
            return AdapterObservation::NotDispatched {
                reason: AdapterFailure::PolicyRejected,
            };
        }
        let Some((host, port, path)) = split_endpoint(request.endpoint()) else {
            return AdapterObservation::NotDispatched {
                reason: AdapterFailure::PolicyRejected,
            };
        };
        // Honor the prepared deadline on this attempt only; the provider is
        // restored to its default policy afterwards.
        let provider = std::mem::take(&mut self.provider).with_deadline_policy(
            semaprax::network_provider::deadline::DeadlinePolicy::new(Duration::from_millis(
                request.deadline_ms(),
            )),
        );
        self.provider = provider;
        let outcome = self.send_once(&host, port, &path, request);
        let provider = std::mem::take(&mut self.provider)
            .with_deadline_policy(semaprax::network_provider::deadline::DeadlinePolicy::default());
        self.provider = provider;
        outcome
    }
}

impl ProviderHttpsAdapter {
    fn send_once(
        &mut self,
        host: &str,
        port: u16,
        path: &str,
        request: &PreparedRequest,
    ) -> AdapterObservation {
        let connection = match self.provider.connect_tls(host, port) {
            Ok(connection) => connection,
            Err(_) => {
                return AdapterObservation::FailedAfterStart {
                    reason: AdapterFailure::Tls,
                };
            }
        };
        let result = self.exchange(connection, host, path, request);
        let _ = self.provider.close(connection);
        result
    }

    fn exchange(
        &mut self,
        connection: semaprax::network_provider::ProviderConnection,
        host: &str,
        path: &str,
        request: &PreparedRequest,
    ) -> AdapterObservation {
        let method = match request.method() {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
        };
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n",
            request.body().len()
        );
        for (name, value) in request.headers() {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        let mut wire = head.into_bytes();
        wire.extend_from_slice(request.body());
        if self.provider.send(connection, &wire).is_err() {
            return AdapterObservation::FailedAfterStart {
                reason: AdapterFailure::Transport,
            };
        }
        let mut received = Vec::new();
        let limit = request
            .max_response_bytes()
            .saturating_add(MAX_RESPONSE_HEADER_BYTES)
            .saturating_add(1);
        loop {
            match self.provider.recv(connection, 8_192) {
                Ok(chunk) if chunk.is_empty() => break,
                Ok(chunk) => {
                    received.extend_from_slice(&chunk);
                    if received.len() > limit {
                        return AdapterObservation::ResponseTooLargeAfterStart;
                    }
                    if response_complete(&received) {
                        break;
                    }
                }
                Err(_) => {
                    return AdapterObservation::FailedAfterStart {
                        reason: AdapterFailure::Transport,
                    };
                }
            }
        }
        split_response(&received, request.max_response_bytes())
    }
}

fn split_endpoint(endpoint: &str) -> Option<(String, u16, String)> {
    let remainder = endpoint.strip_prefix("https://")?;
    let path_start = remainder.find('/').unwrap_or(remainder.len());
    let (authority, path) = remainder.split_at(path_start);
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            let port = port.parse::<u16>().ok()?;
            if port == 0 {
                return None;
            }
            (host.to_owned(), port)
        }
        Some(_) => return None,
        None => (authority.to_owned(), 443),
    };
    if host.is_empty() || host.len() > 253 || !host.is_ascii() {
        return None;
    }
    let path = if path.is_empty() { "/" } else { path };
    if path.len() > 2_048 || !path.is_ascii() {
        return None;
    }
    Some((host, port, path.to_owned()))
}

fn response_complete(received: &[u8]) -> bool {
    let Some(end) = find_header_end(received) else {
        return false;
    };
    let Some(length) = content_length(&received[..end]) else {
        return false;
    };
    received.len() >= end + length
}

fn find_header_end(received: &[u8]) -> Option<usize> {
    received
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|position| position + 4)
}

fn content_length(head: &[u8]) -> Option<usize> {
    let text = std::str::from_utf8(head).ok()?;
    for line in text.split("\r\n").skip(1) {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse::<usize>().ok();
        }
    }
    None
}

fn split_response(received: &[u8], max_body: usize) -> AdapterObservation {
    let Some(end) = find_header_end(received) else {
        return AdapterObservation::FailedAfterStart {
            reason: AdapterFailure::Protocol,
        };
    };
    if end > MAX_RESPONSE_HEADER_BYTES {
        return AdapterObservation::ResponseTooLargeAfterStart;
    }
    let head = &received[..end];
    let status_line = head
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|position| &head[..position])
        .unwrap_or(head);
    if status_line.len() > MAX_STATUS_LINE_BYTES {
        return AdapterObservation::FailedAfterStart {
            reason: AdapterFailure::Protocol,
        };
    }
    let status = std::str::from_utf8(status_line)
        .ok()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .filter(|code| (100..600).contains(code));
    let Some(status) = status else {
        return AdapterObservation::FailedAfterStart {
            reason: AdapterFailure::Protocol,
        };
    };
    let mut body = received[end..].to_vec();
    if let Some(length) = content_length(head) {
        if length > max_body {
            return AdapterObservation::ResponseTooLargeAfterStart;
        }
        body.truncate(length);
    }
    if body.len() > max_body {
        return AdapterObservation::ResponseTooLargeAfterStart;
    }
    AdapterObservation::Response { status, body }
}

/// Deliver one job-completion webhook through the durable production path.
///
/// `deployment_binding` is host operator configuration (never decoded
/// intent); `endpoint_origin` must be the exact decoded telemetry origin the
/// host policy grants. The delivery identity is stable per job, so a second
/// call for the same job -- including after a process restart -- reconciles
/// against the durable marker instead of redispatching.
#[allow(clippy::too_many_arguments)]
pub fn deliver_completion_webhook(
    store: &mut OutboundDeliveryStore<'_>,
    deployment_binding: &str,
    endpoint_origin: &str,
    job_id: i64,
    owner: i64,
    desc: &str,
    webhook_key: &[u8; 32],
    adapter: &mut impl OutboundAdapter,
) -> Result<WebhookSettlement, DeliveryRefusal> {
    if job_id <= 0 || owner <= 0 {
        return Err(DeliveryRefusal::InvalidRequest);
    }
    let policy = OutboundPolicy::new(
        POLICY_ID,
        [endpoint_origin.to_owned()],
        POLICY_REQUEST_BYTES,
        POLICY_RESPONSE_BYTES,
        DELIVERY_DEADLINE_MS,
        POLICY_EXPORT_FIELDS,
        POLICY_EXPORT_LABELS,
    )?;
    let invocation_id = format!("{INVOCATION_PREFIX}{job_id}");
    let capability =
        OutboundCapability::grant_for_trusted_host(deployment_binding, invocation_id, policy)?;
    let (body, signature) = signed_event(job_id, owner, desc, webhook_key);
    let request = HttpRequest {
        method: HttpMethod::Post,
        endpoint: format!("{endpoint_origin}{TELEMETRY_EVENTS_PATH}"),
        request_id: format!("req-{job_id}-{owner}"),
        idempotency_key: COMPLETION_IDEMPOTENCY_KEY.to_owned(),
        content_type: Some(WEBHOOK_CONTENT_TYPE.to_owned()),
        headers: vec![
            HttpHeader::new(EVENT_SCHEMA_HEADER, EVENT_SCHEMA)
                .map_err(|_| DeliveryRefusal::InvalidRequest)?,
            HttpHeader::new("x-webhook-signature", signature)
                .map_err(|_| DeliveryRefusal::InvalidRequest)?,
        ],
        body: body.into_bytes(),
        deadline_ms: DELIVERY_DEADLINE_MS,
    };
    let outcome =
        deliver_http_durable(store, DELIVERY_CAPACITY, None, capability, request, adapter)?;
    Ok(match outcome {
        ServiceHttpDeliveryOutcome::Dispatched(receipt)
        | ServiceHttpDeliveryOutcome::Replayed(receipt) => settle_receipt(&receipt),
        ServiceHttpDeliveryOutcome::Uncertain => WebhookSettlement::Uncertain,
    })
}

fn signed_event(job_id: i64, owner: i64, desc: &str, webhook_key: &[u8; 32]) -> (String, String) {
    let payload = json::render(&JsonValue::Object(vec![
        ("desc".to_owned(), JsonValue::Str(desc.to_owned())),
        ("event".to_owned(), JsonValue::Str(WEBHOOK_EVENT.to_owned())),
        ("job_id".to_owned(), JsonValue::Int(job_id)),
        ("owner".to_owned(), JsonValue::Int(owner)),
        ("schema".to_owned(), JsonValue::Str(EVENT_SCHEMA.to_owned())),
    ]));
    let mut mac = HmacSha256::new_from_slice(webhook_key).expect("HMAC accepts 32-byte keys");
    mac.update(payload.as_bytes());
    let tag = hex(mac.finalize().into_bytes().as_slice());
    let body = json::render(&JsonValue::Object(vec![
        ("desc".to_owned(), JsonValue::Str(desc.to_owned())),
        ("event".to_owned(), JsonValue::Str(WEBHOOK_EVENT.to_owned())),
        ("job_id".to_owned(), JsonValue::Int(job_id)),
        ("owner".to_owned(), JsonValue::Int(owner)),
        ("schema".to_owned(), JsonValue::Str(EVENT_SCHEMA.to_owned())),
        ("signature".to_owned(), JsonValue::Str(tag.clone())),
    ]));
    (body, tag)
}

fn settle_receipt(receipt: &HttpDeliveryReceipt) -> WebhookSettlement {
    match receipt.evidence().disposition() {
        DeliveryDisposition::Accepted { .. } => WebhookSettlement::Delivered {
            evidence_digest: evidence_digest(receipt),
        },
        DeliveryDisposition::Rejected { .. } | DeliveryDisposition::NotDispatched { .. } => {
            WebhookSettlement::Failed
        }
        DeliveryDisposition::Uncertain { .. }
        | DeliveryDisposition::DeadlineUncertain
        | DeliveryDisposition::ResponseTooLargeUncertain => WebhookSettlement::Uncertain,
    }
}

fn evidence_digest(receipt: &HttpDeliveryReceipt) -> String {
    super::content_digest(receipt.evidence().render().as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_split_defensively() {
        assert_eq!(
            split_endpoint("https://collector.example/v1/events"),
            Some(("collector.example".to_owned(), 443, "/v1/events".to_owned()))
        );
        assert_eq!(
            split_endpoint("https://127.0.0.1:8443/hook"),
            Some(("127.0.0.1".to_owned(), 8443, "/hook".to_owned()))
        );
        assert_eq!(split_endpoint("http://plain.example/"), None);
        assert_eq!(split_endpoint("https://"), None);
        assert_eq!(split_endpoint("https://host:0/"), None);
        assert_eq!(split_endpoint("https://user@host/"), None);
        assert_eq!(split_endpoint("https://host:99999/"), None);
        assert_eq!(split_endpoint("https://a:b:c/"), None);
    }

    #[test]
    fn responses_split_with_bounds() {
        let ok = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi";
        assert!(matches!(
            split_response(ok, 8_192),
            AdapterObservation::Response { status: 200, .. }
        ));
        let no_length = b"HTTP/1.1 204 Done\r\n\r\n";
        assert!(matches!(
            split_response(no_length, 8_192),
            AdapterObservation::Response { status: 204, .. }
        ));
        let oversized = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nhi!!";
        assert_eq!(
            split_response(oversized, 2),
            AdapterObservation::ResponseTooLargeAfterStart
        );
        let garbage = b"not a response";
        assert_eq!(
            split_response(garbage, 8_192),
            AdapterObservation::FailedAfterStart {
                reason: AdapterFailure::Protocol
            }
        );
        assert_eq!(
            split_response(b"HTTP/1.1 99 X\r\n\r\n", 8_192),
            AdapterObservation::FailedAfterStart {
                reason: AdapterFailure::Protocol
            }
        );
    }

    #[test]
    fn completion_event_is_schema_bound_signed_and_canonical() {
        let (body, signature) = signed_event(7, 1, "task-1", &[5_u8; 32]);
        assert_eq!(EVENT_SCHEMA_HEADER, "x-semaprax-event-schema");
        assert_eq!(
            super::EVENT_SCHEMA,
            "semaprax.json-event.v1",
            "the accepted configuration profile has one named envelope"
        );
        assert!(body.contains("\"schema\":\"semaprax.json-event.v1\""));
        assert!(body.contains("\"event\":\"job.completed\""));
        assert!(body.contains("\"signature\":\""));
        assert_eq!(signature.len(), 64);
        // The signature covers the exact unsigned payload rendering.
        let payload = json::render(&JsonValue::Object(vec![
            ("desc".to_owned(), JsonValue::Str("task-1".to_owned())),
            ("event".to_owned(), JsonValue::Str(WEBHOOK_EVENT.to_owned())),
            ("job_id".to_owned(), JsonValue::Int(7)),
            ("owner".to_owned(), JsonValue::Int(1)),
            ("schema".to_owned(), JsonValue::Str(EVENT_SCHEMA.to_owned())),
        ]));
        let mut mac = HmacSha256::new_from_slice(&[5_u8; 32]).unwrap();
        mac.update(payload.as_bytes());
        assert_eq!(hex(mac.finalize().into_bytes().as_slice()), signature);
        let reparsed = super::super::json::parse(body.as_bytes(), 4_096).unwrap();
        assert_eq!(
            reparsed.get("signature").unwrap().as_str(),
            Some(signature.as_str())
        );
    }
}
