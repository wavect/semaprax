//! Service-configured loopback TLS evidence for every typed telemetry signal.
//!
//! This is deliberately a sibling of `native_tests`: the private test root is
//! repeated rather than making the existing HTTP corpus a mutable test support
//! API. The only authority in this file is an explicitly constructed trusted
//! host policy for one held loopback listener.

use std::io::{Read as _, Write as _};
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::*;

const ROOT: &str = "MIIDJzCCAg+gAwIBAgIUC3kI/KYpwSCFZIOpQLwZZv3fpIUwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBsxGTAXBgNVBAMMEFNFTUFQUkFYIFRlc3QgQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCtxpzwCk3e4aRY3ozKBTi94gfLHe6yKDfDggOHGiwUGotJ9dVH8e4Hh82JamO+jH694HBmjlbGXF+BY7Gxv/Vz8Z7R9VqS1uND7J4V4pJABLL4H//k/c0WPMopTkQRmVyit34hTob14aL+hPq4DFOtH+FxXiUyPaJp6xP0UH7KTJpSBJfBlTAmJoBuMP7Ara05oozrVuLNzSDaUulGGkA5kUuv2GnPvQjTx8PG14GUfJt6okOD64JJSaoQCrraxyHIG8UmZgnHyoIq3UgFY9gj4haVW6ykKe+bkWVbwCOZcMAffzx+NKDodSahn3Qy2z0eDI0ARMtVFDE+ijtxlG/1AgMBAAGjYzBhMB0GA1UdDgQWBBT4Dg/tRse2xlFPUoKfa/7M5c40VjAfBgNVHSMEGDAWgBT4Dg/tRse2xlFPUoKfa/7M5c40VjAPBgNVHRMBAf8EBTADAQH/MA4GA1UdDwEB/wQEAwIBBjANBgkqhkiG9w0BAQsFAAOCAQEAmEWc71S2305pR9Ps29VDVdwOcVoetWsqEnCsAIHg0qfioQz3mznfxE3gOZ4gm03AOslf2sqq8ev02MnEuZWt7Y7xwstrTyo0EA4mWXzBTz0EX7Qp1PgV4MV7Lifp+Dv5ACDx75bgOziKx+u6VVvR0RoE1tUB3m3ihO7aT0HMXOBvElkuY7Ev+fR7lgSFOPGYV2IIBcfaro0dGJlixyBjP/TLGAr8S6buf0ZFCBKtMriXyfiqcQ8IPeLEOtFGxhrWKoNoRpkYwM5kut27vDkoc5UekFmU4EaGPl0cWEpoky5RMXgrA0hAzKEmgPnbIVplKwdoELQjon+MR1HA9txCeg==";
const LEAF: &str = "MIIDSjCCAjKgAwIBAgIUK81c/KylyZTx6OJ/K9lJP7OLzBgwDQYJKoZIhvcNAQELBQAwGzEZMBcGA1UEAwwQU0VNQVBSQVggVGVzdCBDQTAeFw0yNjA5MDUxNDU3MTlaFw0zNjA5MDIxNDU3MTlaMBQxEjAQBgNVBAMMCWxvY2FsaG9zdDCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAM6ibgX7OJCn5nsP0DH497ZCdsxQN23ifpv3ZWWNbKScZi4k5R0nZqJb/asrOa/vgc/An5YBYdsHV/9SqE7CVxhgCj+sYo6W2RfyDV8PF3fztxg+1Varrm0RcI4DaZN2N7fqdxZPvpIl//3n3J2G6J2d919ZPZpog0ahqlHjfvmIh1ESeS2XIu1T4dHlBvW1m3AgoFneNZDHDQs9ziuKte6KShv2I6rOzIRSC5vHM4YsDC64NANbheAV0L98rc/51A6jJxziKQtpFDhBHGvAhag3JkOUyLP7fiIPiHBI0Qxmh70EBj2EgUo5OqV1pNytbH4zBrKlyjQj+R2o8ReNpY8CAwEAAaOBjDCBiTAUBgNVHREEDTALgglsb2NhbGhvc3QwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEwHQYDVR0OBBYEFD69svZnO8+sMQfesN19Zk40CBU8MB8GA1UdIwQYMBaAFPgOD+1Gx7bGUU9Sgp9r/szlzjRWMA0GCSqGSIb3DQEBCwUAA4IBAQAwcYsnw9zK+9lMrIN6zSxry26FFIjOP/ZRXSeloNPA2Fd2p+16b7RoHL+tcn4P4NMCKsz2Y+faX6lzSzIi0lydRsM8rH3xY4/Y8UDoLyC6zDQXpZNbEyWQALgKoZjV8l4XEbtmhLx++h2wArD/eEneBW3aCL8QzNgTU6gyobp1y6AqxQPnl+2SpBlFtpnoz0W3CCOGc0UiaobxBNTYydtY37vGQPLs32drQ2E0o9RfD+4/MTTkS380fXI4pEW4XOm/AofuMwVz1zkWXY/CzYp+1czf7/sOLDTsuwt0/QJFhK3IGSBL1wH3lU8BUHC6LMysilY3Eujo+Ya7dHAyM0lb";
const LEAF_KEY: &str = "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDOom4F+ziQp+Z7D9Ax+Pe2QnbMUDdt4n6b92VljWyknGYuJOUdJ2aiW/2rKzmv74HPwJ+WAWHbB1f/UqhOwlcYYAo/rGKOltkX8g1fDxd387cYPtVWq65tEXCOA2mTdje36ncWT76SJf/959ydhuidnfdfWT2aaINGoapR4375iIdREnktlyLtU+HR5Qb1tZtwIKBZ3jWQxw0LPc4rirXuikob9iOqzsyEUgubxzOGLAwuuDQDW4XgFdC/fK3P+dQOoycc4ikLaRQ4QRxrwIWoNyZDlMiz+34iD4hwSNEMZoe9BAY9hIFKOTqldaTcrWx+Mwaypco0I/kdqPEXjaWPAgMBAAECggEAS9lKyq5HOq4vB8Aru5Q4lXH7Oo89cXwA3o5m7WqG1TvFtC193oA+h919lW3F/KNNgq2hxsXWHjipYAL+3f4vSzbBvFKyUMXlhYknyFt5UWIoNOGnnOtjGQ0cRDzTbbooxL1vnkSCXxJMz+5iyH4jd+vqyFixKLMxcOVZ6Do6OyzuFK2hq1dp2R+fk0TVyQAFTtqSVC5DR/dxzX+mIkkzJWJvfsTnlBZ19j9q8ft0XnOfEpHDSfxzoOXx1SdF+CvA15kjmWVUQbHTMgcPni90NhomPgdlhqXfHx+N+ar3GJO9+GJ8QGhwPXGRGpa81lkQZMTb0Q+rsbqws3Xvl1Nz4QKBgQDtKB7jWevWtakv6k8i6HVe4iGxBwYAHUKe8IrMZt5HQ0gs4iBU6kwZtgW9c02VeHYHnSf/oEF/2OnXpxyQjiHR5LkcZ87lnuivX0bZo8Ijt1dXfczQFZA/zCfpuoTHSQKD8Mw5MbrQ1XrRZaYZMlZ6f0OBPMN8P1657nVwCg3RIQKBgQDfDXj8HqC2blafwwb2dUvKQSH7J4biz7QFl/ZTCJyEu8SSLNJRnKyrIC5mewdJFM3CT9eqIklNkrxbIqd0URy0i512cVIjQmGTtaD0c3S361N9MStlKwsrCtj7Oy4qBdlq/lG03pMubWntRdXnm6e+l+KG6fZ+h+W5y6MEXLWwrwKBgHsfISoXPQEzPqrJklwlIwonjCZD5zGX/0ZUyzpjDXMh0w66Nt7e5LNUdJZujhDTgTNiu6lSoa6mBoEXGRVTNOurOw8sNZWwckzZwgarpda1EHszrGk7SLBWZUJKuzRbCxtEoEHxN3PD4QdlJl5ea9ccywcFbNfMbnlI+183WQUBAoGAVyqBrC0f6wsFiRuC/g9qldiMOgUBXmOC22i+V0aXO/vQ3rrrWf9bLui9mUjc2P9rRVNEWXVaphkAyLCrNfZ4vEmPOHkieyr2zO1+v+japQEuuE7dwYRnseNkVhGTgdKVW42VSpRseglCCvpulDss+3uJh+WocVwUN15QD2VXj3sCgYAyP2FCNPdfg1r2LcNMn06gwnLz+NHn4HK1PNjrRTQgrKYG9xf8gvM0HgoSdR1mfDjdPqgPMdLFG23jmpOG23waokgIsBl88SGdaCVJ/+Ti4WFHhKkhRwgmNX/4se+JsD5nSGaBwkrZ6uyLs+W39hFa0MQzDdRCQjsuuRWFsn7YpA==";

