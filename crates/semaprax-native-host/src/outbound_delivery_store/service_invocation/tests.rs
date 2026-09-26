use super::*;
use semaprax::outbound_host_adapter::{
    export_after_primary, AdapterFailure, AdapterObservation, DeliveryDisposition, ExportEvent,
    ExportField, HttpHeader, HttpMethod, ObservedPrimary, OutboundCapability, OutboundPolicy,
    PreparedRequest, ProtectedExportValue,
};
use semaprax_native_rust_interop_platform as platform;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("canonicalize test temp root");
        for _ in 0..32 {
            let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(
                "semaprax-service-invocation-{}-{nonce}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        }
        panic!("could not allocate a unique test directory")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct RecordingAdapter(Vec<PreparedRequest>);

impl OutboundAdapter for RecordingAdapter {
    fn send(
        &mut self,
        request: &PreparedRequest,
    ) -> semaprax::outbound_host_adapter::AdapterObservation {
        self.0.push(request.clone());
        semaprax::outbound_host_adapter::AdapterObservation::Response {
            status: 202,
            body: Vec::new(),
        }
    }
}

struct PanicOnDispatch;

impl OutboundAdapter for PanicOnDispatch {
    fn send(&mut self, _: &PreparedRequest) -> semaprax::outbound_host_adapter::AdapterObservation {
        panic!("an uncertain provisional checkpoint must never redispatch")
    }
}

fn capability() -> OutboundCapability {
    OutboundCapability::grant_for_trusted_host(
        "sha256:service-invocation-deployment",
        "service-invocation-invocation",
        OutboundPolicy::new(
            "service-invocation-policy",
            ["https://service-invocation.example.test".to_owned()],
            1_024,
            512,
            5_000,
            4,
            3,
        )
        .expect("bounded fixture policy"),
    )
    .expect("trusted fixture authority")
}

fn request() -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Post,
        endpoint: "https://service-invocation.example.test/events".into(),
        request_id: "service-invocation-request".into(),
        idempotency_key: "service-invocation-key".into(),
        content_type: Some("application/json".into()),
        headers: vec![HttpHeader::new("x-service-invocation", "stable").unwrap()],
        body: br#"{"ready":true}"#.to_vec(),
        deadline_ms: 1_000,
    }
}

const STORE_ID: &str = "filesystem-checkpoint-v1";

fn config_bytes(store_id: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":\"{}\",\"store\":\"{store_id}\"}}",
        SERVICE_OUTBOUND_CONFIG_SCHEMA
    )
    .into_bytes()
}

#[test]
fn config_decodes_only_its_one_canonical_spelling() {
    let decoded = ServiceOutboundConfig::decode(&config_bytes(STORE_ID)).unwrap();
    assert_eq!(decoded.store_id(), STORE_ID);

    assert_eq!(
        ServiceOutboundConfig::decode(b""),
        Err(ServiceOutboundConfigRefusal::TooLarge)
    );
    assert_eq!(
        ServiceOutboundConfig::decode(&vec![b'a'; 5_000]),
        Err(ServiceOutboundConfigRefusal::TooLarge)
    );
    assert_eq!(
        ServiceOutboundConfig::decode(b"{\"store\":\"filesystem-checkpoint-v1\"}"),
        Err(ServiceOutboundConfigRefusal::Malformed)
    );
    assert_eq!(
        ServiceOutboundConfig::decode(&config_bytes("../escape")),
        Err(ServiceOutboundConfigRefusal::Malformed)
    );
    assert_eq!(
        ServiceOutboundConfig::decode(b"{\"schema\":\"semaprax.native-host.service-outbound-config.v1\",\"store\":\"filesystem-checkpoint-v1\",\"extra\":1}"),
        Err(ServiceOutboundConfigRefusal::Malformed)
    );
}

