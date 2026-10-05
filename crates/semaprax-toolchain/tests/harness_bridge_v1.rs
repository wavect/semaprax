//! HP-04/10/12/16 evidence: harness host bridged to the compiler SDK.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use semaprax::live_invocation::model_invoke::ModelFailure;
use semaprax::model_budget_policy::{ProviderAdapterFactory, ProviderRefusal};
use semaprax::provider_adapter_sdk::vendor::{
    HostHttpStreamTransport, OpenAiResponsesAdapter, ProviderHttpRequest, TransportFailureKind,
    TransportPoll,
};
use semaprax::provider_adapter_sdk::{
    run_conformance_suite, AdapterEvent, AdapterInvocationCapability, AdapterPoll, AdapterRequest,
    ExpectedOutcome, ProviderAdapter, RequiredCapabilities, StructuredOutputMode,
};
use semaprax_harness::decision::{Destination, FrozenRoutePlan, PlanSlot};
use semaprax_harness::endpoint::catalog::{AttemptOwner, Capabilities, LogicalModel};
use semaprax_harness::endpoint::{ModelIdentity, Protocol};
use semaprax_toolchain::harness_bridge::model::{LogicalModelFactory, ModelBinding};
use semaprax_toolchain::harness_bridge::policy::provider_policy;
use semaprax_toolchain::harness_bridge::transport::{LoopbackTransport, Secret};

fn delta(text: &str) -> String {
    format!(
        "event: response.output_text.delta\ndata: {}\n\n",
        serde_json::json!({"type": "response.output_text.delta", "delta": text})
    )
}

fn completed(text: &str) -> String {
    format!(
        "event: response.completed\ndata: {}\n\n",
        serde_json::json!({"type": "response.completed", "response": {
            "status": "completed",
            "usage": {"input_tokens": 7, "output_tokens": 3},
            "output": [{"type": "message", "content": [{"type": "output_text", "text": text}]}]
        }})
    )
}

#[derive(Default)]
struct Seen {
    requests: Vec<String>,
    peer_closed: bool,
}

struct Fixture {
    port: u16,
    seen: Arc<Mutex<Seen>>,
}

/// Serves every connection with `events` as chunked SSE written in 7-byte
/// slices (arbitrary boundaries). With `hold`, it stops after the first event
/// and waits for the client to close the socket.
fn serve(events: Vec<String>, hold: bool) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let shared = seen.clone();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { return };
            let (events, shared) = (events.clone(), shared.clone());
            std::thread::spawn(move || handle(conn, events, hold, shared));
        }
    });
    Fixture { port, seen }
}

fn handle(mut conn: TcpStream, events: Vec<String>, hold: bool, seen: Arc<Mutex<Seen>>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let (head_end, length) = loop {
        let n = conn.read(&mut tmp).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..i]).to_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            break (i + 4, len);
        }
    };
    while buf.len() < head_end + length {
        let n = conn.read(&mut tmp).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    seen.lock()
        .unwrap()
        .requests
        .push(String::from_utf8_lossy(&buf).into_owned());
    let _ = conn.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
    );
    for (index, event) in events.iter().enumerate() {
        let chunk = format!("{:x}\r\n{event}\r\n", event.len());
        for piece in chunk.as_bytes().chunks(7) {
            if conn.write_all(piece).is_err() {
                seen.lock().unwrap().peer_closed = true;
                return;
            }
            let _ = conn.flush();
        }
        if hold && index == 0 {
            conn.set_read_timeout(Some(Duration::from_secs(10))).ok();
            let closed = matches!(conn.read(&mut tmp), Ok(0) | Err(_));
            seen.lock().unwrap().peer_closed = closed;
            return;
        }
    }
    let _ = conn.write_all(b"0\r\n\r\n");
}

fn logical(id: &str, upstream: &str, protocol: Protocol) -> LogicalModel {
    LogicalModel {
        id: id.into(),
        endpoint_id: "fixture".into(),
        upstream_model: upstream.into(),
        protocol,
        capabilities: Capabilities {
            tools: false,
            structured_output: false,
            streaming: true,
            usage_reporting: true,
        },
        observed_model_identity: ModelIdentity::Unknown,
        observed_returned_model: None,
        observed_catalog_digest: "sha256:none".into(),
        destination: Destination::Local,
        attempt_owner: AttemptOwner::Semaprax,
        max_context: 8192,
        strength_rank: 1,
    }
}