#[derive(Clone, Copy)]
struct SignalExpectation {
    route: &'static str,
    content_type: &'static str,
    identity_header: &'static str,
    body_marker: &'static str,
}

const METRIC: SignalExpectation = SignalExpectation {
    route: "/v1/metrics",
    content_type: "application/vnd.semaprax.metric.v1+json",
    identity_header: "x-semaprax-observation-id: service-metric-1",
    body_marker: "service.requests",
};
const SPAN: SignalExpectation = SignalExpectation {
    route: "/v1/spans",
    content_type: "application/vnd.semaprax.span.v1+json",
    identity_header: "x-semaprax-span-id: 0123456789abcdef",
    body_marker: "service.authorize",
};
const EVENT: SignalExpectation = SignalExpectation {
    route: "/v1/events",
    content_type: "application/json",
    identity_header: "x-semaprax-event-id: service-event-1",
    body_marker: "service.event",
};

fn decode64(input: &str) -> Vec<u8> {
    fn digit(byte: u8) -> u8 {
        match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("test fixture is base64"),
        }
    }

    let mut output = Vec::new();
    let mut bits = 0u32;
    let mut count = 0u8;
    for byte in input.bytes().filter(|byte| *byte != b'=') {
        bits = (bits << 6) | u32::from(digit(byte));
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push((bits >> count) as u8);
            bits &= (1u32 << count) - 1;
        }
    }
    output
}