/// Configuration alone can never select a store: with no host grant at all,
/// binding is refused before any store exists.
#[test]
fn config_naming_a_store_without_a_host_grant_is_refused() {
    let config = ServiceOutboundConfig::decode(&config_bytes(STORE_ID)).unwrap();
    assert_eq!(
        bind_service_outbound_store(&config, None).err(),
        Some(ServiceOutboundBindingRefusal::NoHostGrant)
    );
}

/// Configuration naming a *different* store than the one the host actually
/// granted is refused too, even though the host does hold some directory.
#[test]
fn config_naming_a_different_store_than_the_host_grant_is_refused() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
    let grant = ServiceOutboundStoreGrant::from_trusted_host(
        "a-different-store-the-host-actually-holds",
        &directory,
        OutboundCheckpointSyncMode::FileOnly,
    );
    let config = ServiceOutboundConfig::decode(&config_bytes(STORE_ID)).unwrap();
    assert_eq!(
        bind_service_outbound_store(&config, Some(&grant)).err(),
        Some(ServiceOutboundBindingRefusal::AuthorityDenied)
    );
}

/// A config naming the exact store the host grants binds to a real store
/// backed by the host-held directory, and it is immediately usable.
#[test]
fn matching_config_and_host_grant_binds_a_real_store() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
    let grant = ServiceOutboundStoreGrant::from_trusted_host(
        STORE_ID,
        &directory,
        OutboundCheckpointSyncMode::FileOnly,
    );
    let config = ServiceOutboundConfig::decode(&config_bytes(STORE_ID)).unwrap();
    let mut store = bind_service_outbound_store(&config, Some(&grant)).unwrap();
    let mut adapter = RecordingAdapter::default();
    let outcome =
        deliver_http_durable(&mut store, 2, None, capability(), request(), &mut adapter).unwrap();
    assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Dispatched(_)));
    assert_eq!(adapter.0.len(), 1);
}

/// A prior terminal reference the host itself durably retained after a
/// completed delivery replays without redispatch, exercised through the
/// production entry point rather than the lower-level session API directly.
#[test]
fn deliver_http_durable_replays_from_a_retained_prior_terminal_reference() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
    let mut store = OutboundDeliveryStore::new(&directory);
    let mut session = HttpDeliverySession::new(2).unwrap();
    let mut adapter = RecordingAdapter::default();
    let prepared = prepare_http_delivery(capability(), request()).unwrap();
    let outcome = session
        .reconcile_durable(prepared, &mut store, &mut adapter)
        .unwrap();
    assert!(matches!(outcome, DurableHttpDeliveryOutcome::Dispatched(_)));
    assert_eq!(adapter.0.len(), 1);
    let checkpoint = session.session_checkpoint().unwrap();
    let digest = checkpoint.digest();
    let capacity = checkpoint.capacity();

    let mut replay_adapter = PanicOnDispatch;
    let replay = deliver_http_durable(
        &mut store,
        capacity,
        Some(PriorTerminalReference {
            digest: &digest,
            capacity,
        }),
        capability(),
        request(),
        &mut replay_adapter,
    )
    .unwrap();
    assert!(
        matches!(replay, ServiceHttpDeliveryOutcome::Replayed(_)),
        "a retained terminal reference must restore and replay, not redispatch"
    );
}