fn adapter(port: u16, secret: Option<&str>) -> OpenAiResponsesAdapter {
    let binding = ModelBinding::from_logical(
        &logical("fast", "fixture-model", Protocol::Responses),
        &format!("http://127.0.0.1:{port}"),
        secret.map(|s| Secret::new(s).unwrap()),
        256,
    )
    .unwrap();
    binding.build().unwrap()
}

fn request() -> AdapterRequest {
    AdapterRequest {
        request_bytes: b"say hello".to_vec(),
        max_response_bytes: 4096,
    }
}

fn cap() -> AdapterInvocationCapability {
    AdapterInvocationCapability::grant("harness-bridge-test")
}

fn required() -> RequiredCapabilities {
    RequiredCapabilities {
        require_streaming: true,
        require_structured_output_mode: Some(StructuredOutputMode::RawText),
        max_request_bytes: 4096,
        max_response_bytes: 4096,
    }
}

/// Polls to a terminal poll, collecting events.
fn drain(a: &mut dyn ProviderAdapter) -> (Vec<AdapterEvent>, AdapterPoll) {
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        assert!(Instant::now() < deadline, "adapter never settled");
        match a.poll() {
            AdapterPoll::Pending => {}
            AdapterPoll::Event(e) => events.push(e),
            other => return (events, other),
        }
    }
}