fn crypto() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

fn trusted_configs() -> (rustls::ClientConfig, rustls::ServerConfig) {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(decode64(ROOT)))
        .expect("test root is a certificate");
    let client = rustls::ClientConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server = rustls::ServerConfig::builder_with_provider(crypto())
        .with_safe_default_protocol_versions()
        .expect("ring has safe protocol versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![rustls::pki_types::CertificateDer::from(decode64(LEAF))],
            rustls::pki_types::PrivatePkcs8KeyDer::from(decode64(LEAF_KEY)).into(),
        )
        .expect("test key matches test certificate");
    (client, server)
}

/// Retain the certificate's localhost identity while pinning this test's only
/// resolution to the held loopback listener.
fn loopback_adapter(tls: rustls::ClientConfig, port: u16) -> NativeHttpsAdapter {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .tls_backend_preconfigured(tls)
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(3)
        .resolve("localhost", SocketAddr::from(([127, 0, 0, 1], port)))
        .build()
        .expect("explicit loopback transport config");
    NativeHttpsAdapter { client }
}

fn read_request(
    stream: &mut rustls::StreamOwned<rustls::ServerConnection, std::net::TcpStream>,
) -> (String, Vec<u8>) {
    let mut headers = Vec::new();
    let mut byte = [0u8; 1];
    while !headers.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .expect("bounded request headers");
        headers.push(byte[0]);
        assert!(headers.len() <= 16_384, "request headers are bounded");
    }
    let headers = String::from_utf8(headers).expect("request headers are ASCII");
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .expect("request has content length")
        .parse::<usize>()
        .expect("content length is decimal");
    let mut body = vec![0; length];
    stream.read_exact(&mut body).expect("request body");
    (headers, body)
}

