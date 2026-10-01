use super::*;
use crate::reference_service::test_support::TempDir;
use semaprax_native_rust_interop_platform as platform;
use std::fs;

const KEY: [u8; 32] = [5; 32];
const DEPLOYMENT: &str = "webhook-v2-test";
const ORIGIN: &str = "https://webhook.example";

#[derive(Default)]
struct RecordingAdapter(Vec<PreparedRequest>);
impl OutboundAdapter for RecordingAdapter {
    fn send(&mut self, request: &PreparedRequest) -> AdapterObservation {
        self.0.push(request.clone());
        AdapterObservation::Response {
            status: 202,
            body: Vec::new(),
        }
    }
}
struct PanicOnDispatch;
impl OutboundAdapter for PanicOnDispatch {
    fn send(&mut self, _: &PreparedRequest) -> AdapterObservation {
        panic!("a retained intent must not dispatch");
    }
}

#[test]
fn webhook_v2_actual_body_binds_timestamp_signature_and_intent_facts() {
    let (_temp, held) = TempDir::hold("webhook-v2-envelope");
    let mut store = OutboundDeliveryStore::new(&held);
    let prepared = prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "public", &KEY, 1000).unwrap();
    assert_eq!(prepared.signed_at, 1000);
    assert!(!prepared.existing_body_digest.is_some());
    let expected_body = prepared.request.body.clone();
    let mut adapter = RecordingAdapter::default();
    prepared.deliver(&mut store, &KEY, &mut adapter).unwrap();
    assert_eq!(adapter.0.len(), 1);
    let actual = &adapter.0[0];
    assert_eq!(actual.endpoint(), "https://webhook.example/v2/events");
    assert_eq!(actual.body(), expected_body);
    let root = json::parse(actual.body(), 8192).unwrap();
    let fields = root
        .closed(&[
            "desc",
            "event",
            "job_id",
            "owner",
            "schema",
            "signature",
            "signed_at",
        ])
        .unwrap();
    assert_eq!(
        root.get("schema").and_then(JsonValue::as_str),
        Some(EVENT_SCHEMA_V2)
    );
    assert_eq!(
        root.get("signed_at").and_then(JsonValue::as_i64),
        Some(1000)
    );
    let signature = root.get("signature").unwrap().as_str().unwrap();
    assert!(actual
        .headers()
        .contains(&(EVENT_SCHEMA_HEADER.to_owned(), EVENT_SCHEMA_V2.to_owned())));
    assert!(actual
        .headers()
        .contains(&("x-webhook-signature".to_owned(), signature.to_owned())));
    let mut unsigned: Vec<_> = fields
        .iter()
        .filter(|(name, _)| name != "signature")
        .cloned()
        .collect();
    let mut mac = HmacSha256::new_from_slice(&KEY).unwrap();
    mac.update(json::render(&JsonValue::Object(unsigned.clone())).as_bytes());
    assert_eq!(hex(&mac.finalize().into_bytes()), signature);
    unsigned
        .iter_mut()
        .find(|(name, _)| name == "signed_at")
        .unwrap()
        .1 = JsonValue::Int(1001);
    let mut changed = HmacSha256::new_from_slice(&KEY).unwrap();
    changed.update(json::render(&JsonValue::Object(unsigned)).as_bytes());
    assert_ne!(
        hex(&changed.finalize().into_bytes()),
        signature,
        "timestamp cannot be changed under the original MAC"
    );
    let repeated = prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "public", &KEY, 1300).unwrap();
    assert_eq!(repeated.signed_at, 1000);
    assert_eq!(repeated.now, 1300);
    assert_eq!(repeated.request.body, actual.body());
    assert_eq!(
        repeated.existing_body_digest.as_deref(),
        Some(repeated.candidate_body_digest.as_str())
    );
    assert!(matches!(
        repeated
            .deliver(&mut store, &KEY, &mut PanicOnDispatch)
            .unwrap(),
        WebhookSettlement::Uncertain
    ));
}

#[test]
fn webhook_v2_reopen_legacy_and_tamper_refuse_without_writing() {
    let (temp, held) = TempDir::hold("webhook-v2-prior");
    {
        let mut store = OutboundDeliveryStore::new(&held);
        prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "public", &KEY, 1000)
            .unwrap()
            .deliver(&mut store, &KEY, &mut RecordingAdapter::default())
            .unwrap();
    }
    let reopened = platform::hold_directory(temp.path()).unwrap();
    let store = OutboundDeliveryStore::new(&reopened);
    let repeated = prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "different", &KEY, 1001).unwrap();
    assert_ne!(
        repeated.existing_body_digest.as_deref(),
        Some(repeated.candidate_body_digest.as_str())
    );
    let marker = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "marker")
        })
        .unwrap();
    for bytes in [
        b"semaprax.outbound.pending-intent.v1\n".to_vec(),
        b"tampered\n".to_vec(),
    ] {
        fs::write(&marker, &bytes).unwrap();
        assert!(matches!(
            prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "public", &KEY, 1001),
            Err(DeliveryRefusal::StoreUnavailable)
        ));
        assert_eq!(fs::read(&marker).unwrap(), bytes);
    }
}

#[test]
fn webhook_v2_competing_preparations_share_the_old_no_redispatch_identity() {
    let (_temp, held) = TempDir::hold("webhook-v2-race");
    let mut store = OutboundDeliveryStore::new(&held);
    let first = prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "public", &KEY, 1000).unwrap();
    let second = prepare(&store, DEPLOYMENT, ORIGIN, 7, 1, "public", &KEY, 1001).unwrap();
    let mut adapter = RecordingAdapter::default();
    first.deliver(&mut store, &KEY, &mut adapter).unwrap();
    assert_eq!(adapter.0.len(), 1);
    assert!(matches!(
        second
            .deliver(&mut store, &KEY, &mut PanicOnDispatch)
            .unwrap(),
        WebhookSettlement::Uncertain
    ));
    assert!(matches!(
        deliver_completion_telemetry(
            &mut store,
            DEPLOYMENT,
            ORIGIN,
            ServiceTelemetryAdapter::SemapraxJsonEvents,
            7,
            1,
            "public",
            &KEY,
            &mut PanicOnDispatch
        )
        .unwrap(),
        WebhookSettlement::Uncertain
    ));
}

#[test]
fn webhook_v2_cannot_use_timestamp_free_legacy_delivery_entry() {
    let (temp, held) = TempDir::hold("webhook-v2-no-bypass");
    let mut store = OutboundDeliveryStore::new(&held);
    assert!(matches!(
        deliver_completion_telemetry(
            &mut store,
            DEPLOYMENT,
            ORIGIN,
            ServiceTelemetryAdapter::SemapraxJsonEventsV2,
            7,
            1,
            "public",
            &KEY,
            &mut PanicOnDispatch
        ),
        Err(DeliveryRefusal::InvalidRequest)
    ));
    assert!(matches!(
        completion_event_len(
            ServiceTelemetryAdapter::SemapraxJsonEventsV2,
            7,
            1,
            "public",
            &KEY
        ),
        Err(DeliveryRefusal::InvalidRequest)
    ));
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
}