#[test]
fn loopback_sse_settles_with_text_and_usage_and_sends_bearer() {
    let fx = serve(
        vec![delta("Hello"), delta(" world"), completed("Hello world")],
        false,
    );
    let mut a = adapter(fx.port, Some("tok-secret-123"));
    a.start(&cap(), &request()).unwrap();
    let (events, end) = drain(&mut a);
    let AdapterPoll::Settled(s) = end else {
        panic!("not settled: {end:?}")
    };
    assert_eq!(s.response_bytes, b"Hello world");
    assert_eq!((s.usage.tokens_in, s.usage.tokens_out), (Some(7), Some(3)));
    assert!(events.iter().any(|e| matches!(e, AdapterEvent::Completed)));
    let deltas: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let AdapterEvent::Delta(d) = e {
                Some(d.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(deltas, vec![b"Hello".to_vec(), b" world".to_vec()]);
    let seen = fx.seen.lock().unwrap();
    let req = &seen.requests[0];
    assert!(req.starts_with("POST /v1/responses HTTP/1.1\r\n"));
    assert!(req.contains("Authorization: Bearer tok-secret-123\r\n"));
    assert!(req.contains("\"model\":\"fixture-model\""));
}

#[test]
fn secret_never_appears_in_debug_output() {
    let binding = ModelBinding::from_logical(
        &logical("fast", "m", Protocol::Responses),
        "http://127.0.0.1:1",
        Some(Secret::new("tok-secret-123").unwrap()),
        16,
    )
    .unwrap();
    assert!(!format!("{binding:?}").contains("tok-secret-123"));
    let t = LoopbackTransport::new(
        "http://127.0.0.1:1",
        Some(Secret::new("tok-secret-123").unwrap()),
    );
    assert!(!format!("{t:?}").contains("tok-secret-123"));
}

#[test]
fn cancel_mid_stream_closes_the_socket_and_is_uncertain() {
    let fx = serve(vec![delta("Hel"), delta("lo"), completed("Hello")], true);
    let mut a = adapter(fx.port, None);
    a.start(&cap(), &request()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        assert!(Instant::now() < deadline);
        if let AdapterPoll::Event(AdapterEvent::Delta(d)) = a.poll() {
            assert_eq!(d, b"Hel");
            break;
        }
    }
    a.cancel("test");
    match a.poll() {
        AdapterPoll::Failed { failure, .. } => assert_eq!(failure, ModelFailure::Cancelled),
        other => panic!("expected cancelled failure, got {other:?}"),
    }
    assert_eq!(
        a.attempt_outcome_class(),
        Some(semaprax::model_budget_policy::classification::AttemptOutcomeClass::Uncertain)
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !fx.seen.lock().unwrap().peer_closed {
        assert!(Instant::now() < deadline, "server never saw the close");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn non_loopback_and_non_http_origins_are_refused() {
    for url in [
        "http://10.0.0.5:80",
        "http://example.com:80",
        "https://127.0.0.1:443",
        "http://127.0.0.1",
        "http://user@127.0.0.1:80",
        "http://127.0.0.1.evil.test:80",
    ] {
        assert_eq!(
            LoopbackTransport::new(url, None).unwrap_err().code,
            "SPX-HPL002",
            "{url}"
        );
        assert!(
            ModelBinding::from_logical(&logical("m", "m", Protocol::Responses), url, None, 16)
                .is_err(),
            "{url}"
        );
    }
}

#[test]
fn transport_refuses_authorization_header_and_reports_not_dispatched_when_down() {
    let mut t = LoopbackTransport::new("http://127.0.0.1:9", None).unwrap();
    let req = |headers| ProviderHttpRequest {
        method: "POST",
        path: "/v1/responses",
        headers,
        body: Vec::new(),
        max_response_bytes: 1024,
    };
    let e = t
        .start(req(vec![("authorization", "Bearer x")]))
        .err()
        .unwrap();
    assert_eq!(e.kind, TransportFailureKind::NotDispatched);
    let e = t.start(req(vec![])).err().unwrap();
    assert_eq!(e.kind, TransportFailureKind::NotDispatched);
}

#[test]
fn response_bytes_are_bounded() {
    let big = "x".repeat(4000);
    let fx = serve(vec![delta(&big), delta(&big)], false);
    let mut t = LoopbackTransport::new(&format!("http://127.0.0.1:{}", fx.port), None).unwrap();
    let mut s = t
        .start(ProviderHttpRequest {
            method: "POST",
            path: "/v1/responses",
            headers: vec![],
            body: b"{}".to_vec(),
            max_response_bytes: 5000,
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline);
        match s.poll() {
            TransportPoll::Failed(f) => {
                assert_eq!(f.kind, TransportFailureKind::CapacityExceeded);
                break;
            }
            TransportPoll::End => panic!("unbounded"),
            _ => {}
        }
    }
}

#[test]
fn non_responses_protocol_is_refused_not_downgraded() {
    let m = logical("m", "m", Protocol::ChatCompletions);
    let e = ModelBinding::from_logical(&m, "http://127.0.0.1:1", None, 16).unwrap_err();
    assert_eq!(e.code, "SPX-HPL032");
}

#[test]
fn factory_is_keyed_by_logical_model_id() {
    let mut f = LogicalModelFactory::new();
    f.insert(
        ModelBinding::from_logical(
            &logical("fast", "m", Protocol::Responses),
            "http://127.0.0.1:1",
            None,
            16,
        )
        .unwrap(),
    );
    assert_eq!(
        f.create("fast").unwrap().capabilities().provider_profile,
        "openai-responses"
    );
    assert!(f.create("other").is_err());
}

fn plan() -> FrozenRoutePlan {
    let slot = |id: &str, authorized| PlanSlot {
        model_id: id.into(),
        destination: Destination::Local,
        authorized,
    };
    FrozenRoutePlan {
        lineage_id: "l1".into(),
        ordered: vec![
            slot("primary", true),
            slot("second", true),
            slot("third", false),
        ],
        decision_digest: "d".into(),
        policy_digest: "p".into(),
    }
}

#[test]
fn frozen_plan_converts_to_provider_policy_with_exact_order_and_flags() {
    let policy = provider_policy(&plan());
    assert_eq!(policy.len(), 3);
    let got: Vec<_> = (0..3)
        .map(|i| {
            (
                policy.slot(i).unwrap().id.clone(),
                policy.slot(i).unwrap().authorized,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("primary".into(), true),
            ("second".into(), true),
            ("third".into(), false)
        ]
    );
    assert_eq!(policy.primary().unwrap().id, "primary");
}

#[test]
fn admit_failover_is_forward_only_and_refuses_unauthorized() {
    let policy = provider_policy(&plan());
    assert_eq!(policy.admit_failover(1, "second"), Ok(()));
    assert!(matches!(
        policy.admit_failover(1, "third"),
        Err(ProviderRefusal::OutOfOrder { .. })
    ));
    assert!(matches!(
        policy.admit_failover(2, "third"),
        Err(ProviderRefusal::NotAuthorized { .. })
    ));
    assert!(matches!(
        policy.admit_failover(3, "x"),
        Err(ProviderRefusal::AlternativesExhausted { next_index: 3 })
    ));
    // Backward / same-slot jumps are not the next index's id.
    assert!(matches!(
        policy.admit_failover(1, "primary"),
        Err(ProviderRefusal::OutOfOrder { .. })
    ));
}

#[test]
fn model_generate_conformance_runs_on_the_bridged_adapter() {
    // HP-16: delegated to the existing SDK conformance suite.
    let fx = serve(
        vec![delta("Hello"), delta(" world"), completed("Hello world")],
        false,
    );
    let mut a = adapter(fx.port, None);
    let report = run_conformance_suite(
        &mut a,
        &cap(),
        &required(),
        &request(),
        None,
        &ExpectedOutcome::Settled {
            response_bytes: b"Hello world".to_vec(),
        },
    );
    println!("{}\nreport digest {}", report.render(), report.digest());
    assert!(report.all_passed(), "{}", report.render());

    let hold = serve(vec![delta("Hel"), delta("lo"), completed("Hello")], true);
    let mut b = adapter(hold.port, None);
    let cancelled = run_conformance_suite(
        &mut b,
        &cap(),
        &required(),
        &request(),
        Some(1),
        &ExpectedOutcome::FailedWithClass(ModelFailure::Cancelled),
    );
    println!("{}", cancelled.render());
    assert!(cancelled.all_passed(), "{}", cancelled.render());
}

#[test]
#[ignore = "provisioned: needs ollama serve at http://127.0.0.1:11434 with qwen2.5:0.5b"]
fn real_ollama_streams_one_generation_with_usage() {
    let binding = ModelBinding::from_logical(
        &logical("local-small", "qwen2.5:0.5b", Protocol::Responses),
        "http://127.0.0.1:11434",
        None,
        64,
    )
    .unwrap();
    let mut a = binding.build().unwrap();
    a.start(
        &cap(),
        &AdapterRequest {
            request_bytes: b"Reply with the single word: pong".to_vec(),
            max_response_bytes: 4096,
        },
    )
    .unwrap();
    let (events, end) = drain(&mut a);
    let AdapterPoll::Settled(s) = end else {
        panic!("not settled: {end:?}")
    };
    let text = String::from_utf8(s.response_bytes).unwrap();
    println!(
        "ollama text={text:?} usage={:?} events={}",
        s.usage,
        events.len()
    );
    assert!(!text.is_empty());
    assert!(s.usage.tokens_in.is_some() && s.usage.tokens_out.is_some());
    assert!(events.iter().any(|e| matches!(e, AdapterEvent::Completed)));
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("hpint-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn full_toolchain_harness_status_returns_builtin_fallbacks() {
    let project = temp_dir("proj");
    let home = temp_dir("home");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
        .args(["harness", "status", "--json"])
        .current_dir(&project)
        .env("SEMAPRAX_HARNESS_HOME", &home)
        .env_remove("SEMAPRAX_COMPILER")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    println!(
        "status={:?} stdout={stdout} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(v.is_object());
    assert!(
        stdout.contains("builtin"),
        "expected builtin fallbacks: {stdout}"
    );
}

#[test]
fn full_toolchain_harness_without_verb_is_a_usage_error() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
        .args(["harness"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn standalone_binary_refuses_harness() {
    let bin =
        std::path::Path::new("/Users/kevin/Documents/ChatGPT/AI-Lang-v090/target/debug/semaprax");
    let Some(bin) = std::env::var_os("SEMAPRAX_STANDALONE")
        .map(std::path::PathBuf::from)
        .or_else(|| bin.exists().then(|| bin.to_path_buf()))
    else {
        eprintln!("no standalone semaprax binary available; skipped");
        return;
    };
    let out = std::process::Command::new(bin)
        .args(["harness", "status"])
        .output()
        .unwrap();
    println!(
        "standalone status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(2));
}