/// The core guard behavior in one process: once the provisional intent for
/// an exact identity and request is durably present, a brand-new session
/// attempting the same call surfaces `Uncertain` and never enters the
/// adapter, even though nothing in that fresh session's own memory recorded
/// the earlier attempt.
#[test]
fn fresh_session_never_redispatches_an_already_committed_provisional_checkpoint() {
    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
    let mut store = OutboundDeliveryStore::new(&directory);
    let mut first_adapter = RecordingAdapter::default();
    let first = deliver_http_durable(
        &mut store,
        3,
        None,
        capability(),
        request(),
        &mut first_adapter,
    )
    .unwrap();
    assert!(matches!(first, ServiceHttpDeliveryOutcome::Dispatched(_)));
    assert_eq!(first_adapter.0.len(), 1);

    // A brand-new session (as a restarted process would construct) attempting
    // the exact same identity and request must not redispatch, and must not
    // even enter the adapter to find out.
    let mut guard_adapter = PanicOnDispatch;
    let second = deliver_http_durable(
        &mut store,
        3,
        None,
        capability(),
        request(),
        &mut guard_adapter,
    )
    .unwrap();
    assert!(matches!(second, ServiceHttpDeliveryOutcome::Uncertain));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// A failing telemetry sink must not change the primary delivery outcome,
/// and the raw secret must never reach either the persisted checkpoint bytes
/// or the telemetry payload bytes -- only its redacted commitment may.
#[test]
fn failing_telemetry_sink_does_not_change_delivery_outcome_and_never_leaks_the_secret() {
    const SECRET: &[u8] = b"top-secret-delivery-marker-9f3e21";

    let temp = TempDirectory::new();
    let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
    let mut store = OutboundDeliveryStore::new(&directory);

    let mut delivery_request = request();
    delivery_request.body = SECRET.to_vec();

    let mut adapter = RecordingAdapter::default();
    let outcome = deliver_http_durable(
        &mut store,
        2,
        None,
        capability(),
        delivery_request,
        &mut adapter,
    )
    .unwrap();
    assert!(matches!(outcome, ServiceHttpDeliveryOutcome::Dispatched(_)));

    // The typed checkpoint store retains only SHA-256 commitments and
    // dispositions; assert that structurally on the real persisted bytes
    // rather than only on the library's documented claim.
    for entry in fs::read_dir(temp.path()).expect("read checkpoint directory") {
        let entry = entry.expect("directory entry");
        if entry.file_type().expect("file type").is_file() {
            let bytes = fs::read(entry.path()).expect("read persisted checkpoint file");
            assert!(
                !contains_bytes(&bytes, SECRET),
                "persisted checkpoint {:?} must never contain the raw secret",
                entry.path()
            );
        }
    }

    struct RecordingFailingAdapter(Vec<u8>);
    impl OutboundAdapter for RecordingFailingAdapter {
        fn send(&mut self, request: &PreparedRequest) -> AdapterObservation {
            self.0 = request.body().to_vec();
            AdapterObservation::FailedAfterStart {
                reason: AdapterFailure::Transport,
            }
        }
    }

    let protected =
        ProtectedExportValue::from_host_bytes(SECRET.to_vec()).expect("bounded protected value");
    let event = ExportEvent {
        stable_event_id: "service-invocation-telemetry-event".into(),
        labels: Vec::new(),
        fields: vec![ExportField {
            name: "delivery-secret".into(),
            value: protected.into_redacted(true),
        }],
    };

    let mut telemetry_adapter = RecordingFailingAdapter(Vec::new());
    let observed: ObservedPrimary<ServiceHttpDeliveryOutcome> = export_after_primary(
        outcome,
        capability(),
        "https://service-invocation.example.test/telemetry".into(),
        1_000,
        event,
        &mut telemetry_adapter,
    );

    assert!(
        matches!(observed.primary, ServiceHttpDeliveryOutcome::Dispatched(_)),
        "a failing telemetry sink must not change the primary delivery outcome"
    );
    let export_result = observed
        .export
        .expect("the telemetry export request itself was independently valid");
    assert!(
        matches!(
            export_result.evidence.disposition(),
            DeliveryDisposition::Uncertain {
                reason: AdapterFailure::Transport
            }
        ),
        "the failing telemetry sink's own outcome is observed separately, not folded into delivery"
    );

    assert!(
        !telemetry_adapter.0.is_empty(),
        "the failing telemetry adapter must still have been entered"
    );
    assert!(
        !contains_bytes(&telemetry_adapter.0, SECRET),
        "telemetry payload bytes must never contain the raw secret"
    );
}