fn serve_all(
    config: rustls::ServerConfig,
    expected: [SignalExpectation; 3],
) -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback bind");
    let port = listener.local_addr().expect("listener address").port();
    let worker = std::thread::spawn(move || {
        let config = Arc::new(config);
        for signal in expected {
            let (socket, _) = listener.accept().expect("one TLS client per signal");
            socket
                .set_read_timeout(Some(std::time::Duration::from_millis(250)))
                .expect("read timeout");
            let connection = rustls::ServerConnection::new(config.clone()).expect("TLS server");
            let mut stream = rustls::StreamOwned::new(connection, socket);
            let (headers, body) = read_request(&mut stream);
            assert!(
                headers.starts_with(&format!("POST {} HTTP/1.1\r\n", signal.route)),
                "collector signal stays on its fixed route"
            );
            assert!(
                headers
                    .lines()
                    .any(|line| line == format!("content-type: {}", signal.content_type)),
                "typed content type reaches the TLS peer"
            );
            assert!(
                headers.lines().any(|line| line == signal.identity_header),
                "typed signal identity reaches the TLS peer"
            );
            assert!(
                String::from_utf8(body)
                    .expect("canonical JSON")
                    .contains(signal.body_marker),
                "typed canonical body reaches the TLS peer"
            );
            stream
                .write_all(
                    b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("bounded response write");
            stream.flush().expect("response flush");
        }
    });
    (port, worker)
}

