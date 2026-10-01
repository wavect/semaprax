//! Opt-in JSON-event v2 preparation. Source admission precedes every write.
use super::*;
use crate::outbound_delivery_store::service_invocation::authenticated_intent::{
    deliver_http_durable_authenticated, http_intent_body_digest, read_http_intent, HttpIntentLookup,
};
use crate::reference_service::decisions::{DecisionEngine, DecisionRefusal};
use semaprax::outbound_host_adapter::prepare_http_delivery;

const EVENT_SCHEMA_V2: &str = "semaprax.json-event.v2";
const EVENTS_PATH_V2: &str = "/v2/events";

/// Retains exactly the body admitted by the source decision and export gate.
/// Reconstructing the envelope after either gate could change its timestamp.
pub(in crate::reference_service) struct PreparedCompletion {
    capability: OutboundCapability,
    request: HttpRequest,
    signature: String,
    signed_at: i64,
    now: i64,
    existing_body_digest: Option<String>,
    candidate_body_digest: String,
}

impl PreparedCompletion {
    pub(in crate::reference_service) fn byte_len(&self) -> usize {
        self.request.body.len()
    }

    pub(in crate::reference_service) fn admitted(
        &self,
        decisions: &DecisionEngine<'_>,
    ) -> Result<bool, DecisionRefusal> {
        decisions.completed_job_webhook_is_admitted(
            self.signature.as_bytes(),
            self.request.body.len() as u64,
            self.signed_at,
            self.now,
            self.existing_body_digest.is_some(),
            self.existing_body_digest
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
            self.candidate_body_digest.as_bytes(),
        )
    }

    pub(in crate::reference_service) fn deliver(
        self,
        store: &mut OutboundDeliveryStore<'_>,
        key: &[u8; 32],
        adapter: &mut impl OutboundAdapter,
    ) -> Result<WebhookSettlement, DeliveryRefusal> {
        let outcome = deliver_http_durable_authenticated(
            store,
            DELIVERY_CAPACITY,
            self.capability,
            self.request,
            self.signed_at,
            key,
            adapter,
        )?;
        Ok(match outcome {
            ServiceHttpDeliveryOutcome::Dispatched(receipt)
            | ServiceHttpDeliveryOutcome::Replayed(receipt) => settle_receipt(&receipt),
            ServiceHttpDeliveryOutcome::Uncertain => WebhookSettlement::Uncertain,
        })
    }
}

pub(in crate::reference_service) fn prepare(
    store: &OutboundDeliveryStore<'_>,
    deployment_binding: &str,
    origin: &str,
    job_id: i64,
    owner: i64,
    desc: &str,
    key: &[u8; 32],
    now: i64,
) -> Result<PreparedCompletion, DeliveryRefusal> {
    if job_id <= 0 || owner <= 0 || now < 0 {
        return Err(DeliveryRefusal::InvalidRequest);
    }
    // Identity depends only on deployment/job/idempotency. This temporary
    // prepared request acquires no filesystem or network effect and cannot
    // dispatch. The actual retained body below reuses a verified prior time.
    let (probe, _) = request(origin, job_id, owner, desc, key, now)?;
    let prepared = prepare_http_delivery(
        completion_capability(deployment_binding, origin, job_id)?,
        probe,
    )?;
    let (signed_at, existing_body_digest) =
        match read_http_intent(store, prepared.pending_identity_key(), key)
            .map_err(|_| DeliveryRefusal::StoreUnavailable)?
        {
            HttpIntentLookup::Absent => (now, None),
            HttpIntentLookup::Authenticated(facts) => {
                (facts.signed_at(), Some(facts.body_digest().to_owned()))
            }
            // A legacy marker has no authenticated prior descriptor or time.
            HttpIntentLookup::LegacyBlocked => return Err(DeliveryRefusal::StoreUnavailable),
        };
    let (request, signature) = request(origin, job_id, owner, desc, key, signed_at)?;
    let candidate_body_digest = http_intent_body_digest(&request.body);
    Ok(PreparedCompletion {
        capability: completion_capability(deployment_binding, origin, job_id)?,
        request,
        signature,
        signed_at,
        now,
        existing_body_digest,
        candidate_body_digest,
    })
}

fn request(
    origin: &str,
    job_id: i64,
    owner: i64,
    desc: &str,
    key: &[u8; 32],
    signed_at: i64,
) -> Result<(HttpRequest, String), DeliveryRefusal> {
    let (body, signature) = signed_event_v2(job_id, owner, desc, key, signed_at);
    Ok((
        HttpRequest {
            method: HttpMethod::Post,
            endpoint: format!("{origin}{EVENTS_PATH_V2}"),
            request_id: format!("req-{job_id}-{owner}"),
            idempotency_key: COMPLETION_IDEMPOTENCY_KEY.to_owned(),
            content_type: Some(WEBHOOK_CONTENT_TYPE.to_owned()),
            headers: vec![
                HttpHeader::new(EVENT_SCHEMA_HEADER, EVENT_SCHEMA_V2)?,
                HttpHeader::new("x-webhook-signature", signature.clone())?,
            ],
            body: body.into_bytes(),
            deadline_ms: DELIVERY_DEADLINE_MS,
        },
        signature,
    ))
}

fn signed_event_v2(
    job_id: i64,
    owner: i64,
    desc: &str,
    key: &[u8; 32],
    signed_at: i64,
) -> (String, String) {
    let mut fields = vec![
        ("desc".to_owned(), JsonValue::Str(desc.to_owned())),
        ("event".to_owned(), JsonValue::Str(WEBHOOK_EVENT.to_owned())),
        ("job_id".to_owned(), JsonValue::Int(job_id)),
        ("owner".to_owned(), JsonValue::Int(owner)),
        (
            "schema".to_owned(),
            JsonValue::Str(EVENT_SCHEMA_V2.to_owned()),
        ),
        ("signed_at".to_owned(), JsonValue::Int(signed_at)),
    ];
    let payload = json::render(&JsonValue::Object(fields.clone()));
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts 32-byte keys");
    mac.update(payload.as_bytes());
    let signature = hex(&mac.finalize().into_bytes());
    fields.push(("signature".to_owned(), JsonValue::Str(signature.clone())));
    fields.sort_by(|left, right| left.0.cmp(&right.0));
    (json::render(&JsonValue::Object(fields)), signature)
}

#[cfg(test)]
mod tests;