fn host_service_configuration(telemetry_origin: &str) -> Vec<u8> {
    let mut value = serde_json::json!({
        "schema": "semaprax.service-config.v1",
        "mode": "host",
        "database": {"adapter":"snapshot","dsn_secret_ref":null,"migration_table":"semaprax_migrations"},
        "http": {"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},
        "secrets": {"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},
        "telemetry": {"adapter":"semaprax-json-events","endpoint_origin":telemetry_origin},
    });
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value).expect("canonical host configuration");
    bytes.push(b'\n');
    bytes
}

fn telemetry_target(configuration: &[u8]) -> TelemetryCollectorTarget {
    let request = crate::project::derive_service_host_adapter_request_v1(configuration)
        .expect("checked host configuration renders an independently valid request");
    TelemetryCollectorTarget::for_trusted_host(
        request
            .telemetry()
            .expect("host request retains telemetry intent")
            .endpoint_origin(),
    )
    .expect("checked host origin is a collector target")
}

fn capability(port: u16, invocation: &str) -> OutboundCapability {
    OutboundCapability::grant_for_trusted_host(
        "sha256:service-native-telemetry",
        invocation,
        OutboundPolicy::new(
            "service.native.telemetry.v1",
            [format!("https://localhost:{port}")],
            4_096,
            512,
            2_000,
            8,
            8,
        )
        .expect("bounded trusted host policy"),
    )
    .expect("trusted host grant")
}

fn metric() -> MetricExport {
    MetricExport {
        stable_metric_id: "service.requests".into(),
        observation_id: "service-metric-1".into(),
        labels: vec![("region".into(), "loopback".into())],
        kind: MetricKind::CounterIncrement(1),
    }
}

fn span() -> SpanExport {
    SpanExport {
        trace_id: "0123456789abcdef0123456789abcdef".into(),
        span_id: "0123456789abcdef".into(),
        parent_span_id: Some("fedcba9876543210".into()),
        name: "service.authorize".into(),
        duration_micros: 42,
        status: SpanStatus::Ok,
        attributes: vec![ExportField {
            name: "region".into(),
            value: ExportFieldValue::Public("loopback".into()),
        }],
    }
}

fn event() -> ExportEvent {
    ExportEvent {
        stable_event_id: "service-event-1".into(),
        labels: vec![("service".into(), "task".into())],
        fields: vec![ExportField {
            name: "kind".into(),
            value: ExportFieldValue::Public("service.event".into()),
        }],
    }
}

#[test]
fn checked_service_host_request_dispatches_all_typed_signals_over_private_tls() {
    let (client, server) = trusted_configs();
    let (port, worker) = serve_all(server, [METRIC, SPAN, EVENT]);
    let origin = format!("https://localhost:{port}");
    let configuration = host_service_configuration(&origin);

    let wrong_port = if port == u16::MAX { port - 1 } else { port + 1 };
    let wrong_policy = OutboundCapability::grant_for_trusted_host(
        "sha256:service-native-telemetry",
        "service-telemetry-wrong-policy",
        OutboundPolicy::new(
            "service.native.telemetry.wrong.v1",
            [format!("https://localhost:{wrong_port}")],
            4_096,
            512,
            2_000,
            8,
            8,
        )
        .expect("individually valid but mismatched policy"),
    )
    .expect("trusted host grant");
    assert!(
        matches!(
            TelemetryCollectorCapability::bind_for_trusted_host(
                wrong_policy,
                telemetry_target(&configuration),
            ),
            Err(CollectorRefusal::AuthorityDenied)
        ),
        "a mismatched host grant refuses before an adapter exists"
    );

    let mut drifted: serde_json::Value = serde_json::from_slice(&configuration).unwrap();
    drifted["telemetry"]["endpoint_origin"] = serde_json::json!("http://localhost:1");
    drifted.sort_all_objects();
    let mut drifted = serde_json::to_vec(&drifted).unwrap();
    drifted.push(b'\n');
    assert!(
        crate::project::derive_service_host_adapter_request_v1(&drifted).is_err(),
        "insecure configuration drift refuses before policy, capability, or adapter work"
    );

    let mut native = loopback_adapter(client, port);
    let mut metric_session = MetricExportSession::new(1).unwrap();
    let metric_receipt = metric_session
        .reconcile(
            TelemetryCollectorCapability::bind_for_trusted_host(
                capability(port, "service-telemetry-metric"),
                telemetry_target(&configuration),
            )
            .unwrap()
            .prepare_metric(2_000, metric())
            .unwrap(),
            &mut native,
        )
        .unwrap();
    assert!(!metric_receipt.was_replayed());

    let mut span_session = SpanExportSession::new(1).unwrap();
    let span_receipt = span_session
        .reconcile(
            TelemetryCollectorCapability::bind_for_trusted_host(
                capability(port, "service-telemetry-span"),
                telemetry_target(&configuration),
            )
            .unwrap()
            .prepare_span(2_000, span())
            .unwrap(),
            &mut native,
        )
        .unwrap();
    assert!(!span_receipt.was_replayed());

    let mut event_session = ExportEventSession::new(1).unwrap();
    let event_receipt = event_session
        .reconcile(
            TelemetryCollectorCapability::bind_for_trusted_host(
                capability(port, "service-telemetry-event"),
                telemetry_target(&configuration),
            )
            .unwrap()
            .prepare_event(2_000, event())
            .unwrap(),
            &mut native,
        )
        .unwrap();
    assert!(!event_receipt.was_replayed());
    worker.join().expect("all three typed TLS requests");

    // The listener is now gone. A regression that re-enters the physical
    // adapter would make this exact replay fail instead of returning a receipt.
    assert!(metric_session
        .reconcile(
            TelemetryCollectorCapability::bind_for_trusted_host(
                capability(port, "service-telemetry-metric"),
                telemetry_target(&configuration),
            )
            .unwrap()
            .prepare_metric(2_000, metric())
            .unwrap(),
            &mut native,
        )
        .unwrap()
        .was_replayed());
    assert!(span_session
        .reconcile(
            TelemetryCollectorCapability::bind_for_trusted_host(
                capability(port, "service-telemetry-span"),
                telemetry_target(&configuration),
            )
            .unwrap()
            .prepare_span(2_000, span())
            .unwrap(),
            &mut native,
        )
        .unwrap()
        .was_replayed());
    assert!(event_session
        .reconcile(
            TelemetryCollectorCapability::bind_for_trusted_host(
                capability(port, "service-telemetry-event"),
                telemetry_target(&configuration),
            )
            .unwrap()
            .prepare_event(2_000, event())
            .unwrap(),
            &mut native,
        )
        .unwrap()
        .was_replayed());
}
